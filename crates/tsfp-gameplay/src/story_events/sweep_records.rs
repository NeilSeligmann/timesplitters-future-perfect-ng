//! T1846 batch 8: the record test `0x91EC0`, the dynamic object query `0x128760` and its apply step `0x129F50`, the
//! three callees of the sweep `0x92360` (`world_queries::sweep`), ported as models verified bit-exactly against the
//! original machine code under the Unicorn oracle (goldens `sweep_records_91ec0`, `sweep_dynamic_128760`,
//! `sweep_apply_129f50`). See docs/t-story-event-sweep-records-t1846.md.
//!
//! Confidence: control flow, data layouts and arithmetic are MEASURED (golden-verified). The geometry tests that the
//! functions call (`0x4B310` swept sphere against a triangle table, `0x4B3B0` swept sphere against a dynamic object)
//! and the effect spawners (`0x129C20`, `0x129CA0`) are the traits [`TriangleTest`], [`DynamicShapeTest`] and
//! [`ImpactEffects`]; the ports are `sweep_triangles`, `sweep_dynamic_shape` (T1846 batch 8, later wave). Field names are
//! INFERRED.
//!
//! The record test walks a quantised box tree in guest memory, so it reads through [`GuestMemory`]; the implementor of
//! `SweepServices` holds the guest image anyway.

// `!(a > b)` / `!(a >= b)` keep the unordered (NaN) behaviour of the original comiss + ja/jb.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::world_queries::{
    ImpactList, ImpactNode, ImpactOther, ImpactPool, Mat4, SweepBox, init_impact_node,
    transform_direction,
};

/// Read access to the guest address space (little endian). Unmapped memory reads as zero in the tests.
pub trait GuestMemory {
    fn read_u8(&self, address: u32) -> u8;
    fn read_u16(&self, address: u32) -> u16 {
        u16::from(self.read_u8(address)) | u16::from(self.read_u8(address.wrapping_add(1))) << 8
    }
    fn read_u32(&self, address: u32) -> u32 {
        u32::from(self.read_u16(address)) | u32::from(self.read_u16(address.wrapping_add(2))) << 16
    }
    fn read_f32(&self, address: u32) -> f32 {
        f32::from_bits(self.read_u32(address))
    }
}

/// `[0x475DCC]` = `0x3B808081` (1/255): scale of the quantised child boxes (MEASURED constant).
pub const QUANT_SCALE: f32 = f32::from_bits(0x3B80_8081);
/// `[0x475C78]` = 1.0.
const ONE: f32 = 1.0;

// ---------------------------------------------------------------------------------------------------------------
// Small pure callees
// ---------------------------------------------------------------------------------------------------------------

/// `0x151E80(matrix, point)`: transforms a point in place, `x' = ((m8 z + m4 y) + m0 x) + m12` and so on (MEASURED).
pub fn transform_point(m: &Mat4, v: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = v;
    [
        ((m[8] * z + m[4] * y) + m[0] * x) + m[12],
        ((m[9] * z + m[5] * y) + m[1] * x) + m[13],
        ((m[10] * z + m[6] * y) + m[2] * x) + m[14],
    ]
}

