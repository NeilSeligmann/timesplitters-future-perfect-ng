// SPDX-License-Identifier: GPL-3.0-or-later
//! The NV2A register combiners as the game binds them, decoded from the pixel shader definition (T2024 slice 3).
//!
//! `SetPixelShader(program)` hands the device a pointer to a pixel shader definition in guest memory. Its first 0x100 bytes
//! follow the Xbox `D3DPIXELSHADERDEF` layout (MEASURED against the ten definitions of one capture: the combiner count and the
//! texture mode word decode to values that fit, docs/t-wgpu-renderer.md Slice 3):
//!
//! | offset | words | field |
//! | ---: | ---: | --- |
//! | 0x00 | 8 | `PSAlphaInputs` (per stage input word, A in bits 31..24, B, C, D in bits 7..0) |
//! | 0x20 | 2 | final combiner inputs ABCD and EFG |
//! | 0x28 | 8 | `PSConstant0` (per stage ARGB constant) |
//! | 0x48 | 8 | `PSConstant1` |
//! | 0x68 | 8 | `PSAlphaOutputs` |
//! | 0x88 | 8 | `PSRGBInputs` |
//! | 0xA8 | 1 | `PSCompareMode` |
//! | 0xAC | 2 | final combiner constants 0 and 1 (ARGB) |
//! | 0xB4 | 8 | `PSRGBOutputs` |
//! | 0xD4 | 1 | `PSCombinerCount` (stage count in the low nibble) |
//! | 0xD8 | 1 | `PSTextureModes` (5 bits per stage, 0 none, 1 2D) |
//!
//! Input byte: bits 3..0 register (0 zero, 1 constant 0, 2 constant 1, 3 fog, 4 diffuse, 5 specular, 8 to 11 texture 0 to 3,
//! 12 spare 0, 13 spare 1, 14 E times F, 15 spare 0 plus specular), bit 4 channel (alpha, else rgb for the colour portion and
//! blue for the alpha portion), bits 7..5 mapping (0 clamp identity, 1 invert, 2 expand, 3 expand negate, 4 half bias, 5 half bias
//! negate, 6 signed identity, 7 signed negate). INFERRED from the NV2A register documentation: the observed words decode to
//! "diffuse x texture" and "texture x constant" this way.
//!
//! Output word: bits 3..0 sum destination, 7..4 AB destination, 11..8 CD destination, bit 12 CD dot product, bit 13 AB dot
//! product, bit 14 mux, bits 16..17 scale (INFERRED: 0 none, 1 times 2, 2 times 4, 3 divide by 2: the observed shaders carry 1,
//! which fits the game writing vertex colours at 0x7F for a times 2 combiner). A destination of 0 discards the value.
//!
//! Stages run in order on one register file (spare 0, spare 1, diffuse, specular and the textures can be written), then the final
//! combiner computes `A * B + (1 - A) * C + D` for the colour and `G` for alpha. Unsupported (flagged, rendered as zero or
//! ignored): fog (the register reads 0, so a fog final combiner passes spare 0 through), textures 1 to 3 are sampled by the replayer since slice 4 (the CPU
//! `evaluate` wrapper still reads them as 0, [`PixelProgram::uses_other_textures`] reports a shader that uses them), the compare mode.

/// Maximum combiner stages.
pub const MAX_STAGES: usize = 8;

/// One general combiner stage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stage {
    pub rgb_inputs: u32,
    pub alpha_inputs: u32,
    pub rgb_outputs: u32,
    pub alpha_outputs: u32,
    pub constant0: [f32; 4],
    pub constant1: [f32; 4],
}

/// A decoded pixel shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelProgram {
    pub stages: [Stage; MAX_STAGES],
    pub count: u32,
    /// Combiner count word >> 8: bit 0 mux on the r0 alpha msb (else lsb), bit 4 unique constant 0 per stage, bit 8 unique constant 1.
    pub flags: u32,
    pub texture_modes: u32,
    pub final_abcd: u32,
    pub final_efg: u32,
    pub final_constant0: [f32; 4],
    pub final_constant1: [f32; 4],
}

pub(crate) fn argb(word: u32) -> [f32; 4] {
    let byte = |shift: u32| ((word >> shift) & 0xFF) as f32 / 255.0;
    [byte(16), byte(8), byte(0), byte(24)]
}

