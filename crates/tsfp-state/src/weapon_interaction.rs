// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed A0D30 interaction continuation (T1680).
//!
//! The original continuation captures the selected player's actor before sending, resolves the
//! target callback after the send returns, invokes that callback synchronously, sets the captured
//! player's interaction flag, and only then reads the target mission key. This module expresses
//! that order without guest addresses. `InteractionTargetStore` owns target callbacks and mission
//! records against `SceneMembership` generations. A runtime host still supplies player ownership,
//! send behavior, and native callback handlers; the existing reference route remains active until
//! the complete path passes replay adoption.

use alloc::{collections::BTreeMap, vec::Vec};

use crate::{
    object_pool::ObjectId,
    scene_membership::{MembershipError, SceneMembership},
    weapon_inventory::{WeaponInventoryStore, WeaponPlayerHandle, WeaponPlayerId},
};

/// Opaque Rust-owned identity for the callback stored on an interaction target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionCallbackId(pub u32);

/// Input origin carried with a native interaction intent.
///
/// This is source metadata from the typed input layer; it does not select or emulate a pad
/// button. The configured action mapping and original hold/use policy remain caller-owned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionInputSource {
    Pad(u32),
    MouseKeyboard,
    Script,
    Replay,
    Network { peer: u8 },
}

/// One native, configured interaction action state for a simulation step.
///
/// Press, held and release are preserved independently because the original use-mask timing
/// and timed-use branch are still open. Consumers decide when to submit an interaction; this
/// value does not synthesize guest input or choose a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteractionIntent {
    pub player: WeaponPlayerId,
    pub step: u64,
    pub source: InteractionInputSource,
    pub pressed: bool,
    pub held: bool,
    pub released: bool,
}

/// Maximum number of eligible objects retained by the original A0D90 candidate builder.
pub const INTERACTION_CANDIDATE_LIMIT: usize = 20;

/// One eligible, already-scored A0D90 candidate in source encounter order.
///
/// Candidate eligibility, ray/visibility checks, and the measured distance/alignment score
/// calculation belong to the neighboring world-query owner. This value only carries the
/// generation-checked object identity and completed score into the bounded rank pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoredInteractionCandidate {
    pub target: ObjectId,
    pub score: f32,
}

/// Select the first strictly lowest-scored candidate from A0D90's bounded encounter-order list.
///
/// The original rank pass keeps the current winner on equal scores and examines no more than
/// the first 20 eligible candidates. Its score formula is supplied by the candidate builder and
/// is intentionally not replaced with a distance-only approximation here.
pub fn rank_interaction_candidates(candidates: &[ScoredInteractionCandidate]) -> Option<ObjectId> {
    let mut best_score = f32::MAX;
    let mut best_target = None;
    for candidate in candidates.iter().take(INTERACTION_CANDIDATE_LIMIT) {
        if candidate.score < best_score {
            best_score = candidate.score;
            best_target = Some(candidate.target);
        }
    }
    best_target
}

/// Rank the measured candidate window and admit its winner only if that object generation is
/// still live in the Rust-owned scene.
///
/// A stale winner is reported instead of silently falling back to a lower-ranked candidate:
/// candidate collection and liveness at scan time belong to the world-query owner, and the
/// original fallback behavior is not established by the bounded A0D90 evidence.
pub fn rank_live_interaction_target(
    scene: &SceneMembership,
    candidates: &[ScoredInteractionCandidate],
) -> Result<Option<ObjectId>, MembershipError> {
    let Some(target) = rank_interaction_candidates(candidates) else {
        return Ok(None);
    };
    if scene.is_active(target) {
        Ok(Some(target))
    } else {
        Err(MembershipError::NotActive)
    }
}

/// One measured mission-marker predicate read from a target's mission table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionMarkerRule {
    /// Only kinds 0xC and 0xD participate in this marking pass.
    pub kind: u32,
    pub mission_key: u32,
}

/// The two independent mission-marker words owned by one mission table record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MissionMarkerEntry {
    /// Definition pointer at record +0; absent records are skipped.
    pub first_rule: Option<MissionMarkerRule>,
    /// The record's +8 word, which the original writes to one on a match.
    pub first_marked: bool,
    /// Optional definition pointer at the nested +0x60 field.
    pub second_rule: Option<MissionMarkerRule>,
    /// The record's +0x14 word, independently written to one on a match.
    pub second_marked: bool,
}

