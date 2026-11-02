//! T1846 batch 8: world matrix builder and world queries against emulated-original golden vectors (MEASURED).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::world_queries::*;

const OBJ: u32 = 0x101_0000;
const OUT: u32 = 0x100_0000;
const PARENT: u32 = 0x103_0000;
const ARGS: u32 = 0x104_0000;

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn matrix_at(case: &Case, address: u32, output: bool) -> Mat4 {
    let mut m = [0.0f32; 16];
    for (i, v) in m.iter_mut().enumerate() {
        let a = address + 4 * i as u32;
        *v = if output {
            case.output_f(a)
        } else {
            case.input_f(a)
        };
    }
    m
}

fn object_from(case: &Case, base: u32) -> WorldMatrixObject {
    let f = |off: u32| case.input_f(base + off);
    let mut plane = [0.0f32; 7];
    for (k, p) in plane.iter_mut().enumerate() {
        *p = f(0x2B8 + 4 * k as u32);
    }
    WorldMatrixObject {
        kind: (case.input(base + 0x30) >> 16) as u8 as i8,
        flags: case.input(base + 0x28),
        position: [f(0x64), f(0x68), f(0x6C)],
        angle_y: f(0x70),
        angle_a: f(0xC0),
        angle_c: f(0xC8),
        matrix: matrix_at(case, base + 0x274, false),
        mirror_word: case.input(base + 0x2B4),
        plane,
    }
}

#[test]
fn world_matrix_builder_matches_original() {
    let cases = parse(include_str!("data/t1846/world_matrix_a14b0.golden"));
    assert!(cases.len() >= 40);
    let (mut kinds, mut rots) = ([0u32; 5], [0u32; 4]);
    for (n, case) in cases.iter().enumerate() {
        let mut object = object_from(case, OBJ);
        let parent_two =
            case.input(OBJ + 0x78) != 0 && (case.input(PARENT + 0x30) >> 16) as u8 == 2;
        let (with_translation, axis) = (case.input(ARGS) != 0, case.input(ARGS + 4));
        kinds[(object.kind.clamp(0, 4)) as usize] += 1;
        rots[((object.mirror_word >> (2 * axis)) & 3) as usize] += 1;
        let mut out = matrix_at(case, OUT, true); // untouched words keep their input value (zero)
        out.fill(0.0);
        build_world_matrix(&mut object, &mut out, with_translation, axis, parent_two);
        let expected_out = matrix_at(case, OUT, true);
        let expected_matrix = matrix_at(case, OBJ + 0x274, true);
        for i in 0..16 {
            assert!(
                same(out[i], expected_out[i]),
                "case {n} out[{i}] {} vs {}",
                out[i],
                expected_out[i]
            );
            assert!(
                same(object.matrix[i], expected_matrix[i]),
                "case {n} matrix[{i}]"
            );
        }
        assert_eq!(object.flags, case.output(OBJ + 0x28), "case {n} flags");
    }
    assert!(kinds.iter().all(|&c| c > 2), "kinds {kinds:?}");
    assert!(rots.iter().all(|&c| c > 5), "rots {rots:?}");
}

#[test]
fn matrix_helpers_semantics() {
    assert_eq!(mat_scale(2.0, 3.0, 4.0)[10], 4.0);
    let t = mat_rotation_y(0.0);
    assert_eq!(t, IDENTITY);
    let m = mat_mul(&mat_scale(2.0, 2.0, 2.0), &IDENTITY);
    assert_eq!(m[0], 2.0);
    let mut k = IDENTITY;
    mat_set_translation(&mut k, 1.0, 2.0, 3.0);
    assert_eq!(&k[12..15], &[1.0, 2.0, 3.0]);
}

const OBJ0: u32 = 0x120_0000;
const NOBJ: u32 = 0x137_0000;
const QARGS: u32 = 0x138_0000;
const POINT: u32 = 0x139_0000;
const HITS: u32 = 0x13B_0000;
const OUTS: u32 = 0x136_0000;
const IDT: u32 = 0x134_0000;
const ALLARR: u32 = 0x135_0000;
const G_BEST: u32 = 0x4D_CAFC;
const G_BESTID: u32 = 0x73_3088;

struct Built {
    world: CollisionWorld,
    face_at: std::collections::HashMap<u32, (usize, usize)>,
}

fn object_index(address: u32) -> usize {
    ((address - OBJ0) / 0x400) as usize
}

