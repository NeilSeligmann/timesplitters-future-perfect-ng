// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-gameplay/modes/tables
//! T1836 wave 11: synthesized-domain roots (T1781/T1782/T1783), translated one statement at a
//! time from the proven C replacements of src/game by tools/t1836_c2rs.py (regenerate, do not
//! edit by hand). Equality with the C is MEASURED by crates/tsfp-c-diff wave11 (120,000 cases at
//! O0/O2/O3), equality with the original XBE is INFERRED. Evidence class of the roots:
//! synthesized-domain (docs/t1836-rust-wave11.md). Placement in this crate is INFERRED.
#![allow(
    unused_mut,
    unused_assignments,
    unused_variables,
    dead_code,
    unused_parens,
    unreachable_code
)]
use super::ObjectMemory as Guest;
include!("../../tsfp-engine-memory/src/cpu.rs");

/// 00012020 cdecl(1). stream_push_dword3.
pub fn stream_push_dword3<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.edx = m.read32(0x55F974u32);
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(c.eax);
    m.write32(c.edx, c.ecx);
    c.ecx = m.read32(0x55F974u32).wrapping_add(4u32);
    m.write32(0x55F974u32, c.ecx);
    c.edx = m.read32(c.eax.wrapping_add(4u32));
    m.write32(c.ecx, c.edx);
    c.ecx = m.read32(0x55F974u32).wrapping_add(4u32);
    m.write32(0x55F974u32, c.ecx);
    c.eax = m.read32(c.eax.wrapping_add(8u32));
    m.write32(c.ecx, c.eax);
    m.write32(0x55F974u32, m.read32(0x55F974u32).wrapping_add(4u32));
}

/// 00022950 cdecl(0). global_563158_equals_word_at_global_56311c_plus4.
pub fn global_563158_equals_word_at_global_56311c_plus4<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x56311Cu32);
    c.edx = m.read32(0x563158u32);
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(c.ecx.wrapping_add(4u32));
    c.eax = u32::from((c.edx == c.esi));
    c.esi = pop(m, c);
}

/// 00024e40 cdecl(1). memory_unit_slot_get_field_0x4.
pub fn memory_unit_slot_get_field_0x4<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.eax = m.read32(48u32.wrapping_mul(c.eax).wrapping_add(0x5655B4u32));
}

/// 00038f80 cdecl(0). slot_count_active_bit0.
pub fn slot_count_active_bit0<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x68C6FCu32);
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.eax = 0u32;
    c.ecx = c.ecx.wrapping_add(0x31Fu32);
    c.esi = 4u32;
    c.edx = ((c.edx & 0xFFFFFF00u32) | 1u32);
    loop {
        if (((m.read8(c.ecx.wrapping_sub(0x1A0u32)) as i32) & ((c.edx as u8) as i32)) != 0) {
            c.eax = c.eax.wrapping_add(1);
        }
        if (((m.read8(c.ecx) as i32) & ((c.edx as u8) as i32)) != 0) {
            c.eax = c.eax.wrapping_add(1);
        }
        if (((m.read8(c.ecx.wrapping_add(0x1A0u32)) as i32) & ((c.edx as u8) as i32)) != 0) {
            c.eax = c.eax.wrapping_add(1);
        }
        if (((m.read8(c.ecx.wrapping_add(0x340u32)) as i32) & ((c.edx as u8) as i32)) != 0) {
            c.eax = c.eax.wrapping_add(1);
        }
        if (((m.read8(c.ecx.wrapping_add(0x4E0u32)) as i32) & ((c.edx as u8) as i32)) != 0) {
            c.eax = c.eax.wrapping_add(1);
        }
        c.ecx = c.ecx.wrapping_add(0x820u32);
        c.esi = c.esi.wrapping_sub(1);
        if !(c.esi != 0u32) {
            break;
        }
    }
    c.esi = pop(m, c);
}

/// 0003eca0 cdecl(1). pool_6b83c8_allocate_locked_block_from_cursor.
pub fn pool_6b83c8_allocate_locked_block_from_cursor<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(0x6B83CCu32);
    c.edx = m.read32(c.ecx.wrapping_add(4u32));
    c.eax = ((c.eax.wrapping_add(0x5Fu32)) & 0xFFFFFFC0u32);
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = c.eax.wrapping_add(0x20u32);
    if (c.esi > c.edx) {
        c.eax = 0u32;
        c.esi = pop(m, c);
        return;
    }
    m.write32(c.ecx.wrapping_add(4u32), c.eax);
    m.write32(c.ecx, 2u32);
    {
        let value = c.ebx;
        push(m, c, value);
    }
    c.esi = 0u32;
    m.write32(c.ecx.wrapping_add(12u32), c.esi);
    m.write32(c.ecx.wrapping_add(16u32), c.esi);
    m.write32(c.ecx.wrapping_add(8u32), c.esi);
    m.write32(0x6B83D0u32, m.read32(0x6B83D0u32).wrapping_add(c.eax));
    m.write32(0x6B83C0u32, m.read32(0x6B83C0u32).wrapping_sub(c.eax));
    c.ebx = m.read32(0x6B83C4u32);
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = c.ecx;
    c.ecx = m.read32(0x6B83CCu32).wrapping_add(c.eax);
    c.ebx = c.ebx.wrapping_sub(c.eax);
    m.write32(0x6B83C4u32, c.ebx);
    c.edx = c.edx.wrapping_sub(c.eax);
    m.write32(0x6B83CCu32, c.ecx);
    c.eax = c.edi.wrapping_add(0x20u32);
    c.edi = pop(m, c);
    c.ebx = pop(m, c);
    m.write32(c.ecx, c.esi);
    m.write32(c.ecx.wrapping_add(8u32), c.esi);
    m.write32(c.ecx.wrapping_add(12u32), c.esi);
    m.write32(c.ecx.wrapping_add(16u32), c.esi);
    m.write32(c.ecx.wrapping_add(4u32), c.edx);
    c.esi = pop(m, c);
}

/// 0003f5e0 cdecl(2). table_6b83e4_entry_get_field_0x28.
pub fn table_6b83e4_entry_get_field_0x28<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(8u32));
    c.ecx = m.read32(c.esp.wrapping_add(4u32));
    c.edx = 48u32.wrapping_mul(c.eax);
    c.ecx = c.ecx.wrapping_shl(4);
    c.eax = m.read32(c.ecx.wrapping_add(0x6B83E4u32));
    c.eax = m.read32(c.eax.wrapping_add(c.edx).wrapping_add(0x28u32));
}

/// 00055ec0 cdecl(0). global_7e71c8_swap_with_6d6058.
// portreg: va=0x00055ec0 tier=1
pub fn global_7e71c8_swap_with_6d6058<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x7E71C8u32);
    c.esp = c.esp.wrapping_sub(0x44u32);
    {
        let value = c.esi;
        push(m, c, value);
    }
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.ecx = 0x11u32;
    c.esi = c.eax;
    c.edi = c.esp.wrapping_add(8u32);
    rep_movsd(m, c);
    c.ecx = 0x11u32;
    c.esi = 0x6D6058u32;
    c.edi = c.eax;
    rep_movsd(m, c);
    c.ecx = 0x11u32;
    c.esi = c.esp.wrapping_add(8u32);
    c.edi = 0x6D6058u32;
    rep_movsd(m, c);
    c.edi = pop(m, c);
    c.esi = pop(m, c);
    c.esp = c.esp.wrapping_add(0x44u32);
}

/// 00058270 cdecl(1). table_7e3954_get_by_mapped_index.
// portreg: va=0x00058270 tier=1
pub fn table_7e3954_get_by_mapped_index<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.eax = ((c.eax & 0xFFFF0000u32)
        | (m.read16(2u32.wrapping_mul(c.eax).wrapping_add(0x7E03C0u32)) as u32));
    if (((c.eax as i16) as i32) < 0) {
        c.eax = 0u32;
        return;
    }
    c.edx = m.read32(0x7E3954u32);
    c.ecx = (((c.eax as i16) as i32) as u32);
    c.eax = m.read32(c.edx.wrapping_add(4u32.wrapping_mul(c.ecx)));
}

/// 000696d0 cdecl(1). slot_7bedc0_is_active.
pub fn slot_7bedc0_is_active<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32)).wrapping_mul(0x1F54u32);
    c.edx = m.read32(c.eax.wrapping_add(0x7BEDC0u32));
    c.ecx = u32::from((c.edx != 0u32));
    c.eax = c.ecx;
}

/// 000697d0 cdecl(3). table_7bedc0_entry_set_fields_0x24_0x28_and_clear_0x34_by_index.
pub fn table_7bedc0_entry_set_fields_0x24_0x28_and_clear_0x34_by_index<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    if (((c.eax as i32) >= 0) && ((c.eax as i32) < 16)) {
        c.eax = c.eax.wrapping_mul(0x1F54u32);
        c.ecx = m.read32(c.eax.wrapping_add(0x7BEDC0u32));
        c.eax = c.eax.wrapping_add(0x7BEDC0u32);
        if (c.ecx == 0u32) {
            c.eax = 0u32;
        }
    } else {
        c.eax = 0u32;
    }
    c.ecx = m.read32(c.eax);
    m.write32(c.ecx.wrapping_add(0x34u32), 0u32);
    c.edx = m.read32(c.eax);
    c.ecx = m.read32(c.esp.wrapping_add(8u32));
    m.write32(c.edx.wrapping_add(0x24u32), c.ecx);
    c.edx = m.read32(c.eax);
    c.eax = m.read32(c.esp.wrapping_add(12u32));
    m.write32(c.edx.wrapping_add(0x28u32), c.eax);
}

