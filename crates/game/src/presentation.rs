//! Render-only camera, particles, and network-body visualization.

use crate::components::DemoBody;
use crate::components::NetworkPath;
use crate::simulation::ARENA_HALF_HEIGHT;
use crate::simulation::ARENA_HALF_WIDTH;
use crate::systems::DemoSystems;
use bevy::prelude::*;
use game_physics::LinearVelocity;
use game_physics::Position;
use game_physics::Rotation;
use lightyear::frame_interpolation::FrameInterpolationSystems;
use lightyear::prelude::Interpolated;
use lightyear::prelude::Predicted;
use lightyear::prelude::Remote;
use lightyear::prelude::Replicate;
use lightyear::prelude::RollbackSystems;

/// Installs all visual behavior in the rendered client world.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    /// Creates persistent scene assets and draws corrected network state.
    fn build(&self, app: &mut App) {
        // Hanabi requires native GPU capabilities absent from this WebGL profile.
        std::cfg_select! {
            not(target_family = "wasm") => { app.add_plugins(crate::particles::ParticlePlugin); }
            _ => {}
        }
        app.add_systems(Startup, setup_scene).add_systems(
            PostUpdate,
            draw_network_bodies
                .in_set(DemoSystems::Presentation)
                .after(FrameInterpolationSystems::Interpolate)
                .after(RollbackSystems::VisualCorrection),
        );
    }
}

