//! T1846 batch 8: record test 0x91EC0, dynamic query 0x128760 and apply step 0x129F50 against emulated-original
//! golden vectors (MEASURED). The heavy callees (0x4B310, 0x4B3B0, 0x129C20, 0x129CA0) are logged stubs whose
//! answers are replayed from the golden input memory.
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::sweep_records::*;
use tsfp_gameplay::story_events::world_queries::*;

const POOL_NODES: usize = 6;
const G_POOL_COUNT: u32 = 0x4E826C;
const G_POOL_BASE: u32 = 0x74C468;

struct CaseMemory<'a>(&'a BTreeMap<u32, u32>);

impl GuestMemory for CaseMemory<'_> {
    fn read_u8(&self, address: u32) -> u8 {
        let word = self.0.get(&(address & !3)).copied().unwrap_or(0);
        (word >> ((address & 3) * 8)) as u8
    }
}

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn words(case: &Case, address: u32, count: u32) -> Vec<u32> {
    (0..count).map(|i| case.input(address + 4 * i)).collect()
}

fn load_pool(case: &Case, base: u32) -> ImpactPool {
    let nodes = (0..POOL_NODES as u32)
        .map(|n| {
            let mut node = [0u32; 14];
            for (i, w) in node.iter_mut().enumerate() {
                *w = case.input(base + 0x38 * n + 4 * i as u32);
            }
            node
        })
        .collect();
    ImpactPool {
        base_address: base,
        free_count: case.input(G_POOL_COUNT) as i32,
        nodes,
    }
}

fn check_pool(
    case: &Case,
    pool: &ImpactPool,
    list: &ImpactList,
    base: u32,
    list_address: u32,
    tag: &str,
) {
    assert_eq!(
        pool.free_count as u32,
        case.output(G_POOL_COUNT),
        "{tag} pool count"
    );
    for (n, node) in pool.nodes.iter().enumerate() {
        for (i, w) in node.iter().enumerate() {
            let a = base + 0x38 * n as u32 + 4 * i as u32;
            assert_eq!(*w, case.output(a), "{tag} pool node {n} word {i}");
        }
    }
    assert_eq!(list.first, case.output(list_address), "{tag} list head");
}

// ---- 0x91EC0 ------------------------------------------------------------------------------------------------

const MOV: u32 = 0x150_0000;
const BOUNDS: u32 = 0x154_0000;
const SWB: u32 = 0x157_0000;
const LISTH: u32 = 0x159_0000;
const MATB: u32 = 0x15A_0000;
const QUEUE: u32 = 0x15C_0000;
const POOL: u32 = 0x15D_0000;

struct ReplayTriangles<'a> {
    case: &'a Case,
    logged: Vec<&'a common::Call>,
    cursor: usize,
    hits: usize,
}

impl TriangleTest for ReplayTriangles<'_> {
    fn test(&mut self, call: &TriangleTestCall) -> TriangleTestResult {
        let n = self.cursor;
        self.cursor += 1;
        let log = self
            .logged
            .get(n)
            .unwrap_or_else(|| panic!("call {n} not logged"));
        assert_eq!(log.callee, 0x4B310);
        let a = &log.args;
        assert_eq!(a.len(), 13 + 14);
        assert_eq!(a[0], SWB);
        assert_eq!(a[1], call.sub_record_address, "sub record");
        assert_eq!(a[2], call.bounds_address);
        assert_eq!(a[3], call.header_plus_0x38);
        assert_eq!(a[4], call.quantised);
        assert_eq!(a[5], call.table_address);
        assert_eq!(a[6], call.entry_address);
        assert_eq!(a[7], call.flag);
        assert_eq!(a[12], u32::from(call.list_flag5));
        assert_eq!(&a[13..27], &call.sweep.words(), "sweep box");
        let e = words(self.case, QUEUE + 36 * n as u32, 9);
        if e[0] != 0 && f32::from_bits(e[8]) < 1.0 {
            self.hits += 1;
        }
        TriangleTestResult {
            code: e[0],
            position: [
                f32::from_bits(e[1]),
                f32::from_bits(e[2]),
                f32::from_bits(e[3]),
            ],
            normal: [
                f32::from_bits(e[4]),
                f32::from_bits(e[5]),
                f32::from_bits(e[6]),
            ],
            f18_bits: e[7],
            fraction: f32::from_bits(e[8]),
        }
    }
}

