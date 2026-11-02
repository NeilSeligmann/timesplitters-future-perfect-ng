// SPDX-License-Identifier: GPL-3.0-or-later
//! NV2A (Kelvin) vertex program interpreter, run on the CPU per vertex (T2024 slice 3).
//!
//! The instruction layout, opcode numbering and the semantic choices are those of `tools/nv2a/isa.py` and
//! `tools/nv2a/interp.py` (docs/vertex-translator.md), which this module ports to Rust. Instruction word: 4 dwords, dword 0
//! unused, the 92 bit envytools word is dwords 1..3 with envytools bit n in dword `3 - n / 32`. Choices (UNSETTLED on real
//! hardware, shared with the Python oracle): `0 * x = 0` and denormal inputs flushed (the GL_NV_vertex_program rules), MAD not
//! fused, ADD reads operands A and C, r12 mirrors `oPos`, a scalar op reads the swizzled C operand's x, paired MAC and ILU
//! instructions write the ILU result to r1, ARL is the floor of the A operand's x.
//!
//! Window space: the title's programs divide by w and apply the viewport themselves (`oPos` is in window pixels, w kept,
//! docs/vertex-translator.md section 10.2), see [`to_clip`].

pub const PROGRAM_SLOTS: usize = 136;
pub const CONSTANT_COUNT: usize = 192;
pub const INPUT_COUNT: usize = 16;
pub const OUTPUT_COUNT: usize = 16;
const TEMP_COUNT: usize = 16;
/// Temporary 12 mirrors the position output (MEASURED consistency in the title's programs).
const POSITION_MIRROR: usize = 12;
const MIN_NORMAL: f32 = 1.175_494_4e-38;

/// A four component register.
pub type Vec4 = [f32; 4];

/// A source operand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    /// 0 none (reads zero), 1 temporary, 2 input, 3 constant.
    pub mux: u32,
    pub register: usize,
    /// `swizzle >> (6 - 2 * i) & 3` is the source component feeding component i.
    pub swizzle: u32,
    pub negate: bool,
}

/// A decoded instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub last: bool,
    pub indexed: bool,
    pub out_is_ilu: bool,
    pub out_address: usize,
    /// 1 output register file, 0 constant file.
    pub out_to_output: bool,
    pub out_mask: u32,
    pub ilu_mask: u32,
    pub temp: usize,
    pub mac_mask: u32,
    pub a: Source,
    pub b: Source,
    pub c: Source,
    pub input: usize,
    pub constant: usize,
    pub mac: u32,
    pub ilu: u32,
}

fn source(bits: u128) -> Source {
    Source {
        mux: (bits & 3) as u32,
        register: ((bits >> 2) & 0xF) as usize,
        swizzle: ((bits >> 6) & 0xFF) as u32,
        negate: (bits >> 14) & 1 != 0,
    }
}

/// Decode four little endian dwords. `None` for an undefined MAC opcode.
pub fn decode(words: [u32; 4]) -> Option<Instruction> {
    let wide = u128::from(words[3]) | (u128::from(words[2]) << 32) | (u128::from(words[1]) << 64);
    let field = |low: u32, high: u32| (wide >> low) & ((1u128 << (high - low + 1)) - 1);
    let mac = field(85, 88) as u32;
    if mac > 13 {
        return None;
    }
    Some(Instruction {
        last: field(0, 0) != 0,
        indexed: field(1, 1) != 0,
        out_is_ilu: field(2, 2) != 0,
        out_address: field(3, 10) as usize,
        out_to_output: field(11, 11) != 0,
        out_mask: field(12, 15) as u32,
        ilu_mask: field(16, 19) as u32,
        temp: field(20, 23) as usize,
        mac_mask: field(24, 27) as u32,
        c: source(field(28, 42)),
        b: source(field(43, 57)),
        a: source(field(58, 72)),
        input: field(73, 76) as usize,
        constant: field(77, 84) as usize,
        mac,
        ilu: field(89, 91) as u32,
    })
}

/// Program memory (136 slots of 4 dwords) as the title loads it.
#[derive(Clone)]
pub struct ProgramMemory {
    pub slots: [[u32; 4]; PROGRAM_SLOTS],
}

impl Default for ProgramMemory {
    fn default() -> Self {
        Self {
            slots: [[0; 4]; PROGRAM_SLOTS],
        }
    }
}

/// The vertex result registers of one run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outputs {
    pub registers: [Vec4; OUTPUT_COUNT],
}

impl Outputs {
    /// `oPos`.
    pub fn position(&self) -> Vec4 {
        self.registers[0]
    }
    /// `oD0`.
    pub fn diffuse(&self) -> Vec4 {
        self.registers[3]
    }
    /// `oD1`.
    pub fn specular(&self) -> Vec4 {
        self.registers[4]
    }
    /// `oT0..oT3`.
    pub fn texcoord(&self, index: usize) -> Vec4 {
        self.registers[9 + index]
    }
}

