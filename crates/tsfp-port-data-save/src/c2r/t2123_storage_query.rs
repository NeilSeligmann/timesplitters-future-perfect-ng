// SPDX-License-Identifier: GPL-3.0-or-later
//! 0x25C30 storage free space query, the T2123 hand port (written from the lifted C and the disassembly: the
//! `shrd` statement is beyond `tools.c2rust`, docs/t2049-batch-notes.md). Moved from tsfp-ports
//! `c2r/t2123_manual.rs` (T3785, rule 37), which re-exports it, so the host lane can dispatch to it.
//! Verification tier 1 (weak, FABRICATED inputs): differential vs the lifted original.
use super::rt::*;

pub const ROOTS: [u32; 1] = [0x25c30];

// portreg: va=0x00025c30 tier=1
/// 0x25C30 `game_storage_free_space_query`: cdecl(index, total_out, free_out), returns AL = success of the drive
/// space query (stdcall 0x37D4F9 on the drive of table 0x5655B0 entry `index`, 0x30 byte entries, letter at +3,
/// in-use flag at +0), the byte counts shifted right by 14 and stored as low dwords. Entry +4 gets a status code
/// from 0x37E9A7 (last error): 3 for errors 0x55, 0xE and 0x52, 2 for 0x70, 0xC for 0x3ED, 6 for 0x48F, 1 otherwise.
pub fn game_storage_free_space_query<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.esp = r.esp.wrapping_sub(0x20);
    r.eax = m.read32(r.esp.wrapping_add(0x24));
    r.push(m, r.ebx);
    r.ebx = 0;
    r.push(m, r.esi);
    r.esi = r.eax.wrapping_add(r.eax.wrapping_mul(2)) << 4;
    let index = r.eax;
    for offset in [0x10u32, 0x14, 0x18, 0x1c, 0x20, 0x24, 0x2c] {
        m.write32(r.esp.wrapping_add(offset), 0);
    }
    r.esi = r.esi.wrapping_add(0x0056_55b0);
    'query: {
        if index >= 0xb || m.read8(r.esi) == 0 {
            break 'query;
        }
        r.eax = set_lo8(r.eax, m.read8(r.esi.wrapping_add(3)) as u32);
        if r.eax as u8 == 0 {
            break 'query;
        }
        r.eax = r.eax as u8 as i8 as i32 as u32;
        r.push(m, r.eax);
        r.ecx = r.esp.wrapping_add(0xc);
        r.push(m, 0x0047_60e8);
        r.push(m, r.ecx);
        r.call(guest, 0x003c_85d4);
        r.esp = r.esp.wrapping_add(0xc);
        r.edx = r.esp.wrapping_add(0x20);
        r.push(m, r.edx);
        r.eax = r.esp.wrapping_add(0x1c);
        r.push(m, r.eax);
        r.ecx = r.esp.wrapping_add(0x18);
        r.push(m, r.ecx);
        r.edx = r.esp.wrapping_add(0x14);
        r.push(m, r.edx);
        r.call(guest, 0x0037_d4f9);
        m.write32(r.esp.wrapping_add(0x2c), r.eax);
        r.eax = m.read32(r.esp.wrapping_add(0x34));
        if r.eax != 0 {
            r.ecx = m.read32(r.esp.wrapping_add(0x10));
            r.edx = m.read32(r.esp.wrapping_add(0x14));
            r.ecx = (r.ecx >> 14) | (r.edx << 18);
            r.edx >>= 14;
            m.write32(r.eax, r.ecx);
        }
        r.eax = m.read32(r.esp.wrapping_add(0x30));
        if r.eax != 0 {
            r.ecx = m.read32(r.esp.wrapping_add(0x18));
            r.edx = m.read32(r.esp.wrapping_add(0x1c));
            r.ecx = (r.ecx >> 14) | (r.edx << 18);
            r.edx >>= 14;
            m.write32(r.eax, r.ecx);
        }
    }
    r.call(guest, 0x0037_e9a7);
    let status: u32 = match r.eax {
        0x55 | 0xe | 0x52 => 3,
        0x70 => 2,
        0x3ed => 0xc,
        0x48f => 6,
        _ => 1,
    };
    r.eax = status;
    m.write32(r.esi.wrapping_add(4), status);
    r.eax = set_lo8(r.eax, m.read8(r.esp.wrapping_add(0x2c)) as u32);
    r.esi = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.ebx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
    r.esp = r.esp.wrapping_add(0x20);
}

/// Hybrid entry: true when `va` is this module's root.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x25c30 => game_storage_free_space_query(m, guest, regs),
        _ => return false,
    }
    true
}
