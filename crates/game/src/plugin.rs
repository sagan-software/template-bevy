//! Application composition through a small configurable plugin.

use bevy::prelude::App;
use game_settings::Config;

/// Composes the selected gameplay mode from validated timing settings and a
/// native inspector choice. Default construction uses the standard settings
/// with inspection disabled.
#[derive(Clone, Copy, Debug)]
pub struct GamePlugin {
    /// Validated settings shared by simulation and networking.
    config: Config,
    /// Native inspector choice for this launch.
    is_editor_enabled: bool,
    /// Application-owned title selected before opening the native window.
    window_title: &'static str,
}

impl Default for GamePlugin {
    /// Retains the standard timing, disabled inspector, and generic starter title.
    fn default() -> Self {
        // Keep timing, inspector selection, and application title independently configurable.
        Self {
            config: Config::default(),
            is_editor_enabled: false,
            window_title: "Bevy game",
        }
    }
}

impl GamePlugin {
    /// Selects validated simulation and replication timing before plugin
    /// installation. The same configuration is retained by the game plugin and
    /// inserted into its worlds.
    #[must_use]
    pub const fn with_config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }
    /// Selects the native inspector when MCP support is compiled. This builder
    /// preserves the configuration and does not change the selected dimension or
    /// networking mode.
    #[must_use]
    pub const fn with_editor(mut self, is_enabled: bool) -> Self {
        self.is_editor_enabled = is_enabled;
        self
    }

    /// Selects the native window title before startup. Applications can use their
    /// Cargo package name; the browser document title remains configured in its
    /// HTML file.
    #[must_use]
    pub const fn with_window_title(mut self, title: &'static str) -> Self {
        self.window_title = title;
        self
    }
}

impl bevy::prelude::Plugin for GamePlugin {
    /// Installs the selected gameplay mode through its owning plugin.
    fn build(&self, app: &mut App) {
        #[cfg(feature = "networked")]
        app.add_plugins(crate::network_game::NetworkGamePlugin::new(
            self.config,
            self.is_editor_enabled,
            self.window_title,
        ));
        #[cfg(not(feature = "networked"))]
        app.add_plugins(crate::local::LocalPlugin::new(
            self.config,
            self.is_editor_enabled,
            self.window_title,
        ));
        #[cfg(all(feature = "dim2", feature = "dim3"))]
        app.add_plugins(crate::dimensions::DimensionsPlugin);
    }
}

/// Covers independent builder choices without opening a window or authority thread.
#[cfg(test)]
mod tests {
    use super::GamePlugin;
    use game_settings::Config;

    /// Retains timing and inspector choices when replacing the application title.
    #[test]
    fn builder_preserves_independent_startup_choices() {
        let config = Config::from_toml_str("tick_rate = \"120 Hz\"")
            .expect("the typed configuration should parse");
        let plugin = GamePlugin::default()
            .with_config(config)
            .with_editor(true)
            .with_window_title("named-game");
        assert_eq!(plugin.config, config);
        assert!(plugin.is_editor_enabled);
        assert_eq!(plugin.window_title, "named-game");
        let defaults = GamePlugin::default();
        assert!(!defaults.is_editor_enabled);
        assert_eq!(defaults.window_title, "Bevy game");
    }
}
