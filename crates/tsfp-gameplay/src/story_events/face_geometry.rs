//! Room face geometry callees of the blast code (T1846 batch 8): `0x3675E0`, `0x3680A0`, `0x368280`, `0x367820`,
//! `0x367CE0` and the room queries they use (`0x877B0`, `0x3660B0`, `0x87720`).
//!
//! Decoded from `build/default.xbe` with `tools/x87_blocks.py` and checked bit for bit against the original running under
//! Unicorn (`tools/x86_oracle.py`, `tools/t1846_golden.py`, vectors in `tests/data/t1846/face_*.golden`): MEASURED for the
//! control flow and arithmetic on synthetic worlds. Names are INFERRED. The image uses `rsqrtss` where this port divides by
//! a square root: QEMU (xemu, Unicorn) implements `rsqrtss` as `1 / sqrt`, so the port matches xemu, not necessarily silicon.
//! Detail: `docs/t-story-event-callees-t1846.md` section 5.

use std::collections::HashMap;

use super::callees_t1846::{rescale_vector, rescale_vector_ordered};
use super::hooks_scene::{
    BLAST_ROOM_IDS, BlastGeometry, BlastPiece, Blob, BlobFace, RoomBox, Vec3,
};

/// The record header the face callees receive in `esi` (`eax` for `0x368280`/`0x367CE0`): a blob at `+0x5F0` or a piece
/// at `+0`. Offsets are within the header.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaceWork {
    /// `+0x00`: the moving point.
    pub p: Vec3,
    /// `+0x0C`, `+0x10`, `+0x14`: heading words (`+0x10` receives `atan2(nx, nz)`).
    pub heading: Vec3,
    /// `+0x18`: the direction (velocity) vector.
    pub q: Vec3,
    /// `+0x24`: attached flag.
    pub attached: i32,
    /// `+0x28`: room id.
    pub room: i32,
    /// `+0x2C`: set to 3 by `0x367CE0` when the room has no faces.
    pub state: i32,
    /// `+0x3C`: face id (0 = null).
    pub face: u32,
}

impl FaceWork {
    /// The header at `blob + 0x5F0`.
    pub fn from_blob(blob: &Blob) -> FaceWork {
        FaceWork {
            p: blob.work_position,
            heading: blob.heading,
            q: blob.work_direction,
            attached: blob.attached,
            room: blob.room,
            state: blob.state_61c,
            face: blob.face.id,
        }
    }

    /// Writes the header back (the face record copy is refreshed by the caller from the world).
    pub fn store_blob(&self, blob: &mut Blob) {
        blob.work_position = self.p;
        blob.heading = self.heading;
        blob.work_direction = self.q;
        blob.attached = self.attached;
        blob.room = self.room;
        blob.state_61c = self.state;
    }
}

impl FaceWork {
    /// The header at the start of a pool piece (`+0x2C` is its status word).
    pub fn from_piece(piece: &BlastPiece) -> FaceWork {
        FaceWork {
            p: piece.position,
            heading: [piece.heading_a, piece.heading_b, piece.spin],
            q: piece.velocity,
            attached: piece.attached,
            room: piece.room,
            state: piece.state as i32,
            face: piece.face.id,
        }
    }

    pub fn store_piece(&self, piece: &mut BlastPiece) {
        piece.position = self.p;
        piece.heading_a = self.heading[0];
        piece.heading_b = self.heading[1];
        piece.spin = self.heading[2];
        piece.velocity = self.q;
        piece.attached = self.attached;
        piece.room = self.room;
        piece.state = self.state as u32;
    }
}

/// A room object as `0x877B0` returns it: `+0x1E4` face count and `+0x1F4` the face pointer array.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoomObject {
    pub faces: Vec<u32>,
}

/// The tables the face callees read, with the blast struct flag `[0x76C238] + 0x3360` (the heading write is pending).
#[derive(Clone, Debug, Default)]
pub struct FaceWorld {
    /// Face records by id (the record address in the image; id 0 is the null pointer).
    pub faces: HashMap<u32, BlobFace>,
    /// `[0x7B9784][room]` (`0x877B0`).
    pub rooms: HashMap<i32, RoomObject>,
    /// The seven room boxes of `[0x76C23C] + 4 + 24 * n` in the order of [`BLAST_ROOM_IDS`] (`0x3660B0`).
    pub boxes: [RoomBox; 7],
    /// `[0x708788][region] + 4`: the portal index list of a region (`0x87720`).
    pub region_portals: HashMap<i32, Vec<usize>>,
    /// `[0x70878C]`: portals as pairs of region ids.
    pub portals: Vec<(i32, i32)>,
    /// `[0x76C238] + 0x3360`.
    pub heading_pending: u32,
}

