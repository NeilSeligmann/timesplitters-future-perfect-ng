//! T1846 batch 8: the world mesh triangle test `0x4B310` and its chain, as pure models.
//!
//! `0x4B310(sweep*, sub, bounds, base*, quantised, table, entry, flag, outPos*, outNormal*, outF18*, outFraction*,
//! flag5)` (cdecl, 13 arguments, called by the record test `0x91EC0`) tests a moving or stationary sphere against one
//! convex polygon ("triangle") of a world mesh bounds record. It dispatches on the first dword of the sweep box:
//!
//! - `stationary != 0`: `*outFraction = 0.0` and [`stationary_test`] (`0x4A9D0`, register call: eax = entry,
//!   ecx = bounds, edx = sub, stack `sweep, base, quantised, table, flag, outPos, outNormal, outF18, flag5`).
//! - `stationary == 0`: `*outFraction = 1.0` and [`moving_test`] (`0x4AE00`, register call: esi = sweep, edi = entry,
//!   ecx = bounds, stack `sub, base, quantised, flag, outPos, outNormal, outF18, outFraction, flag5`).
//!
//! Callees, all ported: `0x48350` ([`point_in_polygon`]), `0x49E60` ([`closest_on_polygon`]), `0x12D80` (the vertex
//! fetch [`vertex`]) and `0x4A270` ([`sphere_edge`]). They are leaves (no further calls), 123 + 184 + 39 + 244
//! instructions.
//!
//! Guest layouts (MEASURED by the golden, names INFERRED):
//! - `entry` (12 bytes): `i8[0]` vertex count `n`, `i8[1..4]` plane normal (scaled by `1/127`), `u8[4]` bits 0..1 and
//!   2..3 are the two projection axes for the inside test, `u8[5]` neighbour count, `u16[8]` offset of the `n` vertex
//!   index bytes inside `[bounds + 0x2C]`, `u16[0xA]` offset (in 2 byte units) of the neighbour list inside
//!   `[bounds + 0x34]` (byte `& 0xF` = edge id).
//! - `sub` vertex array: quantised (`i16 x 3`, stride 8, `value * (1/1024) + base`) or plain `f32 x 3` (stride 16).
//!   `base` is the three floats at `header + 0x38`.
//! - sweep box: see [`SweepBox`]: `start`, `delta`, `radius`.
//! Constants (read from the image): `1/1024`, `1/127` (`0x3C010204`), `0.866`, `1e-4`, `1e-7`, `1.0`, `2.0`, `4.0`.
//!
//! Plane axes are assumed to be 0..2 (value 3 would read the float after the vector in the original). `rsqrtss` is
//! `1/sqrt` (xemu level). The x87 length/discriminant sequences are f64 with one rounding at the store (53 bit
//! precision control as set by the oracle).
//!
//! Confidence: control flow and arithmetic MEASURED (bit-exact against the Unicorn oracle running the original).

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::sweep_records::{GuestMemory, TriangleTest, TriangleTestCall, TriangleTestResult};
use super::world_queries::SweepBox;

/// `[0x475C9C]`: scale of a quantised vertex coordinate.
pub const VERTEX_SCALE: f32 = f32::from_bits(0x3A80_0000);
/// `[0x475CA0]`: scale to the integer grid of the quantised inside test.
pub const GRID_SCALE: f32 = f32::from_bits(0x4480_0000);
/// `[0x476038]` = 1/127.
pub const NORMAL_SCALE: f32 = f32::from_bits(0x3C01_0204);
/// `[0x478614]`: neighbour planes at least this parallel count as one surface.
pub const COPLANAR_DOT: f32 = f32::from_bits(0x3F5D_B22D);
/// `[0x476240]` = 1e-4: minimum length of the edge contact normal.
pub const MIN_LENGTH: f32 = f32::from_bits(0x38D1_B717);
/// `[0x4765F4]` = 1e-7: guard of the `rsqrtss` normalisation.
pub const MIN_LENGTH_SQ: f32 = f32::from_bits(0x33D6_BF95);
const FLT_MAX: f32 = f32::from_bits(0x7F7F_FFFF);

