//! Unit tests for the input vocabulary and live-input plumbing.

use super::{
    AuthoritativeAction, Boost, InputsPlugin, Movement, MovementIntent, PlayerInput,
    add_bindings_to_controlled_player, add_bindings_to_late_action, apply_authoritative_boost,
    apply_authoritative_movement, apply_local_boost, apply_local_movement, clear_local_boost,
    clear_local_movement, normalize_direction,
};
use crate::{components::NetworkDemoStatus, systems::sync_server_status};
use bevy::{input::InputPlugin as BevyInputPlugin, prelude::*, state::app::StatesPlugin};
use bevy_enhanced_input::{
    EnhancedInputPlugin,
    prelude::{
        Action, ActionMock, ActionOf, Actions, Binding, Bindings, Complete, Fire,
        InputContextAppExt, MockEntityWorldMutExt, TriggerState, actions,
    },
};
use lightyear::prelude::{
    Controlled, Interpolated, NetworkTarget, Predicted, Remote, Replicate, Rollback,
    client::ClientPlugins, server::ServerPlugins,
};

/// Builds the smallest rendered-role App that can register the input protocol.
fn headless_client_input_app() -> App {
    let mut app = App::new();
    // Lightyear's shared registries must precede protocol registration, while
    // the low-level input plugin supplies keyboard/gamepad resources without Winit.
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        StatesPlugin,
        BevyInputPlugin,
    ))
    .add_plugins(ClientPlugins::default())
    .add_plugins(InputsPlugin::client());
    app
}

/// Builds the smallest authority-role App that can register the input protocol.
fn headless_server_input_app() -> App {
    let mut app = App::new();
    // Use the same engine substrate as the client fixture so the test isolates
    // the role-specific input plugin rather than unrelated platform services.
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        StatesPlugin,
        BevyInputPlugin,
    ))
    .add_plugins(ServerPlugins::default())
    .add_plugins(InputsPlugin::server());
    app
}

/// Creates the smallest App that evaluates mocked client actions.
fn mock_client_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, EnhancedInputPlugin))
        .add_input_context::<PlayerInput>()
        .init_resource::<NetworkDemoStatus>()
        .add_observer(apply_local_movement)
        .add_observer(clear_local_movement)
        .add_observer(apply_local_boost)
        .add_observer(clear_local_boost);
    // Enhanced Input's context registration observer requires resources
    // initialized in the plugin finish phase before contexts are spawned.
    app.finish();
    let player = app
        .world_mut()
        .spawn((
            PlayerInput,
            Remote,
            Predicted,
            MovementIntent::default(),
            actions!(PlayerInput[
                Action::<Movement>::new(),
                Action::<Boost>::new(),
            ]),
        ))
        .id();
    (app, player)
}

/// Leaves neutral input stationary.
#[test]
fn neutral_direction_stays_zero() {
    assert_eq!(normalize_direction(Vec2::ZERO), Vec2::ZERO);
}

/// Preserves sub-unit analog magnitude for fine movement control.
#[test]
fn analog_direction_preserves_small_magnitude() {
    let value = Vec2::new(0.25, -0.5);
    assert_eq!(normalize_direction(value), value);
}

/// Prevents diagonal digital input from moving faster than one axis.
#[test]
fn diagonal_direction_is_normalized() {
    let direction = normalize_direction(Vec2::ONE);
    assert!((direction.length() - 1.0).abs() < f32::EPSILON);
}

/// Changes boost independently from the current direction.
#[test]
fn movement_intent_keeps_actions_independent() {
    let mut intent = MovementIntent::default();
    intent.set_direction(Vec2::X);
    intent.set_boosting(true);

    assert_eq!(intent.direction(), Vec2::X);
    assert!(intent.boosting());

    intent.set_boosting(false);
    assert_eq!(intent.direction(), Vec2::X);
    assert!(!intent.boosting());
}

/// Adds both role variants without opening a window or installing a renderer.
#[test]
fn input_role_plugins_compose_in_headless_apps() {
    let client = headless_client_input_app();
    let server = headless_server_input_app();

    for app in [&client, &server] {
        assert!(app.is_plugin_added::<InputsPlugin>());
        assert!(app.is_plugin_added::<EnhancedInputPlugin>());
        assert!(!app.is_plugin_added::<WindowPlugin>());
    }
}