/// Tolerance used when testing a point against a neighbour room box (`[0x475D24]`, MEASURED 0.1).
pub const ROOM_BOX_TOLERANCE: f32 = 0.1;
/// Tolerance for "the same vertex" in `0x368280` (`[0x4785E4]`, MEASURED 0.2).
pub const VERTEX_TOLERANCE: f32 = 0.2;
/// Slack added to the squared edge length in `0x367CE0` (`[0x475CE8]`, MEASURED 0.05).
pub const EDGE_SLACK: f32 = 0.05;
/// `rsqrtss` guard of the inlined unit vector code (`[0x4765F4]`, MEASURED 1e-7).
pub const UNIT_EPSILON: f32 = f32::from_bits(0x33D6_BF95);

impl FaceWorld {
    /// `0x3660B0(room)`: the box of one of the seven known rooms as (min, max), zeros for any other id.
    pub fn room_box(&self, room: i32) -> (Vec3, Vec3) {
        match BLAST_ROOM_IDS.iter().position(|&id| id as i32 == room) {
            Some(n) => {
                let b = self.boxes[n];
                ([b.min_x, b.ground_y, b.min_z], [b.max_x, b.max_y, b.max_z])
            }
            None => ([0.0; 3], [0.0; 3]),
        }
    }

    /// `0x87720(region, out, max)`: the distinct regions on the other side of the portals of `region`, in portal order
    /// (at most `max` portals are read). The caller's out array holds ten.
    pub fn neighbor_regions(&self, region: i32, max: usize) -> Vec<i32> {
        let Some(list) = self.region_portals.get(&region) else {
            return Vec::new();
        };
        let mut out: Vec<i32> = Vec::new();
        for &index in list.iter().take(max) {
            let (a, b) = self.portals[index];
            let other = if a == region { b } else { a };
            if !out.contains(&other) {
                out.push(other);
            }
        }
        out
    }

    fn face(&self, id: u32) -> BlobFace {
        *self
            .faces
            .get(&id)
            .unwrap_or_else(|| panic!("face {id:#x} is not in the world"))
    }

    /// Faces of a room, `None` when `[0x7B9784][room]` is null.
    pub fn room_faces(&self, room: i32) -> Option<&[u32]> {
        self.rooms.get(&room).map(|object| object.faces.as_slice())
    }

    /// Refreshes the copy kept in a blob or piece.
    pub fn face_copy(&self, id: u32) -> BlobFace {
        if id == 0 {
            BlobFace::default()
        } else {
            self.face(id)
        }
    }
}

/// `0x3675E0` (register convention: `esi` = [`FaceWork`], `edi` = optional output normal, `ebx` = `[0x76C238]`).
///
/// Slides the header's points onto the plane of the face edge. When the edge's x is constant (`f2 == f5`), the pending flag
/// `[ebx+0x3360]` is 0 and no normal is wanted, `p.x = x0` and the function returns. The same for a constant z (`z0 == z1`)
/// with `p.z = z1`. Otherwise the horizontal edge normal `n` is built (`0x366130` of the cross terms `(e1*e2, 0, -e0*e1)`
/// with `a = (0,e1,0)`, `b = (e0,0,e2)`), returned when wanted, the heading `atan2(n.x, n.z)` is stored to `heading[1]`
/// (`heading[0]`, `heading[2]` cleared) when the flag is set (then the flag is cleared), and both points lose their
/// component along `n`: `p -= ((p - A) . n) n`, `q -= (q . n) n`.
///
/// Both callers in the image (`0x368A10`, `0x367CE0`) pass a non-null `edi` (the blob axis, a stack buffer), so the early
/// returns are reachable only through this API.
pub fn clamp_to_face_segment(
    work: &mut FaceWork,
    face: &BlobFace,
    flag: &mut u32,
    want_normal: bool,
) -> Option<Vec3> {
    clamp_impl(work, face, flag, want_normal, false)
}

