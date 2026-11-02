// SPDX-License-Identifier: GPL-3.0-or-later
//! NV2A vertex program to WGSL compute shader translator (T2024k, docs/t-wgpu-gpu-vertex-programs.md).
//!
//! One program (the slots from a start slot to the FINAL instruction) becomes one compute shader that shades a run of vertices
//! on the GPU and writes the 84 byte [`crate::vertex::Vertex`] records the render pipelines already consume. The arithmetic is
//! `vsh::run` in float32 with the same operation order: operand reads flush denormals, `0 * x = 0`, the sum of a dot product is
//! left to right, MAD is a separate multiply and add, SLT and SGE use the total order with -0 below +0, ARL is the floor of the
//! A operand's x, a paired MAC and ILU instruction writes the ILU result to r1, r12 mirrors `oPos`. Differences from the CPU
//! interpreter are only in the transcendental ILU ops (RSQ, EXP, LOG, LIT's power, the fog exponent): the interpreter computes
//! those in double and rounds once, the GPU uses float32 builtins (see the document for the measured size).
//!
//! The epilogue is `Replayer::shade_vertices` for the default knobs: `vsh::to_clip_with` with the depth clamp, the diffuse colour
//! clamp, the texture coordinate normalisation of linear textures and the fog amount. A vertex with no valid clip position is
//! written with `w = 0`, which the index fix kernel ([`INDEX_FIX_WGSL`]) turns into a dropped (degenerate) triangle.
use std::collections::BTreeSet;
use std::fmt::Write;

use crate::vsh::{self, CONSTANT_COUNT, Instruction, PROGRAM_SLOTS, ProgramMemory, Source};

/// Threads per workgroup of both kernels.
pub const WORKGROUP: u32 = 64;
/// Floats per output vertex record (84 bytes).
pub const VERTEX_FLOATS: u32 = 21;

/// A program translated to WGSL.
#[derive(Clone, Debug)]
pub struct Translation {
    pub wgsl: String,
    /// The input registers the program reads, ascending: input `inputs[k]` is slot `k` of the per vertex input record.
    pub inputs: Vec<usize>,
    /// True when an instruction writes the constant file (the shader then works on a private copy).
    pub writes_constants: bool,
}

/// The instructions of the program at `start` up to and including the FINAL one, and the slot after the last executed one.
pub fn extent(memory: &ProgramMemory, start: usize) -> Vec<Instruction> {
    let mut program = Vec::new();
    for slot in start..PROGRAM_SLOTS {
        let Some(inst) = vsh::decode(memory.slots[slot]) else {
            break;
        };
        program.push(inst);
        if inst.last {
            break;
        }
    }
    program
}

/// A 64 bit FNV-1a key of the program's microcode (every dword of every slot of [`extent`]).
pub fn program_key(memory: &ProgramMemory, start: usize) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |word: u32| {
        for byte in word.to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut slot = start;
    while slot < PROGRAM_SLOTS {
        let words = memory.slots[slot];
        for word in words {
            mix(word);
        }
        let Some(inst) = vsh::decode(words) else {
            break;
        };
        if inst.last {
            break;
        }
        slot += 1;
    }
    hash
}

fn swizzle_letters(swizzle: u32) -> String {
    (0..4)
        .map(|index| ['x', 'y', 'z', 'w'][((swizzle >> (6 - 2 * index)) & 3) as usize])
        .collect()
}

/// WGSL expression of a source operand (after flush, swizzle and negate).
fn read(source: Source, inst: &Instruction, inputs: &[usize], writes_constants: bool) -> String {
    let zero = || {
        if source.negate {
            "bitcast<vec4<f32>>(vec4<u32>(0x80000000u))".to_string()
        } else {
            "vec4<f32>(0.0)".to_string()
        }
    };
    let raw = match source.mux {
        1 => {
            if source.register == 12 {
                "o0".to_string()
            } else {
                format!("r{}", source.register)
            }
        }
        2 => {
            let slot = inputs
                .iter()
                .position(|&r| r == inst.input)
                .expect("collected");
            format!("in{slot}")
        }
        3 => {
            let table = if writes_constants { "lc" } else { "cf" };
            if inst.indexed {
                format!("{table}_rel(a0 + {})", inst.constant)
            } else if inst.constant < CONSTANT_COUNT {
                format!("{table}_at({}u)", inst.constant)
            } else {
                return zero();
            }
        }
        _ => return zero(),
    };
    let function = if source.negate { "fln" } else { "fl" };
    format!("{function}({raw}).{}", swizzle_letters(source.swizzle))
}

