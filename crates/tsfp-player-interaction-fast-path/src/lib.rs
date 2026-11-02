// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]

#[cfg(test)]
extern crate std;

use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;

const CURRENT_PLAYER_RECORD: u32 = 0x007b_0c7c;
const SESSION_FLAGS: u32 = 0x007d_e458;
const SESSION_MODE: u32 = 0x007d_e470;
const PROFILE_POINTER: u32 = 0x1500;
const PROFILE_CONTROL_FLAGS: u32 = 2;
const PROFILE_AUTO_AIM: u8 = 1 << 7;
const PLAYER_STATE: u32 = 0x114;
const PLAYER_LOCK_ON_STATE: u32 = 0xf7c;
const SESSION_BLOCK_FLAG: u8 = 0x40;

/// Dispatch the player C2R family, replacing only D0730's known-disabled prefix.
pub fn dispatch_player_module<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    fallback: impl FnOnce(u32, &mut M, &mut C, &mut ExtRegisters) -> bool,
) -> bool {
    if va == 0x000d_0730 && run_d0730_early_reject(memory, guest, regs).is_some() {
        true
    } else {
        fallback(va, memory, guest, regs)
    }
}

/// Run D0730's translated, known-disabled early return in readable Rust.
///
/// This handles only the sufficient CFF20 rejection cases established by its translated body:
/// Auto Aim disabled, or session flag 0x40 with mode 10, 11, or 16. CFF20 remains a GuestCall so
/// its stack and register effects are preserved by the existing callee path. All other inputs
/// return `None` and continue through the retained D0730 body.
pub fn run_d0730_early_reject<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> Option<()> {
    let record = memory.read32(CURRENT_PLAYER_RECORD);
    if record < 0x1_0000 || record & 3 != 0 {
        return None;
    }
    let profile_slot = record.checked_add(PROFILE_POINTER)?;
    if !memory.is_readable(profile_slot, 4) {
        return None;
    }
    let profile = memory.read32(profile_slot);
    let profile_flags = profile.checked_add(PROFILE_CONTROL_FLAGS)?;
    if !memory.is_readable(profile_flags, 1) {
        return None;
    }
    let auto_aim = memory.read8(profile_flags) & PROFILE_AUTO_AIM != 0;
    let session_blocked = memory.read8(SESSION_FLAGS) & SESSION_BLOCK_FLAG != 0
        && matches!(memory.read32(SESSION_MODE), 10 | 11 | 16);
    if auto_aim && !session_blocked {
        return None;
    }

    let entry_esp = regs.esp;
    regs.esp = regs.esp.wrapping_sub(0xb4);
    regs.eax = record;
    regs.esp = regs.esp.wrapping_sub(4);
    memory.write32(regs.esp, regs.esi);
    regs.esi = 0;
    memory.write32(record.wrapping_add(PLAYER_LOCK_ON_STATE), regs.esi);
    regs.ecx = record;
    regs.esp = regs.esp.wrapping_sub(4);
    memory.write32(regs.esp, regs.edi);
    regs.edi = 0;
    regs.esp = regs.esp.wrapping_sub(4);
    memory.write32(regs.esp, regs.ecx);
    memory.write32(regs.esp.wrapping_add(0x14), regs.edi);

    // Match the lifted `RegsExt::call`: CFF20 owns its internal stack effects, while the
    // caller's EBP is restored by the bridge contract.
    let frame_ebp = regs.ebp;
    guest.call(0x000c_ff20, regs);
    regs.ebp = frame_ebp;
    regs.esp = regs.esp.wrapping_add(4);
    assert_eq!(
        regs.eax, 0,
        "known CFF20 rejection unexpectedly allowed targeting"
    );

    regs.edi = memory.read32(regs.esp);
    regs.esp = regs.esp.wrapping_add(4);
    regs.eax = 0;
    regs.esi = memory.read32(regs.esp);
    regs.esp = regs.esp.wrapping_add(4);
    regs.esp = regs.esp.wrapping_add(0xb4);
    debug_assert_eq!(regs.esp, entry_esp);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tsfp_port_runtime::abi::Fpu;

    struct Memory(BTreeMap<u32, u8>);

    impl Memory {
        fn new() -> Self {
            Self(BTreeMap::new())
        }

        fn put8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }

        fn put32(&mut self, address: u32, value: u32) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.put8(address.wrapping_add(offset as u32), byte);
            }
        }
    }

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.put8(address, value);
        }
    }

    struct DisabledTargetingCall {
        calls: u32,
        state: u32,
        edx: u32,
    }

    impl GuestCall for DisabledTargetingCall {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            assert_eq!(va, 0x000c_ff20);
            self.calls += 1;
            regs.eax = 0;
            regs.ecx = self.state;
            regs.edx = self.edx;
        }
    }

    fn registers() -> ExtRegisters {
        ExtRegisters {
            eax: 1,
            ecx: 2,
            edx: 3,
            esp: 0x0080_0000,
            ebx: 4,
            ebp: 5,
            esi: 6,
            edi: 7,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [Default::default(); 5],
            xmm_hi: [Default::default(); 3],
        }
    }

    #[test]
    fn fabricated_auto_aim_off_early_return_preserves_d0730_stack_and_record_effects() {
        let record = 0x0010_0000;
        let profile = 0x0020_0000;
        let mut memory = Memory::new();
        memory.put32(CURRENT_PLAYER_RECORD, record);
        memory.put32(record + PROFILE_POINTER, profile);
        memory.put8(profile + PROFILE_CONTROL_FLAGS, 0);
        memory.put32(record + PLAYER_STATE, 0x40);
        let mut regs = registers();
        let entry_esp = regs.esp;
        let saved_esi = regs.esi;
        let saved_edi = regs.edi;
        let saved_ebx = regs.ebx;
        let saved_ebp = regs.ebp;
        let mut guest = DisabledTargetingCall {
            calls: 0,
            state: 0x40,
            edx: profile,
        };

        assert_eq!(
            run_d0730_early_reject(&mut memory, &mut guest, &mut regs),
            Some(())
        );
        assert_eq!(guest.calls, 1);
        assert_eq!(regs.esp, entry_esp);
        assert_eq!(regs.eax, 0);
        assert_eq!(regs.esi, saved_esi);
        assert_eq!(regs.edi, saved_edi);
        assert_eq!(regs.ebx, saved_ebx);
        assert_eq!(regs.ebp, saved_ebp);
        assert_eq!(regs.ecx, 0x40);
        assert_eq!(regs.edx, profile);
        assert_eq!(memory.read32(record + PLAYER_LOCK_ON_STATE), 0);
        assert_eq!(memory.read32(entry_esp - 0xb8), saved_esi);
        assert_eq!(memory.read32(entry_esp - 0xbc), saved_edi);
        assert_eq!(memory.read32(entry_esp - 0xc0), record);
        assert_eq!(memory.read32(entry_esp - 0xac), 0);
    }

    #[test]
    fn fabricated_enabled_targeting_falls_back_without_touching_state() {
        let record = 0x0010_0000;
        let profile = 0x0020_0000;
        let mut memory = Memory::new();
        memory.put32(CURRENT_PLAYER_RECORD, record);
        memory.put32(record + PROFILE_POINTER, profile);
        memory.put8(profile + PROFILE_CONTROL_FLAGS, PROFILE_AUTO_AIM);
        memory.put32(record + PLAYER_LOCK_ON_STATE, 0xfeed_beef);
        let mut regs = registers();
        let before = regs;
        let mut guest = DisabledTargetingCall {
            calls: 0,
            state: 0,
            edx: profile,
        };

        assert_eq!(
            run_d0730_early_reject(&mut memory, &mut guest, &mut regs),
            None
        );
        assert_eq!(guest.calls, 0);
        assert_eq!(regs, before);
        assert_eq!(memory.read32(record + PLAYER_LOCK_ON_STATE), 0xfeed_beef);
    }

    #[test]
    fn fabricated_blocked_session_early_return_preserves_mode_register_residue() {
        let record = 0x0010_0000;
        let profile = 0x0020_0000;
        let mut memory = Memory::new();
        memory.put32(CURRENT_PLAYER_RECORD, record);
        memory.put32(record + PROFILE_POINTER, profile);
        memory.put8(profile + PROFILE_CONTROL_FLAGS, PROFILE_AUTO_AIM);
        memory.put8(SESSION_FLAGS, SESSION_BLOCK_FLAG);
        memory.put32(SESSION_MODE, 11);
        memory.put32(record + PLAYER_STATE, 0x80);
        let mut regs = registers();
        let mut guest = DisabledTargetingCall {
            calls: 0,
            state: 0x80,
            edx: 11,
        };

        assert_eq!(
            run_d0730_early_reject(&mut memory, &mut guest, &mut regs),
            Some(())
        );
        assert_eq!(guest.calls, 1);
        assert_eq!(regs.eax, 0);
        assert_eq!(regs.ecx, 0x80);
        assert_eq!(regs.edx, 11);
        assert_eq!(regs.esp, 0x0080_0000);
    }
}