/// The copy of `0x3675E0` that the compiler inlined into `0x369940` (no output normal). The arithmetic is the same except
/// the extended precision sum of squares inside the normalisation, which runs `(z*z + y*y) + x*x`.
pub fn clamp_to_face_segment_inline(work: &mut FaceWork, face: &BlobFace, flag: &mut u32) {
    clamp_impl(work, face, flag, false, true);
}

fn clamp_impl(
    work: &mut FaceWork,
    face: &BlobFace,
    flag: &mut u32,
    want_normal: bool,
    inline: bool,
) -> Option<Vec3> {
    let (f2, f3, f4, f5, f6, f7) = (face.x0, face.y_low, face.z0, face.x1, face.y_high, face.z1);
    if f2 == f5 && *flag == 0 && !want_normal {
        work.p[0] = f2;
        return None;
    }
    if f4 == f7 && *flag == 0 && !want_normal {
        work.p[2] = f7;
        return None;
    }
    let (e0, e1, e2) = (f5 - f2, f6 - f3, f7 - f4);
    let mut normal = [e2 * e1, e0 * 0.0 - e2 * 0.0, 0.0 - e0 * e1];
    rescale_vector_ordered(&mut normal, [0.0, e1, 0.0], [e0, 0.0, e2], inline);
    let v = [
        work.p[0] + (0.0 - f2),
        work.p[1] + (0.0 - f3),
        work.p[2] + (0.0 - f4),
    ];
    if *flag != 0 {
        work.heading[2] = 0.0;
        work.heading[0] = 0.0;
        work.heading[1] = f64::from(normal[0]).atan2(f64::from(normal[2])) as f32;
        *flag = 0;
    }
    let dot = (v[2] * normal[2] + v[1] * normal[1]) + v[0] * normal[0];
    let push = 0.0 - dot;
    work.p[0] = push * normal[0] + work.p[0];
    work.p[2] = push * normal[2] + work.p[2];
    work.p[1] = push * normal[1] + work.p[1];
    let dot2 = (work.q[1] * normal[1] + work.q[2] * normal[2]) + normal[0] * work.q[0];
    let push2 = 0.0 - dot2;
    work.q[0] = push2 * normal[0] + work.q[0];
    work.q[1] = push2 * normal[1] + work.q[1];
    work.q[2] = push2 * normal[2] + work.q[2];
    want_normal.then_some(normal)
}

/// `0x3680A0` (`esi` = header, `edi` = its face): the failure fallback of `0x368280`/`0x367CE0`'s relatives. Turns the
/// direction back toward the face ranges: x below the range makes `q.x` positive, above it negative; y below the range
/// zeroes the attached flag and pushes the direction along the face normal by `-q.y`; y above it makes `q.y` negative; z as
/// x. The ranges are the face fields `+0x20/+0x28` (x), `+0x0C/+0x18` (y) and `+0x24/+0x2C` (z).
pub fn bounce_inside_face(work: &mut FaceWork, face: &BlobFace) {
    let p = work.p;
    if face.x_low > p[0] {
        work.q[0] = work.q[0].abs();
    } else if p[0] > face.x_high {
        work.q[0] = 0.0 - work.q[0].abs();
    }
    if face.y_low > p[1] {
        work.attached = 0;
        let k = 0.0 - work.q[1];
        work.q[1] = 0.0;
        let (e0, e1, e2) = (
            face.x1 - face.x0,
            face.y_high - face.y_low,
            face.z1 - face.z0,
        );
        // Unlike `0x3675E0` the image stores a plain 0 for the y term here.
        let mut normal = [e2 * e1, 0.0, 0.0 - e0 * e1];
        rescale_vector(&mut normal, [0.0, e1, 0.0], [e0, 0.0, e2]);
        work.q[0] = normal[0] * k + work.q[0];
        work.q[1] = normal[1] * k;
        work.q[2] = normal[2] * k + work.q[2];
    } else if p[1] > face.y_high {
        work.q[1] = 0.0 - work.q[1].abs();
    }
    if face.z_low > p[2] {
        work.q[2] = work.q[2].abs();
    } else if p[2] > face.z_high {
        work.q[2] = 0.0 - work.q[2].abs();
    }
}

fn outside_box(min: Vec3, max: Vec3, p: Vec3, tolerance: f32) -> bool {
    min[0] - tolerance > p[0]
        || p[0] > max[0] + tolerance
        || min[1] - tolerance > p[1]
        || p[1] > max[1] + tolerance
        || min[2] - tolerance > p[2]
        || p[2] > max[2] + tolerance
}

