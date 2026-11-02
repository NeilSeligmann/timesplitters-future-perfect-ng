//! T1846 batch 8: the cell geometry under the `0x88950` sweep driver (`SweepGeometry` of `group_members.rs`).
//!
//! Original addresses (names INFERRED, behaviour MEASURED bit-exactly against the original under the Unicorn oracle,
//! goldens `sweep_cells_*` / `sweep_mesh_*` from `tools/t1846_gold/sweep_cells.py` and `sweep_mesh.py`). All
//! functions are plain SSE f32 code in listing order (no fused multiply-add); the vertex tests of `0x47D50` use
//! 32 bit wrapping integer arithmetic on 16 bit grid coordinates.
//!
//! | address | role |
//! |---|---|
//! | `0x88430(from*, vec*, cell, previous, flag, out_from*, out_vec*)` | [`cell_step`] / [`advance_88430`]: leave a cell through the nearest portal polygon |
//! | `0xA3500(cell, from*, vec*, flag, &point, &aux, &int, mask)` | [`cell_test_a3500`]: nearest solid wall hit of the objects registered for a cell |
//! | `0xA3140(obj, flag, &point, &aux, &int, filter)` (+ `edi = from*`, `ebx = vec*`) | [`wall_scan_a3140`]: nearest wall of one object |
//! | `0x477B0(origin*, dir*, v0*, v1*, v2*, &point, &normal)` | [`ray_triangle`]: ray (half line) against a triangle |
//! | `0xA7A00(cell, from*, vec*, flag, &point, &aux, mask, extra1, extra2)` | [`cell_test_a7a00`]: the other per-cell test (`use_a7a00`), callees behind [`Callees7a00`] |
//! | `0xA2CD0(object, from*, vec*, dir*, end*, flag, current, &pt, &aux, &limit, extra1, extra2)` | [`dynamic_scan_a2cd0`]: dynamic object test |
//! | `0x128C60` / `0x49C60` / `0x128E90` | [`buffer_scan_128c60`] / [`box_ray_49c60`] / [`compact_128e90`]: box record scan and the hit buffer |
//! | `0x91390`, `0x913B0`, `0xD6A70` | [`ImpactGlobals`] snapshots |
//! | `0x919D0(class, from*, vec*, flag, &point, &aux, extra)` | [`mesh_ray_919d0`]: ray against an object mesh (tree walk `0x913F0` `walk_913f0`, entry test `0x47D50` [`ray_entry_47d50`], `0x47BD0` [`ray_box_47bd0`], `0x47B30` [`ray_init_47b30`], `0x91960` [`to_local_91960`]) |
//!
//! `0xD9760` (shape scan of kind 8 objects) is ported in `shape_scan.rs` (T1855) and runs behind
//! [`ScanCallees::shape_scan`] unless [`MeshWorld::shape`] overrides it. Objects of kind 8 are filtered out by the mask
//! `-3` of the level 4 prelude, but the `0x33FB50` ground sweep (mask 0) reaches it.
//! `0xA14B0` (object world matrix) is ported in `world_queries.rs` and enters through `world_matrix` closures.
//!
//! Plugging into the driver: build a [`CellGraph`] and a [`SolidWorld`] from the guest memory and run
//! `sweep_88950(&mut SweepCells { graph, solids, callees_a7a00: Some(&mut scanner) }, &args, &mut io)`, where
//! `scanner` is a [`CellScanner`] (`ScanCallees` = [`MeshWorld`]); without `callees_a7a00` only the
//! `0xA3500` variant (`use_a7a00 == false`) returns hits.
//!
//! Globals (guest memory in the original, typed structures here):
//! * `[0x708788]` cell table, entries of `0xB4` bytes, dword `+4` = pointer to a portal list `[count, id0, id1, ...]`.
//! * `[0x70878C]` portal pointer table indexed by id, `[0x7B9754]` highest valid portal id.
//! * `[0x7B975C]` highest cell index with a slot object, `[0x7B9784]` slot object pointer per cell.
//! * `[0x7B54E0 + 4*cell]` dynamic object count, `[0x7B5F00 + 4*cell]` pointer to the array of dynamic objects.
//! * Constants: `0x475D5C` = `f32::MAX`, `0x475C78` = 1.0, `0x4765F4` = 1e-7, `0x475CAC` = 0.0,
//!   `0x47D390`/`0x47D394` = +-0.57.

// `!(a > b)` keeps the unordered (NaN) behaviour of the original `comiss` + `ja`/`jbe`; the ported callees mirror the
// original argument lists.
#![allow(clippy::neg_cmp_op_on_partial_ord, clippy::too_many_arguments)]

use crate::story_events::group_members::{
    AdvanceOut, AdvanceProbe, CellHit, CellProbe, FLOAT_MAX, MIN_LENGTH_SQUARED, SweepGeometry,
};
use crate::story_events::shape_scan::{RealShapeParts, ShapeScanCall, shape_scan_d9760};
use crate::story_events::sweep_records::{GuestMemory, transform_point};
use crate::story_events::world_queries::{Mat4, transform_direction, transpose};

/// `0x47D390`: the portal normal must exceed this (or be below its negative) to pick the dominant axis.
pub const AXIS_LIMIT: f32 = f32::from_bits(0x3F11_EB85);
/// Highest valid cell number of `0x88430` (`cmp edi, 0xC8`).
pub const MAX_CELL: u32 = 0xC8;
/// Object flag bits tested by `0xA3500`.
pub const OBJECT_SOLID: u32 = 0x20;
pub const OBJECT_DISABLED: u32 = 0x100;
/// `0xA3500` ORs this into the query mask before it scans the dynamic objects of the cell.
pub const DYNAMIC_MASK: u32 = 0x0200_0070;
/// Wall flags tested by `0xA3140` when its `filter` argument is non-zero (never from `0xA3500`).
pub const WALL_FILTER_BITS: u32 = 0x3000;

// ---------------------------------------------------------------------------------------------------------------
// 0x88430 cell step
// ---------------------------------------------------------------------------------------------------------------

/// A portal polygon (entry of the table `[0x70878C]`). INFERRED name, layout MEASURED:
/// `+0` first cell, `+4` second cell, `+8` plane normal (3 floats), `+0x14` signed word vertex count,
/// `+0x18` vertices of 12 bytes (the plane point is the first vertex, read even when the count is not positive).
#[derive(Clone, Debug, PartialEq)]
pub struct Portal {
    pub cell_a: u32,
    pub cell_b: u32,
    pub normal: [f32; 3],
    pub plane_point: [f32; 3],
    pub vertices: Vec<[f32; 3]>,
}

impl Portal {
    /// Portal whose plane point is its first vertex (zeros without vertices).
    pub fn new(cell_a: u32, cell_b: u32, normal: [f32; 3], vertices: Vec<[f32; 3]>) -> Self {
        Self {
            cell_a,
            cell_b,
            normal,
            plane_point: vertices.first().copied().unwrap_or([0.0; 3]),
            vertices,
        }
    }
}

/// The cell graph read by `0x88430`: portal lists per cell and the portal table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellGraph {
    /// `[0x7B9754]`: portal ids above it are ignored.
    pub max_portal_id: i32,
    /// Index = cell number (`0..=0xC8`); `None` = null list pointer; the list holds portal ids.
    pub cells: Vec<Option<Vec<i32>>>,
    /// Index = portal id; `None` = null pointer (skipped).
    pub portals: Vec<Option<Portal>>,
}

/// Result of [`cell_step`]: the cell reached and, when it differs from the input, the point where the sweep left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellStep {
    pub next: u32,
    pub entry: Option<[f32; 3]>,
}

fn dot_zyx(a: [f32; 3], b: [f32; 3]) -> f32 {
    // Image order: z product, plus y product, plus x product.
    a[2] * b[2] + a[1] * b[1] + a[0] * b[0]
}

/// Dominant axes `(S, D)` of a portal normal (`esi`, `edx` of the original): the vertex edge test projects on them.
fn portal_axes(normal: [f32; 3]) -> (usize, usize) {
    if -AXIS_LIMIT > normal[0] {
        (2, 1)
    } else if normal[0] > AXIS_LIMIT {
        (1, 2)
    } else if -AXIS_LIMIT > normal[2] {
        (1, 0)
    } else if normal[2] > AXIS_LIMIT {
        (0, 1)
    } else if -AXIS_LIMIT > normal[1] {
        (0, 2)
    } else {
        (2, 0)
    }
}

/// Edge test of the hit point against the portal polygon. `true` = the point is outside (the portal is rejected).
fn outside_polygon(portal: &Portal, hit: [f32; 3], s: usize, d: usize) -> bool {
    let count = portal.vertices.len();
    let mut previous = count.wrapping_sub(1);
    for current in 0..count {
        let (p, c) = (portal.vertices[previous], portal.vertices[current]);
        let a = (p[s] - c[s]) * (hit[d] - p[d]);
        let b = (c[d] - p[d]) * (hit[s] - p[s]);
        if a + b > 0.0 {
            return true;
        }
        previous = current;
    }
    false
}

