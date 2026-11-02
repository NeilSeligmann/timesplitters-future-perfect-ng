//! T1870: the pool class loops `0xDC960` and `0xDC9E0` against emulated-original golden vectors (MEASURED).
//! Golden: `tests/data/t1870/{pool_destroy,pool_late}.golden` (`tools/t1846_gold/svc_pool_loops.py`).
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::svc_effect_tick::{ENTRY_BYTES, POOL_SLOTS};
use tsfp_gameplay::story_events::svc_particle_efbd0::*;
use tsfp_gameplay::story_events::svc_pool_classes::*;

const POOL: u32 = 0x110_0000;
const TYPES: u32 = 0x30;

struct Host {
    memory: BTreeMap<u32, u32>,
    log: Vec<(u32, u32)>,
    prelude: Vec<u32>,
    camera_arg: u32,
}

fn callback(entry_type: u32, late: bool) -> u32 {
    if !late {
        if entry_type % 4 == 0 {
            0
        } else {
            0x50_0000 + 0x10 * entry_type
        }
    } else if entry_type % 5 == 0 {
        0
    } else {
        0x50_0008 + 0x10 * entry_type
    }
}

impl ClassEngine for Host {
    fn callback(&self, entry_type: u32, slot: ClassSlot) -> u32 {
        assert!(entry_type < TYPES);
        match slot {
            ClassSlot::Destroy => callback(entry_type, false),
            ClassSlot::Late => callback(entry_type, true),
            _ => unreachable!(),
        }
    }
    fn run_callback(
        &mut self,
        world: &mut EmberWorld,
        slot: usize,
        _which: ClassSlot,
        address: u32,
    ) {
        assert!(slot < world.pool.entries.len());
        self.log.push((address, POOL + slot as u32 * ENTRY_BYTES));
    }
    fn read_word(&mut self, address: u32) -> u32 {
        *self.memory.get(&address).unwrap_or(&0)
    }
    fn write_word(&mut self, address: u32, value: u32) {
        self.memory.insert(address, value);
    }
    fn late_prelude(&mut self) {
        self.prelude.push(self.camera_arg);
    }
}

fn world_of(case: &Case) -> EmberWorld {
    let mut world = EmberWorld::default();
    world.pool.base = POOL;
    for slot in 0..POOL_SLOTS as u32 {
        let base = POOL + slot * ENTRY_BYTES;
        let mut entry = EffectEntry::default();
        entry.w[0] = u32::MAX;
        for (index, word) in entry.w.iter_mut().enumerate() {
            if let Some(value) = case.ins.get(&(base + 4 * index as u32)) {
                *word = *value;
            }
        }
        world.pool.entries.push(entry);
    }
    world
}

fn host_of(case: &Case) -> Host {
    let memory = case
        .ins
        .iter()
        .filter(|(a, _)| (0x140_0000..0x141_0000).contains(*a))
        .map(|(a, v)| (*a, *v))
        .collect();
    Host {
        memory,
        log: vec![],
        prelude: vec![],
        camera_arg: case.input(0x141_0208),
    }
}

fn expected_calls(case: &Case) -> Vec<(u32, u32)> {
    case.calls
        .iter()
        .filter(|c| c.callee != 0x1C6F0)
        .map(|c| (c.callee, c.args[0]))
        .collect()
}

fn compare(index: usize, case: &Case, world: &EmberWorld, host: &Host) {
    assert_eq!(host.log, expected_calls(case), "case {index} callbacks");
    for (address, want) in &case.outs {
        let got = if (POOL..POOL + POOL_SLOTS as u32 * ENTRY_BYTES).contains(address) {
            let slot = (address - POOL) / ENTRY_BYTES;
            world.pool.entries[slot as usize].w[(((address - POOL) % ENTRY_BYTES) / 4) as usize]
        } else {
            *host
                .memory
                .get(address)
                .unwrap_or_else(|| panic!("case {index}: no word {address:#x}"))
        };
        assert_eq!(got, *want, "case {index} word {address:#x}");
    }
}

#[test]
fn destroy_loop_matches_the_original() {
    let cases = parse(include_str!("data/t1870/pool_destroy.golden"));
    assert!(cases.len() >= 40);
    let (mut calls, mut writes) = (0, 0);
    for (index, case) in cases.iter().enumerate() {
        let mut world = world_of(case);
        let mut host = host_of(case);
        let before = host.memory.clone();
        destroy_loop(&mut world, &mut host);
        compare(index, case, &world, &host);
        calls += host.log.len();
        writes += host
            .memory
            .iter()
            .filter(|(a, v)| before.get(a) != Some(v))
            .count();
    }
    assert!(calls > 100 && writes > 20, "{calls} {writes}");
}

#[test]
fn late_loop_matches_the_original() {
    let cases = parse(include_str!("data/t1870/pool_late.golden"));
    assert!(cases.len() >= 40);
    let mut calls = 0;
    let mut skipped_phase = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut world = world_of(case);
        let mut host = host_of(case);
        late_loop(&mut world, &mut host);
        compare(index, case, &world, &host);
        assert_eq!(
            host.prelude,
            vec![case.input(0x141_0208)],
            "case {index} prelude"
        );
        let expected_prelude = case.calls.first().map(|c| (c.callee, c.args[0]));
        assert_eq!(
            expected_prelude,
            Some((0x1C6F0, case.input(0x141_0208))),
            "case {index}"
        );
        calls += host.log.len();
        skipped_phase += world
            .pool
            .entries
            .iter()
            .filter(|e| e.w[0] != u32::MAX && e.w[6] & 0xFFFF != 1)
            .count();
    }
    assert!(calls > 50 && skipped_phase > 20, "{calls} {skipped_phase}");
}

#[test]
fn class_table_reads_from_an_image() {
    let table = ClassTable {
        records: vec![[0; 16]; 4],
    };
    assert_eq!(table.callback(2, ClassSlot::Destroy), 0);
    assert_eq!(table.callback(99, ClassSlot::Late), 0);
}