fn flush(value: f32) -> f32 {
    if value != 0.0 && value.abs() < MIN_NORMAL {
        0.0f32.copysign(value)
    } else {
        value
    }
}

fn mul(left: f32, right: f32) -> f32 {
    if left == 0.0 || right == 0.0 {
        0.0
    } else {
        left * right
    }
}

fn dot(left: Vec4, right: Vec4, count: usize) -> f32 {
    let mut total = mul(left[0], right[0]);
    for index in 1..count {
        total += mul(left[index], right[index]);
    }
    total
}

/// Total order of the GL_NV_vertex_program compares: -0 below +0.
fn order_key(value: f32) -> (u8, f32, u8) {
    if value.is_nan() {
        (if value.is_sign_negative() { 0 } else { 2 }, 0.0, 0)
    } else {
        (1, value, u8::from(!value.is_sign_negative()))
    }
}

fn less(left: f32, right: f32) -> bool {
    order_key(left) < order_key(right)
}

fn greater_equal(left: f32, right: f32) -> bool {
    order_key(left) >= order_key(right)
}

fn mac_result(op: u32, a: Vec4, b: Vec4, c: Vec4) -> Option<Vec4> {
    let each = |f: &dyn Fn(usize) -> f32| [f(0), f(1), f(2), f(3)];
    Some(match op {
        1 => a,
        2 => each(&|i| mul(a[i], b[i])),
        3 => each(&|i| a[i] + c[i]),
        4 => each(&|i| mul(a[i], b[i]) + c[i]),
        5 => [dot(a, b, 3); 4],
        6 => [dot(a, b, 3) + b[3]; 4],
        7 => [dot(a, b, 4); 4],
        8 => [1.0, mul(a[1], b[1]), a[2], b[3]],
        9 => each(&|i| if a[i] < b[i] { a[i] } else { b[i] }),
        10 => each(&|i| if a[i] >= b[i] { a[i] } else { b[i] }),
        11 => each(&|i| if less(a[i], b[i]) { 1.0 } else { 0.0 }),
        12 => each(&|i| if greater_equal(a[i], b[i]) { 1.0 } else { 0.0 }),
        _ => return None,
    })
}

fn reciprocal(value: f32) -> f32 {
    if value == 0.0 {
        f32::INFINITY.copysign(value)
    } else if value.is_nan() {
        f32::NAN
    } else {
        1.0 / value
    }
}

fn rcc(value: f32) -> f32 {
    let result = reciprocal(value);
    if result.is_nan() {
        return result;
    }
    result
        .abs()
        .clamp(5.421_011e-20, 1.844_674_4e19)
        .copysign(result)
}

fn rsq(value: f32) -> f32 {
    if value.is_nan() {
        return f32::NAN;
    }
    let magnitude = value.abs();
    if magnitude == 0.0 {
        f32::INFINITY
    } else if magnitude.is_infinite() {
        0.0
    } else {
        // One rounding of the double result, like the Python oracle (a float32 sqrt then divide rounds twice).
        (1.0 / f64::from(magnitude).sqrt()) as f32
    }
}

/// EXP of a double argument: `(2^floor, fraction, 2^x, 1)`, computed in double and rounded to float32 once (the oracle).
fn exp_double(value: f64) -> Vec4 {
    if value.is_nan() {
        return [f32::NAN, f32::NAN, f32::NAN, 1.0];
    }
    if value == f64::NEG_INFINITY {
        return [0.0, 0.0, 0.0, 1.0];
    }
    if value == f64::INFINITY {
        return [f32::INFINITY, 0.0, f32::INFINITY, 1.0];
    }
    // `+ 0.0` turns floor(-0) = -0 into +0 so `-0 - floor` stays -0: the oracle's Python `math.floor` returns an int (UNSETTLED
    // on hardware, replicated so the differential is exact).
    let floor = value.floor() + 0.0;
    let integer_part = 2.0f64.powi(floor.clamp(-300.0, 300.0) as i32) as f32;
    if integer_part.is_infinite() {
        return [f32::INFINITY, 0.0, f32::INFINITY, 1.0];
    }
    if integer_part < MIN_NORMAL {
        return [0.0, 0.0, 0.0, 1.0];
    }
    [
        integer_part,
        (value - floor) as f32,
        2.0f64.powf(value) as f32,
        1.0,
    ]
}

fn exp_unit(value: f32) -> Vec4 {
    exp_double(f64::from(value))
}

