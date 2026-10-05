//! Headless physical movement through the plugin's external seam.

use bevy::prelude::*;
use game_physics::Plugin as PhysicsPlugin;

/// Installs the selected Rapier backend without a renderer or native window.
#[test]
fn physics_plugin_installs_in_headless_app() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugin::default()));
    app.update();
}

/// Advances a rigid body through Rapier and copies its state to replication
/// data.
#[test]
fn rapier_advances_position_without_gravity() {
    use bevy::time::TimeUpdateStrategy;
    use game_physics::AngularVelocity;
    use game_physics::LinearVelocity;
    use game_physics::Position;
    use game_physics::Rotation;
    use game_physics::backend::Collider;
    use game_physics::backend::RigidBody;
    use std::time::Duration;

    let period = Duration::from_secs_f64(1.0 / 60.0);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugin::default()));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(period));
    app.insert_resource(Time::<Fixed>::from_duration(period));
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::ball(1.0),
            Position(Vec2::ZERO),
            Rotation::default(),
            LinearVelocity(Vec2::new(100.0, 0.0)),
            AngularVelocity(0.0),
        ))
        .id();
    for _ in 0..10 {
        app.update();
    }
    let position = app
        .world()
        .get::<Position>(body)
        .expect("replication state must remain present")
        .0;
    assert!(
        position.x > 5.0,
        "Rapier did not move the body: {position:?}"
    );
    assert!(
        position.y.abs() < 0.001,
        "gravity must be disabled: {position:?}"
    );
}
