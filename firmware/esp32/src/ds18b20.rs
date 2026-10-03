//! DS18B20 probe in the reservoir, the only device on its 1-Wire bus.
//! Conversions run in the background: each `update` collects the previous
//! reading and starts the next one.

use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{InputOutput, PinDriver};
use esp_idf_svc::hal::interrupt;

const SKIP_ROM: u8 = 0xCC;
const CONVERT: u8 = 0x44;
const READ_SCRATCHPAD: u8 = 0xBE;
const WRITE_SCRATCHPAD: u8 = 0x4E;
/// 11-bit resolution: 0.125 °C, ~375 ms conversion.
const CONFIG_11_BIT: u8 = 0x5F;
const PERIOD_MS: u32 = 800;

pub struct Ds18b20<'d> {
    /// Open drain, with the 4.7 kΩ pull-up on the board.
    pin: PinDriver<'d, InputOutput>,
    last: Option<f32>,
    requested_ms: u32,
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        let mut b = b;
        for _ in 0..8 {
            let mix = (crc ^ b) & 1;
            crc >>= 1;
            if mix != 0 {
                crc ^= 0x8C;
            }
            b >>= 1;
        }
    }
    crc
}

impl<'d> Ds18b20<'d> {
    pub fn new(pin: PinDriver<'d, InputOutput>, now: u32) -> Self {
        let mut t = Ds18b20 { pin, last: None, requested_ms: now };
        let _ = t.pin.set_high();
        if t.reset() {
            t.write_byte(SKIP_ROM);
            t.write_byte(WRITE_SCRATCHPAD);
            t.write_byte(0); // TH alarm, unused
            t.write_byte(0); // TL alarm, unused
            t.write_byte(CONFIG_11_BIT);
        }
        t.start_conversion();
        t
    }

    pub fn celsius(&self) -> Option<f32> {
        self.last
    }

    pub fn update(&mut self, now: u32) {
        if now.wrapping_sub(self.requested_ms) < PERIOD_MS {
            return;
        }
        self.last = self.read_scratchpad();
        self.start_conversion();
        self.requested_ms = now;
    }

    fn start_conversion(&mut self) {
        if self.reset() {
            self.write_byte(SKIP_ROM);
            self.write_byte(CONVERT);
        }
    }

    fn read_scratchpad(&mut self) -> Option<f32> {
        if !self.reset() {
            return None;
        }
        self.write_byte(SKIP_ROM);
        self.write_byte(READ_SCRATCHPAD);
        let mut sp = [0u8; 9];
        for b in &mut sp {
            *b = self.read_byte();
        }
        if crc8(&sp[..8]) != sp[8] {
            return None;
        }
        Some(i16::from_le_bytes([sp[0], sp[1]]) as f32 / 16.0)
    }

    /// Reset pulse; true if a device answered with a presence pulse.
    fn reset(&mut self) -> bool {
        let _ = self.pin.set_low();
        Ets::delay_us(480);
        let present = interrupt::free(|| {
            let _ = self.pin.set_high();
            Ets::delay_us(70);
            self.pin.is_low()
        });
        Ets::delay_us(410);
        present
    }

    fn write_bit(&mut self, bit: bool) {
        interrupt::free(|| {
            let _ = self.pin.set_low();
            if bit {
                Ets::delay_us(6);
                let _ = self.pin.set_high();
                Ets::delay_us(64);
            } else {
                Ets::delay_us(60);
                let _ = self.pin.set_high();
                Ets::delay_us(10);
            }
        });
    }

    fn read_bit(&mut self) -> bool {
        let bit = interrupt::free(|| {
            let _ = self.pin.set_low();
            Ets::delay_us(3);
            let _ = self.pin.set_high();
            Ets::delay_us(10);
            self.pin.is_high()
        });
        Ets::delay_us(53);
        bit
    }

    fn write_byte(&mut self, b: u8) {
        for i in 0..8 {
            self.write_bit(b >> i & 1 != 0);
        }
    }

    fn read_byte(&mut self) -> u8 {
        (0..8).fold(0, |acc, i| acc | (self.read_bit() as u8) << i)
    }
}

