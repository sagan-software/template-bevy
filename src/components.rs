//! ECS data shared by networking, simulation, presentation, and inspection.
//!
//! Feature-local data belongs with its owning plugin. The contracts here cross
//! at least two domains and therefore benefit from one documented definition.

use bevy::{
    ecs::reflect::{ReflectComponent, ReflectResource},
    prelude::{App, Component, Reflect, Resource, Vec2},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

/// Aggregate connection, replication, input, and liveness evidence exposed by BRP.
///
/// The explicit type-path override preserves the existing external resource
/// name, `template_bevy::NetworkDemoStatus`, after moving this type out of the
/// binary crate root.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent BRP probes are clearer than an artificial combined state machine"
)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Reflect, Resource)]
#[reflect(Resource)]
#[type_path = "template_bevy"]
pub struct NetworkDemoStatus {
    /// Whether this launch installed the runtime editor and world inspector.
    pub editor_enabled: bool,
    /// Whether the rendered raw client completed its connection lifecycle.
    pub client_connected: bool,
    /// Whether Lightyear synchronized the timeline that buffers and sends inputs.
    pub input_timeline_synced: bool,
    /// Whether the headless server accepted its Crossbeam mirror connection.
    pub server_mirror_connected: bool,
    /// Number of client-side connection events observed.
    pub client_connection_events: u32,
    /// Number of server-side connection events observed.
    pub server_connection_events: u32,
    /// Number of server connections equipped to send replication.
    pub replication_senders: u32,
    /// Number of receiver-created predicted entities observed by the client.
    pub predicted_entities: u32,
    /// Number of receiver-created interpolated entities observed by the client.
    pub interpolated_entities: u32,
    /// Number of interpolation histories created by the receive path.
    pub interpolation_histories: u32,
    /// Largest number of authoritative samples observed in one history.
    pub interpolation_samples: usize,
    /// Whether at least two authoritative snapshots are available to interpolate.
    pub interpolation_ready: bool,
    /// Whether interpolation failed to become ready before its deadline.
    pub interpolation_deadline_missed: bool,
    /// Number of fixed client ticks completed.
    pub fixed_tick: u64,
    /// Number of fixed authoritative server ticks completed.
    pub server_fixed_tick: u64,
    /// Number of predicted simulation inputs consumed by the client.
    pub input_receipts: u64,
    /// Number of keyboard inputs injected externally or entered physically.
    pub external_input_receipts: u64,
    /// Number of externally observed Space presses.
    pub space_input_receipts: u64,
    /// Number of externally observed right-arrow presses.
    pub arrow_right_input_receipts: u64,
    /// Number of semantic actions generated in the rendered client world.
    pub local_action_receipts: u64,
    /// Number of replicated semantic actions consumed by the authority.
    pub server_action_receipts: u64,
    /// Number of local movement-action triggers observed.
    pub movement_action_receipts: u64,
    /// Number of local boost-action triggers observed.
    pub boost_action_receipts: u64,
}

impl NetworkDemoStatus {
    /// Reports whether both peers and both receiver paths are established.
    #[must_use]
    pub const fn network_ready(&self) -> bool {
        self.client_connected
            && self.input_timeline_synced
            && self.server_mirror_connected
            && self.client_connection_events > 0
            && self.server_connection_events > 0
            && self.replication_senders > 0
            && self.predicted_entities > 0
            && self.interpolated_entities > 0
            && self.interpolation_histories > 0
            && self.interpolation_samples >= 2
            && self.interpolation_ready
            && !self.interpolation_deadline_missed
    }

    /// Reports whether a semantic input was produced locally and consumed remotely.
    #[must_use]
    pub const fn input_path_verified(&self) -> bool {
        self.local_action_receipts > 0 && self.server_action_receipts > 0
    }

