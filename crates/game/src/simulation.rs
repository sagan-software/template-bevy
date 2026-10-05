//! Deterministic 2D gameplay shared by prediction and authority.
//!
//! The ECS systems in this module are deliberately thin adapters around pure
//! calculations. That keeps client prediction and server authority on the same
//! code path while making movement policy straightforward to test and benchmark.

use crate::components::DemoBody;
use crate::components::MotionTelemetry;
use crate::components::NetworkDemoStatus;
use crate::inputs::MovementIntent;
use crate::inputs::PlayerInput;
use crate::systems::DemoSystems;
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use game_physics::LinearVelocity;
use game_physics::PhysicsSystems;
use game_physics::Plugin as PhysicsPlugin;
use game_physics::Position;
use game_physics::RigidBody;
use game_settings::Config;
use lightyear::prelude::Interpolated;
use lightyear::prelude::Predicted;
use lightyear::prelude::Remote;
use lightyear::prelude::Replicate;
use lightyear::prelude::Rollback;

/// Installs physics and the deterministic systems for one application role.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SimulationPlugin {
    /// Determines which local entities this world advances.
    role: SimulationRole,
}

impl SimulationPlugin {
    /// Creates simulation for received, client-predicted entities.
    #[must_use]
    pub(crate) const fn client() -> Self {
        Self {
            role: SimulationRole::Client,
        }
    }

    /// Creates simulation for authoritative entities in the server world.
    #[must_use]
    pub(crate) const fn server() -> Self {
        Self {
            role: SimulationRole::Server,
        }
    }
}

impl Plugin for SimulationPlugin {
    /// Adds shared Rapier setup and only the systems meaningful to this role.
    fn build(&self, app: &mut App) {
        let tick_rate = app
            .world()
            .get_resource::<Config>()
            .copied()
            .unwrap_or_default()
            .tick_rate();
        // Gameplay assigns velocity before the shared fixed Rapier integration.
        app.add_plugins(PhysicsPlugin::default().with_tick_rate(tick_rate))
            .configure_sets(
                FixedUpdate,
                DemoSystems::Simulation.before(PhysicsSystems::Prepare),
            )
            .add_systems(Startup, setup_arena_colliders);

        match self.role {
            SimulationRole::Client => {
                app.add_systems(
                    FixedUpdate,
                    advance_predicted_players.in_set(DemoSystems::Simulation),
                );
            }
            SimulationRole::Server => {
                app.add_systems(
                    FixedUpdate,
                    (advance_authoritative_players, advance_autonomous_bodies)
                        .in_set(DemoSystems::Simulation),
                );
            }
        }
    }
}

/// Creates the four static Rapier segments shared by prediction and authority.
fn setup_arena_colliders(mut commands: Commands<'_, '_>) {
    let top_left = Vec2::new(-ARENA_HALF_WIDTH, ARENA_HALF_HEIGHT);
    let top_right = Vec2::new(ARENA_HALF_WIDTH, ARENA_HALF_HEIGHT);
    let bottom_left = Vec2::new(-ARENA_HALF_WIDTH, -ARENA_HALF_HEIGHT);
    let bottom_right = Vec2::new(ARENA_HALF_WIDTH, -ARENA_HALF_HEIGHT);

    // Matching colliders in both worlds are essential: otherwise client
    // prediction would diverge each time a body reached an arena wall.
    for (name, start, end) in [
        ("Top Arena Wall", top_left, top_right),
        ("Right Arena Wall", top_right, bottom_right),
        ("Bottom Arena Wall", bottom_right, bottom_left),
        ("Left Arena Wall", bottom_left, top_left),
    ] {
        commands.spawn((
            Name::new(name),
            RigidBody::Fixed,
            Transform::default(),
            game_physics::segment(start, end),
        ));
    }
}

/// Selects the entity ownership model advanced in one world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SimulationRole {
    /// Runs prediction for receiver-created local copies.
    Client,
    /// Runs authority for replicated send-side entities.
    Server,
}

/// Copyable input for one autonomous arena-boundary calculation. Position,
/// velocity, and collision half-extents use planar XY coordinates and retain
/// their independent physical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementStep {
    /// Current body center in world-space pixels.
    position: Vec2,
    /// Intended velocity in world-space pixels per second.
    velocity: Vec2,
    /// Collision half-size used to keep the full shape inside the arena.
    half_extent: Vec2,
}