/// Output mapping (xemu `get_output`), `mapping` is bits 15 to 17 of the output word: 1 bias, 2 times 2, 3 times 2 with bias,
/// 4 times 4, 6 divide by 2 (the others are invalid and read as identity).
fn output_map(x: f32, mapping: u32) -> f32 {
    match mapping {
        1 => x - 0.5,
        2 => x * 2.0,
        3 => (x - 0.5) * 2.0,
        4 => x * 4.0,
        6 => x / 2.0,
        _ => x,
    }
}

/// The register file during evaluation.
#[derive(Clone, Copy)]
struct Registers {
    values: [[f32; 4]; 16],
}

fn map_input(value: f32, mapping: u32) -> f32 {
    let unit = value.clamp(0.0, 1.0);
    match mapping {
        0 => unit,
        1 => 1.0 - unit,
        2 => 2.0 * unit - 1.0,
        3 => -(2.0 * unit - 1.0),
        4 => unit - 0.5,
        5 => -(unit - 0.5),
        6 => value.clamp(-1.0, 1.0),
        _ => -value.clamp(-1.0, 1.0),
    }
}

impl Registers {
    fn input(&self, byte: u32, alpha_portion: bool) -> [f32; 3] {
        let value = self.values[(byte & 0xF) as usize];
        let channel = (byte >> 4) & 1 != 0;
        let base = if alpha_portion {
            [if channel { value[3] } else { value[2] }; 3]
        } else if channel {
            [value[3]; 3]
        } else {
            [value[0], value[1], value[2]]
        };
        base.map(|x| map_input(x, byte >> 5))
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; 3]
}

