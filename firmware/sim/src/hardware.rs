//! The physical machine: two stepper axes with endstop switches, and the
//! pump → tube → flow meter path. Faults can be switched on to exercise the
//! firmware's error handling.

use pourcore::motion::{Axis, Endstops};
use serde_json::{json, Value};

/// A stepper axis in velocity mode, ramping at its acceleration. The switch
/// sits at physical position 0 and the carriage can't travel past it.
pub struct SimAxis {
    /// Steps from the endstop switch.
    phys: f64,
    /// What the firmware believes minus where the axis really is.
    offset: i32,
    v: f32,
    target_v: f32,
    accel: f32,
}

impl SimAxis {
    pub fn new(start_steps: f64) -> Self {
        SimAxis { phys: start_steps, offset: 0, v: 0.0, target_v: 0.0, accel: 1000.0 }
    }

    pub fn at_switch(&self) -> bool {
        self.phys <= 0.0
    }

    pub fn tick(&mut self, dt: f32) {
        let dv = self.accel * dt;
        self.v += (self.target_v - self.v).clamp(-dv, dv);
        self.phys += (self.v * dt) as f64;
        if self.phys < 0.0 {
            // Hard stop: the motor stalls against the end of travel.
            self.phys = 0.0;
            self.v = self.v.max(0.0);
        }
    }
}

impl Axis for SimAxis {
    fn position(&self) -> i32 {
        self.phys.round() as i32 + self.offset
    }
    fn is_running(&self) -> bool {
        self.v != 0.0 || self.target_v != 0.0
    }
    fn run_at(&mut self, hz: f32) {
        self.target_v = hz;
    }
    fn stop(&mut self) {
        self.target_v = 0.0;
    }
    fn force_stop_at(&mut self, position: i32) {
        self.v = 0.0;
        self.target_v = 0.0;
        self.offset = position - self.phys.round() as i32;
    }
    fn set_acceleration(&mut self, steps_per_s2: f32) {
        self.accel = steps_per_s2.max(1.0);
    }
}

/// Things that can go wrong, toggled from `POST /api/sim`.
#[derive(Clone, Debug, Default)]
pub struct Faults {
    /// The pump runs but moves no water (empty reservoir, kinked tube).
    pub dry: bool,
    /// Water flows but the meter sends no pulses (unplugged, dead sensor).
    pub meter_dead: bool,
    /// Switches that never close (broken wire).
    pub radial_endstop_stuck: bool,
    pub theta_endstop_stuck: bool,
    /// The DS18B20 doesn't answer.
    pub no_probe: bool,
}

/// Pump, tube, flow meter and reservoir.
pub struct Plant {
    /// The real pump rate at full duty (the firmware's setting is its calibrated guess).
    pub true_gps: f32,
    /// The meter's real pulses per litre (the setting starts at the datasheet value).
    pub true_ppl: f32,
    pub faults: Faults,
    duty: f32,
    run_until: Option<u32>,
    /// Decays after the pump is switched off: the motor coasting.
    coast: f32,
    pulses: f64,
    /// Everything that has come out of the nozzle, as a jug would measure it.
    pub dispensed_g: f64,
    temp_c: f32,
    noise: u32,
}

/// The pump takes this long to coast to a stop.
const COAST_S: f32 = 0.1;

impl Plant {
    pub fn new() -> Self {
        Plant {
            true_gps: 6.6,
            true_ppl: 2010.0,
            faults: Faults::default(),
            duty: 0.0,
            run_until: None,
            coast: 0.0,
            pulses: 0.0,
            dispensed_g: 0.0,
            temp_c: 94.5,
            noise: 0x1234_5678,
        }
    }

    pub fn set_duty(&mut self, duty: f32) {
        self.duty = duty.clamp(0.0, 1.0);
        self.run_until = None;
    }

    pub fn run_for(&mut self, duty: f32, until_ms: u32) {
        self.duty = duty.clamp(0.0, 1.0);
        self.run_until = Some(until_ms);
    }

    pub fn duty(&self) -> f32 {
        self.duty
    }

