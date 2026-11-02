// SPDX-License-Identifier: GPL-3.0-or-later
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
use crate::memory::GuestMemory as Guest;
include!("../../tsfp-engine-memory/src/cpu.rs");

/// 00139160 cdecl(0). hud_local_player_slot_state_not_0xc_or_0x11_and_flag_0x40_clear.
pub fn hud_local_player_slot_state_not_0xc_or_0x11_and_flag_0x40_clear<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.ecx = m.read32(0x7B0C7Cu32);
    c.eax = m.read32(c.ecx.wrapping_add(4u32));
    if ((c.eax as i32) < 0) {
        c.eax = 0u32;
        return;
    }
    c.edx = ((c.edx & 0xFFFFFF00u32) | (m.read8(c.ecx.wrapping_add(12u32)) as u32));
    c.eax = c.eax.wrapping_mul(0x177Cu32).wrapping_add(0x7A6160u32);
    if ((c.edx & 0x40u32) != 0u32) {
        c.eax = 0u32;
        return;
    }
    c.eax = m.read32(c.eax);
    c.eax = (if ((c.eax == 12u32) || (c.eax == 17u32)) {
        0u32
    } else {
        1u32
    });
}

/// 002fa0d0 cdecl(0). mapedit_resource_cost_of_current_selected_kind.
pub fn mapedit_resource_cost_of_current_selected_kind<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x7844A8u32);
    c.eax = m.read32(c.ecx.wrapping_add(0x2F34u32));
    if ((c.eax as i32) < 0) {
        c.eax = 0u32;
        return;
    }
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x762AE0u32));
    c.edx = 1u32;
    let mut slot_is_one: bool = (c.esi == c.edx);
    c.esi = pop(m, c);
    if (slot_is_one && (c.eax != 0x45u32)) {
        c.eax = 0x400u32;
        return;
    }
    c.eax = m.read32(c.ecx.wrapping_add(0x2F34u32));
    m.write32(c.eax.wrapping_mul(4u32).wrapping_add(0x762AE0u32), c.edx);
    c.eax = m.read32(c.ecx.wrapping_add(0x2F34u32));
    if ((c.eax == 0x35u32) || (c.eax == 0x36u32)) {
        c.eax = 0x12Cu32;
        return;
    }
    if (((c.eax == 0x32u32) || (c.eax == 0x33u32)) || (c.eax == 0x34u32)) {
        c.eax = 0xFAu32;
        return;
    }
    c.ecx = u32::from((c.eax == 0x45u32));
    c.ecx = c.ecx.wrapping_sub(1);
    c.ecx = (c.ecx & 0xA000u32);
    c.eax = c.ecx;
}

/// 002fb9d0 cdecl(2). mapedit_cell_at_packed_xy_is_free_in_selected_node_plane.
pub fn mapedit_cell_at_packed_xy_is_free_in_selected_node_plane<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.edx = m.read32(0x7844A8u32);
    c.ecx = m.read32(c.edx.wrapping_add(0x2F18u32));
    c.eax = 0u32;
    if (c.ecx == 0u32) {
        return;
    }
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(c.edx.wrapping_add(0x2F80u32));
    if (c.esi == 0u32) {
        c.esi = pop(m, c);
        return;
    }
    c.edx = m.read32(c.ecx.wrapping_add(0xCu32));
    c.ecx = m.read32(
        c.ecx
            .wrapping_add(c.edx.wrapping_mul(4u32))
            .wrapping_add(0x2Cu32),
    );
    c.esi = ((c.esi & 0xFFFF0000u32) | (m.read16(c.esp.wrapping_add(0xCu32)) as u32));
    c.edx = (((((c.esi as u16) as u32) as i16) as i32) as u32);
    {
        let value = c.edi;
        push(m, c, value);
    }
    if !(((c.edx as i32) >= (m.read32(c.ecx.wrapping_add(0x1Cu32)) as i32))
        || (((((c.esi as u16) as u32) as i16) as i32) < 0))
    {
        c.esi = m.read32(c.esp.wrapping_add(0xCu32));
        c.edi = ((((m.read16(c.esp.wrapping_add(0xCu32)) as u32) as i16) as i32) as u32);
        c.esi = ((c.esi as i32).wrapping_shr(16) as u32);
        c.esi =
            ((c.esi as i32).wrapping_mul((m.read32(c.ecx.wrapping_add(0x14u32)) as i32)) as u32);
        c.ecx = m.read32(
            c.ecx
                .wrapping_add(c.edx.wrapping_mul(4u32))
                .wrapping_add(0x20u32),
        );
        c.esi = c.esi.wrapping_add(c.edi);
        if ((m.read32(c.ecx.wrapping_add(c.esi.wrapping_mul(4u32))) & 0x1FC0u32) == 0u32) {
            c.eax = 1u32;
        }
    }
    c.edi = pop(m, c);
    c.esi = pop(m, c);
}

