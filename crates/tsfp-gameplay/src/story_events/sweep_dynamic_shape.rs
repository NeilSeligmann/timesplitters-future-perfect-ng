//! T1846 batch 8: the dynamic shape test `0x4B3B0` (swept sphere against one dynamic polygon object), its leaves
//! `0x484D0` (point in polygon) and `0x4A270` (swept sphere against one polygon edge), and the two impact effect
//! spawners `0x129C20` (ambient) and `0x129CA0` (contact) called by the dynamic apply step `0x129F50`.
//!
//! All arithmetic is decoded from `build/default.xbe` (`tools/x87_blocks.py` listings) and checked bit exactly against
//! the original under the Unicorn oracle (goldens `sweep_dynamic_*.golden`, see
//! `docs/t-story-event-sweep-dynamic-shape-t1846.md`). SSE code is plain `f32` in listing order (no fused multiply
//! add), x87 sequences are `f64` with one rounding at each store. Field names are INFERRED, offsets and ABI MEASURED.
//!
//! Wiring into `sweep_records.rs`:
//! * [`GuestShapeTest`] implements `DynamicShapeTest` (memory only, `0x4B3B0` is pure).
//! * [`ImpactEffectsPort`] implements `ImpactEffects`; the engine behind it is the [`ImpactServices`] trait.

// Negated float comparisons mirror comiss/jcc flag semantics (NaN takes the 'not greater' branch).
#![allow(
    clippy::neg_cmp_op_on_partial_ord,
    clippy::manual_range_contains,
    clippy::too_many_arguments
)]

use super::sweep_records::{
    ContactEffectCall, DynamicShapeCall, DynamicShapeResult, DynamicShapeTest, GuestMemory,
    ImpactEffects,
};

/// `[0x475C78]`, `[0x475CAC]`, `[0x4760D0]`, `[0x475D10]`, `[0x475DB4]`, `[0x4765F4]` (MEASURED constants).
const ONE: f32 = 1.0;
const MINUS_ONE: f32 = -1.0;
const TWO: f32 = 2.0;
const FOUR: f32 = 4.0;
/// Squared length below which the edge contact normal is left unnormalised.
const NORMAL_EPSILON: f32 = f32::from_bits(0x33D6_BF95);

// ---------------------------------------------------------------------------------------------------------------
// 0x484D0: point in polygon
// ---------------------------------------------------------------------------------------------------------------

/// Layout of a dynamic polygon object (INFERRED names, MEASURED offsets): `u16 +0x2A` vertex count, `f32 +0x30..0x38`
/// plane normal, `i8 +0x3C` and `i8 +0x3D` the projection axes (component indices) of the inside test, vertices of
/// three floats at `+0x40 + 0x1C * i`.
pub const OBJECT_VERTEX_COUNT: u32 = 0x2A;
pub const OBJECT_NORMAL: u32 = 0x30;
pub const OBJECT_AXES: u32 = 0x3C;
pub const OBJECT_VERTICES: u32 = 0x40;
pub const VERTEX_STRIDE: u32 = 0x1C;

fn vertex(object: u32, index: u32) -> u32 {
    object
        .wrapping_add(OBJECT_VERTICES)
        .wrapping_add(VERTEX_STRIDE.wrapping_mul(index))
}

/// The last vertex (`object + 0x24 + 0x1C * count`), the starting "previous" vertex of every edge loop.
fn last_vertex(memory: &dyn GuestMemory, object: u32) -> u32 {
    let count = u32::from(memory.read_u16(object + OBJECT_VERTEX_COUNT));
    object
        .wrapping_add(0x24)
        .wrapping_add(VERTEX_STRIDE.wrapping_mul(count))
}

