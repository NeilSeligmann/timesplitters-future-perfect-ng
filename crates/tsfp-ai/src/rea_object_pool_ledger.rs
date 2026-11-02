// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed object-pool counter transitions from the bounded AI spawn dossier.
//!
//! This models only the measured counter/list transaction order. Callback
//! failure rollback and reentrant mutations remain unresolved (T-ai-spawn-cap-retirement).

/// Pool counters observed at `0x7B4FAC` (slots) and `0x7B65B8` (retired).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolLedger {
    pub slot_count: u32,
    pub retired_count: u32,
    pub capacity: u32,
}

impl PoolLedger {
    /// Original `0xA4C70` predicate: `(slots - retired + requested + 4) < capacity`.
    /// Arithmetic is wrapping because the guest operands are 32-bit unsigned.
    pub fn can_reserve(&self, requested: u32) -> bool {
        self.slot_count
            .wrapping_sub(self.retired_count)
            .wrapping_add(requested)
            .wrapping_add(4)
            < self.capacity
    }

    /// Append publication increments slots; reused slots decrement retired
    /// before the allocation wrapper returns the selected slot.
    pub fn acquire_slot(&mut self, reused: bool) {
        if reused {
            self.retired_count = self.retired_count.wrapping_sub(1);
        } else {
            self.slot_count = self.slot_count.wrapping_add(1);
        }
    }

    /// Object/list publication precedes this retired-count increment.
    pub fn publish_object(&mut self) {
        self.retired_count = self.retired_count.wrapping_add(1);
    }

    /// Unlink/list clearing precedes this retired-count decrement.
    pub fn unlink_object(&mut self) {
        self.retired_count = self.retired_count.wrapping_sub(1);
    }

    pub fn reset(&mut self, capacity: u32) {
        *self = Self {
            capacity,
            ..Self::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Bounded lifted-body arithmetic reference for `0xA4C70`; no claim of
    // whole-spawn equivalence. Tier 1 evidence is weak synthetic input.
    fn lifted_reserve(slots: u32, retired: u32, requested: u32, capacity: u32) -> bool {
        slots
            .wrapping_sub(retired)
            .wrapping_add(requested)
            .wrapping_add(4)
            < capacity
    }

    #[test]
    fn tier1_reservation_differential_weak_synthetic_inputs() {
        let mut seed = 0x51f0_2a91u32;
        for _ in 0..4096 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let slots = seed;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let retired = seed;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let requested = seed & 0x3ff;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let capacity = seed;
            let ledger = PoolLedger {
                slot_count: slots,
                retired_count: retired,
                capacity,
            };
            assert_eq!(
                ledger.can_reserve(requested),
                lifted_reserve(slots, retired, requested, capacity)
            );
        }
    }

    #[test]
    fn mutation_check_detects_missing_retired_adjustment() {
        let ledger = PoolLedger {
            slot_count: 8,
            retired_count: 3,
            capacity: 12,
        };
        let correct = ledger.can_reserve(0);
        let mutated = ledger.slot_count.wrapping_add(4) < ledger.capacity;
        assert_ne!(
            correct, mutated,
            "counter mutation must be killed by fixture"
        );
    }

    #[test]
    fn measured_counter_order_is_explicit_and_typed() {
        let mut pool = PoolLedger {
            slot_count: 5,
            retired_count: 2,
            capacity: 20,
        };
        pool.acquire_slot(true);
        assert_eq!((pool.slot_count, pool.retired_count), (5, 1));
        pool.publish_object();
        assert_eq!((pool.slot_count, pool.retired_count), (5, 2));
        pool.unlink_object();
        assert_eq!((pool.slot_count, pool.retired_count), (5, 1));
        pool.reset(20);
        assert_eq!(
            pool,
            PoolLedger {
                capacity: 20,
                ..PoolLedger::default()
            }
        );
    }
}
