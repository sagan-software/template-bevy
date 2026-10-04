//! Lightyear protocol, Crossbeam endpoints, and server-thread lifecycle.
//!
//! The rendered client and headless authority intentionally live in separate
//! Bevy worlds. They exchange gameplay only through Lightyear's paired
//! Crossbeam transport; a small atomic bridge carries aggregate test evidence.

use crate::{
    components::{
        DemoBody, MotionTelemetry, NetworkDemoStatus, ServerStatusBridge, register_shared_types,
    },
    config::Config,
    inputs::{AuthoritativeAction, Boost, InputsPlugin, Movement, MovementIntent, PlayerInput},
    simulation::{
        AutonomousDrive, INTERPOLATED_HALF_SIZE, PREDICTED_RADIUS, SimulationPlugin,
        position_should_rollback, rotation_should_rollback,
    },
    systems::{advance_demo_tick, sync_server_status},
};
use avian2d::prelude::{
    AngularVelocity, Collider, ColliderDensity, LinearVelocity, Position, Restitution, RigidBody,
    Rotation,
};
use bevy::{
    input::{
        ButtonState, InputPlugin as BevyInputPlugin,
        keyboard::{Key, KeyboardInput, NativeKey},
    },
    prelude::*,
    state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use bevy_enhanced_input::prelude::{
    Action, ActionMock, ActionOf, Bindings, MockEntityWorldMutExt, MockSpan, TriggerState,
};
use lightyear::prelude::input::bei::InputMarker;
use lightyear::{connection::client_of::ClientOf, crossbeam::CrossbeamIo, prelude::*};
use std::{
    fmt, io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use thiserror::Error;

/// Installs the shared protocol and one role's Crossbeam lifecycle.
pub(crate) struct NetworkPlugin {
    /// Whether this world sends authority or receives presentation copies.
    role: NetworkRole,
    /// Duration shared by Lightyear tick managers.
    tick_duration: Duration,
    /// Snapshot cadence used only by the server role.
    replication_interval: Duration,
    /// One endpoint of the in-process paired transport.
    transport: CrossbeamIo,
    /// Aggregate authority evidence shared with the rendered client.
    status_bridge: ServerStatusBridge,
}

impl NetworkPlugin {
    /// Creates networking for the rendered client endpoint.
    #[must_use]
    pub(crate) const fn client(
        tick_duration: Duration,
        transport: CrossbeamIo,
        status_bridge: ServerStatusBridge,
    ) -> Self {
        Self {
            role: NetworkRole::Client,
            tick_duration,
            replication_interval: Duration::ZERO,
            transport,
            status_bridge,
        }
    }

    /// Creates networking for the headless authority endpoint.
    #[must_use]
    pub(crate) const fn server(
        tick_duration: Duration,
        replication_interval: Duration,
        transport: CrossbeamIo,
        status_bridge: ServerStatusBridge,
    ) -> Self {
        Self {
            role: NetworkRole::Server,
            tick_duration,
            replication_interval,
            transport,
            status_bridge,
        }
    }
}

impl Plugin for NetworkPlugin {
    /// Adds shared registration first, followed by the selected connection role.
    fn build(&self, app: &mut App) {
        register_shared_types(app);
        match self.role {
            NetworkRole::Client => self.build_client(app),
            NetworkRole::Server => self.build_server(app),
        }
    }
}

impl NetworkPlugin {
    /// Installs receive-side prediction, interpolation, and client lifecycle.
    fn build_client(&self, app: &mut App) {
        // Protocol correction registration installs Lightyear's shared frame interpolation plugin.
        app.add_plugins(client::ClientPlugins {
            tick_duration: self.tick_duration,
        })
        .init_resource::<PredictionManager>()
        .add_plugins(ProtocolPlugin::client())
        .insert_resource(PendingClientTransport(Some(self.transport.clone())))
        .insert_resource(self.status_bridge.clone())
        .init_resource::<NetworkDemoStatus>()
        .add_observer(observe_client_connected)
        .add_observer(observe_predicted_body)
        .add_observer(observe_interpolated_body)
        .add_observer(observe_interpolation_history)
        .add_systems(Startup, setup_client_connection)
        .add_systems(
            Update,
            (
                sync_input_timeline_readiness,
                refresh_interpolation_samples,
                check_interpolation_deadline,
            ),
        );
    }

    /// Installs send-side authority, replication metadata, and server lifecycle.
    fn build_server(&self, app: &mut App) {
        app.add_plugins(server::ServerPlugins {
            tick_duration: self.tick_duration,
        })
        .add_plugins(ProtocolPlugin::server())
        .insert_resource(ReplicationMetadata::new(self.replication_interval))
        .insert_resource(PendingServerTransport(Some(self.transport.clone())))
        .insert_resource(self.status_bridge.clone())
        .add_observer(add_server_replication_sender)
        .add_observer(spawn_bodies_for_server_connection)
        .add_systems(Startup, setup_server_connection);
    }
}

/// Identifies the connection and replication policies for one Bevy world.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NetworkRole {
    /// Raw Crossbeam client with prediction and interpolation registries.
    Client,
    /// Raw Crossbeam server with authoritative component replication.
    Server,
}

/// Registers one wire-compatible protocol for either network role.
#[derive(Clone, Copy, Debug)]
struct ProtocolPlugin {
    /// Role-specific receive policies layered over shared serialization order.
    role: NetworkRole,
}

impl ProtocolPlugin {
    /// Creates protocol registration for a predicting/interpolating receiver.
    const fn client() -> Self {
        Self {
            role: NetworkRole::Client,
        }
    }

    /// Creates protocol registration for the authoritative sender.
    const fn server() -> Self {
        Self {
            role: NetworkRole::Server,
        }
    }
}

impl Plugin for ProtocolPlugin {
    /// Registers actions and replicated components in identical shared order.
    fn build(&self, app: &mut App) {
        // Enhanced Input owns its own replicated context/action registration.
        // Both roles call the same plugin so action kind ordering cannot drift.
        app.add_plugins(match self.role {
            NetworkRole::Client => InputsPlugin::client(),
            NetworkRole::Server => InputsPlugin::server(),
        });

        // Names use the same replication registration on both peers.
        app.component::<Name>().replicate();
        match self.role {
            NetworkRole::Client => register_predicted_physics(app),
            NetworkRole::Server => {
                app.component::<Position>().replicate();
                app.component::<Rotation>().replicate();
                app.component::<LinearVelocity>().replicate();
                app.component::<AngularVelocity>().replicate();
            }
        }
    }
}

/// Registers client prediction, rollback thresholds, and visual correction for
/// physics transforms. Velocity state stays predicted without visual interpolation
/// or correction.
fn register_predicted_physics(app: &mut App) {
    // Position and rotation share the visual interpolation and rollback-correction path.
    app.component::<Position>()
        .replicate()
        .predict()
        .with_rollback_condition(position_should_rollback)
        .add_linear_interpolation()
        .add_linear_correction();
    app.component::<Rotation>()
        .replicate()
        .predict()
        .with_rollback_condition(rotation_should_rollback)
        .add_linear_interpolation()
        .add_linear_correction();
    // Velocities remain simulation state and do not receive visual smoothing.
    app.component::<LinearVelocity>().replicate().predict();
    app.component::<AngularVelocity>().replicate().predict();
}

/// Holds the client transport until the Startup schedule creates its endpoint.
#[derive(Resource)]
struct PendingClientTransport(
    /// Endpoint consumed exactly once when Startup creates the raw client.
    Option<CrossbeamIo>,
);

/// Holds the server transport until Startup creates its connection mirror.
#[derive(Resource)]
struct PendingServerTransport(
    /// Endpoint consumed exactly once when Startup creates the raw authority.
    Option<CrossbeamIo>,
);

/// Deterministically steps the real two-world protocol without a window or thread.
///
/// This deliberately narrow harness exists for integration tests and automation.
/// It exposes semantic observations instead of either world's mutable internals,
/// preserving the rule that Bevy entity IDs are local to one world.
pub struct HeadlessNetworkHarness {
    /// Renderless predicting client App.
    client: App,
    /// Manually stepped authoritative server App.
    server: App,
}

impl fmt::Debug for HeadlessNetworkHarness {
    /// Omits large opaque Bevy worlds while retaining a useful harness identity.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HeadlessNetworkHarness")
            .finish_non_exhaustive()
    }
}

