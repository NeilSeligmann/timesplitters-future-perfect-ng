// SPDX-License-Identifier: GPL-3.0-or-later
//! Versioned little-endian canonical encoding, independent of Rust layout.
use std::io::{self, Read, Write};
use tsfp_scene_model::*;

pub(crate) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
pub(crate) struct Decoder<R> {
    pub input: R,
    pub remaining: u64,
    pub allocations: usize,
}
impl<R: Read> Decoder<R> {
    fn bytes<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        if self.remaining < N as u64 {
            return Err(invalid("truncated canonical field"));
        }
        let mut bytes = [0; N];
        self.input.read_exact(&mut bytes)?;
        self.remaining -= N as u64;
        Ok(bytes)
    }
    fn reserve<T>(&mut self, count: usize) -> io::Result<Vec<T>> {
        // Every schema element occupies at least one byte on the wire.
        if count as u64 > self.remaining {
            return Err(invalid("element count exceeds remaining frame"));
        }
        let bytes = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| invalid("allocation overflow"))?;
        self.allocations = self
            .allocations
            .checked_sub(bytes)
            .ok_or_else(|| invalid("decoded allocation limit"))?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| invalid("decoded allocation failed"))?;
        Ok(values)
    }
    fn tag(&mut self) -> io::Result<String> {
        let count = u8::decode(self)? as usize;
        if count > 64 {
            return Err(invalid("oversized variant tag"));
        }
        let mut text = Vec::with_capacity(count);
        for _ in 0..count {
            text.push(u8::decode(self)?);
        }
        String::from_utf8(text).map_err(|_| invalid("invalid variant tag"))
    }
}
fn tag(out: &mut impl Write, text: &str) -> io::Result<()> {
    let len = u8::try_from(text.len()).map_err(|_| invalid("variant tag too long"))?;
    len.encode(out)?;
    out.write_all(text.as_bytes())
}
pub(crate) trait Codec: Sized {
    fn encode(&self, out: &mut impl Write) -> io::Result<()>;
    fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self>;
}
macro_rules! integer {
    ($($t:ty),+) => {$(impl Codec for $t {
        fn encode(&self, out: &mut impl Write) -> io::Result<()> { out.write_all(&self.to_le_bytes()) }
        fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> { Ok(Self::from_le_bytes(input.bytes()?)) }
    })+};
}
integer!(u8, u32, u64, i32);
impl Codec for bool {
    fn encode(&self, out: &mut impl Write) -> io::Result<()> {
        u8::from(*self).encode(out)
    }
    fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
        match u8::decode(input)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid("invalid boolean")),
        }
    }
}
impl Codec for f32 {
    fn encode(&self, out: &mut impl Write) -> io::Result<()> {
        self.to_bits().encode(out)
    }
    fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
        Ok(Self::from_bits(u32::decode(input)?))
    }
}
impl<T: Codec> Codec for Vec<T> {
    fn encode(&self, out: &mut impl Write) -> io::Result<()> {
        u32::try_from(self.len())
            .map_err(|_| invalid("vector length overflow"))?
            .encode(out)?;
        for v in self {
            v.encode(out)?;
        }
        Ok(())
    }
    fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
        let count = u32::decode(input)? as usize;
        let mut values = input.reserve::<T>(count)?;
        for _ in 0..count {
            values.push(T::decode(input)?);
        }
        Ok(values)
    }
}
impl<T: Codec, const N: usize> Codec for [T; N] {
    fn encode(&self, out: &mut impl Write) -> io::Result<()> {
        for v in self {
            v.encode(out)?;
        }
        Ok(())
    }
    fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
        // Arrays are small fixed schema fields; preserve the error without unsafe initialization.
        let mut values = Vec::with_capacity(N);
        for _ in 0..N {
            values.push(T::decode(input)?);
        }
        values.try_into().map_err(|_| invalid("array length"))
    }
}
impl<T: Codec> Codec for Option<T> {
    fn encode(&self, out: &mut impl Write) -> io::Result<()> {
        self.is_some().encode(out)?;
        if let Some(value) = self {
            value.encode(out)?;
        }
        Ok(())
    }
    fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
        if bool::decode(input)? {
            Ok(Some(T::decode(input)?))
        } else {
            Ok(None)
        }
    }
}
macro_rules! record {
    ($t:ident { $($f:ident),+ $(,)? }) => {impl Codec for $t {
        fn encode(&self, out: &mut impl Write) -> io::Result<()> {
            let Self { $($f),+ } = self;
            $($f.encode(out)?;)+
            Ok(())
        }
        fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
            Ok(Self { $($f: Codec::decode(input)?),+ })
        }
    }};
}
macro_rules! wrapper {
    ($($t:ident),+ $(,)?) => {$(impl Codec for $t {
        fn encode(&self, out: &mut impl Write) -> io::Result<()> { let Self(v) = self; v.encode(out) }
        fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> { Ok(Self(Codec::decode(input)?)) }
    })+};
}
macro_rules! enumeration {
    ($t:ident { $($v:ident),+ $(,)? }) => {impl Codec for $t {
        fn encode(&self, out: &mut impl Write) -> io::Result<()> {
            match self { $(Self::$v => tag(out, stringify!($v))),+ }
        }
        fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
            match input.tag()?.as_str() { $(stringify!($v) => Ok(Self::$v)),+, _ => Err(invalid(concat!("unknown ", stringify!($t)))) }
        }
    }};
}
macro_rules! tagged {
    ($t:ident { $($v:ident $( { $($f:ident),+ } )? ),+ $(,)? }) => {impl Codec for $t {
        fn encode(&self, out: &mut impl Write) -> io::Result<()> {
            match self { $(Self::$v $( { $($f),+ } )? => {
                tag(out, stringify!($v))?;
                $($($f.encode(out)?;)+)?
            }),+ }
            Ok(())
        }
        fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
            match input.tag()?.as_str() { $(stringify!($v) => Ok(Self::$v $( { $($f: Codec::decode(input)?),+ } )?)),+,
                _ => Err(invalid(concat!("unknown ", stringify!($t)))) }
        }
    }};
}
wrapper!(
    ContentId,
    MeshRef,
    ShaderRef,
    TextureRef,
    CameraId,
    InstanceId,
    MaterialId,
    LightId,
    EffectId,
    ParticleId,
    RenderTargetId
);
record!(Lifetime { slot, generation });
record!(Rect {
    x,
    y,
    width,
    height
});
record!(Camera {
    id,
    view,
    projection,
    position,
    viewport,
    depth_range,
    cut
});
record!(ModelInstance {
    id,
    mesh,
    material,
    transform,
    skin_palette,
    discontinuity
});
record!(TextureBinding {
    slot,
    texture,
    sampler,
    uv_set,
    uv_transform
});
record!(RenderTarget {
    id,
    extent,
    colour,
    depth
});
record!(Material {
    id,
    shader,
    textures,
    parameters,
    raster
});
record!(Light {
    id,
    kind,
    diffuse,
    specular
});
record!(Particle {
    id,
    transform,
    colour,
    uv_rect,
    discontinuity
});
record!(EffectBatch {
    id,
    mesh,
    material,
    particles
});
record!(BlendComponent {
    source,
    destination,
    operation
});
record!(Blend {
    colour,
    alpha,
    constant
});
record!(StencilFace {
    compare,
    fail,
    depth_fail,
    pass
});
record!(Stencil {
    front,
    back,
    reference,
    read_mask,
    write_mask
});
record!(RasterState {
    cull,
    front_face,
    blend,
    depth_compare,
    depth_write,
    depth_bias_constant,
    depth_bias_slope,
    stencil,
    colour_write
});
record!(Sampler {
    min,
    mag,
    mip,
    address,
    border,
    lod_min,
    lod_max,
    lod_bias
});
record!(FrameSample {
    previous_tick,
    current_tick,
    sampling
});
record!(DrawPass {
    camera,
    target,
    extent,
    draws
});
record!(DrawList {
    sample,
    targets,
    cameras,
    models,
    materials,
    lights,
    effects,
    passes
});
enumeration!(ColourFormat {
    Rgba8Unorm,
    Rgba8Srgb,
    Rgba16Float
});
enumeration!(DepthFormat {
    Depth16,
    Depth24Stencil8,
    Depth32Float
});
enumeration!(Compare {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always
});
enumeration!(BlendFactor {
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
    OneMinusConstant
});
enumeration!(BlendOperation {
    Add,
    Subtract,
    ReverseSubtract,
    Min,
    Max
});
enumeration!(StencilOperation {
    Keep,
    Zero,
    Replace,
    IncrementClamp,
    DecrementClamp,
    Invert,
    IncrementWrap,
    DecrementWrap
});
enumeration!(Cull { None, Front, Back });
enumeration!(Winding {
    Clockwise,
    CounterClockwise
});
enumeration!(Filter { Nearest, Linear });
enumeration!(AddressMode {
    Clamp,
    Repeat,
    Mirror,
    Border
});
tagged!(Sampling { Original, Interpolated { alpha } });
tagged!(LightKind { Ambient, Directional { direction }, Point { position, range, attenuation },
    Spot { position, direction, range, attenuation, inner_cos, outer_cos, falloff } });