fn product(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

impl PixelProgram {
    /// Decode the first 0x40 words of a pixel shader definition. `None` for fewer than 0x3C words or a definition with no stage.
    pub fn from_words(words: &[u32]) -> Option<Self> {
        if words.len() < 0x3C {
            return None;
        }
        let word = |offset: usize| words[offset / 4];
        let count = word(0xD4) & 0xF;
        if count == 0 {
            return None; // not a definition (an unreadable or zero filled pointer)
        }
        let flags = word(0xD4) >> 8;
        let mut stages = [Stage::default(); MAX_STAGES];
        for (index, stage) in stages.iter_mut().enumerate() {
            *stage = Stage {
                rgb_inputs: word(0x88 + index * 4),
                alpha_inputs: word(index * 4),
                rgb_outputs: word(0xB4 + index * 4),
                alpha_outputs: word(0x68 + index * 4),
                constant0: argb(word(0x28 + index * 4)),
                constant1: argb(word(0x48 + index * 4)),
            };
        }
        // xemu psh.c: constant 0 and 1 are the stage 0 values in every stage unless the count word asks for unique ones.
        let (first0, first1) = (stages[0].constant0, stages[0].constant1);
        for stage in stages.iter_mut().skip(1) {
            if flags & 0x10 == 0 {
                stage.constant0 = first0;
            }
            if flags & 0x100 == 0 {
                stage.constant1 = first1;
            }
        }
        Some(Self {
            stages,
            count: count.min(MAX_STAGES as u32),
            flags,
            texture_modes: word(0xD8),
            final_abcd: word(0x20),
            final_efg: word(0x24),
            final_constant0: argb(word(0xAC)),
            final_constant1: argb(word(0xB0)),
        })
    }

    /// Stage 0 samples a texture.
    pub fn texture0(&self) -> bool {
        self.texture_modes & 0x1F != 0
    }

    /// Any of the stages 1 to 3 samples a texture (this replayer binds stage 0 only, those read zero).
    pub fn uses_other_textures(&self) -> bool {
        self.texture_modes >> 5 & 0x7FFF != 0
    }

    /// Bit `i` set when stage `i` samples a texture (texture mode not 0).
    pub fn texture_mask(&self) -> u32 {
        (0..4)
            .filter(|stage| (self.texture_modes >> (stage * 5)) & 0x1F != 0)
            .fold(0, |mask, stage| mask | 1 << stage)
    }

    /// The colour the pixel program gives for a diffuse colour and a stage 0 texel (both rgba 0..1). The WGSL fragment stage
    /// mirrors it. Fog reads as zero, the specular colour as zero. Stages 1 to 3 read zero here, see [`Self::evaluate_stages`].
    pub fn evaluate(&self, diffuse: [f32; 4], texel: [f32; 4]) -> [f32; 4] {
        self.evaluate_stages(diffuse, [texel, [0.0; 4], [0.0; 4], [0.0; 4]])
    }

    /// As [`Self::evaluate`] with the texels of all four texture stages (registers 8 to 11); a stage whose texture mode is 0 reads
    /// zero whatever is passed.
    pub fn evaluate_stages(&self, diffuse: [f32; 4], texels: [[f32; 4]; 4]) -> [f32; 4] {
        self.evaluate_colours(diffuse, [0.0; 4], texels)
    }

    /// Evaluate both NV2A vertex colours, including V1 used by lit mesh combiners.
    pub fn evaluate_colours(
        &self,
        diffuse: [f32; 4],
        specular: [f32; 4],
        texels: [[f32; 4]; 4],
    ) -> [f32; 4] {
        let mut registers = Registers {
            values: [[0.0; 4]; 16],
        };
        registers.values[4] = diffuse;
        registers.values[5] = specular;
        let mask = self.texture_mask();
        for (stage, texel) in texels.iter().enumerate() {
            if mask >> stage & 1 != 0 {
                registers.values[8 + stage] = *texel;
            }
        }
        for stage in &self.stages[..self.count as usize] {
            registers.values[1] = stage.constant0;
            registers.values[2] = stage.constant1;
            let rgb_a = registers.input(stage.rgb_inputs >> 24, false);
            let rgb_b = registers.input((stage.rgb_inputs >> 16) & 0xFF, false);
            let rgb_c = registers.input((stage.rgb_inputs >> 8) & 0xFF, false);
            let rgb_d = registers.input(stage.rgb_inputs & 0xFF, false);
            let alpha_a = registers.input(stage.alpha_inputs >> 24, true);
            let alpha_b = registers.input((stage.alpha_inputs >> 16) & 0xFF, true);
            let alpha_c = registers.input((stage.alpha_inputs >> 8) & 0xFF, true);
            let alpha_d = registers.input(stage.alpha_inputs & 0xFF, true);
            let spare_alpha = registers.values[12][3];
            // mux condition (xemu): r0.a >= 0.5 for the msb flavour, else the lsb of the 8 bit alpha. true selects CD.
            let mux_cd = if self.flags & 1 != 0 {
                spare_alpha >= 0.5
            } else {
                (spare_alpha * 255.0) as u32 & 1 == 1
            };
            let write = |registers: &mut Registers,
                         destination: u32,
                         value: [f32; 3],
                         alpha: Option<f32>| {
                if destination != 0 && destination < 16 {
                    let slot = &mut registers.values[destination as usize];
                    match alpha {
                        None => *slot = [value[0], value[1], value[2], slot[3]],
                        Some(a) => slot[3] = a,
                    }
                }
            };
            // colour portion. Output word (xemu `parse_combiner_output`): bits 3..0 CD destination, 7..4 AB destination, 11..8
            // sum or mux destination, 12 CD dot, 13 AB dot, 14 mux, 15..17 mapping, 18 CD blue to alpha, 19 AB blue to alpha.
            let out = stage.rgb_outputs;
            let ab = if out & 0x2000 != 0 {
                dot(rgb_a, rgb_b)
            } else {
                product(rgb_a, rgb_b)
            };
            let cd = if out & 0x1000 != 0 {
                dot(rgb_c, rgb_d)
            } else {
                product(rgb_c, rgb_d)
            };
            let sum = if out & 0x4000 != 0 {
                if mux_cd { cd } else { ab }
            } else {
                [ab[0] + cd[0], ab[1] + cd[1], ab[2] + cd[2]]
            };
            let mapping = (out >> 15) & 7;
            let stored = |v: [f32; 3]| v.map(|x| output_map(x, mapping).clamp(-1.0, 1.0));
            write(&mut registers, (out >> 4) & 0xF, stored(ab), None);
            if out & 0x80000 != 0 {
                write(
                    &mut registers,
                    (out >> 4) & 0xF,
                    [0.0; 3],
                    Some(stored(ab)[2]),
                );
            }
            write(&mut registers, out & 0xF, stored(cd), None);
            if out & 0x40000 != 0 {
                write(&mut registers, out & 0xF, [0.0; 3], Some(stored(cd)[2]));
            }
            write(&mut registers, (out >> 8) & 0xF, stored(sum), None);
            // alpha portion (single channel)
            let out = stage.alpha_outputs;
            let ab = alpha_a[0] * alpha_b[0];
            let cd = alpha_c[0] * alpha_d[0];
            let sum = if out & 0x4000 != 0 {
                if mux_cd { cd } else { ab }
            } else {
                ab + cd
            };
            let mapping = (out >> 15) & 7;
            let stored = |v: f32| output_map(v, mapping).clamp(-1.0, 1.0);
            write(&mut registers, (out >> 4) & 0xF, [0.0; 3], Some(stored(ab)));
            write(&mut registers, out & 0xF, [0.0; 3], Some(stored(cd)));
            write(
                &mut registers,
                (out >> 8) & 0xF,
                [0.0; 3],
                Some(stored(sum)),
            );
        }
        // final combiner
        registers.values[1] = self.final_constant0;
        registers.values[2] = self.final_constant1;
        // register 14 is V1 + R0 (specular plus spare 0, flags in the low byte of the EFG word: 0x80 clamp, 0x40 complement V1,
        // 0x20 complement R0), register 15 is E times F (xemu psh_regs.h; slice 3 had the two swapped)
        let flags = self.final_efg & 0xFF;
        let v1 = registers.values[5];
        let r0 = registers.values[12];
        let mut v1r0 = [0.0f32; 4];
        for channel in 0..3 {
            let left = if flags & 0x40 != 0 {
                1.0 - v1[channel]
            } else {
                v1[channel]
            };
            let right = if flags & 0x20 != 0 {
                1.0 - r0[channel]
            } else {
                r0[channel]
            };
            let total = left + right;
            v1r0[channel] = if flags & 0x80 != 0 {
                total.clamp(0.0, 1.0)
            } else {
                total
            };
        }
        registers.values[14] = v1r0;
        let e = registers.input(self.final_efg >> 24, false);
        let f = registers.input((self.final_efg >> 16) & 0xFF, false);
        let ef = product(e, f);
        registers.values[15] = [ef[0], ef[1], ef[2], 0.0];
        let a = registers.input(self.final_abcd >> 24, false);
        let b = registers.input((self.final_abcd >> 16) & 0xFF, false);
        let c = registers.input((self.final_abcd >> 8) & 0xFF, false);
        let d = registers.input(self.final_abcd & 0xFF, false);
        let mut out = [0.0f32; 4];
        for channel in 0..3 {
            out[channel] = (a[channel] * b[channel] + (1.0 - a[channel]) * c[channel] + d[channel])
                .clamp(0.0, 1.0);
        }
        out[3] = registers.input((self.final_efg >> 8) & 0xFF, true)[0].clamp(0.0, 1.0);
        out
    }

    /// The uniform block the WGSL combiner reads (`struct Combiner` in shader.wgsl), 432 bytes:
    /// head `[count, final_abcd, final_efg, texture mask]`, final constants 0 and 1, then 8 stages of
    /// `[rgb inputs, alpha inputs, rgb outputs, alpha outputs]` and 16 constants (constant 0 and 1 of each stage).
    pub fn uniform_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(432);
        for value in [
            self.count | self.flags << 8,
            self.final_abcd,
            self.final_efg,
            self.texture_mask(),
        ] {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in self.final_constant0.iter().chain(&self.final_constant1) {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for stage in &self.stages {
            for value in [
                stage.rgb_inputs,
                stage.alpha_inputs,
                stage.rgb_outputs,
                stage.alpha_outputs,
            ] {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        for stage in &self.stages {
            for value in stage.constant0.iter().chain(&stage.constant1) {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Words as the observed untextured vertex colour shader carries them (MEASURED shape): colour = diffuse x (1 - 0) times 2,
    /// alpha = diffuse alpha x (1 - 0) times 2, final combiner passes spare 0 (D).
    fn diffuse_only() -> Vec<u32> {
        let mut words = vec![0u32; 0x40];
        words[0x88 / 4] = 0xC420_0000; // A diffuse rgb, B inverted zero (1)
        words[0xB4 / 4] = 0x0001_00C0; // AB to spare 0, scale x2
        words[0x00] = 0xD430_1010; // A diffuse alpha, B inverted zero alpha, C and D zero
        words[0x68 / 4] = 0x0001_00C0;
        words[0x20 / 4] = 0x0000_000C;
        words[0x24 / 4] = 0x0000_1C80; // final alpha G = spare 0 alpha
        words[0xD4 / 4] = 0x0001_1101;
        words
    }

    #[test]
    fn untextured_shader_scales_the_diffuse_colour_by_two() {
        let program = PixelProgram::from_words(&diffuse_only()).unwrap();
        assert!(!program.texture0());
        assert_eq!(program.count, 1);
        // the bound texture is ignored: stage 0 does not sample one
        let out = program.evaluate([0.25, 0.5, 0.1, 0.4], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(out, [0.5, 1.0, 0.2, 0.8]);
    }

    #[test]
    fn diffuse_times_texture() {
        let mut words = diffuse_only();
        words[0x88 / 4] = 0xC4C8_0000; // A diffuse, B texture 0
        words[0x00] = 0xD4D8_1010; // alpha: A diffuse a, B texture a
        words[0xD8 / 4] = 1;
        words[0xB4 / 4] = 0x0000_00C0; // scale x1
        words[0x68 / 4] = 0x0000_00C0;
        let program = PixelProgram::from_words(&words).unwrap();
        assert!(program.texture0());
        let out = program.evaluate([0.5, 1.0, 0.25, 0.5], [0.5, 0.5, 1.0, 0.5]);
        assert_eq!(out, [0.25, 0.5, 0.25, 0.25]);
    }

    #[test]
    fn texture_times_constant_and_sum_form() {
        let mut words = diffuse_only();
        words[0x88 / 4] = 0xC8C1_0000; // A texture 0, B constant 0
        words[0xB4 / 4] = 0x0000_00C0;
        words[0x28 / 4] = 0xFF80_4000; // constant 0 ARGB
        words[0xD8 / 4] = 1;
        let program = PixelProgram::from_words(&words).unwrap();
        let out = program.evaluate([1.0; 4], [1.0, 1.0, 1.0, 1.0]);
        assert!((out[0] - 128.0 / 255.0).abs() < 1e-6);
        assert!((out[1] - 64.0 / 255.0).abs() < 1e-6);
        assert_eq!(out[2], 0.0);
        // sum form: A*B + C*D with the sum destination spare 0
        words[0x88 / 4] = 0xC8C8_C4C4; // tex*tex + diffuse*diffuse
        words[0xB4 / 4] = 0x0000_0C00; // sum destination spare 0 (bits 11..8)
        let sum = PixelProgram::from_words(&words).unwrap();
        let out = sum.evaluate([0.5, 0.0, 0.0, 1.0], [0.25, 0.0, 0.0, 1.0]);
        assert_eq!(out[0], 0.0625 + 0.25);
    }

    #[test]
    fn two_stages_chain_through_spare_registers() {
        // stage 0: spare 0 = diffuse x texture. stage 1: spare 0 = spare 0 x constant 0 (the second stage reads the first's result).
        let mut words = diffuse_only();
        words[0x88 / 4] = 0xC4C8_0000;
        words[0xB4 / 4] = 0x0000_00C0;
        words[0x88 / 4 + 1] = 0xCCC1_0000; // A spare 0, B constant 0
        words[0xB4 / 4 + 1] = 0x0000_00C0;
        words[0x28 / 4 + 1] = 0xFF80_8080; // stage 1 constant 0 = 128/255 grey
        words[0xD4 / 4] = 0x0001_1102;
        words[0xD8 / 4] = 1;
        let program = PixelProgram::from_words(&words).unwrap();
        assert_eq!(program.count, 2);
        let out = program.evaluate([1.0, 0.5, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
        assert!((out[0] - 128.0 / 255.0).abs() < 1e-6);
        assert!((out[1] - 0.5 * 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn final_combiner_forms() {
        let mut words = diffuse_only();
        // fog form: A fog alpha, B fog colour, C spare 0: fog reads 0 so spare 0 passes through
        words[0x20 / 4] = 0x1303_0C00;
        let fog = PixelProgram::from_words(&words).unwrap();
        assert_eq!(
            fog.evaluate([0.25, 0.5, 0.1, 0.4], [0.0; 4])[..3],
            [0.5, 1.0, 0.2]
        );
        // C form with A zero: (1 - 0) * spare 0
        words[0x20 / 4] = 0x0000_0C00;
        let c_form = PixelProgram::from_words(&words).unwrap();
        assert_eq!(c_form.evaluate([0.25, 0.0, 0.0, 0.0], [0.0; 4])[0], 0.5);
        // a final combiner reading spare 1 (never written) gives zero
        words[0x20 / 4] = 0x0000_0D00;
        let spare1 = PixelProgram::from_words(&words).unwrap();
        assert_eq!(spare1.evaluate([0.5; 4], [0.0; 4])[0], 0.0);
    }

    #[test]
    fn mappings_dot_products_and_short_input() {
        let mut words = diffuse_only();
        words[0x88 / 4] = 0xE400_0000; // A diffuse, signed negate: -x, clamped to 0 after the product
        let program = PixelProgram::from_words(&words).unwrap();
        assert_eq!(program.evaluate([0.5; 4], [0.0; 4])[0], 0.0);
        assert!(PixelProgram::from_words(&words[..8]).is_none());
        // AB dot product: dot(diffuse, 1) = r + g + b replicated, scale x1
        let mut words = diffuse_only();
        words[0xB4 / 4] = 0x0000_20C0;
        let dot = PixelProgram::from_words(&words).unwrap();
        assert_eq!(
            dot.evaluate([0.125, 0.25, 0.5, 1.0], [0.0; 4])[..3],
            [0.875; 3]
        );
        // no stage: not a definition
        words[0xD4 / 4] = 0;
        assert!(PixelProgram::from_words(&words).is_none());
    }

    #[test]
    fn uniform_block_has_the_documented_size() {
        let program = PixelProgram::from_words(&diffuse_only()).unwrap();
        assert_eq!(program.uniform_bytes().len(), 432);
    }

    #[test]
    fn texture_mask_and_second_stage_texel() {
        // colour = texture 0 x texture 1 (A = 0xC8, B = 0xC9, no scale), texture modes 0x21
        let mut words = vec![0u32; 0x40];
        words[0x88 / 4] = 0xC8C9_0000;
        words[0xB4 / 4] = 0x0000_00C0;
        words[0x00] = 0xD430_1010;
        words[0x68 / 4] = 0x0000_00C0;
        words[0x20 / 4] = 0x0000_0C00;
        words[0x24 / 4] = 0x0000_1C80;
        words[0xD4 / 4] = 0x0001_1101;
        words[0xD8 / 4] = 0x21;
        let program = PixelProgram::from_words(&words).unwrap();
        assert_eq!(program.texture_mask(), 0b11);
        let out = program.evaluate_stages(
            [1.0; 4],
            [
                [0.5, 1.0, 0.25, 1.0],
                [0.5, 0.5, 1.0, 1.0],
                [0.9; 4],
                [0.9; 4],
            ],
        );
        assert!(
            (out[0] - 0.25).abs() < 1e-6
                && (out[1] - 0.5).abs() < 1e-6
                && (out[2] - 0.25).abs() < 1e-6,
            "{out:?}"
        );
        // the one stage wrapper leaves stage 1 at zero
        assert_eq!(program.evaluate([1.0; 4], [0.5; 4])[0], 0.0);
        // the uniform block carries the mask in head.w
        assert_eq!(&program.uniform_bytes()[12..16], &3u32.to_le_bytes());
    }

    #[test]
    fn blue_to_alpha_copies_the_product_blue_into_the_destination_alpha() {
        // stage 0: AB = diffuse x 1, scale x2, AB to spare 1 (0xD) with AB blue to alpha (0x80000), sum to spare 0.
        // final: A = 0, B = 0, C = 1 (inverted zero), D = zero, alpha G = spare 1 alpha (0x1D)
        let mut words = diffuse_only();
        words[0xB4 / 4] = 0x0009_00D0; // times 2 (0x10000), blue to alpha (0x80000), AB dest spare 1
        words[0x20 / 4] = 0x0000_2000; // C = inverted zero (0x20): colour passes white
        words[0x24 / 4] = 0x0000_1D80; // G = spare 1 alpha (0x1D: register 0xD, alpha channel), E F unused
        let program = PixelProgram::from_words(&words).unwrap();
        let out = program.evaluate([0.125, 0.25, 0.375, 1.0], [0.0; 4]);
        // blue 0.375 x 2 = 0.75
        assert!((out[3] - 0.75).abs() < 1e-6, "{out:?}");
        // without the flag spare 1 alpha stays 0
        words[0xB4 / 4] = 0x0001_00D0;
        let plain = PixelProgram::from_words(&words).unwrap();
        assert_eq!(plain.evaluate([0.125, 0.25, 0.375, 1.0], [0.0; 4])[3], 0.0);
    }

    /// xemu `parse_combiner_output`: CD destination is bits 3..0, AB bits 7..4, the sum or mux bits 11..8 (slice 3 had CD and sum
    /// swapped, which no observed shader noticed because they all write AB).
    #[test]
    fn destination_nibbles_follow_xemu() {
        // AB = diffuse x 1 and CD = diffuse x 1, final C form passes spare 0
        let mut words = diffuse_only();
        words[0x88 / 4] = 0xC420_C420;
        words[0xB4 / 4] = 0x0000_0C00; // sum to spare 0
        let sum = PixelProgram::from_words(&words).unwrap();
        assert_eq!(sum.evaluate([0.25, 0.0, 0.0, 1.0], [0.0; 4])[0], 0.5);
        words[0xB4 / 4] = 0x0000_000C; // CD to spare 0
        let cd = PixelProgram::from_words(&words).unwrap();
        assert_eq!(cd.evaluate([0.25, 0.0, 0.0, 1.0], [0.0; 4])[0], 0.25);
    }

    #[test]
    fn output_bias_and_shift_right() {
        let mut words = diffuse_only();
        words[0xB4 / 4] = 0x0000_80C0; // AB to spare 0 with bias (x - 0.5)
        let bias = PixelProgram::from_words(&words).unwrap();
        assert_eq!(bias.evaluate([0.75, 0.0, 0.0, 1.0], [0.0; 4])[0], 0.25);
        words[0xB4 / 4] = 0x0003_00C0; // divide by 2
        let half = PixelProgram::from_words(&words).unwrap();
        assert_eq!(half.evaluate([0.5, 0.0, 0.0, 1.0], [0.0; 4])[0], 0.25);
    }

    #[test]
    fn constants_are_shared_unless_the_count_word_asks_for_unique_ones() {
        // two stages, both read constant 0 (B = 0xC1): stage 0 sets 0.5, stage 1 sets 1.0
        let mut words = diffuse_only();
        words[0x88 / 4] = 0xC4C1_0000;
        words[0x88 / 4 + 1] = 0xCCC1_0000; // stage 1: spare 0 x constant 0
        words[0xB4 / 4 + 1] = 0x0000_00C0;
        words[0x28 / 4] = 0xFF80_8080;
        words[0x28 / 4 + 1] = 0xFFFF_FFFF;
        words[0xD4 / 4] = 0x0001_0102; // two stages, constant 0 shared (flags bit 4 = word bit 12 clear)
        let shared = PixelProgram::from_words(&words).unwrap();
        assert_eq!(shared.stages[1].constant0, shared.stages[0].constant0);
        // stage 0 (default x2 output) gives 1.0, stage 1 multiplies by constant 0: shared 0.502, unique 1.0
        let shared_out = shared.evaluate([1.0, 0.0, 0.0, 1.0], [0.0; 4])[0];
        assert!((shared_out - 128.0 / 255.0).abs() < 1e-5, "{shared_out}");
        words[0xD4 / 4] = 0x0001_1102; // constant 0 unique (word bit 12)
        let unique = PixelProgram::from_words(&words).unwrap();
        assert_eq!(unique.stages[1].constant0, [1.0; 4]);
        assert!((unique.evaluate([1.0, 0.0, 0.0, 1.0], [0.0; 4])[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn final_combiner_registers_14_and_15() {
        // final: A = 0, B = 0, C = 0, D = register 14 (V1 + R0 = specular 0 + spare 0 here), alpha G from spare 0
        let mut words = diffuse_only();
        words[0x20 / 4] = 0x0000_000E;
        let sum = PixelProgram::from_words(&words).unwrap();
        assert_eq!(sum.evaluate([0.25, 0.0, 0.0, 1.0], [0.0; 4])[0], 0.5);
        // register 15 = E x F: E = spare 0 (0x0C), F = inverted zero (0x20), D = register 15
        words[0x20 / 4] = 0x0000_000F;
        words[0x24 / 4] = 0x0C20_1C80;
        let product = PixelProgram::from_words(&words).unwrap();
        assert_eq!(product.evaluate([0.25, 0.0, 0.0, 1.0], [0.0; 4])[0], 0.5);
    }
}
