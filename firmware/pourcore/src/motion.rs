//! Two-axis polar arm: theta (direct-drive arm swing) and radial (belt-driven
//! carriage along the arm). Both axes run in velocity mode so the nozzle can
//! follow continuously changing patterns, including direction reversals.

use crate::kinematics::{bed_to_arm, ArmPose, BedPoint};
use crate::pattern::{pattern_at, PatternParams};
use crate::settings::Settings;

/// One stepper axis running in velocity mode. Speeds are in steps/second.
pub trait Axis {
    fn position(&self) -> i32;
    fn is_running(&self) -> bool;
    /// Runs at a signed speed, ramping there at the configured acceleration.
    fn run_at(&mut self, hz: f32);
    /// Decelerates to a standstill.
    fn stop(&mut self);
    /// Stops immediately and redefines the current position.
    fn force_stop_at(&mut self, position: i32);
    fn set_acceleration(&mut self, steps_per_s2: f32);
}

const THETA_MAX_DEG_S: f32 = 120.0;
const THETA_ACCEL_DEG_S2: f32 = 900.0;
const RADIAL_MAX_MM_S: f32 = 150.0;
const RADIAL_ACCEL_MM_S2: f32 = 1500.0;
const HOME_DEG_S: f32 = 20.0;
const HOME_MM_S: f32 = 15.0;
const HOME_TIMEOUT_MS: u32 = 20_000;
const CONTROL_PERIOD_MS: u32 = 10;
/// 1/s, position error -> velocity.
const KP: f32 = 15.0;
/// Below this, just stop.
const MIN_HZ: f32 = 20.0;
/// Steps.
const DEADBAND: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Released,
    Homing,
    Hold,
    Track,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HomeStep {
    Radial,
    Theta,
}

/// Endstop switch states, read by the caller each update.
#[derive(Clone, Copy, Debug, Default)]
pub struct Endstops {
    pub radial: bool,
    pub theta: bool,
}

pub struct Motion<A: Axis, E: FnMut(bool)> {
    theta: A,
    radial: A,
    /// Drives the shared, active-low enable line of both drivers.
    enable: E,
    mode: Mode,
    home_step: HomeStep,
    home_start_ms: u32,
    homed: bool,
    home_failed: bool,
    hold_target: ArmPose,
    track: PatternParams,
    track_start_ms: u32,
    last_control_ms: u32,
}

fn centre(s: &Settings) -> ArmPose {
    ArmPose { r: s.center_r, theta_deg: s.center_theta_deg }
}

fn clamp_pose(p: ArmPose, s: &Settings) -> ArmPose {
    ArmPose {
        r: p.r.clamp(s.radial_min_mm, s.radial_max_mm),
        theta_deg: p.theta_deg.clamp(s.theta_min_deg, s.theta_max_deg),
    }
}

/// Velocity-mode tracking: feed-forward from the path plus a correction that
/// is proportional for small errors and time-optimal (sqrt) for large ones.
fn drive_axis(m: &mut impl Axis, target_steps: f32, ff_hz: f32, vmax_hz: f32, accel: f32) {
    let err = target_steps - m.position() as f32;
    if err.abs() <= DEADBAND && ff_hz.abs() < MIN_HZ {
        if m.is_running() {
            m.stop();
        }
        return;
    }
    let corr = (accel * err.abs()).sqrt().min(KP * err.abs()).copysign(err);
    let v = (ff_hz + corr).clamp(-vmax_hz, vmax_hz);
    if v.abs() < MIN_HZ {
        if m.is_running() {
            m.stop();
        }
        return;
    }
    m.run_at(v);
}