/// `0x484D0(point*, object)`: true when `point` projected on the axes `(i8 +0x3C, i8 +0x3D)` is on the non positive
/// side of every edge `(previous, current)`: `(prev.a - cur.a) * (p.b - prev.b) + (p.a - prev.a) * (cur.b - prev.b)`
/// above zero rejects. A zero vertex count accepts. Axis indices outside `0..3` are not supported (the original would read
/// beyond the 3 float point). MEASURED.
pub fn point_in_polygon(memory: &dyn GuestMemory, point: [f32; 3], object: u32) -> bool {
    let first_axis = i32::from(memory.read_u8(object + OBJECT_AXES) as i8);
    let second_axis = i32::from(memory.read_u8(object + OBJECT_AXES + 1) as i8);
    let count = u32::from(memory.read_u16(object + OBJECT_VERTEX_COUNT));
    let component =
        |vertex: u32, axis: i32| memory.read_f32(vertex.wrapping_add((axis * 4) as u32));
    let point_axis = |axis: i32| {
        usize::try_from(axis)
            .ok()
            .and_then(|i| point.get(i))
            .copied()
            .unwrap_or(0.0)
    };
    let (point_a, point_b) = (point_axis(first_axis), point_axis(second_axis));
    let mut previous = last_vertex(memory, object);
    for index in 0..count {
        let current = vertex(object, index);
        let cross = (component(previous, first_axis) - component(current, first_axis))
            * (point_b - component(previous, second_axis))
            + (point_a - component(previous, first_axis))
                * (component(current, second_axis) - component(previous, second_axis));
        if cross > 0.0 {
            return false;
        }
        previous = current;
    }
    true
}

// ---------------------------------------------------------------------------------------------------------------
// 0x4A270: swept sphere against one edge
// ---------------------------------------------------------------------------------------------------------------

/// `f32(sqrt(f64(x)))` (x87 `fsqrt` at 53 bit precision then the `fstp` to f32; equals the correctly rounded f32 root).
fn fsqrt(x: f32) -> f32 {
    f64::from(x).sqrt() as f32
}

/// `0x4A270(start, delta, radius, b0, b1, &best_t, &out_point)` (cdecl, 7 stack args, returns 0/1): sphere centre
/// `start + t * delta` against the segment `b0..b1`. First the infinite line (quadratic with `a = |d x e|` style
/// terms, interior hit when the foot parameter `s` is in `[0, 1]`), otherwise (or when `a == 0`, parallel) the sphere
/// against the point `b0`. A hit needs `0 <= t < *best_t`, then `*best_t = t` and the contact point is written
/// (line foot `b0 + s * e`, or `b0`). On a miss nothing is written. MEASURED.
pub fn edge_sphere_sweep(
    start: [f32; 3],
    delta: [f32; 3],
    radius: f32,
    b0: [f32; 3],
    b1: [f32; 3],
    best_t: &mut f32,
    out_point: &mut [f32; 3],
) -> bool {
    let e = [b1[0] - b0[0], b1[1] - b0[1], b1[2] - b0[2]];
    let w = [start[0] - b0[0], start[1] - b0[1], start[2] - b0[2]];
    let we = (w[2] * e[2] + w[1] * e[1]) + w[0] * e[0];
    let wd = (delta[2] * w[2] + delta[0] * w[0]) + w[1] * delta[1];
    let ww = (w[2] * w[2] + w[1] * w[1]) + w[0] * w[0];
    let de = (delta[2] * e[2] + delta[0] * e[0]) + e[1] * delta[1];
    let ee = (e[2] * e[2] + e[1] * e[1]) + e[0] * e[0];
    let dd = (delta[0] * delta[0] + delta[1] * delta[1]) + delta[2] * delta[2];
    let a = de * de - dd * ee;
    let b = (de * we - ee * wd) * TWO;
    // ucomiss a, 0; jnp: the line test is skipped only when a is exactly zero (ordered equal)
    if a != 0.0 {
        let c = ((ee * radius) * radius + we * we) - ee * ww;
        let discriminant = b * b - (c * a) * FOUR;
        if !(discriminant > 0.0) {
            return false;
        }
        let two_a = a * TWO;
        let root = fsqrt(discriminant);
        let first = ((0.0 - b) - root) / two_a;
        let second = (root - b) / two_a;
        let t = if first > second { second } else { first };
        if 0.0 > t || t >= *best_t {
            return false;
        }
        let qx = start[0] + delta[0] * t;
        let qy = start[1] + t * delta[1];
        let qz = start[2] + t * delta[2];
        let (px, py, pz) = (qx - b0[0], qy - b0[1], qz - b0[2]);
        let s = ((pz * e[2] + py * e[1]) + px * e[0]) / ee;
        if s >= 0.0 && ONE >= s {
            *best_t = t;
            *out_point = [s * e[0] + b0[0], s * e[1] + b0[1], s * e[2] + b0[2]];
            return true;
        }
    }
    // sphere against the vertex b0
    let b = wd * TWO;
    let c = ww - radius * radius;
    let discriminant = b * b - (c * dd) * FOUR;
    if !(discriminant > 0.0) {
        return false;
    }
    let two_dd = dd * TWO;
    let root = fsqrt(discriminant);
    let first = ((0.0 - b) - root) / two_dd;
    let second = (root - b) / two_dd;
    let t = if first > second { second } else { first };
    if !(t >= 0.0) || !(*best_t > t) {
        return false;
    }
    *best_t = t;
    *out_point = b0;
    true
}

