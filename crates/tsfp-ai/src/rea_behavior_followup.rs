// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed decision for the state-6/state-8 follow-up in the AI behavior lane.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowupAction {
    SetModeBit8,
    EnterState6 { target: u32 },
}

/// Inputs observed by original `0x26DD80`, with service calls resolved at the
/// host boundary in their original order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FollowupEvidence {
    pub action_guard_blocked: bool,
    pub actor_level: u32,
    pub current_level: u32,
    pub pair_entry_present: bool,
    pub direction_gate_passed: bool,
    pub state8_pair_selected: bool,
    pub state6_pair_selected: bool,
    pub state6_target: u32,
}

/// Resolve the bounded action after the existing guard, pair and direction
/// services have reported their original results.
pub fn decide_state6_followup(input: FollowupEvidence) -> Option<FollowupAction> {
    if input.action_guard_blocked
        || input.actor_level == input.current_level
        || !input.pair_entry_present
        || !input.direction_gate_passed
    {
        return None;
    }

    if input.state8_pair_selected {
        return Some(FollowupAction::SetModeBit8);
    }
    input
        .state6_pair_selected
        .then_some(FollowupAction::EnterState6 {
            target: input.state6_target,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> FollowupEvidence {
        FollowupEvidence {
            action_guard_blocked: false,
            actor_level: 4,
            current_level: 2,
            pair_entry_present: true,
            direction_gate_passed: true,
            state8_pair_selected: false,
            state6_pair_selected: true,
            state6_target: 0x1234,
        }
    }

    #[test]
    fn state6_followup_respects_each_admission_gate() {
        let base = evidence();
        assert_eq!(
            decide_state6_followup(base),
            Some(FollowupAction::EnterState6 { target: 0x1234 })
        );
        for rejected in [
            FollowupEvidence {
                action_guard_blocked: true,
                ..base
            },
            FollowupEvidence {
                actor_level: 2,
                ..base
            },
            FollowupEvidence {
                pair_entry_present: false,
                ..base
            },
            FollowupEvidence {
                direction_gate_passed: false,
                ..base
            },
        ] {
            assert_eq!(decide_state6_followup(rejected), None);
        }
    }

    #[test]
    fn state8_match_precedes_state6_and_no_match_is_inert() {
        let base = evidence();
        assert_eq!(
            decide_state6_followup(FollowupEvidence {
                state8_pair_selected: true,
                ..base
            }),
            Some(FollowupAction::SetModeBit8)
        );
        assert_eq!(
            decide_state6_followup(FollowupEvidence {
                state6_pair_selected: false,
                ..base
            }),
            None
        );
    }
}
