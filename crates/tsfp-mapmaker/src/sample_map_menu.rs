//! Rust-owned state for the measured Sample Map chooser (T2408).
//!
//! T1714 identifies eight named rows and two explicit zero-label rows. The
//! first zero label is a special return in the measured handler; the trailing
//! zero-label row's meaning remains unresolved. Both stay distinct here.

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SampleMapChoice {
    TrainingMap,
    ALittleHead,
    CaptureTheBag,
    CatRacingXtreme,
    CoinChallenge,
    BunkerAssault,
    BuggyTimeTrial,
    Ts1Redux,
    SpecialReturn,
    UnresolvedTrailingZeroLabel,
}

pub const SAMPLE_MAP_CHOICES: [SampleMapChoice; 10] = [
    SampleMapChoice::TrainingMap,
    SampleMapChoice::ALittleHead,
    SampleMapChoice::CaptureTheBag,
    SampleMapChoice::CatRacingXtreme,
    SampleMapChoice::CoinChallenge,
    SampleMapChoice::BunkerAssault,
    SampleMapChoice::BuggyTimeTrial,
    SampleMapChoice::Ts1Redux,
    SampleMapChoice::SpecialReturn,
    SampleMapChoice::UnresolvedTrailingZeroLabel,
];

impl SampleMapChoice {
    pub const fn label_id(self) -> u32 {
        match self {
            Self::TrainingMap => 0x771,
            Self::ALittleHead => 0x7F4,
            Self::CaptureTheBag => 0x873,
            Self::CatRacingXtreme => 0x879,
            Self::CoinChallenge => 0x899,
            Self::BunkerAssault => 0x8AE,
            Self::BuggyTimeTrial => 0x8F0,
            Self::Ts1Redux => 0x93D,
            Self::SpecialReturn | Self::UnresolvedTrailingZeroLabel => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SampleMapMenuGeneration(u64);

impl SampleMapMenuGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SampleMapMenuTarget {
    generation: SampleMapMenuGeneration,
    choice: SampleMapChoice,
}

/// Renderer row projected from the caller's current row sequence. The two
/// zero-label rows remain distinct through their Rust choice identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleMapMenuRow {
    choice: Option<SampleMapChoice>,
    focused: bool,
}

impl SampleMapMenuRow {
    pub const fn choice(self) -> Option<SampleMapChoice> {
        self.choice
    }

    pub const fn focused(self) -> bool {
        self.focused
    }
}

impl SampleMapMenuTarget {
    pub const fn choice(self) -> SampleMapChoice {
        self.choice
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleMapMenuUpdate {
    Focused(SampleMapChoice),
    SelectionRequested(SampleMapChoice),
    BackRequested,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleMapMenuState {
    generation: SampleMapMenuGeneration,
    choices: Vec<SampleMapChoice>,
    focused: Option<SampleMapChoice>,
}

impl SampleMapMenuState {
    pub fn new(generation: SampleMapMenuGeneration, rows: &[Option<SampleMapChoice>]) -> Self {
        Self {
            generation,
            choices: rows.iter().flatten().copied().collect(),
            focused: None,
        }
    }

    pub const fn generation(&self) -> SampleMapMenuGeneration {
        self.generation
    }

    pub const fn focused(&self) -> Option<SampleMapChoice> {
        self.focused
    }

    pub fn target(&self, choice: SampleMapChoice) -> Option<SampleMapMenuTarget> {
        self.choices
            .contains(&choice)
            .then_some(SampleMapMenuTarget {
                generation: self.generation,
                choice,
            })
    }

    /// Project the caller-supplied row order without changing interaction
    /// state. Rows outside this opening's choice set remain inert.
    pub fn rows(&self, rows: &[Option<SampleMapChoice>]) -> Vec<SampleMapMenuRow> {
        rows.iter()
            .map(|choice| {
                let choice = choice.filter(|choice| self.choices.contains(choice));
                SampleMapMenuRow {
                    choice,
                    focused: choice.is_some() && choice == self.focused,
                }
            })
            .collect()
    }

    pub fn focus(&mut self, target: SampleMapMenuTarget) -> Option<SampleMapMenuUpdate> {
        if target.generation != self.generation || !self.choices.contains(&target.choice) {
            return None;
        }
        self.focused = Some(target.choice);
        Some(SampleMapMenuUpdate::Focused(target.choice))
    }

    pub fn select(&mut self, target: SampleMapMenuTarget) -> Option<SampleMapMenuUpdate> {
        self.focus(target)?;
        Some(SampleMapMenuUpdate::SelectionRequested(target.choice))
    }

    pub const fn back(&self) -> SampleMapMenuUpdate {
        SampleMapMenuUpdate::BackRequested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_preserves_measured_labels_and_distinct_zero_rows() {
        assert_eq!(
            SAMPLE_MAP_CHOICES.map(SampleMapChoice::label_id),
            [0x771, 0x7F4, 0x873, 0x879, 0x899, 0x8AE, 0x8F0, 0x93D, 0, 0]
        );
        assert_ne!(SAMPLE_MAP_CHOICES[8], SAMPLE_MAP_CHOICES[9]);
    }

    #[test]
    fn selection_targets_are_generation_scoped() {
        let rows = SAMPLE_MAP_CHOICES.map(Some);
        let mut first = SampleMapMenuState::new(SampleMapMenuGeneration::new(3), &rows);
        let mut second = SampleMapMenuState::new(SampleMapMenuGeneration::new(4), &rows);
        let target = first.target(SampleMapChoice::SpecialReturn).unwrap();
        assert_eq!(
            first.select(target),
            Some(SampleMapMenuUpdate::SelectionRequested(
                SampleMapChoice::SpecialReturn
            ))
        );
        assert_eq!(second.select(target), None);
    }

    #[test]
    fn renderer_rows_preserve_duplicate_zero_labels_and_rust_focus() {
        let rows = SAMPLE_MAP_CHOICES.map(Some);
        let mut state = SampleMapMenuState::new(SampleMapMenuGeneration::new(8), &rows);
        state.focus(state.target(SampleMapChoice::SpecialReturn).unwrap());

        let projected = state.rows(&rows);
        assert_eq!(projected.len(), 10);
        assert_eq!(projected[8].choice(), Some(SampleMapChoice::SpecialReturn));
        assert!(projected[8].focused());
        assert_eq!(
            projected[9].choice(),
            Some(SampleMapChoice::UnresolvedTrailingZeroLabel)
        );
        assert!(!projected[9].focused());
        assert_eq!(
            [projected[8].choice(), projected[9].choice()],
            [
                Some(SampleMapChoice::SpecialReturn),
                Some(SampleMapChoice::UnresolvedTrailingZeroLabel)
            ]
        );
    }
}
