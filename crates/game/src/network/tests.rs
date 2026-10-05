//! Focused protocol, receive-filter, and lifecycle tests for the network domain.

use super::HeadlessNetworkHarness;
use super::INTERPOLATION_DEADLINE_TICKS;
use super::ServerRuntime;
use super::ServerRuntimeError;
use super::check_interpolation_deadline;
use super::observe_interpolated_body;
use super::observe_interpolation_history;
use super::observe_predicted_body;
use super::refresh_interpolation_samples;
use super::shutdown_server_on_app_exit;
use super::sync_input_timeline_readiness;
use crate::components::DemoBody;
use crate::components::MotionTelemetry;
use crate::components::NetworkDemoStatus;
use crate::components::NetworkPath;
use crate::inputs::AuthoritativeAction;
use crate::inputs::Boost;
use crate::inputs::Movement;
use crate::inputs::PlayerInput;
use crate::simulation::AutonomousDrive;
use crate::simulation::INTERPOLATED_HALF_SIZE;
use crate::simulation::PREDICTED_RADIUS;
use bevy::ecs::reflect::ReflectResource;
use bevy::prelude::App;
use bevy::prelude::AppExit;
use bevy::prelude::AppTypeRegistry;
use bevy::prelude::Entity;
use bevy::prelude::Last;
use bevy::prelude::MinimalPlugins;
use bevy::prelude::Name;
use bevy::prelude::Update;
use bevy::prelude::Vec2;
use bevy::prelude::With;
use bevy::prelude::Without;
use bevy::reflect::PartialReflect;
use bevy::reflect::ReflectRef;
use bevy_enhanced_input::prelude::Action;
use bevy_enhanced_input::prelude::ActionOf;
use bevy_enhanced_input::prelude::Actions;
use bevy_enhanced_input::prelude::Bindings;
use game_physics::Collider;
use game_physics::LinearVelocity;
use game_physics::Position;
use game_physics::RigidBody;
use game_physics::Rotation;
use game_settings::Config;
use lightyear::prelude::ConfirmHistory;
use lightyear::prelude::ConfirmedHistory;
use lightyear::prelude::Controlled;
use lightyear::prelude::ControlledBy;
use lightyear::prelude::ControlledSend;
use lightyear::prelude::InputTimelineConfig;
use lightyear::prelude::Interpolated;
use lightyear::prelude::InterpolationTarget;
use lightyear::prelude::Lifetime;
use lightyear::prelude::LocalTimeline;
use lightyear::prelude::LocalTimelineSync;
use lightyear::prelude::NetworkTarget;
use lightyear::prelude::PeerId;
use lightyear::prelude::Predicted;
use lightyear::prelude::PredictionManager;
use lightyear::prelude::PredictionMetrics;
use lightyear::prelude::PredictionTarget;
use lightyear::prelude::Remote;
use lightyear::prelude::RemoteId;
use lightyear::prelude::Replicate;
use lightyear::prelude::ReplicateLike;
use lightyear::prelude::Rollback;
use lightyear::prelude::Tick;
use lightyear::prelude::client::Client;
use lightyear::prelude::client::InputDelayConfig;
use lightyear::prelude::input::bei::InputMarker;
use std::any::TypeId;
use std::sync::mpsc;
use std::thread;

/// Maximum paired updates allowed for a focused protocol fixture to become
/// ready.
const READY_STEP_LIMIT: usize = 512;

/// Readiness follows timeline synchronization and requires a client endpoint.
#[test]
fn readiness_tracks_local_timeline_and_client_presence() {
    let mut app = App::new();
    app.init_resource::<LocalTimelineSync>()
        .init_resource::<NetworkDemoStatus>()
        .add_systems(Update, sync_input_timeline_readiness);
    let client = app.world_mut().spawn(Client).id();

    for synced in [false, true, true, false] {
        app.world_mut()
            .resource_mut::<LocalTimelineSync>()
            .set_synced(synced);
        app.update();
        assert_eq!(
            app.world()
                .resource::<NetworkDemoStatus>()
                .is_input_timeline_synced,
            synced
        );
    }

    app.world_mut().despawn(client);
    app.world_mut()
        .resource_mut::<LocalTimelineSync>()
        .set_synced(true);
    app.update();
    assert!(
        !app.world()
            .resource::<NetworkDemoStatus>()
            .is_input_timeline_synced
    );
}

