//! T1855: the shape scan root `0xD9760(object, from, direction, end, flag, &point, &aux, extra1)` of kind 8 shape
//! objects, called by the dynamic object scan `0xA2CD0` (`sweep_cells::ScanCallees::shape_scan`).
//!
//! Decoded from `build/default.xbe` (704 instructions) and checked bit for bit against the original running under
//! the Unicorn oracle (`tools/t1846_gold/shape_scan.py`, goldens `shape_scan_d9760_script` with the eight private
//! callees scripted and `shape_scan_d9760_real` with all of them real). See
//! `docs/t-story-event-shape-scan-root-t1855.md`. Names are INFERRED, control flow, layouts and arithmetic MEASURED.
//!
//! The root has four ways out:
//!
//! 1. extra1 bit `0x20` with a link record of type `0x46`: a sphere test (`0x49200`) around the link owner.
//! 2. extra1 bit `0` set, or the object lacks flag `0x200000`: the link record ends (`0xD9510`) go to the oriented
//!    shape test `0x48FE0`.
//! 3. object flag `0x100`: no hit.
//! 4. otherwise: a segment against box prefilter and ray against box (`0x475C0`), the ray moved into the object
//!    frame (rotation about y by the object angle, `0x153440` / `0x1530E0` / `0x151E80`), and every element of the
//!    class list is tested: its bone capsule (`0x48FE0`) must be hit first, then either the capsule hit itself is the
//!    candidate (extra1 bit 1 clear) or the triangles of the element groups (`0x12820`, `0x12BA0`, `0x12A90`,
//!    `0xD6AA0`, inline vertex decode and `0x477B0`) are ray tested. The nearest candidate (squared distance in the
//!    object frame) wins and is moved back to world space; its element index is stored in the global `0x7356E0`.
//!
//! The eight private callees are the [`ShapeParts`] trait; `shape_parts` and `shape_obb` hold their ports and
//! [`RealShapeParts`] ties them together.

// `!(a > b)` keeps the unordered (NaN) behaviour of comiss + jbe.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use super::shape_obb::shape_hit_48fe0;
use super::shape_parts::{
    decode_vertex, link_shape_ends, ray_vs_box, ray_vs_sphere, second_stream_address,
    transform_by_bone_matrix, vertex_slot_address,
};
use super::sweep_cells::ray_triangle;
use super::sweep_records::{GuestMemory, transform_point};
use super::world_queries::{Mat4, mat_rotation_y, transpose};

pub type Vec3 = [f32; 3];
/// One decoded vertex slot: x, y, z as float bits and the flag word `w` (bit 0 = emit a triangle).
pub type Vertex = [u32; 4];

/// `[0x475D0C]` = pi / 180 (MEASURED).
pub const DEGREES_TO_RADIANS: f32 = f32::from_bits(0x3C8E_FA35);
/// `[0x475D5C]` = `f32::MAX`: the initial best squared distance (MEASURED).
pub const FAR: f32 = f32::MAX;
/// `[0x475C9C]` = 1/1024: scale of the 16 bit quantised vertices (MEASURED).
pub const VERTEX_SCALE: f32 = f32::from_bits(0x3A80_0000);
/// Global that receives the element index of the best hit (MEASURED).
pub const BEST_ELEMENT_GLOBAL: u32 = 0x73_56E0;

