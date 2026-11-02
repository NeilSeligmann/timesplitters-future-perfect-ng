//! Compose the measured selector stages of XBE function `0x18AF40`.
//!
//! This is a Rust-owned transition over caller-provided row/global snapshots.
//! It does not own the candidate table, transform/query services, effect pool,
//! or audio route. The type-11 route remains a marker until its full call
//! contract is recovered.

use crate::rea_actor_spawn_branch::{
    ActorSpawnLoopRoute, ActorSpawnSpatialPath, ActorSpawnTerminalDecision, actor_spawn_loop_route,
    actor_spawn_spatial_path, actor_spawn_terminal_decision,
};
use crate::rea_attachment_target::{SelectorCandidateRaw, select_nearest_raw_candidates};
use crate::rea_selected_row_adjustment::{SelectedRowAdjustment, update_actor_94};

/// Ordered raw dispatch decisions made before the 0x18AF40 candidate/terminal
/// transition. The later stack word and `arg+0x14` are independent inputs;
/// neither is inferred from the earlier spatial path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorSpawnDispatchPrefix {
    pub spatial_path: ActorSpawnSpatialPath,
    pub loop_route: ActorSpawnLoopRoute,
}

/// Compose the two measured gates in source order while leaving the intervening
/// transform/helper calls as attachment points. Values are raw snapshots from
/// the caller; this function does not inspect guest memory or call services.
pub fn plan_actor_spawn_dispatch_prefix(
    arg2_field_8c: f32,
    arg2_field_90: f32,
    arg3_is_non_null: bool,
    raw_stack_gate_word: u32,
    arg_14_nonzero: bool,
) -> ActorSpawnDispatchPrefix {
    let spatial_path = actor_spawn_spatial_path(arg2_field_8c, arg2_field_90, arg3_is_non_null);
    let loop_route = actor_spawn_loop_route(raw_stack_gate_word, arg_14_nonzero);
    ActorSpawnDispatchPrefix {
        spatial_path,
        loop_route,
    }
}

/// Bounded action chosen after the two measured 0x18AF40 dispatch gates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActorSpawnDispatchAction {
    /// Raw nonzero local selects the type-11 EA9C0 route. Its full call contract
    /// is intentionally not guessed here.
    Type11Terminal,
    /// Nonzero arg+0x14 bypasses candidate scan and reaches the ordinary
    /// terminal selector plan.
    TerminalWithoutCandidateScan(ActorSpawnTerminalDecision),
    /// Candidate scan is admitted and composed with row adjustment and the
    /// ordinary terminal selector plan.
    CandidateScan(SelectorTransitionResult),
}

/// Result of dispatching a raw snapshot through the bounded parent prefix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorSpawnDispatchResult {
    pub spatial_path: ActorSpawnSpatialPath,
    pub action: ActorSpawnDispatchAction,
}

/// Route the measured early gates into the bounded candidate/terminal pieces.
/// Candidate and adjustment snapshots are consumed only on the candidate arm;
/// all services and actor/global ownership remain at the caller boundary.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_actor_spawn_prefix(
    arg2_field_8c: f32,
    arg2_field_90: f32,
    arg3_is_non_null: bool,
    raw_stack_gate_word: u32,
    arg_14_nonzero: bool,
    candidates: impl IntoIterator<Item = SelectorCandidateRaw>,
    reference: [f32; 3],
    score_reference: [f32; 3],
    score_weights: [f32; 3],
    selector_field_94: u32,
    request_values: [f32; 3],
    adjustment_for_row: impl FnOnce(usize) -> SelectedRowAdjustment,
) -> ActorSpawnDispatchResult {
    let prefix = plan_actor_spawn_dispatch_prefix(
        arg2_field_8c,
        arg2_field_90,
        arg3_is_non_null,
        raw_stack_gate_word,
        arg_14_nonzero,
    );
    let action = match prefix.loop_route {
        ActorSpawnLoopRoute::Type11Terminal => ActorSpawnDispatchAction::Type11Terminal,
        ActorSpawnLoopRoute::TerminalWithoutCandidateScan => {
            ActorSpawnDispatchAction::TerminalWithoutCandidateScan(actor_spawn_terminal_decision(
                selector_field_94,
                request_values,
            ))
        }
        ActorSpawnLoopRoute::CandidateScan => {
            ActorSpawnDispatchAction::CandidateScan(compose_selector_transition(
                candidates,
                reference,
                score_reference,
                score_weights,
                selector_field_94,
                request_values,
                adjustment_for_row,
            ))
        }
    };
    ActorSpawnDispatchResult {
        spatial_path: prefix.spatial_path,
        action,
    }
}