/// 002fe4e0 cdecl(1). mapedit_find_change_entry_index_by_key_word.
pub fn mapedit_find_change_entry_index_by_key_word<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.ecx = m.read32(0x7844A8u32);
    c.edx = (m.read16(c.ecx.wrapping_add(0x306Eu32)) as u32);
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.eax = 0u32;
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.eax = 0xFFFFFFFFu32;
    if ((c.edx as i32) > 0) {
        c.esi = m.read32(c.esp.wrapping_add(0xCu32));
        c.ecx = c.ecx.wrapping_add(0x307Au32);
        c.eax = 0u32;
        loop {
            c.edi = ((((m.read16(c.ecx) as u32) as i16) as i32) as u32);
            if (c.edi == c.esi) {
                break;
            }
            c.eax = c.eax.wrapping_add(1);
            c.ecx = c.ecx.wrapping_add(0xCu32);
            if ((c.eax as i32) >= (c.edx as i32)) {
                c.eax = 0xFFFFFFFFu32;
                break;
            }
        }
    }
    c.edi = pop(m, c);
    c.esi = pop(m, c);
}

/// 00307c20 cdecl(0). mapedit_init_node_pools_and_clear_grid_cells.
pub fn mapedit_init_node_pools_and_clear_grid_cells<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.edx = 0u32;
    {
        let value = c.esi;
        push(m, c, value);
    }
    m.write32(0x78400Cu32, c.edx);
    m.write32(0x784000u32, c.edx);
    c.eax = 0u32;
    {
        let value = c.edi;
        push(m, c, value);
    }
    loop {
        c.ecx = m.read32(0x783FFCu32);
        c.esi = c.eax.wrapping_add(c.ecx).wrapping_add(0x48u32);
        m.write32(c.eax.wrapping_add(c.ecx).wrapping_add(0x40u32), c.esi);
        c.eax = c.eax.wrapping_add(0x48u32);
        if !((c.eax as i32) < 0x11FB8) {
            break;
        }
    }
    c.eax = m.read32(0x783FFCu32);
    m.write32(c.eax.wrapping_add(0x11FF8u32), c.edx);
    c.ecx = m.read32(0x783FFCu32);
    m.write32(0x762CC4u32, c.ecx);
    m.write32(0x783FF8u32, 0x400u32);
    c.eax = 0u32;
    loop {
        c.ecx = m.read32(0x784008u32);
        c.esi = c.eax.wrapping_add(c.ecx).wrapping_add(0x3Cu32);
        m.write32(c.eax.wrapping_add(c.ecx).wrapping_add(0x14u32), c.esi);
        c.eax = c.eax.wrapping_add(0x3Cu32);
        if !((c.eax as i32) < 0x7BFC) {
            break;
        }
    }
    c.eax = m.read32(0x784008u32);
    m.write32(c.eax.wrapping_add(0x7C10u32), c.edx);
    c.ecx = m.read32(0x784008u32);
    c.edi = m.read32(0x784004u32);
    m.write32(0x762CC8u32, c.ecx);
    c.ecx = 0x2580u32;
    c.eax = 0u32;
    rep_stosd(m, c);
    c.edi = pop(m, c);
    m.write32(0x762CC0u32, 0x212u32);
    c.esi = pop(m, c);
}