fn log_unit(value: f32) -> Vec4 {
    if value.is_nan() {
        return [f32::NAN, f32::NAN, f32::NAN, 1.0];
    }
    let magnitude = value.abs();
    if magnitude == 0.0 {
        return [f32::NEG_INFINITY, 1.0, f32::NEG_INFINITY, 1.0];
    }
    if magnitude.is_infinite() {
        return [f32::INFINITY, 1.0, f32::INFINITY, 1.0];
    }
    // Inputs are already flushed, so the value is a normal float32: exponent field and a mantissa in [1, 2).
    let bits = magnitude.to_bits();
    let exponent = f64::from((bits >> 23) as i32 - 127);
    let mantissa = f64::from(f32::from_bits((bits & 0x007F_FFFF) | 0x3F80_0000));
    [
        exponent as f32,
        mantissa as f32,
        (exponent + mantissa.log2()) as f32,
        1.0,
    ]
}

/// `specular ^ power` as the oracle: exact in double through EXP, not a float32 `powf`.
fn lit_power(specular: f32, power: f32) -> f32 {
    if power.is_nan() || specular.is_nan() {
        f32::NAN
    } else if power == 0.0 {
        1.0
    } else if specular == 0.0 {
        if power > 0.0 { 0.0 } else { f32::INFINITY }
    } else if specular.is_infinite() {
        if power > 0.0 { f32::INFINITY } else { 0.0 }
    } else {
        exp_double(f64::from(power) * f64::from(specular).log2())[2]
    }
}

fn lit(c: Vec4) -> Vec4 {
    // Clamp preserves NaN like the oracle (`f32::max` would turn it into 0).
    let power = c[3].clamp(-127.996_09, 127.996_09);
    let diffuse = if c[0] < 0.0 { 0.0 } else { c[0] };
    let specular = if c[1] < 0.0 { 0.0 } else { c[1] };
    let term = if diffuse > 0.0 {
        lit_power(specular, power)
    } else {
        0.0
    };
    [1.0, diffuse, term, 1.0]
}

fn ilu_result(op: u32, c: Vec4) -> Option<Vec4> {
    let scalar = c[0];
    Some(match op {
        1 => c,
        2 => [reciprocal(scalar); 4],
        3 => [rcc(scalar); 4],
        4 => [rsq(scalar); 4],
        5 => exp_unit(scalar),
        6 => log_unit(scalar),
        7 => lit(c),
        _ => return None,
    })
}

fn write_masked(target: &mut Vec4, value: Vec4, mask: u32) {
    for (component, slot) in target.iter_mut().enumerate() {
        if (mask >> (3 - component)) & 1 != 0 {
            *slot = value[component];
        }
    }
}

fn floor_to_int(value: f32) -> i32 {
    if value.is_nan() {
        0
    } else {
        value.floor() as i32
    }
}

/// Output registers start at `(0, 0, 0, 1)` (the NV2A documentation reading, INFERRED, so an `oD0` the program writes as xyz
/// only keeps alpha 1).
const OUTPUT_INIT: Vec4 = [0.0, 0.0, 0.0, 1.0];

/// Run the program that starts at slot `start` until a FINAL instruction (or the end of program memory) on one vertex.
/// `constants` is the hardware constant file and is not modified (constant writes by the program stay local).
pub fn run(
    memory: &ProgramMemory,
    start: usize,
    inputs: &[Vec4; INPUT_COUNT],
    constants: &[Vec4; CONSTANT_COUNT],
) -> Outputs {
    run_inner(memory, start, inputs, constants, None)
}

/// `run` that also appends one line per executed instruction to `trace` (slot, address register, the three operands read, the
/// MAC and ILU results). Debug aid (slice 10, the sprite corner investigation), same arithmetic as `run`.
pub fn run_traced(
    memory: &ProgramMemory,
    start: usize,
    inputs: &[Vec4; INPUT_COUNT],
    constants: &[Vec4; CONSTANT_COUNT],
    trace: &mut Vec<String>,
) -> Outputs {
    run_inner(memory, start, inputs, constants, Some(trace))
}

/// The instructions a run from `start` executes, decoded once (T2136f: a draw runs the same program for every vertex, decoding
/// 128 bit words per vertex was a large part of the vertex program time). Stops at the instruction flagged `last`, an undefined
/// MAC opcode or the end of the program memory, exactly where `run` stops.
pub fn decode_from(memory: &ProgramMemory, start: usize) -> Vec<Instruction> {
    let mut program = Vec::with_capacity(PROGRAM_SLOTS.saturating_sub(start).min(48));
    for slot in start..PROGRAM_SLOTS {
        let Some(inst) = decode(memory.slots[slot]) else {
            break;
        };
        program.push(inst);
        if inst.last {
            break;
        }
    }
    program
}

/// `run` over instructions decoded by [`decode_from`] with the same `start`.
pub fn run_decoded(
    program: &[Instruction],
    start: usize,
    inputs: &[Vec4; INPUT_COUNT],
    constants: &[Vec4; CONSTANT_COUNT],
) -> Outputs {
    run_program(program, start, inputs, constants, None)
}

