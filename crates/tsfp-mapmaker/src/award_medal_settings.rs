//! Rust-owned interaction state for the T1714 Award Medal Settings page.
//!
//! Label identities are measured static evidence. Activation emits a typed
//! request for the existing seam; it does not implement medal mutation.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AwardMedalAction {
    AddNewTrigger,
    MainObjectivesCompleted,
    SecondaryObjectivesCompleted,
    AwardIfAllCompleted,
    AwardIfAnyCompleted,
    Done,
}

pub const AWARD_MEDAL_ACTIONS: [AwardMedalAction; 6] = [
    AwardMedalAction::AddNewTrigger,
    AwardMedalAction::MainObjectivesCompleted,
    AwardMedalAction::SecondaryObjectivesCompleted,
    AwardMedalAction::AwardIfAllCompleted,
    AwardMedalAction::AwardIfAnyCompleted,
    AwardMedalAction::Done,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardMedalGeneration(u64);

impl AwardMedalGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardMedalTarget {
    generation: AwardMedalGeneration,
    action: AwardMedalAction,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwardMedalUpdate {
    Focused(AwardMedalAction),
    ActivationRequested(AwardMedalAction),
    BackRequested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwardMedalPageState {
    generation: AwardMedalGeneration,
    focused: Option<AwardMedalAction>,
}

impl AwardMedalPageState {
    pub const fn new(generation: AwardMedalGeneration) -> Self {
        Self {
            generation,
            focused: None,
        }
    }
    pub const fn generation(&self) -> AwardMedalGeneration {
        self.generation
    }
    pub const fn focused(&self) -> Option<AwardMedalAction> {
        self.focused
    }

    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    pub fn target(&self, action: AwardMedalAction) -> AwardMedalTarget {
        AwardMedalTarget {
            generation: self.generation,
            action,
        }
    }

    pub fn focus(&mut self, target: AwardMedalTarget) -> Option<AwardMedalUpdate> {
        if target.generation != self.generation || !AWARD_MEDAL_ACTIONS.contains(&target.action) {
            return None;
        }
        self.focused = Some(target.action);
        Some(AwardMedalUpdate::Focused(target.action))
    }

    pub fn activate(&mut self, target: AwardMedalTarget) -> Option<AwardMedalUpdate> {
        self.focus(target)?;
        Some(AwardMedalUpdate::ActivationRequested(target.action))
    }

    pub const fn back(&self) -> AwardMedalUpdate {
        AwardMedalUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_and_activation_are_scoped_to_the_live_page_generation() {
        let mut state = AwardMedalPageState::new(AwardMedalGeneration::new(4));
        let target = state.target(AwardMedalAction::AwardIfAllCompleted);
        assert_eq!(
            state.activate(target),
            Some(AwardMedalUpdate::ActivationRequested(
                AwardMedalAction::AwardIfAllCompleted
            ))
        );
        assert_eq!(state.focused(), Some(AwardMedalAction::AwardIfAllCompleted));
        let stale = AwardMedalTarget {
            generation: AwardMedalGeneration::new(3),
            action: AwardMedalAction::Done,
        };
        assert_eq!(state.activate(stale), None);
    }
}
