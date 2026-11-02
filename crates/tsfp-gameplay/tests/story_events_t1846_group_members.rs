//! T1846 batch 8: group member handlers (0x33FB50, 0x33B290), their callees and the sweep driver (0x88C60 ->
//! 0x88950) against golden vectors from the original under the Unicorn oracle
//! (`tools/t1846_gold/group_members.py`). The sweep geometry callbacks are replayed from the script table that is
//! part of every golden case.
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::group_members::*;
use tsfp_gameplay::story_events::prelude_1e2b80::SweepQuery;

const ROWS: u32 = 0x110_0000;
const POS: u32 = 0x111_0000;
const MEMBER: u32 = 0x112_0000;
const OBJ: u32 = 0x112_1000;
const WORDS: u32 = 0x113_0000;
const POSN: u32 = 0x114_0000;
const OBJS: u32 = 0x115_0000;
const SCRIPT: u32 = 0x117_0000;
const REG: u32 = 0x118_0000;
const BLOB: u32 = 0x11B_0000;
const RECS: u32 = 0x11D_0000;
const DESC: u32 = 0x11E_0000;
const FROM: u32 = 0x120_0000;
const VEC: u32 = 0x120_0100;
const OUTP: u32 = 0x120_0200;
const OUTD: u32 = 0x120_0300;
const OUTC: u32 = 0x120_0400;
const OUTH: u32 = 0x120_0500;
const OUT3384: u32 = 0x121_0000;
const STATIC_BASE: u32 = 0x52_4F58;
const ARGS: u32 = 0x130_0000;

fn load(name: &str) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/t1846/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let cases = parse(&text);
    assert!(cases.len() >= 50, "{name}: only {} cases", cases.len());
    cases
}

fn arg(case: &Case, index: u32) -> u32 {
    case.input(ARGS + 4 * index)
}

fn vec3(case: &Case, address: u32) -> [f32; 3] {
    case.vec3(address)
}

fn bits3(v: [f32; 3]) -> [u32; 3] {
    v.map(f32::to_bits)
}

fn blob(case: &Case, words: u32) -> Vec<u8> {
    (0..words)
        .flat_map(|i| case.input(BLOB + 4 * i).to_le_bytes())
        .collect()
}

/// Expected memory: the input value unless overwritten.
fn check_outs(case: &Case, expected: &std::collections::BTreeMap<u32, u32>) {
    assert!(!case.outs.is_empty());
    for (&address, &actual) in &case.outs {
        let want = expected
            .get(&address)
            .copied()
            .unwrap_or_else(|| case.input(address));
        assert_eq!(actual, want, "memory {address:#x}");
    }
}

// ---------------------------------------------------------------- 0x9C0E0
#[test]
fn nearest_row_matches_original() {
    let (mut found, mut none, mut radius_cases) = (0, 0, 0);
    for case in load("group_members_9c0e0") {
        let n = case.input(0x7B6664);
        let rows: Vec<CellRow> = (0..n)
            .map(|i| {
                let at = ROWS + 0x20 * i;
                CellRow {
                    flags: case.input(at + 4),
                    tag: (case.input(at + 8) >> 16) as i16,
                    position: vec3(&case, at + 0xC),
                }
            })
            .collect();
        let id = arg(&case, 1) as i32;
        let radius = f32::from_bits(arg(&case, 2));
        let got = nearest_row(&rows, vec3(&case, POS), id, radius);
        assert_eq!(got as u32, case.eax);
        if got >= 0 {
            found += 1;
        } else {
            none += 1;
        }
        radius_cases += usize::from(radius > 0.0);
    }
    assert!(
        found > 10 && none > 5 && radius_cases > 5,
        "{found} {none} {radius_cases}"
    );
}

// ---------------------------------------------------------------- 0x32A5B0, 0x2F7990
#[test]
fn find_chunk_matches_original() {
    let (mut hits, mut misses) = (0, 0);
    for case in load("group_members_32a5b0") {
        let got = find_chunk(&blob(&case, 0x100), arg(&case, 1));
        assert_eq!(got.map_or(0, |offset| BLOB + offset), case.eax);
        if got.is_some() {
            hits += 1;
        } else {
            misses += 1;
        }
    }
    assert!(hits > 10 && misses > 10, "{hits} {misses}");
}

