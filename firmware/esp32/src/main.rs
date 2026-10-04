//! OpenPour firmware: automated pour-over on an ESP32.
//! See ../../README.md for the hardware and ../../docs for wiring and
//! calibration. The machine logic lives in ../pourcore; this crate connects
//! it to the hardware.
//!
//! Pins (ESP32 DevKit V1, ESP32-WROOM-32; see docs/wiring.md). GPIO 16 is
//! free on WROOM modules; on WROVER (PSRAM) boards move the flow meter.
//!   theta stepper   STEP 26, DIR 25
//!   radial stepper  STEP 33, DIR 32
//!   stepper enable  27 (shared by both drivers, active low)
//!   endstops        theta 18, radial 19 (normally open to GND)
//!   flow meter      16 (pulse output, pulled up to 3V3)
//!   pump PWM        23 (gate of the MOSFET module)
//!   DS18B20         4 (4.7 kΩ pull-up to 3V3)
//!   button          13 (momentary to GND)
//!   status LED      2 (on-board)

mod ds18b20;
mod flowmeter;
mod net;
mod pump;
mod stepper;
mod storage;
mod web;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::hal::gpio::{Input, PinDriver, Pull};
use esp_idf_svc::hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver, Resolution};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::reset;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::sys::esp_timer_get_time;
use pourcore::app::{self, Button, Effect};
use pourcore::brew::Brew;
use pourcore::machine::{Machine, MotionCmd};
use pourcore::meter::FlowMeter;
use pourcore::motion::{Endstops, Motion};
use pourcore::nozzle;
use pourcore::recipes::{self, Recipe};
use pourcore::settings::Settings;
use serde_json::Value;

use crate::ds18b20::Ds18b20;
use crate::flowmeter::PulseCounter;
use crate::net::Net;
use crate::pump::Pump;
use crate::stepper::Stepper;

const STATUS_PERIOD_MS: u32 = 200;
const REBOOT_DELAY_MS: u32 = 1500;
/// A loop iteration longer than this is logged: it delays motion and flow control.
const LOOP_STALL_MS: u32 = 50;

fn millis() -> u32 {
    (unsafe { esp_timer_get_time() } / 1000) as u32
}

type Enable = Box<dyn FnMut(bool)>;

/// Every device behind the brew logic.
struct Devices {
    settings: Settings,
    net: Net,
    pulses: PulseCounter<'static>,
    meter: FlowMeter,
    thermo: Ds18b20<'static>,
    pump: Pump<'static>,
    motion: Motion<Stepper, Enable>,
    theta_endstop: PinDriver<'static, Input>,
    radial_endstop: PinDriver<'static, Input>,
}

impl Devices {
    fn update(&mut self, now: u32) {
        self.meter.on_count(self.pulses.total(), now, self.settings.flow_pulses_per_litre);
        self.thermo.update(now);
        self.pump.update(now);
        let end = Endstops { radial: self.radial_endstop.is_low(), theta: self.theta_endstop.is_low() };
        self.motion.update(now, &self.settings, end);
    }
}

impl Machine for Devices {
    fn now_ms(&self) -> u32 {
        millis()
    }

    fn settings(&self) -> &Settings {
        &self.settings
    }

    fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    fn save_settings(&mut self) {
        stepper::set_inverted(self.settings.invert_theta, self.settings.invert_radial);
        if !storage::save_settings(&self.settings) {
            log::error!("could not save settings");
        }
        self.net.publish_settings(&self.settings);
    }

    fn load_recipe(&mut self, id: &str) -> Option<Recipe> {
        recipes::find(&storage::read_recipes()?, id)
    }

    fn first_recipe_id(&mut self) -> Option<String> {
        recipes::first_id(&storage::read_recipes()?)
    }

    fn save_recipes(&mut self, r: &Value) -> bool {
        storage::save_recipes(r)
    }

    fn grams(&self) -> f32 {
        self.meter.grams()
    }

    fn flow_gps(&self) -> f32 {
        self.meter.flow_gps()
    }

    fn flow_valid(&self) -> bool {
        self.meter.flow_valid()
    }

    fn meter_pulses(&self) -> u32 {
        self.meter.pulses()
    }

    fn reset_poured(&mut self) {
        self.meter.reset();
    }

    fn temp_c(&self) -> Option<f32> {
        self.thermo.celsius()
    }

    fn pump_set(&mut self, duty: f32) {
        self.pump.set(duty);
    }

    fn pump_run_for(&mut self, duty: f32, ms: u32) {
        self.pump.run_for(duty, ms, millis());
    }

    fn pump_duty(&self) -> f32 {
        self.pump.duty()
    }

    fn motion(&mut self, cmd: MotionCmd) {
        let s = &self.settings;
        let m = &mut self.motion;
        match cmd {
            MotionCmd::Home => m.home(millis(), s),
            MotionCmd::Hold => m.hold(s),
            MotionCmd::Park => m.park(s),
            MotionCmd::MoveToBed(p) => m.move_to_bed(p, s),
            MotionCmd::Follow(p) => m.follow(p, millis()),
            MotionCmd::Jog { dr, dtheta } => m.jog(dr, dtheta, s),
            MotionCmd::Release => m.release(),
            MotionCmd::SetCentreHere => {
                let p = m.centre_here(s);
                self.settings.center_r = p.r;
                self.settings.center_theta_deg = p.theta_deg;
                self.save_settings();
            }
        }
    }

