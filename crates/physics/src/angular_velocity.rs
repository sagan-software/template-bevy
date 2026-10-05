//! Angular motion around the gameplay plane's normal.

use bevy::prelude::*;
use serde::Deserialize;
use serde::Serialize;

/// Angular velocity around the gameplay plane normal, measured in radians per
/// second. Attach this component when simulation should retain and replicate
/// rotational motion.
#[derive(Clone, Copy, Component, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
#[type_path = "game_physics"]
pub struct AngularVelocity(
    /// Signed angular speed around the positive Z axis in radians per second.
    /// Positive values rotate counterclockwise in the XY plane; zero retains
    /// the current orientation.
    pub f32,
);
