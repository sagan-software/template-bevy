//! Side-by-side scenes when the project enables both dimensions.

use bevy::camera::Viewport;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use game_physics::rapier3d::prelude::Collider;
use game_physics::rapier3d::prelude::NoUserData;
use game_physics::rapier3d::prelude::RapierPhysicsPlugin;
use game_physics::rapier3d::prelude::Restitution;
use game_physics::rapier3d::prelude::RigidBody;

/// Adds an independent 3D Rapier scene beside the primary 2D gameplay.
pub(crate) struct DimensionsPlugin;

impl Plugin for DimensionsPlugin {
    /// Keeps each renderer's camera and physics components in its own scene.
    fn build(&self, app: &mut App) {
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .add_systems(Startup, setup_scene)
            .add_systems(Update, resize_viewports);
    }
}

/// Identifies the 3D camera without relying on its display name.
#[derive(Component)]
struct ThreeDimensionalCamera;

/// Creates a 3D ball, floor, light, and separate render layer.
fn setup_scene(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1,
            ..default()
        },
        ThreeDimensionalCamera,
        RenderLayers::layer(1),
        Transform::from_xyz(8.0, 6.0, 9.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        Name::new("3D Rapier ball"),
        Mesh3d(meshes.add(Sphere::new(0.6))),
        MeshMaterial3d(materials.add(Color::srgb_u8(255, 157, 70))),
        RigidBody::Dynamic,
        Collider::ball(0.6),
        Restitution::coefficient(0.8),
        RenderLayers::layer(1),
        Transform::from_xyz(0.0, 4.0, 0.0),
    ));
    commands.spawn((
        Name::new("3D Rapier floor"),
        Mesh3d(meshes.add(Cuboid::new(10.0, 0.2, 10.0))),
        MeshMaterial3d(materials.add(Color::srgb_u8(62, 74, 92))),
        RigidBody::Fixed,
        Collider::cuboid(5.0, 0.1, 5.0),
        RenderLayers::layer(1),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        RenderLayers::layer(1),
        Transform::from_xyz(3.0, 6.0, 4.0),
    ));
}

/// Keeps both camera viewports within the window after every size change.
fn resize_viewports(
    windows: Query<'_, '_, &Window>,
    mut cameras: Query<'_, '_, (&mut Camera, Has<ThreeDimensionalCamera>)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some([left, right]) = split_viewports(UVec2::new(
        window.physical_width(),
        window.physical_height(),
    )) else {
        return;
    };
    for (mut camera, three_dimensional) in &mut cameras {
        camera.viewport = Some(if three_dimensional {
            right.clone()
        } else {
            left.clone()
        });
    }
}

/// Divides physical pixels without a gap, overlap, or zero-sized viewport.
fn split_viewports(size: UVec2) -> Option<[Viewport; 2]> {
    if size.x < 2 || size.y == 0 {
        return None;
    }
    let width = size.x / 2;
    Some([
        Viewport {
            physical_position: UVec2::ZERO,
            physical_size: UVec2::new(width, size.y),
            ..default()
        },
        Viewport {
            physical_position: UVec2::new(width, 0),
            physical_size: UVec2::new(size.x - width, size.y),
            ..default()
        },
    ])
}

/// Covers minimized windows and odd physical widths.
#[cfg(test)]
mod tests {
    use super::split_viewports;
    use bevy::prelude::UVec2;

    /// Rejects dimensions that cannot hold two positive viewports.
    #[test]
    fn minimized_windows_do_not_create_invalid_viewports() {
        for size in [UVec2::ZERO, UVec2::new(1, 600), UVec2::new(960, 0)] {
            assert!(split_viewports(size).is_none());
        }
    }

    /// Assigns the odd remaining pixel to the right viewport.
    #[test]
    fn viewports_cover_even_and_odd_windows() {
        for width in [2, 960, 961] {
            let [left, right] = split_viewports(UVec2::new(width, 600)).expect("window is visible");
            assert_eq!(left.physical_position, UVec2::ZERO);
            assert_eq!(right.physical_position.x, left.physical_size.x);
            assert_eq!(left.physical_size.x + right.physical_size.x, width);
            assert_eq!(left.physical_size.y, 600);
            assert_eq!(right.physical_size.y, 600);
        }
    }
}
