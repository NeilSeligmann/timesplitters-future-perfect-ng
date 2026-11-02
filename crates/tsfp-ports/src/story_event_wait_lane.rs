// SPDX-License-Identifier: GPL-3.0-or-later
//! T2458: typed next-pass resume for Story event links.
//!
//! The gameplay engine owns the full typed event lifecycle. This hybrid boundary ports only
//! `0x2351D0`: mode-2 links resume after the preceding event ran; mode-3 links resume only
//! after it completed. Verb dispatch and generic delay countdown remain at lifted boundaries.

use crate::{
    abi::{ExtRegisters, GuestCall},
    c2r::rt::RegsExt,
    memory::GuestMemory,
};

const EVENT_TABLE: u32 = 0x78cd90;
const EVENT_STRIDE: u32 = 0x48;
const MODE_A: u32 = 0x40;
const MODE_B: u32 = 0x64;
const RESUME_A: u32 = 8;
const RESUME_B: u32 = 0x14;

fn write16<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    let [lo, hi] = value.to_le_bytes();
    m.write8(address, lo);
    m.write8(address.wrapping_add(1), hi);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkTargets {
    pub mode_a: u32,
    pub mode_b: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventWaitState {
    pub running: u8,
    pub fired: bool,
    pub delay_bits: u32,
}

/// The `0x2351D0` resume rule. A running pass sets A/B mode 2 on any run and mode 3 on done.
pub fn resume_words(targets: LinkTargets, previous_done: bool) -> (bool, bool) {
    let a = targets.mode_a == 2 || (targets.mode_a == 3 && previous_done);
    let b = targets
        .mode_b
        .is_some_and(|mode| mode == 2 || (mode == 3 && previous_done));
    (a, b)
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
        let value = getenv(c"TSFP_STORY_EVENT_WAIT".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Handles lifted root `0x2351D0`; returns false when the default-off A/B lane is not selected.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    _guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if va != 0x2351d0 || !enabled() {
        return false;
    }
    r.push(m, r.esi);
    let event_table = m.read32(EVENT_TABLE);
    let previous = r.eax.wrapping_sub(event_table) / 0x38;
    let count = m.read32(0x75cca8).wrapping_sub(1);
    if (previous as i32) < (count as i32) {
        let next = m
            .read32(EVENT_TABLE)
            .wrapping_add(previous.wrapping_add(1).wrapping_mul(EVENT_STRIDE));
        let definition = m.read32(next);
        let (resume_a, resume_b) = resume_words(
            LinkTargets {
                mode_a: m.read32(definition.wrapping_add(MODE_A)),
                mode_b: (m.read32(definition.wrapping_add(0x60)) != 0)
                    .then(|| m.read32(definition.wrapping_add(MODE_B))),
            },
            r.edi != 0,
        );
        if resume_a {
            write16(m, next.wrapping_add(RESUME_A), 1);
        }
        if resume_b {
            write16(m, next.wrapping_add(RESUME_B), 1);
        }
    }
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Clone, Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    #[derive(Default)]
    struct NoCalls;
    impl GuestCall for NoCalls {
        fn call(&mut self, _va: u32, _regs: &mut ExtRegisters) {}
    }

    fn put32(m: &mut Memory, address: u32, value: u32) {
        m.write32(address, value);
    }

    #[test]
    fn mode_two_resumes_on_run_and_mode_three_waits_for_done() {
        assert_eq!(
            resume_words(
                LinkTargets {
                    mode_a: 2,
                    mode_b: Some(3)
                },
                false
            ),
            (true, false)
        );
        assert_eq!(
            resume_words(
                LinkTargets {
                    mode_a: 3,
                    mode_b: Some(2)
                },
                true
            ),
            (true, true)
        );
        assert_eq!(
            resume_words(
                LinkTargets {
                    mode_a: 0,
                    mode_b: None
                },
                true
            ),
            (false, false)
        );
    }

    #[test]
    fn fabricated_tier_one_resume_cases_match_lifted_body_and_mutation_guard() {
        for i in 0..3_000u32 {
            let event_base = 0x20000;
            let definition = 0x40000;
            let next = event_base + EVENT_STRIDE;
            let a = i % 4;
            let b = (i / 4) % 4;
            let has_b = i & 2 != 0;
            let done = i & 1 != 0;
            let mut lifted = Memory::default();
            put32(&mut lifted, EVENT_TABLE, event_base);
            put32(&mut lifted, 0x75cca8, 2);
            put32(&mut lifted, next, definition);
            put32(&mut lifted, definition + MODE_A, a);
            put32(&mut lifted, definition + 0x60, u32::from(has_b));
            put32(&mut lifted, definition + MODE_B, b);
            let mut ported = lifted.clone();
            let mut lr: ExtRegisters = unsafe { core::mem::zeroed() };
            lr.eax = event_base;
            lr.edi = u32::from(done);
            lr.esp = 0x10000;
            lr.esi = 0xabcdef01;
            put32(&mut lifted, lr.esp, lr.esi);
            let mut rr = lr;
            let mut guest = NoCalls;
            crate::c2r::data_missions::game_event_instance_mark_next_instance_words_when_definition_modes_2_or_3(&mut lifted, &mut lr);
            assert!(run(0x2351d0, &mut ported, &mut guest, &mut rr));
            assert_eq!(
                ported.read16(next + RESUME_A),
                lifted.read16(next + RESUME_A),
                "A case {i}"
            );
            assert_eq!(
                ported.read16(next + RESUME_B),
                lifted.read16(next + RESUME_B),
                "B case {i}"
            );
            assert_eq!(rr.esp, lr.esp, "stack case {i}");
            assert_eq!(rr.esi, lr.esi, "callee-saved case {i}");
        }
        assert_ne!(
            resume_words(
                LinkTargets {
                    mode_a: 3,
                    mode_b: None
                },
                false
            )
            .0,
            true
        );
    }
}