fn mac_expr(op: u32) -> Option<&'static str> {
    Some(match op {
        1 => "a",
        2 => "mulz(a, b)",
        3 => "a + c",
        4 => "mulz(a, b) + c",
        5 => "vec4<f32>(dot3z(a, b))",
        6 => "vec4<f32>(dot3z(a, b) + b.w)",
        7 => "vec4<f32>(dot4z(a, b))",
        8 => "vec4<f32>(1.0, mz(a.y, b.y), a.z, b.w)",
        9 => "select(b, a, a < b)",
        10 => "select(b, a, a >= b)",
        11 => "select(vec4<f32>(0.0), vec4<f32>(1.0), tkey(a) < tkey(b))",
        12 => "select(vec4<f32>(0.0), vec4<f32>(1.0), tkey(a) >= tkey(b))",
        _ => return None,
    })
}

fn ilu_expr(op: u32) -> Option<&'static str> {
    Some(match op {
        1 => "c",
        2 => "vec4<f32>(rcp(c.x))",
        3 => "vec4<f32>(rcc(c.x))",
        4 => "vec4<f32>(rsq(c.x))",
        5 => "exp_unit(c.x)",
        6 => "log_unit(c.x)",
        7 => "lit(c)",
        _ => return None,
    })
}

/// `register.xyzw = value.xyzw;` for the components of `mask` (bit 3 is x).
fn masked_write(out: &mut String, target: &str, value: &str, mask: u32) {
    for (component, letter) in ['x', 'y', 'z', 'w'].into_iter().enumerate() {
        if (mask >> (3 - component)) & 1 != 0 {
            let _ = writeln!(out, "    {target}.{letter} = {value}.{letter};");
        }
    }
}

fn temp_name(register: usize) -> String {
    if register == 12 {
        "o0".to_string()
    } else {
        format!("r{register}")
    }
}

/// Translate the program of `memory` at `start` into the vertex shading kernel.
pub fn translate(memory: &ProgramMemory, start: usize) -> Translation {
    translate_with(memory, start, false)
}

