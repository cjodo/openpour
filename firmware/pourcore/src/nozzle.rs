//! Where the nozzle is, relative to the dripper centre, for the app's
//! position chart. Status messages are too coarse to draw a spiral, so
//! `Trail` samples at 50 Hz and each status carries the points since the last
//! one, each with its age so the app can replay them at the right moments.

use serde_json::{json, Value};

use crate::kinematics::{arm_to_bed, ArmPose};
use crate::machine::Machine;

const SAMPLE_MS: u32 = 20;
/// Caps a status message if nobody collects for a while.
const MAX_POINTS: usize = 50;
/// Moves smaller than this aren't worth a point (the arm is standing still).
const MIN_STEP_MM: f32 = 0.05;

/// Nozzle (x, y) in mm from the saved dripper centre, x pointing away from
/// the pivot. None until the arm is homed, as the position isn't known.
pub fn position(m: &impl Machine) -> Option<(f32, f32)> {
    if !m.homed() {
        return None;
    }
    let s = m.settings();
    let (theta_deg, r) = m.arm_pose();
    let p = arm_to_bed(ArmPose { r, theta_deg }, ArmPose { r: s.center_r, theta_deg: s.center_theta_deg });
    Some((p.rho * p.phi.cos(), p.rho * p.phi.sin()))
}

fn round1(v: f32) -> f64 {
    (v as f64 * 10.0).round() / 10.0
}

pub fn to_json((x, y): (f32, f32)) -> Value {
    json!([round1(x), round1(y)])
}

#[derive(Default)]
pub struct Trail {
    last_ms: u32,
    /// (machine ms, x, y)
    points: Vec<(u32, f32, f32)>,
    last: Option<(f32, f32)>,
}

impl Trail {
    /// Call every loop.
    pub fn update(&mut self, m: &impl Machine) {
        let now = m.now_ms();
        if now.wrapping_sub(self.last_ms) < SAMPLE_MS {
            return;
        }
        self.last_ms = now;
        let Some(p) = position(m) else {
            self.last = None;
            return;
        };
        if self.last.is_some_and(|(x, y)| (p.0 - x).hypot(p.1 - y) < MIN_STEP_MM) {
            return;
        }
        if self.points.len() == MAX_POINTS {
            self.points.remove(0);
        }
        self.points.push((now, p.0, p.1));
        self.last = Some(p);
    }

    /// The points since the last call, oldest first, as `[[x, y, age_ms], ...]`
    /// where the age is relative to `now` (the status message's `ms`).
    pub fn take_json(&mut self, now: u32) -> Value {
        Value::Array(
            self.points
                .drain(..)
                .map(|(ms, x, y)| json!([round1(x), round1(y), now.wrapping_sub(ms)]))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeMachine;
    use crate::machine::MotionCmd;

    #[test]
    fn unknown_until_homed() {
        let mut m = FakeMachine::default();
        assert_eq!(position(&m), None);
        m.motion(MotionCmd::Home);
        // The fake reports the arm at the pivot (r = 0), i.e. one centre radius behind it.
        let (x, y) = position(&m).unwrap();
        assert!((x + m.settings.center_r).abs() < 0.01 && y.abs() < 0.01, "{x},{y}");
    }

    #[test]
    fn trail_samples_moves_only() {
        let mut m = FakeMachine::default();
        let mut t = Trail::default();
        m.motion(MotionCmd::Home);
        for _ in 0..100 {
            m.tick(10);
            t.update(&m);
        }
        // Standing still: one point (stamped with its age), then nothing new.
        let pts = t.take_json(m.now);
        assert_eq!(pts.as_array().unwrap().len(), 1);
        assert_eq!(pts[0][2], json!(m.now - 20)); // first sample at 20 ms
        assert_eq!(t.take_json(m.now), json!([]));
    }
}
