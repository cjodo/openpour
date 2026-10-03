//! Step generation for both axes from one 40 kHz hardware-timer interrupt.
//!
//! Each axis runs in velocity mode: the main loop sets a target speed, and
//! the interrupt ramps towards it at the axis's acceleration and emits steps
//! from a phase accumulator. Speeds are steps/s in 16.16 fixed point; the
//! interrupt does integer maths only (no FPU use in ISRs on the ESP32).
//!
//! The interrupt is not placed in IRAM, so steps pause while flash is being
//! written (saving settings or recipes). Positions stay exact; the control
//! loop catches up afterwards.

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering::*};

use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::OutputPin;
use esp_idf_svc::sys::*;
use pourcore::motion::Axis;

const TICK_HZ: u32 = 40_000;
const TIMER_RES_HZ: u32 = 1_000_000;
/// At most one step per two ticks (high, then low).
const MAX_HZ: f32 = (TICK_HZ / 2) as f32;
const Q: f32 = 65536.0;
/// The phase accumulator wraps (one step) after TICK_HZ seconds' worth of
/// 16.16 speed: 40 000 << 16 fits in a u32 with room for one more tick.
const STEP_THRESHOLD: u32 = TICK_HZ << 16;

struct Channel {
    step_pin: AtomicI32,
    dir_pin: AtomicI32,
    invert: AtomicBool,
    pos: AtomicI32,
    /// Current speed, written only by the interrupt.
    v: AtomicI32,
    /// Requested speed.
    target: AtomicI32,
    /// Speed change per tick.
    accel: AtomicI32,
    phase: AtomicU32,
    pulse_high: AtomicBool,
    force: AtomicBool,
    force_pos: AtomicI32,
}

impl Channel {
    const fn new() -> Self {
        Channel {
            step_pin: AtomicI32::new(-1),
            dir_pin: AtomicI32::new(-1),
            invert: AtomicBool::new(false),
            pos: AtomicI32::new(0),
            v: AtomicI32::new(0),
            target: AtomicI32::new(0),
            accel: AtomicI32::new(1),
            phase: AtomicU32::new(0),
            pulse_high: AtomicBool::new(false),
            force: AtomicBool::new(false),
            force_pos: AtomicI32::new(0),
        }
    }

    #[inline(always)]
    fn tick(&self) {
        let step = self.step_pin.load(Relaxed);
        if step < 0 {
            return;
        }
        if self.pulse_high.load(Relaxed) {
            unsafe { gpio_set_level(step, 0) };
            self.pulse_high.store(false, Relaxed);
        }
        if self.force.load(Acquire) {
            self.v.store(0, Relaxed);
            self.target.store(0, Relaxed);
            self.phase.store(0, Relaxed);
            self.pos.store(self.force_pos.load(Relaxed), Relaxed);
            self.force.store(false, Release);
            return;
        }

        let target = self.target.load(Relaxed);
        let a = self.accel.load(Relaxed);
        let old = self.v.load(Relaxed);
        let v = if old < target {
            (old + a).min(target)
        } else {
            (old - a).max(target)
        };
        self.v.store(v, Relaxed);
        if v == 0 {
            return;
        }
        if v.signum() != old.signum() {
            // Direction changes: set DIR now, step no earlier than next tick.
            let forward = v > 0;
            let level = forward != self.invert.load(Relaxed);
            unsafe { gpio_set_level(self.dir_pin.load(Relaxed), level as u32) };
            self.phase.store(0, Relaxed);
            return;
        }
        // |v| <= MAX_HZ keeps this below 2 * STEP_THRESHOLD (no overflow).
        let mut ph = self.phase.load(Relaxed) + v.unsigned_abs();
        if ph >= STEP_THRESHOLD {
            ph -= STEP_THRESHOLD;
            unsafe { gpio_set_level(step, 1) };
            self.pulse_high.store(true, Relaxed);
            self.pos.fetch_add(v.signum(), Relaxed);
        }
        self.phase.store(ph, Relaxed);
    }
}

static CHANNELS: [Channel; 2] = [Channel::new(), Channel::new()];

