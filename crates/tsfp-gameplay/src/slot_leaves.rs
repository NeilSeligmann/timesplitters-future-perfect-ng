// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 12 gameplay leaves: player/side slot lookups, record field accessors and global
//! stores, ported as readable Rust from call-free C replacements of `src/game` (see
//! docs/t1836-rust-wave12.md). Each function takes the entry registers `e` and returns the
//! final EAX/ECX/EDX the C body leaves (the ABI contract of the differential harness). Stack
//! arguments are read through `esp` (return address at `esp`), reads and writes keep the C
//! order so aliasing stores behave as in the original.
use crate::{gprs::Gprs, memory::GuestMemory};

/// Table of two-dword side entries, entry `n` at `SIDE_TABLE + 8 * n` points at a side object.
const SIDE_TABLE: u32 = 0x007a_2be4;
/// Same table shifted by one entry, indexed by the `flag == 1` selector.
const SIDE_TABLE_FLAGGED: u32 = 0x007a_2bec;
const EXTENSION_POINTER: u32 = 0x7c;
const EXTENSION_FLAGS: u32 = 0xa0;
const SIDE_FLAGS: u32 = 0x28;

fn arg<M: GuestMemory>(m: &M, esp: u32, index: u32) -> u32 {
    m.read32(esp.wrapping_add(4 + 4 * index))
}

fn side_object<M: GuestMemory>(m: &M, base: u32, selector: u32) -> u32 {
    m.read32(selector.wrapping_mul(8).wrapping_add(base))
}

/// 00274330 game_object_index_extension_mask4. cdecl(1: side index). Bit 2 of the extension
/// flags of the side object.
pub fn side_extension_flag4_by_index<M: GuestMemory>(m: &M, esp: u32, _e: Gprs) -> Gprs {
    let index = arg(m, esp, 0);
    let ecx = side_object(m, SIDE_TABLE, index);
    let edx = m.read32(ecx.wrapping_add(EXTENSION_POINTER));
    Gprs {
        eax: m.read32(edx.wrapping_add(EXTENSION_FLAGS)) & 4,
        ecx,
        edx,
    }
}

/// 00274350 game_object_flag_slot_extension_mask4. cdecl(1: flag). Selects the entry
/// `(flag == 1)` of the flagged table and returns bit 2 of its extension flags.
pub fn side_extension_flag4_by_flag<M: GuestMemory>(m: &M, esp: u32, _e: Gprs) -> Gprs {
    let selector = u32::from(arg(m, esp, 0) == 1);
    let ecx = side_object(m, SIDE_TABLE_FLAGGED, selector);
    let edx = m.read32(ecx.wrapping_add(EXTENSION_POINTER));
    Gprs {
        eax: m.read32(edx.wrapping_add(EXTENSION_FLAGS)) & 4,
        ecx,
        edx,
    }
}

/// 00274380 game_object_index_slot_bit7_clear. cdecl(1: side index). 1 when bit 7 of the
/// side flags is clear.
pub fn side_bit7_clear<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let ecx = side_object(m, SIDE_TABLE, arg(m, esp, 0));
    let flags = m.read32(ecx.wrapping_add(SIDE_FLAGS));
    Gprs {
        eax: !(flags >> 7) & 1,
        ecx,
        edx: e.edx,
    }
}

/// 002744F0 game_object_slot_from_flag. cdecl(1: flag). Side object for the slot after
/// `(flag == 1)`: slot 1 for any flag but 1, slot 2 for flag 1.
pub fn side_object_from_flag<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let flag = arg(m, esp, 0);
    let slot = u32::from(flag == 1) + 1;
    Gprs {
        eax: side_object(m, SIDE_TABLE, slot),
        ecx: flag,
        edx: e.edx,
    }
}

const ENTRY_TABLE: u32 = 0x0074_c458;
const ENTRY_STRIDE_SHIFT: u32 = 9;
const ENTRY_FIELD_1C4: u32 = 0x1c4;