/// 0031a860 cdecl(0). mapedit_cache_five_colour_dwords_of_selected_object_when_flag_0x4000000_and_return_nonnull.
pub fn mapedit_cache_five_colour_dwords_of_selected_object_when_flag_0x4000000_and_retu<
    M: Guest,
>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.ecx = m.read32(0x7844A8u32);
    c.eax = m.read32(c.ecx.wrapping_add(0x2F18u32));
    if ((c.eax != 0u32) && ((m.read32(c.ecx.wrapping_add(0x25Cu32)) & 0x4000000u32) != 0u32)) {
        c.eax = c.eax.wrapping_add(0x18u32);
        c.edx = m.read32(c.eax);
        m.write32(0x783A60u32, c.edx);
        c.edx = m.read32(c.eax.wrapping_add(4u32));
        m.write32(0x783A64u32, c.edx);
        c.edx = m.read32(c.eax.wrapping_add(8u32));
        m.write32(0x783A68u32, c.edx);
        c.edx = m.read32(c.eax.wrapping_add(0xCu32));
        m.write32(0x783A6Cu32, c.edx);
        c.eax = m.read32(c.eax.wrapping_add(0x10u32));
        m.write32(0x783A70u32, c.eax);
    }
    c.edx = m.read32(c.ecx.wrapping_add(0x2F18u32));
    c.eax = u32::from((c.edx != 0u32));
}

/// 0031bd70 cdecl(0). mapedit_has_record_e4_with_field_0x34_equal_5_and_field_0x58_positive.
pub fn mapedit_has_record_e4_with_field_0x34_equal_5_and_field_0x58_positive<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(0x7844A8u32);
    c.edx = m.read32(c.eax.wrapping_add(0x280u32));
    c.ecx = 0u32;
    if ((c.edx as i32) > 0) {
        c.eax = c.eax.wrapping_add(0x2E0u32);
        loop {
            if ((m.read32(c.eax.wrapping_sub(0x24u32)) == 5u32) && ((m.read32(c.eax) as i32) > 0)) {
                c.eax = 1u32;
                return;
            }
            c.ecx = c.ecx.wrapping_add(1);
            c.eax = c.eax.wrapping_add(0xE4u32);
            if !((c.ecx as i32) < (c.edx as i32)) {
                break;
            }
        }
    }
    c.eax = 0u32;
}

/// 0031d310 cdecl(0). mapedit_find_first_record_e4_with_field_0x34_equal_5.
pub fn mapedit_find_first_record_e4_with_field_0x34_equal_5<M: Guest>(m: &mut M, c: &mut Cpu) {
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m.read32(0x7844A8u32);
    c.edx = m.read32(c.edi.wrapping_add(0x280u32));
    c.eax = 0u32;
    c.ecx = 0u32;
    if ((c.edx as i32) > 0) {
        {
            let value = c.esi;
            push(m, c, value);
        }
        c.esi = c.edi.wrapping_add(0x2BCu32);
        loop {
            if (m.read32(c.esi) == 5u32) {
                c.ecx = c.ecx.wrapping_mul(0xE4u32);
                c.eax = c.ecx.wrapping_add(c.edi).wrapping_add(0x288u32);
                break;
            }
            c.ecx = c.ecx.wrapping_add(1);
            c.esi = c.esi.wrapping_add(0xE4u32);
            if ((c.ecx as i32) >= (c.edx as i32)) {
                break;
            }
        }
        c.esi = pop(m, c);
    }
    c.edi = pop(m, c);
}

