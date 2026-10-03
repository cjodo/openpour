//! Pour patterns as a function of time, in bed (dripper-centred) coordinates.

use crate::kinematics::{BedPoint, PI};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pattern {
    #[default]
    Center,
    Circle,
    Spiral,
}

impl Pattern {
    /// Unknown names fall back to `Center`, like the app expects.
    pub fn parse(s: &str) -> Pattern {
        match s {
            "circle" => Pattern::Circle,
            "spiral" => Pattern::Spiral,
            _ => Pattern::Center,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Pattern::Center => "center",
            Pattern::Circle => "circle",
            Pattern::Spiral => "spiral",
        }
    }
}

/// Revolutions taken to sweep from the centre out to the full radius (and the
/// same again to come back in).
pub const SPIRAL_TURNS: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatternParams {
    pub kind: Pattern,
    pub radius_mm: f32,
    /// Revolutions per second around the dripper centre.
    pub rps: f32,
}

impl Default for PatternParams {
    fn default() -> Self {
        PatternParams { kind: Pattern::Center, radius_mm: 0.0, rps: 1.0 }
    }
}

pub fn pattern_at(p: &PatternParams, t: f32) -> BedPoint {
    let phi = 2.0 * PI * p.rps * t;
    match p.kind {
        Pattern::Circle => BedPoint { rho: p.radius_mm, phi },
        Pattern::Spiral => {
            let sweep = SPIRAL_TURNS / p.rps.max(0.05);
            let u = (t % (2.0 * sweep)) / sweep; // 0..2
            let frac = if u <= 1.0 { u } else { 2.0 - u }; // out, then back in
            BedPoint { rho: p.radius_mm * frac, phi }
        }
        Pattern::Center => BedPoint::CENTRE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32, tol: f32) {
        assert!((a - b).abs() <= tol, "{a} != {b} (±{tol})");
    }

    #[test]
    fn circle_keeps_radius() {
        let p = PatternParams { kind: Pattern::Circle, radius_mm: 25.0, rps: 1.0 };
        for i in 0..30 {
            near(pattern_at(&p, i as f32 * 0.1).rho, 25.0, 1e-4);
        }
    }

    #[test]
    fn spiral_sweeps_out_and_back() {
        let p = PatternParams { kind: Pattern::Spiral, radius_mm: 30.0, rps: 1.0 };
        near(pattern_at(&p, 0.0).rho, 0.0, 1e-4);
        near(pattern_at(&p, SPIRAL_TURNS).rho, 30.0, 1e-3);
        near(pattern_at(&p, 2.0 * SPIRAL_TURNS).rho, 0.0, 1e-3);
        for i in 0..400 {
            let rho = pattern_at(&p, i as f32 * 0.05).rho;
            assert!((-1e-4..=30.0 + 1e-4).contains(&rho));
        }
    }

    #[test]
    fn center_pattern_is_still() {
        let p = PatternParams { kind: Pattern::Center, radius_mm: 30.0, rps: 1.0 };
        near(pattern_at(&p, 7.3).rho, 0.0, 1e-6);
    }

    #[test]
    fn parse_pattern() {
        assert_eq!(Pattern::parse("spiral"), Pattern::Spiral);
        assert_eq!(Pattern::parse("circle"), Pattern::Circle);
        assert_eq!(Pattern::parse("nonsense"), Pattern::Center);
        assert_eq!(Pattern::parse(""), Pattern::Center);
    }
}
