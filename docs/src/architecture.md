# Architecture

`crates/app` parses startup choices, loads configuration, adds `game::Plugin`,
and runs the application. A generated project's package name belongs to this crate.

`crates/game` owns scene composition, controls, presentation, and optional networking.
`crates/physics` owns Rapier integration and replicated planar physics state.
`crates/settings` owns validated timing values and configuration errors.
`crates/assets` adds optional Skein glTF component ingestion.

`crates/shaders` owns embedded WGSL, typed materials, and shader pipeline registration.
Rendered starter scenes display its sample; the headless authority remains separate.
`xtask` implements portable project checks and instruction-link setup.

Gameplay and asset integration expose `Plugin` as their composition entry point.
The game plugin provides `with_config`, `with_editor`, and `with_window_title` builders.
The physics plugin provides `with_tick_rate`.
Physics components and validated settings remain public because consumers need
those domain values. The network harness and movement kernel remain public for
integration tests and benchmarks. Internal systems and orchestration stay private.

```rust,ignore
use bevy::prelude::*;
use game::Plugin as GamePlugin;
use game_settings::Config;

fn main() {
    App::new()
        .add_plugins(GamePlugin::default().with_config(Config::default()))
        .run();
}
```

Local mode creates one rendered application and no Lightyear authority.
Networking mode creates separate rendered-client and headless-authority applications.
Each application owns its entities, schedules, protocol registry, and physics state.
Paired Crossbeam channels transport input and snapshots between them.
This starter demonstrates in-process networking; it does not provide Internet matchmaking.

Native networking runs the authority on a joined thread.
Browser networking updates a separate authority application on the browser thread.
The cyan player is predicted; the orange body is snapshot-interpolated.
Readiness requires at least two receiver-created authoritative position samples.
Local input counters and authoritative receipt counters remain separate.

Rapier replaces the previous physics backend in both dimensions.
Gameplay state uses the XY plane, with Z translation and X/Y rotation locked in 3D.
The 3D-only scene uses a 3D camera and meshes around that planar gameplay.
When both dimensions are selected, an independent 3D Rapier scene appears beside
2D gameplay. Camera viewports divide physical pixels and preserve odd window widths.

Gameplay runs before `PhysicsSystems::Prepare` in `FixedUpdate`.
Prepare copies owned state into Rapier before `PhysicsSet::SyncBackend`.
`PhysicsSystems::Capture` copies completed state after `PhysicsSet::Writeback`.
Rapier's fixed integration step and Lightyear's tick use the validated frequency.
The gameplay physics length unit is 100 world units for pixel-scale contact tolerances.
Rapier enhanced determinism is disabled because the current dependency combination
fails to compile; see [dependency compatibility](dependencies.md).

The primary 2D and 3D cameras fit the planar arena at desktop and phone aspect ratios.
The 3D camera retains a zero near plane in world units.