impl<A: Axis, E: FnMut(bool)> Motion<A, E> {
    pub fn new(theta: A, radial: A, enable: E, s: &Settings) -> Self {
        let mut m = Motion {
            theta,
            radial,
            enable,
            mode: Mode::Released,
            home_step: HomeStep::Radial,
            home_start_ms: 0,
            homed: false,
            home_failed: false,
            hold_target: ArmPose::default(),
            track: PatternParams::default(),
            track_start_ms: 0,
            last_control_ms: 0,
        };
        m.theta.set_acceleration(THETA_ACCEL_DEG_S2 * s.theta_steps_per_deg);
        m.radial.set_acceleration(RADIAL_ACCEL_MM_S2 * s.radial_steps_per_mm);
        (m.enable)(false);
        m
    }

    pub fn pose(&self, s: &Settings) -> ArmPose {
        ArmPose {
            r: self.radial.position() as f32 / s.radial_steps_per_mm,
            theta_deg: self.theta.position() as f32 / s.theta_steps_per_deg,
        }
    }

    fn control(&mut self, now: u32, s: &Settings) {
        let mut tgt = self.hold_target;
        let (mut ff_r, mut ff_t) = (0.0, 0.0);
        if self.mode == Mode::Track {
            let dt = CONTROL_PERIOD_MS as f32 / 1000.0;
            let t = now.wrapping_sub(self.track_start_ms) as f32 / 1000.0;
            let c = centre(s);
            tgt = clamp_pose(bed_to_arm(pattern_at(&self.track, t), c), s);
            let next = clamp_pose(bed_to_arm(pattern_at(&self.track, t + dt), c), s);
            ff_r = (next.r - tgt.r) / dt * s.radial_steps_per_mm;
            ff_t = (next.theta_deg - tgt.theta_deg) / dt * s.theta_steps_per_deg;
        }
        drive_axis(
            &mut self.radial,
            tgt.r * s.radial_steps_per_mm,
            ff_r,
            RADIAL_MAX_MM_S * s.radial_steps_per_mm,
            RADIAL_ACCEL_MM_S2 * s.radial_steps_per_mm,
        );
        drive_axis(
            &mut self.theta,
            tgt.theta_deg * s.theta_steps_per_deg,
            ff_t,
            THETA_MAX_DEG_S * s.theta_steps_per_deg,
            THETA_ACCEL_DEG_S2 * s.theta_steps_per_deg,
        );
    }

    fn homing_step(&mut self, now: u32, s: &Settings, end: Endstops) {
        if now.wrapping_sub(self.home_start_ms) > HOME_TIMEOUT_MS {
            self.theta.force_stop_at(0);
            self.radial.force_stop_at(0);
            self.home_failed = true;
            self.release();
            return;
        }
        match self.home_step {
            HomeStep::Radial => {
                if end.radial {
                    self.radial.force_stop_at((s.radial_home_mm * s.radial_steps_per_mm).round() as i32);
                    self.home_step = HomeStep::Theta;
                    self.theta.run_at(-HOME_DEG_S * s.theta_steps_per_deg);
                }
            }
            HomeStep::Theta => {
                if end.theta {
                    self.theta.force_stop_at((s.theta_home_deg * s.theta_steps_per_deg).round() as i32);
                    self.homed = true;
                    self.park(s);
                }
            }
        }
    }

    /// Call as often as possible: homing polls the switches here for a tight
    /// stop, and tracking runs every CONTROL_PERIOD_MS.
    pub fn update(&mut self, now: u32, s: &Settings, end: Endstops) {
        match self.mode {
            Mode::Homing => self.homing_step(now, s, end),
            Mode::Hold | Mode::Track => {
                if now.wrapping_sub(self.last_control_ms) >= CONTROL_PERIOD_MS {
                    self.last_control_ms = now;
                    self.control(now, s);
                }
            }
            Mode::Released => {}
        }
    }

    pub fn home(&mut self, now: u32, s: &Settings) {
        self.homed = false;
        self.home_failed = false;
        (self.enable)(true);
        self.mode = Mode::Homing;
        self.home_step = HomeStep::Radial;
        self.home_start_ms = now;
        self.radial.run_at(-HOME_MM_S * s.radial_steps_per_mm);
    }

