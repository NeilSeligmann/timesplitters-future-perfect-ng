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

/// 00017a80 cdecl(3). selected_buffer_pair_initialize_current_records.
pub fn selected_buffer_pair_initialize_current_records<M: Guest>(m: &mut M, c: &mut Cpu) {
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(0x54E564u32);
    c.eax = m.read32(4u32.wrapping_mul(c.esi).wrapping_add(0x54EEA4u32));
    c.edx = 0u32;
    m.write32(0x54EF0Cu32, c.edx);
    if ((c.eax as i32) >= 0x44) {
        c.eax = 0u32;
        c.esi = pop(m, c);
        return;
    }
    c.ecx = c.eax;
    c.eax = (c.ecx.wrapping_add(8u32.wrapping_mul(c.ecx))).wrapping_shl(4);
    c.ecx = c.ecx.wrapping_mul(0x1Cu32);
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m.read32(4u32.wrapping_mul(c.esi).wrapping_add(0x54EEC8u32));
    c.eax = c.eax.wrapping_add(c.edi);
    c.edi = m.read32(4u32.wrapping_mul(c.esi).wrapping_add(0x54EEE8u32));
    c.esi = m.read32(c.esp.wrapping_add(12u32));
    m.write32(c.ecx.wrapping_add(c.edi), c.esi);
    c.esi = m.read32(c.esp.wrapping_add(16u32));
    c.ecx = c.ecx.wrapping_add(c.edi);
    m.write32(c.ecx.wrapping_add(4u32), c.esi);
    c.esi = m.read32(c.esp.wrapping_add(20u32));
    m.write32(c.ecx.wrapping_add(8u32), c.esi);
    m.write32(c.ecx.wrapping_add(12u32), 0xffff_ffffu32);
    m.write32(c.ecx.wrapping_add(16u32), c.edx);
    m.write32(c.ecx.wrapping_add(20u32), c.edx);
    m.write32(c.ecx.wrapping_add(24u32), c.edx);
    c.edi = pop(m, c);
    let mut offset: u32 = 12u32;
    while (offset <= 0x8Cu32) {
        m.write32(c.eax.wrapping_add(offset), c.edx);
        offset = offset.wrapping_add(16u32);
    }
    c.eax = 1u32;
    c.esi = pop(m, c);
}

/// 000873f0 cdecl(3). room_708788_get_bounds_by_index.
pub fn room_708788_get_bounds_by_index<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(0x708788u32);
    c.eax = c.eax.wrapping_mul(0xB4u32);
    c.edx = m.read32(c.eax.wrapping_add(c.ecx).wrapping_add(0x10u32));
    c.ecx = m.read32(c.esp.wrapping_add(8u32));
    m.write32(c.ecx, c.edx);
    c.edx = m.read32(0x708788u32);
    c.edx = m.read32(c.eax.wrapping_add(c.edx).wrapping_add(0x14u32));
    m.write32(c.ecx.wrapping_add(4u32), c.edx);
    c.edx = m.read32(0x708788u32);
    c.edx = m.read32(c.eax.wrapping_add(c.edx).wrapping_add(0x18u32));
    m.write32(c.ecx.wrapping_add(8u32), c.edx);
    c.ecx = m.read32(0x708788u32);
    c.edx = m.read32(c.eax.wrapping_add(c.ecx).wrapping_add(0x1Cu32));
    c.ecx = m.read32(c.esp.wrapping_add(12u32));
    m.write32(c.ecx, c.edx);
    c.edx = m.read32(0x708788u32);
    c.edx = m.read32(c.eax.wrapping_add(c.edx).wrapping_add(0x20u32));
    m.write32(c.ecx.wrapping_add(4u32), c.edx);
    c.edx = m.read32(0x708788u32);
    c.eax = m.read32(c.eax.wrapping_add(c.edx).wrapping_add(0x24u32));
    m.write32(c.ecx.wrapping_add(8u32), c.eax);
}

