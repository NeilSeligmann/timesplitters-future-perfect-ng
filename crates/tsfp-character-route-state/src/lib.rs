// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed access to address-labelled route-selection state. Opaque word identity
//! domains remain unknown; elapsed-time updates accept the caller's scaled delta.

use tsfp_state::character::{CharacterIndexWord, CharacterObjectReference};
use tsfp_state::{GuestMemory, character::CharacterSet};

const NEXT_SELECTION_WORD_INDEX: usize = 0x680 / 4;
const GOAL_WORD_INDEX: usize = 0x684 / 4;
const ROUTE_RECORD_WORD_INDEX: usize = 0x678 / 4;
const REVERSE_TRACE_WORD_INDICES: [usize; 2] = [0x9a8 / 4, 0x9ac / 4];
const ROUTE_PROGRAM_ELAPSED_INDEX: usize = 0xb30 / 4;
const ACTOR_RETIREMENT_ZERO_WORDS: [usize; 8] = [0x00, 0x04, 0x10, 0x14, 0x18, 0x1c, 0x9c0, 0xbf0];
const ACTOR_RETIREMENT_FINAL_ZERO_WORD: usize = 0xbf4;

/// Controller-entry boundary for pending state requests. Requests stamped
/// during one invocation become eligible only at a later invocation. This
/// local token does not model global ticks, scheduler order, or callbacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PendingStateInvocation(u64);

impl PendingStateInvocation {
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    pub fn accepts_request_written_after(self, request_written_at: Self) -> bool {
        request_written_at.0 < self.0
    }
}

/// Measured actor-extension back-links established by constructor `0xDAC30`.
/// The CharacterSet storage row and constructor-supplied actor slot are kept
/// separate because REA has not established that their index domains match.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActorExtensionBinding {
    pub object: CharacterObjectReference,
    pub actor_slot: u16,
}

/// Opaque route-record word at actor extension offset `+0x678`.
///
/// `0x27CC20` stores the result of its `0x9BCF0` lookup here. The route
/// contract identifies `+0x678` as route state, but not the word's identity
/// domain or lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RouteRecordWord {
    pub raw: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NextRouteSelectionWord {
    pub raw: u32,
}

/// Opaque route-goal word observed at actor extension offset `+0x684`.
/// This wrapper makes no claim about node identity or graph lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RouteGoalWord {
    pub raw: u32,
}

/// Elapsed seconds accumulated at actor extension offset `+0xB30`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RouteProgramElapsed(pub f32);

/// Extension methods live in a leaf crate because `tsfp-state` is size-frozen.
pub trait CharacterRouteSelectionState {
    /// Mirrors the measured constructor writes `E+0 = O` and `E+8 = slot`.
    /// This binds two words only; it does not allocate an Object, publish the
    /// actor to registries, mint a native generation, or define rollback.
    fn bind_actor_extension<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        character_row: usize,
        binding: ActorExtensionBinding,
    ) -> bool;

    /// Applies only the unconditional selected words observed in `0x15DDA0`.
    /// It does not unlink membership, invoke cleanup callbacks, release the
    /// conditional `+0xC08` reference, clear Object state, or define lifetime.
    fn clear_selected_actor_retirement_fields<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        character_row: usize,
    ) -> bool;

    fn route_record_word(&self, slot: usize) -> Option<RouteRecordWord>;

    fn next_route_selection_word(&self, slot: usize) -> Option<NextRouteSelectionWord>;

    fn route_goal_word(&self, slot: usize) -> Option<RouteGoalWord>;

    fn set_next_route_selection_word<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        word: NextRouteSelectionWord,
    ) -> bool;

    fn set_route_record_word<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        word: RouteRecordWord,
    ) -> bool;

    fn set_route_goal_word<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        word: RouteGoalWord,
    ) -> bool;

    /// Commits the bounded state writes of route refresh `0x159E50` after
    /// its caller has selected the current node. A changed current node clears
    /// reverse-trace counts; the optional next-node result updates `+0x680`.
    /// Candidate enumeration and direction-query selection remain outside.
    fn commit_route_refresh<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        current: RouteRecordWord,
        next: Option<NextRouteSelectionWord>,
    ) -> bool;

    fn route_program_elapsed(&self, slot: usize) -> Option<RouteProgramElapsed>;

    /// Adds the already scaled actor update delta to `+0xB30` and mirrors it.
    fn advance_route_program_elapsed<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        scaled_delta_seconds: f32,
    ) -> Option<RouteProgramElapsed>;
}