// Object fields.
const OBJECT_CLASS: u32 = 0x04;
const OBJECT_FLAGS: u32 = 0x28;
const OBJECT_POSITION: u32 = 0x64;
const OBJECT_ANGLE: u32 = 0x70;
const OBJECT_LINK: u32 = 0x7C;
const OBJECT_BOX_OFFSET: u32 = 0x1A8;
const OBJECT_BOX_HALF: u32 = 0x1C0;
/// Flag `0x200000`: the object has a bone shape list, otherwise the link record ends are used.
const FLAG_SHAPE_LIST: u32 = 0x20_0000;
/// Flag `0x100`: the shape is hidden.
const FLAG_HIDDEN: u32 = 0x100;
// extra1 bits.
const EXTRA_LINK_ENDS: u32 = 1;
const EXTRA_TRIANGLES: u32 = 2;
const EXTRA_SPHERE: u32 = 0x20;
// Link record.
const LINK_OWNER: u32 = 0x00;
const LINK_TYPE: u32 = 0x48;
const LINK_SPHERE_SHIFT: u32 = 0x9DC;
const LINK_SPHERE_RADIUS: u32 = 0x9E0;
const LINK_TYPE_SPHERE: u16 = 0x46;
// Class: +4 element list, +0x10 bone matrices, +0x8C record with the element type table at +0x5C.
const CLASS_LIST: u32 = 0x04;
const CLASS_MATRICES: u32 = 0x10;
const CLASS_TYPES: u32 = 0x8C;
const TYPES_TABLE: u32 = 0x5C;
const ELEMENT_SIZE: u32 = 0xC8;
/// Element type whose capsule hit is accepted without a distance test (INFERRED: a primary shape).
const ELEMENT_TYPE_PRIMARY: u32 = 4;
// Element fields.
const ELEMENT_FACES: u32 = 0x00;
const ELEMENT_BONE_MODE: u32 = 0x48;
const ELEMENT_BONE: u32 = 0x49;
const ELEMENT_PREVIOUS: u32 = 0x4A;
const ELEMENT_NEXT: u32 = 0x4B;
const ELEMENT_GROUPS: u32 = 0x60;
const ELEMENT_CAPSULE: u32 = 0xAC;
// Capsule record: two end points, parameter, two bone indices.
const CAPSULE_PARAMETER: u32 = 0x18;
const CAPSULE_BONE_A: u32 = 0x1C;
const CAPSULE_BONE_B: u32 = 0x1E;
// List header: +0 element count, +0xC flags (bit 4 = 16 bit quantised vertices), +0x38 quantisation origin.
const LIST_FLAGS: u32 = 0x0C;
const LIST_ORIGIN: u32 = 0x38;
const LIST_QUANTISED: u32 = 0x10;
const GROUP_SIZE: u32 = 10;

/// What a callee that fills the caller's `point` / `aux` buffers did. `None` means the buffer was left untouched.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PartOut {
    pub ret: u32,
    pub point: Option<Vec3>,
    pub aux: Option<Vec3>,
}

/// Result of `0xD9510(link, &end_a, &end_b, &flag)`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinkEndPoints {
    pub end_a: Vec3,
    pub end_b: Vec3,
    pub flag: u32,
}

/// The private callees of `0xD9760` (ABI MEASURED from the call sites, names INFERRED). Float and vertex arguments
/// are passed by value where the original passes pointers to its stack locals.
pub trait ShapeParts {
    /// `0x49200(from*, direction*, centre*, radius_squared, &point, &aux)` cdecl: ray against a sphere.
    fn ray_sphere(
        &mut self,
        from: Vec3,
        direction: Vec3,
        centre: Vec3,
        radius_squared: f32,
    ) -> PartOut;
    /// `0x475C0(from*, direction*, low*, high*)` cdecl: ray against an axis aligned box, returns 0 or non zero.
    fn ray_box(&mut self, from: Vec3, direction: Vec3, low: Vec3, high: Vec3) -> bool;
    /// `0xD9510(link, &end_a, &end_b, &flag)` cdecl: the end points of the link record shape and its parameter.
    fn link_ends(&mut self, link: u32) -> LinkEndPoints;
    /// `0x48FE0(from*, direction*, first*, second*, parameter, &point, &aux)` cdecl: ray against the oriented shape
    /// between two ends. The element scan passes the bone transformed capsule ends `(a, b)`, the link record path
    /// passes `(end_b, end_a)` of `0xD9510`.
    fn ray_shape(
        &mut self,
        from: Vec3,
        direction: Vec3,
        first: Vec3,
        second: Vec3,
        parameter: u32,
    ) -> PartOut;
    /// `0x12820(list, element, group)` cdecl: address of the first vertex of a group.
    fn vertex_cursor(&mut self, list: u32, element: u32, group: u32) -> u32;
    /// `0x12BA0(list, element, group)` cdecl: address of the first index word of a group (0 when the element has no
    /// vertex data).
    fn index_cursor(&mut self, list: u32, element: u32, group: u32) -> u32;
    /// `0x12A90(&out, list, cursor)` cdecl: decodes the vertex at `cursor` into `out`.
    fn read_vertex(&mut self, list: u32, cursor: u32) -> Vertex;
    /// `0xD6AA0` (eax = element, ecx = bone matrices, edx = &vertex, esi = index code, stack = previous element,
    /// next element): moves the vertex by the bone matrix chosen by the element and the code.
    fn skin_vertex(
        &mut self,
        element: u32,
        matrices: u32,
        vertex: Vertex,
        code: u32,
        previous: u32,
        next: u32,
    ) -> Vertex;
}