    fn homed(&self) -> bool {
        self.motion.homed()
    }

    fn homing_failed(&self) -> bool {
        self.motion.homing_failed()
    }

    fn motion_busy(&self) -> bool {
        self.motion.busy(&self.settings)
    }

    fn motion_mode(&self) -> &'static str {
        self.motion.mode_name(&self.settings)
    }

    fn arm_pose(&self) -> (f32, f32) {
        let p = self.motion.pose(&self.settings);
        (p.theta_deg, p.r)
    }

    fn ap_mode(&self) -> bool {
        self.net.ap_mode()
    }
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let p = Peripherals::take()?;
    let pins = p.pins;

    // Pump off before anything else.
    let ledc_timer = LedcTimerDriver::new(
        p.ledc.timer0,
        &TimerConfig::new().frequency(Hertz(5000)).resolution(Resolution::Bits10),
    )?;
    let pump = Pump::new(LedcDriver::new(p.ledc.channel0, ledc_timer, pins.gpio23)?);

    let button = PinDriver::input(pins.gpio13, Pull::Up)?;
    let mut led = PinDriver::output(pins.gpio2)?;

    if let Err(e) = storage::mount() {
        log::error!("LittleFS mount failed: {e}");
    }
    #[allow(unused_mut)]
    let mut settings = storage::load_settings();
    #[cfg(feature = "wokwi")]
    if settings.wifi_ssid.is_empty() {
        log::info!("Wokwi build: joining Wokwi-GUEST");
        settings.wifi_ssid = "Wokwi-GUEST".into();
    }
    // Wokwi builds put a short test recipe first, so a full brew fits a test run.
    #[cfg(feature = "wokwi")]
    storage::ensure_default_recipes(include_str!("../../wokwi/recipes.json"));
    #[cfg(not(feature = "wokwi"))]
    storage::ensure_default_recipes(web::DEFAULT_RECIPES_JSON);

    let pulses = PulseCounter::new(pins.gpio16)?;
    let thermo = Ds18b20::new(PinDriver::input_output_od(pins.gpio4, Pull::Floating)?, millis());

    let (theta, radial) = stepper::start(
        (pins.gpio26, pins.gpio25, settings.invert_theta),
        (pins.gpio33, pins.gpio32, settings.invert_radial),
    )?;
    let mut enable_pin = PinDriver::output(pins.gpio27)?;
    let enable: Enable = Box::new(move |on| {
        let _ = enable_pin.set_level((!on).into()); // active low
    });
    let motion = Motion::new(theta, radial, enable, &settings);
    let theta_endstop = PinDriver::input(pins.gpio18, Pull::Up)?;
    let radial_endstop = PinDriver::input(pins.gpio19, Pull::Up)?;

    let net = net::start(p.modem, EspSystemEventLoop::take()?, EspDefaultNvsPartition::take()?, &settings)?;
    #[cfg(feature = "wokwi")]
    log::info!(
        "Wokwi: open http://localhost:8180 (forwarded by the VS Code extension); openpour.local and the 10.13.37.x \
         address exist only inside the simulation"
    );

    let mut dev = Devices {
        settings,
        net,
        pulses,
        meter: FlowMeter::default(),
        thermo,
        pump,
        motion,
        theta_endstop,
        radial_endstop,
    };
    let mut brew = Brew::default();
    let mut trail = nozzle::Trail::default();
    let mut button_state = Button::default();
    let mut last_status_ms = 0u32;
    let mut reboot_at: Option<u32> = None;
    let mut last_loop_ms = millis();
    let mut last_stall_log_ms = 0u32;

    loop {
        let now = millis();
        let gap = now.wrapping_sub(last_loop_ms);
        last_loop_ms = now;
        if gap > LOOP_STALL_MS && now.wrapping_sub(last_stall_log_ms) >= 1000 {
            last_stall_log_ms = now;
            log::warn!(target: "loop", "main loop stalled {gap} ms");
        }
        dev.update(now);
        brew.update(&mut dev);
        trail.update(&dev);

        let mut effects = Vec::new();
        if let Some(press) = button_state.update(button.is_low(), now) {
            effects.extend(app::handle_press(press, &mut brew, &mut dev));
        }
        while let Some(line) = dev.net.pop_command() {
            effects.extend(app::handle_command(&line, &mut brew, &mut dev));
        }
        for fx in effects {
            match fx {
                Effect::Notify { kind, msg } => dev.net.send_event(Effect::notify_json(kind, &msg)),
                Effect::Reboot => reboot_at = Some(now.wrapping_add(REBOOT_DELAY_MS)),
            }
        }

        let _ = led.set_level(app::led_on(brew.state(), dev.net.ap_mode(), now).into());

        if dev.net.wants_status() || now.wrapping_sub(last_status_ms) >= STATUS_PERIOD_MS {
            last_status_ms = now;
            let mut status = brew.status(&dev);
            status["path"] = trail.take_json();
            dev.net.send_status(status.to_string());
        }
        if reboot_at.is_some_and(|t| now.wrapping_sub(t) as i32 >= 0) {
            reset::restart();
        }
        FreeRtos::delay_ms(1);
    }
}
