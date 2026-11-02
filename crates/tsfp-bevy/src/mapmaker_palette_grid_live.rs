// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach runtime Palette Grid rows to Rust-owned focus and selection state.
//!
//! T1690 distinguishes the shared builders and page descriptors but does not
//! establish palette-item semantics here. Row identity is therefore a
//! generation-scoped ordinal; the live page retains localization, geometry,
//! and its existing selection callback as the temporary effect seam.

use std::sync::Mutex;

use tsfp_mapmaker::ui::{
    EditorSurface, EditorSurfaceGeneration, MapMakerPage, MapMakerUiCommand, MapMakerUiState,
    MapMakerUiUpdate, PaletteGridChoiceId, PaletteGridGeneration,
};
use tsfp_ui_input::live_page::LivePage;
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_model::mapmaker_palette::{
    PaletteGridChoiceLayout, PaletteGridLayout, build_palette_grid_page,
};
use tsfp_ui_model::model::{Page, WidgetId};

const PALETTE_GRID_BUILDERS: [u32; 2] = [0x31_B8B0, 0x31_B7D0];
const EDITOR_SURFACES: &[EditorSurface] = &[
    EditorSurface::MapSettings,
    EditorSurface::EditorSettings,
    EditorSurface::Controls,
    EditorSurface::MapPreview,
    EditorSurface::PaletteGrid,
    EditorSurface::PaletteProperties,
    EditorSurface::SaveMap,
    EditorSurface::WeaponSets,
    EditorSurface::BotSets,
    EditorSurface::MapDescription,
    EditorSurface::Briefing,
];

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    row_nodes: Vec<u32>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
    row_nodes: Vec::new(),
});

fn is_palette_grid_builder(builder: u32) -> bool {
    PALETTE_GRID_BUILDERS.contains(&builder)
}

/// Advance page identity on entry, and invalidate installed row identity if
/// the live page rebuilds its selectable rows in a different order.
pub fn observe_top(top: Option<(u32, u32)>) {
    let page = top.filter(|(builder, _)| is_palette_grid_builder(*builder));
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    if session.active_page != page {
        session.active_page = page;
        session.row_nodes.clear();
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

/// Opaque per-page row ordinals. These do not decode palette IDs, editor
/// records, descriptor values, or serialized document state.
fn choice_ids_for_count(count: usize) -> Option<Vec<PaletteGridChoiceId>> {
    if count == 0 {
        return None;
    }
    Some(
        (0..count)
            .map(|index| PaletteGridChoiceId::new(index as u64))
            .collect(),
    )
}

fn active_generation(live: &LivePage, cursor_nodes: &[u32]) -> Option<u64> {
    if !is_palette_grid_builder(live.builder) || cursor_nodes.is_empty() {
        return None;
    }
    let mut session = SESSION.lock().ok()?;
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return None;
    }
    if session.row_nodes != cursor_nodes {
        session.row_nodes.clear();
        session.row_nodes.extend_from_slice(cursor_nodes);
        session.generation = session.generation.wrapping_add(1).max(1);
    }
    Some(session.generation)
}

fn choices_for_page(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<PaletteGridChoiceId>> {
    if !is_palette_grid_builder(live.builder) {
        return None;
    }
    if cursor_nodes
        .iter()
        .enumerate()
        .any(|(index, node)| cursor_nodes[..index].contains(node))
    {
        return None;
    }
    choice_ids_for_count(cursor_nodes.len())
}

fn ensure_editor_surface_catalog(state: &mut MapMakerUiState, generation: u64) -> bool {
    let surface_generation = EditorSurfaceGeneration::new(generation);
    state.editor_surface_generation() == Some(surface_generation)
        || state
            .install_editor_surface_catalog(surface_generation, EDITOR_SURFACES.to_vec())
            .is_ok()
}

fn ensure_catalog(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let Some(choices) = choices_for_page(live, cursor_nodes) else {
        return false;
    };
    let Some(generation) = active_generation(live, cursor_nodes) else {
        return false;
    };
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return false;
    };
    if state.page() != MapMakerPage::Editor {
        return false;
    }
    if !ensure_editor_surface_catalog(&mut state, generation) {
        return false;
    }
    if state.editor_surface() != EditorSurface::PaletteGrid {
        let Some(target) = state.editor_surface_target(EditorSurface::PaletteGrid) else {
            return false;
        };
        if !matches!(
            state.dispatch(MapMakerUiCommand::OpenEditorSurface(target)),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::PaletteGrid)
        ) {
            return false;
        }
    }
    state
        .install_palette_grid_catalog(PaletteGridGeneration::new(generation), choices)
        .is_ok()
}

fn page_layout(live: &LivePage, cursor_nodes: &[u32], page: &Page) -> Option<PaletteGridLayout> {
    let choices = choices_for_page(live, cursor_nodes)?;
    if choices.len() != page.widgets.len() {
        return None;
    }
    Some(PaletteGridLayout {
        widget_type: page.widget_type,
        panel_bounds: page.panel_bounds,
        nav: page.nav,
        wrap: page.wrap,
        choices: choices
            .into_iter()
            .zip(page.widgets.iter().cloned())
            .map(|(choice, widget)| PaletteGridChoiceLayout { choice, widget })
            .collect(),
    })
}

