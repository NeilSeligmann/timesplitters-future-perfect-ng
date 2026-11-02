//! T1846 batch 8: world matrix builder `0xA14B0` and its pure matrix helpers, plus the world query functions
//! (`0xC3E60`, `0xC3B10`, sweep `0x92360`). Decoded from `build/default.xbe` with `tools/x87_blocks.py` and checked
//! bit-exactly against the original under Unicorn (`tools/x86_oracle.py`, `tools/t1846_golden.py`, generators in
//! `tools/t1846_gold/world_queries.py`). Golden vectors are MEASURED (emulated original on synthetic inputs); names
//! are INFERRED. Detail: `docs/t-story-event-world-queries-t1846.md`.
//!
//! Matrices are 16 `f32` in row-major order, row vector convention (translation in elements 12..15).

/// 4x4 matrix as the original stores it.
pub type Mat4 = [f32; 16];

/// Identity matrix constant at `0x4E7900` (MEASURED).
pub const IDENTITY: Mat4 = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// Degrees to radians constant at `0x475D0C` (`0x3C8EFA35`, MEASURED).
pub const DEG_TO_RAD: f32 = f32::from_bits(0x3C8E_FA35);

fn sin32(x: f32) -> f32 {
    (x as f64).sin() as f32
}
fn cos32(x: f32) -> f32 {
    (x as f64).cos() as f32
}

/// `0x153240(out)`: writes the identity matrix (MEASURED).
pub fn mat_identity() -> Mat4 {
    IDENTITY
}

/// `0x153600(out, x, y, z)`: identity with diagonal `(x, y, z, 1)` (MEASURED).
pub fn mat_scale(x: f32, y: f32, z: f32) -> Mat4 {
    let mut m = IDENTITY;
    m[0] = x;
    m[5] = y;
    m[10] = z;
    m
}

/// `0x153270(dest, a, b)`: `dest = a * b` with per-lane `((a0*r0 + a1*r1) + a2*r2) + a3*r3` in f32 (MEASURED). The
/// original writes a global scratch matrix at `0x749768` when `dest` aliases an input; that side effect is not
/// modelled (pure value semantics, all inputs are read before any write).
pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0f32; 16];
    for row in 0..4 {
        for col in 0..4 {
            let mut acc = a[row * 4] * b[col];
            acc += a[row * 4 + 1] * b[4 + col];
            acc += a[row * 4 + 2] * b[8 + col];
            acc += a[row * 4 + 3] * b[12 + col];
            out[row * 4 + col] = acc;
        }
    }
    out
}

/// `0x153650(m, n)`: `m = n * m` (pre-multiply), MEASURED.
pub fn mat_premultiply(m: &Mat4, n: &Mat4) -> Mat4 {
    mat_mul(n, m)
}

/// `0x1518F0(m, x, y, z)`: sets the translation row, elements 12..14 (MEASURED).
pub fn mat_set_translation(m: &mut Mat4, x: f32, y: f32, z: f32) {
    m[12] = x;
    m[13] = y;
    m[14] = z;
}

/// `0x153440(out, angle)`: identity with `[0]=cos`, `[2]=-sin`, `[8]=sin`, `[10]=cos` (rotation about Y). The
/// original computes `fsin`/`fcos` in x87 extended precision and rounds once at the store; modelled with f64 (MEASURED
/// against the Unicorn x87 for the golden cases).
pub fn mat_rotation_y(angle: f32) -> Mat4 {
    let mut m = IDENTITY;
    let (s, c) = (sin32(angle), cos32(angle));
    m[10] = c;
    m[8] = s;
    m[2] = -s;
    m[0] = c;
    m
}

/// `0x151650(out, a, b, c)`: full Euler matrix from three angles in radians (`a` = rotation scaled from object `+0xC0`,
/// `b` from `+0x70`, `c` from `+0xC8`). The sin/cos values are rounded to f32 (stored to the stack in the original), the
/// rest is SSE f32 in the original order, `m[1]` and `m[5]` are x87 products rounded once (identical to an f32 multiply
/// because the 48 bit product is exact in extended precision). MEASURED.
pub fn mat_euler(a: f32, b: f32, c: f32) -> Mat4 {
    let (sa, ca) = (sin32(a), cos32(a));
    let (sb, cb) = (sin32(b), cos32(b));
    let (sc, cc) = (sin32(c), cos32(c));
    let zero = 0.0f32;
    let sbsa = sb * sa;
    let cbsa = cb * sa;
    let mut m = [0.0f32; 16];
    m[0] = sbsa * sc + cc * cb;
    m[1] = sc * ca;
    m[2] = (zero - cc * sb) + cbsa * sc;
    m[4] = (zero - sc * cb) + sbsa * cc;
    m[5] = cc * ca;
    m[6] = cbsa * cc + sc * sb;
    m[8] = sb * ca;
    m[9] = zero - sa;
    m[10] = cb * ca;
    m[15] = 1.0;
    m
}

