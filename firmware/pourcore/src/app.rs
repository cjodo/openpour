//! Commands from the app, the front-panel button and the status LED.

use serde_json::{json, Value};

use crate::brew::{Brew, State};
use crate::kinematics::BedPoint;
use crate::machine::{Machine, MotionCmd};

const LONG_PRESS_MS: u32 = 1500;
const DEBOUNCE_MS: u32 = 30;

/// Something the firmware must do after a command.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Send `{"t": kind, "msg": msg}` to every client.
    Notify { kind: &'static str, msg: String },
    /// Wi-Fi credentials changed: restart shortly.
    Reboot,
}

impl Effect {
    fn info(msg: &str) -> Effect {
        Effect::Notify { kind: "info", msg: msg.to_owned() }
    }
    fn error(msg: &str) -> Effect {
        Effect::Notify { kind: "error", msg: msg.to_owned() }
    }

    pub fn notify_json(kind: &str, msg: &str) -> String {
        json!({ "t": kind, "msg": msg }).to_string()
    }
}

fn num(doc: &Value, key: &str, default: f32) -> f32 {
    doc.get(key).and_then(Value::as_f64).map_or(default, |n| n as f32)
}

fn start_brew(brew: &mut Brew, m: &mut impl Machine, id: &str, out: &mut Vec<Effect>) {
    if let Err(e) = brew.start(m, id) {
        out.push(Effect::error(e));
    }
}

