// T2024 stand-in shader. Two vertex modes selected by `params.clip_space`:
//   0: screen space pixel vertices (the HUD/menu layouts of slice 1, xy in pixels, z depth, w ignored)
//   1: clip space vertices produced on the CPU by the NV2A vertex program interpreter (slice 3), passed through
// Two fragment modes selected by `params.combiner`:
//   0: the INFERRED slice 1 stand-in, diffuse colour modulated by the stage 0 texture
//   1: the register combiners decoded from the bound pixel shader definition (src/combiner.rs documents the word layout, this
//      code mirrors `PixelProgram::evaluate`): general stages on one register file, then the final combiner
// Then the optional alpha test.
struct Params {
    screen: vec2<f32>,
    alpha_ref: f32,
    alpha_func: u32,
    textured: u32,
    clip_space: u32,
    combiner: u32,
    pad0: u32,
    // the combiner block, 432 bytes, see PixelProgram::uniform_bytes
    head: vec4<u32>,                 // count, final ABCD, final EFG, texture0
    final_constant0: vec4<f32>,
    final_constant1: vec4<f32>,
    stages: array<vec4<u32>, 8>,     // rgb inputs, alpha inputs, rgb outputs, alpha outputs
    constants: array<vec4<f32>, 16>, // constant 0 and 1 of each stage
    fog_colour: vec4<f32>,           // the combiner's fog register colour (slice 6), alpha 0
    uv_scale01: vec4<f32>,           // texture coordinate scale of stages 0 and 1 (slice 8, small render target surfaces)
    uv_scale23: vec4<f32>,           // stages 2 and 3
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var stage0: texture_2d<f32>;
@group(0) @binding(2) var stage0_sampler: sampler;
@group(0) @binding(3) var stage1: texture_2d<f32>;
@group(0) @binding(4) var stage1_sampler: sampler;
@group(0) @binding(5) var stage2: texture_2d<f32>;
@group(0) @binding(6) var stage2_sampler: sampler;
@group(0) @binding(7) var stage3: texture_2d<f32>;
@group(0) @binding(8) var stage3_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) uv1: vec2<f32>,
    @location(3) uv2: vec2<f32>,
    @location(4) uv3: vec2<f32>,
    @location(5) fog: f32,
    @location(6) specular: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec4<f32>,
    @location(1) colour: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv1: vec2<f32>,
    @location(4) uv2: vec2<f32>,
    @location(5) uv3: vec2<f32>,
    @location(6) fog: f32,
    @location(7) specular: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;
    if (params.clip_space != 0u) {
        out.position = position;
    } else {
        out.position = vec4<f32>(
            position.x / params.screen.x * 2.0 - 1.0,
            1.0 - position.y / params.screen.y * 2.0,
            clamp(position.z, 0.0, 1.0),
            1.0,
        );
    }
    out.colour = colour;
    out.uv = uv;
    out.uv1 = uv1;
    out.uv2 = uv2;
    out.uv3 = uv3;
    out.fog = fog;
    out.specular = specular;
    return out;
}

// Compare codes follow tsfp_wgpu::nv2a::Compare: 0 never .. 7 always.
fn alpha_passes(alpha: f32, reference: f32, func: u32) -> bool {
    switch func {
        case 0u: { return false; }
        case 1u: { return alpha < reference; }
        case 2u: { return alpha == reference; }
        case 3u: { return alpha <= reference; }
        case 4u: { return alpha > reference; }
        case 5u: { return alpha != reference; }
        case 6u: { return alpha >= reference; }
        default: { return true; }
    }
}

fn combiner_map(x: f32, mapping: u32) -> f32 {
    let unit = clamp(x, 0.0, 1.0);
    switch mapping {
        case 0u: { return unit; }
        case 1u: { return 1.0 - unit; }
        case 2u: { return 2.0 * unit - 1.0; }
        case 3u: { return -(2.0 * unit - 1.0); }
        case 4u: { return unit - 0.5; }
        case 5u: { return -(unit - 0.5); }
        case 6u: { return clamp(x, -1.0, 1.0); }
        default: { return -clamp(x, -1.0, 1.0); }
    }
}

fn combiner_input(regs: ptr<function, array<vec4<f32>, 16>>, byte: u32, alpha_portion: bool) -> vec3<f32> {
    let value = (*regs)[byte & 0xFu];
    let channel = ((byte >> 4u) & 1u) != 0u;
    var base = value.rgb;
    if (alpha_portion) {
        if (channel) { base = vec3<f32>(value.a); } else { base = vec3<f32>(value.b); }
    } else if (channel) {
        base = vec3<f32>(value.a);
    }
    let mapping = byte >> 5u;
    return vec3<f32>(combiner_map(base.x, mapping), combiner_map(base.y, mapping), combiner_map(base.z, mapping));
}

