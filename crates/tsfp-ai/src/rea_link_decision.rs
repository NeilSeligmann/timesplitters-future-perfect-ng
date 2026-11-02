// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared pure decision at the documented 0x263ED0 route-link boundary.

use crate::movement_approach::Position3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkFollowInput {
    pub route_kind: u32,
    pub behavior_route_selector: i32,
    pub animation_matches: bool,
    pub actor_position: Position3,
    pub goal_position: Position3,
    pub root_first_z: f32,
    pub root_last_z: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkFollowDecision {
    Return,
    TargetActorPosition,
    TargetGoalPosition,
    AdvanceRoute,
    AskNavigationToAdvance,
    AdvanceCurrentToGoal,
}

/// Preserve the measured route-kind gates and ordered floating-point compare.
pub fn link_follow_decision(input: LinkFollowInput) -> LinkFollowDecision {
    if input.behavior_route_selector == -2 {
        return LinkFollowDecision::Return;
    }
    match input.route_kind {
        0 => LinkFollowDecision::AdvanceRoute,
        1 => {
            let root_dz = input.root_last_z - input.root_first_z;
            let actor_dx = input.actor_position.x - input.goal_position.x;
            let actor_dz = input.actor_position.z - input.goal_position.z;
            let root_distance = root_dz * root_dz;
            let actor_distance = actor_dx * actor_dx + actor_dz * actor_dz;
            if root_distance <= actor_distance || root_distance.is_nan() || actor_distance.is_nan()
            {
                LinkFollowDecision::AskNavigationToAdvance
            } else {
                LinkFollowDecision::AdvanceCurrentToGoal
            }
        }
        _ if input.animation_matches => LinkFollowDecision::TargetGoalPosition,
        _ => LinkFollowDecision::TargetActorPosition,
    }
}
