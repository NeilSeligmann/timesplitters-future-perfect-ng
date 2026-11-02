//! Rust-owned interaction state for the Assault phase start-point page (T2408).
//!
//! T1714 measures the page builder and its action labels. A selected child
//! choice is retained as an opaque Rust-owned value; mapping it to the
//! original document record remains with the caller until that contract is
//! measured.

use crate::assault_start_point_picker::StartPointChoiceId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssaultPhaseAssignmentAction {
    SelectStartPoint,
    Done,
}

pub const ASSAULT_PHASE_ASSIGNMENT_ACTIONS: [AssaultPhaseAssignmentAction; 2] = [
    AssaultPhaseAssignmentAction::SelectStartPoint,
    AssaultPhaseAssignmentAction::Done,
];

/// Ordered renderer row from the same action set used by native input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseAssignmentRow {
    action: AssaultPhaseAssignmentAction,
    focused: bool,
}

impl AssaultPhaseAssignmentRow {
    pub const fn action(self) -> AssaultPhaseAssignmentAction {
        self.action
    }

    pub const fn focused(self) -> bool {
        self.focused
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultPhaseAssignmentGeneration(u64);

impl AssaultPhaseAssignmentGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultPhaseAssignmentTarget {
    generation: AssaultPhaseAssignmentGeneration,
    action: AssaultPhaseAssignmentAction,
}

impl AssaultPhaseAssignmentTarget {
    pub const fn action(self) -> AssaultPhaseAssignmentAction {
        self.action
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseAssignmentCommand {
    Focus(AssaultPhaseAssignmentTarget),
    Move {
        generation: AssaultPhaseAssignmentGeneration,
        delta: i32,
    },
    Activate(AssaultPhaseAssignmentTarget),
    Back(AssaultPhaseAssignmentGeneration),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseAssignmentUpdate {
    Ignored,
    Focused(AssaultPhaseAssignmentAction),
    StartPointSelectionRequested,
    StartPointAssigned(StartPointChoiceId),
    StartPointSelectionCancelled,
    BackRequested,
}

/// Interaction state for one opening of the measured `0x3190C0` page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseAssignmentState {
    generation: AssaultPhaseAssignmentGeneration,
    focused: Option<AssaultPhaseAssignmentAction>,
    selected_start_point: Option<StartPointChoiceId>,
    awaiting_start_point_selection: bool,
    wrap: bool,
}

impl AssaultPhaseAssignmentState {
    pub const fn new(generation: AssaultPhaseAssignmentGeneration, wrap: bool) -> Self {
        Self {
            generation,
            focused: None,
            selected_start_point: None,
            awaiting_start_point_selection: false,
            wrap,
        }
    }

    pub const fn generation(self) -> AssaultPhaseAssignmentGeneration {
        self.generation
    }

    pub const fn focused(self) -> Option<AssaultPhaseAssignmentAction> {
        self.focused
    }

    /// Project renderer rows without changing focus or assignment state.
    pub fn rows(self) -> impl Iterator<Item = AssaultPhaseAssignmentRow> {
        ASSAULT_PHASE_ASSIGNMENT_ACTIONS
            .into_iter()
            .map(move |action| AssaultPhaseAssignmentRow {
                action,
                focused: self.focused == Some(action),
            })
    }

    pub const fn selected_start_point(self) -> Option<StartPointChoiceId> {
        self.selected_start_point
    }

    pub const fn awaiting_start_point_selection(self) -> bool {
        self.awaiting_start_point_selection
    }

    pub const fn target(
        self,
        action: AssaultPhaseAssignmentAction,
    ) -> AssaultPhaseAssignmentTarget {
        AssaultPhaseAssignmentTarget {
            generation: self.generation,
            action,
        }
    }

    pub fn dispatch(
        &mut self,
        command: AssaultPhaseAssignmentCommand,
    ) -> AssaultPhaseAssignmentUpdate {
        match command {
            AssaultPhaseAssignmentCommand::Focus(target) if self.admits(target) => {
                self.focused = Some(target.action);
                AssaultPhaseAssignmentUpdate::Focused(target.action)
            }
            AssaultPhaseAssignmentCommand::Move { generation, delta }
                if generation == self.generation && delta != 0 =>
            {
                let current = self.focused.and_then(|focused| {
                    ASSAULT_PHASE_ASSIGNMENT_ACTIONS
                        .iter()
                        .position(|a| *a == focused)
                });
                let next = match current {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta < 0 => Some(ASSAULT_PHASE_ASSIGNMENT_ACTIONS.len() - 1),
                    None => Some(0),
                };
                let next = match next {
                    Some(index) if index < ASSAULT_PHASE_ASSIGNMENT_ACTIONS.len() => Some(index),
                    _ if self.wrap => Some(if delta < 0 {
                        ASSAULT_PHASE_ASSIGNMENT_ACTIONS.len() - 1
                    } else {
                        0
                    }),
                    _ => None,
                };
                next.map_or(AssaultPhaseAssignmentUpdate::Ignored, |index| {
                    let action = ASSAULT_PHASE_ASSIGNMENT_ACTIONS[index];
                    self.focused = Some(action);
                    AssaultPhaseAssignmentUpdate::Focused(action)
                })
            }
            AssaultPhaseAssignmentCommand::Activate(target) if self.admits(target) => {
                self.focused = Some(target.action);
                match target.action {
                    AssaultPhaseAssignmentAction::SelectStartPoint => {
                        self.awaiting_start_point_selection = true;
                        AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
                    }
                    AssaultPhaseAssignmentAction::Done => {
                        AssaultPhaseAssignmentUpdate::BackRequested
                    }
                }
            }
            AssaultPhaseAssignmentCommand::Back(generation) if generation == self.generation => {
                AssaultPhaseAssignmentUpdate::BackRequested
            }
            _ => AssaultPhaseAssignmentUpdate::Ignored,
        }
    }

    fn admits(self, target: AssaultPhaseAssignmentTarget) -> bool {
        target.generation == self.generation
            && ASSAULT_PHASE_ASSIGNMENT_ACTIONS.contains(&target.action)
    }

    pub(crate) fn assign_start_point(
        &mut self,
        choice: StartPointChoiceId,
    ) -> AssaultPhaseAssignmentUpdate {
        if !self.awaiting_start_point_selection {
            return AssaultPhaseAssignmentUpdate::Ignored;
        }
        self.selected_start_point = Some(choice);
        self.awaiting_start_point_selection = false;
        AssaultPhaseAssignmentUpdate::StartPointAssigned(choice)
    }

    pub(crate) fn cancel_start_point_selection(&mut self) -> AssaultPhaseAssignmentUpdate {
        if !self.awaiting_start_point_selection {
            return AssaultPhaseAssignmentUpdate::Ignored;
        }
        self.awaiting_start_point_selection = false;
        AssaultPhaseAssignmentUpdate::StartPointSelectionCancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_rows_follow_typed_order_and_current_focus() {
        let mut state =
            AssaultPhaseAssignmentState::new(AssaultPhaseAssignmentGeneration::new(12), false);
        state.dispatch(AssaultPhaseAssignmentCommand::Focus(
            state.target(AssaultPhaseAssignmentAction::Done),
        ));
        let rows: Vec<_> = state.rows().collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].action(),
            AssaultPhaseAssignmentAction::SelectStartPoint
        );
        assert!(!rows[0].focused());
        assert_eq!(rows[1].action(), AssaultPhaseAssignmentAction::Done);
        assert!(rows[1].focused());
    }

    #[test]
    fn start_point_and_done_are_distinct_typed_requests() {
        let mut state =
            AssaultPhaseAssignmentState::new(AssaultPhaseAssignmentGeneration::new(4), false);
        assert_eq!(
            state.dispatch(AssaultPhaseAssignmentCommand::Activate(
                state.target(AssaultPhaseAssignmentAction::SelectStartPoint)
            )),
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );
        assert_eq!(
            state.dispatch(AssaultPhaseAssignmentCommand::Activate(
                state.target(AssaultPhaseAssignmentAction::Done)
            )),
            AssaultPhaseAssignmentUpdate::BackRequested
        );
    }

    #[test]
    fn focus_steps_are_bounded_or_wrap_as_supplied_by_the_page() {
        let generation = AssaultPhaseAssignmentGeneration::new(5);
        let mut bounded = AssaultPhaseAssignmentState::new(generation, false);
        assert_eq!(
            bounded.dispatch(AssaultPhaseAssignmentCommand::Move {
                generation,
                delta: -1
            }),
            AssaultPhaseAssignmentUpdate::Focused(AssaultPhaseAssignmentAction::Done)
        );
        assert_eq!(
            bounded.dispatch(AssaultPhaseAssignmentCommand::Move {
                generation,
                delta: 1
            }),
            AssaultPhaseAssignmentUpdate::Ignored
        );
        assert_eq!(
            bounded.dispatch(AssaultPhaseAssignmentCommand::Move {
                generation,
                delta: -1
            }),
            AssaultPhaseAssignmentUpdate::Focused(AssaultPhaseAssignmentAction::SelectStartPoint)
        );
        assert_eq!(
            bounded.dispatch(AssaultPhaseAssignmentCommand::Move {
                generation,
                delta: -1
            }),
            AssaultPhaseAssignmentUpdate::Ignored
        );

        let mut wrapped = AssaultPhaseAssignmentState::new(generation, true);
        assert_eq!(
            wrapped.dispatch(AssaultPhaseAssignmentCommand::Move {
                generation,
                delta: -1
            }),
            AssaultPhaseAssignmentUpdate::Focused(AssaultPhaseAssignmentAction::Done)
        );
    }

    #[test]
    fn stale_page_targets_are_ignored() {
        let mut state =
            AssaultPhaseAssignmentState::new(AssaultPhaseAssignmentGeneration::new(8), false);
        let stale = AssaultPhaseAssignmentTarget {
            generation: AssaultPhaseAssignmentGeneration::new(7),
            action: AssaultPhaseAssignmentAction::SelectStartPoint,
        };
        assert_eq!(
            state.dispatch(AssaultPhaseAssignmentCommand::Activate(stale)),
            AssaultPhaseAssignmentUpdate::Ignored
        );
    }

    #[test]
    fn selected_choice_is_owned_after_a_requested_child_selection() {
        use crate::assault_start_point_picker::StartPointChoiceId;

        let generation = AssaultPhaseAssignmentGeneration::new(7);
        let mut state = AssaultPhaseAssignmentState::new(generation, false);
        assert_eq!(
            state.dispatch(AssaultPhaseAssignmentCommand::Activate(
                state.target(AssaultPhaseAssignmentAction::SelectStartPoint)
            )),
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );
        let choice = StartPointChoiceId::new(0xabc);
        assert_eq!(
            state.assign_start_point(choice),
            AssaultPhaseAssignmentUpdate::StartPointAssigned(choice)
        );
        assert_eq!(state.selected_start_point(), Some(choice));
        assert!(!state.awaiting_start_point_selection());
        assert_eq!(
            state.assign_start_point(choice),
            AssaultPhaseAssignmentUpdate::Ignored
        );
    }

    #[test]
    fn cancelled_child_selection_keeps_the_previous_choice() {
        use crate::assault_start_point_picker::StartPointChoiceId;

        let generation = AssaultPhaseAssignmentGeneration::new(8);
        let mut state = AssaultPhaseAssignmentState::new(generation, false);
        let previous = StartPointChoiceId::new(0xabc);
        assert_eq!(
            state.dispatch(AssaultPhaseAssignmentCommand::Activate(
                state.target(AssaultPhaseAssignmentAction::SelectStartPoint)
            )),
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );
        assert_eq!(
            state.assign_start_point(previous),
            AssaultPhaseAssignmentUpdate::StartPointAssigned(previous)
        );
        assert_eq!(
            state.dispatch(AssaultPhaseAssignmentCommand::Activate(
                state.target(AssaultPhaseAssignmentAction::SelectStartPoint)
            )),
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );
        assert_eq!(
            state.cancel_start_point_selection(),
            AssaultPhaseAssignmentUpdate::StartPointSelectionCancelled
        );
        assert_eq!(state.selected_start_point(), Some(previous));
        assert!(!state.awaiting_start_point_selection());
    }
}