fn run_inner(
    memory: &ProgramMemory,
    start: usize,
    inputs: &[Vec4; INPUT_COUNT],
    constants: &[Vec4; CONSTANT_COUNT],
    trace: Option<&mut Vec<String>>,
) -> Outputs {
    run_program(&decode_from(memory, start), start, inputs, constants, trace)
}

fn run_program(
    program: &[Instruction],
    start: usize,
    inputs: &[Vec4; INPUT_COUNT],
    constants: &[Vec4; CONSTANT_COUNT],
    mut trace: Option<&mut Vec<String>>,
) -> Outputs {
    // The constant file is only copied when the program writes it (a few programs do), otherwise it is read in place.
    let base_constants = constants;
    let mut written_constants: Option<Box<[Vec4; CONSTANT_COUNT]>> = None;
    let mut temps = [[0.0f32; 4]; TEMP_COUNT];
    let mut outputs = [OUTPUT_INIT; OUTPUT_COUNT];
    let mut address = 0i32;
    for (offset, &inst) in program.iter().enumerate() {
        let slot = start + offset;
        let constants: &[Vec4; CONSTANT_COUNT] =
            written_constants.as_deref().unwrap_or(base_constants);
        let read = |source: Source,
                    temps: &[Vec4; TEMP_COUNT],
                    outputs: &[Vec4; OUTPUT_COUNT],
                    constants: &[Vec4; CONSTANT_COUNT]|
         -> Vec4 {
            let raw = match source.mux {
                1 => {
                    if source.register == POSITION_MIRROR {
                        outputs[0]
                    } else {
                        temps[source.register]
                    }
                }
                2 => inputs[inst.input],
                3 => {
                    // ARL may saturate to i32::MAX for huge/positive infinite inputs.
                    // Widen before adding the base so invalid addresses remain zero reads,
                    // rather than panicking only in debug builds.
                    let index =
                        inst.constant as i64 + i64::from(if inst.indexed { address } else { 0 });
                    if (0..CONSTANT_COUNT as i64).contains(&index) {
                        constants[index as usize]
                    } else {
                        [0.0; 4]
                    }
                }
                _ => [0.0; 4],
            };
            let raw = raw.map(flush);
            let mut out = [0.0f32; 4];
            for (index, slot) in out.iter_mut().enumerate() {
                let value = raw[((source.swizzle >> (6 - 2 * index)) & 3) as usize];
                *slot = if source.negate { -value } else { value };
            }
            out
        };
        let a = read(inst.a, &temps, &outputs, constants);
        let b = read(inst.b, &temps, &outputs, constants);
        let c = read(inst.c, &temps, &outputs, constants);
        let mac = mac_result(inst.mac, a, b, c);
        let ilu = ilu_result(inst.ilu, c);
        let new_address = (inst.mac == 13).then(|| floor_to_int(a[0]));
        if let Some(lines) = trace.as_deref_mut() {
            lines.push(format!(
                "{slot}: a0={address} mac{} ilu{} a={a:?} b={b:?} c={c:?} -> mac={mac:?} ilu={ilu:?} new_a0={new_address:?}",
                inst.mac, inst.ilu
            ));
        }
        let paired = inst.mac != 0 && inst.ilu != 0;
        if let Some(value) = mac {
            if !(paired && inst.temp == 1) {
                if inst.temp == POSITION_MIRROR {
                    write_masked(&mut outputs[0], value, inst.mac_mask);
                } else {
                    write_masked(&mut temps[inst.temp], value, inst.mac_mask);
                }
            }
        }
        if let Some(value) = ilu {
            let destination = if paired { 1 } else { inst.temp };
            if destination == POSITION_MIRROR {
                write_masked(&mut outputs[0], value, inst.ilu_mask);
            } else {
                write_masked(&mut temps[destination], value, inst.ilu_mask);
            }
        }
        let routed = if inst.out_is_ilu { ilu } else { mac };
        if let Some(value) = routed.filter(|_| inst.out_mask != 0) {
            if inst.out_to_output && inst.out_address < OUTPUT_COUNT {
                write_masked(&mut outputs[inst.out_address], value, inst.out_mask);
            } else if !inst.out_to_output && inst.out_address < CONSTANT_COUNT {
                let table = written_constants.get_or_insert_with(|| Box::new(*base_constants));
                write_masked(&mut table[inst.out_address], value, inst.out_mask);
            }
        }
        if let Some(next) = new_address {
            address = next;
        }
        if inst.last {
            break;
        }
    }
    Outputs { registers: outputs }
}

