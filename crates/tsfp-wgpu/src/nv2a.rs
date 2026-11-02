// SPDX-License-Identifier: GPL-3.0-or-later
//! NV2A (class 0x97, "NV097") method decoding into a fixed-function pipeline description.
//!
//! The render code reaches the GPU as pushbuffer methods (`RenderDevice::push_method`, header
//! = `count << 18 | method`). The method numbers and the enum values are the public NV2A
//! register names, cross-checked against the xemu `nv2a_regs.h` (see docs/t-wgpu-renderer.md
//! for the pin). Only the methods the title, menu and HUD screens use are decoded. Everything
//! else lands in [`Report`] so the unsupported set is a measured list, not a guess.
use std::collections::BTreeMap;

pub mod method {
    pub const SET_SURFACE_CLIP_HORIZONTAL: u32 = 0x0200;
    pub const SET_SURFACE_CLIP_VERTICAL: u32 = 0x0204;
    pub const SET_ALPHA_TEST_ENABLE: u32 = 0x0300;
    pub const SET_BLEND_ENABLE: u32 = 0x0304;
    pub const SET_CULL_FACE_ENABLE: u32 = 0x0308;
    pub const SET_DEPTH_TEST_ENABLE: u32 = 0x030C;
    pub const SET_STENCIL_TEST_ENABLE: u32 = 0x032C;
    pub const SET_POLYGON_OFFSET_POINT_ENABLE: u32 = 0x0330;
    pub const SET_POLYGON_OFFSET_LINE_ENABLE: u32 = 0x0334;
    pub const SET_POLYGON_OFFSET_FILL_ENABLE: u32 = 0x0338;
    pub const SET_ALPHA_FUNC: u32 = 0x033C;
    pub const SET_ALPHA_REF: u32 = 0x0340;
    pub const SET_BLEND_FUNC_SFACTOR: u32 = 0x0344;
    pub const SET_BLEND_FUNC_DFACTOR: u32 = 0x0348;
    pub const SET_BLEND_COLOR: u32 = 0x034C;
    pub const SET_BLEND_EQUATION: u32 = 0x0350;
    pub const SET_DEPTH_FUNC: u32 = 0x0354;
    pub const SET_COLOR_MASK: u32 = 0x0358;
    pub const SET_DEPTH_MASK: u32 = 0x035C;
    pub const SET_POLYGON_OFFSET_SCALE_FACTOR: u32 = 0x0384;
    pub const SET_POLYGON_OFFSET_BIAS: u32 = 0x0388;
    pub const SET_CULL_FACE: u32 = 0x039C;
    pub const SET_FRONT_FACE: u32 = 0x03A0;
    pub const SET_COLOR_CLEAR_VALUE: u32 = 0x1D90;
    pub const CLEAR_SURFACE: u32 = 0x1D94;
}

/// Comparison function values `0x200..=0x207` (GL style), shared by depth and alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Compare {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl Compare {
    pub fn from_nv(value: u32) -> Option<Self> {
        Some(match value {
            0x200 => Self::Never,
            0x201 => Self::Less,
            0x202 => Self::Equal,
            0x203 => Self::LessEqual,
            0x204 => Self::Greater,
            0x205 => Self::NotEqual,
            0x206 => Self::GreaterEqual,
            0x207 => Self::Always,
            _ => return None,
        })
    }
    /// The code the fragment shader's alpha test switches on.
    pub fn code(self) -> u32 {
        self as u32
    }
}

/// Blend factor values of `SET_BLEND_FUNC_SFACTOR/DFACTOR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Factor {
    Zero,
    One,
    SrcColour,
    OneMinusSrcColour,
    SrcAlpha,
    OneMinusSrcAlpha,
    DstAlpha,
    OneMinusDstAlpha,
    DstColour,
    OneMinusDstColour,
    SrcAlphaSaturate,
    ConstantColour,
    OneMinusConstantColour,
    ConstantAlpha,
    OneMinusConstantAlpha,
}

impl Factor {
    pub fn from_nv(value: u32) -> Option<Self> {
        Some(match value {
            0x0000 => Self::Zero,
            0x0001 => Self::One,
            0x0300 => Self::SrcColour,
            0x0301 => Self::OneMinusSrcColour,
            0x0302 => Self::SrcAlpha,
            0x0303 => Self::OneMinusSrcAlpha,
            0x0304 => Self::DstAlpha,
            0x0305 => Self::OneMinusDstAlpha,
            0x0306 => Self::DstColour,
            0x0307 => Self::OneMinusDstColour,
            0x0308 => Self::SrcAlphaSaturate,
            0x8001 => Self::ConstantColour,
            0x8002 => Self::OneMinusConstantColour,
            0x8003 => Self::ConstantAlpha,
            0x8004 => Self::OneMinusConstantAlpha,
            _ => return None,
        })
    }
}