/// The fields of the world object read and written by `0xA14B0` (byte offsets in the original object, names INFERRED).
#[derive(Clone, Debug, PartialEq)]
pub struct WorldMatrixObject {
    /// `+0x32` (signed byte): shape kind. 0 = identity result, 1 = Y rotation, 2 = Euler, 3 = Y rotation or Euler
    /// depending on the linked parent, other values leave the stored matrix as it is.
    pub kind: i8,
    /// `+0x28`: flag word. Bit `0x1000` = matrix dirty (cleared here), bit `0x2000_0000` = mirrored (x flip).
    pub flags: u32,
    /// `+0x64`, `+0x68`, `+0x6C`: position.
    pub position: [f32; 3],
    /// `+0x70`: angle about Y in degrees (also the middle Euler angle).
    pub angle_y: f32,
    /// `+0xC0`: first Euler angle in degrees.
    pub angle_a: f32,
    /// `+0xC8`: third Euler angle in degrees.
    pub angle_c: f32,
    /// `+0x274..+0x2B4`: cached world matrix.
    pub matrix: Mat4,
    /// `+0x2B4`: two bit mirror-axis codes, one per axis index (`(word >> 2*axis) & 3`).
    pub mirror_word: u32,
    /// `+0x2B8..`: per axis plane coordinate; element `k` is the float at `+0x2B8 + 4k`. For axis index `n` the original
    /// reads the float at `+0x2B4 + 4n` (so `n = 0` reads `mirror_word` as bits).
    pub plane: [f32; 7],
}

/// `0xA14B0(out, object, with_translation, axis)` (cdecl, 4 stack args). MEASURED.
///
/// `link_parent_kind_is_two` is `kind == 2` of the object reached through `[[object+0x78]]` (only evaluated for kind 3;
/// false when `[object+0x78]` is null). Behaviour:
/// - `kind == 0`: `out = identity`, nothing else touched.
/// - flag `0x1000` set: clear it and rebuild `object.matrix` by kind (1: `mat_rotation_y(angle_y*DEG_TO_RAD)`; 2: Euler;
///   3: Euler if the linked parent is kind 2 else Y rotation; other: unchanged); then, when flag `0x2000_0000` is set,
///   `matrix = scale(-1,1,1) * matrix`.
/// - `out = object.matrix`; then by `rot = (mirror_word >> (2*axis)) & 3`: 0 = nothing more; 1/2/3 = `out = out *
///   scale` with `(-1,1,1)`, `(1,-1,1)`, `(1,1,-1)`.
/// - when `with_translation`, the translation row is set from the position, where the mirrored axis coordinate (x for
///   rot 1, y for rot 2, z for rot 3) becomes `e - (p - e)` with `e = plane coordinate of the axis`. For `rot == 0`
///   with `with_translation == false` the function returns before the translation is written.
pub fn build_world_matrix(
    object: &mut WorldMatrixObject,
    out: &mut Mat4,
    with_translation: bool,
    axis: u32,
    link_parent_kind_is_two: bool,
) {
    let rot = (((object.mirror_word as i32) >> ((axis.wrapping_mul(2)) & 31)) & 3) as usize;
    if object.kind == 0 {
        *out = mat_identity();
        return;
    }
    if object.flags & 0x1000 != 0 {
        object.flags &= !0x1000;
        let rad = |deg: f32| deg * DEG_TO_RAD;
        let euler =
            |o: &WorldMatrixObject| mat_euler(rad(o.angle_a), rad(o.angle_y), rad(o.angle_c));
        match object.kind {
            1 => object.matrix = mat_rotation_y(rad(object.angle_y)),
            2 => object.matrix = euler(object),
            3 => {
                object.matrix = if link_parent_kind_is_two {
                    euler(object)
                } else {
                    mat_rotation_y(rad(object.angle_y))
                }
            }
            _ => {}
        }
        if object.flags & 0x2000_0000 != 0 {
            object.matrix = mat_premultiply(&object.matrix, &mat_scale(-1.0, 1.0, 1.0));
        }
    }
    *out = object.matrix;
    if rot == 0 {
        if with_translation {
            mat_set_translation(
                out,
                object.position[0],
                object.position[1],
                object.position[2],
            );
        }
        return;
    }
    let scale = match rot {
        1 => mat_scale(-1.0, 1.0, 1.0),
        2 => mat_scale(1.0, -1.0, 1.0),
        _ => mat_scale(1.0, 1.0, -1.0),
    };
    *out = mat_mul(out, &scale);
    if !with_translation {
        return;
    }
    let e = if axis == 0 {
        f32::from_bits(object.mirror_word)
    } else {
        object.plane.get(axis as usize - 1).copied().unwrap_or(0.0)
    };
    let mirror = |p: f32| e - (p - e);
    let mut p = object.position;
    p[rot - 1] = mirror(p[rot - 1]);
    mat_set_translation(out, p[0], p[1], p[2]);
}

// ---------------------------------------------------------------------------------------------------------------
// Collision height queries: 0xC38F0, 0xA46A0, 0xC3B10, 0xC3E60, 0x3C230
// ---------------------------------------------------------------------------------------------------------------

/// `-FLT_MAX` (`0xFF7FFFFF`, constant at `0x478618`): "nothing found" height, MEASURED.
pub const NO_HIT: f32 = f32::from_bits(0xFF7F_FFFF);

/// A collision face record as read by `0xC38F0` (entry pointed to by the list at object `+0x1F0`). Names INFERRED.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionFace {
    /// `+0x00`: number of edge records tested (unsigned loop count).
    pub edge_count: u32,
    /// `+0x04`: class mask. 0 = matches everything, else must share a bit with the query mask.
    pub mask: u32,
    /// `+0x08` (16 bit): bit `0x0800` = skipped while the global `[0x733084]` is non-zero, bit `0x0010` = flat floor
    /// face (height is the first vertex y, and the point must not lie below it).
    pub flags: u16,
    /// `+0x14`, `+0x18`, `+0x1C`, `+0x20`: x min, z min, x max, z max bounds.
    pub bounds: [f32; 4],
    /// Records of 24 bytes from `+0x24`: `x, y, z, a (+0xC), b (+0x10), unused (+0x14)`. The edge test of vertex `k` is
    /// `(px - x) * b - (pz - z) * a`; the plane height uses the first three vertices.
    pub vertices: Vec<[f32; 6]>,
}