/// The first neighbour room (portal order) that is one of the seven box rooms, differs from `room`, and whose box
/// (grown by [`ROOM_BOX_TOLERANCE`]) contains `p`.
fn neighbor_containing(world: &FaceWorld, room: i32, p: Vec3) -> Option<i32> {
    world
        .neighbor_regions(room, 10)
        .into_iter()
        .find(|&candidate| {
            BLAST_ROOM_IDS.iter().any(|&id| id as i32 == candidate) && candidate != room && {
                let (min, max) = world.room_box(candidate);
                !outside_box(min, max, p, ROOM_BOX_TOLERANCE)
            }
        })
}

/// The inlined unit vector code of `0x368280`: the direction `(x, 0, z)` scaled to unit length, returning `(x', y', z')`
/// where `y'` is `s * 0` (kept for its sign). A zero `x` or `z` divides by the other component's magnitude; otherwise the
/// length is used when its square exceeds [`UNIT_EPSILON`] (`rsqrtss`), and a shorter vector stays unchanged with `y' = 0`.
fn inline_unit(x: f32, z: f32) -> Vec3 {
    if x == 0.0 || z == 0.0 {
        let divisor = if x == 0.0 { z.abs() } else { x.abs() };
        let s = 1.0 / divisor;
        return [s * x, s * 0.0, s * z];
    }
    let len2 = z * z + x * x;
    if len2 > UNIT_EPSILON {
        let s = (1.0 / len2.sqrt()) * 1.0;
        [s * x, s * 0.0, s * z]
    } else {
        [x, 0.0, z]
    }
}

/// `0x368280` (`eax` = header, result in `eax`): move a header that left its face onto a neighbouring face.
///
/// 1. Room box test of the header's point against the room box (`0x3660B0`). Outside: find the first neighbouring room
///    (`0x87720`, up to ten) of the seven box rooms whose box grown by 0.1 contains the point and continue with its faces;
///    none found: [`bounce_inside_face`] and return false.
/// 2. Take the nearer vertex of the current face (squared distance in x/z, the second vertex only when strictly closer
///    does not hold: ties pick vertex B) and look in the room's faces (not the current one) for the face that has a vertex
///    within 0.2 of it in both x and z, whose y range contains the point's y, choosing the largest `|dot|` of the
///    normalised edge directions (the running normalisation of the current direction persists between candidates, as in
///    the image). No candidate: the same fallback.
/// 3. The header's direction `q` is rotated from the current edge frame into the chosen face's frame
///    (`q + d*(v - u)` with `d = q . u`), the face and room are updated and the pending flag is set. Returns true.
pub fn collide_faces(world: &mut FaceWorld, work: &mut FaceWork) -> bool {
    let current_face = world.face_copy(work.face);
    let fail = |work: &mut FaceWork| {
        bounce_inside_face(work, &current_face);
        false
    };
    let p = work.p;
    let (min, max) = world.room_box(work.room);
    let mut room = work.room;
    let outside = min[0] > p[0]
        || p[0] > max[0]
        || min[1] > p[1]
        || p[1] > max[1]
        || min[2] > p[2]
        || p[2] > max[2];
    if outside {
        match neighbor_containing(world, work.room, p) {
            Some(neighbor) => room = neighbor,
            None => return fail(work),
        }
    }
    let Some(object) = world.rooms.get(&room) else {
        return fail(work);
    };
    let candidates = object.faces.clone();
    // Nearest vertex of the current face.
    let (dx0, dz0, dx1, dz1) = (
        current_face.x0 - p[0],
        current_face.z0 - p[2],
        current_face.x1 - p[0],
        current_face.z1 - p[2],
    );
    let d0 = dx0 * dx0 + dz0 * dz0;
    let d1 = dx1 * dx1 + dz1 * dz1;
    let (x, z, first_vertex) = if d1 > d0 {
        (current_face.x0, current_face.z0, true)
    } else {
        (current_face.x1, current_face.z1, false)
    };
    if candidates.is_empty() {
        return fail(work);
    }
    let (edge_x, edge_z) = (
        current_face.x1 - current_face.x0,
        current_face.z1 - current_face.z0,
    );
    let (mut nx, mut nz) = (edge_x, edge_z);
    let mut best = f32::from_bits(0xFF7F_FFFF);
    let mut best_index: Option<usize> = None;
    for (index, &id) in candidates.iter().enumerate() {
        if id == work.face {
            continue;
        }
        let c = world.face(id);
        let near_x = VERTEX_TOLERANCE > (x - c.x0).abs() || VERTEX_TOLERANCE > (x - c.x1).abs();
        if !near_x {
            continue;
        }
        let near_z = VERTEX_TOLERANCE > (z - c.z0).abs() || VERTEX_TOLERANCE > (z - c.z1).abs();
        if !near_z {
            continue;
        }
        if !(p[1] > c.y_low) || !(c.y_high > p[1]) {
            continue;
        }
        let len2 = nz * nz + nx * nx;
        if len2 != 1.0 {
            let r = 1.0 / len2.sqrt();
            let (ax, az) = (r * nx, r * nz);
            nx = ax;
            nz = az;
        }
        let (mut cx, mut cz) = (c.x1 - c.x0, c.z1 - c.z0);
        let clen2 = cz * cz + cx * cx;
        if clen2 != 1.0 {
            let r = 1.0 / clen2.sqrt();
            let (ax, az) = (r * cx, r * cz);
            cx = ax;
            cz = az;
        }
        let dot = (cz * nz + cx * nx).abs();
        if dot > best {
            best = dot;
            best_index = Some(index);
        }
    }
    let Some(best_index) = best_index else {
        return fail(work);
    };
    let chosen_id = candidates[best_index];
    let chosen = world.face(chosen_id);
    let (ux, uz) = if first_vertex {
        (
            current_face.x0 - current_face.x1,
            current_face.z0 - current_face.z1,
        )
    } else {
        (edge_x, edge_z)
    };
    let d0 = (x - chosen.x0).abs() + (z - chosen.z0).abs();
    let d1 = (x - chosen.x1).abs() + (z - chosen.z1).abs();
    let (vx, vz) = if d1 > d0 {
        (chosen.x1 - chosen.x0, chosen.z1 - chosen.z0)
    } else {
        (chosen.x0 - chosen.x1, chosen.z0 - chosen.z1)
    };
    let u = inline_unit(ux, uz);
    let v = inline_unit(vx, vz);
    let dq = (work.q[2] * u[2] + work.q[1] * u[1]) + u[0] * work.q[0];
    work.q[0] = (work.q[0] - dq * u[0]) + dq * v[0];
    work.q[1] = (work.q[1] - dq * u[1]) + dq * v[1];
    work.q[2] = (work.q[2] - dq * u[2]) + dq * v[2];
    work.face = chosen_id;
    work.room = room;
    world.heading_pending = 1;
    true
}