/// Drives neutral, movement, boost, and release through `ActionMock` events.
#[test]
fn action_mock_drives_predicted_semantic_intent() {
    let (mut app, player) = mock_client_app();

    // The untouched context begins neutral before any action is mocked.
    app.update();
    assert_eq!(
        *app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists"),
        MovementIntent::default()
    );

    app.world_mut()
        .entity_mut(player)
        .mock_once::<PlayerInput, Movement>(TriggerState::Fired, Vec2::ONE)
        .expect("movement action should exist");
    app.update();
    let intent = *app
        .world()
        .get::<MovementIntent>(player)
        .expect("intent exists");
    assert!((intent.direction().length() - 1.0).abs() < f32::EPSILON);
    assert!(!intent.boosting());

    app.world_mut()
        .entity_mut(player)
        .mock_once::<PlayerInput, Boost>(TriggerState::Fired, true)
        .expect("boost action should exist");
    app.update();
    assert!(
        app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists")
            .boosting()
    );

    // With the one-update mocks expired, the next evaluation emits
    // Complete for each action and clears both semantic values.
    app.update();
    assert_eq!(
        *app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists"),
        MovementIntent::default()
    );
    let status = app.world().resource::<NetworkDemoStatus>();
    assert!(status.local_action_receipts >= 2);
    assert!(status.movement_action_receipts >= 1);
    assert!(status.boost_action_receipts >= 1);
}

/// Reapplies historical action state without counting it as new live input.
#[test]
fn rollback_action_replay_does_not_increment_live_counters() {
    let (mut app, player) = mock_client_app();
    app.world_mut().spawn(Rollback::FromState);
    app.world_mut()
        .entity_mut(player)
        .mock_once::<PlayerInput, Movement>(TriggerState::Fired, Vec2::X)
        .expect("movement action should exist");

    app.update();

    assert_eq!(
        app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists")
            .direction(),
        Vec2::X
    );
    assert_eq!(
        *app.world().resource::<NetworkDemoStatus>(),
        NetworkDemoStatus::default()
    );
}

/// Attaches all tutorial keyboard and gamepad mappings after control arrives.
#[test]
fn controlled_player_receives_complete_bindings_once() {
    let mut app = App::new();
    app.add_observer(add_bindings_to_controlled_player)
        .add_observer(add_bindings_to_late_action);
    let player = app
        .world_mut()
        .spawn((
            PlayerInput,
            actions!(PlayerInput[
                Action::<Movement>::new(),
                Action::<Boost>::new(),
            ]),
        ))
        .id();

    app.world_mut().entity_mut(player).insert(Controlled);
    app.update();
    let actions = app
        .world()
        .get::<Actions<PlayerInput>>(player)
        .expect("player should own actions");
    let movement = actions
        .iter()
        .find(|&entity| app.world().get::<Action<Movement>>(entity).is_some())
        .expect("movement action should exist");
    let boost = actions
        .iter()
        .find(|&entity| app.world().get::<Action<Boost>>(entity).is_some())
        .expect("boost action should exist");

    let movement_bindings = app
        .world()
        .get::<Bindings>(movement)
        .expect("movement should be bound");
    let movement_values = movement_bindings
        .iter()
        .filter_map(|entity| app.world().get::<Binding>(entity).copied())
        .collect::<Vec<_>>();
    assert_eq!(movement_values.len(), 10);
    for expected in [
        Binding::from(KeyCode::KeyW),
        Binding::from(KeyCode::KeyA),
        Binding::from(KeyCode::KeyS),
        Binding::from(KeyCode::KeyD),
        Binding::from(KeyCode::ArrowUp),
        Binding::from(KeyCode::ArrowLeft),
        Binding::from(KeyCode::ArrowDown),
        Binding::from(KeyCode::ArrowRight),
        Binding::from(GamepadAxis::LeftStickX),
        Binding::from(GamepadAxis::LeftStickY),
    ] {
        assert!(movement_values.contains(&expected));
    }

    let boost_bindings = app
        .world()
        .get::<Bindings>(boost)
        .expect("boost should be bound");
    let boost_values = boost_bindings
        .iter()
        .filter_map(|entity| app.world().get::<Binding>(entity).copied())
        .collect::<Vec<_>>();
    assert_eq!(
        boost_values,
        [
            Binding::from(KeyCode::Space),
            Binding::from(GamepadButton::South),
        ]
    );

    // Re-adding control cannot duplicate relationship children because the
    // binding queries accept only actions that still lack `Bindings`.
    let before = app.world().entities().len();
    app.world_mut().entity_mut(player).remove::<Controlled>();
    app.world_mut().entity_mut(player).insert(Controlled);
    app.update();
    assert_eq!(app.world().entities().len(), before);
}

