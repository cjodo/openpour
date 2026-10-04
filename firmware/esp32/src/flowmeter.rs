//! Hall-effect flow meter on the pulse counter (PCNT). Counts rising edges;
//! the 16-bit hardware count is polled every loop and extended to a running
//! u32 total for pourcore's `FlowMeter`.

use core::time::Duration;

use esp_idf_svc::hal::gpio::{AnyInputPin, InputPin};
use esp_idf_svc::hal::pcnt::config::{
    ChannelConfig, ChannelEdgeAction, ChannelLevelAction, GlitchFilterConfig, UnitConfig,
};
use esp_idf_svc::hal::pcnt::PcntUnitDriver;
use esp_idf_svc::sys::{gpio_pullup_en, EspError};

/// The hardware counter returns to zero when it reaches this.
const HIGH_LIMIT: i32 = 32_767;

pub struct PulseCounter<'d> {
    unit: PcntUnitDriver<'d>,
    last: i32,
    total: u32,
}

impl<'d> PulseCounter<'d> {
    pub fn new(pin: impl InputPin + 'd) -> Result<Self, EspError> {
        let gpio = pin.pin();
        let mut unit = PcntUnitDriver::new(&UnitConfig { low_limit: -1, high_limit: HIGH_LIMIT, ..Default::default() })?;
        // The meter's open-collector output is pulled up to 3V3 externally;
        // the internal pull-up keeps the input defined if that is missing.
        unit.set_glitch_filter(Some(&GlitchFilterConfig { max_glitch: Duration::from_micros(10), ..Default::default() }))?
            .add_channel(Some(pin), None::<AnyInputPin>, &ChannelConfig::default())?
            .set_edge_action(ChannelEdgeAction::Increase, ChannelEdgeAction::Hold)?
            .set_level_action(ChannelLevelAction::Keep, ChannelLevelAction::Keep)?;
        unsafe { gpio_pullup_en(gpio as _) };
        unit.enable()?;
        unit.clear_count()?;
        unit.start()?;
        Ok(PulseCounter { unit, last: 0, total: 0 })
    }

    /// Running pulse total. Call often enough that fewer than 32 767 pulses
    /// arrive between calls (milliseconds, at a few hundred pulses a second).
    pub fn total(&mut self) -> u32 {
        if let Ok(now) = self.unit.get_count() {
            self.total = self.total.wrapping_add((now - self.last).rem_euclid(HIGH_LIMIT) as u32);
            self.last = now;
        }
        self.total
    }
}
