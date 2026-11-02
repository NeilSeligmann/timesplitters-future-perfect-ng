//! T1855: the oriented shape test of the kind 8 shape scan `0xD9760` (MEASURED against the original machine code
//! under the Unicorn oracle, see `tests/story_events_t1855_shape_obb.rs` and docs/t-story-event-shape-scan-obb-t1855.md).
//!
//! | original | instr | role (INFERRED name) |
//! |----------|-------|----------------------|
//! | `0x48FE0` | 153 | [`shape_hit_48fe0`]: segment-with-radius shape against a swept ray, nearest contact |
//! | `0x486C0` | 565 | [`cylinder_ray_486c0`]: ray against a finite cylinder with flat end caps |
//! | `0x48590` | 69 | [`radial_normal_48590`]: contact normal, the part of `to - from` in the cylinder cross-section plane |
//! | `0x1512B0` | 113 | [`basis_1512b0`]: orthonormal basis (two tangents) around an axis |
//!
//! Skeleton shape record (the thing `0xD9760` reaches through `[edi + 0x61]`, see [`SkeletonShape`]): two bone
//! local end points, a squared radius and two bone indices into the skeleton matrix array.
//!
//! Arithmetic follows the image exactly. SSE operations are plain f32 in listing order (no fused multiply-add),
//! comparisons use `comiss` semantics (a NaN takes the `jb`/`jbe` side, never the `ja` side). `rsqrtss` is modelled
//! as `1/sqrt` in f32 (QEMU), so it is xemu level, not NV2A level. The x87 sequences (`fsqrt`, `fdivr`) run with the
//! 53 bit precision control the title sets, so they are modelled in f64 with one rounding to f32 at each `fstp`.

use super::sweep_records::{GuestMemory, transform_point};
use super::world_queries::Mat4;

/// Three floats.
pub type Vec3 = [f32; 3];

/// `[0x475C78]` = 1.0.
const ONE: f32 = 1.0;
/// `[0x475CD4]` = 0.5.
const HALF: f32 = 0.5;
/// `[0x478600]` = 1e-6: shortest accepted squared length of the sweep vector, the segment half axis, and the
/// direction in the cylinder frame; also the "perpendicular to the axis" limit of `|dir . axis|`.
pub const MIN_LENGTH_SQ: f32 = f32::from_bits(0x3586_37BD);
/// `[0x4765F4]` = 1e-7: guard of the `rsqrtss` normalisations.
pub const NORMALISE_GUARD: f32 = f32::from_bits(0x33D6_BF95);
/// `[0x4785FC]` = 0.99999: `|dir . axis|` from this on the ray is parallel to the cylinder axis.
pub const PARALLEL_LIMIT: f32 = f32::from_bits(0x3F7F_FFEF);
/// `[0x475D5C]` = `f32::MAX`: initial best fraction of the nearest contact search.
const FLT_MAX: f32 = f32::from_bits(0x7F7F_FFFF);

fn rsqrt(value: f32) -> f32 {
    ONE / value.sqrt()
}

/// `jb` after `comiss a, b`: below or unordered.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn jb(a: f32, b: f32) -> bool {
    !(a >= b)
}

/// `jbe` after `comiss a, b`: below, equal or unordered.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn jbe(a: f32, b: f32) -> bool {
    !(a > b)
}

/// One decoded skeleton shape record (INFERRED name; the record sits behind `[owner + 0x61]` of the
/// `0xD9760` link record, a null pointer ends the scan of that link).
///
/// | offset | size | field |
/// |--------|------|-------|
/// | `+0x00` | 3 x f32 | `p0`, end point in the space of bone `bone0` |
/// | `+0x0C` | 3 x f32 | `p1`, end point in the space of bone `bone1` |
/// | `+0x18` | f32 | `radius_sq`, SQUARED radius (compared with squared distances) |
/// | `+0x1C` | u16 | `bone0`, index into the matrix array at `[skeleton + 0x10]` (64 bytes each) |
/// | `+0x1E` | u16 | `bone1`, same for `p1` |
///
/// Bytes `+0x20..` are not read by this chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkeletonShape {
    pub p0: Vec3,
    pub p1: Vec3,
    pub radius_sq: f32,
    pub bone0: u16,
    pub bone1: u16,
}

