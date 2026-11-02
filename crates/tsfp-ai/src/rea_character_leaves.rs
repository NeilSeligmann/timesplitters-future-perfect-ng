// SPDX-License-Identifier: GPL-3.0-or-later
//! Character and spawn-table leaves of the REA AI (task filed under T1660, mask bits 21 to 26).
//!
//! Six call-free original functions, each read with REA (Ghidra on `tsfp-analysis-v4.elf`, sha256
//! `e99faaeb...f866f`, MEASURED static). They only read guest tables, so the typed owners take a
//! read-only [`TableView`] and return the original EAX value.
//!
//! * `0x12FC0(a, b)` second dword of the symmetric pair-table entry,
//! * `0x1C5210(group, index)` whether `index` lies outside the span of a spawn group,
//! * `0x1C5250()` number of spawn groups,
//! * `0x2AEFB0(group, slot)` behaviour table lookup,
//! * `0x2B2390()` whether no spawn-table entry has its low flag bit set,
//! * `0x350E70(object, mask)` type-table flag mask test of an object.

/// Read-only guest view the leaves need (the adapter implements it over the live guest memory).
pub trait TableView {
    fn read8(&self, address: u32) -> u8;
    fn read16(&self, address: u32) -> u16;
    fn read32(&self, address: u32) -> u32;
}

/// Pointer to the pair table (`0x74C368`), 12-byte entries in packed triangular order.
pub const PAIR_TABLE_POINTER: u32 = 0x74c368;
const PAIR_ENTRY_STRIDE: u32 = 12;

/// Address of the pair-table entry of an unordered pair (`0x15BD00`, inlined by `0x12FC0`).
pub fn pair_entry_address<M: TableView>(m: &M, a: u32, b: u32) -> u32 {
    let (high, low) = if (a as i32) > (b as i32) {
        (a, b)
    } else {
        (b, a)
    };
    let triangular = ((high.wrapping_sub(1).wrapping_mul(high) as i32) >> 1) as u32;
    m.read32(PAIR_TABLE_POINTER)
        .wrapping_add(triangular.wrapping_add(low).wrapping_mul(PAIR_ENTRY_STRIDE))
}

/// `0x12FC0(a, b)`: dword at `+4` of the pair entry.
pub fn pair_entry_field_4<M: TableView>(m: &M, a: u32, b: u32) -> u32 {
    m.read32(pair_entry_address(m, a, b).wrapping_add(4))
}

/// Number of spawn groups (`0x1C5250`).
pub const SPAWN_GROUP_COUNT: u32 = 0x2f;
/// Group boundary table: `[group] -> first index` (`0x4FC99C`), the span end of `g` is entry `g + 1`.
const GROUP_BOUNDARIES: u32 = 0x4fc99c;

/// `0x1C5250`: number of spawn groups.
pub fn spawn_group_count() -> u32 {
    SPAWN_GROUP_COUNT
}

/// `0x1C5210(group, index)`: signed `index >= span(group)`. Group 0 compares against the first
/// boundary, groups `1..0x2f` against `boundary[g + 1] - boundary[g]`, any other group gives 0.
pub fn group_index_outside<M: TableView>(m: &M, group: u32, index: u32) -> u32 {
    let span = if group == 0 {
        m.read32(GROUP_BOUNDARIES + 4)
    } else if (1..SPAWN_GROUP_COUNT).contains(&group) {
        let base = GROUP_BOUNDARIES.wrapping_add(group.wrapping_mul(4));
        m.read32(base.wrapping_add(4)).wrapping_sub(m.read32(base))
    } else {
        return 0;
    };
    u32::from((index as i32) >= (span as i32))
}

const BEHAVIOUR_TABLE: u32 = 0x520a48;

/// `0x2AEFB0(group, slot)`: dword `group + 4 * slot` of the behaviour table (4-byte entries).
pub fn behaviour_table_entry<M: TableView>(m: &M, group: u32, slot: u32) -> u32 {
    let entry = group.wrapping_add(slot.wrapping_mul(4));
    m.read32(entry.wrapping_mul(4).wrapping_add(BEHAVIOUR_TABLE))
}

/// Spawn table: entry count (`0x7A2BCC`) and 8-byte records (`0x7A2960`), record `+4` points to a
/// descriptor whose byte `+0x28` carries the flags.
const SPAWN_TABLE_COUNT: u32 = 0x7a2bcc;
const SPAWN_TABLE: u32 = 0x7a2960;
const DESCRIPTOR_FLAGS: u32 = 0x28;