fn read_face(case: &Case, a: u32) -> CollisionFace {
    let vertices = (0..5u32)
        .map(|k| {
            let v = a + 0x24 + 0x18 * k;
            let f = |i: u32| case.input_f(v + 4 * i);
            [f(0), f(1), f(2), f(3), f(4), f(5)]
        })
        .collect();
    CollisionFace {
        edge_count: case.input(a),
        mask: case.input(a + 4),
        flags: case.input(a + 8) as u16,
        bounds: [
            case.input_f(a + 0x14),
            case.input_f(a + 0x18),
            case.input_f(a + 0x1C),
            case.input_f(a + 0x20),
        ],
        vertices,
    }
}

fn read_world(case: &Case) -> Built {
    let mut face_at = std::collections::HashMap::new();
    let mut objects = Vec::new();
    for k in 0..case.input(NOBJ) as usize {
        let base = OBJ0 + 0x400 * k as u32;
        let (count, list) = (case.input(base + 0x1E0), case.input(base + 0x1F0));
        let faces = (list != 0).then(|| {
            (0..count)
                .map(|i| {
                    let a = case.input(list + 4 * i);
                    face_at.insert(a, (k, i as usize));
                    read_face(case, a)
                })
                .collect()
        });
        objects.push(CollisionObject {
            faces,
            class_mask: case.input(case.input(base + 4) + 0x30),
            shape_bits: case.input(case.input(base + 0x24) + 0x28) as u8,
            state: case.input(base + 0x28),
        });
    }
    let limit = case.input(0x7B_975C) as i32;
    let id_table = (0..=limit.max(0) as u32)
        .map(|id| {
            if id == 0 {
                0
            } else {
                object_index(case.input(IDT + 4 * id))
            }
        })
        .collect();
    let list_of = |count: u32, array: u32| -> Vec<usize> {
        (0..count)
            .map(|i| object_index(case.input(array + 4 * i)))
            .collect()
    };
    let groups = (0..0xD6u32)
        .map(|g| list_of(case.input(0x7B_54E0 + 4 * g), case.input(0x7B_5F00 + 4 * g)))
        .collect();
    let world = CollisionWorld {
        objects,
        id_table,
        id_limit: limit,
        groups,
        all_objects: list_of(case.input(0x7B_5834), ALLARR),
        ignore_flag: case.input(0x73_3084) != 0,
        best_value: case.input_f(G_BEST),
        best_id: case.input(G_BESTID) as i32,
    };
    Built { world, face_at }
}

fn point_of(case: &Case) -> [f32; 3] {
    case.vec3(POINT)
}

fn check_hit(case: &Case, built: &Built, n: usize, hit: &HeightHit, with_object: bool) {
    let expected = case.output_f(0x13D_0000);
    assert!(
        same(hit.value, expected),
        "case {n} value {} vs {expected}",
        hit.value
    );
    let face_addr = hit
        .face
        .map(|(object, face)| {
            *built
                .face_at
                .iter()
                .find(|(_, v)| **v == (object, face))
                .expect("face address")
                .0
        })
        .unwrap_or(0);
    assert_eq!(face_addr, case.output(OUTS), "case {n} hit face");
    if with_object {
        let object_addr = hit.object.map(|o| OBJ0 + 0x400 * o as u32).unwrap_or(0);
        assert_eq!(object_addr, case.output(OUTS + 4), "case {n} hit object");
    }
}

#[test]
fn face_height_query_matches_original() {
    let cases = parse(include_str!("data/t1846/world_faces_c38f0.golden"));
    assert!(cases.len() >= 40);
    let mut hits = 0;
    for (n, case) in cases.iter().enumerate() {
        let built = read_world(case);
        let (k, mask, radius) = (
            case.input(QARGS) as usize,
            case.input(QARGS + 4),
            case.input_f(QARGS + 8),
        );
        let faces = built.world.objects[k].faces.clone().unwrap_or_default();
        let (value, face) = face_height_query(
            &faces,
            mask,
            point_of(case),
            radius,
            built.world.ignore_flag,
        );
        let hit = HeightHit {
            value,
            face: face.map(|f| (k, f)),
            object: None,
        };
        check_hit(case, &built, n, &hit, false);
        hits += usize::from(face.is_some());
    }
    assert!(hits > 8, "hits {hits}");
}

#[test]
fn group_height_query_matches_original() {
    let cases = parse(include_str!("data/t1846/world_group_a46a0.golden"));
    assert!(cases.len() >= 40);
    let mut hits = 0;
    for (n, case) in cases.iter().enumerate() {
        let built = read_world(case);
        let hit = built
            .world
            .group_height_query(case.input(QARGS) as usize, point_of(case));
        check_hit(case, &built, n, &hit, true);
        hits += usize::from(hit.object.is_some());
    }
    assert!(hits > 8, "hits {hits}");
}

