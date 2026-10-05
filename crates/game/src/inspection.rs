//! BRP observability, diagnostics, status logging, and runtime editor support.
//!
//! Diagnostics are installed only in the rendered App. System-information
//! samples can be relatively expensive or unavailable with dynamic linking,
//! while render diagnostics naturally require an active renderer.

use crate::components::MotionTelemetry;
use crate::components::NetworkDemoStatus;
use crate::systems::DemoSystems;
use crate::systems::advance_demo_tick;
use crate::systems::sync_server_status;
use bevy::diagnostic::EntityCountDiagnosticsPlugin;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::diagnostic::LogDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
#[cfg(all(feature = "mcp", not(target_family = "wasm")))]
use bevy_brp_extras::BrpExtrasPlugin;
#[cfg(all(feature = "mcp", not(target_family = "wasm")))]
use bevy_inspector_egui::bevy_egui::EguiPlugin;
#[cfg(all(feature = "mcp", not(target_family = "wasm")))]
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use game_physics::LinearVelocity;
use game_physics::Position;
use lightyear::prelude::Interpolated;
use lightyear::prelude::Predicted;
use lightyear::prelude::Remote;
use lightyear::prelude::Replicate;
use lightyear::prelude::Rollback;

/// Installs machine-readable inspection and optional same-binary editor UI.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct InspectionPlugin {
    /// Whether to add Egui and the world inspector for this launch.
    is_editor_enabled: bool,
}

impl InspectionPlugin {
    /// Creates inspection support with a runtime-selected editor state.
    #[must_use]
    pub(crate) const fn new(is_editor_enabled: bool) -> Self {
        Self { is_editor_enabled }
    }
}

impl Plugin for InspectionPlugin {
    /// Adds BRP, diagnostic collectors, status aggregation, and optional UI.
    fn build(&self, app: &mut App) {
        // Publish the runtime branch in the same reflected resource agents use
        // for networking, so editor proof does not depend on a screenshot.
        app.world_mut()
            .get_resource_or_init::<NetworkDemoStatus>()
            .is_editor_enabled = self.is_editor_enabled;
        info!(
            is_editor_enabled = self.is_editor_enabled,
            "runtime inspection mode configured"
        );

        // DefaultPlugins already installs FrameCountPlugin. Add the remaining
        // official collectors without duplicating process-global facilities.
        app.add_plugins((
            LogDiagnosticsPlugin::default(),
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            RenderDiagnosticsPlugin,
        ))
        // BRP Extras defensively installs frame diagnostics itself, so add it
        // after the canonical collector list to keep plugin registration unique.
        .configure_sets(First, DemoSystems::Status)
        .add_systems(First, sync_server_status.in_set(DemoSystems::Status))
        .add_systems(FixedFirst, advance_demo_tick)
        .add_systems(
            FixedLast,
            log_network_status.in_set(DemoSystems::FixedTelemetry),
        );

        #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
        app.add_plugins((
            bevy::remote::RemotePlugin::default(),
            bevy::diagnostic::SystemInformationDiagnosticsPlugin,
            BrpExtrasPlugin,
        ));

        #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
        if self.is_editor_enabled {
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
    // Skip rollback frames and emit only on the fixed observation cadence.
    let is_log_tick = status.fixed_tick.is_multiple_of(STATUS_LOG_PERIOD_TICKS);
    if !rollback_clients.is_empty() || status.fixed_tick == 0 || !is_log_tick {
        return;
    }

    // Publish aggregate connection and action evidence before per-body motion details.
    info!(
        tick = status.fixed_tick,
        server_tick = status.server_fixed_tick,
        is_client_connected = status.is_client_connected,
        is_input_timeline_synced = status.is_input_timeline_synced,
        is_server_mirror_connected = status.is_server_mirror_connected,
        client_connection_events = status.client_connection_events,
        server_connection_events = status.server_connection_events,
        replication_senders = status.replication_senders,
        predicted_entities = status.predicted_entities,
        interpolated_entities = status.interpolated_entities,
        interpolation_histories = status.interpolation_histories,
        interpolation_samples = status.interpolation_samples,
        is_interpolation_ready = status.is_interpolation_ready,
        is_interpolation_deadline_missed = status.is_interpolation_deadline_missed,
        input_receipts = status.input_receipts,
        external_input_receipts = status.external_input_receipts,
        local_action_receipts = status.local_action_receipts,
        server_action_receipts = status.server_action_receipts,
        movement_action_receipts = status.movement_action_receipts,
        boost_action_receipts = status.boost_action_receipts,
        "network demo status"
    );
    // Report corrected receiver state with its prediction and authority markers.
    for (name, position, velocity, telemetry, predicted, interpolated, remote, replicate) in &bodies
    {
        // Keep position, velocity, and counters in the same body observation.
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
#[cfg(all(test, feature = "mcp", not(target_family = "wasm")))]
mod tests {
    use super::InspectionPlugin;
    use crate::components::NetworkDemoStatus;
    use bevy::diagnostic::EntityCountDiagnosticsPlugin;
    use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
    use bevy::diagnostic::LogDiagnosticsPlugin;
    use bevy::prelude::App;
    use bevy::prelude::Assets;
    use bevy::prelude::Shader;
    #[cfg(all(feature = "mcp", not(target_family = "wasm")))]
    use bevy_inspector_egui::bevy_egui::EguiPlugin;
    use bevy_inspector_egui::quick::WorldInspectorPlugin;

    /// Keeps the editor disabled by default and runtime-selectable when
    /// requested.
    #[test]
    fn editor_choice_is_runtime_data() {
        assert!(!InspectionPlugin::default().is_editor_enabled);
        assert!(InspectionPlugin::new(true).is_editor_enabled);
    }

    /// Installs non-render diagnostic collectors in a focused App.
    #[test]
    fn diagnostics_are_registered_without_starting_a_window() {
        let mut app = App::new();
        app.add_plugins(InspectionPlugin::new(false));

        assert!(app.is_plugin_added::<LogDiagnosticsPlugin>());
        assert!(app.is_plugin_added::<FrameTimeDiagnosticsPlugin>());
        assert!(app.is_plugin_added::<EntityCountDiagnosticsPlugin>());
        assert!(app.is_plugin_added::<bevy::diagnostic::SystemInformationDiagnosticsPlugin>());
        assert!(!app.is_plugin_added::<EguiPlugin>());
        assert!(!app.is_plugin_added::<WorldInspectorPlugin>());
        assert!(
            !app.world()
                .resource::<NetworkDemoStatus>()
                .is_editor_enabled
        );
    }

    /// Adds editor plugins only for the runtime-enabled branch of the same
    /// build.
    #[test]
    fn editor_flag_changes_plugin_composition() {
        let mut app = App::new();
        // Egui embeds its WGSL during plugin build. A renderer-free fixture can
        // still exercise composition by providing only that asset collection.
        app.init_resource::<Assets<Shader>>();
        app.add_plugins(InspectionPlugin::new(true));

        assert!(app.is_plugin_added::<EguiPlugin>());
        assert!(app.is_plugin_added::<WorldInspectorPlugin>());
        assert!(
            app.world()
                .resource::<NetworkDemoStatus>()
                .is_editor_enabled
        );
    }
}
