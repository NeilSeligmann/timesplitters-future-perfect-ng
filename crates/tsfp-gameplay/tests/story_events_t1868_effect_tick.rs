//! T1868: the effect pool tick `0xDC7B0` and the entry update `0xEFF90` against emulated-original golden vectors (MEASURED),
//! plus semantic unit tests. Golden: `tests/data/t1868/*.golden` (`tools/t1846_gold/svc_effect_tick.py`).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::callees_t1846::ThrottledSound;
use tsfp_gameplay::story_events::group_members::{AdvanceOut, AdvanceProbe};
use tsfp_gameplay::story_events::svc_effect_tick::*;
use tsfp_gameplay::story_events::svc_particle_efbd0::*;

const POOL: u32 = 0x110_0000;
const SLOT_BYTES: u32 = 0x22C;
const STARTS: u32 = 0x132_0000;
const ROWS: u32 = 0x4D_B490;

#[derive(Default)]
struct Recorder {
    log: Vec<(u32, Vec<u32>)>,
    low: u32,
    high: u32,
    calls: u32,
    starts: Vec<u32>,
    cursor: usize,
    cell: u32,
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
    fn submit_sound(&mut self, _request: [u32; 12]) {
        unreachable!()
    }
    fn throttled_sound(&mut self, _sound: ThrottledSound) {
        unreachable!()
    }
    fn region_query(&mut self, _region: [f32; 6]) -> Vec<i32> {
        unreachable!()
    }
    fn advance_cell(&mut self, _probe: &AdvanceProbe) -> AdvanceOut {
        unreachable!()
    }
    fn flag_3533f0(&mut self) -> bool {
        unreachable!()
    }
    fn spawn_emitter(&mut self, _t: u32, _p: [f32; 3], _d: [f32; 3]) {
        unreachable!()
    }
    fn set_scale(&mut self, _scale: f32) {
        unreachable!()
    }
    fn nudge_player(&mut self, _player: usize, _f: f32, _a: f32, _b: f32) {
        unreachable!()
    }
    fn trigger_effect(&mut self, _id: i32, _position: [f32; 3]) {
        unreachable!()
    }
    fn settle(&mut self, _position: [f32; 3]) {
        unreachable!()
    }
    fn light(&mut self, _position: [f32; 3], _radius_squared: f32) {
        unreachable!()
    }
    fn teardown_slot(&mut self, _slot: usize, _entry: &mut EffectEntry) {}
    fn start_kind(&mut self, _world: &mut EmberWorld, _slot: usize, _kind: u32) -> u32 {
        unreachable!()
    }
}

impl TickEngine for Recorder {
    fn start_entry(&mut self, _world: &mut EmberWorld, slot: usize) -> u32 {
        self.log.push((0xEDD40, vec![slot as u32]));
        let go = self.starts[self.cursor];
        self.cursor += 1;
        if go != 0 { slot as u32 + 1 } else { 0 }
    }
    fn trail(&mut self, slot: usize, record: &[u32; TRAIL_WORDS], timer: f32) {
        let mut words = vec![slot as u32];
        words.extend_from_slice(record);
        words.push(timer.to_bits());
        self.log.push((0xBB850, words));
    }
    fn spray(&mut self, call: SprayCall) {
        let bits = |v: [f32; 3]| v.map(f32::to_bits);
        match call {
            SprayCall::Burst {
                mode,
                owner,
                position,
                velocity,
            } => {
                let mut words = vec![mode, owner];
                words.extend(bits(position));
                words.extend(bits(velocity));
                self.log.push((0x345B60, words));
            }
            SprayCall::Streak {
                position,
                velocity,
                cell,
            } => {
                let mut words = vec![cell];
                words.extend(bits(position));
                words.extend(bits(velocity));
                self.log.push((0x345C60, words));
            }
        }
    }
    fn owner_cell(&mut self, _owner: u32) -> u32 {
        self.cell
    }
}

