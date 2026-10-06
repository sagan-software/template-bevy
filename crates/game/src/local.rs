//! Local gameplay without a networking dependency or server.

use bevy::prelude::*;
use game_physics::Collider;
use game_physics::LinearVelocity;
use game_physics::PhysicsSystems;
use game_physics::Plugin as PhysicsPlugin;
use game_physics::Position;
use game_physics::RigidBody;
use game_physics::Rotation;
use game_settings::Config;

/// Composes the local starter scene and fixed physics.
pub(crate) struct LocalPlugin {
    /// Validated simulation timing.
    config: Config,
    /// Inspector capability selected at launch.
    is_editor_enabled: bool,
    /// Native window title supplied by the executable.
    window_title: &'static str,
}

impl LocalPlugin {
    /// Retains startup choices until composition.
    pub(crate) const fn new(
        config: Config,
        is_editor_enabled: bool,
        window_title: &'static str,
    ) -> Self {
        Self {
            config,
            is_editor_enabled,
            window_title,
        }
    }
}

impl Plugin for LocalPlugin {
    /// Adds the renderer, physics, scene, and input systems.
    fn build(&self, app: &mut App) {
        app.register_type::<Config>();
        app.add_plugins(
            DefaultPlugins
                .set(crate::logging::plugin())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: self.window_title.into(),
                        resolution: (960, 600).into(),
                        canvas: Some("#game".into()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: false,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(self.config)
        .insert_resource(Time::<Fixed>::from_duration(
            self.config.tick_rate().period(),
        ))
        .add_plugins(PhysicsPlugin::default().with_tick_rate(self.config.tick_rate()))
        .add_plugins(crate::shader_scene::ShaderScenePlugin)
        .add_systems(Startup, setup_scene)
        .add_systems(FixedUpdate, move_player.before(PhysicsSystems::Prepare));
        #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
        {
            app.add_plugins((
                bevy::remote::RemotePlugin::default(),
                bevy_brp_extras::BrpExtrasPlugin,
            ));
            if self.is_editor_enabled {
                app.add_plugins((
                    bevy_inspector_egui::bevy_egui::EguiPlugin::default(),
                    bevy_inspector_egui::quick::WorldInspectorPlugin::new(),
                ));
            }
        }
        #[cfg(not(all(feature = "mcp", not(target_family = "wasm"))))]
        let _ = self.is_editor_enabled;
    }
}

/// Identifies the local player without conflating it with network ownership.
#[derive(Component)]
struct Player;

/// Builds geometry once and chooses the dimension's camera and materials.
fn setup_scene(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    #[cfg(feature = "dim2")] mut materials: ResMut<'_, Assets<ColorMaterial>>,
    #[cfg(all(feature = "dim3", not(feature = "dim2")))] mut materials: ResMut<
        '_,
        Assets<StandardMaterial>,
    >,
) {
    let mut player = commands.spawn((
        Name::new("Player"),
        Player,
        RigidBody::Dynamic,
        Collider::ball(24.0),
        Position(Vec2::ZERO),
        Rotation::default(),
        LinearVelocity(Vec2::ZERO),
    ));
    #[cfg(feature = "dim2")]
    player.insert((
        Mesh2d(meshes.add(Circle::new(24.0))),
        MeshMaterial2d(materials.add(Color::srgb_u8(61, 214, 255))),
    ));
    #[cfg(all(feature = "dim3", not(feature = "dim2")))]
    player.insert((
        Mesh3d(meshes.add(Sphere::new(24.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(61, 214, 255))),
    ));
    #[cfg(feature = "dim2")]
    commands.spawn((
        Camera2d,
        Projection::Orthographic(crate::camera::planar_projection()),
    ));
    #[cfg(all(feature = "dim3", not(feature = "dim2")))]
    {
        commands.spawn((
            Camera3d::default(),
            // Fit XY gameplay while keeping the 3D near plane at zero world units.
            Projection::Orthographic(OrthographicProjection {
                near: 0.0,
                ..crate::camera::planar_projection()
            }),
            Transform::from_xyz(0.0, 0.0, 750.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));
        commands.spawn((
            PointLight {
                intensity: 10_000_000.0,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_xyz(150.0, 200.0, 300.0),
        ));
    }
}

/// Maps held directional keys to bounded planar velocity each fixed step.
fn move_player(
    keys: Res<'_, ButtonInput<KeyCode>>,
    mut players: Query<'_, '_, &mut LinearVelocity, With<Player>>,
) {
    let mut direction = Vec2::ZERO;
    for (key, vector) in [
        (KeyCode::KeyW, Vec2::Y),
        (KeyCode::ArrowUp, Vec2::Y),
        (KeyCode::KeyS, Vec2::NEG_Y),
        (KeyCode::ArrowDown, Vec2::NEG_Y),
        (KeyCode::KeyA, Vec2::NEG_X),
        (KeyCode::ArrowLeft, Vec2::NEG_X),
        (KeyCode::KeyD, Vec2::X),
        (KeyCode::ArrowRight, Vec2::X),
    ] {
        if keys.pressed(key) {
            direction += vector;
        }
    }
    let speed = if keys.pressed(KeyCode::Space) {
        250.0
    } else {
        145.0
    };
    for mut velocity in &mut players {
        velocity.set_if_neq(LinearVelocity(direction.normalize_or_zero() * speed));
    }
}

/// Tests local input without initializing graphics or a window.
#[cfg(test)]
mod tests {
    use super::Player;
    use super::move_player;
    use bevy::camera::CameraProjection;
    use bevy::prelude::*;
    use game_physics::LinearVelocity;

    /// Fits the local arena without requiring a graphics device or native window.
    #[test]
    fn camera_preserves_arena_at_portrait_and_landscape_sizes() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        #[cfg(feature = "dim2")]
        app.init_resource::<Assets<ColorMaterial>>();
        #[cfg(all(feature = "dim3", not(feature = "dim2")))]
        app.init_resource::<Assets<StandardMaterial>>();
        app.add_systems(Startup, super::setup_scene);
        app.update();
        let mut cameras = app.world_mut().query::<&Projection>();
        let projection = cameras.single(app.world()).expect("the camera must exist");
        let Projection::Orthographic(projection) = projection else {
            panic!("the planar arena requires a fitted orthographic projection");
        };
        for (width, height) in [(960.0, 600.0), (390.0, 844.0)] {
            let mut projection = projection.clone();
            projection.update(width, height);
            assert!(projection.area.width() >= 800.0);
            assert!(projection.area.height() >= 500.0);
        }
    }

    /// Advances the private input system with a selected held-key set.
    fn velocity_for(keys: &[KeyCode]) -> Vec2 {
        let mut app = App::new();
        let mut input = ButtonInput::<KeyCode>::default();
        for key in keys {
            input.press(*key);
        }
        app.insert_resource(input).add_systems(Update, move_player);
        let player = app
            .world_mut()
            .spawn((Player, LinearVelocity(Vec2::splat(99.0))))
            .id();
        let other = app
            .world_mut()
            .spawn(LinearVelocity(Vec2::splat(11.0)))
            .id();
        app.update();
        assert_eq!(
            app.world()
                .get::<LinearVelocity>(other)
                .expect("uncontrolled body must remain")
                .0,
            Vec2::splat(11.0)
        );
        app.world()
            .get::<LinearVelocity>(player)
            .expect("player velocity must remain")
            .0
    }

    /// Treats each advertised directional key consistently.
    #[test]
    fn movement_keys_select_the_expected_direction() {
        for (key, direction) in [
            (KeyCode::KeyW, Vec2::Y),
            (KeyCode::ArrowUp, Vec2::Y),
            (KeyCode::KeyS, Vec2::NEG_Y),
            (KeyCode::ArrowDown, Vec2::NEG_Y),
            (KeyCode::KeyA, Vec2::NEG_X),
            (KeyCode::ArrowLeft, Vec2::NEG_X),
            (KeyCode::KeyD, Vec2::X),
            (KeyCode::ArrowRight, Vec2::X),
        ] {
            assert_eq!(velocity_for(&[key]), direction * 145.0);
        }
    }

    /// Normalizes diagonals, cancels opposing keys, and applies boost.
    #[test]
    fn movement_normalizes_cancels_and_boosts() {
        assert_eq!(velocity_for(&[]), Vec2::ZERO);
        assert_eq!(velocity_for(&[KeyCode::KeyW, KeyCode::KeyS]), Vec2::ZERO);
        assert!((velocity_for(&[KeyCode::KeyW, KeyCode::KeyD]).length() - 145.0).abs() < 0.001);
        assert_eq!(
            velocity_for(&[KeyCode::KeyD, KeyCode::Space]),
            Vec2::X * 250.0
        );
    }
}
