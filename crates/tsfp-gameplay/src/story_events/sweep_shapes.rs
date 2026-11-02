//! T1846 batch 8: the three sphere-versus-shape tests called by the collision sweep `0x92360` (MEASURED against the
//! original machine code under the Unicorn oracle, see `tests/story_events_t1846_sweep_shapes.rs`).
//!
//! All three take the moving sphere (`start`, `radius`) and one shape of a sweep record (`SweepItem`, `0x1C` bytes:
//! `kind` dword, `a` at `+4`, `b` at `+0x10`) and return a contact code (0 = miss, 1 = hit). On a hit they write a
//! contact position, a contact normal and a shape id (a float, stored by the sweep as raw bits in the impact node).
//!
//! | kind | original | shape | `a` | `b` |
//! |------|----------|-------|-----|-----|
//! | 0 | `0x4A670` (100 instr) | sphere | centre | `b.x` = radius |
//! | 1 | `0x4FE00` (212 instr) | oriented box | centre | half extents |
//! | 2 | `0x52550` (126 instr) | capsule | centre | `b.x` = radius, `b.y` = segment end parameter |
//!
//! Helpers decoded with them: `0x49370` (closest point of a segment, returns the squared distance in st0) and
//! `0x151E80` (point transform by a 4x4 matrix with translation at `+0x30`).
//!
//! Arithmetic follows the image exactly: SSE operations are plain f32 in listing order (no fused multiply-add), the
//! x87 sequences (`fsqrt`, `fchs`, `fsub`, and the f64 sum of squares returned by `0x49370`) are f64 with one rounding
//! at each `fstp`. `rsqrtss` is modelled as `1/sqrt` in f32 (QEMU), so it is xemu-level, not NV2A-level.
//!
//! Original out-parameter behaviour: an output a path does not store keeps its previous content (the sweep reuses
//! the same stack slots across shapes, and kinds above 2 re-use the previous results). [`ShapeOut`] therefore
//! persists between calls. Every hit of kinds 0..=2 stores all three outputs (`0x52550` copies the axis below
//! 1e-5 and normalises above 1e-7, and the two ranges overlap, so no offset skips both).
//!
//! Wiring of `SweepServices::shape_test` (see [`ShapeTester`]): one `ShapeTester` per sweep call, reset at the start.

use super::world_queries::{ShapeTestResult, SweepItem};

/// 4x4 row-major matrix as 16 floats (same as `world_queries::Mat4`).
pub type Matrix = [f32; 16];

/// The identity matrix the sweep passes (`0x153240` builds it into a stack temporary).
pub const IDENTITY: Matrix = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// Constants read from the image: `0x475C78` = 1.0, `0x475CD4` = 0.5, `0x4760D0` = -1.0, `0x475D60` = 1e-5,
/// `0x4765F4` = 1e-7.
const NORMAL_EPSILON: f32 = f32::from_bits(0x3727_c5ac);

/// The three out parameters (position, normal, shape id as float).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShapeOut {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub id: f32,
}

/// Exit of `0x4A670`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SphereExit {
    /// `|d|^2 > (r1 + r2)^2`.
    Far,
    /// Intersection parameter `s < 0`.
    BelowZero,
    /// Intersection parameter `s > 1`.
    AboveOne,
    /// `r1^2 - s^2 |d|^2 < 0`.
    NoDepth,
    Hit,
}

/// Exit of `0x4FE00`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxExit {
    /// Clamped point farther than the radius.
    Miss,
    /// Centre outside the box, sphere touches it.
    Outside,
    /// Centre inside the box (penetration along the dominant axis).
    Inside,
}

/// Exit of `0x52550`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapsuleExit {
    Miss,
    /// Offset below 1e-5: normal copied from the axis.
    HitOnAxis,
    HitNormalised,
}

fn sqrt_f32(value: f32) -> f32 {
    // fsqrt in f64 then fstp to f32 is the correctly rounded f32 square root.
    (value as f64).sqrt() as f32
}