/// 00168160 game_entry_74c458_set_field_0x1c4. cdecl(2: id, value). Stores `value` in field
/// 0x1C4 of the 512-byte entry `id & 0xFFFF` of the table at `[0x74C458]`. Returns the byte
/// offset of the entry.
pub fn entry_set_field_1c4<M: GuestMemory>(m: &mut M, esp: u32, _e: Gprs) -> Gprs {
    let id = arg(m, esp, 0);
    let value = arg(m, esp, 1);
    let table = m.read32(ENTRY_TABLE);
    let offset = (id & 0xffff) << ENTRY_STRIDE_SHIFT;
    m.write32(
        offset.wrapping_add(table).wrapping_add(ENTRY_FIELD_1C4),
        value,
    );
    Gprs {
        eax: offset,
        ecx: value,
        edx: table,
    }
}

const RECORD_VEC3: u32 = 0x98;
const RECORD_FLAGS: u32 = 0x4;
const RECORD_FLAG_POSITION_SET: u32 = 0x4000;

/// 0010E9A0 game_record_set_vec3_0x98_flag_0x4000. cdecl(2: record, vec3 pointer). A null
/// record does nothing, else copies the three dwords at `vec3` to record+0x98 and marks the
/// record flags (+4) with 0x4000.
pub fn record_set_position<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let record = arg(m, esp, 0);
    if record == 0 {
        return Gprs {
            eax: 0,
            ecx: e.ecx,
            edx: e.edx,
        };
    }
    let source = arg(m, esp, 1);
    let x = m.read32(source);
    m.write32(record.wrapping_add(RECORD_VEC3), x);
    let y = m.read32(source.wrapping_add(4));
    m.write32(record.wrapping_add(RECORD_VEC3 + 4), y);
    let z = m.read32(source.wrapping_add(8));
    m.write32(record.wrapping_add(RECORD_VEC3 + 8), z);
    let flags = m.read32(record.wrapping_add(RECORD_FLAGS));
    m.write32(
        record.wrapping_add(RECORD_FLAGS),
        flags | RECORD_FLAG_POSITION_SET,
    );
    // The C body leaves the y word in EDX and the z word in ECX.
    Gprs {
        eax: record,
        ecx: z,
        edx: y,
    }
}

/// Signed upper bound and pointer of the object's indexed dword array. The caller supplies
/// the index; the original does not check its lower bound.
const OBJECT_ARRAY_COUNT: u32 = 0x1e8;
const OBJECT_ARRAY_POINTER: u32 = 0x1fc;
const ARRAY_FALLBACK_VALUE: u32 = 0x007a_d288;

/// 00012F30 game_bounded_array_read. cdecl(2: object, index). Reads a dword from the
/// object's indexed array when signed `index < count`, otherwise reads the global fallback.
/// A negative index also enters the array branch; preserve that guest address arithmetic.
pub fn bounded_array_read<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let object = arg(m, esp, 0);
    let index = arg(m, esp, 1);
    let count = m.read32(object.wrapping_add(OBJECT_ARRAY_COUNT));
    if (index as i32) >= count as i32 {
        return Gprs {
            eax: m.read32(ARRAY_FALLBACK_VALUE),
            ecx: object,
            edx: e.edx,
        };
    }
    let array = m.read32(object.wrapping_add(OBJECT_ARRAY_POINTER));
    Gprs {
        eax: m.read32(array.wrapping_add(index.wrapping_mul(4))),
        ecx: array,
        edx: e.edx,
    }
}

#[cfg(test)]
mod bounded_array_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct SparseMemory(BTreeMap<u32, u8>);

    impl GuestMemory for SparseMemory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn signed_bound_and_register_contract() {
        let mut m = SparseMemory::default();
        let esp = 0x1000;
        let object = 0x2000;
        let array = 0x3000;
        let entry = Gprs {
            eax: 9,
            ecx: 8,
            edx: 7,
        };
        m.write32(esp + 4, object);
        m.write32(object + OBJECT_ARRAY_COUNT, 2);
        m.write32(object + OBJECT_ARRAY_POINTER, array);
        m.write32(array + 4, 0x1234);
        m.write32(array.wrapping_sub(4), 0xabcd);
        m.write32(ARRAY_FALLBACK_VALUE, 0x5678);
        for (index, eax, ecx) in [
            (1, 0x1234, array),
            (2, 0x5678, object),
            (u32::MAX, 0xabcd, array),
        ] {
            m.write32(esp + 8, index);
            assert_eq!(
                bounded_array_read(&m, esp, entry),
                Gprs { eax, ecx, edx: 7 }
            );
        }
    }
}

const WEAPON_RECORDS: u32 = 0x004f_9bac;
const WEAPON_RECORD_SIZE: u32 = 0x29c;