/// Leaves server-owned or otherwise uncontrolled action contexts unbound.
#[test]
fn uncontrolled_player_does_not_receive_local_bindings() {
    let mut app = App::new();
    app.add_observer(add_bindings_to_controlled_player)
        .add_observer(add_bindings_to_late_action);
    let player = app
        .world_mut()
        .spawn((
            PlayerInput,
            actions!(PlayerInput[
                Action::<Movement>::new(),
                Action::<Boost>::new(),
            ]),
        ))
        .id();

    // Let relationship observers settle without ever granting local control.
    app.update();

    let actions = app
        .world()
        .get::<Actions<PlayerInput>>(player)
        .expect("player should own actions");
    assert!(
        actions
            .iter()
            .all(|action| app.world().get::<Bindings>(action).is_none())
    );
    assert!(app.world().get::<MovementIntent>(player).is_none());
}

/// Ignores local events whose context is not a receiver-owned predicted player.
#[test]
fn local_observers_reject_wrong_context_ownership() {
    let mut app = App::new();
    app.init_resource::<NetworkDemoStatus>()
        .add_observer(apply_local_movement)
        .add_observer(clear_local_movement)
        .add_observer(apply_local_boost)
        .add_observer(clear_local_boost);
    let expected = MovementIntent {
        direction: Vec2::Y,
        boosting: true,
    };
    let unpredicted = app.world_mut().spawn((PlayerInput, Remote, expected)).id();
    let send_side = app
        .world_mut()
        .spawn((
            PlayerInput,
            Remote,
            Predicted,
            Replicate::to_clients(NetworkTarget::All),
            expected,
        ))
        .id();
    let non_remote = app
        .world_mut()
        .spawn((PlayerInput, Predicted, expected))
        .id();
    let ambiguous = app
        .world_mut()
        .spawn((PlayerInput, Remote, Predicted, Interpolated, expected))
        .id();
    let movement = app.world_mut().spawn(Action::<Movement>::new()).id();
    let boost = app.world_mut().spawn(Action::<Boost>::new()).id();

    for context in [unpredicted, send_side, non_remote, ambiguous] {
        // Both semantic actions use the same strict context ownership filter.
        app.world_mut().trigger(Fire::<Movement> {
            context,
            action: movement,
            value: Vec2::X,
            state: TriggerState::Fired,
            fired_secs: 0.0,
            elapsed_secs: 0.0,
        });
        app.world_mut().trigger(Fire::<Boost> {
            context,
            action: boost,
            value: false,
            state: TriggerState::Fired,
            fired_secs: 0.0,
            elapsed_secs: 0.0,
        });
        // Release events must honor the same ownership rules as held input;
        // otherwise a stray action could clear a valid local intent.
        app.world_mut().trigger(Complete::<Movement> {
            context,
            action: movement,
            value: Vec2::ZERO,
            state: TriggerState::None,
            fired_secs: 0.0,
            elapsed_secs: 0.0,
        });
        app.world_mut().trigger(Complete::<Boost> {
            context,
            action: boost,
            value: false,
            state: TriggerState::None,
            fired_secs: 0.0,
            elapsed_secs: 0.0,
        });

        assert_eq!(app.world().get::<MovementIntent>(context), Some(&expected));
    }
    assert_eq!(
        *app.world().resource::<NetworkDemoStatus>(),
        NetworkDemoStatus::default()
    );
}

/// Handles an action arriving after control without resetting live intent.
#[test]
fn late_action_binding_preserves_existing_intent() {
    let mut app = App::new();
    app.add_observer(add_bindings_to_late_action);
    let expected = MovementIntent {
        direction: Vec2::X,
        boosting: true,
    };
    let player = app
        .world_mut()
        .spawn((PlayerInput, Controlled, expected))
        .id();
    let action = app
        .world_mut()
        .spawn((
            ActionOf::<PlayerInput>::new(player),
            Action::<Movement>::new(),
        ))
        .id();

    app.update();
    assert!(app.world().get::<Bindings>(action).is_some());
    assert_eq!(app.world().get::<MovementIntent>(player), Some(&expected));
}

