//! Rust-owned interaction state for the Map Maker Select Action(s) page.
//!
//! T1714 identifies the page heading, Add New Action row and nested selector
//! handler, but does not define dynamic record identity or mutation effects.
//! This model owns the row focus and emits typed requests; those effects remain
//! at the existing callback seam until the subsystem adoption gate passes.

extern crate alloc;

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionPickerChoice {
    ExistingRow(u16),
    AddNewAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionPickerGeneration(u64);

impl ActionPickerGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActionPickerTarget {
    generation: ActionPickerGeneration,
    choice: ActionPickerChoice,
}

impl ActionPickerTarget {
    pub const fn choice(self) -> ActionPickerChoice {
        self.choice
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionPickerUpdate {
    Focused(ActionPickerChoice),
    SelectionRequested(ActionPickerChoice),
    BackRequested,
}

/// Interaction state for one opening of the dynamic action list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionPickerPageState {
    generation: ActionPickerGeneration,
    choices: Vec<ActionPickerChoice>,
    focused: Option<ActionPickerChoice>,
}

impl ActionPickerPageState {
    pub fn new(generation: ActionPickerGeneration, choices: &[ActionPickerChoice]) -> Option<Self> {
        if choices.is_empty()
            || choices
                .iter()
                .filter(|choice| **choice == ActionPickerChoice::AddNewAction)
                .count()
                != 1
        {
            return None;
        }
        Some(Self {
            generation,
            choices: choices.to_vec(),
            focused: None,
        })
    }

    pub const fn generation(&self) -> ActionPickerGeneration {
        self.generation
    }

    pub fn choices(&self) -> &[ActionPickerChoice] {
        &self.choices
    }

    pub const fn focused(&self) -> Option<ActionPickerChoice> {
        self.focused
    }

    pub fn target(&self, choice: ActionPickerChoice) -> Option<ActionPickerTarget> {
        self.choices
            .contains(&choice)
            .then_some(ActionPickerTarget {
                generation: self.generation,
                choice,
            })
    }

    pub fn focus(&mut self, target: ActionPickerTarget) -> Option<ActionPickerUpdate> {
        if target.generation != self.generation || !self.choices.contains(&target.choice) {
            return None;
        }
        self.focused = Some(target.choice);
        Some(ActionPickerUpdate::Focused(target.choice))
    }

    pub fn activate(&mut self, target: ActionPickerTarget) -> Option<ActionPickerUpdate> {
        self.focus(target)?;
        Some(ActionPickerUpdate::SelectionRequested(target.choice))
    }

    pub const fn back(&self) -> ActionPickerUpdate {
        ActionPickerUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owns_dynamic_rows_and_emits_typed_requests() {
        let choices = [
            ActionPickerChoice::ExistingRow(0),
            ActionPickerChoice::AddNewAction,
            ActionPickerChoice::ExistingRow(1),
        ];
        let mut state =
            ActionPickerPageState::new(ActionPickerGeneration::new(4), &choices).unwrap();
        let target = state.target(ActionPickerChoice::ExistingRow(1)).unwrap();
        assert_eq!(
            state.activate(target),
            Some(ActionPickerUpdate::SelectionRequested(
                ActionPickerChoice::ExistingRow(1)
            ))
        );
        let add = state.target(ActionPickerChoice::AddNewAction).unwrap();
        assert_eq!(
            state.activate(add),
            Some(ActionPickerUpdate::SelectionRequested(
                ActionPickerChoice::AddNewAction
            ))
        );
    }

    #[test]
    fn rejects_missing_add_row_and_stale_or_unknown_targets() {
        assert!(
            ActionPickerPageState::new(
                ActionPickerGeneration::new(1),
                &[ActionPickerChoice::ExistingRow(0)]
            )
            .is_none()
        );
        let mut state = ActionPickerPageState::new(
            ActionPickerGeneration::new(2),
            &[ActionPickerChoice::AddNewAction],
        )
        .unwrap();
        let stale = ActionPickerTarget {
            generation: ActionPickerGeneration::new(1),
            choice: ActionPickerChoice::AddNewAction,
        };
        assert_eq!(state.activate(stale), None);
        assert!(state.target(ActionPickerChoice::ExistingRow(0)).is_none());
    }
}