/// Handles one JSON command line from the WebSocket or a REST POST.
pub fn handle_command(line: &str, brew: &mut Brew, m: &mut impl Machine) -> Vec<Effect> {
    let mut out = Vec::new();
    let Ok(doc) = serde_json::from_str::<Value>(line) else { return out };
    let cmd = doc.get("cmd").and_then(Value::as_str).unwrap_or("");

    match cmd {
        "start" => {
            let id = doc.get("recipe").and_then(Value::as_str).unwrap_or("");
            start_brew(brew, m, id, &mut out);
        }
        "pause" => brew.pause(m),
        "resume" => brew.resume(m),
        "stop" => {
            m.pump_off();
            brew.stop(m);
        }
        _ if brew.active() => out.push(Effect::error("That isn't available while brewing.")),
        "tare" => m.tare(),
        "home" => m.motion(MotionCmd::Home),
        "park" => m.motion(MotionCmd::Park),
        "center" => m.motion(MotionCmd::MoveToBed(BedPoint::CENTRE)),
        "jog" => m.motion(MotionCmd::Jog { dr: num(&doc, "dr", 0.0), dtheta: num(&doc, "dtheta", 0.0) }),
        "setCenter" => {
            m.motion(MotionCmd::SetCentreHere);
            out.push(Effect::info("Dripper centre saved."));
        }
        "release" => m.motion(MotionCmd::Release),
        "prime" => {
            let seconds = num(&doc, "seconds", 3.0).clamp(0.0, 30.0);
            let duty = num(&doc, "duty", 1.0).clamp(0.0, 1.0);
            m.pump_run_for(duty, (seconds * 1000.0) as u32);
        }
        "calScale" => {
            let grams = num(&doc, "grams", 0.0);
            if grams > 0.0 {
                m.calibrate_scale(grams);
                out.push(Effect::info("Calibrating the scale. Keep the weight still."));
            }
        }
        "calPump" => {
            if let Err(e) = brew.calibrate_pump(m) {
                out.push(Effect::error(e));
            }
        }
        "settings" => {
            let data = doc.get("data").unwrap_or(&Value::Null);
            if m.settings_mut().apply_json(data) {
                out.push(Effect::Reboot);
                out.push(Effect::info("Wi-Fi saved. Restarting to connect."));
            } else {
                out.push(Effect::info("Settings saved."));
            }
            m.save_settings();
        }
        "saveRecipes" => {
            let data = doc.get("data").unwrap_or(&Value::Null);
            out.push(if m.save_recipes(data) {
                Effect::Notify { kind: "recipes", msg: "Recipes saved.".into() }
            } else {
                Effect::error("Recipes could not be saved.")
            });
        }
        _ => {}
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Short,
    Long,
}

/// Debounced front-panel button: short press on release, long press while
/// still held.
#[derive(Default)]
pub struct Button {
    was_down: bool,
    down_at_ms: u32,
    long_fired: bool,
}

impl Button {
    pub fn update(&mut self, down: bool, now: u32) -> Option<Press> {
        let mut press = None;
        if down && !self.was_down {
            self.down_at_ms = now;
            self.long_fired = false;
        } else if down && !self.long_fired && now.wrapping_sub(self.down_at_ms) > LONG_PRESS_MS {
            self.long_fired = true;
            press = Some(Press::Long);
        } else if !down && self.was_down && !self.long_fired && now.wrapping_sub(self.down_at_ms) > DEBOUNCE_MS {
            press = Some(Press::Short);
        }
        self.was_down = down;
        press
    }
}

/// Short press: start the last recipe / pause / resume / clear.
/// Long press: stop.
pub fn handle_press(press: Press, brew: &mut Brew, m: &mut impl Machine) -> Vec<Effect> {
    let mut out = Vec::new();
    match press {
        Press::Long => {
            m.pump_off();
            brew.stop(m);
        }
        Press::Short => match brew.state() {
            State::Idle => {
                let last = m.settings().last_recipe.clone();
                let id = if last.is_empty() { m.first_recipe_id().unwrap_or_default() } else { last };
                start_brew(brew, m, &id, &mut out);
            }
            State::Paused => brew.resume(m),
            State::Done | State::Error => brew.stop(m),
            _ => brew.pause(m),
        },
    }
    out
}

/// Off when idle, on while brewing, slow blink in AP mode, fast blink on error.
pub fn led_on(state: State, ap_mode: bool, now: u32) -> bool {
    match state {
        State::Error => (now / 125) % 2 == 1,
        State::Idle | State::Done => ap_mode && (now / 1000) % 2 == 1,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeMachine;

    #[test]
    fn button_short_and_long() {
        let mut b = Button::default();
        assert_eq!(b.update(true, 0), None);
        assert_eq!(b.update(false, 10), None); // bounce
        assert_eq!(b.update(true, 100), None);
        assert_eq!(b.update(false, 300), Some(Press::Short));
        assert_eq!(b.update(true, 1000), None);
        assert_eq!(b.update(true, 2600), Some(Press::Long));
        assert_eq!(b.update(true, 2700), None);
        assert_eq!(b.update(false, 2800), None);
    }

    #[test]
    fn commands_blocked_while_brewing() {
        let mut m = FakeMachine::default();
        let mut brew = Brew::default();
        handle_command(r#"{"cmd":"start","recipe":"v60-single"}"#, &mut brew, &mut m);
        assert!(brew.active());
        let fx = handle_command(r#"{"cmd":"home"}"#, &mut brew, &mut m);
        assert!(matches!(&fx[0], Effect::Notify { kind: "error", .. }));
        handle_command(r#"{"cmd":"stop"}"#, &mut brew, &mut m);
        assert_eq!(brew.state(), State::Finishing);
    }

    #[test]
    fn settings_command_reports_wifi_change() {
        let mut m = FakeMachine::default();
        let mut brew = Brew::default();
        let fx = handle_command(r#"{"cmd":"settings","data":{"parkR":90}}"#, &mut brew, &mut m);
        assert_eq!(fx, vec![Effect::info("Settings saved.")]);
        assert_eq!(m.settings.park_r, 90.0);
        assert_eq!(m.saves, 1);
        let fx = handle_command(r#"{"cmd":"settings","data":{"wifiSsid":"home"}}"#, &mut brew, &mut m);
        assert_eq!(fx[0], Effect::Reboot);
    }

    #[test]
    fn prime_is_clamped() {
        let mut m = FakeMachine::default();
        let mut brew = Brew::default();
        handle_command(r#"{"cmd":"prime","seconds":99,"duty":2}"#, &mut brew, &mut m);
        assert_eq!(m.pump_duty, 1.0);
        assert_eq!(m.pump_stop_at, Some(30_000));
    }

    #[test]
    fn button_starts_the_first_recipe() {
        let mut m = FakeMachine::default();
        let mut brew = Brew::default();
        handle_press(Press::Short, &mut brew, &mut m);
        assert_eq!(brew.state(), State::Preparing);
        handle_press(Press::Short, &mut brew, &mut m);
        assert_eq!(brew.state(), State::Paused);
        handle_press(Press::Long, &mut brew, &mut m);
        assert_eq!(brew.state(), State::Finishing);
    }

    #[test]
    fn led_patterns() {
        assert!(!led_on(State::Idle, false, 1500));
        assert!(led_on(State::Idle, true, 1500));
        assert!(led_on(State::Pouring, false, 0));
        assert!(led_on(State::Error, false, 125) != led_on(State::Error, false, 250));
    }
}