/// Builds one bounded two-world fixture with every receive path ready.
fn ready_harness() -> HeadlessNetworkHarness {
    let mut harness = HeadlessNetworkHarness::new(Config::default());
    for _ in 0..READY_STEP_LIMIT {
        harness.step();
        if harness.status().is_network_ready() {
            return harness;
        }
    }
    panic!("network fixture did not become ready within its bounded steps");
}

/// Copies telemetry from the one locally predicted tutorial body.
fn predicted_telemetry(harness: &mut HeadlessNetworkHarness) -> MotionTelemetry {
    let world = harness.client.world_mut();
    let mut predicted = world.query_filtered::<
        &MotionTelemetry,
        (With<PlayerInput>, With<Predicted>, Without<Replicate>),
    >();
    *predicted
        .single(world)
        .expect("one predicted telemetry component should exist")
}

/// Verifies Lightyear's private replication mode through its reflected shape.
fn assert_single_client_target(target: &dyn PartialReflect, client_id: PeerId) {
    let ReflectRef::Struct(target) = target.reflect_ref() else {
        panic!("a replication target should reflect as a struct");
    };
    let mode = target
        .field("mode")
        .expect("a replication target should reflect its private mode");
    let ReflectRef::Enum(mode) = mode.reflect_ref() else {
        panic!("a replication mode should reflect as an enum");
    };

    // `to_clients` must select the single-server routing mode, and its first
    // tuple field must retain the exact peer selected by the authority.
    assert_eq!(mode.variant_name(), "SingleServer");
    assert_eq!(
        mode.field_at(0)
            .and_then(|target| target.try_downcast_ref::<NetworkTarget>()),
        Some(&NetworkTarget::Single(client_id))
    );
}

/// Requests, joins, and remembers a normal authority exit.
#[test]
fn server_runtime_shutdown_is_explicit_and_idempotent() {
    let mut runtime = ServerRuntime::spawn(|shutdown| {
        while !shutdown.is_requested() {
            thread::yield_now();
        }
        AppExit::Success
    })
    .expect("the test authority thread should start");

    assert!(runtime.is_running());
    assert_eq!(runtime.shutdown_and_join(), Ok(AppExit::Success));
    assert!(!runtime.is_running());
    assert_eq!(runtime.shutdown_and_join(), Ok(AppExit::Success));
}

/// Converts a thread unwind into a typed join error.
#[test]
fn server_runtime_reports_thread_panics() {
    let mut runtime = ServerRuntime::spawn(|_shutdown| panic!("intentional fixture panic"))
        .expect("the test authority thread should start");

    // Joining waits deterministically for the fixture; no timing delay is
    // necessary and the panic is translated at the ownership boundary.
    assert_eq!(
        runtime.shutdown_and_join(),
        Err(ServerRuntimeError::Panicked)
    );
    assert_eq!(
        runtime.shutdown_and_join(),
        Err(ServerRuntimeError::Panicked)
    );
}

/// Uses the RAII fallback to request shutdown and wait for thread completion.
#[test]
fn dropping_server_runtime_joins_the_authority() {
    let (completed_sender, completed_receiver) = mpsc::sync_channel(1);
    let runtime = ServerRuntime::spawn(move |shutdown| {
        while !shutdown.is_requested() {
            thread::yield_now();
        }
        completed_sender
            .send(())
            .expect("the completion receiver should remain alive");
        AppExit::Success
    })
    .expect("the test authority thread should start");

    // `Drop` cannot return until the owned join handle completes, so a
    // non-blocking receive is a deterministic proof that no thread was detached.
    drop(runtime);
    assert_eq!(completed_receiver.try_recv(), Ok(()));
}

