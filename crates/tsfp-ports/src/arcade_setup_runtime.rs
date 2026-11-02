// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/ui_menu
//! Readable runtime adapter for the selected Arcade/Challenge setup edge (T2381).
//!
//! The typed model owns the mode-6/7 record fields. Guest-backed profile, availability, roster,
//! and participant services remain temporary hybrid boundary calls until those neighbours are
//! ported. The complete contract and evidence limits are in `docs/t2381-arcade-owned-handoff.md`.
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
};

const SETUP: u32 = 0x78ad58;

fn call<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    target: u32,
    args: &[u32],
) -> u32 {
    call_cdecl(guest, memory, regs, target, args);
    regs.eax
}

/// Handles only the original selected-descriptor branch (UI modes 6 and 7).
/// `false` leaves every other menu state to its existing dispatcher.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != 0x2d2c70 || !matches!(memory.read32(0x78ad2c), 6 | 7) {
        return false;
    }

    let entry_esp = regs.esp;
    regs.esp = regs.esp.wrapping_sub(0x48);
    let handled = build_selected_setup(memory, guest, regs);
    regs.esp = entry_esp;
    handled
}

fn build_selected_setup<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if call(memory, guest, regs, 0x3533f0, &[]) != 0
        && call(memory, guest, regs, 0x355ec0, &[]) == 0
    {
        return true;
    }

    // Preserve the profile and availability service order at the original call boundary.
    let profile = call(memory, guest, regs, 0x69640, &[]);
    regs.ecx = u32::from(memory.read8(0x78adad));
    regs.ecx = regs.ecx.wrapping_add(regs.ecx.wrapping_mul(4));
    regs.esi = profile
        .wrapping_add(regs.ecx.wrapping_mul(4))
        .wrapping_add(0x24);
    call(memory, guest, regs, 0x389b0, &[]);
    let session_active = call(memory, guest, regs, 0x3533f0, &[]);
    let menu_state = memory.read32(0x78ad2c);
    if session_active == 0 && menu_state != 1 && menu_state != 0x0e {
        call(memory, guest, regs, 0x2cb210, &[]);
    }

    let menu_state = memory.read32(0x78ad2c);
    if menu_state != 0x0f {
        let participants = memory.read8(0x5236c4);
        memory.write8(SETUP, participants);
        memory.write8(SETUP + 1, participants);
        memory.write32(0x78adec, 0);
    }

    let mission = memory.read32(0x78ad40);
    let descriptor = call(memory, guest, regs, 0x22e050, &[mission]);
    let owned = crate::modes_lane::build_runtime_challenge_setup(memory, guest, regs, descriptor);

    // The selected branch initializes these sentinels after the descriptor fields.
    memory.write32(0x78ad9c, u32::MAX);
    memory.write32(0x78ada0, u32::MAX);
    memory.write32(0x78ada4, u32::MAX);
    memory.write32(0x7613d0, 0);
    memory.write32(0x78ada8, u32::MAX);

    if let Some(participant) = owned.variant2_participant {
        memory.write8(0x78ad6c, participant as u8);
        return true;
    }

    // The remaining selected-branch roster work stays a small readable service adapter. The
    // descriptor contract guarantees these team IDs index the original 17-slot presence table.
    let roster = memory.read32(descriptor.wrapping_add(0x6c));
    let flags = memory.read32(descriptor.wrapping_add(0x10));
    let mut present_teams = [false; 17];
    let mut count = 0u32;
    for index in 0..11u32 {
        let entry = roster.wrapping_add(index.wrapping_mul(0x10));
        let id = memory.read32(entry) as i32;
        if id < 0 {
            break;
        }
        let id_byte = id as u8;
        let team = memory.read8(entry.wrapping_add(8));
        memory.write8(SETUP + 0x14 + index, id_byte);
        memory.write8(SETUP + 0x24 + index, team);
        if call(memory, guest, regs, 0x2be270, &[id as u32]) != 0 {
            memory.write8(SETUP + 0x44 + index, memory.read8(entry.wrapping_add(0x0c)));
        }
        let secondary = call(memory, guest, regs, 0x1b5630, &[u32::from(id_byte)]) as u8;
        memory.write8(SETUP + 0x34 + index, secondary);

        if flags & 1 != 0 {
            let team_id = memory.read32(entry.wrapping_add(8)) as usize;
            assert!(team_id < present_teams.len(), "REA team ID outside 0..17");
            present_teams[team_id] = true;
        }
        count += 1;
    }
    memory.write8(SETUP + 2, count.wrapping_sub(1) as u8);
    memory.write8(SETUP + 3, 0);
    if flags & 1 != 0 {
        let mut output_count = 0u8;
        for team in 1..17usize {
            if present_teams[team] {
                memory.write8(SETUP + 4 + u32::from(output_count), team as u8);
                output_count = output_count.wrapping_add(1);
            }
        }
        memory.write8(SETUP + 3, output_count);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes_lane::modes::challenge_setup::LAUNCH_RECORD_BYTES;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            *self.0.get(&address).unwrap_or(&0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[derive(Default)]
    struct Guest(Vec<u32>);
    impl GuestCall for Guest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.0.push(va);
            regs.eax = match va {
                0x22e050 => 0x8000,
                0x2be270 => 1,
                0x1b5630 => 7,
                0x64dd0 => 23,
                _ => 0,
            };
        }
    }

    fn put32(memory: &mut Memory, address: u32, value: u32) {
        for (index, byte) in value.to_le_bytes().into_iter().enumerate() {
            memory.write8(address + index as u32, byte);
        }
    }

    fn registers() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x0012_0000,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0,
            },
            xmm: [crate::abi::Xmm { lanes: [0; 4] }; 5],
            xmm_hi: [crate::abi::Xmm { lanes: [0; 4] }; 3],
        }
    }

    fn descriptor(variant: u32, roster_id: i32) -> Memory {
        let mut memory = Memory::default();
        put32(&mut memory, 0x78ad2c, 7);
        put32(&mut memory, 0x78ad38, 4);
        put32(&mut memory, 0x78ad40, 0x1234);
        put32(&mut memory, 0x5236c4, 1);
        put32(&mut memory, 0x7de455, 0x0a);
        put32(&mut memory, 0x4e8cec, 5);
        put32(&mut memory, 0x4d1f88 + variant * 0x20, 0);
        memory.write8(0x4d1f7c + (variant as u8 as u32) * 0x20, 9);
        put32(&mut memory, 0x8004, variant);
        put32(&mut memory, 0x8008, 2);
        memory.write8(0x800c, 0x33);
        put32(&mut memory, 0x8010, 1);
        put32(&mut memory, 0x806c, 0x9000);
        put32(&mut memory, 0x9000, roster_id as u32);
        put32(&mut memory, 0x9008, 2);
        memory.write8(0x900c, 0x44);
        put32(&mut memory, 0x9010, u32::MAX);
        put32(&mut memory, 0x8090, 4);
        put32(&mut memory, 0x8094, 0x94);
        put32(&mut memory, 0x8098, 0x98);
        put32(&mut memory, 0x809c, 0x9c);
        for offset in 0..LAUNCH_RECORD_BYTES as u32 {
            memory.write8(SETUP + offset, 0xaa);
        }
        memory
    }

    #[test]
    fn mode_seven_uses_owned_fields_and_keeps_existing_roster_services() {
        let mut memory = descriptor(3, 5);
        let mut guest = Guest::default();
        let mut regs = registers();
        assert!(run(0x2d2c70, &mut memory, &mut guest, &mut regs));
        assert_eq!(memory.read8(SETUP + 0x14), 5);
        assert_eq!(memory.read8(SETUP + 0x24), 2);
        assert_eq!(memory.read8(SETUP + 0x34), 7);
        assert_eq!(memory.read8(SETUP + 0x44), 0x44);
        assert_eq!(memory.read8(SETUP + 2), 0);
        assert_eq!(memory.read8(SETUP + 3), 1);
        assert_eq!(memory.read8(SETUP + 4), 2);
        assert_eq!(memory.read32(SETUP + 0x70), 0x1234);
        assert!(guest.0.contains(&0x2be270));
        assert!(guest.0.contains(&0x1b5630));
    }

    #[test]
    fn variant_two_returns_the_original_fallback_participant_early() {
        let mut memory = descriptor(2, -1);
        put32(&mut memory, 0x78ad2c, 6);
        let mut guest = Guest::default();
        let mut regs = registers();
        assert!(run(0x2d2c70, &mut memory, &mut guest, &mut regs));
        assert_eq!(memory.read8(SETUP + 0x14), 23);
        assert!(!guest.0.contains(&0x2be270));
        assert!(!guest.0.contains(&0x1b5630));
        assert!(guest.0.contains(&0x64dd0));
    }

    #[test]
    fn unrelated_menu_state_falls_through_without_effects() {
        let mut memory = descriptor(3, 5);
        put32(&mut memory, 0x78ad2c, 5);
        let before = memory.0.clone();
        let mut guest = Guest::default();
        let mut regs = registers();
        assert!(!run(0x2d2c70, &mut memory, &mut guest, &mut regs));
        assert_eq!(memory.0, before);
        assert!(guest.0.is_empty());
    }
}
