// SPDX-License-Identifier: GPL-3.0-or-later
//! The replay side of the player control lock (docs/t-replay-desync-l3-l4.md, "Control lock").
//!
//! The game freezes the player for a stretch that depends on wall time (intro, cutscene), and the owner keeps moving
//! the mouse meanwhile: the game ignored those look and move inputs. A replay delivers the `T` rows by recorded poll
//! while its own freeze ends at another tick, so rows recorded under the lock must not reach the player and rows
//! recorded after the owner's release must wait for the replay's own release.
//!
//! `recorded_locked` is the recorded `control` decision in force at the row, `replay_locked` the replay's own lock
//! ([`crate::state_digest::control_locked`]). Buttons always pass through (a press that skips a cutscene is not
//! ignored by the game), look and move are the gated part.

use crate::player::{Look, PlayerInput, Vec2};
use std::collections::VecDeque;

/// Rows held back at most (about 17 s of ticks): a lock that never releases must not grow without bound.
const MAX_BACKLOG: usize = 1024;

#[derive(Debug, Default)]
pub struct ControlLock {
    /// Look and move of rows recorded after the owner's release, delivered after the replay's release.
    backlog: VecDeque<PlayerInput>,
}

fn without_look_and_move(mut input: PlayerInput) -> PlayerInput {
    input.move_axis = Vec2::default();
    input.look = match input.look {
        Look::Stick(_) => Look::Stick(Vec2::default()),
        Look::Delta { .. } => Look::Delta {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
        },
    };
    input
}

fn with_look_and_move(mut input: PlayerInput, from: PlayerInput) -> PlayerInput {
    input.move_axis = from.move_axis;
    input.look = from.look;
    input
}

impl ControlLock {
    /// Rows still waiting for the replay's release.
    pub fn backlog(&self) -> usize {
        self.backlog.len()
    }

    /// The input the replay's player gets for one recorded row.
    pub fn apply(
        &mut self,
        input: PlayerInput,
        recorded_locked: bool,
        replay_locked: bool,
    ) -> PlayerInput {
        let neutral = without_look_and_move(input);
        if recorded_locked {
            // The owner's game dropped this look and move. A replay that is free while rows are still owed to it
            // (it released later than the owner) takes them here, in the place of the dropped ones.
            return match (replay_locked, self.backlog.front()) {
                (false, Some(&owed)) => {
                    self.backlog.pop_front();
                    with_look_and_move(neutral, owed)
                }
                _ => neutral,
            };
        }
        if !replay_locked && self.backlog.is_empty() {
            return input;
        }
        if self.backlog.len() < MAX_BACKLOG {
            self.backlog.push_back(input);
        }
        if replay_locked {
            return neutral;
        }
        match self.backlog.pop_front() {
            Some(owed) => with_look_and_move(neutral, owed),
            None => neutral,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{AssistMask, MenuIntent, SourceKind};

    fn row(step: u64, yaw_deg: f32, forward: f32) -> PlayerInput {
        PlayerInput {
            step,
            slot: 0,
            source: SourceKind::MouseKeyboard,
            held: 0,
            pressed: 0,
            released: 0,
            move_axis: Vec2 { x: 0.0, y: forward },
            look: Look::Delta {
                yaw_deg,
                pitch_deg: 0.0,
            },
            triggers: [0.0; 2],
            menu: MenuIntent::default(),
            assists: AssistMask::default(),
        }
    }

    fn yaw(input: &PlayerInput) -> f32 {
        match input.look {
            Look::Delta { yaw_deg, .. } => yaw_deg,
            Look::Stick(_) => panic!("delta look expected"),
        }
    }

    #[test]
    fn unlocked_rows_pass_through_untouched() {
        let mut lock = ControlLock::default();
        let input = row(1, 3.0, 1.0);
        assert_eq!(lock.apply(input, false, false), input);
        assert_eq!(lock.backlog(), 0);
    }

    #[test]
    fn look_recorded_during_the_lock_is_not_applied_after_the_replay_releases_early() {
        let mut lock = ControlLock::default();
        // The owner moved the mouse while locked; the replay's freeze is already over.
        let out = lock.apply(row(1, 5.0, 1.0), true, false);
        assert_eq!(yaw(&out), 0.0);
        assert_eq!(out.move_axis, Vec2::default());
        assert_eq!(lock.backlog(), 0);
    }

    #[test]
    fn post_release_rows_wait_for_the_replays_own_release_and_keep_their_order() {
        let mut lock = ControlLock::default();
        // The owner released at step 3, the replay is locked until step 5.
        for step in 3..5 {
            let out = lock.apply(row(step, step as f32, 1.0), false, true);
            assert_eq!(yaw(&out), 0.0, "step {step} held back");
        }
        assert_eq!(lock.backlog(), 2);
        // Released: the held rows come out first, each behind the current one.
        let first = lock.apply(row(5, 5.0, 1.0), false, false);
        let second = lock.apply(row(6, 6.0, 1.0), false, false);
        assert_eq!((yaw(&first), yaw(&second)), (3.0, 4.0));
        assert_eq!(
            lock.backlog(),
            2,
            "the delay stays until a recorded lock pays it back"
        );
        // The next recorded lock pays the delay back with the owed rows instead of dropping them.
        let third = lock.apply(row(7, 9.0, 1.0), true, false);
        assert_eq!(yaw(&third), 5.0);
    }

    #[test]
    fn buttons_and_row_identity_survive_the_gate() {
        let mut lock = ControlLock::default();
        let mut input = row(9, 2.0, 1.0);
        input.held = 0x80;
        input.pressed = 0x80;
        let out = lock.apply(input, true, true);
        assert_eq!((out.held, out.pressed, out.step), (0x80, 0x80, 9));
        assert_eq!(yaw(&out), 0.0);
    }

    #[test]
    fn a_lock_that_never_releases_keeps_the_backlog_bounded() {
        let mut lock = ControlLock::default();
        for step in 0..(MAX_BACKLOG as u64 + 50) {
            lock.apply(row(step, 1.0, 1.0), false, true);
        }
        assert_eq!(lock.backlog(), MAX_BACKLOG);
    }
}
