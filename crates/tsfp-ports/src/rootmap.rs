// SPDX-License-Identifier: GPL-3.0-or-later
//! T2143: constant-time root lookup. The dispatcher used to find a root VA with `iter().position` over the 782 root table
//! (and a second scan over the 368 weapon and player roots), on every call of every selected or unselected root. A
//! hash table built at compile time replaces those scans: open addressing, linear probing, `CAP` a power of two with at
//! least twice as many slots as roots. The first occurrence of a duplicate VA wins, as `position` did.
//! `no_std`, no allocation.

pub struct RootMap<const CAP: usize> {
    keys: [u32; CAP],
    /// Index into the root table plus one; zero marks an empty slot.
    slots: [u16; CAP],
}

#[inline(always)]
const fn hash(va: u32) -> usize {
    (va.wrapping_mul(0x9E37_79B1) >> 15) as usize
}

impl<const CAP: usize> RootMap<CAP> {
    pub const fn build(roots: &[u32]) -> Self {
        assert!(CAP.is_power_of_two() && roots.len() * 2 <= CAP && roots.len() < u16::MAX as usize);
        let mut keys = [0u32; CAP];
        let mut slots = [0u16; CAP];
        let mut index = 0;
        while index < roots.len() {
            let va = roots[index];
            let mut at = hash(va) & (CAP - 1);
            loop {
                if slots[at] == 0 {
                    keys[at] = va;
                    slots[at] = index as u16 + 1;
                    break;
                }
                if keys[at] == va {
                    break;
                }
                at = (at + 1) & (CAP - 1);
            }
            index += 1;
        }
        Self { keys, slots }
    }

    /// Index of `va` in the table the map was built from.
    #[inline(always)]
    pub fn find(&self, va: u32) -> Option<usize> {
        let mut at = hash(va) & (CAP - 1);
        loop {
            let slot = self.slots[at];
            if slot == 0 {
                return None;
            }
            if self.keys[at] == va {
                return Some(slot as usize - 1);
            }
            at = (at + 1) & (CAP - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_every_root_and_rejects_others() {
        const ROOTS: [u32; 6] = [0x1_1111, 0x2_2222, 0x3_3333, 0x1_1111, 0x4_4444, 0x1];
        static MAP: RootMap<16> = RootMap::build(&ROOTS);
        assert_eq!(MAP.find(0x1_1111), Some(0), "first duplicate wins");
        assert_eq!(MAP.find(0x2_2222), Some(1));
        assert_eq!(MAP.find(0x3_3333), Some(2));
        assert_eq!(MAP.find(0x4_4444), Some(4));
        assert_eq!(MAP.find(0x1), Some(5));
        assert_eq!(MAP.find(0x2), None);
        assert_eq!(MAP.find(0), None);
    }
}