fn sweep_from(case: &Case, address: u32) -> SweepBox {
    let w = |i: u32| case.input_f(address + 4 * i);
    SweepBox {
        stationary: case.input(address),
        start: [w(1), w(2), w(3)],
        delta: [w(4), w(5), w(6)],
        radius: w(7),
        min: [w(8), w(9), w(10)],
        max: [w(11), w(12), w(13)],
    }
}

#[test]
fn record_test_matches_original() {
    let cases = parse(include_str!("data/t1846/sweep_records_91ec0.golden"));
    assert!(cases.len() >= 50);
    let (mut nonzero, mut allocated, mut calls_total, mut sorted, mut empty, mut transformed) =
        (0, 0, 0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let memory = CaseMemory(&case.ins);
        let args = words(case, LISTH + 0x100, 4);
        let mut pool = load_pool(case, POOL);
        let free_before = pool.free_count;
        let mut list = ImpactList {
            first: case.input(LISTH),
            flag4: case.input(LISTH + 4) as u8,
        };
        let mut triangles = ReplayTriangles {
            case,
            logged: case.calls.iter().collect(),
            cursor: 0,
            hits: 0,
        };
        let had_chain = list.first != 0;
        let detail = case.input(BOUNDS + 0x24);
        let call = RecordTestCall {
            mover_address: MOV,
            record_index: args[0] as i32,
            bounds_address: BOUNDS,
            detail_address: detail,
            sweep: sweep_from(case, SWB),
            arg5: args[1],
            other_address: args[2],
            list_address: LISTH,
            body_matrix: [0.0; 16],
            arg9: args[3],
        };
        let mut matrix = [0.0f32; 16];
        for (i, v) in matrix.iter_mut().enumerate() {
            *v = case.input_f(MATB + 4 * i as u32);
        }
        let call = RecordTestCall {
            body_matrix: matrix,
            ..call
        };
        let list_flag5 = (case.input(LISTH + 4) >> 8) as u8;
        let result = {
            let mut env = RecordTestEnv {
                memory: &memory,
                pool: &mut pool,
                list: &mut list,
                list_flag5,
                triangles: &mut triangles,
            };
            env.record_test(&call)
        };
        assert_eq!(
            triangles.cursor,
            case.calls.len(),
            "case {n} stub call count"
        );
        assert_eq!(result, case.eax, "case {n} result");
        check_pool(case, &pool, &list, POOL, LISTH, &format!("case {n}"));
        calls_total += case.calls.len();
        if result != 0 {
            nonzero += 1;
        }
        if pool.free_count < free_before {
            allocated += 1;
            if had_chain {
                sorted += 1;
            }
        }
        if case.calls.is_empty() {
            empty += 1;
        }
        let body_flags = case.input(0x151_0000 + 0x28);
        if body_flags & 8 != 0 && body_flags & 0x20_0000 != 0 && triangles.hits > 0 {
            transformed += 1;
        }
    }
    eprintln!(
        "91EC0: nonzero {nonzero} allocated {allocated} calls {calls_total} sorted {sorted} empty {empty} transformed {transformed}"
    );
    assert!(nonzero >= 20, "nonzero results {nonzero}");
    assert!(allocated >= 20, "pool allocations {allocated}");
    assert!(sorted >= 5, "sorted inserts into a chain {sorted}");
    assert!(empty >= 2, "cases without any triangle test {empty}");
    assert!(calls_total >= 150, "triangle tests {calls_total}");
    assert!(transformed >= 5, "locally transformed hits {transformed}");
}

// ---- 0x128760 -----------------------------------------------------------------------------------------------

const DYN_BODY: u32 = 0x160_0000;
const DYN_VEC: u32 = 0x163_0000;
const DYN_OUT: u32 = 0x164_0000;
const DYN_QUEUE: u32 = 0x165_0000;
const DYN_POOL: u32 = 0x166_0000;
const DYN_LIST: u32 = 0x167_0000;

struct ReplayShapes<'a> {
    case: &'a Case,
    cursor: usize,
}

