//! Rust-owned interaction state for T1714's four-row Award Flags widget.
//!
//! T1714 measures the rank order and the state-dependent View/Edit versus Add
//! labels. The game effects behind those choices remain at the typed callback
//! seam until their data contract is established.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardFlagsGeneration(u64);

impl AwardFlagsGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AwardRank {
    Platinum,
    Gold,
    Silver,
    Bronze,
}

impl AwardRank {
    pub const ALL: [Self; 4] = [Self::Platinum, Self::Gold, Self::Silver, Self::Bronze];

    pub const fn at(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Platinum),
            1 => Some(Self::Gold),
            2 => Some(Self::Silver),
            3 => Some(Self::Bronze),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AwardFlagAction {
    ViewOrEdit,
    Add,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardFlagsTarget {
    generation: AwardFlagsGeneration,
    rank: AwardRank,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwardFlagsUpdate {
    Focused(AwardRank),
    SelectionRequested {
        rank: AwardRank,
        action: AwardFlagAction,
    },
    BackRequested,
}

/// State for one opening of the original four-row Award Flags widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwardFlagsPageState {
    generation: AwardFlagsGeneration,
    actions: [AwardFlagAction; 4],
    focused: Option<AwardRank>,
}

impl AwardFlagsPageState {
    pub const fn new(generation: AwardFlagsGeneration, actions: [AwardFlagAction; 4]) -> Self {
        Self {
            generation,
            actions,
            focused: None,
        }
    }

    pub const fn actions(self) -> [AwardFlagAction; 4] {
        self.actions
    }

    pub const fn focused(self) -> Option<AwardRank> {
        self.focused
    }

    pub const fn target(&self, rank: AwardRank) -> AwardFlagsTarget {
        AwardFlagsTarget {
            generation: self.generation,
            rank,
        }
    }

    pub fn focus(&mut self, target: AwardFlagsTarget) -> Option<AwardFlagsUpdate> {
        if target.generation != self.generation {
            return None;
        }
        self.focused = Some(target.rank);
        Some(AwardFlagsUpdate::Focused(target.rank))
    }

    pub fn activate(&mut self, target: AwardFlagsTarget) -> Option<AwardFlagsUpdate> {
        self.focus(target)?;
        let index = AwardRank::ALL
            .iter()
            .position(|rank| *rank == target.rank)?;
        Some(AwardFlagsUpdate::SelectionRequested {
            rank: target.rank,
            action: self.actions[index],
        })
    }

    pub const fn back(&self) -> AwardFlagsUpdate {
        AwardFlagsUpdate::BackRequested
    }

    /// Check a caller-held batch before applying its effects. Page generation
    /// and row actions must still match this state snapshot.
    pub fn validates_updates(
        &self,
        generation: AwardFlagsGeneration,
        updates: &[AwardFlagsUpdate],
    ) -> bool {
        if generation != self.generation || updates.is_empty() {
            return false;
        }
        let mut focused = self.focused;
        updates.iter().all(|update| match update {
            AwardFlagsUpdate::Focused(rank) => {
                focused = Some(*rank);
                true
            }
            AwardFlagsUpdate::BackRequested => true,
            AwardFlagsUpdate::SelectionRequested { rank, action } => {
                focused == Some(*rank)
                    && AwardRank::ALL
                        .iter()
                        .position(|candidate| candidate == rank)
                        .is_some_and(|index| self.actions[index] == *action)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_uses_owned_rank_and_measured_label_action() {
        let mut state = AwardFlagsPageState::new(
            AwardFlagsGeneration::new(9),
            [
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
            ],
        );
        assert_eq!(
            state.activate(state.target(AwardRank::Gold)),
            Some(AwardFlagsUpdate::SelectionRequested {
                rank: AwardRank::Gold,
                action: AwardFlagAction::Add,
            })
        );
        assert_eq!(state.focused(), Some(AwardRank::Gold));

        let stale = AwardFlagsPageState::new(AwardFlagsGeneration::new(8), state.actions())
            .target(AwardRank::Gold);
        assert_eq!(state.focus(stale), None);
        assert_eq!(state.back(), AwardFlagsUpdate::BackRequested);
    }

    #[test]
    fn update_batch_validation_rejects_stale_generation_and_changed_action() {
        let state = AwardFlagsPageState::new(
            AwardFlagsGeneration::new(9),
            [
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
            ],
        );
        let updates = [
            AwardFlagsUpdate::Focused(AwardRank::Gold),
            AwardFlagsUpdate::SelectionRequested {
                rank: AwardRank::Gold,
                action: AwardFlagAction::Add,
            },
        ];
        assert!(state.validates_updates(AwardFlagsGeneration::new(9), &updates));
        assert!(!state.validates_updates(AwardFlagsGeneration::new(10), &updates));

        let wrong_action = [
            AwardFlagsUpdate::Focused(AwardRank::Gold),
            AwardFlagsUpdate::SelectionRequested {
                rank: AwardRank::Gold,
                action: AwardFlagAction::ViewOrEdit,
            },
        ];
        assert!(!state.validates_updates(AwardFlagsGeneration::new(9), &wrong_action));
        let unfocused_selection = [AwardFlagsUpdate::SelectionRequested {
            rank: AwardRank::Gold,
            action: AwardFlagAction::Add,
        }];
        assert!(!state.validates_updates(AwardFlagsGeneration::new(9), &unfocused_selection));
        assert!(state.validates_updates(
            AwardFlagsGeneration::new(9),
            &[
                AwardFlagsUpdate::Focused(AwardRank::Gold),
                unfocused_selection[0],
            ]
        ));
        assert!(!state.validates_updates(AwardFlagsGeneration::new(9), &[]));
    }
}