/// `0x88430` core: the neighbour cell through the portal of `cell` that the segment `from -> from + vec` crosses
/// nearest to `from` (squared distance of the crossing, first portal wins ties), skipping the portal leading back to
/// `previous`. Returns `cell` itself (no crossing point) when `cell` is 0 or above `0xC8`, has no portal list, `vec`
/// is the zero vector, or no portal qualifies. With `flag != 0` the segment is a ray (no end test). MEASURED.
/// Negative cells read outside the tables in the original and are not modelled (treated as unchanged).
pub fn cell_step(
    graph: &CellGraph,
    from: [f32; 3],
    vec: [f32; 3],
    cell: u32,
    previous: u32,
    flag: u32,
) -> CellStep {
    let unchanged = CellStep {
        next: cell,
        entry: None,
    };
    if cell == 0 || cell > MAX_CELL {
        return unchanged;
    }
    let Some(Some(list)) = graph.cells.get(cell as usize) else {
        return unchanged;
    };
    if vec[0] == 0.0 && vec[1] == 0.0 && vec[2] == 0.0 {
        return unchanged;
    }
    let mut best_len = FLOAT_MAX;
    let mut best_next = cell;
    let mut best_hit = [0.0f32; 3];
    for &id in list {
        if id < 0 || id > graph.max_portal_id {
            continue;
        }
        let Some(Some(portal)) = graph.portals.get(id as usize) else {
            continue;
        };
        let other = if portal.cell_a == cell {
            portal.cell_b
        } else {
            portal.cell_a
        };
        if other == previous {
            continue;
        }
        let n = portal.normal;
        let q = portal.plane_point;
        let dy = from[1] - q[1];
        let dz = from[2] - q[2];
        let dx = from[0] - q[0];
        let side = n[1] * dy + n[2] * dz + dx * n[0];
        let approach = dot_zyx(vec, n);
        if side > 0.0 && approach > 0.0 {
            continue;
        }
        if 0.0 > side && 0.0 > approach {
            continue;
        }
        if portal.cell_a == cell && 0.0 >= approach {
            continue;
        }
        if portal.cell_b == cell && approach >= 0.0 {
            continue;
        }
        let numerator = (q[1] - from[1]) * n[1] + (q[2] - from[2]) * n[2] + (q[0] - from[0]) * n[0];
        if 0.0 > numerator && approach > 0.0 {
            continue;
        }
        if numerator > 0.0 && 0.0 > approach {
            continue;
        }
        if flag == 0 && numerator.abs() > approach.abs() {
            continue;
        }
        let t = numerator / approach;
        let (hx, hy, hz) = (vec[0] * t, vec[1] * t, vec[2] * t);
        let length = hz * hz + hy * hy + hx * hx;
        if length > best_len {
            continue;
        }
        let hit = [from[0] + hx, hy + from[1], from[2] + hz];
        let (s, d) = portal_axes(n);
        if outside_polygon(portal, hit, s, d) {
            continue;
        }
        best_len = length;
        best_next = other;
        best_hit = hit;
    }
    if best_next == cell {
        return unchanged;
    }
    CellStep {
        next: best_next,
        entry: Some(best_hit),
    }
}

/// `0x88430` as `0x88950` calls it: both output pointers alias the input buffers. After a step the start becomes the
/// crossing point and (only with `flag == 0`) the vector the remainder `from + vec - hit`. MEASURED.
pub fn advance_88430(graph: &CellGraph, probe: &AdvanceProbe) -> AdvanceOut {
    let step = cell_step(
        graph,
        probe.from_copy,
        probe.vec_copy,
        probe.cell,
        probe.previous,
        probe.flag,
    );
    let mut out = AdvanceOut {
        next: step.next,
        from_copy: probe.from_copy,
        vec_copy: probe.vec_copy,
    };
    if let Some(hit) = step.entry {
        if probe.flag == 0 {
            out.vec_copy = [0, 1, 2].map(|k| probe.from_copy[k] + probe.vec_copy[k] - hit[k]);
        }
        out.from_copy = hit;
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// 0x477B0 ray against triangle
// ---------------------------------------------------------------------------------------------------------------

/// Hit of [`ray_triangle`]: the point and the (unnormalised) triangle normal `(v2-v0) x (v1-v0)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriangleHit {
    pub point: [f32; 3],
    pub normal: [f32; 3],
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `0x477B0`: intersects the half line `origin + t * dir` (`t >= 0`, no upper bound) with the plane of the triangle and
/// accepts the point when two of the three edge tests pass (the sign of `c . ((vK - p) x edge)` terms is not
/// negative for edges `v1 -> v2` and `v2 -> v0`). `None` for a parallel ray (`denominator == 0`), `t < 0` or a
/// failed edge test. MEASURED.
pub fn ray_triangle(
    origin: [f32; 3],
    dir: [f32; 3],
    v0: [f32; 3],
    v1: [f32; 3],
    v2: [f32; 3],
) -> Option<TriangleHit> {
    let e1 = sub(v1, v0);
    let e2 = sub(v2, v0);
    let normal = [
        e2[2] * e1[1] - e2[1] * e1[2],
        e2[0] * e1[2] - e2[2] * e1[0],
        e2[1] * e1[0] - e2[0] * e1[1],
    ];
    let denominator = dir[0] * normal[0] + dir[1] * normal[1] + dir[2] * normal[2];
    if denominator == 0.0 {
        return None;
    }
    let plane = v0[0] * normal[0] + v0[1] * normal[1] + v0[2] * normal[2];
    let start = origin[0] * normal[0] + origin[1] * normal[1] + origin[2] * normal[2];
    let t = (plane - start) / denominator;
    if 0.0 > t {
        return None;
    }
    let p = [
        dir[0] * t + origin[0],
        dir[1] * t + origin[1],
        dir[2] * t + origin[2],
    ];
    // c = e1 x (v1 - p) in the image's component order.
    let (w0, w1, w2) = (v1[1] - p[1], v1[0] - p[0], v1[2] - p[2]);
    let c = [
        w2 * e1[1] - w0 * e1[2],
        w1 * e1[2] - w2 * e1[0],
        w0 * e1[0] - w1 * e1[1],
    ];
    let f = sub(v2, v1);
    let (u0, u1, u2) = (v2[2] - p[2], v2[1] - p[1], v2[0] - p[0]);
    let term1 = (u0 * f[1] - u1 * f[2]) * c[0];
    let term2 = (u1 * f[0] - u2 * f[1]) * c[2];
    let term3 = (u2 * f[2] - u0 * f[0]) * c[1];
    if 0.0 > term2 + term3 + term1 {
        return None;
    }
    let h = sub(v0, v2);
    let (r0, r1, r2) = (v0[2] - p[2], v0[1] - p[1], v0[0] - p[0]);
    let term1 = (r0 * h[1] - r1 * h[2]) * c[0];
    let term2 = (r1 * h[0] - r2 * h[1]) * c[2];
    let term3 = (r2 * h[2] - r0 * h[0]) * c[1];
    if 0.0 > term2 + term3 + term1 {
        return None;
    }
    Some(TriangleHit { point: p, normal })
}

// ---------------------------------------------------------------------------------------------------------------
// 0xA3140 wall scan, 0xA3500 cell test
// ---------------------------------------------------------------------------------------------------------------

/// Wall primitive of an object (element of the array at `obj+0x1F4`). Layout MEASURED: `+0` collision mask,
/// `+4` flags, `+8` `x0`, `+0xC` `y0`, `+0x10` `z0`, `+0x14` `x1`, `+0x18` `y1`, `+0x1C` `z1` (a vertical quad with
/// corners `(x0,y0,z0) (x0,y1,z0) (x1,y1,z1) (x1,y0,z1)`), `+0x20` min x, `+0x24` min z, `+0x28` max x,
/// `+0x2C` max z of the box used for rejection (y bounds are `y0`, `y1`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wall {
    /// Guest address of the wall (reported as the hit integer, so the model carries it).
    pub address: u32,
    pub mask: u32,
    pub flags: u32,
    pub corner0: [f32; 3],
    pub corner1: [f32; 3],
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
}

/// Solid object as read by `0xA3500`/`0xA3140`: `+4` class pointer whose dword `+0x30` is `class_mask`, `+0x20` mask,
/// `+0x24` owner pointer whose dword `+0x28` is `owner_flags`, `+0x28` flags, `+0x1E4` wall count, `+0x1F4` wall
/// array pointer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidObject {
    pub address: u32,
    /// Guest value of the class pointer `+4` (passed to `0x919D0`).
    pub class_address: u32,
    pub class_mask: u32,
    pub mask: u32,
    pub owner_flags: u32,
    pub flags: u32,
    /// `false` = null wall array pointer (the dynamic scan skips such an object).
    pub has_walls: bool,
    pub walls: Vec<Wall>,
    /// `+0x34` cell the object lives in (read by `0x128C60`).
    pub cell: u32,
    /// `+0x78` link pointer (compared with the class pointers of other objects by `0xA2CD0`).
    pub link: u32,
    /// `[[+0x7C] + 0x24]` flags of the linked record (bit 8 skips the shape scan for `extra1 & 4`).
    pub link_flags: u32,
    /// `+0x1F8` array (count `+0x1C`) of box records scanned by `0x128C60`; empty when the pointer is null.
    pub boxes: Vec<BoxRecord>,
}

/// Objects registered per cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SolidWorld {
    /// `[0x7B975C]`.
    pub max_slot: i32,
    /// `[0x7B9784][cell]` (`None` = null, the original then reads low memory: not modelled, treated as skipped).
    pub slots: Vec<Option<SolidObject>>,
    /// `[0x7B5F00 + 4*cell]` arrays; the count is the length.
    pub dynamic: Vec<Vec<SolidObject>>,
    /// `[0x7B6254]` array (count `[0x7B5834]`) of objects scanned by `0xA7A00` for every cell, also invalid ones.
    pub global_objects: Vec<SolidObject>,
}

