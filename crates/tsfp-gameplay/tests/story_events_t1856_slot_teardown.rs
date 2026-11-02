//! T1856: the effect slot flush `0x103A00` and its closure against emulated-original golden vectors (MEASURED).
//! Every function ran as the original machine code under Unicorn on seeded synthetic worlds (no stubs inside the
//! closure; only external arena callbacks are logged). Generator: `tools/t1846_gold/svc_slot_teardown.py`, data in
//! `tests/data/t1856/slot_teardown*.golden`. Large blocks are compared as FNV-1a hashes of 64 word chunks
//! (golden address = chunk address | 0x80000000).
mod common;
use common::{Case, parse};
use std::cell::RefCell;
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::svc_slot_teardown::*;
use tsfp_gameplay::story_events::sweep_dynamic_shape::{EffectMemory, ImpactServices};
use tsfp_gameplay::story_events::sweep_records::GuestMemory;

const ARGS: u32 = 0x1FF1000;
const MARKER: u32 = 0x1FF0000;
const BLOCK_SIZE: u32 = 0x619C;

#[derive(Default, Clone)]
struct Mem {
    words: BTreeMap<u32, u32>,
}

impl GuestMemory for Mem {
    fn read_u8(&self, address: u32) -> u8 {
        (self.words.get(&(address & !3)).copied().unwrap_or(0) >> ((address & 3) * 8)) as u8
    }
    fn read_u32(&self, address: u32) -> u32 {
        if address & 3 == 0 {
            self.words.get(&address).copied().unwrap_or(0)
        } else {
            u32::from(self.read_u16(address)) | u32::from(self.read_u16(address + 2)) << 16
        }
    }
}

impl EffectMemory for Mem {
    fn write_u8(&mut self, address: u32, value: u8) {
        let word = self.words.entry(address & !3).or_insert(0);
        let shift = (address & 3) * 8;
        *word = (*word & !(0xFF << shift)) | u32::from(value) << shift;
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        if address & 3 == 0 {
            self.words.insert(address, value);
        } else {
            for index in 0..4 {
                self.write_u8(address + index, (value >> (index * 8)) as u8);
            }
        }
    }
}