impl HeadlessNetworkHarness {
    /// Builds paired client and server worlds from the production domain plugins.
    #[must_use]
    pub fn new(config: Config) -> Self {
        let tick_duration = config.tick_rate().period();
        let (client_io, server_io) = CrossbeamIo::new_pair();
        let bridge = ServerStatusBridge::default();

        let mut client = build_headless_base(config);
        client
            .add_plugins(NetworkPlugin::client(
                tick_duration,
                client_io,
                bridge.clone(),
            ))
            .add_plugins(SimulationPlugin::client())
            .add_systems(First, sync_server_status)
            .add_systems(FixedFirst, advance_demo_tick);

        let mut server = build_headless_base(config);
        server
            .insert_resource(bridge.clone())
            .add_plugins(NetworkPlugin::server(
                tick_duration,
                config.replication_interval().duration(),
                server_io,
                bridge,
            ))
            .add_plugins(SimulationPlugin::server())
            .add_systems(FixedFirst, advance_server_tick);

        // Manual `App::update` does not run the runner's plugin lifecycle.
        // Finish both worlds explicitly so Lightyear builds its channel maps
        // before either receive system observes a packet.
        client.finish();
        client.cleanup();
        server.finish();
        server.cleanup();

        Self { client, server }
    }

    /// Advances each world exactly one fixed tick, client first then authority.
    pub fn step(&mut self) {
        // The first client update emits Connect; the server consumes it and
        // sends replication in the same harness step for the next client tick.
        self.client.update();
        self.server.update();
    }

