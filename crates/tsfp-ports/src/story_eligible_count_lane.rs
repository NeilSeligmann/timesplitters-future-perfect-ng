// SPDX-License-Identifier: GPL-3.0-or-later
//! T2453 typed adapter for the Story link-kind-11 eligible/observed count at `0x25F370`.
//!
//! The reducer lives in `tsfp-gameplay::story_events::eligible_count`; this adapter
//! snapshots the neighboring setup query (`0x6C890`) and live actor facts into typed
//! inputs. Selection is opt-in with `TSFP_STORY_ELIGIBLE_COUNT=rust`.

use alloc::vec::Vec;

#[cfg(test)]
use crate::story_eligible_count::StoryEligibleCount;
use crate::story_eligible_count::{
    EligibleSetupRow, LiveActorFacts, StoryEligibleCountInput, story_eligible_count,
};
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::rt::RegsExt,
    memory::GuestMemory,
};

const ROOT: u32 = 0x25f370;
const COUNT_TABLE: u32 = 0x7bc5c0;
const COUNT_STRIDE: u32 = 4;
const SETUP_STRIDE: u32 = 0x98;
const OBJECT_TABLE: u32 = 0x7a3580;
const OBJECT_STRIDE: u32 = 4;

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

#[cfg(not(test))]
fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_ELIGIBLE_COUNT".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

fn setup_is_eligible<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    registers: &mut ExtRegisters,
    setup: u32,
) -> bool {
    let saved = *registers;
    // The neighboring contract remains the source of setup eligibility. The caller
    // frame arguments match 0x25F370's `0x6C890(slot+0x0c, slot+0x2c)` call.
    registers.push(m, m.read32(setup.wrapping_add(0x2c)));
    registers.push(m, m.read32(setup.wrapping_add(0x0c)));
    registers.call(guest, 0x6c890);
    let result = registers.eax != 0;
    registers.esp = registers.esp.wrapping_add(8);
    *registers = saved;
    result
}

fn snapshot<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    registers: &mut ExtRegisters,
    key: u32,
) -> (Vec<EligibleSetupRow>, Vec<LiveActorFacts>, bool) {
    let state = m.read32(0x74c36c);
    let count = m.read32(state.wrapping_add(0x78));
    let setup_table = m.read32(state.wrapping_add(0x74));
    let fallback_step = m.read32(0x7a2820);
    let mut fallback_cursor = 0u32;
    let mut setup_rows = Vec::new();
    for index in 0..count {
        if m.read32(COUNT_TABLE.wrapping_add(index.wrapping_mul(COUNT_STRIDE))) == key {
            fallback_cursor = fallback_cursor.wrapping_add(fallback_step);
            let setup = setup_table.wrapping_add(index.wrapping_mul(SETUP_STRIDE));
            let eligible = setup_is_eligible(m, guest, registers, setup);
            let configured_count = m.read32(setup.wrapping_add(0x60));
            let fallback_count = if eligible && configured_count == 0x7fff_ffff {
                m.read32(fallback_cursor)
            } else {
                0
            };
            setup_rows.push(EligibleSetupRow {
                eligible,
                configured_count,
                fallback_count,
                paired_count: m.read32(fallback_cursor.wrapping_add(8)),
            });
        }
        // Lifted loop advances its paired-table cursor by 0x18 for every scanned
        // row, independently of key match / eligibility.
        fallback_cursor = fallback_cursor.wrapping_add(0x18);
    }

    // Match the lifted getter's RETIRE semantics as well as its guest address.
    let mode = crate::state_wire::get_mode(m) == 10;
    let first = crate::state_wire::get_players(m);
    let end = m.read32(0x74c37c);
    let mut live_actors = Vec::new();
    for index in first..end {
        let object = m.read32(OBJECT_TABLE.wrapping_add(index.wrapping_mul(OBJECT_STRIDE)));
        let data = m.read32(object.wrapping_add(0xbf0));
        if m.read32(data.wrapping_add(0x28)) != key {
            continue;
        }
        live_actors.push(LiveActorFacts {
            mode10_first: m.read32(object.wrapping_add(0x10)),
            mode10_second: m.read32(object.wrapping_add(0x14)),
            other_first: m.read32(object.wrapping_add(0x18)),
            other_second: m.read32(object.wrapping_add(0x1c)),
            flags: m.read8(object.wrapping_add(0x24)),
        });
    }
    (setup_rows, live_actors, mode)
}

