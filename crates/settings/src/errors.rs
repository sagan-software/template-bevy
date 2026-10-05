//! Source-preserving failures that can occur before the Bevy schedules run.

use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// Reports startup failures from the executable boundary. Configuration errors
/// preserve their concrete causes so callers can distinguish unreadable files
/// from invalid settings.
#[derive(Debug, Error)]
pub enum ApplicationError {
    /// The selected configuration could not be read or validated before
    /// application startup. The wrapped error retains its path, category, and
    /// original cause.
    #[error(transparent)]
    Config(
        /// Original configuration failure returned during startup. Inspect this
        /// value for the requested path and its filesystem or TOML cause
        /// without parsing diagnostic text.
        #[from]
        ConfigError,
    ),
}

/// Describes a failure while loading an explicitly selected configuration file.
/// Read and parse variants retain the requested path and their respective
/// source errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The explicitly requested file could not be read as text before parsing.
    /// Inspect the source error for filesystem failures or invalid UTF-8.
    #[error("failed to read configuration file {path}: {source}")]
    Read {
        /// Path the user explicitly selected for configuration loading. It is
        /// retained as supplied so diagnostics identify the attempted file
        /// rather than another default.
        path: PathBuf,
        /// Original error returned while reading the selected file. Its kind
        /// distinguishes filesystem failures, and its diagnostic text remains
        /// available through the error source.
        source: io::Error,
    },

    /// The selected file was readable but did not contain valid configuration.
    /// The source records TOML syntax, unknown fields, incorrect types, or
    /// semantic validation failures.
    #[error("failed to parse configuration file {path}: {source}")]
    Parse {
        /// Path of the readable configuration file whose contents failed
        /// validation. Diagnostics retain this location alongside the parser
        /// cause rather than reporting a default file.
        path: PathBuf,
        /// Original TOML deserialization failure containing syntax or
        /// field-level context. Callers can inspect its concrete type through
        /// the source chain while retaining the selected path.
        source: toml::de::Error,
    },
}

/// Explains a rejected numeric simulation frequency before schedule
/// construction.
///
/// Zero and excessive values have separate variants; excessive values retain
/// the requested and maximum hertz.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TickRateError {
    /// Zero hertz cannot produce a fixed-tick period. Validation rejects this
    /// value before callers can configure a simulation schedule or derive its
    /// duration.
    #[error("tick rate must be greater than zero")]
    Zero,

    /// The requested whole-hertz frequency exceeds the supported maximum. The
    /// payload retains both numbers so callers can report or inspect the failed
    /// bound directly.
    #[error("tick rate {value} Hz exceeds the maximum of {maximum} Hz")]
    TooHigh {
        /// Rejected whole-hertz frequency supplied to validation. This value
        /// exceeds the accompanying maximum and remains available for
        /// diagnostics without converting it to a string.
        value: u32,
        /// Largest accepted whole-hertz frequency at the validation boundary.
        /// The accompanying requested value exceeded this bound, which
        /// currently equals the rate type maximum.
        maximum: u32,
    },
}

/// Explains a rejected snapshot delay expressed in whole milliseconds.
///
/// Zero and excessive delays have separate variants; excessive delays retain
/// the requested and maximum milliseconds.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ReplicationIntervalError {
    /// Zero milliseconds does not define a positive snapshot delay. Validation
    /// rejects this value before the networking plugin can configure its
    /// authoritative replication interval.
    #[error("replication interval must be greater than zero milliseconds")]
    Zero,

    /// The requested whole-millisecond snapshot delay exceeds the supported
    /// maximum. Both numbers remain in the payload so diagnostics and callers
    /// can inspect the failed bound.
    #[error("replication interval {value} ms exceeds the maximum of {maximum} ms")]
    TooHigh {
        /// Rejected snapshot delay supplied to validation in whole
        /// milliseconds. This value exceeds the accompanying maximum and
        /// remains numeric for diagnostics or caller inspection.
        value: u64,
        /// Largest accepted snapshot delay in whole milliseconds at the
        /// validation boundary. The requested value exceeded this bound, which
        /// currently equals the interval type maximum.
        maximum: u64,
    },
}

/// Describes failures while parsing unit-bearing configuration values.
///
/// Categories distinguish lexical grammar, numeric bounds, millisecond
/// precision, and conversion overflow before a validated value is constructed.
#[derive(Debug, Error)]
pub enum UnitParseError {
    /// The tick rate lacks an ASCII whole integer followed by one space and
    /// `Hz`, or its integer exceeds the parser representation.
    #[error("tick rate must use an integer followed by Hz, such as '60 Hz'")]
    TickSyntax,
    /// The tick-rate spelling contains a representable integer and the required
    /// unit, but its frequency falls outside the one-to-one-thousand-hertz
    /// range accepted by simulation.
    #[error(transparent)]
    TickRate(
        /// Numeric frequency validation cause retained after successful lexical
        /// parsing. Inspect its zero or excessive-value variant to distinguish
        /// the rejected bound without parsing text.
        TickRateError,
    ),
    /// Humantime rejected the duration spelling before precision or interval
    /// bounds were checked. The payload preserves the parser cause for
    /// diagnostics and caller inspection.
    #[error(transparent)]
    Duration(
        /// Original humantime parser error returned for the supplied duration
        /// spelling. It remains available through the error source chain
        /// instead of becoming a generic bound failure.
        humantime::DurationError,
    ),
    /// The duration grammar and whole-millisecond precision are valid, but the
    /// snapshot delay falls outside the one-millisecond-to-sixty-second range
    /// enforced before network configuration can begin.
    #[error(transparent)]
    ReplicationInterval(
        /// Numeric snapshot-delay validation cause retained after duration
        /// parsing and precision checks.
        ///
        /// Inspect its zero or excessive-delay variant to distinguish the
        /// rejected interval bound.
        ReplicationIntervalError,
    ),
    /// The parsed duration contains a fraction of a millisecond. This profile
    /// rejects that precision instead of rounding or truncating the requested
    /// snapshot delay.
    #[error("replication interval must use whole milliseconds")]
    Precision,
    /// The parsed whole-millisecond duration cannot fit the numeric
    /// representation accepted by the interval constructor.
    ///
    /// Validation reports overflow before checking the supported snapshot-delay
    /// bound.
    #[error("replication interval exceeds the supported range")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::ApplicationError;
    use super::ConfigError;
    use super::ReplicationIntervalError;
    use super::TickRateError;
    use std::error::Error as StdError;
    use std::io;
    use std::path::PathBuf;

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