/// `0x2B2390()`: 1 when no spawn-table descriptor has flag bit 0 set, else 0.
pub fn spawn_table_has_no_flagged_entry<M: TableView>(m: &M) -> u32 {
    let count = m.read32(SPAWN_TABLE_COUNT) as i32;
    let mut index = 0i32;
    while index < count {
        let record = SPAWN_TABLE.wrapping_add((index as u32).wrapping_mul(8));
        let descriptor = m.read32(record.wrapping_add(4));
        if m.read8(descriptor.wrapping_add(DESCRIPTOR_FLAGS)) & 1 != 0 {
            return 0;
        }
        index += 1;
        // The original re-reads the count after every step.
        if index >= m.read32(SPAWN_TABLE_COUNT) as i32 {
            break;
        }
    }
    1
}

/// Object type table: 20-byte rows from `0x53C508`, row = signed word at `object + 0x48`.
const TYPE_TABLE: u32 = 0x53c508;
/// Byte `0x7DE455` and flags `0x7DE458`: mode gate of the fallback result.
const MODE_BYTE: u32 = 0x7de455;
const MODE_FLAGS: u32 = 0x7de458;
const MODE_ALL_TYPES: u8 = 0x0a;
const MODE_FLAG_ALL_TYPES: u32 = 0x80000;

/// `0x350E70(object, mask)`: type flag mask test. Bit 0 of `mask` asks for a plain hit (result 1
/// when the row word intersects `mask`). Otherwise the row word is masked only while the mode
/// byte is 10 or the flags carry `0x80000`, else 0.
pub fn type_flag_mask_test<M: TableView>(m: &M, object: u32, mask: u32) -> u32 {
    let row = m.read16(object.wrapping_add(0x48)) as i16 as i32 as u32;
    let word = m.read32(row.wrapping_mul(5).wrapping_mul(4).wrapping_add(TYPE_TABLE));
    if mask & 1 != 0 && word & mask != 0 {
        return 1;
    }
    if m.read8(MODE_BYTE) == MODE_ALL_TYPES || m.read32(MODE_FLAGS) & MODE_FLAG_ALL_TYPES != 0 {
        word & mask
    } else {
        0
    }
}

// ---- T2430: weapon-record action tests, pair field 8, type-id class, message 0x3EB ----

/// `0x12FE0(a, b)`: dword at `+8` of the pair entry (`0x15BD00` result, then `[eax + 8]`).
pub fn pair_entry_field_8<M: TableView>(m: &M, a: u32, b: u32) -> u32 {
    m.read32(pair_entry_address(m, a, b).wrapping_add(8))
}

/// Type ids the `0x0D6DD0` jump table classifies as 1 (index byte 0 of its 0x99-entry table over
/// `id - 0x12`; every other id in `0x12..=0xAA` and all ids outside that range give 0).
pub const SPECIAL_TYPE_IDS: [u32; 22] = [
    0x12, 0x13, 0x2d, 0x34, 0x36, 0x42, 0x43, 0x45, 0x4a, 0x50, 0x5c, 0x5e, 0x60, 0x62, 0x69, 0x75,
    0x97, 0x9c, 0xa2, 0xa8, 0xa9, 0xaa,
];

/// `0x0D6DD0(id)`: 1 for a special type id (the jump table), else 0.
pub fn is_special_type_id(id: u32) -> u32 {
    u32::from(SPECIAL_TYPE_IDS.contains(&id))
}

/// `0x0D6ED0(id)`: 1 unless `id` is special or one of `0x4D`, `0x4F`, `0x64`.
pub fn is_ordinary_type_id(id: u32) -> u32 {
    u32::from(is_special_type_id(id) == 0 && !matches!(id, 0x4d | 0x4f | 0x64))
}

/// `0x27CE70(actor, message)`: message `0x3EB` sets ext flag `0x100000` (`ext+0x2c`), clears bit 0
/// of `actor+0x28` and returns `-2`; any other message returns 0 and changes nothing.
pub fn handle_message_3eb<M: TableView>(
    m: &M,
    mut write32: impl FnMut(u32, u32),
    actor: u32,
    message: u32,
) -> u32 {
    if message != 0x3eb {
        return 0;
    }
    let flags = m.read32(actor.wrapping_add(0x7c)).wrapping_add(0x2c);
    write32(flags, m.read32(flags) | 0x10_0000);
    let active = actor.wrapping_add(0x28);
    write32(active, m.read32(active) & !1);
    u32::MAX - 1
}