/// Rust-owned callback and mission state attached to one live native scene object.
/// The object identity is supplied by `SceneMembership`; raw guest pointers do not enter this
/// store. Marker entries are read again after the callback so callback mutations are visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeInteractionTarget {
    callback: Option<InteractionCallbackId>,
    mission_key: u32,
    mission_entries: Vec<Option<MissionMarkerEntry>>,
}

impl NativeInteractionTarget {
    pub fn new(
        callback: Option<InteractionCallbackId>,
        mission_key: u32,
        mission_entries: Vec<Option<MissionMarkerEntry>>,
    ) -> Self {
        Self {
            callback,
            mission_key,
            mission_entries,
        }
    }
}

/// Failure to attach or reacquire a target at its current scene generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionTargetStoreError {
    Scene(MembershipError),
    AlreadyAttached,
    NotAttached,
}

/// Native owner for target callback and mission-marker state.
///
/// Every operation takes the scene owner and exact `ObjectId`, so an old callback continuation
/// cannot silently address a replacement object in the same pool slot. Methods return copied
/// values instead of target borrows, allowing callbacks to re-enter and mutate this store.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InteractionTargetStore {
    targets: BTreeMap<ObjectId, NativeInteractionTarget>,
}

impl InteractionTargetStore {
    /// Attach typed callback and mission state to an already-live native scene object.
    pub fn attach(
        &mut self,
        scene: &SceneMembership,
        object: ObjectId,
        target: NativeInteractionTarget,
    ) -> Result<(), InteractionTargetStoreError> {
        if !scene.is_active(object) {
            return Err(InteractionTargetStoreError::Scene(
                MembershipError::NotActive,
            ));
        }
        if self.targets.contains_key(&object) {
            return Err(InteractionTargetStoreError::AlreadyAttached);
        }

        // A replacement scene generation invalidates any abandoned entry from this slot.
        self.targets.retain(|id, _| id.slot != object.slot);
        self.targets.insert(object, target);
        Ok(())
    }

    /// Resolve the callback only when the continuation reaches callback lookup, after send.
    pub fn callback_for_target(
        &self,
        scene: &SceneMembership,
        object: ObjectId,
    ) -> Result<Option<InteractionCallbackId>, InteractionTargetStoreError> {
        let target = self.live_target(scene, object)?;
        Ok(target.callback)
    }

    /// Change a target's callback from a native handler or lifecycle callback.
    pub fn set_callback(
        &mut self,
        scene: &SceneMembership,
        object: ObjectId,
        callback: Option<InteractionCallbackId>,
    ) -> Result<(), InteractionTargetStoreError> {
        self.live_target_mut(scene, object)?.callback = callback;
        Ok(())
    }

    /// Read the mission key at the post-callback point in the original continuation.
    pub fn mission_key(
        &self,
        scene: &SceneMembership,
        object: ObjectId,
    ) -> Result<u32, InteractionTargetStoreError> {
        Ok(self.live_target(scene, object)?.mission_key)
    }

    /// Publish a callback's mission-key change before the original marker pass reacquires it.
    pub fn set_mission_key(
        &mut self,
        scene: &SceneMembership,
        object: ObjectId,
        mission_key: u32,
    ) -> Result<(), InteractionTargetStoreError> {
        self.live_target_mut(scene, object)?.mission_key = mission_key;
        Ok(())
    }

    /// Apply the post-callback marker pass against the target's current mission key.
    pub fn apply_post_callback_mission_flags(
        &mut self,
        scene: &SceneMembership,
        object: ObjectId,
    ) -> Result<(), InteractionTargetStoreError> {
        let target = self.live_target_mut(scene, object)?;
        apply_post_callback_mission_flags(target.mission_key, &mut target.mission_entries);
        Ok(())
    }

    /// Return an owned marker snapshot for tracing without retaining a mutable target borrow.
    pub fn mission_markers(
        &self,
        scene: &SceneMembership,
        object: ObjectId,
    ) -> Result<Vec<Option<MissionMarkerEntry>>, InteractionTargetStoreError> {
        Ok(self.live_target(scene, object)?.mission_entries.clone())
    }

