//! Rust-owned raw state for the observed 0x453D0 / 0x45410 request-slot path.
//!
//! The original record's field meanings are unknown. This model preserves the measured
//! slot search and writes without assigning semantic names to those dwords.

pub const REQUEST_POOL_SLOTS: usize = 25;
pub const REQUEST_SLOT_DWORDS: usize = 16;
pub const REQUEST_SLOT_STRIDE: usize = 0x40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RawRequestSlot {
    pub dwords: [u32; REQUEST_SLOT_DWORDS],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawRequestPool {
    pub slots: [RawRequestSlot; REQUEST_POOL_SLOTS],
    /// Last cursor value written at 0x6D5EA8. The original's first probe is cursor + 1.
    pub cursor: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetireRequestResult {
    NoMatch,
    Matched { delegated: bool },
    Direct,
}

impl Default for RawRequestPool {
    fn default() -> Self {
        Self {
            slots: [RawRequestSlot::default(); REQUEST_POOL_SLOTS],
            cursor: 0,
        }
    }
}

impl RawRequestPool {
    /// Model 0x453D0: scan 25 entries after the cursor, wrapping modulo 25.
    ///
    /// A slot is available when raw dword +0x3C is zero. The cursor is updated to
    /// the selected index, or to the last probed index when all slots are occupied.
    pub fn find_free_slot_453d0(&mut self) -> Option<usize> {
        let mut index = self.cursor;
        for _ in 0..REQUEST_POOL_SLOTS {
            index = (index + 1) % REQUEST_POOL_SLOTS as u32;
            if self.slots[index as usize].dwords[15] == 0 {
                self.cursor = index;
                return Some(index as usize);
            }
        }
        self.cursor = index;
        None
    }

    /// Model 0x45410's measured successful path. `input[1]` is copied to slot +0x38,
    /// then the original overwrites slot +0x04 with zero and sets raw status +0x3C=2.
    /// The returned value is the caller-provided handle (original EBX).
    pub fn insert_45410(&mut self, input: &[u32; 12], handle: u32) -> Option<u32> {
        let index = self.find_free_slot_453d0()?;
        let slot = &mut self.slots[index].dwords;
        slot[..12].copy_from_slice(input);
        slot[1] = 0;
        slot[15] = 2;
        // The observed store at +0x30 is dword 12 (not part of the copied 12 dwords).
        // Keep +0x30 as the opaque caller handle and +0x38 as the copied input +4.
        slot[12] = handle;
        slot[14] = input[1];
        Some(handle)
    }

    /// Model 0x45460's measured bank selection and handoff to 0x273E0.
    /// The callback is the explicit boundary for 0x273E0; its resource semantics
    /// are not represented here. The raw mask is `0x20000000`; do not interpret it
    /// as a named ownership bit without further evidence.
    pub fn retire_45460(
        &mut self,
        handle: u32,
        bank_managed: bool,
        mut release_273e0: impl FnMut(u32),
    ) -> RetireRequestResult {
        if !bank_managed {
            release_273e0(handle);
            return RetireRequestResult::Direct;
        }

        for slot in &mut self.slots {
            if slot.dwords[12] != handle {
                continue;
            }
            let delegated = slot.dwords[15] == 4;
            if delegated {
                release_273e0(handle);
            }
            slot.dwords[15] = 1;
            return RetireRequestResult::Matched { delegated };
        }
        RetireRequestResult::NoMatch
    }
}

#[cfg(test)]
mod tests {
    use super::{REQUEST_POOL_SLOTS, RawRequestPool, RetireRequestResult};

    #[test]
    fn search_starts_after_cursor_wraps_and_updates_cursor() {
        let mut pool = RawRequestPool::default();
        assert_eq!(pool.find_free_slot_453d0(), Some(1));
        pool.slots[2].dwords[15] = 2;
        assert_eq!(pool.find_free_slot_453d0(), Some(3));
    }

    #[test]
    fn full_pool_returns_none_and_keeps_last_probe_index() {
        let mut pool = RawRequestPool::default();
        for slot in &mut pool.slots {
            slot.dwords[15] = 1;
        }
        assert_eq!(pool.find_free_slot_453d0(), None);
        assert_eq!(pool.cursor, 0);
    }

    #[test]
    fn insertion_copies_twelve_words_then_applies_measured_overwrites() {
        let mut pool = RawRequestPool::default();
        let input = core::array::from_fn(|i| 0x1000 + i as u32);
        assert_eq!(pool.insert_45410(&input, 0xfeed_beef), Some(0xfeed_beef));

        let slot = &pool.slots[1].dwords;
        assert_eq!(slot[0], input[0]);
        assert_eq!(slot[1], 0);
        assert_eq!(slot[11], input[11]);
        assert_eq!(slot[12], 0xfeed_beef);
        assert_eq!(slot[14], input[1]);
        assert_eq!(slot[15], 2);
        assert_eq!(pool.slots.len(), REQUEST_POOL_SLOTS);
    }

    #[test]
    fn bank_managed_retire_delegates_only_state_four_then_sets_state_one() {
        let mut pool = RawRequestPool::default();
        pool.slots[4].dwords[12] = 0x1234;
        pool.slots[4].dwords[15] = 4;
        let mut released = Vec::new();

        assert_eq!(
            pool.retire_45460(0x1234, true, |handle| released.push(handle)),
            RetireRequestResult::Matched { delegated: true }
        );
        assert_eq!(released, [0x1234]);
        assert_eq!(pool.slots[4].dwords[15], 1);
    }

    #[test]
    fn bank_managed_non_four_state_is_marked_without_delegation() {
        let mut pool = RawRequestPool::default();
        pool.slots[0].dwords[12] = 9;
        pool.slots[0].dwords[15] = 2;
        let mut delegated = false;

        assert_eq!(
            pool.retire_45460(9, true, |_| delegated = true),
            RetireRequestResult::Matched { delegated: false }
        );
        assert!(!delegated);
        assert_eq!(pool.slots[0].dwords[15], 1);
    }

    #[test]
    fn untracked_retire_delegates_directly_but_managed_miss_does_not() {
        let mut pool = RawRequestPool::default();
        let mut released = Vec::new();
        assert_eq!(
            pool.retire_45460(77, false, |handle| released.push(handle)),
            RetireRequestResult::Direct
        );
        assert_eq!(
            pool.retire_45460(88, true, |handle| released.push(handle)),
            RetireRequestResult::NoMatch
        );
        assert_eq!(released, [77]);
    }
}
