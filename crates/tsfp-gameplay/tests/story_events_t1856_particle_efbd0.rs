//! T1856: the effect entry spawner `0xEFBD0` and its callees against emulated-original golden vectors (MEASURED),
//! plus semantic unit tests. Golden: `tests/data/t1856/particle_efbd0_*.golden` (`tools/t1846_gold/svc_particle_efbd0.py`).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::callees_t1846::ThrottledSound;
use tsfp_gameplay::story_events::group_members::{AdvanceOut, AdvanceProbe};
use tsfp_gameplay::story_events::prelude_1e2b80::PreludeEffect;
use tsfp_gameplay::story_events::svc_particle_efbd0::*;
use tsfp_gameplay::story_events::sweep_cells::{CellGraph, Portal, advance_88430};
use tsfp_gameplay::story_events::world_queries::{CollisionFace, CollisionObject, CollisionWorld};

const POOL: u32 = 0x110_0000;
const SLOTS: u32 = 6;
const SLOT_BYTES: u32 = 0x22C;
const PL: u32 = 0x120_0000;
const PL_STRIDE: u32 = 0x1584;
const STATS: u32 = 0x122_0000;
const SRC: u32 = 0x130_0000;
const STATB: u32 = 0x130_1000;
const INP: u32 = 0x131_0000;
const STUBIN: u32 = 0x132_0000;
const ARGS: u32 = 0x133_0000;
const OBJ0: u32 = 0x150_0000;
const IDT: u32 = 0x153_0000;
const NOBJ: u32 = 0x154_0000;
const CLASS: usize = 2;
const KINDS: usize = 52;

#[derive(Default)]
struct Recorder {
    log: Vec<(u32, Vec<u32>)>,
    ids: Vec<i32>,
    walk: u32,
    flag: bool,
    low: u32,
    high: u32,
    calls: u32,
}

impl EmberEngine for Recorder {
    fn rng(&mut self) -> u32 {
        self.calls = self.calls.wrapping_add(1);
        let state = (u64::from(self.high) << 32) | u64::from(self.low);
        let next = state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
        self.low = next as u32;
        self.high = (next >> 32) as u32;
        next as u32
    }
    fn submit_sound(&mut self, request: [u32; 12]) {
        self.log.push((0x45820, request.to_vec()));
    }
    fn throttled_sound(&mut self, sound: ThrottledSound) {
        let b = f32::to_bits;
        self.log.push((
            0x46A00,
            vec![
                sound.sound_id,
                b(sound.arg_a),
                b(sound.arg_b),
                b(sound.arg_c),
            ],
        ));
    }
    fn region_query(&mut self, region: [f32; 6]) -> Vec<i32> {
        let mut words = vec![0];
        words.extend(region.iter().map(|v| v.to_bits()));
        self.log.push((0x87590, words));
        self.ids.clone()
    }
    fn advance_cell(&mut self, _probe: &AdvanceProbe) -> AdvanceOut {
        panic!("0x888A0 is stubbed in the root golden")
    }
    fn walk_cells(
        &mut self,
        _count: i32,
        from: [f32; 3],
        cell: u32,
        vec: [f32; 3],
        flag: u32,
    ) -> u32 {
        let mut words = vec![cell, flag];
        words.extend(from.iter().chain(&vec).map(|v| v.to_bits()));
        self.log.push((0x888A0, words));
        self.walk
    }
    fn flag_3533f0(&mut self) -> bool {
        self.log.push((0x3533F0, vec![]));
        self.flag
    }
    fn spawn_emitter(&mut self, template: u32, position: [f32; 3], direction: [f32; 3]) {
        let mut words = vec![0, template, 0];
        words.extend(position.iter().chain(&direction).map(|v| v.to_bits()));
        self.log.push((0x111AE0, words));
    }
    fn set_scale(&mut self, scale: f32) {
        self.log.push((0x10E660, vec![scale.to_bits()]));
    }
    fn nudge_player(&mut self, player: usize, f: f32, a: f32, b: f32) {
        let words = vec![
            PL + player as u32 * PL_STRIDE,
            f.to_bits(),
            a.to_bits(),
            b.to_bits(),
        ];
        self.log.push((0xD1EC0, words));
    }
    fn trigger_effect(&mut self, id: i32, position: [f32; 3]) {
        let mut words = vec![0, id as u32];
        words.extend(position.iter().map(|v| v.to_bits()));
        self.log.push((0xFF8A0, words));
    }
    fn settle(&mut self, position: [f32; 3]) {
        let mut words = vec![0];
        words.extend(position.iter().map(|v| v.to_bits()));
        self.log.push((0xEDBE0, words));
    }
    fn light(&mut self, position: [f32; 3], radius_squared: f32) {
        let mut words = vec![radius_squared.to_bits()];
        words.extend(position.iter().map(|v| v.to_bits()));
        self.log.push((0x186480, words));
    }
    fn teardown_slot(&mut self, _slot: usize, _entry: &mut EffectEntry) {}
    fn start_kind(&mut self, _world: &mut EmberWorld, _slot: usize, kind: u32) -> u32 {
        panic!("kind {kind} handler is an engine service and must not be reached")
    }
}

