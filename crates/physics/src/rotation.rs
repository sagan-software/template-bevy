//! Planar orientation using Bevy's rotation type.

use bevy::prelude::*;
use serde::Deserialize;
use serde::Serialize;

/// Orientation around the gameplay plane normal, represented by Bevy's planar
/// rotation type. Physics derives a Z-axis quaternion from this value before
/// each fixed step.
#[derive(Clone, Copy, Component, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
#[type_path = "game_physics"]
pub struct Rotation(
    /// Planar unit rotation represented by Bevy, with angles measured in
    /// radians. Retain this domain value for physics and networking instead of
    /// storing independent Euler angles.
    pub Rot2,
);

impl Rotation {
    /// Constructs a planar orientation from an angle measured in radians. Bevy
    /// computes its unit rotation; physics uses the result around the positive
    /// Z axis.
    #[must_use]
    pub fn radians(angle: f32) -> Self {
        Self(Rot2::radians(angle))
    }

    /// Returns the shortest signed angle from this orientation to another,
    /// measured in radians. Network corrections use this difference to preserve
    /// rotations across angle wrapping.
    #[must_use]
    pub fn angle_between(self, other: Self) -> f32 {
        self.0.angle_to(other.0)
    }
}

#[cfg(feature = "networked")]
impl lightyear::prelude::Diffable<Rot2> for Rotation {
    /// Returns the neutral correction baseline.
    fn base_value() -> Self {
        Self(Rot2::IDENTITY)
    }
    /// Computes the correction from this value to its replacement.
    fn diff(&self, new: &Self) -> Rot2 {
        new.0 * self.0.inverse()
    }
    /// Applies a correction to retained simulation state.
    fn apply_diff(&mut self, delta: &Rot2) {
        self.0 = *delta * self.0;
    }
}