/// The four outputs of `0x4B310` (the original writes through pointers; unwritten ones keep the previous value).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriangleOut {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// `outF18`: signed penetration depth (<= 0 when overlapping) or the plane distance.
    pub f18: f32,
    pub fraction: f32,
}

fn read_i8(memory: &dyn GuestMemory, address: u32) -> i32 {
    i32::from(memory.read_u8(address) as i8)
}

fn read_i16(memory: &dyn GuestMemory, address: u32) -> i32 {
    i32::from(memory.read_u16(address) as i16)
}

/// `x as i32` as `cvttss2si` does it (out of range and NaN give `i32::MIN`).
fn truncate(value: f32) -> i32 {
    if value.is_nan() || value >= 2147483648.0 || value < -2147483648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// The per-call inputs shared by the ported functions (guest addresses as in [`TriangleTestCall`]).
pub struct Polygon<'a> {
    pub memory: &'a dyn GuestMemory,
    pub sub: u32,
    pub bounds: u32,
    pub base: u32,
    pub quantised: bool,
    pub entry: u32,
}

impl Polygon<'_> {
    fn count(&self) -> i32 {
        read_i8(self.memory, self.entry)
    }
    fn indices(&self) -> u32 {
        self.memory
            .read_u32(self.bounds + 0x2C)
            .wrapping_add(u32::from(self.memory.read_u16(self.entry + 8)))
    }
    /// The vertex index byte `i` (sign extended).
    fn index(&self, i: i32) -> i32 {
        read_i8(self.memory, self.indices().wrapping_add(i as u32))
    }
    fn base_vec(&self) -> [f32; 3] {
        [
            self.memory.read_f32(self.base),
            self.memory.read_f32(self.base.wrapping_add(4)),
            self.memory.read_f32(self.base.wrapping_add(8)),
        ]
    }
    /// `0x12D80` and the inlined copies: vertex `index` of the sub array.
    fn vertex(&self, index: i32) -> [f32; 3] {
        vertex(
            self.memory,
            self.sub,
            self.base_vec(),
            self.quantised,
            index,
        )
    }
    fn plane_normal(&self) -> [f32; 3] {
        let component =
            |offset: u32| read_i8(self.memory, self.entry + offset) as f32 * NORMAL_SCALE;
        [component(1), component(2), component(3)]
    }
}

/// `0x12D80`: vertex `index` of the sub array (quantised: `i16 * 1/1024 + base`; plain: three floats at stride 16).
pub fn vertex(
    memory: &dyn GuestMemory,
    sub: u32,
    base: [f32; 3],
    quantised: bool,
    index: i32,
) -> [f32; 3] {
    if quantised {
        let at = sub.wrapping_add(index.wrapping_mul(8) as u32);
        [
            read_i16(memory, at) as f32 * VERTEX_SCALE + base[0],
            read_i16(memory, at.wrapping_add(2)) as f32 * VERTEX_SCALE + base[1],
            read_i16(memory, at.wrapping_add(4)) as f32 * VERTEX_SCALE + base[2],
        ]
    } else {
        let at = sub.wrapping_add(index.wrapping_mul(16) as u32);
        [
            memory.read_f32(at),
            memory.read_f32(at.wrapping_add(4)),
            memory.read_f32(at.wrapping_add(8)),
        ]
    }
}