fn load(name: &str) -> Vec<Case> {
    let text = std::fs::read_to_string(format!(
        "{}/tests/data/t1856/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let cases = parse(&text);
    assert!(cases.len() >= 50, "{name}: {} cases", cases.len());
    cases
}

fn sine_words() -> Vec<u32> {
    let text = std::fs::read_to_string(format!(
        "{}/tests/data/t1856/slot_teardown_sine.golden",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let case = parse(&text).remove(0);
    (0..2048).map(|i| case.output(SINE_TABLE + 4 * i)).collect()
}

fn memory(case: &Case, with_sine: bool) -> Mem {
    let mut mem = Mem {
        words: case.ins.clone(),
    };
    if with_sine {
        for (index, word) in sine_words().into_iter().enumerate() {
            mem.words.insert(SINE_TABLE + 4 * index as u32, word);
        }
    }
    mem
}

fn arg(case: &Case, index: u32) -> u32 {
    case.input(ARGS + 4 * index)
}

fn fnv(words: impl Iterator<Item = u32>) -> u32 {
    words.fold(0x811C_9DC5u32, |hash, word| {
        (hash ^ word).wrapping_mul(0x0100_0193)
    })
}

/// Compares every golden output word (raw, or the hashed 64 word chunks of the block at `block`).
fn check_outs(label: &str, index: usize, case: &Case, mem: &Mem, block: u32) {
    let mut checked = 0;
    for (&address, &expected) in &case.outs {
        if address & 0x8000_0000 != 0 {
            let start = address & 0x7FFF_FFFF;
            let count = ((block + BLOCK_SIZE - start) / 4).min(64);
            let hash = fnv((0..count).map(|i| mem.read_u32(start + 4 * i)));
            assert_eq!(
                hash, expected,
                "{label} case {index}: block chunk {start:#x} (block {block:#x})"
            );
        } else {
            assert_eq!(
                mem.read_u32(address),
                expected,
                "{label} case {index}: word {address:#x}"
            );
        }
        checked += 1;
    }
    assert!(checked > 0, "{label} case {index}: nothing compared");
}

fn check_calls(label: &str, index: usize, case: &Case, hooks: &RecordedCallbacks) {
    let expected: Vec<(u32, u32)> = case.calls.iter().map(|c| (c.callee, c.args[0])).collect();
    assert_eq!(
        hooks.0, expected,
        "{label} case {index}: external callbacks"
    );
}

#[test]
fn sine_table_matches_original() {
    for (index, word) in sine_words().into_iter().enumerate() {
        assert_eq!(
            sine_table_entry(index as u32).to_bits(),
            word,
            "sine entry {index}"
        );
    }
}

#[test]
fn small_leaves_match_original() {
    let cases = load("slot_teardown_small");
    let mut seen = [0usize; 6];
    for (index, case) in cases.iter().enumerate() {
        let mut mem = memory(case, false);
        let mut hooks = NoHooks;
        let op = case.input(MARKER);
        seen[op as usize] += 1;
        match op {
            1 => assert_eq!(
                texture_index(&mem, arg(case, 0)),
                case.eax,
                "80F20 case {index}"
            ),
            2 => {
                let address = arg(case, 0);
                write_translation_matrix(
                    &mut mem,
                    address,
                    f32::from_bits(arg(case, 1)),
                    f32::from_bits(arg(case, 2)),
                    f32::from_bits(arg(case, 3)),
                );
                check_outs("153550", index, case, &mem, 0);
            }
            3 => {
                let value = Machine {
                    memory: &mut mem,
                    hooks: &mut hooks,
                }
                .rng_int();
                assert_eq!(value, case.eax, "153BB0 case {index}");
                check_outs("153BB0", index, case, &mem, 0);
            }
            4 => {
                let value = Machine {
                    memory: &mut mem,
                    hooks: &mut hooks,
                }
                .rng_float();
                assert_eq!(Some(value.to_bits()), case.st0, "153BD0 case {index}");
                check_outs("153BD0", index, case, &mem, 0);
            }
            5 => {
                let overlap =
                    box_overlap(&mem, arg(case, 0), arg(case, 1), arg(case, 2), arg(case, 3));
                assert_eq!(u32::from(overlap), case.eax, "47560 case {index}");
            }
            other => panic!("unknown op {other}"),
        }
    }
    assert!(seen[1..].iter().all(|&n| n >= 10), "coverage {seen:?}");
}

#[test]
fn entity_query_matches_original() {
    let cases = load("slot_teardown_query");
    let mut found = 0;
    let mut empty = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut mem = memory(case, false);
        let mut hooks = NoHooks;
        let count = Machine {
            memory: &mut mem,
            hooks: &mut hooks,
        }
        .entity_query(arg(case, 1), arg(case, 2), arg(case, 3));
        assert_eq!(count, case.eax, "query case {index} count");
        check_outs("873C0", index, case, &mem, 0);
        if count > 0 {
            found += 1;
        } else {
            empty += 1;
        }
    }
    assert!(found >= 20 && empty >= 3, "found {found} empty {empty}");
}

#[test]
fn patch_matches_original() {
    let cases = load("slot_teardown_patch");
    let (mut outer, mut mode_zero, mut textured) = (0, 0, 0);
    for (index, case) in cases.iter().enumerate() {
        let mut mem = memory(case, false);
        let mut hooks = NoHooks;
        let blk = arg(case, 0);
        let point = |pointer: u32| [case.input_f(pointer), case.input_f(pointer + 4)];
        let flag = arg(case, 8) != 0;
        outer += usize::from(flag);
        mode_zero += usize::from(case.input(blk + 0x618C) == 0);
        textured += usize::from(case.input(blk + 0x6184) != 0xFFFF_FFFF);
        Machine {
            memory: &mut mem,
            hooks: &mut hooks,
        }
        .build_patch(
            blk,
            arg(case, 1),
            arg(case, 2),
            arg(case, 3),
            point(arg(case, 4)),
            point(arg(case, 5)),
            point(arg(case, 6)),
            point(arg(case, 7)),
            flag,
        );
        check_outs("1028E0", index, case, &mem, 0);
    }
    assert!(
        outer >= 15 && mode_zero >= 10 && textured >= 10,
        "{outer} {mode_zero} {textured}"
    );
}

#[test]
fn arena_matches_original() {
    let cases = load("slot_teardown_arena");
    let mut ops = [0usize; 7];
    let mut callbacks = 0;
    for (index, case) in cases.iter().enumerate() {
        let mut mem = memory(case, false);
        let mut hooks = RecordedCallbacks::default();
        let op = case.input(MARKER);
        ops[op as usize] += 1;
        let result = {
            let mut machine = Machine {
                memory: &mut mem,
                hooks: &mut hooks,
            };
            match op {
                1 => {
                    machine.arena_alloc_mode(arg(case, 0), arg(case, 1), arg(case, 2), arg(case, 3))
                }
                2 => {
                    machine.arena_free(arg(case, 0));
                    case.eax
                }
                3 => {
                    machine.arena_free_block(arg(case, 0));
                    case.eax
                }
                4 => machine.arena_reserve(arg(case, 0)),
                5 => machine.arena_alloc(arg(case, 0), arg(case, 1), arg(case, 2)),
                6 => {
                    machine.arena_make_room(arg(case, 0));
                    case.eax
                }
                other => panic!("unknown op {other}"),
            }
        };
        assert_eq!(result, case.eax, "arena case {index} op {op} result");
        check_outs("arena", index, case, &mem, 0);
        check_calls("arena", index, case, &hooks);
        callbacks += hooks.0.len();
    }
    assert!(ops[1..].iter().all(|&n| n >= 5), "coverage {ops:?}");
    assert!(callbacks > 0, "no external callback exercised");
}

#[test]
fn pool_matches_original() {
    let cases = load("slot_teardown_pool");
    let (mut acquires, mut releases) = (0, 0);
    for (index, case) in cases.iter().enumerate() {
        let mut mem = memory(case, false);
        let mut hooks = RecordedCallbacks::default();
        let op = case.input(MARKER);
        {
            let mut machine = Machine {
                memory: &mut mem,
                hooks: &mut hooks,
            };
            if op == 2 {
                machine.pool_release(arg(case, 0));
                releases += 1;
            } else {
                let object = machine.pool_acquire(case.input(ARGS + 0x100), arg(case, 0) != 0);
                assert_eq!(object, case.eax, "pool case {index} object");
                acquires += 1;
            }
        }
        check_outs("pool", index, case, &mem, 0);
        check_calls("pool", index, case, &hooks);
    }
    assert!(acquires >= 20 && releases >= 10, "{acquires} {releases}");
}

#[derive(Default)]
struct NullImpact;

impl ImpactServices for NullImpact {
    fn disturb(&mut self, _: [f32; 3], _: f32, _: u32) {}
    fn spawn_request(&mut self, _: [u32; 12]) {}
    fn build_debris(&mut self, _: u32) {}
    fn ground_height(&mut self, _: [f32; 3], _: f32, _: f32) -> f32 {
        0.0
    }
}

#[test]
fn root_matches_original() {
    let cases = load("slot_teardown");
    let (mut built, mut failed, mut callbacks, mut with_ids) = (0, 0, 0, 0);
    for (index, case) in cases.iter().enumerate() {
        let slot = arg(case, 1);
        let mut mem = memory(case, true);
        let mut hooks = RecordedCallbacks::default();
        let object = Machine {
            memory: &mut mem,
            hooks: &mut hooks,
        }
        .spawn_debris(arg(case, 0), slot);
        assert_eq!(object, case.eax, "root case {index} return");
        let block = if object == 0 {
            0
        } else {
            mem.read_u32(object + 0x18)
        };
        check_outs("root", index, case, &mem, block);
        check_calls("root", index, case, &hooks);
        callbacks += hooks.0.len();
        if object == 0 {
            failed += 1;
        } else {
            built += 1;
            with_ids += usize::from(mem.read_u32(object + 0x98) > 0);
        }
    }
    assert!(built >= 40 && failed >= 3, "built {built} failed {failed}");
    assert!(
        callbacks >= 3 && with_ids >= 5,
        "callbacks {callbacks} ids {with_ids}"
    );
}

#[test]
fn build_debris_adapter_matches_original() {
    let cases = load("slot_teardown");
    for (index, case) in cases.iter().enumerate().take(20) {
        let slot = arg(case, 1);
        let shared = RefCell::new(memory(case, true));
        let mut services = TeardownImpactServices {
            memory: &shared,
            inner: NullImpact,
            hooks: RecordedCallbacks::default(),
        };
        services.build_debris(slot);
        let mem = shared.borrow();
        let object = case.eax;
        let block = if object == 0 {
            0
        } else {
            mem.read_u32(object + 0x18)
        };
        check_outs("adapter", index, case, &mem, block);
        check_calls("adapter", index, case, &services.hooks);
    }
}
