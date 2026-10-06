//! Runs the embedded material through the public shader plugin.
//! Run `cargo run -p game-shaders --example material --features dim2`.

use bevy::prelude::*;
use game_shaders::PatternMaterial2d;
use game_shaders::Plugin;

/// Starts the material guide with Bevy's platform renderer.
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, Plugin::default()))
        .add_systems(Startup, setup)
        .run();
}

/// Gives the shader UV coordinates and a visible mesh.
fn setup(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<PatternMaterial2d>>,
) {
    commands.spawn(Camera2d);
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(240.0, 240.0))),
        MeshMaterial2d(materials.add(PatternMaterial2d {
            color: LinearRgba::BLUE,
        })),
    ));
}