impl CharacterRouteSelectionState for CharacterSet {
    fn clear_selected_actor_retirement_fields<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        character_row: usize,
    ) -> bool {
        let Some(mut body) = self.body_state(character_row) else {
            return false;
        };
        for offset in ACTOR_RETIREMENT_ZERO_WORDS {
            body.words[offset / 4] = 0;
        }
        body.words[ROUTE_RECORD_WORD_INDEX] = u32::MAX;
        body.words[ACTOR_RETIREMENT_FINAL_ZERO_WORD / 4] = 0;
        self.set_body_state(memory, character_row, body)
    }

    fn bind_actor_extension<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        character_row: usize,
        binding: ActorExtensionBinding,
    ) -> bool {
        if self.body_state(character_row).is_none() {
            return false;
        }
        let object_written = self.set_object_reference(memory, character_row, binding.object);
        let slot_written = self.set_index_word(
            memory,
            character_row,
            CharacterIndexWord {
                raw: i32::from(binding.actor_slot),
            },
        );
        debug_assert!(object_written && slot_written);
        object_written && slot_written
    }

    fn route_record_word(&self, slot: usize) -> Option<RouteRecordWord> {
        let body = self.body_state(slot)?;
        Some(RouteRecordWord {
            raw: body.words[ROUTE_RECORD_WORD_INDEX],
        })
    }

    fn next_route_selection_word(&self, slot: usize) -> Option<NextRouteSelectionWord> {
        let body = self.body_state(slot)?;
        Some(NextRouteSelectionWord {
            raw: body.words[NEXT_SELECTION_WORD_INDEX],
        })
    }

    fn route_goal_word(&self, slot: usize) -> Option<RouteGoalWord> {
        let body = self.body_state(slot)?;
        Some(RouteGoalWord {
            raw: body.words[GOAL_WORD_INDEX],
        })
    }

    fn set_next_route_selection_word<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        word: NextRouteSelectionWord,
    ) -> bool {
        let Some(mut body) = self.body_state(slot) else {
            return false;
        };
        body.words[NEXT_SELECTION_WORD_INDEX] = word.raw;
        self.set_body_state(memory, slot, body)
    }

    fn set_route_record_word<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        word: RouteRecordWord,
    ) -> bool {
        let Some(mut body) = self.body_state(slot) else {
            return false;
        };
        body.words[ROUTE_RECORD_WORD_INDEX] = word.raw;
        self.set_body_state(memory, slot, body)
    }

    fn set_route_goal_word<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        word: RouteGoalWord,
    ) -> bool {
        let Some(mut body) = self.body_state(slot) else {
            return false;
        };
        body.words[GOAL_WORD_INDEX] = word.raw;
        self.set_body_state(memory, slot, body)
    }

    fn commit_route_refresh<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        current: RouteRecordWord,
        next: Option<NextRouteSelectionWord>,
    ) -> bool {
        let Some(mut body) = self.body_state(slot) else {
            return false;
        };
        let current_changed = body.words[ROUTE_RECORD_WORD_INDEX] != current.raw;
        body.words[ROUTE_RECORD_WORD_INDEX] = current.raw;
        if current_changed {
            for index in REVERSE_TRACE_WORD_INDICES {
                body.words[index] = 0;
            }
        }
        if let Some(next) = next {
            body.words[NEXT_SELECTION_WORD_INDEX] = next.raw;
        }
        self.set_body_state(memory, slot, body)
    }

    fn route_program_elapsed(&self, slot: usize) -> Option<RouteProgramElapsed> {
        let body = self.body_state(slot)?;
        Some(RouteProgramElapsed(f32::from_bits(
            body.words[ROUTE_PROGRAM_ELAPSED_INDEX],
        )))
    }

    fn advance_route_program_elapsed<M: GuestMemory>(
        &mut self,
        memory: &mut M,
        slot: usize,
        scaled_delta_seconds: f32,
    ) -> Option<RouteProgramElapsed> {
        let mut body = self.body_state(slot)?;
        let elapsed = f32::from_bits(body.words[ROUTE_PROGRAM_ELAPSED_INDEX]);
        let next = RouteProgramElapsed(elapsed + scaled_delta_seconds);
        body.words[ROUTE_PROGRAM_ELAPSED_INDEX] = next.0.to_bits();
        self.set_body_state(memory, slot, body).then_some(next)
    }
}
