//! T1870: the whole pool tick `0xDC7B0` with entries of the other classes (class update callbacks, source flag masking) against
//! emulated-original golden vectors (MEASURED). Golden: `tests/data/t1870/pool_tick_all.golden`
//! (`tools/t1846_gold/svc_pool_loops.py`). The class callbacks are logged stubs that sometimes rewrite one foreign word (scripted tape).
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::callees_t1846::ThrottledSound;
use tsfp_gameplay::story_events::group_members::{AdvanceOut, AdvanceProbe};
use tsfp_gameplay::story_events::svc_effect_tick::*;
use tsfp_gameplay::story_events::svc_particle_efbd0::*;
use tsfp_gameplay::story_events::svc_pool_classes::*;

const POOL: u32 = 0x110_0000;
const MUTATIONS: u32 = 0xE300_0000;
const TAPE: u32 = 0xE400_0000;
const TYPES: u32 = 0x30;

struct Host {
    memory: BTreeMap<u32, u32>,
    mutations: Vec<(u32, u32)>,
    tape: Vec<u32>,
    cursor: usize,
    log: Vec<(u32, u32)>,
}

impl EmberEngine for Host {
    fn rng(&mut self) -> u32 {
        unreachable!()
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

impl TickEngine for Host {
    fn start_entry(&mut self, _world: &mut EmberWorld, _slot: usize) -> u32 {
        unreachable!()
    }
    fn trail(&mut self, _slot: usize, _record: &[u32; TRAIL_WORDS], _timer: f32) {
        unreachable!()
    }
    fn spray(&mut self, _call: SprayCall) {
        unreachable!()
    }
    fn owner_cell(&mut self, _owner: u32) -> u32 {
        unreachable!()
    }
    fn tick_class(&mut self, world: &mut EmberWorld, slot: usize, timer_before: f32) {
        class_tick(world, self, slot, timer_before);
    }
}

impl ClassEngine for Host {
    fn callback(&self, entry_type: u32, slot: ClassSlot) -> u32 {
        assert_eq!(slot, ClassSlot::Update);
        assert!(entry_type < TYPES);
        if entry_type == 0x17 || entry_type % 7 == 0 {
            0
        } else {
            0x50_0004 + 0x10 * entry_type
        }
    }
    fn run_callback(
        &mut self,
        world: &mut EmberWorld,
        slot: usize,
        _which: ClassSlot,
        address: u32,
    ) {
        self.log.push((address, POOL + slot as u32 * ENTRY_BYTES));
        let choice = self.tape[self.cursor] as usize;
        self.cursor += 1;
        if let Some(&(target, value)) = self.mutations.get(choice) {
            self.memory.insert(target, value);
        }
        let _ = world;
    }
    fn read_word(&mut self, address: u32) -> u32 {
        *self.memory.get(&address).unwrap_or(&0)
    }
    fn write_word(&mut self, address: u32, value: u32) {
        self.memory.insert(address, value);
    }
    fn late_prelude(&mut self) {
        unreachable!()
    }
}

fn world_of(case: &Case) -> (EmberWorld, TickGlobals) {
    let mut pool = EffectPool {
        base: POOL,
        dirty: case.input(0x4D_FB1C),
        head: vec![case.input(0x73_88FC) as i32],
        tail: vec![case.input(0x73_68AC) as i32],
        class_of_type: vec![0; TYPES as usize],
        ..EffectPool::default()
    };
    for slot in 0..POOL_SLOTS as u32 {
        let base = POOL + slot * ENTRY_BYTES;
        let mut entry = EffectEntry::default();
        entry.w[0] = u32::MAX;
        for (index, word) in entry.w.iter_mut().enumerate() {
            if let Some(value) = case.ins.get(&(base + 4 * index as u32)) {
                *word = *value;
            }
        }
        pool.entries.push(entry);
    }
    let world = EmberWorld {
        globals: EmberGlobals {
            frame_rate: case.input(0x4E_8CEC) as i32,
            ..EmberGlobals::default()
        },
        pool,
        ..EmberWorld::default()
    };
    let tick = TickGlobals {
        steps: case.input(0x4E_7944) as i32,
        scale: case.input_f(0x4E_794C),
    };
    (world, tick)
}

#[test]
fn pool_tick_with_class_callbacks_matches_the_original() {
    let cases = parse(include_str!("data/t1870/pool_tick_all.golden"));
    assert!(cases.len() >= 30);
    let (mut calls, mut masked, mut mutated, mut released) = (0, 0, 0, 0);
    for (index, case) in cases.iter().enumerate() {
        let (mut world, tick) = world_of(case);
        let memory: BTreeMap<u32, u32> = case
            .ins
            .iter()
            .filter(|(a, _)| **a < 0xE000_0000)
            .map(|(a, v)| (*a, *v))
            .collect();
        let mutations: Vec<(u32, u32)> = (0..6)
            .map(|k| {
                (
                    case.input(MUTATIONS + 8 * k),
                    case.input(MUTATIONS + 8 * k + 4),
                )
            })
            .collect();
        let tape: Vec<u32> = (0..)
            .map(|k| case.ins.get(&(TAPE + 4 * k)))
            .take_while(Option::is_some)
            .map(|v| *v.unwrap())
            .collect();
        let mut host = Host {
            memory,
            mutations,
            tape,
            cursor: 0,
            log: vec![],
        };
        let before: Vec<_> = world.pool.entries.iter().map(|e| e.w[0]).collect();
        pool_tick(&mut world, &mut host, &tick);
        let expected: Vec<(u32, u32)> = case.calls.iter().map(|c| (c.callee, c.args[0])).collect();
        assert_eq!(host.log, expected, "case {index} callbacks");
        for (address, want) in &case.outs {
            let got = if (POOL..POOL + POOL_SLOTS as u32 * ENTRY_BYTES).contains(address) {
                let slot = (address - POOL) / ENTRY_BYTES;
                world.pool.entries[slot as usize].w[(((address - POOL) % ENTRY_BYTES) / 4) as usize]
            } else {
                match *address {
                    0x4D_FB1C => world.pool.dirty,
                    0x73_88FC => world.pool.head[0] as u32,
                    0x73_68AC => world.pool.tail[0] as u32,
                    other => host
                        .memory
                        .get(&other)
                        .copied()
                        .unwrap_or_else(|| case.input(other)),
                }
            };
            assert_eq!(got, *want, "case {index} word {address:#x}");
        }
        calls += host.log.len();
        masked += case
            .ins
            .iter()
            .filter(|(a, v)| {
                (0x145_0000..0x146_0000).contains(*a) && **a % 0x100 == 0x28 && **v & 0x60 != 0
            })
            .count();
        mutated += host.tape.iter().filter(|c| **c < 6).count();
        released += world
            .pool
            .entries
            .iter()
            .zip(&before)
            .filter(|(e, b)| e.w[0] == u32::MAX && **b != u32::MAX)
            .count();
    }
    assert!(
        calls > 100 && masked > 10 && mutated > 10 && released > 5,
        "{calls} {masked} {mutated} {released}"
    );
}
