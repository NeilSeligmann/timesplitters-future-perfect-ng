//! Readable weapon-cycle group lookup from original `0x1B6BB0`.

pub const WEAPON_CYCLE_SLOTS: usize = 70;

/// Return the first ordered entry whose record has `wanted_group`, or `-1`.
/// `ordered_ids` is the shared cycle order; `groups_by_slot` is a matching
/// snapshot of each ordered entry's record `+0x38` group dword.
pub fn first_group_entry(
    ordered_ids: &[u32; WEAPON_CYCLE_SLOTS],
    groups_by_slot: &[u32; WEAPON_CYCLE_SLOTS],
    wanted_group: u32,
) -> i32 {
    for (id, group) in ordered_ids.iter().zip(groups_by_slot) {
        if *group == wanted_group {
            return *id as i32;
        }
    }
    -1
}
