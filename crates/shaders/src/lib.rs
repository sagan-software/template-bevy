//! Embeds portable WGSL materials for native and browser rendering.
//!
//! Add `Plugin` after Bevy's renderer plugins. Select `dim2`, `dim3`, or both
//! to register the corresponding material. Materials require meshes with UVs.
//! Shader source files ship inside native executables and Wasm modules.
//! The fragment uses no textures, storage buffers, or compute stages.

#[cfg(feature = "dim2")]
mod pattern2d;
#[cfg(feature = "dim3")]
mod pattern3d;
mod plugin;
#[cfg(feature = "dim2")]
pub use self::pattern2d::PatternMaterial2d;
#[cfg(feature = "dim3")]
pub use self::pattern3d::PatternMaterial3d;
pub use self::plugin::ShaderPlugin as Plugin;