/// Creates the selected camera for planar networked gameplay.
fn setup_scene(mut commands: Commands<'_, '_>) {
    #[cfg(feature = "dim2")]
    commands.spawn((
        Name::new("2D Camera"),
        Camera2d,
        Projection::Orthographic(crate::camera::planar_projection()),
        Camera {
            order: -1,
            ..default()
        },
    ));

    #[cfg(all(feature = "dim3", not(feature = "dim2")))]
    commands.spawn((
        Name::new("3D Camera"),
        Camera3d::default(),
        // Fit XY gameplay while keeping the 3D near plane at zero world units.
        Projection::Orthographic(OrthographicProjection {
            near: 0.0,
            ..crate::camera::planar_projection()
        }),
        Transform::from_xyz(0.0, 0.0, 750.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Draws receiver state after network correction and interpolation complete.
#[cfg(feature = "dim2")]
fn draw_network_bodies(
    mut gizmos: Gizmos<'_, '_>,
    bodies: Query<
        '_,
        '_,
        (
            &DemoBody,
            &Position,
            &Rotation,
            &LinearVelocity,
            Has<Predicted>,
            Has<Interpolated>,
            Has<Remote>,
            Has<Replicate>,
        ),
    >,
) {
    // Draw arena bounds before receiver bodies so their context remains visible.
    gizmos.rect_2d(
        Isometry2d::IDENTITY,
        Vec2::new(ARENA_HALF_WIDTH * 2.0, ARENA_HALF_HEIGHT * 2.0),
        Color::srgb_u8(62, 74, 92),
    );

    for (body, position, rotation, velocity, predicted, interpolated, remote, replicate) in &bodies
    {
        let received_path = match (predicted, interpolated) {
            (true, false) => Some(NetworkPath::Prediction),
            (false, true) => Some(NetworkPath::Interpolation),
            (false, false) | (true, true) => None,
        };
        let Some(kind) = classify_receiver(body.path(), received_path, remote && !replicate) else {
            continue;
        };
        let color = kind.color();

        match kind {
            ReceiverPresentation::PredictedCircle => {
                gizmos.circle_2d(position.0, body.half_extent().x, color);
            }
            ReceiverPresentation::InterpolatedBox => {
                gizmos.rect_2d(
                    Isometry2d {
                        rotation: rotation.0,
                        translation: position.0,
                    },
                    body.half_extent() * 2.0,
                    color,
                );
            }
        }

        // A heading line makes input response and interpolation delay visible
        // even when the shape's rotation is subtle.
        let heading = velocity.0.try_normalize().unwrap_or(Vec2::X) * 38.0;
        gizmos.line_2d(position.0, position.0 + heading, color);
    }
}

/// Visual kind assigned only to genuine receive-side network entities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReceiverPresentation {
    /// Cyan circle driven by local prediction and correction.
    PredictedCircle,
    /// Orange rectangle driven by confirmed snapshot interpolation.
    InterpolatedBox,
}

impl ReceiverPresentation {
    /// Returns the stable teaching color for this network path.
    const fn color(self) -> Color {
        match self {
            Self::PredictedCircle => Color::srgb_u8(61, 214, 255),
            Self::InterpolatedBox => Color::srgb_u8(255, 157, 70),
        }
    }
}

/// Rejects authoritative send-side markers before selecting a visual shape.
const fn classify_receiver(
    path: NetworkPath,
    received_path: Option<NetworkPath>,
    remote_receiver: bool,
) -> Option<ReceiverPresentation> {
    match (path, received_path, remote_receiver) {
        (NetworkPath::Prediction, Some(NetworkPath::Prediction), true) => {
            Some(ReceiverPresentation::PredictedCircle)
        }
        (NetworkPath::Interpolation, Some(NetworkPath::Interpolation), true) => {
            Some(ReceiverPresentation::InterpolatedBox)
        }
        _ => None,
    }
}

/// Draws the same confirmed state with three-dimensional geometry.
#[cfg(all(feature = "dim3", not(feature = "dim2")))]
fn draw_network_bodies(
    mut gizmos: Gizmos<'_, '_>,
    bodies: Query<
        '_,
        '_,
        (
            &DemoBody,
            &Position,
            &Rotation,
            &LinearVelocity,
            Has<Predicted>,
            Has<Interpolated>,
            Has<Remote>,
            Has<Replicate>,
        ),
    >,
) {
    gizmos.rect(
        Isometry3d::IDENTITY,
        Vec2::new(ARENA_HALF_WIDTH * 2.0, ARENA_HALF_HEIGHT * 2.0),
        Color::srgb_u8(62, 74, 92),
    );
    for (body, position, rotation, velocity, predicted, interpolated, remote, replicate) in &bodies
    {
        let received = match (predicted, interpolated) {
            (true, false) => Some(NetworkPath::Prediction),
            (false, true) => Some(NetworkPath::Interpolation),
            _ => None,
        };
        let Some(kind) = classify_receiver(body.path(), received, remote && !replicate) else {
            continue;
        };
        let center = position.0.extend(0.0);
        match kind {
            ReceiverPresentation::PredictedCircle => {
                gizmos.sphere(
                    Isometry3d::from_translation(center),
                    body.half_extent().x,
                    kind.color(),
                );
            }
            ReceiverPresentation::InterpolatedBox => {
                gizmos.cube(
                    Transform::from_translation(center)
                        .with_rotation(Quat::from_rotation_z(rotation.0.as_radians()))
                        .with_scale((body.half_extent() * 2.0).extend(44.0)),
                    kind.color(),
                );
            }
        }
        gizmos.line(
            center,
            center + (velocity.0 * 0.2).extend(0.0),
            kind.color(),
        );
    }
}

/// Covers receiver classification without constructing a renderer.
#[cfg(test)]
mod tests {
    use super::PresentationPlugin;
    use super::ReceiverPresentation;
    use super::classify_receiver;
    use super::setup_scene;
    use crate::components::NetworkPath;
    use bevy::camera::CameraProjection;
    use bevy::gizmos::GizmoPlugin;
    use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
    use bevy::prelude::*;
    use bevy_hanabi::prelude::EffectAsset;
    use bevy_hanabi::prelude::HanabiPlugin;
    use bevy_hanabi::prelude::ParticleEffect;

    /// Keeps the full planar arena visible in every selected camera dimension.
    #[test]
    fn camera_preserves_arena_at_portrait_and_landscape_sizes() {
        let mut app = App::new();
        app.add_systems(Startup, setup_scene);
        app.update();
        let mut cameras = app.world_mut().query::<&Projection>();
        let projection = cameras.single(app.world()).expect("the camera must exist");
        let Projection::Orthographic(projection) = projection else {
            panic!("the planar arena requires a fitted orthographic projection");
        };
        for (width, height) in [(960.0, 600.0), (390.0, 844.0)] {
            let mut projection = projection.clone();
            projection.update(width, height);
            assert!(projection.area.width() >= 800.0);
            assert!(projection.area.height() >= 500.0);
        }
    }

    /// Builds the rendered domain's platform-neutral main-World substrate.
    fn headless_presentation_app() -> App {
        let mut app = App::new();
        // Asset and gizmo plugins provide the resources used by Startup and
        // PostUpdate without creating a Winit event loop or GPU render sub-app.
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), GizmoPlugin))
            // Hanabi normally receives mesh and shader assets from RenderPlugin.
            // Supplying only those collections lets its main-World systems run
            // without constructing a GPU render sub-app.
            .init_asset::<Mesh>()
            .init_asset::<SkinnedMeshInverseBindposes>()
            .init_asset::<Shader>()
            .add_plugins(PresentationPlugin);
        app
    }

    /// Builds presentation and runs Startup without requiring a window or GPU.
    #[test]
    fn presentation_plugin_composes_in_a_headless_main_world() {
        let mut app = headless_presentation_app();

        // `App::update` exercises project-owned Startup systems but deliberately
        // does not call Hanabi's GPU-dependent plugin finish hook.
        app.update();

        let camera_count = app
            .world_mut()
            .query_filtered::<Entity, With<Camera>>()
            .iter(app.world())
            .count();
        let effect_count = app
            .world_mut()
            .query_filtered::<Entity, With<ParticleEffect>>()
            .iter(app.world())
            .count();
        assert!(app.is_plugin_added::<PresentationPlugin>());
        assert!(app.is_plugin_added::<HanabiPlugin>());
        assert!(!app.is_plugin_added::<WindowPlugin>());
        assert_eq!(camera_count, 1);
        assert_eq!(effect_count, 1);
        assert_eq!(app.world().resource::<Assets<EffectAsset>>().len(), 1);
    }

    /// Recognizes only a remote, receiver-created predicted entity.
    #[test]
    fn predicted_classification_rejects_authority_markers() {
        assert_eq!(
            classify_receiver(NetworkPath::Prediction, Some(NetworkPath::Prediction), true,),
            Some(ReceiverPresentation::PredictedCircle)
        );
        assert_eq!(
            classify_receiver(
                NetworkPath::Prediction,
                Some(NetworkPath::Prediction),
                false,
            ),
            None
        );
        assert_eq!(
            classify_receiver(
                NetworkPath::Prediction,
                Some(NetworkPath::Interpolation),
                true,
            ),
            None
        );
    }

    /// Recognizes only a remote, receiver-created interpolation entity.
    #[test]
    fn interpolated_classification_rejects_incomplete_markers() {
        assert_eq!(
            classify_receiver(
                NetworkPath::Interpolation,
                Some(NetworkPath::Interpolation),
                true,
            ),
            Some(ReceiverPresentation::InterpolatedBox)
        );
        assert_eq!(
            classify_receiver(NetworkPath::Interpolation, None, true),
            None
        );
    }
}
