// SPDX-License-Identifier: GPL-3.0-or-later
//! Latest typed held-button state per local player, consumed by the HUD's guest-thread bridge while it reads that
//! player's eligibility mask (moved from `tsfp-bevy` `input_router`, T3771).
use crate::SLOTS;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tsfp_input::player::PlayerInput;

static BUTTONS: [AtomicU32; SLOTS] = [const { AtomicU32::new(0) }; SLOTS];
static READY: AtomicBool = AtomicBool::new(false);

/// Held buttons of `player`, `None` before the first publication.
pub fn message_buttons(player: usize) -> Option<u32> {
    if !READY.load(Ordering::Acquire) {
        return None;
    }
    BUTTONS
        .get(player)
        .map(|buttons| buttons.load(Ordering::Acquire))
}

/// Held buttons of every slot, 0 for a slot without input.
pub fn snapshot(inputs: &[Option<PlayerInput>; SLOTS]) -> [u32; SLOTS] {
    std::array::from_fn(|player| inputs[player].map_or(0, |input| input.held))
}

pub fn publish(inputs: &[Option<PlayerInput>; SLOTS]) {
    for (buttons, held) in BUTTONS.iter().zip(snapshot(inputs)) {
        buttons.store(held, Ordering::Release);
    }
    READY.store(true, Ordering::Release);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tsfp_input::player::{AssistMask, Look, MenuIntent, SourceKind, Vec2, bits};

    pub(crate) fn input(slot: u8, held: u32) -> PlayerInput {
        PlayerInput {
            step: 1,
            slot,
            source: SourceKind::Replay,
            held,
            pressed: 0,
            released: 0,
            move_axis: Vec2::new(0.0, 0.0),
            look: Look::Stick(Vec2::new(0.0, 0.0)),
            triggers: [0.0; 2],
            menu: MenuIntent {
                accept: false,
                back: false,
            },
            assists: AssistMask::NONE,
        }
    }

    #[test]
    fn snapshot_uses_each_router_slot_and_clears_missing_slots() {
        let inputs = [None, Some(input(1, bits::A | bits::START)), None, None];
        assert_eq!(snapshot(&inputs), [0, bits::A | bits::START, 0, 0]);
    }

    #[test]
    fn publication_is_visible_per_player() {
        publish(&[Some(input(0, bits::B)), None, None, None]);
        assert_eq!(message_buttons(0), Some(bits::B));
        assert_eq!(message_buttons(1), Some(0));
        assert_eq!(message_buttons(7), None);
    }
}
