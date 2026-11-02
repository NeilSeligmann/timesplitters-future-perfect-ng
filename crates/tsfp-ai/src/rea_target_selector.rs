// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable value model for ordered candidate arbitration in REA 0x271CD0.
//!
//! The caller supplies loaded candidates in source-list order and the results
//! of typed world services (directed matrix rows, scenario nodes, group index,
//! and same-class endpoint reservations). This module owns score arbitration
//! and the successful goal-state commit; it does not read guest memory or
//! invent matrix, scenario, roster, or RNG behavior. Verification: Tier 0
//! contract tests against the measured-static contract in
//! `docs/t1662-rea-targets.md`; runtime selection remains disabled.

use crate::{
    navigation_scoring::NodeId, rea_target_handoff::GoalSelectionFlags,
    rea_tick_state::ActorNavigation,
};

const MAX_SOURCE_KEYS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateSourceKey(pub i16);

#[derive(Clone, Copy, Debug)]
pub struct SelectorCandidate<'a> {
    /// Negative source keys terminate the original ten-key list.
    pub source_key: CandidateSourceKey,
    /// The loaded node resolved at the typed world boundary.
    pub node: NodeId,
    /// Directed cost from the actor's current node; absent table data is the
    /// original -1 result, then the ordinary score is clamped to at least 1.
    pub ordinary_matrix_cost: Option<f32>,
    /// Ordered costs from the selector's scenario-node service. Values below
    /// FLT_MAX overwrite the score; the last qualifying value wins.
    pub scenario_matrix_costs: &'a [f32],
    /// Supplied by the typed actor-roster boundary using the same-class
    /// current/goal endpoint reservation rule.
    pub reserved_by_same_class_actor: bool,
    pub region: i32,
    pub current_region: i32,
    pub distance_squared_xz: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct TargetSelectorInput<'a> {
    pub flags: GoalSelectionFlags,
    pub candidates: &'a [SelectorCandidate<'a>],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TargetSelectorOutcome {
    Selected {
        source_key: CandidateSourceKey,
        node: NodeId,
        score: f32,
    },
    /// Nonzero flags return failure without goal or reverse-trace writes.
    NoSelection,
    /// The flags-zero RNG/x87/CRT fallback is an explicit boundary until its
    /// exact rounding and source-list behavior can be ported and verified.
    RandomFallbackRequired { candidate_count: usize },
}

fn ordinary_score(cost: Option<f32>) -> f32 {
    let mut score = cost.unwrap_or(-1.0);
    if score < 1.0 {
        score = 1.0;
    }
    score
}

fn candidate_score(candidate: SelectorCandidate<'_>, flags: GoalSelectionFlags) -> f32 {
    let mut score = ordinary_score(candidate.ordinary_matrix_cost);
    if flags.0 & 1 != 0 {
        for &scenario_cost in candidate.scenario_matrix_costs {
            if scenario_cost < f32::MAX {
                score = scenario_cost;
            }
        }
    }
    score
}

/// Selects by the measured ordered scan and strict-smaller winner update.
/// Loaded-node resolution and matrix/scenario values are service inputs; a
/// negative key ends the list, which contains at most ten source entries.
pub fn select_target(input: TargetSelectorInput<'_>) -> TargetSelectorOutcome {
    let mut best: Option<(CandidateSourceKey, NodeId, f32)> = None;
    let mut candidate_count = 0;

    for candidate in input.candidates.iter().take(MAX_SOURCE_KEYS) {
        if candidate.source_key.0 < 0 {
            break;
        }
        candidate_count += 1;

        if candidate.reserved_by_same_class_actor {
            continue;
        }
        if input.flags.0 & 2 != 0
            && candidate.region != candidate.current_region
            && candidate.distance_squared_xz > 225.0
        {
            continue;
        }

        let score = candidate_score(*candidate, input.flags);
        if best.is_none_or(|(_, _, best_score)| score < best_score) {
            best = Some((candidate.source_key, candidate.node, score));
        }
    }

    if let Some((source_key, node, score)) = best {
        return TargetSelectorOutcome::Selected {
            source_key,
            node,
            score,
        };
    }
    if input.flags.0 == 0 {
        TargetSelectorOutcome::RandomFallbackRequired { candidate_count }
    } else {
        TargetSelectorOutcome::NoSelection
    }
}

