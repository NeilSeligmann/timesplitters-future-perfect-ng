// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared typed weapon-cycle selection from the measured `0x1B6BF0` contract.

pub const WEAPON_CYCLE_SLOTS: usize = 70;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WeaponCycleEntry {
    pub id: u32,
    pub selection_flag_0x10: bool,
    /// Adapter input for the compatibility selector. Rust-owned callers replace this from state.
    pub owned: bool,
}

/// Select the first owned, enabled entry after `previous_entry`, wrapping once.
/// If the previous id is absent, scanning starts from slot one and the final
/// fallback is order slot zero, matching the original's zero-initialized index.
/// If no entry qualifies, return the previous slot's id (or order slot zero when
/// the previous id is absent), as the original loop does on its wrap guard.
pub fn next_selectable(
    entries: &[WeaponCycleEntry; WEAPON_CYCLE_SLOTS],
    previous_entry: u32,
) -> u32 {
    let start = entries
        .iter()
        .position(|entry| entry.id == previous_entry)
        .unwrap_or(0);

    let mut slot = start;
    loop {
        slot = (slot + 1) % WEAPON_CYCLE_SLOTS;
        let entry = entries[slot];
        if entry.selection_flag_0x10 && entry.owned {
            return entry.id;
        }
        if slot == start {
            return entries[start].id;
        }
    }
}
