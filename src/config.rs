//! Validated settings shared by the rendered client and headless authority.
//!
//! TOML exposes explicit human-scale units while semantic Rust types prevent
//! systems from confusing hertz, milliseconds, and durations.

use crate::errors::{ConfigError, ReplicationIntervalError, TickRateError};
use bevy::{
    ecs::reflect::ReflectResource,
    prelude::{Reflect, Resource},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::{fs, path::Path, time::Duration};

/// Runtime settings that are safe to insert into either Bevy world.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Reflect, Resource, Serialize)]
#[reflect(Resource)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Fixed simulation frequency serialized as `tick_rate_hz`.
    #[serde(rename = "tick_rate_hz")]
    tick_rate: TickRate,

    /// Snapshot cadence serialized as `replication_interval_ms`.
    #[serde(rename = "replication_interval_ms")]
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

    /// Returns the validated fixed-simulation frequency.
    #[must_use]
    pub const fn tick_rate(&self) -> TickRate {
        self.tick_rate
    }

    /// Returns the validated network replication cadence.
    #[must_use]
    pub const fn replication_interval(&self) -> ReplicationInterval {
        self.replication_interval
    }
}

/// A validated number of fixed simulation ticks per second.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect)]
pub struct TickRate(
    /// Validated frequency expressed in whole hertz.
    u32,
);

impl TickRate {
    /// Default frequency used by both Lightyear and Avian simulation.
    pub const DEFAULT_HZ: u32 = 60;

    /// Highest supported frequency, preventing accidental sub-millisecond churn.
    pub const MAX_HZ: u32 = 1_000;

    /// Validates a fixed-simulation frequency expressed in hertz.
    ///
    /// # Errors
    ///
    /// Returns [`TickRateError`] when `hertz` is zero or exceeds [`Self::MAX_HZ`].
    pub const fn new(hertz: u32) -> Result<Self, TickRateError> {
        if hertz == 0 {
            Err(TickRateError::Zero)
        } else if hertz > Self::MAX_HZ {
            Err(TickRateError::TooHigh {
                value: hertz,
                maximum: Self::MAX_HZ,
            })
        } else {
            Ok(Self(hertz))
        }
    }

    /// Returns the fixed-simulation frequency in hertz.
    #[must_use]
    pub const fn hertz(self) -> u32 {
        self.0
    }

    /// Converts the frequency into one fixed-tick duration.
    #[must_use]
    pub fn period(self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.0))
    }
}

impl Default for TickRate {
    /// Returns the documented 60 Hz simulation frequency.
    fn default() -> Self {
        Self(Self::DEFAULT_HZ)
    }
}

impl TryFrom<u32> for TickRate {
    /// Validation failure returned for zero or excessive frequencies.
    type Error = TickRateError;

    /// Converts user-facing hertz into a validated simulation rate.
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Serialize for TickRate {
    /// Serializes the semantic rate as its user-facing integer hertz value.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for TickRate {
    /// Deserializes and validates one user-facing integer hertz value.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hertz = u32::deserialize(deserializer)?;
        Self::try_from(hertz).map_err(D::Error::custom)
    }
}

/// A validated delay between authoritative replication snapshots.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect)]
pub struct ReplicationInterval(
    /// Validated snapshot delay expressed in whole milliseconds.
    u64,
);

impl ReplicationInterval {
    /// Default snapshot interval in milliseconds.
    pub const DEFAULT_MILLISECONDS: u64 = 100;

    /// Largest interval supported by this interactive demonstration.
    pub const MAX_MILLISECONDS: u64 = 60_000;

    /// Validates a replication interval expressed in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns [`ReplicationIntervalError`] when `milliseconds` is zero or
    /// exceeds [`Self::MAX_MILLISECONDS`].
    pub const fn new(milliseconds: u64) -> Result<Self, ReplicationIntervalError> {
        if milliseconds == 0 {
            Err(ReplicationIntervalError::Zero)
        } else if milliseconds > Self::MAX_MILLISECONDS {
            Err(ReplicationIntervalError::TooHigh {
                value: milliseconds,
                maximum: Self::MAX_MILLISECONDS,
            })
        } else {
            Ok(Self(milliseconds))
        }
    }

    /// Returns the user-facing interval in milliseconds.
    #[must_use]
    pub const fn milliseconds(self) -> u64 {
        self.0
    }

    /// Converts the interval into the duration expected by Lightyear.
    #[must_use]
    pub const fn duration(self) -> Duration {
        Duration::from_millis(self.0)
    }
}

impl Default for ReplicationInterval {
    /// Returns the documented 100 ms snapshot interval.
    fn default() -> Self {
        Self(Self::DEFAULT_MILLISECONDS)
    }
}

impl TryFrom<u64> for ReplicationInterval {
    /// Validation failure returned for zero or excessive intervals.
    type Error = ReplicationIntervalError;

    /// Converts user-facing milliseconds into a validated snapshot interval.
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Serialize for ReplicationInterval {
    /// Serializes the interval as its user-facing integer millisecond value.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for ReplicationInterval {
    /// Deserializes and validates one user-facing integer millisecond value.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let milliseconds = u64::deserialize(deserializer)?;
        Self::try_from(milliseconds).map_err(D::Error::custom)
    }
}

/// Covers defaults, filesystem boundaries, Serde, and semantic value limits.
#[cfg(all(test, not(coverage)))]
mod tests {
    use super::{Config, ReplicationInterval, TickRate};
    use crate::errors::{ConfigError, ReplicationIntervalError, TickRateError};
    use std::{fs, path::Path};
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
        let config =
            Config::from_toml_str("tick_rate_hz = 120").expect("partial TOML should use defaults");

        assert_eq!(config.tick_rate().hertz(), 120);
        assert_eq!(config.replication_interval().milliseconds(), 100);
    }

    /// Round-trips the simple integer units shown to users.
    #[test]
    fn complete_toml_round_trips() {
        let config = Config::from_toml_str("tick_rate_hz = 144\nreplication_interval_ms = 50\n")
            .expect("complete TOML should parse");
        let encoded = toml::to_string(&config).expect("valid configuration should serialize");
        let decoded = Config::from_toml_str(&encoded).expect("serialized TOML should parse");

        assert_eq!(decoded, config);
        assert!(encoded.contains("tick_rate_hz = 144"));
        assert!(encoded.contains("replication_interval_ms = 50"));
    }

    /// Reads the exact file selected by the caller.
    #[test]
    fn explicit_valid_file_is_loaded() {
        let directory = tempdir().expect("a temporary directory should be available");
        let path = directory.path().join("demo.toml");
        fs::write(&path, "tick_rate_hz = 30\nreplication_interval_ms = 250\n")
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
        fs::write(&path, "tick_rate_hz = [").expect("the malformed fixture should be writable");

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
        let error = Config::from_toml_str("tick_rate_hz = \"fast\"")
            .expect_err("tick rate must be an integer");

        assert!(error.to_string().contains("invalid type"));
    }

    /// Routes semantic newtype failures through Serde's field diagnostics.
    #[test]
    fn semantic_value_errors_surface_through_toml() {
        let zero_tick =
            Config::from_toml_str("tick_rate_hz = 0").expect_err("zero hertz must be rejected");
        let excessive_interval = Config::from_toml_str("replication_interval_ms = 60001")
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