/// The logged words of a golden call in the layout of [`Recorder`] (pointer arguments dropped).
fn normalise(call: &common::Call) -> (u32, Vec<u32>) {
    let a = &call.args;
    let words = match call.callee {
        0x45820 => a[1..13].to_vec(),
        0x87590 => [&a[..1], &a[4..10]].concat(),
        0x888A0 => [&a[1..2], &a[3..4], &a[4..7], &a[7..10]].concat(),
        0x111AE0 => [&a[..3], &a[5..8], &a[8..11]].concat(),
        0x10E660 => a[..1].to_vec(),
        0xD1EC0 => a[..4].to_vec(),
        0xFF8A0 => [&a[..2], &a[3..6]].concat(),
        0xEDBE0 => [&a[1..2], &a[2..5]].concat(),
        0x186480 => [&a[1..2], &a[2..5]].concat(),
        0x46A00 => a[..4].to_vec(),
        0x3533F0 => vec![],
        other => panic!("unexpected stub {other:#x}"),
    };
    (call.callee, words)
}

fn face_of(case: &Case, a: u32) -> CollisionFace {
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

fn collision_of(case: &Case) -> CollisionWorld {
    let objects = (0..case.input(NOBJ))
        .map(|k| {
            let base = OBJ0 + 0x400 * k;
            let (count, list) = (case.input(base + 0x1E0), case.input(base + 0x1F0));
            let faces = (list != 0).then(|| {
                (0..count)
                    .map(|i| face_of(case, case.input(list + 4 * i)))
                    .collect()
            });
            CollisionObject {
                faces,
                class_mask: case.input(case.input(base + 4) + 0x30),
                shape_bits: 0,
                state: 0,
            }
        })
        .collect();
    let limit = case.input(0x7B_975C) as i32;
    let id_table = (0..=limit.max(0) as u32)
        .map(|id| {
            if id == 0 {
                0
            } else {
                ((case.input(IDT + 4 * id) - OBJ0) / 0x400) as usize
            }
        })
        .collect();
    CollisionWorld {
        objects,
        id_table,
        id_limit: limit,
        groups: vec![],
        all_objects: vec![],
        ignore_flag: case.input(0x73_3084) != 0,
        best_value: case.input_f(0x4D_CAFC),
        best_id: case.input(0x73_3088) as i32,
    }
}

fn world_of(case: &Case, kind: u32) -> EmberWorld {
    let mut pool = EffectPool {
        base: POOL,
        dirty: case.input(0x4D_FB1C),
        head: vec![-1; 4],
        tail: vec![-1; 4],
        class_of_type: vec![0; 0x18],
        ..EffectPool::default()
    };
    pool.class_of_type[0x17] = CLASS as u32;
    pool.head[CLASS] = case.input(0x73_88FC + 4 * CLASS as u32) as i32;
    pool.tail[CLASS] = case.input(0x73_68AC + 4 * CLASS as u32) as i32;
    for slot in 0..SLOTS {
        let base = POOL + slot * SLOT_BYTES;
        let mut entry = EffectEntry::default();
        for (index, word) in entry.w.iter_mut().enumerate() {
            *word = case.input(base + 4 * index as u32);
        }
        pool.entries.push(entry);
    }
    let row_at = |k: u32| {
        let a = 0x4D_B490 + 24 * k;
        KindRow {
            effect_a: case.input(a) as i32,
            effect_b: case.input(a + 4) as i32,
            word_8: case.input(a + 8),
            light_radius: case.input_f(a + 12),
            radius: case.input_f(a + 16),
            flags: case.input(a + 20),
        }
    };
    let mut rows = vec![KindRow::default(); KINDS];
    if (kind as usize) < KINDS {
        rows[kind as usize] = row_at(kind);
    }
    let count = case.input(0x79_0950);
    let players = (0..count)
        .map(|i| {
            let base = PL + i * PL_STRIDE;
            PlayerSlot {
                position: case.vec3(base + 0xB4),
                shake: case.input_f(base + 0xFBC),
                shake_limit: case.input_f(base + 0xFC0),
            }
        })
        .collect();
    let mut stats = vec![0.0f32; 0x12 * 0x273];
    for (index, cell) in stats.iter_mut().enumerate() {
        *cell = case.input_f(STATS + 4 * index as u32);
    }
    EmberWorld {
        globals: EmberGlobals {
            frame_rate: case.input(0x4E_8CEC) as i32,
            mode_byte: (case.input(0x7D_E454) >> 8) as u8,
            mode_flags: case.input(0x7D_E458),
            level_id: case.input(0x79_094C) as i32,
            game_state: case.input(0x7D_E30C) as i32,
            clock: case.input(0x7D_E318) as i32,
            throttle_stamp: case.input(0x73_7C5C) as i32,
            extra_players: case.input(0x7A_2958) as i32,
            level_limit: case.input(0x52_36C4) as i32,
            ground_height: case.input_f(0x4D_CB00),
            cell_count: case.input(0x7B_9754) as i32,
        },
        pool,
        rows,
        effect_defined: (0..8)
            .map(|id| case.input(0x4E_0FA0 + 0x1C * id) != 0)
            .collect(),
        players,
        collision: collision_of(case),
        recent: RecentList {
            words: (0..22).map(|i| case.input(0x7A_2900 + 4 * i)).collect(),
            row: case.input(0x7A_295C),
        },
        stats,
    }
}

fn spawn_args(case: &Case) -> SpawnArgs {
    let a = |i: u32| case.input(ARGS + 4 * i);
    let source = (a(4) != 0).then(|| SourceObject {
        address: a(4),
        kind: case.input(SRC + 0x20),
        state: case.input(SRC + 0x28) as i32,
        link: case.input(SRC + 0x7C),
        stat_index: (case.input(SRC + 0x150) != 0).then(|| case.input(STATB) as i32),
    });
    SpawnArgs {
        kind: a(0),
        extra: a(1),
        position: case.vec3(INP),
        direction: (a(3) != 0).then(|| case.vec3(INP + 0x20)),
        source,
        owner: a(5),
        seconds: f32::from_bits(a(6)),
        sound: a(7) != 0,
    }
}

fn engine_of(case: &Case) -> Recorder {
    let count = case.input(STUBIN) as usize;
    Recorder {
        ids: (0..count)
            .map(|i| case.input(STUBIN + 4 + 4 * i as u32) as i32)
            .collect(),
        walk: case.input(STUBIN + 0x100),
        flag: case.input(STUBIN + 0x104) != 0,
        low: case.input(0x4E_7950),
        high: case.input(0x4E_7954),
        calls: case.input(0x74_97C8),
        ..Recorder::default()
    }
}

#[derive(Default)]
struct Seen {
    kind15: usize,
    started: usize,
    released: usize,
    unstarted: usize,
    with_source: usize,
    linked: usize,
    ground: usize,
    nudged: usize,
    sounds: usize,
    recent: usize,
    stat: usize,
    default_path: usize,
    kinds: std::collections::BTreeSet<u32>,
}

/// Compares the state the golden dumps. `full` lists the slots dumped whole (0x8B words), the others 6 words.
fn check_world(n: usize, case: &Case, world: &EmberWorld, engine: &Recorder, full: &[u32]) {
    let expected: Vec<(u32, Vec<u32>)> = case.calls.iter().map(normalise).collect();
    assert_eq!(engine.log, expected, "case {n} engine calls");
    for slot in 0..SLOTS {
        let base = POOL + slot * SLOT_BYTES;
        let words = if full.contains(&slot) { 0x8B } else { 6 };
        for index in 0..words {
            assert_eq!(
                world.pool.entries[slot as usize].w[index as usize],
                case.output(base + 4 * index),
                "case {n} slot {slot} word {index:#x}"
            );
        }
    }
    let class = CLASS as u32;
    assert_eq!(
        world.pool.head[CLASS] as u32,
        case.output(0x73_88FC + 4 * class),
        "case {n} head"
    );
    assert_eq!(
        world.pool.tail[CLASS] as u32,
        case.output(0x73_68AC + 4 * class),
        "case {n} tail"
    );
    assert_eq!(world.pool.dirty, case.output(0x4D_FB1C), "case {n} dirty");
    for (index, player) in world.players.iter().enumerate() {
        let base = PL + index as u32 * PL_STRIDE;
        if case.outs.contains_key(&(base + 0xFBC)) {
            assert_eq!(
                player.shake.to_bits(),
                case.output(base + 0xFBC),
                "case {n} shake"
            );
            assert_eq!(
                player.shake_limit.to_bits(),
                case.output(base + 0xFC0),
                "case {n} limit"
            );
        }
    }
}

fn check_probe_globals(n: usize, case: &Case, world: &EmberWorld) {
    assert_eq!(
        world.collision.best_value.to_bits(),
        case.output(0x4D_CAFC),
        "case {n} best value"
    );
    assert_eq!(
        world.collision.best_id as u32,
        case.output(0x73_3088),
        "case {n} best id"
    );
    assert_eq!(
        world.globals.ground_height.to_bits(),
        case.output(0x4D_CB00),
        "case {n} ground height"
    );
}

fn check_rng(n: usize, case: &Case, engine: &Recorder) {
    let got = (engine.low, engine.high, engine.calls);
    let want = (
        case.output(0x4E_7950),
        case.output(0x4E_7954),
        case.output(0x74_97C8),
    );
    assert_eq!(got, want, "case {n} rng");
}

fn entry_pointer(returned: Option<u32>) -> u32 {
    match returned {
        Some(0) | None => 0,
        Some(slot_plus_one) => POOL + (slot_plus_one - 1) * SLOT_BYTES,
    }
}

fn run_case(n: usize, case: &Case, seen: &mut Seen) {
    let args = spawn_args(case);
    let mut world = world_of(case, args.kind);
    let mut engine = engine_of(case);
    let result = spawn_effect(&mut world, &mut engine, &args);
    let head = case.input(0x73_88FC + 4 * CLASS as u32);
    check_world(n, case, &world, &engine, &[head]);
    assert_eq!(
        entry_pointer(result.returned),
        case.eax,
        "case {n} (kind {:#x}) return",
        args.kind
    );
    check_probe_globals(n, case, &world);
    check_rng(n, case, &engine);
    assert_eq!(
        world.globals.throttle_stamp as u32,
        case.output(0x73_7C5C),
        "case {n} stamp"
    );
    for (index, word) in world.recent.words.iter().enumerate() {
        assert_eq!(
            *word,
            case.output(0x7A_2900 + 4 * index as u32),
            "case {n} recent {index}"
        );
    }
    assert_eq!(
        world.globals.extra_players as u32,
        case.output(0x7A_2958),
        "case {n} extra players / overrun"
    );
    for b in 0..4u32 {
        let index = (b * 0x273 + 0x52) as usize;
        assert_eq!(
            world.stats[index].to_bits(),
            case.output(STATS + 4 * index as u32),
            "case {n} stat {b}"
        );
    }
    // coverage
    let slot = result.slot.expect("allocated");
    let entry = &world.pool.entries[slot];
    seen.kinds.insert(args.kind);
    seen.kind15 += usize::from(args.kind == 15);
    seen.started += usize::from(result.started);
    seen.unstarted += usize::from(!result.started);
    seen.released += usize::from(result.started && result.returned == Some(0));
    seen.with_source += usize::from(args.source.is_some());
    seen.linked += usize::from(entry.w[W_LINK] != 0);
    seen.ground += usize::from(entry.w[W_GROUND] as i32 > 0);
    seen.nudged += usize::from(engine.log.iter().any(|(va, _)| *va == 0xD1EC0));
    seen.sounds += usize::from(engine.log.iter().any(|(va, _)| *va == 0x45820));
    seen.recent += usize::from(args.owner != 0 && world.globals.mode_byte == 0xA);
    seen.stat += usize::from(entry.w[W_MODE] == 1);
    seen.default_path += usize::from(engine.log.iter().any(|(va, _)| *va == 0xFF8A0));
}

#[test]
fn spawn_effect_matches_original() {
    let cases = parse(include_str!("data/t1856/particle_efbd0_root.golden"));
    assert!(cases.len() >= 60, "{} cases", cases.len());
    let mut seen = Seen::default();
    for (n, case) in cases.iter().enumerate() {
        run_case(n, case, &mut seen);
    }
    // the golden must reach every ported path
    assert!(seen.kind15 >= 20, "ember cases {}", seen.kind15);
    assert!(
        seen.started >= 15 && seen.released >= 5 && seen.unstarted >= 15,
        "{} {} {}",
        seen.started,
        seen.released,
        seen.unstarted
    );
    assert!(
        seen.with_source >= 15 && seen.linked >= 5,
        "source {} linked {}",
        seen.with_source,
        seen.linked
    );
    assert!(seen.ground >= 5, "ground {}", seen.ground);
    assert!(seen.nudged >= 3, "nudged {}", seen.nudged);
    assert!(seen.sounds >= 30, "sounds {}", seen.sounds);
    assert!(seen.recent >= 3, "recent {}", seen.recent);
    assert!(seen.stat >= 3, "stat {}", seen.stat);
    assert!(
        seen.default_path >= 2,
        "default start path {}",
        seen.default_path
    );
    assert!(seen.kinds.len() >= 20, "{:?}", seen.kinds);
}

#[test]
fn prelude_adapter_builds_the_original_call() {
    let effect = PreludeEffect::Particle {
        pos: [1.0, 2.0, 3.0],
        dir: [0.0, 1.0, 0.0],
    };
    let mut world = EmberWorld::default();
    world.pool.entries = vec![EffectEntry::default(); 3];
    world.pool.head = vec![0; 4];
    world.pool.tail = vec![2; 4];
    world.pool.entries[0].w[W_NEXT] = 1;
    world.pool.entries[1].w[W_PREV] = 0;
    world.pool.entries[1].w[W_NEXT] = 2;
    world.pool.entries[2].w[W_PREV] = 1;
    world.pool.entries[2].w[W_NEXT] = u32::MAX;
    world.pool.entries[0].w[W_PREV] = u32::MAX;
    world.pool.class_of_type = vec![1; 0x18];
    world.pool.head = vec![-1, 0, -1, -1];
    world.pool.tail = vec![-1, 2, -1, -1];
    world.rows = vec![
        KindRow {
            effect_a: -1,
            effect_b: -1,
            flags: 1,
            ..KindRow::default()
        };
        16
    ];
    world.collision.id_table = vec![0];
    let mut engine = Recorder::default();
    let result = run_prelude_particle(&mut world, &mut engine, &effect).expect("particle");
    assert!(result.started);
    assert_eq!(
        result.returned,
        Some(0),
        "kind 15 is a one shot: its entry is released"
    );
    let calls: Vec<u32> = engine.log.iter().map(|(va, _)| *va).collect();
    assert_eq!(calls, vec![0x87590, 0x111AE0, 0x111AE0, 0x10E660]);
    let emitter = &engine.log[1].1;
    assert_eq!(emitter[1], EMBER_TEMPLATES[0]);
    // position + direction * 0.1 for the stored entry position
    assert_eq!(f32::from_bits(emitter[3]), 1.0 + 0.0 * 0.1);
    assert_eq!(f32::from_bits(emitter[4]), 2.0 + 1.0 * 0.1);
    assert!(run_prelude_particle(&mut world, &mut engine, &PreludeEffect::Flash).is_none());
}

#[test]
fn recent_list_overruns_into_the_next_row() {
    let mut list = RecentList {
        words: vec![10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0],
        row: 0,
    };
    assert_eq!(list.push(0xAA), None);
    assert_eq!(
        list.words[11], 0xAA,
        "count 10 stores on the count word of row 1"
    );
    assert_eq!(list.words[0], 10, "count stays capped");
    let mut list = RecentList {
        words: vec![0; 22],
        row: 1,
    };
    list.push(5);
    list.push(6);
    assert_eq!(&list.words[11..14], &[2, 5, 6]);
}

#[test]
fn truncation_matches_cvttss2si() {
    assert_eq!(cvtt(1.9), 1);
    assert_eq!(cvtt(-1.9), -1);
    assert_eq!(cvtt(f32::NAN), i32::MIN);
    assert_eq!(cvtt(3.0e9), i32::MIN);
}

#[test]
fn every_kind_sound_matches_original() {
    let cases = parse(include_str!("data/t1856/particle_efbd0_sounds.golden"));
    assert!(cases.len() >= 52);
    let mut seen = Seen::default();
    for (n, case) in cases.iter().enumerate() {
        run_case(n, case, &mut seen);
    }
    assert_eq!(seen.kinds.len(), 0x34, "every kind 0..0x33");
    assert!(
        seen.sounds >= 40,
        "{} of {} cases submitted a sound",
        seen.sounds,
        cases.len()
    );
    assert_eq!(seen.started, 0, "the quiet cases never start the entry");
}

#[test]
fn ground_probe_matches_original() {
    let cases = parse(include_str!("data/t1856/particle_efbd0_probe.golden"));
    assert!(cases.len() >= 60);
    let (mut hits, mut walked, mut misses) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut world = world_of(case, 0);
        let mut engine = engine_of(case);
        let bias = case.input_f(ARGS);
        let radius = case.input_f(ARGS + 4);
        let cell = ground_probe(&mut world, &mut engine, case.vec3(INP), bias, radius);
        let expected: Vec<(u32, Vec<u32>)> = case.calls.iter().map(normalise).collect();
        assert_eq!(engine.log, expected, "case {n} calls");
        assert_eq!(cell as u32, case.eax, "case {n} cell");
        check_probe_globals(n, case, &world);
        walked += usize::from(engine.log.iter().any(|(va, _)| *va == 0x888A0));
        hits += usize::from(cell > 0);
        misses += usize::from(cell == 0);
    }
    assert!(
        walked >= 10 && hits >= 5 && misses >= 5,
        "walked {walked} hits {hits} misses {misses}"
    );
}

