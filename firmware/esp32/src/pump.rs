//! 12 V pump on a low-side MOSFET module, 5 kHz PWM speed control.

use esp_idf_svc::hal::ledc::LedcDriver;

pub struct Pump<'d> {
    pwm: LedcDriver<'d>,
    duty: f32,
    stop_at_ms: Option<u32>,
}

impl<'d> Pump<'d> {
    pub fn new(pwm: LedcDriver<'d>) -> Self {
        let mut p = Pump { pwm, duty: 0.0, stop_at_ms: None };
        p.write(0.0);
        p
    }

    fn write(&mut self, d: f32) {
        self.duty = d.clamp(0.0, 1.0);
        let max = self.pwm.get_max_duty();
        let _ = self.pwm.set_duty((self.duty * max as f32 + 0.5) as u32);
    }

    /// Ends timed runs.
    pub fn update(&mut self, now: u32) {
        if self.stop_at_ms.is_some_and(|t| now.wrapping_sub(t) as i32 >= 0) {
            self.set(0.0);
        }
    }

    /// 0..1, cancels any timed run.
    pub fn set(&mut self, duty: f32) {
        self.stop_at_ms = None;
        self.write(duty);
    }

    pub fn run_for(&mut self, duty: f32, ms: u32, now: u32) {
        self.write(duty);
        self.stop_at_ms = Some(now.wrapping_add(ms));
    }

    pub fn duty(&self) -> f32 {
        self.duty
    }
}
