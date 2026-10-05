//! Planar position shared by rendering and replication.

use bevy::prelude::*;
use serde::Deserialize;
use serde::Serialize;

/// Position in world units on the XY gameplay plane. This component requires a
/// transform and remains the source copied into Rapier before each fixed step.
#[derive(Clone, Copy, Component, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
#[type_path = "game_physics"]
#[require(Transform)]
pub struct Position(
    /// Planar world coordinate ordered as X then Y. Physics writes those axes
    /// into a transform with zero Z translation; Lightyear can transmit
    /// position corrections.
    pub Vec2,
);

#[cfg(feature = "networked")]
impl lightyear::prelude::Diffable<Vec2> for Position {
    /// Returns the neutral correction baseline.
    fn base_value() -> Self {
        Self(Vec2::ZERO)
    }
    /// Computes the correction from this value to its replacement.
    fn diff(&self, new: &Self) -> Vec2 {
        new.0 - self.0
    }
    /// Applies a correction to retained simulation state.
    fn apply_diff(&mut self, delta: &Vec2) {
        self.0 += *delta;
    }
}
