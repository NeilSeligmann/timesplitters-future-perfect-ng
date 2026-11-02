// SPDX-License-Identifier: GPL-3.0-or-later
//! T1815 w8 closure-model ports (12 roots). Each body mirrors its unmodified C replacement in
//! `src/game/game_calls_continuation_batch2.c` or `game_band_180000_280000_calls.c`, including
//! saved-register slots below ESP and the exact call frames. See docs/t1815-closure-model.md.
use super::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

/// 000606E0 cdecl(index): clamps the index into [0, [7BB260] - 1]. With [7BB288] set the answer is
/// 00060410(index, 0), otherwise the record pointer inside the table at [6D780C].
pub fn clamped_index_to_record_pointer<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.esi);
    let saved_esi = cpu.esi;
    let mut index = stack_arg(m, cpu, 0);
    if (index as i32) < 0 {
        index = 0;
    }
    cpu.eax = m.read32(0x007B_B260);
    if (index as i32) >= (cpu.eax as i32) {
        index = cpu.eax.wrapping_sub(1);
    }
    cpu.eax = m.read32(0x007B_B288);
    if cpu.eax != 0 {
        cpu.esi = index;
        guest_call(c, m, cpu, 0x0006_0410, 0x0006_0708, 4, &[index, 0]);
        cpu.esi = saved_esi;
        return;
    }
    cpu.ecx = m.read32(0x006D_780C);
    cpu.eax = m.read32(cpu.ecx.wrapping_add(8));
    cpu.edx = if (cpu.eax as i32) < 0 { 0xFFFF_FFFF } else { 0 } & 3;
    cpu.eax = cpu.eax.wrapping_add(cpu.edx);
    cpu.eax = ((cpu.eax as i32) >> 2) as u32;
    cpu.eax = cpu.ecx.wrapping_add(cpu.eax.wrapping_mul(4));
    cpu.ecx = m.read32(cpu.eax);
    cpu.ecx <<= 4;
    cpu.ecx = cpu.ecx.wrapping_add(index);
    cpu.ecx = m.read32(cpu.eax.wrapping_add(cpu.ecx.wrapping_mul(4)).wrapping_add(4));
    cpu.eax = cpu.eax.wrapping_add(4);
    cpu.ecx <<= 6;
    cpu.eax = cpu.eax.wrapping_add(cpu.ecx);
}

/// 00060740 cdecl(index, dst16, dst12, dst4): copies the record found by 000606E0 into up to
/// three optional destinations (4 dwords, 3 dwords from +0x10, 1 dword from +0x1C).
pub fn record_by_index_copy_to_optional_outputs<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let index = stack_arg(m, cpu, 0);
    guest_call(c, m, cpu, 0x0006_06E0, 0x0006_074A, 0, &[index]);
    if cpu.eax == 0 {
        return;
    }
    cpu.ecx = stack_arg(m, cpu, 1);
    if cpu.ecx != 0 {
        for word in 0..4u32 {
            cpu.edx = m.read32(cpu.eax.wrapping_add(4 * word));
            m.write32(cpu.ecx.wrapping_add(4 * word), cpu.edx);
        }
    }
    cpu.ecx = stack_arg(m, cpu, 2);
    if cpu.ecx != 0 {
        for word in 0..3u32 {
            cpu.edx = m.read32(cpu.eax.wrapping_add(0x10 + 4 * word));
            m.write32(cpu.ecx.wrapping_add(4 * word), cpu.edx);
        }
    }
    cpu.ecx = stack_arg(m, cpu, 3);
    if cpu.ecx != 0 {
        cpu.eax = m.read32(cpu.eax.wrapping_add(0x1C));
        m.write32(cpu.ecx, cpu.eax);
    }
}

