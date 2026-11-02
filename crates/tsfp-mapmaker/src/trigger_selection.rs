//! Rust-owned semantic state for the Map Maker trigger selector (T2408).
//!
//! T1714 identifies the controls, but not their lifted callbacks or record
//! mutations. This model emits typed requests and leaves those effects at the
//! existing adapter seam.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TriggerSelectionAction {
    AddTrigger,
    MatchAll,
    MatchAny,
    Negate,
}

pub const TRIGGER_SELECTION_ACTIONS: [TriggerSelectionAction; 4] = [
    TriggerSelectionAction::AddTrigger,
    TriggerSelectionAction::MatchAll,
    TriggerSelectionAction::MatchAny,
    TriggerSelectionAction::Negate,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TriggerSelectionGeneration(u64);

impl TriggerSelectionGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TriggerSelectionTarget {
    generation: TriggerSelectionGeneration,
    action: TriggerSelectionAction,
}

impl TriggerSelectionTarget {
    pub const fn action(self) -> TriggerSelectionAction {
        self.action
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerSelectionUpdate {
    Focused(TriggerSelectionAction),
    ActionRequested(TriggerSelectionAction),
    BackRequested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriggerSelectionPageState {
    generation: TriggerSelectionGeneration,
    focused: Option<TriggerSelectionAction>,
}

impl TriggerSelectionPageState {
    pub const fn new(generation: TriggerSelectionGeneration) -> Self {
        Self {
            generation,
            focused: None,
        }
    }

    pub const fn generation(&self) -> TriggerSelectionGeneration {
        self.generation
    }
    pub const fn focused(&self) -> Option<TriggerSelectionAction> {
        self.focused
    }

    pub fn target(&self, action: TriggerSelectionAction) -> Option<TriggerSelectionTarget> {
        TRIGGER_SELECTION_ACTIONS
            .contains(&action)
            .then_some(TriggerSelectionTarget {
                generation: self.generation,
                action,
            })
    }

    pub fn focus(&mut self, target: TriggerSelectionTarget) -> Option<TriggerSelectionUpdate> {
        if target.generation != self.generation
            || !TRIGGER_SELECTION_ACTIONS.contains(&target.action)
        {
            return None;
        }
        self.focused = Some(target.action);
        Some(TriggerSelectionUpdate::Focused(target.action))
    }

    pub fn activate(&mut self, target: TriggerSelectionTarget) -> Option<TriggerSelectionUpdate> {
        self.focus(target)?;
        Some(TriggerSelectionUpdate::ActionRequested(target.action))
    }

    pub const fn back(&self) -> TriggerSelectionUpdate {
        TriggerSelectionUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_typed_and_stale_targets_are_rejected() {
        let mut state = TriggerSelectionPageState::new(TriggerSelectionGeneration::new(8));
        let target = state.target(TriggerSelectionAction::MatchAny).unwrap();
        assert_eq!(
            state.activate(target),
            Some(TriggerSelectionUpdate::ActionRequested(
                TriggerSelectionAction::MatchAny
            ))
        );
        let stale = TriggerSelectionTarget {
            generation: TriggerSelectionGeneration::new(7),
            action: TriggerSelectionAction::Negate,
        };
        assert_eq!(state.activate(stale), None);
        assert_eq!(state.focused(), Some(TriggerSelectionAction::MatchAny));
    }
}
