//! Cross-domain schedule labels and small observability bridge systems.
//!
//! Feature-local behavior remains with its owning plugin. These contracts exist
//! only where networking, simulation, and inspection need an explicit shared
//! ordering vocabulary.

use crate::components::NetworkDemoStatus;
use crate::components::ServerStatusBridge;
use bevy::prelude::DetectChangesMut;
use bevy::prelude::Query;
use bevy::prelude::Res;
use bevy::prelude::ResMut;
use bevy::prelude::SystemSet;
use bevy::prelude::With;
use lightyear::prelude::Rollback;

/// Names the high-level phases that multiple feature plugins coordinate around.
///
/// Variants are labels, not an implicit order. The composition plugin must use
/// `configure_sets`, `before`, or `after` within each relevant schedule.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) enum DemoSystems {
    /// Deterministic authoritative or predicted fixed-step simulation.
    Simulation,
    /// Fixed-step telemetry that requires completed simulation state.
    FixedTelemetry,
    /// Frame-level synchronization of evidence used by inspection tooling.
    Status,
    /// Render presentation after Lightyear interpolation and correction.
    Presentation,
}

/// Copies monotonic authority evidence into the BRP-visible client resource.
pub(crate) fn sync_server_status(
    bridge: Res<'_, ServerStatusBridge>,
    mut status: ResMut<'_, NetworkDemoStatus>,
) {
    // ECS worlds never cross threads; only copy the bridge's atomic snapshot.
    let mut next = *status;
    next.apply_server_snapshot(bridge.snapshot());
    // BRP polls this resource frequently. Mark it changed only when the server's
    // monotonic evidence actually advanced, preserving useful change detection.
    status.set_if_neq(next);
}

/// Advances the reflected fixed-schedule heartbeat without wrapping.
pub(crate) fn advance_demo_tick(
    mut status: ResMut<'_, NetworkDemoStatus>,
    rollback_clients: Query<'_, '_, (), With<Rollback>>,
) {
    if !rollback_clients.is_empty() {
        // Lightyear re-enters FixedMain for historical ticks. The heartbeat is
        // an executor-liveness signal, not simulated time, so do not replay it.
        return;
    }
    // A monotonically increasing heartbeat lets BRP detect executor stalls.
    status.record_fixed_tick();
}

/// Verifies cross-domain status systems with the smallest viable Bevy Apps.
#[cfg(test)]
mod tests {
    use super::advance_demo_tick;
    use super::sync_server_status;
    use crate::components::NetworkDemoStatus;
    use crate::components::ServerStatusBridge;
    use bevy::ecs::schedule::LogLevel;
    use bevy::ecs::schedule::ScheduleBuildSettings;
    use bevy::ecs::schedule::common_conditions::resource_changed;
    use bevy::prelude::App;
    use bevy::prelude::IntoScheduleConfigs;
    use bevy::prelude::ResMut;
    use bevy::prelude::Resource;
    use bevy::prelude::Update;
    use bevy::prelude::default;
    use lightyear::prelude::Rollback;

    /// Counts schedule runs gated by a real status-resource change.
    #[derive(Default, Resource)]
    struct StatusChangeRuns(
        /// Number of times the change-gated consumer executed.
        u32,
    );

    /// Records one execution of a status change-detection consumer.
    fn count_status_change(mut runs: ResMut<'_, StatusChangeRuns>) {
        runs.0 = runs.0.saturating_add(1);
    }

    /// Elevates project-system ambiguities to test failures in a focused App.
    fn reject_schedule_ambiguities(app: &mut App) {
        // Keep this strict setting on tiny project-owned schedules. Applying it
        // to the integration harness would report third-party engine internals.
        app.configure_schedules(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..default()
        });
    }

    /// Copies every server bridge field into reflected client status.
    #[test]
    fn server_status_is_synchronized() {
        let mut app = App::new();
        let bridge = ServerStatusBridge::default();
        bridge.record_connection();
        bridge.record_replication_sender();
        bridge.record_authoritative_action();
        bridge.record_server_tick();
        app.insert_resource(bridge)
            .init_resource::<NetworkDemoStatus>()
            .add_systems(Update, sync_server_status);
        reject_schedule_ambiguities(&mut app);

        app.update();

        let status = app.world().resource::<NetworkDemoStatus>();
        assert!(status.is_server_mirror_connected);
        assert_eq!(status.server_connection_events, 1);
        assert_eq!(status.replication_senders, 1);
        assert_eq!(status.server_action_receipts, 1);
        assert_eq!(status.server_fixed_tick, 1);
    }

    /// Leaves downstream change detection quiet until the atomic snapshot
    /// advances.
    #[test]
    fn equal_server_snapshot_is_not_republished_as_changed() {
        let mut app = App::new();
        let bridge = ServerStatusBridge::default();
        app.insert_resource(bridge.clone())
            .init_resource::<NetworkDemoStatus>()
            .init_resource::<StatusChangeRuns>()
            .add_systems(
                Update,
                (
                    sync_server_status,
                    count_status_change.run_if(resource_changed::<NetworkDemoStatus>),
                )
                    .chain(),
            );
        reject_schedule_ambiguities(&mut app);

        // A newly inserted resource is changed once, but publishing the same
        // atomic snapshot on the next update must not wake consumers again.
        app.update();
        app.update();
        assert_eq!(app.world().resource::<StatusChangeRuns>().0, 1);

        bridge.record_server_tick();
        app.update();
        assert_eq!(app.world().resource::<StatusChangeRuns>().0, 2);
    }

    /// Advances the heartbeat once per system execution.
    #[test]
    fn fixed_tick_advances() {
        let mut app = App::new();
        app.init_resource::<NetworkDemoStatus>()
            .add_systems(Update, advance_demo_tick);
        reject_schedule_ambiguities(&mut app);

        app.update();
        app.update();

        assert_eq!(app.world().resource::<NetworkDemoStatus>().fixed_tick, 2);
    }

    /// Saturates the heartbeat at the integer limit instead of wrapping.
    #[test]
    fn fixed_tick_saturates() {
        let mut app = App::new();
        app.insert_resource(NetworkDemoStatus {
            fixed_tick: u64::MAX,
            ..NetworkDemoStatus::default()
        })
        .add_systems(Update, advance_demo_tick);
        reject_schedule_ambiguities(&mut app);

        app.update();

        assert_eq!(
            app.world().resource::<NetworkDemoStatus>().fixed_tick,
            u64::MAX
        );
    }

    /// Leaves the wall-clock heartbeat unchanged during historical replay.
    #[test]
    fn fixed_tick_ignores_rollback_replay() {
        let mut app = App::new();
        app.init_resource::<NetworkDemoStatus>()
            .add_systems(Update, advance_demo_tick);
        let client = app.world_mut().spawn(Rollback::FromState).id();
        reject_schedule_ambiguities(&mut app);

        app.update();
        assert_eq!(app.world().resource::<NetworkDemoStatus>().fixed_tick, 0);

        app.world_mut().entity_mut(client).remove::<Rollback>();
        app.update();
        assert_eq!(app.world().resource::<NetworkDemoStatus>().fixed_tick, 1);
    }
}
