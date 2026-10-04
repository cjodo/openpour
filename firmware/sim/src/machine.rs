//! `SimMachine` implements pourcore's `Machine` over the simulated hardware,
//! mirroring `Devices` in firmware/esp32/src/main.rs: same flow meter, same
//! motion controller, same settings and recipe files.

use std::fs;
use std::path::PathBuf;

use pourcore::machine::{Machine, MotionCmd};
use pourcore::meter::FlowMeter;
use pourcore::motion::Motion;
use pourcore::recipes::{self, Recipe};
use pourcore::settings::Settings;
use serde_json::{json, Value};

use crate::hardware::{Plant, SimAxis};

pub const DEFAULT_RECIPES_JSON: &str = include_str!("../../../web/default-recipes.json");

type Enable = Box<dyn FnMut(bool) + Send>;

pub struct SimMachine {
    pub now: u32,
    pub settings: Settings,
    pub plant: Plant,
    meter: FlowMeter,
    motion: Motion<SimAxis, Enable>,
    /// Where settings.json and recipes.json live; None keeps them in memory (tests).
    data_dir: Option<PathBuf>,
    recipes: Value,
}

impl SimMachine {
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        let read = |name: &str| -> Option<Value> {
            let path = data_dir.as_ref()?.join(name);
            serde_json::from_slice(&fs::read(path).ok()?).ok()
        };
        let settings = read("settings.json").map(|v| Settings::from_storage_json(&v)).unwrap_or_default();
        let recipes = read("recipes.json")
            .filter(Value::is_array)
            .unwrap_or_else(|| serde_json::from_str(DEFAULT_RECIPES_JSON).unwrap());

        // The arm starts somewhere away from both switches, as after power-up.
        let radial = SimAxis::new((40.0 * settings.radial_steps_per_mm) as f64);
        let theta = SimAxis::new((20.0 * settings.theta_steps_per_deg) as f64);
        let enable: Enable = Box::new(|_| {});
        let motion = Motion::new(theta, radial, enable, &settings);

        SimMachine { now: 0, settings, plant: Plant::new(), meter: FlowMeter::default(), motion, data_dir, recipes }
    }

    /// Advances the hardware by `ms` milliseconds of machine time.
    pub fn step(&mut self, ms: u32) {
        self.now = self.now.wrapping_add(ms);
        let dt = ms as f32 / 1000.0;
        self.plant.tick(self.now, dt);
        self.motion.axes_mut().0.tick(dt);
        self.motion.axes_mut().1.tick(dt);
        let (theta, radial) = self.motion.axes();
        let end = self.plant.endstops(radial, theta);
        self.motion.update(self.now, &self.settings, end);
        self.meter.on_count(self.plant.pulse_total(), self.now, self.settings.flow_pulses_per_litre);
    }

    /// `GET /api/settings`: what the firmware serves.
    pub fn settings_api_json(&self) -> String {
        let mut v = self.settings.to_api_json();
        v["apMode"] = json!(false);
        v.to_string()
    }

    pub fn recipes_json(&self) -> String {
        self.recipes.to_string()
    }

    fn write(&self, name: &str, contents: String) -> bool {
        match &self.data_dir {
            Some(dir) => fs::write(dir.join(name), contents).is_ok(),
            None => true,
        }
    }
}

impl Machine for SimMachine {
    fn now_ms(&self) -> u32 {
        self.now
    }
    fn settings(&self) -> &Settings {
        &self.settings
    }
    fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }
    fn save_settings(&mut self) {
        if !self.write("settings.json", self.settings.to_storage_json().to_string()) {
            log::error!("could not save settings");
        }
    }
    fn load_recipe(&mut self, id: &str) -> Option<Recipe> {
        recipes::find(&self.recipes, id)
    }
    fn first_recipe_id(&mut self) -> Option<String> {
        recipes::first_id(&self.recipes)
    }
    fn save_recipes(&mut self, r: &Value) -> bool {
        if !r.is_array() || !self.write("recipes.json", r.to_string()) {
            return false;
        }
        self.recipes = r.clone();
        true
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
        self.plant.temp_c()
    }
    fn pump_set(&mut self, duty: f32) {
        self.plant.set_duty(duty);
    }
    fn pump_run_for(&mut self, duty: f32, ms: u32) {
        self.plant.run_for(duty, self.now.wrapping_add(ms));
    }
    fn pump_duty(&self) -> f32 {
        self.plant.duty()
    }
    fn motion(&mut self, cmd: MotionCmd) {
        let s = &self.settings;
        let m = &mut self.motion;
        match cmd {
            MotionCmd::Home => m.home(self.now, s),
            MotionCmd::Hold => m.hold(s),
            MotionCmd::Park => m.park(s),
            MotionCmd::MoveToBed(p) => m.move_to_bed(p, s),
            MotionCmd::Follow(p) => m.follow(p, self.now),
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
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pourcore::app::handle_command;
    use pourcore::brew::{Brew, State};

    fn run(b: &mut Brew, m: &mut SimMachine, ms: u32) {
        for _ in 0..ms {
            m.step(1);
            b.update(m);
        }
    }

    #[test]
    fn brews_with_real_homing_and_meter() {
        let mut m = SimMachine::new(None);
        let mut b = Brew::default();
        b.start(&mut m, "v60-single").unwrap();
        run(&mut b, &mut m, 400_000);
        assert_eq!(b.state(), State::Done, "{}", b.status(&m));
        // The meter's setting (1925 pulses/L) is off from the real 2010, so the
        // cup gets less than the count says until the meter is calibrated.
        let counted = m.grams();
        assert!((counted - 250.0).abs() < 3.0, "counted {counted}");
        let ratio = m.plant.dispensed_g as f32 / counted;
        assert!((ratio - 1925.0 / 2010.0).abs() < 0.01, "{ratio}");
    }

    #[test]
    fn calibrating_the_meter_fixes_the_volume() {
        let mut m = SimMachine::new(None);
        let mut b = Brew::default();
        handle_command(r#"{"cmd":"meterRun"}"#, &mut b, &mut m);
        run(&mut b, &mut m, 60_000);
        assert_eq!(b.state(), State::Idle);
        let jug = m.plant.dispensed_g;
        handle_command(&format!(r#"{{"cmd":"calMeter","ml":{jug}}}"#), &mut b, &mut m);
        assert!((m.settings.flow_pulses_per_litre - 2010.0).abs() < 15.0, "{}", m.settings.flow_pulses_per_litre);
    }

    #[test]
    fn a_stuck_endstop_fails_homing() {
        let mut m = SimMachine::new(None);
        m.plant.faults.radial_endstop_stuck = true;
        let mut b = Brew::default();
        b.start(&mut m, "v60-single").unwrap();
        run(&mut b, &mut m, 25_000);
        assert_eq!(b.state(), State::Error);
        assert!(b.status(&m)["error"].as_str().unwrap().contains("Homing failed"));
    }

    #[test]
    fn a_dead_meter_trips_the_watchdog() {
        let mut m = SimMachine::new(None);
        m.plant.faults.meter_dead = true;
        let mut b = Brew::default();
        b.start(&mut m, "v60-single").unwrap();
        run(&mut b, &mut m, 20_000);
        assert_eq!(b.state(), State::Error);
        assert!(m.plant.dispensed_g > 10.0, "the pump really ran");
    }
}
