//! Native keyboard, controller, and pointer input for Editor Settings rows.
//! Runtime code supplies choices, text, and geometry; mode-change confirmation
//! remains on its separate typed transaction seam.

use crate::{
    input::IntentSet,
    owned_catalogue::{self, CatalogueAction},
};
use tsfp_mapmaker::modes::EditorKind;
use tsfp_mapmaker::ui::{
    EditorKindTarget, EditorSettingChoiceId, EditorSettingTarget, EditorSettingsEditEffect,
    EditorSettingsEditPhase, EditorSurface, MapMakerPage, MapMakerUiCommand, MapMakerUiState,
    MapMakerUiUpdate,
};

use crate::input::MenuIntent as I;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(Option<EditorSettingTarget>),
    Choose {
        target: Option<EditorSettingTarget>,
        choice: Option<EditorSettingChoiceId>,
    },
    ChooseKind(Option<EditorKindTarget>),
    ConfirmKindChange,
    CancelKindChange,
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KindEditAction {
    Request(EditorKind),
    Leave,
    Confirm,
    Cancel,
}

fn is_editor_settings(state: &MapMakerUiState) -> bool {
    state.page() == MapMakerPage::Editor && state.editor_surface() == EditorSurface::EditorSettings
}

pub fn commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if !is_editor_settings(state) {
        return Vec::new();
    }

    let rows = state.editor_settings();
    let mut cursor = state
        .focused_editor_setting()
        .and_then(|focused| rows.iter().position(|row| row.field == focused));
    let mut result = Vec::new();
    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            CatalogueAction::Step(delta) => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => rows.len().checked_sub(1),
                }
                .filter(|&index| index < rows.len());
                if let Some(next) = next
                    && cursor != Some(next)
                    && let Some(target) = state.editor_setting_target(rows[next].field)
                {
                    result.push(MapMakerUiCommand::FocusEditorSetting(target));
                    cursor = Some(next);
                }
            }
            CatalogueAction::Accept | CatalogueAction::Focus(_) => {}
        }
    }

    if result
        .iter()
        .any(|command| matches!(command, MapMakerUiCommand::Back))
    {
        return result;
    }
    let forward = if held.contains(I::Left) && !before.contains(I::Left) {
        Some(false)
    } else if held.contains(I::Right) && !before.contains(I::Right) {
        Some(true)
    } else {
        None
    };
    if let Some(forward) = forward {
        let Some(row) = cursor.and_then(|index| rows.get(index)) else {
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
            && let Some(target) = state.editor_setting_target(row.field)
        {
            result.push(MapMakerUiCommand::SelectEditorSetting { target, choice });
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
    if state.editor_settings_edit().is_some() {
        return dispatch_editor_kind_frame(state, held, before, now_ms, repeat_at);
    }
    commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

fn editor_kind_actions(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<KindEditAction> {
    let Some(edit) = state.editor_settings_edit() else {
        return Vec::new();
    };
    let actions = owned_catalogue::actions(held, before, now_ms, repeat_at, false);
    if edit.phase() == EditorSettingsEditPhase::AwaitingClearConfirmation {
        return actions
            .into_iter()
            .filter_map(|action| match action {
                CatalogueAction::Back => Some(KindEditAction::Cancel),
                CatalogueAction::Accept => Some(KindEditAction::Confirm),
                CatalogueAction::Step(_) | CatalogueAction::Focus(_) => None,
            })
            .collect();
    }
    if edit.phase() != EditorSettingsEditPhase::Editing {
        return Vec::new();
    }
    if actions
        .iter()
        .any(|action| matches!(action, CatalogueAction::Back))
    {
        return vec![KindEditAction::Leave];
    }
    let delta = actions.iter().find_map(|action| match action {
        CatalogueAction::Step(delta) => Some(*delta),
        CatalogueAction::Accept | CatalogueAction::Back | CatalogueAction::Focus(_) => None,
    });
    let horizontal = if held.contains(I::Left) && !before.contains(I::Left) {
        Some(-1)
    } else if held.contains(I::Right) && !before.contains(I::Right) {
        Some(1)
    } else {
        None
    };
    let delta = horizontal.or(delta);
    let Some(delta) = delta else {
        return Vec::new();
    };
    let choices = state.editor_kind_choices();
    let current = choices.iter().position(|kind| *kind == edit.requested());
    let next = match current {
        Some(index) => index.checked_add_signed(delta as isize),
        None if delta > 0 => Some(0),
        None => choices.len().checked_sub(1),
    }
    .filter(|&index| index < choices.len());
    next.and_then(|index| choices.get(index).copied())
        .map_or_else(Vec::new, |kind| vec![KindEditAction::Request(kind)])
}

fn append_effects(
    _state: &mut MapMakerUiState,
    effects: Vec<EditorSettingsEditEffect>,
    updates: &mut Vec<MapMakerUiUpdate>,
) {
    updates.extend(
        effects
            .into_iter()
            .map(MapMakerUiUpdate::EditorSettingsEditEffect),
    );
}

fn dispatch_editor_kind_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    let actions = editor_kind_actions(state, held, before, now_ms, repeat_at);
    let mut updates = Vec::new();
    for action in actions {
        match action {
            KindEditAction::Request(kind) if state.request_editor_kind(kind) => {
                updates.push(MapMakerUiUpdate::EditorSettingsEditRequested(kind));
            }
            KindEditAction::Request(_) => {}
            KindEditAction::Leave => {
                let effects = state.leave_editor_settings_edit();
                append_effects(state, effects, &mut updates);
            }
            KindEditAction::Confirm => {
                let effects = state.confirm_editor_kind_clear();
                append_effects(state, effects, &mut updates);
            }
            KindEditAction::Cancel => {
                let effects = state.cancel_editor_kind_clear();
                append_effects(state, effects, &mut updates);
            }
        }
    }
    updates
}

pub fn pointer_commands(state: &MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiCommand> {
    if !is_editor_settings(state) {
        return Vec::new();
    }
    if state.editor_settings_edit().is_some() {
        return Vec::new();
    }
    match event {
        PointerEvent::Back => vec![MapMakerUiCommand::Back],
        PointerEvent::ChooseKind(_)
        | PointerEvent::ConfirmKindChange
        | PointerEvent::CancelKindChange => Vec::new(),
        PointerEvent::Hover(Some(target)) if state.contains_editor_setting_target(target) => {
            vec![MapMakerUiCommand::FocusEditorSetting(target)]
        }
        PointerEvent::Choose {
            target: Some(target),
            choice: Some(choice),
        } if state.contains_editor_setting_target(target)
            && state
                .editor_settings()
                .iter()
                .any(|row| row.field == target.field() && row.choices.contains(&choice)) =>
        {
            let mut commands = Vec::with_capacity(2);
            if state.focused_editor_setting() != Some(target.field()) {
                commands.push(MapMakerUiCommand::FocusEditorSetting(target));
            }
            commands.push(MapMakerUiCommand::SelectEditorSetting { target, choice });
            commands
        }
        PointerEvent::Hover(None) | PointerEvent::Hover(Some(_)) | PointerEvent::Choose { .. } => {
            Vec::new()
        }
    }
}

pub fn dispatch_pointer(state: &mut MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiUpdate> {
    if state.editor_settings_edit().is_some() {
        match event {
            PointerEvent::ChooseKind(Some(target)) if state.contains_editor_kind_target(target) => {
                return state
                    .request_editor_kind(target.kind())
                    .then_some(MapMakerUiUpdate::EditorSettingsEditRequested(target.kind()))
                    .into_iter()
                    .collect();
            }
            PointerEvent::ConfirmKindChange => {
                let mut updates = Vec::new();
                let effects = state.confirm_editor_kind_clear();
                append_effects(state, effects, &mut updates);
                return updates;
            }
            PointerEvent::CancelKindChange => {
                let mut updates = Vec::new();
                let effects = state.cancel_editor_kind_clear();
                append_effects(state, effects, &mut updates);
                return updates;
            }
            PointerEvent::Back => {
                let back = IntentSet::default().with(I::Back);
                return dispatch_editor_kind_frame(
                    state,
                    back,
                    IntentSet::default(),
                    0,
                    &mut [0; 4],
                );
            }
            PointerEvent::Hover(_) | PointerEvent::Choose { .. } | PointerEvent::ChooseKind(_) => {
                return Vec::new();
            }
        }
    }
    pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{PointerEvent, dispatch_frame, dispatch_pointer};
    use crate::input::{IntentSet, MenuIntent};
    use tsfp_mapmaker::modes::EditorKind;
    use tsfp_mapmaker::ui::{
        EditorKindTarget, EditorSettingChoiceId, EditorSettingField, EditorSettingRow,
        EditorSettingsEditEffect, EditorSettingsEditPhase, EditorSettingsGeneration, EditorSurface,
        EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome,
        MapMakerUiCommand as C, MapMakerUiState, MapMakerUiUpdate as U,
    };

    fn editor_settings_state() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        let list = MapListGeneration::new(1);
        let row = MapListRowId::new(list, 9);
        state
            .install_local_map_rows(list, vec![MapListRow { id: row }])
            .unwrap();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        let U::LoadRequested(request) = state.dispatch(C::SelectMap(row)) else {
            panic!("map selection should request a load");
        };
        state.dispatch(C::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(2),
                vec![EditorSurface::EditorSettings],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::EditorSettings)
            .unwrap();
        state.dispatch(C::OpenEditorSurface(target));
        state
            .install_editor_settings(
                EditorSettingsGeneration::new(3),
                vec![
                    EditorSettingRow {
                        field: EditorSettingField::GridIntensity,
                        choices: vec![
                            EditorSettingChoiceId::new(10),
                            EditorSettingChoiceId::new(11),
                        ],
                        selected: Some(EditorSettingChoiceId::new(10)),
                    },
                    EditorSettingRow {
                        field: EditorSettingField::Music,
                        choices: vec![
                            EditorSettingChoiceId::new(20),
                            EditorSettingChoiceId::new(21),
                        ],
                        selected: Some(EditorSettingChoiceId::new(20)),
                    },
                    EditorSettingRow {
                        field: EditorSettingField::Sounds,
                        choices: vec![
                            EditorSettingChoiceId::new(30),
                            EditorSettingChoiceId::new(31),
                        ],
                        selected: Some(EditorSettingChoiceId::new(30)),
                    },
                ],
            )
            .unwrap();
        state
    }

    #[test]
    fn editor_settings_keyboard_focus_and_choice_are_native_state_updates() {
        let mut state = editor_settings_state();
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert_eq!(
            dispatch_frame(&mut state, down, IntentSet::default(), 10, &mut repeat_at),
            [U::EditorSettingFocused(EditorSettingField::Music)]
        );
        let right = IntentSet::default().with(MenuIntent::Right);
        assert_eq!(
            dispatch_frame(&mut state, right, down, 20, &mut repeat_at),
            [U::EditorSettingChanged {
                field: EditorSettingField::Music,
                choice: EditorSettingChoiceId::new(21),
            }]
        );
        assert_eq!(
            state.editor_settings()[1].selected,
            Some(EditorSettingChoiceId::new(21))
        );
    }

    #[test]
    fn editor_settings_pointer_selects_and_back_invalidates_targets() {
        let mut state = editor_settings_state();
        let target = state
            .editor_setting_target(EditorSettingField::Sounds)
            .unwrap();
        assert_eq!(
            dispatch_pointer(
                &mut state,
                PointerEvent::Choose {
                    target: Some(target),
                    choice: Some(EditorSettingChoiceId::new(31)),
                },
            ),
            [
                U::EditorSettingFocused(EditorSettingField::Sounds),
                U::EditorSettingChanged {
                    field: EditorSettingField::Sounds,
                    choice: EditorSettingChoiceId::new(31),
                },
            ]
        );
        dispatch_pointer(&mut state, PointerEvent::Back);
        assert!(!state.contains_editor_setting_target(target));
        assert!(dispatch_pointer(&mut state, PointerEvent::Hover(Some(target))).is_empty());
    }

    #[test]
    fn editor_kind_keyboard_commits_the_source_ordered_direct_kind() {
        let mut state = editor_settings_state();
        assert!(state.begin_editor_settings_edit(
            EditorKind(4),
            true,
            vec![EditorKind(4), EditorKind(1), EditorKind(7)],
        ));
        let mut repeat_at = [0; 4];
        let right = IntentSet::default().with(MenuIntent::Right);
        assert_eq!(
            dispatch_frame(&mut state, right, IntentSet::default(), 10, &mut repeat_at),
            [U::EditorSettingsEditRequested(EditorKind(1))]
        );
        let back = IntentSet::default().with(MenuIntent::Back);
        assert_eq!(
            dispatch_frame(&mut state, back, right, 20, &mut repeat_at),
            [
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::NotifyEditorKindChanged),
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::PublishRequestedKind(
                    EditorKind(1)
                )),
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::MarkChanged),
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::CloseSettings),
            ]
        );
        assert!(state.close_finished_editor_settings_edit());
        assert_eq!(state.editor_surface(), EditorSurface::MainMenu);
        assert_eq!(state.snapshot().editor_settings_edit, None);
    }

    #[test]
    fn editor_kind_pointer_selects_current_target_and_rejects_stale_target() {
        let mut state = editor_settings_state();
        assert!(state.begin_editor_settings_edit(
            EditorKind(4),
            true,
            vec![EditorKind(4), EditorKind(1), EditorKind(7)],
        ));
        let current = EditorKindTarget::new(EditorSettingsGeneration::new(3), EditorKind(1));
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::ChooseKind(Some(current))),
            [U::EditorSettingsEditRequested(EditorKind(1))]
        );
        assert_eq!(
            state.editor_settings_edit().unwrap().requested(),
            EditorKind(1)
        );

        let stale = EditorKindTarget::new(EditorSettingsGeneration::new(2), EditorKind(7));
        assert!(dispatch_pointer(&mut state, PointerEvent::ChooseKind(Some(stale))).is_empty());
        assert_eq!(
            state.editor_settings_edit().unwrap().requested(),
            EditorKind(1)
        );
    }

    #[test]
    fn editor_kind_pointer_back_and_cancel_keep_the_rust_edit_transaction() {
        let mut state = editor_settings_state();
        assert!(state.begin_editor_settings_edit(
            EditorKind(1),
            true,
            vec![EditorKind(1), EditorKind(0)],
        ));
        let target = EditorKindTarget::new(EditorSettingsGeneration::new(3), EditorKind(0));
        dispatch_pointer(&mut state, PointerEvent::ChooseKind(Some(target)));

        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Back),
            [
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::NotifyEditorKindChanged),
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::RequestClearMapConfirmation),
            ]
        );
        assert_eq!(
            state.editor_settings_edit().unwrap().phase(),
            EditorSettingsEditPhase::AwaitingClearConfirmation
        );
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::CancelKindChange),
            [U::EditorSettingsEditEffect(
                EditorSettingsEditEffect::DismissClearMapConfirmation
            )]
        );
        assert_eq!(
            state.editor_settings_edit().unwrap().phase(),
            EditorSettingsEditPhase::Editing
        );
        assert_eq!(
            state.editor_settings_edit().unwrap().requested(),
            EditorKind(0)
        );
    }

    #[test]
    fn editor_kind_keyboard_confirmation_initializes_before_publish() {
        let mut state = editor_settings_state();
        assert!(state.begin_editor_settings_edit(
            EditorKind(4),
            true,
            vec![EditorKind(4), EditorKind(7)],
        ));
        let mut repeat_at = [0; 4];
        let right = IntentSet::default().with(MenuIntent::Right);
        dispatch_frame(&mut state, right, IntentSet::default(), 10, &mut repeat_at);
        let back = IntentSet::default().with(MenuIntent::Back);
        assert_eq!(
            dispatch_frame(&mut state, back, right, 20, &mut repeat_at),
            [
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::NotifyEditorKindChanged),
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::RequestClearMapConfirmation),
            ]
        );
        assert_eq!(
            state.editor_settings_edit().map(|edit| edit.phase()),
            Some(EditorSettingsEditPhase::AwaitingClearConfirmation)
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_frame(&mut state, accept, back, 30, &mut repeat_at),
            [
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::InitializeMap),
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::PublishRequestedKind(
                    EditorKind(7)
                )),
                U::EditorSettingsEditEffect(EditorSettingsEditEffect::CloseSettings),
            ]
        );
        assert!(state.close_finished_editor_settings_edit());
        assert_eq!(state.editor_surface(), EditorSurface::MainMenu);
    }

    #[test]
    fn editor_kind_pointer_target_is_page_generation_checked() {
        let mut state = editor_settings_state();
        assert!(state.begin_editor_settings_edit(
            EditorKind(4),
            false,
            vec![EditorKind(4), EditorKind(7)],
        ));
        let target = state.editor_kind_target(EditorKind(7)).unwrap();
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::ChooseKind(Some(target))),
            [U::EditorSettingsEditRequested(EditorKind(7))]
        );
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Back),
            [U::EditorSettingsEditEffect(
                EditorSettingsEditEffect::RequestClearMapConfirmation
            )]
        );
        assert!(!state.contains_editor_kind_target(target));
        assert!(dispatch_pointer(&mut state, PointerEvent::ChooseKind(Some(target))).is_empty());
    }
}