/// Applies only the successful selector's measured goal and reverse-trace
/// writes to the Rust-owned actor record. Failure and fallback leave it intact.
pub fn commit_target_selection(
    actor: &mut ActorNavigation,
    outcome: TargetSelectorOutcome,
) -> bool {
    let TargetSelectorOutcome::Selected { node, .. } = outcome else {
        return false;
    };
    let route = actor.route_mut();
    route.goal_node = node;
    route.reverse_trace_within = 0;
    route.reverse_trace_other = 0;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn candidate<'a>(
        key: i16,
        node: u32,
        ordinary_matrix_cost: Option<f32>,
        scenario_matrix_costs: &'a [f32],
        region: i32,
        current_region: i32,
        distance_squared_xz: f32,
    ) -> SelectorCandidate<'a> {
        SelectorCandidate {
            source_key: CandidateSourceKey(key),
            node: NodeId(node),
            ordinary_matrix_cost,
            scenario_matrix_costs,
            reserved_by_same_class_actor: false,
            region,
            current_region,
            distance_squared_xz,
        }
    }

    fn selected(outcome: TargetSelectorOutcome) -> (CandidateSourceKey, NodeId, f32) {
        let TargetSelectorOutcome::Selected {
            source_key,
            node,
            score,
        } = outcome
        else {
            panic!("expected selected target")
        };
        (source_key, node, score)
    }

    #[test]
    fn strict_score_ties_keep_the_first_source_key_and_negative_ends_the_list() {
        let candidates = [
            candidate(4, 40, Some(3.0), &[], 1, 1, 0.0),
            candidate(8, 80, Some(3.0), &[], 1, 1, 0.0),
            candidate(-1, 10, Some(0.0), &[], 1, 1, 0.0),
            candidate(9, 90, Some(0.0), &[], 1, 1, 0.0),
        ];
        assert_eq!(
            selected(select_target(TargetSelectorInput {
                flags: GoalSelectionFlags(0),
                candidates: &candidates,
            })),
            (CandidateSourceKey(4), NodeId(40), 3.0)
        );
    }

    #[test]
    fn missing_and_subunit_ordinary_costs_clamp_to_one() {
        let candidates = [
            candidate(1, 10, None, &[], 0, 0, 0.0),
            candidate(2, 20, Some(0.5), &[], 0, 0, 0.0),
        ];
        assert_eq!(
            selected(select_target(TargetSelectorInput {
                flags: GoalSelectionFlags(0),
                candidates: &candidates,
            })),
            (CandidateSourceKey(1), NodeId(10), 1.0)
        );
    }

    #[test]
    fn last_qualifying_scenario_cost_overwrites_instead_of_minimizing() {
        let candidates = [
            candidate(1, 10, Some(20.0), &[7.0, 9.0], 0, 0, 0.0),
            candidate(2, 20, Some(8.0), &[], 0, 0, 0.0),
        ];
        assert_eq!(
            selected(select_target(TargetSelectorInput {
                flags: GoalSelectionFlags(1),
                candidates: &candidates,
            })),
            (CandidateSourceKey(2), NodeId(20), 8.0)
        );
    }

    #[test]
    fn scenario_sentinel_does_not_overwrite_and_negative_value_does() {
        let candidates = [candidate(1, 10, Some(12.0), &[-1.0, f32::MAX], 0, 0, 0.0)];
        assert_eq!(
            selected(select_target(TargetSelectorInput {
                flags: GoalSelectionFlags(1),
                candidates: &candidates,
            })),
            (CandidateSourceKey(1), NodeId(10), -1.0)
        );
    }

    #[test]
    fn reservations_always_reject_and_flag_two_uses_strict_distance_boundary() {
        let candidates = [
            SelectorCandidate {
                reserved_by_same_class_actor: true,
                ..candidate(1, 10, Some(0.0), &[], 1, 0, 0.0)
            },
            candidate(2, 20, Some(2.0), &[], 1, 0, 225.0),
            candidate(3, 30, Some(1.0), &[], 1, 0, 225.01),
        ];
        assert_eq!(
            selected(select_target(TargetSelectorInput {
                flags: GoalSelectionFlags(2),
                candidates: &candidates,
            })),
            (CandidateSourceKey(2), NodeId(20), 2.0)
        );
    }

    #[test]
    fn nonzero_failure_and_zero_flag_fallback_do_not_commit_goal_state() {
        let candidates = [candidate(1, 10, Some(2.0), &[], 0, 0, 0.0)];
        let rejected = [SelectorCandidate {
            reserved_by_same_class_actor: true,
            ..candidates[0]
        }];
        assert_eq!(
            select_target(TargetSelectorInput {
                flags: GoalSelectionFlags(2),
                candidates: &rejected,
            }),
            TargetSelectorOutcome::NoSelection
        );
        assert_eq!(
            select_target(TargetSelectorInput {
                flags: GoalSelectionFlags(0),
                candidates: &rejected,
            }),
            TargetSelectorOutcome::RandomFallbackRequired { candidate_count: 1 }
        );

        let mut actor = ActorNavigation::default();
        actor.route_mut().goal_node = NodeId(99);
        actor.route_mut().reverse_trace_within = 3;
        actor.route_mut().reverse_trace_other = 4;
        assert!(!commit_target_selection(
            &mut actor,
            TargetSelectorOutcome::NoSelection
        ));
        assert!(!commit_target_selection(
            &mut actor,
            TargetSelectorOutcome::RandomFallbackRequired { candidate_count: 1 }
        ));
        assert_eq!(actor.route().goal_node, NodeId(99));
        assert_eq!(actor.route().reverse_trace_within, 3);
        assert_eq!(actor.route().reverse_trace_other, 4);

        let result = select_target(TargetSelectorInput {
            flags: GoalSelectionFlags(0),
            candidates: &candidates,
        });
        assert!(commit_target_selection(&mut actor, result));
        assert_eq!(actor.route().goal_node, NodeId(10));
        assert_eq!(actor.route().reverse_trace_within, 0);
        assert_eq!(actor.route().reverse_trace_other, 0);
    }
}