// ---------------------------------------------------------------------------------------------------------------
// 0x4B3B0: swept sphere against a polygon object
// ---------------------------------------------------------------------------------------------------------------

/// `0x4B3B0(start*, delta*, radius, object, skip_flip, &position, &normal, &f18, &fraction, no_edges)` (cdecl, 10
/// stack args, returns 0/1; MEASURED). `skip_flip` (stack arg 4) and `no_edges` (arg 9) are 0 at the only call site
/// (`0x128760`), both are ported.
///
/// 1. Plane distances `d0` (start) and `d1` (start + delta) of the plane through the last vertex with the object
///    normal; a start behind the plane (`d0 < 0`, unless `skip_flip`) mirrors the plane (distances and normal negated).
/// 2. `|d0| < radius`: already touching, `t = 0` and `*f18 = d0 - radius`; else a hit needs `d0 > radius > d1`,
///    `t = (d0 - radius) / (d0 - d1)` and `*f18 = 0`. Rejected when `t > *fraction`.
/// 3. The contact point is `start + t * delta - (*f18 + radius) * normal`. When [`point_in_polygon`] accepts it the
///    plane normal is the result and `*fraction = t`.
/// 4. Otherwise (unless `no_edges`) every edge `(previous, current)` runs [`edge_sphere_sweep`] with a local best
///    `t` starting at 1.0 and the contact point written to `position`; with at least one hit the normal is
///    `(start + t * delta - position)`, normalised with `rsqrt` when its squared length exceeds 1e-7, `*f18 =
///    min(0, |position - start| - radius)` (x87) and `*fraction = t`.
///
/// `out.position`, `out.f18_bits` are written as the original does, also on a failing return; `out.code` is the return
/// value. A zero vertex count never reaches the edge loop.
#[allow(clippy::too_many_arguments)]
pub fn dynamic_shape_test(
    memory: &dyn GuestMemory,
    start: [f32; 3],
    delta: [f32; 3],
    radius: f32,
    object: u32,
    skip_flip: bool,
    no_edges: bool,
    out: &mut DynamicShapeResult,
) -> u32 {
    dynamic_shape_test_routed(
        memory, start, delta, radius, object, skip_flip, no_edges, out,
    )
    .0
}

/// Where [`dynamic_shape_test_routed`] left (for coverage assertions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeRoute {
    /// Plane test failed (`d0 <= radius` or `radius <= d1`).
    PlaneRejected,
    /// `t > *fraction`.
    FractionRejected,
    /// Contact point inside the polygon.
    PlaneHit,
    /// Outside, edges skipped (`no_edges`) or no vertices.
    OutsideNoEdges,
    /// Edge loop found no hit.
    EdgeMiss,
    /// Edge loop found at least one hit.
    EdgeHit,
}

/// [`dynamic_shape_test`] plus the route taken.
#[allow(clippy::too_many_arguments)]
pub fn dynamic_shape_test_routed(
    memory: &dyn GuestMemory,
    start: [f32; 3],
    delta: [f32; 3],
    radius: f32,
    object: u32,
    skip_flip: bool,
    no_edges: bool,
    out: &mut DynamicShapeResult,
) -> (u32, ShapeRoute) {
    let (code, route) = shape_test_inner(
        memory, start, delta, radius, object, skip_flip, no_edges, out,
    );
    out.code = code;
    (code, route)
}