/// `0x48350`: is `point` inside the polygon projected on the two axes? The test is `cross <= 0` for every edge
/// (counter clockwise winding); the quantised variant works on the 1/1024 integer grid with 32 bit wrapping products.
pub fn point_in_polygon(polygon: &Polygon, point: [f32; 3]) -> bool {
    let selector = polygon.memory.read_u8(polygon.entry + 4);
    let axis_a = usize::from(selector & 3);
    let axis_b = usize::from((selector >> 2) & 3);
    let count = polygon.count();
    if count <= 0 {
        return true;
    }
    let last = polygon.index(count - 1);
    if polygon.quantised {
        let base = polygon.base_vec();
        let grid_a = i32::from(truncate((point[axis_a] - base[axis_a]) * GRID_SCALE) as i16);
        let grid_b = i32::from(truncate((point[axis_b] - base[axis_b]) * GRID_SCALE) as i16);
        let word = |index: i32, axis: usize| {
            read_i16(
                polygon.memory,
                polygon
                    .sub
                    .wrapping_add(index.wrapping_mul(8) as u32)
                    .wrapping_add(2 * axis as u32),
            )
        };
        let mut previous = last;
        for i in 0..count {
            let current = polygon.index(i);
            let first = (word(current, axis_b) - word(previous, axis_b))
                .wrapping_mul(grid_a - word(previous, axis_a));
            let second = (word(previous, axis_a) - word(current, axis_a))
                .wrapping_mul(grid_b - word(previous, axis_b));
            if first.wrapping_add(second) > 0 {
                return false;
            }
            previous = current;
        }
        true
    } else {
        let mut previous = polygon.vertex(last);
        for i in 0..count {
            let current = polygon.vertex(polygon.index(i));
            let a = point[axis_a] - previous[axis_a];
            let b = point[axis_b] - previous[axis_b];
            let cross =
                (current[axis_b] - previous[axis_b]) * a + (previous[axis_a] - current[axis_a]) * b;
            if cross > 0.0 {
                return false;
            }
            previous = current;
        }
        true
    }
}

/// Result of `0x49E60`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClosestEdge {
    /// Squared distance (returned in st0); `FLT_MAX` when no edge was measured.
    pub distance_sq: f32,
    /// Closest point on the polygon boundary (0 when never set; the original leaves stack garbage then).
    pub point: [f32; 3],
    /// `*out1`: start vertex slot of the closest edge (`(i != 0 ? i : n) - 1`).
    pub start_slot: i32,
    /// `*out2`: end vertex slot `i` of the closest edge.
    pub end_slot: i32,
}

/// `0x49E60`: closest point of the polygon boundary to `point`, by projecting on each edge (clamped to the segment).
pub fn closest_on_polygon(polygon: &Polygon, point: [f32; 3]) -> ClosestEdge {
    let count = polygon.count();
    let mut best = ClosestEdge {
        distance_sq: FLT_MAX,
        point: [0.0; 3],
        start_slot: 0,
        end_slot: 0,
    };
    if count <= 0 {
        return best;
    }
    let mut previous = polygon.vertex(polygon.index(count - 1));
    let mut i = 0;
    while i < count {
        let current = polygon.vertex(polygon.index(i));
        let mut delta = [
            point[0] - previous[0],
            point[1] - previous[1],
            point[2] - previous[2],
        ];
        let edge = [
            current[0] - previous[0],
            current[1] - previous[1],
            current[2] - previous[2],
        ];
        let dot = edge[2] * delta[2] + edge[1] * delta[1] + edge[0] * delta[0];
        let t;
        if !(0.0 < dot) && !dot.is_nan() {
            t = 0.0f32;
        } else {
            let length_sq = edge[2] * edge[2] + edge[1] * edge[1] + edge[0] * edge[0];
            if !(dot < length_sq) && !dot.is_nan() {
                t = 1.0;
                delta = [delta[0] - edge[0], delta[1] - edge[1], delta[2] - edge[2]];
            } else {
                t = dot / length_sq;
                delta = [
                    delta[0] - t * edge[0],
                    delta[1] - t * edge[1],
                    delta[2] - t * edge[2],
                ];
            }
        }
        let closest = [
            t * edge[0] + previous[0],
            t * edge[1] + previous[1],
            t * edge[2] + previous[2],
        ];
        let distance_sq = delta[2] * delta[2] + delta[1] * delta[1] + delta[0] * delta[0];
        if best.distance_sq > distance_sq {
            best.point = closest;
            best.distance_sq = distance_sq;
            best.start_slot = (if i != 0 { i } else { count }) - 1;
            best.end_slot = i;
        }
        previous = current;
        i += 1;
        // The original re-reads the count byte each iteration (unchanged).
    }
    best
}