    pub fn homed(&self) -> bool {
        self.homed
    }

    pub fn homing_failed(&self) -> bool {
        self.home_failed
    }

    /// Homing, or still travelling to a fixed target.
    pub fn busy(&self, s: &Settings) -> bool {
        match self.mode {
            Mode::Homing => true,
            Mode::Hold => {
                let er = (self.hold_target.r * s.radial_steps_per_mm - self.radial.position() as f32).abs();
                let et =
                    (self.hold_target.theta_deg * s.theta_steps_per_deg - self.theta.position() as f32).abs();
                er > DEADBAND * 2.0 || et > DEADBAND * 2.0 || self.radial.is_running() || self.theta.is_running()
            }
            _ => false,
        }
    }

    pub fn move_to(&mut self, target: ArmPose, s: &Settings) {
        if !self.homed {
            return;
        }
        self.hold_target = clamp_pose(target, s);
        self.mode = Mode::Hold;
    }

    /// Relative to the dripper centre.
    pub fn move_to_bed(&mut self, p: BedPoint, s: &Settings) {
        self.move_to(bed_to_arm(p, centre(s)), s);
    }

    pub fn park(&mut self, s: &Settings) {
        self.move_to(ArmPose { r: s.park_r, theta_deg: s.park_theta_deg }, s);
    }

    /// Starts the pattern at t = 0 now.
    pub fn follow(&mut self, p: PatternParams, now: u32) {
        if !self.homed {
            return;
        }
        self.track = p;
        self.track_start_ms = now;
        self.mode = Mode::Track;
    }

    /// Stay where we are.
    pub fn hold(&mut self, s: &Settings) {
        if !self.homed {
            return;
        }
        self.hold_target = clamp_pose(self.pose(s), s);
        self.mode = Mode::Hold;
    }

    fn target_or_pose(&self, s: &Settings) -> ArmPose {
        if self.mode == Mode::Hold {
            self.hold_target
        } else {
            self.pose(s)
        }
    }

    pub fn jog(&mut self, dr: f32, dtheta_deg: f32, s: &Settings) {
        if !self.homed {
            return;
        }
        let base = self.target_or_pose(s);
        self.move_to(ArmPose { r: base.r + dr, theta_deg: base.theta_deg + dtheta_deg }, s);
    }

    /// The pose to store as the dripper centre.
    pub fn centre_here(&self, s: &Settings) -> ArmPose {
        self.target_or_pose(s)
    }

    /// De-energises the motors (position is lost).
    pub fn release(&mut self) {
        self.theta.stop();
        self.radial.stop();
        (self.enable)(false);
        self.homed = false;
        self.mode = Mode::Released;
    }

    pub fn mode_name(&self, s: &Settings) -> &'static str {
        match self.mode {
            Mode::Homing => "homing",
            Mode::Hold if self.busy(s) => "moving",
            Mode::Hold => "holding",
            Mode::Track => "pouring",
            Mode::Released => "released",
        }
    }
}

#[cfg(test)]
pub(crate) mod sim {
    use super::Axis;

    /// An ideal axis: integrates speed with acceleration limits, 1 ms steps.
    #[derive(Default)]
    pub struct SimAxis {
        pub pos: f64,
        pub v: f32,
        pub target_v: f32,
        pub accel: f32,
    }

    impl SimAxis {
        pub fn tick(&mut self, dt: f32) {
            let dv = self.accel * dt;
            self.v += (self.target_v - self.v).clamp(-dv, dv);
            self.pos += (self.v * dt) as f64;
        }
    }