#[allow(clippy::too_many_arguments)]
fn shape_test_inner(
    memory: &dyn GuestMemory,
    start: [f32; 3],
    delta: [f32; 3],
    radius: f32,
    object: u32,
    skip_flip: bool,
    no_edges: bool,
    out: &mut DynamicShapeResult,
) -> (u32, ShapeRoute) {
    let count = u32::from(memory.read_u16(object + OBJECT_VERTEX_COUNT));
    let mut normal = [
        memory.read_f32(object + OBJECT_NORMAL),
        memory.read_f32(object + OBJECT_NORMAL + 4),
        memory.read_f32(object + OBJECT_NORMAL + 8),
    ];
    let last = last_vertex(memory, object);
    let (px, py, pz) = (
        memory.read_f32(last),
        memory.read_f32(last + 4),
        memory.read_f32(last + 8),
    );
    let plane = (pz * normal[2] + py * normal[1]) + normal[0] * px;
    let end = [
        delta[0] + start[0],
        delta[1] + start[1],
        delta[2] + start[2],
    ];
    let start_raw = (normal[2] * start[2] + normal[0] * start[0]) + normal[1] * start[1];
    let end_raw = (normal[0] * end[0] + normal[1] * end[1]) + normal[2] * end[2];
    let mut d0 = start_raw - plane;
    let mut d1 = end_raw - plane;
    if !skip_flip && 0.0 > d0 {
        d0 = 0.0 - d0;
        d1 = 0.0 - d1;
        normal = [
            normal[0] * MINUS_ONE,
            normal[1] * MINUS_ONE,
            normal[2] * MINUS_ONE,
        ];
    }
    let (t, f18) = if radius > f32::from_bits(d0.to_bits() & 0x7FFF_FFFF) {
        (0.0f32, d0 - radius)
    } else {
        if !(d0 > radius) || !(radius > d1) {
            return (0, ShapeRoute::PlaneRejected);
        }
        ((d0 - radius) / (d0 - d1), 0.0f32)
    };
    out.f18_bits = f18.to_bits();
    if t > out.fraction {
        return (0, ShapeRoute::FractionRejected);
    }
    let reach = f18 + radius;
    out.position = [
        (delta[0] * t + start[0]) - reach * normal[0],
        (delta[1] * t + start[1]) - reach * normal[1],
        (delta[2] * t + start[2]) - reach * normal[2],
    ];
    if point_in_polygon(memory, out.position, object) {
        out.normal = normal;
        out.fraction = t;
        return (1, ShapeRoute::PlaneHit);
    }
    if no_edges || count == 0 {
        return (0, ShapeRoute::OutsideNoEdges);
    }
    let mut best = ONE;
    let mut hits = 0u32;
    let mut previous = last;
    for index in 0..count {
        let current = vertex(object, index);
        let read = |address: u32| {
            [
                memory.read_f32(address),
                memory.read_f32(address + 4),
                memory.read_f32(address + 8),
            ]
        };
        if edge_sphere_sweep(
            start,
            delta,
            radius,
            read(previous),
            read(current),
            &mut best,
            &mut out.position,
        ) {
            hits += 1;
        }
        previous = current;
    }
    if hits == 0 {
        return (0, ShapeRoute::EdgeMiss);
    }
    let position = out.position;
    let mut contact_normal = [
        (start[0] + delta[0] * best) - position[0],
        (start[1] + delta[1] * best) - position[1],
        (delta[2] * best + start[2]) - position[2],
    ];
    let squared = (contact_normal[0] * contact_normal[0] + contact_normal[1] * contact_normal[1])
        + contact_normal[2] * contact_normal[2];
    if squared > NORMAL_EPSILON {
        // rsqrtss is 1/sqrt in f32 at xemu level, times 1.0 (no refinement step)
        let scale = (1.0f32 / squared.sqrt()) * ONE;
        for component in &mut contact_normal {
            *component *= scale;
        }
    }
    out.normal = contact_normal;
    let (dx, dy, dz) = (
        f64::from(position[0]) - f64::from(start[0]),
        f64::from(position[1]) - f64::from(start[1]),
        f64::from(position[2]) - f64::from(start[2]),
    );
    let distance = (((dz * dz) + (dy * dy)) + (dx * dx)).sqrt();
    let clearance = (distance - f64::from(radius)) as f32;
    out.f18_bits = if clearance > 0.0 {
        0.0f32.to_bits()
    } else {
        clearance.to_bits()
    };
    out.fraction = best;
    (1, ShapeRoute::EdgeHit)
}

