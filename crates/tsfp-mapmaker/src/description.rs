//! Native Map Description text editing state (T2408).
//!
//! The runtime caller provides the current text and byte limit, and receives a
//! typed commit request. This module owns edits and never reads or writes guest
//! memory. T1714 measures the Map Settings action label and order but does not
//! establish the description page's capacity, charset, or persistence rules.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// Identity of one installed description field; stale input cannot edit a new
/// page after it has been replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DescriptionGeneration(u64);

impl DescriptionGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Target for input resolved against the currently installed text field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DescriptionTarget {
    generation: DescriptionGeneration,
}

impl DescriptionTarget {
    pub const fn generation(self) -> DescriptionGeneration {
        self.generation
    }
}

/// Keyboard operations on the Rust-owned description buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptionCommand {
    Insert(char),
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    /// Place the caret at a Unicode scalar boundary supplied by a native UI
    /// hit test. This is the Rust view contract; original game caret semantics
    /// remain an open REA question.
    PlaceCaret(usize),
    Commit,
    Cancel,
}

/// Rejected session input. Capacity is supplied by the live document adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptionError {
    InitialTextTooLong,
    StaleTarget,
    UnsupportedCharacter,
}

/// Typed outcome of a description editing command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DescriptionOutcome {
    Changed,
    Unchanged,
    CommitRequested(DescriptionCommit),
    Cancelled(DescriptionGeneration),
}

/// Native document update requested by Enter. Persistence remains the typed
/// caller's responsibility until the native document service accepts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptionCommit {
    pub generation: DescriptionGeneration,
    pub text: String,
}

/// Immutable text and caret projection for a native Map Maker renderer.
/// `caret` counts Unicode scalar values in `text`; it is not a byte offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptionView {
    pub target: DescriptionTarget,
    pub text: String,
    pub caret: usize,
    pub byte_limit: usize,
}

/// One active, Rust-owned Map Description edit session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapDescriptionSession {
    generation: DescriptionGeneration,
    chars: Vec<char>,
    cursor: usize,
    byte_limit: usize,
}

impl MapDescriptionSession {
    /// `byte_limit` is an explicit runtime contract; this constructor does not
    /// guess the original game's page-specific maximum.
    pub fn new(
        generation: DescriptionGeneration,
        initial: &str,
        byte_limit: usize,
    ) -> Result<Self, DescriptionError> {
        if initial.len() > byte_limit {
            return Err(DescriptionError::InitialTextTooLong);
        }
        let chars: Vec<char> = initial.chars().collect();
        let cursor = chars.len();
        Ok(Self {
            generation,
            chars,
            cursor,
            byte_limit,
        })
    }

