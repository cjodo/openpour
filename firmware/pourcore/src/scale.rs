//! Load-cell signal processing: tare, calibration, smoothing and flow rate.
//! The HX711 driver feeds raw samples in; nothing here touches hardware.

const AVG_SAMPLES: i64 = 16;
const HIST: usize = 40; // 40 x 50 ms = 2 s
const PUSH_MS: u32 = 50;
const FLOW_WINDOW_MS: u32 = 1000;
const CONNECTED_MS: u32 = 500;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Op {
    None,
    Tare,
    Calibrate(f32),
}

#[derive(Clone, Copy, Default)]
struct Sample {
    ms: u32,
    g: f32,
}

pub struct ScaleFilter {
    offset: i32,
    filtered: f32,
    last_sample_ms: Option<u32>,
    op: Op,
    acc: i64,
    acc_n: i64,
    // Decimated history for flow-rate estimation.
    hist: [Sample; HIST],
    head: usize,
    count: usize,
    last_push_ms: u32,
}

impl Default for ScaleFilter {
    fn default() -> Self {
        ScaleFilter {
            offset: 0,
            filtered: 0.0,
            last_sample_ms: None,
            op: Op::None,
            acc: 0,
            acc_n: 0,
            hist: [Sample::default(); HIST],
            head: 0,
            count: 0,
            last_push_ms: 0,
        }
    }
}

impl ScaleFilter {
    /// Feed one raw HX711 reading. Returns a new counts-per-gram factor when a
    /// calibration finishes; the caller stores it in the settings.
    pub fn on_sample(&mut self, raw: i32, now: u32, counts_per_gram: f32) -> Option<f32> {
        self.last_sample_ms = Some(now);

        if self.op != Op::None {
            self.acc += raw as i64;
            self.acc_n += 1;
            if self.acc_n < AVG_SAMPLES {
                return None;
            }
            let avg = (self.acc / self.acc_n) as i32;
            let op = core::mem::replace(&mut self.op, Op::None);
            match op {
                Op::Tare => {
                    self.offset = avg;
                    self.filtered = 0.0;
                    self.head = 0;
                    self.count = 0;
                }
                Op::Calibrate(grams) if grams > 0.0 => {
                    let f = (avg - self.offset) as f32 / grams;
                    if f.abs() > 1.0 {
                        return Some(f);
                    }
                }
                _ => {}
            }
            return None;
        }

        let g = (raw - self.offset) as f32 / counts_per_gram;
        self.filtered += 0.35 * (g - self.filtered);
        if now.wrapping_sub(self.last_push_ms) >= PUSH_MS {
            self.last_push_ms = now;
            self.hist[self.head] = Sample { ms: now, g: self.filtered };
            self.head = (self.head + 1) % HIST;
            if self.count < HIST {
                self.count += 1;
            }
        }
        None
    }

    /// Filtered weight since the last tare.
    pub fn grams(&self) -> f32 {
        self.filtered
    }

    fn back(&self, i: usize) -> Sample {
        self.hist[(self.head + HIST - i) % HIST]
    }

    fn sample_ago(&self, age_ms: u32) -> Option<Sample> {
        if self.count < 2 {
            return None;
        }
        let newest = self.back(1);
        (2..=self.count)
            .map(|i| self.back(i))
            .find(|s| newest.ms.wrapping_sub(s.ms) >= age_ms)
    }

    /// Enough history for `flow_gps()` to mean something.
    pub fn flow_valid(&self) -> bool {
        self.sample_ago(FLOW_WINDOW_MS).is_some()
    }

    /// Grams/second over the last ~1 s.
    pub fn flow_gps(&self) -> f32 {
        match self.sample_ago(FLOW_WINDOW_MS) {
            Some(old) => {
                let newest = self.back(1);
                (newest.g - old.g) * 1000.0 / newest.ms.wrapping_sub(old.ms) as f32
            }
            None => 0.0,
        }
    }

    /// A sample arrived recently.
    pub fn connected(&self, now: u32) -> bool {
        self.last_sample_ms.is_some_and(|t| now.wrapping_sub(t) < CONNECTED_MS)
    }

    /// Asynchronous: averages the next samples. See `busy()`.
    pub fn tare(&mut self) {
        self.start(Op::Tare);
    }

    /// Asynchronous: the known weight must be on the platform.
    pub fn calibrate(&mut self, known_grams: f32) {
        self.start(Op::Calibrate(known_grams));
    }

    fn start(&mut self, op: Op) {
        self.op = op;
        self.acc = 0;
        self.acc_n = 0;
    }

    pub fn busy(&self) -> bool {
        self.op != Op::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPG: f32 = 400.0;

    fn feed(s: &mut ScaleFilter, raw: i32, from_ms: u32, n: u32) -> u32 {
        for i in 0..n {
            s.on_sample(raw, from_ms + i * 12, CPG);
        }
        from_ms + n * 12
    }

    #[test]
    fn tare_then_weigh() {
        let mut s = ScaleFilter::default();
        s.tare();
        let t = feed(&mut s, 10_000, 0, 16);
        assert!(!s.busy());
        feed(&mut s, 10_000 + 40 * 400, t, 60);
        assert!((s.grams() - 40.0).abs() < 0.01);
    }

    #[test]
    fn calibrate_returns_factor() {
        let mut s = ScaleFilter::default();
        s.tare();
        feed(&mut s, 1000, 0, 16);
        s.calibrate(100.0);
        let mut got = None;
        for i in 0..16 {
            got = got.or(s.on_sample(1000 + 42_000, 500 + i, CPG));
        }
        assert_eq!(got, Some(420.0));
    }

    #[test]
    fn flow_rate_from_history() {
        let mut s = ScaleFilter::default();
        // 5 g/s ramp, one sample every 10 ms.
        for i in 0..300u32 {
            let g = 5.0 * i as f32 * 0.01;
            s.on_sample((g * CPG) as i32, i * 10, CPG);
        }
        assert!(s.flow_valid());
        assert!((s.flow_gps() - 5.0).abs() < 0.2, "{}", s.flow_gps());
    }

    #[test]
    fn disconnected_without_samples() {
        let mut s = ScaleFilter::default();
        assert!(!s.connected(0));
        s.on_sample(0, 1000, CPG);
        assert!(s.connected(1200));
        assert!(!s.connected(1600));
    }
}