/// `SET_BLEND_EQUATION` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Equation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}

impl Equation {
    pub fn from_nv(value: u32) -> Option<Self> {
        Some(match value {
            0x8006 => Self::Add,
            0x800A => Self::Subtract,
            0x800B => Self::ReverseSubtract,
            0x8007 => Self::Min,
            0x8008 => Self::Max,
            _ => return None,
        })
    }
}

/// Fixed-function state the pushbuffer methods have set so far. The hashable part is the
/// pipeline key, `blend_colour` and `alpha_ref` are dynamic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PipelineState {
    pub blend_enable: bool,
    pub source: Factor,
    pub destination: Factor,
    pub equation: Equation,
    /// ARGB, as `SET_BLEND_COLOR` stores it.
    pub blend_colour: u32,
    pub depth_test: bool,
    pub depth_func: Compare,
    pub depth_write: bool,
    /// Polygon offset enable words (NV097 0x330/0x334/0x338). The current renderer only
    /// rasterizes filled triangles; point and line polygon modes are not represented.
    pub polygon_offset_point: bool,
    pub polygon_offset_line: bool,
    pub polygon_offset_fill: bool,
    /// Raw float32 method payloads, with write tracking for the opt-in depth-bias model.
    pub polygon_offset_scale_bits: u32,
    pub polygon_offset_bias_bits: u32,
    pub polygon_offset_scale_written: bool,
    pub polygon_offset_bias_written: bool,
    pub cull: Option<CullFace>,
    pub front_clockwise: bool,
    pub alpha_test: bool,
    pub alpha_func: Compare,
    /// 0..=255
    pub alpha_ref: u8,
    /// `SET_COLOR_MASK` raw value (bytes: 0x01000000 alpha, 0x00010000 red, 0x100 green, 1 blue).
    pub colour_mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CullFace {
    Front,
    Back,
    FrontAndBack,
}

impl Default for PipelineState {
    /// The D3D8 device reset state: no blending (one, zero), depth test off until a method
    /// enables it, all colour channels written.
    fn default() -> Self {
        Self {
            blend_enable: false,
            source: Factor::One,
            destination: Factor::Zero,
            equation: Equation::Add,
            blend_colour: 0,
            depth_test: false,
            depth_func: Compare::LessEqual,
            depth_write: true,
            polygon_offset_point: false,
            polygon_offset_line: false,
            polygon_offset_fill: false,
            polygon_offset_scale_bits: 0,
            polygon_offset_bias_bits: 0,
            polygon_offset_scale_written: false,
            polygon_offset_bias_written: false,
            cull: None,
            front_clockwise: false,
            alpha_test: false,
            alpha_func: Compare::Always,
            alpha_ref: 0,
            colour_mask: 0x0101_0101,
        }
    }
}

/// A clear the pushbuffer requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClearRequest {
    pub colour: Option<u32>,
    pub depth: bool,
    /// Colour channels the clear writes (bit 0 red, 1 green, 2 blue, 3 alpha; `CLEAR_SURFACE` bits 4 to 7). 0xF clears all.
    pub channels: u8,
    /// Colour channels `SET_COLOR_MASK` enabled when the clear was pushed (same bits as `channels`). NV2A clears honour it
    /// (xemu `pgraph` clear: the per channel write enables), slice 8.
    pub write_mask: u8,
}

impl ClearRequest {
    /// The clear with the colour mask applied: channels the mask disables are not written, no channels left means no colour clear.
    pub fn masked(self) -> Self {
        let channels = self.channels & self.write_mask;
        Self {
            colour: self.colour.filter(|_| channels != 0),
            channels,
            ..self
        }
    }
}

