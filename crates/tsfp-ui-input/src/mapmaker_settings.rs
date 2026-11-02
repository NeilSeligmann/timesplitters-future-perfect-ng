//! Native keyboard, controller, and pointer input for the owned Map Settings
//! rows. Runtime code owns hit geometry, labels, and concrete option values.

use crate::{
    input::IntentSet,
    owned_catalogue::{self, CatalogueAction},
};
use tsfp_mapmaker::ui::{
    EditorSurface, MapMakerPage, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
    MapSettingChoiceId, MapSettingField, MapSettingTarget, MapSettingsAction, MapSettingsEntry,
    MapSettingsEntryTarget,
};

use crate::input::MenuIntent as I;

/// Pointer input after the native page hit test resolves a semantic field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(Option<MapSettingTarget>),
    HoverEntry(Option<MapSettingsEntryTarget>),
    ActivateEntry(Option<MapSettingsEntryTarget>),
    Choose {
        target: Option<MapSettingTarget>,
        choice: Option<MapSettingChoiceId>,
    },
    Back,
}

fn is_map_settings(state: &MapMakerUiState) -> bool {
    state.page() == MapMakerPage::Editor && state.editor_surface() == EditorSurface::MapSettings
}

/// Resolve keyboard/controller intents to typed Map Settings commands.
/// Up/Down use the shared edge/repeat policy to move between rows; Left/Right
/// change the focused row's runtime-supplied choice on key-down edges. The
/// horizontal policy is INFERRED pending the original-route replay.
pub fn commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if !is_map_settings(state) {
        return Vec::new();
    }

    let rows = state.map_settings();
    let entries = state.map_settings_entries();
    let mut cursor = state
        .focused_map_settings_entry()
        .and_then(|focused| entries.iter().position(|entry| *entry == focused));
    let mut result = Vec::new();
    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            CatalogueAction::Step(delta) => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => entries.len().checked_sub(1),
                }
                .filter(|&index| index < entries.len());
                if let Some(next) = next
                    && cursor != Some(next)
                {
                    match entries[next] {
                        MapSettingsEntry::Field(field) => {
                            if let Some(target) = state.map_setting_target(field) {
                                result.push(MapMakerUiCommand::FocusMapSetting(target));
                                cursor = Some(next);
                            }
                        }
                        MapSettingsEntry::Action(_) => {
                            if let Some(target) = state.map_settings_entry_target(entries[next]) {
                                result.push(MapMakerUiCommand::FocusMapSettingsEntry(target));
                                cursor = Some(next);
                            }
                        }
                    }
                }
            }
            CatalogueAction::Accept => {
                if let Some(MapSettingsEntry::Action(_)) =
                    cursor.and_then(|index| entries.get(index))
                    && let Some(target) = cursor
                        .and_then(|index| entries.get(index))
                        .and_then(|entry| state.map_settings_entry_target(*entry))
                {
                    result.push(MapMakerUiCommand::ActivateMapSettingsAction(target));
                }
            }
            CatalogueAction::Focus(_) => {}
        }
    }

    if result
        .iter()
        .any(|command| matches!(command, MapMakerUiCommand::Back))
    {
        return result;
    }
    let horizontal = if held.contains(I::Left) && !before.contains(I::Left) {
        Some(false)
    } else if held.contains(I::Right) && !before.contains(I::Right) {
        Some(true)
    } else {
        None
    };
    if let Some(forward) = horizontal {
        let Some(MapSettingsEntry::Field(field)) = cursor.and_then(|index| entries.get(index))
        else {
            return result;
        };
        let Some(row) = rows.iter().find(|row| row.field == *field) else {
            return result;
        };
        let current = row
            .selected
            .and_then(|selected| row.choices.iter().position(|choice| *choice == selected));
        let next = match (current, forward) {
            (Some(index), true) => index.checked_add(1),
            (Some(index), false) => index.checked_sub(1),
            (None, true) => Some(0),
            (None, false) => row.choices.len().checked_sub(1),
        }
        .filter(|&index| index < row.choices.len());
        if let Some(choice) = next.and_then(|index| row.choices.get(index)).copied()
            && let Some(target) = state.map_setting_target(row.field)
        {
            result.push(MapMakerUiCommand::SelectMapSetting { target, choice });
        }
    }
    result
}

/// Apply one semantic input frame directly to the Rust-owned setting state.
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

