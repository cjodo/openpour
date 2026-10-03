//! Polar-arm kinematics. Pure math, so it is unit tested on the host.
//!
//! The arm pivots about a vertical axis. The nozzle carriage slides along the
//! arm. A machine pose is therefore (r, theta):
//!   r          distance from the pivot axis to the nozzle, mm
//!   theta_deg  arm angle about the pivot, degrees
//!
//! Recipes describe pours in bed coordinates, centred on the dripper:
//!   rho  distance from the dripper centre, mm
//!   phi  angle around the dripper centre, radians
//!
//! `centre` is the machine pose that puts the nozzle over the dripper centre.
//! It is measured once with the jog/calibrate screen, which absorbs any
//! mechanical offset in the printed parts.

pub use core::f32::consts::PI;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ArmPose {
    pub r: f32,
    pub theta_deg: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BedPoint {
    pub rho: f32,
    pub phi: f32,
}

impl BedPoint {
    pub const CENTRE: BedPoint = BedPoint { rho: 0.0, phi: 0.0 };
}

pub fn bed_to_arm(p: BedPoint, centre: ArmPose) -> ArmPose {
    let x = centre.r + p.rho * p.phi.cos();
    let y = p.rho * p.phi.sin();
    ArmPose {
        r: (x * x + y * y).sqrt(),
        theta_deg: centre.theta_deg + y.atan2(x).to_degrees(),
    }
}

pub fn arm_to_bed(a: ArmPose, centre: ArmPose) -> BedPoint {
    let t = (a.theta_deg - centre.theta_deg).to_radians();
    let x = a.r * t.cos() - centre.r;
    let y = a.r * t.sin();
    BedPoint {
        rho: (x * x + y * y).sqrt(),
        phi: y.atan2(x),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CENTRE: ArmPose = ArmPose { r: 110.0, theta_deg: 5.0 };

    fn near(a: f32, b: f32, tol: f32) {
        assert!((a - b).abs() <= tol, "{a} != {b} (±{tol})");
    }

    #[test]
    fn centre_maps_to_centre_pose() {
        let a = bed_to_arm(BedPoint::CENTRE, CENTRE);
        near(a.r, CENTRE.r, 1e-4);
        near(a.theta_deg, CENTRE.theta_deg, 1e-4);
    }

    #[test]
    fn round_trip() {
        for rho in (0..=40).step_by(5).map(|r| r as f32) {
            for i in 0..=12 {
                let phi = -3.0 + 0.5 * i as f32;
                let b = arm_to_bed(bed_to_arm(BedPoint { rho, phi }, CENTRE), CENTRE);
                near(b.rho, rho, 1e-3);
                if rho > 0.0 {
                    near(b.phi, phi, 1e-3);
                }
            }
        }
    }

    #[test]
    fn radial_and_tangential_points() {
        // Straight out along the arm: only r changes.
        let out = bed_to_arm(BedPoint { rho: 20.0, phi: 0.0 }, CENTRE);
        near(out.r, 130.0, 1e-4);
        near(out.theta_deg, CENTRE.theta_deg, 1e-4);
        // Sideways: r grows a little, theta swings by atan(20/110).
        let side = bed_to_arm(BedPoint { rho: 20.0, phi: PI / 2.0 }, CENTRE);
        near(side.r, (110.0f32 * 110.0 + 400.0).sqrt(), 1e-3);
        near(side.theta_deg, CENTRE.theta_deg + 20.0f32.atan2(110.0).to_degrees(), 1e-3);
    }
}
