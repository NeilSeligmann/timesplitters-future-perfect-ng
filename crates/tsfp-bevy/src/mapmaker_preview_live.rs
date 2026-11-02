// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach the loaded-editor preview rows to Rust-owned page state.
//!
//! T1715 supplies the ordered preview table. The runtime page supplies only
//! the currently available rows, localized labels, and presentation metadata;
//! preview eligibility and launch effects remain with their typed owners.

use std::sync::Mutex;

use tsfp_mapmaker::ui::{
    EditorPreviewChoiceId, EditorPreviewGeneration, EditorSurface, MapMakerPage, MapMakerUiCommand,
    MapMakerUiState, MapMakerUiUpdate,
};
use tsfp_ui_input::live_page::{LivePage, LiveText};
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_model::mapmaker_preview::{
    EditorPreviewChoiceLayout, EditorPreviewLayout, build_editor_preview_page,
};
use tsfp_ui_model::model::{Page, WidgetId};

pub const BUILDER: u32 = 0x31_4420;

/// T1715 row labels in the original fifteen-row selector table. The host page
/// may contain an available subset; unknown, duplicated, or reordered rows
/// fail closed rather than acquiring an invented launch meaning.
const PREVIEW_ROW_LABELS: [u32; 15] = [
    0x16F6, 0x16C9, 0x16C5, 0x16CF, 0x16C3, 0x16C1, 0x16D1, 0x16DF, 0x16E1, 0x16D8, 0x16D3, 0x16C7,
    0x16CB, 0x16DB, 0x16E3,
];

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    updates: Vec<MapMakerUiUpdate>,
}

/// Typed outcomes captured while one loaded Preview Map page is active.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewUpdateBatch {
    pub generation: EditorPreviewGeneration,
    pub updates: Vec<MapMakerUiUpdate>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
    updates: Vec::new(),
});

pub fn observe_top(top: Option<(u32, u32)>) {
    let page = top.filter(|(builder, _)| *builder == BUILDER);
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    change_active_page(&mut session, page);
}

