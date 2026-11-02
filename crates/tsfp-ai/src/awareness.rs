// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned value types and ordered source-alert processing for REA awareness.
//!
//! This module is deliberately not wired into gameplay. It models the measured
//! `0x25FAF0` source-record pass over typed state and keeps the neighboring
//! perception, target, group-mask, node-key and node-ID domains distinct.
//! Verification: Tier 1 differential for source-record processing against lifted
//! `0x25FAF0`; perception/target value boundaries are contract tests only.

use crate::navigation_scoring::{ActorId, ObjectId};
pub use crate::rea_ai_types::{GroupMask, NodeKey};
pub use crate::rea_awareness_sources::{
    AlertSource, AwarenessAlertWorld, AwarenessSourceRecord, OwnedAwarenessAlertEffects,
    ReaAwarenessSourceStore, process_awareness_sources, process_owned_awareness_sources,
};
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TargetHandle {
    pub object: ObjectId,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActorClassId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum PerceptionCode {
    Rejected = 0,
    Geometric = 1,
    Timed = 2,
    CloseRange = 3,
}

/// Retain the original EAX value if a future source revision adds a result code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerceptionResult {
    Known(PerceptionCode),
    Other(u32),
}

/// Rust-owned representation of the actor-extension cached eligibility DWORD
/// at `+0xADC`. Its global value domain is unresolved, so this type deliberately
/// keeps all 32 bits and has no implicit Boolean conversion or zero default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwarenessCache {
    raw_result: u32,
}

impl AwarenessCache {
    pub const fn from_raw(raw_result: u32) -> Self {
        Self { raw_result }
    }

    pub const fn raw_result(self) -> u32 {
        self.raw_result
    }

    pub fn store_raw_result(&mut self, raw_result: u32) {
        self.raw_result = raw_result;
    }

    /// Preserve the existing translated consumer's low-byte interpretation.
    /// This is not a claim that the stored cache is Boolean.
    pub const fn legacy_low_byte_nonzero(self) -> bool {
        self.raw_result as u8 != 0
    }

    /// Full-DWORD TEST-style interpretation used by the measured fallback.
    pub const fn full_dword_nonzero(self) -> bool {
        self.raw_result != 0
    }

    /// Apply the value written by either bounded reset writer (`0x15F8C0` or
    /// `0x2B4C70`) after its own path gates have selected that store.
    pub fn clear_at_known_reset(&mut self) {
        self.raw_result = 0;
    }
}

impl From<u32> for PerceptionResult {
    fn from(raw: u32) -> Self {
        let known = match raw {
            0 => Some(PerceptionCode::Rejected),
            1 => Some(PerceptionCode::Geometric),
            2 => Some(PerceptionCode::Timed),
            3 => Some(PerceptionCode::CloseRange),
            _ => None,
        };
        known.map_or(Self::Other(raw), Self::Known)
    }
}

