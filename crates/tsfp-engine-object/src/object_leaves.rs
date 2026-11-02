// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 7 object leaves. Each body ports a call-free C replacement of `src/game`, see
//! docs/t1836-rust-wave7.md. Every function takes the entry registers `e` and returns the exact
//! final EAX/ECX/EDX. Stack arguments are read through `esp` (the guest stack pointer at entry,
//! return address at `esp`), in C order, so a store aliasing an argument slot behaves as in C.
use super::{ObjectMemory, gprs::Gprs};

const MODE_BYTE: u32 = 0x007d_e455;

fn arg<M: ObjectMemory>(m: &M, esp: u32, index: u32) -> u32 {
    m.read32(esp.wrapping_add(4 + 4 * index))
}

/// 000174E0 game_object_mode_selected_flag_pair_intersects. cdecl(3): (object, mask_lo, mask_hi).
/// The flag pair sits at object+0x10 in mode 10, else at object+0x18. EDX is untouched.
// portreg: va=0x000174e0 tier=1
pub fn mode_selected_flag_pair_intersects<M: ObjectMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let mode = m.read8(MODE_BYTE);
    let object = arg(m, esp, 0);
    let offset = if mode == 10 { 0x10 } else { 0x18 };
    let mut eax = m.read32(object.wrapping_add(offset));
    let mut ecx = m.read32(object.wrapping_add(offset).wrapping_add(4));
    eax &= arg(m, esp, 1);
    ecx &= arg(m, esp, 2);
    eax |= ecx;
    Gprs {
        eax: u32::from(eax != 0),
        ecx,
        edx: e.edx,
    }
}

/// 00071740 game_object_set_field_0x24_and_flag_0x2c_bit0. cdecl(2): (object, value).
pub fn set_field_0x24_and_flag_0x2c_bit0<M: ObjectMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let eax = arg(m, esp, 0);
    let flags = m.read32(eax.wrapping_add(0x2c));
    m.write32(eax.wrapping_add(0x2c), flags | 1);
    let ecx = arg(m, esp, 1);
    m.write32(eax.wrapping_add(0x24), ecx);
    Gprs {
        eax,
        ecx,
        edx: e.edx,
    }
}

/// 00264720 game_object_ext_set_9d0_if_state_3. cdecl(1). child = [object+0x7C]; stores 1 at
/// child+0x9D0 when child+0x20 == 3. EAX = child.
pub fn ext_set_9d0_if_state_3<M: ObjectMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let object = arg(m, esp, 0);
    let eax = m.read32(object.wrapping_add(0x7c));
    if m.read32(eax.wrapping_add(0x20)) == 3 {
        m.write32(eax.wrapping_add(0x9d0), 1);
    }
    Gprs {
        eax,
        ecx: e.ecx,
        edx: e.edx,
    }
}

