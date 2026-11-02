//! T2408: attach the measured Palette Properties action page to owned state.
//! Labels, geometry, and callback effects remain with the live runtime seam.

use std::sync::Mutex;

use tsfp_mapmaker::ui::{
    EditorMenuGeneration, EditorSurface, EditorSurfaceGeneration, MapListGeneration, MapListRow,
    MapListRowId, MapLoadOutcome, MapMakerPage, MapMakerUiCommand, MapMakerUiUpdate,
    PalettePropertiesGeneration, PalettePropertyAction,
};
use tsfp_ui_input::live_page::{LivePage, LiveText};
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_model::mapmaker_palette_properties::{
    PalettePropertiesLayout, PalettePropertyActionLayout, build_palette_properties_page,
};
use tsfp_ui_model::model::Page;

const BUILDER: u32 = 0x32_8D20;
const SURFACES: &[EditorSurface] = &[
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
const ACTIONS: &[PalettePropertyAction] = &[
    PalettePropertyAction::LightRow,
    PalettePropertyAction::ChangePalette,
];

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
});

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeCatalogue {
    actions: Vec<PalettePropertyAction>,
    by_node: Vec<(u32, PalettePropertyAction)>,
}

fn action_for_label(label: u32) -> Option<PalettePropertyAction> {
    Some(match label {
        0x0BC4 => PalettePropertyAction::LightRow,
        0x0BC6 => PalettePropertyAction::ChangePalette,
        _ => return None,
    })
}

fn runtime_catalogue(live: &LivePage, cursor_nodes: &[u32]) -> Option<RuntimeCatalogue> {
    let mut actions = Vec::with_capacity(ACTIONS.len());
    let mut by_node = Vec::with_capacity(ACTIONS.len());
    for node in cursor_nodes {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let LiveText::Id(label) = &row.text else {
            return None;
        };
        let action = action_for_label(*label)?;
        if actions.contains(&action) {
            return None;
        }
        actions.push(action);
        by_node.push((*node, action));
    }
    (actions == ACTIONS).then_some(RuntimeCatalogue { actions, by_node })
}