    /// Reports whether networking, fixed updates, and authoritative input are live.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.network_ready()
            && self.fixed_tick > 0
            && self.server_fixed_tick > 0
            && self.input_receipts > 0
            && self.input_path_verified()
    }

    /// Copies the latest thread-safe server evidence into this reflected resource.
    pub(crate) const fn apply_server_snapshot(&mut self, snapshot: ServerStatusSnapshot) {
        self.server_mirror_connected = snapshot.mirror_connected;
        self.server_connection_events = snapshot.connection_events;
        self.replication_senders = snapshot.replication_senders;
        self.server_action_receipts = snapshot.action_receipts;
        self.server_fixed_tick = snapshot.fixed_tick;
    }

    /// Records one completed fixed client tick without risking integer overflow.
    pub(crate) const fn record_fixed_tick(&mut self) {
        self.fixed_tick = self.fixed_tick.saturating_add(1);
    }
}

/// Distinguishes the two network receive paths demonstrated by a body.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect)]
pub enum NetworkPath {
    /// Client prediction plus correction of a locally controlled body.
    Prediction,
    /// Snapshot interpolation of a server-authoritative remote body.
    Interpolation,
}

/// Identifies a networked demonstration body and its collision/render extent.
#[derive(Clone, Copy, Component, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct DemoBody {
    /// Network receive path this body teaches.
    path: NetworkPath,
    /// Half-size used by collision and presentation code.
    half_extent: Vec2,
}

impl DemoBody {
    /// Creates metadata for a client-predicted circular body.
    #[must_use]
    pub const fn predicted(radius: f32) -> Self {
        Self {
            path: NetworkPath::Prediction,
            half_extent: Vec2::splat(radius),
        }
    }

    /// Creates metadata for a snapshot-interpolated rectangular body.
    #[must_use]
    pub const fn interpolated(half_extent: Vec2) -> Self {
        Self {
            path: NetworkPath::Interpolation,
            half_extent,
        }
    }

    /// Returns the network receive path this body demonstrates.
    #[must_use]
    pub const fn path(&self) -> NetworkPath {
        self.path
    }

    /// Returns the body's collision and presentation half-size.
    #[must_use]
    pub const fn half_extent(&self) -> Vec2 {
        self.half_extent
    }
}

/// Records deterministic motion evidence for one simulation body.
#[derive(Clone, Copy, Component, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub struct MotionTelemetry {
    /// Position recorded by the preceding simulation step.
    previous_position: Vec2,
    /// Number of fixed steps consumed by this body.
    fixed_ticks: u64,
    /// Number of boundary responses that reversed its direction.
    direction_changes: u32,
    /// Number of simulation inputs consumed by this body.
    input_receipts: u64,
    /// Total world-space distance accumulated across fixed steps.
    distance_travelled: f32,
}

impl MotionTelemetry {
    /// Initializes motion evidence at an authoritative spawn position.
    #[must_use]
    pub const fn at(position: Vec2) -> Self {
        Self {
            previous_position: position,
            fixed_ticks: 0,
            direction_changes: 0,
            input_receipts: 0,
            distance_travelled: 0.0,
        }
    }

    /// Records one deterministic simulation step.
    pub fn record_step(&mut self, position: Vec2, direction_changed: bool, input_consumed: bool) {
        // Measure from the preceding sample before advancing the stored anchor.
        self.distance_travelled += (position - self.previous_position).length();
        self.previous_position = position;

        // Saturating counters keep long-running observability from wrapping.
        self.fixed_ticks = self.fixed_ticks.saturating_add(1);
        if input_consumed {
            self.input_receipts = self.input_receipts.saturating_add(1);
        }
        if direction_changed {
            self.direction_changes = self.direction_changes.saturating_add(1);
        }
    }

    /// Realigns the distance anchor without recording a replayed simulation step.
    pub(crate) const fn resynchronize_position(&mut self, position: Vec2) {
        self.previous_position = position;
    }

    /// Returns the most recently sampled position.
    #[must_use]
    pub const fn previous_position(&self) -> Vec2 {
        self.previous_position
    }

    /// Returns the number of fixed steps recorded for this body.
    #[must_use]
    pub const fn fixed_ticks(&self) -> u64 {
        self.fixed_ticks
    }