/// 0015C390 cdecl(object): marks the owner at [object+0x7C] pending (bit 4 of +0x2C), clears
/// mask 0xFFFFFF9E in [[owner]+0x28] and, when signed [owner+8] >= [790950], calls 0015BCC0.
pub fn owner_mark_pending_and_remove_from_active_list<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let object = stack_arg(m, cpu, 0);
    cpu.eax = m.read32(object.wrapping_add(0x7C));
    cpu.ecx = m.read32(cpu.eax.wrapping_add(0x2C));
    if cpu.ecx & 4 != 0 {
        cpu.eax = 0;
        return;
    }
    cpu.ecx |= 4;
    m.write32(cpu.eax.wrapping_add(0x2C), cpu.ecx);
    cpu.ecx = m.read32(cpu.eax);
    let word = m.read32(cpu.ecx.wrapping_add(0x28)) & 0xFFFF_FF9E;
    m.write32(cpu.ecx.wrapping_add(0x28), word);
    cpu.ecx = m.read32(cpu.eax.wrapping_add(8));
    if (cpu.ecx as i32) < (m.read32(0x0079_0950) as i32) {
        cpu.eax = 0;
        return;
    }
    cpu.edx = m.read32(cpu.eax);
    let argument = cpu.edx;
    guest_call(c, m, cpu, 0x0015_BCC0, 0x0015_C3BE, 0, &[argument]);
    cpu.eax = 1;
}

/// 0017A190 cdecl(object): validates the handle at [object+0xC08] against the 0x2E8-byte owner
/// table at [72F5E8] (stale handle cleared), resolves the record through 0017A120 and returns the
/// 16-byte row at record+0x144 for its signed index, 0 when absent.
pub fn object_handle_to_owner_row_pointer<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let entry = cpu.esp;
    cpu.edx = stack_arg(m, cpu, 0);
    cpu.ecx = m.read32(cpu.edx.wrapping_add(0xC08));
    m.write32(entry.wrapping_sub(4), cpu.esi);
    cpu.eax = 0;
    if cpu.ecx != 0 {
        cpu.esi = m.read32(0x0072_F5E8);
        cpu.eax = (cpu.ecx >> 22).wrapping_mul(0x2E8).wrapping_add(cpu.esi);
        if m.read32(cpu.eax.wrapping_add(0x2C)) != cpu.ecx {
            m.write32(cpu.edx.wrapping_add(0xC08), 0);
            cpu.eax = 0;
        } else {
            cpu.esi = m.read32(cpu.eax.wrapping_add(0x7C));
            let argument = cpu.edx;
            guest_call(c, m, cpu, 0x0017_A120, 0x0017_A1CE, 4, &[argument]);
            if cpu.eax != 0 && cpu.esi != 0 {
                cpu.eax = m.read32(cpu.eax.wrapping_add(0x2C));
                if (cpu.eax as i32) >= 0
                    && (cpu.eax as i32) < (m.read32(cpu.esi.wrapping_add(0x164)) as i32)
                {
                    cpu.eax = (cpu.eax << 4).wrapping_add(cpu.esi).wrapping_add(0x144);
                } else {
                    cpu.eax = 0;
                }
            } else {
                cpu.eax = 0;
            }
        }
    }
    cpu.esi = m.read32(entry.wrapping_sub(4));
}

/// 0018AC60 cdecl(key): object = 0023FF20(key), then [[object+0x7C]+0x114] = 1 when found.
pub fn object_by_key_set_child_field_0x114<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = stack_arg(m, cpu, 0);
    let key = cpu.eax;
    guest_call(c, m, cpu, 0x0023_FF20, 0x0018_AC6A, 0, &[key]);
    if cpu.eax != 0 {
        cpu.ecx = m.read32(cpu.eax.wrapping_add(0x7C));
        m.write32(cpu.ecx.wrapping_add(0x114), 1);
    }
}

/// 001EC8E0 cdecl(): 1 when object key 0x47415330 is absent and [75BF0C] == 1.
pub fn object_gas0_absent_and_global_is_one<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, 0x0023_FF20, 0x001E_C8EA, 0, &[0x4741_5330]);
    cpu.eax = if cpu.eax == 0 {
        u32::from(m.read32(0x0075_BF0C) == 1)
    } else {
        0
    };
}