/// Arguments of `0xD9760` (the pointer arguments are given by value, `&point` and `&aux` are always non null).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShapeScanCall {
    pub object: u32,
    pub from: Vec3,
    pub direction: Vec3,
    pub end: Vec3,
    pub flag: u32,
    pub extra1: u32,
}

/// Everything `0xD9760` returns and writes. `point` / `aux` are `Some` when the original wrote the caller's buffers,
/// `best_element` when it stored the global [`BEST_ELEMENT_GLOBAL`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShapeScanOutcome {
    pub ret: u32,
    pub point: Option<Vec3>,
    pub aux: Option<Vec3>,
    pub best_element: Option<u32>,
}

impl From<PartOut> for ShapeScanOutcome {
    fn from(out: PartOut) -> Self {
        Self {
            ret: out.ret,
            point: out.point,
            aux: out.aux,
            best_element: None,
        }
    }
}

fn read_vec3(memory: &dyn GuestMemory, address: u32) -> Vec3 {
    [
        memory.read_f32(address),
        memory.read_f32(address + 4),
        memory.read_f32(address + 8),
    ]
}

fn read_matrix(memory: &dyn GuestMemory, address: u32) -> Mat4 {
    std::array::from_fn(|k| memory.read_f32(address + 4 * k as u32))
}

fn signed_byte(memory: &dyn GuestMemory, address: u32) -> i32 {
    i32::from(memory.read_u8(address) as i8)
}

/// Squared distance in the order of the original: `(dz^2 + dy^2) + dx^2`.
fn distance_squared(point: Vec3, rel: Vec3) -> f32 {
    let dy = point[1] - rel[1];
    let dx = point[0] - rel[0];
    let dz = point[2] - rel[2];
    (dz * dz + dy * dy) + dx * dx
}

/// Segment `from -> end` entirely beyond one face of the box (the cheap rejection of `0xD9760` when `flag == 0`).
fn segment_outside_box(from: Vec3, end: Vec3, low: Vec3, high: Vec3) -> bool {
    (0..3).any(|axis| {
        (low[axis] > from[axis] && low[axis] > end[axis])
            || (from[axis] > high[axis] && end[axis] > high[axis])
    })
}

/// The box around the object: centre `position + offset`, half extent at `+0x1C0`, summed in the original order.
fn object_box(memory: &dyn GuestMemory, object: u32) -> (Vec3, Vec3) {
    let position = read_vec3(memory, object + OBJECT_POSITION);
    let offset = read_vec3(memory, object + OBJECT_BOX_OFFSET);
    let half = read_vec3(memory, object + OBJECT_BOX_HALF);
    let low = [
        (position[0] + offset[0]) - half[0],
        (offset[1] + position[1]) - half[1],
        (position[2] + offset[2]) - half[2],
    ];
    let high = [
        (half[0] + position[0]) + offset[0],
        (offset[1] + half[1]) + position[1],
        (position[2] + offset[2]) + half[2],
    ];
    (low, high)
}

/// Normalises the best aux vector the way the original does: x87 with a 53 bit mantissa (modelled in f64), the sum
/// `(z*z + y*y) + x*x`, `1 / sqrt`, every component multiplied and rounded once to f32 (MEASURED).
fn normalise_x87(aux: Vec3) -> Vec3 {
    let (x, y, z) = (f64::from(aux[0]), f64::from(aux[1]), f64::from(aux[2]));
    let mut sum = z * z;
    sum += y * y;
    sum += x * x;
    let inverse = 1.0 / sum.sqrt();
    [
        (x * inverse) as f32,
        (y * inverse) as f32,
        (inverse * z) as f32,
    ]
}

/// The best candidate so far (the stack locals of the original).
struct Best {
    distance: f32,
    point: Vec3,
    aux: Vec3,
    element: u32,
    /// The local at frame offset -0x140: set to "primary type" by the simple candidate path.
    primary: bool,
}