#[test]
fn id_height_query_matches_original() {
    let cases = parse(include_str!("data/t1846/world_id_c3b10.golden"));
    assert!(cases.len() >= 40);
    let (mut hits, mut group_wins, mut raised) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut built = read_world(case);
        let (id, skip, radius) = (
            case.input(QARGS) as i32,
            case.input(QARGS + 4) != 0,
            case.input_f(QARGS + 8),
        );
        let hit = built
            .world
            .id_height_query(id, point_of(case), radius, skip);
        check_hit(case, &built, n, &hit, true);
        assert!(
            same(built.world.best_value, case.output_f(G_BEST)),
            "case {n} global best"
        );
        assert_eq!(
            built.world.best_id as u32,
            case.output(G_BESTID),
            "case {n} global id"
        );
        hits += usize::from(hit.face.is_some());
        group_wins += usize::from(hit.object.is_some());
        raised += usize::from(case.output(G_BESTID) != 0);
    }
    assert!(
        hits > 5 && group_wins > 2 && raised > 1,
        "{hits} {group_wins} {raised}"
    );
}

#[test]
fn region_height_query_matches_original() {
    let cases = parse(include_str!("data/t1846/world_region_c3e60.golden"));
    assert!(cases.len() >= 40);
    let (mut hits, mut multi, mut fallback) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut built = read_world(case);
        let skip = case.input(QARGS) != 0;
        let (bias, half) = (case.input_f(QARGS + 4), case.input_f(QARGS + 8));
        let count = case.input(HITS) as usize;
        let ids: Vec<i32> = (0..count as u32)
            .map(|i| case.input(HITS + 4 + 4 * i) as i32)
            .collect();
        let point = point_of(case);
        let mut seen_box = None;
        let hit = built.world.region_height_query(
            point,
            bias,
            half,
            |b| {
                seen_box = Some(b);
                ids.clone()
            },
            skip,
        );
        let b = seen_box.expect("region query called");
        assert!(
            same(b[0], point[0] - half)
                && same(b[4], bias + point[1])
                && same(b[5], half + point[2])
        );
        check_hit(case, &built, n, &hit, true);
        assert!(
            same(built.world.best_value, case.output_f(G_BEST)),
            "case {n} global best"
        );
        assert_eq!(
            built.world.best_id as u32,
            case.output(G_BESTID),
            "case {n} global id"
        );
        hits += usize::from(hit.face.is_some());
        multi += usize::from(count > 1);
        fallback += usize::from(count == 0);
    }
    assert!(
        hits > 8 && multi > 8 && fallback > 3,
        "{hits} {multi} {fallback}"
    );
}

#[test]
fn name_lookup_semantics() {
    let table: [&[u8]; 3] = [b"x", b"alpha", b"beta"];
    assert_eq!(find_name_index(&table, b"beta"), Some(2));
    assert_eq!(find_name_index(&table, b"x"), None);
    assert_eq!(find_name_index(&table, b""), None);
}

const S_MOV: u32 = 0x150_0000;
const S_BODY: u32 = 0x151_0000;
const S_ARGS: u32 = 0x15A_0100;
const S_VEC: u32 = 0x15A_0000;
const S_HEAD: u32 = 0x158_0000;
const S_POOL: u32 = 0x159_0000;
const S_OTHER: u32 = 0x157_0000;
const S_PART: u32 = 0x156_0000;
const S_LOG: u32 = 0x15B_1000;
const Q_SHAPE: u32 = 0x15C_0000;
const Q_RECORD: u32 = 0x15C_4000;
const Q_DYNAMIC: u32 = 0x15C_8000;
const LOG_WORDS: usize = 400;

/// Replays the logging stubs of the generator: pops the scripted outputs and writes the same log words.
struct Scripted<'a> {
    case: &'a Case,
    cursors: [u32; 3],
    log: Vec<u32>,
}

impl Scripted<'_> {
    fn pop(&mut self, queue: usize) -> u32 {
        let base = [Q_SHAPE, Q_RECORD, Q_DYNAMIC][queue];
        let word = self.case.input(base + 4 * self.cursors[queue]);
        self.cursors[queue] += 1;
        word
    }
    fn popf(&mut self, queue: usize) -> f32 {
        f32::from_bits(self.pop(queue))
    }
    fn push_floats(&mut self, values: &[f32]) {
        self.log.extend(values.iter().map(|v| v.to_bits()));
    }
}