/// 00012F50 game_indexed_record_field_c. cdecl(1: index). Dword +0xC of record `index` of the
/// table whose base is stored at `[0x4F9BAC]`, 0 for the index -1.
pub fn indexed_record_field_c<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let index = arg(m, esp, 0);
    if index == u32::MAX {
        return Gprs { eax: 0, ..e };
    }
    let base = m.read32(WEAPON_RECORDS);
    Gprs {
        eax: m.read32(
            index
                .wrapping_mul(WEAPON_RECORD_SIZE)
                .wrapping_add(base)
                .wrapping_add(0xc),
        ),
        ecx: base,
        edx: e.edx,
    }
}

/// 00150200 game_gp_b2_three_global_outputs. cdecl(3: out_a, out_b, out_c). Copies the globals
/// 0x749160, 0x749158 and 0x749148 to the three output pointers in that order.
// portreg: va=0x00150200 tier=1
pub fn three_global_outputs<M: GuestMemory>(m: &mut M, esp: u32, _e: Gprs) -> Gprs {
    let (out_a, out_b, out_c) = (arg(m, esp, 0), arg(m, esp, 1), arg(m, esp, 2));
    let first = m.read32(0x0074_9160);
    m.write32(out_a, first);
    let second = m.read32(0x0074_9158);
    m.write32(out_b, second);
    let third = m.read32(0x0074_9148);
    m.write32(out_c, third);
    Gprs {
        eax: second,
        ecx: out_c,
        edx: third,
    }
}

/// 00227BE0 game_set_global_pair_75c148. cdecl(2: a, b). `[0x75C148] = a`, `[0x75C150] = b`.
pub fn set_global_pair_75c148<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let (first, second) = (arg(m, esp, 0), arg(m, esp, 1));
    m.write32(0x0075_c148, first);
    m.write32(0x0075_c150, second);
    Gprs {
        eax: first,
        ecx: second,
        edx: e.edx,
    }
}

/// 0023EF70 game_set_global_78cd54. cdecl(1: value). `[0x78CD54] = value`, returns value.
pub fn set_global_78cd54<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let value = arg(m, esp, 0);
    m.write32(0x0078_cd54, value);
    Gprs { eax: value, ..e }
}

/// 0025B770 game_store_five_args_78b540. cdecl(5: a, b, c, d, e). Stores a, c, d, e to
/// 0x78B540.. and b to 0x78B550, returns e.
// portreg: va=0x0025b770 tier=0
pub fn store_five_args_78b540<M: GuestMemory>(m: &mut M, esp: u32, e: Gprs) -> Gprs {
    let a = |index| arg(m, esp, index);
    let (first, second, third, fourth, fifth) = (a(0), a(1), a(2), a(3), a(4));
    m.write32(0x0078_b540, first);
    m.write32(0x0078_b544, third);
    m.write32(0x0078_b548, fourth);
    m.write32(0x0078_b54c, fifth);
    m.write32(0x0078_b550, second);
    Gprs { eax: fifth, ..e }
}

/// 002586A0 game_clear_three_globals. cdecl(0). Zeroes 0x75F5CC, 0x5201E8 and 0x5201EC.
// portreg: va=0x002586a0 tier=1
pub fn clear_three_globals<M: GuestMemory>(m: &mut M, e: Gprs) -> Gprs {
    m.write32(0x0075_f5cc, 0);
    m.write32(0x0052_01e8, 0);
    m.write32(0x0052_01ec, 0);
    Gprs { eax: 0, ..e }
}

/// 0009E750 game_any_index_flagged. cdecl(1: object). 1 when any of the first `count` indices
/// (signed byte at +0x30, dwords from +0x38) selects a non-zero byte of the table whose base is
/// stored at `[0x7B977C]`.
pub fn any_index_flagged<M: GuestMemory>(m: &M, esp: u32, e: Gprs) -> Gprs {
    let object = arg(m, esp, 0);
    let count = i32::from(m.read8(object.wrapping_add(0x30)) as i8);
    let table = m.read32(0x007b_977c);
    let flagged = (0..count).any(|index| {
        let selected = m.read32(object.wrapping_add(0x38 + 4 * index as u32));
        m.read8(table.wrapping_add(selected)) != 0
    });
    Gprs {
        eax: u32::from(flagged),
        ..e
    }
}
