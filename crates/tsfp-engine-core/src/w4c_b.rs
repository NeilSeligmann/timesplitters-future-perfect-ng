// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4c engine/core ports, second half (Tier 0: translated, compiles, smoke tested).
//! Translated from the lifted originals; nested guest calls are recorded through the closure
//! model. Register residue other than EAX (return value) is not modelled. Functions are named by
//! behaviour and carry their original VA (RVA form as in the function tables).
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

const LAUNCH_FLAG_6B7A94: u32 = 0x006B_7A94;
const LAUNCH_FLAG_6B7A98: u32 = 0x006B_7A98;
const LAUNCH_FLAG_6B7A9C: u32 = 0x006B_7A9C;
const LAUNCH_FLAG_6B7AA0: u32 = 0x006B_7AA0;
const LAUNCH_FLAG_6B7AA4: u32 = 0x006B_7AA4;
const LAUNCH_VALUE_4C0468: u32 = 0x004C_0468;
const LAUNCH_ARRAY_6B7A28: u32 = 0x006B_7A28;
const STREAM_GROUP_TABLE: u32 = 0x006B_83D8;
const STREAM_ENTRY_TABLE: u32 = 0x006B_83E4;
const STREAM_ENTRY_STRIDE: u32 = 0x30;
const PROFILE_TABLE: u32 = 0x0051_5FF8;
const PROFILE_STRIDE: u32 = 0x80;

/// 0003D2E0 cdecl(text): launch-argument handler, stores atol-like callee 003C8FD0(text) at
/// 6B7A90 and returns 1 (EAX).
pub fn launch_arg_set_6b7a90_from_atol<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let text = stack_arg(m, cpu, 0);
    guest_call(c, m, cpu, 0x003C_8FD0, 0x0003_D2EA, 0, &[text]);
    m.write32(0x006B_7A90, cpu.eax);
    cpu.eax = 1;
}

fn set_flag_returning_one<M: GuestMemory>(m: &mut M, cpu: &mut Cpu, address: u32) {
    m.write32(address, 1);
    cpu.eax = 1;
}

/// 0003D300 launch-argument handler: sets 6B7A94 to 1, returns 1.
pub fn launch_arg_set_flag_6b7a94<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    set_flag_returning_one(m, cpu, LAUNCH_FLAG_6B7A94);
}
/// 0003D310 launch-argument handler: sets 6B7A98 to 1, returns 1.
pub fn launch_arg_set_flag_6b7a98<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    set_flag_returning_one(m, cpu, LAUNCH_FLAG_6B7A98);
}
/// 0003D320 launch-argument handler: clears 4C0468, returns 1.
pub fn launch_arg_clear_4c0468<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    m.write32(LAUNCH_VALUE_4C0468, 0);
    cpu.eax = 1;
}
/// 0003D330 launch-argument handler: sets 6B7A9C to 1, returns 1.
pub fn launch_arg_set_flag_6b7a9c<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    set_flag_returning_one(m, cpu, LAUNCH_FLAG_6B7A9C);
}
/// 0003D340 launch-argument handler: sets 6B7AA0 to 1, returns 1.
pub fn launch_arg_set_flag_6b7aa0<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    set_flag_returning_one(m, cpu, LAUNCH_FLAG_6B7AA0);
}
/// 0003D350 launch-argument handler: sets 6B7AA4 to 1, returns 1.
pub fn launch_arg_set_flag_6b7aa4<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    set_flag_returning_one(m, cpu, LAUNCH_FLAG_6B7AA4);
}

/// 0003D360 cdecl(index): EAX = dword array 6B7A28[index] (no bounds check in the original).
pub fn launch_array_6b7a28_get<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    let index = stack_arg(m, cpu, 0);
    cpu.eax = m.read32(LAUNCH_ARRAY_6B7A28.wrapping_add(index.wrapping_mul(4)));
}

/// 0003DB70: clears the dword at 6B7B10.
pub fn global_6b7b10_clear<M: GuestMemory>(m: &mut M) {
    m.write32(0x006B_7B10, 0);
}

/// 0003ED60: zeroes the eight dwords 6B83D8..6B83F4 (EAX = 0 at exit).
pub fn globals_6b83d8_block_clear<M: GuestMemory>(m: &mut M, cpu: &mut Cpu) {
    for i in 0..8 {
        m.write32(STREAM_GROUP_TABLE + 4 * i, 0);
    }
    cpu.eax = 0;
}