/// `DynamicShapeTest` over guest memory: `0x4B3B0` with both flags 0 as the dynamic query calls it.
pub struct GuestShapeTest<'a> {
    pub memory: &'a dyn GuestMemory,
}

impl DynamicShapeTest for GuestShapeTest<'_> {
    fn test(&mut self, call: &DynamicShapeCall, out: &mut DynamicShapeResult) {
        dynamic_shape_test(
            self.memory,
            call.start,
            call.delta,
            call.radius,
            call.object,
            false,
            false,
            out,
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Effect spawners 0x129C20 and 0x129CA0
// ---------------------------------------------------------------------------------------------------------------

/// Writable guest memory for the effect slot table, the contact pool bytes and the rng.
pub trait EffectMemory: GuestMemory {
    fn write_u8(&mut self, address: u32, value: u8);
    fn write_u32(&mut self, address: u32, value: u32);
    fn write_f32(&mut self, address: u32, value: f32) {
        self.write_u32(address, value.to_bits());
    }
}

/// Engine systems called by the spawners (names INFERRED, addresses and arguments MEASURED).
pub trait ImpactServices {
    /// `0xE94C0(&position, 36.0, 0)`: nearby object disturbance at the ambient effect position.
    fn disturb(&mut self, position: [f32; 3], radius: f32, flags: u32);
    /// `0x45820(&record)`: submit a 0x30 byte effect request (words as initialised by `0x45390`).
    fn spawn_request(&mut self, record: [u32; 12]);
    /// `0x103A00(0, slot)`: tear down the effect owned by a slot being evicted or reaped (`slot` is the guest address).
    fn build_debris(&mut self, slot: u32);
    /// `0xC3E60(&point, 0.01, 0.5, &out, 0, 0)`: ground height below `point` (`world_queries::region_height_query`).
    fn ground_height(&mut self, point: [f32; 3], y_bias: f32, half_extent: f32) -> f32;
}

/// Rng state low and high dword and call counter of `0x153B40`.
pub const RNG_LOW: u32 = 0x4E_7950;
pub const RNG_HIGH: u32 = 0x4E_7954;
pub const RNG_CALLS: u32 = 0x74_97C8;
/// Effect slot table: 4 slots of `0x6C` bytes at `0x742360` (`+0` in use, `+4` owner object), round robin cursor
/// `[0x74235C]`, reap stamp `[0x742358]` compared with the frame counter `[0x7497A8]`.
pub const SLOT_TABLE: u32 = 0x74_2360;
pub const SLOT_STRIDE: u32 = 0x6C;
pub const SLOT_CURSOR: u32 = 0x74_235C;
pub const REAP_STAMP: u32 = 0x74_2358;
pub const FRAME_COUNTER: u32 = 0x74_97A8;
/// Pool of 0x4C byte entries at `0x7ABFA0` (the objects' two index bytes `+0xE`, `+0xF` refer to them) and the mode
/// dword `[0x7ABF80]`.
pub const POOL_BASE: u32 = 0x7A_BFA0;
pub const POOL_STRIDE: u32 = 0x4C;
pub const POOL_MODE: u32 = 0x7A_BF80;

/// `0x153B40`: 64 bit LCG `state = state * 0x343FD + 0x269EC3`, returns the low dword, counts calls (MEASURED).
pub fn rng(memory: &mut dyn EffectMemory) -> u32 {
    let calls = memory.read_u32(RNG_CALLS).wrapping_add(1);
    memory.write_u32(RNG_CALLS, calls);
    let state = (u64::from(memory.read_u32(RNG_HIGH)) << 32) | u64::from(memory.read_u32(RNG_LOW));
    let next = state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
    memory.write_u32(RNG_LOW, next as u32);
    memory.write_u32(RNG_HIGH, (next >> 32) as u32);
    next as u32
}

/// `0x45390` record layout: `[0] id, [2] 1.0, [4] 0x14, [5] 1.0, [6] 1.0, [11] 50.0`, rest zero.
fn spawn_record() -> [u32; 12] {
    let mut words = [0u32; 12];
    words[2] = 0x3F80_0000;
    words[4] = 0x14;
    words[5] = 0x3F80_0000;
    words[6] = 0x3F80_0000;
    words[11] = 0x4248_0000;
    words
}

/// `0x129C20(&position)`: ambient effect. `0xE94C0(position, 36.0, 0)`, then a request with id `0xD4B + (rng & 3)`,
/// position at words 7..9, byte `+0x28 = 1`, scale 50.0, submitted with `0x45820`. MEASURED (logging stubs).
pub fn ambient_effect(
    memory: &mut dyn EffectMemory,
    services: &mut dyn ImpactServices,
    position: [f32; 3],
) {
    services.disturb(position, 36.0, 0);
    let random = rng(memory);
    let mut record = spawn_record();
    record[0] = (random & 3) + 0xD4B;
    for axis in 0..3 {
        record[7 + axis] = position[axis].to_bits();
    }
    record[10] = 1;
    services.spawn_request(record);
}

/// `0x128EF0(object)` (register `eax`): marks an object's effect as released: `[object] = 0` when its word `+0x28`
/// has a bit of `0x40C`, else `-60.0`; for each of the signed bytes `+0xE`, `+0xF` that is `>= 0` the pool entry
/// `0x7ABFA0 + 0x4C * index` gets its word `+4` cleared and the byte becomes `0xFF`. MEASURED.
pub fn release_object(memory: &mut dyn EffectMemory, object: u32) {
    let value = if memory.read_u16(object + 0x28) & 0x40C != 0 {
        0.0
    } else {
        -60.0
    };
    memory.write_f32(object, value);
    for offset in [0xE, 0xF] {
        let index = i32::from(memory.read_u8(object + offset) as i8);
        if index >= 0 {
            memory.write_u32(
                POOL_BASE
                    .wrapping_add(4)
                    .wrapping_add((index as u32).wrapping_mul(POOL_STRIDE)),
                0,
            );
            memory.write_u8(object + offset, 0xFF);
        }
    }
}

/// `0x129440(object)`: claims a slot for `object`. Returns 0 when an in use slot already belongs to `object`.
/// Otherwise the slot at the round robin cursor is taken (an in use one is first evicted: [`release_object`] of its
/// owner, `services.build_debris(slot)`, flag cleared), marked in use with `object` as owner, and the cursor advances
/// by one modulo 4 (C remainder). Returns the slot's guest address. MEASURED.
pub fn claim_slot(
    memory: &mut dyn EffectMemory,
    services: &mut dyn ImpactServices,
    object: u32,
) -> u32 {
    for slot in 0..4u32 {
        let address = SLOT_TABLE + slot * SLOT_STRIDE;
        if memory.read_u32(address) != 0 && memory.read_u32(address + 4) == object {
            return 0;
        }
    }
    let cursor = memory.read_u32(SLOT_CURSOR);
    let slot = SLOT_TABLE.wrapping_add(cursor.wrapping_mul(SLOT_STRIDE));
    if memory.read_u32(slot) != 0 {
        let owner = memory.read_u32(slot + 4);
        release_object(memory, owner);
        services.build_debris(slot);
        memory.write_u32(slot, 0);
    }
    memory.write_u32(slot, 1);
    memory.write_u32(slot + 4, object);
    memory.write_u32(SLOT_CURSOR, ((cursor as i32).wrapping_add(1) % 4) as u32);
    slot
}

/// `0x129500()`: at most once per frame (`[0x742358] != [0x7497A8]`) reaps the first in use slot found from the
/// cursor: [`release_object`] of its owner, `services.build_debris`, flag cleared, stamp set to the frame counter.
/// The scan wraps at 4 and the cursor itself is not stored. MEASURED.
pub fn reap_slot(memory: &mut dyn EffectMemory, services: &mut dyn ImpactServices) {
    if memory.read_u32(REAP_STAMP) == memory.read_u32(FRAME_COUNTER) {
        return;
    }
    let mut index = memory.read_u32(SLOT_CURSOR);
    for _ in 0..4 {
        let slot = SLOT_TABLE.wrapping_add(index.wrapping_mul(SLOT_STRIDE));
        if memory.read_u32(slot) != 0 {
            let owner = memory.read_u32(slot + 4);
            release_object(memory, owner);
            services.build_debris(slot);
            memory.write_u32(slot, 0);
            let frame = memory.read_u32(FRAME_COUNTER);
            memory.write_u32(REAP_STAMP, frame);
            return;
        }
        index = index.wrapping_add(1);
        if index as i32 >= 4 {
            index = 0;
        }
    }
}

/// `0x129CA0` (register convention `edi` = object, `ebx` = `call.w0`; stack: edge coordinates pointer, direction
/// pointer, weight, position pointer, which may be null: `position == None`). MEASURED.
///
/// * Rejected (calls [`release_object`] and returns) when `object[+0x2C] != 0` and `(w0[+4])[+0x30] & object[+0x2C]
///   == 0`, or when [`claim_slot`] returns 0.
/// * Fills the slot: `+8` = u16 `object[+0x28]` (`| 0x200` when `w0[+0x20] != 2`), `+0xC` = `-1` or the entry of the
///   table `[[[w0+4]+4]+0x10]` indexed by `object[+4]`, `+0x10` = `object[+8]`, corner pointers `+0x14/+0x18/+0x1C/
///   +0x20` = `object+0x40/0x5C/0x94/0x78`, `+0x34/+0x38/+0x3C` pool entry pointers from the object bytes `+0xE`,
///   `+0xF` (count in `+0x34`, which is decremented once more when `[0x7ABF80] != 2` and byte `+0xF` is valid),
///   `+0x5C/+0x60` the edge coordinates, `+0x24..+0x2C` velocity `= (rng/2^32 * weight * direction)` (x scaled by 3.0
///   in x and z, x87 f64) plus `25.0 * w0[+0xF8..+0x100]`, `+0x30` = weight.
/// * With a position and `slot[+8] & 0x1000 == 0`: `slot[+0x40]` = `ground_height(direction + position, 0.01, 0.5)` and
///   `slot[+8] |= 1`.
/// * Finally [`reap_slot`].
pub fn contact_effect_at(
    memory: &mut dyn EffectMemory,
    services: &mut dyn ImpactServices,
    w0: u32,
    object: u32,
    edge_coordinates: [f32; 2],
    direction: [f32; 3],
    weight: f32,
    position: Option<[f32; 3]>,
) {
    let mask = memory.read_u32(object + 0x2C);
    if mask != 0 {
        let shared = memory.read_u32(w0 + 4);
        if memory.read_u32(shared + 0x30) & mask == 0 {
            release_object(memory, object);
            return;
        }
    }
    let slot = claim_slot(memory, services, object);
    if slot == 0 {
        release_object(memory, object);
        return;
    }
    memory.write_u32(slot + 8, u32::from(memory.read_u16(object + 0x28)));
    memory.write_u32(slot + 0xC, 0xFFFF_FFFF);
    let table = {
        let first = memory.read_u32(w0 + 4);
        let second = memory.read_u32(first + 4);
        memory.read_u32(second + 0x10)
    };
    if table != 0 {
        let entry = memory.read_u32(object + 4).wrapping_shl(4);
        let value = memory.read_u32(entry.wrapping_add(table));
        memory.write_u32(slot + 0xC, value);
    }
    if memory.read_u32(w0 + 0x20) != 2 {
        let flags = memory.read_u32(slot + 8);
        memory.write_u32(slot + 8, flags | 0x200);
    }
    memory.write_u32(slot + 0x34, 0);
    let first_byte = i32::from(memory.read_u8(object + 0xE) as i8);
    if first_byte >= 0 {
        memory.write_u32(
            slot + 0x38,
            POOL_BASE.wrapping_add((first_byte as u32).wrapping_mul(POOL_STRIDE)),
        );
        memory.write_u32(slot + 0x34, 1);
    }
    let second_byte = i32::from(memory.read_u8(object + 0xF) as i8);
    if second_byte >= 0 {
        if memory.read_u32(POOL_MODE) != 2 {
            let count = memory.read_u32(slot + 0x34).wrapping_sub(1);
            memory.write_u32(slot + 0x34, count);
        }
        memory.write_u32(
            slot + 0x3C,
            POOL_BASE.wrapping_add((second_byte as u32).wrapping_mul(POOL_STRIDE)),
        );
        let count = memory.read_u32(slot + 0x34).wrapping_add(1);
        memory.write_u32(slot + 0x34, count);
    }
    memory.write_u32(slot + 0x14, object + 0x40);
    memory.write_u32(slot + 0x18, object + 0x5C);
    memory.write_u32(slot + 0x1C, object + 0x94);
    memory.write_u32(slot + 0x20, object + 0x78);
    memory.write_u32(slot + 0x5C, edge_coordinates[0].to_bits());
    memory.write_u32(slot + 0x60, edge_coordinates[1].to_bits());
    let identity = memory.read_u32(object + 8);
    memory.write_u32(slot + 0x10, identity);
    // x87: unit = raw * 2^-32 (f64), v = unit * weight, velocity components rounded to f32 at each store
    let raw = rng(memory);
    let scale = f64::from(f32::from_bits(0x2F80_0000));
    let spread = f64::from(raw) * scale * f64::from(weight);
    let triple = f64::from(3.0f32);
    let velocity_x = (spread * f64::from(direction[0]) * triple) as f32;
    let velocity_y = (spread * f64::from(direction[1])) as f32;
    let velocity_z = (spread * f64::from(direction[2]) * triple) as f32;
    memory.write_f32(slot + 0x24, velocity_x);
    memory.write_f32(slot + 0x28, velocity_y);
    memory.write_f32(slot + 0x2C, velocity_z);
    let gain = 25.0f32;
    let boost_x = memory.read_f32(w0 + 0xF8) * gain;
    let boost_z = memory.read_f32(w0 + 0x100) * gain;
    let boost_y = memory.read_f32(w0 + 0xFC) * gain;
    let velocity_z = boost_z + memory.read_f32(slot + 0x2C);
    let velocity_x = memory.read_f32(slot + 0x24) + boost_x;
    let velocity_y = memory.read_f32(slot + 0x28) + boost_y;
    memory.write_f32(slot + 0x2C, velocity_z);
    memory.write_f32(slot + 0x24, velocity_x);
    memory.write_f32(slot + 0x28, velocity_y);
    memory.write_f32(slot + 0x30, weight);
    if let Some(position) = position {
        let flags = memory.read_u32(slot + 8);
        if flags & 0x1000 == 0 {
            let point = [
                direction[0] + position[0],
                direction[1] + position[1],
                direction[2] + position[2],
            ];
            let height = services.ground_height(point, 0.01, 0.5);
            memory.write_f32(slot + 0x40, height);
            let flags = memory.read_u32(slot + 8);
            memory.write_u32(slot + 8, flags | 1);
        }
    }
    reap_slot(memory, services);
}

/// `0x129CA0` for a [`ContactEffectCall`] (position always present).
pub fn contact_effect(
    memory: &mut dyn EffectMemory,
    services: &mut dyn ImpactServices,
    call: &ContactEffectCall,
) {
    contact_effect_at(
        memory,
        services,
        call.w0,
        call.object,
        call.edge_coordinates,
        call.direction,
        call.weight,
        Some(call.position),
    );
}

/// `ImpactEffects` over guest memory and the engine services. `sweep_records::apply_dynamic_contact` needs the memory shared
/// with `GuestMemory` reads of the same sweep: use a `RefCell` wrapper implementing both traits.
pub struct ImpactEffectsPort<'a, M: EffectMemory, S: ImpactServices> {
    pub memory: &'a mut M,
    pub services: &'a mut S,
}

impl<M: EffectMemory, S: ImpactServices> ImpactEffects for ImpactEffectsPort<'_, M, S> {
    fn spawn_ambient(&mut self, position: [f32; 3]) {
        ambient_effect(self.memory, self.services, position);
    }

    fn spawn_contact(&mut self, call: &ContactEffectCall) {
        contact_effect(self.memory, self.services, call);
    }
}