/// 00069930 cdecl(2). profile_stats_entry_ptr_stride_0x18_at_0x490_by_slot_and_index.
pub fn profile_stats_entry_ptr_stride_0x18_at_0x490_by_slot_and_index<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    if ((m.read32(0x7DE458u32) & 0x800000u32) != 0) {
        c.eax = m.read32(c.esp.wrapping_add(8u32));
        c.ecx = 0x6F11C0u32;
    } else {
        c.eax = m.read32(c.esp.wrapping_add(4u32)).wrapping_mul(0x1F54u32);
        c.ecx = m.read32(c.eax.wrapping_add(0x7BEDC0u32));
        c.ecx = (if (c.ecx != 0) {
            c.eax.wrapping_add(0x7C0050u32)
        } else {
            0x6F11C0u32
        });
        c.eax = m.read32(c.esp.wrapping_add(8u32));
    }
    c.eax = c
        .ecx
        .wrapping_add(24u32.wrapping_mul(c.eax))
        .wrapping_add(0x490u32);
}

/// 0006b3d0 cdecl(1). find_id_in_three_lists_74c36c.
pub fn find_id_in_three_lists_74c36c<M: Guest>(m: &mut M, c: &mut Cpu) {
    let mut st: u32 = 0;
    'machine: loop {
        match st {
            0 => {
                {
                    let value = c.esi;
                    push(m, c, value);
                }
                c.esi = m.read32(0x74C36Cu32);
                c.edx = m.read32(c.esi.wrapping_add(0x10u32));
                c.eax = 0u32;
                {
                    let value = c.edi;
                    push(m, c, value);
                }
                c.edi = m.read32(c.esp.wrapping_add(12u32));
                if ((c.edx as i32) > 0) {
                    c.ecx = m.read32(c.esi.wrapping_add(12u32));
                    loop {
                        if (m.read32(c.ecx) == c.edi) {
                            {
                                st = 1;
                                continue 'machine;
                            }
                        }
                        c.eax = c.eax.wrapping_add(1);
                        c.ecx = c.ecx.wrapping_add(0x60u32);
                        if !((c.eax as i32) < (c.edx as i32)) {
                            break;
                        }
                    }
                }
                c.edx = m.read32(c.esi.wrapping_add(0x88u32));
                c.eax = 0u32;
                if ((c.edx as i32) > 0) {
                    c.ecx = m.read32(c.esi.wrapping_add(0x84u32));
                    loop {
                        if (m.read32(c.ecx) == c.edi) {
                            {
                                st = 1;
                                continue 'machine;
                            }
                        }
                        c.eax = c.eax.wrapping_add(1);
                        c.ecx = c.ecx.wrapping_add(0x8Cu32);
                        if !((c.eax as i32) < (c.edx as i32)) {
                            break;
                        }
                    }
                }
                c.edx = m.read32(c.esi.wrapping_add(0xE0u32));
                c.eax = 0u32;
                if ((c.edx as i32) > 0) {
                    c.ecx = m.read32(c.esi.wrapping_add(0xDCu32));
                    loop {
                        if (m.read32(c.ecx) == c.edi) {
                            {
                                st = 2;
                                continue 'machine;
                            }
                        }
                        c.eax = c.eax.wrapping_add(1);
                        c.ecx = c.ecx.wrapping_add(0x40u32);
                        if !((c.eax as i32) < (c.edx as i32)) {
                            break;
                        }
                    }
                }
                c.edi = pop(m, c);
                c.esi = pop(m, c);
                c.eax = 0u32;
                return;
                st = 1;
            }
            1 => {
                c.eax = m.read32(c.ecx.wrapping_add(12u32));
                {
                    st = 3;
                    continue 'machine;
                }
                st = 2;
            }
            2 => {
                c.eax = m.read32(c.ecx.wrapping_add(8u32));
                st = 3;
            }
            3 => {
                c.eax = ((((m.read16(2u32.wrapping_mul(c.eax).wrapping_add(0x78B860u32)) as u32)
                    as i16) as i32) as u32);
                c.edx = m.read32(0x791D4Cu32);
                c.ecx = 48u32.wrapping_mul(c.eax);
                c.eax = m.read32(c.ecx.wrapping_add(c.edx).wrapping_add(0x28u32));
                c.edi = pop(m, c);
                c.eax = c.eax.wrapping_shr(25);
                c.esi = pop(m, c);
                c.eax = (c.eax & 1u32);
                break 'machine;
            }
            _ => break 'machine,
        }
    }
}

/// 00071e60 cdecl(0). global_7bb1e8_swap_fields_0x1e0_0x1e4.
pub fn global_7bb1e8_swap_fields_0x1e0_0x1e4<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x7BB1E8u32);
    c.edx = m.read32(c.eax.wrapping_add(0x1E4u32));
    c.ecx = m.read32(c.eax.wrapping_add(0x1E0u32));
    m.write32(c.eax.wrapping_add(0x1E0u32), c.edx);
    c.eax = m.read32(0x7BB1E8u32);
    m.write32(c.eax.wrapping_add(0x1E4u32), c.ecx);
}

/// 00080ad0 cdecl(3). g701cc4_set_fields_0x74_0x78_0x7c.
pub fn g701cc4_set_fields_0x74_0x78_0x7c<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x701CC4u32);
    c.ecx = m.read32(c.esp.wrapping_add(4u32));
    c.edx = m.read32(c.esp.wrapping_add(8u32));
    m.write32(c.eax.wrapping_add(0x74u32), c.ecx);
    c.ecx = m.read32(c.esp.wrapping_add(12u32));
    m.write32(c.eax.wrapping_add(0x78u32), c.edx);
    m.write32(c.eax.wrapping_add(0x7Cu32), c.ecx);
}

/// 00081c20 cdecl(1). record_4b85c8_get_word_0x26_by_slot.
pub fn record_4b85c8_get_word_0x26_by_slot<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(4u32.wrapping_mul(c.eax).wrapping_add(0x7BA040u32));
    c.edx = m.read32(0x4B85C8u32);
    c.ecx = c.ecx.wrapping_mul(0x34u32);
    c.eax = m.read32(c.edx);
    c.eax = (m.read16(c.ecx.wrapping_add(c.eax).wrapping_add(0x26u32)) as u32);
}

/// 000879a0 cdecl(1). table_70878c_entry_has_word_0x16_set.
pub fn table_70878c_entry_has_word_0x16_set<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(0x70878Cu32);
    c.edx = m.read32(c.ecx.wrapping_add(4u32.wrapping_mul(c.eax)));
    c.eax = u32::from(((m.read16(c.edx.wrapping_add(0x16u32)) as u32) != 0xFFFFu32));
}

/// 000879c0 cdecl(1). table_70878c_entry_set_word_0x16_ffff.
// portreg: va=0x000879c0 tier=1
pub fn table_70878c_entry_set_word_0x16_ffff<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(0x70878Cu32);
    c.eax = m.read32(c.ecx.wrapping_add(4u32.wrapping_mul(c.eax)));
    if (c.eax != 0u32) {
        write16(m, c.eax.wrapping_add(0x16u32), (0xFFFFu32 as u16));
    }
}

/// 000879e0 cdecl(1). table_70878c_entry_set_word_0x16_zero.
pub fn table_70878c_entry_set_word_0x16_zero<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(0x70878Cu32);
    c.eax = m.read32(c.ecx.wrapping_add(4u32.wrapping_mul(c.eax)));
    if (c.eax != 0u32) {
        write16(m, c.eax.wrapping_add(0x16u32), (0u32 as u16));
    }
}

/// 0009a390 cdecl(2). list_7b6770_get_items_end_ptr_and_field4.
pub fn list_7b6770_get_items_end_ptr_and_field4<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(c.esp.wrapping_add(4u32));
    c.eax = m.read32(0x7B6770u32);
    c.eax = m.read32(c.eax.wrapping_add(4u32.wrapping_mul(c.ecx)));
    c.edx = m.read32(c.eax);
    c.eax = c
        .eax
        .wrapping_add(4u32.wrapping_mul(c.edx).wrapping_add(0x18u32));
    c.edx = m.read32(c.esp.wrapping_add(8u32));
    m.write32(c.edx, c.eax);
    c.eax = m.read32(0x7B6770u32);
    c.ecx = m.read32(c.eax.wrapping_add(4u32.wrapping_mul(c.ecx)));
    c.eax = m.read32(c.ecx.wrapping_add(4u32));
}