fn command_for_native(
    state: &MapMakerUiState,
    choices: &[PaletteGridChoiceId],
    command: NativeCommand,
) -> Option<MapMakerUiCommand> {
    let target = |choice| state.palette_grid_choice_target(choice);
    match command {
        NativeCommand::Focus(index) => usize::try_from(index)
            .ok()
            .and_then(|index| choices.get(index))
            .copied()
            .and_then(target)
            .map(MapMakerUiCommand::FocusPaletteGridChoice),
        NativeCommand::Select(index) => usize::try_from(index)
            .ok()
            .and_then(|index| choices.get(index))
            .copied()
            .and_then(target)
            .map(MapMakerUiCommand::ActivatePaletteGridChoice),
        NativeCommand::Accept => state
            .focused_palette_grid_choice()
            .and_then(target)
            .map(MapMakerUiCommand::ActivatePaletteGridChoice),
        NativeCommand::Back => Some(MapMakerUiCommand::Back),
        _ => None,
    }
}

fn dispatch_native_commands(
    state: &mut MapMakerUiState,
    choices: &[PaletteGridChoiceId],
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

/// Keep the page's original callback as the temporary selection-effect seam,
/// while native focus and activation update the typed Rust-owned state.
pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if !is_palette_grid_builder(current.descriptor.builder)
        || !ensure_catalog(&current.live, &current.cursor_nodes)
    {
        return;
    }
    let Some(choices) = choices_for_page(&current.live, &current.cursor_nodes) else {
        return;
    };
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    let _updates = dispatch_native_commands(&mut state, &choices, commands);
}

/// Project the active runtime palette rows through the renderer model, keeping
/// live values and all original page metadata while binding row identity to
/// the current Rust-owned choice generation.
pub fn project_owned_page(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    focused: Option<WidgetId>,
    page: Page,
) -> Option<Page> {
    if live.slot != slot || !is_palette_grid_builder(live.builder) {
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
        && let Some(target) = state.palette_grid_choice_target(choice)
    {
        let _ = state.dispatch(MapMakerUiCommand::FocusPaletteGridChoice(target));
    }
    let projection = build_palette_grid_page(&state.snapshot(), page.id, layout).ok()?;
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
    use super::{
        choice_ids_for_count, choices_for_page, dispatch_native_commands,
        ensure_editor_surface_catalog, page_layout,
    };
    use tsfp_mapmaker::ui::{
        EditorSurface, EditorSurfaceGeneration, MapMakerUiCommand, MapMakerUiState,
        MapMakerUiUpdate, PaletteGridChoiceId, PaletteGridGeneration,
    };
    use tsfp_ui_input::live_page::LivePage;
    use tsfp_ui_input::native_page::NativeCommand;
    use tsfp_ui_model::model::{Page, PageId, Widget, WidgetId, WidgetKind};

    #[test]
    fn runtime_rows_receive_only_page_local_opaque_ordinals() {
        assert_eq!(
            choice_ids_for_count(3),
            Some(vec![
                PaletteGridChoiceId::new(0),
                PaletteGridChoiceId::new(1),
                PaletteGridChoiceId::new(2),
            ])
        );
        assert_eq!(choice_ids_for_count(0), None);
    }

    #[test]
    fn live_widget_order_binds_to_opaque_rows_and_rejects_duplicate_targets() {
        let live = LivePage {
            slot: 12,
            builder: 0x31_B8B0,
            state: 1,
            cursor_node: 21,
            rows: Vec::new(),
            units: (1.0, 1.0),
        };
        let nodes = [21, 22];
        let mut page = Page::new(PageId(0x52_E000), 7);
        page.widgets
            .push(Widget::new(WidgetId(1), WidgetKind::Button));
        page.widgets
            .push(Widget::new(WidgetId(2), WidgetKind::Button));

        let layout = page_layout(&live, &nodes, &page).expect("matching runtime rows");
        assert_eq!(layout.choices[0].choice, PaletteGridChoiceId::new(0));
        assert_eq!(layout.choices[1].choice, PaletteGridChoiceId::new(1));
        assert!(choices_for_page(&live, &[21, 21]).is_none());
    }

    #[test]
    fn native_focus_activation_and_back_update_the_owned_palette_grid() {
        let choices = [PaletteGridChoiceId::new(0), PaletteGridChoiceId::new(1)];
        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::BeginEditor);
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(4),
                vec![EditorSurface::PaletteProperties, EditorSurface::PaletteGrid],
            )
            .unwrap();
        let properties = state
            .editor_surface_target(EditorSurface::PaletteProperties)
            .unwrap();
        assert_eq!(
            state.dispatch(MapMakerUiCommand::OpenEditorSurface(properties)),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::PaletteProperties)
        );
        // Opening a leaf editor page invalidates its action catalog. The live
        // Palette Grid attachment must restore the typed seam before routing.
        assert!(
            state
                .editor_surface_target(EditorSurface::PaletteGrid)
                .is_none()
        );
        assert!(ensure_editor_surface_catalog(&mut state, 9));
        let surface = state
            .editor_surface_target(EditorSurface::PaletteGrid)
            .unwrap();
        assert_eq!(
            state.dispatch(MapMakerUiCommand::OpenEditorSurface(surface)),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::PaletteGrid)
        );
        state
            .install_palette_grid_catalog(PaletteGridGeneration::new(9), choices.to_vec())
            .unwrap();

        assert_eq!(
            dispatch_native_commands(
                &mut state,
                &choices,
                &[NativeCommand::Focus(1), NativeCommand::Accept],
            ),
            vec![
                MapMakerUiUpdate::PaletteGridChoiceFocused(choices[1]),
                MapMakerUiUpdate::PaletteGridChoiceActivated(choices[1]),
            ]
        );
        assert_eq!(
            dispatch_native_commands(&mut state, &choices, &[NativeCommand::Back]),
            vec![MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::PaletteProperties
            )]
        );
    }
}
