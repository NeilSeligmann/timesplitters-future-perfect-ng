//! Renderer projection for the Rust-owned Map Maker playable-mode page.

use crate::ui::{MapMakerPage, MapMakerUiState, ModeChoiceId};

/// One row in the runtime-provided mode order, marked from Rust-owned focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeChoiceRow {
    id: ModeChoiceId,
    focused: bool,
}

impl ModeChoiceRow {
    pub const fn id(self) -> ModeChoiceId {
        self.id
    }

    pub const fn focused(self) -> bool {
        self.focused
    }
}

/// Project the current mode selection only. Selected configuration and
/// preview-row identity remain separate state and are not renderer focus.
pub fn render_rows(state: &MapMakerUiState) -> Vec<ModeChoiceRow> {
    if state.page() != MapMakerPage::ModeSelection {
        return Vec::new();
    }
    state
        .mode_choices()
        .iter()
        .map(|choice| ModeChoiceRow {
            id: choice.id,
            focused: state.focused_mode() == Some(choice.id),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::{
        AssaultModeConfiguration, EditorKind, GameType, LaunchVariant, ModeConfiguration,
        ModeSettings, PreviewRow, PreviewSelection, ThemeId,
    };
    use crate::ui::{MapMakerUiCommand, ModeChoice};

    fn configuration() -> ModeConfiguration {
        ModeConfiguration::Assault(AssaultModeConfiguration {
            settings: ModeSettings {
                editor_kind: EditorKind(7),
                theme: ThemeId(3),
                preview: PreviewSelection {
                    row: PreviewRow(4),
                    variant: LaunchVariant(0x15),
                    game_type: GameType(9),
                },
                row_options: 0x12,
                duration: 900,
            },
            phases: Default::default(),
        })
    }

    #[test]
    fn projects_mode_choice_rows_in_runtime_order_with_focus() {
        let first = ModeChoiceId::new(0xfeed);
        let second = ModeChoiceId::new(0xbeef);
        let mut state = MapMakerUiState::default();
        state
            .install_mode_choices(vec![
                ModeChoice {
                    id: first,
                    configuration: configuration(),
                },
                ModeChoice {
                    id: second,
                    configuration: configuration(),
                },
            ])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenModeSelection);
        state.dispatch(MapMakerUiCommand::FocusMode(second));

        let rows = render_rows(&state);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id(), first);
        assert!(!rows[0].focused());
        assert_eq!(rows[1].id(), second);
        assert!(rows[1].focused());

        state.dispatch(MapMakerUiCommand::SelectMode(second));
        assert!(render_rows(&state).is_empty());
        assert_eq!(state.selected_mode_id(), Some(second));
        assert_eq!(
            state.selected_mode().map(ModeConfiguration::kind),
            Some(crate::modes::MapMode::Assault)
        );
    }
}
