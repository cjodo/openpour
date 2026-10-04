//! One stepper axis and its endstop switch. Counts STEP pulses (direction
//! from DIR: high = towards larger positions, as firmware/esp32/src/stepper.rs
//! drives it) and closes SW to ground while the axis sits at the switch,
//! which is at position 0. The axis can't travel past it.
//!
//! Attributes: `startSteps` (how far from the switch the axis starts),
//! `invert` (1 if the motor is wired backwards, like the firmware's
//! "Reverse … direction" setting) and `stuck` (a slider: 1 = the switch never
//! closes, like a broken wire).

#[path = "../../api.rs"]
mod api;

use std::ffi::c_void;

use api::*;

struct Chip {
    dir: Pin,
    sw: Pin,
    invert: u32,
    stuck: u32,
    pos: i64,
    closed: bool,
}

impl Chip {
    fn update_switch(&mut self) {
        let stuck = unsafe { attr_read_float(self.stuck) } >= 0.5;
        let closed = self.pos <= 0 && !stuck;
        if closed != self.closed {
            self.closed = closed;
            // Closed pulls the line to ground; open leaves it to the ESP32's pull-up.
            unsafe { pin_mode(self.sw, if closed { OUTPUT_LOW } else { INPUT }) };
        }
    }
}

extern "C" fn on_step(data: *mut c_void, _pin: Pin, _value: u32) {
    let c = unsafe { &mut *(data as *mut Chip) };
    let forward = (unsafe { pin_read(c.dir) } == HIGH) != (unsafe { attr_read(c.invert) } != 0);
    c.pos = if forward { c.pos + 1 } else { (c.pos - 1).max(0) };
    c.update_switch();
}

/// Picks up changes to the `stuck` slider while the axis is still.
extern "C" fn on_poll(data: *mut c_void) {
    let c = unsafe { &mut *(data as *mut Chip) };
    c.update_switch();
}

#[export_name = "chipInit"]
pub extern "C" fn chip_init() {
    let start = attr_init(b"startSteps\0", 3200);
    let chip = Box::leak(Box::new(Chip {
        dir: pin_init(b"DIR\0", INPUT),
        sw: pin_init(b"SW\0", INPUT),
        invert: attr_init(b"invert\0", 0),
        stuck: attr_init_float(b"stuck\0", 0.0),
        pos: unsafe { attr_read(start) } as i64,
        closed: false,
    }));
    let data = chip as *mut Chip as *mut c_void;
    let step = pin_init(b"STEP\0", INPUT);
    watch(step, RISING, data, on_step);
    chip.update_switch();
    let t = timer(data, on_poll);
    unsafe { timer_start(t, 10_000, true) };
}