/// 0010EBD0 game_object_is_field_0x50_equal_2. cdecl(1). A null object gives 0 with ECX/EDX kept.
pub fn is_field_0x50_equal_2<M: ObjectMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let object = arg(m, esp, 0);
    if object == 0 {
        return Gprs {
            eax: 0,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    let edx = m.read32(object.wrapping_add(0x50));
    let ecx = u32::from(edx == 2);
    Gprs { eax: ecx, ecx, edx }
}

/// 00234270 game_is_object_field_34_nonzero_null_safe. cdecl(1). A null object gives 0 with ECX/EDX kept.
// portreg: va=0x00234270 tier=1
pub fn is_field_0x34_nonzero_null_safe<M: ObjectMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let object = arg(m, esp, 0);
    if object == 0 {
        return Gprs {
            eax: 0,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    let edx = m.read32(object.wrapping_add(0x34));
    let ecx = u32::from(edx != 0);
    Gprs { eax: ecx, ecx, edx }
}

/// 0036CC10 game_object_store_three_args_into_fields_0x24_0x28_0x2c. cdecl(4). The fourth
/// argument is read after the first store, so a store over its slot changes the value stored last.
pub fn store_three_args_into_fields_0x24_0x28_0x2c<M: ObjectMemory>(
    m: &mut M,
    esp: u32,
    _e: Gprs,
) -> Gprs {
    let eax = arg(m, esp, 0);
    let mut ecx = arg(m, esp, 1);
    let edx = arg(m, esp, 2);
    m.write32(eax.wrapping_add(0x24), ecx);
    ecx = arg(m, esp, 3);
    m.write32(eax.wrapping_add(0x28), edx);
    m.write32(eax.wrapping_add(0x2c), ecx);
    Gprs { eax, ecx, edx }
}

/// 000D7550 game_object_set_field_0x84_and_0xa58_from_table. cdecl(2): (object, index). The table
/// word at 4D1E28 + 20*index is read after the store to object+0x84.
// portreg: va=0x000d7550 tier=1
pub fn set_field_0x84_and_0xa58_from_table<M: ObjectMemory>(m: &mut M, esp: u32, _e: Gprs) -> Gprs {
    let mut eax = arg(m, esp, 1);
    let ecx = arg(m, esp, 0);
    m.write32(ecx.wrapping_add(0x84), eax);
    eax = eax.wrapping_mul(5);
    let edx = m.read32(eax.wrapping_mul(4).wrapping_add(0x004d_1e28));
    m.write32(ecx.wrapping_add(0xa58), edx);
    Gprs { eax, ecx, edx }
}

/// 0015BCA0 game_array_7a3580_append_obj_field_0x7c. cdecl(1). Appends [object+0x7C] at
/// 7A3580 + 4*count and bumps the count word at 74C37C. EDX is untouched.
pub fn array_7a3580_append_field_0x7c<M: ObjectMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let object = arg(m, esp, 0);
    let ecx = m.read32(object.wrapping_add(0x7c));
    let mut eax = m.read32(0x0074_c37c);
    m.write32(eax.wrapping_mul(4).wrapping_add(0x007a_3580), ecx);
    eax = eax.wrapping_add(1);
    m.write32(0x0074_c37c, eax);
    Gprs {
        eax,
        ecx,
        edx: e.edx,
    }
}

/// 00177E90 game_object_child_0x7c_fields_0x56c_0x578_0x57c_all_nonnegative. cdecl(1). Signed
/// compares, ECX holds the last field read.
pub fn child_fields_all_nonnegative<M: ObjectMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let object = arg(m, esp, 0);
    let child = m.read32(object.wrapping_add(0x7c));
    let mut ecx = m.read32(child.wrapping_add(0x56c));
    if (ecx as i32) < 0 {
        return Gprs {
            eax: 0,
            ecx,
            edx: e.edx,
        };
    }
    ecx = m.read32(child.wrapping_add(0x578));
    if (ecx as i32) < 0 {
        return Gprs {
            eax: 0,
            ecx,
            edx: e.edx,
        };
    }
    ecx = m.read32(child.wrapping_add(0x57c));
    Gprs {
        eax: u32::from((ecx as i32) >= 0),
        ecx,
        edx: e.edx,
    }
}

