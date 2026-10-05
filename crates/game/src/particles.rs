//! Native GPU particle assets and startup composition.

use bevy::prelude::*;
use bevy_hanabi::Attribute;
use bevy_hanabi::ColorBlendMask;
use bevy_hanabi::ColorBlendMode;
use bevy_hanabi::ColorOverLifetimeModifier;
use bevy_hanabi::EffectAsset;
use bevy_hanabi::Gradient;
use bevy_hanabi::HanabiPlugin;
use bevy_hanabi::LinearDragModifier;
use bevy_hanabi::Module;
use bevy_hanabi::ParticleEffect;
use bevy_hanabi::SetAttributeModifier;
use bevy_hanabi::SetPositionCircleModifier;
use bevy_hanabi::ShapeDimension;
use bevy_hanabi::SpawnerSettings;

/// Keeps native particle systems separate from the browser's WebGL renderer.
pub(super) struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    /// Registers Hanabi and creates the reusable effect once at startup.
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .add_systems(Startup, setup_particles);
    }
}

/// Creates one asset and attaches it to the scene's particle landmark.
fn setup_particles(mut commands: Commands<'_, '_>, mut effects: ResMut<'_, Assets<EffectAsset>>) {
    // Per-frame systems reuse the effect instead of rebuilding its GPU graph.
    let effect = create_network_spark_effect();
    commands.spawn((
        Name::new("Hanabi Network Sparks"),
        ParticleEffect::new(effects.add(effect)),
        Transform::from_xyz(0.0, -155.0, 10.0),
    ));
}

/// Authors a particle effect constrained to the two-dimensional XY plane.
fn create_network_spark_effect() -> EffectAsset {
    let mut module = Module::default();
    // Hanabi is a 3D particle simulator even in a 2D game. A Z-axis circle and
    // zero-Z velocity keep every spark in the camera's XY gameplay plane.
    let init_position = planar_spawn_position(&mut module);
    let init_velocity =
        SetAttributeModifier::new(Attribute::VELOCITY, module.lit(Vec3::new(0.0, 72.0, 0.0)));
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, module.lit(0.85));
    let init_size = SetAttributeModifier::new(Attribute::SIZE, module.lit(8.0));
    let drag = LinearDragModifier::new(module.lit(1.4));

    // Build the GPU graph in the same order a particle experiences it: assign
    // initial attributes once, apply drag each simulation tick, then color the
    // resulting particle while rendering it.
    let effect = EffectAsset::new(256, SpawnerSettings::rate(75.0.into()), module)
        .with_name("2D Network Sparks")
        .init(init_position)
        .init(init_velocity)
        .init(init_lifetime)
        .init(init_size)
        .update(drag)
        .render(ColorOverLifetimeModifier {
            gradient: particle_gradient(),
            blend: ColorBlendMode::Overwrite,
            mask: ColorBlendMask::RGBA,
        });
    // Keep sparks above the arena gizmos without giving them real Z motion.
    #[cfg(feature = "dim2")]
    let effect = {
        let mut planar_effect = effect;
        planar_effect.z_layer_2d = 10.0;
        planar_effect
    };
    effect
}

/// Defines color and alpha at birth, transit, and expiration in lifetime order.
fn particle_gradient() -> Gradient<Vec4> {
    // The color keys tell a short visual story over each particle's lifetime:
    // a bright network-blue birth, a warm transit phase, then a transparent
    // fade that avoids particles popping out of existence.
    let mut gradient = Gradient::new();
    gradient.add_key(0.0, Vec4::new(0.20, 0.85, 1.0, 0.9));
    gradient.add_key(0.45, Vec4::new(0.95, 0.42, 0.18, 0.7));
    gradient.add_key(1.0, Vec4::new(0.22, 0.08, 0.35, 0.0));

    gradient
}

/// Creates the particle birth circle in XY coordinates around the Z axis.
fn planar_spawn_position(module: &mut Module) -> SetPositionCircleModifier {
    // All generated birth coordinates share the gameplay plane's axis and origin.
    SetPositionCircleModifier {
        center: module.lit(Vec3::ZERO),
        axis: module.lit(Vec3::Z),
        radius: module.lit(12.0),
        dimension: ShapeDimension::Surface,
    }
}