    /// Returns the number of recorded boundary direction changes.
    #[must_use]
    pub const fn direction_changes(&self) -> u32 {
        self.direction_changes
    }

    /// Returns the number of simulation inputs consumed by this body.
    #[must_use]
    pub const fn input_receipts(&self) -> u64 {
        self.input_receipts
    }

    /// Returns total world-space distance accumulated by this body.
    #[must_use]
    pub const fn distance_travelled(&self) -> f32 {
        self.distance_travelled
    }
}

/// Publishes server evidence across the App/thread boundary without sharing ECS state.
#[derive(Clone, Debug, Default, Resource)]
pub(crate) struct ServerStatusBridge(
    /// Atomics shared by the independently owned client and server Worlds.
    Arc<ServerStatusAtoms>,
);

impl ServerStatusBridge {
    /// Records a connected server-side Crossbeam mirror.
    pub(crate) fn record_connection(&self) {
        // Release writes pair with the client world's Acquire snapshot loads.
        self.0.mirror_connected.store(true, Ordering::Release);
        saturating_increment_u32(&self.0.connection_events);
    }

    /// Records installation of one server replication sender.
    pub(crate) fn record_replication_sender(&self) {
        saturating_increment_u32(&self.0.replication_senders);
    }

    /// Records one semantic action consumed by the authoritative server.
    pub(crate) fn record_authoritative_action(&self) {
        saturating_increment_u64(&self.0.action_receipts);
    }

    /// Records one completed authoritative fixed tick.
    pub(crate) fn record_server_tick(&self) {
        saturating_increment_u64(&self.0.fixed_tick);
    }

    /// Captures one internally consistent-enough observability sample.
    #[must_use]
    pub(crate) fn snapshot(&self) -> ServerStatusSnapshot {
        // Each field is monotonic, so independent Acquire loads are sufficient
        // for telemetry even though this is not a transactional game-state view.
        ServerStatusSnapshot {
            mirror_connected: self.0.mirror_connected.load(Ordering::Acquire),
            connection_events: self.0.connection_events.load(Ordering::Acquire),
            replication_senders: self.0.replication_senders.load(Ordering::Acquire),
            action_receipts: self.0.action_receipts.load(Ordering::Acquire),
            fixed_tick: self.0.fixed_tick.load(Ordering::Acquire),
        }
    }
}

/// Atomic counters owned jointly by the client and server thread handles.
#[derive(Debug, Default)]
struct ServerStatusAtoms {
    /// Whether the authority accepted its raw mirror connection.
    mirror_connected: AtomicBool,
    /// Number of server connection lifecycle events observed.
    connection_events: AtomicU32,
    /// Number of replication senders installed on server connections.
    replication_senders: AtomicU32,
    /// Number of semantic actions consumed by the authority.
    action_receipts: AtomicU64,
    /// Number of completed authoritative fixed ticks.
    fixed_tick: AtomicU64,
}

/// Copyable view of the atomics consumed by the rendered client world.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ServerStatusSnapshot {
    /// Whether the raw mirror is connected.
    mirror_connected: bool,
    /// Number of server connection events.
    connection_events: u32,
    /// Number of installed replication senders.
    replication_senders: u32,
    /// Number of authoritative action receipts.
    action_receipts: u64,
    /// Number of completed authoritative fixed ticks.
    fixed_tick: u64,
}

/// Atomically increments a 32-bit observability counter without wrapping.
fn saturating_increment_u32(counter: &AtomicU32) {
    // `fetch_update` retries if another thread wins the race, while saturation
    // preserves the monotonic status contract at the integer limit.
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
        Some(value.saturating_add(1))
    });
}

/// Atomically increments a 64-bit observability counter without wrapping.
fn saturating_increment_u64(counter: &AtomicU64) {
    // The closure always returns `Some`, so failure only causes an internal
    // compare-exchange retry rather than dropping an observation.
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
        Some(value.saturating_add(1))
    });
}