impl MovementStep {
    /// Creates one pure movement sample for tests, benchmarks, or simulation.
    /// Callers supply the center, intended velocity, and collision half-extents
    /// in the same XY coordinate system.
    #[must_use]
    pub const fn new(position: Vec2, velocity: Vec2, half_extent: Vec2) -> Self {
        Self {
            position,
            velocity,
            half_extent,
        }
    }

    /// Returns the current center in XY world-space pixels without changing the
    /// movement sample. This value identifies the position used for the next
    /// boundary calculation.
    #[must_use]
    pub const fn position(self) -> Vec2 {
        self.position
    }

    /// Returns the planar velocity in XY world-space pixels per second without
    /// changing the sample. Boundary results preserve inward components and
    /// reflect only outward components.
    #[must_use]
    pub const fn velocity(self) -> Vec2 {
        self.velocity
    }

    /// Returns the shape half-size in XY world-space pixels without changing the
    /// movement sample. Boundary calculations subtract these extents to keep the
    /// complete body inside the arena.
    #[must_use]
    pub const fn half_extent(self) -> Vec2 {
        self.half_extent
    }
}

/// Result of resolving one movement sample against the rectangular arena. It
/// stores the resulting planar velocity and whether either component
/// reflected at a reached boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementStepResult {
    /// Velocity after reflecting any outward-facing boundary component.
    velocity: Vec2,
    /// Whether either axis changed direction during this step.
    is_direction_changed: bool,
}

impl MovementStepResult {
    /// Returns the planar velocity in XY world-space pixels per second without
    /// changing the sample. Boundary results preserve inward components and
    /// reflect only outward components.
    #[must_use]
    pub const fn velocity(self) -> Vec2 {
        self.velocity
    }

    /// Reports whether the boundary calculation reflected at least one velocity
    /// component. A reached wall does not change direction when the body already
    /// travels inward.
    #[must_use]
    pub const fn is_direction_changed(self) -> bool {
        self.is_direction_changed
    }
}

/// Converts a semantic XY direction and boost state into velocity in pixels
/// per second. Oversized directions are normalized to prevent diagonal
/// movement from increasing speed.
#[must_use]
pub fn movement_velocity(direction: Vec2, is_boosting: bool) -> Vec2 {
    // Input policy already caps direction at unit length, but clamp here too so
    // callers outside Enhanced Input cannot accidentally gain diagonal speed.
    let direction = if direction.length_squared() > 1.0 {
        direction.normalize_or_zero()
    } else {
        direction
    };
    let speed = if is_boosting {
        PLAYER_SPEED * BOOST_MULTIPLIER
    } else {
        PLAYER_SPEED
    };
    direction * speed
}

/// Reflects only velocity components pointing outward through a reached arena
/// boundary. Shape half-extents determine each limit, and inward velocity
/// remains unchanged after an earlier bounce.
#[must_use]
pub fn reflect_at_bounds(step: MovementStep) -> MovementStepResult {
    let x_limit = ARENA_HALF_WIDTH - step.half_extent.x;
    let y_limit = ARENA_HALF_HEIGHT - step.half_extent.y;
    let mut velocity = step.velocity;
    let is_reflect_x = is_axis_outgoing(step.position.x, velocity.x, x_limit);
    let is_reflect_y = is_axis_outgoing(step.position.y, velocity.y, y_limit);

    // A body already at a wall may be travelling inward after a previous
    // bounce. Reflecting it again would trap it in an oscillation at the edge.
    if is_reflect_x {
        velocity.x = -velocity.x;
    }
    if is_reflect_y {
        velocity.y = -velocity.y;
    }

    MovementStepResult {
        velocity,
        is_direction_changed: is_reflect_x || is_reflect_y,
    }
}

/// Reflects a coordinate only when its velocity points through the reached wall.
const fn is_axis_outgoing(position: f32, velocity: f32, limit: f32) -> bool {
    let is_left_outgoing = position <= -limit && velocity < 0.0;
    let is_right_outgoing = position >= limit && velocity > 0.0;
    is_left_outgoing || is_right_outgoing
}