/// `0x166770(list, node)`: sorted insert into the impact list by the float at `+0x1C`, ascending, after equal keys.
/// `ja` semantics: a NaN key never compares greater (MEASURED). `slot` is the pool slot of the new node.
pub fn insert_sorted(list: &mut ImpactList, pool: &mut ImpactPool, slot: usize) {
    let base = pool.base_address;
    let slot_of = |address: u32| (address.wrapping_sub(base) / 0x38) as usize;
    let key = |pool: &ImpactPool, address: u32| f32::from_bits(pool.nodes[slot_of(address)][7]);
    let new_address = base + 0x38 * slot as u32;
    let new_key = f32::from_bits(pool.nodes[slot][7]);
    let head = list.first;
    if head == 0 || key(pool, head) > new_key {
        pool.nodes[slot][13] = head;
        list.first = new_address;
        return;
    }
    let mut current = head;
    loop {
        let next = pool.nodes[slot_of(current)][13];
        if next == 0 || key(pool, next) > new_key {
            pool.nodes[slot][13] = next;
            pool.nodes[slot_of(current)][13] = new_address;
            return;
        }
        current = next;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x91EC0: the record test
// ---------------------------------------------------------------------------------------------------------------

/// Result of the stubbed `0x4B310` call (swept sphere against one triangle of a record's table).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TriangleTestResult {
    /// eax: contact code (0 = no contact).
    pub code: u32,
    /// Out vector at E-0x18 (contact position).
    pub position: [f32; 3],
    /// Out vector at E-0x24 (contact normal).
    pub normal: [f32; 3],
    /// Out dword at E-0x28 (passed on as `f18`, INFERRED shape id bits).
    pub f18_bits: u32,
    /// Out float at E-0x30 (fraction of the move; accepted only when `1.0 > fraction`).
    pub fraction: f32,
}

/// The 13 arguments of `0x4B310` (cdecl), in call order. Pointers are guest addresses; `sweep` is the 14 dword box.
#[derive(Clone, Debug, PartialEq)]
pub struct TriangleTestCall {
    /// 1: the sweep box (`E+0x18` of `0x91EC0`).
    pub sweep: SweepBox,
    /// 2: vertex sub-array entry: `[record + 0x18 * selector + 4] + index * (8 if quantised else 16)`.
    pub sub_record_address: u32,
    /// 3: the bounds record (`E+0xC`).
    pub bounds_address: u32,
    /// 4: `header + 0x38`.
    pub header_plus_0x38: u32,
    /// 5: `header.dword[+0xC] & 0x10`.
    pub quantised: u32,
    /// 6: `[bounds + 0x1C]`, the 12 byte triangle table.
    pub table_address: u32,
    /// 7: address of the 12 byte table entry.
    pub entry_address: u32,
    /// 8: 1 when the entry selector bits (`byte[+4] & 0xC0`) are zero, else 0.
    pub flag: u32,
    /// 13: the byte at `+5` of the impact list head.
    pub list_flag5: u8,
}

/// The stubbed engine call `0x4B310`.
pub trait TriangleTest {
    fn test(&mut self, call: &TriangleTestCall) -> TriangleTestResult;
}

/// All state `0x91EC0` touches besides guest memory.
pub struct RecordTestEnv<'a> {
    pub memory: &'a dyn GuestMemory,
    pub pool: &'a mut ImpactPool,
    pub list: &'a mut ImpactList,
    /// The byte at `+5` of the list head (read by the original, not modelled by `ImpactList`).
    pub list_flag5: u8,
    pub triangles: &'a mut dyn TriangleTest,
}

/// The call-invariant arguments of `0x91EC0` (everything except `detail` and `aabb`, which change in the recursion).
struct Fixed<'a> {
    mover: u32,
    record_index: i32,
    bounds: u32,
    sweep: &'a SweepBox,
    arg5: u32,
    other: Option<ImpactOther>,
    matrix: &'a Mat4,
    arg9: u32,
}

fn read_matrix(memory: &dyn GuestMemory, address: u32) -> Mat4 {
    let mut m = [0.0f32; 16];
    for (i, v) in m.iter_mut().enumerate() {
        *v = memory.read_f32(address.wrapping_add(4 * i as u32));
    }
    m
}

fn read_vec3(memory: &dyn GuestMemory, address: u32) -> [f32; 3] {
    [
        memory.read_f32(address),
        memory.read_f32(address + 4),
        memory.read_f32(address + 8),
    ]
}

