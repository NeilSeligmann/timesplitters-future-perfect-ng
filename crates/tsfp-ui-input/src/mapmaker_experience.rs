//! Typed input bridge for the measured Beginner/Advanced prompt rows.
//!
//! The row order is supplied by the live prompt: Beginner is row 0 and
//! Advanced is row 1. The editor experience stays separate from playable mode
//! and preview-row identities.

use crate::native_page::{NativeCommand, Outcome};
use tsfp_mapmaker::ui::{EditorExperience, MapMakerUiState};

/// Apply one activated row from the Beginner/Advanced prompt to owned UI state.
/// Unknown rows leave the current state untouched.
pub fn select_prompt_row(state: &mut MapMakerUiState, row: u32) -> bool {
    let experience = match row {
        0 => EditorExperience::Beginner,
        1 => EditorExperience::Advanced,
        _ => return false,
    };
    state.set_experience(experience);
    true
}

/// Track native page focus and record an activated prompt row from keyboard,
/// controller, or pointer commands. Keyboard Accept has no row argument, so
/// its row comes from the focus tracked through the native input outcomes.
pub fn apply_native_outcome(
    state: &mut MapMakerUiState,
    focused_row: &mut Option<u32>,
    command: NativeCommand,
    outcome: Outcome,
) -> bool {
    match (command, outcome) {
        (NativeCommand::Focus(requested), Outcome::Focused { row, .. }) if requested == row => {
            *focused_row = Some(row);
            false
        }
        (NativeCommand::Step(_), Outcome::Stepped(result)) => {
            *focused_row = Some(result.cursor);
            false
        }
        (
            NativeCommand::Select(row),
            Outcome::Activated {
                row_handler: true, ..
            },
        ) => {
            *focused_row = Some(row);
            select_prompt_row(state, row)
        }
        (
            NativeCommand::Accept,
            Outcome::Activated {
                row_handler: true, ..
            },
        ) => focused_row.is_some_and(|row| select_prompt_row(state, row)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_rows_update_only_the_owned_editor_experience() {
        let mut state = MapMakerUiState::default();

        assert!(select_prompt_row(&mut state, 0));
        assert_eq!(state.experience(), EditorExperience::Beginner);
        assert_eq!(state.page(), tsfp_mapmaker::ui::MapMakerPage::EditorMenu);
        assert!(state.selected_mode().is_none());

        assert!(select_prompt_row(&mut state, 1));
        assert_eq!(state.experience(), EditorExperience::Advanced);
        assert!(!select_prompt_row(&mut state, 2));
        assert_eq!(state.experience(), EditorExperience::Advanced);
    }

    #[test]
    fn keyboard_down_and_enter_record_the_focused_prompt_row() {
        use crate::{input::MenuIntent, keyboard, widgets::Step};

        let mut state = MapMakerUiState::default();
        let mut focused_row = Some(0);
        assert_eq!(keyboard::menu_intent(0x4000_0051), Some(MenuIntent::Down));
        assert!(!apply_native_outcome(
            &mut state,
            &mut focused_row,
            NativeCommand::Step(Step::Down),
            Outcome::Stepped(crate::widgets::StepResult {
                cursor: 1,
                column: 0,
                moved: true,
                tick: false,
                value: None,
                scrolled: false,
            }),
        ));
        assert_eq!(focused_row, Some(1));

        assert_eq!(keyboard::menu_intent(13), Some(MenuIntent::Accept));
        assert!(apply_native_outcome(
            &mut state,
            &mut focused_row,
            NativeCommand::Accept,
            Outcome::Activated {
                row_handler: true,
                page_handler: false,
                kind: false,
            },
        ));
        assert_eq!(state.experience(), EditorExperience::Advanced);
    }

    #[test]
    fn prompt_state_changes_only_after_a_row_handler_activates() {
        let mut state = MapMakerUiState::default();
        let mut focused_row = Some(0);
        assert!(!apply_native_outcome(
            &mut state,
            &mut focused_row,
            NativeCommand::Accept,
            Outcome::Activated {
                row_handler: false,
                page_handler: true,
                kind: false,
            },
        ));
        assert!(!apply_native_outcome(
            &mut state,
            &mut focused_row,
            NativeCommand::Select(4),
            Outcome::Activated {
                row_handler: true,
                page_handler: false,
                kind: false,
            },
        ));
        assert_eq!(state.experience(), EditorExperience::Advanced);
    }
}