    pub const fn generation(&self) -> DescriptionGeneration {
        self.generation
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub const fn byte_limit(&self) -> usize {
        self.byte_limit
    }

    pub fn target(&self) -> DescriptionTarget {
        DescriptionTarget {
            generation: self.generation,
        }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn view(&self) -> DescriptionView {
        DescriptionView {
            target: self.target(),
            text: self.text(),
            caret: self.cursor,
            byte_limit: self.byte_limit,
        }
    }

    pub fn apply(
        &mut self,
        target: DescriptionTarget,
        command: DescriptionCommand,
    ) -> Result<DescriptionOutcome, DescriptionError> {
        if target.generation != self.generation {
            return Err(DescriptionError::StaleTarget);
        }

        let outcome = match command {
            DescriptionCommand::Insert(ch) => {
                if ch == '\0' || ch.is_control() {
                    return Err(DescriptionError::UnsupportedCharacter);
                }
                if self.text().len().saturating_add(ch.len_utf8()) > self.byte_limit {
                    DescriptionOutcome::Unchanged
                } else {
                    self.chars.insert(self.cursor, ch);
                    self.cursor += 1;
                    DescriptionOutcome::Changed
                }
            }
            DescriptionCommand::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.chars.remove(self.cursor);
                DescriptionOutcome::Changed
            }
            DescriptionCommand::Delete if self.cursor < self.chars.len() => {
                self.chars.remove(self.cursor);
                DescriptionOutcome::Changed
            }
            DescriptionCommand::MoveLeft if self.cursor > 0 => {
                self.cursor -= 1;
                DescriptionOutcome::Changed
            }
            DescriptionCommand::MoveRight if self.cursor < self.chars.len() => {
                self.cursor += 1;
                DescriptionOutcome::Changed
            }
            DescriptionCommand::PlaceCaret(index) if index <= self.chars.len() => {
                if index == self.cursor {
                    DescriptionOutcome::Unchanged
                } else {
                    self.cursor = index;
                    DescriptionOutcome::Changed
                }
            }
            DescriptionCommand::Commit => DescriptionOutcome::CommitRequested(DescriptionCommit {
                generation: self.generation,
                text: self.text(),
            }),
            DescriptionCommand::Cancel => DescriptionOutcome::Cancelled(self.generation),
            DescriptionCommand::Backspace
            | DescriptionCommand::Delete
            | DescriptionCommand::MoveLeft
            | DescriptionCommand::MoveRight
            | DescriptionCommand::PlaceCaret(_) => DescriptionOutcome::Unchanged,
        };
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(initial: &str, limit: usize) -> MapDescriptionSession {
        MapDescriptionSession::new(DescriptionGeneration::new(7), initial, limit).unwrap()
    }

    #[test]
    fn edits_unicode_by_character_but_enforces_the_runtime_byte_limit() {
        let mut state = session("a", 3);
        let target = state.target();
        assert_eq!(
            state.apply(target, DescriptionCommand::Insert('é')),
            Ok(DescriptionOutcome::Changed)
        );
        assert_eq!(state.text(), "aé");
        assert_eq!(
            state.apply(target, DescriptionCommand::Insert('z')),
            Ok(DescriptionOutcome::Unchanged)
        );
        assert_eq!(state.text(), "aé");
    }

    #[test]
    fn cursor_insert_delete_backspace_and_commit_are_owned() {
        let mut state = session("map", 16);
        let target = state.target();
        assert_eq!(state.cursor(), 3);
        state.apply(target, DescriptionCommand::MoveLeft).unwrap();
        state
            .apply(target, DescriptionCommand::Insert('X'))
            .unwrap();
        state.apply(target, DescriptionCommand::Delete).unwrap();
        state.apply(target, DescriptionCommand::MoveLeft).unwrap();
        state.apply(target, DescriptionCommand::Backspace).unwrap();
        assert_eq!(state.text(), "mX");
        assert_eq!(
            state.apply(target, DescriptionCommand::Commit),
            Ok(DescriptionOutcome::CommitRequested(DescriptionCommit {
                generation: DescriptionGeneration::new(7),
                text: "mX".into(),
            }))
        );
    }

    #[test]
    fn native_view_and_caret_hit_test_use_unicode_scalar_boundaries() {
        let mut state = session("Aé界", 16);
        let target = state.target();
        assert_eq!(
            state.view(),
            DescriptionView {
                target,
                text: "Aé界".into(),
                caret: 3,
                byte_limit: 16,
            }
        );

        assert_eq!(
            state.apply(target, DescriptionCommand::PlaceCaret(1)),
            Ok(DescriptionOutcome::Changed)
        );
        assert_eq!(state.view().caret, 1);
        assert_eq!(
            state.apply(target, DescriptionCommand::Insert('!')),
            Ok(DescriptionOutcome::Changed)
        );
        assert_eq!(state.view().text, "A!é界");
        assert_eq!(state.view().caret, 2);
        assert_eq!(
            state.apply(target, DescriptionCommand::PlaceCaret(5)),
            Ok(DescriptionOutcome::Unchanged)
        );
        assert_eq!(state.view().caret, 2);
    }

    #[test]
    fn rejects_stale_targets_unsupported_characters_and_oversized_initial_text() {
        let mut state = session("map", 3);
        let stale = DescriptionTarget {
            generation: DescriptionGeneration::new(6),
        };
        assert_eq!(
            state.apply(stale, DescriptionCommand::Insert('!')),
            Err(DescriptionError::StaleTarget)
        );
        assert_eq!(
            state.apply(state.target(), DescriptionCommand::Insert('\n')),
            Err(DescriptionError::UnsupportedCharacter)
        );
        assert_eq!(
            MapDescriptionSession::new(DescriptionGeneration::new(1), "four", 3),
            Err(DescriptionError::InitialTextTooLong)
        );
    }

    #[test]
    fn ui_state_binds_description_session_to_active_surface_generation() {
        use crate::ui::{
            EditorSurface, EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId,
            MapLoadOutcome, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
        };

        let mut ui = MapMakerUiState::default();
        let row = MapListRowId::new(MapListGeneration::new(3), 1);
        ui.install_local_map_rows(MapListGeneration::new(3), vec![MapListRow { id: row }])
            .unwrap();
        ui.dispatch(MapMakerUiCommand::OpenStorage);
        ui.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let MapMakerUiUpdate::LoadRequested(request) =
            ui.dispatch(MapMakerUiCommand::SelectMap(row))
        else {
            panic!("selecting a map should request a load");
        };
        ui.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        ui.install_editor_surface_catalog(
            EditorSurfaceGeneration::new(17),
            vec![EditorSurface::MapDescription],
        )
        .unwrap();
        let surface = ui
            .editor_surface_target(EditorSurface::MapDescription)
            .unwrap();
        assert_eq!(
            ui.dispatch(MapMakerUiCommand::OpenEditorSurface(surface)),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MapDescription)
        );

        let input = ui
            .install_map_description_session(surface, "map", 8)
            .unwrap();
        assert_eq!(
            ui.map_description_view().unwrap(),
            crate::ui::MapDescriptionPageView {
                surface,
                field: DescriptionView {
                    target: input,
                    text: "map".into(),
                    caret: 3,
                    byte_limit: 8,
                },
            }
        );
        assert_eq!(
            ui.apply_map_description_command(surface, input, DescriptionCommand::PlaceCaret(1)),
            Ok(DescriptionOutcome::Changed)
        );
        assert_eq!(ui.map_description_view().unwrap().field.caret, 1);
        assert_eq!(
            ui.apply_map_description_command(surface, input, DescriptionCommand::Insert('!')),
            Ok(DescriptionOutcome::Changed)
        );
        assert_eq!(ui.map_description_session().unwrap().text(), "m!ap");
        assert_eq!(
            ui.apply_map_description_command(surface, input, DescriptionCommand::Commit),
            Ok(DescriptionOutcome::CommitRequested(DescriptionCommit {
                generation: DescriptionGeneration::new(17),
                text: "m!ap".into(),
            }))
        );

        ui.dispatch(MapMakerUiCommand::Back);
        assert!(ui.map_description_session().is_none());
        assert_eq!(
            ui.apply_map_description_command(surface, input, DescriptionCommand::Insert('?')),
            Err(DescriptionError::StaleTarget)
        );
    }
}
