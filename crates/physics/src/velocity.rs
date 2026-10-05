//! Planar linear motion shared by simulation and replication.

use bevy::prelude::*;
use serde::Deserialize;
use serde::Serialize;

/// Linear motion measured in world units per second on the XY gameplay plane.
/// Simulation updates this component before the physics adapter installs
/// backend velocity.
#[derive(Clone, Copy, Component, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
#[type_path = "game_physics"]
pub struct LinearVelocity(
    /// Velocity ordered as X then Y, measured in world units per second. The 3D
    /// adapter appends zero Z velocity to preserve planar movement.
    pub Vec2,
);
