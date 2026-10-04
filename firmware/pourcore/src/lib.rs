//! OpenPour logic that does not touch hardware: kinematics, pour patterns,
//! flow control, flow-meter counting, motion control, the brew state machine,
//! command handling, the log ring and telemetry recording. The ESP32 crate
//! supplies the devices behind [`machine::Machine`] and [`motion::Axis`];
//! everything here is unit tested on the host with `cargo test`.

pub mod app;
pub mod brew;
pub mod flow;
pub mod kinematics;
pub mod logbuf;
pub mod machine;
pub mod meter;
pub mod motion;
pub mod nozzle;
pub mod pattern;
pub mod recipes;
pub mod settings;
pub mod telemetry;

#[cfg(test)]
mod fake;