impl SweepServices for Scripted<'_> {
    fn shape_test(&mut self, item: &SweepItem, start: [f32; 3], radius: f32) -> ShapeTestResult {
        if item.kind == 0 {
            self.log
                .extend([0x4A670, item.b[0].to_bits(), radius.to_bits()]);
            self.push_floats(&item.a);
            self.push_floats(&item.b);
            self.push_floats(&start);
        } else {
            self.log.extend([
                if item.kind == 1 { 0x4FE00 } else { 0x52550 },
                radius.to_bits(),
            ]);
            self.push_floats(&start);
            self.push_floats(&item.a);
            self.push_floats(&item.b);
        }
        let position = [self.popf(0), self.popf(0), self.popf(0)];
        let normal = [self.popf(0), self.popf(0), self.popf(0)];
        let shape_id = self.pop(0);
        let code = self.pop(0);
        ShapeTestResult {
            code,
            position,
            normal,
            shape_id,
        }
    }

    fn record_test(&mut self, call: &RecordTestCall) -> u32 {
        self.log.extend([
            0x91EC0,
            call.mover_address,
            call.record_index as u32,
            call.bounds_address,
            call.detail_address,
        ]);
        self.log.extend(call.sweep.words());
        self.log
            .extend([call.arg5, call.other_address, call.list_address]);
        self.push_floats(&call.body_matrix);
        self.log.push(call.arg9);
        self.pop(1)
    }

    fn dynamic_query(
        &mut self,
        body_address: u32,
        other_address: u32,
        start: [f32; 3],
        delta: [f32; 3],
        radius: f32,
        list_address: u32,
        _list: &mut ImpactList,
    ) -> (i32, Vec<DynamicEntry>) {
        self.log.extend([0x128760, body_address, other_address]);
        self.push_floats(&start);
        self.push_floats(&delta);
        self.log.extend([radius.to_bits(), list_address]);
        let count = self.pop(2) as i32;
        let _second = self.pop(2);
        let mut entries = Vec::new();
        for _ in 0..3 {
            let w0 = self.pop(2);
            let mut vectors = [[0.0f32; 3]; 3];
            for vector in vectors.iter_mut() {
                for component in vector.iter_mut() {
                    *component = self.popf(2);
                }
            }
            let (_a, _b) = (self.pop(2), self.pop(2));
            let object = self.pop(2);
            entries.push(DynamicEntry {
                w0,
                vector_a: vectors[0],
                vector_b: vectors[1],
                vector_c: vectors[2],
                object,
            });
        }
        (count, entries)
    }

    fn dynamic_object_info(&mut self, address: u32) -> (f32, u16) {
        (
            self.case.input_f(address),
            self.case.input(address + 0x28) as u16,
        )
    }

    fn dynamic_apply(&mut self, entry: &DynamicEntry) {
        self.log.extend([0x129F50, entry.w0, entry.object]);
        self.push_floats(&entry.vector_a);
        self.push_floats(&entry.vector_c);
        self.push_floats(&entry.vector_b);
        self.log.push(1.0f32.to_bits());
    }
}

fn read_header(case: &Case) -> SweepHeader {
    let header = case.input(S_MOV + 4);
    let count = case.input(header);
    let records = (0..count)
        .map(|k| {
            let base = header - count * 0xC8 + k * 0xC8;
            let item_count = case.input(base + 0x58) as u16 as i16;
            let items_address = case.input(base + 0x8C);
            let items = (0..item_count.max(0) as u32)
                .map(|j| {
                    let a = items_address + 0x1C * j;
                    SweepItem {
                        kind: case.input(a),
                        a: case.vec3(a + 4),
                        b: case.vec3(a + 0x10),
                    }
                })
                .collect();
            let bounds_address = case.input(base + 0xA8);
            let bounds = (bounds_address != 0).then(|| {
                let f = |i: u32| case.input_f(bounds_address + 4 * i);
                SweepBounds {
                    address: bounds_address,
                    aabb: [f(0), f(1), f(2), f(3), f(4), f(5)],
                    detail_address: case.input(bounds_address + 0x24),
                }
            });
            SweepRecord {
                mask: case.input(base + 0x5C),
                matrix_index: (case.input(base + 0x48) >> 8) as u8 as i8,
                item_count,
                items,
                bounds,
            }
        })
        .collect();
    SweepHeader {
        records,
        h4: case.input(header + 4) as i32,
    }
}