/// The golden call in the layout of [`Recorder`] (pointer arguments dropped).
fn normalise(call: &common::Call) -> (u32, Vec<u32>) {
    let a = &call.args;
    let words = match call.callee {
        0xEDD40 => vec![(a[0] - POOL) / SLOT_BYTES],
        0xBB850 => [
            &[(a[0] - 0xFC - POOL) / SLOT_BYTES][..],
            &a[3..26],
            &a[2..3],
        ]
        .concat(),
        0x345B60 => [&a[0..2], &a[5..11]].concat(),
        0x345C60 => [&a[2..3], &a[5..11]].concat(),
        other => panic!("unexpected stub {other:#x}"),
    };
    (call.callee, words)
}

fn world_of(case: &Case) -> (EmberWorld, TickGlobals) {
    let mut pool = EffectPool {
        base: POOL,
        dirty: case.input(0x4D_FB1C),
        head: vec![-1; 4],
        tail: vec![-1; 4],
        class_of_type: vec![0; 0x18],
        ..EffectPool::default()
    };
    pool.class_of_type[0x17] = 2;
    for class in [0u32, 2] {
        pool.head[class as usize] = case.input(0x73_88FC + 4 * class) as i32;
        pool.tail[class as usize] = case.input(0x73_68AC + 4 * class) as i32;
    }
    for slot in 0..POOL_SLOTS as u32 {
        let base = POOL + slot * SLOT_BYTES;
        let mut entry = EffectEntry::default();
        entry.w[0] = u32::MAX;
        for (index, word) in entry.w.iter_mut().enumerate() {
            if let Some(value) = case.ins.get(&(base + 4 * index as u32)) {
                *word = *value;
            }
        }
        pool.entries.push(entry);
    }
    let rows = (0..52u32)
        .map(|k| {
            let a = ROWS + 24 * k;
            KindRow {
                effect_a: case.input(a) as i32,
                effect_b: case.input(a + 4) as i32,
                word_8: case.input(a + 8),
                light_radius: case.input_f(a + 12),
                radius: case.input_f(a + 16),
                flags: case.input(a + 20),
            }
        })
        .collect();
    let world = EmberWorld {
        globals: EmberGlobals {
            frame_rate: case.input(0x4E_8CEC) as i32,
            mode_byte: (case.input(0x7D_E454) >> 8) as u8,
            ..EmberGlobals::default()
        },
        pool,
        rows,
        ..EmberWorld::default()
    };
    let tick = TickGlobals {
        steps: case.input(0x4E_7944) as i32,
        scale: case.input_f(0x4E_794C),
    };
    (world, tick)
}

fn engine_of(case: &Case) -> Recorder {
    Recorder {
        low: case.input(0x4E_7950),
        high: case.input(0x4E_7954),
        calls: case.input(0x74_97C8),
        starts: (0..40).map(|i| case.input(STARTS + 4 * i)).collect(),
        cell: case.input(0x130_0034),
        ..Recorder::default()
    }
}

fn world_word(world: &EmberWorld, engine: &Recorder, address: u32) -> u32 {
    let pool_end = POOL + POOL_SLOTS as u32 * SLOT_BYTES;
    if (POOL..pool_end).contains(&address) {
        let slot = (address - POOL) / SLOT_BYTES;
        let index = ((address - POOL) % SLOT_BYTES) / 4;
        return world.pool.entries[slot as usize].w[index as usize];
    }
    match address {
        0x4D_FB1C => world.pool.dirty,
        0x4E_7950 => engine.low,
        0x4E_7954 => engine.high,
        0x74_97C8 => engine.calls,
        a if (0x73_88FC..0x73_88FC + 16).contains(&a) => {
            world.pool.head[((a - 0x73_88FC) / 4) as usize] as u32
        }
        a if (0x73_68AC..0x73_68AC + 16).contains(&a) => {
            world.pool.tail[((a - 0x73_68AC) / 4) as usize] as u32
        }
        other => panic!("unexpected output address {other:#x}"),
    }
}

fn check(n: usize, case: &Case, world: &EmberWorld, engine: &Recorder) {
    let expected: Vec<(u32, Vec<u32>)> = case.calls.iter().map(normalise).collect();
    assert_eq!(engine.log, expected, "case {n} engine calls");
    assert!(!case.outs.is_empty());
    for (address, want) in &case.outs {
        assert_eq!(
            world_word(world, engine, *address),
            *want,
            "case {n} word at {address:#x}"
        );
    }
}

