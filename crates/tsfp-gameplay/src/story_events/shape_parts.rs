//! T1855: the seven helper functions of the kind 8 shape scan `0xD9760` that are not part of the oriented shape chain
//! (MEASURED: bit for bit against the original machine code under the Unicorn oracle, see
//! `tests/story_events_t1855_shape_parts.rs` and `docs/t-story-event-shape-scan-parts-t1855.md`).
//!
//! | original | ABI | role (names INFERRED) |
//! |----------|-----|------------------------|
//! | `0x475C0` | cdecl `(a, b, c, d)` float vector pointers, returns eax 0/1 | ray against an axis aligned box, cross-multiplied slab test |
//! | `0x49200` | cdecl `(origin, dir, centre, r2, out_point, out_offset)`, eax 0/1 | ray against a sphere (link type `0x46`) |
//! | `0xD6AA0` | registers `eax` object, `ecx` matrix base, `edx` point, `esi` mode; stack `(first, second)` | transform a point by a per bone matrix |
//! | `0xD9510` | cdecl `(link, out_top, out_bottom, out_radius)` | end points and radius of a link record's capsule |
//! | `0x12820` | cdecl `(shape, mesh, face)`, returns pointer | address of a face vertex slot |
//! | `0x12A90` | cdecl `(out, shape, src)` | decode a vertex (compressed or float) |
//! | `0x12BA0` | cdecl `(shape, mesh, face)`, returns pointer or 0 | address of the second per vertex stream |
//!
//! Arithmetic follows the image: SSE operations are plain f32 in listing order (no fused multiply-add). `0x49200`
//! uses x87 `fsqrt`/`fsub` at the title's 53 bit precision control, so they are f64 operations rounded once to f32.
//! NaN operand payload selection of SSE (first operand wins) is not modelled; only finite inputs are verified.
//!
//! Record layouts decoded here:
//! - shape record (the object a link points at, `shape`): flags dword at `+0x0C` (bit `0x10` = compressed
//!   vertices), vertex bias `float[3]` at `+0x38`.
//! - mesh record: stream pointer at `+0x04` (vertex slots, 8 bytes compressed or 16 bytes float; the second stream
//!   lives 2 bytes (compressed) or 12 bytes (float) into each slot).
//! - face record: vertex slot index `u16` at `+0x06`.
//! - link record (`0xD9510`): object pointer at `+0x00`, type index `i16` at `+0x48`, base position `float[3]` at
//!   `+0xBB4`.
//! - type table at `0x53A3E4`, 0x30 bytes per entry (filled at runtime): `+0x00` half length, `+0x08` radius bits.
//! - object (`0xD9510` second path): kind byte `+0x32`, position `+0x64..`, offset `+0x1A8..`, extents `+0x1AC` and
//!   `+0x1C4`.
//! - per bone matrix array (`0xD6AA0`): 64 byte matrices indexed by the signed byte at `object+0x49`; `object+0x48`
//!   non zero selects the mode dispatch.

// `!(a >= b)` keeps the unordered (NaN) behaviour of the original comiss + jb.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::sweep_records::GuestMemory;
use super::sweep_shapes::transform_point;
use super::world_queries::Mat4;

/// `[0x475C9C]` = `0x3A800000` = 1/1024: scale of a compressed vertex component.
pub const VERTEX_SCALE: f32 = f32::from_bits(0x3A80_0000);
/// `[0x475D5C]` = `0x7F7FFFFF` = `f32::MAX`: initial best ray parameter of `0x49200`.
pub const RAY_PARAMETER_INIT: f32 = f32::from_bits(0x7F7F_FFFF);
/// `[0x475CD4]` = 0.5.
const HALF: f32 = 0.5;
/// `[0x4785E0]` = `0x3F333333` (0.7): threshold of the extent test in `0xD9510`.
const EXTENT_THRESHOLD: f32 = f32::from_bits(0x3F33_3333);
/// `[0x4785E4]` = `0x3E4CCCCD` (0.2).
const EXTENT_FACTOR: f32 = f32::from_bits(0x3E4C_CCCD);
/// Runtime type table of `0xD9510`.
pub const TYPE_TABLE: u32 = 0x53_A3E4;