    /// Retire target state and scene membership together for exactly one live generation.
    pub fn retire(
        &mut self,
        scene: &mut SceneMembership,
        object: ObjectId,
    ) -> Result<NativeInteractionTarget, InteractionTargetStoreError> {
        if !scene.is_active(object) {
            return Err(InteractionTargetStoreError::Scene(
                MembershipError::NotActive,
            ));
        }
        if !self.targets.contains_key(&object) {
            return Err(InteractionTargetStoreError::NotAttached);
        }

        let target = self.targets.remove(&object).expect("preflighted target");
        if let Err(error) = scene.retire(object) {
            self.targets.insert(object, target);
            return Err(InteractionTargetStoreError::Scene(error));
        }
        Ok(target)
    }

    fn live_target(
        &self,
        scene: &SceneMembership,
        object: ObjectId,
    ) -> Result<&NativeInteractionTarget, InteractionTargetStoreError> {
        if !scene.is_active(object) {
            return Err(InteractionTargetStoreError::Scene(
                MembershipError::NotActive,
            ));
        }
        self.targets
            .get(&object)
            .ok_or(InteractionTargetStoreError::NotAttached)
    }

    fn live_target_mut(
        &mut self,
        scene: &SceneMembership,
        object: ObjectId,
    ) -> Result<&mut NativeInteractionTarget, InteractionTargetStoreError> {
        if !scene.is_active(object) {
            return Err(InteractionTargetStoreError::Scene(
                MembershipError::NotActive,
            ));
        }
        self.targets
            .get_mut(&object)
            .ok_or(InteractionTargetStoreError::NotAttached)
    }
}

/// Apply the source-observed 2345D0 post-callback marker pass to Rust-owned table entries.
///
/// The caller supplies the target's mission key after its callback has completed. Each present
/// entry's first and second predicates are independent; a match writes the corresponding flag
/// to one. This preserves the callback-before-key-read chronology without a guest address.
pub fn apply_post_callback_mission_flags(
    target_mission_key: u32,
    entries: &mut [Option<MissionMarkerEntry>],
) {
    for entry in entries.iter_mut().flatten() {
        if entry.first_rule.is_some_and(|rule| {
            (0xC..=0xD).contains(&rule.kind) && rule.mission_key == target_mission_key
        }) {
            entry.first_marked = true;
        }
        if entry.second_rule.is_some_and(|rule| {
            (0xC..=0xD).contains(&rule.kind) && rule.mission_key == target_mission_key
        }) {
            entry.second_marked = true;
        }
    }
}

/// Player and actor identities captured before the network send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapturedInteractionPlayer {
    pub player: WeaponPlayerHandle,
    pub actor: Option<ObjectId>,
}

/// Outcome when a native player lifetime is not available at the interaction boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionSubmissionOutcome {
    /// No native player could be captured, so the typed path made no interaction changes.
    MissingPlayer,
    /// Submission completed, with the callback branch recorded for trace comparison.
    Completed { callback_invoked: bool },
}

/// Failure during the native owner continuation. Each failure preserves the original operation
/// order: target lookup/callback/marker errors stop at their point, and a player retired by a
/// callback is rejected before its interaction flag can touch a replacement lifetime.
#[derive(Debug, PartialEq, Eq)]
pub enum NativeInteractionError<E> {
    Target(InteractionTargetStoreError),
    Handler(E),
    CapturedPlayerRetired(WeaponPlayerHandle),
}

/// Native effects around the Rust-owned interaction continuation.
///
/// Both calls receive the typed owners because the sender and callback may re-enter. No store
/// borrow is held by the continuation across either call. Callback lookup occurs after `send`,
/// and mission flags are applied from the then-current target state after the callback and player
/// flag write.
pub trait NativeInteractionHandler {
    type Error;

    fn send_interaction(
        &mut self,
        player: WeaponPlayerId,
        target: ObjectId,
        scene: &mut SceneMembership,
        targets: &mut InteractionTargetStore,
    );

    fn invoke_target_callback(
        &mut self,
        callback: InteractionCallbackId,
        target: ObjectId,
        captured_actor: Option<ObjectId>,
        scene: &mut SceneMembership,
        targets: &mut InteractionTargetStore,
    ) -> Result<(), Self::Error>;
}