fn key_type_0x4c_global_zero<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
    key: u32,
    return_pc: u32,
) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.esi);
    cpu.esi = 1;
    guest_call(c, m, cpu, 0x0015_BA10, return_pc, 4, &[key]);
    if cpu.eax == 0 {
        cpu.eax = cpu.esi;
    } else {
        cpu.ecx = m.read32(cpu.eax.wrapping_add(0x34));
        cpu.eax = 0;
        if cpu.ecx == 0x4C {
            cpu.ecx = m.read32(0x0075_BF0C);
            cpu.eax = u32::from(cpu.ecx == 0);
        }
    }
    cpu.esi = m.read32(entry.wrapping_sub(4));
}

/// 001EC980 cdecl(): record of key 0x47415330 has type 0x4C and [75BF0C] == 0 (missing: true).
pub fn object_key_gas0_type_0x4c<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    key_type_0x4c_global_zero(c, m, cpu, 0x4741_5330, 0x001E_C990)
}

/// 001EC9C0 cdecl(): as 001EC980 for key 0x47415331.
pub fn object_key_gas1_type_0x4c<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    key_type_0x4c_global_zero(c, m, cpu, 0x4741_5331, 0x001E_C9D0)
}

/// 001ECA00 cdecl(): either of the records for keys 0x47415330 / 0x47415331 has type 0x4C.
pub fn object_keys_gas0_or_gas1_type_0x4c<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.esi);
    m.write32(entry.wrapping_sub(8), cpu.edi);
    cpu.edi = 0;
    guest_call(c, m, cpu, 0x0015_BA10, 0x001E_CA0E, 8, &[0x4741_5330]);
    cpu.esi = cpu.eax;
    guest_call(c, m, cpu, 0x0015_BA10, 0x001E_CA1A, 12, &[0x4741_5331]);
    let second = cpu.eax;
    cpu.ecx = 0x4C;
    if cpu.esi != 0 && m.read32(cpu.esi.wrapping_add(0x34)) == cpu.ecx {
        cpu.edi = 1;
    }
    if second != 0 && m.read32(second.wrapping_add(0x34)) == cpu.ecx {
        cpu.eax = 1;
    } else {
        cpu.eax = cpu.edi;
    }
    cpu.edi = m.read32(entry.wrapping_sub(8));
    cpu.esi = m.read32(entry.wrapping_sub(4));
}

/// 002346A0 cdecl(keys, object): 1 when the 0009A4C0 key of [object+0x678] equals one of the
/// non-negative key words at keys+4, +8, +0xC, +0x10.
pub fn object_key_matches_any_of_four<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = stack_arg(m, cpu, 1);
    if cpu.eax == 0 {
        cpu.eax = 0;
        return;
    }
    cpu.eax = m.read32(cpu.eax.wrapping_add(0x678));
    let argument = cpu.eax;
    guest_call(c, m, cpu, 0x0009_A4C0, 0x0023_46B4, 0, &[argument]);
    cpu.edx = stack_arg(m, cpu, 0);
    let mut offset = 4u32;
    while offset <= 0xC {
        cpu.ecx = m.read32(cpu.edx.wrapping_add(offset));
        if (cpu.ecx as i32) > -1 && cpu.eax == cpu.ecx {
            cpu.eax = 1;
            return;
        }
        offset += 4;
    }
    cpu.edx = m.read32(cpu.edx.wrapping_add(0x10));
    cpu.eax = u32::from((cpu.edx as i32) > -1 && cpu.eax == cpu.edx);
}

/// 00247A10 cdecl(id): entry object of pool 78B720 (0x5C-byte entries, scanned from the top)
/// whose object's +0x230 equals `id`.
pub fn pool_find_entry_object_by_sequence_id<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let pool = 0x0078_B720;
    let mut found = false;
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.esi);
    m.write32(entry.wrapping_sub(8), cpu.edi);
    guest_call(c, m, cpu, 0x0015_6E50, 0x0024_7A1C, 8, &[pool]);
    cpu.esi = cpu.eax;
    guest_call(c, m, cpu, 0x0015_6E80, 0x0024_7A28, 12, &[pool]);
    if cpu.eax != 0 {
        cpu.edi = stack_arg(m, cpu, 0);
        cpu.ecx = cpu.eax.wrapping_mul(0x5C).wrapping_add(cpu.esi).wrapping_add(0xC);
        loop {
            cpu.edx = m.read32(cpu.ecx.wrapping_sub(0x64));
            cpu.ecx = cpu.ecx.wrapping_sub(0x5C);
            cpu.eax = cpu.eax.wrapping_sub(1);
            if cpu.edx != 0 {
                cpu.edx = m.read32(cpu.ecx);
                if cpu.edx != 0 && m.read32(cpu.edx.wrapping_add(0x230)) == cpu.edi {
                    cpu.eax = m.read32(
                        cpu.eax.wrapping_mul(0x5C).wrapping_add(cpu.esi).wrapping_add(0xC),
                    );
                    found = true;
                    break;
                }
            }
            if cpu.eax == 0 {
                break;
            }
        }
    }
    if !found {
        cpu.eax = 0;
    }
    cpu.edi = m.read32(entry.wrapping_sub(8));
    cpu.esi = m.read32(entry.wrapping_sub(4));
}

