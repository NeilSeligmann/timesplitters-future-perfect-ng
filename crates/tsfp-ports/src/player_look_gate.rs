// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/gameplay_player
//! T2194 readable original targeting gate; default for selected player roots after scoped Tier 2.
//! State projection and ABI are adapters; eligibility uses the shared Rust-owned contract.
//! Verification Tier 2 on observed original look routes; unobserved branches Tier 1 FABRICATED, not whole-mission proof.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
use core::sync::atomic::{AtomicU8, Ordering};

#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-input/src/look_gate.rs"]
mod contract;
pub use contract::TargetingState;

pub const ROOTS: [u32; 1] = [0xcff20];
static SELECTION: AtomicU8 = AtomicU8::new(0);
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

/// Original gate admitted by look/movement/zoom Tier 2; env 0 retains the translated A/B path.
pub fn selected() -> bool {
    let mut selection = SELECTION.load(Ordering::Relaxed);
    if selection == 0 {
        // SAFETY: getenv returns a NUL-terminated C string or null; only byte 0 is read.
        let enabled = unsafe {
            let value = getenv(c"TSFP_NATIVE_LOOK_GATE".as_ptr());
            value.is_null() || value.read() == b'1' as core::ffi::c_char
        };
        selection = if enabled { 2 } else { 1 };
        SELECTION.store(selection, Ordering::Relaxed);
    }
    selection == 2
}

/// Native handler with no guest-state writes. Unknown/disabled roots do nothing.
/// The original cdecl return leaves ESP at entry and preserves EBX/ESI.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    enabled: bool,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !enabled || va != ROOTS[0] {
        return false;
    }
    let player = memory.read32(regs.esp.wrapping_add(4));
    let profile = memory.read32(player.wrapping_add(0x1500));
    let owner = memory.read32(player.wrapping_add(0x14));
    let object = memory.read32(owner.wrapping_add(0x7c));
    let input = TargetingState {
        auto_aim: memory.read8(profile.wrapping_add(2)) & 0x80 != 0
            && !crate::raw_look::assists_disabled(memory, player),
        alternate_input: memory.read32(player.wrapping_add(0xf5c)) != 0,
        player_mode: memory.read32(player.wrapping_add(0x114)),
        session_flags: crate::state_wire::get_flags_low8(memory),
        session_mode: memory.read32(0x7de470),
    };
    let mut object_allows = false;
    if input.requires_object_predicate() {
        let (esp, ebx, esi, ebp) = (regs.esp, regs.ebx, regs.esi, regs.ebp);
        regs.esp = esp.wrapping_sub(12);
        memory.write32(esp.wrapping_sub(4), ebx);
        memory.write32(esp.wrapping_sub(8), esi);
        memory.write32(regs.esp, object);
        regs.esi = object;
        guest.call(0x1b6ae0, regs);
        object_allows = regs.eax != 0;
        regs.esp = esp;
        regs.ebx = ebx;
        regs.esi = esi;
        regs.ebp = ebp;
    }
    regs.eax = u32::from(input.allows(object_allows));
    true
}