/// Window space position to wgpu clip space. The program's `oPos` is `(x, y, z, w)` in window pixels with the perspective
/// divide done (INFERRED from the 41 of 46 programs of docs/vertex-translator.md section 10.2). Depth is recovered as
/// `(z - c59.z) / c58.z` when the viewport scale constant is non zero. Returns `None` when `w` is 0 or any value is not finite.
/// A negative `w` (behind the eye) is returned as the clip space vector it inverts to, so the GPU clips the triangle against the
/// near plane (slice 5, before it the triangle was dropped and in level frames showed holes).
pub fn to_clip(
    position: Vec4,
    screen: (f32, f32),
    constants: &[Vec4; CONSTANT_COUNT],
) -> Option<Vec4> {
    to_clip_with(position, screen, constants, true)
}

/// [`to_clip`] with the depth clamp optional (`clamp_depth` false keeps the true clip space z, so the GPU clips against the depth
/// range; slice 5 experiment `no_depth_clamp`).
pub fn to_clip_with(
    position: Vec4,
    screen: (f32, f32),
    constants: &[Vec4; CONSTANT_COUNT],
    clamp_depth: bool,
) -> Option<Vec4> {
    let [x, y, z, w] = position;
    if w.abs().partial_cmp(&1e-6) != Some(std::cmp::Ordering::Greater)
        || !x.is_finite()
        || !y.is_finite()
        || !z.is_finite()
        || !w.is_finite()
    {
        return None;
    }
    let scale = constants[58][2];
    let offset = constants[59][2];
    let depth = if scale != 0.0 && scale.is_finite() {
        (z - offset) / scale
    } else {
        z
    };
    let ndc_x = x / screen.0 * 2.0 - 1.0;
    let ndc_y = 1.0 - y / screen.1 * 2.0;
    let depth = if clamp_depth {
        depth.clamp(0.0, 1.0)
    } else {
        depth
    };
    Some([ndc_x * w, ndc_y * w, depth * w, w])
}

/// The host renderer's own raster map (`tools/nv2a/viewport.py` `live_raster_position`, the xemu derived "live raster" profile of
/// `src/gpu/live_vk_pipeline.c`) applied to the program's `oPos`: `|w|` clamped to `2^-64 ..= 2^64` with its sign, x and y
/// truncated to sixteenths of a pixel, depth `z / clip_max` (16777215) with no offset undone and no clamp, clip
/// `((2 xy - screen) / screen * w, z / clip_max * w, w)`. Slice 6 experiment `host_raster`.
pub fn to_clip_host_raster(position: Vec4, screen: (f32, f32), clip_max: f32) -> Option<Vec4> {
    let [x, y, z, w] = position;
    if !x.is_finite() || !y.is_finite() || !z.is_finite() || !w.is_finite() {
        return None;
    }
    let low = f32::from_bits(0x1F80_0000);
    let high = f32::from_bits(0x5F80_0000);
    let w = if w > 0.0 || w.to_bits() == 0 {
        w.clamp(low, high)
    } else {
        w.clamp(-high, -low)
    };
    let sx = (x * 16.0).trunc() / 16.0;
    let sy = (y * 16.0).trunc() / 16.0;
    let ndc_x = (2.0 * sx - screen.0) / screen.0;
    let ndc_y = -((2.0 * sy - screen.1) / screen.1);
    Some([ndc_x * w, ndc_y * w, z / clip_max * w, w])
}