unsafe extern "C" fn on_alarm(
    _timer: gptimer_handle_t,
    _event: *const gptimer_alarm_event_data_t,
    _ctx: *mut c_void,
) -> bool {
    for c in &CHANNELS {
        c.tick();
    }
    false // no task woken
}

fn output(pin: i32) -> Result<(), EspError> {
    esp!(unsafe { gpio_reset_pin(pin) })?;
    esp!(unsafe { gpio_set_direction(pin, gpio_mode_t_GPIO_MODE_OUTPUT) })?;
    esp!(unsafe { gpio_set_level(pin, 0) })
}

/// One axis's handle for the main loop.
pub struct Stepper {
    ch: &'static Channel,
}

impl Stepper {
    fn new(
        idx: usize,
        step: impl OutputPin,
        dir: impl OutputPin,
        invert: bool,
    ) -> Result<Stepper, EspError> {
        let ch = &CHANNELS[idx];
        let (step, dir) = (step.pin() as i32, dir.pin() as i32);
        output(step)?;
        output(dir)?;
        ch.invert.store(invert, Relaxed);
        ch.dir_pin.store(dir, Relaxed);
        ch.step_pin.store(step, Release);
        Ok(Stepper { ch })
    }
}

impl Axis for Stepper {
    fn position(&self) -> i32 {
        self.ch.pos.load(Relaxed)
    }

    fn is_running(&self) -> bool {
        self.ch.v.load(Relaxed) != 0 || self.ch.target.load(Relaxed) != 0
    }

    fn run_at(&mut self, hz: f32) {
        self.ch.target.store((hz.clamp(-MAX_HZ, MAX_HZ) * Q) as i32, Relaxed);
    }

    fn stop(&mut self) {
        self.ch.target.store(0, Relaxed);
    }

    fn force_stop_at(&mut self, position: i32) {
        self.ch.force_pos.store(position, Relaxed);
        self.ch.force.store(true, Release);
        // The interrupt applies it within one tick (25 µs).
        for _ in 0..1000 {
            if !self.ch.force.load(Acquire) {
                return;
            }
            Ets::delay_us(1);
        }
    }

    fn set_acceleration(&mut self, steps_per_s2: f32) {
        let per_tick = (steps_per_s2 * Q / TICK_HZ as f32) as i32;
        self.ch.accel.store(per_tick.max(1), Relaxed);
    }
}

/// Applies the direction settings. Takes effect at the next direction change,
/// so call it while the axes are stopped (as they are when settings change).
pub fn set_inverted(theta: bool, radial: bool) {
    CHANNELS[0].invert.store(theta, Relaxed);
    CHANNELS[1].invert.store(radial, Relaxed);
}

/// Configures the pins and starts the step interrupt. Call once, from the
/// task whose core should service the interrupt.
pub fn start(
    theta: (impl OutputPin, impl OutputPin, bool),
    radial: (impl OutputPin, impl OutputPin, bool),
) -> Result<(Stepper, Stepper), EspError> {
    let theta = Stepper::new(0, theta.0, theta.1, theta.2)?;
    let radial = Stepper::new(1, radial.0, radial.1, radial.2)?;

    let cfg = gptimer_config_t {
        clk_src: soc_periph_gptimer_clk_src_t_GPTIMER_CLK_SRC_DEFAULT,
        direction: gptimer_count_direction_t_GPTIMER_COUNT_UP,
        resolution_hz: TIMER_RES_HZ,
        ..Default::default()
    };
    let mut timer: gptimer_handle_t = ptr::null_mut();
    unsafe {
        esp!(gptimer_new_timer(&cfg, &mut timer))?;
        let cbs = gptimer_event_callbacks_t { on_alarm: Some(on_alarm) };
        esp!(gptimer_register_event_callbacks(timer, &cbs, ptr::null_mut()))?;
        let mut alarm = gptimer_alarm_config_t {
            alarm_count: (TIMER_RES_HZ / TICK_HZ) as u64,
            reload_count: 0,
            ..Default::default()
        };
        alarm.flags.set_auto_reload_on_alarm(1);
        esp!(gptimer_set_alarm_action(timer, &alarm))?;
        esp!(gptimer_enable(timer))?;
        esp!(gptimer_start(timer))?;
    }
    Ok((theta, radial))
}
