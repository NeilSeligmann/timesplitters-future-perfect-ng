//! Renderer projection for the Rust-owned Assault Options menu.

use crate::assault_options::{
    ASSAULT_OPTIONS_ORDER, AssaultOption, AssaultOptionTarget, AssaultOptionsState,
};

/// Ordered action row with focus taken from the same state used by input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultOptionRow {
    option: AssaultOption,
    target: AssaultOptionTarget,
    focused: bool,
}

impl AssaultOptionRow {
    pub const fn option(self) -> AssaultOption {
        self.option
    }

    /// Generation-qualified identity for native pointer activation.
    pub const fn target(self) -> AssaultOptionTarget {
        self.target
    }

    pub const fn focused(self) -> bool {
        self.focused
    }
}

/// Project the canonical action order without inventing labels or geometry.
pub fn render_rows(state: &AssaultOptionsState) -> [AssaultOptionRow; 4] {
    ASSAULT_OPTIONS_ORDER.map(|option| AssaultOptionRow {
        option,
        target: state.target(option),
        focused: state.focused() == Some(option),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assault_options::{AssaultOptionsCommand, AssaultOptionsGeneration};

    #[test]
    fn projects_canonical_action_order_and_owned_focus() {
        let mut state = AssaultOptionsState::new(AssaultOptionsGeneration::new(4));
        state.dispatch(AssaultOptionsCommand::Focus(
            state.target(AssaultOption::Defenders),
        ));

        let rows = render_rows(&state);
        assert_eq!(rows.map(AssaultOptionRow::option), ASSAULT_OPTIONS_ORDER);
        assert!(
            rows.iter()
                .all(|row| row.target() == state.target(row.option()))
        );
        assert!(rows[2].focused());
        assert!(
            rows.iter()
                .enumerate()
                .all(|(i, row)| row.focused() == (i == 2))
        );
    }
}