    /// Returns a copy of the BRP-compatible client status resource.
    #[must_use]
    pub fn status(&self) -> NetworkDemoStatus {
        *self.client.world().resource::<NetworkDemoStatus>()
    }

    /// Mocks held movement and boost actions on the locally controlled context.
    ///
    /// Returns `false` until the predicted context and its replicated action
    /// entities have arrived from the authority.
    pub fn mock_actions(&mut self, movement: Vec2, boosting: bool) -> bool {
        let Some(player) = controlled_player(self.client.world_mut()) else {
            return false;
        };
        let movement_result = self
            .client
            .world_mut()
            .entity_mut(player)
            .mock::<PlayerInput, Movement>(TriggerState::Fired, movement, MockSpan::Manual);
        let boost_state = if boosting {
            TriggerState::Fired
        } else {
            TriggerState::None
        };
        let boost_result = self
            .client
            .world_mut()
            .entity_mut(player)
            .mock::<PlayerInput, Boost>(boost_state, boosting, MockSpan::Manual);
        movement_result.is_ok() && boost_result.is_ok()
    }

    /// Releases both mocked actions while preserving normal transition events.
    pub fn release_actions(&mut self) -> bool {
        let Some(player) = controlled_player(self.client.world_mut()) else {
            return false;
        };
        let movement_result = self
            .client
            .world_mut()
            .entity_mut(player)
            .mock::<PlayerInput, Movement>(TriggerState::None, Vec2::ZERO, MockSpan::Manual);
        let boost_result = self
            .client
            .world_mut()
            .entity_mut(player)
            .mock::<PlayerInput, Boost>(TriggerState::None, false, MockSpan::Manual);
        movement_result.is_ok() && boost_result.is_ok()
    }

    /// Presses the same keyboard bindings used by people and BRP automation.
    pub fn press_demo_keys(&mut self) {
        self.write_demo_key_events(ButtonState::Pressed);
    }

    /// Releases the keyboard bindings after a held-input integration probe.
    pub fn release_demo_keys(&mut self) {
        self.write_demo_key_events(ButtonState::Released);
    }