impl SolidWorld {
    /// The object with this guest address among the slot, dynamic and global objects.
    pub fn find(&self, address: u32) -> Option<&SolidObject> {
        let slots = self.slots.iter().flatten();
        let dynamic = self.dynamic.iter().flatten();
        slots
            .chain(dynamic)
            .chain(&self.global_objects)
            .find(|object| object.address == address)
    }
}

/// Hit of [`wall_scan_a3140`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallHit {
    pub point: [f32; 3],
    /// Normal of the hit triangle (unnormalised); `(0, 1, 0)` is the untouched default and never returned.
    pub normal: [f32; 3],
    pub wall: u32,
}

fn reject_by_box(wall: &Wall, from: [f32; 3], vec: [f32; 3]) -> bool {
    let (y_min, y_max) = (wall.corner0[1], wall.corner1[1]);
    (wall.min_x > from[0] && wall.min_x > from[0] + vec[0])
        || (from[0] > wall.max_x && from[0] + vec[0] > wall.max_x)
        || (y_min > from[1] && y_min > vec[1] + from[1])
        || (from[1] > y_max && vec[1] + from[1] > y_max)
        || (wall.min_z > from[2] && wall.min_z > vec[2] + from[2])
        || (from[2] > wall.max_z && from[2] + vec[2] > wall.max_z)
}

/// `0xA3140(obj, flag, ..)` with `edi = from`, `ebx = vec`: nearest hit among the walls of `object`. A wall is skipped
/// when `filter` and its flags have none of [`WALL_FILTER_BITS`], when its mask is non-zero and shares no bit with the
/// class mask, or (with `flag == 0`) when its box lies entirely beyond the segment. The quad is tested as the
/// triangles `(A, B, C)` then `(C, D, A)`; a hit counts only when `vec . normal <= 0` (front face). The nearest
/// squared distance from `from` wins (strict), `None` when nothing was hit. MEASURED.
pub fn wall_scan_a3140(
    object: &SolidObject,
    flag: u32,
    filter: bool,
    from: [f32; 3],
    vec: [f32; 3],
) -> Option<WallHit> {
    let mut best = FLOAT_MAX;
    let mut found: Option<WallHit> = None;
    for wall in &object.walls {
        if filter && wall.flags & WALL_FILTER_BITS == 0 {
            continue;
        }
        if wall.mask != 0 && object.class_mask & wall.mask == 0 {
            continue;
        }
        if flag == 0 && reject_by_box(wall, from, vec) {
            continue;
        }
        let [x0, y0, z0] = wall.corner0;
        let [x1, y1, z1] = wall.corner1;
        let (a, b, c, d) = ([x0, y0, z0], [x0, y1, z0], [x1, y1, z1], [x1, y0, z1]);
        let first =
            ray_triangle(from, vec, a, b, c).filter(|hit| !(dot_zyx(vec, hit.normal) > 0.0));
        let hit = match first {
            Some(hit) => hit,
            None => match ray_triangle(from, vec, c, d, a) {
                Some(hit) if !(dot_zyx(vec, hit.normal) > 0.0) => hit,
                _ => continue,
            },
        };
        let p = hit.point;
        let distance = (p[2] - from[2]) * (p[2] - from[2])
            + (p[1] - from[1]) * (p[1] - from[1])
            + (p[0] - from[0]) * (p[0] - from[0]);
        if best > distance {
            best = distance;
            found = Some(WallHit {
                point: p,
                normal: hit.normal,
                wall: wall.address,
            });
        }
    }
    found
}

/// `0xA3500(cell, from, vec, flag, &point, &aux, &int, mask)`: nearest wall hit over the slot object of `cell` and
/// the dynamic objects of `cell`. Returns `ret` = address of the object that was hit (0 = none), `point`, `aux`
/// (wall normal, unnormalised) and `int_out` (wall address); the outputs are only written by the original on a hit,
/// the zero [`CellHit`] otherwise. The distance limit starts at `|vec|^2` (`flag == 0`) or `f32::MAX`. MEASURED.
///
/// * Slot object (only when `cell <= max_slot`): needs [`OBJECT_SOLID`] in its flags and no bit of `mask` in its mask.
/// * Dynamic objects (always scanned, `mask |= DYNAMIC_MASK`): the owner flags must have bit 1 or 2, the object flags
///   [`OBJECT_SOLID`] but not [`OBJECT_DISABLED`], no bit of the extended mask in the object mask, and a wall array.
pub fn cell_test_a3500(world: &SolidWorld, probe: &CellProbe) -> CellHit {
    let (from, vec) = (probe.from, probe.vec);
    let mut limit = if probe.flag == 0 {
        vec[2] * vec[2] + vec[1] * vec[1] + vec[0] * vec[0]
    } else {
        FLOAT_MAX
    };
    let cell = probe.cell as i32;
    let mut best: Option<(u32, WallHit)> = None;
    let mut consider = |object: &SolidObject, limit: &mut f32| {
        if let Some(hit) = wall_scan_a3140(object, probe.flag, false, from, vec) {
            let p = hit.point;
            let distance = (p[2] - from[2]) * (p[2] - from[2])
                + (p[1] - from[1]) * (p[1] - from[1])
                + (p[0] - from[0]) * (p[0] - from[0]);
            if *limit > distance {
                *limit = distance;
                best = Some((object.address, hit));
            }
        }
    };
    if cell <= world.max_slot {
        if let Some(Some(object)) = world.slots.get(cell as usize) {
            if object.flags & OBJECT_SOLID != 0 && object.mask & probe.extra[0] == 0 {
                consider(object, &mut limit);
            }
        }
    }
    let mask = probe.extra[0] | DYNAMIC_MASK;
    if let Some(objects) = world.dynamic.get(cell as usize) {
        for object in objects {
            if object.owner_flags & 6 == 0
                || object.flags & OBJECT_SOLID == 0
                || object.flags & OBJECT_DISABLED != 0
                || object.mask & mask != 0
                || !object.has_walls
            {
                continue;
            }
            consider(object, &mut limit);
        }
    }
    match best {
        Some((address, hit)) => CellHit {
            ret: address,
            point: hit.point,
            aux: hit.normal,
            int_out: hit.wall,
        },
        None => CellHit::default(),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0xA7A00 cell test
// ---------------------------------------------------------------------------------------------------------------

/// Running best hit of `0xA7A00` (the locals `E-0x48..` point, `E-0x3C..` aux, `E-0x50` squared limit). The unported
/// callees update it in place through pointers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BestHit {
    pub point: [f32; 3],
    pub aux: [f32; 3],
    pub limit: f32,
}

/// Arguments of the dynamic object scan `0xA2CD0` (12 cdecl arguments): the object, the segment (`from`, `vec`), the
/// unit direction and end point `from + vec`, the flag, the best object so far, the best buffers, `extra1`, `extra2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicScan {
    pub object: u32,
    pub from: [f32; 3],
    pub vec: [f32; 3],
    pub direction: [f32; 3],
    pub end: [f32; 3],
    pub flag: u32,
    pub current: u32,
    pub extra1: u32,
    pub extra2: u32,
}

/// The callees of `0xA7A00` that are not ported (see `docs/t-story-event-sweep-cells-t1846.md` for sizes).
pub trait Callees7a00 {
    /// `0x919D0(class, from, vec, flag, &point, &aux, 0)`: segment against the static mesh of a slot object.
    fn mesh_ray(
        &mut self,
        class: u32,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
    ) -> Option<([f32; 3], [f32; 3])>;
    /// `0x91390()`: snapshot of the 32 byte block `[0x70AA58]` into `[0x70AAAC]` (called when a mesh hit is accepted).
    fn note_mesh_hit(&mut self);
    /// `0x128C60(object, from, vec, flag, extra2, &best.point, &best.aux, &best.limit)`: non-zero = improved `best`.
    fn buffer_scan(
        &mut self,
        object: &SolidObject,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
        extra2: u32,
        best: &mut BestHit,
    ) -> bool;
    /// `0xA2CD0(..)`: returns the new best object (the input one when nothing improved), updating `best`.
    fn dynamic_scan(&mut self, object: &SolidObject, call: &DynamicScan, best: &mut BestHit)
    -> u32;
}

fn distance_squared(point: [f32; 3], from: [f32; 3]) -> f32 {
    let dy = point[1] - from[1];
    let dx = point[0] - from[0];
    let dz = point[2] - from[2];
    dz * dz + dy * dy + dx * dx
}