/// Joins through the production `AppExit` system before resource drop.
#[test]
fn app_exit_system_performs_explicit_join() {
    let runtime = ServerRuntime::spawn(|shutdown| {
        while !shutdown.is_requested() {
            thread::yield_now();
        }
        AppExit::Success
    })
    .expect("the test authority thread should start");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(runtime)
        .add_systems(Last, shutdown_server_on_app_exit);

    app.world_mut().write_message(AppExit::Success);
    app.update();

    assert!(!app.world().resource::<ServerRuntime>().is_running());
}

/// Registers both typed actions in each role through the shared protocol path.
#[test]
fn protocol_roles_register_identical_action_types() {
    let harness = HeadlessNetworkHarness::new(Config::default());

    for world in [harness.client.world(), harness.server.world()] {
        assert_eq!(world.resource::<Config>(), &Config::default());
        let registry = world.resource::<AppTypeRegistry>().read();
        let config_registration = registry
            .get(TypeId::of::<Config>())
            .expect("headless composition should register Config for reflection");
        assert!(config_registration.data::<ReflectResource>().is_some());
        assert!(world.components().component_id::<PlayerInput>().is_some());
        assert!(
            world
                .components()
                .component_id::<Action<Movement>>()
                .is_some()
        );
        assert!(world.components().component_id::<Action<Boost>>().is_some());
    }
}

/// Matches the official balanced delay policy needed by independently run
/// peers.
#[test]
fn client_input_timeline_uses_balanced_delay() {
    let mut harness = HeadlessNetworkHarness::new(Config::default());
    harness.step();
    let actual = harness.client.world().resource::<InputTimelineConfig>();
    let expected = InputTimelineConfig::default().with_input_delay(InputDelayConfig::balanced());

    // Reflection compares Lightyear's intentionally private nested config,
    // avoiding a brittle debug-string assertion while proving all three delay
    // bounds and the unchanged default synchronization policy.
    assert_eq!(actual.reflect_partial_eq(&expected), Some(true));
}

/// Repairs the send marker if replicated component arrival order leaves it
/// absent.
#[test]
fn client_reconciles_live_input_marker() {
    let mut harness = ready_harness();
    let action = {
        let world = harness.client.world_mut();
        let mut movement_actions = world.query_filtered::<Entity, (
            With<ActionOf<PlayerInput>>,
            With<Action<Movement>>,
            With<Bindings>,
            With<ConfirmHistory>,
            With<InputMarker<PlayerInput>>,
        )>();
        movement_actions
            .single(world)
            .expect("one send-eligible movement action should exist")
    };

    // Simulate the receive-order gap after all other eligibility components
    // have settled, then let the production client schedule repair it.
    harness
        .client
        .world_mut()
        .entity_mut(action)
        .remove::<InputMarker<PlayerInput>>();
    assert!(
        harness
            .client
            .world()
            .get::<InputMarker<PlayerInput>>(action)
            .is_none()
    );

    harness.step();

    assert!(
        harness
            .client
            .world()
            .get::<InputMarker<PlayerInput>>(action)
            .is_some()
    );
}

