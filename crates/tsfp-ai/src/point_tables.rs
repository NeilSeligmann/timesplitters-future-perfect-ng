// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable owners for the AI "point" record tables and the owner slot test (T2422).
//!
//! The guest keeps three 32-byte-stride record tables (base pointers `0x715F2C`, `0x715F34`,
//! `0x715F38`). A record id selects the table: ids below [`PRIMARY_KEY_LIMIT`] live in the
//! primary table, ids from [`SECONDARY_ID_BASE`] up in the secondary one, and a small band
//! after the secondary range in the tertiary table. Both lookups are exact-match binary
//! searches over the first dword of each record. Nothing here touches guest memory: callers
//! supply the key accessor, so the same owner serves the runtime adapter and the tests.
//!
//! Sources: lifted `sub_0009CC40` (0x9CC40), `sub_00059D20` (0x59D20), `sub_0009A420` (0x9A420)
//! and `sub_0009A3C0` (0x9A3C0). Evidence: INFERRED from the lifted C, Tier 1 weak
//! differentials in `tests/point_tables_diff.rs`.

/// Record ids from here on are looked up in the secondary table (0x9CC40 falls through to 0x9A3C0).
pub const PRIMARY_KEY_LIMIT: i32 = 0x2329;
/// First id of the secondary table; its record index is `id - SECONDARY_ID_BASE`.
pub const SECONDARY_ID_BASE: i32 = 0x1f40;
/// End (exclusive) of the secondary id range that always resolves to the secondary table.
pub const SECONDARY_ID_END: i32 = 0x209e;
/// Extended secondary end (exclusive) used while the game is on level [`EXTENDED_SECONDARY_LEVEL`].
pub const SECONDARY_ID_END_EXTENDED: i32 = 0x2440;
/// Level id whose secondary range is [`SECONDARY_ID_END_EXTENDED`].
pub const EXTENDED_SECONDARY_LEVEL: u32 = 0x64;
/// First id of the tertiary table; its record index is `id - TERTIARY_ID_BASE`.
pub const TERTIARY_ID_BASE: i32 = 0x209e;
/// End (exclusive) of the tertiary id range.
pub const TERTIARY_ID_END: i32 = 0x20b2;
/// Size in bytes of one record in every point table.
pub const RECORD_STRIDE: u32 = 32;

/// Exact-match binary search over `count` records sorted by signed key.
///
/// Returns the record index or `None`. The probe sequence (start at `count >> 1`, bisect between
/// a moving low and high bound, stop when the midpoint repeats) is the original's, so a table
/// that is not sorted gives the same answer as the guest.
pub fn binary_search_key(count: i32, key: i32, mut key_at: impl FnMut(i32) -> i32) -> Option<i32> {
    let mut high = count;
    let mut low = 0i32;
    let mut mid = count >> 1;
    if mid == -1 {
        return None;
    }
    loop {
        let probe = key_at(mid);
        if key >= probe {
            if key == probe {
                return Some(mid);
            }
            low = mid;
        } else {
            high = mid;
        }
        let previous = mid;
        mid = (high.wrapping_sub(low) >> 1).wrapping_add(low);
        if mid == previous {
            return None;
        }
    }
}

/// Which table an id belongs to and the record index inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordSlot {
    Primary(u32),
    Secondary(u32),
    Tertiary(u32),
}

/// Table and record index for a record id (0x9A420). `level` is the game level word at `0x79094C`.
///
/// Ids -1 map to no record (`None`). The primary table is indexed by the raw id with no bound,
/// matching the original.
pub fn select_record(id: i32, level: u32) -> Option<RecordSlot> {
    if id == -1 {
        return None;
    }
    let in_secondary = (SECONDARY_ID_BASE..SECONDARY_ID_END).contains(&id)
        || (level == EXTENDED_SECONDARY_LEVEL
            && (SECONDARY_ID_BASE..SECONDARY_ID_END_EXTENDED).contains(&id));
    if in_secondary {
        return Some(RecordSlot::Secondary(
            id.wrapping_sub(SECONDARY_ID_BASE) as u32
        ));
    }
    if (TERTIARY_ID_BASE..TERTIARY_ID_END).contains(&id) {
        return Some(RecordSlot::Tertiary(
            id.wrapping_sub(TERTIARY_ID_BASE) as u32
        ));
    }
    Some(RecordSlot::Primary(id as u32))
}

/// Address of record `index` in a table starting at `base`.
pub fn record_address(base: u32, index: u32) -> u32 {
    base.wrapping_add(index.wrapping_mul(RECORD_STRIDE))
}

/// Id-to-index lookup of 0x9CC40: keys below [`PRIMARY_KEY_LIMIT`] search the primary table and
/// return its index, larger keys search the secondary table and return `index + 0x1F40`.
/// `None` is the original's `-1`.
pub fn key_to_record_id(
    key: i32,
    primary_count: i32,
    primary_key_at: impl FnMut(i32) -> i32,
    secondary_count: i32,
    secondary_key_at: impl FnMut(i32) -> i32,
) -> Option<i32> {
    if key < PRIMARY_KEY_LIMIT {
        binary_search_key(primary_count, key, primary_key_at)
    } else {
        binary_search_key(secondary_count, key, secondary_key_at)
            .map(|index| index.wrapping_add(SECONDARY_ID_BASE))
    }
}

/// Raw predicate used by original helper `0x59D20`: true when either selected
/// dword equals `wanted`. REA `ev_8f6ef09b` measures the source fields at
/// `+0xD4` and `+0x12C` of the record pointer loaded from `*(arg1 + 4)`; the
/// semantic meaning of those fields and the pointer remains unresolved.
pub fn owner_has_either_slot(slot_a: u32, slot_b: u32, wanted: u32) -> bool {
    slot_a == wanted || slot_b == wanted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_present_and_misses_absent() {
        let keys = [1, 4, 9, 16, 25, 36, 49];
        let at = |i: i32| keys[i as usize];
        for (i, k) in keys.iter().enumerate() {
            // the original's probe never tests index 0 of an odd table before count/2, but exact keys are found
            assert_eq!(binary_search_key(keys.len() as i32, *k, at), Some(i as i32));
        }
        assert_eq!(binary_search_key(keys.len() as i32, 5, at), None);
        assert_eq!(binary_search_key(0, 5, at), None);
    }

    #[test]
    fn record_selection_bands() {
        assert_eq!(select_record(-1, 0), None);
        assert_eq!(select_record(0x1f40, 0), Some(RecordSlot::Secondary(0)));
        assert_eq!(select_record(0x209d, 0), Some(RecordSlot::Secondary(0x15d)));
        assert_eq!(select_record(0x209e, 0), Some(RecordSlot::Tertiary(0)));
        assert_eq!(select_record(0x20b2, 0), Some(RecordSlot::Primary(0x20b2)));
        assert_eq!(
            select_record(0x2000, 0x64),
            Some(RecordSlot::Secondary(0xc0))
        );
        assert_eq!(
            select_record(0x2300, 0x64),
            Some(RecordSlot::Secondary(0x3c0))
        );
        assert_eq!(select_record(0x2300, 0), Some(RecordSlot::Primary(0x2300)));
    }

    #[test]
    fn slot_test() {
        assert!(owner_has_either_slot(1, 2, 2));
        assert!(!owner_has_either_slot(1, 2, 3));
    }
}
