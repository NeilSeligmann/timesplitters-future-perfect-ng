// SPDX-License-Identifier: GPL-3.0-or-later
//! T3879: the deterministic replay branch of C `lowmem_map_impl`.
//!
//! The cursor belongs to the process, not a heap or a model instance. Freeing a
//! mapping never rewinds it; only reaching the window limit wraps it. An exhausted
//! window fails closed instead of taking C's nondeterministic MAP_32BIT fallback.

use std::sync::{Mutex, OnceLock};

use tsfp_engine_memory::guest_regions::ALLOCATION_GRANULARITY;

use crate::placement::{HINT_BASE, HINT_LIMIT};

static ENABLED: OnceLock<bool> = OnceLock::new();
static CURSOR: Mutex<Cursor> = Mutex::new(Cursor::new());

pub(crate) fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var("TSFP_GUEST_PLACE").as_deref() == Ok("det"))
}

pub(crate) fn map(length: u64, fixed: impl FnMut(u64, u32) -> Option<u32>) -> Option<u32> {
    // A poisoned cursor cannot safely establish the next replay address.
    CURSOR.lock().ok()?.map(length, fixed)
}

struct Cursor {
    next: u64,
}

impl Cursor {
    const fn new() -> Self {
        Self { next: HINT_BASE }
    }

    fn map(&mut self, length: u64, mut fixed: impl FnMut(u64, u32) -> Option<u32>) -> Option<u32> {
        let granularity = u64::from(ALLOCATION_GRANULARITY);
        let span = length.checked_add(granularity - 1)? & !(granularity - 1);
        if length == 0 || span > HINT_LIMIT - HINT_BASE {
            return None;
        }
        // Match C: finish the current sweep, then wrap once to reuse freed gaps.
        for _ in 0..2 {
            while self
                .next
                .checked_add(span)
                .is_some_and(|end| end <= HINT_LIMIT)
            {
                let base = self.next as u32;
                if let Some(address) = fixed(length, base) {
                    self.next += span;
                    return Some(address);
                }
                // A collision advances one allocation unit, not the requested span.
                self.next += granularity;
            }
            self.next = HINT_BASE;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_lengths_round_to_64k_and_freed_gaps_wait_for_wrap() {
        let mut cursor = Cursor::new();
        let mut requested = Vec::new();
        for length in [1, 0x10000, 0x10001, 0x21000] {
            cursor.map(length, |actual_length, address| {
                requested.push((actual_length, address));
                Some(address)
            });
        }
        assert_eq!(
            requested,
            [
                (1, 0x40000000),
                (0x10000, 0x40010000),
                (0x10001, 0x40020000),
                (0x21000, 0x40040000),
            ]
        );
        assert_eq!(cursor.next, HINT_BASE + 0x70000);
    }

    #[test]
    fn collision_moves_one_64k_unit_even_for_a_large_mapping() {
        let mut cursor = Cursor::new();
        let mut attempts = Vec::new();
        let result = cursor.map(0x21000, |length, address| {
            assert_eq!(length, 0x21000);
            attempts.push(address);
            (attempts.len() == 3).then_some(address)
        });
        assert_eq!(attempts, [0x40000000, 0x40010000, 0x40020000]);
        assert_eq!(result, Some(0x40020000));
        assert_eq!(cursor.next, HINT_BASE + 0x50000);
    }

    #[test]
    fn window_end_wraps_once_before_reusing_low_addresses() {
        let mut cursor = Cursor {
            next: HINT_LIMIT - 0x10000,
        };
        let mut attempts = Vec::new();
        let result = cursor.map(0x10000, |_, address| {
            attempts.push(address);
            (u64::from(address) == HINT_BASE).then_some(address)
        });
        assert_eq!(attempts, [0xefff0000, 0x40000000]);
        assert_eq!(result, Some(HINT_BASE as u32));
        assert_eq!(cursor.next, HINT_BASE + 0x10000);
    }

    #[test]
    fn rounded_span_must_fit_before_the_exact_map_is_attempted() {
        let mut cursor = Cursor {
            next: HINT_LIMIT - 0x10000,
        };
        let result = cursor.map(0x10001, |length, address| {
            assert_eq!(length, 0x10001);
            assert_eq!(u64::from(address), HINT_BASE);
            Some(address)
        });
        assert_eq!(result, Some(HINT_BASE as u32));
        assert_eq!(cursor.next, HINT_BASE + 0x20000);
    }

    #[test]
    fn exhaustion_is_bounded_and_never_uses_random_placement() {
        let mut cursor = Cursor::new();
        let mut attempts = 0;
        assert_eq!(
            cursor.map(0x10000, |_, _| {
                attempts += 1;
                None
            }),
            None
        );
        assert_eq!(attempts, 2 * (HINT_LIMIT - HINT_BASE) / 0x10000);
        assert_eq!(cursor.next, HINT_BASE);
        for length in [0, u64::MAX, HINT_LIMIT - HINT_BASE + 1] {
            assert_eq!(
                cursor.map(length, |_, _| panic!("invalid size reached mmap")),
                None
            );
        }
    }
}