/// 000877c0 cdecl(2). level_room_sub_entry_ptr_from_packed_handle_room_in_high_half_index_low_half.
pub fn level_room_sub_entry_ptr_from_packed_handle_room_in_high_half_index_low_half<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(0x7B9784u32);
    c.edx = m.read32(c.ecx.wrapping_add(4u32.wrapping_mul(c.eax)));
    c.eax = m.read32(c.edx.wrapping_add(4u32));
    c.ecx = m.read32(c.eax.wrapping_add(4u32));
    c.eax = m.read32(c.esp.wrapping_add(8u32));
    c.edx = c.eax;
    {
        let value = c.esi;
        push(m, c, value);
    }
    c.esi = m.read32(c.ecx);
    c.edx = ((c.edx as i32).wrapping_shr(16) as u32)
        .wrapping_sub(c.esi)
        .wrapping_mul(0xC8u32);
    c.ecx = m.read32(c.edx.wrapping_add(c.ecx).wrapping_add(0x78u32));
    c.eax = (c.eax & 0xFFFFu32);
    c.eax = c.ecx.wrapping_add(24u32.wrapping_mul(c.eax));
    c.esi = pop(m, c);
}

/// 000d6920 cdecl(0). player_records_clear_all_and_reset_count.
pub fn player_records_clear_all_and_reset_count<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(0x79094Cu32);
    c.ecx = u32::from((c.eax != 0x65u32));
    {
        let value = c.edi;
        push(m, c, value);
    }
    c.edi = m.read32(0x7356D8u32);
    c.eax = 0u32;
    c.ecx = (((c.ecx.wrapping_sub(1u32)) & 0xFFFFFFE4u32)
        .wrapping_add(0x20u32)
        .wrapping_mul(0xEA0u32)
        .wrapping_add(15u32)
        & 0xFFFFFFF0u32)
        .wrapping_shr(2);
    rep_stosd(m, c);
    m.write32(0x7B0C44u32, c.eax);
    c.edi = pop(m, c);
}

/// 001b66e0 cdecl(5). ammo_decrement_word_counter_clamp_zero_and_inc_shots_0x10c.
pub fn ammo_decrement_word_counter_clamp_zero_and_inc_shots_0x10c<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(1u32)));
    m.write32(
        c.eax.wrapping_add(0x10Cu32),
        m.read32(c.eax.wrapping_add(0x10Cu32)).wrapping_add(1u32),
    );
    c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(4u32)));
    c.edx = 0u32;
    m.write32(c.esp.wrapping_sub(4u32), c.esi);
    let mut displacement: u32 = 0x38u32;
    if ((c.ecx == 0u32) || (c.ecx == 1u32)) {
        c.eax = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
        displacement = 0x61Eu32;
    }
    c.ecx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(2u32)));
    c.esi = ((c.esi & 0xFFFF0000u32)
        | ((m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(3u32))) as u16) as u32));
    let mut field: u32 = c
        .eax
        .wrapping_add(c.ecx.wrapping_mul(2u32))
        .wrapping_add(displacement);
    write16(
        m,
        field,
        ((m.read16(field) as i32).wrapping_sub(((c.esi as u16) as i32)) as u16),
    );
    if ((((m.read16(field) as u32) as i16) as i32) < 0) {
        write16(m, field, (c.edx as u16));
    }
    c.esi = m.read32(c.esp.wrapping_sub(4u32));
}