/// 0003ED90 cdecl(group): stream-block pool group (16-byte stride: count at 6B83D8, entry array
/// pointer at 6B83E4, 0x30-byte entries). Each entry whose +8 is nonzero has its callback at +0xC
/// called cdecl(entry[0], entry[1], index, 1); every entry then has +8/+0x14/+0x28 cleared and
/// +0x10/+0x18/+0x24 set to -1. EAX = 0 at exit.
pub fn stream_block_pool_group_reset<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let group = stack_arg(m, cpu, 0).wrapping_shl(4);
    cpu.eax = 0;
    let mut index = 0i32;
    if (m.read32(group.wrapping_add(STREAM_GROUP_TABLE)) as i32) <= 0 {
        return;
    }
    let mut offset = 0u32;
    loop {
        let entry = m.read32(group.wrapping_add(STREAM_ENTRY_TABLE)).wrapping_add(offset);
        if m.read32(entry.wrapping_add(8)) != 0 {
            let first = m.read32(entry);
            let second = m.read32(entry.wrapping_add(4));
            let callback = m.read32(entry.wrapping_add(0xC));
            // Four saved registers (EBX, EDI, EBP, ESI) sit below the entry ESP.
            guest_call(c, m, cpu, callback, 0x0003_EDCD, 16, &[first, second, index as u32, 1]);
        }
        m.write32(entry.wrapping_add(8), 0);
        m.write32(entry.wrapping_add(0x10), u32::MAX);
        m.write32(entry.wrapping_add(0x24), u32::MAX);
        m.write32(entry.wrapping_add(0x14), 0);
        m.write32(entry.wrapping_add(0x28), 0);
        m.write32(entry.wrapping_add(0x18), u32::MAX);
        index += 1;
        offset = offset.wrapping_add(STREAM_ENTRY_STRIDE);
        if index >= m.read32(group.wrapping_add(STREAM_GROUP_TABLE)) as i32 {
            break;
        }
    }
    cpu.eax = 0;
}

/// 0003EE00: runs 0003ED90 for groups 0 and 1 (return PC 0003EE09, one saved ESI below).
pub fn release_stream_block_pool_groups_0_and_1<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    for group in 0..2u32 {
        guest_call(c, m, cpu, 0x0003_ED90, 0x0003_EE09, 4, &[group]);
    }
}

/// 000454F0: tail jump to 00027700 with the caller's frame unchanged.
pub fn wrapper_sound_data_load<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    c.call(m, 0x0002_7700, cpu);
}

/// 00196450: tail jump to 00037300 with the caller's frame unchanged.
pub fn thunk_launch_new_image_31b10_wrapper<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    c.call(m, 0x0003_7300, cpu);
}

/// 00056EC0: copies launch flags 6B7A94/98/9C/A0/A4 to 6D6128/6D612C/6D6108/6D610C/6D6110 and
/// 4C0468 to 4D1940, clears 6D615C, calls 000CB3B0([7B0CC0]) then stores 003C848C() at 6D6140.
pub fn launch_flags_copy_and_init<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    m.write32(0x006D_6128, m.read32(LAUNCH_FLAG_6B7A94));
    m.write32(0x006D_6108, m.read32(LAUNCH_FLAG_6B7A9C));
    m.write32(0x006D_612C, m.read32(LAUNCH_FLAG_6B7A98));
    m.write32(0x004D_1940, m.read32(LAUNCH_VALUE_4C0468));
    m.write32(0x006D_610C, m.read32(LAUNCH_FLAG_6B7AA0));
    m.write32(0x006D_6110, m.read32(LAUNCH_FLAG_6B7AA4));
    m.write32(0x006D_615C, 0);
    let argument = m.read32(0x007B_0CC0);
    guest_call(c, m, cpu, 0x000C_B3B0, 0x0005_6F19, 0, &[argument]);
    guest_call(c, m, cpu, 0x003C_848C, 0x0005_6F21, 0, &[]);
    m.write32(0x006D_6140, cpu.eax);
}

/// 0006A500 cdecl(key): scans the 0x80-byte records at 515FF8 (first record's word at +4 zero ends
/// the table, as does any later record whose word at +4 is zero) for a record whose dword at
/// +8 equals key (the lifted compare reads [record+4+4]). EAX = record address, or 0.
/// INFERRED: the table name "find by field 0x4" disagrees with the +8 compare in the lifted code.
pub fn table_515ff8_find_entry<M: GuestMemory>(m: &M, cpu: &mut Cpu) {
    let key = stack_arg(m, cpu, 0);
    cpu.eax = 0;
    if m.read16(PROFILE_TABLE + 4) == 0 {
        return;
    }
    let mut cursor = PROFILE_TABLE + 4;
    let mut index = 0u32;
    loop {
        if m.read32(cursor.wrapping_add(4)) == key {
            cpu.eax = index.wrapping_shl(7).wrapping_add(PROFILE_TABLE);
            return;
        }
        cursor = cursor.wrapping_add(PROFILE_STRIDE);
        index += 1;
        if m.read16(cursor) == 0 {
            return;
        }
    }
}

/// 00121A90 cdecl(record, owner): zeroes 0x1F4 bytes at record, stores owner at +0 and -1 at
/// +0x54 and +0x74.
// portreg: va=0x00121a90 tier=0
pub fn init_record_owner_ids_minus1<M: GuestMemory>(m: &mut M, cpu: &Cpu) {
    let record = stack_arg(m, cpu, 0);
    let owner = stack_arg(m, cpu, 1);
    for i in 0..0x7D {
        m.write32(record.wrapping_add(4 * i), 0);
    }
    m.write32(record, owner);
    m.write32(record.wrapping_add(0x54), u32::MAX);
    m.write32(record.wrapping_add(0x74), u32::MAX);
}

