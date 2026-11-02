//! Rust-owned interaction state for the T1714 kind-specific map picker.
//!
//! T1714 establishes three prompt identities, but does not establish the
//! selected map record or the callback's selection effects. Those remain at
//! the existing typed caller seam.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KindPickerChoice {
    SelectObjectFromMap,
    SelectPickupFromMap,
    SelectDoorFromMap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KindPickerGeneration(u64);

impl KindPickerGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KindPickerTarget {
    generation: KindPickerGeneration,
    choice: KindPickerChoice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindPickerUpdate {
    Focused(KindPickerChoice),
    SelectionRequested(KindPickerChoice),
    BackRequested,
}

/// Interaction state for the single measured prompt row on one page opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindPickerPageState {
    generation: KindPickerGeneration,
    choice: KindPickerChoice,
    focused: bool,
}

impl KindPickerPageState {
    pub const fn new(generation: KindPickerGeneration, choice: KindPickerChoice) -> Self {
        Self {
            generation,
            choice,
            focused: false,
        }
    }

    pub const fn choice(self) -> KindPickerChoice {
        self.choice
    }

    pub const fn is_focused(self) -> bool {
        self.focused
    }

    pub const fn target(&self) -> KindPickerTarget {
        KindPickerTarget {
            generation: self.generation,
            choice: self.choice,
        }
    }

    pub fn focus(&mut self, target: KindPickerTarget) -> Option<KindPickerUpdate> {
        if target.generation != self.generation || target.choice != self.choice {
            return None;
        }
        self.focused = true;
        Some(KindPickerUpdate::Focused(self.choice))
    }

    pub fn activate(&mut self, target: KindPickerTarget) -> Option<KindPickerUpdate> {
        self.focus(target)?;
        Some(KindPickerUpdate::SelectionRequested(self.choice))
    }

    pub const fn back(&self) -> KindPickerUpdate {
        KindPickerUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owns_measured_prompt_and_emits_typed_requests() {
        let mut state = KindPickerPageState::new(
            KindPickerGeneration::new(7),
            KindPickerChoice::SelectPickupFromMap,
        );
        let target = state.target();
        assert_eq!(
            state.focus(target),
            Some(KindPickerUpdate::Focused(
                KindPickerChoice::SelectPickupFromMap
            ))
        );
        assert_eq!(
            state.activate(target),
            Some(KindPickerUpdate::SelectionRequested(
                KindPickerChoice::SelectPickupFromMap
            ))
        );
        assert!(state.is_focused());
    }

    #[test]
    fn rejects_stale_target_and_keeps_back_typed() {
        let mut state = KindPickerPageState::new(
            KindPickerGeneration::new(9),
            KindPickerChoice::SelectDoorFromMap,
        );
        let stale = KindPickerPageState::new(
            KindPickerGeneration::new(8),
            KindPickerChoice::SelectDoorFromMap,
        )
        .target();
        assert_eq!(state.focus(stale), None);
        assert_eq!(state.activate(stale), None);
        assert_eq!(state.back(), KindPickerUpdate::BackRequested);
    }
}