/// `0x15FE00`'s early descriptor callback path admits code 1 when descriptor
/// `+0x54` bit `0x200` is clear, `+0x58` bit 0 is set, and the mode-0
/// eligibility call succeeds. `None` leaves the later geometric/timer stages
/// undecided; it does not mean rejection.
pub fn descriptor_callback_admission(
    descriptor_flags_54: u32,
    descriptor_flags_58: u8,
    eligibility_result: u32,
) -> Option<PerceptionResult> {
    (descriptor_flags_54 & 0x200 == 0 && descriptor_flags_58 & 1 != 0 && eligibility_result != 0)
        .then_some(PerceptionResult::Known(PerceptionCode::Geometric))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EligibilityOutcome {
    /// `0x15E120` returns a full DWORD; consumers differ in how they test it.
    pub raw_result: u32,
    /// Eligibility may replace the actor's target as a side effect.
    pub target: Option<TargetHandle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetReservation {
    pub actor: ActorId,
    pub class_id: ActorClassId,
    pub current_target: Option<TargetHandle>,
    pub goal_target: Option<TargetHandle>,
}

/// A secondary target carries two independent identifiers: its own serialized
/// key and the primary whose Boolean occupancy it claims. Neither is a loaded
/// object address or a target node ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PrimaryTargetId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecondaryTargetClaim {
    pub handle: TargetHandle,
    pub own_key: NodeKey,
    pub primary: PrimaryTargetId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimMutation {
    Occupancy {
        primary: PrimaryTargetId,
        claimed: bool,
    },
    CurrentSecondary(Option<TargetHandle>),
    SavedSecondary(Option<TargetHandle>),
}

/// Rust-owned claim state for the measured current/saved secondary pointers.
/// Occupancy is Boolean like the original primary flag; it is not a reference
/// count. Keeping this model separate from `TargetReservations` preserves the
/// original distinction between primary occupancy and endpoint reservations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NavigationClaimState {
    pub current: Option<SecondaryTargetClaim>,
    pub saved: Option<SecondaryTargetClaim>,
    primary_occupancy: BTreeMap<PrimaryTargetId, bool>,
    mutations: Vec<ClaimMutation>,
}

impl NavigationClaimState {
    pub fn primary_is_claimed(&self, primary: PrimaryTargetId) -> bool {
        self.primary_occupancy
            .get(&primary)
            .copied()
            .unwrap_or(false)
    }

    /// Primary records currently held by this actor. A saved handle can remain
    /// installed after its Boolean occupancy has been released, so lifecycle
    /// cleanup must use this state rather than infer ownership from pointers.
    pub fn occupied_primaries(&self) -> impl Iterator<Item = PrimaryTargetId> + '_ {
        self.primary_occupancy
            .iter()
            .filter_map(|(primary, occupied)| occupied.then_some(*primary))
    }

    pub fn mutations(&self) -> &[ClaimMutation] {
        &self.mutations
    }

    /// Acquiring a new secondary sets its referenced primary's occupancy, then
    /// installs the current pointer. It does not manufacture a second claim
    /// when a saved snapshot is later taken.
    pub fn acquire(&mut self, target: SecondaryTargetClaim) {
        self.set_occupancy(target.primary, true);
        self.current = Some(target);
        self.mutations
            .push(ClaimMutation::CurrentSecondary(Some(target.handle)));
    }

    pub fn save_current(&mut self) {
        self.saved = self.current;
        self.mutations.push(ClaimMutation::SavedSecondary(
            self.saved.map(|target| target.handle),
        ));
    }

    /// State 11 releases the saved secondary's referenced primary on its exit
    /// guard, but retains both current and saved handles for the later behavior
    /// transition. The false guard has no effects.
    pub fn state11_exit(&mut self, exit_guard: bool) {
        if exit_guard {
            if let Some(saved) = self.saved {
                self.set_occupancy(saved.primary, false);
            }
        }
    }

    /// State 12's timeout releases the saved reference before clearing current;
    /// the saved snapshot itself remains installed, as in the original stores.
    pub fn state12_timeout(&mut self, timed_out: bool) {
        if !timed_out {
            return;
        }
        if let Some(saved) = self.saved {
            self.set_occupancy(saved.primary, false);
        }
        self.current = None;
        self.mutations.push(ClaimMutation::CurrentSecondary(None));
    }

    /// Apply the bounded mode-10 cleanup after callbacks have run. The caller
    /// passes the post-callback masks because the original tests live state
    /// after message dispatch. Return order is observable: current claim/pointer
    /// cleanup precedes the optional saved cleanup.
    pub fn mode10_cleanup(&mut self, old_low_mask: u32, new_low_mask: u32) {
        if old_low_mask & 0x70000 == 0 || new_low_mask & 0x4070060 != 0 {
            return;
        }
        if let Some(current) = self.current.take() {
            self.set_occupancy(current.primary, false);
            self.mutations.push(ClaimMutation::CurrentSecondary(None));
        }
        if old_low_mask & 0x60000 != 0 {
            if let Some(saved) = self.saved.take() {
                self.set_occupancy(saved.primary, false);
                self.mutations.push(ClaimMutation::SavedSecondary(None));
            }
        }
    }

    fn set_occupancy(&mut self, primary: PrimaryTargetId, claimed: bool) {
        self.primary_occupancy.insert(primary, claimed);
        self.mutations
            .push(ClaimMutation::Occupancy { primary, claimed });
    }
}

