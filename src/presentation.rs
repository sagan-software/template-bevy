//! Render-only camera, particles, and network-body visualization.

use crate::{
    components::{DemoBody, NetworkPath},
    simulation::{ARENA_HALF_HEIGHT, ARENA_HALF_WIDTH},
    systems::DemoSystems,
};
use avian2d::prelude::{LinearVelocity, Position, Rotation};
use bevy::prelude::*;
use bevy_hanabi::Gradient;
use bevy_hanabi::prelude::*;
use lightyear::{
    frame_interpolation::FrameInterpolationSystems,
    prelude::{Interpolated, Predicted, Remote, Replicate, RollbackSystems},
};

/// Installs all visual behavior in the rendered client world.
#[derive(Clone, Copy, Debug, Default)]
pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    /// Creates persistent scene assets and draws corrected network state.
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .add_systems(Startup, setup_scene)
            .add_systems(
                PostUpdate,
                draw_network_bodies
                    .in_set(DemoSystems::Presentation)
                    .after(FrameInterpolationSystems::Interpolate)
                    .after(RollbackSystems::VisualCorrection),
            );
    }
}

/// Creates the 2D camera and one reusable Hanabi particle landmark.
fn setup_scene(mut commands: Commands<'_, '_>, mut effects: ResMut<'_, Assets<EffectAsset>>) {
    commands.spawn((
        Name::new("2D Camera"),
        Camera2d,
        Camera {
            order: -1,
            ..default()
        },
    ));

    // The effect asset is authored once during Startup. Per-frame systems only
    // update particle simulation and never rebuild gradients or shader modules.
    let effect = create_network_spark_effect();
    commands.spawn((
        Name::new("2D Hanabi Network Sparks"),
        ParticleEffect::new(effects.add(effect)),
        Transform::from_xyz(0.0, -155.0, 10.0),
    ));
}

/// Authors a particle effect constrained to the two-dimensional XY plane.
fn create_network_spark_effect() -> EffectAsset {
    // The color keys tell a short visual story over each particle's lifetime:
    // a bright network-blue birth, a warm transit phase, then a transparent
    // fade that avoids particles popping out of existence.
    let mut gradient = Gradient::new();
    gradient.add_key(0.0, Vec4::new(0.20, 0.85, 1.0, 0.9));
    gradient.add_key(0.45, Vec4::new(0.95, 0.42, 0.18, 0.7));
    gradient.add_key(1.0, Vec4::new(0.22, 0.08, 0.35, 0.0));

    let mut module = Module::default();
    // Hanabi is a 3D particle simulator even in a 2D game. A Z-axis circle and
    // zero-Z velocity keep every spark in the camera's XY gameplay plane.
    let init_position = SetPositionCircleModifier {
        center: module.lit(Vec3::ZERO),
        axis: module.lit(Vec3::Z),
        radius: module.lit(12.0),
        dimension: ShapeDimension::Surface,
    };
    let init_velocity =
        SetAttributeModifier::new(Attribute::VELOCITY, module.lit(Vec3::new(0.0, 72.0, 0.0)));
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, module.lit(0.85));
    let init_size = SetAttributeModifier::new(Attribute::SIZE, module.lit(8.0));
    let drag = LinearDragModifier::new(module.lit(1.4));

    // Build the GPU graph in the same order a particle experiences it: assign
    // initial attributes once, apply drag each simulation tick, then color the
    // resulting particle while rendering it.
    let mut effect = EffectAsset::new(256, SpawnerSettings::rate(75.0.into()), module)
        .with_name("2D Network Sparks")
        .init(init_position)
        .init(init_velocity)
        .init(init_lifetime)
        .init(init_size)
        .update(drag)
        .render(ColorOverLifetimeModifier {
            gradient,
            blend: ColorBlendMode::Overwrite,
            mask: ColorBlendMask::RGBA,
        });
    // Keep sparks above the arena gizmos without giving them real Z motion.
    effect.z_layer_2d = 10.0;
    effect
}

/// Draws receiver state after network correction and interpolation complete.
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
                        rotation: Rot2 {
                            sin: rotation.sin,
                            cos: rotation.cos,
                        },
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

/// Covers receiver classification without constructing a renderer.
#[cfg(all(test, not(coverage)))]
mod tests {
    use super::{PresentationPlugin, ReceiverPresentation, classify_receiver};
    use crate::components::NetworkPath;
    use bevy::{gizmos::GizmoPlugin, mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
    use bevy_hanabi::prelude::{EffectAsset, HanabiPlugin, ParticleEffect};

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
            .query_filtered::<Entity, With<Camera2d>>()
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
