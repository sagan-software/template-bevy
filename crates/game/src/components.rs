//! ECS data shared by networking, simulation, presentation, and inspection.
//!
//! Feature-local data belongs with its owning plugin. The contracts here cross
//! at least two domains and therefore benefit from one documented definition.

use bevy::ecs::reflect::ReflectComponent;
use bevy::ecs::reflect::ReflectResource;
use bevy::prelude::App;
use bevy::prelude::Component;
use bevy::prelude::Reflect;
use bevy::prelude::Resource;
use bevy::prelude::Vec2;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

/// Aggregate connection, replication, input, and liveness evidence exposed by
/// BRP.
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
    /// Whether this launch installed the native world inspector. The value
    /// records the selected runtime mode and remains false for browser launches.
    pub is_editor_enabled: bool,
    /// Whether the rendered raw client completed its connection lifecycle. This
    /// flag alone does not prove replication, input synchronization, or authority
    /// progress.
    pub is_client_connected: bool,
    /// Whether Lightyear synchronized the timeline that buffers and sends
    /// Whether Lightyear synchronized the client timeline used to buffer and
    /// transmit inputs. Readiness also requires the connection and replicated
    /// action paths.
    pub is_input_timeline_synced: bool,
    /// Whether the headless authority accepted its Crossbeam mirror connection.
    /// The status bridge publishes this observation from the independently
    /// updated server world.
    pub is_server_mirror_connected: bool,
    /// Number of client connection events observed since world creation. The
    /// counter saturates at its maximum value rather than wrapping to zero.
    pub client_connection_events: u32,
    /// Number of server connection events observed since world creation. The
    /// authority publishes this saturating counter through the shared atomic
    /// status bridge.
    pub server_connection_events: u32,
    /// Number of server connections equipped to send replication. A positive
    /// value confirms that the authority installed the send-side component
    /// replication path.
    pub replication_senders: u32,
    /// Number of receiver-created predicted bodies observed by the client. This
    /// counter distinguishes actual receive-side creation from locally spawned
    /// authority demonstration bodies.
    pub predicted_entities: u32,
    /// Number of receiver-created interpolated bodies observed by the client.
    /// This counter distinguishes actual receive-side creation from locally
    /// spawned authority demonstration bodies.
    pub interpolated_entities: u32,
    /// Number of position interpolation histories created by the receive path. A
    /// history must contain authoritative snapshots before interpolation can
    /// become ready.
    pub interpolation_histories: u32,
    /// Largest authoritative position sample count observed in one interpolation
    /// history. At least two samples are required before the interpolation
    /// readiness flag becomes true.
    pub interpolation_samples: usize,
    /// Whether at least two authoritative snapshots are available to
    /// Whether an observed position history contains at least two authoritative
    /// snapshots. The flag records readiness independently from connection and
    /// fixed-tick progress.
    pub is_interpolation_ready: bool,
    /// Whether interpolation failed to become ready before the configured fixed-
    /// tick deadline. This terminal observation remains set even if samples
    /// arrive afterward.
    pub is_interpolation_deadline_missed: bool,
    /// Number of completed fixed client ticks since world creation. This
    /// saturating heartbeat allows inspection to distinguish active simulation
    /// from a connected stalled client.
    pub fixed_tick: u64,
    /// Number of completed fixed authority ticks since world creation. The server
    /// publishes this saturating heartbeat independently from the rendered client
    /// update loop.
    pub server_fixed_tick: u64,
    /// Number of predicted simulation inputs consumed by the client. The
    /// saturating counter records simulation progress separately from action
    /// generation and server receipt.
    pub input_receipts: u64,
    /// Number of observed physical or externally injected keyboard presses. This
    /// saturating counter provides input evidence without implying that the
    /// authority consumed those presses.
    pub external_input_receipts: u64,
    /// Number of observed Space key presses, including external injection. This
    /// saturating counter records the hardware boost binding separately from
    /// semantic action triggers.
    pub space_input_receipts: u64,
    /// Number of observed right-arrow key presses, including external injection.
    /// This saturating counter records the hardware movement binding separately
    /// from semantic action triggers.
    pub arrow_right_input_receipts: u64,
    /// Number of semantic movement or boost actions generated locally. This
    /// saturating counter requires a separate positive server receipt to prove
    /// transport consumption.
    pub local_action_receipts: u64,
    /// Number of replicated semantic actions consumed by the authority. The
    /// shared status bridge publishes this saturating counter independently from
    /// local action generation.
    pub server_action_receipts: u64,
    /// Number of semantic local movement action triggers observed. This
    /// saturating counter records movement events independently from boost events
    /// and authority-side action consumption.
    pub movement_action_receipts: u64,
    /// Number of semantic local boost action triggers observed. This saturating
    /// counter records boost events independently from movement events and
    /// authority-side action consumption.
    pub boost_action_receipts: u64,
}

