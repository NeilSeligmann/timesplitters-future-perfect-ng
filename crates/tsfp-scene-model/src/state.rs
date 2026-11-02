// SPDX-License-Identifier: GPL-3.0-or-later
//! Backend-neutral original-preset raster state; no D3D/NV2A numeric slots.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendFactor {
    Zero,
    One,
    SourceColour,
    OneMinusSourceColour,
    SourceAlpha,
    OneMinusSourceAlpha,
    DestinationColour,
    OneMinusDestinationColour,
    DestinationAlpha,
    OneMinusDestinationAlpha,
    SourceAlphaSaturated,
    Constant,
    OneMinusConstant,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendComponent {
    pub source: BlendFactor,
    pub destination: BlendFactor,
    pub operation: BlendOperation,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blend {
    pub colour: BlendComponent,
    pub alpha: BlendComponent,
    pub constant: [f32; 4],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StencilOperation {
    Keep,
    Zero,
    Replace,
    IncrementClamp,
    DecrementClamp,
    Invert,
    IncrementWrap,
    DecrementWrap,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StencilFace {
    pub compare: Compare,
    pub fail: StencilOperation,
    pub depth_fail: StencilOperation,
    pub pass: StencilOperation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stencil {
    pub front: StencilFace,
    pub back: StencilFace,
    pub reference: u8,
    pub read_mask: u8,
    pub write_mask: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cull {
    None,
    Front,
    Back,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Winding {
    Clockwise,
    CounterClockwise,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterState {
    pub cull: Cull,
    pub front_face: Winding,
    /// None disables blending. The shader recipe owns any alpha-test function.
    pub blend: Option<Blend>,
    /// None disables depth testing; the backend must also disable depth writes.
    pub depth_compare: Option<Compare>,
    pub depth_write: bool,
    pub depth_bias_constant: i32,
    pub depth_bias_slope: f32,
    pub stencil: Option<Stencil>,
    /// RGBA write enables, in that order.
    pub colour_write: [bool; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Nearest,
    Linear,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressMode {
    Clamp,
    Repeat,
    Mirror,
    Border,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sampler {
    pub min: Filter,
    pub mag: Filter,
    pub mip: Filter,
    pub address: [AddressMode; 3],
    pub border: [f32; 4],
    pub lod_min: f32,
    pub lod_max: f32,
    pub lod_bias: f32,
}
