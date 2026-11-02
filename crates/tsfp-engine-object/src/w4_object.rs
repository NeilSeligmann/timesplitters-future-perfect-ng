// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4 object ports (breadth first, Tier 0). Readable translations of lifted originals;
//! nested guest calls go through the closure model (`Callee`/`guest_call`). Register residue
//! (EAX/ECX/EDX at exit) is not modelled except EAX where it is the return value.
use super::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

const SLOT_FLAGS_ARRAY: u32 = 0x007A_2820;
const NAV_RECORD_TABLE: u32 = 0x0074_C36C;
const SLOT_STRIDE: u32 = 0x98;

fn slot_entry<M: GuestMemory>(m: &M, index: u32) -> u32 {
    m.read32(SLOT_FLAGS_ARRAY).wrapping_add(index.wrapping_mul(24))
}

fn slot_record<M: GuestMemory>(m: &M, index: u32) -> u32 {
    let table = m.read32(NAV_RECORD_TABLE);
    m.read32(table.wrapping_add(0x74)).wrapping_add(index.wrapping_mul(SLOT_STRIDE))
}

/// 00162200 cdecl(index): clears entry+0x10 of array 7A2820 (24-byte entries), then sets it to 1
/// when callee 00234270([slot+0x3C]) returns nonzero.
pub fn array_entry_flag_from_object_field_3c<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let index = stack_arg(m, cpu, 0);
    let record = slot_record(m, index);
    let entry = slot_entry(m, index);
    m.write32(entry.wrapping_add(0x10), 0);
    let object = m.read32(record.wrapping_add(0x3C));
    guest_call(c, m, cpu, 0x0023_4270, 0x0016_2234, 4, &[object]);
    if cpu.eax != 0 {
        m.write32(entry.wrapping_add(0x10), 1);
    }
}

/// 001622C0 cdecl(index): for each of the [790950] players (stride 0xEA0 in the table at 7356D8,
/// id at +0x678, record = [715F2C] + id*32 read at +0) sets entry+0x10 to 1 when that value is
/// valid (not -1, > 0) and equals one of the slot's four ids at +0x38..+0x44.
pub fn array_entry_flag_when_slot_matches_record_ids<M: GuestMemory>(
    m: &mut M,
    cpu: &Cpu,
) {
    let index = stack_arg(m, cpu, 0);
    let record = slot_record(m, index);
    let entry = slot_entry(m, index);
    let mut player = 0i32;
    let mut count = m.read32(0x0079_0950) as i32;
    if count <= 0 {
        return;
    }
    let mut offset = 0u32;
    loop {
        let table = m.read32(0x0073_56D8);
        let id = m.read32(table.wrapping_add(offset).wrapping_add(0x678));
        let records = m.read32(0x0071_5F2C);
        let value = m.read32(id.wrapping_shl(5).wrapping_add(records));
        if value != 0xFFFF_FFFF {
            let hit = [0x38u32, 0x3C, 0x40, 0x44]
                .iter()
                .any(|field| m.read32(record.wrapping_add(*field)) == value);
            if hit {
                m.write32(entry.wrapping_add(0x10), 1);
            }
        }
        count = m.read32(0x0079_0950) as i32;
        player += 1;
        offset = offset.wrapping_add(0xEA0);
        if player >= count {
            break;
        }
    }
}

/// 00087260 cdecl(a, b, list, count, ids): for each u16 id in `ids[0..count]` whose room record
/// ([708788] + id*0xB4) lacks mark 0x10 at +0xB2: sets the mark, calls 00047560(a, b, rec+0x10,
/// rec+0x1C) and appends `result | id*2` (u16) to `list[1..]`. list[0] holds the running count.
/// Returns 0 in EAX.
// portreg: va=0x00087260
pub fn room_collect_unmarked_records_into_list<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let (a, b) = (stack_arg(m, cpu, 0), stack_arg(m, cpu, 1));
    let list = stack_arg(m, cpu, 2);
    let count = stack_arg(m, cpu, 3) as i32;
    let ids = stack_arg(m, cpu, 4);
    let mut used = u32::from(m.read16(list));
    if count > 0 {
        let mut base = m.read32(0x0070_8788);
        for i in 0..count as u32 {
            let id = u32::from(m.read16(ids.wrapping_add(i * 2)));
            let rec = id.wrapping_mul(0xB4).wrapping_add(base);
            if m.read8(rec.wrapping_add(0xB2)) & 0x10 != 0 {
                continue;
            }
            m.write8(rec.wrapping_add(0xB2), m.read8(rec.wrapping_add(0xB2)) | 0x10);
            guest_call(
                c, m, cpu, 0x0004_7560, 0x0008_72C1, 16,
                &[a, b, rec.wrapping_add(0x10), rec.wrapping_add(0x1C)],
            );
            let value = (cpu.eax | (id << 1)) as u16;
            m.write8(list.wrapping_add(used * 2 + 2), value as u8);
            m.write8(list.wrapping_add(used * 2 + 3), (value >> 8) as u8);
            used += 1;
            base = m.read32(0x0070_8788);
        }
    }
    let used = used as u16;
    m.write8(list, used as u8);
    m.write8(list.wrapping_add(1), (used >> 8) as u8);
    cpu.eax = 0;
}

// portreg: va=0x0005f430
/// 0005F430 cdecl(prop): calls 0005F040([prop+4]) then sets bit 1 of +0x18 and bit 0x200000 of +0x28.
pub fn prop_anim_advance_track_then_set_flags<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let prop = stack_arg(m, cpu, 0);
    let track = m.read32(prop.wrapping_add(4));
    guest_call(c, m, cpu, 0x0005_F040, 0x0005_F43E, 4, &[track]);
    m.write32(prop.wrapping_add(0x18), m.read32(prop.wrapping_add(0x18)) | 2);
    m.write32(prop.wrapping_add(0x28), m.read32(prop.wrapping_add(0x28)) | 0x0020_0000);
}

/// 0033CC90 is a tail jump to 0033CB90 with the caller's frame untouched.
pub fn tail_jump_to_33cb90<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    c.call(m, 0x0033_CB90, cpu);
}

/// 00167780 cdecl(object): when [[object+4]+0x10] is nonzero calls 00153190(that, matrix, node)
/// with matrix = [74C458] + (u16 [object+0x80] << 9), node = [[[object+4]+4]+4], then zeroes
/// floats +0x70, +0xC0, +0xC8; always sets bit 0x200000 of +0x28.
pub fn object_copy_matrices_to_entry_clear_fields<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let object = stack_arg(m, cpu, 0);
    let link = m.read32(object.wrapping_add(4));
    let source = m.read32(link.wrapping_add(0x10));
    if source != 0 {
        let inner = m.read32(link.wrapping_add(4));
        let node = m.read32(inner.wrapping_add(4));
        let matrix = (m.read32(object.wrapping_add(0x80)) & 0xFFFF)
            .wrapping_shl(9)
            .wrapping_add(m.read32(0x0074_C458));
        guest_call(c, m, cpu, 0x0015_3190, 0x0016_77B3, 4, &[source, matrix, node]);
        for field in [0xC0u32, 0x70, 0xC8] {
            m.write32(object.wrapping_add(field), 0);
        }
    }
    m.write32(object.wrapping_add(0x28), m.read32(object.wrapping_add(0x28)) | 0x0020_0000);
}
