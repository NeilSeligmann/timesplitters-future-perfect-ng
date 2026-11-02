// SPDX-License-Identifier: GPL-3.0-or-later
//! Exhaustive schema walk: float bits, never derived float equality or Debug serialization.
use crate::Failure;
use tsfp_scene_model::*;

pub(crate) struct Budget(pub usize);
impl Budget {
    fn visit(&mut self, path: &str) -> Result<(), Failure> {
        if self.0 == 0 {
            return Err(Failure::Limit { field: path.into() });
        }
        self.0 -= 1;
        Ok(())
    }
}
pub(crate) trait Field {
    fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure>;
}
fn different(path: &str, expected: impl std::fmt::Debug, actual: impl std::fmt::Debug) -> Failure {
    Failure::Different {
        field: path.into(),
        oracle: format!("{expected:?}"),
        native: format!("{actual:?}"),
    }
}
macro_rules! scalar {
    ($($t:ty),+ $(,)?) => {$(impl Field for $t {
        fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
            budget.visit(path)?;
            if self == other { Ok(()) } else { Err(different(path, self, other)) }
        }
    })+};
}
scalar!(u8, u32, u64, i32, bool, usize);
impl Field for f32 {
    fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
        self.to_bits().compare(&other.to_bits(), path, budget)
    }
}
impl<T: Field, const N: usize> Field for [T; N] {
    fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
        for (i, (a, b)) in self.iter().zip(other).enumerate() {
            a.compare(b, &format!("{path}[{i}]"), budget)?;
        }
        Ok(())
    }
}
impl<T: Field> Field for Vec<T> {
    fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
        self.len()
            .compare(&other.len(), &format!("{path}.len"), budget)?;
        for (i, (a, b)) in self.iter().zip(other).enumerate() {
            budget.visit(path)?;
            a.compare(b, &format!("{path}[{i}]"), budget)?;
        }
        Ok(())
    }
}
impl<T: Field> Field for Option<T> {
    fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
        budget.visit(path)?;
        match (self, other) {
            (Some(a), Some(b)) => a.compare(b, path, budget),
            (None, None) => Ok(()),
            _ => Err(different(path, self.is_some(), other.is_some())),
        }
    }
}
// Destructure without `..`: a contract field addition must update this walker.
macro_rules! record {
    ($t:ident { $($f:ident),+ $(,)? }) => {impl Field for $t {
        fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
            let Self { $($f: _),+ } = self;
            $(self.$f.compare(&other.$f, &format!("{path}.{}", stringify!($f)), budget)?;)+
            Ok(())
        }
    }};
}
macro_rules! wrapper {
    ($($t:ident),+ $(,)?) => {$(impl Field for $t {
        fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
            let Self(value) = self;
            value.compare(&other.0, path, budget)
        }
    })+};
}
// Enumerated explicitly so a newly added enum variant cannot silently compare equal.
macro_rules! enumeration {
    ($t:ident { $($v:ident),+ $(,)? }) => {impl Field for $t {
        fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
            fn tag(v: &$t) -> &'static str { match v { $($t::$v => stringify!($v)),+ } }
            budget.visit(path)?;
            if tag(self) == tag(other) { Ok(()) } else { Err(different(path, tag(self), tag(other))) }
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

// Each enum is decomposed into a variant marker and its complete payload.
macro_rules! tagged {
    ($t:ident { $($v:ident $( { $($f:ident),+ } )? ),+ $(,)? }) => {
        impl Field for $t {
            fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
                fn tag(value: &$t) -> &'static str { match value {
                    $($t::$v $( { $($f: _),+ } )? => stringify!($v)),+
                }}
                budget.visit(path)?;
                if tag(self) != tag(other) { return Err(different(path, tag(self), tag(other))); }
                match self {
                    $($t::$v $( { $($f),+ } )? => {
                        $( $( { let a = $f;
                            if let $t::$v { $f: b, .. } = other {
                                a.compare(b, &format!("{path}.{}", stringify!($f)), budget)?;
                            }
                        } )+ )?
                    }),+
                }
                Ok(())
            }
        }
    };
}
tagged!(Sampling { Original, Interpolated { alpha } });
tagged!(LightKind { Ambient, Directional { direction }, Point { position, range, attenuation },
    Spot { position, direction, range, attenuation, inner_cos, outer_cos, falloff } });
tagged!(Draw { Clear { colour, depth, stencil }, Mesh { instance, lights, scissor },
    Particles { batch, lights, scissor } });

impl Field for Target {
    fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
        budget.visit(path)?;
        match self {
            Self::Surface => match other {
                Self::Surface => Ok(()),
                _ => Err(different(path, self, other)),
            },
            Self::Offscreen(a) => match other {
                Self::Offscreen(b) => a.compare(b, path, budget),
                _ => Err(different(path, self, other)),
            },
        }
    }
}
impl Field for TextureSource {
    fn compare(&self, other: &Self, path: &str, budget: &mut Budget) -> Result<(), Failure> {
        budget.visit(path)?;
        match self {
            Self::Asset(a) => match other {
                Self::Asset(b) => a.compare(b, path, budget),
                _ => Err(different(path, self, other)),
            },
            Self::RenderTarget(a) => match other {
                Self::RenderTarget(b) => a.compare(b, path, budget),
                _ => Err(different(path, self, other)),
            },
        }
    }
}
