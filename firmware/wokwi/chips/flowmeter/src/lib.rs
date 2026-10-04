//! The pump + hall-effect flow meter. Watches the pump's PWM gate signal
//! (GPIO 23) to measure its duty, works out how much water that moves, and
//! pulses OUT (to GPIO 16) at the meter's pulses per litre.
//!
//! Attributes, also sliders while the simulation runs: `gpsAtFull` (the
//! pump's real rate), `pulsesPerLitre` (the meter's real factor, which the
//! firmware has to calibrate for), `dry` (1 = no water moves) and
//! `meterDead` (1 = water moves but no pulses).

#[path = "../../api.rs"]
mod api;

use std::ffi::c_void;

use api::*;

/// Duty is re-measured and pulses are generated this often.
const TICK_US: u32 = 1000;
/// The pump takes this long to coast to a stop.
const COAST_S: f64 = 0.1;

struct Chip {
    pump: Pin,
    out: Pin,
    gps_at_full: u32,
    pulses_per_litre: u32,
    dry: u32,
    meter_dead: u32,
    /// PWM measurement since the last tick.
    level: bool,
    last_edge_ns: f64,
    high_ns: f64,
    window_start_ns: f64,
    /// Effective pump speed, 0..1, with coast-down.
    speed: f64,
    /// Fractional pulses owed.
    owed: f64,
    pulse_high: bool,
}

extern "C" fn on_pump_edge(data: *mut c_void, _pin: Pin, value: u32) {
    let c = unsafe { &mut *(data as *mut Chip) };
    let now = unsafe { get_sim_nanos() };
    if c.level {
        c.high_ns += now - c.last_edge_ns;
    }
    c.level = value == HIGH;
    c.last_edge_ns = now;
}

extern "C" fn on_tick(data: *mut c_void) {
    let c = unsafe { &mut *(data as *mut Chip) };
    let now = unsafe { get_sim_nanos() };

    // Duty over this window: high time / window length.
    if c.level {
        c.high_ns += now - c.last_edge_ns;
    }
    let window = now - c.window_start_ns;
    let duty = if window > 0.0 { (c.high_ns / window).clamp(0.0, 1.0) } else { 0.0 };
    c.high_ns = 0.0;
    c.last_edge_ns = now;
    c.window_start_ns = now;

    let dt = TICK_US as f64 / 1e6;
    c.speed = if duty > 0.0 { duty } else { (c.speed - dt / COAST_S).max(0.0) };

    let (gps, ppl, dry, dead) = unsafe {
        (
            attr_read_float(c.gps_at_full) as f64,
            attr_read_float(c.pulses_per_litre) as f64,
            attr_read_float(c.dry) >= 0.5,
            attr_read_float(c.meter_dead) >= 0.5,
        )
    };
    if !dry && !dead {
        c.owed += c.speed * gps * dt * ppl / 1000.0;
    }

    // One pulse is one tick high then at least one tick low: up to 500 Hz,
    // far above a real meter's few tens of hertz.
    if c.pulse_high {
        unsafe { pin_write(c.out, LOW) };
        c.pulse_high = false;
    } else if c.owed >= 1.0 {
        c.owed -= 1.0;
        unsafe { pin_write(c.out, HIGH) };
        c.pulse_high = true;
    }
}

#[export_name = "chipInit"]
pub extern "C" fn chip_init() {
    let chip = Box::leak(Box::new(Chip {
        pump: pin_init(b"PUMP\0", INPUT),
        out: pin_init(b"OUT\0", OUTPUT_LOW),
        gps_at_full: attr_init_float(b"gpsAtFull\0", 6.6),
        pulses_per_litre: attr_init_float(b"pulsesPerLitre\0", 2010.0),
        dry: attr_init_float(b"dry\0", 0.0),
        meter_dead: attr_init_float(b"meterDead\0", 0.0),
        level: false,
        last_edge_ns: 0.0,
        high_ns: 0.0,
        window_start_ns: 0.0,
        speed: 0.0,
        owed: 0.0,
        pulse_high: false,
    }));
    let data = chip as *mut Chip as *mut c_void;
    chip.level = unsafe { pin_read(chip.pump) } == HIGH;
    watch(chip.pump, BOTH, data, on_pump_edge);
    let t = timer(data, on_tick);
    unsafe { timer_start(t, TICK_US, true) };
}
