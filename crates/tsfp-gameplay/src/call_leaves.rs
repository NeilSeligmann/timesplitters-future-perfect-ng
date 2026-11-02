// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 13 gameplay roots that make one nested guest call, ported over the T1815 closure
//! model (`tsfp_engine_object::closure`): the callee is an opaque stub that returns seeded
//! EAX/ECX/EDX, so equality with the C replacement is proven for the call trace plus the root's
//! own logic (docs/t1836-rust-wave13.md). Register results follow the C bodies exactly.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call},
    memory::GuestMemory,
};

const MODE_BYTE: u32 = 0x007d_e455;

/// 002EF160 game_current_mode_profile_unlock_flag_is_set. cdecl(0). 69640 returns the current
/// profile context pointer. Mode byte 0x7DE455 and profile byte 0x7DE456 select an entry of the
/// 32-byte table at 0x4D1F84: kind 1 is unlocked, kind 2 is unlocked when the profile dword
/// `context + mode * 20 + 0x24` is non-zero.
pub fn profile_unlock_flag_is_set<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, 0x0006_9640, 0x002E_F165, 0, &[]);
    let mode = u32::from(m.read8(MODE_BYTE));
    let profile = u32::from(m.read8(0x007d_e456)) << 5;
    cpu.ecx = cpu.eax.wrapping_add(mode * 20).wrapping_add(0x24);
    cpu.edx = profile;
    cpu.eax = m.read32(profile.wrapping_add(0x004d_1f84));
    let unlocked = cpu.eax == 1 || (cpu.eax == 2 && m.read32(cpu.ecx) != 0);
    cpu.eax = u32::from(unlocked);
}

/// 002E4470 game_mode_0xa_multiplayer_pair_state_or_flag_bit9_active. cdecl(0). Active in mode
/// 0xA with more than one player when CB0F0 reports it, else when bit 9 of `[0x7DE458]` is set.
pub fn multiplayer_pair_state_active<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    if m.read8(MODE_BYTE) != 0x0a || (m.read32(0x0079_0950) as i32) <= 1 {
        cpu.eax = 0;
        return;
    }
    guest_call(c, m, cpu, 0x000c_b0f0, 0x002e_4487, 0, &[]);
    if cpu.eax != 0 {
        cpu.eax = 1;
        return;
    }
    cpu.eax = u32::from(m.read32(0x007d_e458) & 0x200 != 0);
}

/// 002E4CB0 game_set_current_player_7ba9e0_from_7b0c7c_field_4_and_reset_pad_when_slot_flag_bit0.
/// cdecl(0). Publishes the player index of `[0x7B0C7C]` to `[0x7BA9E0]`, resets the pad of that
/// player (42870) when the slot flag byte has bit 0, then calls 55D80 on the pad block. The C
/// body ends by reading ECX back from the dword below ESP (the callee's argument slot).
// portreg: va=0x002e4cb0 tier=0
pub fn set_current_player_and_reset_pad<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.ecx = m.read32(0x007b_0c7c);
    cpu.eax = m.read32(cpu.ecx.wrapping_add(4));
    m.write32(0x007b_a9e0, cpu.eax);
    cpu.eax = m.read32(cpu.eax.wrapping_mul(4).wrapping_add(0x0076_1c48));
    if m.read8(cpu.eax) & 1 != 0 {
        cpu.ecx = m.read32(cpu.ecx);
        let argument = cpu.ecx;
        guest_call(c, m, cpu, 0x0004_2870, 0x002e_4cd2, 0, &[argument]);
    }
    cpu.edx = m.read32(0x007b_0cc0).wrapping_add(0x224);
    let argument = cpu.edx;
    guest_call(c, m, cpu, 0x0005_5d80, 0x002e_4ce7, 0, &[argument]);
    cpu.ecx = m.read32(cpu.esp.wrapping_sub(4));
}

