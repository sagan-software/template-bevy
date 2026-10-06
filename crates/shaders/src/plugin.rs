//! Registers shader sources and the selected dimension material plugins.

use bevy::asset::embedded_asset;
use bevy::prelude::App;

/// Installs embedded shader sources and material pipelines after Bevy plugins.
/// Requires Bevy's asset and renderer plugins first. It does not add cameras
/// or spawn scene entities.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShaderPlugin {}

impl bevy::prelude::Plugin for ShaderPlugin {
    /// Registers the selected materials without adding cameras or scene entities.
    fn build(&self, app: &mut App) {
        // Keep the planar source inside the executable instead of requiring a copied asset.
        embedded_asset!(app, "shaders/pattern2d.wgsl");
        // Give the spatial source the same crate-owned namespace across generated projects.
        embedded_asset!(app, "shaders/pattern3d.wgsl");
        // Bind the planar color uniform through Bevy's 2D material pipeline.
        #[cfg(feature = "dim2")]
        app.add_plugins(bevy::sprite_render::Material2dPlugin::<
            crate::PatternMaterial2d,
        >::default());
        // Bind the spatial color uniform through Bevy's separate 3D pipeline.
        #[cfg(feature = "dim3")]
        app.add_plugins(bevy::pbr::MaterialPlugin::<crate::PatternMaterial3d>::default());
    }
}

/// Checks the private embedded registry identity independently of asset loading.
#[cfg(test)]
mod tests {
    use super::ShaderPlugin;
    use bevy::asset::io::embedded::EmbeddedAssetRegistry;
    use bevy::prelude::*;
    use std::path::Path;

    /// Registers both source files even when no dimension material is selected.
    #[test]
    fn embedded_registry_contains_the_authored_sources() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Shader>()
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .add_plugins(ShaderPlugin::default());
        let registry = app.world().resource::<EmbeddedAssetRegistry>();
        for (path, source) in [
            (
                "game_shaders/shaders/pattern2d.wgsl",
                include_bytes!("shaders/pattern2d.wgsl").as_slice(),
            ),
            (
                "game_shaders/shaders/pattern3d.wgsl",
                include_bytes!("shaders/pattern3d.wgsl").as_slice(),
            ),
        ] {
            let asset = registry
                .remove_asset(Path::new(path))
                .expect("embedded source must exist");
            assert_eq!(asset.value(), source);
        }
    }
}
