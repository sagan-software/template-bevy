// Bevy 0.19.1 material interface: https://github.com/bevyengine/bevy/tree/v0.19.1/examples/shader
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material_color: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    // UVs are dimensionless. Repeat eight bands per UV unit with half-width bright bands.
    let band = select(0.3, 1.0, fract((mesh.uv.x + mesh.uv.y) * 8.0) < 0.5);
    // Scale linear RGB brightness to 30% or 100% while retaining the supplied alpha.
    return vec4<f32>(material_color.rgb * band, material_color.a);
}