    /// Reports whether both actions use bindings instead of enabled test mocks.
    #[must_use]
    pub fn live_input_actions_ready(&mut self) -> bool {
        let world = self.client.world_mut();
        let mut actions = world.query_filtered::<(&Bindings, &ActionMock), (
            With<ActionOf<PlayerInput>>,
            With<ConfirmHistory>,
            With<InputMarker<PlayerInput>>,
            Or<(With<Action<Movement>>, With<Action<Boost>>)>,
        )>();
        let mut action_count = 0;
        for (_, mocked) in actions.iter(world) {
            if mocked.enabled {
                return false;
            }
            action_count += 1;
        }
        action_count == 2
    }

    /// Writes one pressed or released message for each tutorial keyboard action.
    fn write_demo_key_events(&mut self, state: ButtonState) {
        for key_code in [KeyCode::ArrowRight, KeyCode::Space] {
            // BRP Extras writes this same Bevy message. The logical key is not
            // used by physical-code bindings, so an unidentified value keeps
            // the harness independent from keyboard layout and text input.
            self.client.world_mut().write_message(KeyboardInput {
                key_code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                window: Entity::PLACEHOLDER,
                repeat: false,
                text: None,
            });
        }
    }

    /// Requests clean exit from both manually stepped Apps and consumes the harness.
    ///
    /// Returns the client and server exit values in that order. [`None`] means
    /// one App lost Bevy's required [`AppExit`] message resource.
    #[must_use]
    pub fn shutdown(mut self) -> Option<[AppExit; 2]> {
        // Publish both exits before either World is dropped so the test proves
        // that each independently composed App retained Bevy's lifecycle channel.
        self.client.world_mut().write_message(AppExit::Success);
        self.server.world_mut().write_message(AppExit::Success);

        // Give both schedules one final pass so network plugins can observe the
        // ordinary Bevy exit path before their Worlds are consumed and dropped.
        self.client.update();
        self.server.update();
        Some([self.client.should_exit()?, self.server.should_exit()?])
    }

    /// Returns the current authoritative player center, when spawned.
    #[must_use]
    pub fn authoritative_player_position(&mut self) -> Option<Vec2> {
        let world = self.server.world_mut();
        let mut query = world.query_filtered::<&Position, (With<PlayerInput>, With<Replicate>)>();
        query.iter(world).next().map(|position| position.0)
    }

    /// Returns the current authoritative player velocity, when spawned.
    #[must_use]
    pub fn authoritative_player_velocity(&mut self) -> Option<Vec2> {
        let world = self.server.world_mut();
        let mut query =
            world.query_filtered::<&LinearVelocity, (With<PlayerInput>, With<Replicate>)>();
        query.iter(world).next().map(|velocity| velocity.0)
    }

    /// Reports whether the authoritative player is currently boosting.
    #[must_use]
    pub fn authoritative_player_boosting(&mut self) -> Option<bool> {
        let world = self.server.world_mut();
        let mut query =
            world.query_filtered::<&MovementIntent, (With<PlayerInput>, With<Replicate>)>();
        query.iter(world).next().map(|intent| intent.boosting())
    }

    /// Returns the current predicted receiver center, when spawned.
    #[must_use]
    pub fn predicted_player_position(&mut self) -> Option<Vec2> {
        let world = self.client.world_mut();
        let mut query = world
            .query_filtered::<&Position, (With<PlayerInput>, With<Predicted>, Without<Replicate>)>(
            );
        query.iter(world).next().map(|position| position.0)
    }
}

/// Builds common deterministic infrastructure for one manually stepped world.
fn build_headless_base(config: Config) -> App {
    let tick_duration = config.tick_rate().period();
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        StatesPlugin,
        BevyInputPlugin,
    ))
    // The deterministic harness exposes the same reflected configuration as
    // production composition, without depending on the rendered root plugin.
    .register_type::<Config>()
    .insert_resource(config)
    .insert_resource(Time::<Fixed>::from_duration(tick_duration))
    .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
    app
}

