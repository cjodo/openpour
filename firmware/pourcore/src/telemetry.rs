//! Records a numeric trace of every brew and calibration: one run per brew,
//! sampled at `Settings::telemetry_hz`. The firmware streams samples to the
//! app and writes each run to a CSV file.

use serde_json::{json, Value};

use crate::brew::{Brew, State};
use crate::machine::Machine;

/// CSV columns, also the order of each sample's JSON array.
pub const COLUMNS: [&str; 12] = [
    "t_s", "state", "stage", "poured_g", "flow_gps", "target_g", "target_flow_gps", "duty", "pulses", "temp_c",
    "theta_deg", "r_mm",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// Seconds since the run started.
    pub t_s: f32,
    pub state: &'static str,
    /// 1-based stage number, 0 outside a recipe.
    pub stage: usize,
    pub poured_g: f32,
    pub flow_gps: f32,
    pub target_g: f32,
    pub target_flow_gps: f32,
    pub duty: f32,
    pub pulses: u32,
    pub temp_c: Option<f32>,
    pub theta_deg: f32,
    pub r_mm: f32,
}

impl Sample {
    pub fn to_csv(&self) -> String {
        let temp = self.temp_c.map_or(String::new(), |t| format!("{t:.2}"));
        format!(
            "{:.2},{},{},{:.2},{:.3},{:.1},{:.2},{:.3},{},{},{:.2},{:.2}\n",
            self.t_s, self.state, self.stage, self.poured_g, self.flow_gps, self.target_g, self.target_flow_gps,
            self.duty, self.pulses, temp, self.theta_deg, self.r_mm
        )
    }

    /// Compact form for the WebSocket, in `COLUMNS` order.
    pub fn to_json(&self) -> Value {
        json!([
            round(self.t_s, 2), self.state, self.stage, round(self.poured_g, 2), round(self.flow_gps, 3),
            round(self.target_g, 1), round(self.target_flow_gps, 2), round(self.duty, 3), self.pulses,
            self.temp_c.map(|t| round(t, 2)), round(self.theta_deg, 2), round(self.r_mm, 2)
        ])
    }
}