/// `0xA7A00(cell, from, vec, flag, &point, &aux, extra0, extra1, extra2)`: the sweep cell test of
/// `use_a7a00` (called by `0x88C60`, e.g. with `extra = (-3, 0, 0)` from the level 4 prelude). Nearest hit among
/// (1) the slot object of the cell (mesh ray `0x919D0` if its mask misses `extra0`; the wall scan `0xA3140` with the
/// filter when `extra1 & 0x40`; the buffer scan `0x128C60` when `extra2 != 0`), (2) the dynamic objects of the cell
/// and (3) the global objects (`0xA2CD0` for each, plus `0xA3140` when `extra1 & 0x40` for (2) only). Cells outside
/// `1..=max_slot` skip (1) and (2). The limit starts at `|vec|^2` (`flag == 0`) or `f32::MAX`; every accepted hit
/// needs `limit > distance^2` (strict). `ret` is the address of the object that owns the best hit. The unit
/// direction handed to `0xA2CD0` is `vec * (1 / sqrt(len2))` when `len2 > 1e-7` (QEMU `rsqrtss`, xemu level), else
/// `vec`. Slot/dynamic object filters: slot needs [`OBJECT_SOLID`] and `mask` miss (`extra0`); dynamic and global
/// objects need non-negative flags, owner flag bit 0, [`OBJECT_SOLID`] and `mask` miss. MEASURED against the original
/// with the four callees scripted.
pub fn cell_test_a7a00(
    world: &SolidWorld,
    callees: &mut dyn Callees7a00,
    probe: &CellProbe,
) -> CellHit {
    let (from, vec, flag) = (probe.from, probe.vec, probe.flag);
    let [mask, extra1, extra2] = probe.extra;
    let end = [vec[0] + from[0], vec[1] + from[1], vec[2] + from[2]];
    let length_squared = vec[2] * vec[2] + vec[1] * vec[1] + vec[0] * vec[0];
    let mut direction = vec;
    if length_squared > MIN_LENGTH_SQUARED {
        let inverse = 1.0 / length_squared.sqrt() * 1.0;
        direction = [vec[0] * inverse, vec[1] * inverse, vec[2] * inverse];
    }
    let mut best = BestHit {
        point: [0.0; 3],
        aux: [0.0; 3],
        limit: if flag == 0 { length_squared } else { FLOAT_MAX },
    };
    let mut owner = 0u32;
    let cell = probe.cell as i32;
    let take_wall_hit = |object: &SolidObject, best: &mut BestHit| -> bool {
        match wall_scan_a3140(object, flag, true, from, vec) {
            Some(hit) => {
                let distance = distance_squared(hit.point, from);
                if best.limit > distance {
                    best.point = hit.point;
                    best.aux = hit.normal;
                    best.limit = distance;
                    return true;
                }
                false
            }
            None => false,
        }
    };
    let scan_dynamic = |object: &SolidObject,
                        owner: &mut u32,
                        best: &mut BestHit,
                        callees: &mut dyn Callees7a00| {
        *owner = callees.dynamic_scan(
            object,
            &DynamicScan {
                object: object.address,
                from,
                vec,
                direction,
                end,
                flag,
                current: *owner,
                extra1,
                extra2,
            },
            best,
        );
    };
    let eligible = |object: &SolidObject| -> bool {
        (object.flags as i32) >= 0
            && object.owner_flags & 1 != 0
            && object.flags & OBJECT_SOLID != 0
            && object.mask & mask == 0
    };
    if cell > 0 && cell <= world.max_slot {
        if let Some(Some(slot)) = world.slots.get(cell as usize) {
            if slot.flags & OBJECT_SOLID != 0 {
                if slot.mask & mask == 0 {
                    if let Some((point, aux)) =
                        callees.mesh_ray(slot.class_address, from, vec, flag)
                    {
                        let distance = distance_squared(point, from);
                        if best.limit > distance {
                            callees.note_mesh_hit();
                            best.point = point;
                            best.aux = aux;
                            best.limit = distance;
                            owner = slot.address;
                        }
                    }
                    if extra1 & 0x40 != 0 && take_wall_hit(slot, &mut best) {
                        owner = slot.address;
                    }
                }
                if extra2 != 0 && callees.buffer_scan(slot, from, vec, flag, extra2, &mut best) {
                    owner = slot.address;
                }
            }
        }
        if let Some(objects) = world.dynamic.get(cell as usize) {
            for object in objects {
                if !eligible(object) {
                    continue;
                }
                scan_dynamic(object, &mut owner, &mut best, callees);
                if extra1 & 0x40 != 0 && take_wall_hit(object, &mut best) {
                    owner = object.address;
                }
            }
        }
    }
    for object in &world.global_objects {
        if !eligible(object) {
            continue;
        }
        scan_dynamic(object, &mut owner, &mut best, callees);
    }
    if owner == 0 {
        return CellHit::default();
    }
    CellHit {
        ret: owner,
        point: best.point,
        aux: best.aux,
        int_out: 0,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x49C60 / 0x128C60 box record scan, small helpers (0x91390, 0x913B0, 0xD6A70, 0x128E90)
// ---------------------------------------------------------------------------------------------------------------

/// Box record (pointed to by the array at `object+0x1F8`) as read by `0x128C60` / `0x49C60`: `+0` float (negative =
/// disabled), `+0x0C` signed word portal index, `+0x28` word flags, `+0x2A` word vertex count, `+0x30` plane normal,
/// `+0x3C`/`+0x3D`/`+0x3E` signed byte axes, `+0x40` plane point = vertex 0, vertices of 0x1C bytes from `+0x40`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoxRecord {
    pub address: u32,
    pub enabled: f32,
    pub portal: i16,
    pub flags: u16,
    pub normal: [f32; 3],
    pub axes: [i8; 3],
    pub plane_point: [f32; 3],
    pub vertices: Vec<[f32; 3]>,
}

/// Result of [`box_ray_49c60`]: the squared distance (stored through the original's `out_d2` once the plane hit is
/// accepted, even when the limit test then fails) and the hit point (success only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxRayHit {
    pub d2: Option<f32>,
    pub point: Option<[f32; 3]>,
}

/// `0x49C60(from, vec, limit, record, &point, &d2)`: the half line `from + t * vec` against the plane of a box record,
/// `t = (plane . n - from . n) / (vec . n)`; rejected for `vec . n == 0`, `t < 0`, `|vec|^2 t^2 > limit`, or when
/// the projected point (axes `axes[0]`, `axes[1]`) lies outside the vertex polygon (edge test as in
/// [`cell_step`]). The third coordinate comes from `axes[2]`. Axes are assumed to be 0..2. MEASURED.
pub fn box_ray_49c60(from: [f32; 3], vec: [f32; 3], limit: f32, record: &BoxRecord) -> BoxRayHit {
    let miss = BoxRayHit {
        d2: None,
        point: None,
    };
    let n = record.normal;
    let denominator = vec[2] * n[2] + vec[1] * n[1] + vec[0] * n[0];
    if denominator == 0.0 {
        return miss;
    }
    let q = record.plane_point;
    let plane = q[2] * n[2] + q[1] * n[1] + q[0] * n[0];
    let start = from[2] * n[2] + from[1] * n[1] + from[0] * n[0];
    let t = (plane - start) / denominator;
    if 0.0 > t {
        return miss;
    }
    let d2 = (vec[0] * vec[0] + vec[1] * vec[1] + vec[2] * vec[2]) * t * t;
    if d2 > limit {
        return BoxRayHit {
            d2: Some(d2),
            point: None,
        };
    }
    let [ax, ay, az] = record.axes.map(|axis| axis as usize);
    let at = |axis: usize| vec[axis] * t + from[axis];
    let (px, py) = (at(ax), at(ay));
    let count = record.vertices.len();
    let mut previous = count.wrapping_sub(1);
    for current in 0..count {
        let (p, c) = (record.vertices[previous], record.vertices[current]);
        let a = (p[ax] - c[ax]) * (py - p[ay]);
        let b = (c[ay] - p[ay]) * (px - p[ax]);
        if a + b > 0.0 {
            return BoxRayHit {
                d2: Some(d2),
                point: None,
            };
        }
        previous = current;
    }
    let mut point = [0.0f32; 3];
    point[ax] = px;
    point[ay] = py;
    point[az] = at(az);
    BoxRayHit {
        d2: Some(d2),
        point: Some(point),
    }
}

/// One of the (up to 5) hit records of a [`HitBuffer`]: `+0` object, `+4` point, `+0x10` vec, `+0x1C` normal,
/// `+0x28` squared distance, `+0x2C` cell of the object, `+0x30` box record address.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HitRecord {
    pub object: u32,
    pub point: [f32; 3],
    pub vec: [f32; 3],
    pub normal: [f32; 3],
    pub d2: f32,
    pub cell: u32,
    pub record: u32,
}

/// The hit buffer `extra2` of `0xA7A00` points at: `+0` count, `+4` flag (set when a record with flag bit 8 hit),
/// records of 0x34 bytes from `+8` (at most 5 are stored, further hits only set the flag and the outputs).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HitBuffer {
    pub count: u32,
    pub flag: u32,
    pub records: Vec<HitRecord>,
}

/// Capacity of a [`HitBuffer`].
pub const HIT_CAPACITY: u32 = 5;

/// Globals touched by the hit bookkeeping: `[0x70AA58..]` current block, `[0x70AAAC..]` saved block (8 dwords each),
/// `[0x7AC5A0..]` scratch block, `[0x7356E0]` -> `[0x7356DC]` shape snapshot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImpactGlobals {
    pub current: [u32; 8],
    pub saved: [u32; 8],
    pub scratch: [u32; 8],
    pub shape_source: u32,
    pub shape_saved: u32,
}

impl ImpactGlobals {
    /// `0x91390()`: `saved = current` (32 byte copy).
    pub fn snapshot_91390(&mut self) {
        self.saved = self.current;
    }
    /// `0x913B0(block*)`: `saved = *block`.
    pub fn snapshot_913b0(&mut self, block: [u32; 8]) {
        self.saved = block;
    }
    /// `0xD6A70()`: `[0x7356DC] = [0x7356E0]`.
    pub fn snapshot_d6a70(&mut self) {
        self.shape_saved = self.shape_source;
    }
}

/// `0x128E90(buffer, limit)`: removes (from the last record to the first) every record whose squared distance is
/// greater than `limit`, compacting the rest. MEASURED.
pub fn compact_128e90(buffer: &mut HitBuffer, limit: f32) {
    let mut index = buffer.records.len();
    while index > 0 {
        index -= 1;
        if buffer.records[index].d2 > limit {
            buffer.records.remove(index);
            buffer.count -= 1;
        }
    }
}

