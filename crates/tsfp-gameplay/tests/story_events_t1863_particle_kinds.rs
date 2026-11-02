//! T1863: the per kind handlers of the effect start `0xEDD40` against emulated-original golden vectors (MEASURED), plus
//! semantic unit tests. Golden: `tests/data/t1863/particle_kinds_*.golden` (`tools/t1846_gold/svc_particle_kinds.py`).
mod common;
use std::collections::BTreeMap;

use common::{Call, Case, parse};
use tsfp_gameplay::story_events::callees_t1846::ThrottledSound;
use tsfp_gameplay::story_events::group_members::{AdvanceOut, AdvanceProbe};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_particle_efbd0::*;
use tsfp_gameplay::story_events::svc_particle_kinds::*;

const POOL: u32 = 0x110_0000;
const SLOTS: u32 = 6;
const SLOT_BYTES: u32 = 0x22C;
const PL: u32 = 0x120_0000;
const PL_STRIDE: u32 = 0x1584;
const ARGS: u32 = 0x133_0000;
const DEB0: u32 = 0x163_0000;
const DEB_STRIDE: u32 = 0x400;
const CLASS: usize = 2;
const KINDS: usize = 52;

/// Engine stand-in: guest memory seeded from the golden inputs, the rng, and a log of every engine call.
#[derive(Default)]
struct Recorder {
    mem: BTreeMap<u32, u32>,
    log: Vec<(u32, Vec<u32>)>,
    low: u32,
    high: u32,
    calls: u32,
    gate: u32,
    objects: u32,
    handles: u32,
}