fn statics(case: &Case, count: u32) -> Vec<StaticEntry> {
    (0..count)
        .map(|i| {
            let base = STATIC_BASE + 0xC4 * i;
            StaticEntry {
                key: case.input(base + 8),
                flags_b8: case.input(base + 0xB8),
                values: (0..6).map(|w| case.input(base + 0x10 + 0x1C * w)).collect(),
            }
        })
        .collect()
}

#[test]
fn static_value_matches_original() {
    let mut selected = 0;
    for case in load("group_members_2f7990") {
        let table = statics(&case, 4);
        let index = arg(&case, 0) as usize;
        let got = static_value(&table, &blob(&case, 0x100), index).expect("valid blob");
        assert_eq!(got, case.eax);
        selected += usize::from(table[index].flags_b8 & 4 != 0);
    }
    assert!(selected > 10);
}

// ---------------------------------------------------------------- 0x3384F0
fn registry(case: &Case) -> Vec<RegistryEntry> {
    (0..case.input(0x732E64))
        .map(|i| {
            let entry = REG + 0xD8 * i;
            let obj = case.input(entry);
            RegistryEntry {
                object: obj,
                flags_28: case.input(obj + 0x28),
                position: vec3(case, obj + 0x64),
                offset: vec3(case, obj + 0x1A8),
                match_c: case.input(entry + 0xC),
                match_14: case.input(entry + 0x14),
                record_flags: (case.input(case.input(entry + 0x18)) >> 8) as u8,
                word_38: case.input(entry + 0x38),
            }
        })
        .collect()
}

fn mode_is_3(case: &Case) -> bool {
    (case.input(0x7DE454) >> 8) & 0xFF == 3
}

#[test]
fn registry_matches_original() {
    let (mut with_matches, mut empty, mut mode3) = (0, 0, 0);
    for case in load("group_members_3384f0") {
        let entries = registry(&case);
        let found = registry_matches(
            &entries,
            mode_is_3(&case),
            arg(&case, 0),
            arg(&case, 2) != 0,
            arg(&case, 3),
        );
        assert_eq!(found.len() as u32, case.eax);
        let mut expected = std::collections::BTreeMap::new();
        if arg(&case, 1) != 0 {
            for (slot, &index) in found.iter().enumerate() {
                expected.insert(OUT3384 + 4 * slot as u32, REG + 0xD8 * index as u32);
            }
        }
        check_outs(&case, &expected);
        if found.is_empty() {
            empty += 1;
        } else {
            with_matches += 1;
        }
        mode3 += usize::from(mode_is_3(&case));
    }
    assert!(
        with_matches > 10 && empty > 3 && mode3 > 10,
        "{with_matches} {empty} {mode3}"
    );
}

// ---------------------------------------------------------------- 0x33B290
fn static_data(case: &Case) -> StaticData {
    StaticData {
        record_indices: (0..3).map(|i| case.input(RECS + 0x8C * i + 0x84)).collect(),
        statics: statics(case, 4),
        blob: blob(case, 0x100),
        registry: registry(case),
        mode_is_3: mode_is_3(case),
    }
}

fn apply_emitted(
    case: &Case,
    emitted: &[Emitted],
    acc: u32,
    expected: &mut std::collections::BTreeMap<u32, u32>,
) {
    for (k, item) in emitted.iter().enumerate() {
        let at = acc + k as u32;
        if arg(case, 1) != 0 {
            expected.insert(WORDS + 4 * at, item.word as u32);
        }
        if arg(case, 2) != 0 {
            for (axis, value) in item.position.iter().enumerate() {
                expected.insert(POSN + 12 * at + 4 * axis as u32, value.to_bits());
            }
        }
        if arg(case, 3) != 0 {
            expected.insert(OBJS + 4 * at, item.object);
        }
    }
}

#[test]
fn kind_8_9_matches_original() {
    let (mut some, mut zero, mut capped) = (0, 0, 0);
    for case in load("group_members_33b290") {
        let data = static_data(&case);
        let descriptor = case.input(DESC + 0xC) as u16 as i16;
        let emitted = member_kind_8_9(&data, descriptor).expect("valid world");
        let acc = arg(&case, 4);
        assert_eq!(acc + emitted.len() as u32, case.eax);
        let mut expected = std::collections::BTreeMap::new();
        apply_emitted(&case, &emitted, acc, &mut expected);
        check_outs(&case, &expected);
        match emitted.len() {
            0 => zero += 1,
            MAX_MATCHES => {
                capped += 1;
                some += 1;
            }
            _ => some += 1,
        }
    }
    assert!(
        some > 10 && zero > 3 && capped > 0,
        "{some} {zero} {capped}"
    );
}