/// Output targets of [`buffer_scan_128c60`]: optional point and aux (the box normal bits) and the limit.
pub struct ScanTargets<'a> {
    pub point: Option<&'a mut [f32; 3]>,
    pub aux: Option<&'a mut [f32; 3]>,
    pub limit: &'a mut f32,
}

/// `0x128C60(object, from, vec, flag, buffer, &point, &aux, &limit)`: walks the box records of the object. For every
/// enabled record (`enabled >= 0`) that [`box_ray_49c60`] hits within `*limit`: appends a [`HitRecord`] (while the
/// buffer holds fewer than 5), sets `buffer.flag` when flag bit 8 is set, and then accepts the record when its flags
/// have one of `0x40C` and not `0x4000`, and, if flag bit 1 is set, the portal polygon
/// (`graph.portals[record.portal]`) agrees with the side the ray comes from (`vec . normal >= 0` needs
/// `object.cell == cell_a`, `< 0` needs `!=`). An accepted record writes point/aux/limit and refreshes the globals
/// (`scratch = [-1; 8]` with `scratch[0] = object.class_address`, then `saved = scratch`). Returns whether anything was
/// accepted. The `flag` argument is unused by the original. MEASURED.
pub fn buffer_scan_128c60(
    object: &SolidObject,
    graph: &CellGraph,
    from: [f32; 3],
    vec: [f32; 3],
    buffer: &mut HitBuffer,
    globals: &mut ImpactGlobals,
    targets: &mut ScanTargets,
) -> bool {
    let mut accepted = false;
    for record in &object.boxes {
        if record.enabled < 0.0 || record.enabled.is_nan() {
            continue;
        }
        let hit = box_ray_49c60(from, vec, *targets.limit, record);
        let (Some(point), Some(d2)) = (hit.point, hit.d2) else {
            continue;
        };
        if buffer.count < HIT_CAPACITY {
            buffer.records.push(HitRecord {
                object: object.address,
                point,
                vec,
                normal: record.normal,
                d2,
                cell: object.cell,
                record: record.address,
            });
            buffer.count += 1;
        }
        if record.flags & 0x100 != 0 {
            buffer.flag = 1;
        }
        if record.flags & 0x40C == 0 || record.flags & 0x4000 != 0 {
            continue;
        }
        if record.flags & 2 != 0 {
            let Some(Some(portal)) = graph.portals.get(record.portal.max(0) as usize) else {
                continue;
            };
            let approach =
                portal.normal[2] * vec[2] + portal.normal[1] * vec[1] + portal.normal[0] * vec[0];
            let same_cell = object.cell == portal.cell_a;
            let pass = if approach < 0.0 || approach.is_nan() {
                // `jb` taken (also for NaN): the second compare `0 <= approach` then only passes for approach < 0.
                approach < 0.0 && !same_cell
            } else {
                same_cell
            };
            if !pass {
                continue;
            }
        }
        if let Some(out) = targets.point.as_deref_mut() {
            *out = point;
        }
        if let Some(out) = targets.aux.as_deref_mut() {
            *out = record.normal;
        }
        *targets.limit = d2;
        globals.scratch = [u32::MAX; 8];
        globals.scratch[0] = object.class_address;
        globals.snapshot_913b0(globals.scratch);
        accepted = true;
    }
    accepted
}

// ---------------------------------------------------------------------------------------------------------------
// 0xA2CD0 dynamic object scan
// ---------------------------------------------------------------------------------------------------------------

/// `[object + 0x20] & ` this mask skips the box record scan of `0xA2CD0` (bits `0x3841C38`).
pub const BOX_SCAN_SKIP_MASK: u32 = 0x0384_1C38;
/// Shape code of `[object + 0x20]` that selects the shape scan `0xD9760` instead of the mesh ray `0x919D0`.
pub const SHAPE_KIND: u32 = 8;

/// The two geometry callees of `0xA2CD0` that are not ported (`docs/t-story-event-sweep-cells-t1846.md`).
pub trait ScanCallees {
    /// `0x919D0(class, from, vec, flag, &point, &aux, extra1)`: ray against the mesh of an object class. It rewrites
    /// the impact block `globals.current` when a triangle is accepted ([`mesh_ray_919d0`]).
    fn mesh_ray(
        &mut self,
        globals: &mut ImpactGlobals,
        class: u32,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
        extra1: u32,
    ) -> Option<([f32; 3], [f32; 3])>;
    /// `0xD9760(object, from, direction, end, flag, &point, &aux, extra1)`: ray against a shape object.
    fn shape_scan(
        &mut self,
        globals: &mut ImpactGlobals,
        object: u32,
        from: [f32; 3],
        direction: [f32; 3],
        end: [f32; 3],
        flag: u32,
        extra1: u32,
    ) -> Option<([f32; 3], [f32; 3])>;
}

/// [`ScanCallees`] with the ported mesh ray ([`mesh_ray_919d0`]) over guest memory. `world_matrix` is
/// `0xA14B0(out, object, 1, 0)` (see `world_queries::build_world_matrix`); the shape scan `0xD9760` is the ported
/// `shape_scan::shape_scan_d9760` over `memory` unless `shape` overrides it. A mesh ray that returns 1 without
/// outputs (`extra & 0x80`) is reported as no hit: the original leaves its stack buffers untouched and the callers then read stale values.
pub struct MeshWorld<'a> {
    pub memory: &'a dyn GuestMemory,
    pub world_matrix: &'a mut dyn FnMut(u32) -> Mat4,
    #[allow(clippy::type_complexity)]
    pub shape: Option<
        &'a mut dyn FnMut(
            u32,
            [f32; 3],
            [f32; 3],
            [f32; 3],
            u32,
            u32,
        ) -> Option<([f32; 3], [f32; 3])>,
    >,
}

impl ScanCallees for MeshWorld<'_> {
    fn mesh_ray(
        &mut self,
        globals: &mut ImpactGlobals,
        class: u32,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
        extra1: u32,
    ) -> Option<([f32; 3], [f32; 3])> {
        let result = mesh_ray_919d0(
            self.memory,
            self.world_matrix,
            globals,
            class,
            from,
            vec,
            flag,
            extra1,
        );
        match (result.hit, result.point, result.aux) {
            (true, Some(point), Some(aux)) => Some((point, aux)),
            _ => None,
        }
    }

    fn shape_scan(
        &mut self,
        globals: &mut ImpactGlobals,
        object: u32,
        from: [f32; 3],
        direction: [f32; 3],
        end: [f32; 3],
        flag: u32,
        extra1: u32,
    ) -> Option<([f32; 3], [f32; 3])> {
        if let Some(shape) = self.shape.as_mut() {
            return shape(object, from, direction, end, flag, extra1);
        }
        // No override: the ported root `0xD9760` over the guest memory (T1855).
        let call = ShapeScanCall {
            object,
            from,
            direction,
            end,
            flag,
            extra1,
        };
        let mut parts = RealShapeParts {
            memory: self.memory,
            world_matrix: &mut *self.world_matrix,
        };
        let outcome = shape_scan_d9760(self.memory, &mut parts, &call);
        if let Some(element) = outcome.best_element {
            globals.shape_source = element;
        }
        match (outcome.ret != 0, outcome.point, outcome.aux) {
            (true, Some(point), Some(aux)) => Some((point, aux)),
            _ => None,
        }
    }
}

/// `(+4 class pointer, +0x78 link)` of the object that was the best one so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ObjectLinks {
    pub class_address: u32,
    pub link: u32,
}

/// `0xA2CD0(object, from, vec, direction, end, flag, current, &best.point, &best.aux, &best.limit, extra1, extra2)`:
/// tests one dynamic object and returns the new best object (`call.current` when nothing better was found).
///
/// 1. `current != 0`: when `current.link == object.class` the object belongs to the best one and the call returns
///    `current` at once; when `object.link == current.class` the limit is raised to `f32::MAX` for this call and put
///    back afterwards.
/// 2. Shape kind 8 uses `shape_scan` (skipped when `extra1 & 4` and `link_flags & 8`), every other kind
///    `mesh_ray`. A hit nearer than `best.limit` becomes the best (point, aux, limit; `0xD6A70` for kind 8 else
///    `0x91390`; with a hit buffer holding records, `0x128E90` drops the farther ones).
/// 3. Unless `object.mask & BOX_SCAN_SKIP_MASK`, with a hit buffer (`extra2`), the box records are scanned
///    ([`buffer_scan_128c60`]); an accepted record makes the object the best.
///
/// `buffer` is `Some` exactly when `extra2 != 0`. MEASURED against the original with both callees scripted.
pub fn dynamic_scan_a2cd0(
    graph: &CellGraph,
    globals: &mut ImpactGlobals,
    callees: &mut dyn ScanCallees,
    object: &SolidObject,
    current_links: Option<ObjectLinks>,
    call: &DynamicScan,
    best: &mut BestHit,
    mut buffer: Option<&mut HitBuffer>,
) -> u32 {
    let mut saved_limit = None;
    if call.current != 0 {
        let links = current_links.unwrap_or_default();
        if links.link == object.class_address {
            return call.current;
        }
        if object.link == links.class_address {
            saved_limit = Some(best.limit);
            best.limit = FLOAT_MAX;
        }
    }
    let mut result = call.current;
    let skip_scan = object.mask == SHAPE_KIND && call.extra1 & 4 != 0 && object.link_flags & 8 != 0;
    if !skip_scan {
        let hit = if object.mask == SHAPE_KIND {
            callees.shape_scan(
                globals,
                object.address,
                call.from,
                call.direction,
                call.end,
                call.flag,
                call.extra1,
            )
        } else {
            callees.mesh_ray(
                globals,
                object.class_address,
                call.from,
                call.vec,
                call.flag,
                call.extra1,
            )
        };
        if let Some((point, aux)) = hit {
            let distance = distance_squared(point, call.from);
            if best.limit > distance {
                if object.mask == SHAPE_KIND {
                    globals.snapshot_d6a70();
                } else {
                    globals.snapshot_91390();
                }
                best.point = point;
                best.aux = aux;
                best.limit = distance;
                result = object.address;
                if let Some(buffer) = buffer.as_deref_mut() {
                    if buffer.count != 0 {
                        compact_128e90(buffer, distance);
                    }
                }
            }
        }
    }
    if let Some(limit) = saved_limit {
        best.limit = limit;
    }
    if object.mask & BOX_SCAN_SKIP_MASK == 0 {
        if let Some(buffer) = buffer {
            let mut targets = ScanTargets {
                point: Some(&mut best.point),
                aux: Some(&mut best.aux),
                limit: &mut best.limit,
            };
            if buffer_scan_128c60(
                object,
                graph,
                call.from,
                call.vec,
                buffer,
                globals,
                &mut targets,
            ) {
                result = object.address;
            }
        }
    }
    result
}

