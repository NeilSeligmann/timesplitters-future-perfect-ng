//! Typed input ownership for the measured-static selected-entry message rows.
//!
//! The original row values and callback effects remain at the caller seam.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectedEntryField {
    MessageName,
    DisplayMessage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SelectedEntryGeneration(u64);

impl SelectedEntryGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SelectedEntryTarget {
    generation: SelectedEntryGeneration,
    field: SelectedEntryField,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedEntryUpdate {
    Focused(SelectedEntryField),
    ActivationRequested(SelectedEntryField),
    BackRequested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedEntryPageState {
    generation: SelectedEntryGeneration,
    fields: [SelectedEntryField; 2],
    focused: Option<SelectedEntryField>,
}

impl SelectedEntryPageState {
    pub const fn new(generation: SelectedEntryGeneration, fields: [SelectedEntryField; 2]) -> Self {
        Self {
            generation,
            fields,
            focused: None,
        }
    }

    pub const fn fields(self) -> [SelectedEntryField; 2] {
        self.fields
    }

    pub const fn focused(self) -> Option<SelectedEntryField> {
        self.focused
    }

    pub const fn target(&self, field: SelectedEntryField) -> SelectedEntryTarget {
        SelectedEntryTarget {
            generation: self.generation,
            field,
        }
    }

    pub fn focus(&mut self, target: SelectedEntryTarget) -> Option<SelectedEntryUpdate> {
        if target.generation != self.generation || !self.fields.contains(&target.field) {
            return None;
        }
        self.focused = Some(target.field);
        Some(SelectedEntryUpdate::Focused(target.field))
    }

    pub fn activate(&mut self, target: SelectedEntryTarget) -> Option<SelectedEntryUpdate> {
        self.focus(target)?;
        Some(SelectedEntryUpdate::ActivationRequested(target.field))
    }

    pub const fn back(&self) -> SelectedEntryUpdate {
        SelectedEntryUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELDS: [SelectedEntryField; 2] = [
        SelectedEntryField::MessageName,
        SelectedEntryField::DisplayMessage,
    ];

    #[test]
    fn owns_generation_checked_field_focus_and_activation() {
        let mut state = SelectedEntryPageState::new(SelectedEntryGeneration::new(4), FIELDS);
        let target = state.target(SelectedEntryField::DisplayMessage);
        assert_eq!(
            state.focus(target),
            Some(SelectedEntryUpdate::Focused(
                SelectedEntryField::DisplayMessage
            ))
        );
        assert_eq!(
            state.activate(target),
            Some(SelectedEntryUpdate::ActivationRequested(
                SelectedEntryField::DisplayMessage
            ))
        );
        assert_eq!(state.focused(), Some(SelectedEntryField::DisplayMessage));
    }

    #[test]
    fn rejects_stale_targets_and_keeps_back_typed() {
        let mut state = SelectedEntryPageState::new(SelectedEntryGeneration::new(5), FIELDS);
        let stale = SelectedEntryPageState::new(SelectedEntryGeneration::new(4), FIELDS)
            .target(SelectedEntryField::MessageName);
        assert_eq!(state.focus(stale), None);
        assert_eq!(state.back(), SelectedEntryUpdate::BackRequested);
    }
}
