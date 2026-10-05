//! Validated simulation frequency and its unit-bearing text boundary.

use crate::errors::TickRateError;
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

/// A validated frequency between one and one thousand whole hertz. Construct it
/// numerically or parse an explicit unit-bearing string before configuring
/// simulation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect)]
#[reflect(opaque, Serialize, Deserialize)]
pub struct TickRate(
    /// Validated frequency expressed in whole hertz.
    u32,
);

impl TickRate {
    /// Default frequency of sixty fixed ticks per second. Both Lightyear and
    /// Rapier derive their simulation periods from this value unless
    /// configuration overrides it.
    pub const DEFAULT_HZ: u32 = 60;

    /// Highest accepted frequency of one thousand whole hertz. Larger numeric
    /// or textual values fail validation before they can configure an
    /// application schedule.
    pub const MAX_HZ: u32 = 1_000;

    /// Validates a fixed-simulation frequency expressed in whole hertz.
    /// Accepted values produce a rate whose period can configure both
    /// networking and physics schedules.
    ///
    /// # Errors
    ///
    /// Returns [`TickRateError`] when `hertz` is zero or exceeds
    /// [`Self::MAX_HZ`].
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

    /// Returns the validated whole number of ticks per second. The value
    /// remains between one and one thousand hertz and excludes the textual unit
    /// suffix.
    #[must_use]
    pub const fn hertz(self) -> u32 {
        self.0
    }

    /// Converts whole hertz into the duration of one fixed tick. The numerator
    /// represents one second; division by ticks per second yields seconds per
    /// tick.
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
    /// Serializes the semantic rate as its canonical unit-bearing hertz string.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Read the validated semantic value before emitting its text representation.
        let hertz = self.0;
        // Emit one explicit unit so configuration round trips cannot lose its meaning.
        serializer.serialize_str(&format!("{hertz} Hz"))
    }
}

impl<'de> Deserialize<'de> for TickRate {
    /// Deserializes and validates one unit-bearing hertz string.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

impl FromStr for TickRate {
    /// Keeps lexical failures separate from validated frequency bounds.
    type Err = UnitParseError;

    /// Accepts ASCII integer hertz followed by one space and `Hz`.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Keep the accepted spelling explicit instead of guessing absent units.
        let digits = value
            .strip_suffix(" Hz")
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
            .ok_or(UnitParseError::TickSyntax)?;
        let hertz = digits
            .parse::<u32>()
            .map_err(|_integer_error| UnitParseError::TickSyntax)?;
        Self::new(hertz).map_err(UnitParseError::TickRate)
    }
}