/// 0009a4c0 cdecl(1). entry_id_get_key.
// portreg: va=0x0009a4c0 tier=1
pub fn entry_id_get_key<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    if (c.eax == 0xffff_ffffu32) {
        return;
    }
    if (((c.eax as i32) >= 0x209E) && ((c.eax as i32) < 0x20B2)) {
        c.eax = 0xFFFFFFFEu32;
        return;
    }
    if (((c.eax as i32) >= 0x1F40) && ((c.eax as i32) < 0x209E)) {
        c.ecx = m.read32(0x715F34u32);
        c.eax = (c.eax.wrapping_sub(0x1F40u32)).wrapping_shl(5);
        c.eax = m.read32(c.eax.wrapping_add(c.ecx));
        return;
    }
    c.edx = m.read32(0x715F2Cu32);
    c.eax = c.eax.wrapping_shl(5);
    c.eax = m.read32(c.eax.wrapping_add(c.edx));
}

/// 0009a570 cdecl(1). table_715f30_entry_set_flag_0x10000.
pub fn table_715f30_entry_set_flag_0x10000<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(0x715F30u32);
    c.eax = c.ecx.wrapping_add(20u32.wrapping_mul(c.eax));
    m.write32(c.eax, (m.read32(c.eax) | 0x10000u32));
}

/// 0009c520 cdecl(2). list_7b6770_get_vec3_0xc.
pub fn list_7b6770_get_vec3_0xc<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x7B6770u32);
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.eax = m.read32(c.ecx.wrapping_add(4u32.wrapping_mul(c.eax)));
    c.edx = m.read32(c.eax.wrapping_add(12u32));
    c.ecx = m.read32(c.esp.wrapping_add(8u32));
    m.write32(c.ecx, c.edx);
    c.edx = m.read32(c.eax.wrapping_add(16u32));
    m.write32(c.ecx.wrapping_add(4u32), c.edx);
    c.eax = m.read32(c.eax.wrapping_add(20u32));
    m.write32(c.ecx.wrapping_add(8u32), c.eax);
}

/// 0009c550 cdecl(2). list_7b6770_get_items_ptr_and_first_dword.
pub fn list_7b6770_get_items_ptr_and_first_dword<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x7B6770u32);
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.eax = m.read32(c.ecx.wrapping_add(4u32.wrapping_mul(c.eax)));
    c.ecx = m.read32(c.esp.wrapping_add(8u32));
    c.edx = c.eax.wrapping_add(0x18u32);
    m.write32(c.ecx, c.edx);
    c.eax = m.read32(c.eax);
}

/// 0009e610 cdecl(1). table_791d4c_get_field_0x10.
pub fn table_791d4c_get_field_0x10<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(0x791D4Cu32);
    c.eax = m.read32(
        48u32
            .wrapping_mul(c.eax)
            .wrapping_add(c.ecx)
            .wrapping_add(16u32),
    );
}

/// 000dc270 cdecl(1). table_4deb54_get_field.
pub fn table_4deb54_get_field<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    if (c.eax == 0xffff_ffffu32) {
        c.eax = 0u32;
    } else {
        c.eax = c.eax.wrapping_shl(6);
        c.eax = m.read32(c.eax.wrapping_add(0x4DEB54u32));
    }
}

/// 000dca40 cdecl(2). table_4deb28_copy_vec3_0x4.
pub fn table_4deb28_copy_vec3_0x4<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(c.esp.wrapping_add(8u32));
    c.eax = c.eax.wrapping_shl(6);
    c.edx = m.read32(c.eax.wrapping_add(0x4DEB2Cu32));
    m.write32(c.ecx, c.edx);
    c.edx = m.read32(c.eax.wrapping_add(0x4DEB30u32));
    m.write32(c.ecx.wrapping_add(4u32), c.edx);
    c.eax = m.read32(c.eax.wrapping_add(0x4DEB34u32));
    m.write32(c.ecx.wrapping_add(8u32), c.eax);
}

/// 000dca70 cdecl(0). table_737bd4_collect_entries_with_type_flag_8.
pub fn table_737bd4_collect_entries_with_type_flag_8<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x737BD4u32);
    {
        let value = c.ebx;
        push(m, c, value);
    }
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = 0u32;
    {
        let value = c.edi;
        push(m, c, value);
    }
    m.write32(0x736674u32, c.esi);
    c.eax = c.eax.wrapping_add(0x458u32);
    c.edi = 4u32;
    c.ebx = ((c.ebx & 0xFFFFFF00u32) | 8u32);
    loop {
        c.ecx = m.read32(c.eax.wrapping_sub(0x458u32));
        c.edx = c.eax.wrapping_sub(0x458u32);
        if (c.ecx != 0xffff_ffffu32) {
            c.ecx = c.ecx.wrapping_shl(6);
            if (((m.read8(c.ecx.wrapping_add(0x4DEB54u32)) as i32) & ((c.ebx as u8) as i32)) != 0) {
                m.write32(4u32.wrapping_mul(c.esi).wrapping_add(0x738B78u32), c.edx);
                c.esi = c.esi.wrapping_add(1);
            }
        }
        c.ecx = m.read32(c.eax.wrapping_sub(0x22Cu32));
        c.edx = c.eax.wrapping_sub(0x22Cu32);
        if (c.ecx != 0xffff_ffffu32) {
            c.ecx = c.ecx.wrapping_shl(6);
            if (((m.read8(c.ecx.wrapping_add(0x4DEB54u32)) as i32) & ((c.ebx as u8) as i32)) != 0) {
                m.write32(4u32.wrapping_mul(c.esi).wrapping_add(0x738B78u32), c.edx);
                c.esi = c.esi.wrapping_add(1);
            }
        }
        c.ecx = m.read32(c.eax);
        if (c.ecx != 0xffff_ffffu32) {
            c.ecx = c.ecx.wrapping_shl(6);
            if (((m.read8(c.ecx.wrapping_add(0x4DEB54u32)) as i32) & ((c.ebx as u8) as i32)) != 0) {
                m.write32(4u32.wrapping_mul(c.esi).wrapping_add(0x738B78u32), c.eax);
                c.esi = c.esi.wrapping_add(1);
            }
        }
        c.ecx = m.read32(c.eax.wrapping_add(0x22Cu32));
        c.edx = c.eax.wrapping_add(0x22Cu32);
        if (c.ecx != 0xffff_ffffu32) {
            c.ecx = c.ecx.wrapping_shl(6);
            if (((m.read8(c.ecx.wrapping_add(0x4DEB54u32)) as i32) & ((c.ebx as u8) as i32)) != 0) {
                m.write32(4u32.wrapping_mul(c.esi).wrapping_add(0x738B78u32), c.edx);
                c.esi = c.esi.wrapping_add(1);
            }
        }
        c.ecx = m.read32(c.eax.wrapping_add(0x458u32));
        c.edx = c.eax.wrapping_add(0x458u32);
        if (c.ecx != 0xffff_ffffu32) {
            c.ecx = c.ecx.wrapping_shl(6);
            if (((m.read8(c.ecx.wrapping_add(0x4DEB54u32)) as i32) & ((c.ebx as u8) as i32)) != 0) {
                m.write32(4u32.wrapping_mul(c.esi).wrapping_add(0x738B78u32), c.edx);
                c.esi = c.esi.wrapping_add(1);
            }
        }
        c.eax = c.eax.wrapping_add(0xADCu32);
        c.edi = c.edi.wrapping_sub(1);
        if !(c.edi != 0u32) {
            break;
        }
    }
    c.edi = pop(m, c);
    m.write32(0x736674u32, c.esi);
    c.esi = pop(m, c);
    m.write32(0x4DFB1Cu32, 0u32);
    c.ebx = pop(m, c);
}

/// 000e0250 cdecl(1). table_737bd4_clear_matching_owner_0x104.
// portreg: va=0x000e0250 tier=1
pub fn table_737bd4_clear_matching_owner_0x104<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x737E44u32);
    c.edx = m.read32(0x736680u32);
    c.ecx = c.ecx.wrapping_add(1);
    if ((c.edx as i32) >= (c.ecx as i32)) {
        return;
    }
    c.eax = c.edx.wrapping_mul(0x22Cu32);
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(0x737BD4u32);
    c.eax = c.eax.wrapping_add(c.esi.wrapping_add(0x104u32));
    c.esi = m.read32(c.esp.wrapping_add(8u32));
    c.ecx = c.ecx.wrapping_sub(c.edx);
    loop {
        if (m.read32(c.eax.wrapping_sub(0x104u32)) == 0x10u32) {
            c.edx = m.read32(c.eax);
            if ((c.edx != 0u32) && (c.edx == c.esi)) {
                m.write32(c.eax, 0u32);
            }
        }
        c.eax = c.eax.wrapping_add(0x22Cu32);
        c.ecx = c.ecx.wrapping_sub(1);
        if !(c.ecx != 0u32) {
            break;
        }
    }
    c.esi = pop(m, c);
}

