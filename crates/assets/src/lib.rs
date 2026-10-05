//! Optional Blender glTF component ingestion through Skein.
//!
//! Add `Plugin` after Bevy's asset and glTF plugins, then register the reflected
//! component types exported by Blender. Skein reads those components from glTF
//! extras when scenes load. The application owns remote inspection; this plugin
//! preserves its remote methods instead of installing another BRP server.

/// Adds Skein after the application installs Bevy's glTF plugins. Register
/// exported reflected component types before loading scenes containing
/// Blender-authored glTF extras.
#[derive(Clone, Copy, Debug, Default)]
pub struct Plugin;

impl bevy::prelude::Plugin for Plugin {
    /// Installs reflected component ingestion at the asset boundary.
    fn build(&self, app: &mut bevy::prelude::App) {
        // The application owns BRP. Skein only adds ingestion and its extensions.
        app.add_plugins(bevy_skein::SkeinPlugin { handle_brp: false });
    }
}

/// Checks that optional ingestion preserves the application's BRP ownership.
#[cfg(all(test, feature = "mcp"))]
mod tests {
    use super::Plugin;
    use bevy::gltf::GltfPlugin;
    use bevy::prelude::*;
    use bevy::remote::RemoteMethods;
    use bevy::remote::RemotePlugin;

    /// Adds Skein endpoints without replacing existing remote method handlers.
    #[test]
    fn ingestion_preserves_remote_methods() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            GltfPlugin::default(),
            RemotePlugin::default(),
        ));
        let before = app.world().resource::<RemoteMethods>().methods();
        app.add_plugins(Plugin);
        let after = app.world().resource::<RemoteMethods>().methods();
        assert!(before.iter().all(|method| after.contains(method)));
        assert!(app.is_plugin_added::<RemotePlugin>());
    }
}