impl SkeletonShape {
    /// Reads the record at `address` from guest memory (the same loads `0xD9760` does at `0xD9B27..0xD9BB6`).
    pub fn read(memory: &dyn GuestMemory, address: u32) -> Self {
        let vec = |at: u32| [0, 4, 8].map(|k| memory.read_f32(at + k));
        Self {
            p0: vec(address),
            p1: vec(address + 0x0C),
            radius_sq: memory.read_f32(address + 0x18),
            bone0: memory.read_u16(address + 0x1C),
            bone1: memory.read_u16(address + 0x1E),
        }
    }

    /// Guest address of the 64 byte bone matrix `bone` of a skeleton whose matrix array starts at `matrices`.
    pub fn matrix_address(matrices: u32, bone: u16) -> u32 {
        matrices.wrapping_add(u32::from(bone) << 6)
    }

    /// The two end points in the scan frame (`0x151E80` of `p0` with the `bone0` matrix, of `p1` with `bone1`).
    pub fn world_points(&self, bone0: &Mat4, bone1: &Mat4) -> (Vec3, Vec3) {
        (
            transform_point(bone0, self.p0),
            transform_point(bone1, self.p1),
        )
    }
}

/// `0x1512B0(tangent1, tangent2, axis, skip_normalise)`: builds the two tangents of the axis `n`.
///
/// Unless `skip_normalise` (the original flag is non zero) `n` is first scaled in place by `rsqrt(|n|^2)` when
/// `|n|^2 > 1e-7`. Then the tangent `t1` is `(-n.z, 0, n.x) * rsqrt(n.x^2 + n.z^2)` when `|n.x| >= |n.y|`, else
/// `(0, n.z, -n.y) * rsqrt(n.y^2 + n.z^2)`, and `t2 = n x t1`. Returns `(t1, t2)`.
pub fn basis_1512b0(n: &mut Vec3, skip_normalise: bool) -> (Vec3, Vec3) {
    if !skip_normalise {
        let length_sq = (n[0] * n[0] + n[1] * n[1]) + n[2] * n[2];
        if length_sq > NORMALISE_GUARD {
            let scale = rsqrt(length_sq) * ONE;
            n[0] *= scale;
            n[1] *= scale;
            n[2] *= scale;
        }
    }
    let (abs_x, abs_y) = (n[0].abs(), n[1].abs());
    let t1 = if !jb(abs_x, abs_y) {
        let scale = rsqrt(n[0] * n[0] + n[2] * n[2]) * ONE;
        [0.0 - n[2] * scale, 0.0, n[0] * scale]
    } else {
        let scale = rsqrt(n[1] * n[1] + n[2] * n[2]) * ONE;
        [0.0, n[2] * scale, 0.0 - n[1] * scale]
    };
    let t2 = [
        n[1] * t1[2] - t1[1] * n[2],
        t1[0] * n[2] - n[0] * t1[2],
        t1[1] * n[0] - n[1] * t1[0],
    ];
    (t1, t2)
}

