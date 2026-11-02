// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{RasterState, Sampler};

/// SHA-256 of an immutable resource in the loader's versioned canonical format.
/// Content includes layout, topology/format and payload, not just raw bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentId(pub [u8; 32]);

macro_rules! asset_refs {
    ($($name:ident),+) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub ContentId);
    )+};
}
asset_refs!(MeshRef, TextureRef, ShaderRef);

/// Rust owner-issued lifetime identity. Generation zero is invalid; reuse of a
/// slot always increments generation. Resource content identities are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lifetime {
    pub slot: u64,
    pub generation: u64,
}
macro_rules! owner_ids {
    ($($name:ident),+) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub Lifetime);
    )+};
}
owner_ids!(
    CameraId,
    InstanceId,
    MaterialId,
    LightId,
    EffectId,
    ParticleId,
    RenderTargetId
);

pub type Matrix4 = [[f32; 4]; 4];
pub const IDENTITY: Matrix4 = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

/// Pixel rectangle, half-open on right/bottom. Extents must not overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    pub id: CameraId,
    pub view: Matrix4,
    pub projection: Matrix4,
    pub position: [f32; 3],
    pub viewport: Rect,
    pub depth_range: [f32; 2],
    /// Current endpoint is used for every sample across a camera cut.
    pub cut: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelInstance {
    pub id: InstanceId,
    /// One immutable primitive group, including topology, vertex layout,
    /// index range and bind-pose geometry in the resource manifest.
    pub mesh: MeshRef,
    pub material: MaterialId,
    pub transform: Matrix4,
    /// Joint matrices in mesh joint order, empty for an unskinned mesh.
    pub skin_palette: Vec<Matrix4>,
    /// Spawn/teleport/owner changes snap to current, never interpolate IDs.
    pub discontinuity: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextureBinding {
    pub slot: u8,
    pub texture: TextureSource,
    pub sampler: Sampler,
    pub uv_set: u8,
    pub uv_transform: [[f32; 3]; 3],
}

/// Mutable render targets use lifetime identities, not immutable content hashes.
/// A pass may sample a previously written target, never its active attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureSource {
    Asset(TextureRef),
    RenderTarget(RenderTargetId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColourFormat {
    Rgba8Unorm,
    Rgba8Srgb,
    Rgba16Float,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepthFormat {
    Depth16,
    Depth24Stencil8,
    Depth32Float,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderTarget {
    pub id: RenderTargetId,
    pub extent: [u32; 2],
    pub colour: ColourFormat,
    pub depth: Option<DepthFormat>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub id: MaterialId,
    /// Native shader recipe, including alpha test/fog/combiner semantics and
    /// parameter layout. This is not NV2A microcode or a D3D handle. The
    /// materials lane supplies faithful recipes; unsupported recipes fail.
    pub shader: ShaderRef,
    pub textures: Vec<TextureBinding>,
    pub parameters: Vec<[f32; 4]>,
    pub raster: RasterState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LightKind {
    Ambient,
    Directional {
        direction: [f32; 3],
    },
    Point {
        position: [f32; 3],
        range: f32,
        attenuation: [f32; 3],
    },
    Spot {
        position: [f32; 3],
        direction: [f32; 3],
        range: f32,
        attenuation: [f32; 3],
        inner_cos: f32,
        outer_cos: f32,
        falloff: f32,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    pub id: LightId,
    pub kind: LightKind,
    pub diffuse: [f32; 4],
    pub specular: [f32; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    pub id: ParticleId,
    pub transform: Matrix4,
    pub colour: [f32; 4],
    pub uv_rect: [f32; 4],
    pub discontinuity: bool,
}
/// Already ordered particles; no backend sorting or simulation/RNG updates.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectBatch {
    pub id: EffectId,
    pub mesh: MeshRef,
    pub material: MaterialId,
    pub particles: Vec<Particle>,
}
