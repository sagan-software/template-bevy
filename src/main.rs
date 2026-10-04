//! Thin command-line entry point for the networked Bevy demonstration.

use bevy::prelude::{App, AppExit};
use clap::Parser as _;
use template_bevy::{ApplicationError, Cli, Config, TemplateBevyPlugin};

/// Loads startup choices, composes the game, and hands control to Bevy.
fn main() -> Result<AppExit, ApplicationError> {
    // Clap owns command-line syntax while `Config` owns semantic validation.
    // Keeping those boundaries separate gives each failure a useful message.
    let cli = Cli::parse();
    let config = Config::load(cli.config_path())?;

    // All gameplay and tooling live behind one reusable plugin so tests and
    // other binaries can construct the same application without copying setup.
    let mut app = App::new();
    app.add_plugins(TemplateBevyPlugin::new(config, cli.editor_enabled()));
    Ok(app.run())
}