#[test]
fn pool_matches_original() {
    let cases = parse(include_str!("data/t1856/particle_efbd0_pool.golden"));
    assert!(cases.len() >= 40);
    let (mut allocations, mut releases, mut noops) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut world = world_of(case, 0);
        let mut engine = Recorder::default();
        let head = case.input(0x73_88FC + 4 * CLASS as u32);
        let mut full = vec![head];
        if case.input(ARGS) == 0xDC4A0 {
            let slot = world.pool.allocate(&mut engine, EFFECT_TYPE).expect("slot");
            assert_eq!(
                POOL + slot as u32 * SLOT_BYTES,
                case.eax,
                "case {n} allocated slot"
            );
            allocations += 1;
        } else {
            let victim = (case.input(ARGS + 4) - POOL) / SLOT_BYTES;
            full.push(victim);
            noops += usize::from(case.input(case.input(ARGS + 4)) == u32::MAX);
            world.pool.release(&mut engine, victim as usize);
            releases += 1;
        }
        check_world(n, case, &world, &engine, &full);
    }
    assert!(
        allocations >= 15 && releases >= 15 && noops >= 1,
        "{allocations} {releases} {noops}"
    );
}

#[test]
fn recent_list_matches_original() {
    let cases = parse(include_str!("data/t1856/particle_efbd0_recent.golden"));
    assert!(cases.len() >= 30);
    let mut overruns = 0;
    for (n, case) in cases.iter().enumerate() {
        let mut list = RecentList {
            words: (0..22).map(|i| case.input(0x7A_2900 + 4 * i)).collect(),
            row: case.input(0x7A_295C),
        };
        let count = list.words[list.row as usize * 11];
        overruns += usize::from(count >= 10);
        let stray = list.push(case.input(ARGS));
        for (index, word) in list.words.iter().enumerate() {
            assert_eq!(
                *word,
                case.output(0x7A_2900 + 4 * index as u32),
                "case {n} word {index}"
            );
        }
        // word 22 is the global [0x7A2958]: the overrun of row 1 lands there
        let mut word22 = case.input(0x7A_2958);
        if let Some((22, value)) = stray {
            word22 = value;
        }
        assert_eq!(word22, case.output(0x7A_2958), "case {n} stray store");
    }
    assert!(overruns >= 5, "overrun cases {overruns}");
}

