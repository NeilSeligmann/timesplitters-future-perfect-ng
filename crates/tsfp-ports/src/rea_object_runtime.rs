// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/engine_object_3
// portreg: replaces=tsfp-engine-physics/fp_wave10
//! T2433: lane adapter for three readable engine/object frontier leaves (owner:
//! `tsfp-gameplay/src/rea_object_flags.rs`). The owner sees plain values, this file reads and
//! writes the live image at its edge (a TEMPORARY-SHIM until T2044 moves the object and entry
//! records into Rust-owned state, removal task T2044).
//!
//! | VA | ABI | result |
//! |---|---|---|
//! | `0xB0770` | cdecl(object) | EAX = 1 when `[object+0x90]` is the current or previous frame stamp |
//! | `0x160F50` | cdecl(object) | EAX = 1 for a type 8 object whose extension tag is `VICE` or `VICO` |
//! | `0x162520` | cdecl(index) | writes `[0x7A2820][index*24+0x10]`, EAX = record pointer |
//!
//! Scratch registers are reproduced as the original leaves them (ECX, and EDX for `0x162520`), the
//! callee-saved ones are untouched. All three roots are leaves (no guest calls), so an invalid
//! guest address is rolled back to the retained original by `anim_lane::guarded`.
//!
//! A/B: lane roots (`--rust-ports`, group `object`), declined unless `TSFP_REA_OBJECT_NATIVE=1`.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
    rea_object_flags as owner,
};

/// `[0x7DE318]`: current frame counter.
pub const FRAME_COUNTER: u32 = 0x007d_e318;
/// `[0x4E7944]`: distance to the previous frame counter.
pub const FRAME_DELTA: u32 = 0x004e_7944;
/// Object field stamped with the frame counter.
pub const OBJECT_STAMP: u32 = 0x90;
/// Object type dword.
pub const OBJECT_TYPE: u32 = 0x20;
/// Object extension pointer.
pub const OBJECT_EXTENSION: u32 = 0x7c;
/// Extension field holding the tagged record pointer, whose dword `+8` is the tag.
pub const EXTENSION_TAGGED: u32 = 0xbf0;
/// Pointer to the record owner whose `+0x74` is the base of the `0x98`-byte records.
pub const RECORD_OWNER_POINTER: u32 = 0x0074_c36c;
pub const RECORD_BASE_OFFSET: u32 = 0x74;
pub const RECORD_STRIDE: u32 = 0x98;
/// Pointer to the entry array (stride 24, flag at `+0x10`).
pub const ENTRY_ARRAY_POINTER: u32 = 0x007a_2820;
pub const ENTRY_STRIDE: u32 = 24;
pub const ENTRY_FLAG_OFFSET: u32 = 0x10;
/// Slot row 0 of the two-table family: `0x7A2DE4 - 0x60`, rows `0x60` bytes apart.
pub const SLOT_TABLE: u32 = 0x007a_2d84;
pub const SLOT_STRIDE: u32 = 0x60;

pub const ROOTS: [u32; 3] = [0xb0770, 0x160f50, 0x162520];
/// The A/B switch of these roots (selector and T3738 `--t2-offline` opt-in registry).
pub const OPT_IN: crate::runtime::OptIn = crate::runtime::OptIn {
    env: c"TSFP_REA_OBJECT_NATIVE",
    on: c"1",
    roots: &ROOTS,
};

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

/// The A/B switch: the Rust owner runs only with `TSFP_REA_OBJECT_NATIVE=1`.
#[cfg(not(test))]
fn enabled() -> bool {
    use core::sync::atomic::{AtomicU8, Ordering::Relaxed};
    static STATE: AtomicU8 = AtomicU8::new(0);
    match STATE.load(Relaxed) {
        0 => {
            let value = unsafe { getenv(OPT_IN.env.as_ptr()) };
            let on = !value.is_null()
                && unsafe { core::ffi::CStr::from_ptr(value) }.to_bytes() == OPT_IN.on.to_bytes();
            STATE.store(if on { 2 } else { 1 }, Relaxed);
            on
        }
        state => state == 2,
    }
}