/// Record table pointer cell (`0x4F9BAC`) and its 0x29C-byte records (flag byte at `+0xc`).
const RECORD_TABLE_POINTER: u32 = 0x4f9bac;
const RECORD_STRIDE: u32 = 0x29c;
/// Which of the two per-record value columns applies: the original picks column 0 only when the
/// extension's record index (`ext+0x204`) is valid and the record's bit 0 is clear, else column 1.
fn weapon_record_column<M: TableView>(m: &M, object: u32) -> u32 {
    let index = m.read32(m.read32(object.wrapping_add(0x7c)).wrapping_add(0x204));
    if index == u32::MAX {
        return 1;
    }
    let record = m
        .read32(RECORD_TABLE_POINTER)
        .wrapping_add(index.wrapping_mul(RECORD_STRIDE));
    u32::from(m.read8(record.wrapping_add(0xc)) & 1 != 0)
}

/// Three-table lookup shared by `0x26CC80` and `0x26CD50`: `bases[i] + 4 * column`.
fn weapon_action_values<M: TableView>(m: &M, object: u32, bases: [u32; 3]) -> [u32; 3] {
    let column = weapon_record_column(m, object);
    bases.map(|base| m.read32(base.wrapping_add(column * 4)))
}

/// `0x26CC80(object)`: 1 when the owner record's slot `+0xd4` (`0x59D70`) equals any of the three
/// column values at `0x5202F8`, `0x520308`, `0x5202E8`, else 0.
pub fn weapon_action_slot_matches<M: TableView>(m: &M, object: u32) -> u32 {
    let slot = m.read32(m.read32(object.wrapping_add(4)).wrapping_add(0xd4));
    let values = weapon_action_values(m, object, [0x5202f8, 0x520308, 0x5202e8]);
    u32::from(values.contains(&slot))
}

/// `0x26CD50(object)`: 1 when the owner record holds any of the three column values (tables
/// `0x520338`, `0x520328`, `0x520318`) in slot `+0xd4` or `+0x12c` (`0x59D20`), else 0.
pub fn weapon_action_owner_has<M: TableView>(m: &M, object: u32) -> u32 {
    let record = m.read32(object.wrapping_add(4));
    let (a, b) = (
        m.read32(record.wrapping_add(0xd4)),
        m.read32(record.wrapping_add(0x12c)),
    );
    let values = weapon_action_values(m, object, [0x520338, 0x520328, 0x520318]);
    u32::from(
        values
            .iter()
            .any(|&w| crate::point_tables::owner_has_either_slot(a, b, w)),
    )
}

/// `0x26CE20(actor)`: the AI action helper returns an XMM0 scalar. The selected value comes from
/// the first matching owner slot in the original ordered table list. A valid weapon-record index
/// with byte `+0xc` bit 0 clear selects column 0; invalid indices and set bits select column 1.
/// The values are `-90`, `90`, or `180` degrees; no matching slot returns `+0`.
///
/// The table ordering is observable because the original returns on the first matching slot.
pub fn weapon_action_yaw_offset_bits<M: TableView>(m: &M, actor: u32) -> u32 {
    let column = weapon_record_column(m, actor);
    let owner = m.read32(actor.wrapping_add(4));
    let record = m.read32(owner.wrapping_add(4));
    let slots = (
        m.read32(record.wrapping_add(0xd4)),
        m.read32(record.wrapping_add(0x12c)),
    );
    // Each tuple is (table, scalar-result). Order follows the original conditional returns.
    let ordered = [
        (0x520308, 0x478634),
        (0x5202f8, 0x478628),
        (0x5202e8, 0x475c84),
        (0x520338, 0x475c84),
        (0x520328, 0x478628),
        (0x520318, 0x478634),
        (0x5203b8, 0x475c84),
    ];
    for (table, value) in ordered {
        let wanted = m.read32(table + column * 4);
        if crate::point_tables::owner_has_either_slot(slots.0, slots.1, wanted) {
            return m.read32(value);
        }
    }
    0
}
