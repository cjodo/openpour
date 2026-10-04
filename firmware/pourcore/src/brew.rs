//! Runs a recipe: zero the flow meter, home, then for each stage pour to the
//! cumulative target with the stage's flow rate and pattern, wait, and finally
//! park. Also runs the pump and flow-meter calibration dispenses.

use log::{debug, error, info, warn};
use serde_json::{json, Map, Value};

use crate::flow::{should_stop_pour, FlowController};
use crate::kinematics::BedPoint;
use crate::machine::{Machine, MotionCmd};
use crate::pattern::pattern_at;
use crate::recipes::{Recipe, Stage};

/// Pump on this long ...
const NO_FLOW_MS: u32 = 5000;
/// ... must add at least this much.
const NO_FLOW_MIN_G: f32 = 2.0;
const OVERFLOW_MARGIN_G: f32 = 60.0;
const FLOW_SETTLE_MS: u32 = 1500;
const PUMP_CAL_MS: u32 = 10_000;
const PUMP_CAL_SETTLE_MS: u32 = 2000;
/// The meter calibration dispenses this much (by the current calibration) ...
const METER_CAL_G: f32 = 200.0;
/// ... or runs this long, whichever comes first.
const METER_CAL_MAX_MS: u32 = 60_000;
/// Controller internals are logged at debug level this often while pouring.
const DEBUG_PERIOD_MS: u32 = 500;
/// Log target for everything here.
const T: &str = "brew";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Preparing,
    Pouring,
    Waiting,
    Paused,
    Finishing,
    Done,
    Error,
    Calibrating,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Preparing => "preparing",
            State::Pouring => "pouring",
            State::Waiting => "waiting",
            State::Paused => "paused",
            State::Finishing => "finishing",
            State::Done => "done",
            State::Error => "error",
            State::Calibrating => "calibrating",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CalStep {
    Prepare,
    Run,
    Settle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CalKind {
    /// Full power for a fixed time: measures the pump's rate.
    Pump,
    /// Dispense a known amount into a jug for the user to measure.
    Meter,
}

impl CalKind {
    fn name(self) -> &'static str {
        match self {
            CalKind::Pump => "pump",
            CalKind::Meter => "meter",
        }
    }
}

pub struct Brew {
    st: State,
    paused_from: State,
    recipe: Recipe,
    stage_idx: usize,
    /// Cumulative grams at the end of this stage.
    stage_target: f32,
    state_start_ms: u32,
    paused_in_state_ms: u32,
    last_update_ms: u32,
    elapsed_ms: u32,
    prep_moved: bool,
    finish_to_idle: bool,
    flow_check_ms: u32,
    flow_check_g: f32,
    fc: FlowController,
    cal_step: CalStep,
    cal_kind: CalKind,
    last_debug_ms: u32,
    error: String,
    message: String,
}

impl Default for Brew {
    fn default() -> Self {
        Brew {
            st: State::Idle,
            paused_from: State::Idle,
            recipe: Recipe::default(),
            stage_idx: 0,
            stage_target: 0.0,
            state_start_ms: 0,
            paused_in_state_ms: 0,
            last_update_ms: 0,
            elapsed_ms: 0,
            prep_moved: false,
            finish_to_idle: false,
            flow_check_ms: 0,
            flow_check_g: 0.0,
            fc: FlowController::default(),
            cal_step: CalStep::Prepare,
            cal_kind: CalKind::Pump,
            last_debug_ms: 0,
            error: String::new(),
            message: String::new(),
        }
    }
}

impl Brew {
    pub fn state(&self) -> State {
        self.st
    }

    /// Anything but Idle / Done / Error.
    pub fn active(&self) -> bool {
        !matches!(self.st, State::Idle | State::Done | State::Error)
    }

    /// 1-based number of the current stage, 0 outside a recipe.
    pub fn stage_number(&self) -> usize {
        let in_recipe = !matches!(self.st, State::Idle | State::Calibrating) && !self.recipe.stages.is_empty();
        if in_recipe { self.stage_idx + 1 } else { 0 }
    }