tagged!(Draw { Clear { colour, depth, stencil }, Mesh { instance, lights, scissor },
    Particles { batch, lights, scissor } });

impl Codec for Target {
    fn encode(&self, out: &mut impl Write) -> io::Result<()> {
        match self {
            Self::Surface => tag(out, "Surface"),
            Self::Offscreen(v) => {
                tag(out, "Offscreen")?;
                v.encode(out)
            }
        }
    }
    fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
        match input.tag()?.as_str() {
            "Surface" => Ok(Self::Surface),
            "Offscreen" => Ok(Self::Offscreen(Codec::decode(input)?)),
            _ => Err(invalid("unknown Target")),
        }
    }
}
impl Codec for TextureSource {
    fn encode(&self, out: &mut impl Write) -> io::Result<()> {
        match self {
            Self::Asset(v) => {
                tag(out, "Asset")?;
                v.encode(out)
            }
            Self::RenderTarget(v) => {
                tag(out, "RenderTarget")?;
                v.encode(out)
            }
        }
    }
    fn decode<R: Read>(input: &mut Decoder<R>) -> io::Result<Self> {
        match input.tag()?.as_str() {
            "Asset" => Ok(Self::Asset(Codec::decode(input)?)),
            "RenderTarget" => Ok(Self::RenderTarget(Codec::decode(input)?)),
            _ => Err(invalid("unknown TextureSource")),
        }
    }
}
use crate::{Capture, Frame, Gaps};
record!(Capture {
    contract_version,
    decoder_version,
    original_revision,
    original_prebuilt,
    route,
    input,
    stream,
    resources,
    first_present,
    frame_count
});
record!(Gaps {
    dropped_records,
    missing_resources,
    unsupported_states,
    unmatched_views
});
record!(Frame {
    present,
    gaps,
    draws
});
