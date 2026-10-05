//! Reflected physics components remain usable through BRP inspection.

use bevy::ecs::reflect::ReflectComponent;
use bevy::prelude::Vec2;
use bevy::reflect::FromReflect;
use bevy::reflect::Reflect;
use bevy::reflect::TypeRegistry;
use game_physics::AngularVelocity;
use game_physics::LinearVelocity;
use game_physics::Position;
use game_physics::Rotation;
use std::any::TypeId;

/// Registers the component capability required by strict world queries.
#[test]
fn physics_components_support_reflected_world_access() {
    let mut registry = TypeRegistry::default();
    registry.register::<Position>();
    registry.register::<Rotation>();
    registry.register::<LinearVelocity>();
    registry.register::<AngularVelocity>();
    for (identity, path) in [
        (TypeId::of::<Position>(), "game_physics::Position"),
        (TypeId::of::<Rotation>(), "game_physics::Rotation"),
        (
            TypeId::of::<LinearVelocity>(),
            "game_physics::LinearVelocity",
        ),
        (
            TypeId::of::<AngularVelocity>(),
            "game_physics::AngularVelocity",
        ),
    ] {
        let registration = registry.get(identity).expect("component is registered");
        assert!(registration.data::<ReflectComponent>().is_some());
        assert_eq!(registration.type_info().type_path(), path);
    }
}

/// Preserves component values when inspector edits and replication clone them.
#[test]
fn reflected_physics_values_preserve_their_component_types() {
    let values: [Box<dyn Reflect>; 4] = [
        Box::new(Position(Vec2::new(3.0, -4.0))),
        Box::new(Rotation::radians(0.5)),
        Box::new(LinearVelocity(Vec2::new(-2.0, 7.0))),
        Box::new(AngularVelocity(0.25)),
    ];
    for value in values {
        let mut cloned = value.reflect_clone().expect("component must clone");
        cloned
            .try_apply(value.as_partial_reflect())
            .expect("same type must apply");
        assert_eq!(
            cloned.reflect_partial_eq(value.as_partial_reflect()),
            Some(true)
        );
        cloned
            .set(value.reflect_clone().expect("component must clone"))
            .expect("same type must set");
    }
    assert_eq!(
        Position::from_reflect(&Position(Vec2::X)),
        Some(Position(Vec2::X))
    );
    assert_eq!(
        Rotation::from_reflect(&Rotation::default()),
        Some(Rotation::default())
    );
    assert_eq!(
        LinearVelocity::from_reflect(&LinearVelocity(Vec2::Y)),
        Some(LinearVelocity(Vec2::Y))
    );
    assert_eq!(
        AngularVelocity::from_reflect(&AngularVelocity(2.0)),
        Some(AngularVelocity(2.0))
    );
}