/// 001B5840 game_object_flag_0xc_0x40_or_kind_1_record_29c_flag_20000_clear. cdecl(2):
/// (object, record). 1 when byte object+0xC has bit 0x40, else 0 unless record+0x38 == 1, then
/// 1 when the 0x29C-byte entry (index record+0x30) of the table at [4F9BAC] has no flag 0x20000.
// portreg: va=0x001b5840 tier=1
pub fn flag_0x40_or_kind_1_record_flag_20000_clear<M: ObjectMemory>(
    m: &M,
    esp: u32,
    e: Gprs,
) -> Gprs {
    let object = arg(m, esp, 0);
    if m.read8(object.wrapping_add(12)) & 0x40 != 0 {
        return Gprs {
            eax: 1,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    let record = arg(m, esp, 1);
    if m.read32(record.wrapping_add(0x38)) != 1 {
        return Gprs {
            eax: 0,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    let mut ecx = m.read32(record.wrapping_add(0x30));
    let edx = m.read32(0x004f_9bac);
    ecx = ecx.wrapping_mul(0x29c);
    let flags = m.read32(ecx.wrapping_add(edx).wrapping_add(12));
    Gprs {
        eax: u32::from(flags & 0x2_0000 == 0),
        ecx,
        edx,
    }
}

/// 001B6540 game_object_stash_field_0x94_to_0x9c_if_no_player_or_flag_759ed4_clear. cdecl(1).
/// EDX = 0. Moves [obj+0x94] to obj+0x9C and clears +0x94 and +0x98 unless the object's +8 is
/// below [790950] while [759ED4] is nonzero. A null object still reads at address 0x94.
pub fn stash_field_0x94_to_0x9c<M: ObjectMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let eax = arg(m, esp, 0);
    let mut ecx = e.ecx;
    let mut do_move = true;
    if eax != 0 {
        ecx = m.read32(eax.wrapping_add(8));
        if (ecx as i32) < (m.read32(0x0079_0950) as i32) && m.read32(0x0075_9ed4) != 0 {
            do_move = false;
        }
    }
    if do_move {
        ecx = m.read32(eax.wrapping_add(0x94));
        if ecx != 0 {
            m.write32(eax.wrapping_add(0x9c), ecx);
            m.write32(eax.wrapping_add(0x94), 0);
            m.write32(eax.wrapping_add(0x98), 0);
        }
    }
    Gprs { eax, ecx, edx: 0 }
}

/// 001B6A10 game_object_field_0x80_set_and_not_masked_by_flag_7de458_bit12. cdecl(2):
/// (object, enable). 1 unless [obj+0x80] is set and (enable == 0 or flag 0x1000 of [7DE458]).
// portreg: va=0x001b6a10 tier=1
pub fn field_0x80_set_and_not_masked<M: ObjectMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let mut eax = arg(m, esp, 0);
    let ecx = m.read32(eax.wrapping_add(0x80));
    if ecx != 0 {
        eax = arg(m, esp, 1);
        if eax == 0 {
            return Gprs {
                eax: 0,
                ecx,
                edx: e.edx,
            };
        }
        eax = m.read32(0x007d_e458);
        if eax & 0x1000 != 0 {
            return Gprs {
                eax: 0,
                ecx,
                edx: e.edx,
            };
        }
    }
    Gprs {
        eax: 1,
        ecx,
        edx: e.edx,
    }
}

/// 00242EE0 game_object_type_entry_519758_flag_mask_hit_or_kind_9_or_0xc. cdecl(1). The type
/// entry (0xB4 bytes at 519758, index [[obj+7C]+C] in 0..30, else address 0) has flags at +0x90,
/// tested against [[obj+4]+0x30]. Without a hit, 1 when [[obj+7C]+10] is 9 or 12.
pub fn type_entry_flag_mask_hit_or_kind_9_or_12<M: ObjectMemory>(
    m: &M,
    esp: u32,
    _e: Gprs,
) -> Gprs {
    let mut edx = arg(m, esp, 0);
    let mut ecx = m.read32(edx.wrapping_add(0x7c));
    let mut eax = m.read32(ecx.wrapping_add(12));
    eax = if (eax as i32) >= 0 && eax < 30 {
        eax.wrapping_mul(0xb4).wrapping_add(0x0051_9758)
    } else {
        0
    };
    edx = m.read32(edx.wrapping_add(4));
    edx = m.read32(edx.wrapping_add(0x30));
    if m.read32(eax.wrapping_add(0x90)) & edx != 0 {
        return Gprs { eax: 1, ecx, edx };
    }
    ecx = m.read32(ecx.wrapping_add(0x10));
    Gprs {
        eax: u32::from(ecx == 9 || ecx == 12),
        ecx,
        edx,
    }
}

fn type_table_word<M: ObjectMemory>(m: &M, index: u32) -> u32 {
    m.read32(index.wrapping_mul(20).wrapping_add(0x0053_c508))
}

/// 00350D40 game_type_table_dword_by_object_word_0x48_when_mode_0xa_or_flag_bit19. cdecl(1).
/// index = sign-extended word [obj+0x48]. 0 unless the mode byte [7DE455] is 10 or [7DE458] has
/// bit 0x80000, else the dword at 53C508 + 20*index.
pub fn type_table_dword_by_word_0x48<M: ObjectMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let mode = m.read8(MODE_BYTE);
    let object = arg(m, esp, 0);
    let eax = m.read16(object.wrapping_add(0x48)) as i16 as i32 as u32;
    if mode != 10 && m.read32(0x007d_e458) & 0x8_0000 == 0 {
        return Gprs {
            eax: 0,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    Gprs {
        eax: type_table_word(m, eax),
        ecx: eax.wrapping_mul(5),
        edx: e.edx,
    }
}

/// 00350E70 game_type_table_flag_mask_test_by_object_word_0x48. cdecl(2): (object, mask).
/// With bit 1 of the mask set the table word is tested unconditionally (EDX = 5*index), else
/// as in 00350D40 the mode/flag gate applies. Result is the table word & mask (or 1 on early hit).
pub fn type_table_flag_mask_test<M: ObjectMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let ecx = arg(m, esp, 1);
    let object = arg(m, esp, 0);
    let mut eax = m.read16(object.wrapping_add(0x48)) as i16 as i32 as u32;
    let mut edx = e.edx;
    if ecx & 1 != 0 {
        edx = eax.wrapping_mul(5);
        if m.read32(edx.wrapping_mul(4).wrapping_add(0x0053_c508)) & ecx != 0 {
            return Gprs { eax: 1, ecx, edx };
        }
    }
    if m.read8(MODE_BYTE) != 10 && m.read32(0x007d_e458) & 0x8_0000 == 0 {
        return Gprs { eax: 0, ecx, edx };
    }
    eax = eax.wrapping_mul(5);
    eax = m.read32(eax.wrapping_mul(4).wrapping_add(0x0053_c508)) & ecx;
    Gprs { eax, ecx, edx }
}

/// 0037A6F0 game_object_type_8_copy_vec3_from_ext_child_0xaa0_fields_0x14_0x18_0x1c.
/// cdecl(2): (object, out). Only for object+0x20 == 8 with non-null [obj+0x7C] and
/// [child+0xAA0]: copies three words (+0x14, +0x18, +0x1C) to out, reading each after the
/// previous store. Early returns leave EAX holding the last pointer read.
pub fn type_8_copy_vec3_from_ext_child<M: ObjectMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let mut eax = arg(m, esp, 0);
    if m.read32(eax.wrapping_add(32)) != 8 {
        return Gprs {
            eax,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    eax = m.read32(eax.wrapping_add(0x7c));
    if eax == 0 {
        return Gprs {
            eax,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    eax = m.read32(eax.wrapping_add(0xaa0));
    if eax == 0 {
        return Gprs {
            eax,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    let mut edx = m.read32(eax.wrapping_add(20));
    let ecx = arg(m, esp, 1);
    m.write32(ecx, edx);
    edx = m.read32(eax.wrapping_add(24));
    m.write32(ecx.wrapping_add(4), edx);
    eax = m.read32(eax.wrapping_add(28));
    m.write32(ecx.wrapping_add(8), eax);
    Gprs { eax, ecx, edx }
}

/// 00087840 game_object_field_0x1f0_or_room_entry_field_0x7c_by_flag. cdecl(2): (index, flag).
/// flag != 0: EAX = [[[7B9784] + 4*index] + 0x1F0]. Else the room table at [708788] (0xB4-byte
/// entries): entry = [index*0xB4 + table], EAX = [entry - 0xC8 * [entry] + 0x7C].
// portreg: va=0x00087840 tier=1
pub fn field_0x1f0_or_room_entry_field_0x7c<M: ObjectMemory>(m: &M, esp: u32, _e: Gprs) -> Gprs {
    let flag = arg(m, esp, 1) != 0;
    let mut eax = arg(m, esp, 0);
    if flag {
        let ecx = m.read32(0x007b_9784);
        let edx = m.read32(ecx.wrapping_add(eax.wrapping_mul(4)));
        eax = m.read32(edx.wrapping_add(0x1f0));
        return Gprs { eax, ecx, edx };
    }
    let ecx = m.read32(0x0070_8788);
    eax = eax.wrapping_mul(0xb4);
    eax = m.read32(eax.wrapping_add(ecx));
    let edx = m.read32(eax).wrapping_mul(0xc8);
    eax = eax.wrapping_sub(edx);
    eax = m.read32(eax.wrapping_add(0x7c));
    Gprs { eax, ecx, edx }
}