/// `0x367CE0(out_axis)` (`eax` = header): attach a header to the nearest face of its room.
///
/// * The room has no faces: `state = 3`, nothing else.
/// * Else the face minimising the summed squared distance to both vertices wins when that sum is at most the squared edge
///   length plus 0.05 (the first such face on ties). It becomes the header's face, the pending flag is set, `0x3675E0`
///   projects the header and yields the normal, `q.y += -(n . q_before)`, `attached = 1`, and the normal is the result.
/// * No face qualifies: the first neighbour room (as in [`collide_faces`]) that contains the point becomes the room;
///   otherwise the direction is bounced back inside the room box (no tolerance, zero box for unknown rooms).
///
/// Returns the normal when a face was attached.
pub fn attach_nearest_face(world: &mut FaceWorld, work: &mut FaceWork) -> Option<Vec3> {
    let faces: Vec<u32> = match world.room_faces(work.room) {
        Some(list) if !list.is_empty() => list.to_vec(),
        _ => {
            work.state = 3;
            return None;
        }
    };
    let p = work.p;
    let mut best_distance = f32::MAX;
    let mut best: Option<u32> = None;
    for &id in &faces {
        let f = world.face(id);
        let dz0 = f.z0 - p[2];
        let dz1 = f.z1 - p[2];
        let dx1 = f.x1 - p[0];
        let dx0 = f.x0 - p[0];
        let distance = (((dx1 * dx1) + (dz0 * dz0)) + (dz1 * dz1)) + (dx0 * dx0);
        if best_distance > distance {
            let ex = f.x1 - f.x0;
            let ez = f.z1 - f.z0;
            let limit = (ex * ex + ez * ez) + EDGE_SLACK;
            if limit >= distance {
                best_distance = distance;
                best = Some(id);
            }
        }
    }
    if let Some(id) = best {
        let saved = work.q;
        work.face = id;
        world.heading_pending = 1;
        let face = world.face(id);
        let mut flag = world.heading_pending;
        let normal = clamp_to_face_segment(work, &face, &mut flag, true).expect("normal requested");
        world.heading_pending = flag;
        let dot = (normal[2] * saved[2] + normal[1] * saved[1]) + normal[0] * saved[0];
        work.q[1] = (0.0 - dot) + work.q[1];
        work.attached = 1;
        return Some(normal);
    }
    if let Some(neighbor) = neighbor_containing(world, work.room, p) {
        work.room = neighbor;
        return None;
    }
    let (min, max) = world.room_box(work.room);
    if min[0] > p[0] {
        work.q[0] = work.q[0].abs();
    }
    if p[0] > max[0] {
        work.q[0] = 0.0 - work.q[0].abs();
    }
    if min[2] > p[2] {
        work.q[2] = work.q[2].abs();
    }
    if p[2] > max[2] {
        work.q[2] = 0.0 - work.q[2].abs();
    }
    None
}

