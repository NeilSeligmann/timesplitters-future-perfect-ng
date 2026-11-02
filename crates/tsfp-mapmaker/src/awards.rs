//! Rust-owned state for the T1714 Map Maker Awards chooser.
//!
//! T1714 identifies the two award rows and Done, but not their callback
//! effects. This model emits typed requests and leaves those effects at the
//! existing adapter seam.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AwardsChoice {
    StoryAwards,
    AssaultAwards,
    Done,
}

pub const AWARDS_CHOICES: [AwardsChoice; 3] = [
    AwardsChoice::StoryAwards,
    AwardsChoice::AssaultAwards,
    AwardsChoice::Done,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardsGeneration(u64);

impl AwardsGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AwardsTarget {
    generation: AwardsGeneration,
    choice: AwardsChoice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AwardsUpdate {
    Focused(AwardsChoice),
    ChoiceRequested(AwardsChoice),
    BackRequested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AwardsPageState {
    generation: AwardsGeneration,
    focused: Option<AwardsChoice>,
}

impl AwardsPageState {
    pub const fn new(generation: AwardsGeneration) -> Self {
        Self {
            generation,
            focused: None,
        }
    }

    pub const fn generation(&self) -> AwardsGeneration {
        self.generation
    }

    pub const fn focused(&self) -> Option<AwardsChoice> {
        self.focused
    }

    pub fn target(&self, choice: AwardsChoice) -> Option<AwardsTarget> {
        AWARDS_CHOICES.contains(&choice).then_some(AwardsTarget {
            generation: self.generation,
            choice,
        })
    }

    pub fn focus(&mut self, target: AwardsTarget) -> Option<AwardsUpdate> {
        if target.generation != self.generation || !AWARDS_CHOICES.contains(&target.choice) {
            return None;
        }
        self.focused = Some(target.choice);
        Some(AwardsUpdate::Focused(target.choice))
    }

    pub fn activate(&mut self, target: AwardsTarget) -> Option<AwardsUpdate> {
        self.focus(target)?;
        Some(AwardsUpdate::ChoiceRequested(target.choice))
    }

    pub const fn back(&self) -> AwardsUpdate {
        AwardsUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_typed_and_stale_targets_are_rejected() {
        let mut state = AwardsPageState::new(AwardsGeneration::new(12));
        let target = state.target(AwardsChoice::AssaultAwards).unwrap();
        assert_eq!(
            state.activate(target),
            Some(AwardsUpdate::ChoiceRequested(AwardsChoice::AssaultAwards))
        );
        let stale = AwardsTarget {
            generation: AwardsGeneration::new(11),
            choice: AwardsChoice::Done,
        };
        assert_eq!(state.activate(stale), None);
        assert_eq!(state.focused(), Some(AwardsChoice::AssaultAwards));
        assert_eq!(state.generation().get(), 12);
    }
}
