# Shaders

Every generated project includes `crates/shaders`. Local and networked clients
install its material pipelines automatically and display a striped shader sample.
The headless authority does not install rendering plugins. Shader support does
not depend on Blender assets, networking, inspection, or the generation options.

## Selected approach

Verified on 2026-10-05 against the latest stable Bevy 0.19.1 and Rust 1.99.0.
The template uses WGSL with typed Rust materials, following Bevy's
[2D material example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/shader/shader_material_2d.rs)
and [3D material example](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/shader/shader_material.rs).
The locked renderer uses wgpu and Naga 29.0.4 with naga_oil 0.22.0.
No additional shader compiler, nightly toolchain, or graphics plugin is required.

[WGSL](https://www.w3.org/TR/WGSL/) is the WebGPU shading language.
Bevy adds its own import and material binding substitutions before WGSL validation.
This template targets that Bevy 0.19.1 interface rather than claiming complete
conformance with every WGSL extension or hardware capability.

[Rust GPU](https://github.com/Rust-GPU/rust-gpu) compiles Rust shader crates to
SPIR-V. Its [current toolchain](https://github.com/Rust-GPU/rust-gpu/blob/main/rust-toolchain.toml)
requires nightly 2026-07-03 with compiler development components.
Its [build guide](https://rust-gpu.github.io/rust-gpu/book/building-rust-gpu.html)
also describes the separate compiler and SPIRV-Tools build.
A browser integration needs an additional supported conversion path to WGSL or
GLSL. Rust GPU can suit a project that needs shared CPU/GPU Rust code, but its
compiler pin and conversion pipeline add build and compatibility work here.
WGSL fits this template's stable compiler, existing Bevy renderer, and browser target.

## Files and bindings

- `crates/shaders/src/shaders/pattern2d.wgsl` imports Bevy's 2D vertex output.
- `crates/shaders/src/shaders/pattern3d.wgsl` imports Bevy's 3D vertex output.
- `PatternMaterial2d` implements `Material2d` when `dim2` is enabled.
- `PatternMaterial3d` implements `Material` when `dim3` is enabled.
- `game_shaders::Plugin` embeds both files and installs the selected material pipelines.

Without dimension features, the shader plugin registers the embedded sources
and omits the built-in material pipelines. Shader tests cover this configuration
as well as both material dimensions.

Each material stores a `LinearRgba` color and defaults to white.
Rust's `AsBindGroup` maps that color to binding 0 as one 16-byte `vec4<f32>` uniform.
Bevy supplies the material bind group through `#{MATERIAL_BIND_GROUP}`.
Keep Rust's binding attributes and WGSL's group, binding, and field types aligned.

Meshes must provide UV coordinates through `Mesh::ATTRIBUTE_UV_0`.
The fragment repeats eight diagonal bands per dimensionless UV unit.
Bright bands cover half each repetition; dark bands use 30% linear RGB brightness.
The shader retains the color's alpha, while the material's opaque mode does not blend transparency.
The 2D and 3D materials have separate vertex interfaces and asset types.

Shader files are [embedded assets](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_asset/src/io/embedded/mod.rs).
Their paths begin with `embedded://game_shaders/shaders/` and remain stable when
the application is renamed. Native packages and Wasm modules contain the source;
no loose shader files or Trunk copy directives are required.

## Use a material

The game plugin already installs the shader plugin. For an independent Bevy app,
add `game_shaders::Plugin::default()` after `DefaultPlugins`.
Add `game-shaders` as a dependency of any crate that directly uses its materials.
The workspace dependency is already configured.

```rust,ignore
use bevy::prelude::*;
use game_shaders::PatternMaterial2d;

fn spawn_sample(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<PatternMaterial2d>>,
) {
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(80.0, 80.0))),
        MeshMaterial2d(materials.add(PatternMaterial2d {
            color: Color::srgb_u8(61, 214, 255).to_linear(),
        })),
    ));
}
```

For 3D, use `PatternMaterial3d`, `Mesh3d`, and `MeshMaterial3d` with a UV-mapped mesh.
The standalone guide runs without networking or Blender assets:

```sh
cargo run -p game-shaders --example material --features dim2
```

Keep shader ownership in this crate so unrelated gameplay edits can reuse it.
Before changing uniforms, update the Rust material and WGSL declaration together.
When adding another material, register its source and pipeline through the shader plugin.

## Development and browser targets

Edit the WGSL files directly. Ordinary builds embed the edited bytes during compilation.
For native source watching, run the application with `--features dev`.
The shader crate enables Bevy's embedded asset watcher and file watcher in that mode.
Keep features consistent across workers to retain the matching build cache.

The existing `web` feature selects WebGL2 for `wasm32-unknown-unknown`.
[wgpu 29.0.4](https://github.com/gfx-rs/wgpu/blob/v29.0.4/README.md)
supports native backends, WebGL2, and WebGPU.
Its WebGL backend translates WGSL to GLSL through Naga.
The starter fragment uses float32 UV arithmetic and a color uniform.
It uses no compute stages, storage buffers, textures, subgroup operations, or float64 values.

Bevy's material preprocessing supplies each backend's binding layout.
The same authored fragments therefore serve native and WebGL2 builds.
Browser compute effects need a WebGPU target and supported adapter capabilities;
the existing native Hanabi effects remain excluded from this WebGL2 profile.
The template does not automatically switch browsers to WebGPU.

```sh
trunk build
trunk serve
```

## Verification

```sh
cargo test -p game-shaders --features dim2,dim3
cargo xtask check
cargo xtask generate-matrix
```

The shader tests load the embedded files through Bevy's asset reader and loader.
They validate the fragments with Bevy's shader preprocessor and no optional GPU capabilities.
A malformed fragment must fail that gate. The test vertex interface isolates the
fragment; real native and browser launches verify Bevy's complete GPU pipeline.

Game tests require custom materials on UV-mapped sample meshes in the selected dimensions.
Generation checks compile native configurations and build 2D and 3D Wasm outputs.
Compilation alone does not prove that a shader renders. Before publishing changes,
inspect the native and browser samples and check shader or pipeline errors.