/// `0x4A670(a, r1, start, radius, pos, normal, id)`: sphere (centre `a`, radius `r1`) against the moving sphere.
pub fn sphere_vs_sphere(
    a: [f32; 3],
    r1: f32,
    start: [f32; 3],
    radius: f32,
    out: &mut ShapeOut,
) -> SphereExit {
    let dy = start[1] - a[1];
    let dz = start[2] - a[2];
    let dx = start[0] - a[0];
    let len2 = (dz * dz + dy * dy) + dx * dx;
    let reach = r1 + radius;
    if len2 > reach * reach {
        return SphereExit::Far;
    }
    let inverse = 1.0f32 / len2;
    let r2_squared = radius * radius;
    let r1_squared = r1 * r1;
    let s = ((r1_squared - r2_squared) * inverse + 1.0) * 0.5;
    if 0.0 > s {
        return SphereExit::BelowZero;
    }
    if s > 1.0 {
        return SphereExit::AboveOne;
    }
    let depth_squared = r1_squared - (s * s) * len2;
    if 0.0 > depth_squared {
        return SphereExit::NoDepth;
    }
    let (sx, sy, sz) = (s * dx, s * dy, s * dz);
    out.position = [a[0] + sx, a[1] + sy, a[2] + sz];
    let scale = -((inverse as f64).sqrt());
    out.normal = [
        (dx as f64 * scale) as f32,
        (dy as f64 * scale) as f32,
        (dz as f64 * scale) as f32,
    ];
    out.id = -((depth_squared as f64).sqrt() as f32);
    SphereExit::Hit
}

/// `0x151E80(matrix, point)`: `p' = rows . p + translation` with the summation order of the image
/// (`z` term first, then `y`, then `x`, then the translation).
pub fn transform_point(m: &Matrix, p: [f32; 3]) -> [f32; 3] {
    [
        ((m[8] * p[2] + m[4] * p[1]) + m[0] * p[0]) + m[12],
        ((m[9] * p[2] + m[5] * p[1]) + m[1] * p[0]) + m[13],
        ((m[10] * p[2] + m[6] * p[1]) + m[2] * p[0]) + m[14],
    ]
}

/// `0x4FE00(start, radius, a, b, matrix, pos, normal, id)`: sphere against the box with centre `a` and half extents
/// `b`, orientation rows `matrix[0..3]`, `[4..7]`, `[8..11]`. The local point is clamped, then transformed back
/// with the whole matrix (translation included, which the sweep keeps at identity).
pub fn sphere_vs_box(
    start: [f32; 3],
    radius: f32,
    a: [f32; 3],
    b: [f32; 3],
    m: &Matrix,
    out: &mut ShapeOut,
) -> BoxExit {
    let dy = start[1] - a[1];
    let dz = start[2] - a[2];
    let dx = start[0] - a[0];
    let local = [
        (m[2] * dz + m[1] * dy) + m[0] * dx,
        (m[6] * dz + m[5] * dy) + m[4] * dx,
        (m[10] * dz + m[9] * dy) + m[8] * dx,
    ];
    let mut clamped = [local[0], local[1], local[2]];
    let mut moved = false;
    for axis in 0..2 {
        let negative = 0.0f32 - b[axis];
        if negative > local[axis] {
            clamped[axis] = negative;
            moved = true;
        } else if local[axis] > b[axis] {
            clamped[axis] = b[axis];
            moved = true;
        }
    }
    let negative_z = 0.0f32 - b[2];
    let outside = if negative_z > local[2] {
        clamped[2] = negative_z;
        true
    } else if local[2] > b[2] {
        clamped[2] = b[2];
        true
    } else {
        moved
    };
    if outside {
        let q = transform_point(m, clamped);
        let (ez, ey, ex) = (dz - q[2], dy - q[1], dx - q[0]);
        let len2 = (ez * ez + ey * ey) + ex * ex;
        if 0.0 > radius * radius - len2 {
            return BoxExit::Miss;
        }
        let distance = sqrt_f32(len2);
        out.position = [a[0] + q[0], q[1] + a[1], a[2] + q[2]];
        let inverse = 1.0f32 / distance;
        out.normal = [inverse * ex, inverse * ey, inverse * ez];
        out.id = (distance as f64 - radius as f64) as f32;
        return BoxExit::Outside;
    }
    // Centre inside: push out along the axis with the largest local magnitude.
    let magnitudes = [local[0].abs(), local[1].abs(), local[2].abs()];
    let mut best = magnitudes[0];
    let mut axis = 0;
    if magnitudes[1] > best {
        best = magnitudes[1];
        axis = 1;
    }
    if magnitudes[2] > best {
        best = magnitudes[2];
        axis = 2;
    }
    out.position = start;
    let mut unit = [0.0f32; 3];
    unit[axis] = if local[axis] > 0.0 { 1.0 } else { -1.0 };
    out.normal = transform_point(m, unit);
    out.id = 0.0f32 - ((b[axis] - best) + radius);
    BoxExit::Inside
}