/// `0xC38F0(count, list, mask, point, radius, out_hit)` (cdecl). Highest face surface at or below the point. MEASURED.
///
/// For each face: skipped when its mask is non-zero and shares no bit with `mask`, when flag `0x0800` is set and
/// `ignore_flag` (`[0x733084]`) is non-zero, when the floor flag `0x10` is set and `point.y < vertices[0].y`, when the
/// point (grown by `radius`) is outside the x/z bounds, or when for any edge `0 - radius > (px - x) * b - (pz - z) * a`.
/// The height is `vertices[0].y` for floor faces, else the plane through the first three vertices evaluated at (x, z)
/// in the original f32 operation order. A face wins when `y > best` and `point.y >= y` (comiss semantics, NaN never
/// wins). Returns the best height (`NO_HIT` if none) and the winning face index.
pub fn face_height_query(
    faces: &[CollisionFace],
    mask: u32,
    point: [f32; 3],
    radius: f32,
    ignore_flag: bool,
) -> (f32, Option<usize>) {
    let (px, py, pz) = (point[0], point[1], point[2]);
    let mut best = NO_HIT;
    let mut best_face = None;
    for (index, face) in faces.iter().enumerate() {
        if face.mask != 0 && mask & face.mask == 0 {
            continue;
        }
        if face.flags & 0x0800 != 0 && ignore_flag {
            continue;
        }
        let floor = face.flags & 0x10 != 0;
        let v0 = face.vertices[0];
        if floor && !(py >= v0[1]) {
            continue;
        }
        let [xmin, zmin, xmax, zmax] = face.bounds;
        if !(px + radius >= xmin) || !(xmax >= px - radius) {
            continue;
        }
        if !(radius + pz >= zmin) || !(zmax >= pz - radius) {
            continue;
        }
        let negative_radius = 0.0f32 - radius;
        let outside = face.vertices[..face.edge_count as usize].iter().any(|v| {
            let value = (px - v[0]) * v[4] - (pz - v[2]) * v[3];
            negative_radius > value
        });
        if outside {
            continue;
        }
        let y = if floor {
            v0[1]
        } else {
            let (v1, v2) = (face.vertices[1], face.vertices[2]);
            let dz1 = v1[2] - v0[2];
            let dz2 = v2[2] - v1[2];
            let dy1 = v1[1] - v0[1];
            let dy2 = v2[1] - v1[1];
            let dx2 = v2[0] - v1[0];
            let dx1 = v1[0] - v0[0];
            let a = dz2 * dy1 - dz1 * dy2;
            let b = dx1 * dy2 - dx2 * dy1;
            let n = dx2 * dz1 - dx1 * dz2;
            let mut sum = v0[2] * b + v0[0] * a;
            sum += n * v0[1];
            sum -= a * px;
            sum -= b * pz;
            sum / n
        };
        if !(y > best) || !(py >= y) {
            continue;
        }
        best = y;
        best_face = Some(index);
    }
    (best, best_face)
}

/// A world object as far as the height queries read it (object record offsets noted, names INFERRED).
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionObject {
    /// `[+0x1F0]` list and `[+0x1E0]` count. `None` when the list pointer is null (object skipped by group scans).
    pub faces: Option<Vec<CollisionFace>>,
    /// `[[+4] + 0x30]`: class mask passed to `0xC38F0`.
    pub class_mask: u32,
    /// Byte `[[+0x24] + 0x28]`: group scans require `& 6 != 0`.
    pub shape_bits: u8,
    /// `[+0x28]`: group scans require bit `0x40` set and bit 31 clear.
    pub state: u32,
}

/// Result of a height query: the value plus the hit face and the owning object (the two out pointers).
#[derive(Clone, Debug, PartialEq)]
pub struct HeightHit {
    pub value: f32,
    /// `(object index, face index)`; the original returns the face address.
    pub face: Option<(usize, usize)>,
    /// Object index, only set by the group scan path (`0xA46A0`); the original returns the object address.
    pub object: Option<usize>,
}

/// The globals and tables read by the height queries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollisionWorld {
    pub objects: Vec<CollisionObject>,
    /// `[0x7B9784]` table by id (valid ids `1..=id_limit`) mapping to an object index.
    pub id_table: Vec<usize>,
    /// `[0x7B975C]`: highest valid id.
    pub id_limit: i32,
    /// Per group lists (counts at `0x7B54E0 + 4g`, pointer arrays at `0x7B5F00 + 4g`) of object indices.
    pub groups: Vec<Vec<usize>>,
    /// `[0x7B5834]` count and `[0x7B6254]` array: the all objects list scanned when the group has no hit.
    pub all_objects: Vec<usize>,
    /// `[0x733084]`: faces with flag `0x0800` are ignored while non-zero.
    pub ignore_flag: bool,
    /// `[0x4DCAFC]`: best height of the current `0xC3E60` run (set to `NO_HIT` there, raised by `0xC3B10`).
    pub best_value: f32,
    /// `[0x733088]`: id that last raised `best_value`.
    pub best_id: i32,
}

impl CollisionWorld {
    fn object_usable_for_scan(&self, index: usize) -> bool {
        let object = &self.objects[index];
        object.faces.is_some()
            && object.shape_bits & 6 != 0
            && object.state & 0x40 != 0
            && (object.state as i32) >= 0
    }

