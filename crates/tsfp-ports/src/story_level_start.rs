// SPDX-License-Identifier: GPL-3.0-or-later
//! T2462: readable Story level-start state transition for the `0x228180` boundary.
//!
//! This module deliberately models only effects with an explicit typed contract. Descriptor
//! initialization, arena allocation, event-pool ownership, and non-Story dispatch remain named
//! neighbor responsibilities; the runtime adapter must keep those paths on the lifted body.

pub const STORY_MODE: u8 = 10;
pub const ADMISSION_SKIP_LEVEL: u32 = 0x64;
pub const LEVEL_START_ROOT: u32 = 0x228180;

fn write16<M: crate::memory::GuestMemory>(memory: &mut M, address: u32, value: u16) {
    let [lo, hi] = value.to_le_bytes();
    memory.write8(address, lo);
    memory.write8(address.wrapping_add(1), hi);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoryLevelStartInput {
    pub mode: u8,
    pub level_id: u32,
    /// Number of fixed 0x4c-byte slot records allocated for this level.
    pub allocated_slots: u32,
    /// Number of records populated by the selected level table.
    pub table_slots: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoryLevelStartEffects {
    pub reset_slots: u32,
    pub clear_objective_counters: bool,
    pub register_story_table: bool,
    pub invoke_objective_admission: bool,
}

/// Per-record state owned by the Story level-start port. The lifted `rep stosd` clears 19
/// dwords at each 0x4c-byte record; the three words marked separately below are subsequently
/// initialized to the guest sentinels seen in the lifted body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorySlotReset {
    pub words: [u32; 19],
    pub status: u8,
    pub active_mask: u16,
    pub dependencies: [u32; 5],
}

impl StorySlotReset {
    pub const fn cleared() -> Self {
        Self {
            words: [0; 19],
            status: 0,
            active_mask: 0,
            dependencies: [u32::MAX; 5],
        }
    }
}

/// Keep the native Story branch explicitly opt-in until its replay adoption gate passes.
/// `None`, unset, and any value other than `rust` preserve the lifted path.
pub const fn selector_enabled(value: Option<&[u8]>) -> bool {
    matches!(value, Some(b"rust"))
}

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

#[cfg(not(test))]
fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_LEVEL_START".as_ptr());
        !value.is_null() && selector_enabled(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}

#[cfg(test)]
fn enabled() -> bool {
    true
}

fn reset_slot_record<M: crate::memory::GuestMemory>(memory: &mut M, slot: u32) {
    let reset = StorySlotReset::cleared();
    for (word, value) in reset.words.iter().enumerate() {
        memory.write32(slot.wrapping_add((word as u32) * 4), *value);
    }
    memory.write8(slot.wrapping_add(0xb), reset.status);
    memory.write8(slot.wrapping_add(0x48), 0xff);
    write16(memory, slot.wrapping_add(0x24), reset.active_mask);
    write16(memory, slot.wrapping_add(0x26), reset.active_mask);
    write16(memory, slot.wrapping_add(0x28), 0xffff);
    write16(memory, slot.wrapping_add(2), 0);
    write16(memory, slot.wrapping_add(4), 0);
    write16(memory, slot.wrapping_add(6), 0);
    let links = memory.read32(slot.wrapping_add(0xc));
    if links != 0 {
        memory.write32(links, u32::MAX);
        for link in 0..10u32 {
            memory.write32(links.wrapping_add(0x2c + link * 4), u32::MAX);
            memory.write32(links.wrapping_add(0x30 + link * 4), 0);
            memory.write32(links.wrapping_add(0x70 + link * 4), 0);
        }
    }
    for field in 0..5u32 {
        memory.write32(
            slot.wrapping_add(0x10 + field * 4),
            reset.dependencies[field as usize],
        );
    }
}

/// Story-only default-off adapter for the measured reset/admission portion of `0x228180`.
/// Table lookup, registration, descriptor reset, and entry callbacks remain guest calls at their
/// original seams; the fixed slot and counter writes are native and driven by the typed plan.
pub fn run<M: crate::memory::GuestMemory, C: crate::abi::GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut crate::abi::ExtRegisters,
) -> bool {
    use crate::abi::call_cdecl;
    use crate::memory::GuestMemory as _;

    if !enabled() || va != LEVEL_START_ROOT || memory.read8(0x7de455) != STORY_MODE {
        return false;
    }

    let level_id = memory.read32(0x79094c);
    call_cdecl(guest, memory, regs, 0x6a850, &[level_id]);
    let table = regs.eax;
    call_cdecl(guest, memory, regs, 0x233f90, &[]);
    let count = regs.eax as i32;
    let slots = count.saturating_add(0x2d).max(0) as u32;
    let effects = plan_story_level_start(StoryLevelStartInput {
        mode: STORY_MODE,
        level_id,
        allocated_slots: slots,
        table_slots: if table == 0 { 0 } else { slots },
    })
    .expect("Story mode checked above");

    memory.write32(0x78cec8, 0);
    memory.write32(0x78ced0, u32::MAX);
    memory.write32(0x75c148, 0);
    memory.write32(0x75c150, 0);
    let slot_table = memory.read32(0x78cecc);
    let mut index = 0u32;
    if effects.reset_slots != 0 {
        loop {
            reset_slot_record(memory, slot_table.wrapping_add(index.wrapping_mul(0x4c)));
            index = index.wrapping_add(1);
            call_cdecl(guest, memory, regs, 0x233f90, &[]);
            let next_slots = (regs.eax as i32).saturating_add(0x2d).max(0) as u32;
            if index >= next_slots {
                break;
            }
        }
    }
    for address in [0x75c134, 0x75c138, 0x75c13c] {
        memory.write32(address, 0);
    }
    if effects.invoke_objective_admission {
        call_cdecl(guest, memory, regs, 0x227e90, &[]);
    }
    if table != 0 {
        let callback = memory.read32(table.wrapping_add(0x108));
        if callback != 0 {
            guest.call(callback, regs);
        }
    }

    // The mode descriptor reset is an adjacent guest-owned contract. Preserve its boundary.
    let flags = crate::state_wire::get_flags(memory);
    if (flags as u8) & 0x40 != 0 || flags & 0x0800_0000 != 0 {
        call_cdecl(guest, memory, regs, 0x22ceb0, &[]);
    }
    true
}