// portreg: va=0x00109990
/// 00109990 cdecl(0). pool_73bd40_collect_types_0x19_0x1a.
pub fn pool_73bd40_collect_types_0x19_0x1a<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x73BD40u32);
    c.edx = 0u32;
    m.write32(c.esp.wrapping_sub(4u32), c.esi);
    m.write32(0x73B920u32, c.edx);
    c.esi = 100u32;
    loop {
        c.ecx = m.read32(c.eax);
        if (((c.ecx as i32) >= 25) && ((c.ecx as i32) <= 26)) {
            m.write32(0x73BBB0u32.wrapping_add(c.edx.wrapping_mul(4u32)), c.eax);
            c.edx = c.edx.wrapping_add(1);
        }
        c.eax = c.eax.wrapping_add(0xD0u32);
        c.esi = c.esi.wrapping_sub(1);
        if !(c.esi != 0u32) {
            break;
        }
    }
    m.write32(0x73B920u32, c.edx);
    m.write32(0x4E60E0u32, 0u32);
    c.esi = m.read32(c.esp.wrapping_sub(4u32));
}

/// 001118a0 cdecl(4). global_73bd68_set_four_vec3_0x9c_to_0xc8.
// portreg: va=0x001118a0
pub fn global_73bd68_set_four_vec3_0x9c_to_0xc8<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x73BD68u32);
    let mut i: u32 = 0u32;
    while (i < 4u32) {
        c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(i)));
        c.edx = m.read32(c.ecx);
        m.write32(
            c.eax
                .wrapping_add(0x9Cu32)
                .wrapping_add(12u32.wrapping_mul(i)),
            c.edx,
        );
        c.edx = m.read32(c.ecx.wrapping_add(4u32));
        m.write32(
            c.eax
                .wrapping_add(0xA0u32)
                .wrapping_add(12u32.wrapping_mul(i)),
            c.edx,
        );
        c.ecx = m.read32(c.ecx.wrapping_add(8u32));
        m.write32(
            c.eax
                .wrapping_add(0xA4u32)
                .wrapping_add(12u32.wrapping_mul(i)),
            c.ecx,
        );
        i = i.wrapping_add(1);
    }
}

/// 00119c20 cdecl(0). global_7ac804_current_entry_get_field_0.
pub fn global_7ac804_current_entry_get_field_0<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x7AC808u32);
    c.ecx = m.read32(0x7AC804u32);
    c.eax = c.eax.wrapping_mul(5u32);
    c.eax = m.read32(c.ecx.wrapping_add(c.eax.wrapping_mul(8u32)));
}

/// 00142990 cdecl(0). current_slot_7a6160_set_field_0x1750_3.
pub fn current_slot_7a6160_set_field_0x1750_3<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x7B0C7Cu32);
    c.eax = m.read32(c.eax.wrapping_add(4u32));
    if ((c.eax as i32) >= 0) {
        c.eax = c.eax.wrapping_mul(0x177Cu32);
        m.write32(c.eax.wrapping_add(0x7A78B0u32), 3u32);
    }
}

/// 00156db0 cdecl(4). node_pool_init_freelist.
// portreg: va=0x00156db0
pub fn node_pool_init_freelist<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(3u32)));
    c.edx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(2u32)));
    m.write32(c.esp.wrapping_sub(4u32), c.esi);
    c.esi = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = 0u32;
    let mut count: u32 = c.eax;
    m.write32(c.esi, c.edx);
    m.write32(c.esi.wrapping_add(8u32), c.eax);
    m.write32(c.esi.wrapping_add(12u32), c.eax);
    if (count != 0u32) {
        m.write32(c.esp.wrapping_sub(8u32), c.ebx);
        c.ebx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
        m.write32(c.esp.wrapping_sub(12u32), c.edi);
        c.edi = count;
        loop {
            c.eax = c.edx;
            c.edx = c.edx.wrapping_add(c.ebx);
            c.edi = c.edi.wrapping_sub(1);
            m.write32(c.eax, c.ecx);
            c.ecx = c.eax;
            if !(c.edi != 0u32) {
                break;
            }
        }
        c.edi = m.read32(c.esp.wrapping_sub(12u32));
        c.ebx = m.read32(c.esp.wrapping_sub(8u32));
    }
    m.write32(c.esi.wrapping_add(4u32), c.ecx);
    c.esi = m.read32(c.esp.wrapping_sub(4u32));
}

/// 00165760 cdecl(0). entry_pool_74c458_reset_free_id_lists_clear_slot_table_and_reset_cursors.
// portreg: va=0x00165760
pub fn entry_pool_74c458_reset_free_id_lists_clear_slot_table_and_reset_cursors<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.ecx = m.read32(0x74C49Cu32);
    c.eax = 0x58u32;
    write16(m, 0x74C45Cu32, (c.eax as u16));
    write16(m, 0x74C45Eu32, (c.eax as u16));
    c.eax = c.ecx.wrapping_add(0x60u32);
    c.ecx = (c.ecx & 0xFFFFu32);
    m.write32(c.esp.wrapping_sub(4u32), c.ebx);
    write16(m, 0x74C460u32, (c.eax as u16));
    c.ecx = c.ecx.wrapping_add(0x60u32);
    c.ebx = 0u32;
    c.eax = 0u32;
    if ((c.ecx as i32) > 0) {
        c.ecx = m.read32(0x74C46Cu32);
        c.edx = m.read32(0x74C480u32);
        m.write32(c.esp.wrapping_sub(8u32), c.ebp);
        m.write32(c.esp.wrapping_sub(12u32), c.esi);
        c.esi = m.read32(0x74C494u32);
        m.write32(c.esp.wrapping_sub(16u32), c.edi);
        c.edi = m.read32(0x74C488u32);
        c.edx = c.edx.wrapping_sub(c.ecx);
        c.esi = c.esi.wrapping_sub(c.ecx);
        loop {
            write16(
                m,
                c.edi.wrapping_add(c.eax.wrapping_mul(2u32)),
                (c.eax as u16),
            );
            c.ebp = (c.eax | 0x10000u32);
            m.write32(c.edx.wrapping_add(c.ecx), c.ebp);
            m.write32(c.ecx, c.ebx);
            m.write32(c.esi.wrapping_add(c.ecx), c.ebx);
            c.ebp = (m.read16(0x74C49Cu32) as u32);
            c.eax = c.eax.wrapping_add(1);
            c.ebp = c.ebp.wrapping_add(0x60u32);
            c.ecx = c.ecx.wrapping_add(4u32);
            if !((c.eax as i32) < (c.ebp as i32)) {
                break;
            }
        }
        c.edi = m.read32(c.esp.wrapping_sub(16u32));
        c.esi = m.read32(c.esp.wrapping_sub(12u32));
        c.ebp = m.read32(c.esp.wrapping_sub(8u32));
    }
    c.eax = m.read32(0x74C464u32).wrapping_add(0x78u32);
    c.ecx = 0x5Du32;
    loop {
        m.write32(c.eax.wrapping_sub(0x20u32), c.ebx);
        m.write32(c.eax, c.ebx);
        m.write32(c.eax.wrapping_add(4u32), c.ebx);
        m.write32(c.eax.wrapping_sub(8u32), c.ebx);
        m.write32(c.eax.wrapping_sub(4u32), c.ebx);
        c.eax = c.eax.wrapping_add(0x80u32);
        c.ecx = c.ecx.wrapping_sub(1);
        if !(c.ecx != 0u32) {
            break;
        }
    }
    c.edx = m.read32(0x74C478u32);
    c.eax = 0xFFFFFFFFu32;
    m.write32(0x74C484u32, c.ebx);
    m.write32(0x74C474u32, c.ebx);
    m.write32(0x74C47Cu32, c.ebx);
    m.write32(0x74C490u32, c.ebx);
    write16(m, 0x74C470u32, (c.ebx as u16));
    m.write32(0x74C450u32, c.eax);
    m.write32(0x74C468u32, c.edx);
    m.write32(0x4E826Cu32, c.eax);
    c.ebx = m.read32(c.esp.wrapping_sub(4u32));
}

/// 0016b170 cdecl(1). table_74c464_slot_is_used.
pub fn table_74c464_slot_is_used<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.edx = m.read32(0x74C464u32);
    c.eax = c.eax.wrapping_shl(7);
    m.write32(c.esp.wrapping_sub(4u32), c.esi);
    c.esi = m.read32(c.eax.wrapping_add(c.edx).wrapping_add(0x58u32));
    c.ecx = u32::from((c.esi != 0u32));
    c.esi = m.read32(c.esp.wrapping_sub(4u32));
    c.eax = c.ecx;
}

/// 0016b190 cdecl(1). table_74c464_free_slot.
pub fn table_74c464_free_slot<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = m.read32(0x74C464u32);
    c.eax = (c.eax.wrapping_shl(7)).wrapping_add(c.ecx);
    write16(
        m,
        0x74C470u32,
        ((m.read16(0x74C470u32) as u32).wrapping_sub(1u32) as u16),
    );
    c.ecx = 0u32;
    m.write32(c.eax.wrapping_add(0x58u32), c.ecx);
    m.write32(c.eax.wrapping_add(0x78u32), c.ecx);
    m.write32(c.eax.wrapping_add(0x7Cu32), c.ecx);
}