/// 0018E8D0 cdecl(a, b): calls 00022040(&a, &b) (launch info query writing into the two argument
/// slots), stores them to 74CF68/74CF6C, calls 0003C330() then 0003C2D0() which never returns.
pub fn launch_info_query_store_pair<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let slot_a = cpu.esp.wrapping_add(4);
    let slot_b = cpu.esp.wrapping_add(8);
    guest_call(c, m, cpu, 0x0002_2040, 0x0018_E8DF, 0, &[slot_a, slot_b]);
    m.write32(0x0074_CF68, m.read32(slot_a));
    m.write32(0x0074_CF6C, m.read32(slot_b));
    guest_call(c, m, cpu, 0x0003_C330, 0x0018_E8FA, 0, &[]);
    guest_call(c, m, cpu, 0x0003_C2D0, 0x0018_E8FF, 0, &[]);
}

/// 00257B50: resets 75F5A8/75F5AC (-10)/75F5B0 (0.0). Mode [79094C] == 0x1E: calls
/// 00128440(1, 0x24D, 0, 0x5201C0, 0x75F5B4) then 00087600(2, 8); modes 0x21 or 0x22 tail jump to
/// 001F2560; other modes return.
pub fn init_globals_75f5a8_then_dispatch_by_mode<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let mode = m.read32(0x0079_094C) as i32;
    m.write32(0x0075_F5AC, 0xFFFF_FFF6);
    m.write32(0x0075_F5A8, 0);
    m.write32(0x0075_F5B0, 0);
    match mode {
        0x1E => {
            guest_call(c, m, cpu, 0x0012_8440, 0x0025_7B91, 0, &[1, 0x24D, 0, 0x0052_01C0, 0x0075_F5B4]);
            // The five 128440 arguments are still on the stack (cleaned at 00257B9A).
            guest_call(c, m, cpu, 0x0008_7600, 0x0025_7B9A, 20, &[2, 8]);
        }
        0x21 | 0x22 => c.call(m, 0x001F_2560, cpu),
        _ => {}
    }
}

/// 00288B80: fills the float at +0x18 of 0x1F4 32-byte records of the buffer at [78B1EC] with the
/// constant at 475D5C, then writes terminator 0x1F3 at +0x3E80 and 0 at +0x3E84.
pub fn init_buffer_78b1ec_floats_and_terminator<M: GuestMemory>(m: &mut M) {
    let value = m.read32(0x0047_5D5C);
    let mut offset = 0u32;
    while offset < 0x3E80 {
        let base = m.read32(0x0078_B1EC);
        m.write32(base.wrapping_add(offset).wrapping_add(0x18), value);
        offset += 0x20;
    }
    let base = m.read32(0x0078_B1EC);
    m.write32(base.wrapping_add(0x3E80), 0x1F3);
    let base = m.read32(0x0078_B1EC);
    m.write32(base.wrapping_add(0x3E84), 0);
}

/// 002BD970 cdecl(params): fills the effect parameter struct (floats copied bitwise from
/// 47DE90 -> +0xC, 475D20 -> +0x10, 49F52C -> +0, 49F528 -> +4; +8/+0x1C/+0x20 = 0.0) and stores
/// two texture-table entries at +0x14/+0x18 from 00080F20(4) and (8) when [4C0454] == 4,
/// else from 00080F20(6) and (5).
pub fn init_effect_params_with_two_texture_entries<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let params = stack_arg(m, cpu, 0);
    m.write32(params.wrapping_add(0xC), m.read32(0x0047_DE90));
    m.write32(params.wrapping_add(0x10), m.read32(0x0047_5D20));
    m.write32(params, m.read32(0x0049_F52C));
    m.write32(params.wrapping_add(8), 0);
    m.write32(params.wrapping_add(4), m.read32(0x0049_F528));
    m.write32(params.wrapping_add(0x1C), 0);
    m.write32(params.wrapping_add(0x20), 0);
    let (first, second) = if m.read32(0x004C_0454) == 4 { (4, 8) } else { (6, 5) };
    // One saved ESI sits below the entry ESP.
    guest_call(c, m, cpu, 0x0008_0F20, if first == 4 { 0x002B_D9CA } else { 0x002B_D9E3 }, 4, &[first]);
    m.write32(params.wrapping_add(0x14), cpu.eax);
    // The first call's argument is still on the stack (popped after both calls): 8 bytes below the entry ESP.
    guest_call(c, m, cpu, 0x0008_0F20, if first == 4 { 0x002B_D9D4 } else { 0x002B_D9ED }, 8, &[second]);
    m.write32(params.wrapping_add(0x18), cpu.eax);
}