/// Replays deterministic state without inflating monotonic runtime evidence.
#[test]
fn prediction_rollback_does_not_double_count_observability() {
    let mut harness = ready_harness();
    assert!(harness.is_mock_actions_applied(Vec2::X, true));
    for _ in 0..8 {
        harness.step();
    }

    // Seed telemetry with an intentionally unrelated anchor. Historical replay
    // must realign this position without counting the correction as travelled
    // distance; otherwise the following live sample would report a huge jump.
    {
        let world = harness.client.world_mut();
        let player = {
            let mut predicted = world
                .query_filtered::<Entity, (With<PlayerInput>, With<Predicted>, Without<Replicate>)>(
                );
            predicted
                .single(world)
                .expect("one predicted player should own telemetry")
        };
        world
            .entity_mut(player)
            .insert(MotionTelemetry::at(Vec2::splat(10_000.0)));
    }

    let status_before = harness.status();
    let telemetry_before = predicted_telemetry(&mut harness);
    let (rollbacks_before, rollback_ticks_before, rollback_tick) = {
        let world = harness.client.world();
        let metrics = world.resource::<PredictionMetrics>();
        let rollbacks_before = metrics.rollbacks;
        let rollback_ticks_before = metrics.rollback_ticks;
        let current_tick = world.resource::<LocalTimeline>().tick();
        (rollbacks_before, rollback_ticks_before, current_tick - 4)
    };
    // Force the same input-origin rollback mode Lightyear uses after a remote
    // input mismatch, while choosing a deterministic four-tick test window.
    harness
        .client
        .world()
        .resource::<PredictionManager>()
        .set_rollback_tick(rollback_tick);
    harness
        .client
        .world_mut()
        .insert_resource(Rollback::FromInputs);

    harness.step();

    let status_after = harness.status();
    let telemetry_after = predicted_telemetry(&mut harness);
    let metrics_after = harness.client.world().resource::<PredictionMetrics>();
    assert_eq!(metrics_after.rollbacks, rollbacks_before + 1);
    assert_eq!(metrics_after.rollback_ticks, rollback_ticks_before + 4);

    // The four historical FixedMain passes rebuild rollback-managed gameplay
    // state, while every monotonic counter advances only for the new live tick.
    assert_eq!(status_after.fixed_tick, status_before.fixed_tick + 1);
    assert_eq!(
        status_after.input_receipts,
        status_before.input_receipts + 1
    );
    assert_eq!(
        status_after.local_action_receipts,
        status_before.local_action_receipts + 2
    );
    assert_eq!(
        status_after.movement_action_receipts,
        status_before.movement_action_receipts + 1
    );
    assert_eq!(
        status_after.boost_action_receipts,
        status_before.boost_action_receipts + 1
    );
    assert_eq!(
        telemetry_after.fixed_ticks(),
        telemetry_before.fixed_ticks() + 1
    );
    assert_eq!(
        telemetry_after.input_receipts(),
        telemetry_before.input_receipts() + 1
    );
    let distance_delta =
        telemetry_after.distance_travelled() - telemetry_before.distance_travelled();
    assert!(
        (0.0..=5.0).contains(&distance_delta),
        "one boosted 60 Hz step should not include replayed distance: {distance_delta}"
    );
}

/// Accepts only unambiguous receiver-owned prediction, interpolation, and
/// history markers.
#[test]
fn receive_observers_reject_invalid_marker_combinations() {
    let mut app = App::new();
    app.init_resource::<NetworkDemoStatus>()
        .add_observer(observe_predicted_body)
        .add_observer(observe_interpolated_body)
        .add_observer(observe_interpolation_history);

    let rejected_predicted = app
        .world_mut()
        .spawn((
            Remote,
            Predicted,
            Name::new("send-side predicted fixture"),
            Position(Vec2::ZERO),
            Rotation::default(),
            LinearVelocity(Vec2::ZERO),
            Replicate::to_clients(NetworkTarget::All),
        ))
        .id();
    let rejected_interpolated = app
        .world_mut()
        .spawn((
            Remote,
            Interpolated,
            Name::new("send-side interpolation fixture"),
            Position(Vec2::ZERO),
            Rotation::default(),
            ConfirmedHistory::<Position>::default(),
            Replicate::to_clients(NetworkTarget::All),
        ))
        .id();
    let rejected_ambiguous = app
        .world_mut()
        .spawn((
            Remote,
            Predicted,
            Interpolated,
            Name::new("ambiguous receiver fixture"),
            Position(Vec2::ZERO),
            Rotation::default(),
            LinearVelocity(Vec2::ZERO),
            ConfirmedHistory::<Position>::default(),
        ))
        .id();
    app.update();

    let status = app.world().resource::<NetworkDemoStatus>();
    assert_eq!(status.predicted_entities, 0);
    assert_eq!(status.interpolated_entities, 0);
    assert_eq!(status.interpolation_histories, 0);
    assert!(app.world().get::<DemoBody>(rejected_predicted).is_none());
    assert!(app.world().get::<DemoBody>(rejected_interpolated).is_none());
    assert!(app.world().get::<DemoBody>(rejected_ambiguous).is_none());

    // Matching receiver markers prove the same observers accept the intended
    // path instead of passing only because every fixture was rejected.
    let accepted_predicted = app
        .world_mut()
        .spawn((
            Remote,
            Predicted,
            Name::new("receive-side predicted fixture"),
            Position(Vec2::ZERO),
            Rotation::default(),
            LinearVelocity(Vec2::ZERO),
        ))
        .id();
    let accepted_interpolated = app
        .world_mut()
        .spawn((
            Remote,
            Interpolated,
            Name::new("receive-side interpolation fixture"),
            Position(Vec2::ZERO),
            Rotation::default(),
            ConfirmedHistory::<Position>::default(),
        ))
        .id();
    app.update();

    let status = app.world().resource::<NetworkDemoStatus>();
    assert_eq!(status.predicted_entities, 1);
    assert_eq!(status.interpolated_entities, 1);
    assert_eq!(status.interpolation_histories, 1);
    assert!(app.world().get::<DemoBody>(accepted_predicted).is_some());
    assert!(app.world().get::<DemoBody>(accepted_interpolated).is_some());
}

