//! Validated startup settings shared by game plugins and the executable.
//!
//! Load TOML through `Config` before creating the application. `TickRate` stores
//! bounded whole hertz; `ReplicationInterval` stores a bounded `Duration`.
//! Both values accept explicit units at text boundaries and serialize canonical
//! strings. Error types distinguish filesystem, syntax, precision, and bounds failures.

mod config;
mod errors;
mod replication_interval;
mod tick_rate;

pub use self::config::Config;
pub use self::errors::ApplicationError;
pub use self::errors::ConfigError;
pub use self::errors::ReplicationIntervalError;
pub use self::errors::TickRateError;
pub use self::errors::UnitParseError;
pub use self::replication_interval::ReplicationInterval;
pub use self::tick_rate::TickRate;
