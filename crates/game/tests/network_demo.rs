//! Deterministic end-to-end coverage for the two-world Lightyear demonstration.

use bevy::prelude::AppExit;
use bevy::prelude::Vec2;
use game::HeadlessNetworkHarness;
use game_settings::Config;

/// Maximum paired updates allowed for connection and snapshot readiness.
const READY_STEP_LIMIT: usize = 512;
/// Paired updates used to prove sustained network input and movement.
const INPUT_STEP_COUNT: usize = 64;
/// Paired updates used to prove both executors remain live after the input
/// probe.
const POST_INPUT_STEP_COUNT: usize = 2;
/// Paired updates allowed for a released input to clear buffered network ticks.
const INPUT_RELEASE_STEP_COUNT: usize = 16;

/// Proves connection, prediction, interpolation, input replication, and
/// liveness.
#[test]
fn headless_network_demo_exercises_the_real_protocol() {
    let mut harness = HeadlessNetworkHarness::new(Config::default());

    let mut ready = false;
    for _ in 0..READY_STEP_LIMIT {
        harness.step();
        let status = harness.status();
        if status.is_network_ready() && status.server_fixed_tick > 0 {
            ready = true;
            break;
        }
    }
    assert!(
        ready,
        "network did not become ready within the bounded harness"
    );

    let status_before_input = harness.status();
    let authoritative_before = harness
        .authoritative_player_position()
        .expect("authority should have spawned the player");
    let authoritative_velocity_before = harness
        .authoritative_player_velocity()
        .expect("authority should expose the player's velocity");
    assert_eq!(harness.authoritative_player_boosting(), Some(false));
    let predicted_before = harness
        .predicted_player_position()
        .expect("client should have received the predicted player");
    assert!(harness.is_mock_actions_applied(Vec2::X, true));

    for _ in 0..INPUT_STEP_COUNT {
        harness.step();
    }

    let status = harness.status();
    let authoritative_after = harness
        .authoritative_player_position()
        .expect("authoritative player should remain present");
    let authoritative_velocity_after = harness
        .authoritative_player_velocity()
        .expect("authoritative velocity should remain observable");
    let predicted_after = harness
        .predicted_player_position()
        .expect("predicted player should remain present");
    assert!(
        status.local_action_receipts >= status_before_input.local_action_receipts.saturating_add(2)
    );
    assert!(
        status.server_action_receipts
            >= status_before_input.server_action_receipts.saturating_add(2)
    );
    assert!(status.movement_action_receipts > status_before_input.movement_action_receipts);
    assert!(status.boost_action_receipts > status_before_input.boost_action_receipts);
    assert!(status.fixed_tick > status_before_input.fixed_tick);
    assert!(status.server_fixed_tick > status_before_input.server_fixed_tick);

    // A positive horizontal authoritative velocity proves Movement crossed the
    // transport; the server-side semantic state independently proves Boost did.
    assert!(authoritative_velocity_after.x > 0.0);
    assert!(authoritative_velocity_after.y.abs() <= f32::EPSILON);
    assert!(authoritative_velocity_after.length() > authoritative_velocity_before.length());
    assert_eq!(harness.authoritative_player_boosting(), Some(true));
    assert_ne!(authoritative_after, authoritative_before);
    assert_ne!(predicted_after, predicted_before);

    assert!(harness.is_actions_released());
    for _ in 0..POST_INPUT_STEP_COUNT {
        harness.step();
    }
    let status_after_input = harness.status();
    assert!(status_after_input.fixed_tick > status.fixed_tick);
    assert!(status_after_input.server_fixed_tick > status.server_fixed_tick);
    assert_eq!(
        harness.shutdown(),
        Some([AppExit::Success, AppExit::Success])
    );
}

/// Proves keyboard messages use the same authoritative path as BRP automation.
#[test]
fn headless_network_demo_transports_live_keyboard_bindings() {
    let mut harness = HeadlessNetworkHarness::new(Config::default());

    let mut ready = false;
    for _ in 0..READY_STEP_LIMIT {
        harness.step();
        if harness.status().is_network_ready() {
            ready = true;
            break;
        }
    }
    assert!(ready, "live-input fixture did not become ready");
    assert!(harness.is_live_input_actions_ready());

    let status_before_input = harness.status();
    harness.press_demo_keys();
    for _ in 0..INPUT_STEP_COUNT {
        harness.step();
    }

    let status = harness.status();
    let authoritative_velocity = harness
        .authoritative_player_velocity()
        .expect("authority should expose the player's velocity");
    assert!(status.external_input_receipts >= status_before_input.external_input_receipts + 2);
    assert!(status.local_action_receipts >= status_before_input.local_action_receipts + 2);
    assert!(status.server_action_receipts >= status_before_input.server_action_receipts + 2);
    assert!(status.movement_action_receipts > status_before_input.movement_action_receipts);
    assert!(status.boost_action_receipts > status_before_input.boost_action_receipts);
    assert!(authoritative_velocity.x > 0.0);
    assert!(authoritative_velocity.y.abs() <= f32::EPSILON);
    assert_eq!(harness.authoritative_player_boosting(), Some(true));
    assert!(harness.is_live_input_actions_ready());

    harness.release_demo_keys();
    for _ in 0..INPUT_RELEASE_STEP_COUNT {
        harness.step();
    }
    assert_eq!(harness.authoritative_player_velocity(), Some(Vec2::ZERO));
    assert_eq!(harness.authoritative_player_boosting(), Some(false));
    assert_eq!(
        harness.shutdown(),
        Some([AppExit::Success, AppExit::Success])
    );
}
