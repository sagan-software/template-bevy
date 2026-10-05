//! Validated settings shared by the rendered client and headless authority.
//!
//! TOML exposes explicit human-scale units while semantic Rust types prevent
//! systems from confusing hertz, milliseconds, and durations.

use crate::ReplicationInterval;
use crate::TickRate;
use crate::errors::ConfigError;
use bevy::ecs::reflect::ReflectResource;
use bevy::prelude::Reflect;
use bevy::prelude::Resource;
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::path::Path;

/// Validated runtime settings shared by the client and authority. Load explicit
/// TOML before startup, or use defaults for sixty hertz and hundred-millisecond
/// replication.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Reflect, Resource, Serialize)]
#[reflect(Resource)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Fixed simulation frequency serialized as a unit-bearing `tick_rate`
    /// string.
    tick_rate: TickRate,

    /// Snapshot cadence serialized as a unit-bearing `replication_interval`
    /// string.
    replication_interval: ReplicationInterval,
}

impl Config {
    /// Loads an explicitly selected TOML file or returns built-in defaults.
    ///
    /// No filesystem access occurs when `path` is [`None`]. An explicit path is
    /// never allowed to fail silently because doing so would hide user typos.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Read`] when the selected file cannot be read and
    /// [`ConfigError::Parse`] when its contents are invalid.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let Some(path) = path else {
            return Ok(Self::default());
        };

        // Attach the path to I/O errors so startup diagnostics name the file.
        let source = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        // Deserialize after reading the entire file so Serde can report the
        // precise unknown, malformed, or out-of-range field.
        Self::from_toml_str(&source).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Parses configuration from a TOML document without filesystem context.
    ///
    /// # Errors
    ///
    /// Returns TOML's structured deserialization error for malformed syntax,
    /// unknown fields, wrong value types, or semantic newtype validation.
    pub fn from_toml_str(source: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(source)
    }

    /// Returns the validated fixed-simulation frequency shared by the client
    /// and authority. Derive the fixed-step duration through the returned rate
    /// rather than storing another setting.
    #[must_use]
    pub const fn tick_rate(&self) -> TickRate {
        self.tick_rate
    }

    /// Returns the validated delay between network snapshots. The returned
    /// value retains whole-millisecond precision and converts to the duration
    /// accepted by the networking plugin.
    #[must_use]
    pub const fn replication_interval(&self) -> ReplicationInterval {
        self.replication_interval
    }
}

/// Covers defaults, filesystem boundaries, Serde, and semantic value limits.
#[cfg(test)]
mod tests {
    use super::Config;
    use crate::ReplicationInterval;
    use crate::ReplicationIntervalError;
    use crate::TickRate;
    use crate::TickRateError;
    use crate::errors::ConfigError;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    /// Keeps the documented built-in timing values stable.
    #[test]
    fn defaults_are_sixty_hertz_and_one_hundred_milliseconds() {
        let config = Config::default();

        assert_eq!(config.tick_rate().hertz(), 60);
        assert_eq!(config.replication_interval().milliseconds(), 100);
    }

    /// Avoids filesystem access when no path was explicitly selected.
    #[test]
    fn absent_path_uses_defaults() {
        let config = Config::load(None).expect("default configuration should be valid");

        assert_eq!(config, Config::default());
    }

    /// Applies every default when the selected document is empty.
    #[test]
    fn empty_toml_uses_defaults() {
        let config = Config::from_toml_str("").expect("empty TOML should use defaults");

        assert_eq!(config, Config::default());
    }

    /// Applies defaults to fields omitted from a partial document.
    #[test]
    fn partial_toml_uses_field_defaults() {
        let config = Config::from_toml_str("tick_rate = '120 Hz'")
            .expect("partial TOML should use defaults");

        assert_eq!(config.tick_rate().hertz(), 120);
        assert_eq!(config.replication_interval().milliseconds(), 100);
    }

    /// Round-trips the simple integer units shown to users.
    #[test]
    fn complete_toml_round_trips() {
        let config =
            Config::from_toml_str("tick_rate = '144 Hz'\nreplication_interval = '50 ms'\n")
                .expect("complete TOML should parse");
        let encoded = toml::to_string(&config).expect("valid configuration should serialize");
        let decoded = Config::from_toml_str(&encoded).expect("serialized TOML should parse");

        assert_eq!(decoded, config);
        assert!(encoded.contains("144 Hz"));
        assert!(encoded.contains("50 ms"));
    }