pub fn observe_top(top: Option<(u32, u32)>) {
    let page = top.filter(|(builder, _)| *builder == BUILDER);
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    if session.active_page != page {
        session.active_page = page;
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

/// Admit the measured kind-A properties layout to the native default-style
/// drawer. Other selected-entry variants keep their original page draw.
pub fn native_draw_candidate(live: &LivePage, slot: u32, cursor_nodes: &[u32]) -> bool {
    let mapmaker_enabled = crate::storage_live::mapmaker_ui_enabled();
    let Ok(session) = SESSION.lock() else {
        return false;
    };
    native_draw_eligible(
        live,
        slot,
        cursor_nodes,
        mapmaker_enabled,
        session.active_page,
        session.generation,
    )
}

fn native_draw_eligible(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    mapmaker_enabled: bool,
    active_page: Option<(u32, u32)>,
    generation: u64,
) -> bool {
    mapmaker_enabled
        && live.builder == BUILDER
        && generation != 0
        && active_page == Some((BUILDER, slot))
        && runtime_catalogue(live, cursor_nodes).is_some()
}

fn owned_state(live: &LivePage, cursor_nodes: &[u32], generation: u64) -> Option<RuntimeCatalogue> {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let catalogue = runtime_catalogue(live, cursor_nodes)?;
    let mut state = crate::storage_live::mapmaker_ui().lock().ok()?;
    if state.page() != MapMakerPage::Editor {
        state.dispatch(MapMakerUiCommand::BeginEditor);
    }
    let surface_generation = EditorSurfaceGeneration::new(generation);
    if state.editor_surface_generation() != Some(surface_generation) {
        state
            .install_editor_surface_catalog(surface_generation, SURFACES.to_vec())
            .ok()?;
    }
    if state.editor_surface() != EditorSurface::PaletteProperties {
        let target = state.editor_surface_target(EditorSurface::PaletteProperties)?;
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(target));
    }
    let action_generation = PalettePropertiesGeneration::new(generation);
    if state.snapshot().palette_properties_generation != Some(action_generation) {
        state
            .install_palette_properties_catalog(action_generation, catalogue.actions.clone())
            .ok()?;
    }
    Some(catalogue)
}

/// Build the owned renderer model only for the measured two-action page. Other
/// selected-entry variants keep their caller-built page unchanged.
pub fn project_owned_page(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    page: Page,
) -> Option<Page> {
    if live.builder != BUILDER || !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let (generation, active_page) = SESSION
        .lock()
        .ok()
        .map(|session| (session.generation, session.active_page))?;
    if generation == 0 || active_page != Some((BUILDER, slot)) {
        return None;
    }
    let catalogue = owned_state(live, cursor_nodes, generation)?;
    if live.rows.len() != page.widgets.len() || live.rows.len() != catalogue.actions.len() {
        return None;
    }
    let actions = live
        .rows
        .iter()
        .zip(page.widgets.iter())
        .map(|(row, widget)| {
            let LiveText::Id(label) = &row.text else {
                return None;
            };
            Some(PalettePropertyActionLayout {
                action: action_for_label(*label)?,
                widget: widget.clone(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let state = crate::storage_live::mapmaker_ui().lock().ok()?;
    let projection = build_palette_properties_page(
        &state.snapshot(),
        page.id,
        PalettePropertiesLayout {
            widget_type: page.widget_type,
            panel_bounds: page.panel_bounds,
            nav: page.nav,
            wrap: page.wrap,
            actions,
        },
    )
    .ok()?;
    let mut owned_page = projection.page;
    owned_page.highlight_edges = page.highlight_edges;
    owned_page.handlers = page.handlers;
    owned_page.links = page.links;
    owned_page.enter = page.enter;
    owned_page.exit = page.exit;
    Some(owned_page)
}

pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder != BUILDER || !crate::storage_live::mapmaker_ui_enabled() {
        return;
    }
    let (generation, active_page) = match SESSION.lock() {
        Ok(session) => (session.generation, session.active_page),
        Err(_) => return,
    };
    if generation == 0 || active_page != Some((BUILDER, current.slot)) {
        return;
    }
    let Some(catalogue) = owned_state(&current.live, &current.cursor_nodes, generation) else {
        return;
    };
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    let mut repeat_at = [0; 4];
    for command in commands {
        match *command {
            NativeCommand::Focus(index) => {
                route_action(&mut state, &catalogue, &current.cursor_nodes, index, false);
            }
            NativeCommand::Select(index) => {
                route_action(&mut state, &catalogue, &current.cursor_nodes, index, true);
            }
            NativeCommand::Accept => {
                tsfp_ui_input::mapmaker_palette_properties::dispatch_frame(
                    &mut state,
                    tsfp_ui_input::IntentSet::default().with(tsfp_ui_input::MenuIntent::Accept),
                    tsfp_ui_input::IntentSet::default(),
                    0,
                    &mut repeat_at,
                );
            }
            NativeCommand::Back => {
                tsfp_ui_input::mapmaker_palette_properties::dispatch_pointer(
                    &mut state,
                    tsfp_ui_input::mapmaker_palette_properties::PointerEvent::Back,
                );
            }
            NativeCommand::Step(step) => {
                let intent = match step {
                    tsfp_ui_input::widgets::Step::Up => Some(tsfp_ui_input::MenuIntent::Up),
                    tsfp_ui_input::widgets::Step::Down => Some(tsfp_ui_input::MenuIntent::Down),
                    tsfp_ui_input::widgets::Step::Left | tsfp_ui_input::widgets::Step::Right => {
                        None
                    }
                };
                if let Some(intent) = intent {
                    tsfp_ui_input::mapmaker_palette_properties::dispatch_frame(
                        &mut state,
                        tsfp_ui_input::IntentSet::default().with(intent),
                        tsfp_ui_input::IntentSet::default(),
                        0,
                        &mut repeat_at,
                    );
                }
            }
            // Adjust, Scroll, ResumeGame and the page buttons (Delete, Shortcut, Join) do nothing here.
            _ => {}
        }
    }
}

fn route_action(
    state: &mut tsfp_mapmaker::ui::MapMakerUiState,
    catalogue: &RuntimeCatalogue,
    cursor_nodes: &[u32],
    index: u32,
    activate: bool,
) -> Option<MapMakerUiUpdate> {
    let Some(node) = cursor_nodes.get(index as usize) else {
        return None;
    };
    let Some((_, action)) = catalogue
        .by_node
        .iter()
        .find(|(candidate, _)| candidate == node)
    else {
        return None;
    };
    let Some(target) = state.palette_property_target(*action) else {
        return None;
    };
    let event = if activate {
        tsfp_ui_input::mapmaker_palette_properties::PointerEvent::Activate(Some(target))
    } else {
        tsfp_ui_input::mapmaker_palette_properties::PointerEvent::Hover(Some(target))
    };
    tsfp_ui_input::mapmaker_palette_properties::dispatch_pointer(state, event)
        .into_iter()
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_mapmaker::ui::{MapMakerUiState, MapMakerUiUpdate, PalettePropertyTarget};
    use tsfp_ui_input::live_page::{LiveOption, LiveRow};

    fn row(node: u32, label: u32) -> LiveRow {
        LiveRow {
            node,
            node_type: 7,
            id: node as i32,
            flags: 0,
            text: LiveText::Id(label),
            font: 0,
            value_font: 0,
            align: 0,
            rect: None,
            clip: None,
            value: None,
            activate: 0,
            draw: 0,
            kind: 0,
            selected: 0,
            options: Vec::<LiveOption>::new(),
            brightness: 0.0,
            height: 0.0,
            text_align: 0,
            text_x: 0.0,
            text_y: 0.0,
        }
    }

    #[test]
    fn t1690_catalogue_requires_the_measured_ordered_actions() {
        let live = LivePage {
            slot: 1,
            builder: BUILDER,
            state: 1,
            cursor_node: 10,
            rows: vec![row(10, 0x0BC4), row(11, 0x0BC6)],
            units: (1.0, 1.0),
        };
        let catalogue = runtime_catalogue(&live, &[10, 11]).unwrap();
        assert_eq!(catalogue.actions, ACTIONS);
        assert_eq!(runtime_catalogue(&live, &[11, 10]), None);
        assert_eq!(runtime_catalogue(&live, &[10]), None);
    }

    #[test]
    fn native_draw_shape_requires_exact_kind_a_action_rows() {
        let live = LivePage {
            slot: 1,
            builder: BUILDER,
            state: 1,
            cursor_node: 10,
            rows: vec![row(10, 0x0BC4), row(11, 0x0BC6)],
            units: (1.0, 1.0),
        };

        assert!(runtime_catalogue(&live, &[10, 11]).is_some());
        assert!(runtime_catalogue(&live, &[11, 10]).is_none());
        assert!(runtime_catalogue(&live, &[10]).is_none());
        assert!(
            runtime_catalogue(
                &LivePage {
                    rows: vec![row(10, 0x0BC4), row(11, 0x0BC8)],
                    ..live
                },
                &[10, 11]
            )
            .is_none()
        );
    }

    #[test]
    fn native_draw_requires_opt_in_active_page_and_live_generation() {
        let live = LivePage {
            slot: 4,
            builder: BUILDER,
            state: 1,
            cursor_node: 10,
            rows: vec![row(10, 0x0BC4), row(11, 0x0BC6)],
            units: (1.0, 1.0),
        };
        assert!(native_draw_eligible(
            &live,
            4,
            &[10, 11],
            true,
            Some((BUILDER, 4)),
            1
        ));
        assert!(!native_draw_eligible(
            &live,
            4,
            &[10, 11],
            false,
            Some((BUILDER, 4)),
            1
        ));
        assert!(!native_draw_eligible(
            &live,
            4,
            &[10, 11],
            true,
            Some((BUILDER, 3)),
            1
        ));
        assert!(!native_draw_eligible(
            &live,
            4,
            &[10, 11],
            true,
            Some((BUILDER, 4)),
            0
        ));
    }

    fn palette_state() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        let row = MapListRowId::new(MapListGeneration::new(3), 8);
        state
            .install_local_map_rows(MapListGeneration::new(3), vec![MapListRow { id: row }])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let MapMakerUiUpdate::LoadRequested(request) =
            state.dispatch(MapMakerUiCommand::SelectMap(row))
        else {
            panic!("selecting a map should start a typed load");
        };
        state.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_original_editor_menu(EditorMenuGeneration::new(4))
            .unwrap();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(4),
                vec![EditorSurface::PaletteProperties],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::PaletteProperties)
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(target));
        state
            .install_palette_properties_catalog(
                PalettePropertiesGeneration::new(5),
                ACTIONS.to_vec(),
            )
            .unwrap();
        state
    }

    #[test]
    fn native_focus_and_accept_update_generation_qualified_actions() {
        let mut state = palette_state();
        let catalogue = RuntimeCatalogue {
            actions: ACTIONS.to_vec(),
            by_node: vec![
                (10, PalettePropertyAction::LightRow),
                (11, PalettePropertyAction::ChangePalette),
            ],
        };
        assert_eq!(
            route_action(&mut state, &catalogue, &[10, 11], 1, false),
            Some(MapMakerUiUpdate::PalettePropertyFocused(
                PalettePropertyAction::ChangePalette
            ))
        );
        assert_eq!(
            route_action(&mut state, &catalogue, &[10, 11], 1, true),
            Some(MapMakerUiUpdate::PalettePageReentered)
        );
        assert_eq!(
            route_action(&mut state, &catalogue, &[10, 11], 2, true),
            None
        );
        assert_ne!(
            state.palette_property_target(PalettePropertyAction::ChangePalette),
            Some(PalettePropertyTarget::new(
                PalettePropertiesGeneration::new(99),
                PalettePropertyAction::ChangePalette,
            ))
        );
    }
}