/// Snapshot of other actors' endpoint reservations used by target selection.
#[derive(Clone, Copy, Debug)]
pub struct TargetReservations<'a> {
    pub actors: &'a [TargetReservation],
}

impl TargetReservations<'_> {
    /// The original selector rejects an endpoint already held as another same-
    /// class actor's current target or goal. It does not mutate or claim targets.
    pub fn is_reserved(
        &self,
        requester: ActorId,
        class_id: ActorClassId,
        candidate: TargetHandle,
    ) -> bool {
        self.actors.iter().any(|other| {
            other.actor != requester
                && other.class_id == class_id
                && (other.current_target == Some(candidate) || other.goal_target == Some(candidate))
        })
    }
}

/// Signed result stored at recipient extension `+0xAC8`; `-1` is the original
/// unresolved-node sentinel and remains distinct from a valid `NodeId`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlertNodeIndex(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlertRecipientState {
    pub actor: ActorId,
    pub has_target_alert_behavior: bool,
    pub flags: u32,
    pub alert_mask: GroupMask,
    pub node: AlertNodeIndex,
}

pub trait AlertRecipientEffects {
    fn notify_removed_group_mask(&mut self, actor: ActorId, removed: GroupMask);
}

/// Apply the measured `0x267FF0` target-alert recipient branch. Caller has
/// already selected the group and excluded actor. This branch requires the
/// target-alert behavior bit and an unset alert flag; it stores a nonzero
/// source-node value (including sentinel -1), sets flag `0x400`, notifies about
/// removed groups, then intersects the recipient mask.
pub fn apply_group_alert_to_recipient(
    recipient: &mut AlertRecipientState,
    group_mask: GroupMask,
    source_node: AlertNodeIndex,
    target_node: Option<AlertNodeIndex>,
    effects: &mut impl AlertRecipientEffects,
) {
    if !recipient.has_target_alert_behavior || recipient.flags & 0x400 != 0 {
        return;
    }

    recipient.node = if source_node.0 != 0 {
        source_node
    } else {
        target_node.expect("original target-node path requires a live target")
    };
    recipient.flags |= 0x400;
    let removed = GroupMask(recipient.alert_mask.0 & !group_mask.0);
    if removed.0 != 0 {
        effects.notify_removed_group_mask(recipient.actor, removed);
        recipient.alert_mask.0 &= group_mask.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perception_domain_preserves_four_codes_and_unknown_raw_values() {
        assert_eq!(
            PerceptionResult::from(0),
            PerceptionResult::Known(PerceptionCode::Rejected)
        );
        assert_eq!(
            PerceptionResult::from(1),
            PerceptionResult::Known(PerceptionCode::Geometric)
        );
        assert_eq!(
            PerceptionResult::from(2),
            PerceptionResult::Known(PerceptionCode::Timed)
        );
        assert_eq!(
            PerceptionResult::from(3),
            PerceptionResult::Known(PerceptionCode::CloseRange)
        );
        assert_eq!(
            PerceptionResult::from(0x100),
            PerceptionResult::Other(0x100)
        );
    }

    #[test]
    fn awareness_cache_preserves_full_word_and_distinct_legacy_byte_read() {
        let mut cache = AwarenessCache::from_raw(0x100);
        assert_eq!(cache.raw_result(), 0x100);
        assert!(cache.full_dword_nonzero());
        assert!(!cache.legacy_low_byte_nonzero());
        cache.clear_at_known_reset();
        assert_eq!(cache.raw_result(), 0);
        assert!(!cache.full_dword_nonzero());
        cache.store_raw_result(0x200);
        assert_eq!(cache.raw_result(), 0x200);
        assert!(cache.full_dword_nonzero());
        assert!(!cache.legacy_low_byte_nonzero());
    }

    #[test]
    fn descriptor_callback_admission_requires_all_three_measured_conditions() {
        assert_eq!(
            descriptor_callback_admission(0, 1, 1),
            Some(PerceptionResult::Known(PerceptionCode::Geometric))
        );
        assert_eq!(descriptor_callback_admission(0x200, 1, 1), None);
        assert_eq!(descriptor_callback_admission(0, 0, 1), None);
        assert_eq!(descriptor_callback_admission(0, 1, 0), None);
    }

    #[test]
    fn target_reservations_are_class_scoped_and_do_not_mutate() {
        let target = TargetHandle {
            object: ObjectId(41),
            generation: 7,
        };
        let actors = [
            TargetReservation {
                actor: ActorId(1),
                class_id: ActorClassId(9),
                current_target: None,
                goal_target: Some(target),
            },
            TargetReservation {
                actor: ActorId(2),
                class_id: ActorClassId(10),
                current_target: Some(target),
                goal_target: None,
            },
        ];
        let reservations = TargetReservations { actors: &actors };
        assert!(reservations.is_reserved(ActorId(3), ActorClassId(9), target));
        assert!(!reservations.is_reserved(ActorId(3), ActorClassId(11), target));
        assert!(!reservations.is_reserved(ActorId(1), ActorClassId(9), target));
        assert!(!reservations.is_reserved(
            ActorId(3),
            ActorClassId(9),
            TargetHandle {
                object: target.object,
                generation: 8,
            }
        ));
    }

    #[test]
    fn secondary_claim_lifecycle_keeps_snapshot_and_boolean_occupancy_distinct() {
        let target = SecondaryTargetClaim {
            handle: TargetHandle {
                object: ObjectId(41),
                generation: 3,
            },
            own_key: NodeKey(9001),
            primary: PrimaryTargetId(7),
        };
        let mut state = NavigationClaimState::default();
        state.acquire(target);
        state.save_current();
        assert!(state.primary_is_claimed(target.primary));

        state.state11_exit(false);
        assert!(state.primary_is_claimed(target.primary));
        state.state11_exit(true);
        assert!(!state.primary_is_claimed(target.primary));
        assert_eq!(state.current, Some(target));
        assert_eq!(state.saved, Some(target));

        state.acquire(target);
        state.state12_timeout(true);
        assert!(!state.primary_is_claimed(target.primary));
        assert_eq!(state.current, None);
        assert_eq!(state.saved, Some(target));
    }

    #[test]
    fn mode_cleanup_uses_post_callback_masks_and_preserves_store_order() {
        let current = SecondaryTargetClaim {
            handle: TargetHandle {
                object: ObjectId(10),
                generation: 1,
            },
            own_key: NodeKey(9002),
            primary: PrimaryTargetId(1),
        };
        let saved = SecondaryTargetClaim {
            handle: TargetHandle {
                object: ObjectId(11),
                generation: 1,
            },
            own_key: NodeKey(9003),
            primary: PrimaryTargetId(2),
        };
        let mut state = NavigationClaimState::default();
        state.acquire(current);
        state.save_current();
        state.saved = Some(saved);
        state.mutations.clear();

        state.mode10_cleanup(0x40000, 0x10000);
        assert_eq!(state.current, Some(current));
        assert_eq!(state.saved, Some(saved));
        state.mode10_cleanup(0x40000, 4);
        assert_eq!(state.current, None);
        assert_eq!(state.saved, None);
        assert_eq!(
            state.mutations(),
            &[
                ClaimMutation::Occupancy {
                    primary: current.primary,
                    claimed: false,
                },
                ClaimMutation::CurrentSecondary(None),
                ClaimMutation::Occupancy {
                    primary: saved.primary,
                    claimed: false,
                },
                ClaimMutation::SavedSecondary(None),
            ]
        );
    }
}