/// 0033ACB0 game_players_apply_class_entry_from_class_array_and_sync_slot_byte. cdecl(0). For
/// each player runs D7550(object, class + 1) on the player's object and mirrors the object's
/// class byte (+0x84) into the byte table at 0x7DE424 when it differs. Saves EDI, EBX and ESI
/// below ESP like the original (they stay unchanged).
pub fn players_apply_class_entry<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.edi);
    cpu.eax = m.read32(0x0079_0950);
    if (cpu.eax as i32) <= 0 {
        return;
    }
    m.write32(entry.wrapping_sub(8), cpu.ebx);
    m.write32(entry.wrapping_sub(12), cpu.esi);
    let (mut player, mut table_offset) = (0u32, 0u32);
    loop {
        cpu.eax = m.read32(0x007b_0c48);
        cpu.edx = m.read32(0x0078_4034u32.wrapping_add(player * 4));
        cpu.ecx = m.read32(table_offset.wrapping_add(cpu.eax).wrapping_add(0x14));
        let object = m.read32(cpu.ecx.wrapping_add(0x7c));
        cpu.edx = cpu.edx.wrapping_add(1);
        let class = cpu.edx;
        guest_call(c, m, cpu, 0x000d_7550, 0x0033_acdb, 12, &[object, class]);
        cpu.eax = m.read32(object.wrapping_add(8));
        let slot_byte = cpu.eax.wrapping_add(0x007d_e424);
        cpu.ecx = u32::from(m.read8(slot_byte));
        cpu.edx = m.read32(object.wrapping_add(0x84));
        if cpu.ecx != cpu.edx {
            m.write8(slot_byte, cpu.edx as u8);
        }
        cpu.eax = m.read32(0x0079_0950);
        player += 1;
        table_offset = table_offset.wrapping_add(0x1584);
        if (player as i32) >= (cpu.eax as i32) {
            return;
        }
    }
}

/// 00338150 t1479_clear_indexed_slot. cdecl(1: slot). Runs 45590(slot), then publishes the slot
/// in `[0x765E4C]`. ESI holds the slot and is saved below ESP.
pub fn clear_indexed_slot<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    cpu.esi = m.read32(cpu.esp.wrapping_add(4));
    let slot = cpu.esi;
    guest_call(c, m, cpu, 0x0004_5590, 0x0033_815b, 4, &[slot]);
    m.write32(0x0076_5e4c, slot);
    cpu.esi = m.read32(cpu.esp.wrapping_sub(4));
}

/// 00275640 game_stamp_side_entry_74c3b8_when_table_7a2be4_flag_changes_in_mode_2. cdecl(1:
/// side). Only when byte 0x7DE402 is set and the mode byte 0x7DE455 is 2: asks 274380(side)
/// (bit 7 of the side flags clear) and, when the answer inverted differs from the dword at
/// `entry[side].+8` of the table `[0x74C3B8]` (16-byte entries), stores it there together with
/// the stamp `[0x7DE318]` in the dword at +12. Only the low bytes of EAX and EDX are replaced
/// on the early exits. ESI is saved below ESP.
pub fn stamp_side_entry_on_flag_change<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = (cpu.eax & 0xffff_ff00) | u32::from(m.read8(0x007d_e402));
    if cpu.eax & 0xff == 0 {
        return;
    }
    cpu.edx = (cpu.edx & 0xffff_ff00) | u32::from(m.read8(MODE_BYTE));
    cpu.eax = u32::from(cpu.edx & 0xff == 2);
    if cpu.eax == 0 {
        return;
    }
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    cpu.esi = m.read32(cpu.esp.wrapping_add(4));
    let side = cpu.esi;
    guest_call(c, m, cpu, 0x0027_4380, 0x0027_5666, 4, &[side]);
    cpu.ecx = m.read32(0x0074_c3b8);
    cpu.eax = u32::from(cpu.eax == 0);
    cpu.esi <<= 4;
    let entry = cpu.esi.wrapping_add(cpu.ecx);
    cpu.edx = m.read32(entry.wrapping_sub(8));
    if cpu.eax != cpu.edx {
        m.write32(entry.wrapping_sub(8), cpu.eax);
        cpu.eax = m.read32(0x0074_c3b8);
        cpu.ecx = m.read32(0x007d_e318);
        m.write32(cpu.esi.wrapping_add(cpu.eax).wrapping_sub(4), cpu.ecx);
    }
    cpu.esi = m.read32(cpu.esp.wrapping_sub(4));
}