/// `0x48590(a, b, from, to)` (register ABI: `ecx = a`, `edx = b`, `esi = from`, `edi = to`, stack = output).
///
/// With `d = to - from`: `out = b (b . d) + a (a . d)`, scaled by `rsqrt(|out|^2)` when `|out|^2 > 1e-7`.
/// `a` and `b` are the orthonormal tangents, so this is `d` projected on their plane and normalised: the radial
/// contact normal of the cylinder side (INFERRED).
pub fn radial_normal_48590(a: Vec3, b: Vec3, from: Vec3, to: Vec3) -> Vec3 {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let u = (b[2] * d[2] + b[0] * d[0]) + d[1] * b[1];
    let v = (a[1] * d[1] + a[0] * d[0]) + a[2] * d[2];
    let mut out = [
        b[0] * u + a[0] * v,
        a[1] * v + u * b[1],
        b[2] * u + v * a[2],
    ];
    let length_sq = (out[0] * out[0] + out[1] * out[1]) + out[2] * out[2];
    if length_sq > NORMALISE_GUARD {
        let scale = rsqrt(length_sq) * ONE;
        out[0] *= scale;
        out[1] *= scale;
        out[2] *= scale;
    }
    out
}

/// The three out arrays of `0x486C0` (`t` fractions, hit points, hit normals; two entries each). Entries the
/// call does not store keep their previous content, so the struct is passed by `&mut`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CylinderHits {
    pub t: [f32; 2],
    pub points: [Vec3; 2],
    pub normals: [Vec3; 2],
}

/// Inputs of `0x486C0` (stack arguments `origin, dir, centre, radius_sq, length, t, points, normals` and `eax` =
/// `axis`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CylinderQuery {
    /// `eax`: cylinder axis (any length; normalised on a copy, not necessarily unit).
    pub axis: Vec3,
    /// Stack arg 0: ray origin in the cylinder frame.
    pub origin: Vec3,
    /// Stack arg 1: ray vector (the hit point is `origin + t * dir`).
    pub dir: Vec3,
    /// Stack arg 2: cylinder centre.
    pub centre: Vec3,
    /// Stack arg 3: squared cylinder radius.
    pub radius_sq: f32,
    /// Stack arg 4: full axis length (the caps sit at +-`length / 2` from the centre).
    pub length: f32,
}

/// Whether `s` lies inside the closed interval spanned by `s_bot` and `s_top` (the `comiss` ladder of the side
/// hit acceptance; ordering is decided by `s_bot < s_top`, a NaN picks the `s_bot <= s <= s_top` side and rejects).
fn within(s: f32, s_top: f32, s_bot: f32) -> bool {
    if !jb(s_bot, s_top) {
        !(jb(s, s_top) || jb(s_bot, s))
    } else {
        !(jb(s, s_bot) || jb(s_top, s))
    }
}

