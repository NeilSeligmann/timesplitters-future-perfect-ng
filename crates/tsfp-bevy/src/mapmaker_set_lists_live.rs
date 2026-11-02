// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach the T1714 Weapon/Bot Set list surfaces to owned editor state.
//!
//! The helper addresses are INFERRED page roots: T1714 measures their names and
//! row capacities but finds no stored page descriptors. Registration therefore
//! requires the matching typed Map Settings request, the Rust UI switch, and an
//! exact live cursor-row count. Row labels and effects remain runtime supplied.

use std::sync::Mutex;

use tsfp_mapmaker::ui::{
    EditorSetEntry, EditorSetEntryId, EditorSetListGeneration, EditorSetListKind, MapMakerPage,
    MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
};
use tsfp_ui_input::live_page::LivePage;
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_model::mapmaker_set_lists::{
    EditorSetEntryLayout, EditorSetListLayout, build_editor_set_list_page,
};
use tsfp_ui_model::model::{Page, WidgetId};

pub const BOT_SET_LIST_BUILDER: u32 = 0x2B_F830;
pub const WEAPON_SET_LIST_BUILDER: u32 = 0x2C_1830;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
});

pub fn is_candidate_builder(builder: u32) -> bool {
    matches!(builder, BOT_SET_LIST_BUILDER | WEAPON_SET_LIST_BUILDER)
}

fn kind_for_builder(builder: u32) -> Option<EditorSetListKind> {
    match builder {
        BOT_SET_LIST_BUILDER => Some(EditorSetListKind::Bot),
        WEAPON_SET_LIST_BUILDER => Some(EditorSetListKind::Weapon),
        _ => None,
    }
}

fn expected_kind(state: &MapMakerUiState, builder: u32) -> Option<EditorSetListKind> {
    let kind = kind_for_builder(builder)?;
    (state.page() == MapMakerPage::Editor && state.editor_surface() == kind.surface())
        .then_some(kind)
}

/// Only the typed preceding Map Settings action can opt a helper candidate into
/// the live registry. Without this state the old page retains the lifted path.
pub fn registered(builder: u32) -> bool {
    crate::storage_live::mapmaker_ui_enabled()
        && crate::storage_live::mapmaker_ui()
            .lock()
            .is_ok_and(|state| expected_kind(&state, builder).is_some())
}