/// Finds the one local controlled prediction context without leaking its entity ID.
fn controlled_player(world: &mut World) -> Option<Entity> {
    let mut query =
        world.query_filtered::<Entity, (With<PlayerInput>, With<Predicted>, With<Controlled>)>();
    query.iter(world).next()
}

/// Starts the rendered world's sole raw client endpoint.
fn setup_client_connection(
    mut commands: Commands<'_, '_>,
    mut transport: ResMut<'_, PendingClientTransport>,
) {
    // Consume the endpoint once so repeated startup cannot create another client.
    let Some(client_io) = transport.0.take() else {
        error!("Crossbeam client transport was already consumed");
        return;
    };
    // Attach prediction and replication to the raw transport endpoint.
    let client = commands
        .spawn((
            Name::new("Crossbeam Prediction Client"),
            Client,
            client::RawClient,
            ReplicationReceiver,
            PingManager::default(),
            Link::default(),
            client_io,
        ))
        .id();

    // `Connect` deliberately traverses the raw transport lifecycle instead of
    // using a same-world shortcut that would hide networking from the example.
    commands.trigger(Connect { entity: client });
    info!(?client, "created Crossbeam client endpoint");
}

/// Publishes when Lightyear can actually buffer and send client input ticks.
fn sync_input_timeline_readiness(
    clients: Query<'_, '_, (), With<Client>>,
    timeline: Res<'_, LocalTimelineSync>,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    // Lightyear 0.29 stores synchronization readiness on the local timeline resource.
    let is_synced = !clients.is_empty() && timeline.is_synced();
    if status.input_timeline_synced != is_synced {
        status.input_timeline_synced = is_synced;
    }
}

/// Starts the authority and its server-side Crossbeam mirror.
fn setup_server_connection(
    mut commands: Commands<'_, '_>,
    mut transport: ResMut<'_, PendingServerTransport>,
) {
    // Consume the server endpoint before creating its connection entities.
    let Some(server_io) = transport.0.take() else {
        error!("Crossbeam server transport was already consumed");
        return;
    };
    // The authority owns the server lifecycle independently of the peer mirror.
    let server = commands
        .spawn((
            Name::new("Embedded Lightyear Server"),
            Server::default(),
            server::RawServer,
            Link::default(),
            Linked,
            server::Started,
        ))
        .id();
    // The mirror links the remote client's transport to this authority.
    let mirror = commands
        .spawn((
            Name::new("Crossbeam Server Mirror"),
            LinkOf { server },
            PingManager::default(),
            Link::default(),
            server_io,
        ))
        .id();

    // The Crossbeam plugin converts LinkStart into Linked; the raw connection
    // layer then promotes that link to the normal Connected lifecycle.
    commands.trigger(LinkStart { entity: mirror });
    info!(
        ?server,
        ?mirror,
        "created embedded Crossbeam server endpoint"
    );
}

/// Equips each server-side client link to publish replicated entities.
fn add_server_replication_sender(
    trigger: On<'_, '_, Add, LinkOf>,
    mut commands: Commands<'_, '_>,
    status: Res<'_, ServerStatusBridge>,
) {
    commands.entity(trigger.entity).insert(ReplicationSender);
    status.record_replication_sender();
    info!(connection = ?trigger.entity, "installed Lightyear replication sender");
}

/// Creates both demonstrations after a raw peer is confirmed connected.
fn spawn_bodies_for_server_connection(
    trigger: On<'_, '_, Add, Connected>,
    clients: Query<'_, '_, &RemoteId, With<ClientOf>>,
    mut commands: Commands<'_, '_>,
    status: Res<'_, ServerStatusBridge>,
) {
    let Ok(remote_id) = clients.get(trigger.entity) else {
        return;
    };
    let client_id = remote_id.0;

    status.record_connection();
    let player = spawn_predicted_body(&mut commands, trigger.entity, client_id);
    spawn_player_actions(&mut commands, player);
    spawn_interpolated_body(&mut commands, trigger.entity, client_id);
    info!(connection = ?trigger.entity, ?client_id, "Crossbeam server mirror connected");
}