/// `0x486C0`: ray against a finite cylinder with flat caps. Returns the number of contacts (0..=2) and fills
/// the first entries of `out`.
///
/// The ray direction is expressed in the frame (`t1`, `t2`, `axis`) as `(a1, a2, a3)` and normalised (inverse
/// length `inv`, 0 when `|dir|^2 <= 1e-6`), the origin relative to the centre as `(b1, b2, b3)`. Cases:
/// - `|a3| >= 0.99999` (parallel): miss when `b1^2 + b2^2 > radius_sq`, else both cap crossings, count 2.
/// - `|a3| <= 1e-6` (perpendicular): miss when `|b3|` is beyond the half length or the quadratic has a negative
///   discriminant; two side hits (disc > 0) or one tangent hit.
/// - otherwise the two caps are tried first (hit when the point lies inside the disc), then, while fewer than two
///   hits exist, the side quadratic; a side root counts only inside the cap interval. A negative side
///   discriminant returns 0 even when a cap hit was stored.
pub fn cylinder_ray_486c0(query: &CylinderQuery, out: &mut CylinderHits) -> u32 {
    let half = query.length * HALF;
    let radius_sq = query.radius_sq;
    let (origin, dir) = (query.origin, query.dir);
    let mut n = query.axis;
    let (t1, t2) = basis_1512b0(&mut n, false);

    let mut a1 = (dir[1] * t1[1] + dir[2] * t1[2]) + t1[0] * dir[0];
    let mut a2 = (dir[1] * t2[1] + dir[2] * t2[2]) + t2[0] * dir[0];
    let mut a3 = (dir[1] * n[1] + dir[2] * n[2]) + n[0] * dir[0];
    let length_sq = (a3 * a3 + a2 * a2) + a1 * a1;
    // x87: fsqrt, fdivr 1.0, fstp; the products are fld/fmul/fstp of f32 values (exact, one rounding).
    let inv = if length_sq > MIN_LENGTH_SQ {
        let inverse = (f64::from(ONE) / f64::from(length_sq).sqrt()) as f32;
        a1 *= inverse;
        a2 *= inverse;
        a3 *= inverse;
        inverse
    } else {
        0.0
    };

    let q = [
        origin[0] - query.centre[0],
        origin[1] - query.centre[1],
        origin[2] - query.centre[2],
    ];
    let b1 = (q[2] * t1[2] + q[1] * t1[1]) + q[0] * t1[0];
    let b2 = (q[2] * t2[2] + q[1] * t2[1]) + q[0] * t2[0];
    let b3 = (q[2] * n[2] + q[1] * n[1]) + q[0] * n[0];

    let abs_a3 = a3.abs();
    let hit_point = |t: f32| {
        [
            t * dir[0] + origin[0],
            dir[1] * t + origin[1],
            t * dir[2] + origin[2],
        ]
    };

    if !jb(abs_a3, PARALLEL_LIMIT) {
        let radial_sq = b2 * b2 + b1 * b1;
        if jb(radius_sq, radial_sq) {
            return 0;
        }
        let k = inv / a3;
        out.t[0] = (half - b3) * k;
        out.t[1] = ((0.0 - half) - b3) * k;
        out.normals[0] = n;
        out.normals[1] = [0.0 - n[0], 0.0 - n[1], 0.0 - n[2]];
        out.points[0] = hit_point(out.t[0]);
        out.points[1] = hit_point(out.t[1]);
        return 2;
    }

    if !jb(MIN_LENGTH_SQ, abs_a3) {
        // Perpendicular to the axis: only the side quadratic.
        if b3.abs() > half {
            return 0;
        }
        let a_coeff = a2 * a2 + a1 * a1;
        let b_coeff = b2 * a2 + b1 * a1;
        let c_coeff = (b2 * b2 + b1 * b1) - radius_sq;
        let disc = b_coeff * b_coeff - c_coeff * a_coeff;
        if 0.0 > disc {
            return 0;
        }
        if disc > 0.0 {
            let root = (f64::from(disc).sqrt()) as f32;
            let k = inv / a_coeff;
            out.t[0] = ((0.0 - b_coeff) - root) * k;
            out.t[1] = (root - b_coeff) * k;
            for index in 0..2 {
                out.points[index] = hit_point(out.t[index]);
                out.normals[index] = radial_normal_48590(t2, t1, query.centre, out.points[index]);
            }
            return 2;
        }
        out.t[0] = 0.0 - (b_coeff / a_coeff) * inv;
        out.points[0] = hit_point(out.t[0]);
        out.normals[0] = radial_normal_48590(t2, t1, query.centre, out.points[0]);
        return 1;
    }

    // General case: end caps first.
    let r = ONE / a3;
    let s_top = (half - b3) * r;
    let mut count = 0usize;
    let cap_radial = |s: f32| {
        let y = s * a2 + b2;
        let x = s * a1 + b1;
        y * y + x * x
    };
    if !jb(radius_sq, cap_radial(s_top)) {
        out.normals[0] = n;
        out.t[0] = s_top * inv;
        out.points[0] = hit_point(out.t[0]);
        count = 1;
    }
    let s_bot = ((0.0 - half) - b3) * r;
    if !jb(radius_sq, cap_radial(s_bot)) {
        out.normals[count] = [0.0 - n[0], 0.0 - n[1], 0.0 - n[2]];
        out.t[count] = s_bot * inv;
        out.points[count] = hit_point(out.t[count]);
        count += 1;
    }
    if count == 2 {
        return 2;
    }

    let a_coeff = a2 * a2 + a1 * a1;
    let b_coeff = b2 * a2 + b1 * a1;
    let c_coeff = (b2 * b2 + b1 * b1) - radius_sq;
    let disc = b_coeff * b_coeff - c_coeff * a_coeff;
    if 0.0 > disc {
        return 0;
    }
    let mut store_side = |s: f32, count: &mut usize| {
        out.t[*count] = s * inv;
        out.points[*count] = hit_point(out.t[*count]);
        out.normals[*count] = radial_normal_48590(t2, t1, query.centre, out.points[*count]);
        *count += 1;
    };
    if disc > 0.0 {
        let root = (f64::from(disc).sqrt()) as f32;
        let k = ONE / a_coeff;
        let first = ((0.0 - b_coeff) - root) * k;
        if within(first, s_top, s_bot) {
            store_side(first, &mut count);
        }
        if count == 2 {
            return 2;
        }
        let second = (root - b_coeff) * k;
        if within(second, s_top, s_bot) {
            store_side(second, &mut count);
        }
    } else {
        let only = 0.0 - b_coeff / a_coeff;
        if within(only, s_top, s_bot) {
            store_side(only, &mut count);
        }
    }
    count as u32
}

