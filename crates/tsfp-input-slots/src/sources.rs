// SPDX-License-Identifier: GPL-3.0-or-later
//! The device behind each router slot, for native pages that answer each player on its own controller port (T3772).
//!
//! Router slot n is guest controller port n (T3771). While a Rust page owns input the pad presses never reach the router
//! (they are held back from the guest), so a page that must tell the players apart (the profile selection page of a
//! two player game, `docs/t3772-profile-page-ports.md`) maps each device to its slot here: the router publishes its slot
//! owners every tick, the page input reads [`SlotSources::pad_slot`] and [`SlotSources::keyboard_slot`].
use crate::SLOTS;
use std::sync::Mutex;
use tsfp_input::router::{InputSource, PadId};

/// The device owning each router slot (`None`: the slot is free).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotSources(pub [Option<InputSource>; SLOTS]);

impl SlotSources {
    /// The slot of pad `pad` (the Bevy gamepad entity index the router uses). A pad the router has not placed yet is
    /// slot 0's, the single player behaviour before T3772.
    pub fn pad_slot(&self, pad: PadId) -> usize {
        self.slot_of(InputSource::Pad(pad))
    }

    /// The slot of the mouse and keyboard player (slot 0 when the router has not placed it).
    pub fn keyboard_slot(&self) -> usize {
        self.slot_of(InputSource::MouseKeyboard)
    }

    fn slot_of(&self, source: InputSource) -> usize {
        self.0
            .iter()
            .position(|owner| *owner == Some(source))
            .unwrap_or(0)
    }
}

static SOURCES: Mutex<SlotSources> = Mutex::new(SlotSources([None; SLOTS]));

/// Router tick: the current owner of every slot.
pub fn publish(owners: [Option<InputSource>; SLOTS]) {
    *SOURCES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = SlotSources(owners);
}

/// The last published slot owners.
pub fn current() -> SlotSources {
    *SOURCES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_and_keyboard_resolve_to_their_router_slot() {
        let sources = SlotSources([
            Some(InputSource::MouseKeyboard),
            Some(InputSource::Pad(11)),
            None,
            None,
        ]);
        assert_eq!(sources.keyboard_slot(), 0);
        assert_eq!(sources.pad_slot(11), 1);
        // A pad the router has not placed stays player 1's, as before T3772.
        assert_eq!(sources.pad_slot(6), 0);
        let swapped = SlotSources([
            Some(InputSource::Pad(6)),
            Some(InputSource::MouseKeyboard),
            None,
            None,
        ]);
        assert_eq!(swapped.keyboard_slot(), 1);
        assert_eq!(swapped.pad_slot(6), 0);
    }

    #[test]
    fn publish_and_read_back() {
        let owners = [None, Some(InputSource::Pad(3)), None, None];
        publish(owners);
        assert_eq!(current().pad_slot(3), 1);
        publish([None; SLOTS]);
    }
}
