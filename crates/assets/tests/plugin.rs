//! External composition without a graphics device or window.

use bevy::prelude::*;

/// Registers glTF ingestion before Skein installs its processing observer.
#[test]
fn assets_plugin_installs_after_gltf() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::mesh::MeshPlugin)
        .add_plugins(bevy::gltf::GltfPlugin::default())
        .add_plugins(game_assets::Plugin);
    assert!(app.is_plugin_added::<bevy_skein::SkeinPlugin>());
}

/// Component exported by Blender and instantiated through Skein reflection.
#[derive(Component, Reflect)]
#[reflect(Component)]
#[type_path = "game_assets_test"]
struct BlenderMarker {
    /// Scene-authored movement speed in world units per second.
    speed: f32,
}

/// Exercises component ingestion instead of checking plugin registration alone.
#[test]
fn skein_ingests_reflected_gltf_components() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::mesh::MeshPlugin)
        .add_plugins(bevy::gltf::GltfPlugin::default())
        .add_plugins(game_assets::Plugin)
        .register_type::<BlenderMarker>();
    let entity = app
        .world_mut()
        .spawn((
            Name::new("Blender authored object"),
            GltfExtras {
                value: r#"{"skein":[{"game_assets_test::BlenderMarker":{"speed":7.0}}]}"#.into(),
            },
        ))
        .id();
    app.world_mut().flush();
    assert_eq!(
        app.world()
            .get::<BlenderMarker>(entity)
            .expect("Skein must insert the reflected component")
            .speed,
        7.0
    );
}