/// `0xD9760`. Reads the object graph through `memory`, calls the private callees through `parts`.
pub fn shape_scan_d9760(
    memory: &dyn GuestMemory,
    parts: &mut dyn ShapeParts,
    call: &ShapeScanCall,
) -> ShapeScanOutcome {
    let object = call.object;
    let class = memory.read_u32(object + OBJECT_CLASS);
    let list = memory.read_u32(class + CLASS_LIST);
    let elements = list.wrapping_sub(memory.read_u32(list).wrapping_mul(ELEMENT_SIZE));
    let link = memory.read_u32(object + OBJECT_LINK);

    if call.extra1 & EXTRA_SPHERE != 0 && memory.read_u16(link + LINK_TYPE) == LINK_TYPE_SPHERE {
        let owner = memory.read_u32(link + LINK_OWNER);
        let radius = memory.read_f32(link + LINK_SPHERE_RADIUS);
        let centre = [
            memory.read_f32(owner + OBJECT_POSITION) + memory.read_f32(link + LINK_SPHERE_SHIFT),
            memory.read_f32(owner + OBJECT_POSITION + 4),
            memory.read_f32(owner + OBJECT_POSITION + 8),
        ];
        return parts
            .ray_sphere(call.from, call.direction, centre, radius * radius)
            .into();
    }
    let flags = memory.read_u32(object + OBJECT_FLAGS);
    if call.extra1 & EXTRA_LINK_ENDS != 0 || flags & FLAG_SHAPE_LIST == 0 {
        let ends = parts.link_ends(link);
        return parts
            .ray_shape(call.from, call.direction, ends.end_b, ends.end_a, ends.flag)
            .into();
    }
    if flags & FLAG_HIDDEN != 0 {
        return ShapeScanOutcome::default();
    }

    let (low, high) = object_box(memory, object);
    if call.flag == 0 && segment_outside_box(call.from, call.end, low, high) {
        return ShapeScanOutcome::default();
    }
    if !parts.ray_box(call.from, call.direction, low, high) {
        return ShapeScanOutcome::default();
    }

    // Move the ray into the object frame.
    let position = read_vec3(memory, object + OBJECT_POSITION);
    let angle = memory.read_f32(object + OBJECT_ANGLE) * DEGREES_TO_RADIANS;
    let forward = mat_rotation_y(angle);
    let inverse = transpose(&forward);
    let relative = [
        call.from[0] - position[0],
        call.from[1] - position[1],
        call.from[2] - position[2],
    ];
    let relative = transform_point(&inverse, relative);
    let direction = transform_point(&inverse, call.direction);

    let count = memory.read_u32(list) as i32;
    let mut best = Best {
        distance: FAR,
        point: [0.0; 3],
        aux: [0.0, 1.0, 0.0],
        element: 0,
        primary: false,
    };
    let matrices = memory.read_u32(class + CLASS_MATRICES);
    for index in 0..count.max(0) as u32 {
        let element = elements.wrapping_add(index.wrapping_mul(ELEMENT_SIZE));
        let neighbour = |offset: u32| {
            let relative_index = signed_byte(memory, element + offset);
            if relative_index >= 0 {
                elements.wrapping_add((relative_index as u32).wrapping_mul(ELEMENT_SIZE))
            } else {
                0
            }
        };
        let (previous, next) = (neighbour(ELEMENT_PREVIOUS), neighbour(ELEMENT_NEXT));
        let capsule = memory.read_u32(element + ELEMENT_CAPSULE);
        if capsule == 0 {
            continue;
        }
        let bone_end = |offset: u32, base: u32| {
            let bone = u32::from(memory.read_u16(capsule + offset));
            let matrix = read_matrix(memory, (bone << 6).wrapping_add(matrices));
            transform_point(&matrix, read_vec3(memory, base))
        };
        let end_a = bone_end(CAPSULE_BONE_A, capsule);
        let end_b = bone_end(CAPSULE_BONE_B, capsule + 12);
        let parameter = memory.read_u32(capsule + CAPSULE_PARAMETER);
        let shape = parts.ray_shape(relative, direction, end_a, end_b, parameter);
        if shape.ret == 0 {
            continue;
        }
        let shape_point = shape.point.unwrap_or_default();
        let shape_aux = shape.aux.unwrap_or_default();
        if call.extra1 & EXTRA_TRIANGLES == 0 {
            let types = memory.read_u32(memory.read_u32(class + CLASS_TYPES) + TYPES_TABLE);
            let primary = memory.read_u32(types.wrapping_add(4 * index)) == ELEMENT_TYPE_PRIMARY;
            if !primary && best.primary {
                continue;
            }
            let distance = distance_squared(shape_point, relative);
            best.primary = primary;
            if primary || best.distance > distance {
                best.aux = shape_aux;
                best.point = shape_point;
                best.distance = distance;
                best.element = index;
            }
            continue;
        }
        let mut scan = TriangleScan {
            memory,
            parts: &mut *parts,
            list,
            matrices,
            element,
            previous,
            next,
            relative,
            direction,
            index,
        };
        scan.run(&mut best);
    }
    if !(FAR > best.distance) {
        return ShapeScanOutcome::default();
    }
    // Back to world space.
    let local_point = transform_point(&forward, best.point);
    let point = [
        local_point[0] + position[0],
        local_point[1] + position[1],
        position[2] + local_point[2],
    ];
    let aux = transform_point(&forward, normalise_x87(best.aux));
    ShapeScanOutcome {
        ret: 1,
        point: Some(point),
        aux: Some(aux),
        best_element: Some(best.element),
    }
}

