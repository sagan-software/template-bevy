//! Composes the starter through its public plugin and settings APIs.
//!
//! Choose a dimension before running this example. Add networking and MCP as
//! independent Cargo features when required. The plugin owns scene composition
//! while this executable retains startup configuration. Run with
//! `cargo run -p game --example game --features dim2,networked,mcp`.

use bevy::prelude::App;
use game_settings::Config;

/// Launches the same domain plugins used by the generated application.
fn main() {
    App::new()
        .add_plugins(game::Plugin::default().with_config(Config::default()))
        .run();
}