fn change_active_page(session: &mut Session, page: Option<(u32, u32)>) {
    if session.active_page != page {
        session.active_page = page;
        session.updates.clear();
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

fn choice_ids_for_labels(labels: &[u32]) -> Option<Vec<EditorPreviewChoiceId>> {
    let mut previous = None;
    let choices = labels
        .iter()
        .map(|label| {
            let index = PREVIEW_ROW_LABELS
                .iter()
                .position(|expected| expected == label)?;
            if previous.is_some_and(|previous| index <= previous) {
                return None;
            }
            previous = Some(index);
            Some(EditorPreviewChoiceId::new(index as u64))
        })
        .collect::<Option<Vec<_>>>()?;
    (!choices.is_empty()).then_some(choices)
}

fn live_choice_ids(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<EditorPreviewChoiceId>> {
    if live.builder != BUILDER || cursor_nodes.is_empty() {
        return None;
    }
    let labels = cursor_nodes
        .iter()
        .map(|node| {
            let row = live.rows.iter().find(|row| row.node == *node)?;
            let LiveText::Id(label) = &row.text else {
                return None;
            };
            Some(*label)
        })
        .collect::<Option<Vec<_>>>()?;
    choice_ids_for_labels(&labels)
}

fn ensure_catalog(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let Some(choices) = live_choice_ids(live, cursor_nodes) else {
        return false;
    };
    let generation = match SESSION.lock() {
        Ok(session)
            if session.active_page == Some((live.builder, live.slot))
                && session.generation != 0 =>
        {
            EditorPreviewGeneration::new(session.generation)
        }
        _ => return false,
    };
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return false;
    };
    if state.page() != MapMakerPage::Editor {
        return false;
    }
    if state.editor_surface() != EditorSurface::MapPreview {
        let Some(target) = state.editor_surface_target(EditorSurface::MapPreview) else {
            return false;
        };
        if !matches!(
            state.dispatch(MapMakerUiCommand::OpenEditorSurface(target)),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MapPreview)
        ) {
            return false;
        }
    }
    state
        .install_editor_preview_catalog(generation, choices)
        .is_ok()
}

fn page_layout(live: &LivePage, cursor_nodes: &[u32], page: &Page) -> Option<EditorPreviewLayout> {
    let choices = live_choice_ids(live, cursor_nodes)?;
    if choices.len() != page.widgets.len() {
        return None;
    }
    Some(EditorPreviewLayout {
        widget_type: page.widget_type,
        panel_bounds: page.panel_bounds,
        nav: page.nav,
        wrap: page.wrap,
        choices: choices
            .into_iter()
            .zip(page.widgets.iter().cloned())
            .map(|(choice, widget)| EditorPreviewChoiceLayout { choice, widget })
            .collect(),
    })
}

fn command_for_native(
    state: &MapMakerUiState,
    choices: &[EditorPreviewChoiceId],
    command: NativeCommand,
) -> Option<MapMakerUiCommand> {
    let target = |choice| state.editor_preview_choice_target(choice);
    match command {
        NativeCommand::Focus(index) => choices
            .get(index as usize)
            .copied()
            .and_then(target)
            .map(MapMakerUiCommand::FocusEditorPreviewChoice),
        NativeCommand::Select(index) => choices
            .get(index as usize)
            .copied()
            .and_then(target)
            .map(MapMakerUiCommand::ActivateEditorPreviewChoice),
        NativeCommand::Accept => state
            .focused_editor_preview_choice()
            .and_then(target)
            .map(MapMakerUiCommand::ActivateEditorPreviewChoice),
        NativeCommand::Back => Some(MapMakerUiCommand::Back),
        _ => None,
    }
}

fn dispatch_native_commands(
    state: &mut MapMakerUiState,
    choices: &[EditorPreviewChoiceId],
    commands: &[NativeCommand],
) -> Vec<MapMakerUiUpdate> {
    commands
        .iter()
        .filter_map(|command| {
            let typed = command_for_native(state, choices, *command)?;
            match state.dispatch(typed) {
                MapMakerUiUpdate::Ignored => None,
                update => Some(update),
            }
        })
        .collect()
}

/// Apply the same typed focus and activation intent as the registered native
/// page commands. Existing preview callbacks remain the temporary effect seam.
pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder != BUILDER
        || !ensure_catalog(&current.live, &current.cursor_nodes)
    {
        return;
    }
    let Some(choices) = live_choice_ids(&current.live, &current.cursor_nodes) else {
        return;
    };
    let generation = match SESSION.lock() {
        Ok(session)
            if session.active_page == Some((current.live.builder, current.live.slot))
                && session.generation != 0 =>
        {
            session.generation
        }
        _ => return,
    };
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    let updates = dispatch_native_commands(&mut state, &choices, commands);
    drop(state);
    if updates.is_empty() {
        return;
    }
    if let Ok(mut session) = SESSION.lock()
        && session.active_page == Some((current.live.builder, current.live.slot))
        && session.generation == generation
    {
        session.updates.extend(updates);
    }
}

/// Drain typed focus, activation, and Back outcomes for the active preview
/// caller. Page changes invalidate undrained outcomes and advance generation.
fn take_session_updates(session: &mut Session) -> Option<PreviewUpdateBatch> {
    if session.active_page.is_none() || session.updates.is_empty() {
        return None;
    }
    Some(PreviewUpdateBatch {
        generation: EditorPreviewGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}

pub fn take_updates() -> Option<PreviewUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    take_session_updates(&mut session)
}

/// Project measured, currently available preview rows through the native UI
/// model while retaining the live localization and geometry.
pub fn project_owned_page(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    focused: Option<WidgetId>,
    page: Page,
) -> Option<Page> {
    if live.slot != slot {
        return None;
    }
    let layout = page_layout(live, cursor_nodes, &page)?;
    if !ensure_catalog(live, cursor_nodes) {
        return None;
    }
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return None;
    };
    if let Some(choice) = focused
        .and_then(|widget| live.cursor_index(widget.0 as usize, cursor_nodes))
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| layout.choices.get(index))
        .map(|row| row.choice)
        && let Some(target) = state.editor_preview_choice_target(choice)
    {
        let _ = state.dispatch(MapMakerUiCommand::FocusEditorPreviewChoice(target));
    }
    let projection = build_editor_preview_page(&state.snapshot(), page.id, layout).ok()?;
    let mut projected_page = projection.page;
    projected_page.highlight_edges = page.highlight_edges;
    projected_page.handlers = page.handlers;
    projected_page.links = page.links;
    projected_page.enter = page.enter;
    projected_page.exit = page.exit;
    Some(projected_page)
}