/// Autonomous drive retained for the interpolation-only authority example.
#[derive(Clone, Copy, Component, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub(crate) struct AutonomousDrive {
    /// Current deterministic velocity in pixels per second.
    velocity: Vec2,
}

impl AutonomousDrive {
    /// Starts an autonomous body with the supplied velocity.
    #[must_use]
    pub(crate) const fn new(velocity: Vec2) -> Self {
        Self { velocity }
    }

    /// Returns the current autonomous velocity.
    #[must_use]
    pub(crate) const fn velocity(self) -> Vec2 {
        self.velocity
    }
}

/// Applies semantic controls to the locally predicted replica.
fn advance_predicted_players(
    mut status: ResMut<'_, NetworkDemoStatus>,
    rollback_clients: Query<'_, '_, (), With<Rollback>>,
    bodies: Query<
        '_,
        '_,
        (
            &Position,
            &mut LinearVelocity,
            &MovementIntent,
            &mut MotionTelemetry,
        ),
        (
            With<PlayerInput>,
            With<Remote>,
            With<Predicted>,
            Without<Interpolated>,
            Without<Replicate>,
            Without<AutonomousDrive>,
        ),
    >,
) {
    // The same helper is used by the authority, which is the central invariant
    // behind deterministic client prediction and rollback.
    let is_live_tick = rollback_clients.is_empty();
    let receipts = apply_player_intents(bodies, is_live_tick);
    if is_live_tick {
        // Telemetry and this BRP-facing aggregate are intentionally monotonic
        // wall-clock evidence. Count only the ordinary fixed tick that follows
        // any resimulation, never its historical replay passes.
        status.input_receipts = status.input_receipts.saturating_add(receipts);
    }
}

/// Applies received controls to the server-authoritative player entity.
fn advance_authoritative_players(
    bodies: Query<
        '_,
        '_,
        (
            &Position,
            &mut LinearVelocity,
            &MovementIntent,
            &mut MotionTelemetry,
        ),
        (With<PlayerInput>, With<Replicate>, Without<AutonomousDrive>),
    >,
) {
    // No rendered-client entity can match in this separate server World.
    apply_player_intents(bodies, true);
}

/// Maps every matching intent to velocity and returns consumed-input count.
fn apply_player_intents<F: QueryFilter>(
    mut bodies: Query<
        '_,
        '_,
        (
            &Position,
            &mut LinearVelocity,
            &MovementIntent,
            &mut MotionTelemetry,
        ),
        F,
    >,
    is_record_telemetry: bool,
) -> u64 {
    let mut receipts = 0_u64;
    for (position, mut velocity, intent, mut telemetry) in &mut bodies {
        let next_velocity = movement_velocity(intent.direction(), intent.is_boosting());
        // Equal writes would falsely advertise a changed network component on
        // every tick, so preserve Bevy's change-detection signal for real input.
        velocity.set_if_neq(LinearVelocity(next_velocity));
        if is_record_telemetry {
            telemetry.record_step(position.0, false, true);
        } else {
            // A replay is not new motion evidence. Realign the anchor so the
            // following live tick also excludes any historical correction.
            telemetry.resynchronize_position(position.0);
        }
        receipts = receipts.saturating_add(1);
    }
    receipts
}

/// Advances only the server's interpolation demonstration body autonomously.
fn advance_autonomous_bodies(
    bodies: Query<
        '_,
        '_,
        (
            &Position,
            &mut LinearVelocity,
            &mut AutonomousDrive,
            &DemoBody,
            &mut MotionTelemetry,
        ),
        (With<Replicate>, Without<MovementIntent>),
    >,
) {
    apply_autonomous_drive(bodies);
}

