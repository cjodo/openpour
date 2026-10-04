//! Flow-meter signal processing: pulse counting, volume and flow rate.
//! The ESP32 pulse counter feeds in a running total; nothing here touches
//! hardware. 1 ml of water is 1 g, so volumes are reported in grams.

const HIST: usize = 40; // 40 x 50 ms = 2 s
const PUSH_MS: u32 = 50;
const FLOW_WINDOW_MS: u32 = 1000;

#[derive(Clone, Copy, Default)]
struct Sample {
    ms: u32,
    g: f32,
}

pub struct FlowMeter {
    last_total: Option<u32>,
    pulses: u32,
    pulses_per_litre: f32,
    // Decimated history for flow-rate estimation.
    hist: [Sample; HIST],
    head: usize,
    count: usize,
    last_push_ms: u32,
}

impl Default for FlowMeter {
    fn default() -> Self {
        FlowMeter {
            last_total: None,
            pulses: 0,
            pulses_per_litre: 1000.0,
            hist: [Sample::default(); HIST],
            head: 0,
            count: 0,
            last_push_ms: 0,
        }
    }
}

impl FlowMeter {
    /// Feed the counter's running pulse total. It may wrap.
    pub fn on_count(&mut self, total: u32, now: u32, pulses_per_litre: f32) {
        if let Some(last) = self.last_total {
            self.pulses = self.pulses.wrapping_add(total.wrapping_sub(last));
        }
        self.last_total = Some(total);
        self.pulses_per_litre = pulses_per_litre.max(1.0);

        if self.count == 0 || now.wrapping_sub(self.last_push_ms) >= PUSH_MS {
            self.last_push_ms = now;
            self.hist[self.head] = Sample { ms: now, g: self.grams() };
            self.head = (self.head + 1) % HIST;
            if self.count < HIST {
                self.count += 1;
            }
        }
    }

    /// Water through the meter since the last reset.
    pub fn grams(&self) -> f32 {
        self.pulses as f32 * 1000.0 / self.pulses_per_litre
    }

    /// Raw pulses since the last reset, for calibration.
    pub fn pulses(&self) -> u32 {
        self.pulses
    }

    pub fn reset(&mut self) {
        self.pulses = 0;
        self.head = 0;
        self.count = 0;
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
}

/// Pulses per litre from a measured dispense, or None if the run was too
/// small to trust.
pub fn calibrate(pulses: u32, actual_ml: f32) -> Option<f32> {
    (pulses >= 50 && actual_ml >= 10.0).then(|| pulses as f32 * 1000.0 / actual_ml)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PPL: f32 = 2000.0;

    #[test]
    fn counts_to_grams() {
        let mut m = FlowMeter::default();
        m.on_count(500, 0, PPL);
        m.on_count(600, 10, PPL);
        assert!((m.grams() - 50.0).abs() < 0.01);
        m.reset();
        assert_eq!(m.grams(), 0.0);
        m.on_count(620, 20, PPL);
        assert!((m.grams() - 10.0).abs() < 0.01);
    }

    #[test]
    fn survives_counter_wrap() {
        let mut m = FlowMeter::default();
        m.on_count(u32::MAX - 9, 0, PPL);
        m.on_count(10, 10, PPL);
        assert_eq!(m.pulses(), 20);
    }

    #[test]
    fn flow_rate_from_history() {
        let mut m = FlowMeter::default();
        // 5 g/s = 10 pulses/s at 2000 pulses/L, read every 10 ms.
        for i in 0..300u32 {
            m.on_count(i / 10, i * 10, PPL);
        }
        assert!(m.flow_valid());
        assert!((m.flow_gps() - 5.0).abs() < 0.6, "{}", m.flow_gps());
    }

    #[test]
    fn calibration_factor() {
        assert_eq!(calibrate(420, 200.0), Some(2100.0));
        assert_eq!(calibrate(10, 200.0), None);
        assert_eq!(calibrate(420, 0.0), None);
    }
}