/// 00248910 cdecl(object): pool 78B720 entry object whose owner (+0x10) is `object`, only when
/// the state word [[object+0x150]+0x114] is 0x1000.
pub fn pool_find_entry_object_by_owner_when_state_0x1000<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.ebx);
    m.write32(entry.wrapping_sub(8), cpu.edi);
    cpu.edi = stack_arg(m, cpu, 0);
    cpu.eax = m.read32(cpu.edi.wrapping_add(0x150));
    cpu.ecx = m.read32(cpu.eax.wrapping_add(0x114));
    cpu.ebx = 0;
    if cpu.ecx == 0x1000 {
        let pool = 0x0078_B720;
        m.write32(entry.wrapping_sub(12), cpu.esi);
        cpu.esi = 0;
        loop {
            guest_call(c, m, cpu, 0x0015_6E50, 0x0024_893E, 12, &[pool]);
            cpu.ecx = m.read32(cpu.eax.wrapping_add(cpu.esi).wrapping_add(4));
            cpu.eax = cpu.eax.wrapping_add(cpu.esi);
            if cpu.ecx != 0 && m.read32(cpu.eax.wrapping_add(0x10)) == cpu.edi {
                cpu.ebx = m.read32(cpu.eax.wrapping_add(0xC));
                break;
            }
            cpu.esi = cpu.esi.wrapping_add(0x5C);
            if (cpu.esi as i32) >= 0x170 {
                break;
            }
        }
        cpu.esi = m.read32(entry.wrapping_sub(12));
    }
    cpu.eax = cpu.ebx;
    cpu.edi = m.read32(entry.wrapping_sub(8));
    cpu.ebx = m.read32(entry.wrapping_sub(4));
}

/// 000DB480 cdecl(object, first_key, second_key): walks the list at [object+0x230] (next at +0x18)
/// for the node whose first two words equal the keys and moves it to the tail with 000DB3B0.
/// Found: EAX=object, ECX=[object+0x238], EDX=[object+0x234]. Not found: EAX=0, ECX=second, EDX=first.
pub fn object_list_find_node_by_key_pair_and_move_to_tail<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let object = stack_arg(m, cpu, 0);
    let mut node = m.read32(object.wrapping_add(0x230));
    cpu.eax = node;
    if node == 0 {
        return;
    }
    let first_key = stack_arg(m, cpu, 1);
    let second_key = stack_arg(m, cpu, 2);
    cpu.ecx = second_key;
    cpu.edx = first_key;
    while m.read32(node) != first_key || m.read32(node.wrapping_add(4)) != second_key {
        node = m.read32(node.wrapping_add(0x18));
        cpu.eax = node;
        if node == 0 {
            return;
        }
    }
    guest_call(c, m, cpu, 0x000D_B3B0, 0x000D_B4B0, 4, &[object, node]);
    cpu.eax = object;
    cpu.ecx = m.read32(object.wrapping_add(0x238));
    cpu.edx = m.read32(object.wrapping_add(0x234));
}