/// 0016b7c0 cdecl(2). table_74c464_slot_set_field_0x7c.
pub fn table_74c464_slot_set_field_0x7c<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
    c.edx = m.read32(0x74C464u32);
    c.eax = c.eax.wrapping_shl(7);
    m.write32(c.eax.wrapping_add(c.edx).wrapping_add(0x7Cu32), c.ecx);
}

/// 0017f780 cdecl(0). table_74c510_restore_from_74c514_set_count_74c66c.
pub fn table_74c510_restore_from_74c514_set_count_74c66c<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x74C59Cu32);
    m.write32(c.esp.wrapping_sub(4u32), c.esi);
    c.esi = m.read32(0x74C514u32);
    m.write32(c.esp.wrapping_sub(8u32), c.edi);
    c.edi = m.read32(0x74C510u32);
    c.ecx = 0x5Au32;
    loop {
        let mut value: u32 = m.read32(c.esi);
        m.write32(c.edi, value);
        let mut step: u32 = (if c.df { 0xFFFFFFFCu32 } else { 4u32 });
        c.esi = c.esi.wrapping_add(step);
        c.edi = c.edi.wrapping_add(step);
        c.ecx = c.ecx.wrapping_sub(1);
        if !(c.ecx != 0u32) {
            break;
        }
    }
    c.edi = m.read32(c.esp.wrapping_sub(8u32));
    m.write32(0x74C66Cu32, c.eax);
    c.esi = m.read32(c.esp.wrapping_sub(4u32));
}

/// 00183040 cdecl(1). set_record_24_word_2_to_1_by_index_74c6f8.
pub fn set_record_24_word_2_to_1_by_index_74c6f8<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = m.read32(0x74C6F8u32);
    c.eax = c.eax.wrapping_mul(3u32);
    write16(
        m,
        c.ecx
            .wrapping_add(c.eax.wrapping_mul(8u32))
            .wrapping_add(2u32),
        (1u32 as u16),
    );
}

/// 001b5660 cdecl(2). select_entry_stride_0x7c_at_offset_0x68_or_first_if_id_negative.
pub fn select_entry_stride_0x7c_at_offset_0x68_or_first_if_id_negative<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
    c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.eax = c.eax.wrapping_mul(0x7Cu32);
    c.edx = m.read32(c.eax.wrapping_add(c.ecx).wrapping_add(0x68u32));
    c.eax = c.eax.wrapping_add(c.ecx.wrapping_add(0x68u32));
    if ((c.edx as i32) < 0) {
        c.eax = c.ecx.wrapping_add(0x68u32);
    }
}

// portreg: va=0x001b6740
/// 001b6740 cdecl(1). object_field_0x44_set_and_current_player_flag_4000_or_mode_0xa_record_flag_0x40.
pub fn object_field_0x44_set_and_current_player_flag_4000_or_mode_0xa_record_flag_0x40<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = m.read32(c.eax.wrapping_add(0x44u32));
    if (c.ecx != 0u32) {
        c.eax = m.read32(c.eax.wrapping_add(0x30u32));
        c.ecx = m.read32(0x4F9BACu32);
        c.eax = c.eax.wrapping_mul(0x29Cu32).wrapping_add(c.ecx);
        c.ecx = m.read32(0x7B0C7Cu32);
        c.edx = m.read32(c.ecx.wrapping_add(12u32));
        if ((c.edx & 0x4000u32) != 0u32) {
            c.eax = 1u32;
            return;
        }
        if (((m.read8(0x7DE455u32) as u32) == 10u32)
            && (((m.read8(c.eax.wrapping_add(0x68u32)) as u32) & 0x40u32) != 0u32))
        {
            c.eax = 1u32;
            return;
        }
    }
    c.eax = 0u32;
}

/// 001b6c50 cdecl(1). current_player_object_0x44_matches_selected_slot_204_vs_table_4fa138.
pub fn current_player_object_0x44_matches_selected_slot_204_vs_table_4fa138<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(0x7B0C7Cu32);
    c.ecx = m.read32(c.eax.wrapping_add(0x14u32));
    c.eax = m.read32(c.ecx.wrapping_add(0x7Cu32));
    c.ecx = m.read32(c.eax.wrapping_add(0x80u32));
    if (c.ecx == 0u32) {
        c.edx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
        c.ecx = m.read32(c.edx.wrapping_add(0x44u32));
        if (c.ecx != 0u32) {
            c.ecx = m.read32(c.eax.wrapping_add(0x94u32));
            c.edx = m.read32(c.eax.wrapping_add(0x204u32));
            c.ecx = c.ecx.wrapping_mul(0x3Cu32);
            if (c.edx == m.read32(c.ecx.wrapping_add(0x4FA138u32))) {
                c.eax = 1u32;
                return;
            }
        }
    }
    c.eax = 0u32;
}

// portreg: va=0x001b7cc0
/// 001b7cc0 cdecl(1). object_has_record_29c_with_valid_id_and_current_player_ready.
pub fn object_has_record_29c_with_valid_id_and_current_player_ready<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.eax = m.read32(c.eax.wrapping_add(0x30u32));
    if (c.eax != 0xFFFFFFFFu32) {
        c.ecx = m.read32(0x7B0C7Cu32);
        c.edx = m.read32(c.ecx.wrapping_add(4u32));
        if ((c.edx as i32) >= 0) {
            c.edx = m.read32(c.ecx.wrapping_add(0xE84u32));
            if (c.edx != 0u32) {
                c.ecx = m.read32(0x4F9BACu32);
                c.eax = c.eax.wrapping_mul(0x29Cu32);
                if (m.read32(c.eax.wrapping_add(c.ecx)) != 0xFFFFFFFFu32) {
                    c.eax = 1u32;
                    return;
                }
            }
        }
    }
    c.eax = 0u32;
}

/// 001b90f0 cdecl(0). flags_7de458_0x60000000_clear_and_object_7b0cc0_flag_0x21c_bit2_set.
pub fn flags_7de458_0x60000000_clear_and_object_7b0cc0_flag_0x21c_bit2_set<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    if ((m.read32(0x7DE458u32) & 0x60000000u32) == 0u32) {
        c.eax = m.read32(0x7B0CC0u32);
        if (((m.read8(c.eax.wrapping_add(0x21Cu32)) as u32) & 4u32) != 0u32) {
            c.eax = 1u32;
            return;
        }
    }
    c.eax = 0u32;
}

/// 0020fa50 cdecl(2). object_75c0a0_replace_matching_handle_fields_0x43c_to_0x448.
pub fn object_75c0a0_replace_matching_handle_fields_0x43c_to_0x448<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.ecx = m.read32(0x75C0A0u32);
    c.edx = m.read32(c.ecx.wrapping_add(0x43Cu32));
    m.write32(c.esp.wrapping_sub(4u32), c.ebx);
    m.write32(c.esp.wrapping_sub(8u32), c.ebp);
    m.write32(c.esp.wrapping_sub(12u32), c.esi);
    c.esi = 0u32;
    m.write32(c.esp.wrapping_sub(16u32), c.edi);
    if (c.edx != 0u32) {
        c.ebx = m.read32(0x72F5E8u32);
        c.eax = (c.edx.wrapping_shr(22)).wrapping_mul(0x2E8u32);
        c.edi = m.read32(c.eax.wrapping_add(c.ebx).wrapping_add(0x2Cu32));
        c.eax = c.eax.wrapping_add(c.ebx);
        if (c.edi != c.edx) {
            m.write32(c.ecx.wrapping_add(0x43Cu32), c.esi);
            c.eax = 0u32;
        }
    } else {
        c.eax = 0u32;
    }
    c.ebx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.edi = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
    if (c.eax == c.ebx) {
        c.eax = m.read32(c.edi.wrapping_add(0x2Cu32));
        m.write32(c.ecx.wrapping_add(0x43Cu32), c.eax);
    }
    let mut offset: u32 = 0x440u32;
    while (offset <= 0x448u32) {
        c.edx = m.read32(c.ecx.wrapping_add(offset));
        if (c.edx != c.esi) {
            c.ebp = m.read32(0x72F5E8u32);
            c.eax = (c.edx.wrapping_shr(22))
                .wrapping_mul(0x2E8u32)
                .wrapping_add(c.ebp);
            if (m.read32(c.eax.wrapping_add(0x2Cu32)) != c.edx) {
                m.write32(c.ecx.wrapping_add(offset), c.esi);
                c.eax = 0u32;
            }
        } else {
            c.eax = 0u32;
        }
        if (c.eax == c.ebx) {
            if (offset == 0x444u32) {
                c.eax = m.read32(c.edi.wrapping_add(0x2Cu32));
                m.write32(c.ecx.wrapping_add(offset), c.eax);
            } else {
                c.edx = m.read32(c.edi.wrapping_add(0x2Cu32));
                m.write32(c.ecx.wrapping_add(offset), c.edx);
            }
        }
        offset = offset.wrapping_add(4u32);
    }
    c.edi = m.read32(c.esp.wrapping_sub(16u32));
    c.esi = m.read32(c.esp.wrapping_sub(12u32));
    c.ebp = m.read32(c.esp.wrapping_sub(8u32));
    c.ebx = m.read32(c.esp.wrapping_sub(4u32));
}