// ---------------------------------------------------------------------------------------------------------------
// 0x475C0
// ---------------------------------------------------------------------------------------------------------------

/// Exit of [`ray_vs_box`]. Everything but `Hit` returns 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RayBoxExit {
    /// Both x distances negative (`0x47798` via `0x47629`).
    MissX,
    /// Both y distances negative (`0x47674`).
    MissY,
    /// Cross product test of the x/y slabs, branch where `ab_y*lo_x > lo_y*ab_x` (`0x476CC`).
    MissXyGreater,
    /// The same, other branch (`0x476E2`).
    MissXyLess,
    /// Both z distances negative (`0x4773F`).
    MissZ,
    /// z slab against the carried x/y bound, first form (`0x47788`).
    MissZCombinedHigh,
    /// z slab against the carried x/y bound, second form (`0x47796`).
    MissZCombinedLow,
    Hit,
}

impl RayBoxExit {
    /// eax of the original.
    pub fn hit(self) -> bool {
        self == RayBoxExit::Hit
    }
}

/// `0x475C0(a, b, c, d)`: `a` origin, `b` direction, `c` and `d` the two box corners (their order does not matter, the
/// per axis distances are sorted). Per axis with `b < 0` the distances are `a - d`, `a - c` and `|b| = 0 - b`,
/// otherwise `d - a`, `c - a`, `b`. A miss is a pair of negative distances on x, y or z, or a failed cross
/// multiplied comparison of the sorted slab entries (no division). MEASURED.
pub fn ray_vs_box(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) -> RayBoxExit {
    let zero = 0.0f32;
    // Per axis (u from d, v from c, |b|).
    let axis = |k: usize| -> (f32, f32, f32) {
        if zero > b[k] {
            (a[k] - d[k], a[k] - c[k], zero - b[k])
        } else {
            (d[k] - a[k], c[k] - a[k], b[k])
        }
    };
    let (u_x, v_x, ab_x) = axis(0);
    if zero > v_x && zero > u_x {
        return RayBoxExit::MissX;
    }
    let (u_y, v_y, ab_y) = axis(1);
    if zero > v_y && zero > u_y {
        return RayBoxExit::MissY;
    }
    let (lo_x, hi_x) = if v_x > u_x { (u_x, v_x) } else { (v_x, u_x) };
    let (lo_y, hi_y) = if v_y > u_y { (u_y, v_y) } else { (v_y, u_y) };
    let ab_y_hi_x = ab_y * hi_x;
    let ab_y_lo_x = ab_y * lo_x;
    let lo_y_ab_x = lo_y * ab_x;
    let hi_y_ab_x = hi_y * ab_x;
    let (carried, carried_ab) = if ab_y_lo_x > lo_y_ab_x {
        if ab_y_lo_x > hi_y_ab_x {
            return RayBoxExit::MissXyGreater;
        }
        (lo_x, ab_x)
    } else {
        if lo_y_ab_x > ab_y_hi_x {
            return RayBoxExit::MissXyLess;
        }
        (lo_y, ab_y)
    };
    let (u_z, v_z, ab_z) = axis(2);
    if zero > v_z && zero > u_z {
        return RayBoxExit::MissZ;
    }
    let (lo_z, hi_z) = if v_z > u_z { (u_z, v_z) } else { (v_z, u_z) };
    let carried_ab_z = ab_z * carried;
    let lo_z_carried_ab = lo_z * carried_ab;
    if lo_z_carried_ab > carried_ab_z {
        let (bound, bound_ab) = if hi_y_ab_x > ab_y_hi_x {
            (hi_x, ab_x)
        } else {
            (hi_y, ab_y)
        };
        if bound_ab * lo_z > bound * ab_z {
            RayBoxExit::MissZCombinedHigh
        } else {
            RayBoxExit::Hit
        }
    } else if carried_ab_z > hi_z * carried_ab {
        RayBoxExit::MissZCombinedLow
    } else {
        RayBoxExit::Hit
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x49200
// ---------------------------------------------------------------------------------------------------------------

/// Hit of [`ray_vs_sphere`]: the original stores `point` through its fifth argument and `offset` through its sixth
/// (each only when the pointer is non null).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaySphereHit {
    /// `origin + t * dir`.
    pub point: [f32; 3],
    /// `point - centre`.
    pub offset: [f32; 3],
    /// Chosen ray parameter `t`.
    pub parameter: f32,
}

/// `0x49200(origin, dir, centre, radius_squared, out_point, out_offset)`: smallest non negative root of
/// `|origin + t dir - centre|^2 = r2`, using `b = dir . w`, `c = |w|^2 - r2`, `disc = b b - c`. No hit when
/// `disc < 0` or both roots are negative. The square root and the second root `s - b` are x87 operations at 53 bit
/// precision (f64, rounded to f32 at the store). MEASURED.
pub fn ray_vs_sphere(
    origin: [f32; 3],
    dir: [f32; 3],
    centre: [f32; 3],
    radius_squared: f32,
) -> Option<RaySphereHit> {
    let wz = origin[2] - centre[2];
    let wx = origin[0] - centre[0];
    let wy = origin[1] - centre[1];
    let b = (dir[2] * wz + dir[0] * wx) + wy * dir[1];
    let c = ((wz * wz + wy * wy) + wx * wx) - radius_squared;
    let disc = b * b - c;
    if !(disc >= 0.0) {
        return None;
    }
    let s = (f64::from(disc).sqrt()) as f32;
    let first = (0.0f32 - b) - s;
    let second = (f64::from(s) - f64::from(b)) as f32;
    let mut best = RAY_PARAMETER_INIT;
    let mut found = false;
    if first >= 0.0 && best > first {
        found = true;
        best = first;
    }
    if second >= 0.0 && best > second {
        best = second;
    } else if !found {
        return None;
    }
    let point = [
        dir[0] * best + origin[0],
        best * dir[1] + origin[1],
        dir[2] * best + origin[2],
    ];
    Some(RaySphereHit {
        point,
        offset: [
            point[0] - centre[0],
            point[1] - centre[1],
            point[2] - centre[2],
        ],
        parameter: best,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// 0xD6AA0
// ---------------------------------------------------------------------------------------------------------------

/// `0xD6AA0` (register ABI: `eax` = `object`, `ecx` = `matrices`, `edx` = point, `esi` = `mode`, two stack arguments
/// `first` and `second` that are only used as object pointers): transforms the point (in place in the original, with
/// `0x151E80`, the fourth word is untouched) by the matrix `matrices + 64 * (i8) owner[+0x49]`, where `owner` is
/// - `object` when `object[+0x48] == 0` (mode ignored),
/// - otherwise by `mode`: 0 -> `first`, 8 -> `second`, 0x10 -> `object`, anything else: nothing is transformed
///   (returns `None`).
///
/// The two stack slots are overwritten by the tail call in the original, the caller cleans them up. MEASURED.
pub fn transform_by_bone_matrix(
    memory: &dyn GuestMemory,
    object: u32,
    first: u32,
    second: u32,
    mode: u32,
    matrices: u32,
    point: [f32; 3],
) -> Option<[f32; 3]> {
    let owner = if memory.read_u8(object.wrapping_add(0x48)) == 0 {
        object
    } else {
        match mode {
            0 => first,
            8 => second,
            0x10 => object,
            _ => return None,
        }
    };
    let index = i32::from(memory.read_u8(owner.wrapping_add(0x49)) as i8);
    let address = matrices.wrapping_add((index << 6) as u32);
    let mut matrix: Mat4 = [0.0; 16];
    for (slot, value) in matrix.iter_mut().enumerate() {
        *value = memory.read_f32(address.wrapping_add(4 * slot as u32));
    }
    Some(transform_point(&matrix, point))
}

// ---------------------------------------------------------------------------------------------------------------
// 0xD9510
// ---------------------------------------------------------------------------------------------------------------

/// Results of [`link_shape_ends`] (stored through the second, third and fourth argument; the original skips a store
/// when the pointer is null, except for the first two in the object kind 2 path where they must be valid).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkEnds {
    /// Second argument.
    pub upper: [f32; 3],
    /// Third argument.
    pub lower: [f32; 3],
    /// Fourth argument: raw dword `[type_table + 0x30 * type + 8]` (a float, copied as bits).
    pub radius_bits: u32,
}

/// Which path `0xD9510` took.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkEndsPath {
    /// Object kind 2: oriented by the world matrix `0xA14B0(out, object, 1, 0)`.
    Oriented,
    /// Other kinds, `|y - lo| < 0.7`: ends straight above and at the position.
    Short,
    /// Other kinds, extent at least 0.7.
    Tall,
}

/// `0xD9510(link, out_upper, out_lower, out_radius)`. `world_matrix(object)` stands for `0xA14B0(out, object, 1, 0)`
/// (only called for kind 2) and returns the 16 floats; only the second row (`[4..7]`, the up axis) is used.
/// With `T = [TYPE_TABLE + 0x30 * (i16) link[+0x48]]` and the `float[3]` base at `link[+0xBB4]`:
/// - kind 2: `upper = base + 0.5 * up`, `lower = upper - T * up`.
/// - otherwise (object fields: position `p` at `+0x64`, offset `o` at `+0x1A8`, extents `h1 = +0x1AC`,
///   `h2 = +0x1C4`; `lo = (h1 + p.y) - h2`, `hi = (h1 + h2) + p.y`, `d = hi - lo`): with `|p.y - lo| < 0.7` upper =
///   `(p.x, T + p.y, p.z)`, lower = `p`; else upper = `(o.x + p.x, hi - 0.2 d, o.z + p.z)`, lower =
///   `(o.x + p.x, lo, o.z + p.z)`. (`h1` stands for the float at `+0x1AC`, `h2` for `+0x1C4`.)
///
/// MEASURED.
pub fn link_shape_ends(
    memory: &dyn GuestMemory,
    link: u32,
    world_matrix: &mut dyn FnMut(u32) -> Mat4,
) -> (LinkEnds, LinkEndsPath) {
    let object = memory.read_u32(link);
    let type_index = i32::from(memory.read_u16(link.wrapping_add(0x48)) as i16);
    let entry = TYPE_TABLE.wrapping_add((type_index * 48) as u32);
    let half_length = memory.read_f32(entry);
    let radius_bits = memory.read_u32(entry.wrapping_add(8));
    if memory.read_u8(object.wrapping_add(0x32)) == 2 {
        let matrix = world_matrix(object);
        let (m4, m5, m6) = (matrix[4], matrix[5], matrix[6]);
        let base = [
            memory.read_u32(link.wrapping_add(0xBB4)),
            memory.read_u32(link.wrapping_add(0xBB8)),
            memory.read_u32(link.wrapping_add(0xBBC)),
        ]
        .map(f32::from_bits);
        let upper = [
            m4 * HALF + base[0],
            m5 * HALF + base[1],
            m6 * HALF + base[2],
        ];
        let lower = [
            upper[0] - half_length * m4,
            upper[1] - half_length * m5,
            upper[2] - half_length * m6,
        ];
        return (
            LinkEnds {
                upper,
                lower,
                radius_bits,
            },
            LinkEndsPath::Oriented,
        );
    }
    let field = |offset: u32| memory.read_f32(object.wrapping_add(offset));
    let (x, y, z) = (field(0x64), field(0x68), field(0x6C));
    let extent = field(0x1AC);
    let lo = (extent + y) - field(0x1C4);
    let hi = (extent + field(0x1C4)) + y;
    let spread = hi - lo;
    if EXTENT_THRESHOLD > (y - lo).abs() {
        let ends = LinkEnds {
            upper: [x, half_length + y, z],
            lower: [x, y, z],
            radius_bits,
        };
        return (ends, LinkEndsPath::Short);
    }
    let sx = field(0x1A8) + x;
    let sz = field(0x1B0) + z;
    let ends = LinkEnds {
        upper: [sx, hi - spread * EXTENT_FACTOR, sz],
        lower: [sx, lo, sz],
        radius_bits,
    };
    (ends, LinkEndsPath::Tall)
}

// ---------------------------------------------------------------------------------------------------------------
// 0x12820, 0x12A90, 0x12BA0
// ---------------------------------------------------------------------------------------------------------------

const COMPRESSED_FLAG: u32 = 0x10;

fn compressed(memory: &dyn GuestMemory, shape: u32) -> bool {
    memory.read_u32(shape.wrapping_add(0xC)) & COMPRESSED_FLAG != 0
}

/// `0x12820(shape, mesh, face)`: address of the vertex slot `mesh[+4] + face.u16[+6] * (8 | 16)` (8 bytes when the
/// shape is compressed). No null check. MEASURED.
pub fn vertex_slot_address(memory: &dyn GuestMemory, shape: u32, mesh: u32, face: u32) -> u32 {
    let stride = if compressed(memory, shape) { 8 } else { 16 };
    let slot = u32::from(memory.read_u16(face.wrapping_add(6)));
    memory
        .read_u32(mesh.wrapping_add(4))
        .wrapping_add(slot * stride)
}

/// `0x12A90(out, shape, src)`: the four words stored at `out`. Compressed shapes: `x = i16[0] * (1/1024) +
/// shape[+0x38]`, `y` with `[+2]`/`[+0x3C]`, `z` with `[+4]`/`[+0x40]` (convert, multiply, add, in that order), and
/// the fourth word is `i16[6]` sign extended to 32 bits OR `0x3F800000` (so a plain 1.0 for small non negative
/// values). Float shapes copy the four dwords. MEASURED.
pub fn decode_vertex(memory: &dyn GuestMemory, shape: u32, src: u32) -> [u32; 4] {
    if !compressed(memory, shape) {
        return [0, 4, 8, 12].map(|offset| memory.read_u32(src.wrapping_add(offset)));
    }
    let component = |index: u32| {
        let raw = f32::from(memory.read_u16(src.wrapping_add(2 * index)) as i16);
        (raw * VERTEX_SCALE + memory.read_f32(shape.wrapping_add(0x38 + 4 * index))).to_bits()
    };
    let w = i32::from(memory.read_u16(src.wrapping_add(6)) as i16) as u32;
    [component(0), component(1), component(2), w | 0x3F80_0000]
}

/// `0x12BA0(shape, mesh, face)`: address of the second stream entry, `mesh[+4] + slot * 8 + 2` (compressed) or
/// `mesh[+4] + slot * 16 + 12`; 0 when `mesh[+4]` is null. MEASURED.
pub fn second_stream_address(memory: &dyn GuestMemory, shape: u32, mesh: u32, face: u32) -> u32 {
    let base = memory.read_u32(mesh.wrapping_add(4));
    if base == 0 {
        return 0;
    }
    let slot = u32::from(memory.read_u16(face.wrapping_add(6)));
    if compressed(memory, shape) {
        base.wrapping_add(slot * 8 + 2)
    } else {
        base.wrapping_add(slot * 16 + 12)
    }
}
