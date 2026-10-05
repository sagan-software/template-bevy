//! Adds Skein after Bevy's glTF infrastructure.

use bevy::prelude::*;

/// Builds the same asset plugin composition used by generated applications.
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(game_assets::Plugin)
        .run();
}
