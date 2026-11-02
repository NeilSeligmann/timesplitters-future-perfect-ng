// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 13 ui roots that make one nested guest call, ported over the T1815 closure model
//! (opaque seeded callee, call trace compared, docs/t1836-rust-wave13.md).
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

/// 0022DE90 game_is_state_7de470_6_to_8_and_not_flags_bit23_and_bit6_both_set. cdecl(0). False
/// when 64D40 reports the blocking flag pair, else true for UI state 6..=8.
pub fn state_6_to_8_unless_blocked<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, 0x0006_4d40, 0x0022_de95, 0, &[]);
    if cpu.eax != 0 {
        cpu.eax = 0;
        return;
    }
    let state = m.read32(0x007d_e470) as i32;
    cpu.eax = u32::from((6..=8).contains(&state));
}

/// 002BFEC0 game_setup_78adae_is_0x13_and_arg_below_9_minus_field_c_of_context_69640.
/// cdecl(1: count). Only for setup 0x13: `count >= 9 - context[+0xC]` (signed) where the context
/// comes from 69640.
pub fn setup_slots_available<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    if m.read8(0x0078_adae) != 0x13 {
        cpu.eax = 0;
        return;
    }
    guest_call(c, m, cpu, 0x0006_9640, 0x002b_fece, 0, &[]);
    cpu.edx = m.read32(cpu.eax.wrapping_add(0xc));
    cpu.eax = stack_arg(m, cpu, 0);
    cpu.ecx = 9u32.wrapping_sub(cpu.edx);
    cpu.eax = u32::from((cpu.eax as i32) >= (cpu.ecx as i32));
}

/// Shared body of 000662D0 and 000662F0: `66280(first, mode, second)`, leaving the callee's
/// registers (ECX holds the first argument and EAX the second until the call).
fn call_66280_with_mode<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    mode: u32,
    return_pc: u32,
) {
    cpu.eax = stack_arg(m, cpu, 1);
    cpu.ecx = stack_arg(m, cpu, 0);
    let (first, second) = (cpu.ecx, cpu.eax);
    guest_call(c, m, cpu, 0x0006_6280, return_pc, 0, &[first, mode, second]);
}

/// 000662D0 game_wrapper_66280_arg_0. cdecl(2: first, second): `66280(first, 0, second)`.
pub fn wrapper_66280_mode_0<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    call_66280_with_mode(c, m, cpu, 0, 0x0006_62e1);
}

/// 000662F0 game_wrapper_66280_arg_1. cdecl(2: first, second): `66280(first, 1, second)`.
pub fn wrapper_66280_mode_1<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    call_66280_with_mode(c, m, cpu, 1, 0x0006_6301);
}

/// 00075F40 game_object_flags_clear_and_chain_predicate_zero. cdecl(1: object). 1 when the
/// byte at +0x40 has none of the 0x30 bits and the chain predicate 747C0(object) is 0.
pub fn flags_clear_and_chain_predicate_zero<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let object = stack_arg(m, cpu, 0);
    cpu.eax = object;
    if m.read8(object.wrapping_add(0x40)) & 0x30 != 0 {
        cpu.eax = 0;
        return;
    }
    guest_call(c, m, cpu, 0x0007_47c0, 0x0007_5f50, 0, &[object]);
    cpu.eax = u32::from(cpu.eax == 0);
}

const TEXT_ID_BASE: u32 = 0xb2b;

/// 00064EF0 game_copy_text_id_plus_0xb2b_to_buffer. cdecl(2: destination, id). Copies the
/// NUL-terminated text 3D7B0(id + 0xB2B) to the destination, terminator included. EAX and EDX
/// end one past the terminator in source and destination.
pub fn copy_text_to_buffer<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    cpu.eax = stack_arg(m, cpu, 1).wrapping_add(TEXT_ID_BASE);
    let text_id = cpu.eax;
    guest_call(c, m, cpu, 0x0003_d7b0, 0x0006_4eff, 0, &[text_id]);
    cpu.edx = stack_arg(m, cpu, 0);
    loop {
        let byte = m.read8(cpu.eax);
        cpu.eax = cpu.eax.wrapping_add(1);
        m.write8(cpu.edx, byte);
        cpu.edx = cpu.edx.wrapping_add(1);
        if byte == 0 {
            cpu.ecx = (cpu.ecx & 0xffff_ff00) | u32::from(byte);
            return;
        }
    }
}

const SELECTED_MODE_BYTE: u32 = 0x0078_adad;
const SELECTED_SETUP_BYTE: u32 = 0x0078_adae;
const SETUP_KIND_TABLE: u32 = 0x004d_1f84;
const CURRENT_GAME_MODE: u32 = 0x0079_094c;
const PROFILE_MODE_STRIDE: u32 = 20;
const PROFILE_UNLOCK_FIELD: u32 = 0x24;
const SETUP_KIND_STRIDE: u32 = 0x20;
const LOCAL_GAME_MODE: u32 = 0x65;