/// Intercepts only the 0x25F370 count root; returns false for the lifted path.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    registers: &mut ExtRegisters,
) -> bool {
    if va != ROOT || !enabled() {
        return false;
    }
    let key = m.read32(registers.esp.wrapping_add(4));
    let out = m.read32(registers.esp.wrapping_add(8));
    let (setup_rows, live_actors, mode10) = snapshot(m, guest, registers, key);
    let result = story_eligible_count(StoryEligibleCountInput {
        mode10,
        setup_rows: &setup_rows,
        live_actors: &live_actors,
    });
    m.write32(out, result.target);
    registers.eax = result.total;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::story_eligible_count::StoryEligibleCount;
    use std::collections::{BTreeMap, VecDeque};

    struct Ram(BTreeMap<u32, u8>);
    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    fn put32(memory: &mut Ram, at: u32, value: u32) {
        for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
            memory.write8(at.wrapping_add(index as u32), byte);
        }
    }
    struct Eligibility(VecDeque<bool>);
    impl GuestCall for Eligibility {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            assert_eq!(va, 0x6c890);
            regs.eax = u32::from(self.0.pop_front().expect("eligibility fixture"));
        }
    }
    fn fixture(seed: &mut u32, eligibilities: &[bool]) -> Ram {
        let mut memory = Ram(BTreeMap::new());
        let key = 0x4f4d_4248;
        let count = eligibilities.len() as u32;
        put32(&mut memory, 0x74c36c, 0x10000);
        put32(&mut memory, 0x10074, 0x20000);
        put32(&mut memory, 0x10078, count);
        put32(&mut memory, 0x7a2820, 4);
        put32(&mut memory, 0x790950, 0);
        put32(&mut memory, 0x74c37c, 4);
        memory.write8(0x7de455, 10);
        for (index, eligible) in eligibilities.iter().copied().enumerate() {
            let i = index as u32;
            let setup = 0x20000 + i * SETUP_STRIDE;
            put32(&mut memory, COUNT_TABLE + i * COUNT_STRIDE, key);
            put32(&mut memory, setup + 0x0c, u32::from(eligible) << 18);
            put32(&mut memory, setup + 0x2c, 0);
            let amount = if i % 5 == 0 { 0x7fff_ffff } else { (i + 1) % 7 };
            put32(&mut memory, setup + 0x60, amount);
            put32(&mut memory, (i + 1) * 4, (i % 3) + 1);
        }
        for index in 0..4u32 {
            *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let object = 0x30000 + index * 0x1000;
            let data = object + 0x800;
            put32(&mut memory, OBJECT_TABLE + index * 4, object);
            put32(&mut memory, object + 0xbf0, data);
            put32(
                &mut memory,
                data + 0x28,
                if index == 3 { key ^ 1 } else { key },
            );
            put32(&mut memory, object + 0x10, *seed & 1);
            put32(&mut memory, object + 0x14, (*seed >> 1) & 1);
            put32(&mut memory, object + 0x18, (*seed >> 2) & 1);
            put32(&mut memory, object + 0x1c, (*seed >> 3) & 1);
            memory.write8(object + 0x24, (*seed >> 8) as u8);
        }
        memory
    }
    fn registers() -> ExtRegisters {
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x8000;
        regs
    }

    #[test]
    fn switch_is_explicit_and_default_off() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }

    #[test]
    fn reducer_matches_lifted_aggregate_over_3000_fabricated_cases() {
        // Other state-wire tests may leave the process-wide owner in audit/own mode.
        // This oracle fixture compares the lifted guest path, so make that contract explicit.
        crate::state_wire::RETIRE.reset();
        crate::state_wire::RETIRE.set_mode(crate::state_wire::game_state::RETIRE_OFF);
        let mut seed = 0x2453_17u32;
        for case in 0..3000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let n = (seed & 7) + 1;
            let eligibility: Vec<bool> = (0..n)
                .map(|i| ((seed.rotate_left(i) >> (i & 15)) & 1) != 0)
                .collect();
            let mut rust_seed = seed;
            let mut lifted_seed = seed;
            let mut rust_memory = fixture(&mut rust_seed, &eligibility);
            let mut lifted_memory = fixture(&mut lifted_seed, &eligibility);
            let mut rust_regs = registers();
            let mut lifted_regs = registers();
            put32(&mut rust_memory, 0x8004, 0x4f4d_4248);
            put32(&mut rust_memory, 0x8008, 0x9000);
            put32(&mut lifted_memory, 0x8004, 0x4f4d_4248);
            put32(&mut lifted_memory, 0x8008, 0x9000);
            let mut rust_eligibility = Eligibility(eligibility.iter().copied().collect());
            let mut lifted_eligibility = Eligibility(eligibility.iter().copied().collect());
            assert!(run(
                ROOT,
                &mut rust_memory,
                &mut rust_eligibility,
                &mut rust_regs
            ));
            // Keep the game-state owner in its default pass-through mode; the fixture's
            // guest mode byte selects the mode-10 actor fields in the lifted body.
            crate::c2r::t2049_exec_001::game_slot_node_totals_for_key_with_object_count(
                &mut lifted_memory,
                &mut lifted_eligibility,
                &mut lifted_regs,
            );
            assert_eq!(
                rust_regs.eax,
                lifted_regs.eax,
                "case {case}, target Rust/lifted={}/{}, eligible={eligibility:?}, rust seed={rust_seed:#x}, lifted seed={lifted_seed:#x}, lifted-live={}, lifted-paired={}, mode={}, actors={:?}",
                rust_memory.read32(0x9000),
                lifted_memory.read32(0x9000),
                lifted_memory.read32(0x7fec),
                lifted_memory.read32(0x7ff0),
                crate::state_wire::get_mode(&rust_memory),
                (0..4u32)
                    .map(|i| {
                        let o = rust_memory.read32(OBJECT_TABLE + i * 4);
                        let d = rust_memory.read32(o + 0xbf0);
                        (
                            i,
                            rust_memory.read32(d + 0x28),
                            rust_memory.read32(o + 0x10),
                            rust_memory.read32(o + 0x18),
                            rust_memory.read8(o + 0x24),
                        )
                    })
                    .collect::<Vec<_>>(),
            );
            assert_eq!(
                rust_memory.read32(0x9000),
                lifted_memory.read32(0x9000),
                "case {case}"
            );
        }
    }

    #[test]
    fn target_mutation_changes_link_kind_11_satisfaction() {
        let setup = [EligibleSetupRow {
            eligible: true,
            configured_count: 1,
            fallback_count: 0,
            paired_count: 0,
        }];
        let actors = [LiveActorFacts {
            mode10_first: 1,
            ..Default::default()
        }];
        let correct = story_eligible_count(StoryEligibleCountInput {
            mode10: true,
            setup_rows: &setup,
            live_actors: &actors,
        });
        let mutated = StoryEligibleCount {
            target: correct.target + 1,
            total: correct.total,
        };
        assert_eq!(correct.total, correct.target);
        assert_ne!(mutated.total, mutated.target);
    }
}
