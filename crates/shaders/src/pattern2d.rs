//! Defines the typed uniform and fragment shader for 2d meshes.

use bevy::prelude::Asset;
use bevy::prelude::LinearRgba;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::Material2d;

/// Opaque UV bands with a linear RGBA color uniform.
/// Meshes must supply UV coordinates. The default color is white.
/// Set the tint before assigning this material to a mesh.
#[derive(Asset, AsBindGroup, Clone, Copy, Debug, TypePath)]
pub struct PatternMaterial2d {
    /// Linear color multiplied by the shader's alternating brightness bands.
    /// The shader scales RGB brightness and retains this alpha; material rendering
    /// remains opaque. Convert sRGB colors before assignment.
    #[uniform(0)]
    pub color: LinearRgba,
}

impl Default for PatternMaterial2d {
    /// Selects white bands before the caller supplies a tint.
    fn default() -> Self {
        Self {
            color: LinearRgba::WHITE,
        }
    }
}

impl Material2d for PatternMaterial2d {
    /// Selects the embedded WGSL fragment program for this dimension.
    fn fragment_shader() -> ShaderRef {
        "embedded://game_shaders/shaders/pattern2d.wgsl".into()
    }
}