impl GuestMemory for Recorder {
    fn read_u8(&self, address: u32) -> u8 {
        let word = self.mem.get(&(address & !3)).copied().unwrap_or(0);
        (word >> (8 * (address & 3))) as u8
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        let shift = 8 * (address & 3);
        let word = self.mem.entry(address & !3).or_insert(0);
        *word = (*word & !(0xFF << shift)) | (u32::from(value) << shift);
    }
    fn read_u32(&self, address: u32) -> u32 {
        assert_eq!(address & 3, 0, "unaligned read {address:#x}");
        self.mem.get(&address).copied().unwrap_or(0)
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        assert_eq!(address & 3, 0, "unaligned write {address:#x}");
        self.mem.insert(address, value);
    }
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
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
    fn throttled_sound(&mut self, _sound: ThrottledSound) {
        unreachable!("no throttled sound in the kind handlers")
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
        unreachable!("kind 15 is not under test")
    }
    fn set_scale(&mut self, _scale: f32) {
        unreachable!()
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
    fn trigger_effect(&mut self, _id: i32, _position: [f32; 3]) {
        unreachable!()
    }
    fn settle(&mut self, position: [f32; 3]) {
        let mut words = vec![0];
        words.extend(bits(&position));
        self.log.push((0xEDBE0, words));
    }
    fn light(&mut self, position: [f32; 3], radius_squared: f32) {
        let mut words = vec![radius_squared.to_bits()];
        words.extend(bits(&position));
        self.log.push((0x186480, words));
    }
    fn teardown_slot(&mut self, _slot: usize, _entry: &mut EffectEntry) {}
    fn start_kind(&mut self, world: &mut EmberWorld, slot: usize, kind: u32) -> u32 {
        start_kind_handler(world, self, slot, kind).expect("kind has a handler")
    }
}

impl KindEngine for Recorder {
    fn guest(&mut self) -> &mut dyn GuestMemory {
        self
    }
    fn emitter(&mut self, template: u32, flags: u32, position: [f32; 3], direction: [f32; 3]) {
        let mut words = vec![0, template, flags];
        words.extend(bits(&position));
        words.extend(bits(&direction));
        self.log.push((0x111AE0, words));
    }
    fn emitter_param(&mut self, param: EmitterParam) {
        let (callee, words) = match param {
            EmitterParam::Scale(v) => (0x10E660, bits(&[v])),
            EmitterParam::Span(a, b) => (0x10EAE0, bits(&[a, b])),
            EmitterParam::Param70(v) => (0x10EB30, bits(&[v])),
            EmitterParam::Param74(v) => (0x10EB70, bits(&[v])),
            EmitterParam::Vec98(v) => (0x10E9D0, bits(&v)),
            EmitterParam::Attach(handle) => (0x10E8C0, vec![handle, 0]),
        };
        self.log.push((callee, words));
    }
    fn actor_kick(&mut self, handle: u32) {
        self.log.push((0x126260, vec![handle, 0]));
    }
    fn actor_gate(&mut self, actor: u32) -> bool {
        self.log.push((0x278040, vec![actor]));
        self.gate != 0
    }
    fn flash(&mut self, vector: [f32; 3], _origin: [f32; 3]) {
        self.log.push((0xEDCB0, bits(&vector)));
    }
    fn beam(&mut self, handle: u32, position: [f32; 3], direction: [f32; 3]) {
        let mut words = vec![handle];
        words.extend(bits(&position));
        words.extend(bits(&direction));
        words.extend(bits(&[1.0, 1.0]));
        self.log.push((0xFAC70, words));
    }
    fn screen_flash(&mut self, value: f32) {
        self.log.push((0x668A0, bits(&[value])));
    }
    fn debris(&mut self, call: DebrisCall) -> u32 {
        match call {
            DebrisCall::Create {
                class,
                position,
                rotation,
            } => {
                let mut words = vec![class];
                words.extend(bits(&position));
                words.extend(bits(&rotation));
                self.log.push((0xA6F90, words));
                self.objects += 1;
                DEB0 + DEB_STRIDE * (self.objects - 1)
            }
            DebrisCall::Handle { object } => {
                self.log.push((0x16C170, vec![object, 2]));
                self.handles += 1;
                0x5000 + 0x10 * self.handles
            }
            DebrisCall::Bind { handle, object } => {
                self.log
                    .push((0x172760, vec![handle, object, 0x11, u32::MAX, 0x180]));
                0
            }
            DebrisCall::Limit { handle } => {
                self.log.push((0x16B790, vec![handle, 8, 0xFF]));
                0
            }
            DebrisCall::Launch {
                handle,
                position,
                velocity,
            } => {
                let mut words = vec![handle];
                words.extend(bits(&position));
                words.extend(bits(&velocity));
                self.log.push((0x16C980, words));
                0
            }
            DebrisCall::Finish {
                object,
                life,
                count,
            } => {
                self.log
                    .push((0xAE240, vec![object, life.to_bits(), count]));
                0
            }
        }
    }
}

/// The logged words of a golden call in the layout of [`Recorder`] (pointer arguments dropped).
fn normalise(call: &Call) -> (u32, Vec<u32>) {
    let a = &call.args;
    let words = match call.callee {
        0x45820 => a[1..13].to_vec(),
        0x111AE0 => [&a[..3], &a[5..8], &a[8..11]].concat(),
        0x10E660 | 0x10EB30 | 0x10EB70 | 0x668A0 => a[..1].to_vec(),
        0x10EAE0 | 0x10E8C0 | 0x126260 | 0x16C170 => a[..2].to_vec(),
        0x10E9D0 => a[1..4].to_vec(),
        0x278040 => a[..1].to_vec(),
        0xEDCB0 => a[..3].to_vec(),
        0xFAC70 => [&a[..1], &a[5..8], &a[8..11], &a[3..5]].concat(),
        0xA6F90 => [&a[..1], &a[5..8], &a[8..11]].concat(),
        0x172760 | 0x16B790 | 0xAE240 => a.clone(),
        0x16C980 => [&a[..1], &a[3..6], &a[6..9]].concat(),
        0xD1EC0 => a[..4].to_vec(),
        0xEDBE0 | 0x186480 => [&a[1..2], &a[2..5]].concat(),
        other => panic!("unexpected stub {other:#x}"),
    };
    (call.callee, words)
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
    let a = 0x4D_B490 + 24 * kind;
    let mut rows = vec![KindRow::default(); KINDS];
    rows[kind as usize] = KindRow {
        effect_a: case.input(a) as i32,
        effect_b: case.input(a + 4) as i32,
        word_8: case.input(a + 8),
        light_radius: case.input_f(a + 12),
        radius: case.input_f(a + 16),
        flags: case.input(a + 20),
    };
    let players = (0..case.input(0x79_0950))
        .map(|i| {
            let base = PL + i * PL_STRIDE;
            PlayerSlot {
                position: case.vec3(base + 0xB4),
                shake: case.input_f(base + 0xFBC),
                shake_limit: case.input_f(base + 0xFC0),
            }
        })
        .collect();
    EmberWorld {
        globals: EmberGlobals {
            mode_byte: (case.input(0x7D_E454) >> 8) as u8,
            level_id: case.input(0x79_094C) as i32,
            ..EmberGlobals::default()
        },
        pool,
        rows,
        effect_defined: vec![false; 8],
        players,
        ..EmberWorld::default()
    }
}

fn engine_of(case: &Case) -> Recorder {
    Recorder {
        mem: case.ins.clone().into_iter().collect(),
        low: case.input(0x4E_7950),
        high: case.input(0x4E_7954),
        calls: case.input(0x74_97C8),
        gate: case.input(ARGS + 0x10),
        ..Recorder::default()
    }
}

fn check(n: usize, kind: u32, case: &Case, world: &EmberWorld, engine: &Recorder, entry_slot: u32) {
    let expected: Vec<(u32, Vec<u32>)> = case.calls.iter().map(normalise).collect();
    assert_eq!(engine.log, expected, "case {n} kind {kind} engine calls");
    for slot in 0..SLOTS {
        let base = POOL + slot * SLOT_BYTES;
        let words = if slot == entry_slot { 0x8B } else { 6 };
        for index in 0..words {
            assert_eq!(
                world.pool.entries[slot as usize].w[index as usize],
                case.output(base + 4 * index),
                "case {n} kind {kind} slot {slot} word {index:#x}"
            );
        }
    }
    let class = CLASS as u32;
    assert_eq!(
        world.pool.head[CLASS] as u32,
        case.output(0x73_88FC + 4 * class)
    );
    assert_eq!(
        world.pool.tail[CLASS] as u32,
        case.output(0x73_68AC + 4 * class)
    );
    assert_eq!(world.pool.dirty, case.output(0x4D_FB1C), "case {n} dirty");
    for (index, player) in world.players.iter().enumerate() {
        let base = PL + index as u32 * PL_STRIDE;
        assert_eq!(player.shake.to_bits(), case.output(base + 0xFBC));
        assert_eq!(player.shake_limit.to_bits(), case.output(base + 0xFC0));
    }
    assert_eq!(
        (engine.low, engine.high),
        (case.output(0x4E_7950), case.output(0x4E_7954)),
        "case {n} kind {kind} rng state"
    );
    assert_eq!(
        engine.calls,
        case.output(0x74_97C8),
        "case {n} rng call count"
    );
    for (address, value) in &case.outs {
        if (DEB0..DEB0 + 3 * DEB_STRIDE).contains(address) {
            assert_eq!(
                engine.mem.get(address).copied().unwrap_or(0),
                *value,
                "case {n} kind {kind} object word {address:#x}"
            );
        }
    }
}

#[derive(Default)]
struct Seen {
    per_kind: BTreeMap<u32, usize>,
    calls: BTreeMap<u32, usize>,
    released: usize,
    settled: usize,
}

#[test]
fn every_kind_handler_matches_original() {
    let cases = parse(include_str!("data/t1863/particle_kinds_root.golden"));
    assert!(cases.len() >= 200);
    let mut seen = Seen::default();
    for (n, case) in cases.iter().enumerate() {
        let entry = case.input(ARGS + 4);
        let kind = case.input(ARGS + 8);
        let slot = (entry - POOL) / SLOT_BYTES;
        let mut world = world_of(case, kind);
        let mut engine = engine_of(case);
        let result = start_effect(&mut world, &mut engine, slot as usize);
        check(n, kind, case, &world, &engine, slot);
        let pointer = if result == 0 { 0 } else { entry };
        assert_eq!(pointer, case.eax, "case {n} kind {kind} result");
        *seen.per_kind.entry(kind).or_default() += 1;
        for (callee, _) in &engine.log {
            *seen.calls.entry(*callee).or_default() += 1;
        }
        if result == 0 {
            seen.released += 1;
        } else {
            seen.settled += 1;
        }
    }
    let handled: Vec<u32> = HANDLED_KINDS.to_vec();
    assert_eq!(seen.per_kind.keys().copied().collect::<Vec<_>>(), handled);
    assert!(seen.released > 0 && seen.settled > 0);
    for callee in [
        0x111AE0, 0x10E660, 0x10EAE0, 0x10EB30, 0x10EB70, 0x10E9D0, 0x10E8C0, 0x126260, 0x278040,
        0xEDCB0, 0xFAC70, 0x668A0, 0x45820, 0xA6F90, 0x16C170, 0x172760, 0x16B790, 0x16C980,
        0xAE240, 0xD1EC0,
    ] {
        assert!(
            seen.calls.contains_key(&callee),
            "golden never reaches {callee:#x}"
        );
    }
}