/// 00227400 cdecl(1). is_slot_4c_status_above_1_by_index.
pub fn is_slot_4c_status_above_1_by_index<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = m.read32(0x78CECCu32);
    c.eax = c.eax.wrapping_mul(0x4Cu32).wrapping_add(c.ecx);
    c.eax = ((c.eax & 0xFFFFFF00u32) | (m.read8(c.eax.wrapping_add(11u32)) as u32));
    let mut status: u8 = (c.eax as u8);
    c.eax = u32::from((((status as u32) != 0u32) && ((status as u32) != 1u32)));
}

/// 002275f0 cdecl(1). slot_4c_set_status_5_if_status_2_or_3.
pub fn slot_4c_set_status_5_if_status_2_or_3<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = m.read32(0x78CECCu32);
    c.eax = c.eax.wrapping_mul(0x4Cu32).wrapping_add(c.ecx);
    c.ecx = ((c.ecx & 0xFFFFFF00u32) | (m.read8(c.eax.wrapping_add(11u32)) as u32));
    if ((((c.ecx as u8) as u32) == 2u32) || (((c.ecx as u8) as u32) == 3u32)) {
        m.write8(c.eax.wrapping_add(11u32), (5u32 as u8));
    }
}

/// 00227620 cdecl(1). slot_4c_demote_status_2_or_3_to_1_decrement_type_count.
pub fn slot_4c_demote_status_2_or_3_to_1_decrement_type_count<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = m.read32(0x78CECCu32);
    c.eax = c.eax.wrapping_mul(0x4Cu32).wrapping_add(c.ecx);
    c.ecx = ((c.ecx & 0xFFFFFF00u32) | (m.read8(c.eax.wrapping_add(11u32)) as u32));
    if ((((c.ecx as u8) as u32) == 2u32) || (((c.ecx as u8) as u32) == 3u32)) {
        m.write8(c.eax.wrapping_add(11u32), (1u32 as u8));
        c.eax = (m.read8(c.eax.wrapping_add(8u32)) as u32);
        let mut counter: u32 = c.eax.wrapping_mul(4u32).wrapping_add(0x75C134u32);
        m.write32(counter, m.read32(counter).wrapping_sub(1u32));
    }
}

/// 00227880 cdecl(1). slot_4c_current_add_unique_dword_to_list_of_10_at_data_plus_4.
pub fn slot_4c_current_add_unique_dword_to_list_of_10_at_data_plus_4<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    let mut st: u32 = 0;
    let mut first: u32 = Default::default();
    'machine: loop {
        match st {
            0 => {
                c.eax = m.read32(0x78CED0u32);
                c.ecx = m.read32(0x78CECCu32);
                c.eax = c.eax.wrapping_mul(0x4Cu32);
                c.edx = m.read32(c.eax.wrapping_add(c.ecx).wrapping_add(12u32));
                m.write32(c.esp.wrapping_sub(4u32), c.esi);
                c.esi = m.read32(c.edx);
                m.write32(c.esp.wrapping_sub(8u32), c.edi);
                c.edi = m.read32(c.edx.wrapping_add(4u32));
                c.ecx = c.edx.wrapping_add(4u32);
                c.eax = 0u32;
                first = c.edi;
                c.edi = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
                if (first != c.esi) {
                    loop {
                        if (m.read32(c.ecx) == c.edi) {
                            {
                                st = 1;
                                continue 'machine;
                            }
                        }
                        c.eax = c.eax.wrapping_add(1);
                        c.ecx = c.ecx.wrapping_add(4u32);
                        if ((c.eax as i32) >= 10) {
                            {
                                st = 1;
                                continue 'machine;
                            }
                        }
                        if !(m.read32(c.ecx) != c.esi) {
                            break;
                        }
                    }
                }
                m.write32(
                    c.edx
                        .wrapping_add(c.eax.wrapping_mul(4u32))
                        .wrapping_add(4u32),
                    c.edi,
                );
                st = 1;
            }
            1 => {
                c.edi = m.read32(c.esp.wrapping_sub(8u32));
                c.esi = m.read32(c.esp.wrapping_sub(4u32));
                break 'machine;
            }
            _ => break 'machine,
        }
    }
}

/// 002278e0 cdecl(1). slot_4c_current_add_unique_id_to_minus1_terminated_list_at_0x10.
pub fn slot_4c_current_add_unique_id_to_minus1_terminated_list_at_0x10<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    let mut st: u32 = 0;
    'machine: loop {
        match st {
            0 => {
                c.ecx = m.read32(0x78CED0u32).wrapping_mul(0x4Cu32);
                m.write32(c.esp.wrapping_sub(4u32), c.ebx);
                m.write32(c.esp.wrapping_sub(8u32), c.esi);
                c.ecx = c.ecx.wrapping_add(m.read32(0x78CECCu32));
                c.edx = m.read32(c.ecx.wrapping_add(0x10u32));
                c.eax = c.ecx.wrapping_add(0x10u32);
                c.esi = 0u32;
                m.write32(c.esp.wrapping_sub(12u32), c.edi);
                c.edi = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
                if (c.edx != 0xFFFFFFFFu32) {
                    c.edx = c.eax;
                    loop {
                        if (m.read32(c.edx) == c.edi) {
                            {
                                st = 1;
                                continue 'machine;
                            }
                        }
                        c.ebx = m.read32(c.eax.wrapping_add(4u32));
                        c.eax = c.eax.wrapping_add(4u32);
                        c.esi = c.esi.wrapping_add(1);
                        c.edx = c.eax;
                        if !(c.ebx != 0xFFFFFFFFu32) {
                            break;
                        }
                    }
                }
                m.write32(
                    c.ecx
                        .wrapping_add(c.esi.wrapping_mul(4u32))
                        .wrapping_add(0x10u32),
                    c.edi,
                );
                st = 1;
            }
            1 => {
                c.edi = m.read32(c.esp.wrapping_sub(12u32));
                c.esi = m.read32(c.esp.wrapping_sub(8u32));
                c.ebx = m.read32(c.esp.wrapping_sub(4u32));
                break 'machine;
            }
            _ => break 'machine,
        }
    }
}

/// 00227980 cdecl(5). slot_4c_lookup_dword_in_data_table_at_0x54_by_id_and_sub_index.
pub fn slot_4c_lookup_dword_in_data_table_at_0x54_by_id_and_sub_index<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    let mut st: u32 = 0;
    'machine: loop {
        match st {
            0 => {
                c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
                c.ecx = m.read32(0x78CECCu32);
                c.eax = c.eax.wrapping_mul(0x4Cu32);
                m.write32(c.esp.wrapping_sub(4u32), c.esi);
                c.esi = m.read32(c.eax.wrapping_add(c.ecx).wrapping_add(12u32));
                m.write32(c.esp.wrapping_sub(8u32), c.edi);
                c.edi = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
                c.ecx = 0u32;
                c.edx = 0u32;
                c.eax = c.esi.wrapping_add(0x2Cu32);
                loop {
                    if (c.edi == m.read32(c.eax.wrapping_sub(0x28u32))) {
                        c.edx =
                            m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(2u32)));
                        c.ecx = c.ecx.wrapping_add(c.edx);
                        c.eax = m.read32(
                            c.esi
                                .wrapping_add(c.ecx.wrapping_mul(4u32))
                                .wrapping_add(0x54u32),
                        );
                        {
                            st = 1;
                            continue 'machine;
                        }
                    }
                    c.ecx = c.ecx.wrapping_add(m.read32(c.eax));
                    c.edx = c.edx.wrapping_add(1);
                    c.eax = c.eax.wrapping_add(4u32);
                    if !((c.edx as i32) < 10) {
                        break;
                    }
                }
                c.eax = 0u32;
                st = 1;
            }
            1 => {
                c.edi = m.read32(c.esp.wrapping_sub(8u32));
                c.esi = m.read32(c.esp.wrapping_sub(4u32));
                break 'machine;
            }
            _ => break 'machine,
        }
    }
}

/// 00227a30 cdecl(0). clear_byte_48_of_current_slot_4c.
pub fn clear_byte_48_of_current_slot_4c<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x78CED0u32);
    c.ecx = m.read32(0x78CECCu32);
    c.eax = c.eax.wrapping_mul(0x4Cu32);
    m.write8(
        c.eax.wrapping_add(c.ecx).wrapping_add(0x48u32),
        (0u32 as u8),
    );
}

/// 00227a50 cdecl(1). set_dword_3c_of_current_slot_4c_from_arg.
// portreg: va=0x00227a50
pub fn set_dword_3c_of_current_slot_4c_from_arg<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x78CED0u32);
    c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.eax = c.eax.wrapping_mul(0x4Cu32);
    c.edx = m.read32(0x78CECCu32);
    m.write32(c.eax.wrapping_add(c.edx).wrapping_add(0x3Cu32), c.ecx);
}

