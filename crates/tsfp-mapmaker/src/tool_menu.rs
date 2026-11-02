//! Rust-owned semantic state for the Map Maker Preview/tool menu (T2408).
//!
//! T1714 measures the ordered tool rows and their enabled-row handler, but it
//! does not specify what the handler changes. This module owns focus and typed
//! requests while leaving those effects at the existing callback seam.

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolMenuChoice {
    MapmakerPreview,
    Story,
    Deathmatch,
    TeamDeathmatch,
    CaptureTheBag,
    BagTag,
    Elimination,
    Shrink,
    Vampire,
    Thief,
    Virus,
    Zones,
    Assault,
    Gladiator,
    MonkeyAssistant,
    Help,
}

/// Canonical game-type rows from T1714, followed by its separate help row.
pub const TOOL_MENU_CHOICES: [ToolMenuChoice; 15] = [
    ToolMenuChoice::MapmakerPreview,
    ToolMenuChoice::Story,
    ToolMenuChoice::Deathmatch,
    ToolMenuChoice::TeamDeathmatch,
    ToolMenuChoice::CaptureTheBag,
    ToolMenuChoice::BagTag,
    ToolMenuChoice::Elimination,
    ToolMenuChoice::Shrink,
    ToolMenuChoice::Vampire,
    ToolMenuChoice::Thief,
    ToolMenuChoice::Virus,
    ToolMenuChoice::Zones,
    ToolMenuChoice::Assault,
    ToolMenuChoice::Gladiator,
    ToolMenuChoice::MonkeyAssistant,
];

impl ToolMenuChoice {
    pub const fn game_type(self) -> Option<u32> {
        match self {
            Self::MapmakerPreview => Some(0),
            Self::Story => Some(2),
            Self::Deathmatch => Some(3),
            Self::TeamDeathmatch => Some(4),
            Self::CaptureTheBag => Some(5),
            Self::BagTag => Some(6),
            Self::Elimination => Some(7),
            Self::Shrink => Some(8),
            Self::Vampire => Some(9),
            Self::Thief => Some(10),
            Self::Virus => Some(12),
            Self::Zones => Some(15),
            Self::Assault => Some(16),
            Self::Gladiator => Some(18),
            Self::MonkeyAssistant => Some(19),
            Self::Help => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToolMenuGeneration(u64);

impl ToolMenuGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToolMenuTarget {
    generation: ToolMenuGeneration,
    choice: ToolMenuChoice,
}

impl ToolMenuTarget {
    pub const fn choice(self) -> ToolMenuChoice {
        self.choice
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMenuUpdate {
    Focused(ToolMenuChoice),
    ActionRequested(ToolMenuChoice),
    BackRequested,
}

/// Caller-owned effect request. T1714 identifies enabled rows but does not
/// establish how the original callback applies a selected mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolMenuEffect {
    pub generation: ToolMenuGeneration,
    pub update: ToolMenuUpdate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolMenuPageState {
    generation: ToolMenuGeneration,
    choices: Vec<ToolMenuChoice>,
    focused: Option<ToolMenuChoice>,
}

impl ToolMenuPageState {
    /// Install only rows whose live page has the measured enabled-row handler.
    pub fn new(generation: ToolMenuGeneration, rows: &[Option<ToolMenuChoice>]) -> Self {
        Self {
            generation,
            choices: rows.iter().flatten().copied().collect(),
            focused: None,
        }
    }

    pub const fn generation(&self) -> ToolMenuGeneration {
        self.generation
    }

    /// Qualify an update before it crosses the existing callback seam.
    pub const fn effect(&self, update: ToolMenuUpdate) -> ToolMenuEffect {
        ToolMenuEffect {
            generation: self.generation,
            update,
        }
    }

    /// Reject delayed or caller-constructed effects that no longer belong to
    /// this page opening or to one of its enabled actions.
    pub fn accepts_effect(&self, effect: ToolMenuEffect) -> bool {
        if effect.generation != self.generation {
            return false;
        }
        match effect.update {
            ToolMenuUpdate::Focused(choice) | ToolMenuUpdate::ActionRequested(choice) => {
                self.choices.contains(&choice)
            }
            ToolMenuUpdate::BackRequested => true,
        }
    }

    pub const fn focused(&self) -> Option<ToolMenuChoice> {
        self.focused
    }

    pub fn target(&self, choice: ToolMenuChoice) -> Option<ToolMenuTarget> {
        self.choices.contains(&choice).then_some(ToolMenuTarget {
            generation: self.generation,
            choice,
        })
    }

    pub fn focus(&mut self, target: ToolMenuTarget) -> Option<ToolMenuUpdate> {
        if target.generation != self.generation || !self.choices.contains(&target.choice) {
            return None;
        }
        self.focused = Some(target.choice);
        Some(ToolMenuUpdate::Focused(target.choice))
    }

    pub fn activate(&mut self, target: ToolMenuTarget) -> Option<ToolMenuUpdate> {
        self.focus(target)?;
        Some(ToolMenuUpdate::ActionRequested(target.choice))
    }

    pub const fn back(&self) -> ToolMenuUpdate {
        ToolMenuUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_game_type_rows_keep_the_original_non_contiguous_values() {
        let game_types = TOOL_MENU_CHOICES.map(|choice| choice.game_type().unwrap());
        assert_eq!(
            game_types,
            [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 16, 18, 19]
        );
        assert_eq!(ToolMenuChoice::Help.game_type(), None);
    }

    #[test]
    fn requests_are_generation_scoped_and_disabled_rows_have_no_target() {
        let rows = [
            Some(ToolMenuChoice::Story),
            None,
            Some(ToolMenuChoice::Assault),
        ];
        let generation = ToolMenuGeneration::new(8);
        let mut state = ToolMenuPageState::new(generation, &rows);
        let target = state.target(ToolMenuChoice::Assault).unwrap();
        assert_eq!(
            state.activate(target),
            Some(ToolMenuUpdate::ActionRequested(ToolMenuChoice::Assault))
        );
        assert_eq!(state.target(ToolMenuChoice::Deathmatch), None);

        let stale = ToolMenuTarget {
            generation: ToolMenuGeneration::new(7),
            choice: ToolMenuChoice::Story,
        };
        assert_eq!(state.activate(stale), None);
        assert_eq!(state.focused(), Some(ToolMenuChoice::Assault));
    }

    #[test]
    fn effect_validation_rejects_stale_pages_and_disabled_choices() {
        let state = ToolMenuPageState::new(
            ToolMenuGeneration::new(8),
            &[Some(ToolMenuChoice::Story), None],
        );
        assert!(
            state.accepts_effect(
                state.effect(ToolMenuUpdate::ActionRequested(ToolMenuChoice::Story,))
            )
        );
        assert!(!state.accepts_effect(
            state.effect(ToolMenuUpdate::ActionRequested(ToolMenuChoice::Assault,))
        ));
        assert!(!state.accepts_effect(ToolMenuEffect {
            generation: ToolMenuGeneration::new(7),
            update: ToolMenuUpdate::BackRequested,
        }));
        assert!(state.accepts_effect(state.effect(ToolMenuUpdate::BackRequested)));
    }
}