/// Run the A0D30 continuation against the subsystem's Rust-owned player and target stores.
///
/// Player/actor capture is supplied as generation-checked typed identities before entry. The
/// callback is resolved only after the sender returns. The player's flag is committed to that
/// captured inventory lifetime after callback completion, then target mission flags reacquire
/// the current target/key. This preserves the measured partial-effect chronology if a callback
/// retires either identity.
pub fn submit_owned_interaction<const PLAYER_SLOTS: usize, H: NativeInteractionHandler>(
    players: &mut WeaponInventoryStore<PLAYER_SLOTS>,
    targets: &mut InteractionTargetStore,
    scene: &mut SceneMembership,
    captured: CapturedInteractionPlayer,
    target: ObjectId,
    handler: &mut H,
) -> Result<InteractionSubmissionOutcome, NativeInteractionError<H::Error>> {
    if players.get_handle(captured.player).is_none() {
        return Ok(InteractionSubmissionOutcome::MissingPlayer);
    }

    handler.send_interaction(captured.player.player(), target, scene, targets);
    let callback = targets
        .callback_for_target(scene, target)
        .map_err(NativeInteractionError::Target)?;
    if let Some(callback) = callback {
        handler
            .invoke_target_callback(callback, target, captured.actor, scene, targets)
            .map_err(NativeInteractionError::Handler)?;
    }

    let player = players.get_handle_mut(captured.player).ok_or(
        NativeInteractionError::CapturedPlayerRetired(captured.player),
    )?;
    player.interaction_flag = true;
    targets
        .apply_post_callback_mission_flags(scene, target)
        .map_err(NativeInteractionError::Target)?;

    Ok(InteractionSubmissionOutcome::Completed {
        callback_invoked: callback.is_some(),
    })
}

/// Narrow host contract around the measured A0D30 continuation.
///
/// Implementations must look up the callback only when `callback_for_target` is called (after
/// `send_interaction`), and `apply_post_callback_mission_flags` must read the target's current
/// mission key at that point (after the callback and player flag write). No object or player
/// borrow may span the send or callback, which can re-enter and mutate those owners.
pub trait InteractionHost {
    type Error;

    /// Capture the current player record and controlled actor before any send or callback.
    fn capture_player(&mut self, player: WeaponPlayerId) -> Option<CapturedInteractionPlayer>;

    /// Send the target reference. The original continuation proceeds regardless of network gates.
    fn send_interaction(&mut self, player: WeaponPlayerId, target: ObjectId);

    /// Resolve the target's current callback after the send has returned.
    fn callback_for_target(
        &mut self,
        target: ObjectId,
    ) -> Result<Option<InteractionCallbackId>, Self::Error>;

    /// Invoke the callback synchronously with the saved target and captured actor identity.
    fn invoke_target_callback(
        &mut self,
        callback: InteractionCallbackId,
        target: ObjectId,
        captured_actor: Option<ObjectId>,
    ) -> Result<(), Self::Error>;

    /// Set the interaction flag on the captured player lifetime, not a newly-current player.
    fn set_captured_player_interaction_flag(
        &mut self,
        player: WeaponPlayerHandle,
    ) -> Result<(), Self::Error>;

    /// Apply mission flags using the target's key as it exists after the callback.
    fn apply_post_callback_mission_flags(&mut self, target: ObjectId) -> Result<(), Self::Error>;
}