/// 00228fb0 cdecl(0). zero_fill_buffer_75cc8c_and_set_flag_0x8000_if_state_7de470_4.
pub fn zero_fill_buffer_75cc8c_and_set_flag_0x8000_if_state_7de470_4<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.edx = m.read32(0x75CC8Cu32);
    m.write32(c.esp.wrapping_sub(4u32), c.edi);
    c.eax = 0u32;
    c.ecx = 0x675u32;
    c.edi = c.edx;
    loop {
        m.write32(c.edi, c.eax);
        c.edi = c
            .edi
            .wrapping_add((if c.df { 0xFFFFFFFCu32 } else { 4u32 }));
        c.ecx = c.ecx.wrapping_sub(1);
        if !(c.ecx != 0u32) {
            break;
        }
    }
    c.ecx = 0x672u32;
    c.edi = c.edx.wrapping_add(12u32);
    loop {
        m.write32(c.edi, c.eax);
        c.edi = c
            .edi
            .wrapping_add((if c.df { 0xFFFFFFFCu32 } else { 4u32 }));
        c.ecx = c.ecx.wrapping_sub(1);
        if !(c.ecx != 0u32) {
            break;
        }
    }
    let mut mode: u32 = m.read32(0x7DE470u32);
    c.edi = m.read32(c.esp.wrapping_sub(4u32));
    if (mode == 4u32) {
        c.eax = m.read32(0x7B0C48u32);
        m.write32(
            c.eax.wrapping_add(12u32),
            (m.read32(c.eax.wrapping_add(12u32)) | 0x8000u32),
        );
    }
}

/// 0022bdc0 cdecl(0). get_3_minus_dword_28_of_object_75cc98.
pub fn get_3_minus_dword_28_of_object_75cc98<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x75CC98u32);
    c.edx = m.read32(c.ecx.wrapping_add(0x28u32));
    c.eax = 3u32.wrapping_sub(c.edx);
}

/// 0022d110 cdecl(3). get_text_line_ptrs_75c770_and_75c370_by_index_null_if_empty.
pub fn get_text_line_ptrs_75c770_and_75c370_by_index_null_if_empty<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.edx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(2u32)));
    c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.eax = c.edx.wrapping_mul(0xFFu32);
    m.write32(c.esp.wrapping_sub(4u32), c.esi);
    c.esi = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
    m.write32(c.ecx, 0u32);
    c.eax = c.eax.wrapping_add(0x75C770u32);
    m.write32(c.esi, 0u32);
    if ((m.read8(c.eax) as u32) != 0u32) {
        m.write32(c.ecx, c.eax);
    }
    c.ecx = m.read32(0x7B0C7Cu32);
    c.eax = m.read32(c.ecx.wrapping_add(4u32));
    c.eax = (c.edx.wrapping_add(c.eax.wrapping_mul(2u32))).wrapping_mul(0xFFu32);
    c.ecx = ((c.ecx & 0xFFFFFF00u32) | (m.read8(c.eax.wrapping_add(0x75C370u32)) as u32));
    c.eax = c.eax.wrapping_add(0x75C370u32);
    if (((c.ecx as u8) as u32) != 0u32) {
        m.write32(c.esi, c.eax);
    }
    c.esi = m.read32(c.esp.wrapping_sub(4u32));
}

/// 0022d410 cdecl(2). get_text_line_ptrs_75c988_and_75ca88_null_if_empty.
pub fn get_text_line_ptrs_75c988_and_75ca88_null_if_empty<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
    c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
    m.write32(c.eax, 0u32);
    m.write32(c.ecx, 0u32);
    c.edx = ((c.edx & 0xFFFFFF00u32) | (m.read8(0x75C988u32) as u32));
    if (((c.edx as u8) as u32) != 0u32) {
        m.write32(c.eax, 0x75C988u32);
    }
    c.eax = m.read32(0x7B0C7Cu32);
    c.eax = m.read32(c.eax.wrapping_add(4u32)).wrapping_mul(0xFFu32);
    c.edx = ((c.edx & 0xFFFFFF00u32) | (m.read8(c.eax.wrapping_add(0x75CA88u32)) as u32));
    c.eax = c.eax.wrapping_add(0x75CA88u32);
    if (((c.edx as u8) as u32) != 0u32) {
        m.write32(c.ecx, c.eax);
    }
}

/// 0022d460 cdecl(3). get_text_line_ptrs_75c988_75ca88_75c160_null_if_empty.
pub fn get_text_line_ptrs_75c988_75ca88_75c160_null_if_empty<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(c.esp.wrapping_add(8u32));
    c.edx = m.read32(c.esp.wrapping_add(12u32));
    m.write32(c.eax, 0u32);
    m.write32(c.ecx, 0u32);
    {
        let value = c.ebx;
        push(m, c, value);
    }
    m.write32(c.edx, 0u32);
    let mut flag: u8 = m.read8(0x75C988u32);
    c.ebx = ((c.ebx & 0xFFFFFF00u32) | (flag as u32));
    if ((flag as u32) != 0u32) {
        m.write32(c.eax, 0x75C988u32);
    }
    c.eax = m.read32(0x7B0C7Cu32);
    c.eax = m.read32(c.eax.wrapping_add(4u32));
    c.eax = c.eax.wrapping_mul(0xFFu32);
    flag = m.read8(c.eax.wrapping_add(0x75CA88u32));
    c.ebx = ((c.ebx & 0xFFFFFF00u32) | (flag as u32));
    c.eax = c.eax.wrapping_add(0x75CA88u32);
    c.ebx = pop(m, c);
    if ((flag as u32) != 0u32) {
        m.write32(c.ecx, c.eax);
    }
    c.ecx = m.read32(0x7B0C7Cu32);
    c.eax = m.read32(c.ecx.wrapping_add(4u32));
    c.eax = c.eax.wrapping_mul(0xFFu32);
    flag = m.read8(c.eax.wrapping_add(0x75C160u32));
    c.ecx = ((c.ecx & 0xFFFFFF00u32) | (flag as u32));
    c.eax = c.eax.wrapping_add(0x75C160u32);
    if ((flag as u32) != 0u32) {
        m.write32(c.edx, c.eax);
    }
}

/// 00230cc0 cdecl(1). get_dword_14_of_descriptor_by_index_from_tables_510788_513d04.
pub fn get_dword_14_of_descriptor_by_index_from_tables_510788_513d04<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    if ((c.eax as i32) < 0x15) {
        c.eax = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x510788u32));
    } else {
        c.eax = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x513D04u32));
    }
    c.eax = m.read32(c.eax.wrapping_add(0x14u32));
}

/// 002743d0 cdecl(1). slot_object_self_owned_with_state_flag_a0_bit0.
pub fn slot_object_self_owned_with_state_flag_a0_bit0<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(c.esp.wrapping_add(4u32));
    c.eax = m.read32(c.ecx.wrapping_mul(8u32).wrapping_add(0x7A2BE4u32));
    c.eax = m.read32(c.eax.wrapping_add(0x7Cu32));
    if ((m.read32(c.eax.wrapping_add(0x1Cu32)) == c.ecx)
        && (((m.read8(c.eax.wrapping_add(0xA0u32)) as u32) & 1u32) != 0u32))
    {
        c.eax = 1u32;
    } else {
        c.eax = 0u32;
    }
}

/// 002744a0 cdecl(1). object_table_7a2be4_ext_field_3c.
pub fn object_table_7a2be4_ext_field_3c<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(c.eax.wrapping_mul(8u32).wrapping_add(0x7A2BE4u32));
    c.edx = m.read32(c.ecx.wrapping_add(0x7Cu32));
    c.eax = m.read32(c.edx.wrapping_add(0x3Cu32));
}

/// 00274590 cdecl(0). recount_characters_per_side_into_table.
pub fn recount_characters_per_side_into_table<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x74C3B8u32);
    c.eax = 0u32;
    m.write32(c.ecx, c.eax);
    c.edx = m.read32(0x74C3B8u32);
    m.write32(c.edx.wrapping_add(4u32), c.eax);
    c.ecx = m.read32(0x74C3B8u32);
    m.write32(c.ecx.wrapping_add(0x10u32), c.eax);
    c.edx = m.read32(0x74C3B8u32);
    m.write32(c.edx.wrapping_add(0x14u32), c.eax);
    c.ecx = m.read32(0x790950u32);
    if ((c.ecx as i32) > (c.eax as i32)) {
        loop {
            c.ecx = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x7A3580u32));
            c.edx = m.read32(c.ecx.wrapping_add(0x84u32));
            c.ecx = m.read32(0x74C3B8u32);
            c.edx = c.edx.wrapping_shl(4);
            c.ecx = c.edx.wrapping_add(c.ecx).wrapping_sub(0xCu32);
            m.write32(c.ecx, m.read32(c.ecx).wrapping_add(1u32));
            c.ecx = m.read32(0x790950u32);
            c.eax = c.eax.wrapping_add(1);
            if !((c.eax as i32) < (c.ecx as i32)) {
                break;
            }
        }
    }
    c.eax = c.ecx;
    if ((c.eax as i32) < (m.read32(0x74C37Cu32) as i32)) {
        loop {
            c.edx = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x7A3580u32));
            c.ecx = m.read32(c.edx.wrapping_add(0x84u32));
            c.edx = m.read32(0x74C3B8u32);
            c.ecx = c.ecx.wrapping_shl(4);
            c.ecx = c.ecx.wrapping_add(c.edx).wrapping_sub(0x10u32);
            m.write32(c.ecx, m.read32(c.ecx).wrapping_add(1u32));
            c.ecx = m.read32(0x74C37Cu32);
            c.eax = c.eax.wrapping_add(1);
            if !((c.eax as i32) < (c.ecx as i32)) {
                break;
            }
        }
    }
}