// Output mapping, bits 15 to 17 of the output word (xemu get_output): 1 bias, 2 times 2, 3 times 2 with bias, 4 times 4, 6 divide by 2.
fn combiner_output_map(x: vec3<f32>, output: u32) -> vec3<f32> {
    switch (output >> 15u) & 7u {
        case 1u: { return x - vec3<f32>(0.5); }
        case 2u: { return x * 2.0; }
        case 3u: { return (x - vec3<f32>(0.5)) * 2.0; }
        case 4u: { return x * 4.0; }
        case 6u: { return x / 2.0; }
        default: { return x; }
    }
}

fn combiner_write_rgb(regs: ptr<function, array<vec4<f32>, 16>>, destination: u32, value: vec3<f32>) {
    if (destination != 0u && destination < 16u) {
        let old = (*regs)[destination];
        (*regs)[destination] = vec4<f32>(value, old.a);
    }
}

fn combiner_write_alpha(regs: ptr<function, array<vec4<f32>, 16>>, destination: u32, value: f32) {
    if (destination != 0u && destination < 16u) {
        let old = (*regs)[destination];
        (*regs)[destination] = vec4<f32>(old.rgb, value);
    }
}

fn run_combiner(diffuse: vec4<f32>, specular: vec4<f32>, texels: array<vec4<f32>, 4>, fog: f32) -> vec4<f32> {
    var regs: array<vec4<f32>, 16>;
    for (var i = 0u; i < 16u; i = i + 1u) {
        regs[i] = vec4<f32>(0.0);
    }
    regs[3] = vec4<f32>(params.fog_colour.rgb, fog);
    regs[4] = diffuse;
    regs[5] = specular;
    // head.w is the mask of the stages that sample a texture, registers 8 to 11
    for (var t = 0u; t < 4u; t = t + 1u) {
        if (((params.head.w >> t) & 1u) != 0u) {
            regs[8u + t] = texels[t];
        }
    }
    // head.x: stage count in the low byte, the combiner count word flags (>> 8) above: bit 0 mux on the r0 alpha msb
    let stage_count = params.head.x & 0xFFu;
    let mux_msb = ((params.head.x >> 8u) & 1u) != 0u;
    for (var stage = 0u; stage < stage_count; stage = stage + 1u) {
        let words = params.stages[stage];
        regs[1] = params.constants[stage * 2u];
        regs[2] = params.constants[stage * 2u + 1u];
        let spare_alpha = regs[12].a;
        var mux_cd = (u32(spare_alpha * 255.0) & 1u) == 1u;
        if (mux_msb) { mux_cd = spare_alpha >= 0.5; }
        // colour portion. Output word: bits 3..0 CD destination, 7..4 AB destination, 11..8 sum or mux destination, 12 CD dot,
        // 13 AB dot, 14 mux, 15..17 mapping, 18 CD blue to alpha, 19 AB blue to alpha (xemu parse_combiner_output)
        let out = words.z;
        let a = combiner_input(&regs, words.x >> 24u, false);
        let b = combiner_input(&regs, (words.x >> 16u) & 0xFFu, false);
        let c = combiner_input(&regs, (words.x >> 8u) & 0xFFu, false);
        let d = combiner_input(&regs, words.x & 0xFFu, false);
        var ab = a * b;
        if ((out & 0x2000u) != 0u) { ab = vec3<f32>(dot(a, b)); }
        var cd = c * d;
        if ((out & 0x1000u) != 0u) { cd = vec3<f32>(dot(c, d)); }
        var sum = ab + cd;
        if ((out & 0x4000u) != 0u) {
            if (mux_cd) { sum = cd; } else { sum = ab; }
        }
        let ab_stored = clamp(combiner_output_map(ab, out), vec3<f32>(-1.0), vec3<f32>(1.0));
        let cd_stored = clamp(combiner_output_map(cd, out), vec3<f32>(-1.0), vec3<f32>(1.0));
        let sum_stored = clamp(combiner_output_map(sum, out), vec3<f32>(-1.0), vec3<f32>(1.0));
        combiner_write_rgb(&regs, (out >> 4u) & 0xFu, ab_stored);
        if ((out & 0x80000u) != 0u) { combiner_write_alpha(&regs, (out >> 4u) & 0xFu, ab_stored.z); }
        combiner_write_rgb(&regs, out & 0xFu, cd_stored);
        if ((out & 0x40000u) != 0u) { combiner_write_alpha(&regs, out & 0xFu, cd_stored.z); }
        combiner_write_rgb(&regs, (out >> 8u) & 0xFu, sum_stored);
        // alpha portion
        let alpha_out = words.w;
        let alpha_a = combiner_input(&regs, words.y >> 24u, true).x;
        let alpha_b = combiner_input(&regs, (words.y >> 16u) & 0xFFu, true).x;
        let alpha_c = combiner_input(&regs, (words.y >> 8u) & 0xFFu, true).x;
        let alpha_d = combiner_input(&regs, words.y & 0xFFu, true).x;
        let alpha_ab = alpha_a * alpha_b;
        let alpha_cd = alpha_c * alpha_d;
        var alpha_sum = alpha_ab + alpha_cd;
        if ((alpha_out & 0x4000u) != 0u) {
            if (mux_cd) { alpha_sum = alpha_cd; } else { alpha_sum = alpha_ab; }
        }
        let alpha_ab_stored = clamp(combiner_output_map(vec3<f32>(alpha_ab), alpha_out).x, -1.0, 1.0);
        let alpha_cd_stored = clamp(combiner_output_map(vec3<f32>(alpha_cd), alpha_out).x, -1.0, 1.0);
        let alpha_sum_stored = clamp(combiner_output_map(vec3<f32>(alpha_sum), alpha_out).x, -1.0, 1.0);
        combiner_write_alpha(&regs, (alpha_out >> 4u) & 0xFu, alpha_ab_stored);
        combiner_write_alpha(&regs, alpha_out & 0xFu, alpha_cd_stored);
        combiner_write_alpha(&regs, (alpha_out >> 8u) & 0xFu, alpha_sum_stored);
    }
    // final combiner: A*B + (1 - A)*C + D, alpha from G. Register 14 is V1 + R0 (flags in the EFG low byte), 15 is E times F.
    regs[1] = params.final_constant0;
    regs[2] = params.final_constant1;
    let efg = params.head.z;
    let final_flags = efg & 0xFFu;
    var left = regs[5].rgb;
    if ((final_flags & 0x40u) != 0u) { left = vec3<f32>(1.0) - left; }
    var right = regs[12].rgb;
    if ((final_flags & 0x20u) != 0u) { right = vec3<f32>(1.0) - right; }
    var v1r0 = left + right;
    if ((final_flags & 0x80u) != 0u) { v1r0 = clamp(v1r0, vec3<f32>(0.0), vec3<f32>(1.0)); }
    regs[14] = vec4<f32>(v1r0, 0.0);
    let e = combiner_input(&regs, efg >> 24u, false);
    let f = combiner_input(&regs, (efg >> 16u) & 0xFFu, false);
    regs[15] = vec4<f32>(e * f, 0.0);
    let abcd = params.head.y;
    let fa = combiner_input(&regs, abcd >> 24u, false);
    let fb = combiner_input(&regs, (abcd >> 16u) & 0xFFu, false);
    let fc = combiner_input(&regs, (abcd >> 8u) & 0xFFu, false);
    let fd = combiner_input(&regs, abcd & 0xFFu, false);
    let rgb = clamp(fa * fb + (vec3<f32>(1.0) - fa) * fc + fd, vec3<f32>(0.0), vec3<f32>(1.0));
    let alpha = clamp(combiner_input(&regs, (efg >> 8u) & 0xFFu, true).x, 0.0, 1.0);
    return vec4<f32>(rgb, alpha);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    var sampled = textureSample(stage0, stage0_sampler, in.uv * params.uv_scale01.xy);
    if ((params.pad0 & 1u) != 0u) { sampled.a = 1.0; }
    if ((params.pad0 & 2u) != 0u) { return vec4<f32>(sampled.rgb, 1.0); }
    var colour = in.colour;
    if (params.combiner != 0u) {
        var texels: array<vec4<f32>, 4>;
        texels[0] = sampled;
        let mask = params.head.w;
        if ((mask & 2u) != 0u) { texels[1] = textureSample(stage1, stage1_sampler, in.uv1 * params.uv_scale01.zw); }
        if ((mask & 4u) != 0u) { texels[2] = textureSample(stage2, stage2_sampler, in.uv2 * params.uv_scale23.xy); }
        if ((mask & 8u) != 0u) { texels[3] = textureSample(stage3, stage3_sampler, in.uv3 * params.uv_scale23.zw); }
        colour = run_combiner(in.colour, in.specular, texels, in.fog);
    } else if (params.textured != 0u) {
        colour = colour * sampled;
    }
    if (params.alpha_func != 7u && !alpha_passes(colour.a, params.alpha_ref, params.alpha_func)) {
        discard;
    }
    return colour;
}
