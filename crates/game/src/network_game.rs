//! Root composition for the rendered client and embedded headless authority.

use crate::components::ServerStatusBridge;
use crate::inspection::InspectionPlugin;
use crate::network::NetworkPlugin;
use crate::network::ServerShutdown;
use crate::network::advance_server_tick;
use crate::network::request_server_exit;
use crate::presentation::PresentationPlugin;
use crate::simulation::SimulationPlugin;
use crate::systems::DemoSystems;
use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::winit::WinitSettings;
use game_settings::Config;
use lightyear::crossbeam::CrossbeamIo;

// Native authority startup retains its thread owner and exit handling together.
std::cfg_select! {
    not(target_family = "wasm") => {
        use crate::network::ServerRuntime;
        use crate::network::shutdown_server_on_app_exit;
        /// Converts authority startup failure into an unsuccessful application exit.
        fn install_server_runtime(app: &mut App, runtime: std::io::Result<ServerRuntime>) {
            // A client without its authority must terminate instead of remaining unhealthy.
            match runtime {
                Ok(runtime) => {
                    // Retain the join owner until the client follows its ordinary exit path.
                    app.insert_resource(runtime)
                        .add_systems(Last, shutdown_server_on_app_exit);
                }
                Err(error) => {
                    error!(%error, "failed to start embedded Lightyear server thread");
                    app.world_mut().write_message(AppExit::error());
                }
            }
        }

    }
    _ => {}
}

/// Composes the complete tutorial application from focused domain plugins.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct NetworkGamePlugin {
    /// Validated timing settings inserted into both Bevy worlds.
    config: Config,
    /// Runtime switch controlling installation of the inspector UI.
    is_editor_enabled: bool,
    /// Native window title supplied by the executable.
    window_title: &'static str,
}

impl NetworkGamePlugin {
    /// Creates the root plugin from validated settings and a runtime editor
    /// choice.
    #[must_use]
    pub(crate) const fn new(
        config: Config,
        is_editor_enabled: bool,
        window_title: &'static str,
    ) -> Self {
        Self {
            config,
            is_editor_enabled,
            window_title,
        }
    }
}

impl Plugin for NetworkGamePlugin {
    /// Builds the rendered world, then starts and retains its authority owner.
    fn build(&self, app: &mut App) {
        let tick_duration = self.config.tick_rate().period();
        let (client_io, server_io) = CrossbeamIo::new_pair();
        let server_status = ServerStatusBridge::default();

        // Register the same semantic config that is inserted as a resource so
        // BRP and the runtime inspector can describe it without a global type list.
        install_reflected_config(app, self.config);

        install_renderer(app, tick_duration, self.window_title);
        app.add_plugins(NetworkPlugin::client(
            tick_duration,
            client_io,
            server_status.clone(),
        ))
        .add_plugins(SimulationPlugin::client())
        .add_plugins(PresentationPlugin)
        .add_plugins(InspectionPlugin::new(self.is_editor_enabled));

        // The closure constructs, runs, and drops the server App on its owning
        // OS thread. No Bevy World or executor is ever re-entered from another.
        std::cfg_select! {
        not(target_family = "wasm") => {
            let config = self.config;
            let runtime = ServerRuntime::spawn(move |shutdown| {
                let mut server = build_threaded_server(config, server_io, server_status, shutdown);
                server.run()
            });
            install_server_runtime(app, runtime);
        }
        _ => {
            let mut server = build_threaded_server(
                self.config,
                server_io,
                server_status,
                ServerShutdown::default(),
            );
            server.finish();
            server.cleanup();
            app.insert_non_send(BrowserAuthority(server))
                .add_systems(First, update_browser_authority);
        }
        }
    }
}

/// Installs windowed rendering and continuous network scheduling.
fn install_renderer(app: &mut App, tick_duration: std::time::Duration, window_title: &str) {
    // DefaultPlugins belongs only to the rendered world. The authority gets
    // a deliberately smaller executor below and never initializes a window.
    app.add_plugins(
        DefaultPlugins
            .set(crate::logging::plugin())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: window_title.into(),
                    resolution: (960, 600).into(),
                    canvas: Some("#game".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
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
    .configure_sets(PostUpdate, DemoSystems::Presentation);
}

/// Registers and inserts validated settings in either independently owned
/// World.
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

    // Lightyear's server plugins requires state infrastructure even though this
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
#[cfg(test)]
mod tests {
    use super::NetworkGamePlugin;
    use super::build_threaded_server;
    use super::install_reflected_config;
    #[cfg(not(target_family = "wasm"))]
    use super::install_server_runtime;
    use crate::components::ServerStatusBridge;
    use crate::network::NetworkPlugin;
    use crate::network::ServerShutdown;
    use crate::simulation::SimulationPlugin;
    use bevy::ecs::reflect::ReflectResource;
    use bevy::prelude::App;
    use bevy::prelude::AppTypeRegistry;
    use bevy::prelude::WindowPlugin;
    use bevy::reflect::TypeRegistry;
    use bevy::state::app::StatesPlugin;
    use game_settings::Config;
    use lightyear::crossbeam::CrossbeamIo;
    use std::any::TypeId;

    /// Retains validated config and the runtime-only editor choice.
    #[test]
    fn root_plugin_records_construction_choices() {
        let plugin = NetworkGamePlugin::new(Config::default(), true, "test-game");

        assert_eq!(plugin.config, Config::default());
        assert!(plugin.is_editor_enabled);
    }

    /// Terminates a client whose authority could not start without unwinding.
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn failed_authority_start_requests_an_error_exit() {
        let mut app = App::new();
        install_server_runtime(&mut app, Err(std::io::Error::other("thread unavailable")));
        assert!(
            !app.world()
                .contains_resource::<crate::network::ServerRuntime>()
        );
        assert_eq!(app.should_exit(), Some(bevy::prelude::AppExit::error()));
    }

    /// Retains a successfully started authority until its owner requests
    /// shutdown.
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn successful_authority_start_retains_its_runtime() {
        let mut app = App::new();
        let runtime =
            crate::network::ServerRuntime::spawn(|_shutdown| bevy::prelude::AppExit::Success)
                .expect("test authority must start");
        install_server_runtime(&mut app, Ok(runtime));
        assert!(
            app.world()
                .contains_resource::<crate::network::ServerRuntime>()
        );
        assert_eq!(app.should_exit(), None);
        app.world_mut()
            .resource_mut::<crate::network::ServerRuntime>()
            .shutdown_and_join()
            .expect("test authority must stop");
    }

    /// Proves the rendered-world setup exposes config values and resource
    /// metadata.
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

    /// Requires Config's registration to carry Bevy's reflected-resource
    /// adapter.
    fn assert_reflected_config(app: &App) {
        let registry = app.world().resource::<AppTypeRegistry>().read();
        let registration = registered_config(&registry);

        assert!(registration.data::<ReflectResource>().is_some());
    }

    /// Retrieves the Config registration while keeping assertion setup
    /// readable.
    fn registered_config(registry: &TypeRegistry) -> &bevy::reflect::TypeRegistration {
        registry
            .get(TypeId::of::<Config>())
            .expect("Config should be registered for reflection")
    }
}

/// Owns the browser authority on the browser's single executor thread.
#[cfg(target_family = "wasm")]
struct BrowserAuthority(App);

/// Updates the browser authority without a native thread or nested event loop.
#[cfg(target_family = "wasm")]
fn update_browser_authority(mut authority: NonSendMut<'_, BrowserAuthority>) {
    authority.0.update();
}