    fn scan_object(&self, index: usize, point: [f32; 3]) -> (f32, Option<usize>) {
        let object = &self.objects[index];
        let faces = object.faces.as_deref().unwrap_or(&[]);
        face_height_query(faces, object.class_mask, point, 0.0, self.ignore_flag)
    }

    /// `0xA46A0(group, point, out_hit, out_object)`: best height over the objects of `group` (then, when none of them
    /// produced a hit, over `all_objects`), using radius 0. An object counts when its list is set, `shape_bits & 6`,
    /// state bit `0x40` set and state bit 31 clear; it wins when its height is `> best`. MEASURED.
    pub fn group_height_query(&self, group: usize, point: [f32; 3]) -> HeightHit {
        let mut best = HeightHit {
            value: NO_HIT,
            face: None,
            object: None,
        };
        let scan = |list: &[usize], best: &mut HeightHit| {
            for &index in list {
                if !self.object_usable_for_scan(index) {
                    continue;
                }
                let (value, face) = self.scan_object(index, point);
                if value > best.value {
                    *best = HeightHit {
                        value,
                        face: face.map(|f| (index, f)),
                        object: Some(index),
                    };
                }
            }
        };
        let group_list = self.groups.get(group).map(Vec::as_slice).unwrap_or(&[]);
        scan(group_list, &mut best);
        if best.object.is_none() {
            scan(&self.all_objects, &mut best);
        }
        best
    }

    /// `0xC3B10(id, point, radius, out_hit, out_object, skip_group_scan)`. MEASURED.
    ///
    /// An id outside `1..=id_limit` returns `NO_HIT` with both outputs cleared. Otherwise `0xC38F0` runs over the id's
    /// object faces (its height `d1` raises `best_value`/`best_id` when `d1 > best_value`) and, unless
    /// `skip_group_scan`, `group_height_query(id, point)` gives `d2` (the group scan uses the id as group number);
    /// `d2 > d1` replaces the result (face and object come from the group scan), else the object output is cleared.
    pub fn id_height_query(
        &mut self,
        id: i32,
        point: [f32; 3],
        radius: f32,
        skip_group_scan: bool,
    ) -> HeightHit {
        if id <= 0 || id > self.id_limit {
            return HeightHit {
                value: NO_HIT,
                face: None,
                object: None,
            };
        }
        let index = self.id_table[id as usize];
        let object = &self.objects[index];
        let faces = object.faces.as_deref().unwrap_or(&[]);
        let (d1, face1) =
            face_height_query(faces, object.class_mask, point, radius, self.ignore_flag);
        if d1 > self.best_value {
            self.best_value = d1;
            self.best_id = id;
        }
        let second = if skip_group_scan {
            HeightHit {
                value: NO_HIT,
                face: None,
                object: None,
            }
        } else {
            self.group_height_query(id as usize, point)
        };
        if second.value > d1 {
            HeightHit {
                value: second.value,
                face: second.face,
                object: second.object,
            }
        } else {
            HeightHit {
                value: d1,
                face: face1.map(|f| (index, f)),
                object: None,
            }
        }
    }

    /// `0xC3E60(point, y_bias, half_extent, out_hit, out_object, skip_group_scan)`. MEASURED (hit loop checked
    /// through a stubbed `0x87590`).
    ///
    /// Queries the region box `(x-h, y, z-h)..(x+h, y+y_bias, z+h)` with `region_query` (the original's `0x87590`, ring of
    /// 30 ids, see `callees_t1846::region_query_block`), then runs `id_height_query(id, (x, y+y_bias, z), 0, flag)` for
    /// each returned id. The result with the largest height wins (`d > best` strictly, `best` starts at `NO_HIT`, NaN
    /// never wins). When the query returned exactly zero ids, `group_height_query(0xD5, point)` is the only candidate
    /// (group `0xD5` appears as a constant). Sets `best_value = NO_HIT` first. Returns the best value when a hit face
    /// was recorded, else `NO_HIT` (the original first calls `0x3C230` with an empty string, which never finds
    /// anything; the alternative result `-1.0` of `[0x4760D0]` is dead code).
    pub fn region_height_query(
        &mut self,
        point: [f32; 3],
        y_bias: f32,
        half_extent: f32,
        mut region_query: impl FnMut([f32; 6]) -> Vec<i32>,
        skip_group_scan: bool,
    ) -> HeightHit {
        let [x, y, z] = point;
        let region = [
            x - half_extent,
            y,
            z - half_extent,
            x + half_extent,
            y_bias + y,
            half_extent + z,
        ];
        let ids: Vec<i32> = region_query(region).into_iter().take(30).collect();
        let query_point = [x, y_bias + y, z];
        self.best_value = NO_HIT;
        let mut best = HeightHit {
            value: NO_HIT,
            face: None,
            object: None,
        };
        let consider = |candidate: HeightHit, best: &mut HeightHit| {
            if candidate.value > best.value {
                *best = candidate;
            }
        };
        for &id in &ids {
            let candidate = self.id_height_query(id, query_point, 0.0, skip_group_scan);
            consider(candidate, &mut best);
        }
        if ids.is_empty() {
            let candidate = self.group_height_query(0xD5, point);
            consider(candidate, &mut best);
        }
        if best.face.is_some() {
            best
        } else {
            HeightHit {
                value: NO_HIT,
                face: best.face,
                object: best.object,
            }
        }
    }
}