/// 0031dbc0 cdecl(1). mapedit_no_record_b4_with_word_equal_arg_and_dword_a8_equal_1.
pub fn mapedit_no_record_b4_with_word_equal_arg_and_dword_a8_equal_1<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(0x7844A8u32);
    c.edx = m.read32(c.eax.wrapping_add(0x81A4u32));
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.ecx = 0u32;
    {
        let value = c.edi;
        push(m, c, value);
    }
    if ((c.edx as i32) > 0) {
        c.esi = m.read32(c.esp.wrapping_add(0xCu32));
        c.eax = c.eax.wrapping_add(0x8250u32);
        loop {
            c.edi = ((((m.read16(c.eax.wrapping_sub(0x9Cu32)) as u32) as i16) as i32) as u32);
            if ((c.edi == c.esi) && (m.read32(c.eax) != 1u32)) {
                c.edi = pop(m, c);
                c.eax = 0u32;
                c.esi = pop(m, c);
                return;
            }
            c.ecx = c.ecx.wrapping_add(1);
            c.eax = c.eax.wrapping_add(0xB4u32);
            if !((c.ecx as i32) < (c.edx as i32)) {
                break;
            }
        }
    }
    c.edi = pop(m, c);
    c.eax = 1u32;
    c.esi = pop(m, c);
}

/// 0031e450 cdecl(0). ui_is_record_rows_panel_pointer_not_a_record_b4_of_editor_object.
pub fn ui_is_record_rows_panel_pointer_not_a_record_b4_of_editor_object<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.ecx = m.read32(0x7844A8u32);
    c.edx = m.read32(c.ecx.wrapping_add(0x81A4u32));
    c.eax = 0u32;
    {
        let value = c.esi;
        push(m, c, value);
    }
    if ((c.edx as i32) > 0) {
        c.esi = m.read32(0x7630ACu32);
        c.ecx = c.ecx.wrapping_add(0x81A8u32);
        loop {
            if (c.ecx == c.esi) {
                c.eax = 0u32;
                c.esi = pop(m, c);
                return;
            }
            c.eax = c.eax.wrapping_add(1);
            c.ecx = c.ecx.wrapping_add(0xB4u32);
            if !((c.eax as i32) < (c.edx as i32)) {
                break;
            }
        }
    }
    c.eax = 1u32;
    c.esi = pop(m, c);
}

/// 0031e8c0 cdecl(2). mapedit_record_220_of_kind_1_contains_member_b4_index.
pub fn mapedit_record_220_of_kind_1_contains_member_b4_index<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(8u32));
    c.ecx = m.read32(0x7844A8u32);
    c.eax = c.eax.wrapping_mul(0x220u32);
    c.ecx = c.eax.wrapping_add(c.ecx).wrapping_add(0x41E4u32);
    if ((m.read8(c.ecx.wrapping_add(4u32)) as u32) != 1u32) {
        c.eax = 0u32;
        return;
    }
    c.edx = m.read32(c.ecx.wrapping_add(0x50u32));
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.eax = 0u32;
    if ((c.edx as i32) > 0) {
        c.esi = m.read32(c.esp.wrapping_add(8u32));
        c.ecx = c.ecx.wrapping_add(0x54u32);
        loop {
            if (m.read32(c.ecx) == c.esi) {
                c.eax = 1u32;
                c.esi = pop(m, c);
                return;
            }
            c.eax = c.eax.wrapping_add(1);
            c.ecx = c.ecx.wrapping_add(4u32);
            if !((c.eax as i32) < (c.edx as i32)) {
                break;
            }
        }
    }
    c.eax = 0u32;
    c.esi = pop(m, c);
}

/// 0031e910 cdecl(1). mapedit_index_of_record_b4_in_editor_object_defaulting_to_active_record.
pub fn mapedit_index_of_record_b4_in_editor_object_defaulting_to_active_record<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.ecx = m.read32(0x7844A8u32);
    c.edx = m.read32(c.ecx.wrapping_add(0x81A4u32));
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(c.esp.wrapping_add(8u32));
    let mut fallback: u32 = m.read32(0x762F98u32);
    if (c.esi == 0u32) {
        c.esi = fallback;
    }
    c.eax = 0u32;
    if ((c.edx as i32) > 0) {
        c.ecx = c.ecx.wrapping_add(0x81A8u32);
        loop {
            if (c.ecx == c.esi) {
                c.esi = pop(m, c);
                return;
            }
            c.eax = c.eax.wrapping_add(1);
            c.ecx = c.ecx.wrapping_add(0xB4u32);
            if !((c.eax as i32) < (c.edx as i32)) {
                break;
            }
        }
    }
    c.eax = 0xFFFFFFFFu32;
    c.esi = pop(m, c);
}