/// Resolve a pointer event from the page registry through generation-checked
/// targets. Choosing an option never accepts a stale field or choice ID.
pub fn pointer_commands(state: &MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiCommand> {
    if !is_map_settings(state) {
        return Vec::new();
    }
    match event {
        PointerEvent::Back => vec![MapMakerUiCommand::Back],
        PointerEvent::HoverEntry(Some(target))
            if state.contains_map_settings_entry_target(target) =>
        {
            vec![MapMakerUiCommand::FocusMapSettingsEntry(target)]
        }
        PointerEvent::ActivateEntry(Some(target))
            if state.contains_map_settings_entry_target(target)
                && matches!(target.entry(), MapSettingsEntry::Action(_)) =>
        {
            vec![MapMakerUiCommand::ActivateMapSettingsAction(target)]
        }
        PointerEvent::Hover(Some(target)) if state.contains_map_setting_target(target) => {
            vec![MapMakerUiCommand::FocusMapSetting(target)]
        }
        PointerEvent::Choose {
            target: Some(target),
            choice: Some(choice),
        } if state.contains_map_setting_target(target)
            && state
                .map_settings()
                .iter()
                .any(|row| row.field == target.field() && row.choices.contains(&choice)) =>
        {
            let mut commands = Vec::with_capacity(2);
            if state.focused_map_setting() != Some(target.field()) {
                commands.push(MapMakerUiCommand::FocusMapSetting(target));
            }
            commands.push(MapMakerUiCommand::SelectMapSetting { target, choice });
            commands
        }
        PointerEvent::Hover(None)
        | PointerEvent::Hover(Some(_))
        | PointerEvent::HoverEntry(None)
        | PointerEvent::HoverEntry(Some(_))
        | PointerEvent::ActivateEntry(None)
        | PointerEvent::ActivateEntry(Some(_))
        | PointerEvent::Choose { .. } => Vec::new(),
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
    use tsfp_mapmaker::ui::{
        EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome,
        MapMakerUiCommand as C, MapSettingsGeneration,
    };

    fn settings_state() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        let list = MapListGeneration::new(1);
        state
            .install_local_map_rows(
                list,
                vec![MapListRow {
                    id: MapListRowId::new(list, 7),
                }],
            )
            .unwrap();
        let request = match state.dispatch(C::SelectMap(MapListRowId::new(list, 7))) {
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
                vec![EditorSurface::MapSettings],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::MapSettings)
            .unwrap();
        state.dispatch(C::OpenEditorSurface(target));
        state
            .install_map_settings(
                MapSettingsGeneration::new(3),
                vec![
                    tsfp_mapmaker::ui::MapSettingRow {
                        field: MapSettingField::Theme,
                        choices: vec![MapSettingChoiceId::new(10), MapSettingChoiceId::new(11)],
                        selected: Some(MapSettingChoiceId::new(10)),
                    },
                    tsfp_mapmaker::ui::MapSettingRow {
                        field: MapSettingField::DisableModes,
                        choices: vec![MapSettingChoiceId::new(16), MapSettingChoiceId::new(17)],
                        selected: Some(MapSettingChoiceId::new(16)),
                    },
                    tsfp_mapmaker::ui::MapSettingRow {
                        field: MapSettingField::Music,
                        choices: vec![MapSettingChoiceId::new(20), MapSettingChoiceId::new(21)],
                        selected: Some(MapSettingChoiceId::new(20)),
                    },
                ],
            )
            .unwrap();
        state
    }

    #[test]
    fn keyboard_focus_and_choice_change_update_owned_state() {
        let mut state = settings_state();
        let mut repeat = [0; 4];
        let down = IntentSet::default().with(I::Down);
        dispatch_frame(&mut state, down, IntentSet::default(), 1, &mut repeat);
        assert_eq!(
            state.focused_map_setting(),
            Some(MapSettingField::DisableModes)
        );
        dispatch_frame(&mut state, down, IntentSet::default(), 2, &mut repeat);
        assert_eq!(state.focused_map_setting(), Some(MapSettingField::Music));

        let right = IntentSet::default().with(I::Right);
        dispatch_frame(&mut state, right, IntentSet::default(), 3, &mut repeat);
        assert_eq!(
            state.snapshot().map_settings[2].selected,
            Some(MapSettingChoiceId::new(21))
        );
    }

    #[test]
    fn disable_modes_row_accepts_runtime_choice_without_interpreting_mask() {
        let mut state = settings_state();
        let target = state
            .map_setting_target(MapSettingField::DisableModes)
            .unwrap();
        let selected = MapSettingChoiceId::new(17);
        assert_eq!(
            dispatch_pointer(
                &mut state,
                PointerEvent::Choose {
                    target: Some(target),
                    choice: Some(selected),
                },
            ),
            vec![
                MapMakerUiUpdate::MapSettingFocused(MapSettingField::DisableModes),
                MapMakerUiUpdate::MapSettingChanged {
                    field: MapSettingField::DisableModes,
                    choice: selected,
                }
            ]
        );
        assert_eq!(state.snapshot().map_settings[1].selected, Some(selected));
    }

    #[test]
    fn pointer_choice_uses_current_typed_target() {
        let mut state = settings_state();
        let target = state.map_setting_target(MapSettingField::Music).unwrap();
        let selected = MapSettingChoiceId::new(21);
        assert_eq!(
            dispatch_pointer(
                &mut state,
                PointerEvent::Choose {
                    target: Some(target),
                    choice: Some(selected),
                },
            ),
            vec![
                MapMakerUiUpdate::MapSettingFocused(MapSettingField::Music),
                MapMakerUiUpdate::MapSettingChanged {
                    field: MapSettingField::Music,
                    choice: selected,
                }
            ]
        );
        assert_eq!(state.snapshot().map_settings[2].selected, Some(selected));
    }

    #[test]
    fn stale_pointer_target_is_ignored_after_returning_to_menu() {
        let mut state = settings_state();
        let target = state.map_setting_target(MapSettingField::Theme).unwrap();
        state.dispatch(C::Back);
        assert!(
            pointer_commands(
                &state,
                PointerEvent::Choose {
                    target: Some(target),
                    choice: Some(MapSettingChoiceId::new(11)),
                }
            )
            .is_empty()
        );
    }

    fn settings_with_actions() -> MapMakerUiState {
        let mut state = settings_state();
        let rows = state.map_settings().to_vec();
        state
            .install_map_settings_page(
                MapSettingsGeneration::new(4),
                rows,
                vec![
                    MapSettingsEntry::Field(MapSettingField::Theme),
                    MapSettingsEntry::Action(MapSettingsAction::ViewEditWeaponSets),
                    MapSettingsEntry::Action(MapSettingsAction::ViewEditBotSets),
                    MapSettingsEntry::Field(MapSettingField::Music),
                    MapSettingsEntry::Field(MapSettingField::DisableModes),
                    MapSettingsEntry::Action(MapSettingsAction::EditMapDescription),
                    MapSettingsEntry::Action(MapSettingsAction::Briefing),
                ],
            )
            .unwrap();
        state
    }

    #[test]
    fn keyboard_focus_and_accept_dispatch_ordered_map_settings_action() {
        let mut state = settings_with_actions();
        let mut repeat = [0; 4];
        let down = IntentSet::default().with(I::Down);
        assert_eq!(
            dispatch_frame(&mut state, down, IntentSet::default(), 1, &mut repeat),
            vec![MapMakerUiUpdate::MapSettingsEntryFocused(
                MapSettingsEntry::Action(MapSettingsAction::ViewEditWeaponSets)
            )]
        );
        let accept = IntentSet::default().with(I::Accept);
        assert_eq!(
            dispatch_frame(&mut state, accept, IntentSet::default(), 2, &mut repeat),
            vec![MapMakerUiUpdate::MapSettingsActionRequested(
                MapSettingsAction::ViewEditWeaponSets
            )]
        );
    }

    #[test]
    fn pointer_action_requires_current_map_settings_generation() {
        let mut state = settings_with_actions();
        let target = state
            .map_settings_entry_target(MapSettingsEntry::Action(MapSettingsAction::ViewEditBotSets))
            .unwrap();
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::ActivateEntry(Some(target))),
            vec![MapMakerUiUpdate::MapSettingsActionRequested(
                MapSettingsAction::ViewEditBotSets
            )]
        );
        let rows = state.map_settings().to_vec();
        let entries = state.map_settings_entries().to_vec();
        state
            .install_map_settings_page(MapSettingsGeneration::new(5), rows, entries)
            .unwrap();
        assert!(pointer_commands(&state, PointerEvent::ActivateEntry(Some(target))).is_empty());
    }

    #[test]
    fn keyboard_and_pointer_route_briefing_to_a_typed_request() {
        let mut state = settings_with_actions();
        let entry = MapSettingsEntry::Action(MapSettingsAction::Briefing);
        let target = state.map_settings_entry_target(entry).unwrap();

        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::ActivateEntry(Some(target))),
            vec![MapMakerUiUpdate::MapSettingsActionRequested(
                MapSettingsAction::Briefing
            )]
        );

        state.dispatch(C::FocusMapSettingsEntry(target));
        let mut repeat = [0; 4];
        let accept = IntentSet::default().with(I::Accept);
        assert_eq!(
            dispatch_frame(&mut state, accept, IntentSet::default(), 1, &mut repeat),
            vec![MapMakerUiUpdate::MapSettingsActionRequested(
                MapSettingsAction::Briefing
            )]
        );
    }
}