/// Spawns the exact authoritative bodies, targets, ownership, and action order.
#[test]
fn authority_spawns_documented_network_bundles() {
    let mut harness = ready_harness();
    let world = harness.server.world_mut();
    let player = {
        let mut query = world.query_filtered::<Entity, (With<PlayerInput>, With<Replicate>)>();
        query
            .single(world)
            .expect("one authoritative player should exist")
    };

    let controlled_by = *world
        .get::<ControlledBy>(player)
        .expect("the player should be owned by its connection");
    let client_id = world
        .get::<RemoteId>(controlled_by.owner)
        .expect("the owner should identify its remote peer")
        .0;
    let player_ref = world.entity(player);
    assert_eq!(
        player_ref.get::<Name>().map(Name::as_str),
        Some("Predicted Circle")
    );
    assert_eq!(player_ref.get::<RigidBody>(), Some(&RigidBody::Dynamic));
    assert!(player_ref.contains::<ControlledSend>());
    assert!(!player_ref.contains::<Controlled>());
    assert_eq!(controlled_by.lifetime, Lifetime::SessionBased);
    assert_eq!(
        player_ref.get::<DemoBody>().map(DemoBody::path),
        Some(NetworkPath::Prediction)
    );
    assert_eq!(
        player_ref.get::<DemoBody>().map(DemoBody::half_extent),
        Some(Vec2::splat(PREDICTED_RADIUS))
    );
    let player_collider = player_ref
        .get::<Collider>()
        .expect("the predicted player should have a collider");
    let player_radius = player_collider
        .as_ball()
        .expect("the predicted collider should be circular")
        .radius();
    assert!((player_radius - PREDICTED_RADIUS).abs() <= f32::EPSILON);
    assert_eq!(
        player_ref.get::<Replicate>(),
        Some(&Replicate::to_clients(NetworkTarget::Single(client_id)))
    );
    assert_single_client_target(
        player_ref
            .get::<PredictionTarget>()
            .expect("the player should select its predicting client"),
        client_id,
    );

    let action_entities = player_ref
        .get::<Actions<PlayerInput>>()
        .expect("the player should own both protocol actions")
        .iter()
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(action_entities.len(), 2);
    assert!(world.get::<Action<Movement>>(action_entities[0]).is_some());
    assert!(world.get::<Action<Boost>>(action_entities[1]).is_some());
    for action in action_entities {
        assert_eq!(
            world.get::<ActionOf<PlayerInput>>(action),
            Some(&ActionOf::new(player))
        );
        assert_eq!(
            world.get::<ReplicateLike>(action).map(|link| link.root),
            Some(player)
        );
        assert!(world.get::<AuthoritativeAction>(action).is_some());
    }

    let interpolated = {
        let mut query = world.query_filtered::<
            Entity,
            (With<AutonomousDrive>, With<Replicate>, Without<PlayerInput>),
        >();
        query
            .single(world)
            .expect("one interpolation authority should exist")
    };
    let interpolated_ref = world.entity(interpolated);
    let interpolated_controlled_by = *interpolated_ref
        .get::<ControlledBy>()
        .expect("the interpolation authority should belong to its client session");
    assert_eq!(
        interpolated_ref.get::<Name>().map(Name::as_str),
        Some("Interpolated Box")
    );
    assert!(interpolated_ref.contains::<ControlledSend>());
    assert!(!interpolated_ref.contains::<Controlled>());
    assert_eq!(interpolated_controlled_by.owner, controlled_by.owner);
    assert_eq!(interpolated_controlled_by.lifetime, Lifetime::SessionBased);
    assert_eq!(
        interpolated_ref.get::<DemoBody>().map(DemoBody::path),
        Some(NetworkPath::Interpolation)
    );
    assert_eq!(
        interpolated_ref
            .get::<DemoBody>()
            .map(DemoBody::half_extent),
        Some(INTERPOLATED_HALF_SIZE)
    );
    let interpolated_collider = interpolated_ref
        .get::<Collider>()
        .expect("the interpolation authority should have a collider");
    let half_extent = interpolated_collider
        .as_cuboid()
        .expect("the interpolated collider should be rectangular")
        .half_extents();
    assert_eq!(
        Vec2::new(half_extent.x, half_extent.y),
        INTERPOLATED_HALF_SIZE
    );
    #[cfg(all(feature = "dim3", not(feature = "dim2")))]
    assert_eq!(half_extent.z, INTERPOLATED_HALF_SIZE.y);
    assert_eq!(
        interpolated_ref.get::<Replicate>(),
        Some(&Replicate::to_clients(NetworkTarget::Single(client_id)))
    );
    assert_single_client_target(
        interpolated_ref
            .get::<InterpolationTarget>()
            .expect("the autonomous body should select its interpolating client"),
        client_id,
    );
}