/// One line per instruction of the program at `start` (until the `last` flag), for experiments and reports.
pub fn describe(memory: &ProgramMemory, start: usize) -> String {
    let mut text = String::new();
    for slot in start..PROGRAM_SLOTS {
        let Some(inst) = decode(memory.slots[slot]) else {
            text.push_str(&format!("  {slot}: undefined\n"));
            break;
        };
        let source = |s: Source| {
            let name = match s.mux {
                1 => format!("r{}", s.register),
                2 => format!("v{}", inst.input),
                3 => format!(
                    "c[{}{}]",
                    inst.constant,
                    if inst.indexed { "+a0" } else { "" }
                ),
                _ => "0".to_string(),
            };
            format!(
                "{}{name}.{:02x}",
                if s.negate { "-" } else { "" },
                s.swizzle
            )
        };
        let out = if inst.out_to_output {
            format!("o{}", inst.out_address)
        } else {
            format!("c{}", inst.out_address)
        };
        text.push_str(&format!(
            "  {slot}: mac{} ilu{} a={} b={} c={} t{} m{:x}/{:x} {}={}.{:x}{}\n",
            inst.mac,
            inst.ilu,
            source(inst.a),
            source(inst.b),
            source(inst.c),
            inst.temp,
            inst.mac_mask,
            inst.ilu_mask,
            out,
            if inst.out_is_ilu { "ilu" } else { "mac" },
            inst.out_mask,
            if inst.last { " LAST" } else { "" }
        ));
        if inst.last {
            break;
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an instruction word from named fields, layout as `tools/nv2a/isa.py` FIELDS (dword, low bit).
    #[derive(Default)]
    struct Builder {
        words: [u32; 4],
    }

    impl Builder {
        fn set(mut self, dword: usize, low: u32, value: u32) -> Self {
            self.words[dword] |= value << low;
            self
        }
        fn mac(self, op: u32) -> Self {
            self.set(1, 21, op)
        }
        fn ilu(self, op: u32) -> Self {
            self.set(1, 25, op)
        }
        fn input(self, register: u32) -> Self {
            self.set(1, 9, register)
        }
        fn constant(self, index: u32) -> Self {
            self.set(1, 13, index)
        }
        /// Operand A: mux, temp, swizzle (xyzw is 0x1b).
        fn a(self, mux: u32, temp: u32, swizzle: u32, negate: bool) -> Self {
            self.set(2, 26, mux)
                .set(2, 28, temp)
                .set(1, 0, swizzle)
                .set(1, 8, u32::from(negate))
        }
        fn b(self, mux: u32, temp: u32, swizzle: u32) -> Self {
            self.set(2, 11, mux).set(2, 13, temp).set(2, 17, swizzle)
        }
        fn c(self, mux: u32, temp: u32, swizzle: u32) -> Self {
            self.set(3, 28, mux)
                .set(3, 30, temp & 3)
                .set(2, 0, temp >> 2)
                .set(2, 2, swizzle)
        }
        fn temp(self, register: u32, mac_mask: u32, ilu_mask: u32) -> Self {
            self.set(3, 20, register)
                .set(3, 24, mac_mask)
                .set(3, 16, ilu_mask)
        }
        fn output(self, address: u32, mask: u32, is_ilu: bool) -> Self {
            self.set(3, 3, address)
                .set(3, 11, 1)
                .set(3, 12, mask)
                .set(3, 2, u32::from(is_ilu))
        }
        fn last(self) -> Self {
            self.set(3, 0, 1)
        }
        fn build(self) -> [u32; 4] {
            self.words
        }
    }

    const XYZW: u32 = 0x1b;

    fn program(words: &[[u32; 4]]) -> ProgramMemory {
        let mut memory = ProgramMemory::default();
        for (slot, word) in words.iter().enumerate() {
            memory.slots[slot] = *word;
        }
        memory
    }

    fn zero_inputs() -> [Vec4; INPUT_COUNT] {
        [[0.0; 4]; INPUT_COUNT]
    }

    fn zero_constants() -> [Vec4; CONSTANT_COUNT] {
        [[0.0; 4]; CONSTANT_COUNT]
    }

    #[test]
    fn decode_reads_the_documented_fields() {
        let word = Builder::default()
            .mac(7)
            .ilu(2)
            .input(3)
            .constant(58)
            .a(2, 0, 0xE4, true)
            .temp(5, 0xF, 0x1)
            .output(9, 0xC, false)
            .last()
            .build();
        let inst = decode(word).expect("valid");
        assert_eq!(
            (inst.mac, inst.ilu, inst.input, inst.constant),
            (7, 2, 3, 58)
        );
        assert!(inst.a.negate && inst.last && inst.out_to_output);
        assert_eq!(inst.a.mux, 2);
        assert_eq!(inst.a.swizzle, 0xE4);
        assert_eq!((inst.temp, inst.mac_mask, inst.ilu_mask), (5, 0xF, 0x1));
        assert_eq!((inst.out_address, inst.out_mask), (9, 0xC));
        // opcode 14 and 15 of the MAC field are undefined
        assert!(decode(Builder::default().mac(14).build()).is_none());
    }

    #[test]
    fn mov_input_to_position() {
        let memory = program(&[Builder::default()
            .mac(1)
            .input(1)
            .a(2, 0, XYZW, false)
            .output(0, 0xF, false)
            .last()
            .build()]);
        let mut inputs = zero_inputs();
        inputs[1] = [1.0, 2.0, 3.0, 4.0];
        let out = run(&memory, 0, &inputs, &zero_constants());
        assert_eq!(out.position(), [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn swizzle_and_negate() {
        // swizzle wzyx = 0b00_01_10_11 reversed: x<-w(3), y<-z(2), z<-y(1), w<-x(0) = 0xC6 -> 11 10 01 00
        let memory = program(&[Builder::default()
            .mac(1)
            .input(1)
            .a(2, 0, 0b11_10_01_00, true)
            .output(3, 0xF, false)
            .last()
            .build()]);
        let mut inputs = zero_inputs();
        inputs[1] = [1.0, 2.0, 3.0, 4.0];
        let out = run(&memory, 0, &inputs, &zero_constants());
        assert_eq!(out.diffuse(), [-4.0, -3.0, -2.0, -1.0]);
    }

    #[test]
    fn dp4_with_a_constant_matrix_row() {
        // oT0 = dp4(v1, c10)  = 1*0.5 + 2*(-1) + 3*2 + 4*0.25 = 0.5 - 2 + 6 + 1 = 5.5
        let memory = program(&[Builder::default()
            .mac(7)
            .input(1)
            .constant(10)
            .a(2, 0, XYZW, false)
            .b(3, 0, XYZW)
            .output(9, 0xF, false)
            .last()
            .build()]);
        let mut inputs = zero_inputs();
        inputs[1] = [1.0, 2.0, 3.0, 4.0];
        let mut constants = zero_constants();
        constants[10] = [0.5, -1.0, 2.0, 0.25];
        let out = run(&memory, 0, &inputs, &constants);
        assert_eq!(out.texcoord(0), [5.5; 4]);
    }

    #[test]
    fn mad_uses_a_times_b_plus_c() {
        // o4 = v1 * c1 + c2 per component
        let memory = program(&[Builder::default()
            .mac(4)
            .input(1)
            .constant(1)
            .a(2, 0, XYZW, false)
            .b(3, 0, XYZW)
            .c(1, 2, XYZW)
            .temp(2, 0xF, 0)
            .output(4, 0xF, false)
            .last()
            .build()]);
        // c operand reads temp r2 which starts at zero -> o4 = v1 * c1
        let mut inputs = zero_inputs();
        inputs[1] = [1.0, 2.0, 3.0, 4.0];
        let mut constants = zero_constants();
        constants[1] = [2.0, 3.0, 4.0, 5.0];
        let out = run(&memory, 0, &inputs, &constants);
        assert_eq!(out.specular(), [2.0, 6.0, 12.0, 20.0]);
    }

    #[test]
    fn rcc_and_ilu_scalar_to_output() {
        // oFog (address 5) = RCP of c operand x. c operand: constant register via mux 3 reads the instruction's constant field.
        let memory = program(&[Builder::default()
            .ilu(2)
            .constant(7)
            .c(3, 0, XYZW)
            .output(5, 0x8, true)
            .last()
            .build()]);
        let mut constants = zero_constants();
        constants[7] = [4.0, 0.0, 0.0, 0.0];
        let out = run(&memory, 0, &zero_inputs(), &constants);
        assert_eq!(out.registers[5][0], 0.25);
        // RCP of zero is +infinity, RCC clamps it
        assert_eq!(rcc(0.0), 1.844_674_4e19);
        assert_eq!(rcc(-0.0), -1.844_674_4e19);
        assert_eq!(rcc(1e30), 5.421_011e-20);
    }

    #[test]
    fn r12_mirrors_the_position_output() {
        // 1: oPos.xyz = v1 * c58 (MUL).  2: oPos.xyz = r12 + c59 (ADD reads A and C: A=r12, C=const)
        let memory = program(&[
            Builder::default()
                .mac(2)
                .input(1)
                .constant(58)
                .a(2, 0, XYZW, false)
                .b(3, 0, XYZW)
                .output(0, 0xE, false)
                .build(),
            Builder::default()
                .mac(3)
                .constant(59)
                .a(1, 12, XYZW, false)
                .c(3, 0, XYZW)
                .output(0, 0xE, false)
                .last()
                .build(),
        ]);
        let mut inputs = zero_inputs();
        inputs[1] = [1.0, 2.0, 3.0, 1.0];
        let mut constants = zero_constants();
        constants[58] = [10.0, 20.0, 30.0, 0.0];
        constants[59] = [1.0, 2.0, 3.0, 0.0];
        let out = run(&memory, 0, &inputs, &constants);
        // x = 1*10 + 1, y = 2*20 + 2, z = 3*30 + 3, w keeps its start value 1
        assert_eq!(out.position(), [11.0, 42.0, 93.0, 1.0]);
    }

    #[test]
    fn relative_constant_read_adds_a0() {
        // ARL a0 <- floor(v1.x); then oD0 = c[5 + a0]
        let memory = program(&[
            Builder::default()
                .mac(13)
                .input(1)
                .a(2, 0, XYZW, false)
                .build(),
            Builder::default()
                .mac(1)
                .constant(5)
                .set(3, 1, 1)
                .a(3, 0, XYZW, false)
                .output(3, 0xF, false)
                .last()
                .build(),
        ]);
        let mut inputs = zero_inputs();
        inputs[1] = [2.7, 0.0, 0.0, 0.0];
        let mut constants = zero_constants();
        constants[7] = [9.0, 8.0, 7.0, 6.0];
        let out = run(&memory, 0, &inputs, &constants);
        assert_eq!(out.diffuse(), [9.0, 8.0, 7.0, 6.0]);
        for extreme in [f32::MAX, f32::INFINITY, f32::MIN, f32::NEG_INFINITY] {
            inputs[1][0] = extreme;
            let out = run(&memory, 0, &inputs, &constants);
            assert_eq!(out.diffuse(), [0.0; 4], "out-of-range ARL {extreme}");
        }
    }

    #[test]
    fn negative_address_register_selects_a_table_row_and_the_trace_agrees() {
        // The fire sprite program (slice 10): ARL a0 <- floor(v1.x) with v1.x = -68, then oD0 = c[96 + a0] = c[28]. The traced run
        // must give the same outputs as the plain run and report the address register it used.
        let memory = program(&[
            Builder::default()
                .mac(13)
                .input(1)
                .a(2, 0, XYZW, false)
                .build(),
            Builder::default()
                .mac(1)
                .constant(96)
                .set(3, 1, 1)
                .a(3, 0, XYZW, false)
                .output(3, 0xF, false)
                .last()
                .build(),
        ]);
        let mut inputs = zero_inputs();
        inputs[1] = [-68.0, 0.0, 0.0, 0.0];
        let mut constants = zero_constants();
        constants[28] = [-1.0, 1.0, 1.0, 1.0];
        constants[96] = [5.0; 4];
        let plain = run(&memory, 0, &inputs, &constants);
        let mut trace = Vec::new();
        let traced = run_traced(&memory, 0, &inputs, &constants, &mut trace);
        assert_eq!(plain.diffuse(), [-1.0, 1.0, 1.0, 1.0]);
        assert_eq!(traced, plain);
        assert_eq!(trace.len(), 2);
        assert!(trace[1].contains("a0=-68"), "{}", trace[1]);
    }

    #[test]
    fn execution_starts_at_the_given_slot_and_stops_at_final() {
        let first = Builder::default()
            .mac(1)
            .constant(1)
            .a(3, 0, XYZW, false)
            .output(3, 0xF, false)
            .last()
            .build();
        let second = Builder::default()
            .mac(1)
            .constant(2)
            .a(3, 0, XYZW, false)
            .output(3, 0xF, false)
            .last()
            .build();
        let memory = program(&[first, second]);
        let mut constants = zero_constants();
        constants[1] = [1.0; 4];
        constants[2] = [2.0; 4];
        assert_eq!(
            run(&memory, 0, &zero_inputs(), &constants).diffuse(),
            [1.0; 4]
        );
        assert_eq!(
            run(&memory, 1, &zero_inputs(), &constants).diffuse(),
            [2.0; 4]
        );
    }

    #[test]
    fn zero_times_infinity_is_zero_and_slt_orders_signed_zero() {
        assert_eq!(mul(0.0, f32::INFINITY), 0.0);
        assert!(less(-0.0, 0.0));
        assert!(!less(0.0, -0.0));
        assert!(greater_equal(0.0, -0.0));
    }

    #[test]
    fn dst_lit_and_log_exp() {
        assert_eq!(
            mac_result(8, [9.0, 2.0, 3.0, 9.0], [9.0, 5.0, 9.0, 7.0], [0.0; 4]),
            Some([1.0, 10.0, 3.0, 7.0])
        );
        assert_eq!(lit([0.5, 0.25, 0.0, 2.0]), [1.0, 0.5, 0.0625, 1.0]);
        assert_eq!(lit([-1.0, 0.25, 0.0, 2.0]), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(exp_unit(3.5)[0], 8.0);
        assert!((exp_unit(3.5)[1] - 0.5).abs() < 1e-6);
        assert_eq!(log_unit(10.0)[0], 3.0);
        assert!((log_unit(10.0)[1] - 1.25).abs() < 1e-6);
    }

    #[test]
    fn viewport_round_trip_to_clip_space() {
        // A program with c58 = (320, -240, 8388607.5, 1), c59 = (320, 240, 8388607.5, 0) maps clip (cx, cy, cz, w) to
        // window (cx/w*320 + 320, -cy/w*240 + 240, cz/w*scale + offset, w).
        let mut constants = zero_constants();
        constants[58] = [320.0, -240.0, 100.0, 1.0];
        constants[59] = [320.0, 240.0, 0.0, 0.0];
        // clip (0.5, -0.5, 0.25) at w = 2: window x = 0.25*320+320 = 400, y = 0.25*240+240 = 300, z = 0.125*100 = 12.5
        let clip = to_clip([400.0, 300.0, 12.5, 2.0], (640.0, 480.0), &constants).unwrap();
        // ndc x = 400/640*2-1 = 0.25, ndc y = 1 - 300/480*2 = -0.25, depth 0.125; times w = 2
        assert_eq!(clip, [0.5, -0.5, 0.25, 2.0]);
        // behind the eye
        // slice 5: a vertex behind the eye (w < 0) is passed on as a clip space vector so the GPU clips the triangle against the
        // near plane; only w = 0 and non finite values are refused
        assert!(
            to_clip([0.0, 0.0, 0.0, -1.0], (640.0, 480.0), &constants)
                .is_some_and(|clip| clip[3] < 0.0)
        );
        assert!(to_clip([0.0, 0.0, 0.0, 0.0], (640.0, 480.0), &constants).is_none());
        assert!(to_clip([f32::NAN, 0.0, 0.0, 1.0], (640.0, 480.0), &constants).is_none());
    }
}