/// Spawns the authoritative player targeted for prediction by one client.
fn spawn_predicted_body(
    commands: &mut Commands<'_, '_>,
    connection: Entity,
    client_id: PeerId,
) -> Entity {
    let position = Vec2::new(-210.0, -70.0);
    commands
        .spawn((
            Name::new("Predicted Circle"),
            PlayerInput,
            MovementIntent::default(),
            DemoBody::predicted(PREDICTED_RADIUS),
            MotionTelemetry::at(position),
            RigidBody::Dynamic,
            Collider::circle(PREDICTED_RADIUS),
            ColliderDensity(1.0),
            Restitution::new(1.0),
            Position(position),
            Rotation::default(),
            LinearVelocity(Vec2::ZERO),
            AngularVelocity(0.8),
            (
                Replicate::to_clients(NetworkTarget::Single(client_id)),
                PredictionTarget::to_clients(NetworkTarget::Single(client_id)),
                ControlledBy {
                    owner: connection,
                    lifetime: default(),
                },
            ),
        ))
        .id()
}

/// Spawns the two server-owned BEI action entities in protocol order.
fn spawn_player_actions(commands: &mut Commands<'_, '_>, player: Entity) {
    // Action entities replicate like their context. Their live values travel in
    // Lightyear's input snapshots rather than ordinary component replication.
    commands.spawn((
        ActionOf::<PlayerInput>::new(player),
        Action::<Movement>::new(),
        ReplicateLike { root: player },
        AuthoritativeAction,
    ));
    commands.spawn((
        ActionOf::<PlayerInput>::new(player),
        Action::<Boost>::new(),
        ReplicateLike { root: player },
        AuthoritativeAction,
    ));
}

/// Spawns the session-owned authority targeted for snapshot interpolation.
fn spawn_interpolated_body(commands: &mut Commands<'_, '_>, connection: Entity, client_id: PeerId) {
    let position = Vec2::new(190.0, 75.0);
    let velocity = Vec2::new(-112.0, 138.0);
    commands.spawn((
        Name::new("Interpolated Box"),
        DemoBody::interpolated(INTERPOLATED_HALF_SIZE),
        AutonomousDrive::new(velocity),
        MotionTelemetry::at(position),
        RigidBody::Dynamic,
        Collider::rectangle(
            INTERPOLATED_HALF_SIZE.x * 2.0,
            INTERPOLATED_HALF_SIZE.y * 2.0,
        ),
        ColliderDensity(1.0),
        Restitution::new(1.0),
        Position(position),
        Rotation::default(),
        LinearVelocity(velocity),
        AngularVelocity(-0.55),
        (
            Replicate::to_clients(NetworkTarget::Single(client_id)),
            InterpolationTarget::to_clients(NetworkTarget::Single(client_id)),
            // Lightyear owns disconnect cleanup through this relationship. The
            // receiver may gain `Controlled`, but no input context is attached
            // because this autonomous demonstration has no `PlayerInput`.
            ControlledBy {
                owner: connection,
                lifetime: Lifetime::SessionBased,
            },
        ),
    ));
}

/// Records the rendered raw endpoint completing its connection lifecycle.
fn observe_client_connected(
    trigger: On<'_, '_, Add, Connected>,
    clients: Query<'_, '_, &RemoteId, With<client::RawClient>>,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    let Ok(remote_id) = clients.get(trigger.entity) else {
        return;
    };
    status.client_connected = true;
    status.client_connection_events = status.client_connection_events.saturating_add(1);
    info!(client = ?trigger.entity, remote = ?remote_id.0, "Crossbeam client connected");
}

