// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 14 ai roots that call other guest functions, ported over the T1815 closure model
//! (opaque seeded callee, call trace compared, docs/t1836-rust-wave14.md).
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

/// 00012FE0 game_pair_table_74c368_entry_addr_wrapper_c. cdecl(2: a, b). Address of the pair
/// table entry `15BD00(a, b)`; returns its dword at +8.
pub fn pair_table_entry_field_8<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let (first, second) = (stack_arg(m, cpu, 0), stack_arg(m, cpu, 1));
    cpu.eax = second;
    cpu.ecx = first;
    guest_call(c, m, cpu, 0x0015_bd00, 0x0001_2fef, 0, &[first, second]);
    cpu.eax = m.read32(cpu.eax.wrapping_add(8));
}

/// 002B2390 game_table_7a2960_no_entry_has_flag_bit0_in_field_28. cdecl(0). 1 when no entry
/// `i` (15BE60(i), then its object at +4) of the 15BE50-sized table has bit 0 of the byte at
/// +0x28. ESI is saved below ESP, the callee frames sit 4 bytes lower.
pub fn table_7a2960_has_no_flagged_entry<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let mut index = 0u32;
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    guest_call(c, m, cpu, 0x0015_be50, 0x002b_2398, 4, &[]);
    if (cpu.eax as i32) > 0 {
        loop {
            guest_call(c, m, cpu, 0x0015_be60, 0x002b_23a6, 4, &[index]);
            cpu.eax = m.read32(cpu.eax.wrapping_add(4));
            cpu.ecx = (cpu.ecx & 0xffff_ff00) | u32::from(m.read8(cpu.eax.wrapping_add(0x28)));
            if cpu.ecx & 1 != 0 {
                cpu.eax = 0;
                return;
            }
            index = index.wrapping_add(1);
            guest_call(c, m, cpu, 0x0015_be50, 0x002b_23ba, 4, &[]);
            if (index as i32) >= (cpu.eax as i32) {
                break;
            }
        }
    }
    cpu.eax = 1;
}

/// 0025F320 game_char_set_ext_field_4_from_table_78b2e0_entry_by_anim_key. cdecl(1: character).
/// For a character whose brain object (+0xBF0 of its extension) uses the handler 163830, looks
/// up the animation key's entry (25FA10) in the 32-byte table at 0x78B2E0 and stores its +0x10
/// object into extension+4 when that object is of kind 8, else 0. ESI is saved below ESP.
// portreg: va=0x0025f320 tier=0
pub fn char_set_ext_field_4_from_anim_entry<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = stack_arg(m, cpu, 0);
    m.write32(cpu.esp.wrapping_sub(4), cpu.esi);
    cpu.esi = m.read32(cpu.eax.wrapping_add(0x7c));
    cpu.eax = m.read32(cpu.esi.wrapping_add(0xbf0));
    cpu.ecx = m.read32(cpu.eax.wrapping_add(0x6c));
    if cpu.ecx == 0x0016_3830 {
        cpu.ecx = m.read32(cpu.eax.wrapping_add(0x38));
        let key = cpu.ecx;
        guest_call(c, m, cpu, 0x0025_fa10, 0x0025_f346, 4, &[key]);
        cpu.eax = (cpu.eax << 5).wrapping_add(0x0078_b2e0);
        cpu.eax = m.read32(cpu.eax.wrapping_add(0x10));
        let kind_ok = cpu.eax != 0 && m.read32(cpu.eax.wrapping_add(0x20)) == 8;
        m.write32(cpu.esi.wrapping_add(4), if kind_ok { cpu.eax } else { 0 });
    }
    cpu.esi = m.read32(cpu.esp.wrapping_sub(4));
}

const OBJECT_RECORD_OWNER: u32 = 0x7c;
const OWNER_RECORD_ID: u32 = 0x204;
const RECORD_ROWS: u32 = 0x004f_9bac;
const RECORD_ROW_SIZE: u32 = 0x29c;

/// Probe shared by 0026CC80 and 0026CD50: selects 1 when the owner's record id (+0x204) is -1 or
/// the record row has flag bit 0 at +0xC, else 0, then asks `callee(object, table[selected])`.
/// ESI is the owner and EDI the object while the call runs.
fn probe<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    table: u32,
    callee: u32,
    return_pc: u32,
) {
    cpu.eax = m.read32(cpu.esi.wrapping_add(OWNER_RECORD_ID));
    if cpu.eax == u32::MAX {
        cpu.eax = 1;
    } else {
        cpu.ecx = m.read32(RECORD_ROWS);
        cpu.eax = cpu.eax.wrapping_mul(RECORD_ROW_SIZE);
        let flags = m.read8(cpu.eax.wrapping_add(cpu.ecx).wrapping_add(0xc));
        cpu.eax = u32::from(flags & 1 != 0);
    }
    cpu.edx = m.read32(cpu.eax.wrapping_mul(4).wrapping_add(table));
    let (object, word) = (cpu.edi, cpu.edx);
    guest_call(c, m, cpu, callee, return_pc, 8, &[object, word]);
}

/// Runs three probes in order and answers 1 on the first hit. ESI and EDI are saved below ESP and
/// restored from there.
fn any_probe_hits<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    callee: u32,
    probes: [(u32, u32); 3],
) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.esi);
    m.write32(entry.wrapping_sub(8), cpu.edi);
    cpu.edi = stack_arg(m, cpu, 0);
    cpu.esi = m.read32(cpu.edi.wrapping_add(OBJECT_RECORD_OWNER));
    let mut hit = false;
    for (table, return_pc) in probes {
        probe(c, m, cpu, table, callee, return_pc);
        if cpu.eax != 0 {
            hit = true;
            break;
        }
    }
    cpu.edi = m.read32(entry.wrapping_sub(8));
    if hit {
        cpu.eax = 1;
    }
    cpu.esi = m.read32(entry.wrapping_sub(4));
}

/// 0026CC80 game_object_any_probe_hits_a. cdecl(1: object). Probe 00059D70 over the tables
/// 0x5202F8, 0x520308 and 0x5202E8.
pub fn object_any_probe_hits_a<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    any_probe_hits(
        c,
        m,
        cpu,
        0x0005_9d70,
        [(0x0052_02f8, 0x0026_ccbe), (0x0052_0308, 0x0026_ccfa), (0x0052_02e8, 0x0026_cd36)],
    );
}

/// 0026CD50 game_object_any_probe_hits_b. Same shape with 00059D20 and tables 0x520338,
/// 0x520328, 0x520318.
pub fn object_any_probe_hits_b<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    any_probe_hits(
        c,
        m,
        cpu,
        0x0005_9d20,
        [(0x0052_0338, 0x0026_cd8e), (0x0052_0328, 0x0026_cdca), (0x0052_0318, 0x0026_ce06)],
    );
}