/// `0x3C230(name)`: index (>= 1) of the first entry of a name table (entries `1..count`, entry 0 unused) equal to
/// `name`, else `None`. An empty `name` never matches (the function returns 0 before comparing). Sets `[0x6B7A20]` to
/// the index (0 if none) in the original. MEASURED by reading; not golden-tested (needs the runtime table). INFERRED
/// meaning: console variable or debug flag lookup.
pub fn find_name_index(table: &[&[u8]], name: &[u8]) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    (1..table.len()).find(|&i| table[i] == name)
}

// ---------------------------------------------------------------------------------------------------------------
// Small pure callees of the sweep: 0x4A800, 0x4A210, 0x152090, 0x1530E0, 0x1665A0, 0x1665E0, 0x1665F0, 0x389B0
// ---------------------------------------------------------------------------------------------------------------

/// `[0x476240]` (`0x38D1B717`): per axis |delta| below this counts as no motion (MEASURED).
pub const MOTION_EPSILON: f32 = f32::from_bits(0x38D1_B717);
/// `[0x4765F4]` (`0x33D6BF95`): squared length above which the node normal is renormalised (MEASURED).
pub const NORMAL_EPSILON_SQ: f32 = f32::from_bits(0x33D6_BF95);

/// The 0x38 byte sweep record built by `0x4A800` (stack local of `0x92360`). Word layout: `+0x00` stationary flag,
/// `+0x04` start, `+0x10` delta (only rewritten while moving), `+0x1C` radius, `+0x20` box min, `+0x2C` box max.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SweepBox {
    pub stationary: u32,
    pub start: [f32; 3],
    pub delta: [f32; 3],
    pub radius: f32,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl SweepBox {
    /// The 14 dwords of the record.
    pub fn words(&self) -> [u32; 14] {
        let bits = |v: f32| v.to_bits();
        [
            self.stationary,
            bits(self.start[0]),
            bits(self.start[1]),
            bits(self.start[2]),
            bits(self.delta[0]),
            bits(self.delta[1]),
            bits(self.delta[2]),
            bits(self.radius),
            bits(self.min[0]),
            bits(self.min[1]),
            bits(self.min[2]),
            bits(self.max[0]),
            bits(self.max[1]),
            bits(self.max[2]),
        ]
    }

    /// `0x4A800(box, start, delta, radius)` (MEASURED). Builds the AABB of a sphere of `radius` swept from `start`
    /// along `delta`. A delta whose three components all have `|d| < MOTION_EPSILON` is "stationary" (flag 1, box is
    /// `start +- radius`, delta left unchanged). Otherwise the delta is stored and per axis a negative delta (`0 > d`)
    /// gives `min = (s + d) - r`, `max = s + r` (y: `r + s`), else `min = s - r`, `max = (s + d) + r` (y: `(r + s) + d`).
    pub fn build(&mut self, start: [f32; 3], delta: [f32; 3], radius: f32) {
        self.start = start;
        self.radius = radius;
        let stationary = delta.iter().all(|d| MOTION_EPSILON > d.abs());
        self.stationary = u32::from(stationary);
        if stationary {
            for axis in 0..3 {
                self.min[axis] = start[axis] - radius;
                self.max[axis] = if axis == 1 {
                    radius + start[1]
                } else {
                    start[axis] + radius
                };
            }
            return;
        }
        self.delta = delta;
        for axis in 0..3 {
            let (s, d) = (start[axis], delta[axis]);
            if 0.0 > d {
                self.min[axis] = (s + d) - radius;
                self.max[axis] = if axis == 1 { radius + s } else { s + radius };
            } else {
                self.min[axis] = s - radius;
                self.max[axis] = if axis == 1 {
                    (radius + s) + d
                } else {
                    (s + d) + radius
                };
            }
        }
    }

    /// `0x4A210(box, query)`: true when the box overlaps the query AABB `[min xyz, max xyz]`; any NaN compare counts
    /// as "not greater" (MEASURED).
    pub fn overlaps(&self, query: &[f32; 6]) -> bool {
        !(query[0] > self.max[0]
            || self.min[0] > query[3]
            || query[1] > self.max[1]
            || self.min[1] > query[4]
            || query[2] > self.max[2]
            || self.min[2] > query[5])
    }
}

/// `0x152090(matrix, vector)`: rotates `v` in place by the 3x3 part of the matrix (row vector convention,
/// `v' = v * M`), f32 in the original sum order `(a + b) + c` (MEASURED).
pub fn transform_direction(m: &Mat4, v: &mut [f32; 3]) {
    let [x, y, z] = *v;
    v[0] = (m[8] * z + m[4] * y) + m[0] * x;
    v[1] = (m[9] * z + m[5] * y) + m[1] * x;
    v[2] = (m[10] * z + m[6] * y) + m[2] * x;
}

/// `0x1530E0(out, matrix)`: the transposed matrix (MEASURED).
pub fn transpose(m: &Mat4) -> Mat4 {
    let mut out = [0.0f32; 16];
    for row in 0..4 {
        for col in 0..4 {
            out[col * 4 + row] = m[row * 4 + col];
        }
    }
    out
}

/// One 0x38 byte impact record from the pool (`0x1665A0`), as 14 dwords. Offsets: `+0x00` position, `+0x0C` normal,
/// `+0x18` shape id (as float bits), `+0x1C` float, `+0x24` / `+0x28` the two objects, `+0x2C` u16 flags, `+0x2E`
/// u16 kind, `+0x30` u16 part index, `+0x34` next link (INFERRED names).
pub type ImpactNode = [u32; 14];