/// Run the native interaction continuation in the source-observed A0D30 order.
///
/// Missing native player ownership fails closed before sending. Callback or target errors stop
/// the continuation at that point, matching the original synchronous fault chronology; the host
/// must generation-check identities when each operation is reacquired.
pub fn submit_interaction<H: InteractionHost>(
    host: &mut H,
    player: WeaponPlayerId,
    target: ObjectId,
) -> Result<InteractionSubmissionOutcome, H::Error> {
    let Some(captured) = host.capture_player(player) else {
        return Ok(InteractionSubmissionOutcome::MissingPlayer);
    };

    host.send_interaction(player, target);
    let callback = host.callback_for_target(target)?;
    if let Some(callback) = callback {
        host.invoke_target_callback(callback, target, captured.actor)?;
    }
    host.set_captured_player_interaction_flag(captured.player)?;
    host.apply_post_callback_mission_flags(target)?;

    Ok(InteractionSubmissionOutcome::Completed {
        callback_invoked: callback.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(slot: u32, score: f32) -> ScoredInteractionCandidate {
        ScoredInteractionCandidate {
            target: ObjectId {
                slot,
                generation: 1,
            },
            score,
        }
    }

    #[test]
    fn interaction_candidate_rank_preserves_encounter_order_on_equal_scores() {
        let candidates = [candidate(2, 4.0), candidate(7, 4.0), candidate(9, 5.0)];

        assert_eq!(
            rank_interaction_candidates(&candidates),
            Some(candidates[0].target)
        );
    }

    #[test]
    fn interaction_candidate_rank_keeps_only_strictly_lower_score_updates() {
        let candidates = [candidate(2, 4.0), candidate(7, 3.5), candidate(9, 3.5)];

        assert_eq!(
            rank_interaction_candidates(&candidates),
            Some(candidates[1].target)
        );
    }

    #[test]
    fn interaction_candidate_rank_ignores_candidates_after_the_twentieth() {
        let mut candidates = alloc::vec![candidate(0, 4.0); INTERACTION_CANDIDATE_LIMIT];
        candidates[0] = candidate(0, 3.0);
        candidates.push(candidate(20, -1.0));

        assert_eq!(
            rank_interaction_candidates(&candidates),
            Some(candidates[0].target)
        );
    }

    #[test]
    fn interaction_candidate_rank_matches_finite_initial_best_behavior() {
        let candidates = [
            candidate(2, f32::MAX),
            candidate(7, f32::INFINITY),
            candidate(9, f32::NAN),
        ];

        assert_eq!(rank_interaction_candidates(&candidates), None);
    }

    #[test]
    fn live_interaction_rank_returns_the_active_winner_generation() {
        let mut scene = SceneMembership::default();
        let first = scene.spawn(2).unwrap();
        let winner = scene.spawn(7).unwrap();
        let candidates = [
            ScoredInteractionCandidate {
                target: first,
                score: 4.0,
            },
            ScoredInteractionCandidate {
                target: winner,
                score: 3.0,
            },
        ];

        assert_eq!(
            rank_live_interaction_target(&scene, &candidates),
            Ok(Some(winner))
        );
    }

    #[test]
    fn live_interaction_rank_does_not_fall_back_from_a_stale_winner() {
        let mut scene = SceneMembership::default();
        let active_candidate = scene.spawn(2).unwrap();
        let stale_winner = scene.spawn(7).unwrap();
        scene.retire(stale_winner).unwrap();
        let candidates = [
            ScoredInteractionCandidate {
                target: active_candidate,
                score: 4.0,
            },
            ScoredInteractionCandidate {
                target: stale_winner,
                score: 3.0,
            },
        ];

        assert_eq!(
            rank_live_interaction_target(&scene, &candidates),
            Err(MembershipError::NotActive)
        );
    }

    #[test]
    fn live_interaction_rank_returns_none_for_an_empty_or_unrankable_window() {
        let scene = SceneMembership::default();
        assert_eq!(rank_live_interaction_target(&scene, &[]), Ok(None));
        assert_eq!(
            rank_live_interaction_target(&scene, &[candidate(1, f32::NAN)]),
            Ok(None)
        );
    }

    #[derive(Default)]
    struct NativeHost {
        calls: alloc::vec::Vec<&'static str>,
        callback_after_send: Option<InteractionCallbackId>,
        callback_mission_key: Option<u32>,
        invoked: Option<(InteractionCallbackId, ObjectId, Option<ObjectId>)>,
        retire_in_send: bool,
        retire_in_callback: bool,
    }

    impl NativeInteractionHandler for NativeHost {
        type Error = ();

        fn send_interaction(
            &mut self,
            _player: WeaponPlayerId,
            target: ObjectId,
            scene: &mut SceneMembership,
            targets: &mut InteractionTargetStore,
        ) {
            self.calls.push("send");
            if let Some(callback) = self.callback_after_send {
                targets.set_callback(scene, target, Some(callback)).unwrap();
            }
            if self.retire_in_send {
                targets.retire(scene, target).unwrap();
            }
        }

        fn invoke_target_callback(
            &mut self,
            callback: InteractionCallbackId,
            target: ObjectId,
            captured_actor: Option<ObjectId>,
            scene: &mut SceneMembership,
            targets: &mut InteractionTargetStore,
        ) -> Result<(), Self::Error> {
            self.calls.push("callback");
            self.invoked = Some((callback, target, captured_actor));
            if let Some(key) = self.callback_mission_key {
                targets.set_mission_key(scene, target, key).unwrap();
            }
            if self.retire_in_callback {
                targets.retire(scene, target).unwrap();
            }
            Ok(())
        }
    }

    fn owned_fixture() -> (
        crate::weapon_inventory::WeaponSubsystemState<2>,
        SceneMembership,
        CapturedInteractionPlayer,
        ObjectId,
    ) {
        let mut scene = SceneMembership::default();
        let target = scene.spawn(8).unwrap();
        let actor = scene.spawn(9).unwrap();
        let mut subsystem = crate::weapon_inventory::WeaponSubsystemState::<2>::new();
        let player = WeaponPlayerId::new(0);
        let player_handle = subsystem
            .players
            .begin_lifetime(player, Default::default())
            .unwrap();
        let initial_callback = InteractionCallbackId(17);
        subsystem
            .interaction_targets
            .attach(
                &scene,
                target,
                NativeInteractionTarget::new(
                    Some(initial_callback),
                    23,
                    alloc::vec![Some(MissionMarkerEntry {
                        first_rule: Some(MissionMarkerRule {
                            kind: 0xC,
                            mission_key: 0xfeed,
                        }),
                        first_marked: false,
                        second_rule: Some(MissionMarkerRule {
                            kind: 0xD,
                            mission_key: 0xfeed,
                        }),
                        second_marked: false,
                    })],
                ),
            )
            .unwrap();
        (
            subsystem,
            scene,
            CapturedInteractionPlayer {
                player: player_handle,
                actor: Some(actor),
            },
            target,
        )
    }

    #[test]
    fn subsystem_owner_keeps_interaction_targets_with_scene_lifetimes() {
        let mut scene = SceneMembership::default();
        let target = scene.spawn(4).unwrap();
        let mut subsystem = crate::weapon_inventory::WeaponSubsystemState::<2>::new();
        let callback = InteractionCallbackId(17);

        subsystem
            .interaction_targets
            .attach(
                &scene,
                target,
                NativeInteractionTarget::new(Some(callback), 23, Vec::new()),
            )
            .unwrap();
        assert_eq!(
            subsystem
                .interaction_targets
                .callback_for_target(&scene, target),
            Ok(Some(callback))
        );

        subsystem
            .interaction_targets
            .retire(&mut scene, target)
            .unwrap();
        assert!(
            subsystem
                .interaction_targets
                .callback_for_target(&scene, target)
                .is_err()
        );
    }

    #[test]
    fn native_interaction_reacquires_callback_and_mission_key_around_reentrant_calls() {
        let (mut subsystem, mut scene, captured, target) = owned_fixture();
        let mut host = NativeHost {
            callback_after_send: Some(InteractionCallbackId(19)),
            callback_mission_key: Some(0xfeed),
            ..NativeHost::default()
        };

        let outcome = subsystem
            .submit_native_interaction(&mut scene, captured, target, &mut host)
            .unwrap();

        assert_eq!(
            outcome,
            InteractionSubmissionOutcome::Completed {
                callback_invoked: true
            }
        );
        assert_eq!(host.calls, ["send", "callback"]);
        assert_eq!(
            host.invoked,
            Some((InteractionCallbackId(19), target, captured.actor))
        );
        assert!(
            subsystem
                .players
                .get_handle(captured.player)
                .unwrap()
                .interaction_flag
        );
        let markers = subsystem
            .interaction_targets
            .mission_markers(&scene, target)
            .unwrap();
        let marker = markers[0].unwrap();
        assert!(marker.first_marked);
        assert!(marker.second_marked);
    }

    #[test]
    fn native_interaction_sets_captured_player_flag_before_post_callback_target_failure() {
        let (mut subsystem, mut scene, captured, target) = owned_fixture();
        let mut host = NativeHost {
            retire_in_callback: true,
            ..NativeHost::default()
        };

        let result = subsystem.submit_native_interaction(&mut scene, captured, target, &mut host);

        assert_eq!(
            result,
            Err(NativeInteractionError::Target(
                InteractionTargetStoreError::Scene(MembershipError::NotActive)
            ))
        );
        assert_eq!(host.calls, ["send", "callback"]);
        assert!(
            subsystem
                .players
                .get_handle(captured.player)
                .unwrap()
                .interaction_flag
        );
    }

    #[test]
    fn native_interaction_resolves_callback_after_sender_retires_target() {
        let (mut subsystem, mut scene, captured, target) = owned_fixture();
        let mut host = NativeHost {
            retire_in_send: true,
            ..NativeHost::default()
        };

        let result = subsystem.submit_native_interaction(&mut scene, captured, target, &mut host);

        assert_eq!(
            result,
            Err(NativeInteractionError::Target(
                InteractionTargetStoreError::Scene(MembershipError::NotActive)
            ))
        );
        assert_eq!(host.calls, ["send"]);
        assert!(
            !subsystem
                .players
                .get_handle(captured.player)
                .unwrap()
                .interaction_flag
        );
    }
}