#[cfg(test)]
fn enabled() -> bool {
    true
}

/// Selector hook: a root is declined (the retained original runs) unless the switch is on.
pub fn declined(va: u32) -> bool {
    ROOTS.contains(&va) && !enabled()
}

fn argument<M: GuestMemory>(m: &M, regs: &ExtRegisters) -> u32 {
    m.read32(regs.esp.wrapping_add(4))
}

/// `0xB0770`.
pub fn object_stamp_is_recent<M: GuestMemory>(m: &M, regs: &mut ExtRegisters) {
    let stamp = owner::FrameStamp {
        current: m.read32(FRAME_COUNTER),
        previous_delta: m.read32(FRAME_DELTA),
    };
    let mut recent = false;
    if stamp.current != 0 {
        let object = argument(m, regs);
        let object_stamp = m.read32(object.wrapping_add(OBJECT_STAMP));
        // the original leaves the stamp in ECX once the frame counter is non-zero
        regs.ecx = object_stamp;
        recent = stamp.is_recent(object_stamp);
    }
    regs.eax = u32::from(recent);
}

/// `0x160F50`.
pub fn object_is_vice_or_vico<M: GuestMemory>(m: &M, regs: &mut ExtRegisters) {
    let object = argument(m, regs);
    let extension = m.read32(object.wrapping_add(OBJECT_EXTENSION));
    regs.ecx = extension;
    let tagged = m.read32(object.wrapping_add(OBJECT_TYPE)) == owner::TAGGED_OBJECT_TYPE && {
        let record = m.read32(extension.wrapping_add(EXTENSION_TAGGED));
        owner::is_vice_or_vico_tag(m.read32(record.wrapping_add(8)))
    };
    regs.eax = u32::from(tagged);
}

/// `0x162450`, `0x162520` and `0x163220` share this body, `rule` selects the member.
pub fn set_entry_flag<M: GuestMemory>(
    m: &mut M,
    regs: &mut ExtRegisters,
    rule: owner::EntryFlagRule,
) {
    let index = argument(m, regs);
    let owner_record = m.read32(RECORD_OWNER_POINTER);
    let record = m
        .read32(owner_record.wrapping_add(RECORD_BASE_OFFSET))
        .wrapping_add(index.wrapping_mul(RECORD_STRIDE));
    let entry = m
        .read32(ENTRY_ARRAY_POINTER)
        .wrapping_add(index.wrapping_mul(ENTRY_STRIDE));
    let word_offset = match rule.slot_word {
        owner::SlotWord::First => 0,
        owner::SlotWord::Second => 4,
    };
    let mask = m.read32(record.wrapping_add(rule.mask_offset));
    let flag = rule.flag(mask, |slot| {
        m.read32(
            SLOT_TABLE
                .wrapping_add(slot.wrapping_mul(SLOT_STRIDE))
                .wrapping_add(word_offset),
        )
    });
    m.write32(entry.wrapping_add(ENTRY_FLAG_OFFSET), flag);
    regs.eax = record;
    regs.ecx = owner::SLOT_COUNT;
    regs.edx = owner::SLOT_COUNT + 2;
}

/// Runs one root on guest memory. `false` for other VAs.
pub fn run_original_entry<M: GuestMemory>(va: u32, m: &mut M, regs: &mut ExtRegisters) -> bool {
    match va {
        0xb0770 => object_stamp_is_recent(m, regs),
        0x160f50 => object_is_vice_or_vico(m, regs),
        0x162520 => set_entry_flag(m, regs, owner::RULE_162520),
        _ => return false,
    }
    true
}

/// Lane entry (module of `lane_inv`).
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) || !enabled() {
        return false;
    }
    crate::anim_lane::guarded(va, m, guest, regs, |view, _guest, regs| {
        run_original_entry(va, view, regs)
    })
}