/// [`Callees7a00`] on top of the ported callees: `0xA2CD0` ([`dynamic_scan_a2cd0`]), `0x128C60`
/// ([`buffer_scan_128c60`]) and `0x91390`, with the two unported geometry tests behind [`ScanCallees`] (`0x919D0`
/// serves both the slot object ray and `0xA2CD0`). `buffers` holds the hit buffers by guest address (`extra2`); an
/// unknown non-zero `extra2` is treated as having no buffer.
pub struct CellScanner<'a> {
    pub graph: &'a CellGraph,
    pub world: &'a SolidWorld,
    pub globals: &'a mut ImpactGlobals,
    pub geometry: &'a mut dyn ScanCallees,
    pub buffers: &'a mut std::collections::BTreeMap<u32, HitBuffer>,
}

impl Callees7a00 for CellScanner<'_> {
    fn mesh_ray(
        &mut self,
        class: u32,
        from: [f32; 3],
        vec: [f32; 3],
        flag: u32,
    ) -> Option<([f32; 3], [f32; 3])> {
        self.geometry
            .mesh_ray(self.globals, class, from, vec, flag, 0)
    }

    fn note_mesh_hit(&mut self) {
        self.globals.snapshot_91390();
    }

    fn buffer_scan(
        &mut self,
        object: &SolidObject,
        from: [f32; 3],
        vec: [f32; 3],
        _flag: u32,
        extra2: u32,
        best: &mut BestHit,
    ) -> bool {
        let Some(buffer) = self.buffers.get_mut(&extra2) else {
            return false;
        };
        let mut targets = ScanTargets {
            point: Some(&mut best.point),
            aux: Some(&mut best.aux),
            limit: &mut best.limit,
        };
        buffer_scan_128c60(
            object,
            self.graph,
            from,
            vec,
            buffer,
            self.globals,
            &mut targets,
        )
    }

    fn dynamic_scan(
        &mut self,
        object: &SolidObject,
        call: &DynamicScan,
        best: &mut BestHit,
    ) -> u32 {
        let links = self.world.find(call.current).map(|current| ObjectLinks {
            class_address: current.class_address,
            link: current.link,
        });
        dynamic_scan_a2cd0(
            self.graph,
            self.globals,
            self.geometry,
            object,
            links,
            call,
            best,
            self.buffers.get_mut(&call.extra2),
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------
// 0x919D0 mesh ray (and 0x47B30, 0x47BD0, 0x91960, 0x913F0, 0x47D50)
// ---------------------------------------------------------------------------------------------------------------

/// `[0x476038]`: scale of the signed bytes of a triangle table entry normal (`1/127`).
pub const NORMAL_BYTE_SCALE: f32 = f32::from_bits(0x3C01_0204);
/// `[0x475C9C]` / `[0x475CA0]`: scale of a quantised vertex coordinate and the inverse grid scale.
pub const VERTEX_SCALE: f32 = f32::from_bits(0x3A80_0000);
pub const GRID_SCALE: f32 = f32::from_bits(0x4480_0000);
/// `[0x475DCC]`: scale of the quantised child boxes (`1/255`).
pub const BOX_SCALE: f32 = f32::from_bits(0x3B80_8081);

/// The 0x28 byte ray record built by `0x47B30`: origin, direction, reciprocal direction (0 for a zero component)
/// and the current length (shrunk by every accepted hit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub inverse: [f32; 3],
    pub length: f32,
}

/// `0x47B30(ray, origin, direction, length)`. MEASURED.
pub fn ray_init_47b30(origin: [f32; 3], direction: [f32; 3], length: f32) -> Ray {
    let inverse = direction.map(|d| if d == 0.0 { 0.0 } else { 1.0 / d });
    Ray {
        origin,
        direction,
        inverse,
        length,
    }
}

/// One slab pair of [`ray_box_47bd0`] after the first axis: merges `(ta, tb)` into `(near, far)`; `None` = miss.
fn merge_slab(near: f32, far: f32, ta: f32, tb: f32, length: f32) -> Option<(f32, f32)> {
    let (slab_near, slab_far) = if ta > tb { (tb, ta) } else { (ta, tb) };
    let (mut near, mut far) = (near, far);
    if far > slab_far {
        if 0.0 > slab_far {
            return None;
        }
        far = slab_far;
    }
    if slab_near > near {
        if slab_near > length {
            return None;
        }
        near = slab_near;
    }
    if near > far {
        return None;
    }
    Some((near, far))
}

/// `0x47BD0(ray, bounds)`: slab test of the ray against the box `[min xyz, max xyz]` within `ray.length`. Following
/// the original the final `near > far` check is made by the last non-parallel axis pair among z and y only (a ray
/// parallel to y returns true without it). MEASURED.
// portreg: va=0x00047bd0 tier=1
pub fn ray_box_47bd0(ray: &Ray, bounds: &[f32; 6]) -> bool {
    let (o, inv, length) = (ray.origin, ray.inverse, ray.length);
    let (near, far);
    if !(ray.direction[0] == 0.0) {
        let ta = (bounds[0] - o[0]) * inv[0];
        let tb = (bounds[3] - o[0]) * inv[0];
        let (n, f) = if ta > tb { (tb, ta) } else { (ta, tb) };
        if 0.0 > f || n > length {
            return false;
        }
        (near, far) = (n, f);
    } else {
        if bounds[0] > o[0] || o[0] > bounds[3] {
            return false;
        }
        (near, far) = (0.0, length);
    }
    let (mut near, mut far) = (near, far);
    if !(ray.direction[2] == 0.0) {
        let ta = (bounds[2] - o[2]) * inv[2];
        let tb = (bounds[5] - o[2]) * inv[2];
        match merge_slab(near, far, ta, tb, length) {
            Some(merged) => (near, far) = merged,
            None => return false,
        }
    } else if bounds[2] > o[2] || o[2] > bounds[5] {
        return false;
    }
    if !(ray.direction[1] == 0.0) {
        let ta = (bounds[1] - o[1]) * inv[1];
        let tb = (bounds[4] - o[1]) * inv[1];
        return merge_slab(near, far, ta, tb, length).is_some();
    }
    !(bounds[1] > o[1] || o[1] > bounds[4])
}

/// `0x91960`: the ray start and direction in the local frame of a matrix: `R^T (pos - translation)` and `R^T vec`
/// (`R^T` through `0x1530E0`, `0x152090`). MEASURED.
pub fn to_local_91960(
    matrix: &Mat4,
    position: [f32; 3],
    direction: [f32; 3],
) -> ([f32; 3], [f32; 3]) {
    let mut local = [
        position[0] - matrix[12],
        position[1] - matrix[13],
        position[2] - matrix[14],
    ];
    let mut vector = direction;
    let transposed = transpose(matrix);
    transform_direction(&transposed, &mut local);
    transform_direction(&transposed, &mut vector);
    (local, vector)
}

/// `cvttss2si` followed by `movsx r32, r16`: the low 16 bits of the truncated value (indefinite `0x80000000` for a
/// NaN or out of range input).
fn truncate_to_i16(value: f32) -> i32 {
    let truncated = if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    };
    i32::from(truncated as i16)
}