/// What the decoder saw and could not apply. Reported by the replay tool and asserted by tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Methods with no decoder, count per method number.
    pub unhandled_methods: BTreeMap<u32, usize>,
    /// Values the decoder had no mapping for: (method, value) -> count.
    pub unmapped_values: BTreeMap<(u32, u32), usize>,
    /// Calls the wgpu device does not execute yet, by name (state handlers, deferred states,
    /// vertex programs, pixel shaders, constants).
    pub ignored_calls: BTreeMap<&'static str, usize>,
    /// Primitive/vertex layouts the replayer skipped: (primitive, stride) -> draws.
    pub skipped_draws: BTreeMap<(u32, u32), usize>,
    /// Draws that referenced a texture id the stream did not provide.
    pub missing_textures: BTreeMap<u32, usize>,
    /// Draws executed through the vertex program interpreter (slice 3).
    pub vsh_draws: usize,
    /// Vertices run through the interpreter.
    pub vsh_vertices: usize,
    /// Triangles drawn with at least one vertex behind the eye (clip w < 0, clipped by the GPU).
    pub vsh_triangles_behind_eye: usize,
    /// Triangles dropped because a vertex had w <= 0 or a non finite position (no clipping is done).
    pub vsh_triangles_dropped: usize,
    /// Distinct vertex programs seen: start slot and instruction count.
    pub vsh_programs: BTreeMap<(u32, u32), usize>,
    /// Textures decoded, per hardware format code (a decode is counted when the texel data changed).
    pub texture_formats: BTreeMap<u32, usize>,
    /// Textures that could not be decoded: format code (or 0x100 for texel data missing from the image) -> count.
    pub texture_failed: BTreeMap<u32, usize>,
    /// Pixel shader definitions seen (the first 0x40 words at the pointer), by pointer.
    pub pixel_shaders: BTreeMap<u32, Vec<u32>>,
    /// Pixel shader definitions the stage 0 combiner model does not cover, by reason.
    pub pixel_shader_notes: BTreeMap<&'static str, usize>,
    /// Per frame count of draws by distinct pixel shader pointer.
    pub draws_by_pixel_shader: BTreeMap<u32, usize>,
}

impl Report {
    pub fn ignore(&mut self, name: &'static str) {
        *self.ignored_calls.entry(name).or_insert(0) += 1;
    }
    pub fn is_clean(&self) -> bool {
        self.unhandled_methods.is_empty()
            && self.unmapped_values.is_empty()
            && self.skipped_draws.is_empty()
            && self.missing_textures.is_empty()
    }
}

/// The method number of a pushbuffer header.
pub fn header_method(header: u32) -> u32 {
    header & 0x1FFC
}

impl PipelineState {
    /// Apply one method. Returns a clear request for `CLEAR_SURFACE`. `clear_colour` is the
    /// `SET_COLOR_CLEAR_VALUE` register, kept by the caller.
    pub fn apply(
        &mut self,
        header: u32,
        value: u32,
        clear_colour: &mut u32,
        report: &mut Report,
    ) -> Option<ClearRequest> {
        use method::*;
        let number = header_method(header);
        match number {
            SET_ALPHA_TEST_ENABLE => self.alpha_test = value != 0,
            SET_BLEND_ENABLE => self.blend_enable = value != 0,
            SET_CULL_FACE_ENABLE => {
                if value == 0 {
                    self.cull = None;
                } else if self.cull.is_none() {
                    self.cull = Some(CullFace::Back);
                }
            }
            SET_DEPTH_TEST_ENABLE => self.depth_test = value != 0,
            SET_POLYGON_OFFSET_POINT_ENABLE => match value {
                0 => self.polygon_offset_point = false,
                1 => self.polygon_offset_point = true,
                _ => unmapped(report, number, value),
            },
            SET_POLYGON_OFFSET_LINE_ENABLE => match value {
                0 => self.polygon_offset_line = false,
                1 => self.polygon_offset_line = true,
                _ => unmapped(report, number, value),
            },
            SET_POLYGON_OFFSET_FILL_ENABLE => match value {
                0 => self.polygon_offset_fill = false,
                1 => self.polygon_offset_fill = true,
                _ => unmapped(report, number, value),
            },
            SET_POLYGON_OFFSET_SCALE_FACTOR => {
                if f32::from_bits(value).is_finite() {
                    self.polygon_offset_scale_bits = value;
                    self.polygon_offset_scale_written = true;
                } else {
                    unmapped(report, number, value);
                }
            }
            SET_POLYGON_OFFSET_BIAS => {
                if f32::from_bits(value).is_finite() {
                    self.polygon_offset_bias_bits = value;
                    self.polygon_offset_bias_written = true;
                } else {
                    unmapped(report, number, value);
                }
            }
            SET_ALPHA_FUNC => match Compare::from_nv(value) {
                Some(func) => self.alpha_func = func,
                None => unmapped(report, number, value),
            },
            SET_ALPHA_REF => self.alpha_ref = value as u8,
            SET_BLEND_FUNC_SFACTOR => match Factor::from_nv(value) {
                Some(factor) => self.source = factor,
                None => unmapped(report, number, value),
            },
            SET_BLEND_FUNC_DFACTOR => match Factor::from_nv(value) {
                Some(factor) => self.destination = factor,
                None => unmapped(report, number, value),
            },
            SET_BLEND_COLOR => self.blend_colour = value,
            SET_BLEND_EQUATION => match Equation::from_nv(value) {
                Some(equation) => self.equation = equation,
                None => unmapped(report, number, value),
            },
            SET_DEPTH_FUNC => match Compare::from_nv(value) {
                Some(func) => self.depth_func = func,
                None => unmapped(report, number, value),
            },
            SET_COLOR_MASK => self.colour_mask = value,
            SET_DEPTH_MASK => self.depth_write = value != 0,
            SET_CULL_FACE => {
                let face = match value {
                    0x404 => Some(CullFace::Front),
                    0x405 => Some(CullFace::Back),
                    0x408 => Some(CullFace::FrontAndBack),
                    _ => None,
                };
                match face {
                    Some(face) => self.cull = Some(face),
                    None => unmapped(report, number, value),
                }
            }
            SET_FRONT_FACE => match value {
                0x900 => self.front_clockwise = true,
                0x901 => self.front_clockwise = false,
                _ => unmapped(report, number, value),
            },
            SET_COLOR_CLEAR_VALUE => *clear_colour = value,
            CLEAR_SURFACE => {
                // Bits: 0 depth, 1 stencil, 4..7 colour channels.
                return Some(ClearRequest {
                    colour: (value & 0xF0 != 0).then_some(*clear_colour),
                    depth: value & 1 != 0,
                    channels: ((value >> 4) & 0xF) as u8,
                    write_mask: u8::from(self.colour_mask & 0x0001_0000 != 0)
                        | u8::from(self.colour_mask & 0x0000_0100 != 0) << 1
                        | u8::from(self.colour_mask & 0x0000_0001 != 0) << 2
                        | u8::from(self.colour_mask & 0x0100_0000 != 0) << 3,
                });
            }
            // Methods that do not change what the replayer draws: surface setup and the
            // stencil state (not applied, the depth buffer has no stencil plane yet).
            SET_SURFACE_CLIP_HORIZONTAL | SET_SURFACE_CLIP_VERTICAL => {
                report.ignore("surface_clip")
            }
            SET_STENCIL_TEST_ENABLE
            | 0x0360
            | 0x0364
            | 0x0368
            | 0x036C
            | 0x0370
            | 0x0374
            | 0x0378 => report.ignore("stencil_state"),
            0x0100 => {}
            other => *report.unhandled_methods.entry(other).or_insert(0) += 1,
        }
        None
    }
}