/// Installs client-local physics and render interpolation on a predicted copy.
fn observe_predicted_body(
    trigger: On<'_, '_, Add, (Remote, Predicted, Name, Position, Rotation, LinearVelocity)>,
    bodies: Query<
        '_,
        '_,
        (&Name, &Position),
        (
            With<Remote>,
            With<Predicted>,
            Without<Interpolated>,
            Without<Replicate>,
            Without<DemoBody>,
        ),
    >,
    mut commands: Commands<'_, '_>,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    // Ignore entities until all prediction markers and physics state have arrived.
    let Ok((name, position)) = bodies.get(trigger.entity) else {
        return;
    };
    // Add client-local physics and enable frame interpolation on the received body.
    commands.entity(trigger.entity).insert((
        DemoBody::predicted(PREDICTED_RADIUS),
        MotionTelemetry::at(position.0),
        RigidBody::Dynamic,
        Collider::circle(PREDICTED_RADIUS),
        ColliderDensity(1.0),
        Restitution::new(1.0),
        FrameInterpolate,
    ));
    // Publish readiness only after the received body has its local simulation components.
    status.predicted_entities = status.predicted_entities.saturating_add(1);
    info!(entity = ?trigger.entity, body = %name, "prediction receive path became active");
}

/// Adds presentation metadata to an interpolation-only receiver copy.
fn observe_interpolated_body(
    trigger: On<'_, '_, Add, (Remote, Interpolated, Name, Position, Rotation)>,
    bodies: Query<
        '_,
        '_,
        (&Name, &Position),
        (
            With<Remote>,
            With<Interpolated>,
            Without<Predicted>,
            Without<Replicate>,
            Without<DemoBody>,
        ),
    >,
    mut commands: Commands<'_, '_>,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    let Ok((name, position)) = bodies.get(trigger.entity) else {
        return;
    };
    // Interpolation owns this entity's presentation; adding a RigidBody here
    // would make local physics fight received confirmed snapshots.
    commands
        .entity(trigger.entity)
        .insert(DemoBody::interpolated(INTERPOLATED_HALF_SIZE));
    status.interpolated_entities = status.interpolated_entities.saturating_add(1);
    info!(entity = ?trigger.entity, body = %name, position = ?position.0, "interpolation receive path became active");
}

/// Records creation of receiver-owned confirmed position history.
fn observe_interpolation_history(
    trigger: On<'_, '_, Add, (Remote, ConfirmedHistory<Position>)>,
    histories: Query<
        '_,
        '_,
        &ConfirmedHistory<Position>,
        (
            With<Interpolated>,
            With<Remote>,
            Without<Predicted>,
            Without<Replicate>,
        ),
    >,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    if histories.get(trigger.entity).is_ok() {
        status.interpolation_histories = status.interpolation_histories.saturating_add(1);
    }
}

/// Refreshes readiness only when Lightyear mutates a confirmed history.
fn refresh_interpolation_samples(
    histories: Query<
        '_,
        '_,
        &ConfirmedHistory<Position>,
        (
            With<Interpolated>,
            With<Remote>,
            Without<Predicted>,
            Without<Replicate>,
            Changed<ConfirmedHistory<Position>>,
        ),
    >,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    let samples = histories
        .iter()
        .map(ConfirmedHistory::len)
        .max()
        .unwrap_or(0);
    status.interpolation_samples = status.interpolation_samples.max(samples);
    if samples >= 2 && !status.interpolation_ready {
        status.interpolation_ready = true;
        info!(
            samples,
            "runtime success: interpolation has two confirmed samples"
        );
    }
}

/// Marks a missing interpolation interval after a bounded fixed-tick deadline.
fn check_interpolation_deadline(mut status: ResMut<'_, NetworkDemoStatus>) {
    if status.fixed_tick >= INTERPOLATION_DEADLINE_TICKS
        && !status.interpolation_ready
        && !status.interpolation_deadline_missed
    {
        status.interpolation_deadline_missed = true;
        error!(
            samples = status.interpolation_samples,
            "interpolation readiness deadline missed"
        );
    }
}

/// Owns the headless authority thread until an explicit or fallback shutdown.
#[derive(Debug, Resource)]
pub struct ServerRuntime {
    /// Shared flag observed by the server App every frame.
    shutdown: ServerShutdown,
    /// Join handle present until the first successful or failed join attempt.
    handle: Option<JoinHandle<AppExit>>,
    /// Exit value retained so repeated shutdown calls stay idempotent.
    exit: Option<AppExit>,
    /// Join failure retained so repeated shutdown calls preserve the terminal result.
    error: Option<ServerRuntimeError>,
}