// portreg: va=0x001c8c20
/// 001c8c20 cdecl(8). player_object_hit_reaction_allowed_by_anim_flag_bits_and_field_0x124.
pub fn player_object_hit_reaction_allowed_by_anim_flag_bits_and_field_0x124<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    let mut st: u32 = 0;
    'machine: loop {
        match st {
            0 => {
                c.edx = m.read32(c.esp.wrapping_add(4).wrapping_add(4u32.wrapping_mul(0u32)));
                c.eax = m.read32(c.edx.wrapping_add(0x20u32));
                c.esp = c.esp.wrapping_sub(8u32);
                m.write32(c.esp.wrapping_sub(4u32), c.esi);
                m.write32(c.esp.wrapping_sub(8u32), c.edi);
                c.edi = m.read32(c.esp.wrapping_add(0x10u32));
                if (c.eax == 8u32) {
                    c.esi = m.read32(c.edx.wrapping_add(0x7Cu32));
                    if ((m.read32(c.esi.wrapping_add(0x28u32)) & 0x800000u32) != 0u32) {
                        c.eax = ((((m.read16(c.edi.wrapping_mul(2u32).wrapping_add(0x7E03C0u32))
                            as u32) as i16) as i32) as u32);
                        c.eax = (m.read32(c.eax.wrapping_mul(8u32).wrapping_add(0x7DE660u32))
                            & 0x1000028u32);
                        c.ecx = 0u32;
                        if (c.eax == 0u32) {
                            {
                                st = 3;
                                continue 'machine;
                            }
                        }
                    }
                    c.ecx = m.read32(c.edx.wrapping_add(4u32));
                    c.eax = m.read32(c.ecx.wrapping_add(0xD4u32));
                    if (c.eax != 0u32) {
                        c.eax = ((((m.read16(c.eax.wrapping_mul(2u32).wrapping_add(0x7E03C0u32))
                            as u32) as i16) as i32) as u32);
                        c.eax = (m.read32(c.eax.wrapping_mul(8u32).wrapping_add(0x7DE660u32))
                            & 0x20u32);
                        c.ecx = 0u32;
                        if (c.eax != 0u32) {
                            c.ecx = (m.read32(c.esi.wrapping_add(0x14u32)) & 1u32);
                            c.eax = c.ecx;
                            if (c.eax == 0u32) {
                                {
                                    st = 3;
                                    continue 'machine;
                                }
                            }
                        }
                    }
                }
                m.write32(c.esp.wrapping_sub(12u32), c.ebx);
                c.ebx = m.read32(c.edx.wrapping_add(4u32));
                c.esi = m.read32(c.ebx.wrapping_add(0xD4u32));
                if (c.esi != 0u32) {
                    c.edx = ((((m.read16(c.edi.wrapping_mul(2u32).wrapping_add(0x7E03C0u32)) as u32)
                        as i16) as i32) as u32);
                    c.ecx = m.read32(c.edx.wrapping_mul(8u32).wrapping_add(0x7DE660u32));
                    c.eax = m.read32(c.edx.wrapping_mul(8u32).wrapping_add(0x7DE664u32));
                    m.write32(c.esp.wrapping_add(4u32), c.eax);
                    c.eax = (c.ecx & 0x20u32);
                    c.edx = 0u32;
                    if (c.eax == 0u32) {
                        c.edx = ((((m.read16(c.esi.wrapping_mul(2u32).wrapping_add(0x7E03C0u32))
                            as u32) as i16) as i32) as u32);
                        c.eax = (m.read32(c.edx.wrapping_mul(8u32).wrapping_add(0x7DE660u32))
                            & 0x1C00u32);
                        c.edx = 0u32;
                        if (c.eax != 0u32) {
                            c.ecx = (c.ecx & 0x1000000u32);
                            c.eax = 0u32;
                            if (c.ecx == 0u32) {
                                c.eax = m.read32(c.ebx.wrapping_add(0x124u32));
                                if ((c.eax == 0u32)
                                    && (((m.read8(c.ebx.wrapping_add(0x128u32)) as u32) & 4u32)
                                        == 0u32))
                                {
                                    {
                                        st = 1;
                                        continue 'machine;
                                    }
                                }
                            }
                        }
                    }
                }
                c.eax = ((((m.read16(c.edi.wrapping_mul(2u32).wrapping_add(0x7E03C0u32)) as u32)
                    as i16) as i32) as u32);
                c.ecx = m.read32(c.eax.wrapping_mul(8u32).wrapping_add(0x7DE660u32));
                c.edx = m.read32(c.eax.wrapping_mul(8u32).wrapping_add(0x7DE664u32));
                c.eax = (c.ecx & 0x20u32);
                m.write32(c.esp.wrapping_add(4u32), c.edx);
                c.edx = 0u32;
                if ((c.eax == 0u32) && (c.esi != 0u32)) {
                    c.ecx = (c.ecx & 8u32);
                    c.eax = 0u32;
                    if (c.ecx == 0u32) {
                        c.eax = ((((m.read16(c.esi.wrapping_mul(2u32).wrapping_add(0x7E03C0u32))
                            as u32) as i16) as i32) as u32);
                        c.eax =
                            (m.read32(c.eax.wrapping_mul(8u32).wrapping_add(0x7DE660u32)) & 8u32);
                        c.ecx = 0u32;
                        if (c.eax != 0u32) {
                            c.eax = m.read32(c.ebx.wrapping_add(0x124u32));
                            if (c.eax == 0u32) {
                                {
                                    st = 1;
                                    continue 'machine;
                                }
                            }
                        }
                    }
                }
                c.eax = 1u32;
                {
                    st = 2;
                    continue 'machine;
                }
                st = 1;
            }
            1 => {
                c.eax = 0u32;
                st = 2;
            }
            2 => {
                c.ebx = m.read32(c.esp.wrapping_sub(12u32));
                {
                    st = 4;
                    continue 'machine;
                }
                st = 3;
            }
            3 => {
                c.eax = 0u32;
                st = 4;
            }
            4 => {
                c.edi = m.read32(c.esp.wrapping_sub(8u32));
                c.esi = m.read32(c.esp.wrapping_sub(4u32));
                c.esp = c.esp.wrapping_add(8u32);
                break 'machine;
            }
            _ => break 'machine,
        }
    }
}