fn unmapped(report: &mut Report, method: u32, value: u32) {
    *report.unmapped_values.entry((method, value)).or_insert(0) += 1;
}

/// `D3DCOLOR` style ARGB to straight RGBA floats.
pub fn argb_to_rgba(argb: u32) -> [f32; 4] {
    let channel = |shift: u32| ((argb >> shift) & 0xFF) as f32 / 255.0;
    [channel(16), channel(8), channel(0), channel(24)]
}

#[cfg(test)]
mod tests {
    use super::{PipelineState, Report, method};

    #[test]
    fn polygon_offset_methods_publish_finite_fill_state() {
        let mut state = PipelineState::default();
        let mut report = Report::default();
        let mut clear_colour = 0;

        state.apply(
            method::SET_POLYGON_OFFSET_FILL_ENABLE,
            1,
            &mut clear_colour,
            &mut report,
        );
        state.apply(
            method::SET_POLYGON_OFFSET_SCALE_FACTOR,
            1.5f32.to_bits(),
            &mut clear_colour,
            &mut report,
        );
        state.apply(
            method::SET_POLYGON_OFFSET_BIAS,
            (-12.0f32).to_bits(),
            &mut clear_colour,
            &mut report,
        );

        assert!(state.polygon_offset_fill);
        assert!(state.polygon_offset_scale_written);
        assert!(state.polygon_offset_bias_written);
        assert_eq!(state.polygon_offset_scale_bits, 1.5f32.to_bits());
        assert_eq!(state.polygon_offset_bias_bits, (-12.0f32).to_bits());

        state.apply(
            method::SET_POLYGON_OFFSET_SCALE_FACTOR,
            f32::INFINITY.to_bits(),
            &mut clear_colour,
            &mut report,
        );
        assert_eq!(state.polygon_offset_scale_bits, 1.5f32.to_bits());
        assert_eq!(
            report.unmapped_values.get(&(
                method::SET_POLYGON_OFFSET_SCALE_FACTOR,
                f32::INFINITY.to_bits()
            )),
            Some(&1)
        );
    }
}