/// 00188840 cdecl(value, object): sets bit 0x10 of [object+0x28], calls 0009E720(object, 0x1887F0)
/// and stores the value at [object+0x1A0]. ESI is saved at entry-4 and restored.
pub fn object_set_flag_install_callback_store_value<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let entry = cpu.esp;
    m.write32(entry.wrapping_sub(4), cpu.esi);
    cpu.esi = stack_arg(m, cpu, 1);
    cpu.eax = m.read32(cpu.esi.wrapping_add(0x28));
    cpu.eax |= 0x10;
    m.write32(entry.wrapping_sub(8), 0x0018_87F0);
    m.write32(entry.wrapping_sub(12), cpu.esi);
    m.write32(cpu.esi.wrapping_add(0x28), cpu.eax);
    let arguments = [m.read32(entry.wrapping_sub(12)), m.read32(entry.wrapping_sub(8))];
    guest_call(c, m, cpu, 0x0009_E720, 0x0018_8859, 4, &arguments);
    cpu.eax = stack_arg(m, cpu, 0);
    m.write32(cpu.esi.wrapping_add(0x1A0), cpu.eax);
    cpu.esi = m.read32(entry.wrapping_sub(4));
}

/// 000AE910 cdecl(): initialises the free list pool 7329F0 through 00156DB0(pool, stride 0x24,
/// storage 7327B0, count 0x10).
pub fn pool_7329f0_init<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    guest_call(c, m, cpu, 0x0015_6DB0, 0x000A_E923, 0, &[0x0073_29F0, 0x24, 0x0073_27B0, 0x10]);
}

/// 00190680 cdecl(): 000827B0(4) then EAX = 3.
pub fn set_globals_to_4_return_3<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    guest_call(c, m, cpu, 0x0008_27B0, 0x0019_0687, 0, &[4]);
    cpu.eax = 3;
}

/// 00190A70 cdecl(): 000827F0() then 00024E40(its EAX); leaves the callee's registers.
pub fn memory_unit_current_slot_field_0x4<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, 0x0008_27F0, 0x0019_0A75, 0, &[]);
    let argument = cpu.eax;
    guest_call(c, m, cpu, 0x0002_4E40, 0x0019_0A7B, 0, &[argument]);
}

/// 00190690 cdecl(): 000827F0() then 00024E60(its EAX); EAX = 1 when its low byte is non-zero else 6.
pub fn memory_unit_current_slot_active_one_else_six<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, 0x0008_27F0, 0x0019_0695, 0, &[]);
    let argument = cpu.eax;
    guest_call(c, m, cpu, 0x0002_4E60, 0x0019_069B, 0, &[argument]);
    cpu.eax = if cpu.eax & 0xFF != 0 { 1 } else { 6 };
}

/// 00190AC0 cdecl(index): 00024E90(index) zero-extended to a byte.
// portreg: va=0x00190ac0 tier=0
pub fn scaled_table_byte_zero_extended<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = stack_arg(m, cpu, 0);
    let argument = cpu.eax;
    guest_call(c, m, cpu, 0x0002_4E90, 0x0019_0ACA, 0, &[argument]);
    cpu.eax &= 0xFF;
}

/// 00240560 cdecl(key): [0023FF20(key) + 0x7C], 0 when the object is absent.
pub fn field_7c_of_object_found_by_key<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = stack_arg(m, cpu, 0);
    let argument = cpu.eax;
    guest_call(c, m, cpu, 0x0023_FF20, 0x0024_056A, 0, &[argument]);
    cpu.eax = if cpu.eax != 0 { m.read32(cpu.eax.wrapping_add(0x7C)) } else { 0 };
}

/// 001DE560 cdecl(): 1 when [[0023FF20(0x44524157)+0x7C]+0xAC] is zero (reads without a null check).
pub fn object_key_draw_field7c_ac_zero<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, 0x0023_FF20, 0x001D_E56A, 0, &[0x4452_4157]);
    cpu.eax = m.read32(cpu.eax.wrapping_add(0x7C));
    cpu.edx = m.read32(cpu.eax.wrapping_add(0xAC));
    cpu.ecx = u32::from(cpu.edx == 0);
    cpu.eax = cpu.ecx;
}

