//! Displays the embedded fragment shader in each rendered starter scene.

use bevy::prelude::*;

/// Installs material pipelines and visible samples only in rendered worlds.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ShaderScenePlugin;

impl Plugin for ShaderScenePlugin {
    /// Gives local and networked clients the same portable shader example.
    fn build(&self, app: &mut App) {
        app.add_plugins(game_shaders::Plugin::default());
        #[cfg(feature = "dim2")]
        app.add_systems(Startup, setup_planar);
        #[cfg(feature = "dim3")]
        app.add_systems(Startup, setup_spatial);
    }
}

/// Draws a UV-mapped shader sample inside the lower-left arena corner.
#[cfg(feature = "dim2")]
fn setup_planar(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<game_shaders::PatternMaterial2d>>,
) {
    // Dimensions and positions use the same world units as the planar arena.
    commands.spawn((
        Name::new("2D shader sample"),
        Mesh2d(meshes.add(Rectangle::new(80.0, 80.0))),
        MeshMaterial2d(materials.add(game_shaders::PatternMaterial2d {
            color: Color::srgb_u8(61, 214, 255).to_linear(),
        })),
        Transform::from_xyz(-300.0, -180.0, 0.0),
    ));
}

/// Draws a UV-mapped shader sample in the primary or separate preview scene.
#[cfg(feature = "dim3")]
fn setup_spatial(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<game_shaders::PatternMaterial3d>>,
) {
    use bevy::camera::visibility::RenderLayers;
    // The preview uses metre-sized geometry. The standalone 3D arena uses its
    // existing planar world units. Keep the sample on its camera's render layer.
    let (extent, position, layer) = std::cfg_select! {
        feature = "dim2" => { (Vec3::splat(1.2), Vec3::new(-2.0, 0.7, 0.0), RenderLayers::layer(1)) }
        _ => { (Vec3::new(80.0, 80.0, 24.0), Vec3::new(-300.0, -180.0, 0.0), RenderLayers::default()) }
    };
    commands.spawn((
        Name::new("3D shader sample"),
        Mesh3d(meshes.add(Cuboid::from_size(extent))),
        MeshMaterial3d(materials.add(game_shaders::PatternMaterial3d {
            color: Color::srgb_u8(255, 157, 70).to_linear(),
        })),
        Transform::from_translation(position),
        layer,
    ));
}

/// Verifies sample composition without a native window or graphics device.
#[cfg(test)]
mod tests {
    use super::ShaderScenePlugin;
    use bevy::prelude::*;

    /// Requires each selected material on a UV-mapped mesh after Startup.
    #[test]
    fn selected_samples_use_custom_materials_and_mesh_uvs() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Shader>()
            .init_asset::<Image>()
            .add_plugins(ShaderScenePlugin);
        app.update();
        #[cfg(feature = "dim2")]
        {
            let mut query = app
                .world_mut()
                .query::<(&Mesh2d, &MeshMaterial2d<game_shaders::PatternMaterial2d>)>();
            let (mesh, material) = query
                .single(app.world())
                .expect("one planar shader sample must exist");
            assert!(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&mesh.0)
                    .expect("sample mesh must exist")
                    .contains_attribute(Mesh::ATTRIBUTE_UV_0)
            );
            assert_eq!(
                app.world()
                    .resource::<Assets<game_shaders::PatternMaterial2d>>()
                    .get(&material.0)
                    .expect("sample material must exist")
                    .color,
                Color::srgb_u8(61, 214, 255).to_linear()
            );
        }
        #[cfg(feature = "dim3")]
        {
            use bevy::camera::visibility::RenderLayers;
            let mut query = app.world_mut().query::<(
                &Mesh3d,
                &MeshMaterial3d<game_shaders::PatternMaterial3d>,
                &RenderLayers,
                &Transform,
            )>();
            let (mesh, material, layer, transform) = query
                .single(app.world())
                .expect("one spatial shader sample must exist");
            assert!(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&mesh.0)
                    .expect("sample mesh must exist")
                    .contains_attribute(Mesh::ATTRIBUTE_UV_0)
            );
            assert_eq!(
                app.world()
                    .resource::<Assets<game_shaders::PatternMaterial3d>>()
                    .get(&material.0)
                    .expect("sample material must exist")
                    .color,
                Color::srgb_u8(255, 157, 70).to_linear()
            );
            #[cfg(feature = "dim2")]
            {
                assert_eq!(*layer, RenderLayers::layer(1));
                assert_eq!(transform.translation, Vec3::new(-2.0, 0.7, 0.0));
            }
            #[cfg(not(feature = "dim2"))]
            {
                assert_eq!(*layer, RenderLayers::default());
                assert_eq!(transform.translation, Vec3::new(-300.0, -180.0, 0.0));
            }
        }
    }
}