// ---------------------------------------------------------------- scripted geometry
struct Script<'a> {
    case: &'a Case,
    index: u32,
    log: Vec<Vec<u32>>,
}

impl Script<'_> {
    fn record(&mut self) -> [u32; 8] {
        let base = SCRIPT + 32 * self.index;
        self.index += 1;
        std::array::from_fn(|i| self.case.input(base + 4 * i as u32))
    }
}

impl SweepGeometry for Script<'_> {
    fn cell_test(&mut self, probe: &CellProbe) -> CellHit {
        let record = self.record();
        let mut entry = vec![
            if probe.variant_a7a00 {
                0xA7A00
            } else {
                0xA3500
            },
            probe.cell,
            probe.flag,
        ];
        entry.extend(&probe.extra[..if probe.variant_a7a00 { 3 } else { 1 }]);
        entry.extend(bits3(probe.from));
        entry.extend(bits3(probe.vec));
        self.log.push(entry);
        let f = |i: usize| f32::from_bits(record[i]);
        CellHit {
            ret: record[0],
            point: [f(1), f(2), f(3)],
            aux: [f(4), f(5), f(6)],
            int_out: record[7],
        }
    }
    fn advance(&mut self, probe: &AdvanceProbe) -> AdvanceOut {
        let record = self.record();
        let mut entry = vec![0x88430, probe.cell, probe.previous, probe.flag];
        entry.extend(bits3(probe.from_copy));
        entry.extend(bits3(probe.vec_copy));
        self.log.push(entry);
        let f = |i: usize| f32::from_bits(record[i]);
        AdvanceOut {
            next: record[0],
            from_copy: [f(1), f(2), f(3)],
            vec_copy: [f(4), f(5), f(6)],
        }
    }
}

/// Golden `call` lines of the geometry stubs in a comparable shape.
fn golden_geometry_calls(case: &Case) -> Vec<Vec<u32>> {
    case.calls
        .iter()
        .map(|call| {
            let a = &call.args;
            let mut entry = vec![call.callee];
            match call.callee {
                0xA7A00 => {
                    entry.extend([a[0], a[3], a[6], a[7], a[8]]);
                    entry.extend(&a[9..15]);
                }
                0xA3500 => {
                    entry.extend([a[0], a[3], a[7]]);
                    entry.extend(&a[8..14]);
                }
                0x88430 => {
                    entry.extend([a[2], a[3], a[4]]);
                    entry.extend(&a[7..13]);
                }
                other => panic!("unexpected callee {other:#x}"),
            }
            entry
        })
        .collect()
}

fn sweep_io(case: &Case, with_cell: bool, with_hit: bool) -> SweepIo {
    SweepIo {
        position: Some(vec3(case, OUTP)),
        direction: Some(vec3(case, OUTD)),
        cell: with_cell.then(|| case.input(OUTC)),
        hit: with_hit.then(|| case.input(OUTH)),
        ..Default::default()
    }
}

fn check_sweep_outputs(case: &Case, io: &SweepIo, ret: u32) {
    assert_eq!(ret, case.eax, "return value");
    let mut expected = std::collections::BTreeMap::new();
    for (axis, value) in io.position.unwrap().iter().enumerate() {
        expected.insert(OUTP + 4 * axis as u32, value.to_bits());
    }
    for (axis, value) in io.direction.unwrap().iter().enumerate() {
        expected.insert(OUTD + 4 * axis as u32, value.to_bits());
    }
    if let Some(cell) = io.cell {
        expected.insert(OUTC, cell);
    }
    if let Some(hit) = io.hit {
        expected.insert(OUTH, hit);
    }
    check_outs(case, &expected);
}

#[derive(Default)]
struct SweepStats {
    hits: usize,
    misses: usize,
    early: usize,
    multi_cell: usize,
    cell_stores: usize,
    a3500: usize,
}

fn run_sweep_case(
    case: &Case,
    driver: impl FnOnce(&mut Script, &mut SweepIo) -> u32,
    with_cell: bool,
    with_hit: bool,
    stats: &mut SweepStats,
) {
    let mut script = Script {
        case,
        index: 0,
        log: Vec::new(),
    };
    let mut io = sweep_io(case, with_cell, with_hit);
    let ret = driver(&mut script, &mut io);
    check_sweep_outputs(case, &io, ret);
    assert_eq!(script.log, golden_geometry_calls(case), "geometry calls");
    if ret != 0 {
        stats.hits += 1;
    } else if script.log.is_empty() {
        stats.early += 1;
    } else {
        stats.misses += 1;
    }
    stats.multi_cell += usize::from(script.log.len() > 2);
    stats.cell_stores += usize::from(with_cell);
    stats.a3500 += script.log.iter().filter(|e| e[0] == 0xA3500).count().min(1);
}

