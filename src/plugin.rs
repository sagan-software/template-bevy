//! Root composition for the rendered client and embedded headless authority.

use crate::{
    components::ServerStatusBridge,
    config::Config,
    inspection::InspectionPlugin,
    network::{
        NetworkPlugin, ServerRuntime, ServerShutdown, advance_server_tick, request_server_exit,
        shutdown_server_on_app_exit,
    },
    presentation::PresentationPlugin,
    simulation::SimulationPlugin,
    systems::DemoSystems,
};
use bevy::{app::ScheduleRunnerPlugin, prelude::*, state::app::StatesPlugin, winit::WinitSettings};
use lightyear::crossbeam::CrossbeamIo;

/// Composes the complete tutorial application from focused domain plugins.
#[derive(Clone, Copy, Debug)]
pub struct TemplateBevyPlugin {
    /// Validated timing settings inserted into both Bevy worlds.
    config: Config,
    /// Runtime switch controlling installation of the inspector UI.
    editor_enabled: bool,
}

impl TemplateBevyPlugin {
    /// Creates the root plugin from validated settings and a runtime editor choice.
    #[must_use]
    pub const fn new(config: Config, editor_enabled: bool) -> Self {
        Self {
            config,
            editor_enabled,
        }
    }
}

impl Plugin for TemplateBevyPlugin {
    /// Builds the rendered world, then starts and retains its authority owner.
    fn build(&self, app: &mut App) {
        let tick_duration = self.config.tick_rate().period();
        let (client_io, server_io) = CrossbeamIo::new_pair();
        let server_status = ServerStatusBridge::default();

        // Register the same semantic config that is inserted as a resource so
        // BRP and the runtime inspector can describe it without a global type list.
        install_reflected_config(app, self.config);

        // DefaultPlugins belongs only to the rendered world. The authority gets
        // a deliberately smaller executor below and never initializes a window.
        app.add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: concat!(
                        env!("CARGO_PKG_NAME"),
                        ": networked 2D prediction + interpolation"
                    )
                    .into(),
                    resolution: (960, 600).into(),
                    ..default()
                }),
                ..default()
            }),
        )
        // Network ticks must continue at full cadence while the tutorial
        // window is unfocused. Reactive low-power Winit updates can otherwise
        // let the independent authority outrun a paused predicting client.
        .insert_resource(WinitSettings::continuous())
        .insert_resource(Time::<Fixed>::from_duration(tick_duration))
        .configure_sets(FixedUpdate, DemoSystems::Simulation)
        .configure_sets(FixedLast, DemoSystems::FixedTelemetry)
        .configure_sets(PostUpdate, DemoSystems::Presentation)
        .add_plugins(NetworkPlugin::client(
            tick_duration,
            client_io,
            server_status.clone(),
        ))
        .add_plugins(SimulationPlugin::client())
        .add_plugins(PresentationPlugin)
        .add_plugins(InspectionPlugin::new(self.editor_enabled));

        // The closure constructs, runs, and drops the server App on its owning
        // OS thread. No Bevy World or executor is ever re-entered from another.
        let config = self.config;
        let runtime = ServerRuntime::spawn(move |shutdown| {
            let mut server = build_threaded_server(config, server_io, server_status, shutdown);
            server.run()
        })
        .unwrap_or_else(|error| {
            // A rendered client without its paired authority can never satisfy
            // this template's network contract. Fail construction immediately
            // instead of opening a window that is permanently unhealthy.
            panic!("failed to start embedded Lightyear server thread: {error}")
        });
        app.insert_resource(runtime)
            .add_systems(Last, shutdown_server_on_app_exit);
    }
}

/// Registers and inserts validated settings in either independently owned World.
fn install_reflected_config(app: &mut App, config: Config) {
    // Reflection registration must happen in each App because client and
    // authority worlds have independent type registries as well as resources.
    app.register_type::<Config>().insert_resource(config);
}

/// Builds the minimal authority whose schedule runner owns its update loop.
fn build_threaded_server(
    config: Config,
    server_io: CrossbeamIo,
    status: ServerStatusBridge,
    shutdown: ServerShutdown,
) -> App {
    let tick_duration = config.tick_rate().period();
    let mut app = App::new();

    // Use the rendered App's exact registration path so inspection metadata
    // cannot silently differ between the two independently owned worlds.
    install_reflected_config(&mut app, config);

    // Replicon's server backend requires state infrastructure even though this
    // tutorial intentionally defines no high-level gameplay State.
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(tick_duration)),
        TransformPlugin,
        StatesPlugin,
    ))
    .insert_resource(Time::<Fixed>::from_duration(tick_duration))
    .insert_resource(shutdown)
    .configure_sets(FixedUpdate, DemoSystems::Simulation)
    .add_plugins(NetworkPlugin::server(
        tick_duration,
        config.replication_interval().duration(),
        server_io,
        status,
    ))
    .add_plugins(SimulationPlugin::server())
    .add_systems(First, request_server_exit)
    .add_systems(FixedFirst, advance_server_tick);
    app
}

/// Covers construction-only choices without starting the full rendered runner.
#[cfg(all(test, not(coverage)))]
mod tests {
    use super::{TemplateBevyPlugin, build_threaded_server, install_reflected_config};
    use crate::{
        Config,
        components::ServerStatusBridge,
        network::{NetworkPlugin, ServerShutdown},
        simulation::SimulationPlugin,
    };
    use bevy::{
        ecs::reflect::ReflectResource,
        prelude::{App, AppTypeRegistry, WindowPlugin},
        reflect::TypeRegistry,
        state::app::StatesPlugin,
    };
    use lightyear::crossbeam::CrossbeamIo;
    use std::any::TypeId;

    /// Retains validated config and the runtime-only editor choice.
    #[test]
    fn root_plugin_records_construction_choices() {
        let plugin = TemplateBevyPlugin::new(Config::default(), true);

        assert_eq!(plugin.config, Config::default());
        assert!(plugin.editor_enabled);
    }

    /// Proves the rendered-world setup exposes config values and resource metadata.
    #[test]
    fn rendered_config_setup_registers_resource_reflection() {
        let mut app = App::new();

        // Exercise the platform-neutral portion of rendered composition; the
        // full root plugin intentionally remains in the external Winit smoke test.
        install_reflected_config(&mut app, Config::default());

        assert_reflected_config(&app);
    }

    /// Builds authority composition without a renderer, window, or live thread.
    #[test]
    fn authority_builder_contains_only_headless_domains() {
        let (_client_io, server_io) = CrossbeamIo::new_pair();
        let app = build_threaded_server(
            Config::default(),
            server_io,
            ServerStatusBridge::default(),
            ServerShutdown::default(),
        );

        assert_eq!(*app.world().resource::<Config>(), Config::default());
        assert_reflected_config(&app);
        assert!(app.is_plugin_added::<StatesPlugin>());
        assert!(app.is_plugin_added::<NetworkPlugin>());
        assert!(app.is_plugin_added::<SimulationPlugin>());
        assert!(!app.is_plugin_added::<WindowPlugin>());
    }

    /// Requires Config's registration to carry Bevy's reflected-resource adapter.
    fn assert_reflected_config(app: &App) {
        let registry = app.world().resource::<AppTypeRegistry>().read();
        let registration = registered_config(&registry);

        assert!(registration.data::<ReflectResource>().is_some());
    }

    /// Retrieves the Config registration while keeping assertion setup readable.
    fn registered_config(registry: &TypeRegistry) -> &bevy::reflect::TypeRegistration {
        registry
            .get(TypeId::of::<Config>())
            .expect("Config should be registered for reflection")
    }
}