/// `0x4A270(start*, delta*, radius, A*, B*, bestT*, outPoint*)`: sweep a sphere (`start + t * delta`, radius) against
/// the segment `A..B`. Solves the quadratic against the infinite cylinder (parallel case: against point `A`), keeps the
/// smaller root, and accepts the cylinder hit only when its projection `s` on the segment is within `[0, 1]`, else
/// falls back to the sphere at `A`. On a hit `*best_t = t` and `out_point` is the contact point on the segment.
pub fn sphere_edge(
    start: [f32; 3],
    delta: [f32; 3],
    radius: f32,
    a: [f32; 3],
    b: [f32; 3],
    best_t: &mut f32,
    out_point: &mut [f32; 3],
) -> bool {
    let e = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let g = [start[0] - a[0], start[1] - a[1], start[2] - a[2]];
    let ge = g[2] * e[2] + g[1] * e[1] + g[0] * e[0];
    let gd = delta[2] * g[2] + delta[0] * g[0] + g[1] * delta[1];
    let gg = g[2] * g[2] + g[1] * g[1] + g[0] * g[0];
    let de = delta[2] * e[2] + delta[0] * e[0] + e[1] * delta[1];
    let ee = e[2] * e[2] + e[1] * e[1] + e[0] * e[0];
    let dd = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];
    let quad_a = de * de - dd * ee;
    let quad_b = (de * ge - ee * gd) * 2.0;
    // `ucomiss` + parity trick: jump to the point case only when `quad_a == 0` exactly (NaN goes on).
    if quad_a != 0.0 || quad_a.is_nan() {
        let disc = quad_b * quad_b - (((ee * radius) * radius + ge * ge) - ee * gg) * quad_a * 4.0;
        if !(disc > 0.0) {
            return false;
        }
        let denom = quad_a * 2.0;
        let root = (disc as f64).sqrt() as f32;
        let mut t = (0.0 - quad_b - root) / denom;
        let t2 = (root - quad_b) / denom;
        if t > t2 {
            t = t2;
        }
        if 0.0 > t || t >= *best_t {
            return false;
        }
        let q = [
            start[0] + delta[0] * t,
            start[1] + delta[1] * t,
            start[2] + delta[2] * t,
        ];
        let s = ((q[2] - a[2]) * e[2] + (q[1] - a[1]) * e[1] + (q[0] - a[0]) * e[0]) / ee;
        if s >= 0.0 && 1.0 >= s {
            *best_t = t;
            *out_point = [s * e[0] + a[0], s * e[1] + a[1], s * e[2] + a[2]];
            return true;
        }
    }
    // Sphere against point A.
    let two_gd = gd * 2.0;
    let disc = two_gd * two_gd - ((gg - radius * radius) * dd) * 4.0;
    if !(disc > 0.0) {
        return false;
    }
    let denom = dd * 2.0;
    let root = (disc as f64).sqrt() as f32;
    let mut t = (0.0 - two_gd - root) / denom;
    let t2 = (root - two_gd) / denom;
    if t > t2 {
        t = t2;
    }
    if t < 0.0 || t.is_nan() {
        return false;
    }
    if !(*best_t > t) {
        return false;
    }
    *best_t = t;
    *out_point = a;
    true
}

fn rsqrt(value: f32) -> f32 {
    1.0 / value.sqrt()
}

