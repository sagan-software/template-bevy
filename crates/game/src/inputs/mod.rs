//! Typed player actions shared by prediction and the authoritative server.
//!
//! Bevy Enhanced Input turns hardware state into semantic actions. Lightyear
//! then records those action states per network tick so prediction, rollback,
//! and the server all consume the same values.

use crate::components::NetworkDemoStatus;
use crate::components::ServerStatusBridge;
use bevy::ecs::relationship::Relationship as _;
use bevy::prelude::*;
use bevy_enhanced_input::prelude::Action;
use bevy_enhanced_input::prelude::ActionOf;
use bevy_enhanced_input::prelude::Actions;
use bevy_enhanced_input::prelude::Axial;
use bevy_enhanced_input::prelude::Binding;
use bevy_enhanced_input::prelude::Bindings;
use bevy_enhanced_input::prelude::Cardinal;
use bevy_enhanced_input::prelude::Complete;
use bevy_enhanced_input::prelude::DeadZone;
use bevy_enhanced_input::prelude::Fire;
use bevy_enhanced_input::prelude::InputAction;
use bevy_enhanced_input::prelude::WithBundle;
use lightyear::prelude::ConfirmHistory;
use lightyear::prelude::Controlled;
use lightyear::prelude::InputTimelineConfig;
use lightyear::prelude::Interpolated;
use lightyear::prelude::Predicted;
use lightyear::prelude::Remote;
use lightyear::prelude::Replicate;
use lightyear::prelude::Rollback;
use lightyear::prelude::client::InputDelayConfig;
use lightyear::prelude::input::bei::InputMarker;
use lightyear::prelude::input::bei::InputPlugin;
use lightyear::prelude::input::bei::InputRegistryExt;
use serde::Deserialize;
use serde::Serialize;

/// Registers the shared input protocol and the observers for one network role.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InputsPlugin {
    /// Role-specific consumers installed after shared action registration.
    role: InputRole,
}

impl InputsPlugin {
    /// Creates input handling for the rendered, predicting client.
    #[must_use]
    pub(crate) const fn client() -> Self {
        Self {
            role: InputRole::Client,
        }
    }

    /// Creates input handling for the headless authority.
    #[must_use]
    pub(crate) const fn server() -> Self {
        Self {
            role: InputRole::Server,
        }
    }
}

impl Plugin for InputsPlugin {
    /// Registers actions identically in both worlds, then adds role-local
    /// adapters.
    fn build(&self, app: &mut App) {
        // Registration order is part of the network protocol. Keeping both
        // roles behind this one plugin prevents their action registries drifting.
        app.add_plugins(InputPlugin::<PlayerInput>::default());
        app.register_input_action::<Movement>();
        app.register_input_action::<Boost>();

        match self.role {
            InputRole::Client => {
                // Balanced delay gives the independent server time to receive each input.
                app.insert_resource(
                    InputTimelineConfig::default().with_input_delay(InputDelayConfig::balanced()),
                )
                .add_observer(add_bindings_to_controlled_player)
                .add_observer(add_bindings_to_late_action)
                .add_observer(apply_local_movement)
                .add_observer(clear_local_movement)
                .add_observer(apply_local_boost)
                .add_observer(clear_local_boost)
                .add_systems(PreUpdate, reconcile_live_input_markers)
                .add_systems(Update, observe_external_keyboard_input);
            }
            InputRole::Server => {
                app.add_observer(apply_authoritative_movement)
                    .add_observer(clear_authoritative_movement)
                    .add_observer(apply_authoritative_boost)
                    .add_observer(clear_authoritative_boost);
            }
        }
    }
}

/// Selects which side of the protocol consumes action events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputRole {
    /// Receives hardware input and predicts the locally controlled body.
    Client,
    /// Receives Lightyear input snapshots and advances authoritative state.
    Server,
}

/// Input context attached to the player controlled through Lightyear.
#[derive(Component, Clone, Debug, Deserialize, PartialEq, Reflect, Serialize)]
pub(crate) struct PlayerInput;