/// Requires two changed confirmed samples before declaring interpolation ready.
#[test]
fn interpolation_readiness_tracks_zero_one_and_two_samples() {
    let mut app = App::new();
    app.init_resource::<NetworkDemoStatus>()
        .add_systems(Update, refresh_interpolation_samples);
    let history = app
        .world_mut()
        .spawn((
            ConfirmedHistory::<Position>::default(),
            Interpolated,
            Remote,
        ))
        .id();

    app.update();
    assert_eq!(
        app.world()
            .resource::<NetworkDemoStatus>()
            .interpolation_samples,
        0
    );

    // Mutating the history exercises the production `Changed` filter.
    app.world_mut()
        .get_mut::<ConfirmedHistory<Position>>(history)
        .expect("history exists")
        .insert_present(Tick(1), Position(Vec2::ZERO));
    app.update();
    let status = app.world().resource::<NetworkDemoStatus>();
    assert_eq!(status.interpolation_samples, 1);
    assert!(!status.is_interpolation_ready);

    app.world_mut()
        .get_mut::<ConfirmedHistory<Position>>(history)
        .expect("history exists")
        .insert_present(Tick(2), Position(Vec2::X));
    app.update();
    let status = app.world().resource::<NetworkDemoStatus>();
    assert_eq!(status.interpolation_samples, 2);
    assert!(status.is_interpolation_ready);
}

/// Misses the deadline only once the threshold is reached without readiness.
#[test]
fn interpolation_deadline_requires_missing_evidence() {
    let mut app = App::new();
    app.insert_resource(NetworkDemoStatus {
        fixed_tick: INTERPOLATION_DEADLINE_TICKS - 1,
        ..NetworkDemoStatus::default()
    })
    .add_systems(Update, check_interpolation_deadline);

    app.update();
    assert!(
        !app.world()
            .resource::<NetworkDemoStatus>()
            .is_interpolation_deadline_missed
    );

    // Reaching the threshold is healthy when two samples are already ready.
    {
        let mut status = app.world_mut().resource_mut::<NetworkDemoStatus>();
        status.fixed_tick = INTERPOLATION_DEADLINE_TICKS;
        status.is_interpolation_ready = true;
    }
    app.update();
    assert!(
        !app.world()
            .resource::<NetworkDemoStatus>()
            .is_interpolation_deadline_missed
    );

    app.world_mut()
        .resource_mut::<NetworkDemoStatus>()
        .is_interpolation_ready = false;
    app.update();
    assert!(
        app.world()
            .resource::<NetworkDemoStatus>()
            .is_interpolation_deadline_missed
    );
}