/// Hit of [`ray_entry_47d50`]: position and plane normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntryHit {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// `0x47D50(ray, vertices, bounds, base, quantised, entry, cull, &position, &normal)`: the ray against one 12 byte
/// triangle table entry (layout as in `sweep_triangles.rs`: `i8[0]` vertex count, `i8[1..4]` normal times `1/127`,
/// `u8[4]` bits 0..1 / 2..3 / 4..5 the three axes, `u16[8]` offset of the vertex index bytes inside
/// `[bounds + 0x2C]`). `cull` rejects faces whose direction dot normal is not negative, otherwise only a parallel
/// ray is rejected. The hit plane distance must lie in `0..=ray.length`; the polygon test projects the hit on the
/// first two axes (`i16` grid for `quantised`, vertices `i16 x 3` at stride 8 dequantised with `base` and
/// [`VERTEX_SCALE`], else `f32 x 3` at stride 16) and requires one orientation of all edges (`cull`: not
/// positive). On success `ray.length = t` and the position (axes 1..3 written in order) and normal are returned.
/// Axes are assumed to be a permutation of 0..2. MEASURED.
pub fn ray_entry_47d50(
    memory: &dyn GuestMemory,
    ray: &mut Ray,
    vertices: u32,
    bounds: u32,
    base: u32,
    quantised: bool,
    entry: u32,
    cull: bool,
) -> Option<EntryHit> {
    let normal_byte = |k: u32| f32::from(memory.read_u8(entry + k) as i8) * NORMAL_BYTE_SCALE;
    let normal = [normal_byte(1), normal_byte(2), normal_byte(3)];
    let denominator = dot_zyx(ray.direction, normal);
    if cull {
        if denominator >= 0.0 {
            return None;
        }
    } else if denominator == 0.0 {
        return None;
    }
    let count = i32::from(memory.read_u8(entry) as i8);
    let index_base = memory
        .read_u32(bounds + 0x2C)
        .wrapping_add(u32::from(memory.read_u16(entry + 8)));
    let index = |k: i32| -> u32 {
        let byte = memory.read_u8(index_base.wrapping_add(k as u32)) as i8;
        i32::from(byte) as u32
    };
    let stride: u32 = if quantised { 8 } else { 16 };
    let vertex_address = |k: i32| vertices.wrapping_add(index(k).wrapping_mul(stride));
    let last = vertex_address(count - 1);
    let base_value = |axis: usize| memory.read_f32(base + 4 * axis as u32);
    let plane_distance = if quantised {
        let coordinate = |axis: u32| {
            f32::from(memory.read_u16(last + 2 * axis) as i16) * VERTEX_SCALE
                + base_value(axis as usize)
        };
        (coordinate(2) * normal[2] + coordinate(1) * normal[1]) + coordinate(0) * normal[0]
    } else {
        let coordinate = |axis: u32| memory.read_f32(last + 4 * axis);
        (coordinate(2) * normal[2] + coordinate(1) * normal[1]) + coordinate(0) * normal[0]
    };
    let origin_distance =
        (ray.origin[2] * normal[2] + ray.origin[1] * normal[1]) + ray.origin[0] * normal[0];
    let t = (plane_distance - origin_distance) / denominator;
    if 0.0 > t || t > ray.length {
        return None;
    }
    let axes_byte = memory.read_u8(entry + 4);
    let axis1 = usize::from(axes_byte & 3);
    let axis2 = usize::from((axes_byte >> 2) & 3);
    let axis3 = usize::from((axes_byte >> 4) & 3);
    let along = |axis: usize| ray.direction[axis] * t + ray.origin[axis];
    let (p1, p2) = (along(axis1), along(axis2));
    let mut positive_ok = true;
    let mut negative_ok = true;
    let mut check = |positive: bool| -> bool {
        if cull {
            return !positive;
        }
        if positive {
            negative_ok = false;
            positive_ok
        } else {
            positive_ok = false;
            negative_ok
        }
    };
    if quantised {
        let i1 = truncate_to_i16((p1 - base_value(axis1)) * GRID_SCALE);
        let i2 = truncate_to_i16((p2 - base_value(axis2)) * GRID_SCALE);
        let coordinate = |address: u32, axis: usize| {
            i32::from(memory.read_u16(address + 2 * axis as u32) as i16)
        };
        let mut previous = last;
        for k in 0..count.max(0) {
            let current = vertex_address(k);
            let (prev1, prev2) = (coordinate(previous, axis1), coordinate(previous, axis2));
            let (cur1, cur2) = (coordinate(current, axis1), coordinate(current, axis2));
            let edge = (prev1 - cur1)
                .wrapping_mul(i2 - prev2)
                .wrapping_add((cur2 - prev2).wrapping_mul(i1 - prev1));
            if !check(edge > 0) {
                return None;
            }
            previous = current;
        }
    } else {
        let coordinate = |address: u32, axis: usize| memory.read_f32(address + 4 * axis as u32);
        let mut previous = last;
        for k in 0..count.max(0) {
            let current = vertex_address(k);
            let a = (coordinate(previous, axis1) - coordinate(current, axis1))
                * (p2 - coordinate(previous, axis2));
            let b = (coordinate(current, axis2) - coordinate(previous, axis2))
                * (p1 - coordinate(previous, axis1));
            if !check(a + b > 0.0) {
                return None;
            }
            previous = current;
        }
    }
    ray.length = t;
    let mut position = [0.0f32; 3];
    position[axis1] = p1;
    position[axis2] = p2;
    position[axis3] = along(axis3);
    Some(EntryHit { position, normal })
}

/// Output of the triangle table walk `0x913F0`: the last accepted hit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct WalkOut {
    position: [f32; 3],
    normal: [f32; 3],
}

/// Inputs of `0x913F0` that stay the same through the recursion.
struct Walk<'a> {
    memory: &'a dyn GuestMemory,
    class: u32,
    record_index: u32,
    /// The bounds record `T` (`+0x1C` triangle table, `+0x24` node array, `+0x2C` vertex index bytes).
    bounds: u32,
    flags: u32,
}

fn read_box(memory: &dyn GuestMemory, address: u32) -> [f32; 6] {
    std::array::from_fn(|k| memory.read_f32(address + 4 * k as u32))
}