/// The triangle groups of one element (the `extra1 & 2` branch).
struct TriangleScan<'a> {
    memory: &'a dyn GuestMemory,
    parts: &'a mut dyn ShapeParts,
    list: u32,
    matrices: u32,
    element: u32,
    previous: u32,
    next: u32,
    relative: Vec3,
    direction: Vec3,
    index: u32,
}

impl TriangleScan<'_> {
    fn bone_matrix_of(&self, owner: u32) -> Mat4 {
        let bone = signed_byte(self.memory, owner + ELEMENT_BONE) as u32;
        read_matrix(self.memory, (bone << 6).wrapping_add(self.matrices))
    }

    /// The inline copy of `0xD6AA0` used for the vertices after the first two of a face.
    fn skin(&self, mut vertex: Vertex, code: u32) -> Vertex {
        let owner = if self.memory.read_u8(self.element + ELEMENT_BONE_MODE) == 0 {
            self.element
        } else {
            match code {
                0 => self.previous,
                8 => self.next,
                0x10 => self.element,
                _ => return vertex,
            }
        };
        let moved = transform_point(&self.bone_matrix_of(owner), vertex_xyz(vertex));
        vertex[..3].copy_from_slice(&moved.map(f32::to_bits));
        vertex
    }

    /// The inline vertex decode (same arithmetic as `0x12A90`).
    fn decode(&self, cursor: u32) -> Vertex {
        let memory = self.memory;
        if memory.read_u32(self.list + LIST_FLAGS) & LIST_QUANTISED == 0 {
            return std::array::from_fn(|k| memory.read_u32(cursor + 4 * k as u32));
        }
        let axis = |k: u32| {
            let raw = f32::from(memory.read_u16(cursor + 2 * k) as i16);
            (raw * VERTEX_SCALE + memory.read_f32(self.list + LIST_ORIGIN + 4 * k)).to_bits()
        };
        let flags = i32::from(memory.read_u16(cursor + 6) as i16) as u32 | 0x3F80_0000;
        [axis(0), axis(1), axis(2), flags]
    }

    fn run(&mut self, best: &mut Best) {
        let memory = self.memory;
        let mut group = memory.read_u32(self.element + ELEMENT_GROUPS);
        if signed_byte(memory, group + 3) < 0 {
            return;
        }
        let quantised = memory.read_u32(self.list + LIST_FLAGS) & LIST_QUANTISED != 0;
        let stride = if quantised { 8 } else { 16 };
        loop {
            let first = memory.read_u16(group + 4) as i16 as i32 as u32;
            let mut face = memory
                .read_u32(self.element + ELEMENT_FACES)
                .wrapping_add(first << 4);
            let mut vertex_cursor = self.parts.vertex_cursor(self.list, self.element, group);
            let mut index_cursor = self.parts.index_cursor(self.list, self.element, group);
            loop {
                let word = memory.read_u32(face);
                let last = word & 0x8000 != 0;
                let vertices = (word & 0x7FFF) as i32;
                let restart_parity = (!(word >> 16)) & 1;
                let mut parity = restart_parity;
                // The first two vertices of a face go through the callees.
                let mut ring: [Vertex; 3] = [[0; 4]; 3];
                for slot in ring.iter_mut().take(2) {
                    let vertex = self.parts.read_vertex(self.list, vertex_cursor);
                    vertex_cursor = vertex_cursor.wrapping_add(stride);
                    let code = memory
                        .read_u32(index_cursor)
                        .wrapping_mul(2)
                        .wrapping_add(0x40);
                    index_cursor = index_cursor.wrapping_add(0x10);
                    *slot = self.parts.skin_vertex(
                        self.element,
                        self.matrices,
                        vertex,
                        code,
                        self.previous,
                        self.next,
                    );
                }
                if vertices > 2 {
                    let mut phase = 0usize;
                    let mut load = 2usize;
                    for _ in 0..vertices - 2 {
                        let decoded = self.decode(vertex_cursor);
                        vertex_cursor = vertex_cursor.wrapping_add(stride);
                        let code = memory
                            .read_u32(index_cursor)
                            .wrapping_mul(2)
                            .wrapping_add(0x40);
                        index_cursor = index_cursor.wrapping_add(0x10);
                        ring[load] = self.skin(decoded, code);
                        let first_vertex = ring[phase];
                        let (mut second, mut third) = (ring[(phase + 1) % 3], ring[load]);
                        if third[3] & 1 != 0 {
                            if parity != 0 {
                                std::mem::swap(&mut second, &mut third);
                            }
                            if let Some(hit) = ray_triangle(
                                self.relative,
                                self.direction,
                                vertex_xyz(first_vertex),
                                vertex_xyz(second),
                                vertex_xyz(third),
                            ) {
                                let distance = distance_squared(hit.point, self.relative);
                                if best.distance > distance {
                                    best.aux = hit.normal;
                                    best.point = hit.point;
                                    best.distance = distance;
                                    best.element = self.index;
                                }
                            }
                            parity = 1 - parity;
                        } else {
                            parity = restart_parity;
                        }
                        load = phase;
                        phase = (phase + 1) % 3;
                    }
                }
                if last {
                    break;
                }
                face = face.wrapping_add(0x10);
            }
            let next_marker = signed_byte(memory, group + GROUP_SIZE + 3);
            group = group.wrapping_add(GROUP_SIZE);
            if next_marker < 0 {
                break;
            }
        }
    }
}