    impl Axis for SimAxis {
        fn position(&self) -> i32 {
            self.pos.round() as i32
        }
        fn is_running(&self) -> bool {
            self.v != 0.0
        }
        fn run_at(&mut self, hz: f32) {
            self.target_v = hz;
        }
        fn stop(&mut self) {
            self.target_v = 0.0;
        }
        fn force_stop_at(&mut self, position: i32) {
            self.v = 0.0;
            self.target_v = 0.0;
            self.pos = position as f64;
        }
        fn set_acceleration(&mut self, a: f32) {
            self.accel = a;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sim::SimAxis;
    use super::*;
    use crate::pattern::Pattern;

    type M = Motion<SimAxis, fn(bool)>;

    fn run(m: &mut M, s: &Settings, now: &mut u32, ms: u32, end: impl Fn(&M) -> Endstops) {
        for _ in 0..ms {
            *now += 1;
            let e = end(m);
            m.update(*now, s, e);
            m.theta.tick(0.001);
            m.radial.tick(0.001);
        }
    }

    /// Switches close when the axes reach their home positions (counted from
    /// a start pose that is not known to the firmware).
    fn switches(s: &Settings) -> impl Fn(&M) -> Endstops {
        let r_home = s.radial_home_mm * s.radial_steps_per_mm;
        let t_home = s.theta_home_deg * s.theta_steps_per_deg;
        move |m: &M| Endstops {
            radial: (m.radial.pos as f32) <= r_home,
            theta: (m.theta.pos as f32) <= t_home,
        }
    }

    fn homed_motion(s: &Settings, now: &mut u32) -> M {
        let mut m: M = Motion::new(SimAxis::default(), SimAxis::default(), |_| {}, s);
        m.radial.pos = (90.0 * s.radial_steps_per_mm) as f64;
        m.theta.pos = (-50.0 * s.theta_steps_per_deg) as f64;
        m.home(*now, s);
        run(&mut m, s, now, 5000, switches(s));
        assert!(m.homed() && !m.homing_failed());
        run(&mut m, s, now, 3000, |_| Endstops::default());
        m
    }

    #[test]
    fn homes_and_parks() {
        let s = Settings::default();
        let mut now = 0;
        let m = homed_motion(&s, &mut now);
        assert!(!m.busy(&s));
        let p = m.pose(&s);
        assert!((p.r - s.park_r).abs() < 0.1, "{p:?}");
        assert!((p.theta_deg - s.park_theta_deg).abs() < 0.3, "{p:?}");
        assert_eq!(m.mode_name(&s), "holding");
    }

    #[test]
    fn homing_times_out_without_switches() {
        let s = Settings::default();
        let mut now = 0;
        let mut m: M = Motion::new(SimAxis::default(), SimAxis::default(), |_| {}, &s);
        m.home(now, &s);
        run(&mut m, &s, &mut now, 21_000, |_| Endstops::default());
        assert!(m.homing_failed() && !m.homed());
        assert_eq!(m.mode_name(&s), "released");
    }

    #[test]
    fn tracks_a_circle() {
        let s = Settings::default();
        let mut now = 0;
        let mut m = homed_motion(&s, &mut now);
        m.move_to_bed(BedPoint { rho: 25.0, phi: 0.0 }, &s);
        run(&mut m, &s, &mut now, 2000, |_| Endstops::default());
        let p = PatternParams { kind: Pattern::Circle, radius_mm: 25.0, rps: 0.5 };
        m.follow(p, now);
        let mut worst: f32 = 0.0;
        for _ in 0..400 {
            run(&mut m, &s, &mut now, 10, |_| Endstops::default());
            let b = crate::kinematics::arm_to_bed(m.pose(&s), centre(&s));
            worst = worst.max((b.rho - 25.0).abs());
        }
        assert!(worst < 1.0, "nozzle strayed {worst} mm from the circle");
    }

    #[test]
    fn moves_are_clamped_to_limits() {
        let s = Settings::default();
        let mut now = 0;
        let mut m = homed_motion(&s, &mut now);
        m.jog(500.0, 500.0, &s);
        run(&mut m, &s, &mut now, 4000, |_| Endstops::default());
        let p = m.pose(&s);
        assert!(p.r <= s.radial_max_mm + 0.1 && p.theta_deg <= s.theta_max_deg + 0.2, "{p:?}");
    }
}
