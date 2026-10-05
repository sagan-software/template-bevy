//! Rapier physics with explicit replicated planar state.
//!
//! Add `Plugin` after the application installs Bevy's transform systems.
//! `Position`, `Rotation`, and velocity components retain gameplay state while
//! ordered systems synchronize Rapier before simulation and capture its results.
//! Dimension features select the backend; networking adds Lightyear correction
//! traits without changing the planar component vocabulary.

mod angular_velocity;
mod plugin;
mod position;
mod rotation;
mod velocity;

pub use self::angular_velocity::AngularVelocity;
pub use self::plugin::PhysicsPlugin as Plugin;
pub use self::position::Position;
pub use self::rotation::Rotation;
pub use self::velocity::LinearVelocity;

#[cfg(feature = "dim2")]
#[path = "backend2d.rs"]
mod dimension;
#[cfg(all(feature = "dim3", not(feature = "dim2")))]
#[path = "backend3d.rs"]
mod dimension;
pub use self::dimension::backend;

pub use self::backend::Collider;
pub use self::backend::ColliderMassProperties;
pub use self::backend::Restitution;
pub use self::backend::RigidBody;
pub use self::plugin::PhysicsSystems;

/// Creates a segment from planar world coordinates ordered as X then Y. The 3D
/// backend appends zero Z to both endpoints without reversing their order.
#[must_use]
pub fn segment(start: bevy::prelude::Vec2, end: bevy::prelude::Vec2) -> Collider {
    dimension::segment(start, end)
}

/// Creates a box from XY half-extents measured in world units. The 3D backend
/// uses the Y half-extent as its Z half-extent; full side lengths double each
/// half-extent.
#[must_use]
pub fn rectangle(half_extent: bevy::prelude::Vec2) -> Collider {
    dimension::rectangle(half_extent)
}

/// Direct Rapier 2D capabilities for additional application-owned scenes.
#[cfg(feature = "dim2")]
pub use bevy_rapier2d as rapier2d;
/// Direct Rapier 3D capabilities for additional application-owned scenes.
#[cfg(feature = "dim3")]
pub use bevy_rapier3d as rapier3d;
