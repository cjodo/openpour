//! Everything the brew logic and command handling need from the hardware.
//! The firmware implements this over the real devices; tests use a fake.

use serde_json::Value;

use crate::kinematics::BedPoint;
use crate::pattern::PatternParams;
use crate::recipes::Recipe;
use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MotionCmd {
    Home,
    Hold,
    Park,
    MoveToBed(BedPoint),
    Follow(PatternParams),
    Jog { dr: f32, dtheta: f32 },
    /// Stores the current pose as the dripper centre and saves the settings.
    SetCentreHere,
    Release,
}

pub trait Machine {
    fn now_ms(&self) -> u32;

    fn settings(&self) -> &Settings;
    fn settings_mut(&mut self) -> &mut Settings;
    fn save_settings(&mut self);

    fn load_recipe(&mut self, id: &str) -> Option<Recipe>;
    fn first_recipe_id(&mut self) -> Option<String>;
    /// Replaces the whole recipe file. `recipes` must be an array.
    fn save_recipes(&mut self, recipes: &Value) -> bool;

    /// Water through the flow meter since `reset_poured`. 1 ml = 1 g.
    fn grams(&self) -> f32;
    fn flow_gps(&self) -> f32;
    fn flow_valid(&self) -> bool;
    /// Raw meter pulses since `reset_poured`, for calibration.
    fn meter_pulses(&self) -> u32;
    fn reset_poured(&mut self);

    fn temp_c(&self) -> Option<f32>;

    /// 0..1, cancels any timed run.
    fn pump_set(&mut self, duty: f32);
    fn pump_run_for(&mut self, duty: f32, ms: u32);
    fn pump_off(&mut self) {
        self.pump_set(0.0);
    }
    fn pump_duty(&self) -> f32;

    fn motion(&mut self, cmd: MotionCmd);
    fn homed(&self) -> bool;
    fn homing_failed(&self) -> bool;
    fn motion_busy(&self) -> bool;
    fn motion_mode(&self) -> &'static str;
    /// Current arm angle (degrees) and carriage radius (mm), for telemetry.
    fn arm_pose(&self) -> (f32, f32);

    fn ap_mode(&self) -> bool;
}