/// Registers shared reflected contracts in one canonical order.
pub(crate) fn register_shared_types(app: &mut App) {
    // A single helper prevents role-specific plugin code from drifting into
    // incompatible or incomplete reflection registration lists.
    app.register_type::<NetworkDemoStatus>()
        .register_type::<NetworkPath>()
        .register_type::<DemoBody>()
        .register_type::<MotionTelemetry>();
}

/// Covers reflected compatibility, domain constructors, and shared telemetry.
#[cfg(all(test, not(coverage)))]
mod tests {
    use super::{
        DemoBody, MotionTelemetry, NetworkDemoStatus, NetworkPath, ServerStatusBridge,
        register_shared_types,
    };
    use bevy::{
        prelude::{App, AppTypeRegistry, TypePath, Vec2},
        reflect::TypeRegistry,
    };
    use std::any::TypeId;

    /// Preserves the BRP resource name used by agent tooling.
    #[test]
    fn status_keeps_its_root_type_path() {
        assert_eq!(
            <NetworkDemoStatus as TypePath>::type_path(),
            "template_bevy::NetworkDemoStatus"
        );
    }

    /// Keeps each body constructor explicit about its network presentation path.
    #[test]
    fn body_constructors_preserve_shape_and_path() {
        let predicted = DemoBody::predicted(24.0);
        let interpolated = DemoBody::interpolated(Vec2::new(28.0, 22.0));

        assert_eq!(predicted.path(), NetworkPath::Prediction);
        assert_eq!(predicted.half_extent(), Vec2::splat(24.0));
        assert_eq!(interpolated.path(), NetworkPath::Interpolation);
        assert_eq!(interpolated.half_extent(), Vec2::new(28.0, 22.0));
    }

    /// Records distance, receipts, ticks, and direction changes together.
    #[test]
    fn motion_telemetry_records_a_step() {
        let mut telemetry = MotionTelemetry::at(Vec2::ZERO);

        telemetry.record_step(Vec2::new(3.0, 4.0), true, true);
        telemetry.record_step(Vec2::new(3.0, 4.0), false, false);

        assert_eq!(telemetry.previous_position(), Vec2::new(3.0, 4.0));
        assert_eq!(telemetry.fixed_ticks(), 2);
        assert_eq!(telemetry.input_receipts(), 1);
        assert_eq!(telemetry.direction_changes(), 1);
        assert!((telemetry.distance_travelled() - 5.0).abs() < f32::EPSILON);
    }

    /// Saturates long-running telemetry counters instead of wrapping to zero.
    #[test]
    fn motion_telemetry_counters_saturate() {
        let mut telemetry = MotionTelemetry {
            previous_position: Vec2::ZERO,
            fixed_ticks: u64::MAX,
            direction_changes: u32::MAX,
            input_receipts: u64::MAX,
            distance_travelled: 0.0,
        };

        telemetry.record_step(Vec2::ZERO, true, true);

        assert_eq!(telemetry.fixed_ticks(), u64::MAX);
        assert_eq!(telemetry.input_receipts(), u64::MAX);
        assert_eq!(telemetry.direction_changes(), u32::MAX);
    }

    /// Shares monotonically increasing authority evidence across cloned handles.
    #[test]
    fn server_bridge_clones_share_evidence() {
        let writer = ServerStatusBridge::default();
        let reader = writer.clone();

        writer.record_connection();
        writer.record_replication_sender();
        writer.record_authoritative_action();
        writer.record_server_tick();

        let snapshot = reader.snapshot();
        assert!(snapshot.mirror_connected);
        assert_eq!(snapshot.connection_events, 1);
        assert_eq!(snapshot.replication_senders, 1);
        assert_eq!(snapshot.action_receipts, 1);
        assert_eq!(snapshot.fixed_tick, 1);
    }

