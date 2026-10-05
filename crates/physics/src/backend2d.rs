//! Adapts planar gameplay state to the selected 2D Rapier representation.

use self::backend::Collider;
use self::backend::Velocity;
use bevy::prelude::Bundle;
use bevy::prelude::Vec2;
pub(crate) use bevy_rapier2d::math::Vect as PhysicsVector;
pub use bevy_rapier2d::prelude as backend;
pub(crate) use bevy_rapier2d::rapier::dynamics::IntegrationParameters;

/// Maps XY endpoints into backend coordinates without reordering them.
pub(crate) fn segment(start: Vec2, end: Vec2) -> Collider {
    Collider::segment(start, end)
}

/// Maps planar half-extents into the selected backend's box geometry.
pub(crate) fn rectangle(half_extent: Vec2) -> Collider {
    Collider::cuboid(half_extent.x, half_extent.y)
}

/// Creates backend motion while retaining the planar gameplay constraint.
pub(crate) fn velocity(linear: Vec2, angular: f32) -> impl Bundle {
    Velocity { linear, angular }
}

/// Reads rotation around the gameplay plane's Z axis after integration.
pub(crate) const fn angular_velocity(velocity: &Velocity) -> f32 {
    velocity.angular
}