/// Result of the measured candidate-update and terminal-branch ordering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectorTransitionResult {
    /// Original index of the nearest candidate that passed both filters.
    pub selected_row_index: Option<usize>,
    /// Updated raw actor field `+0x94`; absent when no candidate was selected.
    pub updated_field_94: Option<f32>,
    /// Terminal call-site decision, reached whether or not a candidate won.
    pub terminal: ActorSpawnTerminalDecision,
}

/// Compose candidate selection, selected-row adjustment, and the terminal arm.
///
/// The raw `selector_field_94` is a separate caller field checked after the
/// candidate-search sentinel branch (observed as `[EDI + 0x94]`): no candidate
/// skips only the row adjustment, then still reaches the terminal decision.
/// The adjustment callback must supply the selected row's raw vector/global
/// snapshot and original actor scalar. It is not invoked when there is no row.
#[allow(clippy::too_many_arguments)]
pub fn compose_selector_transition(
    candidates: impl IntoIterator<Item = SelectorCandidateRaw>,
    reference: [f32; 3],
    score_reference: [f32; 3],
    score_weights: [f32; 3],
    selector_field_94: u32,
    request_values: [f32; 3],
    adjustment_for_row: impl FnOnce(usize) -> SelectedRowAdjustment,
) -> SelectorTransitionResult {
    let selected_row_index =
        select_nearest_raw_candidates(candidates, reference, score_reference, score_weights);
    let updated_field_94 = selected_row_index.map(|index| {
        let adjustment = adjustment_for_row(index);
        update_actor_94(adjustment)
    });
    let terminal = actor_spawn_terminal_decision(selector_field_94, request_values);

    SelectorTransitionResult {
        selected_row_index,
        updated_field_94,
        terminal,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActorSpawnDispatchAction, compose_selector_transition, dispatch_actor_spawn_prefix,
        plan_actor_spawn_dispatch_prefix,
    };
    use crate::rea_actor_spawn_branch::{
        ActorSpawnLoopRoute, ActorSpawnSpatialPath, ActorSpawnTerminalPlan,
        actor_spawn_terminal_plan,
    };
    use crate::rea_attachment_target::SelectorCandidateRaw;
    use crate::rea_selected_row_adjustment::SelectedRowAdjustment;

    fn candidate(index: usize) -> SelectorCandidateRaw {
        SelectorCandidateRaw {
            row_index: index,
            vector_a8_ac_b0: [1.0, 0.0, 0.0],
            vector_b4_b8_bc: [0.0, 2.0, 0.0],
            field_114: 0,
        }
    }

    fn adjustment(row_b4_b8_bc: [f32; 3]) -> SelectedRowAdjustment {
        SelectedRowAdjustment {
            row_b4_b8_bc,
            local_14_18_1c: [0.0; 3],
            local_24: 0.0,
            actor_94: 0.0,
            squared_length_threshold: 0.000_000_1,
            divisor: 60.0,
            signed_scale: 1.0,
            step_scale: 1.0,
            clamp_bounds: [-1.0, 1.0],
        }
    }

    #[test]
    fn selected_row_is_adjusted_before_terminal_request_plan() {
        let result = compose_selector_transition(
            [candidate(7)],
            [0.0; 3],
            [0.0; 3],
            [1.0; 3],
            0x482,
            [1.0, 2.0, 3.0],
            |index| {
                assert_eq!(index, 7);
                adjustment([0.0, 2.0, 0.0])
            },
        );

        assert_eq!(result.selected_row_index, Some(7));
        // The selected row normalizes to dy=1 while local_24 is zero, so the
        // observed `dy > local_24` arm subtracts one step from actor+0x94.
        assert!(result.updated_field_94.unwrap() < 0.0);
        assert_eq!(result.terminal.plan.ea9c0_argument_1, 30);
        assert_eq!(result.terminal.plan.dffa0_float_argument_bits, 0x4000_0000);
        assert_eq!(result.terminal.plan.ea9c0_fourth_argument_bits, 0x4040_0000);
        assert_eq!(result.terminal.plan.loop_slot_value_bits, 0x3D4C_CCCD);
        assert_eq!(result.terminal.request.unwrap().words()[0], 0x4c4);
    }

    #[test]
    fn no_candidate_skips_adjustment_but_still_builds_terminal_arm() {
        let result =
            compose_selector_transition([], [0.0; 3], [0.0; 3], [1.0; 3], 0x482, [0.0; 3], |_| {
                panic!("no selected row means no adjustment snapshot is requested")
            });

        assert_eq!(result.selected_row_index, None);
        assert_eq!(result.updated_field_94, None);
        assert_eq!(result.terminal.plan.ea9c0_argument_1, 30);
        assert!(result.terminal.request.is_some());
    }

    #[test]
    fn terminal_branch_preserves_raw_effect_and_slot_constants_for_both_arms() {
        assert_eq!(
            actor_spawn_terminal_plan(0x482),
            ActorSpawnTerminalPlan {
                dffa0_selector: 2,
                dffa0_float_argument_bits: 0x4000_0000,
                ea9c0_argument_1: 30,
                ea9c0_fourth_argument_bits: 0x4040_0000,
                submit_sound_request: true,
                loop_slot_value_bits: 0x3D4C_CCCD,
            }
        );
        assert_eq!(
            actor_spawn_terminal_plan(0x481),
            ActorSpawnTerminalPlan {
                dffa0_selector: 0,
                dffa0_float_argument_bits: 0x4000_0000,
                ea9c0_argument_1: 4,
                ea9c0_fourth_argument_bits: 0x4040_0000,
                submit_sound_request: true,
                loop_slot_value_bits: 0x3D4C_CCCD,
            }
        );
    }

    #[test]
    fn dispatch_prefix_keeps_early_spatial_and_later_stack_gates_independent() {
        let result = plan_actor_spawn_dispatch_prefix(0.0, 0.0, false, 0, true);
        assert_eq!(result.spatial_path, ActorSpawnSpatialPath::TransformHelpers);
        assert_eq!(
            result.loop_route,
            ActorSpawnLoopRoute::TerminalWithoutCandidateScan
        );

        // A nonzero later local chooses the type-11 route even though the
        // earlier spatial gate independently chooses the arg3 bypass path.
        let result = plan_actor_spawn_dispatch_prefix(0.0, 0.0, true, 2, false);
        assert_eq!(result.spatial_path, ActorSpawnSpatialPath::Arg3Bypass);
        assert_eq!(result.loop_route, ActorSpawnLoopRoute::Type11Terminal);
    }

    #[test]
    fn dispatch_uses_candidates_only_on_the_candidate_scan_route() {
        let terminal = dispatch_actor_spawn_prefix(
            0.0,
            0.0,
            false,
            0,
            true,
            [],
            [0.0; 3],
            [0.0; 3],
            [1.0; 3],
            0x482,
            [1.0, 2.0, 3.0],
            |_| panic!("terminal route must not request a candidate adjustment"),
        );
        assert!(matches!(
            terminal.action,
            ActorSpawnDispatchAction::TerminalWithoutCandidateScan(_)
        ));

        let type11 = dispatch_actor_spawn_prefix(
            0.0,
            0.0,
            true,
            1,
            false,
            [],
            [0.0; 3],
            [0.0; 3],
            [1.0; 3],
            0x482,
            [1.0, 2.0, 3.0],
            |_| panic!("type-11 route must not request a candidate adjustment"),
        );
        assert_eq!(type11.action, ActorSpawnDispatchAction::Type11Terminal);

        let candidate_route = dispatch_actor_spawn_prefix(
            1.0,
            1.0,
            true,
            0,
            false,
            [candidate(7)],
            [0.0; 3],
            [0.0; 3],
            [1.0; 3],
            0x482,
            [1.0, 2.0, 3.0],
            |_| adjustment([0.0, 2.0, 0.0]),
        );
        let ActorSpawnDispatchAction::CandidateScan(transition) = candidate_route.action else {
            panic!("zero stack gate and arg+0x14 must enter candidate scan");
        };
        assert_eq!(transition.selected_row_index, Some(7));
    }
}