/// 0024D260 cdecl(dst): copies the three words at +0x64 of the object with key 0x54524350 to dst.
pub fn position_of_tagged_object<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, 0x0023_FF20, 0x0024_D26A, 0, &[0x5452_4350]);
    if cpu.eax == 0 {
        return;
    }
    cpu.edx = m.read32(cpu.eax.wrapping_add(0x64));
    cpu.ecx = stack_arg(m, cpu, 0);
    m.write32(cpu.ecx, cpu.edx);
    cpu.edx = m.read32(cpu.eax.wrapping_add(0x68));
    m.write32(cpu.ecx.wrapping_add(4), cpu.edx);
    cpu.eax = m.read32(cpu.eax.wrapping_add(0x6C));
    m.write32(cpu.ecx.wrapping_add(8), cpu.eax);
    cpu.eax = 1;
}

/// 001CD560 cdecl(bit): 1 when [75AEF4] is set, else bit (bit & 31) of [00069630() + 0xBF0].
pub fn profile_bit_set_or_override<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = m.read32(0x0075_AEF4);
    if cpu.eax != 0 {
        cpu.eax = 1;
        return;
    }
    guest_call(c, m, cpu, 0x0006_9630, 0x001C_D574, 0, &[]);
    cpu.ecx = stack_arg(m, cpu, 0);
    cpu.eax = m.read32(cpu.eax.wrapping_add(0xBF0));
    cpu.edx = 1u32 << (cpu.ecx & 31);
    cpu.eax = u32::from(cpu.eax & cpu.edx != 0);
}

/// 0025EF30 cdecl(bit): 2 when the 0x238-byte record of [[7B0C7C]] has the bit at +4, else 1 when
/// [7DE45C] has 0x10000, else 1 when 00035DE60() is zero.
pub fn player_record_bit_test_returns_two_else_default<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = m.read32(0x007B_0C7C);
    cpu.edx = m.read32(cpu.eax);
    cpu.ecx = stack_arg(m, cpu, 0);
    cpu.edx = cpu.edx.wrapping_mul(0x238);
    cpu.eax = 1u32 << (cpu.ecx & 31);
    cpu.ecx = m.read32(0x0075_F5D0);
    if m.read32(cpu.edx.wrapping_add(cpu.ecx).wrapping_add(4)) & cpu.eax != 0 {
        cpu.eax = 2;
        return;
    }
    if m.read32(0x007D_E45C) & 0x10000 != 0 {
        cpu.eax = 1;
        return;
    }
    guest_call(c, m, cpu, 0x0003_5DE60, 0x0025_EF6B, 0, &[]);
    cpu.eax = u32::from(cpu.eax == 0);
}

/// 001B7E80 cdecl(index): id from the 0x29C-byte record at [4F9BAC] + index * 0x29C (negative index: 0).
pub fn record_29c_id_or_default<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    cpu.eax = stack_arg(m, cpu, 0);
    if (cpu.eax as i32) < 0 {
        cpu.eax = 0;
        return;
    }
    cpu.ecx = m.read32(0x004F_9BAC);
    cpu.eax = cpu.eax.wrapping_mul(0x29C);
    cpu.edx = (cpu.edx & 0xFFFF_FF00) | u32::from(m.read8(0x007D_E455));
    cpu.eax = cpu.eax.wrapping_add(cpu.ecx);
    cpu.ecx = m.read32(cpu.eax.wrapping_add(0x1F8));
    if cpu.edx & 0xFF != 0x0A {
        guest_call(c, m, cpu, 0x0015_3B40, 0x001B_7EC5, 0, &[]);
        cpu.eax = 0x1D2;
        return;
    }
    if (cpu.ecx as i32) > 0 {
        cpu.eax = cpu.ecx;
        return;
    }
    cpu.eax = if m.read32(cpu.eax.wrapping_add(0xC)) & 2 != 0 { 0x609 } else { 0x61A };
}

/// 00208AC0 cdecl(): 0 when 00227400(4) is zero, else 1 when 002273C0(4) is zero.
pub fn slot_4c_status_above_1_and_not_5<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    guest_call(c, m, cpu, 0x0022_7400, 0x0020_8AC7, 0, &[4]);
    if cpu.eax == 0 {
        cpu.eax = 0;
        return;
    }
    guest_call(c, m, cpu, 0x0022_73C0, 0x0020_8AD5, 0, &[4]);
    cpu.eax = u32::from(cpu.eax == 0);
}
