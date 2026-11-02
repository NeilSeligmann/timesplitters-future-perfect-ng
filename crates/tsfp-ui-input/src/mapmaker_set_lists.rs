//! Native keyboard, controller, and pointer input for Weapon/Bot Set lists.
//!
//! Runtime widgets provide labels and hit geometry. Rust owns ordered row
//! identity, focus, and the typed selection request.

use crate::{input::IntentSet, owned_catalogue};
use tsfp_mapmaker::ui::{
    EditorSetEntryId, EditorSetEntryTarget, EditorSetListKind, MapMakerPage, MapMakerUiCommand,
    MapMakerUiState, MapMakerUiUpdate,
};

use crate::input::MenuIntent as I;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(Option<EditorSetEntryTarget>),
    Activate(Option<EditorSetEntryTarget>),
    Back,
}

fn is_active(state: &MapMakerUiState) -> bool {
    state.page() == MapMakerPage::Editor
        && state
            .editor_set_list_kind()
            .is_some_and(|list| list.surface() == state.editor_surface())
}

/// Resolve navigation through the shared edge/repeat input policy. Accept
/// emits a typed profile-row request and leaves its data effect to the page
/// handler.
pub fn commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if !is_active(state) {
        return Vec::new();
    }
    let entries = state.editor_set_entries();
    let mut cursor = state
        .focused_editor_set_entry()
        .and_then(|focused| entries.iter().position(|entry| entry.id == focused));
    let mut result = Vec::new();
    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            owned_catalogue::CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            owned_catalogue::CatalogueAction::Step(delta) => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => entries.len().checked_sub(1),
                }
                .filter(|&index| index < entries.len());
                if let Some(next) = next
                    && cursor != Some(next)
                    && let Some(target) = state.editor_set_entry_target(entries[next].id)
                {
                    result.push(MapMakerUiCommand::FocusEditorSetEntry(target));
                    cursor = Some(next);
                }
            }
            owned_catalogue::CatalogueAction::Accept => {
                if let Some(target) = cursor
                    .and_then(|index| entries.get(index))
                    .and_then(|entry| state.editor_set_entry_target(entry.id))
                {
                    result.push(MapMakerUiCommand::ActivateEditorSetEntry(target));
                }
            }
            owned_catalogue::CatalogueAction::Focus(_) => {}
        }
    }
    result
}

pub fn dispatch_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

pub fn pointer_commands(state: &MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiCommand> {
    if !is_active(state) {
        return Vec::new();
    }
    match event {
        PointerEvent::Back => vec![MapMakerUiCommand::Back],
        PointerEvent::Hover(Some(target)) if state.contains_editor_set_entry_target(target) => {
            vec![MapMakerUiCommand::FocusEditorSetEntry(target)]
        }
        PointerEvent::Activate(Some(target)) if state.contains_editor_set_entry_target(target) => {
            vec![MapMakerUiCommand::ActivateEditorSetEntry(target)]
        }
        PointerEvent::Hover(None)
        | PointerEvent::Hover(Some(_))
        | PointerEvent::Activate(None)
        | PointerEvent::Activate(Some(_)) => Vec::new(),
    }
}

pub fn dispatch_pointer(state: &mut MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiUpdate> {
    pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MenuIntent;
    use tsfp_mapmaker::ui::{
        EditorSetEntry, EditorSetListGeneration, EditorSurface, EditorSurfaceGeneration,
        MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome, MapMakerUiCommand as C,
    };

    fn list_state(list: EditorSetListKind) -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        let map_generation = MapListGeneration::new(1);
        let map = MapListRowId::new(map_generation, 9);
        state
            .install_local_map_rows(map_generation, vec![MapListRow { id: map }])
            .unwrap();
        let request = match state.dispatch(C::SelectMap(map)) {
            MapMakerUiUpdate::LoadRequested(request) => request,
            update => panic!("unexpected map selection: {update:?}"),
        };
        state.dispatch(C::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(2),
                vec![EditorSurface::WeaponSets, EditorSurface::BotSets],
            )
            .unwrap();
        let target = state.editor_surface_target(list.surface()).unwrap();
        state.dispatch(C::OpenEditorSurface(target));
        state
            .install_editor_set_list(
                EditorSetListGeneration::new(3),
                list,
                vec![
                    EditorSetEntry {
                        id: EditorSetEntryId::new(40),
                    },
                    EditorSetEntry {
                        id: EditorSetEntryId::new(41),
                    },
                ],
            )
            .unwrap();
        state
    }

    #[test]
    fn keyboard_focus_accept_and_pointer_back_use_owned_list_state() {
        let mut state = list_state(EditorSetListKind::Weapon);
        let mut repeat = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert_eq!(
            dispatch_frame(&mut state, down, IntentSet::default(), 1, &mut repeat),
            vec![MapMakerUiUpdate::EditorSetEntryFocused(
                EditorSetEntryId::new(41)
            )]
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_frame(&mut state, accept, IntentSet::default(), 2, &mut repeat),
            vec![MapMakerUiUpdate::EditorSetEntryActivated {
                list: EditorSetListKind::Weapon,
                entry: EditorSetEntryId::new(41),
            }]
        );

        let target = state
            .editor_set_entry_target(EditorSetEntryId::new(41))
            .unwrap();
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Back),
            vec![MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::MainMenu
            )]
        );
        assert!(pointer_commands(&state, PointerEvent::Activate(Some(target))).is_empty());

        let mut keyboard_back_state = list_state(EditorSetListKind::Bot);
        let back = IntentSet::default().with(MenuIntent::Back);
        assert_eq!(
            dispatch_frame(
                &mut keyboard_back_state,
                back,
                IntentSet::default(),
                3,
                &mut repeat,
            ),
            vec![MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::MainMenu
            )]
        );
    }

    #[test]
    fn bot_list_enforces_measured_capacity_and_pointer_activation() {
        let mut state = list_state(EditorSetListKind::Bot);
        assert_eq!(EditorSetListKind::Weapon.capacity(), 18);
        assert_eq!(EditorSetListKind::Bot.capacity(), 16);
        assert_eq!(
            tsfp_mapmaker::ui::MapSettingsAction::ViewEditWeaponSets.destination_surface(),
            Some(EditorSurface::WeaponSets)
        );
        assert_eq!(
            tsfp_mapmaker::ui::MapSettingsAction::ViewEditBotSets.destination_surface(),
            Some(EditorSurface::BotSets)
        );
        let too_many = (0..17)
            .map(|id| EditorSetEntry {
                id: EditorSetEntryId::new(id),
            })
            .collect();
        assert_eq!(
            state.install_editor_set_list(
                EditorSetListGeneration::new(4),
                EditorSetListKind::Bot,
                too_many,
            ),
            Err(tsfp_mapmaker::ui::EditorSetListError::TooManyEntries)
        );
        let target = state
            .editor_set_entry_target(EditorSetEntryId::new(41))
            .unwrap();
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Activate(Some(target))),
            vec![MapMakerUiUpdate::EditorSetEntryActivated {
                list: EditorSetListKind::Bot,
                entry: EditorSetEntryId::new(41),
            }]
        );
    }
}
