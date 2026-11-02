//! Rust-owned selection state for the Assault phase start-point child page.
//!
//! The source row set and its assignment callback are not specified by the
//! available REA findings. Callers supply opaque per-opening identities and
//! retain the mapping to their native document records outside this model.

extern crate alloc;

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StartPointChoiceId(u64);

impl StartPointChoiceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StartPointPickerGeneration(u64);

impl StartPointPickerGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StartPointPickerTarget {
    generation: StartPointPickerGeneration,
    choice: StartPointChoiceId,
}

impl StartPointPickerTarget {
    pub const fn choice(self) -> StartPointChoiceId {
        self.choice
    }

    pub const fn generation(self) -> StartPointPickerGeneration {
        self.generation
    }
}

/// Renderer-facing row identity and focus for one picker generation.
/// Labels, geometry, and document assignment remain with the page adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartPointPickerRow {
    index: usize,
    target: StartPointPickerTarget,
    focused: bool,
}

impl StartPointPickerRow {
    pub const fn index(self) -> usize {
        self.index
    }

    pub const fn target(self) -> StartPointPickerTarget {
        self.target
    }

    pub const fn focused(self) -> bool {
        self.focused
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartPointPickerCommand {
    Focus(StartPointPickerTarget),
    Move {
        generation: StartPointPickerGeneration,
        delta: i32,
    },
    Activate(StartPointPickerTarget),
    Back(StartPointPickerGeneration),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartPointPickerUpdate {
    Ignored,
    Focused(StartPointChoiceId),
    SelectionRequested(StartPointChoiceId),
    BackRequested,
}

/// Owned interaction state for one opening of the dynamic child list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartPointPickerState {
    generation: StartPointPickerGeneration,
    choices: Vec<StartPointChoiceId>,
    focused: Option<StartPointChoiceId>,
    wrap: bool,
}

impl StartPointPickerState {
    /// Reject duplicate identities; empty runtime lists remain representable.
    pub fn new(
        generation: StartPointPickerGeneration,
        choices: &[StartPointChoiceId],
        wrap: bool,
    ) -> Option<Self> {
        if choices
            .iter()
            .enumerate()
            .any(|(index, choice)| choices[..index].contains(choice))
        {
            return None;
        }
        Some(Self {
            generation,
            choices: choices.to_vec(),
            focused: None,
            wrap,
        })
    }

    pub const fn generation(&self) -> StartPointPickerGeneration {
        self.generation
    }

    pub fn choices(&self) -> &[StartPointChoiceId] {
        &self.choices
    }

    /// Ordered renderer rows built from the same owned identities used by
    /// keyboard/controller focus and generation-checked activation.
    pub fn rows(&self) -> impl Iterator<Item = StartPointPickerRow> + '_ {
        let generation = self.generation;
        let focused = self.focused;
        self.choices
            .iter()
            .copied()
            .enumerate()
            .map(move |(index, choice)| StartPointPickerRow {
                index,
                target: StartPointPickerTarget { generation, choice },
                focused: focused == Some(choice),
            })
    }

    pub const fn focused(&self) -> Option<StartPointChoiceId> {
        self.focused
    }

    pub fn target(&self, choice: StartPointChoiceId) -> Option<StartPointPickerTarget> {
        self.choices
            .contains(&choice)
            .then_some(StartPointPickerTarget {
                generation: self.generation,
                choice,
            })
    }

    pub fn target_at(&self, index: usize) -> Option<StartPointPickerTarget> {
        self.choices
            .get(index)
            .copied()
            .and_then(|choice| self.target(choice))
    }

    pub fn dispatch(&mut self, command: StartPointPickerCommand) -> StartPointPickerUpdate {
        match command {
            StartPointPickerCommand::Focus(target) if self.admits(target) => {
                self.focused = Some(target.choice);
                StartPointPickerUpdate::Focused(target.choice)
            }
            StartPointPickerCommand::Activate(target) if self.admits(target) => {
                self.focused = Some(target.choice);
                StartPointPickerUpdate::SelectionRequested(target.choice)
            }
            StartPointPickerCommand::Move { generation, delta }
                if generation == self.generation && delta != 0 && !self.choices.is_empty() =>
            {
                let current = self
                    .focused
                    .and_then(|choice| self.choices.iter().position(|row| *row == choice));
                let next = match current {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta < 0 => Some(self.choices.len() - 1),
                    None => Some(0),
                };
                let next = match next {
                    Some(index) if index < self.choices.len() => Some(index),
                    _ if self.wrap => Some(if delta < 0 { self.choices.len() - 1 } else { 0 }),
                    _ => None,
                };
                next.map_or(StartPointPickerUpdate::Ignored, |index| {
                    let choice = self.choices[index];
                    self.focused = Some(choice);
                    StartPointPickerUpdate::Focused(choice)
                })
            }
            StartPointPickerCommand::Back(generation) if generation == self.generation => {
                StartPointPickerUpdate::BackRequested
            }
            _ => StartPointPickerUpdate::Ignored,
        }
    }

    fn admits(&self, target: StartPointPickerTarget) -> bool {
        target.generation == self.generation && self.choices.contains(&target.choice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> [StartPointChoiceId; 2] {
        [
            StartPointChoiceId::new(0xabc),
            StartPointChoiceId::new(0xdef),
        ]
    }

    #[test]
    fn preserves_opaque_rows_and_generation_qualifies_activation() {
        let [first, second] = ids();
        let mut state =
            StartPointPickerState::new(StartPointPickerGeneration::new(4), &[first, second], false)
                .unwrap();
        let target = state.target(second).unwrap();
        assert_eq!(target.choice(), second);
        assert_eq!(
            state.dispatch(StartPointPickerCommand::Activate(target)),
            StartPointPickerUpdate::SelectionRequested(second)
        );

        let stale =
            StartPointPickerState::new(StartPointPickerGeneration::new(3), &[first, second], false)
                .unwrap()
                .target(second)
                .unwrap();
        assert_eq!(
            state.dispatch(StartPointPickerCommand::Activate(stale)),
            StartPointPickerUpdate::Ignored
        );
    }

    #[test]
    fn rejects_duplicate_ids_and_keeps_empty_lists_safe() {
        let [first, _] = ids();
        assert!(
            StartPointPickerState::new(StartPointPickerGeneration::new(1), &[first, first], false,)
                .is_none()
        );

        let generation = StartPointPickerGeneration::new(2);
        let mut empty = StartPointPickerState::new(generation, &[], true).unwrap();
        assert_eq!(
            empty.dispatch(StartPointPickerCommand::Move {
                generation,
                delta: 1,
            }),
            StartPointPickerUpdate::Ignored
        );
        assert_eq!(empty.rows().count(), 0);
    }

    #[test]
    fn renderer_rows_keep_order_focus_and_generation_qualified_targets() {
        let [first, second] = ids();
        let generation = StartPointPickerGeneration::new(9);
        let mut state = StartPointPickerState::new(generation, &[first, second], false).unwrap();
        let second_target = state.target(second).unwrap();
        assert_eq!(
            state.dispatch(StartPointPickerCommand::Focus(second_target)),
            StartPointPickerUpdate::Focused(second)
        );

        let rows: Vec<_> = state.rows().collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].index(), 0);
        assert_eq!(rows[0].target().choice(), first);
        assert_eq!(rows[0].target().generation(), generation);
        assert!(!rows[0].focused());
        assert_eq!(rows[1].index(), 1);
        assert_eq!(rows[1].target().choice(), second);
        assert!(rows[1].focused());
    }
}
