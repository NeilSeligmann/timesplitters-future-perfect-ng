// D3D8-style fixed function: color = texture * diffuse, alpha blended by pipeline.
#import bevy_sprite::mesh2d_functions::{get_world_from_local, mesh2d_position_local_to_clip}

struct Vtx {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(4) color: vec4<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) diffuse: vec4<f32>,
};
@group(2) @binding(0) var tex: texture_2d<f32>;
@group(2) @binding(1) var samp: sampler;
@group(2) @binding(2) var<uniform> modulate: vec4<f32>;

@vertex
fn vertex(v: Vtx) -> VOut {
    var o: VOut;
    let world = get_world_from_local(v.instance_index);
    o.clip = mesh2d_position_local_to_clip(world, vec4<f32>(v.position, 1.0));
    o.uv = v.uv;
    o.diffuse = v.color * modulate;
    return o;
}
@fragment
fn fragment(i: VOut) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, i.uv) * i.diffuse;
}