/// `0x49370(p, a, b)`: closest point of the segment `a..b` to `p`. Returns the point and the squared distance as the
/// f64 left in st0 (products exact, sums rounded to 53 bits).
pub fn segment_closest_point(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> ([f32; 3], f64) {
    let (mut vx, mut vy, mut vz) = (p[0] - a[0], p[1] - a[1], p[2] - a[2]);
    let (dz, dy, dx) = (b[2] - a[2], b[1] - a[1], b[0] - a[0]);
    let dot = (dz * vz + dy * vy) + dx * vx;
    let t: f32;
    if !(0.0 >= dot) {
        let length2 = (dz * dz + dy * dy) + dx * dx;
        if !(dot >= length2) {
            t = dot / length2;
            vx -= t * dx;
            vy -= t * dy;
            vz -= t * dz;
        } else {
            t = 1.0;
            vx -= dx;
            vy -= dy;
            vz -= dz;
        }
    } else {
        t = 0.0;
    }
    let point = [t * dx + a[0], t * dy + a[1], t * dz + a[2]];
    let (fx, fy, fz) = (vx as f64, vy as f64, vz as f64);
    let squared = (fz * fz + fy * fy) + fx * fx;
    (point, squared)
}

/// `0x52550(start, radius, a, b, matrix, pos, normal, id)`: capsule with axis `matrix[4..7]`, centre `a`, radius
/// `b.x` and half length `b.y - b.x` against the moving sphere.
pub fn sphere_vs_capsule(
    start: [f32; 3],
    radius: f32,
    a: [f32; 3],
    b: [f32; 3],
    m: &Matrix,
    out: &mut ShapeOut,
) -> CapsuleExit {
    let extent = b[1] - b[0];
    let axis = [m[4], m[5], m[6]];
    let end_a = [
        axis[0] * extent + a[0],
        axis[1] * extent + a[1],
        axis[2] * extent + a[2],
    ];
    let negative = 0.0f32 - extent;
    let end_b = [
        axis[0] * negative + a[0],
        axis[1] * negative + a[1],
        axis[2] * negative + a[2],
    ];
    let (closest, squared) = segment_closest_point(start, end_a, end_b);
    let distance_squared = squared as f32;
    let reach = b[0] + radius;
    let depth_squared = reach * reach - distance_squared;
    if 0.0 > depth_squared {
        return CapsuleExit::Miss;
    }
    let (ey, ez, ex) = (
        start[1] - closest[1],
        start[2] - closest[2],
        start[0] - closest[0],
    );
    let len2 = (ez * ez + ey * ey) + ex * ex;
    let mut exit = CapsuleExit::HitOnAxis;
    if NORMAL_EPSILON > len2 {
        out.normal = axis;
    } else if len2 > f32::from_bits(0x33d6_bf95) {
        let inverse = (1.0f32 / sqrt_f32(len2)) * 1.0;
        out.normal = [inverse * ex, inverse * ey, inverse * ez];
        exit = CapsuleExit::HitNormalised;
    }
    let normal = out.normal;
    out.position = [
        normal[0] * b[0] + closest[0],
        normal[1] * b[0] + closest[1],
        normal[2] * b[0] + closest[2],
    ];
    out.id = -((depth_squared as f64).sqrt() as f32);
    exit
}

/// Dispatch of `0x92360` for item kinds 0..=2 (identity matrix); returns the contact code (0 or 1).
pub fn shape_test(item: &SweepItem, start: [f32; 3], radius: f32, out: &mut ShapeOut) -> u32 {
    match item.kind {
        0 => (sphere_vs_sphere(item.a, item.b[0], start, radius, out) == SphereExit::Hit) as u32,
        1 => (sphere_vs_box(start, radius, item.a, item.b, &IDENTITY, out) != BoxExit::Miss) as u32,
        2 => {
            (sphere_vs_capsule(start, radius, item.a, item.b, &IDENTITY, out) != CapsuleExit::Miss)
                as u32
        }
        _ => 0,
    }
}

/// Persistent out-parameter state of one sweep call, usable as the body of `SweepServices::shape_test`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ShapeTester {
    pub out: ShapeOut,
}

impl ShapeTester {
    /// Same signature and result as `SweepServices::shape_test`.
    pub fn shape_test(
        &mut self,
        item: &SweepItem,
        start: [f32; 3],
        radius: f32,
    ) -> ShapeTestResult {
        let code = shape_test(item, start, radius, &mut self.out);
        ShapeTestResult {
            code,
            position: self.out.position,
            normal: self.out.normal,
            shape_id: self.out.id.to_bits(),
        }
    }
}