/// 00274620 cdecl(0). clear_256_byte_block_74c3b8_and_set_fields_8_18_to_minus1.
pub fn clear_256_byte_block_74c3b8_and_set_fields_8_18_to_minus1<M: Guest>(m: &mut M, c: &mut Cpu) {
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m.read32(0x74C3B8u32);
    c.eax = 0u32;
    c.ecx = 0x40u32;
    rep_stosd(m, c);
    c.ecx = m.read32(0x74C3B8u32);
    c.eax = (c.eax | 0xFFFFFFFFu32);
    m.write32(c.ecx.wrapping_add(8u32), c.eax);
    c.edx = m.read32(0x74C3B8u32);
    m.write32(c.edx.wrapping_add(0x18u32), c.eax);
    c.edi = pop(m, c);
}

/// 002be670 cdecl(1). record_30_field_c_by_index.
pub fn record_30_field_c_by_index<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.eax = c.eax.wrapping_mul(3u32);
    c.eax = c.eax.wrapping_shl(4);
    c.eax = m.read32(c.eax.wrapping_add(0x521A24u32));
}

/// 002bea00 cdecl(1). table_4d239c_entry_by_row_arg_and_column_by_state_78ad2c.
pub fn table_4d239c_entry_by_row_arg_and_column_by_state_78ad2c<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x78AD2Cu32);
    c.ecx = 0u32;
    if ((c.eax == 5u32) && ((m.read8(0x78ADAEu32) as u32) == 0x10u32)) {
        c.ecx = 1u32;
    }
    c.eax = c.eax.wrapping_sub(0xBu32);
    if (c.eax == 0u32) {
        c.ecx = 2u32;
    } else {
        c.eax = c.eax.wrapping_sub(2u32);
        if (c.eax == 0u32) {
            c.eax = m.read32(c.esp.wrapping_add(4u32));
            c.eax = c.eax.wrapping_mul(0xFu32);
            c.ecx = 3u32;
            c.eax = c.eax.wrapping_add(c.ecx);
            c.eax = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x4D239Cu32));
            return;
        }
    }
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.eax = c.eax.wrapping_mul(0xFu32);
    c.eax = c.eax.wrapping_add(c.ecx);
    c.eax = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x4D239Cu32));
}

/// 002db4d0 cdecl(0). copy_buffer_789824_to_789820.
pub fn copy_buffer_789824_to_789820<M: Guest>(m: &mut M, c: &mut Cpu) {
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(0x789824u32);
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m.read32(0x789820u32);
    c.ecx = 0xEE0Au32;
    rep_movsd(m, c);
    c.edi = pop(m, c);
    c.esi = pop(m, c);
}

/// 002db4f0 cdecl(0). copy_buffer_789820_to_789824.
pub fn copy_buffer_789820_to_789824<M: Guest>(m: &mut M, c: &mut Cpu) {
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(0x789820u32);
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m.read32(0x789824u32);
    c.ecx = 0xEE0Au32;
    rep_movsd(m, c);
    c.edi = pop(m, c);
    c.esi = pop(m, c);
}

/// 002f7770 cdecl(1). record_c4_type_descriptor_524be0_by_index.
pub fn record_c4_type_descriptor_524be0_by_index<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.eax = c.eax.wrapping_mul(0xC4u32);
    c.eax = ((((m.read16(c.eax.wrapping_add(0x524F5Au32)) as u32) as i16) as i32) as u32);
    c.eax = c.eax.wrapping_add(c.eax.wrapping_mul(2u32));
    c.eax = c.eax.wrapping_mul(8u32).wrapping_add(0x524BE0u32);
}

/// 002fb1f0 cdecl(1). set_fields_2f2e_2f30_264_of_7844a8_from_word_triplet_arg.
pub fn set_fields_2f2e_2f30_264_of_7844a8_from_word_triplet_arg<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    let mut word: u16 = m.read16(c.eax);
    c.ecx = ((c.ecx & 0xFFFF0000u32) | (word as u32));
    c.edx = m.read32(0x7844A8u32);
    write16(m, c.edx.wrapping_add(0x2F2Eu32), word);
    word = m.read16(c.eax.wrapping_add(2u32));
    c.ecx = ((c.ecx & 0xFFFF0000u32) | (word as u32));
    c.edx = m.read32(0x7844A8u32);
    write16(m, c.edx.wrapping_add(0x2F30u32), word);
    c.eax = ((((m.read16(c.eax.wrapping_add(4u32)) as u32) as i16) as i32) as u32);
    c.ecx = m.read32(0x7844A8u32);
    m.write32(c.ecx.wrapping_add(0x264u32), c.eax);
}

/// 002fc0a0 cdecl(0). sync_global_784020_with_field_204_of_7844a8_and_mark_dirty_bits_0x12.
pub fn sync_global_784020_with_field_204_of_7844a8_and_mark_dirty_bits_0x12<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(0x7844A8u32);
    c.ecx = m.read32(c.eax.wrapping_add(0x204u32));
    if (c.ecx != m.read32(0x784020u32)) {
        m.write32(0x784020u32, c.ecx);
        m.write32(
            c.eax.wrapping_add(0x238u32),
            (m.read32(c.eax.wrapping_add(0x238u32)) | 0x12u32),
        );
    }
}

/// 003118b0 cdecl(2). record_58_copy_60_bytes_from_field_10.
pub fn record_58_copy_60_bytes_from_field_10<M: Guest>(m: &mut M, c: &mut Cpu) {
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(c.esp.wrapping_add(0xCu32));
    c.esi = c.esi.wrapping_mul(0x58u32);
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m.read32(c.esp.wrapping_add(0xCu32));
    c.esi = c.esi.wrapping_add(0x52CF48u32);
    c.ecx = 0xFu32;
    rep_movsd(m, c);
    c.edi = pop(m, c);
    c.esi = pop(m, c);
}

/// 0031db00 cdecl(1). is_count_of_records_b4_with_dword_equal_arg_below_limit_for_kind.
pub fn is_count_of_records_b4_with_dword_equal_arg_below_limit_for_kind<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.ecx = m.read32(0x7844A8u32);
    c.eax = m.read32(c.ecx.wrapping_add(0x81A4u32));
    c.edx = m.read32(c.esp.wrapping_add(4u32));
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = 0u32;
    if ((c.eax as i32) > 0) {
        c.ecx = c.ecx.wrapping_add(0x81A8u32);
        loop {
            if (m.read32(c.ecx) == c.edx) {
                c.esi = c.esi.wrapping_add(1);
            }
            c.ecx = c.ecx.wrapping_add(0xB4u32);
            c.eax = c.eax.wrapping_sub(1);
            if !(c.eax != 0u32) {
                break;
            }
        }
    }
    c.eax = c.edx;
    if (c.eax == 0u32) {
        c.eax = 0x1Eu32;
    } else {
        c.eax = c.eax.wrapping_sub(1);
        if (c.eax == 0u32) {
            c.eax = 0x32u32;
        } else {
            c.eax = c.eax.wrapping_sub(1);
            c.eax = (if (c.eax == 0u32) { 0x32u32 } else { 0u32 });
        }
    }
    c.ecx = u32::from(((c.esi as i32) < (c.eax as i32)));
    c.esi = pop(m, c);
    c.eax = c.ecx;
}

/// 0031f220 cdecl(0). max_field_ac_of_record_b4_over_active_record_220_members.
pub fn max_field_ac_of_record_b4_over_active_record_220_members<M: Guest>(m: &mut M, c: &mut Cpu) {
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(0x7844A8u32);
    c.ecx = m.read32(c.esi.wrapping_add(0x41E0u32));
    c.eax = 0u32;
    if ((c.ecx as i32) > 0) {
        {
            let value = c.edi;
            push(m, c, value);
        }
        c.edx = c.esi.wrapping_add(0x41E4u32);
        c.edi = c.ecx;
        loop {
            if ((m.read8(c.edx.wrapping_add(4u32)) as u32) == 1u32) {
                c.ecx = m.read32(c.edx);
                c.ecx = c.ecx.wrapping_mul(0xB4u32);
                c.ecx = c.ecx.wrapping_add(c.esi).wrapping_add(0x81A8u32);
                c.ecx = m.read32(c.ecx.wrapping_add(0xACu32));
                if ((c.eax as i32) < (c.ecx as i32)) {
                    c.eax = c.ecx;
                }
            }
            c.edx = c.edx.wrapping_add(0x220u32);
            c.edi = c.edi.wrapping_sub(1);
            if !(c.edi != 0u32) {
                break;
            }
        }
        c.edi = pop(m, c);
    }
    c.esi = pop(m, c);
}