/// `0x4A9D0`: stationary sphere (start, radius) against the polygon. Returns the contact code (0 or 1).
pub fn stationary_test(
    polygon: &Polygon,
    sweep: &SweepBox,
    flag: bool,
    flag5: u8,
    table: u32,
    out: &mut TriangleOut,
) -> u32 {
    let inner = polygon;
    let memory = inner.memory;
    let start = sweep.start;
    let radius = sweep.radius;
    let normal = inner.plane_normal();
    let count = inner.count();
    let v = inner.vertex(inner.index(count.wrapping_sub(1)));
    let plane = (v[2] * normal[2] + v[1] * normal[1]) + v[0] * normal[0];
    let mut distance =
        ((start[0] * normal[0] + start[1] * normal[1]) + normal[2] * start[2]) - plane;
    if f32::from_bits(distance.to_bits() & 0x7FFF_FFFF) > radius {
        return 0;
    }
    let mut face_normal = normal;
    if !flag && 0.0 > distance {
        distance = 0.0 - distance;
        face_normal = [0.0 - normal[0], 0.0 - normal[1], 0.0 - normal[2]];
    }
    let overlap = distance - radius;
    out.f18 = if 0.0 > overlap { overlap } else { 0.0 };
    let position = [
        start[0] - face_normal[0] * distance,
        start[1] - face_normal[1] * distance,
        start[2] - face_normal[2] * distance,
    ];
    out.position = position;
    if point_in_polygon(polygon, position) {
        out.normal = face_normal;
        return 1;
    }
    if flag5 != 0 {
        return 0;
    }
    let closest = closest_on_polygon(polygon, start);
    if closest.distance_sq >= radius * radius {
        return 0;
    }
    let neighbours = memory.read_u32(inner.bounds + 0x34);
    let list = neighbours.wrapping_add(u32::from(memory.read_u16(inner.entry + 0xA)) * 2);
    let neighbour_count = i32::from(memory.read_u8(inner.entry + 5));
    let mut accept_face = false;
    for k in 0..neighbour_count {
        let edge_id = i32::from(memory.read_u8(list.wrapping_add((k * 2) as u32)) & 0xF);
        let next = (edge_id + 1) % count;
        let (prev_slot, end_slot) = (closest.start_slot, closest.end_slot);
        if (edge_id == prev_slot && next == end_slot) || (edge_id == end_slot && next == prev_slot)
        {
            let other = table.wrapping_add((k * 12) as u32);
            let component = |offset: u32, own: f32| {
                (read_i8(memory, other + offset) as f32 * NORMAL_SCALE) * own
            };
            let dot = (component(3, normal[2]) + component(2, normal[1])) + component(1, normal[0]);
            // `ja` (0.866 > dot) leaves the loop for the edge contact; otherwise the face result stands.
            accept_face = !(COPLANAR_DOT > dot);
            break;
        }
    }
    if accept_face {
        out.normal = face_normal;
        return 1;
    }
    out.position = closest.point;
    let n = [
        start[0] - closest.point[0],
        start[1] - closest.point[1],
        start[2] - closest.point[2],
    ];
    out.normal = n;
    let sum = (f64::from(n[2]) * f64::from(n[2]) + f64::from(n[1]) * f64::from(n[1]))
        + f64::from(n[0]) * f64::from(n[0]);
    let length = sum.sqrt() as f32;
    let depth = length - radius;
    out.f18 = if depth > 0.0 { 0.0 } else { depth };
    if !(length > MIN_LENGTH) {
        out.normal = face_normal;
        return 1;
    }
    let inverse = 1.0 / length;
    out.normal = [n[0] * inverse, n[1] * inverse, n[2] * inverse];
    1
}

