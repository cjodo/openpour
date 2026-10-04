//! A fixed-size ring of recent log lines. The firmware's logger pushes every
//! record here; the app reads new lines live and backfills from it when it
//! connects.

use std::collections::VecDeque;

use log::Level;
use serde_json::{json, Value};

/// Longer messages are cut, so one chatty line can't eat the buffer.
pub const MAX_MSG: usize = 160;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Increases by one per line, starting at 1, so readers can tell what they missed.
    pub seq: u32,
    /// Milliseconds since boot.
    pub ms: u32,
    pub level: Level,
    pub target: String,
    pub msg: String,
}

impl Entry {
    pub fn to_json(&self) -> Value {
        json!({
            "seq": self.seq,
            "ms": self.ms,
            "level": level_name(self.level),
            "target": self.target,
            "msg": self.msg,
        })
    }
}

pub fn level_name(l: Level) -> &'static str {
    match l {
        Level::Error => "error",
        Level::Warn => "warn",
        Level::Info => "info",
        Level::Debug => "debug",
        Level::Trace => "trace",
    }
}

fn truncate(mut s: String) -> String {
    if s.len() > MAX_MSG {
        let mut cut = MAX_MSG - 1;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push('…');
    }
    s
}

pub struct LogRing {
    entries: VecDeque<Entry>,
    capacity: usize,
    next_seq: u32,
}

impl LogRing {
    pub fn new(capacity: usize) -> Self {
        LogRing { entries: VecDeque::with_capacity(capacity), capacity: capacity.max(1), next_seq: 1 }
    }

    /// Stores a line, overwriting the oldest when full. Returns its sequence number.
    pub fn push(&mut self, ms: u32, level: Level, target: &str, msg: String) -> u32 {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1).max(1);
        self.entries.push_back(Entry { seq, ms, level, target: target.to_owned(), msg: truncate(msg) });
        seq
    }

    /// Sequence number of the newest line, or 0 if empty.
    pub fn last_seq(&self) -> u32 {
        self.entries.back().map_or(0, |e| e.seq)
    }

    /// Lines newer than `after` (0 for everything), and how many were
    /// overwritten before the reader got to them.
    pub fn since(&self, after: u32) -> (Vec<&Entry>, u32) {
        let Some(oldest) = self.entries.front() else { return (Vec::new(), 0) };
        let dropped = oldest.seq.saturating_sub(after.saturating_add(1));
        (self.entries.iter().filter(|e| e.seq > after).collect(), if after == 0 { 0 } else { dropped })
    }

    /// `{"entries": [...], "dropped": n}` for the app.
    pub fn since_json(&self, after: u32) -> Value {
        let (entries, dropped) = self.since(after);
        json!({ "entries": entries.iter().map(|e| e.to_json()).collect::<Vec<_>>(), "dropped": dropped })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_newest_lines() {
        let mut r = LogRing::new(3);
        for i in 0..5 {
            r.push(i * 10, Level::Info, "t", format!("line {i}"));
        }
        let (all, dropped) = r.since(0);
        assert_eq!(dropped, 0);
        assert_eq!(all.iter().map(|e| e.seq).collect::<Vec<_>>(), [3, 4, 5]);
        assert_eq!(r.last_seq(), 5);
    }

    #[test]
    fn reports_lines_a_reader_missed() {
        let mut r = LogRing::new(3);
        for i in 0..6 {
            r.push(i, Level::Warn, "t", "x".into());
        }
        // The reader last saw seq 1; 2 and 3 were overwritten.
        let (new, dropped) = r.since(1);
        assert_eq!(new.len(), 3);
        assert_eq!(dropped, 2);
        assert_eq!(r.since(6).0.len(), 0);
    }

    #[test]
    fn long_messages_are_cut() {
        let mut r = LogRing::new(2);
        r.push(0, Level::Info, "t", "é".repeat(200));
        let e = &r.since(0).0[0];
        assert!(e.msg.len() <= MAX_MSG + '…'.len_utf8());
        assert!(e.msg.ends_with('…'));
        assert_eq!(e.to_json()["level"], "info");
    }
}