fn vertex_xyz(vertex: Vertex) -> Vec3 {
    [
        f32::from_bits(vertex[0]),
        f32::from_bits(vertex[1]),
        f32::from_bits(vertex[2]),
    ]
}

/// [`ShapeParts`] over the ported callees (`shape_parts`, `shape_obb`). `world_matrix` is `0xA14B0(out, object, 1, 0)`
/// (`world_queries::build_world_matrix`), only needed by `0xD9510` for owner objects of kind 2.
pub struct RealShapeParts<'a> {
    pub memory: &'a dyn GuestMemory,
    pub world_matrix: &'a mut dyn FnMut(u32) -> Mat4,
}

impl ShapeParts for RealShapeParts<'_> {
    fn ray_sphere(
        &mut self,
        from: Vec3,
        direction: Vec3,
        centre: Vec3,
        radius_squared: f32,
    ) -> PartOut {
        match ray_vs_sphere(from, direction, centre, radius_squared) {
            Some(hit) => PartOut {
                ret: 1,
                point: Some(hit.point),
                aux: Some(hit.offset),
            },
            None => PartOut::default(),
        }
    }

    fn ray_box(&mut self, from: Vec3, direction: Vec3, low: Vec3, high: Vec3) -> bool {
        ray_vs_box(from, direction, low, high).hit()
    }

    fn link_ends(&mut self, link: u32) -> LinkEndPoints {
        let (ends, _path) = link_shape_ends(self.memory, link, &mut *self.world_matrix);
        LinkEndPoints {
            end_a: ends.upper,
            end_b: ends.lower,
            flag: ends.radius_bits,
        }
    }

    fn ray_shape(
        &mut self,
        from: Vec3,
        direction: Vec3,
        first: Vec3,
        second: Vec3,
        parameter: u32,
    ) -> PartOut {
        match shape_hit_48fe0(from, direction, first, second, f32::from_bits(parameter)) {
            Some(contact) => PartOut {
                ret: 1,
                point: Some(contact.position),
                aux: Some(contact.normal),
            },
            None => PartOut::default(),
        }
    }

    fn vertex_cursor(&mut self, list: u32, element: u32, group: u32) -> u32 {
        vertex_slot_address(self.memory, list, element, group)
    }

    fn index_cursor(&mut self, list: u32, element: u32, group: u32) -> u32 {
        second_stream_address(self.memory, list, element, group)
    }

    fn read_vertex(&mut self, list: u32, cursor: u32) -> Vertex {
        decode_vertex(self.memory, list, cursor)
    }

    fn skin_vertex(
        &mut self,
        element: u32,
        matrices: u32,
        mut vertex: Vertex,
        code: u32,
        previous: u32,
        next: u32,
    ) -> Vertex {
        let moved = transform_by_bone_matrix(
            self.memory,
            element,
            previous,
            next,
            code,
            matrices,
            vertex_xyz(vertex),
        );
        if let Some(point) = moved {
            vertex[..3].copy_from_slice(&point.map(f32::to_bits));
        }
        vertex
    }
}