/// `0x367820(previous)` (`esi` = the blob itself, not the work header; stack argument = the previous position):
/// find the face of the blob's room that the segment from `previous` to the blob position crosses, and reflect onto it.
///
/// A face qualifies when the blob's y is inside its y range, the two end points lie strictly on opposite sides of the
/// face plane (using the unnormalised normal `(e2*e1, ., -e0*e1)`) and the crossing point projects between the edge
/// vertices. The first qualifying face wins: it becomes the blob's face, the work position is projected onto the face
/// plane, the work direction loses its normal component (`q += -(q . n) n`, with y taken from the old face frame
/// as `-(q_old . n) + k n.y`), `attached = 1`, the old normal becomes the steering axis, the heading words are written
/// and the pending flag ends cleared. Without a qualifying face nothing changes.
pub fn find_hit_and_reflect(world: &mut FaceWorld, blob: &mut Blob, previous: Vec3) {
    let Some(list) = world.room_faces(blob.room) else {
        return;
    };
    let list = list.to_vec();
    let p = blob.position;
    for &id in &list {
        let f = world.face(id);
        if !(f.y_high >= p[1]) || !(p[1] >= f.y_low) {
            continue;
        }
        let (e0, e2, e1) = (f.x1 - f.x0, f.z1 - f.z0, f.y_high - f.y_low);
        let c0 = e0 * 0.0 - e2 * 0.0;
        let a = e2 * e1;
        let b = 0.0 - e0 * e1;
        let d = f.z0 * b + f.x0 * a;
        let side_current = (p[2] * b + p[0] * a) - d;
        let side_previous = (previous[2] * b + previous[0] * a) - d;
        let product = side_previous * side_current;
        if !(0.0 > product) {
            continue;
        }
        let abs_previous = side_previous.abs();
        let abs_current = side_current.abs();
        let sum = abs_previous + abs_current;
        let t = if sum == 0.0 {
            f32::from_bits(0x3F00_0000)
        } else {
            abs_current / sum
        };
        let keep = 1.0 - t;
        let hit_x = previous[0] * t + keep * p[0];
        let hit_z = previous[2] * t + keep * p[2];
        let projection = hit_z * e2 + hit_x * e0;
        let start = f.z0 * e2 + f.x0 * e0;
        if !(projection >= start) {
            continue;
        }
        let end = f.z1 * e2 + f.x1 * e0;
        if !(end >= projection) {
            continue;
        }
        reflect_onto_face(world, blob, id, [a, c0, b]);
        return;
    }
}