pub fn observe_top(top: Option<(u32, u32)>) {
    let active = top.filter(|(builder, _)| registered(*builder));
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    if session.active_page != active {
        session.active_page = active;
        if active.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

fn generation(builder: u32, slot: u32) -> Option<EditorSetListGeneration> {
    let session = SESSION.lock().ok()?;
    (session.active_page == Some((builder, slot)))
        .then(|| EditorSetListGeneration::new(session.generation))
}

/// Build the runtime-backed row projection only when REA's measured capacity
/// and the outstanding typed destination agree. No profile data is mutated.
pub fn project_owned_page(
    builder: u32,
    slot: u32,
    live: &LivePage,
    cursor_nodes: &[u32],
    mut page: Page,
) -> Option<Page> {
    let kind = kind_for_builder(builder)?;
    if !registered(builder) || cursor_nodes.len() != kind.capacity() {
        return None;
    }
    let generation = generation(builder, slot)?;
    let mut state = crate::storage_live::mapmaker_ui().lock().ok()?;
    let rows: Vec<_> = (0..kind.capacity())
        .map(|index| EditorSetEntry {
            id: EditorSetEntryId::new(index as u64),
        })
        .collect();
    if state.editor_set_list_kind() != Some(kind)
        || state.snapshot().editor_set_list_generation != Some(generation)
        || state.editor_set_entries() != rows.as_slice()
    {
        state.install_editor_set_list(generation, kind, rows).ok()?;
    }

    let entries = cursor_nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let row_index = live.rows.iter().position(|row| row.node == *node)?;
            let widget_id = tsfp_ui_input::live_page::widget_id(row_index);
            let widget = page
                .widgets
                .iter()
                .find(|widget| widget.id == widget_id)?
                .clone();
            Some(EditorSetEntryLayout {
                entry: *state.editor_set_entries().get(index)?,
                widget,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let snapshot = state.snapshot();
    let projected = build_editor_set_list_page(
        &snapshot,
        page.id,
        EditorSetListLayout {
            kind,
            widget_type: page.widget_type,
            panel_bounds: page.panel_bounds.clone(),
            nav: page.nav.clone(),
            wrap: page.wrap.clone(),
            entries,
        },
    )
    .ok()?;
    page.initial_focus = projected.page.initial_focus;
    Some(page)
}

/// Apply native row focus/activation to the typed list state. `ui_live` still
/// forwards the same commands through the original row handler effect seam.
pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if !is_candidate_builder(current.descriptor.builder)
        || !registered(current.descriptor.builder)
        || current.cursor_nodes.len()
            != kind_for_builder(current.descriptor.builder).map_or(0, |kind| kind.capacity())
    {
        return;
    }
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    let _ = dispatch_native_commands(&mut state, commands);
}

/// Translate registered-page commands into the typed list state. Pointer
/// focus/click and keyboard/pad focus/accept share this mapping.
fn dispatch_native_commands(
    state: &mut MapMakerUiState,
    commands: &[NativeCommand],
) -> Vec<MapMakerUiUpdate> {
    let mut updates = Vec::new();
    for command in commands {
        let index = match *command {
            NativeCommand::Focus(index) | NativeCommand::Select(index) => index as usize,
            NativeCommand::Accept => state
                .focused_editor_set_entry()
                .and_then(|id| {
                    state
                        .editor_set_entries()
                        .iter()
                        .position(|entry| entry.id == id)
                })
                .unwrap_or(usize::MAX),
            NativeCommand::Back => {
                let update = state.dispatch(MapMakerUiCommand::Back);
                if update != MapMakerUiUpdate::Ignored {
                    updates.push(update);
                }
                continue;
            }
            _ => continue,
        };
        let Some(entry) = state.editor_set_entries().get(index) else {
            continue;
        };
        let Some(target) = state.editor_set_entry_target(entry.id) else {
            continue;
        };
        match *command {
            NativeCommand::Focus(_) => {
                let update = state.dispatch(MapMakerUiCommand::FocusEditorSetEntry(target));
                if update != MapMakerUiUpdate::Ignored {
                    updates.push(update);
                }
            }
            NativeCommand::Select(_) | NativeCommand::Accept => {
                let update = state.dispatch(MapMakerUiCommand::ActivateEditorSetEntry(target));
                if update != MapMakerUiUpdate::Ignored {
                    updates.push(update);
                }
            }
            _ => {}
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_mapmaker::ui::{
        EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome,
        MapMakerUiCommand as C, MapMakerUiUpdate,
    };

    fn owned_list_state(kind: EditorSetListKind) -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        let maps = MapListGeneration::new(1);
        let row = MapListRowId::new(maps, 4);
        state
            .install_local_map_rows(maps, vec![MapListRow { id: row }])
            .unwrap();
        let request = match state.dispatch(C::SelectMap(row)) {
            MapMakerUiUpdate::LoadRequested(request) => request,
            update => panic!("unexpected map selection: {update:?}"),
        };
        state.dispatch(C::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(EditorSurfaceGeneration::new(2), vec![kind.surface()])
            .unwrap();
        let target = state.editor_surface_target(kind.surface()).unwrap();
        state.dispatch(C::OpenEditorSurface(target));
        state
    }

    #[test]
    fn t1714_list_builder_candidates_match_the_measured_capacities() {
        assert_eq!(
            kind_for_builder(BOT_SET_LIST_BUILDER),
            Some(EditorSetListKind::Bot)
        );
        assert_eq!(
            kind_for_builder(WEAPON_SET_LIST_BUILDER),
            Some(EditorSetListKind::Weapon)
        );
        assert_eq!(EditorSetListKind::Bot.capacity(), 16);
        assert_eq!(EditorSetListKind::Weapon.capacity(), 18);
        assert_eq!(kind_for_builder(0x31_33E0), None);
    }

    #[test]
    fn typed_destination_must_match_the_profile_list_builder() {
        let bot = owned_list_state(EditorSetListKind::Bot);
        let weapon = owned_list_state(EditorSetListKind::Weapon);
        assert_eq!(
            expected_kind(&bot, BOT_SET_LIST_BUILDER),
            Some(EditorSetListKind::Bot)
        );
        assert_eq!(expected_kind(&bot, WEAPON_SET_LIST_BUILDER), None);
        assert_eq!(
            expected_kind(&weapon, WEAPON_SET_LIST_BUILDER),
            Some(EditorSetListKind::Weapon)
        );
        assert_eq!(expected_kind(&weapon, BOT_SET_LIST_BUILDER), None);
    }

    #[test]
    fn pointer_focus_and_click_emit_typed_set_entry_updates() {
        let kind = EditorSetListKind::Bot;
        let mut state = owned_list_state(kind);
        let generation = EditorSetListGeneration::new(3);
        let entries: Vec<_> = (0..kind.capacity())
            .map(|index| EditorSetEntry {
                id: EditorSetEntryId::new(index as u64),
            })
            .collect();
        state
            .install_editor_set_list(generation, kind, entries.clone())
            .unwrap();

        let updates = dispatch_native_commands(
            &mut state,
            &[NativeCommand::Focus(5), NativeCommand::Select(5)],
        );

        assert_eq!(
            updates,
            vec![
                MapMakerUiUpdate::EditorSetEntryFocused(entries[5].id),
                MapMakerUiUpdate::EditorSetEntryActivated {
                    list: kind,
                    entry: entries[5].id,
                },
            ]
        );
        assert_eq!(state.focused_editor_set_entry(), Some(entries[5].id));
        assert!(dispatch_native_commands(&mut state, &[NativeCommand::Select(16)]).is_empty());
    }
}