/// `0x4AE00`: moving sphere (`start + t * delta`) against the polygon. Returns the contact code (0 or 1); `out.fraction`
/// must hold the incoming bound (the dispatcher sets 1.0) and receives `t`.
pub fn moving_test(
    polygon: &Polygon,
    sweep: &SweepBox,
    flag: bool,
    flag5: u8,
    out: &mut TriangleOut,
) -> u32 {
    let inner = polygon;
    let (start, delta, radius) = (sweep.start, sweep.delta, sweep.radius);
    let end = [
        start[0] + delta[0],
        delta[1] + start[1],
        start[2] + delta[2],
    ];
    let mut normal = inner.plane_normal();
    let count = inner.count();
    let v = inner.vertex(inner.index(count.wrapping_sub(1)));
    let plane = (v[2] * normal[2] + v[1] * normal[1]) + v[0] * normal[0];
    let start_side = (start[2] * normal[2] + start[0] * normal[0]) + normal[1] * start[1];
    let end_side = (normal[2] * end[2] + normal[1] * end[1]) + normal[0] * end[0];
    let mut a = start_side - plane;
    let mut b = end_side - plane;
    if !flag && 0.0 > a {
        a = 0.0 - a;
        b = 0.0 - b;
        normal = [normal[0] * -1.0, normal[1] * -1.0, normal[2] * -1.0];
    }
    let t;
    if radius > f32::from_bits(a.to_bits() & 0x7FFF_FFFF) {
        out.f18 = a - radius;
        t = 0.0f32;
    } else {
        if !(a > radius) || !(radius > b) {
            return 0;
        }
        t = (a - radius) / (a - b);
        out.f18 = 0.0;
    }
    if t > out.fraction {
        return 0;
    }
    let offset = radius + out.f18;
    out.position = [
        (t * delta[0] + start[0]) - offset * normal[0],
        (delta[1] * t + start[1]) - offset * normal[1],
        (delta[2] * t + start[2]) - offset * normal[2],
    ];
    if point_in_polygon(polygon, out.position) {
        out.normal = normal;
        out.fraction = t;
        return 1;
    }
    if flag5 != 0 || count <= 0 {
        return 0;
    }
    let mut best_t = 1.0f32;
    let mut hits = 0;
    let mut previous = v;
    for i in 0..count {
        let current = polygon_vertex(polygon, i);
        let mut point = out.position;
        if sphere_edge(
            start,
            delta,
            radius,
            previous,
            current,
            &mut best_t,
            &mut point,
        ) {
            hits += 1;
            out.position = point;
        }
        previous = current;
    }
    if hits == 0 {
        return 0;
    }
    let q = [
        start[0] + best_t * delta[0],
        delta[1] * best_t + start[1],
        start[2] + delta[2] * best_t,
    ];
    let mut n = [
        q[0] - out.position[0],
        q[1] - out.position[1],
        q[2] - out.position[2],
    ];
    out.normal = n;
    let length_sq = (n[0] * n[0] + n[1] * n[1]) + n[2] * n[2];
    if length_sq > MIN_LENGTH_SQ {
        let inverse = rsqrt(length_sq) * 1.0;
        n = [n[0] * inverse, n[1] * inverse, n[2] * inverse];
        out.normal = n;
    }
    let d = [
        f64::from(out.position[0]) - f64::from(start[0]),
        f64::from(out.position[1]) - f64::from(start[1]),
        f64::from(out.position[2]) - f64::from(start[2]),
    ];
    let distance = (((d[2] * d[2] + d[1] * d[1]) + d[0] * d[0]).sqrt() - f64::from(radius)) as f32;
    out.f18 = if distance > 0.0 { 0.0 } else { distance };
    out.fraction = best_t;
    1
}

fn polygon_vertex(polygon: &Polygon, slot: i32) -> [f32; 3] {
    polygon.vertex(polygon.index(slot))
}

/// `0x4B310`: the dispatcher. `out` holds the caller's previous values (only written fields change).
pub fn triangle_test(
    memory: &dyn GuestMemory,
    call: &TriangleTestCall,
    out: &mut TriangleOut,
) -> u32 {
    let polygon = Polygon {
        memory,
        sub: call.sub_record_address,
        bounds: call.bounds_address,
        base: call.header_plus_0x38,
        quantised: call.quantised != 0,
        entry: call.entry_address,
    };
    let flag = call.flag != 0;
    if call.sweep.stationary != 0 {
        out.fraction = 0.0;
        stationary_test(
            &polygon,
            &call.sweep,
            flag,
            call.list_flag5,
            call.table_address,
            out,
        )
    } else {
        out.fraction = 1.0;
        moving_test(&polygon, &call.sweep, flag, call.list_flag5, out)
    }
}

/// [`TriangleTest`] over a guest memory view: the implementation of the engine call `0x4B310` for the record test.
pub struct WorldMeshTriangles<'a> {
    pub memory: &'a dyn GuestMemory,
}

impl TriangleTest for WorldMeshTriangles<'_> {
    fn test(&mut self, call: &TriangleTestCall) -> TriangleTestResult {
        let mut out = TriangleOut {
            position: [0.0; 3],
            normal: [0.0; 3],
            f18: 1.0,
            fraction: 1.0,
        };
        let code = triangle_test(self.memory, call, &mut out);
        TriangleTestResult {
            code,
            position: out.position,
            normal: out.normal,
            f18_bits: out.f18.to_bits(),
            fraction: out.fraction,
        }
    }
}