#[test]
fn sweep_matches_original_with_scripted_heavy_callees() {
    let cases = parse(include_str!("data/t1846/world_sweep_92360.golden"));
    assert!(cases.len() >= 40);
    let (mut results, mut nodes, mut records_tested, mut dynamics, mut locals) = (0, 0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let link = case.input(S_BODY + 0x78);
        let parent_two = link != 0 && (case.input(case.input(link) + 0x30) >> 16) as u8 == 2;
        let mut body = SweepBody {
            address: S_BODY,
            object: object_from(case, S_BODY),
            box_offset: [
                case.input_f(S_BODY + 0x1A8),
                case.input_f(S_BODY + 0x1AC),
                case.input_f(S_BODY + 0x1B0),
            ],
            box_half: [
                case.input_f(S_BODY + 0x1C0),
                case.input_f(S_BODY + 0x1C4),
                case.input_f(S_BODY + 0x1C8),
            ],
            link_parent_kind_is_two: parent_two,
            field_80: case.input(S_BODY + 0x80),
        };
        let mover = SweepMover {
            address: S_MOV,
            mask: case.input(S_MOV + 0x30),
            part_matrices: (0..4)
                .map(|k| matrix_at(case, S_PART + 0x40 * k, false))
                .collect(),
        };
        let header = read_header(case);
        let other_address = case.input(S_ARGS + 12);
        let args = SweepArgs {
            start: case.vec3(S_VEC),
            delta: case.vec3(S_VEC + 0x10),
            radius: case.input_f(S_ARGS + 8),
            arg5: case.input(S_ARGS),
            other: (other_address != 0).then(|| ImpactOther {
                address: other_address,
                kind_20: case.input(S_OTHER + 0x20),
            }),
            list_address: S_HEAD,
            dynamic: case.input(S_ARGS + 16) != 0,
            arg9: case.input(S_ARGS + 4),
        };
        let mut list = ImpactList {
            first: case.input(S_HEAD),
            flag4: case.input(S_HEAD + 4) as u8,
        };
        let mut pool = ImpactPool {
            base_address: S_POOL,
            free_count: case.input(0x4E_826C) as i32,
            nodes: (0..6u32)
                .map(|k| {
                    let mut node = [0u32; 14];
                    for (w, slot) in node.iter_mut().enumerate() {
                        *slot = case.input(S_POOL + 0x38 * k + 4 * w as u32);
                    }
                    node
                })
                .collect(),
        };
        let mut services = Scripted {
            case,
            cursors: [0; 3],
            log: Vec::new(),
        };
        let result = sweep_body(
            &mut body,
            &mover,
            &header,
            &args,
            &mut list,
            &mut pool,
            &mut services,
        );
        assert_eq!(result, case.eax, "case {n} result");
        let mut log = services.log.clone();
        log.resize(LOG_WORDS, 0);
        for (w, word) in log.iter().enumerate() {
            assert_eq!(
                *word,
                case.output(S_LOG + 4 * w as u32),
                "case {n} log word {w}"
            );
        }
        for k in 0..6u32 {
            for w in 0..14u32 {
                assert_eq!(
                    pool.nodes[k as usize][w as usize],
                    case.output(S_POOL + 0x38 * k + 4 * w),
                    "case {n} node {k} word {w}"
                );
            }
        }
        assert_eq!(
            pool.free_count as u32,
            case.output(0x4E_826C),
            "case {n} pool count"
        );
        assert_eq!(list.first, case.output(S_HEAD), "case {n} list head");
        assert_eq!(
            body.object.flags,
            case.output(S_BODY + 0x28),
            "case {n} body flags"
        );
        let expected = matrix_at(case, S_BODY + 0x274, true);
        for i in 0..16 {
            assert!(
                same(body.object.matrix[i], expected[i]),
                "case {n} body matrix {i}"
            );
        }
        results += usize::from(result != 0);
        nodes += usize::from(list.first != case.input(S_HEAD));
        records_tested += usize::from(services.log.contains(&0x91EC0));
        dynamics += usize::from(services.log.contains(&0x129F50));
        locals += usize::from(
            header.h4 > 0 && body.object.flags & 8 != 0 && body.object.flags & 0x20_0000 != 0,
        );
    }
    assert!(
        results > 10 && nodes > 10 && records_tested > 5 && dynamics > 2 && locals > 3,
        "{results} {nodes} {records_tested} {dynamics} {locals}"
    );
}