#[test]
fn stat_add_matches_original() {
    let cases = parse(include_str!("data/t1856/particle_efbd0_stat.golden"));
    assert!(cases.len() >= 40);
    let (mut added, mut rejected) = (0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut world = world_of(case, 0);
        let mut engine = engine_of(case);
        let before = world.stats.clone();
        let a = case.input(ARGS) as i32;
        let b = case.input(ARGS + 4) as i32;
        stat_add(
            &mut world,
            &mut engine,
            a,
            b,
            f32::from_bits(case.input(ARGS + 8)),
        );
        let expected: Vec<(u32, Vec<u32>)> = case.calls.iter().map(normalise).collect();
        assert_eq!(engine.log, expected, "case {n} calls");
        for slot in 0..0x12u32 {
            for offset in [0x52u32, 0, 5] {
                let index = (slot * 0x273 + offset) as usize;
                assert_eq!(
                    world.stats[index].to_bits(),
                    case.output(STATS + 4 * index as u32),
                    "case {n} cell {index}"
                );
            }
        }
        if world.stats == before {
            rejected += 1;
        } else {
            added += 1;
        }
    }
    assert!(
        added >= 5 && rejected >= 5,
        "added {added} rejected {rejected}"
    );
}

#[test]
fn start_effect_matches_original() {
    let cases = parse(include_str!("data/t1856/particle_efbd0_start.golden"));
    assert!(cases.len() >= 50);
    let (mut released, mut settled, mut nudged, mut triggered) = (0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let base = case.input(ARGS + 4);
        let kind = case.input(ARGS + 8);
        let mut world = world_of(case, kind);
        let mut engine = engine_of(case);
        let slot = ((base - POOL) / SLOT_BYTES) as usize;
        let result = start_effect(&mut world, &mut engine, slot);
        check_world(n, case, &world, &engine, &[slot as u32]);
        let pointer = if result == 0 {
            0
        } else {
            POOL + (result - 1) * SLOT_BYTES
        };
        assert_eq!(pointer, case.eax, "case {n} (kind {kind}) return");
        released += usize::from(result == 0);
        settled += usize::from(engine.log.iter().any(|(va, _)| *va == 0xEDBE0));
        nudged += usize::from(engine.log.iter().any(|(va, _)| *va == 0xD1EC0));
        triggered += usize::from(engine.log.iter().any(|(va, _)| *va == 0xFF8A0));
    }
    assert!(
        released >= 10 && settled >= 10 && nudged >= 3 && triggered >= 3,
        "{released} {settled} {nudged} {triggered}"
    );
}