/// Impact node pool of `0x1665A0`: `[0x4E826C]` free count, `[0x74C468]` base, nodes of 0x38 bytes. Allocation takes
/// the node at index `free_count - 1` (post decrement); with no free node it returns null (MEASURED).
#[derive(Clone, Debug, PartialEq)]
pub struct ImpactPool {
    /// `[0x74C468]`: address of node 0.
    pub base_address: u32,
    pub free_count: i32,
    pub nodes: Vec<ImpactNode>,
}

impl ImpactPool {
    pub fn allocate(&mut self) -> Option<usize> {
        if self.free_count <= 0 {
            return None;
        }
        self.free_count -= 1;
        Some(self.free_count as usize)
    }
}

/// The impact list head: `+0` first node (address), `+4` byte flag read by the sweep after the dynamic query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImpactList {
    pub first: u32,
    pub flag4: u8,
}

/// `0x1665E0(head, node)`: push front: `node.next(+0x34) = head.first; head.first = node_address` (MEASURED).
pub fn push_impact_node(head: &mut ImpactList, node: &mut ImpactNode, node_address: u32) {
    node[13] = head.first;
    head.first = node_address;
}

fn set_half(node: &mut ImpactNode, byte_offset: usize, value: u16) {
    let word = byte_offset / 4;
    let shift = (byte_offset % 4) * 8;
    node[word] = (node[word] & !(0xFFFF << shift)) | (u32::from(value) << shift);
}

/// The second object of an impact as read by `0x1665F0`: its address and the dword at `+0x20`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImpactOther {
    pub address: u32,
    pub kind_20: u32,
}

/// `0x1665F0(node, position, normal, f1c, f18, flags16, body, other, part_index)` (MEASURED, golden-verified through
/// the sweep). Fills the node. `part_kind` is the byte at `+0x18` of the part record selected by `part_index` (the
/// sweep always passes index -1, so `None`). With an `other` object whose `kind_20 == 8` the normal is replaced by the
/// horizontal vector `(-n.x, 0, -n.z)` renormalised with `rsqrt` when its squared length exceeds
/// `NORMAL_EPSILON_SQ`, objects go to `+0x24 = body`, `+0x28 = other`, kind 3. Otherwise `+0x24 = other`, `+0x28 =
/// body`, and the kind is 5 for part kinds 1 or 2, 4 when `flags16 & 0x800`, else `body_field_80 != 0`.
#[allow(clippy::too_many_arguments)]
pub fn init_impact_node(
    node: &mut ImpactNode,
    position: [f32; 3],
    normal: [f32; 3],
    f1c: f32,
    f18_bits: u32,
    flags16: u16,
    body: u32,
    other: Option<ImpactOther>,
    part_index: i32,
    part_kind: Option<u8>,
    body_field_80: u32,
) {
    node[7] = f1c.to_bits();
    node[6] = f18_bits;
    for axis in 0..3 {
        node[axis] = position[axis].to_bits();
    }
    set_half(node, 0x2C, flags16);
    set_half(node, 0x30, part_index as u16);
    let other_address = other.map_or(0, |o| o.address);
    if let Some(o) = other.filter(|o| o.kind_20 == 8) {
        let nx = 0.0f32 - normal[0];
        let nz = 0.0f32 - normal[2];
        node[4] = 0.0f32.to_bits();
        node[3] = nx.to_bits();
        node[5] = nz.to_bits();
        let length_squared = nx * nx + nz * nz;
        node[9] = body;
        node[10] = o.address;
        set_half(node, 0x2E, 3);
        if length_squared > NORMAL_EPSILON_SQ {
            let scale = (1.0f32 / length_squared.sqrt()) * 1.0f32;
            node[3] = (nx * scale).to_bits();
            node[4] = (scale * 0.0f32).to_bits();
            node[5] = (nz * scale).to_bits();
        }
        return;
    }
    for axis in 0..3 {
        node[3 + axis] = normal[axis].to_bits();
    }
    node[9] = other_address;
    node[10] = body;
    let has_part = part_index >= 0;
    if has_part && matches!(part_kind, Some(1) | Some(2)) {
        set_half(node, 0x2E, 5);
        return;
    }
    if flags16 & 0x800 != 0 {
        set_half(node, 0x2E, 4);
        return;
    }
    set_half(node, 0x2E, u16::from(body_field_80 != 0));
}

// ---------------------------------------------------------------------------------------------------------------
// The sweep: 0x92360
// ---------------------------------------------------------------------------------------------------------------

/// A shape of a sweep record (`0x1C` bytes at `[record+0x8C] + 0x1C * j`): `kind` dword then `a` (`+4`) and `b` (`+0x10`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepItem {
    pub kind: u32,
    pub a: [f32; 3],
    pub b: [f32; 3],
}

/// Bounds the sweep box is tested against: the first six floats of `[record+0xA8]`; `detail_address` is the dword at
/// `+0x24` of that bounds record (non-zero required), `address` the bounds record pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepBounds {
    pub address: u32,
    pub aabb: [f32; 6],
    pub detail_address: u32,
}

/// One `0xC8` byte record of the header's array (fields used by the sweep).
#[derive(Clone, Debug, PartialEq)]
pub struct SweepRecord {
    /// `+0x5C`: mask, 0 = always, else must share a bit with the mover mask.
    pub mask: u32,
    /// `+0x49` (signed byte): index of the part matrix of the mover (`mover.part_matrices`).
    pub matrix_index: i8,
    /// `+0x58` (signed 16 bit): shape count; 0 selects the `0x91EC0` path, negative skips the shapes.
    pub item_count: i16,
    /// `[+0x8C]` array of shapes (only `item_count` entries are read).
    pub items: Vec<SweepItem>,
    /// `[+0xA8]`; `None` skips the record.
    pub bounds: Option<SweepBounds>,
}