    /// Running pulse total, as the ESP32's counter reports it.
    pub fn pulse_total(&self) -> u32 {
        self.pulses as u64 as u32
    }

    pub fn temp_c(&self) -> Option<f32> {
        (!self.faults.no_probe).then_some(self.temp_c)
    }

    /// ±3 % ripple, like a peristaltic pump's rollers.
    fn ripple(&mut self) -> f32 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        1.0 + ((self.noise % 1000) as f32 / 1000.0 - 0.5) * 0.06
    }

    pub fn tick(&mut self, now: u32, dt: f32) {
        if self.run_until.is_some_and(|t| now.wrapping_sub(t) as i32 >= 0) {
            self.set_duty(0.0);
        }
        self.coast = if self.duty > 0.0 { self.duty } else { (self.coast - dt / COAST_S).max(0.0) };
        let g = if self.faults.dry { 0.0 } else { self.coast * self.true_gps * self.ripple() * dt };
        self.dispensed_g += g as f64;
        if !self.faults.meter_dead {
            self.pulses += (g * self.true_ppl / 1000.0) as f64;
        }
        self.temp_c = (self.temp_c - 0.004 * dt).max(20.0);
    }

    pub fn endstops(&self, radial: &SimAxis, theta: &SimAxis) -> Endstops {
        Endstops {
            radial: radial.at_switch() && !self.faults.radial_endstop_stuck,
            theta: theta.at_switch() && !self.faults.theta_endstop_stuck,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "dispensedG": self.dispensed_g,
            "trueGps": self.true_gps,
            "truePpl": self.true_ppl,
            "tempC": self.temp_c,
            "faults": {
                "dry": self.faults.dry,
                "meterDead": self.faults.meter_dead,
                "radialEndstopStuck": self.faults.radial_endstop_stuck,
                "thetaEndstopStuck": self.faults.theta_endstop_stuck,
                "noProbe": self.faults.no_probe,
            },
        })
    }

    /// Applies `POST /api/sim` fields; unknown keys are ignored.
    pub fn apply(&mut self, v: &Value) {
        let b = |k: &str| v.get(k).and_then(Value::as_bool);
        let f = |k: &str| v.get(k).and_then(Value::as_f64).map(|n| n as f32);
        let faults = v.get("faults").unwrap_or(v);
        let fb = |k: &str| faults.get(k).and_then(Value::as_bool);
        if let Some(x) = fb("dry") { self.faults.dry = x; }
        if let Some(x) = fb("meterDead") { self.faults.meter_dead = x; }
        if let Some(x) = fb("radialEndstopStuck") { self.faults.radial_endstop_stuck = x; }
        if let Some(x) = fb("thetaEndstopStuck") { self.faults.theta_endstop_stuck = x; }
        if let Some(x) = fb("noProbe") { self.faults.no_probe = x; }
        if let Some(x) = f("trueGps") { self.true_gps = x.max(0.0); }
        if let Some(x) = f("truePpl") { self.true_ppl = x.max(1.0); }
        if let Some(x) = f("tempC") { self.temp_c = x; }
        if b("resetDispensed") == Some(true) { self.dispensed_g = 0.0; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_stops_at_its_switch() {
        let mut a = SimAxis::new(100.0);
        a.set_acceleration(10_000.0);
        a.run_at(-500.0);
        for _ in 0..1000 {
            a.tick(0.001);
        }
        assert!(a.at_switch());
        a.force_stop_at(800);
        assert_eq!(a.position(), 800);
        assert!(!a.is_running());
    }

    #[test]
    fn meter_counts_what_the_pump_moves() {
        let mut p = Plant::new();
        p.set_duty(1.0);
        for ms in 1..=10_000 {
            p.tick(ms, 0.001);
        }
        assert!((p.dispensed_g - 66.0).abs() < 2.0, "{}", p.dispensed_g);
        assert!((p.pulse_total() as f64 - p.dispensed_g * 2.010).abs() < 2.0);
        p.faults.meter_dead = true;
        let before = p.pulse_total();
        p.tick(10_001, 0.5);
        assert_eq!(p.pulse_total(), before);
    }
}