    /// Saturates every cross-thread counter instead of wrapping runtime evidence.
    #[test]
    fn server_bridge_counters_saturate() {
        let bridge = ServerStatusBridge::default();
        bridge
            .0
            .connection_events
            .store(u32::MAX, std::sync::atomic::Ordering::Relaxed);
        bridge
            .0
            .replication_senders
            .store(u32::MAX, std::sync::atomic::Ordering::Relaxed);
        bridge
            .0
            .action_receipts
            .store(u64::MAX, std::sync::atomic::Ordering::Relaxed);
        bridge
            .0
            .fixed_tick
            .store(u64::MAX, std::sync::atomic::Ordering::Relaxed);

        bridge.record_connection();
        bridge.record_replication_sender();
        bridge.record_authoritative_action();
        bridge.record_server_tick();

        let snapshot = bridge.snapshot();
        assert_eq!(snapshot.connection_events, u32::MAX);
        assert_eq!(snapshot.replication_senders, u32::MAX);
        assert_eq!(snapshot.action_receipts, u64::MAX);
        assert_eq!(snapshot.fixed_tick, u64::MAX);
    }

    /// Requires complete networking, liveness, and input evidence for health.
    #[test]
    fn status_health_requires_every_runtime_layer() {
        let mut status = healthy_status();

        assert!(status.network_ready());
        assert!(status.input_path_verified());
        assert!(status.is_healthy());

        status.interpolation_deadline_missed = true;
        assert!(!status.network_ready());
        assert!(!status.is_healthy());
    }

    /// Checks each independent readiness probe instead of accepting partial health.
    #[test]
    fn status_rejects_each_missing_network_probe() {
        let break_probes: [fn(&mut NetworkDemoStatus); 12] = [
            |status| status.client_connected = false,
            |status| status.input_timeline_synced = false,
            |status| status.server_mirror_connected = false,
            |status| status.client_connection_events = 0,
            |status| status.server_connection_events = 0,
            |status| status.replication_senders = 0,
            |status| status.predicted_entities = 0,
            |status| status.interpolated_entities = 0,
            |status| status.interpolation_histories = 0,
            |status| status.interpolation_samples = 1,
            |status| status.interpolation_ready = false,
            |status| status.interpolation_deadline_missed = true,
        ];

        for break_probe in break_probes {
            let mut status = healthy_status();
            break_probe(&mut status);
            assert!(!status.network_ready());
            assert!(!status.is_healthy());
        }
    }

    /// Requires both sides of the semantic input path and an advancing heartbeat.
    #[test]
    fn status_rejects_incomplete_liveness_or_input_evidence() {
        let break_probes: [fn(&mut NetworkDemoStatus); 5] = [
            |status| status.fixed_tick = 0,
            |status| status.server_fixed_tick = 0,
            |status| status.input_receipts = 0,
            |status| status.local_action_receipts = 0,
            |status| status.server_action_receipts = 0,
        ];

        for break_probe in break_probes {
            let mut status = healthy_status();
            break_probe(&mut status);
            assert!(!status.is_healthy());
        }
    }

    /// Registers every shared reflected contract in the app type registry.
    #[test]
    fn shared_types_are_registered() {
        let mut app = App::new();
        register_shared_types(&mut app);
        let registry = app.world().resource::<AppTypeRegistry>().read();

        assert_registered::<NetworkDemoStatus>(&registry);
        assert_registered::<NetworkPath>(&registry);
        assert_registered::<DemoBody>(&registry);
        assert_registered::<MotionTelemetry>(&registry);
    }

    /// Asserts that one type is present in Bevy's reflection registry.
    fn assert_registered<T: 'static>(registry: &TypeRegistry) {
        assert!(registry.get(TypeId::of::<T>()).is_some());
    }

    /// Builds the smallest status containing complete runtime evidence.
    fn healthy_status() -> NetworkDemoStatus {
        NetworkDemoStatus {
            client_connected: true,
            input_timeline_synced: true,
            server_mirror_connected: true,
            client_connection_events: 1,
            server_connection_events: 1,
            replication_senders: 1,
            predicted_entities: 1,
            interpolated_entities: 1,
            interpolation_histories: 1,
            interpolation_samples: 2,
            interpolation_ready: true,
            fixed_tick: 1,
            server_fixed_tick: 1,
            input_receipts: 1,
            local_action_receipts: 1,
            server_action_receipts: 1,
            ..NetworkDemoStatus::default()
        }
    }
}