impl DynamicShapeTest for ReplayShapes<'_> {
    fn test(&mut self, call: &DynamicShapeCall, out: &mut DynamicShapeResult) {
        let n = self.cursor;
        self.cursor += 1;
        let log = self
            .case
            .calls
            .get(n)
            .unwrap_or_else(|| panic!("call {n} not logged"));
        assert_eq!(log.callee, 0x4B3B0);
        let a = &log.args;
        assert_eq!(a.len(), 10 + 6);
        assert_eq!(a[0], DYN_VEC);
        assert_eq!(a[1], DYN_VEC + 0x10);
        assert_eq!(a[2], call.radius.to_bits());
        assert_eq!(a[3], call.object);
        for k in 0..3 {
            assert_eq!(a[10 + k], call.start[k].to_bits());
            assert_eq!(a[13 + k], call.delta[k].to_bits());
        }
        let e = words(self.case, DYN_QUEUE + 40 * n as u32, 10);
        out.code = e[0];
        if e[0] != 0 {
            out.position = [
                f32::from_bits(e[1]),
                f32::from_bits(e[2]),
                f32::from_bits(e[3]),
            ];
            out.normal = [
                f32::from_bits(e[4]),
                f32::from_bits(e[5]),
                f32::from_bits(e[6]),
            ];
            if e[9] != 0 {
                out.f18_bits = e[7];
                out.fraction = f32::from_bits(e[8]);
            }
        }
    }
}

#[test]
fn dynamic_query_matches_original() {
    let cases = parse(include_str!("data/t1846/sweep_dynamic_128760.golden"));
    assert!(cases.len() >= 50);
    let (mut entries_total, mut capped, mut nodes, mut skipped_float, mut no_array) =
        (0, 0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let memory = CaseMemory(&case.ins);
        let mut pool = load_pool(case, DYN_POOL);
        assert_eq!(case.input(G_POOL_BASE), DYN_POOL);
        let free_before = pool.free_count;
        let mut list = ImpactList {
            first: case.input(DYN_LIST),
            flag4: 0,
        };
        let extra = words(case, DYN_LIST + 0x100, 2);
        let initial = case.input(DYN_OUT) as i32;
        let mut result = DynamicResult {
            count: initial,
            entries: Vec::new(),
        };
        let mut shapes = ReplayShapes { case, cursor: 0 };
        dynamic_query(
            &memory,
            &mut shapes,
            &mut pool,
            &mut list,
            DYN_BODY,
            extra[0],
            case.vec3(DYN_VEC),
            case.vec3(DYN_VEC + 0x10),
            f32::from_bits(extra[1]),
            &mut result,
        );
        assert_eq!(shapes.cursor, case.calls.len(), "case {n} stub call count");
        assert_eq!(case.eax, 0);
        check_pool(case, &pool, &list, DYN_POOL, DYN_LIST, &format!("case {n}"));
        assert_eq!(result.count as u32, case.output(DYN_OUT), "case {n} count");
        for (j, entry) in result.entries.iter().enumerate() {
            let base = DYN_OUT + 8 + (initial as u32 + j as u32) * 0x34;
            for (w, value) in entry.iter().enumerate() {
                assert_eq!(
                    *value,
                    case.output(base + 4 * w as u32),
                    "case {n} entry {j} word {w}"
                );
            }
        }
        // untouched entries keep their input
        for slot in 0..5u32 {
            let appended =
                slot >= initial as u32 && slot < initial as u32 + result.entries.len() as u32;
            if !appended {
                for w in 0..13u32 {
                    let a = DYN_OUT + 8 + slot * 0x34 + 4 * w;
                    assert_eq!(
                        case.input(a),
                        case.output(a),
                        "case {n} untouched entry {slot}"
                    );
                }
            }
        }
        entries_total += result.entries.len();
        if initial >= 5 && case.calls.iter().len() > 0 {
            capped += 1;
        }
        if pool.free_count < free_before {
            nodes += 1;
        }
        if case.input(DYN_BODY + 0x1F8) == 0 {
            no_array += 1;
        }
        skipped_float += case
            .input(DYN_BODY + 0x1C)
            .saturating_sub(case.calls.len() as u32) as usize;
    }
    eprintln!(
        "128760: entries {entries_total} capped {capped} nodes {nodes} skipped {skipped_float} no_array {no_array}"
    );
    assert!(entries_total >= 40);
    assert!(capped >= 3, "result buffer full cases {capped}");
    assert!(nodes >= 20);
    assert!(
        skipped_float >= 10,
        "objects skipped by the float test {skipped_float}"
    );
    assert!(no_array >= 1);
}