/// The contact `0x48FE0` reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeContact {
    pub position: Vec3,
    pub normal: Vec3,
}

/// `0x48FE0(origin, dir, p0, p1, radius_sq, out_position, out_normal)`: the segment `p0..p1` with squared radius
/// `radius_sq`, in the scan frame, against the ray `origin + t * dir`. Returns the contact with the smallest
/// `t >= 0` (position and normal), or `None` (the original returns 0 and leaves both outputs untouched).
///
/// Rejects when `|dir|^2 < 1e-6` or the squared half segment length (`d = p1 - mid`, summed z, y, x) is below 1e-6.
/// Otherwise calls [`cylinder_ray_486c0`] with `mid = (p0 + p1) * 0.5`, axis `d` and length `2 sqrt(|d|^2)`.
/// No upper bound on `t` is applied here.
pub fn shape_hit_48fe0(
    origin: Vec3,
    dir: Vec3,
    p0: Vec3,
    p1: Vec3,
    radius_sq: f32,
) -> Option<ShapeContact> {
    let dir_sq = (dir[0] * dir[0] + dir[1] * dir[1]) + dir[2] * dir[2];
    if MIN_LENGTH_SQ > dir_sq {
        return None;
    }
    let mid = [
        (p0[0] + p1[0]) * HALF,
        (p0[1] + p1[1]) * HALF,
        (p0[2] + p1[2]) * HALF,
    ];
    let d = [p1[0] - mid[0], p1[1] - mid[1], p1[2] - mid[2]];
    let half_sq = (d[2] * d[2] + d[1] * d[1]) + d[0] * d[0];
    if MIN_LENGTH_SQ > half_sq {
        return None;
    }
    let sqrt = (f64::from(half_sq).sqrt()) as f32;
    let length = sqrt + sqrt;
    let mut hits = CylinderHits {
        t: [ONE, ONE],
        points: [[0.0; 3]; 2],
        normals: [[0.0; 3]; 2],
    };
    let query = CylinderQuery {
        axis: d,
        origin,
        dir,
        centre: mid,
        radius_sq,
        length,
    };
    let count = cylinder_ray_486c0(&query, &mut hits) as usize;
    let mut best = FLT_MAX;
    let mut chosen = None;
    for index in 0..count.min(2) {
        let t = hits.t[index];
        if jb(t, 0.0) || jbe(best, t) {
            continue;
        }
        chosen = Some(index);
        best = t;
    }
    chosen.map(|index| ShapeContact {
        position: hits.points[index],
        normal: hits.normals[index],
    })
}