/// Accepts only marked server actions and records authoritative receipt evidence.
#[test]
fn authoritative_observer_filters_wrong_role_actions() {
    let mut app = App::new();
    let bridge = crate::components::ServerStatusBridge::default();
    app.insert_resource(bridge)
        .init_resource::<NetworkDemoStatus>()
        .add_observer(apply_authoritative_movement)
        .add_systems(Update, sync_server_status);
    let player = app
        .world_mut()
        .spawn((
            PlayerInput,
            MovementIntent::default(),
            Replicate::to_clients(NetworkTarget::All),
        ))
        .id();
    let correct_action = app
        .world_mut()
        .spawn((Action::<Movement>::new(), AuthoritativeAction))
        .id();
    let wrong_action = app.world_mut().spawn(Action::<Movement>::new()).id();

    app.world_mut().trigger(Fire::<Movement> {
        context: player,
        action: wrong_action,
        value: Vec2::X,
        state: TriggerState::Fired,
        fired_secs: 0.0,
        elapsed_secs: 0.0,
    });
    assert_eq!(
        *app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists"),
        MovementIntent::default()
    );

    app.world_mut().trigger(Fire::<Movement> {
        context: player,
        action: correct_action,
        value: Vec2::Y,
        state: TriggerState::Fired,
        fired_secs: 0.0,
        elapsed_secs: 0.0,
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists")
            .direction(),
        Vec2::Y
    );
    assert_eq!(
        app.world()
            .resource::<NetworkDemoStatus>()
            .server_action_receipts,
        1
    );
}

/// Applies Boost only through an action explicitly owned by the authority.
#[test]
fn authoritative_boost_observer_filters_wrong_role_actions() {
    let mut app = App::new();
    let bridge = crate::components::ServerStatusBridge::default();
    app.insert_resource(bridge)
        .init_resource::<NetworkDemoStatus>()
        .add_observer(apply_authoritative_boost)
        .add_systems(Update, sync_server_status);
    let player = app
        .world_mut()
        .spawn((
            PlayerInput,
            MovementIntent::default(),
            Replicate::to_clients(NetworkTarget::All),
        ))
        .id();
    let correct_action = app
        .world_mut()
        .spawn((Action::<Boost>::new(), AuthoritativeAction))
        .id();
    let wrong_action = app.world_mut().spawn(Action::<Boost>::new()).id();

    // An unmarked client-side action entity must not mutate server intent.
    app.world_mut().trigger(Fire::<Boost> {
        context: player,
        action: wrong_action,
        value: true,
        state: TriggerState::Fired,
        fired_secs: 0.0,
        elapsed_secs: 0.0,
    });
    assert!(
        !app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists")
            .boosting()
    );

    // The authority marker selects the matching protocol action and records receipt evidence.
    app.world_mut().trigger(Fire::<Boost> {
        context: player,
        action: correct_action,
        value: true,
        state: TriggerState::Fired,
        fired_secs: 0.0,
        elapsed_secs: 0.0,
    });
    app.update();
    assert!(
        app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists")
            .boosting()
    );
    assert_eq!(
        app.world()
            .resource::<NetworkDemoStatus>()
            .server_action_receipts,
        1
    );
}

/// Ensures the mock component itself remains part of the testable API path.
#[test]
fn action_mock_is_installed_on_the_target_action() {
    let (mut app, player) = mock_client_app();
    app.world_mut()
        .entity_mut(player)
        .mock_once::<PlayerInput, Movement>(TriggerState::Fired, Vec2::X)
        .expect("movement action should exist");
    let actions = app
        .world()
        .get::<Actions<PlayerInput>>(player)
        .expect("player should own actions");
    let movement = actions
        .iter()
        .find(|&entity| app.world().get::<Action<Movement>>(entity).is_some())
        .expect("movement action should exist");

    assert!(
        app.world()
            .get::<ActionMock>(movement)
            .is_some_and(|mocked| mocked.enabled)
    );
}

/// Produces one cardinal direction before cumulative opposite keys cancel.
#[test]
fn keyboard_directions_produce_and_cancel_semantic_movement() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, BevyInputPlugin, EnhancedInputPlugin))
        .add_input_context::<PlayerInput>()
        .init_resource::<NetworkDemoStatus>()
        .add_observer(add_bindings_to_controlled_player)
        .add_observer(add_bindings_to_late_action)
        .add_observer(apply_local_movement)
        .add_observer(clear_local_movement);
    app.finish();
    let player = app
        .world_mut()
        .spawn((
            PlayerInput,
            Remote,
            Predicted,
            MovementIntent::default(),
            actions!(PlayerInput[Action::<Movement>::new()]),
        ))
        .id();
    app.world_mut().entity_mut(player).insert(Controlled);
    app.update();

    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::KeyW);
    }
    app.update();

    assert_eq!(
        app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists")
            .direction(),
        Vec2::Y
    );

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyS);
    app.update();

    assert_eq!(
        app.world()
            .get::<MovementIntent>(player)
            .expect("intent exists")
            .direction(),
        Vec2::ZERO
    );
}
