//! Checks the public shader material contract without a graphics device.

#[cfg(any(feature = "dim2", feature = "dim3"))]
use bevy::prelude::LinearRgba;
#[cfg(any(feature = "dim2", feature = "dim3"))]
use bevy::shader::ShaderRef;
#[cfg(feature = "dim2")]
use game_shaders::PatternMaterial2d;
#[cfg(feature = "dim3")]
use game_shaders::PatternMaterial3d;
use game_shaders::Plugin as ShaderPlugin;

/// Keeps plugin construction independent of the renderer and platform.
#[test]
fn plugin_is_a_default_value() {
    let _plugin = ShaderPlugin::default();
}

/// Uses an embedded fragment source and a typed linear color in 2D.
#[cfg(feature = "dim2")]
#[test]
fn planar_material_has_an_embedded_shader_and_editable_color() {
    use bevy::sprite_render::Material2d;
    let mut material = PatternMaterial2d::default();
    assert_eq!(material.color, LinearRgba::WHITE);
    material.color = LinearRgba::BLUE;
    assert_eq!(material.color, LinearRgba::BLUE);
    let ShaderRef::Path(path) = PatternMaterial2d::fragment_shader() else {
        panic!("the material must use an embedded shader path");
    };
    assert_eq!(
        path.to_string(),
        "embedded://game_shaders/shaders/pattern2d.wgsl"
    );
}

/// Uses a separate 3D fragment source with the same uniform shape.
#[cfg(feature = "dim3")]
#[test]
fn spatial_material_has_an_embedded_shader_and_editable_color() {
    use bevy::pbr::Material;
    let mut material = PatternMaterial3d::default();
    assert_eq!(material.color, LinearRgba::WHITE);
    material.color = LinearRgba::RED;
    assert_eq!(material.color, LinearRgba::RED);
    let ShaderRef::Path(path) = PatternMaterial3d::fragment_shader() else {
        panic!("the material must use an embedded shader path");
    };
    assert_eq!(
        path.to_string(),
        "embedded://game_shaders/shaders/pattern3d.wgsl"
    );
}

/// Loads an embedded shader through Bevy's real asset source and loader.
fn load_shader(path: &'static str) -> bevy::prelude::Shader {
    use bevy::prelude::*;
    use bevy::shader::ShaderLoader;
    use std::time::Duration;
    use std::time::Instant;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Shader>()
        .register_asset_loader(ShaderLoader)
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .add_plugins(ShaderPlugin::default());
    #[cfg(feature = "dim2")]
    assert!(app.is_plugin_added::<bevy::sprite_render::Material2dPlugin<PatternMaterial2d>>());
    #[cfg(feature = "dim3")]
    assert!(app.is_plugin_added::<MaterialPlugin<PatternMaterial3d>>());
    let handle = app.world().resource::<AssetServer>().load::<Shader>(path);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        if let Some(shader) = app.world().resource::<Assets<Shader>>().get(&handle) {
            return shader.clone();
        }
        assert!(
            Instant::now() < deadline,
            "embedded shader loading timed out: {path}"
        );
        std::thread::yield_now();
    }
}

/// Validates the authored fragment with Bevy's preprocessor and downlevel capabilities.
fn validate_fragment(
    shader: bevy::prelude::Shader,
    import_path: &str,
) -> Result<(), bevy::shader::ShaderCacheError> {
    use bevy::prelude::Assets;
    use bevy::prelude::Shader;
    use bevy::render::render_resource::DownlevelFlags;
    use bevy::render::render_resource::WgpuFeatures;
    use bevy::shader::ShaderCache;
    use bevy::shader::ShaderDefVal;
    let mut shaders = Assets::<Shader>::default();
    // This small vertex interface isolates the fragment's validation. Real Bevy
    // vertex interfaces and GPU pipelines are checked by native/browser launches.
    let vertex_source = format!(
        "#define_import_path {import_path}\nstruct VertexOutput {{ @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, }}"
    );
    let vertex = shaders.add(Shader::from_wgsl(vertex_source, "test/vertex.wgsl"));
    let fragment = shaders.add(shader);
    let mut cache = ShaderCache::new(
        (),
        WgpuFeatures::empty(),
        DownlevelFlags::empty(),
        |(), _, _| Ok(()),
    );
    for handle in [&vertex, &fragment] {
        cache.set_shader(
            handle.id(),
            shaders.get(handle).expect("test shader must exist").clone(),
        );
    }
    cache.get(
        0,
        fragment.id(),
        &[ShaderDefVal::UInt("MATERIAL_BIND_GROUP".into(), 2)],
    )?;
    Ok(())
}

/// Keeps both authored shaders loadable and valid without optional GPU capabilities.
#[test]
fn embedded_fragments_load_and_validate_for_downlevel_rendering() {
    for (path, import_path) in [
        (
            "embedded://game_shaders/shaders/pattern2d.wgsl",
            "bevy_sprite::mesh2d_vertex_output",
        ),
        (
            "embedded://game_shaders/shaders/pattern3d.wgsl",
            "bevy_pbr::forward_io",
        ),
    ] {
        let shader = load_shader(path);
        validate_fragment(shader, import_path).expect("the authored fragment must validate");
    }
}

/// Confirms that the validation gate rejects malformed WGSL instead of passing any source.
#[test]
fn malformed_fragment_fails_validation() {
    let shader = bevy::prelude::Shader::from_wgsl("@fragment fn fragment( {", "test/broken.wgsl");
    assert!(validate_fragment(shader, "bevy_sprite::mesh2d_vertex_output").is_err());
}