fn reflect_onto_face(world: &mut FaceWorld, blob: &mut Blob, id: u32, unnormalised: Vec3) {
    let f = world.face(id);
    let (e0, e1, e2) = (f.x1 - f.x0, f.y_high - f.y_low, f.z1 - f.z0);
    let mut old_normal = unnormalised;
    rescale_vector(&mut old_normal, [0.0, e1, 0.0], [e0, 0.0, e2]);
    let saved_direction = blob.work_direction;
    blob.face = f;
    world.heading_pending = 1;
    let mut normal = [e2 * e1, e0 * 0.0 - e2 * 0.0, 0.0 - e0 * e1];
    let offset = [
        blob.work_position[0] + (0.0 - f.x0),
        blob.work_position[1] + (0.0 - f.y_low),
        (0.0 - f.z0) + blob.work_position[2],
    ];
    rescale_vector(&mut normal, [0.0, e1, 0.0], [e0, 0.0, e2]);
    blob.heading[0] = 0.0;
    blob.heading[1] = f64::from(normal[0]).atan2(f64::from(normal[2])) as f32;
    blob.heading[2] = 0.0;
    world.heading_pending = 0;
    let distance = (offset[2] * normal[2] + offset[1] * normal[1]) + offset[0] * normal[0];
    let k1 = 0.0 - distance;
    blob.work_position[0] = k1 * normal[0] + blob.work_position[0];
    blob.work_position[1] = k1 * normal[1] + blob.work_position[1];
    blob.work_position[2] = k1 * normal[2] + blob.work_position[2];
    let along = (blob.work_direction[2] * normal[2] + normal[0] * blob.work_direction[0])
        + blob.work_direction[1] * normal[1];
    let k2 = 0.0 - along;
    blob.work_direction[0] = k2 * normal[0] + blob.work_direction[0];
    blob.work_direction[2] = k2 * normal[2] + blob.work_direction[2];
    let old_along = (saved_direction[2] * old_normal[2] + saved_direction[1] * old_normal[1])
        + saved_direction[0] * old_normal[0];
    blob.attached = 1;
    blob.work_direction[1] = ((0.0 - old_along) + k2 * normal[1]) + blob.work_direction[1];
    blob.axis = old_normal;
}

/// [`BlastGeometry`] over a [`FaceWorld`]: the four room callees of `0x368A10` and the piece attach of `0x369940`.
pub struct RoomGeometry<'a> {
    pub world: &'a mut FaceWorld,
}

impl RoomGeometry<'_> {
    fn refresh(&self, blob: &mut Blob) {
        blob.face = self.world.face_copy(blob.face.id);
    }
}

impl BlastGeometry for RoomGeometry<'_> {
    /// `0x3675E0` with `edi` = the blob axis: the normal becomes the new axis.
    fn slide_on_face(&mut self, blob: &mut Blob) {
        let mut work = FaceWork::from_blob(blob);
        let face = self.world.face_copy(work.face);
        let mut flag = self.world.heading_pending;
        let normal = clamp_to_face_segment(&mut work, &face, &mut flag, true);
        self.world.heading_pending = flag;
        work.store_blob(blob);
        if let Some(normal) = normal {
            blob.axis = normal;
        }
    }

    fn collide(&mut self, blob: &mut Blob) {
        let mut work = FaceWork::from_blob(blob);
        collide_faces(self.world, &mut work);
        work.store_blob(blob);
        blob.face = self.world.face_copy(work.face);
    }

    fn find_hit_and_reflect(&mut self, blob: &mut Blob, previous: Vec3) {
        find_hit_and_reflect(self.world, blob, previous);
        self.refresh(blob);
    }

    /// `0x367CE0` with the blob axis as the output normal.
    fn attach_to_nearest_face(&mut self, blob: &mut Blob) {
        let mut work = FaceWork::from_blob(blob);
        let normal = attach_nearest_face(self.world, &mut work);
        work.store_blob(blob);
        blob.face = self.world.face_copy(work.face);
        if let Some(normal) = normal {
            blob.axis = normal;
        }
    }

    /// The inlined `0x3675E0` of `0x369940`: no output normal.
    fn slide_piece(&mut self, piece: &mut BlastPiece) {
        let mut work = FaceWork::from_piece(piece);
        let face = self.world.face_copy(work.face);
        let mut flag = self.world.heading_pending;
        clamp_to_face_segment_inline(&mut work, &face, &mut flag);
        self.world.heading_pending = flag;
        work.store_piece(piece);
    }

    fn collide_piece(&mut self, piece: &mut BlastPiece) {
        let mut work = FaceWork::from_piece(piece);
        collide_faces(self.world, &mut work);
        work.store_piece(piece);
        piece.face = self.world.face_copy(work.face);
    }

    /// `0x367CE0` with a null output pointer.
    fn attach_piece(&mut self, piece: &mut BlastPiece) {
        let mut work = FaceWork::from_piece(piece);
        attach_nearest_face(self.world, &mut work);
        work.store_piece(piece);
        piece.face = self.world.face_copy(work.face);
    }
}
