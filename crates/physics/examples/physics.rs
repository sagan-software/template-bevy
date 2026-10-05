//! Installs the selected physics backend without a renderer.

use bevy::prelude::*;
use game_physics::Plugin as PhysicsPlugin;

/// Exercises plugin composition through the public crate API.
fn main() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, PhysicsPlugin::default()));
    app.update();
}