impl NetworkDemoStatus {
    /// Reports whether both peers, input synchronization, replication senders, and
    /// receiver paths are established. Interpolation must contain two snapshots
    /// and must have met its fixed-tick deadline.
    #[must_use]
    pub const fn is_network_ready(&self) -> bool {
        self.is_peers_connected()
            && self.is_receivers_ready()
            && self.is_interpolation_ready
            && !self.is_interpolation_deadline_missed
    }

    /// Requires actual lifecycle events and send-side replication on both peers.
    const fn is_peers_connected(&self) -> bool {
        self.is_client_connected
            && self.is_input_timeline_synced
            && self.is_server_mirror_connected
            && self.client_connection_events > 0
            && self.server_connection_events > 0
            && self.replication_senders > 0
    }

    /// Requires both replicated body kinds and a usable authoritative history.
    const fn is_receivers_ready(&self) -> bool {
        self.predicted_entities > 0
            && self.interpolated_entities > 0
            && self.interpolation_histories > 0
            && self.interpolation_samples >= 2
    }

    /// Reports whether at least one semantic action was produced locally and
    /// consumed by the authority. This checks both receipt counters without
    /// requiring matching counts.
    #[must_use]
    pub const fn is_input_path_verified(&self) -> bool {
        self.local_action_receipts > 0 && self.server_action_receipts > 0
    }

    /// Reports whether networking, fixed updates, and authoritative input are
    /// live. Both worlds must have advanced and both ends must have observed
    /// semantic input actions.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.is_network_ready()
            && self.fixed_tick > 0
            && self.server_fixed_tick > 0
            && self.input_receipts > 0
            && self.is_input_path_verified()
    }

    /// Copies the latest thread-safe server evidence into this reflected
    /// resource.
    pub(crate) const fn apply_server_snapshot(&mut self, snapshot: ServerStatusSnapshot) {
        // Read every authority counter from the same bridge snapshot.
        self.is_server_mirror_connected = snapshot.is_mirror_connected;
        self.server_connection_events = snapshot.connection_events;
        self.replication_senders = snapshot.replication_senders;
        self.server_action_receipts = snapshot.action_receipts;
        self.server_fixed_tick = snapshot.fixed_tick;
    }

    /// Records one completed fixed client tick without risking integer
    /// overflow.
    pub(crate) const fn record_fixed_tick(&mut self) {
        self.fixed_tick = self.fixed_tick.saturating_add(1);
    }
}

/// Distinguishes the two network receive paths demonstrated by a body.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Reflect)]
pub(crate) enum NetworkPath {
    /// Client prediction plus correction of a locally controlled body.
    Prediction,
    /// Snapshot interpolation of a server-authoritative remote body.
    Interpolation,
}

/// Identifies a networked demonstration body and its collision/render extent.
#[derive(Clone, Copy, Component, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub(crate) struct DemoBody {
    /// Network receive path this body teaches.
    path: NetworkPath,
    /// Half-size used by collision and presentation code.
    half_extent: Vec2,
}