/// 002BE9B0 game_setup_entry_available_for_selected_mode_and_net_session. cdecl(0).
/// Reads the selected setup kind from a 0x20-byte table. Kind 1 is always unlocked; kind 2
/// needs the selected mode's profile unlock word. Availability then requires local game mode
/// 0x65 or a successful session query at 003533F0. The profile pointer comes from 00069640.
/// Register writes and both guest-call return PCs match the original port (Tier 1 historical
/// lifted-C differential; this readability edit has only Tier 0 synthetic coverage).
pub fn setup_entry_available<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    guest_call(c, m, cpu, 0x0006_9640, 0x002b_e9b5, 0, &[]);
    let profile = cpu.eax;
    let selected_mode = u32::from(m.read8(SELECTED_MODE_BYTE));
    let selected_setup = u32::from(m.read8(SELECTED_SETUP_BYTE));
    cpu.ecx = selected_mode.wrapping_mul(5);
    cpu.edx = selected_setup.wrapping_mul(SETUP_KIND_STRIDE);
    cpu.ecx = profile
        .wrapping_add(selected_mode.wrapping_mul(PROFILE_MODE_STRIDE))
        .wrapping_add(PROFILE_UNLOCK_FIELD);
    cpu.eax = m.read32(cpu.edx.wrapping_add(SETUP_KIND_TABLE));
    if cpu.eax != 1 && !(cpu.eax == 2 && m.read32(cpu.ecx) != 0) {
        cpu.eax = 0;
        return;
    }
    if m.read32(CURRENT_GAME_MODE) == LOCAL_GAME_MODE {
        cpu.eax = 1;
        return;
    }
    guest_call(c, m, cpu, 0x0035_33f0, 0x002b_e9f0, 0, &[]);
    cpu.eax = u32::from(cpu.eax != 0);
}

const MEMORY_UNIT_SELECTION_RESET: u32 = 0x0008_27b0;
const MEMORY_UNIT_RESET_RETURN_PC: u32 = 0x0019_0a87;
const MEMORY_UNIT_PENDING_ACTION: u32 = 0x007b_978c;

/// 00190A80 resets the selected memory-unit slot and page, then clears the
/// pending memory-unit action. The selection reset is delegated to 000827B0;
/// its zero argument is the reset value. The pending-action field is a guest
/// global shared with the memory-unit menu (field purpose INFERRED).
pub fn reset_memory_unit_globals<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, MEMORY_UNIT_SELECTION_RESET, MEMORY_UNIT_RESET_RETURN_PC, 0, &[0]);
    m.write32(MEMORY_UNIT_PENDING_ACTION, 0);
}

/// 0012D1E0 game_hud_state_7490ac_reset_defaults_by_player_count. cdecl(0). Resets the twelve
/// HUD state dwords at 0x7490AC.., then derives dwords +0x08 and +0x1C from two player-count
/// queries (1CDAE0): value `2 * (count <= 2) + 1` and `count <= 2`. ESI is pushed around the
/// body (stack slot written, value restored).
pub fn hud_state_reset_by_player_count<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    const BASE: u32 = 0x0074_90ac;
    cpu.eax = 0;
    for word in 0..12 {
        m.write32(BASE + 4 * word, 0);
    }
    cpu.esp = cpu.esp.wrapping_sub(4);
    m.write32(cpu.esp, cpu.esi);
    m.write32(BASE + 0x30, 0);
    cpu.esi = 1;
    m.write32(BASE + 0x34, 0);
    m.write32(BASE + 4, 1);
    guest_call(c, m, cpu, 0x001c_dae0, 0x0012_d239, 0, &[]);
    cpu.ecx = u32::from((cpu.eax as i32) <= 2);
    for offset in [0xc, 0x10, 0x2c, 0x14, 0x34] {
        m.write32(BASE + offset, 1);
    }
    cpu.ecx = cpu.ecx.wrapping_mul(2).wrapping_add(1);
    m.write32(BASE + 8, cpu.ecx);
    guest_call(c, m, cpu, 0x001c_dae0, 0x0012_d26e, 0, &[]);
    cpu.edx = u32::from((cpu.eax as i32) <= 2);
    for offset in [0x1c, 0x20, 0x24, 0x28, 0x30] {
        m.write32(BASE + offset, 1);
    }
    cpu.esi = m.read32(cpu.esp);
    cpu.esp = cpu.esp.wrapping_add(4);
    m.write32(BASE + 0x18, cpu.edx);
}