fn round(v: f32, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (v as f64 * k).round() / k
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A run began. `header` is the CSV preamble: `#` comment lines, then the column row.
    Start { run: u32, kind: &'static str, header: String },
    Sample { run: u32, sample: Sample },
    /// The run ended in `state` ("done", "idle" or "error").
    End { run: u32, state: &'static str },
}

fn recording(s: State) -> bool {
    !matches!(s, State::Idle | State::Done | State::Error)
}

pub struct Recorder {
    firmware: String,
    next_run: u32,
    run: Option<u32>,
    start_ms: u32,
    next_sample_ms: u32,
}

impl Recorder {
    /// `first_run` continues the numbering of runs already stored.
    pub fn new(firmware: &str, first_run: u32) -> Self {
        Recorder { firmware: firmware.to_owned(), next_run: first_run.max(1), run: None, start_ms: 0, next_sample_ms: 0 }
    }

    pub fn current_run(&self) -> Option<u32> {
        self.run
    }

    /// Call every loop after `Brew::update`.
    pub fn update(&mut self, brew: &Brew, m: &impl Machine) -> Vec<Event> {
        let mut out = Vec::new();
        let now = m.now_ms();
        let active = recording(brew.state());

        if active && self.run.is_none() {
            let run = self.next_run;
            self.next_run += 1;
            self.run = Some(run);
            self.start_ms = now;
            self.next_sample_ms = now;
            let kind = if brew.state() == State::Calibrating { "calibration" } else { "brew" };
            out.push(Event::Start { run, kind, header: self.header(run, kind, brew, m) });
        }
        let Some(run) = self.run else { return out };

        let period = (1000.0 / m.settings().telemetry_hz.clamp(1.0, 50.0)) as u32;
        if !active || now.wrapping_sub(self.next_sample_ms) as i32 >= 0 {
            out.push(Event::Sample { run, sample: self.sample(brew, m) });
            self.next_sample_ms = self.next_sample_ms.wrapping_add(period);
            // Fell far behind (a stalled loop): resync instead of bursting.
            if now.wrapping_sub(self.next_sample_ms) as i32 > period as i32 {
                self.next_sample_ms = now.wrapping_add(period);
            }
        }
        if !active {
            out.push(Event::End { run, state: brew.state().name() });
            self.run = None;
        }
        out
    }

    fn sample(&self, brew: &Brew, m: &impl Machine) -> Sample {
        let (theta_deg, r_mm) = m.arm_pose();
        Sample {
            t_s: m.now_ms().wrapping_sub(self.start_ms) as f32 / 1000.0,
            state: brew.state().name(),
            stage: brew.stage_number(),
            poured_g: m.grams(),
            flow_gps: m.flow_gps(),
            target_g: brew.stage_target(),
            target_flow_gps: brew.target_flow(),
            duty: m.pump_duty(),
            pulses: m.meter_pulses(),
            temp_c: m.temp_c(),
            theta_deg,
            r_mm,
        }
    }

    fn header(&self, run: u32, kind: &str, brew: &Brew, m: &impl Machine) -> String {
        let s = m.settings();
        let settings = json!({
            "flowPulsesPerLitre": s.flow_pulses_per_litre,
            "pumpGpsAtFull": s.pump_gps_at_full,
            "pumpMinDuty": s.pump_min_duty,
            "pumpLagS": s.pump_lag_s,
            "telemetryHz": s.telemetry_hz,
            "centerR": s.center_r,
            "centerThetaDeg": s.center_theta_deg,
        });
        let mut h = format!(
            "# openpour run {run}\n# kind: {kind}\n# firmware: {}\n# started_ms: {}\n# settings: {settings}\n",
            self.firmware,
            m.now_ms()
        );
        if let Some(cal) = brew.calibration_kind() {
            h += &format!("# calibration: {cal}\n");
        } else {
            h += &format!("# recipe: {}\n", brew.recipe_json());
        }
        h += &COLUMNS.join(",");
        h.push('\n');
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeMachine;

    fn run(b: &mut Brew, r: &mut Recorder, m: &mut FakeMachine, ms: u32) -> Vec<Event> {
        let mut ev = Vec::new();
        for _ in 0..ms / 10 {
            m.tick(10);
            b.update(m);
            ev.extend(r.update(b, m));
        }
        ev
    }

    fn samples(ev: &[Event]) -> Vec<&Sample> {
        ev.iter().filter_map(|e| if let Event::Sample { sample, .. } = e { Some(sample) } else { None }).collect()
    }

    #[test]
    fn records_one_run_per_brew() {
        let mut m = FakeMachine::default();
        let mut b = Brew::default();
        let mut r = Recorder::new("test", 7);
        assert!(run(&mut b, &mut r, &mut m, 1000).is_empty(), "nothing recorded while idle");

        b.start(&mut m, "v60-single").unwrap();
        let ev = run(&mut b, &mut r, &mut m, 600_000);
        let Event::Start { run: id, kind, header } = &ev[0] else { panic!("{:?}", ev[0]) };
        assert_eq!((*id, *kind), (7, "brew"));
        assert!(header.contains("# recipe: {") && header.contains("\"id\":\"v60-single\""));
        assert!(header.ends_with(&(COLUMNS.join(",") + "\n")));
        assert_eq!(ev.last(), Some(&Event::End { run: 7, state: "done" }));
        assert_eq!(r.current_run(), None);

        let s = samples(&ev);
        let last = s.last().unwrap();
        assert!((last.poured_g - m.grams()).abs() < 0.01);
        assert!(s.iter().any(|x| x.state == "pouring" && x.target_flow_gps > 0.0 && x.duty > 0.0));
        assert_eq!(last.to_csv().split(',').count(), COLUMNS.len());
        assert_eq!(last.to_json().as_array().unwrap().len(), COLUMNS.len());

        b.stop(&mut m);
        b.calibrate_pump(&mut m).unwrap();
        let ev = run(&mut b, &mut r, &mut m, 30_000);
        assert!(matches!(&ev[0], Event::Start { run: 8, kind: "calibration", header } if header.contains("# calibration: pump")));
    }

    #[test]
    fn samples_at_the_configured_rate() {
        for hz in [10.0, 50.0] {
            let mut m = FakeMachine::default();
            m.settings.telemetry_hz = hz;
            let mut b = Brew::default();
            let mut r = Recorder::new("test", 1);
            b.start(&mut m, "v60-single").unwrap();
            let n = samples(&run(&mut b, &mut r, &mut m, 10_000)).len() as f32;
            assert!((n - hz * 10.0).abs() <= 2.0, "{hz} Hz gave {n} samples in 10 s");
        }
    }
}