#[cfg(test)]
mod tests {
    use super::{PREVIEW_ROW_LABELS, choice_ids_for_labels, dispatch_native_commands};
    use tsfp_mapmaker::ui::{
        EditorPreviewChoiceId, EditorPreviewGeneration, EditorSurface, EditorSurfaceGeneration,
        MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
    };
    use tsfp_ui_input::native_page::NativeCommand;

    #[test]
    fn available_preview_rows_keep_the_measured_table_identity_and_order() {
        let choices = choice_ids_for_labels(&[
            PREVIEW_ROW_LABELS[0],
            PREVIEW_ROW_LABELS[2],
            PREVIEW_ROW_LABELS[12],
            PREVIEW_ROW_LABELS[14],
        ])
        .expect("known ordered available rows");
        assert_eq!(
            choices,
            vec![
                EditorPreviewChoiceId::new(0),
                EditorPreviewChoiceId::new(2),
                EditorPreviewChoiceId::new(12),
                EditorPreviewChoiceId::new(14),
            ]
        );
    }

    #[test]
    fn unknown_duplicate_and_reordered_preview_rows_fail_closed() {
        assert!(choice_ids_for_labels(&[PREVIEW_ROW_LABELS[0], 0xFFFF]).is_none());
        assert!(choice_ids_for_labels(&[PREVIEW_ROW_LABELS[2], PREVIEW_ROW_LABELS[2]]).is_none());
        assert!(choice_ids_for_labels(&[PREVIEW_ROW_LABELS[3], PREVIEW_ROW_LABELS[2]]).is_none());
    }

    #[test]
    fn keyboard_focus_accept_and_back_emit_typed_preview_updates() {
        let choices = [
            EditorPreviewChoiceId::new(2),
            EditorPreviewChoiceId::new(12),
        ];
        let mut state = MapMakerUiState::default();
        let _ = state.dispatch(MapMakerUiCommand::BeginEditor);
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(4),
                vec![EditorSurface::MapPreview],
            )
            .expect("preview surface catalog");
        let surface = state
            .editor_surface_target(EditorSurface::MapPreview)
            .expect("preview target");
        assert_eq!(
            state.dispatch(MapMakerUiCommand::OpenEditorSurface(surface)),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MapPreview)
        );
        state
            .install_editor_preview_catalog(EditorPreviewGeneration::new(9), choices.to_vec())
            .expect("preview choice catalog");

        assert_eq!(
            dispatch_native_commands(
                &mut state,
                &choices,
                &[NativeCommand::Focus(1), NativeCommand::Accept],
            ),
            vec![
                MapMakerUiUpdate::EditorPreviewChoiceFocused(choices[1]),
                MapMakerUiUpdate::EditorPreviewSelectionRequested(
                    tsfp_mapmaker::modes::PreviewSelection::from_row(
                        tsfp_mapmaker::modes::PreviewRow(12),
                    )
                    .unwrap(),
                ),
            ]
        );
        assert_eq!(
            dispatch_native_commands(&mut state, &choices, &[NativeCommand::Back]),
            vec![MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::MainMenu
            )]
        );
    }

    #[test]
    fn preview_updates_drain_once_with_the_active_page_generation() {
        let mut session = super::Session {
            active_page: Some((super::BUILDER, 12)),
            generation: 7,
            updates: vec![MapMakerUiUpdate::EditorMenuActionFocused(
                tsfp_mapmaker::ui::EditorMenuAction::PreviewMap,
            )],
        };
        assert_eq!(
            super::take_session_updates(&mut session),
            Some(super::PreviewUpdateBatch {
                generation: EditorPreviewGeneration::new(7),
                updates: vec![MapMakerUiUpdate::EditorMenuActionFocused(
                    tsfp_mapmaker::ui::EditorMenuAction::PreviewMap,
                )],
            })
        );
        assert!(super::take_session_updates(&mut session).is_none());
    }

    #[test]
    fn preview_page_change_invalidates_queued_updates_and_advances_generation() {
        let mut session = super::Session {
            active_page: Some((super::BUILDER, 12)),
            generation: 7,
            updates: vec![MapMakerUiUpdate::EditorPreviewSelectionRequested(
                tsfp_mapmaker::modes::PreviewSelection::from_row(tsfp_mapmaker::modes::PreviewRow(
                    2,
                ))
                .unwrap(),
            )],
        };
        super::change_active_page(&mut session, Some((super::BUILDER, 13)));
        assert_eq!(session.generation, 8);
        assert!(session.updates.is_empty());
        assert!(super::take_session_updates(&mut session).is_none());
    }
}