#[test]
fn sine_table_matches_the_original_init_function() {
    let cases = parse(include_str!("data/t1868/effect_trig.golden"));
    let case = &cases[0];
    let table = trig_table();
    assert_eq!(table.len(), 0x800);
    let mut digest = 0x811C_9DC5u32;
    for value in table {
        for byte in value.to_bits().to_le_bytes() {
            digest = (digest ^ u32::from(byte)).wrapping_mul(0x0100_0193);
        }
    }
    assert_eq!(digest, case.output(0xF000_0000));
    for k in 0..64u32 {
        assert_eq!(
            table[32 * k as usize].to_bits(),
            case.output(0x7A_38A0 + 4 * 32 * k),
            "sample {k}"
        );
    }
}

#[derive(Default)]
struct Seen {
    started: usize,
    refused: usize,
    released: usize,
    trails: usize,
    sprayed: usize,
    waiting_left: usize,
    kinds: std::collections::BTreeSet<u32>,
}

#[test]
fn effect_update_matches_the_original() {
    let cases = parse(include_str!("data/t1868/effect_update.golden"));
    assert!(cases.len() >= 40);
    let mut seen = Seen::default();
    for (n, case) in cases.iter().enumerate() {
        let (mut world, tick) = world_of(case);
        let mut engine = engine_of(case);
        let target = target_slot(case);
        let before = world.pool.entries[target].clone();
        effect_update(&mut world, &mut engine, target, &tick);
        check(n, case, &world, &engine);
        let after = &world.pool.entries[target];
        seen.kinds.insert(before.w[W_KIND]);
        seen.started += engine.log.iter().filter(|(c, _)| *c == 0xEDD40).count();
        seen.refused += usize::from(
            engine.log.iter().any(|(c, _)| *c == 0xEDD40)
                && engine.cursor > 0
                && after.w[W_FLAGS] & FLAG_WAITING == 0
                && before.w[W_FLAGS] & FLAG_WAITING != 0,
        );
        seen.released += usize::from(after.w[W_TYPE] == u32::MAX && before.w[W_TYPE] != u32::MAX);
        seen.trails += engine.log.iter().filter(|(c, _)| *c == 0xBB850).count();
        seen.sprayed += engine.log.iter().filter(|(c, _)| *c == 0x345B60).count();
        seen.waiting_left += usize::from(after.w[W_FLAGS] & FLAG_WAITING != 0);
    }
    assert!(
        seen.started > 5
            && seen.released > 2
            && seen.trails > 10
            && seen.sprayed > 10
            && seen.waiting_left > 2,
        "coverage"
    );
}

/// The slot the original was called with: the generator records the argument at `0x1330000`.
fn target_slot(case: &Case) -> usize {
    ((case.input(0x133_0000) - POOL) / SLOT_BYTES) as usize
}

#[test]
fn pool_tick_matches_the_original() {
    let cases = parse(include_str!("data/t1868/effect_tick.golden"));
    assert!(cases.len() >= 40);
    let mut seen = Seen::default();
    for (n, case) in cases.iter().enumerate() {
        let (mut world, tick) = world_of(case);
        let mut engine = engine_of(case);
        let before: Vec<_> = world.pool.entries.clone();
        pool_tick(&mut world, &mut engine, &tick);
        check(n, case, &world, &engine);
        seen.started += engine.log.iter().filter(|(c, _)| *c == 0xEDD40).count();
        seen.trails += engine.log.iter().filter(|(c, _)| *c == 0xBB850).count();
        seen.sprayed += engine.log.iter().filter(|(c, _)| *c == 0x345B60).count();
        seen.released += world
            .pool
            .entries
            .iter()
            .zip(&before)
            .filter(|(after, before)| after.w[W_TYPE] == u32::MAX && before.w[W_TYPE] != u32::MAX)
            .count();
        for (after, before) in world.pool.entries.iter().zip(&before) {
            seen.waiting_left += usize::from(
                after.w[W_FLAGS] & FLAG_WAITING != 0 && before.w[W_FLAGS] & FLAG_WAITING != 0,
            );
        }
    }
    assert!(
        seen.started > 10
            && seen.released > 5
            && seen.trails > 30
            && seen.sprayed > 10
            && seen.waiting_left > 5,
        "coverage"
    );
}