impl DemoBody {
    /// Creates metadata for a client-predicted circular body.
    #[must_use]
    pub(crate) const fn predicted(radius: f32) -> Self {
        Self {
            path: NetworkPath::Prediction,
            half_extent: Vec2::splat(radius),
        }
    }

    /// Creates metadata for a snapshot-interpolated rectangular body.
    #[must_use]
    pub(crate) const fn interpolated(half_extent: Vec2) -> Self {
        Self {
            path: NetworkPath::Interpolation,
            half_extent,
        }
    }

    /// Returns the network receive path this body demonstrates.
    #[must_use]
    pub(crate) const fn path(&self) -> NetworkPath {
        self.path
    }

    /// Returns the body's collision and presentation half-size.
    #[must_use]
    pub(crate) const fn half_extent(&self) -> Vec2 {
        self.half_extent
    }
}

/// Records deterministic motion evidence for one simulation body.
#[derive(Clone, Copy, Component, Debug, PartialEq, Reflect)]
#[reflect(Component)]
pub(crate) struct MotionTelemetry {
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
    pub(crate) const fn at(position: Vec2) -> Self {
        Self {
            previous_position: position,
            fixed_ticks: 0,
            direction_changes: 0,
            input_receipts: 0,
            distance_travelled: 0.0,
        }
    }

    /// Records one deterministic simulation step.
    pub(crate) fn record_step(
        &mut self,
        position: Vec2,
        is_direction_changed: bool,
        is_input_consumed: bool,
    ) {
        // Measure from the preceding sample before advancing the stored anchor.
        self.distance_travelled += (position - self.previous_position).length();
        self.previous_position = position;

        // Saturating counters keep long-running observability from wrapping.
        self.fixed_ticks = self.fixed_ticks.saturating_add(1);
        if is_input_consumed {
            self.input_receipts = self.input_receipts.saturating_add(1);
        }
        if is_direction_changed {
            self.direction_changes = self.direction_changes.saturating_add(1);
        }
    }

    /// Realigns the distance anchor without recording a replayed simulation
    /// step.
    pub(crate) const fn resynchronize_position(&mut self, position: Vec2) {
        self.previous_position = position;
    }

    /// Returns the most recently sampled position.
    #[must_use]
    #[cfg(test)]
    pub(crate) const fn previous_position(&self) -> Vec2 {
        self.previous_position
    }

    /// Returns the number of fixed steps recorded for this body.
    #[must_use]
    pub(crate) const fn fixed_ticks(&self) -> u64 {
        self.fixed_ticks
    }

    /// Returns the number of recorded boundary direction changes.
    #[must_use]
    pub(crate) const fn direction_changes(&self) -> u32 {
        self.direction_changes
    }

    /// Returns the number of simulation inputs consumed by this body.
    #[must_use]
    pub(crate) const fn input_receipts(&self) -> u64 {
        self.input_receipts
    }

    /// Returns total world-space distance accumulated by this body.
    #[must_use]
    pub(crate) const fn distance_travelled(&self) -> f32 {
        self.distance_travelled
    }
}

/// Publishes server evidence across the App/thread boundary without sharing ECS
/// state.
#[derive(Clone, Debug, Default, Resource)]
pub(crate) struct ServerStatusBridge(
    /// Atomics shared by the independently owned client and server Worlds.
    Arc<ServerStatusAtoms>,
);

impl ServerStatusBridge {
    /// Records a connected server-side Crossbeam mirror.
    pub(crate) fn record_connection(&self) {
        // Release writes pair with the client world's Acquire snapshot loads.
        self.0.is_mirror_connected.store(true, Ordering::Release);
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
            is_mirror_connected: self.0.is_mirror_connected.load(Ordering::Acquire),
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
    is_mirror_connected: AtomicBool,
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
    is_mirror_connected: bool,
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
    let _previous_value = counter.try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
        Some(value.saturating_add(1))
    });
}