/// [`translate`], or with `raw` a kernel whose epilogue stores the 16 output registers (64 floats per vertex, the test harness).
pub fn translate_with(memory: &ProgramMemory, start: usize, raw: bool) -> Translation {
    let program = extent(memory, start);
    let writes_constants = program.iter().any(|inst| {
        let routed = if inst.out_is_ilu {
            inst.ilu_defined()
        } else {
            inst.mac_defined()
        };
        routed && inst.out_mask != 0 && !inst.out_to_output && inst.out_address < CONSTANT_COUNT
    });
    let mut inputs = BTreeSet::new();
    for inst in &program {
        for source in [inst.a, inst.b, inst.c] {
            if source.mux == 2 {
                inputs.insert(inst.input);
            }
        }
    }
    let inputs: Vec<usize> = inputs.into_iter().collect();
    let mut body = String::new();
    for (slot, inst) in program.iter().enumerate() {
        let _ = writeln!(body, "    // slot {slot}");
        let _ = writeln!(body, "    {{");
        for (name, source) in [("a", inst.a), ("b", inst.b), ("c", inst.c)] {
            let _ = writeln!(
                body,
                "    let {name} = {};",
                read(source, inst, &inputs, writes_constants)
            );
        }
        let mac = mac_expr(inst.mac);
        let ilu = ilu_expr(inst.ilu);
        if let Some(expr) = mac {
            let _ = writeln!(body, "    let mac = {expr};");
        }
        if let Some(expr) = ilu {
            let _ = writeln!(body, "    let ilu = {expr};");
        }
        let paired = inst.mac != 0 && inst.ilu != 0;
        if mac.is_some() && !(paired && inst.temp == 1) {
            masked_write(&mut body, &temp_name(inst.temp), "mac", inst.mac_mask);
        }
        if ilu.is_some() {
            let destination = if paired { 1 } else { inst.temp };
            masked_write(&mut body, &temp_name(destination), "ilu", inst.ilu_mask);
        }
        let routed = if inst.out_is_ilu {
            ilu.map(|_| "ilu")
        } else {
            mac.map(|_| "mac")
        };
        if let Some(value) = routed.filter(|_| inst.out_mask != 0) {
            if inst.out_to_output && inst.out_address < vsh::OUTPUT_COUNT {
                masked_write(
                    &mut body,
                    &format!("o{}", inst.out_address),
                    value,
                    inst.out_mask,
                );
            } else if !inst.out_to_output && inst.out_address < CONSTANT_COUNT {
                masked_write(
                    &mut body,
                    &format!("lc[{}]", inst.out_address),
                    value,
                    inst.out_mask,
                );
            }
        }
        if inst.mac == 13 {
            let _ = writeln!(body, "    a0 = i32(floor(select(a.x, 0.0, isnan_f(a.x))));");
        }
        let _ = writeln!(body, "    }}");
    }
    let mut wgsl = String::with_capacity(body.len() + 8192);
    wgsl.push_str(PRELUDE);
    let count = inputs.len().max(1);
    let _ = writeln!(wgsl, "const INPUT_COUNT: u32 = {}u;", inputs.len());
    let _ = writeln!(wgsl, "const INPUT_STRIDE: u32 = {count}u;");
    wgsl.push_str(
        "@compute @workgroup_size(64)\nfn main(@builtin(global_invocation_id) gid: vec3<u32>) {\n",
    );
    wgsl.push_str("    let v = gid.x;\n    if (v >= job.vertex_count) { return; }\n");
    for (slot, register) in inputs.iter().enumerate() {
        let _ = writeln!(
            wgsl,
            "    let in{slot} = inputs[job.input_base + v * INPUT_STRIDE + {slot}u]; // v{register}"
        );
    }
    if writes_constants {
        wgsl.push_str("    for (var i = 0u; i < 192u; i = i + 1u) { lc[i] = constants[job.constants_base + i]; }\n");
    }
    for register in 0..16 {
        if register != 12 {
            let _ = writeln!(wgsl, "    var r{register} = vec4<f32>(0.0);");
        }
    }
    for register in 0..16 {
        let _ = writeln!(wgsl, "    var o{register} = vec4<f32>(0.0, 0.0, 0.0, 1.0);");
    }
    wgsl.push_str("    var a0: i32 = 0;\n");
    wgsl.push_str(&body);
    if raw {
        wgsl.push_str("    let base = (job.out_base + v) * 64u;\n");
        for register in 0..16 {
            for (component, letter) in ['x', 'y', 'z', 'w'].into_iter().enumerate() {
                let _ = writeln!(
                    wgsl,
                    "    vertices[base + {}u] = o{register}.{letter};",
                    register * 4 + component
                );
            }
        }
        wgsl.push_str("}\n");
    } else {
        wgsl.push_str(EPILOGUE);
    }
    Translation {
        wgsl,
        inputs,
        writes_constants,
    }
}

trait Defined {
    fn mac_defined(&self) -> bool;
    fn ilu_defined(&self) -> bool;
}

impl Defined for Instruction {
    fn mac_defined(&self) -> bool {
        mac_expr(self.mac).is_some()
    }
    fn ilu_defined(&self) -> bool {
        ilu_expr(self.ilu).is_some()
    }
}

/// Per job parameters (uniform, one 256 byte aligned slot per job, dynamic offset). Mirrors `struct Job` of [`PRELUDE`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Job {
    pub vertex_count: u32,
    /// Offsets in vec4 units into the inputs and constants buffers, and the vertex index of the first output record.
    pub input_base: u32,
    pub constants_base: u32,
    pub out_base: u32,
    pub screen: [f32; 2],
    /// Fog mode (0 to 5) or `u32::MAX` for no fog, and the two parameters.
    pub fog_mode: u32,
    pub fog_parameters: [f32; 2],
    /// Width and height of the linear texture of stages 0 to 3 (0 when the stage is not normalised).
    pub sizes: [[f32; 2]; 4],
}

impl Job {
    /// Uniform slot size, the dynamic offset alignment of every adapter that matters (limits can be 256 at most).
    pub const SLOT: u64 = 256;

