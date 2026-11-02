// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential for the kind-33 lifted predicate contract.
//! WEAK EVIDENCE: fabricated deterministic inputs; not an owner-recording replay.

extern crate alloc;

#[path = "../src/port_scaffolds/story_kind_33_table_field_predicate_t2485.rs"]
mod generated_port;
use generated_port::PortState;
use std::collections::BTreeMap;
use tsfp_ports::abi::{ExtRegisters, GuestCall};
use tsfp_ports::memory::GuestMemory;
use tsfp_ports::story_event_condition_lane::{self, ConditionEvaluator, PlayerIdVariant};

#[unsafe(no_mangle)]
extern "C" fn xbox_ReadTimeStampCounter() -> u64 {
    0
}

const RECORD: u32 = 0x4000;
const ENTRY: u32 = 0x5000;

#[derive(Default)]
struct Memory(BTreeMap<u32, u8>);
impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }
    fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| self.read8(address + i as u32)))
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
    fn write32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address + offset as u32, byte);
        }
    }
}

struct Lookup {
    entry: u32,
}
impl GuestCall for Lookup {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        assert_eq!(address, 0xB7600, "kind-33 lifted table lookup helper");
        regs.eax = self.entry;
    }
}

fn lifted_body(state: &PortState) -> Option<bool> {
    let record = state.conditionrecord.first()?;
    let entry = state.storyconditiontableentry.as_ref()?;
    if record.kind != 33 || entry.key != record.args[0] {
        return None;
    }
    let mut memory = Memory::default();
    memory.write32(RECORD, record.kind);
    memory.write32(RECORD + 4, record.args[0] as u32);
    memory.write32(ENTRY + 0x18, entry.value_0x18);
    // The lifted switch reads its target from the guest jump table; seed the
    // kind-33 arm identified in the retained body.
    memory.write32(0x235f1c + record.kind * 4, 0x235eb4);
    let mut regs: ExtRegisters = unsafe { std::mem::zeroed() };
    regs.esp = 0x9000;
    regs.edi = RECORD;
    let ran = tsfp_ports::c2r::run(
        0x2359A0,
        &mut memory,
        &mut Lookup { entry: ENTRY },
        &mut regs,
    );
    assert!(ran, "retained lifted body is registered at 0x2359A0");
    Some(regs.eax != 0)
}

fn rust_port(state: &PortState) -> Option<bool> {
    let record = state.conditionrecord.first()?;
    let entry = state.storyconditiontableentry.as_ref()?;
    let mut facts = Facts(entry);
    story_event_condition_lane::evaluate(
        story_event_condition_lane::ConditionRecord {
            kind: record.kind,
            args: record.args,
        },
        &mut facts,
    )
}

struct Facts<'a>(&'a generated_port::StoryConditionTableEntry);
impl ConditionEvaluator for Facts<'_> {
    fn player_count(&self) -> usize {
        0
    }
    fn player_passes(&mut self, _: usize, _: PlayerIdVariant, _: [i32; 4]) -> bool {
        false
    }
    fn first_player_ext_c08_nonzero(&mut self) -> bool {
        false
    }
    fn slot_status(&self, _: u32) -> Option<u8> {
        None
    }
    fn tag_exists(&self, _: i32) -> bool {
        false
    }
    fn tagged_object_child_state(&self, _: i32) -> Option<u32> {
        None
    }
    fn condition_table_field_0x18(&self, key: i32) -> Option<u32> {
        (key == self.0.key).then_some(self.0.value_0x18)
    }
}

#[test]
fn tier1_lifted_differential_3000_seeds_weak() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..3000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let key = ((seed >> 17) as i32) % 17;
        let entry_key = if seed & 4 == 0 {
            key
        } else {
            key.wrapping_add(1)
        };
        let entry = (seed & 2 != 0).then_some(generated_port::StoryConditionTableEntry {
            key: entry_key,
            value_0x18: (seed >> 29) as u32,
        });
        let input = PortState {
            conditionrecord: vec![generated_port::ConditionRecord {
                kind: 33,
                args: [key, 0, 0, 0],
            }],
            storyconditiontableentry: entry,
            result: None,
        };
        assert_eq!(rust_port(&input), lifted_body(&input), "seed={seed:#018x}");
        let mut scaffold_state = input.clone();
        generated_port::port_step(&mut scaffold_state);
        assert_eq!(
            scaffold_state.result,
            lifted_body(&input),
            "scaffold seed={seed:#018x}"
        );
    }
}

#[test]
fn kind_33_declines_wrong_kind_and_unavailable_lookup() {
    let record = generated_port::ConditionRecord {
        kind: 32,
        args: [1, 0, 0, 0],
    };
    let entry = generated_port::StoryConditionTableEntry {
        key: 1,
        value_0x18: 1,
    };
    assert_eq!(generated_port::evaluate_key(&record, Some(&entry)), None);
    let record = generated_port::ConditionRecord {
        kind: 33,
        args: [1, 0, 0, 0],
    };
    assert_eq!(generated_port::evaluate_key(&record, None), None);
}
