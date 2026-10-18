//! systima — a retro-cyberpunk terminal system monitor.
//!
//! The binary in `main.rs` is a thin shell; everything lives here so the
//! benchmarks and tests can drive the real code paths off-screen.

pub mod app;
pub mod bench;
pub mod fixtures;
pub mod format;
pub mod history;
pub mod input;
pub mod sampler;
pub mod snapshot;
pub mod theme;
pub mod ui;
