//! BRP observability, diagnostics, status logging, and runtime editor support.
//!
//! Diagnostics are installed only in the rendered App. System-information
//! samples can be relatively expensive or unavailable with dynamic linking,
//! while render diagnostics naturally require an active renderer.

use crate::{
    components::{MotionTelemetry, NetworkDemoStatus},
    systems::{DemoSystems, advance_demo_tick, sync_server_status},
};
use avian2d::prelude::{LinearVelocity, Position};
use bevy::{
    diagnostic::{
        EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin,
        SystemInformationDiagnosticsPlugin,
    },
    prelude::*,
    remote::RemotePlugin,
    render::diagnostic::RenderDiagnosticsPlugin,
};
use bevy_brp_extras::BrpExtrasPlugin;
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use lightyear::prelude::{Interpolated, Predicted, Remote, Replicate, Rollback};

/// Installs machine-readable inspection and optional same-binary editor UI.
#[derive(Clone, Copy, Debug, Default)]
pub struct InspectionPlugin {
    /// Whether to add Egui and the world inspector for this launch.
    editor_enabled: bool,
}

impl InspectionPlugin {
    /// Creates inspection support with a runtime-selected editor state.
    #[must_use]
    pub const fn new(editor_enabled: bool) -> Self {
        Self { editor_enabled }
    }
}

impl Plugin for InspectionPlugin {
    /// Adds BRP, diagnostic collectors, status aggregation, and optional UI.
    fn build(&self, app: &mut App) {
        // Publish the runtime branch in the same reflected resource agents use
        // for networking, so editor proof does not depend on a screenshot.
        app.init_resource::<NetworkDemoStatus>();
        app.world_mut()
            .resource_mut::<NetworkDemoStatus>()
            .editor_enabled = self.editor_enabled;
        info!(
            editor_enabled = self.editor_enabled,
            "runtime inspection mode configured"
        );

        // DefaultPlugins already installs FrameCountPlugin. Add the remaining
        // official collectors without duplicating process-global facilities.
        app.add_plugins((
            RemotePlugin::default(),
            LogDiagnosticsPlugin::default(),
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            SystemInformationDiagnosticsPlugin,
            RenderDiagnosticsPlugin,
        ))
        // BRP Extras defensively installs frame diagnostics itself, so add it
        // after the canonical collector list to keep plugin registration unique.
        .add_plugins(BrpExtrasPlugin)
        .configure_sets(First, DemoSystems::Status)
        .add_systems(First, sync_server_status.in_set(DemoSystems::Status))
        .add_systems(FixedFirst, advance_demo_tick)
        .add_systems(
            FixedLast,
            log_network_status.in_set(DemoSystems::FixedTelemetry),
        );

        if self.editor_enabled {
            // This is a runtime branch in every build: players and agents use
            // the exact same executable as developers when inspection is needed.
            app.add_plugins((EguiPlugin::default(), WorldInspectorPlugin::new()));
        }
    }
}

/// Emits periodic connection, replication, action, and motion evidence.
fn log_network_status(
    status: Res<'_, NetworkDemoStatus>,
    rollback_clients: Query<'_, '_, (), With<Rollback>>,
    bodies: Query<
        '_,
        '_,
        (
            &Name,
            &Position,
            &LinearVelocity,
            &MotionTelemetry,
            Has<Predicted>,
            Has<Interpolated>,
            Has<Remote>,
            Has<Replicate>,
        ),
    >,
) {
    if !rollback_clients.is_empty()
        || status.fixed_tick == 0
        || !status.fixed_tick.is_multiple_of(STATUS_LOG_PERIOD_TICKS)
    {
        return;
    }

    info!(
        tick = status.fixed_tick,
        server_tick = status.server_fixed_tick,
        client_connected = status.client_connected,
        input_timeline_synced = status.input_timeline_synced,
        server_mirror_connected = status.server_mirror_connected,
        client_connection_events = status.client_connection_events,
        server_connection_events = status.server_connection_events,
        replication_senders = status.replication_senders,
        predicted_entities = status.predicted_entities,
        interpolated_entities = status.interpolated_entities,
        interpolation_histories = status.interpolation_histories,
        interpolation_samples = status.interpolation_samples,
        interpolation_ready = status.interpolation_ready,
        interpolation_deadline_missed = status.interpolation_deadline_missed,
        input_receipts = status.input_receipts,
        external_input_receipts = status.external_input_receipts,
        local_action_receipts = status.local_action_receipts,
        server_action_receipts = status.server_action_receipts,
        movement_action_receipts = status.movement_action_receipts,
        boost_action_receipts = status.boost_action_receipts,
        "network demo status"
    );
    for (name, position, velocity, telemetry, predicted, interpolated, remote, replicate) in &bodies
    {
        info!(
            body = %name,
            position = ?position.0,
            velocity = ?velocity.0,
            predicted,
            interpolated,
            remote,
            replicate,
            fixed_ticks = telemetry.fixed_ticks(),
            direction_changes = telemetry.direction_changes(),
            input_receipts = telemetry.input_receipts(),
            distance = telemetry.distance_travelled(),
            "network body telemetry"
        );
    }
}

/// Fixed-tick interval between structured observability snapshots.
const STATUS_LOG_PERIOD_TICKS: u64 = 128;

/// Covers runtime construction choices without opening an OS window.
#[cfg(all(test, not(coverage)))]
mod tests {
    use super::InspectionPlugin;
    use crate::components::NetworkDemoStatus;
    use bevy::{
        diagnostic::{
            EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin,
            SystemInformationDiagnosticsPlugin,
        },
        prelude::{App, Assets, Shader},
    };
    use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};

    /// Keeps the editor disabled by default and runtime-selectable when requested.
    #[test]
    fn editor_choice_is_runtime_data() {
        assert!(!InspectionPlugin::default().editor_enabled);
        assert!(InspectionPlugin::new(true).editor_enabled);
    }

    /// Installs non-render diagnostic collectors in a focused App.
    #[test]
    fn diagnostics_are_registered_without_starting_a_window() {
        let mut app = App::new();
        app.add_plugins(InspectionPlugin::new(false));

        assert!(app.is_plugin_added::<LogDiagnosticsPlugin>());
        assert!(app.is_plugin_added::<FrameTimeDiagnosticsPlugin>());
        assert!(app.is_plugin_added::<EntityCountDiagnosticsPlugin>());
        assert!(app.is_plugin_added::<SystemInformationDiagnosticsPlugin>());
        assert!(!app.is_plugin_added::<EguiPlugin>());
        assert!(!app.is_plugin_added::<WorldInspectorPlugin>());
        assert!(!app.world().resource::<NetworkDemoStatus>().editor_enabled);
    }

    /// Adds editor plugins only for the runtime-enabled branch of the same build.
    #[test]
    fn editor_flag_changes_plugin_composition() {
        let mut app = App::new();
        // Egui embeds its WGSL during plugin build. A renderer-free fixture can
        // still exercise composition by providing only that asset collection.
        app.init_resource::<Assets<Shader>>();
        app.add_plugins(InspectionPlugin::new(true));

        assert!(app.is_plugin_added::<EguiPlugin>());
        assert!(app.is_plugin_added::<WorldInspectorPlugin>());
        assert!(app.world().resource::<NetworkDemoStatus>().editor_enabled);
    }
}