/// 0022e050 cdecl(1). state_descriptor_get_by_index.
// portreg: va=0x0022e050
pub fn state_descriptor_get_by_index<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    if ((c.eax as i32) < 0x15) {
        c.eax = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x510788u32));
    } else {
        c.eax = m.read32(c.eax.wrapping_mul(4u32).wrapping_add(0x513D04u32));
    }
}

/// 0025a080 cdecl(0). player_7b0c7c_set_flag_f5c_when_flag_0x20_and_state_not_0x650.
pub fn player_7b0c7c_set_flag_f5c_when_flag_0x20_and_state_not_0x650<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(0x7B0C7Cu32);
    m.write32(c.eax.wrapping_add(0xF5Cu32), 0u32);
    c.eax = m.read32(0x7B0C7Cu32);
    if ((((m.read8(c.eax.wrapping_add(0xCu32)) as u32) & 0x20u32) != 0u32)
        && ((m.read32(c.eax.wrapping_add(0x114u32)) & 0x650u32) == 0u32))
    {
        m.write32(c.eax.wrapping_add(0xF5Cu32), 1u32);
    }
}

/// 0025b720 cdecl(0). player_7b0c7c_shift_history_1564_1560_155c_when_ext_field_678_changes.
pub fn player_7b0c7c_shift_history_1564_1560_155c_when_ext_field_678_changes<M: Guest>(
    m: &mut M,
    c: &mut Cpu,
) {
    c.eax = m.read32(0x7B0C7Cu32);
    c.ecx = m.read32(c.eax.wrapping_add(0x14u32));
    c.ecx = m.read32(c.ecx.wrapping_add(0x7Cu32));
    c.edx = m.read32(c.ecx.wrapping_add(0x678u32));
    if (c.edx == m.read32(c.eax.wrapping_add(0x155Cu32))) {
        return;
    }
    c.edx = m.read32(c.eax.wrapping_add(0x1560u32));
    m.write32(c.eax.wrapping_add(0x1564u32), c.edx);
    c.eax = m.read32(0x7B0C7Cu32);
    c.edx = m.read32(c.eax.wrapping_add(0x155Cu32));
    m.write32(c.eax.wrapping_add(0x1560u32), c.edx);
    c.eax = m.read32(c.ecx.wrapping_add(0x678u32));
    c.ecx = m.read32(0x7B0C7Cu32);
    m.write32(c.ecx.wrapping_add(0x155Cu32), c.eax);
}

/// 002743a0 cdecl(1). actor_side_record_7a2be4_flag_0x80_is_clear.
pub fn actor_side_record_7a2be4_flag_0x80_is_clear<M: Guest>(m: &mut M, c: &mut Cpu) {
    c.eax = m.read32(c.esp.wrapping_add(4u32));
    c.ecx = m.read32(c.eax.wrapping_add(0x7Cu32));
    c.edx = m.read32(c.ecx.wrapping_add(0x84u32));
    c.eax = m.read32(c.edx.wrapping_mul(8u32).wrapping_add(0x7A2BE4u32));
    c.eax = (m.read8(c.eax.wrapping_add(0x28u32)) as u32);
    c.eax = (c.eax ^ 0x80u32);
    c.eax = c.eax.wrapping_shr(7);
}