    pub fn bytes(&self) -> [u8; 96] {
        let mut out = [0u8; 96];
        let mut put = |index: usize, word: u32| {
            out[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        };
        put(0, self.vertex_count);
        put(1, self.input_base);
        put(2, self.constants_base);
        put(3, self.out_base);
        put(4, self.screen[0].to_bits());
        put(5, self.screen[1].to_bits());
        put(6, self.fog_mode);
        put(7, 0);
        put(8, self.fog_parameters[0].to_bits());
        put(9, self.fog_parameters[1].to_bits());
        put(10, 0);
        put(11, 0);
        for (stage, size) in self.sizes.iter().enumerate() {
            put(12 + stage * 2, size[0].to_bits());
            put(13 + stage * 2, size[1].to_bits());
        }
        out
    }
}

/// Bindings (group 0): 0 inputs (read), 1 constants (read), 2 vertices as floats (read_write), 3 job (uniform, dynamic offset),
/// 4 indices (read_write, used by the index fix kernel only).
const PRELUDE: &str = r"
struct Job {
    vertex_count: u32,
    input_base: u32,
    constants_base: u32,
    out_base: u32,
    screen: vec2<f32>,
    fog_mode: u32,
    pad0: u32,
    fog_parameters: vec2<f32>,
    pad1: vec2<f32>,
    sizes: array<vec4<f32>, 2>, // stage 0 and 1 in the first vec4, stage 2 and 3 in the second
};
@group(0) @binding(0) var<storage, read> inputs: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> constants: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> vertices: array<f32>;
@group(0) @binding(3) var<uniform> job: Job;
var<private> lc: array<vec4<f32>, 192>;

const MIN_NORMAL: f32 = 1.1754944e-38;
const INF_BITS: u32 = 0x7f800000u;

fn isnan_f(x: f32) -> bool { return (bitcast<u32>(x) & 0x7fffffffu) > INF_BITS; }
fn isfinite_f(x: f32) -> bool { return (bitcast<u32>(x) & 0x7fffffffu) < INF_BITS; }
fn copysign_f(magnitude: f32, sign: f32) -> f32 {
    return bitcast<f32>((bitcast<u32>(magnitude) & 0x7fffffffu) | (bitcast<u32>(sign) & 0x80000000u));
}
fn pos_inf() -> f32 { return bitcast<f32>(INF_BITS); }
fn neg_inf() -> f32 { return bitcast<f32>(0xff800000u); }

// operand flush: denormals become a signed zero (integer ops, so a driver that flushes float denormals cannot change the sign)
fn fl(v: vec4<f32>) -> vec4<f32> {
    let bits = bitcast<vec4<u32>>(v);
    let denormal = ((bits & vec4<u32>(0x7f800000u)) == vec4<u32>(0u)) & ((bits & vec4<u32>(0x007fffffu)) != vec4<u32>(0u));
    return bitcast<vec4<f32>>(select(bits, bits & vec4<u32>(0x80000000u), denormal));
}
// flush then negate by flipping the sign bit (a float negation may not keep the sign of a NaN on every driver)
fn fln(v: vec4<f32>) -> vec4<f32> {
    return bitcast<vec4<f32>>(bitcast<vec4<u32>>(fl(v)) ^ vec4<u32>(0x80000000u));
}
fn cf_at(i: u32) -> vec4<f32> { return constants[job.constants_base + i]; }
fn cf_rel(i: i32) -> vec4<f32> {
    if (i >= 0 && i < 192) { return constants[job.constants_base + u32(i)]; }
    return vec4<f32>(0.0);
}
fn lc_at(i: u32) -> vec4<f32> { return lc[i]; }
fn lc_rel(i: i32) -> vec4<f32> {
    if (i >= 0 && i < 192) { return lc[u32(i)]; }
    return vec4<f32>(0.0);
}
fn mz(l: f32, r: f32) -> f32 { return select(l * r, 0.0, l == 0.0 || r == 0.0); }
fn mulz(l: vec4<f32>, r: vec4<f32>) -> vec4<f32> {
    return select(l * r, vec4<f32>(0.0), (l == vec4<f32>(0.0)) | (r == vec4<f32>(0.0)));
}
fn dot3z(l: vec4<f32>, r: vec4<f32>) -> f32 { return (mz(l.x, r.x) + mz(l.y, r.y)) + mz(l.z, r.z); }
fn dot4z(l: vec4<f32>, r: vec4<f32>) -> f32 { return ((mz(l.x, r.x) + mz(l.y, r.y)) + mz(l.z, r.z)) + mz(l.w, r.w); }
// total order key of the GL_NV_vertex_program compares: -0 below +0, -NaN lowest, +NaN highest
fn tkey(x: vec4<f32>) -> vec4<i32> {
    let b = bitcast<vec4<i32>>(x);
    return b ^ ((b >> vec4<u32>(31u)) & vec4<i32>(0x7fffffff));
}
fn rcp(v: f32) -> f32 {
    if (v == 0.0) { return copysign_f(pos_inf(), v); }
    return 1.0 / v;
}
fn rcc(v: f32) -> f32 {
    let r = rcp(v);
    if (isnan_f(r)) { return r; }
    return copysign_f(clamp(abs(r), 5.421011e-20, 1.8446744e19), r);
}
fn rsq(v: f32) -> f32 {
    if (isnan_f(v)) { return v; }
    let m = abs(v);
    if (m == 0.0) { return pos_inf(); }
    if (!isfinite_f(m)) { return 0.0; }
    return 1.0 / sqrt(m);
}
// 2^x as the interpreter's exp_double third component, edge cases included
fn exp3(x: f32) -> f32 {
    if (isnan_f(x)) { return x; }
    if (x == neg_inf()) { return 0.0; }
    if (x == pos_inf()) { return x; }
    let f = floor(x);
    if (f >= 128.0) { return pos_inf(); }
    if (f < -126.0) { return 0.0; }
    return exp2(x);
}
fn exp_unit(x: f32) -> vec4<f32> {
    if (isnan_f(x)) { return vec4<f32>(x, x, x, 1.0); }
    if (x == neg_inf()) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    if (x == pos_inf()) { return vec4<f32>(x, 0.0, x, 1.0); }
    var f = floor(x);
    f = select(f, 0.0, f == 0.0);
    if (f >= 128.0) { return vec4<f32>(pos_inf(), 0.0, pos_inf(), 1.0); }
    if (f < -126.0) { return vec4<f32>(0.0, 0.0, 0.0, 1.0); }
    let integer_part = bitcast<f32>(u32(i32(f) + 127) << 23u);
    return vec4<f32>(integer_part, x - f, exp2(x), 1.0);
}
fn log_unit(x: f32) -> vec4<f32> {
    if (isnan_f(x)) { return vec4<f32>(x, x, x, 1.0); }
    let m = abs(x);
    if (m == 0.0) { return vec4<f32>(neg_inf(), 1.0, neg_inf(), 1.0); }
    if (!isfinite_f(m)) { return vec4<f32>(pos_inf(), 1.0, pos_inf(), 1.0); }
    let bits = bitcast<u32>(m);
    let e = f32(i32(bits >> 23u) - 127);
    let mant = bitcast<f32>((bits & 0x007fffffu) | 0x3f800000u);
    return vec4<f32>(e, mant, e + log2(mant), 1.0);
}
fn lit_power(specular: f32, power: f32) -> f32 {
    if (isnan_f(power) || isnan_f(specular)) { return bitcast<f32>(0x7fc00000u); }
    if (power == 0.0) { return 1.0; }
    if (specular == 0.0) { if (power > 0.0) { return 0.0; } return pos_inf(); }
    if (!isfinite_f(specular)) { if (power > 0.0) { return pos_inf(); } return 0.0; }
    return exp3(power * log2(specular));
}
fn lit(c: vec4<f32>) -> vec4<f32> {
    var power = c.w;
    if (c.w < -127.99609) { power = -127.99609; } else if (c.w > 127.99609) { power = 127.99609; }
    var diffuse = c.x;
    if (c.x < 0.0) { diffuse = 0.0; }
    var specular = c.y;
    if (c.y < 0.0) { specular = 0.0; }
    var term = 0.0;
    if (diffuse > 0.0) { term = lit_power(specular, power); }
    return vec4<f32>(1.0, diffuse, term, 1.0);
}
fn fog_amount(distance: f32) -> f32 {
    let mode = job.fog_mode;
    let p0 = job.fog_parameters.x;
    let p1 = job.fog_parameters.y;
    var value: f32;
    if (mode == 0u || mode == 3u) {
        value = (p0 + distance * p1) - 1.0;
    } else if (mode == 1u || mode == 4u) {
        value = (p0 + exp2(distance * p1 * 16.0)) - 1.5;
    } else {
        value = (p0 + exp2(-distance * distance * p1 * p1 * 32.0)) - 1.5;
    }
    if (mode >= 3u) { value = abs(value); }
    if (isnan_f(value)) {
        if (mode <= 1u || mode == 3u) { value = 1.0; } else { value = 0.0; }
    }
    return clamp(value, 0.0, 1.0);
}
fn normalise(c: vec4<f32>, size: vec2<f32>) -> vec2<f32> {
    if (size.x != 0.0 && size.y != 0.0) { return vec2<f32>(c.x / size.x, c.y / size.y); }
    return c.xy;
}
";

/// After the program: clip position (`vsh::to_clip_with`, depth clamp on), colour, texture coordinates and fog into the 84 byte
/// record. An invalid position is written as `w = 0` (every valid one has `|w| > 1e-6`).
const EPILOGUE: &str = r"
    let p = o0;
    let ok = abs(p.w) > 1e-6 && isfinite_f(p.x) && isfinite_f(p.y) && isfinite_f(p.z) && isfinite_f(p.w);
    let scale = constants[job.constants_base + 58u].z;
    let offset = constants[job.constants_base + 59u].z;
    var depth = p.z;
    if (scale != 0.0 && isfinite_f(scale)) { depth = (p.z - offset) / scale; }
    depth = clamp(depth, 0.0, 1.0);
    let ndc_x = p.x / job.screen.x * 2.0 - 1.0;
    let ndc_y = 1.0 - p.y / job.screen.y * 2.0;
    var clip = vec4<f32>(0.0);
    if (ok) { clip = vec4<f32>(ndc_x * p.w, ndc_y * p.w, depth * p.w, p.w); }
    let colour = clamp(o3, vec4<f32>(0.0), vec4<f32>(1.0));
    let uv0 = normalise(o9, job.sizes[0].xy);
    let uv1 = normalise(o10, job.sizes[0].zw);
    let uv2 = normalise(o11, job.sizes[1].xy);
    let uv3 = normalise(o12, job.sizes[1].zw);
    var fog = 0.0;
    if (job.fog_mode != 0xffffffffu) { fog = fog_amount(o5.x); }
    let base = (job.out_base + v) * 21u;
    vertices[base + 0u] = clip.x;
    vertices[base + 1u] = clip.y;
    vertices[base + 2u] = clip.z;
    vertices[base + 3u] = clip.w;
    vertices[base + 4u] = colour.x;
    vertices[base + 5u] = colour.y;
    vertices[base + 6u] = colour.z;
    vertices[base + 7u] = colour.w;
    vertices[base + 8u] = uv0.x;
    vertices[base + 9u] = uv0.y;
    vertices[base + 10u] = uv1.x;
    vertices[base + 11u] = uv1.y;
    vertices[base + 12u] = uv2.x;
    vertices[base + 13u] = uv2.y;
    vertices[base + 14u] = uv3.x;
    vertices[base + 15u] = uv3.y;
    vertices[base + 16u] = fog;
    let specular = clamp(o4, vec4<f32>(0.0), vec4<f32>(1.0));
    vertices[base + 17u] = specular.x;
    vertices[base + 18u] = specular.y;
    vertices[base + 19u] = specular.z;
    vertices[base + 20u] = specular.w;
}
";

/// The index fix kernel: one thread per primitive of a job (`vertex_count` is the primitive count, `input_base` the first index,
/// `constants_base` the indices per primitive, 2 or 3). A primitive with a vertex whose record has `w = 0` is made degenerate.
pub const INDEX_FIX_WGSL: &str = r"
struct Job {
    vertex_count: u32,
    input_base: u32,
    constants_base: u32,
    out_base: u32,
};
@group(0) @binding(0) var<storage, read> unused0: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> unused1: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> vertices: array<f32>;
@group(0) @binding(3) var<uniform> job: Job;
@group(0) @binding(4) var<storage, read_write> indices: array<u32>;
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let primitive = gid.x;
    if (primitive >= job.vertex_count) { return; }
    let first = job.input_base + primitive * job.constants_base;
    var bad = false;
    for (var k = 0u; k < job.constants_base; k = k + 1u) {
        if (vertices[indices[first + k] * 21u + 3u] == 0.0) { bad = true; }
    }
    if (bad) {
        let keep = indices[first];
        for (var k = 1u; k < job.constants_base; k = k + 1u) { indices[first + k] = keep; }
    }
}
";
