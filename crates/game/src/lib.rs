//! Composes local or networked gameplay through one configurable Bevy plugin.
//!
//! Construct `Plugin` with validated settings and the optional inspector choice.
//! Dimension features select the rendered starter scene. Networking adds a
//! predicting client and independent authority with shared input registration.
//! Deterministic simulation helpers remain available for headless tests and
//! benchmarks; presentation and platform integration stay behind the plugin.

// Select the complete gameplay mode once while keeping the same crate-root exports.
std::cfg_select! {
    feature = "networked" => {
        mod components;
        mod inputs;
        mod inspection;
        mod network;
        mod network_game;
        mod presentation;
        #[cfg(not(target_family = "wasm"))]
        #[path = "particles.rs"]
        mod particles;
        mod simulation;
        mod systems;

        pub use self::components::NetworkDemoStatus;
        pub use self::network::HeadlessNetworkHarness;
        pub use self::simulation::MovementStep;
        pub use self::simulation::MovementStepResult;
        pub use self::simulation::movement_velocity;
        pub use self::simulation::reflect_at_bounds;
    }
    _ => { mod local; }
}
mod camera;
#[cfg(all(feature = "dim2", feature = "dim3"))]
mod dimensions;
mod logging;
mod plugin;
mod shader_scene;
pub use self::plugin::GamePlugin as Plugin;
