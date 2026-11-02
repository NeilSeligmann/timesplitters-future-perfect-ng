//! Original weapon-entry cycling used by the player inventory selector.
//! The guest adapter supplies the 70 ordered entries and their typed gates.

pub const ORDER_LEN: usize = 70;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryState {
    pub entry: u32,
    pub has_flag_0x10: bool,
    pub owned: bool,
}

/// Return the first eligible entry after `previous`, wrapping in source order.
/// If no entry is eligible, return the previous entry's table value (or entry 0
/// when the previous value is absent), matching the original unchanged result.
pub fn next_entry(entries: &[EntryState; ORDER_LEN], previous: u32) -> u32 {
    let start = entries
        .iter()
        .position(|e| e.entry == previous)
        .unwrap_or(0);
    for distance in 1..=ORDER_LEN {
        let index = (start + distance) % ORDER_LEN;
        let candidate = entries[index];
        if candidate.has_flag_0x10 && candidate.owned {
            return candidate.entry;
        }
    }
    entries[start].entry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(entries: &[EntryState; ORDER_LEN], previous: u32) -> u32 {
        let mut start = 0;
        for (i, entry) in entries.iter().enumerate() {
            if entry.entry == previous {
                start = i;
                break;
            }
        }
        let mut i = start;
        loop {
            i += 1;
            if i == ORDER_LEN {
                i = 0;
            }
            let item = entries[i];
            if item.has_flag_0x10 && item.owned {
                return item.entry;
            }
            if i == start {
                return entries[start].entry;
            }
        }
    }

    #[test]
    fn tier1_weak_differential_uses_independent_loop_reference() {
        let mut seed = 0xA511_E9B3u32;
        for _ in 0..4000 {
            let mut entries = [EntryState {
                entry: 0,
                has_flag_0x10: false,
                owned: false,
            }; ORDER_LEN];
            for (i, entry) in entries.iter_mut().enumerate() {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                entry.entry = 1000 + i as u32;
                entry.has_flag_0x10 = seed & 1 != 0;
                entry.owned = seed & 2 != 0;
            }
            let previous = match seed % 3 {
                0 => entries[(seed as usize >> 4) % ORDER_LEN].entry,
                1 => u32::MAX,
                _ => 1000,
            };
            assert_eq!(
                next_entry(&entries, previous),
                reference(&entries, previous)
            );
        }
    }

    #[test]
    fn mutation_checks_cover_gates_wrap_and_full_cycle() {
        let mut entries = [EntryState {
            entry: 0,
            has_flag_0x10: false,
            owned: false,
        }; ORDER_LEN];
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.entry = i as u32;
        }
        entries[0].has_flag_0x10 = true;
        entries[0].owned = true;
        entries[69].has_flag_0x10 = true;
        entries[69].owned = true;
        assert_eq!(next_entry(&entries, 68), 69); // eligible before wrap
        assert_eq!(next_entry(&entries, 69), 0); // wrap
        entries[0].owned = false;
        assert_eq!(next_entry(&entries, 69), 69); // no eligible candidate
        entries[0].owned = true;
        entries[0].has_flag_0x10 = false;
        assert_eq!(next_entry(&entries, 69), 69); // flag gate
    }
}