/// `0x913F0(class, record, bounds, node, parentBox, ray, &position, &normal, flags)`: walks the node tree of one
/// record (12 byte nodes at `[bounds + 0x24]`): a node with a non-zero `u16[+6]` is a leaf holding `u16[+0xA]`
/// triangle table entries from `u16[+8]`, any other node has two children at `u16[+8]` and `u16[+0xA]` (tested with
/// [`ray_box_47bd0`] against boxes dequantised from the parent box with [`BOX_SCALE`] when `byte[M + 0xC]` has bit 7,
/// else walked directly). Leaf entries are filtered by `flags` bit 3 / bit 4 (selector 1 / 2 are skipped) and the
/// plane record flags, then tested with [`ray_entry_47d50`]; an accepted entry rewrites the impact block
/// `[0x70AA58..]` (class, record, selector, vertex index, entry index, plane data) and the outputs. Returns whether
/// any entry was accepted. MEASURED.
// portreg: va=0x000913f0 tier=1
fn walk_913f0(
    walk: &Walk,
    globals: &mut ImpactGlobals,
    node: u32,
    parent: [f32; 6],
    ray: &mut Ray,
    out: &mut WalkOut,
) -> bool {
    let memory = walk.memory;
    let mesh = memory.read_u32(walk.class.wrapping_add(4));
    let object = memory.read_u32(walk.class);
    let count = memory.read_u32(mesh);
    let record = mesh.wrapping_add(walk.record_index.wrapping_sub(count).wrapping_mul(0xC8));
    if memory.read_u16(node + 6) != 0 {
        return walk_leaf(walk, globals, node, ray, out, mesh, object, record);
    }
    let nodes = memory.read_u32(walk.bounds + 0x24);
    let first = nodes.wrapping_add(u32::from(memory.read_u16(node + 8)) * 12);
    let second = nodes.wrapping_add(u32::from(memory.read_u16(node + 0xA)) * 12);
    if memory.read_u8(mesh + 0xC) & 0x80 == 0 {
        let a = walk_913f0(walk, globals, first, parent, ray, out);
        let b = walk_913f0(walk, globals, second, parent, ray, out);
        return a | b;
    }
    let extent = [
        (parent[3] - parent[0]) * BOX_SCALE,
        (parent[4] - parent[1]) * BOX_SCALE,
        (parent[5] - parent[2]) * BOX_SCALE,
    ];
    let child_box = |child: u32| -> [f32; 6] {
        let byte = |k: u32| f32::from(memory.read_u8(child + k));
        [
            byte(0) * extent[0] + parent[0],
            byte(1) * extent[1] + parent[1],
            byte(2) * extent[2] + parent[2],
            byte(3) * extent[0] + parent[0],
            byte(4) * extent[1] + parent[1],
            byte(5) * extent[2] + parent[2],
        ]
    };
    let mut result = false;
    for child in [first, second] {
        let bounds = child_box(child);
        if !ray_box_47bd0(ray, &bounds) {
            if child == first {
                continue;
            }
            return result;
        }
        result |= walk_913f0(walk, globals, child, bounds, ray, out);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn walk_leaf(
    walk: &Walk,
    globals: &mut ImpactGlobals,
    node: u32,
    ray: &mut Ray,
    out: &mut WalkOut,
    mesh: u32,
    object: u32,
    record: u32,
) -> bool {
    let memory = walk.memory;
    let entries = u32::from(memory.read_u16(node + 0xA));
    let first_entry = u32::from(memory.read_u16(node + 8));
    let table = memory.read_u32(walk.bounds + 0x1C);
    let quantised = memory.read_u8(mesh + 0xC) & 0x10 != 0;
    let mut result = false;
    for k in 0..entries {
        let entry = table.wrapping_add((first_entry + k) * 12);
        let byte4 = memory.read_u8(entry + 4);
        let selector = u32::from(byte4 >> 6);
        let reference = u32::from(memory.read_u16(entry + 6));
        let sub_record = record.wrapping_add(selector * 0x18);
        let plane_ref = memory
            .read_u32(record.wrapping_add(0x60 + 4 * selector))
            .wrapping_add(reference * 10);
        if walk.flags & 8 != 0 && byte4 & 0xC0 == 0x40 {
            continue;
        }
        if walk.flags & 0x10 != 0 && byte4 & 0xC0 == 0x80 {
            continue;
        }
        let plane_index = memory.read_u16(plane_ref) as i16;
        let mut plane = 0u32;
        if plane_index >= 0 {
            plane = memory
                .read_u32(mesh + 0x14)
                .wrapping_add((i32::from(plane_index) * 48) as u32);
            let plane_flags = memory.read_u32(plane + 0x1C);
            if plane_flags & 2 != 0 {
                if plane_flags & 1 == 0 {
                    continue;
                }
            } else if plane_flags & 0x80000 != 0 || byte4 & 0xC0 == 0x40 {
                continue;
            }
        }
        let vertices = memory.read_u32(sub_record.wrapping_add(4)).wrapping_add(
            u32::from(memory.read_u16(plane_ref + 6)) * if quantised { 8 } else { 16 },
        );
        let cull = selector == 0 || (selector == 2 && memory.read_u32(object + 0x20) != 0x10000);
        let Some(hit) = ray_entry_47d50(
            memory,
            ray,
            vertices,
            walk.bounds,
            mesh + 0x38,
            quantised,
            entry,
            cull,
        ) else {
            continue;
        };
        let (plane_word, plane_value, plane_byte) = if plane != 0 {
            let first_word = memory.read_u32(plane) as i32;
            let word = if first_word >= 0 {
                memory.read_u32(
                    memory
                        .read_u32(mesh + 0x10)
                        .wrapping_add((first_word as u32) << 4),
                )
            } else {
                u32::MAX
            };
            (
                word,
                memory.read_u32(plane + 0x1C),
                u32::from(memory.read_u8(plane + 0x18)),
            )
        } else {
            (u32::MAX, 0, 0)
        };
        globals.current = [
            walk.class,
            walk.record_index,
            selector,
            reference,
            first_entry + k,
            plane_word,
            plane_byte,
            plane_value,
        ];
        *out = WalkOut {
            position: hit.position,
            normal: hit.normal,
        };
        result = true;
    }
    result
}

/// Result of [`mesh_ray_919d0`]: the return value and the points the original writes (only on a hit it computed).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeshRayResult {
    pub hit: bool,
    pub point: Option<[f32; 3]>,
    pub aux: Option<[f32; 3]>,
}

/// `0x919D0(class, from, vec, flag, &point, &aux, extra)`: ray (`flag == 0`: segment of fraction 1, else half line)
/// against the mesh of a class (`class[0]` object, `class[4]` mesh with `N` records of 0xC8 bytes before it,
/// `class[0x10]` bone matrices, `class[0x30]` mask). Steps (MEASURED):
/// 1. `mesh[4] > 0` and object flag `0x100` -> no hit. The ray must cross the object box (position + offset +- half
///    extent at `+0x1A8`/`+0x1C0`).
/// 2. `extra & 0x80` and `object[0x20] != 2` -> returns 1 without outputs (`point`/`aux` stay `None`, the
///    original leaves its stack buffers untouched).
/// 3. With `object[0x32] != 0` the ray moves to the object frame (`world_matrix` = `0xA14B0(out, object, 1, 0)`,
///    then [`to_local_91960`]); with `mesh[4] > 0`, object flags `8` and `0x200000` ("per record matrices") every
///    record uses its own bone matrix `class[0x10] + (byte record[0x49] << 6)` on top.
/// 4. Records whose mask `record[0x5C]` shares no bit with `class[0x30]` (or is zero), with bounds `record[0xA8]`
///    non-null, a node array `bounds[0x24]` and a ray hit on the bounds are walked (`walk_913f0`).
/// 5. A hit needs `ray.length < tmax`; the point goes back through the bone and object matrices, the normal is
///    rotated the same way and flipped to face against `vec` (`0 < vec . n`, also for NaN).
pub fn mesh_ray_919d0(
    memory: &dyn GuestMemory,
    world_matrix: &mut dyn FnMut(u32) -> Mat4,
    globals: &mut ImpactGlobals,
    class: u32,
    from: [f32; 3],
    vec: [f32; 3],
    flag: u32,
    extra: u32,
) -> MeshRayResult {
    let miss = MeshRayResult::default();
    let object = memory.read_u32(class);
    let mesh = memory.read_u32(class + 4);
    let count = memory.read_u32(mesh);
    let records = mesh.wrapping_sub(count.wrapping_mul(0xC8));
    let mesh_variant = memory.read_u32(mesh + 4) as i32;
    let object_flags = memory.read_u32(object + 0x28);
    let tmax = if flag == 0 { 1.0 } else { FLOAT_MAX };
    if mesh_variant > 0 && object_flags & 0x100 != 0 {
        return miss;
    }
    let f = |offset: u32| memory.read_f32(object + offset);
    let bounds = [
        (f(0x1A8) + f(0x64)) - f(0x1C0),
        (f(0x68) + f(0x1AC)) - f(0x1C4),
        (f(0x6C) + f(0x1B0)) - f(0x1C8),
        (f(0x1A8) + f(0x1C0)) + f(0x64),
        (f(0x1C4) + f(0x68)) + f(0x1AC),
        (f(0x6C) + f(0x1B0)) + f(0x1C8),
    ];
    let mut ray = ray_init_47b30(from, vec, tmax);
    if !ray_box_47bd0(&ray, &bounds) {
        return miss;
    }
    if extra & 0x80 != 0 && memory.read_u32(object + 0x20) != 2 {
        return MeshRayResult {
            hit: true,
            ..Default::default()
        };
    }
    let per_record = mesh_variant > 0 && object_flags & 8 != 0 && object_flags & 0x20_0000 != 0;
    let moved = memory.read_u8(object + 0x32) != 0;
    let (mut local_from, mut local_vec) = (from, vec);
    let mut object_matrix = [0.0f32; 16];
    if moved {
        object_matrix = world_matrix(object);
        (local_from, local_vec) = to_local_91960(&object_matrix, from, vec);
        if !per_record {
            ray = ray_init_47b30(local_from, local_vec, ray.length);
        }
    }
    let bones = memory.read_u32(class + 0x10);
    let class_mask = memory.read_u32(class + 0x30);
    let mut point = [0.0f32; 3];
    let mut normal = [0.0f32, 1.0, 0.0];
    for index in 0..count as i32 {
        let record = records.wrapping_add((index as u32).wrapping_mul(0xC8));
        let record_mask = memory.read_u32(record + 0x5C);
        if record_mask != 0 && class_mask & record_mask == 0 {
            continue;
        }
        let record_bounds = memory.read_u32(record + 0xA8);
        if record_bounds == 0 || memory.read_u32(record_bounds + 0x24) == 0 {
            continue;
        }
        let mut bone = [0.0f32; 16];
        if per_record {
            let slot = i32::from(memory.read_u8(record + 0x49) as i8);
            bone = read_matrix(memory, bones.wrapping_add((slot as u32) << 6));
            let (bone_from, bone_vec) = to_local_91960(&bone, local_from, local_vec);
            ray = ray_init_47b30(bone_from, bone_vec, ray.length);
        }
        if !ray_box_47bd0(&ray, &read_box(memory, record_bounds)) {
            continue;
        }
        let walk = Walk {
            memory,
            class,
            record_index: index as u32,
            bounds: record_bounds,
            flags: extra,
        };
        let mut out = WalkOut::default();
        let root = memory.read_u32(record_bounds + 0x24);
        if !walk_913f0(
            &walk,
            globals,
            root,
            read_box(memory, record_bounds),
            &mut ray,
            &mut out,
        ) {
            continue;
        }
        point = out.position;
        normal = out.normal;
        if per_record {
            point = transform_point(&bone, point);
            transform_direction(&bone, &mut normal);
        }
    }
    if !(tmax > ray.length) {
        return miss;
    }
    if moved {
        point = transform_point(&object_matrix, point);
        transform_direction(&object_matrix, &mut normal);
    }
    let dot = dot_zyx(vec, normal);
    if !(dot <= 0.0) {
        normal = [0.0 - normal[0], 0.0 - normal[1], 0.0 - normal[2]];
    }
    MeshRayResult {
        hit: true,
        point: Some(point),
        aux: Some(normal),
    }
}

fn read_matrix(memory: &dyn GuestMemory, address: u32) -> Mat4 {
    std::array::from_fn(|k| memory.read_f32(address + 4 * k as u32))
}

// ---------------------------------------------------------------------------------------------------------------
// SweepGeometry
// ---------------------------------------------------------------------------------------------------------------

/// [`SweepGeometry`] over ported cell code: [`advance_88430`], [`cell_test_a3500`] and (with `callees`)
/// [`cell_test_a7a00`]. Without `callees` the `use_a7a00` variant yields an empty [`CellHit`] (no hit).
pub struct SweepCells<'a> {
    pub graph: &'a CellGraph,
    pub solids: &'a SolidWorld,
    /// The unported callees of `0xA7A00` for `variant_a7a00` probes.
    pub callees_a7a00: Option<&'a mut dyn Callees7a00>,
}

impl SweepGeometry for SweepCells<'_> {
    fn cell_test(&mut self, probe: &CellProbe) -> CellHit {
        if probe.variant_a7a00 {
            return match self.callees_a7a00.as_mut() {
                Some(callees) => cell_test_a7a00(self.solids, &mut **callees, probe),
                None => CellHit::default(),
            };
        }
        cell_test_a3500(self.solids, probe)
    }

    fn advance(&mut self, probe: &AdvanceProbe) -> AdvanceOut {
        advance_88430(self.graph, probe)
    }
}