// ---- 0x129F50 -----------------------------------------------------------------------------------------------

const APPLY_OBJ: u32 = 0x170_0000;
const APPLY_A: u32 = 0x171_0000;
const APPLY_C: u32 = 0x172_0000;
const APPLY_B: u32 = 0x173_0000;
const APPLY_ARGS: u32 = 0x174_0000;

#[derive(Default)]
struct RecordedEffects {
    ambient: Vec<[f32; 3]>,
    contact: Vec<ContactEffectCall>,
}

impl ImpactEffects for RecordedEffects {
    fn spawn_ambient(&mut self, position: [f32; 3]) {
        self.ambient.push(position);
    }
    fn spawn_contact(&mut self, call: &ContactEffectCall) {
        self.contact.push(*call);
    }
}

#[test]
fn apply_step_matches_original() {
    let cases = parse(include_str!("data/t1846/sweep_apply_129f50.golden"));
    assert!(cases.len() >= 50);
    let (mut reflected, mut plain, mut nan_cases) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let memory = CaseMemory(&case.ins);
        let geometry = DynamicObjectGeometry::read(&memory, APPLY_OBJ);
        let (w0, weight) = (case.input(APPLY_ARGS), case.input_f(APPLY_ARGS + 4));
        let mut effects = RecordedEffects::default();
        apply_dynamic_contact(
            w0,
            APPLY_OBJ,
            &geometry,
            case.vec3(APPLY_A),
            case.vec3(APPLY_C),
            case.vec3(APPLY_B),
            weight,
            &mut effects,
        );
        assert_eq!(case.calls.len(), 2, "case {n}");
        let (ambient, contact) = (&case.calls[0], &case.calls[1]);
        assert_eq!(ambient.callee, 0x129C20);
        assert_eq!(effects.ambient.len(), 1);
        assert_eq!(ambient.args[0], APPLY_A);
        for k in 0..3 {
            assert_eq!(
                ambient.args[1 + k],
                effects.ambient[0][k].to_bits(),
                "case {n} ambient"
            );
        }
        assert_eq!(contact.callee, 0x129CA0);
        assert_eq!(contact.regs[4], APPLY_OBJ, "edi");
        let got = effects.contact[0];
        assert_eq!((got.w0, got.object), (w0, APPLY_OBJ));
        let a = &contact.args;
        assert_eq!(a.len(), 4 + 2 + 3 + 3);
        assert_eq!(a[3], APPLY_A);
        assert!(
            same(f32::from_bits(a[2]), got.weight),
            "case {n} weight {} vs {}",
            f32::from_bits(a[2]),
            got.weight
        );
        for k in 0..2 {
            assert!(
                same(f32::from_bits(a[4 + k]), got.edge_coordinates[k]),
                "case {n} edge {k}"
            );
        }
        for k in 0..3 {
            assert!(
                same(f32::from_bits(a[6 + k]), got.direction[k]),
                "case {n} direction {k}"
            );
            assert_eq!(a[9 + k], got.position[k].to_bits(), "case {n} position {k}");
        }
        if geometry.flags_byte_29 & 1 != 0 {
            reflected += 1;
        } else {
            plain += 1;
        }
        if got.direction.iter().any(|v| v.is_nan()) {
            nan_cases += 1;
        }
    }
    eprintln!("129F50: reflected {reflected} plain {plain} nan {nan_cases}");
    assert!(reflected >= 15 && plain >= 15);
}

#[test]
fn insert_sorted_orders_by_key_after_equal_keys() {
    let node = |key: f32, next: u32| {
        let mut n = [0u32; 14];
        n[7] = key.to_bits();
        n[13] = next;
        n
    };
    let mut pool = ImpactPool {
        base_address: 0x1000,
        free_count: 0,
        nodes: vec![node(0.5, 0), node(0.5, 0), node(0.25, 0x1000)],
    };
    let mut list = ImpactList {
        first: 0x1000 + 0x70,
        flag4: 0,
    };
    insert_sorted(&mut list, &mut pool, 1);
    // 0.25 -> 0.5 (slot 0) -> new equal key 0.5 (slot 1)
    assert_eq!(pool.nodes[2][13], 0x1000);
    assert_eq!(pool.nodes[0][13], 0x1038);
    assert_eq!(pool.nodes[1][13], 0);
    assert_eq!(list.first, 0x1070);
}