/// Two-axis intent produced by keyboards or a gamepad stick.
#[derive(Debug, InputAction)]
#[action_output(Vec2)]
pub(crate) struct Movement;

/// Button intent that temporarily increases movement speed.
#[derive(Debug, InputAction)]
#[action_output(bool)]
pub(crate) struct Boost;

/// Latest semantic controls consumed by deterministic simulation.
#[derive(Clone, Copy, Component, Debug, Default, PartialEq)]
pub(crate) struct MovementIntent {
    /// Unit-length-or-smaller direction in world-space XY coordinates.
    direction: Vec2,
    /// Whether the speed multiplier is currently active.
    is_boosting: bool,
}

impl MovementIntent {
    /// Returns the normalized movement direction.
    #[must_use]
    pub(crate) const fn direction(self) -> Vec2 {
        self.direction
    }

    /// Reports whether the boost action is currently held.
    #[must_use]
    pub(crate) const fn is_boosting(self) -> bool {
        self.is_boosting
    }

    /// Replaces raw input with its speed-safe semantic direction.
    fn set_direction(&mut self, value: Vec2) {
        self.direction = normalize_direction(value);
    }

    /// Records the current button state without changing direction.
    const fn set_boosting(&mut self, is_boosting: bool) {
        self.is_boosting = is_boosting;
    }
}

/// Marks action entities authored and consumed in the authority world.
#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct AuthoritativeAction;

/// Repairs receive-order races after a replicated action becomes locally owned.
fn reconcile_live_input_markers(
    actions: Query<
        '_,
        '_,
        (Entity, &ActionOf<PlayerInput>),
        (
            With<Bindings>,
            With<ConfirmHistory>,
            Without<InputMarker<PlayerInput>>,
        ),
    >,
    controlled_players: Query<'_, '_, (), (With<PlayerInput>, With<Controlled>)>,
    mut commands: Commands<'_, '_>,
) {
    // Lightyear normally adds this marker when Bindings or ConfirmHistory
    // arrives. Replication can map ActionOf to the predicted context later,
    // after both one-shot observers ran, so reconcile the final relationship
    // before FixedPreUpdate buffers the action for transport.
    actions
        .iter()
        .filter(|(_, action_of)| controlled_players.contains(action_of.get()))
        .for_each(|(action, _)| {
            commands
                .entity(action)
                .insert(InputMarker::<PlayerInput>::default());
        });
}

/// Adds hardware bindings after Lightyear marks a received context as local.
fn add_bindings_to_controlled_player(
    trigger: On<'_, '_, Add, Controlled>,
    players: Query<'_, '_, Option<&Actions<PlayerInput>>, With<PlayerInput>>,
    movement_actions: Query<'_, '_, (), (With<Action<Movement>>, Without<Bindings>)>,
    boost_actions: Query<'_, '_, (), (With<Action<Boost>>, Without<Bindings>)>,
    mut commands: Commands<'_, '_>,
) {
    let Ok(Some(actions)) = players.get(trigger.entity) else {
        return;
    };

    // Action entities are server-authored and replicated. Bindings are local
    // presentation/input data, so they must never be replicated by the server.
    for action in actions.iter() {
        attach_local_bindings(
            action,
            &movement_actions,
            &boost_actions,
            commands.reborrow(),
        );
    }
    commands
        .entity(trigger.entity)
        .insert_if_new(MovementIntent::default());
}

/// Handles the inverse arrival order where an action follows `Controlled`.
fn add_bindings_to_late_action(
    trigger: On<'_, '_, Add, ActionOf<PlayerInput>>,
    action_of: Query<'_, '_, &ActionOf<PlayerInput>>,
    controlled_players: Query<'_, '_, (), (With<PlayerInput>, With<Controlled>)>,
    movement_actions: Query<'_, '_, (), (With<Action<Movement>>, Without<Bindings>)>,
    boost_actions: Query<'_, '_, (), (With<Action<Boost>>, Without<Bindings>)>,
    mut commands: Commands<'_, '_>,
) {
    let Ok(action_of) = action_of.get(trigger.entity) else {
        return;
    };
    if !controlled_players.contains(action_of.get()) {
        return;
    }

    // Supporting both structural arrival orders makes the observer independent
    // of packet/component ordering and remains idempotent through `Without`.
    attach_local_bindings(
        trigger.entity,
        &movement_actions,
        &boost_actions,
        commands.reborrow(),
    );
    commands
        .entity(action_of.get())
        .insert_if_new(MovementIntent::default());
}

