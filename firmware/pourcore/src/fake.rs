//! A simulated machine for host tests: the pump fills the cup at a rate set by
//! its duty, the scale sees it, and motion completes instantly.

use serde_json::Value;

use crate::machine::{Machine, MotionCmd};
use crate::recipes::{self, Recipe};
use crate::settings::Settings;

pub struct FakeMachine {
    pub now: u32,
    pub settings: Settings,
    pub saves: u32,
    pub recipes: Value,
    /// Grams actually in the cup.
    pub water: f32,
    /// Real pump rate at full duty (the settings hold the calibrated guess).
    pub true_gps: f32,
    /// Pump runs but nothing comes out.
    pub dry: bool,
    pub scale_ok: bool,
    pub tare_g: f32,
    pub pump_duty: f32,
    pub pump_stop_at: Option<u32>,
    pub homed: bool,
    /// Grams the scale reported over the last second, for the flow estimate.
    pub history: Vec<(u32, f32)>,
}

impl Default for FakeMachine {
    fn default() -> Self {
        FakeMachine {
            now: 0,
            settings: Settings::default(),
            saves: 0,
            recipes: serde_json::from_str(include_str!("../../../web/default-recipes.json")).unwrap(),
            water: 0.0,
            true_gps: 5.0,
            dry: false,
            scale_ok: true,
            tare_g: 0.0,
            pump_duty: 0.0,
            pump_stop_at: None,
            homed: false,
            history: Vec::new(),
        }
    }
}

impl FakeMachine {
    pub fn tick(&mut self, ms: u32) {
        self.now += ms;
        if self.pump_stop_at.is_some_and(|t| self.now >= t) {
            self.pump_off();
        }
        if !self.dry {
            self.water += self.pump_duty * self.true_gps * ms as f32 / 1000.0;
        }
        let g = self.grams();
        self.history.push((self.now, g));
        let now = self.now;
        self.history.retain(|(t, _)| now - t <= 1000);
    }
}

impl Machine for FakeMachine {
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
        self.saves += 1;
    }
    fn load_recipe(&mut self, id: &str) -> Option<Recipe> {
        recipes::find(&self.recipes, id)
    }
    fn first_recipe_id(&mut self) -> Option<String> {
        recipes::first_id(&self.recipes)
    }
    fn save_recipes(&mut self, r: &Value) -> bool {
        if !r.is_array() {
            return false;
        }
        self.recipes = r.clone();
        true
    }
    fn grams(&self) -> f32 {
        self.water - self.tare_g
    }
    fn flow_gps(&self) -> f32 {
        match (self.history.first(), self.history.last()) {
            (Some(a), Some(b)) if b.0 > a.0 => (b.1 - a.1) * 1000.0 / (b.0 - a.0) as f32,
            _ => 0.0,
        }
    }
    fn flow_valid(&self) -> bool {
        self.history.len() > 50
    }
    fn scale_connected(&self) -> bool {
        self.scale_ok
    }
    fn scale_busy(&self) -> bool {
        false
    }
    fn tare(&mut self) {
        self.tare_g = self.water;
    }
    fn calibrate_scale(&mut self, _known_grams: f32) {}
    fn temp_c(&self) -> Option<f32> {
        Some(93.0)
    }
    fn pump_set(&mut self, duty: f32) {
        self.pump_stop_at = None;
        self.pump_duty = duty.clamp(0.0, 1.0);
    }
    fn pump_run_for(&mut self, duty: f32, ms: u32) {
        self.pump_duty = duty.clamp(0.0, 1.0);
        self.pump_stop_at = Some(self.now + ms);
    }
    fn pump_duty(&self) -> f32 {
        self.pump_duty
    }
    fn motion(&mut self, cmd: MotionCmd) {
        match cmd {
            MotionCmd::Home => self.homed = true,
            MotionCmd::Release => self.homed = false,
            _ => {}
        }
    }
    fn homed(&self) -> bool {
        self.homed
    }
    fn homing_failed(&self) -> bool {
        false
    }
    fn motion_busy(&self) -> bool {
        false
    }
    fn motion_mode(&self) -> &'static str {
        if self.homed { "holding" } else { "released" }
    }
    fn ap_mode(&self) -> bool {
        false
    }
}
