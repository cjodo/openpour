//! OpenPour logic that does not touch hardware: kinematics, pour patterns,
//! flow control, scale filtering, motion control, the brew state machine and
//! command handling. The ESP32 crate supplies the devices behind
//! [`machine::Machine`] and [`motion::Axis`]; everything here is unit tested
//! on the host with `cargo test`.

pub mod app;
pub mod brew;
pub mod flow;
pub mod kinematics;
pub mod machine;
pub mod motion;
pub mod pattern;
pub mod recipes;
pub mod scale;
pub mod settings;

#[cfg(test)]
mod fake;