/// Atomically increments a 64-bit observability counter without wrapping.
fn saturating_increment_u64(counter: &AtomicU64) {
    // The closure always returns `Some`, so failure only causes an internal
    // compare-exchange retry rather than dropping an observation.
    let _previous_value = counter.try_update(Ordering::AcqRel, Ordering::Acquire, |value| {
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
#[cfg(test)]
mod tests {
    use super::DemoBody;
    use super::MotionTelemetry;
    use super::NetworkDemoStatus;
    use super::NetworkPath;
    use super::ServerStatusBridge;
    use super::register_shared_types;
    use bevy::prelude::App;
    use bevy::prelude::AppTypeRegistry;
    use bevy::prelude::TypePath;
    use bevy::prelude::Vec2;
    use bevy::reflect::TypeRegistry;
    use std::any::TypeId;

    /// Preserves the BRP resource name used by agent tooling.
    #[test]
    fn status_keeps_its_root_type_path() {
        assert_eq!(
            <NetworkDemoStatus as TypePath>::type_path(),
            "template_bevy::NetworkDemoStatus"
        );
    }

    /// Keeps each body constructor explicit about its network presentation
    /// path.
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

    /// Shares monotonically increasing authority evidence across cloned
    /// handles.
    #[test]
    fn server_bridge_clones_share_evidence() {
        let writer = ServerStatusBridge::default();
        let reader = writer.clone();

        writer.record_connection();
        writer.record_replication_sender();
        writer.record_authoritative_action();
        writer.record_server_tick();

        let snapshot = reader.snapshot();
        assert!(snapshot.is_mirror_connected);
        assert_eq!(snapshot.connection_events, 1);
        assert_eq!(snapshot.replication_senders, 1);
        assert_eq!(snapshot.action_receipts, 1);
        assert_eq!(snapshot.fixed_tick, 1);
    }

    /// Saturates every cross-thread counter instead of wrapping runtime
    /// evidence.
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

        assert!(status.is_network_ready());
        assert!(status.is_input_path_verified());
        assert!(status.is_healthy());

        status.is_interpolation_deadline_missed = true;
        assert!(!status.is_network_ready());
        assert!(!status.is_healthy());
    }

    /// Checks each independent readiness probe instead of accepting partial
    /// health.
    #[test]
    fn status_rejects_each_missing_network_probe() {
        let break_probes: [fn(&mut NetworkDemoStatus); 12] = [
            |status| status.is_client_connected = false,
            |status| status.is_input_timeline_synced = false,
            |status| status.is_server_mirror_connected = false,
            |status| status.client_connection_events = 0,
            |status| status.server_connection_events = 0,
            |status| status.replication_senders = 0,
            |status| status.predicted_entities = 0,
            |status| status.interpolated_entities = 0,
            |status| status.interpolation_histories = 0,
            |status| status.interpolation_samples = 1,
            |status| status.is_interpolation_ready = false,
            |status| status.is_interpolation_deadline_missed = true,
        ];

        for break_probe in break_probes {
            let mut status = healthy_status();
            break_probe(&mut status);
            assert!(!status.is_network_ready());
            assert!(!status.is_healthy());
        }
    }

    /// Requires both sides of the semantic input path and an advancing
    /// heartbeat.
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
            is_client_connected: true,
            is_input_timeline_synced: true,
            is_server_mirror_connected: true,
            client_connection_events: 1,
            server_connection_events: 1,
            replication_senders: 1,
            predicted_entities: 1,
            interpolated_entities: 1,
            interpolation_histories: 1,
            interpolation_samples: 2,
            is_interpolation_ready: true,
            fixed_tick: 1,
            server_fixed_tick: 1,
            input_receipts: 1,
            local_action_receipts: 1,
            server_action_receipts: 1,
            ..NetworkDemoStatus::default()
        }
    }
}
