// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed local decision model for the measured 0x2737C0 target handoff.
//!
//! This module models the ordered branch selection and applies its effects to
//! Rust-owned actor state. Scenario population, selector inputs, actor/world
//! reads, and the runtime adapter remain explicit boundaries.
//! Verification: Tier 0 contract tests against the static REA contract in
//! `docs/t1662-rea-targets.md`; this is not Tier 1 or replay evidence.

use crate::{
    movement_approach::Position3,
    rea_target_selector::{TargetSelectorOutcome, commit_target_selection},
    rea_tick_state::{BehaviorPair, LocalState, ReaTickState},
};
use alloc::vec::Vec;

pub const HANDOFF_BEHAVIOR: BehaviorPair = BehaviorPair { low: 8, high: 0 };
pub const PRESENCE_BEHAVIOR: BehaviorPair = BehaviorPair {
    low: 0x0020_0000,
    high: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetObjectId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GoalSelectionFlags(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandoffBranch {
    PresenceBehavior,
    InactiveActor,
    ForcedGoalSelection,
    NavigationStateAlreadyActive,
    TypeZeroTargetHandoff,
    TypeOneTargetHandoff,
    TypeOneGoalSelection,
    TypeTwoThreeGoalSelection,
    TypeZeroOneDistanceRequest,
    TypeTwoThreeRoomRelocation,
    TypeTwoThreeDistanceRequest,
    NoAction,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandoffEffect {
    SelectGoal(GoalSelectionFlags),
    RequestLocalState(u32),
    RequestBehavior(BehaviorPair),
    StoreTarget(TargetObjectId),
    CopyGoalPosition(Position3),
}

#[derive(Clone, Debug, PartialEq)]
pub struct HandoffDecision {
    pub branch: HandoffBranch,
    /// Effects are in original call/write order. A goal selection can mutate
    /// Rust-owned goal state before the following local-state request.
    pub effects: Vec<HandoffEffect>,
}

/// Snapshot of typed actor and world checks made by the `0x2737C0` boundary.
/// Values produced by partial services are supplied explicitly; this function
/// never reads guest memory or invents scenario/target data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandoffInput {
    pub presence_behavior_required: bool,
    pub actor_has_navigation_flag: bool,
    pub actor_reference_is_special: bool,
    pub forced_goal_check: bool,
    pub goal_key_is_listed: bool,
    pub local_state: u32,
    pub actor_selected: bool,
    pub route_kind: u32,
    pub route_timer: f32,
    pub special_type_one_predicate: bool,
    pub group_target: Option<TargetObjectId>,
    pub type_one_goal_selection_succeeds: bool,
    pub type_two_three_goal_selection_succeeds: bool,
    pub goal_distance_squared_xz: f32,
    pub actor_room: u8,
    pub goal_room: u8,
    pub goal_position: Position3,
}

fn decision(branch: HandoffBranch, effects: &[HandoffEffect]) -> HandoffDecision {
    HandoffDecision {
        branch,
        effects: effects.into(),
    }
}

/// Selects the first measured handoff branch from `0x2737C0`.
///
/// The ordered thresholds are strict as documented: timer `> 10`, `> 7`,
/// `> 5`; distance-squared `> 9`, `> 0.25`, and `> 2.25`. The force flag only
/// causes the candidate-key membership check. No behavior request is committed
/// here; it is returned as an effect for the typed behavior-request seam.
pub fn decide_target_handoff(input: HandoffInput) -> HandoffDecision {
    if input.presence_behavior_required {
        return decision(
            HandoffBranch::PresenceBehavior,
            &[HandoffEffect::RequestBehavior(PRESENCE_BEHAVIOR)],
        );
    }
    if !input.actor_has_navigation_flag {
        return decision(HandoffBranch::InactiveActor, &[]);
    }
    if (input.forced_goal_check || input.actor_reference_is_special) && !input.goal_key_is_listed {
        let mut effects = alloc::vec![HandoffEffect::SelectGoal(GoalSelectionFlags(0))];
        if input.local_state != 1 {
            effects.push(HandoffEffect::RequestLocalState(1));
        }
        return HandoffDecision {
            branch: HandoffBranch::ForcedGoalSelection,
            effects,
        };
    }
    if matches!(input.local_state, 1 | 4) {
        return decision(HandoffBranch::NavigationStateAlreadyActive, &[]);
    }

    let target_handoff = input.actor_selected
        && match input.route_kind {
            0 => input.route_timer > 10.0,
            1 => input.special_type_one_predicate,
            _ => false,
        }
        && input.group_target.is_some();
    if target_handoff {
        let target = input.group_target.expect("checked target handoff");
        return decision(
            if input.route_kind == 0 {
                HandoffBranch::TypeZeroTargetHandoff
            } else {
                HandoffBranch::TypeOneTargetHandoff
            },
            &[
                HandoffEffect::StoreTarget(target),
                HandoffEffect::RequestBehavior(HANDOFF_BEHAVIOR),
            ],
        );
    }

    if input.actor_selected
        && input.route_kind == 1
        && input.group_target.is_none()
        && input.route_timer > 7.0
        && input.type_one_goal_selection_succeeds
    {
        return decision(
            HandoffBranch::TypeOneGoalSelection,
            &[
                HandoffEffect::SelectGoal(GoalSelectionFlags(1)),
                HandoffEffect::RequestLocalState(1),
            ],
        );
    }
    if input.actor_selected
        && matches!(input.route_kind, 2 | 3)
        && input.route_timer > 5.0
        && input.type_two_three_goal_selection_succeeds
    {
        return decision(
            HandoffBranch::TypeTwoThreeGoalSelection,
            &[
                HandoffEffect::SelectGoal(GoalSelectionFlags(2)),
                HandoffEffect::RequestLocalState(4),
            ],
        );
    }

    if matches!(input.route_kind, 0 | 1) && input.goal_distance_squared_xz > 9.0 {
        return decision(
            HandoffBranch::TypeZeroOneDistanceRequest,
            &[HandoffEffect::RequestLocalState(1)],
        );
    }
    if matches!(input.route_kind, 2 | 3)
        && input.goal_distance_squared_xz > 0.25
        && input.actor_room == 0
        && input.goal_room == 0
    {
        return decision(
            HandoffBranch::TypeTwoThreeRoomRelocation,
            &[HandoffEffect::CopyGoalPosition(input.goal_position)],
        );
    }
    if matches!(input.route_kind, 2 | 3) && input.goal_distance_squared_xz > 2.25 {
        return decision(
            HandoffBranch::TypeTwoThreeDistanceRequest,
            &[HandoffEffect::RequestLocalState(1)],
        );
    }

    decision(HandoffBranch::NoAction, &[])
}

/// Result of applying the ordered handoff effects to Rust-owned actor state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HandoffCommit {
    pub goal_selection_committed: bool,
    pub local_state_queued: bool,
    pub behavior_queued: bool,
    pub target_stored: bool,
    pub world_position_copied: bool,
}

/// Applies a handoff decision in its original effect order. The selector result
/// is supplied by the typed navigation boundary; absent or unsuccessful
/// selection leaves the goal unchanged while later handoff effects still run.
pub fn apply_target_handoff(
    actor: &mut ReaTickState,
    decision: &HandoffDecision,
    mut goal_selection: Option<TargetSelectorOutcome>,
) -> HandoffCommit {
    let mut commit = HandoffCommit::default();
    for effect in &decision.effects {
        match *effect {
            HandoffEffect::SelectGoal(_) => {
                if let Some(outcome) = goal_selection.take() {
                    commit.goal_selection_committed =
                        commit_target_selection(actor.actor_navigation_mut(), outcome);
                }
            }
            HandoffEffect::RequestLocalState(value) => {
                let requested = LocalState(value);
                if actor.local_state() != requested {
                    actor.queue_local_state(requested);
                    commit.local_state_queued = true;
                }
            }
            HandoffEffect::RequestBehavior(pair) => {
                commit.behavior_queued |= actor.request_behavior(pair);
            }
            HandoffEffect::StoreTarget(target) => {
                actor.set_handoff_target(target);
                commit.target_stored = true;
            }
            HandoffEffect::CopyGoalPosition(position) => {
                actor.set_world_position(position);
                commit.world_position_copied = true;
            }
        }
    }
    commit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        awareness::AwarenessCache,
        navigation_scoring::NodeId,
        rea_target_selector::CandidateSourceKey,
        rea_tick_state::{BehaviorMode, ReaTickSnapshot, TransitionProgress},
    };

    fn actor_state(local_state: u32) -> ReaTickState {
        ReaTickState::new(ReaTickSnapshot {
            mode: BehaviorMode::Other,
            mode10_active: BehaviorPair::default(),
            other_active: BehaviorPair::default(),
            previous_behavior: BehaviorPair::default(),
            pending_behavior: BehaviorPair::default(),
            local_state: LocalState(local_state),
            previous_local_state: LocalState(0),
            pending_local_state: None,
            transition_progress: TransitionProgress::default(),
            awareness_cache: AwarenessCache::from_raw(0),
        })
    }

    fn input() -> HandoffInput {
        HandoffInput {
            presence_behavior_required: false,
            actor_has_navigation_flag: true,
            actor_reference_is_special: false,
            forced_goal_check: false,
            goal_key_is_listed: true,
            local_state: 0,
            actor_selected: false,
            route_kind: 0,
            route_timer: 0.0,
            special_type_one_predicate: false,
            group_target: None,
            type_one_goal_selection_succeeds: false,
            type_two_three_goal_selection_succeeds: false,
            goal_distance_squared_xz: 0.0,
            actor_room: 1,
            goal_room: 1,
            goal_position: Position3 {
                x: 10.0,
                y: 2.0,
                z: -3.0,
            },
        }
    }

    #[test]
    fn presence_and_active_flag_precede_all_goal_logic() {
        let presence = decide_target_handoff(HandoffInput {
            presence_behavior_required: true,
            actor_has_navigation_flag: false,
            ..input()
        });
        assert_eq!(presence.branch, HandoffBranch::PresenceBehavior);
        assert_eq!(
            presence.effects,
            [HandoffEffect::RequestBehavior(PRESENCE_BEHAVIOR)]
        );

        let inactive = decide_target_handoff(HandoffInput {
            actor_has_navigation_flag: false,
            forced_goal_check: true,
            ..input()
        });
        assert_eq!(inactive.branch, HandoffBranch::InactiveActor);
        assert!(inactive.effects.is_empty());
    }

    #[test]
    fn forced_check_only_selects_when_goal_key_is_absent_and_then_requests_state() {
        let absent = decide_target_handoff(HandoffInput {
            forced_goal_check: true,
            goal_key_is_listed: false,
            ..input()
        });
        assert_eq!(absent.branch, HandoffBranch::ForcedGoalSelection);
        assert_eq!(
            absent.effects,
            [
                HandoffEffect::SelectGoal(GoalSelectionFlags(0)),
                HandoffEffect::RequestLocalState(1)
            ]
        );

        let listed = decide_target_handoff(HandoffInput {
            forced_goal_check: true,
            goal_key_is_listed: true,
            route_kind: 9,
            ..input()
        });
        assert_eq!(listed.branch, HandoffBranch::NoAction);

        let special_reference = decide_target_handoff(HandoffInput {
            actor_reference_is_special: true,
            goal_key_is_listed: false,
            ..input()
        });
        assert_eq!(special_reference.branch, HandoffBranch::ForcedGoalSelection);
    }

    #[test]
    fn active_navigation_states_return_before_type_handoff() {
        for local_state in [1, 4] {
            let got = decide_target_handoff(HandoffInput {
                local_state,
                actor_selected: true,
                route_timer: 20.0,
                group_target: Some(TargetObjectId(44)),
                ..input()
            });
            assert_eq!(got.branch, HandoffBranch::NavigationStateAlreadyActive);
            assert!(got.effects.is_empty());
        }
    }

    #[test]
    fn target_handoff_requires_selected_actor_and_strict_type_zero_timer() {
        let target = TargetObjectId(7);
        let at_threshold = decide_target_handoff(HandoffInput {
            actor_selected: true,
            route_timer: 10.0,
            group_target: Some(target),
            ..input()
        });
        assert_eq!(at_threshold.branch, HandoffBranch::NoAction);

        let after_threshold = decide_target_handoff(HandoffInput {
            actor_selected: true,
            route_timer: 10.01,
            group_target: Some(target),
            ..input()
        });
        assert_eq!(after_threshold.branch, HandoffBranch::TypeZeroTargetHandoff);
        assert_eq!(
            after_threshold.effects,
            [
                HandoffEffect::StoreTarget(target),
                HandoffEffect::RequestBehavior(HANDOFF_BEHAVIOR)
            ]
        );
    }

    #[test]
    fn selected_type_one_uses_predicate_or_goal_selection_in_source_order() {
        let target = TargetObjectId(8);
        let handoff = decide_target_handoff(HandoffInput {
            actor_selected: true,
            route_kind: 1,
            special_type_one_predicate: true,
            group_target: Some(target),
            ..input()
        });
        assert_eq!(handoff.branch, HandoffBranch::TypeOneTargetHandoff);

        let selection = decide_target_handoff(HandoffInput {
            actor_selected: true,
            route_kind: 1,
            route_timer: 7.1,
            type_one_goal_selection_succeeds: true,
            ..input()
        });
        assert_eq!(selection.branch, HandoffBranch::TypeOneGoalSelection);
        assert_eq!(
            selection.effects,
            [
                HandoffEffect::SelectGoal(GoalSelectionFlags(1)),
                HandoffEffect::RequestLocalState(1)
            ]
        );
    }

    #[test]
    fn selected_type_two_three_selects_then_requests_state_four() {
        let got = decide_target_handoff(HandoffInput {
            actor_selected: true,
            route_kind: 3,
            route_timer: 5.01,
            type_two_three_goal_selection_succeeds: true,
            ..input()
        });
        assert_eq!(got.branch, HandoffBranch::TypeTwoThreeGoalSelection);
        assert_eq!(
            got.effects,
            [
                HandoffEffect::SelectGoal(GoalSelectionFlags(2)),
                HandoffEffect::RequestLocalState(4)
            ]
        );
    }

    #[test]
    fn type_zero_one_distance_uses_strict_nine_squared_threshold() {
        let at_threshold = decide_target_handoff(HandoffInput {
            goal_distance_squared_xz: 9.0,
            ..input()
        });
        assert_eq!(at_threshold.branch, HandoffBranch::NoAction);

        let beyond = decide_target_handoff(HandoffInput {
            goal_distance_squared_xz: 9.01,
            ..input()
        });
        assert_eq!(beyond.branch, HandoffBranch::TypeZeroOneDistanceRequest);
    }

    #[test]
    fn type_two_three_relocation_precedes_far_distance_request() {
        let relocation = decide_target_handoff(HandoffInput {
            route_kind: 2,
            goal_distance_squared_xz: 1.0,
            actor_room: 0,
            goal_room: 0,
            ..input()
        });
        assert_eq!(relocation.branch, HandoffBranch::TypeTwoThreeRoomRelocation);
        assert_eq!(
            relocation.effects,
            [HandoffEffect::CopyGoalPosition(input().goal_position)]
        );

        let request = decide_target_handoff(HandoffInput {
            route_kind: 2,
            goal_distance_squared_xz: 2.26,
            actor_room: 1,
            goal_room: 1,
            ..input()
        });
        assert_eq!(request.branch, HandoffBranch::TypeTwoThreeDistanceRequest);
    }

    #[test]
    fn forced_selection_commits_owned_goal_before_queuing_the_state_request() {
        let decision = decide_target_handoff(HandoffInput {
            forced_goal_check: true,
            goal_key_is_listed: false,
            ..input()
        });
        let mut actor = actor_state(0);
        actor
            .actor_navigation_mut()
            .route_mut()
            .reverse_trace_within = 3;
        actor.actor_navigation_mut().route_mut().reverse_trace_other = 4;
        let selected = TargetSelectorOutcome::Selected {
            source_key: CandidateSourceKey(5),
            node: NodeId(44),
            score: 1.0,
        };

        let applied = apply_target_handoff(&mut actor, &decision, Some(selected));

        assert!(applied.goal_selection_committed);
        assert!(applied.local_state_queued);
        assert_eq!(actor.actor_navigation().route().goal_node, NodeId(44));
        assert_eq!(actor.actor_navigation().route().reverse_trace_within, 0);
        assert_eq!(actor.actor_navigation().route().reverse_trace_other, 0);
        assert_eq!(actor.pending_local_state(), Some(LocalState(1)));
    }

    #[test]
    fn target_handoff_stores_the_object_and_queues_behavior_on_owned_state() {
        let decision = decide_target_handoff(HandoffInput {
            actor_selected: true,
            route_timer: 10.01,
            group_target: Some(TargetObjectId(77)),
            ..input()
        });
        let mut actor = actor_state(0);

        let applied = apply_target_handoff(&mut actor, &decision, None);

        assert!(applied.target_stored);
        assert!(applied.behavior_queued);
        assert_eq!(actor.handoff_target(), Some(TargetObjectId(77)));
        assert_eq!(actor.pending_behavior(), HANDOFF_BEHAVIOR);
        assert_eq!(actor.actor_navigation().movement_target(), None);
    }

    #[test]
    fn room_relocation_updates_world_position_not_navigation_movement_target() {
        let goal_position = Position3 {
            x: 10.0,
            y: 2.0,
            z: -3.0,
        };
        let decision = decide_target_handoff(HandoffInput {
            route_kind: 2,
            goal_distance_squared_xz: 0.26,
            actor_room: 0,
            goal_room: 0,
            goal_position,
            ..input()
        });
        let mut actor = actor_state(0);

        let applied = apply_target_handoff(&mut actor, &decision, None);

        assert!(applied.world_position_copied);
        assert_eq!(actor.world_position(), Some(goal_position));
        assert_eq!(actor.actor_navigation().movement_target(), None);
    }
}
