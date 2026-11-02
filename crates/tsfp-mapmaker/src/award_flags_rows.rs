//! Renderer projection for the measured four-row Award Flags widget.

use crate::award_flags::{AwardFlagAction, AwardFlagsPageState, AwardFlagsTarget, AwardRank};

/// One measured rank row with its state-dependent action and Rust-owned focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwardFlagsRow {
    rank: AwardRank,
    target: AwardFlagsTarget,
    action: AwardFlagAction,
    focused: bool,
}

impl AwardFlagsRow {
    pub const fn rank(self) -> AwardRank {
        self.rank
    }

    pub const fn action(self) -> AwardFlagAction {
        self.action
    }

    /// Generation-qualified identity for native pointer activation.
    pub const fn target(self) -> AwardFlagsTarget {
        self.target
    }

    pub const fn focused(self) -> bool {
        self.focused
    }
}

/// Preserve the measured rank order and action table for caller rendering.
pub fn render_rows(state: &AwardFlagsPageState) -> [AwardFlagsRow; 4] {
    let actions = state.actions();
    std::array::from_fn(|index| {
        let rank = AwardRank::ALL[index];
        AwardFlagsRow {
            rank,
            target: state.target(rank),
            action: actions[index],
            focused: state.focused() == Some(rank),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::award_flags::AwardFlagsGeneration;

    #[test]
    fn projects_measured_rank_order_action_and_owned_focus() {
        let mut state = AwardFlagsPageState::new(
            AwardFlagsGeneration::new(12),
            [
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
            ],
        );
        state.focus(state.target(AwardRank::Silver));

        let rows = render_rows(&state);
        assert_eq!(
            rows.map(AwardFlagsRow::rank),
            [
                AwardRank::Platinum,
                AwardRank::Gold,
                AwardRank::Silver,
                AwardRank::Bronze
            ]
        );
        assert_eq!(rows.map(AwardFlagsRow::action), state.actions());
        assert!(
            rows.iter()
                .all(|row| row.target() == state.target(row.rank()))
        );
        assert_eq!(
            rows.map(AwardFlagsRow::focused),
            [false, false, true, false]
        );
    }
}