impl ServerRuntime {
    /// Starts a named authority thread using the supplied world-owning runner.
    ///
    /// # Errors
    ///
    /// Returns the operating-system thread creation error without panicking.
    pub(crate) fn spawn(
        runner: impl FnOnce(ServerShutdown) -> AppExit + Send + 'static,
    ) -> io::Result<Self> {
        let shutdown = ServerShutdown::default();
        let thread_shutdown = shutdown.clone();
        let handle = thread::Builder::new()
            .name("embedded-lightyear-server".into())
            .spawn(move || runner(thread_shutdown))?;
        Ok(Self {
            shutdown,
            handle: Some(handle),
            exit: None,
            error: None,
        })
    }

    /// Requests server exit and waits for its schedule runner to finish.
    ///
    /// # Errors
    ///
    /// Returns [`ServerRuntimeError::Panicked`] if the authority thread unwound.
    pub fn shutdown_and_join(&mut self) -> Result<AppExit, ServerRuntimeError> {
        self.shutdown.request();
        let Some(handle) = self.handle.take() else {
            if let Some(error) = self.error {
                return Err(error);
            }
            return Ok(self.exit.clone().unwrap_or(AppExit::Success));
        };
        let Ok(exit) = handle.join() else {
            self.error = Some(ServerRuntimeError::Panicked);
            return Err(ServerRuntimeError::Panicked);
        };
        self.exit = Some(exit.clone());
        Ok(exit)
    }

    /// Reports whether the authority thread still has an outstanding join.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.handle.is_some()
    }
}

impl Drop for ServerRuntime {
    /// Provides a last-resort join when a caller drops an App without `AppExit`.
    fn drop(&mut self) {
        if self.handle.is_some()
            && let Err(error) = self.shutdown_and_join()
        {
            error!(%error, "embedded server fallback shutdown failed");
        }
    }
}

/// Explains why joining the embedded authority did not complete normally.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ServerRuntimeError {
    /// The authority thread unwound before returning its Bevy exit value.
    #[error("embedded server thread panicked")]
    Panicked,
}

/// Cloneable shutdown signal inserted into the server world.
#[derive(Clone, Debug, Default, Resource)]
pub(crate) struct ServerShutdown(
    /// Monotonic flag shared with the rendered world's runtime owner.
    Arc<AtomicBool>,
);

impl ServerShutdown {
    /// Publishes one monotonic shutdown request to the authority thread.
    fn request(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Reads the latest request using the matching acquire ordering.
    fn is_requested(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// Converts the shared shutdown flag into Bevy's normal exit message.
pub(crate) fn request_server_exit(
    shutdown: Res<'_, ServerShutdown>,
    mut exits: MessageWriter<'_, AppExit>,
) {
    if shutdown.is_requested() {
        exits.write(AppExit::Success);
    }
}

/// Advances the authority heartbeat sampled through the atomic status bridge.
pub(crate) fn advance_server_tick(bridge: Res<'_, ServerStatusBridge>) {
    // This probe lives in the server FixedFirst schedule, so a stalled server
    // cannot look healthy merely because the rendered client keeps updating.
    bridge.record_server_tick();
}

/// Explicitly shuts down and joins the authority when the client emits `AppExit`.
pub(crate) fn shutdown_server_on_app_exit(
    mut exits: MessageReader<'_, '_, AppExit>,
    mut runtime: ResMut<'_, ServerRuntime>,
) {
    if exits.read().next().is_none() || !runtime.is_running() {
        return;
    }
    match runtime.shutdown_and_join() {
        Ok(exit) => info!(?exit, "embedded Lightyear server joined"),
        Err(error) => error!(%error, "embedded Lightyear server join failed"),
    }
}

/// Fixed-tick deadline for obtaining two authoritative snapshots.
pub(crate) const INTERPOLATION_DEADLINE_TICKS: u64 = 128;

/// Covers protocol bundles, receive filters, and lifecycle without windows or ports.
#[cfg(all(test, not(coverage)))]
mod tests;