/// Applies the pure arena response to autonomous ECS bodies.
fn apply_autonomous_drive<F: QueryFilter>(
    mut bodies: Query<
        '_,
        '_,
        (
            &Position,
            &mut LinearVelocity,
            &mut AutonomousDrive,
            &DemoBody,
            &mut MotionTelemetry,
        ),
        F,
    >,
) {
    for (position, mut velocity, mut drive, body, mut telemetry) in &mut bodies {
        let result = reflect_at_bounds(MovementStep::new(
            position.0,
            drive.velocity(),
            body.half_extent(),
        ));
        let next_velocity = result.velocity();
        // The drive and replicated velocity change only at a bounce. Avoid
        // turning a steady trajectory into component churn for downstream systems.
        drive.set_if_neq(AutonomousDrive::new(next_velocity));
        velocity.set_if_neq(LinearVelocity(next_velocity));
        telemetry.record_step(position.0, result.is_direction_changed(), false);
    }
}

/// Requests rollback when positions differ by at least one centimeter.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Lightyear rollback callbacks require shared-reference parameters"
)]
#[must_use]
pub(crate) fn is_position_should_rollback(this: &Position, that: &Position) -> bool {
    (this.0 - that.0).length() >= ROLLBACK_THRESHOLD
}

/// Requests rollback after roughly half a degree of rotational divergence.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "Lightyear rollback callbacks require shared-reference parameters"
)]
#[must_use]
pub(crate) fn is_rotation_should_rollback(
    this: &game_physics::Rotation,
    that: &game_physics::Rotation,
) -> bool {
    this.angle_between(*that) >= ROLLBACK_THRESHOLD
}

/// Horizontal arena half-size in world-space pixels.
pub(crate) const ARENA_HALF_WIDTH: f32 = 360.0;
/// Vertical arena half-size in world-space pixels.
pub(crate) const ARENA_HALF_HEIGHT: f32 = 220.0;
/// Collision and presentation radius of the predicted circle.
pub(crate) const PREDICTED_RADIUS: f32 = 24.0;
/// Collision and presentation half-size of the interpolated box.
pub(crate) const INTERPOLATED_HALF_SIZE: Vec2 = Vec2::new(28.0, 22.0);
/// Baseline player movement speed in pixels per second.
const PLAYER_SPEED: f32 = 145.0;
/// Multiplicative speed increase while the Boost action is held.
const BOOST_MULTIPLIER: f32 = 1.75;
/// Divergence at which Lightyear should correct predicted physics.
const ROLLBACK_THRESHOLD: f32 = 0.01;

/// Covers movement policy, arena boundaries, and rollback thresholds.
#[cfg(test)]
mod tests {
    use super::ARENA_HALF_HEIGHT;
    use super::ARENA_HALF_WIDTH;
    use super::AutonomousDrive;
    use super::BOOST_MULTIPLIER;
    use super::INTERPOLATED_HALF_SIZE;
    use super::MovementStep;
    use super::PLAYER_SPEED;
    use super::PREDICTED_RADIUS;
    use super::ROLLBACK_THRESHOLD;
    use super::SimulationPlugin;
    use super::advance_autonomous_bodies;
    use super::advance_predicted_players;
    use super::is_position_should_rollback;
    use super::is_rotation_should_rollback;
    use super::movement_velocity;
    use super::reflect_at_bounds;
    use super::setup_arena_colliders;
    use crate::components::DemoBody;
    use crate::components::MotionTelemetry;
    use crate::components::NetworkDemoStatus;
    use crate::inputs::MovementIntent;
    use crate::inputs::PlayerInput;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;
    use game_physics::Collider;
    use game_physics::LinearVelocity;
    use game_physics::Position;
    use game_physics::RigidBody;
    use game_physics::Rotation;
    use lightyear::prelude::Interpolated;
    use lightyear::prelude::NetworkTarget;
    use lightyear::prelude::Predicted;
    use lightyear::prelude::Remote;
    use lightyear::prelude::Replicate;
    use lightyear::prelude::client::ClientPlugins;
    use lightyear::prelude::server::ServerPlugins;

    /// Builds the smallest client-role App that satisfies simulation
    /// registration.
    fn headless_client_simulation_app() -> App {
        let mut app = App::new();
        // Lightyear owns the replication registry consumed by its Rapier adapter;
        // these headless plugins provide that substrate without a window or GPU.
        app.add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .add_plugins(ClientPlugins::default())
            .add_plugins(SimulationPlugin::client());
        app
    }

    /// Builds the smallest server-role App that satisfies simulation
    /// registration.
    fn headless_server_simulation_app() -> App {
        let mut app = App::new();
        // Mirror the client fixture so only the role-specific simulation systems differ.
        app.add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .add_plugins(ServerPlugins::default())
            .add_plugins(SimulationPlugin::server());
        app
    }