/// Attaches the appropriate keyboard and gamepad mappings to one action.
fn attach_local_bindings(
    action: Entity,
    movement_actions: &Query<'_, '_, (), (With<Action<Movement>>, Without<Bindings>)>,
    boost_actions: &Query<'_, '_, (), (With<Action<Boost>>, Without<Bindings>)>,
    mut commands: Commands<'_, '_>,
) {
    if movement_actions.contains(action) {
        // Cardinal presets use cumulative accumulation, so opposite keys cancel.
        // Dead-zone processing keeps analog stick drift from moving the player.
        commands.entity(action).insert(Bindings::spawn((
            Cardinal::wasd_keys(),
            Cardinal::arrows(),
            Axial::left_stick().with(DeadZone::default()),
        )));
    } else if boost_actions.contains(action) {
        commands.entity(action).insert(Bindings::spawn((
            Spawn(Binding::from(KeyCode::Space)),
            Spawn(Binding::from(GamepadButton::South)),
        )));
    }
}

/// Applies a client action to the locally predicted context.
fn apply_local_movement(
    trigger: On<'_, '_, Fire<Movement>>,
    mut players: Query<
        '_,
        '_,
        &mut MovementIntent,
        (
            With<PlayerInput>,
            With<Remote>,
            With<Predicted>,
            Without<Interpolated>,
            Without<Replicate>,
        ),
    >,
    rollback_clients: Query<'_, '_, (), With<Rollback>>,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    let Ok(mut intent) = players.get_mut(trigger.context) else {
        return;
    };

    intent.set_direction(trigger.value);
    if rollback_clients.is_empty() {
        // Lightyear replays action events to reconstruct gameplay state. These
        // counters describe new live input, so replay must not count again.
        status.local_action_receipts = status.local_action_receipts.saturating_add(1);
        status.movement_action_receipts = status.movement_action_receipts.saturating_add(1);
    }
}

/// Clears predicted movement when the two-axis action returns to neutral.
fn clear_local_movement(
    trigger: On<'_, '_, Complete<Movement>>,
    mut players: Query<
        '_,
        '_,
        &mut MovementIntent,
        (
            With<PlayerInput>,
            With<Remote>,
            With<Predicted>,
            Without<Interpolated>,
            Without<Replicate>,
        ),
    >,
) {
    if let Ok(mut intent) = players.get_mut(trigger.context) {
        intent.set_direction(Vec2::ZERO);
    }
}

/// Applies a held client boost action to the predicted context.
fn apply_local_boost(
    trigger: On<'_, '_, Fire<Boost>>,
    mut players: Query<
        '_,
        '_,
        &mut MovementIntent,
        (
            With<PlayerInput>,
            With<Remote>,
            With<Predicted>,
            Without<Interpolated>,
            Without<Replicate>,
        ),
    >,
    rollback_clients: Query<'_, '_, (), With<Rollback>>,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    let Ok(mut intent) = players.get_mut(trigger.context) else {
        return;
    };

    intent.set_boosting(trigger.value);
    if rollback_clients.is_empty() {
        // Preserve monotonic external evidence while still applying the action
        // value on every historical simulation tick.
        status.local_action_receipts = status.local_action_receipts.saturating_add(1);
        status.boost_action_receipts = status.boost_action_receipts.saturating_add(1);
    }
}

