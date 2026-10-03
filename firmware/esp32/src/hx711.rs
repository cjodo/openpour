//! HX711 load-cell ADC, bit-banged. Channel A, gain 128.

use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{Input, Output, PinDriver};
use esp_idf_svc::hal::interrupt;

pub struct Hx711<'d> {
    dout: PinDriver<'d, Input>,
    sck: PinDriver<'d, Output>,
}

impl<'d> Hx711<'d> {
    pub fn new(dout: PinDriver<'d, Input>, mut sck: PinDriver<'d, Output>) -> Self {
        let _ = sck.set_low();
        Hx711 { dout, sck }
    }

    /// A conversion is waiting (10 or 80 SPS depending on the board).
    pub fn is_ready(&self) -> bool {
        self.dout.is_low()
    }

    /// Reads the waiting sample. Holding SCK high for more than 60 µs powers
    /// the chip down, so the 25 clock pulses run with interrupts off (~50 µs).
    pub fn read(&mut self) -> i32 {
        let Hx711 { dout, sck } = self;
        let raw = interrupt::free(|| {
            let mut v: u32 = 0;
            for _ in 0..24 {
                let _ = sck.set_high();
                Ets::delay_us(1);
                v = (v << 1) | dout.is_high() as u32;
                let _ = sck.set_low();
                Ets::delay_us(1);
            }
            // 25th pulse selects channel A, gain 128 for the next conversion.
            let _ = sck.set_high();
            Ets::delay_us(1);
            let _ = sck.set_low();
            v
        });
        ((raw << 8) as i32) >> 8 // sign-extend 24 bits
    }
}