/// Plan the Story-owned portion of the level-start transition.
///
/// Level `0x64` is the observed admission skip sentinel. A non-Story mode has no Story effects.
/// The caller retains lifted behavior for any memory shape it cannot represent.
pub fn plan_story_level_start(input: StoryLevelStartInput) -> Option<StoryLevelStartEffects> {
    if input.mode != STORY_MODE {
        return None;
    }
    Some(StoryLevelStartEffects {
        reset_slots: input.allocated_slots,
        clear_objective_counters: true,
        register_story_table: input.table_slots != 0,
        invoke_objective_admission: input.level_id != ADMISSION_SKIP_LEVEL,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        abi::{ExtRegisters, Fpu, GuestCall, Xmm},
        memory::GuestMemory,
    };
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    #[derive(Clone, Default)]
    struct Ram(Rc<RefCell<BTreeMap<u32, u8>>>);
    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            *self.0.borrow().get(&address).unwrap_or(&0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.borrow_mut().insert(address, value);
        }
    }

    #[derive(Clone)]
    struct Stubs {
        memory: Ram,
        count: u32,
        calls: Vec<u32>,
    }
    impl GuestCall for Stubs {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            let caller_esp = regs.esp;
            regs.esp = caller_esp.wrapping_sub(4);
            self.memory.write32(regs.esp, 0xfeed_cafe);
            self.calls.push(va);
            match va {
                0x6a850 => regs.eax = 0x50000,
                0x233f90 => regs.eax = self.count,
                0x227e90 | 0x22ceb0 => {}
                _ => panic!("unexpected Story level-start call {va:#x}"),
            }
            regs.esp = caller_esp;
        }
    }

    fn fixture(level_id: u32, count: u32, seed: u32) -> (Ram, Stubs, ExtRegisters) {
        let mut memory = Ram::default();
        for (address, value) in [
            (0x78cecc, 0x30000),
            (0x7de455, STORY_MODE as u32),
            (0x79094c, level_id),
            (0x7de458, 0),
        ] {
            memory.write32(address, value);
        }
        let slot_count = count.saturating_add(0x2d).min(0x70);
        for offset in 0..slot_count * 0x4c {
            memory.write8(
                0x30000 + offset,
                (seed.wrapping_add(offset * 31) >> 13) as u8,
            );
        }
        let regs = ExtRegisters {
            eax: 0x1111,
            ecx: 0x2222,
            edx: 0x3333,
            esp: 0x7000,
            ebx: 0x4444,
            ebp: 0x5555,
            esi: 0x6666,
            edi: 0x7777,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        };
        (
            memory.clone(),
            Stubs {
                memory,
                count,
                calls: Vec::new(),
            },
            regs,
        )
    }

    fn compare_run(level_id: u32, count: u32, seed: u32) {
        let (mut rust_memory, mut rust_calls, mut rust_regs) = fixture(level_id, count, seed);
        let (mut lifted_memory, mut lifted_calls, mut lifted_regs) = fixture(level_id, count, seed);
        assert!(run(
            LEVEL_START_ROOT,
            &mut rust_memory,
            &mut rust_calls,
            &mut rust_regs
        ));
        crate::c2r::data_missions::game_slot_4c_table_reset_and_register_from_mode_table(
            &mut lifted_memory,
            &mut lifted_calls,
            &mut lifted_regs,
        );
        for address in [
            0x78cec8, 0x78cecc, 0x78ced0, 0x75c134, 0x75c138, 0x75c13c, 0x75c148, 0x75c150,
        ] {
            assert_eq!(
                rust_memory.read32(address),
                lifted_memory.read32(address),
                "{address:#x}"
            );
        }
        let slots = count.saturating_add(0x2d).min(0x70);
        for offset in 0..slots * 0x4c {
            assert_eq!(
                rust_memory.read8(0x30000 + offset),
                lifted_memory.read8(0x30000 + offset),
                "slot byte {offset:#x}"
            );
        }
        assert_eq!(rust_calls.calls, lifted_calls.calls);
    }

    #[test]
    fn tier1_seeded_native_adapter_matches_lifted_body() {
        let mut state = 0x2462_2281u32;
        for _ in 0..3000 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let level = if state & 0x1f == 0 {
                ADMISSION_SKIP_LEVEL
            } else {
                state % 0x100
            };
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            compare_run(level, state % 0x24, state);
        }
    }

    #[test]
    fn mutations_to_mode_guard_and_admission_skip_are_detected() {
        let story = StoryLevelStartInput {
            mode: STORY_MODE,
            level_id: 7,
            allocated_slots: 3,
            table_slots: 2,
        };
        let non_story = StoryLevelStartInput {
            mode: STORY_MODE + 1,
            ..story
        };
        let skip = StoryLevelStartInput {
            level_id: ADMISSION_SKIP_LEVEL,
            ..story
        };
        assert!(
            plan_story_level_start(story)
                .unwrap()
                .clear_objective_counters
        );
        assert_eq!(plan_story_level_start(non_story), None);
        assert!(
            !plan_story_level_start(skip)
                .unwrap()
                .invoke_objective_admission
        );
    }

    #[test]
    fn reset_state_uses_measured_guest_sentinels() {
        let reset = StorySlotReset::cleared();
        assert_eq!(reset.words, [0; 19]);
        assert_eq!(reset.status, 0);
        assert_eq!(reset.active_mask, 0);
        assert_eq!(reset.dependencies, [u32::MAX; 5]);
    }

    #[test]
    fn selector_is_default_off_and_requires_exact_opt_in() {
        assert!(!selector_enabled(None));
        assert!(!selector_enabled(Some(b"")));
        assert!(!selector_enabled(Some(b"Rust")));
        assert!(selector_enabled(Some(b"rust")));
    }
}