/// The header (`[mover+4]`): `+0` record count (the array lies immediately before the header), `+4` signed value.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepHeader {
    pub records: Vec<SweepRecord>,
    pub h4: i32,
}

/// The mover passed as first argument: `+0` body, `+4` header, `+0x10` part matrix array, `+0x30` mask.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepMover {
    pub address: u32,
    pub mask: u32,
    pub part_matrices: Vec<Mat4>,
}

/// The moving body (`[mover]`, `esi`).
#[derive(Clone, Debug, PartialEq)]
pub struct SweepBody {
    pub address: u32,
    /// Fields read by `0xA14B0` (`+0x28`, `+0x32`, `+0x64..`, ...).
    pub object: WorldMatrixObject,
    /// `+0x1A8`, `+0x1AC`, `+0x1B0`: bound centre offset.
    pub box_offset: [f32; 3],
    /// `+0x1C0`, `+0x1C4`, `+0x1C8`: bound half extents.
    pub box_half: [f32; 3],
    /// `kind == 2` of `[[object+0x78]]`, see `build_world_matrix`.
    pub link_parent_kind_is_two: bool,
    /// Dword at `+0x80` (impact kind selector).
    pub field_80: u32,
}

/// Result of a shape test (`0x4A670` kind 0, `0x4FE00` kind 1, `0x52550` kind 2): non-zero return means contact; the
/// position, normal and shape id are the three out parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShapeTestResult {
    pub code: u32,
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub shape_id: u32,
}

/// Arguments of `0x91EC0` (records without shapes), all as the sweep passes them.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordTestCall {
    pub mover_address: u32,
    pub record_index: i32,
    pub bounds_address: u32,
    pub detail_address: u32,
    pub sweep: SweepBox,
    pub arg5: u32,
    pub other_address: u32,
    pub list_address: u32,
    /// The matrix of `0xA14B0` (zero when the body kind is 0).
    pub body_matrix: Mat4,
    pub arg9: u32,
}

/// One entry of the `0x128760` result (13 dwords): `w0`, vectors at `+4`, `+0x10`, `+0x1C`, object at `+0x30`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicEntry {
    pub w0: u32,
    pub vector_a: [f32; 3],
    pub vector_b: [f32; 3],
    pub vector_c: [f32; 3],
    pub object: u32,
}

/// The heavy world services the sweep calls. None of them is decoded here: their arguments are the contract.
pub trait SweepServices {
    /// `0x4A670(item.a, item.b.x, start, radius, ..)` for kind 0, `0x4FE00(start, radius, a, b, identity, ..)` for kind 1,
    /// `0x52550` (same shape as 4FE00) for kind 2.
    fn shape_test(&mut self, item: &SweepItem, start: [f32; 3], radius: f32) -> ShapeTestResult;
    /// `0x91EC0` (359 instructions, 11 arguments): returns a code OR-ed into the sweep result.
    fn record_test(&mut self, call: &RecordTestCall) -> u32;
    /// `0x128760(body, other, start, delta, radius, out, list)`: dynamic object query, up to 3 entries, plus the raw
    /// `count` and second dword the original leaves in the result struct.
    fn dynamic_query(
        &mut self,
        body_address: u32,
        other_address: u32,
        start: [f32; 3],
        delta: [f32; 3],
        radius: f32,
        list_address: u32,
        list: &mut ImpactList,
    ) -> (i32, Vec<DynamicEntry>);
    /// First float and u16 at `+0x28` of a dynamic entry's object (`[object]`, `word [object + 0x28]`).
    fn dynamic_object_info(&mut self, address: u32) -> (f32, u16);
    /// `0x129F50(w0, object, vector_a, vector_c, vector_b, 1.0)` applied to a dynamic entry (return ignored).
    fn dynamic_apply(&mut self, entry: &DynamicEntry);
}

/// The scalar arguments of `0x92360` after the mover.
#[derive(Clone, Copy, Debug)]
pub struct SweepArgs {
    /// E+8: start point.
    pub start: [f32; 3],
    /// E+0xC: movement.
    pub delta: [f32; 3],
    /// E+0x10: sphere radius.
    pub radius: f32,
    /// E+0x14: stored in the node flags (low 16 bits) and passed to `0x91EC0`.
    pub arg5: u32,
    /// E+0x18: second object (address, `+0x20` dword); `None` when null.
    pub other: Option<ImpactOther>,
    /// E+0x1C: address of the impact list head.
    pub list_address: u32,
    /// E+0x20 non-zero: run the dynamic object query.
    pub dynamic: bool,
    /// E+0x24: passed to `0x91EC0`.
    pub arg9: u32,
}

