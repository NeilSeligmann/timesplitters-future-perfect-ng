// SPDX-License-Identifier: GPL-3.0-or-later
//! Direct weapon slot requests per local slot (0 = none, 1..=9), written by the router tick, read by the game side lane
//! (T2179 design, docs/t-kbm-keymap.md). The latest request wins until taken. Moved from `tsfp-bevy` (T3771).
use crate::SLOTS;
use std::sync::atomic::{AtomicU8, Ordering};

static REQUESTS: [AtomicU8; SLOTS] = [const { AtomicU8::new(0) }; SLOTS];

/// Store the latest request of a slot; out of range slots are ignored.
pub fn store(slot: usize, number: u8) {
    if let Some(cell) = REQUESTS.get(slot) {
        cell.store(number, Ordering::Release);
    }
}

/// Take (and clear) the pending weapon slot request of a local slot.
pub fn take(slot: u8) -> Option<u8> {
    let cell = REQUESTS.get(usize::from(slot))?;
    Some(cell.swap(0, Ordering::AcqRel)).filter(|n| *n != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailbox_is_taken_once() {
        store(2, 5);
        assert_eq!(take(2), Some(5));
        assert_eq!(take(2), None);
        assert_eq!(take(9), None);
        store(9, 1);
    }
}
