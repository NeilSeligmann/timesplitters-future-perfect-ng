//! Rust-owned state for the T1714 Map Maker object-kind chooser.
//!
//! The original row callbacks are not established, so activation produces a
//! typed request and leaves their effects at the existing caller seam.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObjectKindChoice {
    Door,
    AutoGun,
    Camera,
    Done,
}

pub const OBJECT_KIND_CHOICES: [ObjectKindChoice; 4] = [
    ObjectKindChoice::Door,
    ObjectKindChoice::AutoGun,
    ObjectKindChoice::Camera,
    ObjectKindChoice::Done,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectKindGeneration(u64);

impl ObjectKindGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObjectKindTarget {
    generation: ObjectKindGeneration,
    choice: ObjectKindChoice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKindUpdate {
    Focused(ObjectKindChoice),
    ChoiceRequested(ObjectKindChoice),
    BackRequested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectKindPageState {
    generation: ObjectKindGeneration,
    focused: Option<ObjectKindChoice>,
}

impl ObjectKindPageState {
    pub const fn new(generation: ObjectKindGeneration) -> Self {
        Self {
            generation,
            focused: None,
        }
    }

    pub const fn generation(&self) -> ObjectKindGeneration {
        self.generation
    }

    pub const fn focused(&self) -> Option<ObjectKindChoice> {
        self.focused
    }

    pub fn target(&self, choice: ObjectKindChoice) -> Option<ObjectKindTarget> {
        OBJECT_KIND_CHOICES
            .contains(&choice)
            .then_some(ObjectKindTarget {
                generation: self.generation,
                choice,
            })
    }

    pub fn focus(&mut self, target: ObjectKindTarget) -> Option<ObjectKindUpdate> {
        if target.generation != self.generation || !OBJECT_KIND_CHOICES.contains(&target.choice) {
            return None;
        }
        self.focused = Some(target.choice);
        Some(ObjectKindUpdate::Focused(target.choice))
    }

    pub fn activate(&mut self, target: ObjectKindTarget) -> Option<ObjectKindUpdate> {
        self.focus(target)?;
        Some(ObjectKindUpdate::ChoiceRequested(target.choice))
    }

    pub const fn back(&self) -> ObjectKindUpdate {
        ObjectKindUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_choices_are_generation_scoped() {
        let mut state = ObjectKindPageState::new(ObjectKindGeneration::new(9));
        let target = state.target(ObjectKindChoice::Camera).unwrap();
        assert_eq!(
            state.activate(target),
            Some(ObjectKindUpdate::ChoiceRequested(ObjectKindChoice::Camera))
        );
        assert_eq!(state.focused(), Some(ObjectKindChoice::Camera));
        assert_eq!(state.generation().get(), 9);

        let stale = ObjectKindTarget {
            generation: ObjectKindGeneration::new(8),
            choice: ObjectKindChoice::Done,
        };
        assert_eq!(state.activate(stale), None);
    }
}