struct GraphEngine {
    graph: CellGraph,
}

impl EmberEngine for GraphEngine {
    fn rng(&mut self) -> u32 {
        0
    }
    fn submit_sound(&mut self, _request: [u32; 12]) {}
    fn throttled_sound(&mut self, _sound: ThrottledSound) {}
    fn region_query(&mut self, _region: [f32; 6]) -> Vec<i32> {
        vec![]
    }
    fn advance_cell(&mut self, probe: &AdvanceProbe) -> AdvanceOut {
        advance_88430(&self.graph, probe)
    }
    fn flag_3533f0(&mut self) -> bool {
        false
    }
    fn spawn_emitter(&mut self, _t: u32, _p: [f32; 3], _d: [f32; 3]) {}
    fn set_scale(&mut self, _scale: f32) {}
    fn nudge_player(&mut self, _p: usize, _f: f32, _a: f32, _b: f32) {}
    fn trigger_effect(&mut self, _id: i32, _p: [f32; 3]) {}
    fn settle(&mut self, _p: [f32; 3]) {}
    fn light(&mut self, _p: [f32; 3], _r: f32) {}
    fn teardown_slot(&mut self, _slot: usize, _entry: &mut EffectEntry) {}
    fn start_kind(&mut self, _w: &mut EmberWorld, _slot: usize, _kind: u32) -> u32 {
        0
    }
}