    /// Converts neutral, axial, diagonal, and boosted inputs consistently.
    #[test]
    fn movement_velocity_applies_normalization_and_boost() {
        assert_eq!(movement_velocity(Vec2::ZERO, false), Vec2::ZERO);
        assert_eq!(movement_velocity(Vec2::X, false), Vec2::X * PLAYER_SPEED);
        assert!((movement_velocity(Vec2::ONE, false).length() - PLAYER_SPEED).abs() < 0.001);
        assert_eq!(
            movement_velocity(Vec2::X, true),
            Vec2::X * PLAYER_SPEED * BOOST_MULTIPLIER
        );
    }

    /// Adds both simulation roles without initializing Winit or a renderer.
    #[test]
    fn simulation_role_plugins_compose_in_headless_apps() {
        let client = headless_client_simulation_app();
        let server = headless_server_simulation_app();

        for app in [&client, &server] {
            assert!(app.is_plugin_added::<SimulationPlugin>());
            assert!(app.is_plugin_added::<game_physics::Plugin>());
            assert!(!app.is_plugin_added::<WindowPlugin>());
        }
    }

    /// Advances only the receiver-owned predicted player through semantic
    /// input.
    #[test]
    fn predicted_control_ignores_interpolation_only_receivers() {
        let mut app = App::new();
        app.init_resource::<NetworkDemoStatus>()
            .add_systems(Update, advance_predicted_players);
        let untouched_velocity = Vec2::splat(99.0);
        let predicted = app
            .world_mut()
            .spawn((
                PlayerInput,
                Remote,
                Predicted,
                MovementIntent::default(),
                Position(Vec2::ZERO),
                LinearVelocity(untouched_velocity),
                MotionTelemetry::at(Vec2::ZERO),
            ))
            .id();
        let interpolated = app
            .world_mut()
            .spawn((
                PlayerInput,
                Remote,
                Interpolated,
                MovementIntent::default(),
                Position(Vec2::ZERO),
                LinearVelocity(untouched_velocity),
                MotionTelemetry::at(Vec2::ZERO),
            ))
            .id();
        let non_remote = app
            .world_mut()
            .spawn((
                PlayerInput,
                Predicted,
                MovementIntent::default(),
                Position(Vec2::ZERO),
                LinearVelocity(untouched_velocity),
                MotionTelemetry::at(Vec2::ZERO),
            ))
            .id();
        let ambiguous = app
            .world_mut()
            .spawn((
                PlayerInput,
                Remote,
                Predicted,
                Interpolated,
                MovementIntent::default(),
                Position(Vec2::ZERO),
                LinearVelocity(untouched_velocity),
                MotionTelemetry::at(Vec2::ZERO),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .get::<LinearVelocity>(predicted)
                .expect("predicted velocity should exist")
                .0,
            Vec2::ZERO
        );
        assert_eq!(
            app.world()
                .get::<LinearVelocity>(interpolated)
                .expect("interpolated velocity should exist")
                .0,
            untouched_velocity
        );
        assert_eq!(
            app.world()
                .get::<MotionTelemetry>(predicted)
                .expect("predicted telemetry should exist")
                .fixed_ticks(),
            1
        );
        assert_eq!(
            app.world()
                .get::<MotionTelemetry>(interpolated)
                .expect("interpolated telemetry should exist")
                .fixed_ticks(),
            0
        );
        for rejected in [non_remote, ambiguous] {
            assert_eq!(
                app.world()
                    .get::<LinearVelocity>(rejected)
                    .expect("rejected velocity should exist")
                    .0,
                untouched_velocity
            );
            assert_eq!(
                app.world()
                    .get::<MotionTelemetry>(rejected)
                    .expect("rejected telemetry should exist")
                    .fixed_ticks(),
                0
            );
        }
        assert_eq!(
            app.world().resource::<NetworkDemoStatus>().input_receipts,
            1
        );
    }

    /// Advances autonomous interpolation sources but never player-intent
    /// bodies.
    #[test]
    fn interpolation_source_ignores_player_control() {
        let mut app = App::new();
        app.add_systems(Update, advance_autonomous_bodies);
        let untouched_velocity = Vec2::splat(99.0);
        let drive_velocity = Vec2::new(12.0, -8.0);
        let autonomous = app
            .world_mut()
            .spawn((
                Replicate::to_clients(NetworkTarget::All),
                AutonomousDrive::new(drive_velocity),
                DemoBody::interpolated(INTERPOLATED_HALF_SIZE),
                Position(Vec2::ZERO),
                LinearVelocity(untouched_velocity),
                MotionTelemetry::at(Vec2::ZERO),
            ))
            .id();
        let player_controlled = app
            .world_mut()
            .spawn((
                PlayerInput,
                Replicate::to_clients(NetworkTarget::All),
                MovementIntent::default(),
                AutonomousDrive::new(drive_velocity),
                DemoBody::interpolated(INTERPOLATED_HALF_SIZE),
                Position(Vec2::ZERO),
                LinearVelocity(untouched_velocity),
                MotionTelemetry::at(Vec2::ZERO),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .get::<LinearVelocity>(autonomous)
                .expect("autonomous velocity should exist")
                .0,
            drive_velocity
        );
        assert_eq!(
            app.world()
                .get::<LinearVelocity>(player_controlled)
                .expect("player velocity should exist")
                .0,
            untouched_velocity
        );
        assert_eq!(
            app.world()
                .get::<MotionTelemetry>(autonomous)
                .expect("autonomous telemetry should exist")
                .input_receipts(),
            0
        );
        assert_eq!(
            app.world()
                .get::<MotionTelemetry>(player_controlled)
                .expect("player telemetry should exist")
                .fixed_ticks(),
            0
        );
    }

    /// Reflects outward movement at each wall while preserving tangent speed.
    #[test]
    fn arena_reflects_every_outward_wall() {
        let x = 360.0 - PREDICTED_RADIUS;
        let y = 220.0 - PREDICTED_RADIUS;
        for (position, velocity, expected) in [
            (
                Vec2::new(-x, 0.0),
                Vec2::new(-2.0, 3.0),
                Vec2::new(2.0, 3.0),
            ),
            (Vec2::new(x, 0.0), Vec2::new(2.0, 3.0), Vec2::new(-2.0, 3.0)),
            (
                Vec2::new(0.0, -y),
                Vec2::new(2.0, -3.0),
                Vec2::new(2.0, 3.0),
            ),
            (Vec2::new(0.0, y), Vec2::new(2.0, 3.0), Vec2::new(2.0, -3.0)),
        ] {
            let result = reflect_at_bounds(MovementStep::new(
                position,
                velocity,
                Vec2::splat(PREDICTED_RADIUS),
            ));
            assert_eq!(result.velocity(), expected);
            assert!(result.is_direction_changed());
        }
    }

    /// Reflects both components when a body moves outward through a corner.
    #[test]
    fn arena_reflects_corner_movement() {
        let result = reflect_at_bounds(MovementStep::new(
            Vec2::new(
                360.0 - INTERPOLATED_HALF_SIZE.x,
                220.0 - INTERPOLATED_HALF_SIZE.y,
            ),
            Vec2::new(2.0, 3.0),
            INTERPOLATED_HALF_SIZE,
        ));

        assert_eq!(result.velocity(), Vec2::new(-2.0, -3.0));
        assert!(result.is_direction_changed());
    }

    /// Preserves ordinary movement while the body remains strictly inside the
    /// arena.
    #[test]
    fn arena_preserves_interior_movement() {
        let velocity = Vec2::new(2.0, -3.0);
        let result = reflect_at_bounds(MovementStep::new(
            Vec2::new(100.0, -100.0),
            velocity,
            Vec2::splat(PREDICTED_RADIUS),
        ));

        assert_eq!(result.velocity(), velocity);
        assert!(!result.is_direction_changed());
    }

    /// Leaves an inward-moving body alone even when its center is on a wall.
    #[test]
    fn arena_does_not_re_reflect_inward_movement() {
        let result = reflect_at_bounds(MovementStep::new(
            Vec2::new(360.0 - PREDICTED_RADIUS, 0.0),
            Vec2::new(-2.0, 3.0),
            Vec2::splat(PREDICTED_RADIUS),
        ));

        assert_eq!(result.velocity(), Vec2::new(-2.0, 3.0));
        assert!(!result.is_direction_changed());
    }

    /// Distinguishes rollback values below, at, and above the threshold.
    #[test]
    fn rollback_predicates_use_inclusive_thresholds() {
        let origin = Position(Vec2::ZERO);
        assert!(!is_position_should_rollback(
            &origin,
            &Position(Vec2::new(ROLLBACK_THRESHOLD * 0.5, 0.0))
        ));
        assert!(is_position_should_rollback(
            &origin,
            &Position(Vec2::new(ROLLBACK_THRESHOLD, 0.0))
        ));
        assert!(is_position_should_rollback(
            &origin,
            &Position(Vec2::new(ROLLBACK_THRESHOLD * 2.0, 0.0))
        ));

        let rotation = Rotation::default();
        assert!(!is_rotation_should_rollback(
            &rotation,
            &Rotation::radians(ROLLBACK_THRESHOLD * 0.5)
        ));
        assert!(is_rotation_should_rollback(
            &rotation,
            &Rotation::radians(ROLLBACK_THRESHOLD)
        ));
        assert!(is_rotation_should_rollback(
            &rotation,
            &Rotation::radians(ROLLBACK_THRESHOLD * 2.0)
        ));
    }

    /// Creates exactly four named static segment colliders for either world.
    #[test]
    fn arena_setup_creates_four_static_boundaries() {
        let mut app = App::new();
        app.add_systems(Startup, setup_arena_colliders);

        app.update();

        let mut walls = app
            .world_mut()
            .query::<(&Name, &RigidBody, &Collider)>()
            .iter(app.world())
            .map(|(name, body, collider)| {
                let segment = collider
                    .as_segment()
                    .expect("every arena collider should be a segment");
                let start = segment.a();
                let end = segment.b();
                #[cfg(all(feature = "dim3", not(feature = "dim2")))]
                {
                    assert_eq!(start.z, 0.0);
                    assert_eq!(end.z, 0.0);
                }
                (
                    name.as_str().to_owned(),
                    *body,
                    Vec2::new(start.x, start.y),
                    Vec2::new(end.x, end.y),
                )
            })
            .collect::<Vec<_>>();
        walls.sort_unstable_by(|left, right| left.0.cmp(&right.0));

        assert_eq!(
            walls,
            [
                (
                    "Bottom Arena Wall".to_owned(),
                    RigidBody::Fixed,
                    Vec2::new(ARENA_HALF_WIDTH, -ARENA_HALF_HEIGHT),
                    Vec2::new(-ARENA_HALF_WIDTH, -ARENA_HALF_HEIGHT),
                ),
                (
                    "Left Arena Wall".to_owned(),
                    RigidBody::Fixed,
                    Vec2::new(-ARENA_HALF_WIDTH, -ARENA_HALF_HEIGHT),
                    Vec2::new(-ARENA_HALF_WIDTH, ARENA_HALF_HEIGHT),
                ),
                (
                    "Right Arena Wall".to_owned(),
                    RigidBody::Fixed,
                    Vec2::new(ARENA_HALF_WIDTH, ARENA_HALF_HEIGHT),
                    Vec2::new(ARENA_HALF_WIDTH, -ARENA_HALF_HEIGHT),
                ),
                (
                    "Top Arena Wall".to_owned(),
                    RigidBody::Fixed,
                    Vec2::new(-ARENA_HALF_WIDTH, ARENA_HALF_HEIGHT),
                    Vec2::new(ARENA_HALF_WIDTH, ARENA_HALF_HEIGHT),
                ),
            ]
        );

        // Exact endpoints above catch placement regressions; lengths make the
        // intended 720-by-440 arena dimensions explicit to tutorial readers.
        for (name, _, start, end) in &walls {
            let expected_length = if name.starts_with("Top") || name.starts_with("Bottom") {
                ARENA_HALF_WIDTH * 2.0
            } else {
                ARENA_HALF_HEIGHT * 2.0
            };
            assert!((start.distance(*end) - expected_length).abs() < f32::EPSILON);
        }
    }
}
