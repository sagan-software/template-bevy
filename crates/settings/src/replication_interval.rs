//! Bounded snapshot durations and their human-readable text boundary.

use crate::errors::ReplicationIntervalError;
use crate::errors::UnitParseError;
use bevy::prelude::Reflect;
use bevy::reflect::ReflectDeserialize;
use bevy::reflect::ReflectSerialize;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de::Error as _;
use std::str::FromStr;
use std::time::Duration;

/// A validated delay between authoritative snapshots, stored as a duration.
/// Accepted values contain whole milliseconds and remain between one
/// millisecond and sixty seconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect)]
#[reflect(opaque, Serialize, Deserialize)]
pub struct ReplicationInterval(
    /// Positive snapshot duration bounded to sixty seconds.
    Duration,
);

impl ReplicationInterval {
    /// Default delay of one hundred milliseconds between authoritative
    /// snapshots. Use this value when configuration omits the replication
    /// interval or callers request the default.
    pub const DEFAULT_MILLISECONDS: u64 = 100;

    /// Largest accepted snapshot delay, expressed as sixty thousand
    /// milliseconds. Longer numeric or textual intervals fail validation before
    /// the networking plugin receives a duration.
    pub const MAX_MILLISECONDS: u64 = 60_000;

    /// Validates a snapshot delay expressed in whole milliseconds. Accepted
    /// values become durations between one millisecond and sixty seconds
    /// without losing precision during conversion.
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
            Ok(Self(Duration::from_millis(milliseconds)))
        }
    }

    /// Returns the validated snapshot delay as a whole number of milliseconds.
    /// Construction bounds this value to sixty thousand milliseconds, so
    /// conversion cannot truncate it.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "validation bounds the duration to 60,000 milliseconds"
    )]
    pub const fn milliseconds(self) -> u64 {
        self.0.as_millis() as u64
    }

    /// Returns the stored duration expected by Lightyear when configuring
    /// replication.
    ///
    /// This conversion preserves the validated snapshot delay and performs no
    /// additional parsing or rounding.
    #[must_use]
    pub const fn duration(self) -> Duration {
        self.0
    }
}

impl Default for ReplicationInterval {
    /// Returns the documented 100 ms snapshot interval.
    fn default() -> Self {
        Self(Duration::from_millis(Self::DEFAULT_MILLISECONDS))
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
    /// Serializes the interval as its canonical unit-bearing millisecond
    /// string.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Read the validated semantic value before emitting its text representation.
        let milliseconds = self.milliseconds();
        // Emit one explicit unit so configuration round trips cannot lose its meaning.
        serializer.serialize_str(&format!("{milliseconds} ms"))
    }
}

impl<'de> Deserialize<'de> for ReplicationInterval {
    /// Deserializes and validates one unit-bearing duration string.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

impl FromStr for ReplicationInterval {
    /// Retains syntax, precision and interval-validation failures.
    type Err = UnitParseError;

    /// Accepts humantime durations with whole-millisecond precision.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Humantime 2.4 controls the input grammar; this profile bounds precision.
        let duration = humantime::parse_duration(value).map_err(UnitParseError::Duration)?;
        if duration.subsec_nanos() % 1_000_000 != 0 {
            return Err(UnitParseError::Precision);
        }
        let milliseconds = u64::try_from(duration.as_millis())
            .map_err(|_integer_error| UnitParseError::Overflow)?;
        Self::new(milliseconds).map_err(UnitParseError::ReplicationInterval)
    }
}