fn load_graph(case: &Case) -> CellGraph {
    let cells_base = case.input(0x70_8788);
    let table = case.input(0x70_878C);
    let cells = (0..=0xC8u32)
        .map(|cell| {
            let list = case.input(cells_base.wrapping_add(0xB4 * cell + 4));
            if list == 0 {
                return None;
            }
            let count = case.input(list) as i32;
            Some(
                (0..count.max(0) as u32)
                    .map(|i| case.input(list + 4 + 4 * i) as i32)
                    .collect(),
            )
        })
        .collect();
    let portals = (0..=0xC8u32)
        .map(|id| {
            let base = case.input(table + 4 * id);
            if base == 0 {
                return None;
            }
            let count = case.input(base + 0x14) as u16 as i16;
            Some(Portal {
                cell_a: case.input(base),
                cell_b: case.input(base + 4),
                normal: case.vec3(base + 8),
                plane_point: case.vec3(base + 0x18),
                vertices: (0..count.max(0) as u32)
                    .map(|k| case.vec3(base + 0x18 + 12 * k))
                    .collect(),
            })
        })
        .collect();
    CellGraph {
        max_portal_id: case.input(0x7B_9754) as i32,
        cells,
        portals,
    }
}

#[test]
fn cell_walk_matches_original() {
    let cases = parse(include_str!("data/t1856/particle_efbd0_walk.golden"));
    assert!(cases.len() >= 60);
    let (mut moved, mut stayed, mut bounded) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut engine = GraphEngine {
            graph: load_graph(case),
        };
        let from = case.vec3(0x150_0000);
        let vec = case.vec3(0x150_0100);
        let flag = case.input(ARGS);
        // arguments are not recorded in the golden: recover them from the dumped call is impossible, so the cell is
        // stored by the generator next to the flag
        let cell = case.input(ARGS + 4);
        let got = walk_cells(
            &mut engine,
            case.input(0x7B_9754) as i32,
            from,
            cell,
            vec,
            flag,
        );
        assert_eq!(got, case.eax, "case {n} final cell");
        moved += usize::from(got != cell);
        stayed += usize::from(got == cell);
        bounded += usize::from(got == cell + 100 || got + 100 == cell);
    }
    assert!(moved >= 10 && stayed >= 10, "moved {moved} stayed {stayed}");
    assert!(
        bounded >= 3,
        "cases that end at the 100 step bound: {bounded}"
    );
}
