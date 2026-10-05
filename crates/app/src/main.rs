//! Command-line entry point for the selected Bevy game.
//!
//! Native launches parse configuration and inspector choices before installing
//! the game plugin. Browser launches use built-in defaults without process
//! arguments or filesystem configuration. This executable owns startup and
//! returns configuration failures to its caller; reusable gameplay and validated
//! settings remain in their respective workspace crates.

use crate::cli::Cli;
use bevy::prelude::App;
use bevy::prelude::AppExit;
#[cfg(not(target_family = "wasm"))]
use clap::Parser;
use game_settings::ApplicationError;
use game_settings::Config;

mod cli;

/// Loads startup choices, composes the game, and hands control to Bevy.
fn main() -> Result<AppExit, ApplicationError> {
    // Clap owns command-line syntax while `Config` owns semantic validation.
    // Keeping those boundaries separate gives each failure a useful message.
    #[cfg(not(target_family = "wasm"))]
    let cli = Cli::parse();
    #[cfg(target_family = "wasm")]
    let cli = Cli::default();
    let config = Config::load(cli.config_path())?;

    // All gameplay and tooling live behind one reusable plugin so tests and
    // other binaries can construct the same application without copying setup.
    let mut app = App::new();
    app.add_plugins(
        game::Plugin::default()
            .with_config(config)
            .with_window_title(env!("CARGO_PKG_NAME"))
            .with_editor(cli.is_editor_enabled()),
    );
    #[cfg(feature = "skein")]
    app.add_plugins(game_assets::Plugin);
    Ok(app.run())
}