impl RecordTestEnv<'_> {
    /// `0x91EC0(mover, record_index, bounds, detail, aabb, sweep, arg5, other, list, body_matrix, arg9)` (cdecl, 11
    /// stack arguments), the implementation of `SweepServices::record_test`. `aabb` (arg 5) is the same pointer as
    /// `bounds` at the sweep's call site, so it is read from `bounds_address`. Returns the OR of the contact codes.
    ///
    /// A node with `u16[+6] != 0` is a leaf: its `u16[+8]` / `u16[+0xA]` select `count` entries of the 12 byte triangle
    /// table at `[bounds + 0x1C]`. Each entry picks (via selector bits `byte[+4] >> 6` and `u16[+6]`) a 10 byte
    /// record of the sweep record (`[record + 0x60 + 4 * selector]`) whose first i16 may name a part (`>= 0`) whose
    /// flags (`[[header + 0x14] + 0x30 * part + 0x1C]`) skip the entry. Surviving entries go to `0x4B310`; a contact with
    /// `fraction < 1.0` is transformed into world space when the body is locally transformed (`header.h4 > 0`, body
    /// flags `8` and `0x200000`, part matrix `[mover + 0x10] + 64 * byte[record + 0x49]`) and/or when `byte[body + 0x32]`
    /// is set (`body_matrix`), allocates a pool node (none left: return the codes so far), fills it with `0x1665F0`
    /// semantics and inserts it sorted. A node with `u16[+6] == 0` is an inner node: with `header.byte[+0xC]` negative
    /// each of its two children (indices `u16[+8]`, `u16[+0xA]` into `[bounds + 0x24]`) whose quantised box
    /// (`byte * ((max - min) * (1/255)) + min`) overlaps the sweep box is visited recursively.
    pub fn record_test(&mut self, call: &super::world_queries::RecordTestCall) -> u32 {
        let memory = self.memory;
        let other = (call.other_address != 0).then(|| ImpactOther {
            address: call.other_address,
            kind_20: memory.read_u32(call.other_address + 0x20),
        });
        let fixed = Fixed {
            mover: call.mover_address,
            record_index: call.record_index,
            bounds: call.bounds_address,
            sweep: &call.sweep,
            arg5: call.arg5,
            other,
            matrix: &call.body_matrix,
            arg9: call.arg9,
        };
        let mut aabb = [0.0f32; 6];
        for (i, v) in aabb.iter_mut().enumerate() {
            *v = memory.read_f32(call.bounds_address + 4 * i as u32);
        }
        self.walk(&fixed, call.detail_address, &aabb)
    }

    fn walk(&mut self, fixed: &Fixed, detail: u32, aabb: &[f32; 6]) -> u32 {
        let memory = self.memory;
        let body = memory.read_u32(fixed.mover);
        let header = memory.read_u32(fixed.mover + 4);
        let record_count = memory.read_u32(header) as i32;
        let record = header.wrapping_add(
            fixed
                .record_index
                .wrapping_sub(record_count)
                .wrapping_mul(0xC8) as u32,
        );
        if memory.read_u16(detail + 6) == 0 {
            return self.inner(fixed, detail, aabb, header);
        }
        let count = u32::from(memory.read_u16(detail + 0xA));
        let table = memory.read_u32(fixed.bounds + 0x1C);
        let mut result = 0u32;
        for step in 0..count {
            let index = u32::from(memory.read_u16(detail + 8)) + step;
            let entry = table.wrapping_add(index.wrapping_mul(12));
            let selector_byte = memory.read_u8(entry + 4);
            let selector = u32::from(selector_byte >> 6);
            let item = memory
                .read_u32(record + 0x60 + 4 * selector)
                .wrapping_add(u32::from(memory.read_u16(entry + 6)) * 10);
            let first = memory.read_u16(item) as i16;
            if first >= 0 {
                let parts = memory.read_u32(header + 0x14);
                let flags =
                    memory.read_u32(parts.wrapping_add((i32::from(first) * 0x30) as u32) + 0x1C);
                if flags & 2 != 0 {
                    if flags & 1 == 0 {
                        continue;
                    }
                } else if fixed.arg9 & flags != 0 || selector_byte & 0xC0 == 0x40 {
                    continue;
                }
            }
            let quantised = memory.read_u32(header + 0xC) & 0x10;
            let plane = record + selector * 24;
            let sub_index = u32::from(memory.read_u16(item + 6));
            let sub_base = memory.read_u32(plane + 4);
            let sub_record = if quantised != 0 {
                sub_base.wrapping_add(sub_index * 8)
            } else {
                sub_base.wrapping_add(sub_index << 4)
            };
            let hit = self.triangles.test(&TriangleTestCall {
                sweep: *fixed.sweep,
                sub_record_address: sub_record,
                bounds_address: fixed.bounds,
                header_plus_0x38: header + 0x38,
                quantised,
                table_address: table,
                entry_address: entry,
                flag: u32::from(selector_byte & 0xC0 == 0),
                list_flag5: self.list_flag5,
            });
            if hit.code == 0 || !(ONE > hit.fraction) {
                continue;
            }
            let (mut position, mut normal) = (hit.position, hit.normal);
            let body_flags = memory.read_u32(body + 0x28);
            if memory.read_u32(header + 4) as i32 > 0
                && body_flags & 8 != 0
                && body_flags & 0x20_0000 != 0
            {
                let part_matrix_index = i32::from(memory.read_u8(record + 0x49) as i8);
                let part = read_matrix(
                    memory,
                    memory
                        .read_u32(fixed.mover + 0x10)
                        .wrapping_add((part_matrix_index << 6) as u32),
                );
                position = transform_point(&part, position);
                transform_direction(&part, &mut normal);
            }
            if memory.read_u8(body + 0x32) != 0 {
                position = transform_point(fixed.matrix, position);
                transform_direction(fixed.matrix, &mut normal);
            }
            let Some(slot) = self.pool.allocate() else {
                return result;
            };
            let part_kind = (first >= 0).then(|| {
                let x = memory.read_u32(body + 4);
                let parts = memory.read_u32(memory.read_u32(x + 4) + 0x14);
                memory.read_u8(parts.wrapping_add((i32::from(first) * 0x30) as u32) + 0x18)
            });
            init_impact_node(
                &mut self.pool.nodes[slot],
                position,
                normal,
                hit.fraction,
                hit.f18_bits,
                fixed.arg5 as u16,
                body,
                fixed.other,
                i32::from(first),
                part_kind,
                memory.read_u32(body + 0x80),
            );
            insert_sorted(self.list, self.pool, slot);
            result |= hit.code;
        }
        result
    }

    fn inner(&mut self, fixed: &Fixed, detail: u32, aabb: &[f32; 6], header: u32) -> u32 {
        let memory = self.memory;
        if memory.read_u8(header + 0xC) & 0x80 == 0 {
            return 0;
        }
        let extent = [
            (aabb[3] - aabb[0]) * QUANT_SCALE,
            (aabb[4] - aabb[1]) * QUANT_SCALE,
            (aabb[5] - aabb[2]) * QUANT_SCALE,
        ];
        let nodes = memory.read_u32(fixed.bounds + 0x24);
        let mut result = 0u32;
        for child_field in [8u32, 0xA] {
            let child = nodes.wrapping_add(u32::from(memory.read_u16(detail + child_field)) * 12);
            let mut local = [0.0f32; 6];
            for axis in 0..3 {
                let low = f32::from(memory.read_u8(child + axis as u32));
                let high = f32::from(memory.read_u8(child + 3 + axis as u32));
                local[axis] = low * extent[axis] + aabb[axis];
                local[3 + axis] = high * extent[axis] + aabb[axis];
            }
            if fixed.sweep.overlaps(&local) {
                result |= self.walk(fixed, child, &local);
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x128760: dynamic object query
// ---------------------------------------------------------------------------------------------------------------

/// The 10 arguments of the stubbed `0x4B3B0` (swept sphere against one dynamic object).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicShapeCall {
    pub start: [f32; 3],
    pub delta: [f32; 3],
    pub radius: f32,
    pub object: u32,
}

/// Outputs of `0x4B3B0`. `f18_bits` and `fraction` are pre-set to 1.0 by the caller; the callee may leave them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicShapeResult {
    pub code: u32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub f18_bits: u32,
    pub fraction: f32,
}

pub trait DynamicShapeTest {
    /// `out` holds the caller's initial scalars (both 1.0); on `code != 0` the vectors are written by the callee.
    fn test(&mut self, call: &DynamicShapeCall, out: &mut DynamicShapeResult);
}

/// The result struct (`+0` count, `+4` untouched, entries of 13 dwords from `+8`, at most 5).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DynamicResult {
    pub count: i32,
    pub entries: Vec<[u32; 13]>,
}