#[test]
fn sweep_88c60_matches_original() {
    let mut stats = SweepStats::default();
    for case in load("group_members_88c60") {
        let with_cell = arg(&case, 6) != 0;
        let (from, vec) = (vec3(&case, FROM), vec3(&case, VEC));
        run_sweep_case(
            &case,
            |script, io| {
                sweep_88c60(
                    script,
                    arg(&case, 0),
                    from,
                    vec,
                    arg(&case, 3),
                    [arg(&case, 7), arg(&case, 8), arg(&case, 9)],
                    io,
                )
                .expect("no null store")
            },
            with_cell,
            false,
            &mut stats,
        );
    }
    assert!(stats.hits > 10 && stats.misses > 3 && stats.early > 2 && stats.multi_cell > 10);
    assert!(stats.cell_stores > 10);
}

#[test]
fn sweep_88950_matches_original() {
    let mut stats = SweepStats::default();
    for case in load("group_members_88950") {
        let use_a7a00 = arg(&case, 0) != 0;
        let with_cell = arg(&case, 4) != 0;
        let with_hit = arg(&case, 5) != 0;
        let args = SweepArgs {
            cell: arg(&case, 9),
            from: vec3(&case, FROM),
            vec: vec3(&case, VEC),
            use_a7a00,
            flag: arg(&case, 1),
            extra: [arg(&case, 6), arg(&case, 7), arg(&case, 8)],
        };
        run_sweep_case(
            &case,
            |script, io| sweep_88950(script, &args, io).expect("no null store"),
            with_cell,
            with_hit,
            &mut stats,
        );
    }
    assert!(stats.hits > 10 && stats.misses > 3 && stats.early > 2 && stats.multi_cell > 10);
    assert!(stats.a3500 > 5, "the 0xA3500 variant must be covered");
}

#[test]
fn prelude_sweep_wrapper_matches_driver() {
    // `prelude_sweep` is the 0x88C60 call of the level 4 prelude: world, from, (0,1,0) up, flag 1, extras (-3,0,0).
    let case = &load("group_members_88c60")[0];
    let mut script = Script {
        case,
        index: 0,
        log: Vec::new(),
    };
    let query = SweepQuery {
        world: arg(case, 0),
        from: vec3(case, FROM),
        up: vec3(case, VEC),
        reach: vec3(case, OUTP),
    };
    let result = prelude_sweep(&mut script, &query, [0.0; 4]);
    let mut again = Script {
        case,
        index: 0,
        log: Vec::new(),
    };
    let mut io = SweepIo {
        position: Some(query.reach),
        direction: Some([0.0; 3]),
        cell: None,
        hit: None,
        ..Default::default()
    };
    sweep_88c60(
        &mut again,
        query.world,
        query.from,
        query.up,
        1,
        [-3i32 as u32, 0, 0],
        &mut io,
    )
    .unwrap();
    assert_eq!(bits3(result.reach), bits3(io.position.unwrap()));
    assert_eq!(
        bits3([result.hit[0], result.hit[1], result.hit[2]]),
        bits3(io.direction.unwrap())
    );
}

// ---------------------------------------------------------------- 0x33FB50
struct SweepScriptEnv<'a> {
    case: &'a Case,
    rows: Vec<CellRow>,
    flag: bool,
    calls: Vec<Vec<u32>>,
}

impl CellEnv for SweepScriptEnv<'_> {
    fn flag_74c3b0(&self) -> bool {
        self.flag
    }
    fn sweep(
        &mut self,
        world: u32,
        from: [f32; 3],
        up: [f32; 3],
        _slot: u32,
        io: &mut SweepIo,
    ) -> Result<(), MemberError> {
        let word = |i: u32| self.case.input(SCRIPT + 4 * i);
        let f = |i: u32| f32::from_bits(word(i));
        let mut entry = vec![world];
        entry.extend(bits3(from));
        entry.extend(bits3(up));
        self.calls.push(entry);
        if word(0) == 1 {
            io.position = Some([f(1), f(2), f(3)]);
            io.direction = Some([f(4), f(5), f(6)]);
        }
        if word(0) == 1 || word(0) == 2 {
            io.cell = Some(word(7));
        }
        Ok(())
    }
    fn rows(&self) -> &[CellRow] {
        &self.rows
    }
}

