// SPDX-License-Identifier: GPL-3.0-or-later
//! T2452: typed endpoint for Story event link mode 9's slot-status query.
//!
//! The retained `0x235240` callback owns link dispatch and calls `0x2273C0` for mode 9.
//! This adapter replaces only that query. Event instances, links and their fixed-tick
//! `+0x38` delay remain separate typed state in `tsfp-gameplay::story_events::Engine`.
use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::rt::RegsExt,
    memory::GuestMemory,
};

const SLOT_TABLE: u32 = 0x78cecc;
const SLOT_STRIDE: u32 = 0x4c;
const STATUS_OFFSET: u32 = 0x0b;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotStatus(u8);

impl SlotStatus {
    pub const COMPLETE: Self = Self(5);

    pub const fn from_raw(value: u8) -> Self {
        Self(value)
    }
    pub const fn raw(self) -> u8 {
        self.0
    }
    pub const fn is_complete(self) -> bool {
        self.0 == Self::COMPLETE.0
    }
}

/// The only link predicate routed through this guest query is mode 9: slot status exactly 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotStatusLink {
    pub slot_id: u32,
}

pub trait SlotStatusQuery {
    fn status(&self, slot_id: u32) -> SlotStatus;
}

pub fn link_satisfied(link: SlotStatusLink, slots: &impl SlotStatusQuery) -> bool {
    slots.status(link.slot_id).is_complete()
}

struct GuestSlots<'a, M> {
    memory: &'a M,
    base: u32,
}
impl<M: GuestMemory> SlotStatusQuery for GuestSlots<'_, M> {
    fn status(&self, slot_id: u32) -> SlotStatus {
        let address = self
            .base
            .wrapping_add(slot_id.wrapping_mul(SLOT_STRIDE))
            .wrapping_add(STATUS_OFFSET);
        SlotStatus::from_raw(self.memory.read8(address))
    }
}

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
        let value = getenv(c"TSFP_STORY_EVENT_LINK".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Handles lifted query root `0x2273C0`; returns false for the default lifted path.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    _guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if va != 0x2273c0 || !enabled() {
        return false;
    }
    let slot_id = m.read32(r.esp.wrapping_add(4));
    r.push(m, r.ebx);
    let slots = m.read32(SLOT_TABLE);
    let link = SlotStatusLink { slot_id };
    let result = link_satisfied(
        link,
        &GuestSlots {
            memory: m,
            base: slots,
        },
    );
    // Match the lifted root's observable result and scratch-register contract.
    r.edx = slots;
    r.ecx = u32::from(result);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.eax = r.ecx;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Ram(BTreeMap<u32, u8>);
    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    struct NoCalls;
    impl GuestCall for NoCalls {
        fn call(&mut self, _: u32, _: &mut ExtRegisters) {
            panic!("unexpected call")
        }
    }

    fn put32(memory: &mut Ram, at: u32, value: u32) {
        for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
            memory.write8(at.wrapping_add(index as u32), byte);
        }
    }
    fn fixture(index: u32, status: u8) -> Ram {
        let mut memory = Ram(BTreeMap::new());
        put32(&mut memory, SLOT_TABLE, 0x30000);
        put32(&mut memory, 0x7004, index);
        memory.write8(
            0x30000u32
                .wrapping_add(index.wrapping_mul(SLOT_STRIDE))
                .wrapping_add(STATUS_OFFSET),
            status,
        );
        memory
    }
    fn registers() -> ExtRegisters {
        let mut registers: ExtRegisters = unsafe { core::mem::zeroed() };
        registers.esp = 0x7000;
        registers.ebx = 0x1234_5678;
        registers
    }

    #[test]
    fn switch_is_explicit_and_default_off() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }

    #[test]
    fn typed_query_matches_lifted_body_over_3000_fabricated_cases() {
        let mut seed = 0x2452_17u32;
        for _ in 0..3000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let index = (seed >> 8) & 0x3f;
            let status = (seed >> 16) as u8;
            let mut memory = fixture(index, status);
            let mut lifted_memory = fixture(index, status);
            let mut rust_regs = registers();
            let mut lifted_regs = registers();
            assert!(run(0x2273c0, &mut memory, &mut NoCalls, &mut rust_regs));
            crate::c2r::t2049_exec_002::game_is_slot_4c_status_5_by_index(
                &mut lifted_memory,
                &mut lifted_regs,
            );
            assert_eq!(rust_regs.eax, lifted_regs.eax);
            assert_eq!(rust_regs.ecx, lifted_regs.ecx);
            assert_eq!(rust_regs.edx, lifted_regs.edx);
            assert_eq!(rust_regs.esp, lifted_regs.esp);
            assert_eq!(rust_regs.ebx, lifted_regs.ebx);
            assert_eq!(memory.0, lifted_memory.0);
        }
    }

    #[test]
    fn status_mutation_breaks_the_link_predicate() {
        let mut memory = fixture(3, 5);
        let link = SlotStatusLink { slot_id: 3 };
        {
            let base = memory.read32(SLOT_TABLE);
            assert!(link_satisfied(
                link,
                &GuestSlots {
                    memory: &memory,
                    base
                }
            ));
        }
        memory.write8(0x30000 + 3 * SLOT_STRIDE + STATUS_OFFSET, 4);
        let base = memory.read32(SLOT_TABLE);
        assert!(!link_satisfied(
            link,
            &GuestSlots {
                memory: &memory,
                base
            }
        ));
    }

    #[test]
    fn link_only_fires_on_exact_status_five() {
        for status in 0..=u8::MAX {
            assert_eq!(SlotStatus::from_raw(status).is_complete(), status == 5);
        }
    }
}