/// `0x92360(mover, start, delta, radius, arg5, other, list, dynamic, arg9)` (cdecl, 9 stack args). Decoded in full from
/// the machine code; the control flow, the box maths, matrix inversions, node fill and result OR are MEASURED
/// bit-exactly with logging stubs for the heavy callees (see the test), the callees' own results are not. Returns
/// the OR of every contact code.
///
/// Steps: (1) build the body box `centre = position + offset`, `+- half` (min `(c - h)`, max `(c + h)`, z max
/// `(oz + hz) + pz`), build the sweep box with `0x4A800`, return 0 when it does not overlap the body box (`0x4A210`).
/// (2) `flag` = header `h4 > 0` and body flags `& 8` and `& 0x200000`. For a body of kind != 0 compute its matrix
/// (`0xA14B0(out, body, 1, 0)`), the local start `start - translation` and the local delta rotated by the transposed
/// matrix; without flag the box is rebuilt from them. For kind 0 and flag, local start/delta are copies. (3) For each
/// record whose mask matches (`mover.mask & mask != 0` or mask 0), has bounds with a detail pointer: when flag, the box
/// is rebuilt from the local vectors moved into the space of the part matrix (`start - translation`, both rotated by the
/// transposed part matrix); the box must overlap the bounds. A record with `item_count == 0` calls `record_test`
/// (`0x91EC0`) and ORs the result. Otherwise each shape is tested (kinds 0, 1, 2 only; any other kind re-uses the
/// previous code and the previous out values), and for every non-zero code a node is taken from the pool, filled by
/// `0x1665F0` (shape id as float bits, position, normal, flags, body, other, index -1), pushed on the list head and the
/// code ORed (no free node: nothing). (4) With `dynamic`: `dynamic_query`; unless the head's `flag4` is set, every
/// entry with object float `>= 0` and object word `& 0x40C == 0` is passed to `dynamic_apply`.
pub fn sweep_body(
    body: &mut SweepBody,
    mover: &SweepMover,
    header: &SweepHeader,
    args: &SweepArgs,
    list: &mut ImpactList,
    pool: &mut ImpactPool,
    services: &mut dyn SweepServices,
) -> u32 {
    let object = &body.object;
    let (position, offset, half) = (object.position, body.box_offset, body.box_half);
    let body_box: [f32; 6] = [
        position[0] + offset[0] - half[0],
        position[1] + offset[1] - half[1],
        offset[2] + position[2] - half[2],
        position[0] + offset[0] + half[0],
        position[1] + offset[1] + half[1],
        (offset[2] + half[2]) + position[2],
    ];
    let mut sweep = SweepBox::default();
    sweep.build(args.start, args.delta, args.radius);
    if !sweep.overlaps(&body_box) {
        return 0;
    }
    let state = body.object.flags;
    let use_local = header.h4 > 0 && state & 8 != 0 && state & 0x20_0000 != 0;
    let mut result = 0u32;
    let mut body_matrix: Mat4 = [0.0; 16];
    let mut local_start = [0.0f32; 3];
    let mut local_delta = [0.0f32; 3];
    if body.object.kind != 0 {
        let link = body.link_parent_kind_is_two;
        build_world_matrix(&mut body.object, &mut body_matrix, true, 0, link);
        local_start = [
            args.start[0] - body_matrix[12],
            args.start[1] - body_matrix[13],
            args.start[2] - body_matrix[14],
        ];
        local_delta = args.delta;
        let inverse = transpose(&body_matrix);
        transform_direction(&inverse, &mut local_start);
        transform_direction(&inverse, &mut local_delta);
        if !use_local {
            sweep.build(local_start, local_delta, args.radius);
        }
    } else if use_local {
        local_start = args.start;
        local_delta = args.delta;
    }
    let (mut shape_position, mut shape_normal, mut shape_id) = ([0.0f32; 3], [0.0f32; 3], 0u32);
    for (index, record) in header.records.iter().enumerate() {
        if record.mask != 0 && mover.mask & record.mask == 0 {
            continue;
        }
        let Some(bounds) = record.bounds else {
            continue;
        };
        if bounds.detail_address == 0 {
            continue;
        }
        if use_local {
            let part = &mover.part_matrices[record.matrix_index as usize];
            let mut start = [
                local_start[0] - part[12],
                local_start[1] - part[13],
                local_start[2] - part[14],
            ];
            let mut delta = local_delta;
            let inverse = transpose(part);
            transform_direction(&inverse, &mut start);
            transform_direction(&inverse, &mut delta);
            sweep.build(start, delta, args.radius);
        }
        if !sweep.overlaps(&bounds.aabb) {
            continue;
        }
        if record.item_count == 0 {
            result |= services.record_test(&RecordTestCall {
                mover_address: mover.address,
                record_index: index as i32,
                bounds_address: bounds.address,
                detail_address: bounds.detail_address,
                sweep,
                arg5: args.arg5,
                other_address: args.other.map_or(0, |o| o.address),
                list_address: args.list_address,
                body_matrix,
                arg9: args.arg9,
            });
            continue;
        }
        let mut code = 0u32;
        for item in record.items.iter().take(record.item_count.max(0) as usize) {
            if item.kind <= 2 {
                let hit = services.shape_test(item, args.start, args.radius);
                code = hit.code;
                shape_position = hit.position;
                shape_normal = hit.normal;
                shape_id = hit.shape_id;
            }
            if code == 0 {
                continue;
            }
            let Some(slot) = pool.allocate() else {
                continue;
            };
            init_impact_node(
                &mut pool.nodes[slot],
                shape_position,
                shape_normal,
                0.0,
                shape_id,
                args.arg5 as u16,
                body.address,
                args.other,
                -1,
                None,
                body.field_80,
            );
            let address = pool.base_address + 0x38 * slot as u32;
            push_impact_node(list, &mut pool.nodes[slot], address);
            result |= code;
        }
    }
    if args.dynamic {
        let (count, entries) = services.dynamic_query(
            body.address,
            args.other.map_or(0, |o| o.address),
            args.start,
            args.delta,
            args.radius,
            args.list_address,
            list,
        );
        if list.flag4 == 0 {
            for entry in entries.iter().take(count.max(0) as usize) {
                let (value, word) = services.dynamic_object_info(entry.object);
                if !(value >= 0.0) || word & 0x40C != 0 {
                    continue;
                }
                services.dynamic_apply(entry);
            }
        }
    }
    result
}
