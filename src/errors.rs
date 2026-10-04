//! Source-preserving failures that can occur before the Bevy schedules run.

use std::{io, path::PathBuf};
use thiserror::Error;

/// Describes a recoverable failure while constructing the application.
#[derive(Debug, Error)]
pub enum ApplicationError {
    /// Configuration could not be read or validated.
    #[error(transparent)]
    Config(
        /// Source-preserving failure from the configuration boundary.
        #[from]
        ConfigError,
    ),
}

/// Describes a failure at the configuration-file boundary.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The explicitly requested file could not be read.
    #[error("failed to read configuration file {path}: {source}")]
    Read {
        /// Path the user asked the application to read.
        path: PathBuf,
        /// Filesystem failure returned by the standard library.
        #[source]
        source: io::Error,
    },

    /// The file contents were not valid template configuration.
    #[error("failed to parse configuration file {path}: {source}")]
    Parse {
        /// Path whose contents failed to parse.
        path: PathBuf,
        /// TOML parser failure containing the field-level context.
        #[source]
        source: toml::de::Error,
    },
}

/// Explains why a requested fixed-simulation frequency is invalid.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TickRateError {
    /// A zero-hertz simulation can never advance.
    #[error("tick rate must be greater than zero")]
    Zero,

    /// The requested frequency exceeds the documented safety limit.
    #[error("tick rate {value} Hz exceeds the maximum of {maximum} Hz")]
    TooHigh {
        /// Rejected frequency in hertz.
        value: u32,
        /// Largest frequency accepted by this template.
        maximum: u32,
    },
}

/// Explains why a requested replication interval is invalid.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReplicationIntervalError {
    /// A zero interval would request continuous replication.
    #[error("replication interval must be greater than zero milliseconds")]
    Zero,

    /// The requested delay exceeds the documented safety limit.
    #[error("replication interval {value} ms exceeds the maximum of {maximum} ms")]
    TooHigh {
        /// Rejected interval in milliseconds.
        value: u64,
        /// Largest interval accepted by this template.
        maximum: u64,
    },
}

/// Exercises error context, causal chains, and conversions.
#[cfg(all(test, not(coverage)))]
mod tests {
    use super::{ApplicationError, ConfigError, ReplicationIntervalError, TickRateError};
    use std::{error::Error as StdError, io, path::PathBuf};

    /// Includes the requested path while preserving the I/O source.
    #[test]
    fn read_error_preserves_context_and_source() {
        let error = ConfigError::Read {
            path: PathBuf::from("missing.toml"),
            source: io::Error::new(io::ErrorKind::NotFound, "fixture is absent"),
        };

        assert_eq!(
            error.to_string(),
            "failed to read configuration file missing.toml: fixture is absent"
        );
        assert_eq!(
            StdError::source(&error).map(ToString::to_string),
            Some("fixture is absent".to_owned())
        );
    }

    /// Includes the requested path while preserving the TOML source.
    #[test]
    fn parse_error_preserves_context_and_source() {
        let source = toml::from_str::<u8>("not valid TOML")
            .expect_err("the fixture should not parse as an integer");
        let error = ConfigError::Parse {
            path: PathBuf::from("broken.toml"),
            source,
        };

        assert!(
            error
                .to_string()
                .starts_with("failed to parse configuration file broken.toml:")
        );
        assert!(
            StdError::source(&error)
                .is_some_and(|source| source.downcast_ref::<toml::de::Error>().is_some())
        );
    }

    /// Converts configuration failures into the application boundary error.
    #[test]
    fn application_error_wraps_config_error_transparently() {
        let error = ApplicationError::from(ConfigError::Read {
            path: PathBuf::from("settings.toml"),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        });

        assert_eq!(
            error.to_string(),
            "failed to read configuration file settings.toml: denied"
        );
        assert!(
            StdError::source(&error)
                .is_some_and(|source| source.downcast_ref::<io::Error>().is_some())
        );
        assert!(matches!(error, ApplicationError::Config(_)));
    }

    /// Gives zero and excessive tick rates distinct, actionable messages.
    #[test]
    fn tick_rate_errors_are_specific() {
        assert_eq!(
            TickRateError::Zero.to_string(),
            "tick rate must be greater than zero"
        );
        assert_eq!(
            TickRateError::TooHigh {
                value: 1_001,
                maximum: 1_000,
            }
            .to_string(),
            "tick rate 1001 Hz exceeds the maximum of 1000 Hz"
        );
    }

    /// Gives zero and excessive replication intervals distinct messages.
    #[test]
    fn replication_interval_errors_are_specific() {
        assert_eq!(
            ReplicationIntervalError::Zero.to_string(),
            "replication interval must be greater than zero milliseconds"
        );
        assert_eq!(
            ReplicationIntervalError::TooHigh {
                value: 60_001,
                maximum: 60_000,
            }
            .to_string(),
            "replication interval 60001 ms exceeds the maximum of 60000 ms"
        );
    }
}