    /// Cumulative grams the current stage pours to (0 outside a recipe).
    pub fn stage_target(&self) -> f32 {
        if self.stage_number() > 0 { self.stage_target } else { 0.0 }
    }

    /// The flow rate being asked of the pump right now.
    pub fn target_flow(&self) -> f32 {
        if self.st == State::Pouring { self.stage().flow_gps } else { 0.0 }
    }

    /// "pump" or "meter" while a calibration runs.
    pub fn calibration_kind(&self) -> Option<&'static str> {
        (self.st == State::Calibrating).then(|| self.cal_kind.name())
    }

    /// The running recipe, for logs and telemetry headers.
    pub fn recipe_json(&self) -> Value {
        let r = &self.recipe;
        json!({
            "id": r.id,
            "name": r.name,
            "minTemp": r.min_temp_c,
            "stages": r.stages.iter().map(|s| json!({
                "name": s.name,
                "water": s.water_g,
                "flow": s.flow_gps,
                "pattern": s.pattern.kind.name(),
                "radius": s.pattern.radius_mm,
                "rps": s.pattern.rps,
                "wait": s.wait_s,
            })).collect::<Vec<_>>(),
        })
    }

    fn stage(&self) -> &Stage {
        &self.recipe.stages[self.stage_idx]
    }

    fn enter(&mut self, s: State, now: u32) {
        if s != self.st {
            info!(target: T, "{} -> {}", self.st.name(), s.name());
        }
        self.st = s;
        self.state_start_ms = now;
    }

    fn fail(&mut self, m: &mut impl Machine, msg: &str) {
        error!(target: T, "{msg}");
        m.pump_off();
        m.motion(MotionCmd::Hold);
        self.error = msg.to_owned();
        self.enter(State::Error, m.now_ms());
    }

    fn reset_flow_watchdog(&mut self, m: &impl Machine) {
        self.flow_check_ms = m.now_ms();
        self.flow_check_g = m.grams();
    }

    fn begin_stage(&mut self, m: &mut impl Machine, i: usize) {
        self.stage_idx = i;
        self.stage_target += self.stage().water_g;
        self.fc.gps_at_full = m.settings().pump_gps_at_full;
        self.fc.min_duty = m.settings().pump_min_duty;
        self.fc.reset();
        let st = self.stage();
        info!(
            target: T,
            "stage {}/{} \"{}\": +{} g to {} g at {} g/s, {} r={} mm, then wait {} s",
            i + 1,
            self.recipe.stages.len(),
            st.name,
            st.water_g,
            self.stage_target,
            st.flow_gps,
            st.pattern.kind.name(),
            st.pattern.radius_mm,
            st.wait_s
        );
        if self.stage().water_g <= 0.0 {
            m.motion(MotionCmd::MoveToBed(BedPoint::CENTRE));
            self.enter(State::Waiting, m.now_ms());
            return;
        }
        m.motion(MotionCmd::Follow(self.stage().pattern));
        self.reset_flow_watchdog(m);
        self.enter(State::Pouring, m.now_ms());
    }

    /// Logged when a stage's wait ends: where the water really settled.
    fn log_stage_result(&self, m: &impl Machine) {
        let w = m.grams();
        info!(
            target: T,
            "stage {} settled at {:.1} g (target {:.1}, {:+.1} g)",
            self.stage_idx + 1,
            w,
            self.stage_target,
            w - self.stage_target
        );
    }

    fn finish(&mut self, m: &mut impl Machine, to_idle: bool) {
        m.pump_off();
        self.finish_to_idle = to_idle;
        m.motion(if m.homed() { MotionCmd::Park } else { MotionCmd::Release });
        self.enter(State::Finishing, m.now_ms());
    }

    pub fn start(&mut self, m: &mut impl Machine, recipe_id: &str) -> Result<(), &'static str> {
        if self.active() {
            return Err("A brew is already running.");
        }
        let Some(recipe) = m.load_recipe(recipe_id) else {
            return Err("That recipe was not found or has no stages.");
        };
        if m.settings().last_recipe != recipe_id {
            m.settings_mut().last_recipe = recipe_id.to_owned();
            m.save_settings();
        }
        self.recipe = recipe;
        self.error.clear();
        self.message.clear();
        self.stage_idx = 0;
        self.stage_target = 0.0;
        self.elapsed_ms = 0;
        self.prep_moved = false;
        info!(
            target: T,
            "start \"{}\" ({}): {} stages, {} g, min {} °C, water {}",
            self.recipe.name,
            self.recipe.id,
            self.recipe.stages.len(),
            self.recipe.total_water(),
            self.recipe.min_temp_c,
            m.temp_c().map_or("unknown".into(), |t| format!("{t:.1} °C"))
        );
        if let Some(t) = m.temp_c().filter(|t| *t < self.recipe.min_temp_c) {
            warn!(target: T, "water is {t:.1} °C, below the recipe's {} °C", self.recipe.min_temp_c);
        }
        m.reset_poured();
        if !m.homed() {
            m.motion(MotionCmd::Home);
        }
        self.enter(State::Preparing, m.now_ms());
        Ok(())
    }

    pub fn calibrate_pump(&mut self, m: &mut impl Machine) -> Result<(), &'static str> {
        self.calibrate(m, CalKind::Pump)
    }

    /// Dispenses about METER_CAL_G for the user to measure; then the app sends
    /// the measured amount with the `calMeter` command.
    pub fn calibrate_meter(&mut self, m: &mut impl Machine) -> Result<(), &'static str> {
        self.calibrate(m, CalKind::Meter)
    }

    fn calibrate(&mut self, m: &mut impl Machine, kind: CalKind) -> Result<(), &'static str> {
        if self.active() {
            return Err("Wait for the brew to finish first.");
        }
        self.error.clear();
        self.message.clear();
        self.prep_moved = false;
        self.cal_step = CalStep::Prepare;
        self.cal_kind = kind;
        info!(target: T, "{} calibration started", kind.name());
        m.reset_poured();
        if !m.homed() {
            m.motion(MotionCmd::Home);
        }
        self.enter(State::Calibrating, m.now_ms());
        Ok(())
    }

    pub fn pause(&mut self, m: &mut impl Machine) {
        if !matches!(self.st, State::Pouring | State::Waiting | State::Preparing) {
            return;
        }
        info!(target: T, "paused while {} at {:.1} g", self.st.name(), m.grams());
        m.pump_off();
        m.motion(MotionCmd::Hold);
        self.paused_from = self.st;
        self.paused_in_state_ms = m.now_ms().wrapping_sub(self.state_start_ms);
        self.st = State::Paused;
    }

    pub fn resume(&mut self, m: &mut impl Machine) {
        if self.st != State::Paused {
            return;
        }
        info!(target: T, "resumed {}", self.paused_from.name());
        self.st = self.paused_from;
        self.state_start_ms = m.now_ms().wrapping_sub(self.paused_in_state_ms);
        match self.st {
            State::Pouring => {
                m.motion(MotionCmd::Follow(self.stage().pattern));
                self.reset_flow_watchdog(m);
            }
            State::Waiting => m.motion(MotionCmd::MoveToBed(BedPoint::CENTRE)),
            _ => self.prep_moved = false,
        }
    }

    pub fn stop(&mut self, m: &mut impl Machine) {
        if matches!(self.st, State::Done | State::Error) {
            m.pump_off();
            self.error.clear();
            self.enter(State::Idle, m.now_ms());
        } else if self.active() {
            info!(target: T, "stopped by the user while {} at {:.1} g", self.st.name(), m.grams());
            self.finish(m, true);
        }
    }

    /// Homed with the nozzle sitting at `at`: ready to pour.
    fn prepared(&mut self, m: &mut impl Machine, at: BedPoint) -> bool {
        if m.homing_failed() {
            self.fail(m, "Homing failed: an endstop never triggered. Check the switches and wiring.");
            return false;
        }
        if !m.homed() {
            return false;
        }
        if !self.prep_moved {
            m.motion(MotionCmd::MoveToBed(at));
            self.prep_moved = true;
            return false;
        }
        !m.motion_busy()
    }

    fn update_pouring(&mut self, m: &mut impl Machine, now: u32, dt: u32) {
        self.elapsed_ms += dt;
        let w = m.grams();
        let f = m.flow_gps();

        if w > self.recipe.total_water() + OVERFLOW_MARGIN_G {
            error!(
                target: T,
                "overflow guard: {w:.1} g counted, limit {:.1} g (recipe {} + {OVERFLOW_MARGIN_G})",
                self.recipe.total_water() + OVERFLOW_MARGIN_G,
                self.recipe.total_water()
            );
            self.fail(m, "The flow meter counted more water than the recipe holds. Stopped to prevent an overflow.");
            return;
        }
        let lag = m.settings().pump_lag_s;
        if should_stop_pour(w, f, lag, self.stage_target) {
            info!(
                target: T,
                "stage {} pump off at {w:.1} g, flow {f:.2} g/s, lead {lag} s (target {:.1})",
                self.stage_idx + 1,
                self.stage_target
            );
            m.pump_off();
            m.motion(MotionCmd::MoveToBed(BedPoint::CENTRE));
            self.enter(State::Waiting, now);
            return;
        }
        let settled = m.flow_valid() && now.wrapping_sub(self.state_start_ms) > FLOW_SETTLE_MS;
        let target = self.stage().flow_gps;
        let duty = self.fc.update(target, f, settled, dt as f32 / 1000.0);
        m.pump_set(duty);
        if now.wrapping_sub(self.last_debug_ms) >= DEBUG_PERIOD_MS {
            self.last_debug_ms = now;
            debug!(
                target: "flow",
                "target {target} g/s, measured {f:.2} (settled {settled}), duty {duty:.3}, integral {:+.3}, {:.1} g to go",
                self.fc.integral(),
                self.stage_target - (w + f.max(0.0) * lag)
            );
        }

        if now.wrapping_sub(self.flow_check_ms) >= NO_FLOW_MS {
            if w - self.flow_check_g < NO_FLOW_MIN_G {
                error!(
                    target: T,
                    "dry-flow watchdog: {:.1} g in {} ms (need {NO_FLOW_MIN_G}) at duty {duty:.2}, {} pulses",
                    w - self.flow_check_g,
                    now.wrapping_sub(self.flow_check_ms),
                    m.meter_pulses()
                );
                self.fail(m, "No water is flowing. Refill the reservoir or check the pump tubing and flow meter.");
                return;
            }
            self.reset_flow_watchdog(m);
        }
    }

    fn update_cal(&mut self, m: &mut impl Machine, now: u32) {
        match self.cal_step {
            CalStep::Prepare => {
                if self.prepared(m, BedPoint::CENTRE) {
                    m.pump_set(1.0);
                    self.cal_step = CalStep::Run;
                    self.state_start_ms = now;
                }
            }
            CalStep::Run => {
                let ran = now.wrapping_sub(self.state_start_ms);
                let done = match self.cal_kind {
                    CalKind::Pump => ran >= PUMP_CAL_MS,
                    CalKind::Meter => m.grams() >= METER_CAL_G || ran >= METER_CAL_MAX_MS,
                };
                if done {
                    info!(target: T, "calibration run ended after {ran} ms with {:.1} g counted", m.grams());
                    m.pump_off();
                    self.cal_step = CalStep::Settle;
                    self.state_start_ms = now;
                }
            }
            CalStep::Settle => {
                if now.wrapping_sub(self.state_start_ms) < PUMP_CAL_SETTLE_MS {
                    return;
                }
                if m.grams() < 2.0 {
                    self.fail(m, "The flow meter counted almost no water. Is the reservoir primed and the meter wired?");
                    return;
                }
                match self.cal_kind {
                    CalKind::Pump => {
                        let gps = m.grams() / (PUMP_CAL_MS as f32 / 1000.0);
                        info!(target: "cal", "pump: {gps:.3} g/s at full power (was {:.3})", m.settings().pump_gps_at_full);
                        m.settings_mut().pump_gps_at_full = gps;
                        m.save_settings();
                        self.message = format!("Pump calibrated at {gps:.2} g/s.");
                    }
                    CalKind::Meter => {
                        info!(target: "cal", "meter: dispensed {:.1} g by count ({} pulses); waiting for the measured amount", m.grams(), m.meter_pulses());
                        self.message = format!(
                            "The meter counted {:.0} ml. Weigh or measure the water and enter the real amount.",
                            m.grams()
                        );
                    }
                }
                self.finish(m, true);
            }
        }
    }

    pub fn update(&mut self, m: &mut impl Machine) {
        let now = m.now_ms();
        let dt = now.wrapping_sub(self.last_update_ms);
        self.last_update_ms = now;

        match self.st {
            State::Preparing => {
                // Travel to where the first pattern starts before any water flows.
                let start = pattern_at(&self.recipe.stages[0].pattern, 0.0);
                if self.prepared(m, start) {
                    self.begin_stage(m, 0);
                }
            }
            State::Pouring => self.update_pouring(m, now, dt),
            State::Waiting => {
                self.elapsed_ms += dt;
                if now.wrapping_sub(self.state_start_ms) >= (self.stage().wait_s * 1000.0) as u32 {
                    self.log_stage_result(m);
                    if self.stage_idx + 1 < self.recipe.stages.len() {
                        self.begin_stage(m, self.stage_idx + 1);
                    } else {
                        self.finish(m, false);
                    }
                }
            }
            State::Finishing => {
                if !m.motion_busy() {
                    m.motion(MotionCmd::Release);
                    let next = if self.finish_to_idle { State::Idle } else { State::Done };
                    self.enter(next, now);
                }
            }
            State::Calibrating => self.update_cal(m, now),
            State::Idle | State::Paused | State::Done | State::Error => {}
        }
    }

    /// The status message pushed to the app over the WebSocket.
    pub fn status(&self, m: &impl Machine) -> Value {
        let now = m.now_ms();
        let mut o = Map::new();
        o.insert("t".into(), json!("status"));
        o.insert("state".into(), json!(self.st.name()));
        if self.st == State::Paused {
            o.insert("pausedFrom".into(), json!(self.paused_from.name()));
        }
        let brewing = self.st != State::Idle && self.st != State::Calibrating && !self.recipe.stages.is_empty();
        if brewing {
            let stage = self.stage();
            o.insert("recipe".into(), json!(self.recipe.id));
            o.insert("recipeName".into(), json!(self.recipe.name));
            o.insert("stage".into(), json!(self.stage_idx));
            o.insert("stages".into(), json!(self.recipe.stages.len()));
            o.insert("stageName".into(), json!(stage.name));
            o.insert("target".into(), json!(self.stage_target));
            o.insert("total".into(), json!(self.recipe.total_water()));
            o.insert("targetFlow".into(), json!(stage.flow_gps));
            o.insert("minTemp".into(), json!(self.recipe.min_temp_c));
            o.insert("elapsed".into(), json!(self.elapsed_ms as f32 / 1000.0));
            let waiting = self.st == State::Waiting
                || (self.st == State::Paused && self.paused_from == State::Waiting);
            if waiting {
                let in_state = if self.st == State::Paused {
                    self.paused_in_state_ms
                } else {
                    now.wrapping_sub(self.state_start_ms)
                };
                o.insert("waitLeft".into(), json!((stage.wait_s - in_state as f32 / 1000.0).max(0.0)));
            }
        }
        o.insert("poured".into(), json!(m.grams()));
        o.insert("flow".into(), json!(m.flow_gps()));
        o.insert("duty".into(), json!(m.pump_duty()));
        if let Some(t) = m.temp_c() {
            o.insert("temp".into(), json!(t));
        }
        o.insert("motion".into(), json!(m.motion_mode()));
        o.insert("homed".into(), json!(m.homed()));
        if let Some(p) = crate::nozzle::position(m) {
            o.insert("nozzle".into(), crate::nozzle::to_json(p));
        }
        if !self.error.is_empty() {
            o.insert("error".into(), json!(self.error));
        }
        if !self.message.is_empty() {
            o.insert("message".into(), json!(self.message));
        }
        Value::Object(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{capture_logs, logged, FakeMachine};

    fn has(lines: &[String], needle: &str) -> bool {
        lines.iter().any(|l| l.contains(needle))
    }

    fn run(b: &mut Brew, m: &mut FakeMachine, ms: u32) {
        for _ in 0..ms / 10 {
            m.tick(10);
            b.update(m);
        }
    }

    #[test]
    fn brews_a_recipe_to_its_targets() {
        capture_logs();
        let mut m = FakeMachine::default();
        let mut b = Brew::default();
        b.start(&mut m, "v60-single").unwrap();
        assert_eq!(m.settings.last_recipe, "v60-single");
        run(&mut b, &mut m, 600_000);
        assert_eq!(b.state(), State::Done, "{}", b.status(&m));
        let total = b.recipe.total_water();
        assert!((m.water - total).abs() < 5.0, "poured {} of {total}", m.water);
        assert_eq!(m.pump_duty, 0.0);
        assert!(!m.homed());
        b.stop(&mut m);
        assert_eq!(b.state(), State::Idle);

        let log = logged();
        assert!(has(&log, "INFO brew: start \"V60 single cup\""), "{log:#?}");
        assert!(has(&log, "INFO brew: stage 1/"));
        assert!(has(&log, "pump off at"));
        assert!(has(&log, "INFO brew: stage 1 settled at"));
        assert!(has(&log, "INFO brew: finishing -> done"));
        assert!(has(&log, "DEBUG flow: target"));
    }

    #[test]
    fn pause_stops_the_pump_and_resume_continues() {
        let mut m = FakeMachine::default();
        let mut b = Brew::default();
        b.start(&mut m, "v60-single").unwrap();
        run(&mut b, &mut m, 3000);
        assert_eq!(b.state(), State::Pouring);
        b.pause(&mut m);
        assert_eq!(m.pump_duty, 0.0);
        let before = m.water;
        run(&mut b, &mut m, 5000);
        assert_eq!(b.state(), State::Paused);
        assert!(m.water - before < 1.0);
        assert_eq!(b.status(&m)["pausedFrom"], "pouring");
        b.resume(&mut m);
        run(&mut b, &mut m, 600_000);
        assert_eq!(b.state(), State::Done);
    }

    #[test]
    fn dry_pump_is_an_error() {
        capture_logs();
        let mut m = FakeMachine { dry: true, ..Default::default() };
        let mut b = Brew::default();
        b.start(&mut m, "v60-single").unwrap();
        run(&mut b, &mut m, 10_000);
        assert_eq!(b.state(), State::Error);
        assert!(b.status(&m)["error"].as_str().unwrap().contains("No water"));
        assert_eq!(m.pump_duty, 0.0);
        let log = logged();
        assert!(has(&log, "ERROR brew: dry-flow watchdog: 0.0 g in 5000 ms"), "{log:#?}");
        assert!(has(&log, "-> error"));
    }

    #[test]
    fn refuses_unknown_recipe() {
        let mut m = FakeMachine::default();
        let mut b = Brew::default();
        assert!(b.start(&mut m, "nope").is_err());
        assert!(!b.active());
    }

    #[test]
    fn pump_calibration_measures_rate() {
        let mut m = FakeMachine::default();
        m.true_gps = 7.5;
        let mut b = Brew::default();
        b.calibrate_pump(&mut m).unwrap();
        run(&mut b, &mut m, 20_000);
        assert_eq!(b.state(), State::Idle);
        assert!((m.settings.pump_gps_at_full - 7.5).abs() < 0.1);
    }

    #[test]
    fn meter_calibration_dispenses_and_stops() {
        let mut m = FakeMachine::default();
        let mut b = Brew::default();
        b.calibrate_meter(&mut m).unwrap();
        run(&mut b, &mut m, 120_000);
        assert_eq!(b.state(), State::Idle);
        assert!((m.grams() - METER_CAL_G).abs() < 5.0, "{}", m.grams());
        assert!(b.status(&m)["message"].as_str().unwrap().contains("counted"));
    }
}