/// Clears predicted boost state when the button is released.
fn clear_local_boost(
    trigger: On<'_, '_, Complete<Boost>>,
    mut players: Query<
        '_,
        '_,
        &mut MovementIntent,
        (
            With<PlayerInput>,
            With<Remote>,
            With<Predicted>,
            Without<Interpolated>,
            Without<Replicate>,
        ),
    >,
) {
    if let Ok(mut intent) = players.get_mut(trigger.context) {
        intent.set_boosting(false);
    }
}

/// Applies a received movement snapshot to the authoritative context.
fn apply_authoritative_movement(
    trigger: On<'_, '_, Fire<Movement>>,
    actions: Query<'_, '_, (), (With<Action<Movement>>, With<AuthoritativeAction>)>,
    mut players: Query<'_, '_, &mut MovementIntent, (With<PlayerInput>, With<Replicate>)>,
    bridge: Res<'_, ServerStatusBridge>,
) {
    // Ignore locally authored actions on the authority receive path.
    if !actions.contains(trigger.action) {
        return;
    }
    let Ok(mut intent) = players.get_mut(trigger.context) else {
        return;
    };

    // Apply the snapshot before publishing its authority receipt.
    intent.set_direction(trigger.value);
    bridge.record_authoritative_action();
}

/// Clears authoritative movement when the replicated action becomes neutral.
fn clear_authoritative_movement(
    trigger: On<'_, '_, Complete<Movement>>,
    actions: Query<'_, '_, (), (With<Action<Movement>>, With<AuthoritativeAction>)>,
    mut players: Query<'_, '_, &mut MovementIntent, (With<PlayerInput>, With<Replicate>)>,
) {
    if actions.contains(trigger.action)
        && let Ok(mut intent) = players.get_mut(trigger.context)
    {
        intent.set_direction(Vec2::ZERO);
    }
}

/// Applies a received boost snapshot to the authoritative context.
fn apply_authoritative_boost(
    trigger: On<'_, '_, Fire<Boost>>,
    actions: Query<'_, '_, (), (With<Action<Boost>>, With<AuthoritativeAction>)>,
    mut players: Query<'_, '_, &mut MovementIntent, (With<PlayerInput>, With<Replicate>)>,
    bridge: Res<'_, ServerStatusBridge>,
) {
    // Ignore locally authored actions on the authority receive path.
    if !actions.contains(trigger.action) {
        return;
    }
    let Ok(mut intent) = players.get_mut(trigger.context) else {
        return;
    };

    // Apply the snapshot before publishing its authority receipt.
    intent.set_boosting(trigger.value);
    bridge.record_authoritative_action();
}

/// Clears authoritative boost state after the replicated button release.
fn clear_authoritative_boost(
    trigger: On<'_, '_, Complete<Boost>>,
    actions: Query<'_, '_, (), (With<Action<Boost>>, With<AuthoritativeAction>)>,
    mut players: Query<'_, '_, &mut MovementIntent, (With<PlayerInput>, With<Replicate>)>,
) {
    if actions.contains(trigger.action)
        && let Ok(mut intent) = players.get_mut(trigger.context)
    {
        intent.set_boosting(false);
    }
}

/// Retains low-level evidence that BRP injection reached Bevy's input resource.
fn observe_external_keyboard_input(
    keys: Res<'_, ButtonInput<KeyCode>>,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    // These counters complement semantic action and server counters; they do
    // not stand in for the networked action path.
    if keys.just_pressed(KeyCode::Space) {
        status.external_input_receipts = status.external_input_receipts.saturating_add(1);
        status.space_input_receipts = status.space_input_receipts.saturating_add(1);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        status.external_input_receipts = status.external_input_receipts.saturating_add(1);
        status.arrow_right_input_receipts = status.arrow_right_input_receipts.saturating_add(1);
    }
}

/// Caps diagonals at unit length while preserving analog magnitudes below one.
fn normalize_direction(value: Vec2) -> Vec2 {
    if value.length_squared() > 1.0 {
        value.normalize_or_zero()
    } else {
        value
    }
}

/// Covers the public input vocabulary and the private live-input plumbing.
#[cfg(test)]
mod tests;
