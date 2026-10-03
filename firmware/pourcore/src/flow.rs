//! Pump flow control. The scale measures what actually landed, so the pump
//! does not need to be precise: duty = feed-forward from the calibrated pump
//! rate + a slow integral correction from the measured flow.

#[derive(Clone, Debug)]
pub struct FlowController {
    /// Grams/second at 100% duty (pump calibration).
    pub gps_at_full: f32,
    /// Below this the pump stalls.
    pub min_duty: f32,
    /// Duty per gram of accumulated flow error.
    pub ki: f32,
    integ: f32,
}

impl Default for FlowController {
    fn default() -> Self {
        FlowController { gps_at_full: 6.0, min_duty: 0.25, ki: 0.05, integ: 0.0 }
    }
}

impl FlowController {
    pub fn reset(&mut self) {
        self.integ = 0.0;
    }

    pub fn update(&mut self, target_gps: f32, measured_gps: f32, measured_valid: bool, dt: f32) -> f32 {
        if target_gps <= 0.0 {
            return 0.0;
        }
        if measured_valid {
            self.integ += self.ki * (target_gps - measured_gps) * dt;
            self.integ = self.integ.clamp(-0.5, 0.5);
        }
        let d = (target_gps / self.gps_at_full.max(0.1) + self.integ).clamp(0.0, 1.0);
        d.max(self.min_duty)
    }
}

/// True once the water already in flight (plus scale latency) will reach the
/// target, so the pump should stop now.
pub fn should_stop_pour(grams: f32, flow_gps: f32, lag_s: f32, target_g: f32) -> bool {
    grams + flow_gps.max(0.0) * lag_s >= target_g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32, tol: f32) {
        assert!((a - b).abs() <= tol, "{a} != {b} (±{tol})");
    }

    #[test]
    fn feed_forward_and_clamp() {
        let mut fc = FlowController { gps_at_full: 8.0, min_duty: 0.2, ..Default::default() };
        near(fc.update(4.0, 0.0, false, 0.02), 0.5, 1e-4);
        near(fc.update(0.5, 0.0, false, 0.02), 0.2, 1e-4); // min duty
        near(fc.update(20.0, 0.0, false, 0.02), 1.0, 1e-4); // max duty
        near(fc.update(0.0, 0.0, false, 0.02), 0.0, 1e-4); // off
    }

    #[test]
    fn integral_raises_duty_when_slow() {
        let mut fc = FlowController { gps_at_full: 8.0, ..Default::default() };
        let first = fc.update(4.0, 2.0, true, 0.1);
        let mut later = first;
        for _ in 0..50 {
            later = fc.update(4.0, 2.0, true, 0.1);
        }
        assert!(later > first);
    }

    #[test]
    fn should_stop_pour_accounts_for_lag() {
        assert!(!should_stop_pour(40.0, 4.0, 0.5, 50.0));
        assert!(should_stop_pour(48.5, 4.0, 0.5, 50.0));
        assert!(!should_stop_pour(49.0, -10.0, 0.5, 50.0)); // negative flow ignored
    }
}