#[test]
fn kind_0_7_matches_original() {
    let (mut swept, mut plain, mut computed, mut cached, mut flagged, mut early_slot) =
        (0, 0, 0, 0, 0, 0);
    for case in load("group_members_33fb50") {
        let n = case.input(0x7B6664);
        let rows: Vec<CellRow> = (0..n)
            .map(|i| {
                let at = ROWS + 0x20 * i;
                CellRow {
                    flags: case.input(at + 4),
                    tag: (case.input(at + 8) >> 16) as i16,
                    position: vec3(&case, at + 0xC),
                }
            })
            .collect();
        let pointer = case.input(0x74C3B0);
        let flag = pointer != 0 && case.input(pointer) != 0;
        let mut object = CellObject {
            flags_28: case.input(OBJ + 0x28),
            world: case.input(OBJ + 0x34),
            position: vec3(&case, OBJ + 0x64),
            animation: case.input(OBJ + 0x94) as u16,
            nearest_cache: case.input(OBJ + 0x110) as i32,
            offset: vec3(&case, OBJ + 0x1A8),
        };
        let (before_cache, before_flags) = (object.nearest_cache, object.flags_28);
        let mut env = SweepScriptEnv {
            case: &case,
            rows,
            flag,
            calls: Vec::new(),
        };
        let emitted = member_kind_0_7(&mut env, &mut object, OBJ, MEMBER, [0.0; 3]).expect("ok");
        let acc = arg(&case, 4);
        assert_eq!(acc + 1, case.eax);
        let mut expected = std::collections::BTreeMap::new();
        expected.insert(OBJ + 0x28, object.flags_28);
        expected.insert(OBJ + 0x110, object.nearest_cache as u32);
        apply_emitted(&case, &[emitted], acc, &mut expected);
        check_outs(&case, &expected);
        // The sweep stub was called exactly when the original called it, with the same arguments.
        let golden: Vec<Vec<u32>> = case
            .calls
            .iter()
            .map(|c| {
                assert_eq!(c.callee, 0x88C60);
                assert_eq!(c.args[3], 1);
                assert_eq!(&c.args[7..10], &[0, 0, 0]);
                let mut entry = vec![c.args[0]];
                entry.extend(&c.args[10..16]);
                entry
            })
            .collect();
        assert_eq!(env.calls, golden, "sweep calls");
        if env.calls.is_empty() {
            plain += 1;
        } else {
            swept += 1;
            early_slot += usize::from(case.input(SCRIPT) == 0);
        }
        if before_cache < 0 {
            computed += 1;
        } else {
            cached += 1;
        }
        flagged += usize::from(object.flags_28 != before_flags);
    }
    assert!(
        swept > 10 && plain > 5 && computed > 10 && cached > 5 && flagged > 3 && early_slot > 0
    );
}

#[test]
fn deep_members_dispatch_pushes_words_and_syncs_flags() {
    struct Env;
    impl CellEnv for Env {
        fn flag_74c3b0(&self) -> bool {
            true
        }
        fn sweep(
            &mut self,
            _: u32,
            _: [f32; 3],
            _: [f32; 3],
            _: u32,
            _: &mut SweepIo,
        ) -> Result<(), MemberError> {
            Ok(())
        }
        fn rows(&self) -> &[CellRow] {
            static ROWS: [CellRow; 1] = [CellRow {
                flags: 1,
                tag: 2,
                position: [0.0; 3],
            }];
            &ROWS
        }
    }
    let mut env = Env;
    let object = CellObject {
        flags_28: 0,
        world: 2,
        position: [1.0; 3],
        animation: 0,
        nearest_cache: -1,
        offset: [0.0; 3],
    };
    let mut members = DeepMembers {
        env: &mut env,
        objects: [(5, object)].into(),
        descriptor_words: Default::default(),
        data: StaticData::default(),
        member_slot: 0,
        stack_garbage: [0.0; 3],
    };
    let (mut flags, mut out) = (0u32, Vec::new());
    members.dispatch(0, 5, &mut flags, &mut out).unwrap();
    assert_eq!((flags, out.as_slice()), (0x10, &[0][..]));
    members.dispatch(3, 5, &mut flags, &mut out).unwrap();
    assert_eq!(out.len(), 1, "other kinds add nothing");
    assert!(members.dispatch(8, 5, &mut flags, &mut out).is_err());
}