/// `0x128760(body, other, start, delta, radius, result, list)`, cdecl. For each object pointer of `[body + 0x1F8]`
/// (`[body + 0x1C]` of them) whose first float is `>= 0.0`, `0x4B3B0` is asked; a contact allocates a pool node
/// (position, normal, `other`, `body`, kind 2, flags 0, `f18`, fraction) inserted sorted into `list`, and, while
/// `result.count < 5`, appends an entry: `[body, position, delta, object[+0x30..+0x3C], 0.0, body[+0x34], object]`.
/// Always returns 0 (MEASURED). `pool.nodes[slot]` words not named here keep their old value.
#[allow(clippy::too_many_arguments)]
pub fn dynamic_query(
    memory: &dyn GuestMemory,
    shapes: &mut dyn DynamicShapeTest,
    pool: &mut ImpactPool,
    list: &mut ImpactList,
    body: u32,
    other: u32,
    start: [f32; 3],
    delta: [f32; 3],
    radius: f32,
    result: &mut DynamicResult,
) {
    let array = memory.read_u32(body + 0x1F8);
    if array == 0 {
        return;
    }
    let count = memory.read_u32(body + 0x1C) as i32;
    for index in 0..count.max(0) as u32 {
        let object = memory.read_u32(array + 4 * index);
        if !(memory.read_f32(object) >= 0.0) {
            continue;
        }
        let mut out = DynamicShapeResult {
            code: 0,
            position: [0.0; 3],
            normal: [0.0; 3],
            f18_bits: ONE.to_bits(),
            fraction: ONE,
        };
        shapes.test(
            &DynamicShapeCall {
                start,
                delta,
                radius,
                object,
            },
            &mut out,
        );
        if out.code == 0 {
            continue;
        }
        if let Some(slot) = pool.allocate() {
            let node: &mut ImpactNode = &mut pool.nodes[slot];
            for axis in 0..3 {
                node[axis] = out.position[axis].to_bits();
                node[3 + axis] = out.normal[axis].to_bits();
            }
            node[9] = other;
            node[10] = body;
            node[11] = 2 << 16; // half +0x2C = 0, half +0x2E = 2
            node[7] = out.fraction.to_bits();
            node[6] = out.f18_bits;
            insert_sorted(list, pool, slot);
        }
        if result.count < 5 {
            let mut entry = [0u32; 13];
            entry[0] = body;
            for axis in 0..3 {
                entry[1 + axis] = out.position[axis].to_bits();
                entry[4 + axis] = delta[axis].to_bits();
                entry[7 + axis] = memory.read_u32(object + 0x30 + 4 * axis as u32);
            }
            entry[10] = 0;
            entry[11] = memory.read_u32(body + 0x34);
            entry[12] = object;
            result.entries.push(entry);
            result.count += 1;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x129F50: apply step
// ---------------------------------------------------------------------------------------------------------------

/// `[0x475D10]` = 2.0 and `[0x475CD4]` = 0.5 (MEASURED constants).
const REFLECT_FACTOR: f32 = 2.0;
const WEIGHT_SCALE: f32 = 0.5;

/// The fields of a dynamic object read by `0x129F50`: triangle corners at `+0x40`, `+0x5C`, `+0x94` and the byte at
/// `+0x29` (bit 0 = reflect the direction about the contact normal).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicObjectGeometry {
    pub corner0: [f32; 3],
    pub corner1: [f32; 3],
    pub corner2: [f32; 3],
    pub flags_byte_29: u8,
}

impl DynamicObjectGeometry {
    pub fn read(memory: &dyn GuestMemory, object: u32) -> Self {
        Self {
            corner0: read_vec3(memory, object + 0x40),
            corner1: read_vec3(memory, object + 0x5C),
            corner2: read_vec3(memory, object + 0x94),
            flags_byte_29: memory.read_u8(object + 0x29),
        }
    }
}

/// Call of the stubbed `0x129CA0` (register convention: `edi` = object, `ebx` = `w0`; cdecl stack args).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactEffectCall {
    pub w0: u32,
    pub object: u32,
    /// Arg 1: two floats, the contact position expressed along the triangle edges (`dot(p - c0, e1) / |e1|^2` and the
    /// same for `e2`).
    pub edge_coordinates: [f32; 2],
    /// Arg 2: the unit movement direction (reflected when `flags_byte_29 & 1`).
    pub direction: [f32; 3],
    /// Arg 3: the weight (scaled by 0.5 when reflected).
    pub weight: f32,
    /// Arg 4 (pointee): the contact position.
    pub position: [f32; 3],
}

/// The two effect spawners `0x129F50` calls: `0x129C20(position)` (ambient effect) then `0x129CA0` (contact effect).
pub trait ImpactEffects {
    fn spawn_ambient(&mut self, position: [f32; 3]);
    fn spawn_contact(&mut self, call: &ContactEffectCall);
}

/// `0x129F50(w0, object, position, normal, movement, weight)` (cdecl, 6 stack args; float args by value). x87 parts are
/// f64 with one rounding at each store (MEASURED). `position` is `vector_a`, `normal` `vector_c`, `movement`
/// `vector_b` of the `DynamicEntry`; the sweep passes weight 1.0.
#[allow(clippy::too_many_arguments)]
pub fn apply_dynamic_contact(
    w0: u32,
    object: u32,
    geometry: &DynamicObjectGeometry,
    position: [f32; 3],
    normal: [f32; 3],
    movement: [f32; 3],
    weight: f32,
    effects: &mut dyn ImpactEffects,
) {
    let [c0, c1, c2] = [geometry.corner0, geometry.corner1, geometry.corner2];
    // |movement| in f64 (x87), summed (x^2 + y^2) + z^2.
    let (bx, by, bz) = (
        f64::from(movement[0]),
        f64::from(movement[1]),
        f64::from(movement[2]),
    );
    let length = (((bx * bx) + (by * by)) + (bz * bz)).sqrt();
    // SSE part, plain f32 in listing order.
    let (px, py, pz) = (
        position[0] - c0[0],
        position[1] - c0[1],
        position[2] - c0[2],
    );
    let (ux, uy, uz) = (c1[0] - c0[0], c1[1] - c0[1], c1[2] - c0[2]);
    let (vx, vy, vz) = (c2[0] - c0[0], c2[1] - c0[1], c2[2] - c0[2]);
    let dot_u = (pz * uz + py * uy) + px * ux;
    let length_u = (uz * uz + uy * uy) + ux * ux;
    let dot_v = (pz * vz + py * vy) + px * vx;
    let length_v = (vz * vz + vy * vy) + vx * vx;
    let edge_coordinates = [dot_u / length_u, dot_v / length_v];
    let inverse = 1.0f64 / length;
    let mut direction = [
        (inverse * bx) as f32,
        (inverse * by) as f32,
        (inverse * bz) as f32,
    ];
    let mut weight = weight;
    if geometry.flags_byte_29 & 1 != 0 {
        let dot = (normal[1] * direction[1] + normal[2] * direction[2]) + normal[0] * direction[0];
        let twice = dot * REFLECT_FACTOR;
        direction = [
            direction[0] - twice * normal[0],
            direction[1] - normal[1] * twice,
            direction[2] - normal[2] * twice,
        ];
        weight *= WEIGHT_SCALE;
    }
    effects.spawn_ambient(position);
    effects.spawn_contact(&ContactEffectCall {
        w0,
        object,
        edge_coordinates,
        direction,
        weight,
        position,
    });
}
