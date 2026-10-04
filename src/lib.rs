//! Reusable building blocks for the observable, networked Bevy demonstration.
//!
//! The binary target is intentionally only a composition root. Application
//! behavior lives in small feature modules so examples, tests, and benchmarks
//! all exercise the same production code.

pub mod cli;
pub mod components;
pub mod config;
pub mod errors;
pub mod inputs;
pub mod inspection;
pub mod network;
pub mod plugin;
pub mod presentation;
pub mod simulation;
pub mod systems;

pub use cli::Cli;
pub use config::Config;
pub use errors::ApplicationError;
pub use plugin::TemplateBevyPlugin;