    /// Reads the exact file selected by the caller.
    #[test]
    fn explicit_valid_file_is_loaded() {
        let directory = tempdir().expect("a temporary directory should be available");
        let path = directory.path().join("demo.toml");
        fs::write(
            &path,
            "tick_rate = '30 Hz'\nreplication_interval = '250 ms'\n",
        )
        .expect("the fixture should be writable");

        let config = Config::load(Some(&path)).expect("the fixture should load");

        assert_eq!(config.tick_rate().hertz(), 30);
        assert_eq!(config.replication_interval().milliseconds(), 250);
    }

    /// Retains the missing path and underlying I/O source.
    #[test]
    fn explicit_missing_file_is_an_error() {
        let path = Path::new("definitely-not-a-template-config.toml");
        let error = Config::load(Some(path)).expect_err("missing files must not fall back");

        assert!(matches!(error, ConfigError::Read { path: actual, .. } if actual == path));
    }

    /// Rejects an explicit directory path as unreadable configuration input.
    #[test]
    fn explicit_unreadable_path_is_an_error() {
        let directory = tempdir().expect("a temporary directory should be available");
        let path = directory.path();

        let error = Config::load(Some(path)).expect_err("a directory is not a TOML document");

        assert!(matches!(error, ConfigError::Read { path: actual, .. } if actual == path));
    }

    /// Retains the malformed path and underlying TOML source.
    #[test]
    fn malformed_file_is_an_error() {
        let directory = tempdir().expect("a temporary directory should be available");
        let path = directory.path().join("broken.toml");
        fs::write(&path, "tick_rate = [").expect("the malformed fixture should be writable");

        let error = Config::load(Some(&path)).expect_err("malformed TOML must fail");

        assert!(matches!(error, ConfigError::Parse { path: actual, .. } if actual == path));
    }

    /// Rejects misspelled settings instead of silently ignoring them.
    #[test]
    fn unknown_fields_are_rejected() {
        let error = Config::from_toml_str("tick_rates_hz = 60")
            .expect_err("unknown fields must fail loudly");

        assert!(error.to_string().contains("unknown field"));
    }

    /// Rejects a field with the wrong primitive representation.
    #[test]
    fn wrong_value_type_is_rejected() {
        let error =
            Config::from_toml_str("tick_rate = 60").expect_err("tick rate must be an integer");

        assert!(error.to_string().contains("invalid type"));
    }

    /// Routes semantic newtype failures through Serde's field diagnostics.
    #[test]
    fn semantic_value_errors_surface_through_toml() {
        let zero_tick =
            Config::from_toml_str("tick_rate = '0 Hz'").expect_err("zero hertz must be rejected");
        let excessive_interval = Config::from_toml_str("replication_interval = '60001 ms'")
            .expect_err("an excessive replication interval must be rejected");

        assert!(
            zero_tick
                .to_string()
                .contains("tick rate must be greater than zero")
        );
        assert!(
            excessive_interval
                .to_string()
                .contains("replication interval 60001 ms exceeds the maximum")
        );
    }

    /// Covers the lower and upper tick-rate validation boundaries.
    #[test]
    fn tick_rate_boundaries_are_validated() {
        assert_eq!(TickRate::new(0), Err(TickRateError::Zero));
        assert_eq!(
            TickRate::new(TickRate::MAX_HZ)
                .expect("the documented maximum should be valid")
                .hertz(),
            TickRate::MAX_HZ
        );
        assert_eq!(
            TickRate::new(TickRate::MAX_HZ + 1),
            Err(TickRateError::TooHigh {
                value: TickRate::MAX_HZ + 1,
                maximum: TickRate::MAX_HZ,
            })
        );
    }

    /// Covers the lower and upper replication-interval boundaries.
    #[test]
    fn replication_interval_boundaries_are_validated() {
        assert_eq!(
            ReplicationInterval::new(0),
            Err(ReplicationIntervalError::Zero)
        );
        assert_eq!(
            ReplicationInterval::new(ReplicationInterval::MAX_MILLISECONDS)
                .expect("the documented maximum should be valid")
                .milliseconds(),
            ReplicationInterval::MAX_MILLISECONDS
        );
        assert_eq!(
            ReplicationInterval::new(ReplicationInterval::MAX_MILLISECONDS + 1),
            Err(ReplicationIntervalError::TooHigh {
                value: ReplicationInterval::MAX_MILLISECONDS + 1,
                maximum: ReplicationInterval::MAX_MILLISECONDS,
            })
        );
    }

    /// Converts semantic units into durations without losing their meaning.
    #[test]
    fn semantic_units_convert_to_durations() {
        assert_eq!(
            TickRate::new(4)
                .expect("four hertz should be valid")
                .period()
                .as_millis(),
            250
        );
        assert_eq!(
            ReplicationInterval::new(250)
                .expect("250 milliseconds should be valid")
                .duration()
                .as_millis(),
            250
        );
    }
}
