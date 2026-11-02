// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach the loaded editor's Map Settings rows to typed Rust state.
//! Runtime labels, layout and option text remain the page-data seam; existing
//! handlers remain the effect seam until the Rule 24 adoption gate passes.

use std::sync::Mutex;

use tsfp_mapmaker::ui::{
    EDITOR_CHILD_SURFACES, EditorSurface, EditorSurfaceGeneration, MapMakerPage, MapMakerUiCommand,
    MapMakerUiState, MapSettingChoiceId, MapSettingField, MapSettingRow, MapSettingsAction,
    MapSettingsEntry, MapSettingsGeneration,
};
use tsfp_ui_input::live_page::{LivePage, LiveText};
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_model::mapmaker_settings::{
    MapSettingsEntryLayout, MapSettingsLayout, build_map_settings_page,
};
use tsfp_ui_model::model::Page;

const MAP_SETTINGS_BUILDER: u32 = 0x31_33E0;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
}

impl Session {
    fn observe(&mut self, top: Option<(u32, u32)>) {
        let page = top.filter(|(builder, _)| *builder == MAP_SETTINGS_BUILDER);
        if self.active_page != page {
            self.active_page = page;
            if page.is_some() {
                self.generation = self.generation.wrapping_add(1).max(1);
            }
        }
    }

    fn generation_for_settings_page(&mut self, builder: u32, slot: u32) -> Option<u64> {
        if builder != MAP_SETTINGS_BUILDER {
            return None;
        }
        let page = (builder, slot);
        self.observe(Some(page));
        (self.active_page == Some(page) && self.generation != 0).then_some(self.generation)
    }
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
});

/// Invalidate semantic row IDs whenever the live page leaves or reopens.
pub fn observe_top(top: Option<(u32, u32)>) {
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    session.observe(top);
}

fn generation_for_settings_page(builder: u32, slot: u32) -> Option<u64> {
    SESSION
        .lock()
        .ok()?
        .generation_for_settings_page(builder, slot)
}

fn entry_for_label(label: u32) -> Option<MapSettingsEntry> {
    use MapSettingField as F;
    use MapSettingsAction as A;
    use MapSettingsEntry::{Action, Field};
    Some(match label {
        0x0AB9 => Field(F::Theme),
        0x03B7 => Field(F::WeaponSet),
        0x03B8 => Field(F::BotSet),
        0x0AC6 => Field(F::DisableModes),
        0x0ABB => Field(F::Music),
        0x0ABC => Field(F::SpawnSounds),
        0x0ABE => Field(F::Sky),
        0x0ABF => Field(F::IndoorLevel),
        0x0AC0 => Field(F::Weather),
        0x0B08 => Action(A::ViewEditWeaponSets),
        0x0B09 => Action(A::ViewEditBotSets),
        0x0AC5 => Action(A::EditMapDescription),
        0x03DB => Action(A::Briefing),
        _ => return None,
    })
}

fn label_entry(row: &tsfp_ui_input::live_page::LiveRow) -> Option<MapSettingsEntry> {
    match &row.text {
        LiveText::Id(label) => entry_for_label(*label),
        LiveText::None | LiveText::Bytes(_) => None,
    }
}

#[derive(Clone, Debug)]
struct RuntimeCatalogue {
    entries: Vec<MapSettingsEntry>,
    rows: Vec<MapSettingRow>,
    by_node: Vec<(u32, MapSettingsEntry)>,
}

fn runtime_catalogue(live: &LivePage, cursor_nodes: &[u32]) -> Option<RuntimeCatalogue> {
    let mut entries = Vec::with_capacity(cursor_nodes.len());
    let mut rows = Vec::new();
    let mut by_node = Vec::with_capacity(cursor_nodes.len());
    for node in cursor_nodes {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let entry = label_entry(row)?;
        if entries.contains(&entry) {
            return None;
        }
        entries.push(entry);
        by_node.push((*node, entry));
        if let MapSettingsEntry::Field(field) = entry {
            let choices: Vec<_> = (0..row.options.len())
                .map(|index| MapSettingChoiceId::new(index as u64))
                .collect();
            let selected = choices.get(row.selected as usize).copied();
            rows.push(MapSettingRow {
                field,
                choices,
                selected,
            });
        }
    }
    (!entries.is_empty()).then_some(RuntimeCatalogue {
        entries,
        rows,
        by_node,
    })
}

fn install_owned_state(
    state: &mut MapMakerUiState,
    catalogue: &RuntimeCatalogue,
    generation: u64,
) -> Option<()> {
    if state.page() != MapMakerPage::Editor {
        state.dispatch(MapMakerUiCommand::BeginEditor);
    }
    let surface_generation = EditorSurfaceGeneration::new(generation);
    if state.editor_surface_generation() != Some(surface_generation) {
        state
            .install_editor_surface_catalog(surface_generation, EDITOR_CHILD_SURFACES.to_vec())
            .ok()?;
    }
    if state.editor_surface() != EditorSurface::MapSettings {
        let target = state.editor_surface_target(EditorSurface::MapSettings)?;
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(target));
    }
    // Opening Map Settings clears the prior surface catalog. Restore it on
    // this pass so the first text-action input can resolve its destination.
    if state.editor_surface_generation() != Some(surface_generation) {
        state
            .install_editor_surface_catalog(surface_generation, EDITOR_CHILD_SURFACES.to_vec())
            .ok()?;
    }
    let settings_generation = MapSettingsGeneration::new(generation);
    if state.map_settings_generation() != Some(settings_generation) {
        state
            .install_map_settings_page(
                settings_generation,
                catalogue.rows.clone(),
                catalogue.entries.clone(),
            )
            .ok()?;
    }
    Some(())
}

fn owned_state(live: &LivePage, cursor_nodes: &[u32], generation: u64) -> Option<RuntimeCatalogue> {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let catalogue = runtime_catalogue(live, cursor_nodes)?;
    let mut state = crate::storage_live::mapmaker_ui().lock().ok()?;
    install_owned_state(&mut state, &catalogue, generation)?;
    Some(catalogue)
}

/// Project the live Map Settings page through Rust-owned typed focus and action
/// state while preserving runtime labels, header rows, geometry and callbacks.
pub fn project_owned_map_settings_page(
    live: &LivePage,
    cursor_nodes: &[u32],
    page: Page,
) -> Option<Page> {
    if live.builder != MAP_SETTINGS_BUILDER || !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    // The projection itself is authoritative evidence that this live page is
    // current. Initialize its generation here so an input event on the first
    // frame does not depend on the observer having run in an earlier frame.
    let generation = generation_for_settings_page(live.builder, live.slot)?;
    let catalogue = owned_state(live, cursor_nodes, generation)?;
    if page.widgets.len() != live.rows.len() {
        return None;
    }
    let mut layout_entries = Vec::with_capacity(page.widgets.len());
    for (row, widget) in live.rows.iter().zip(page.widgets.iter()) {
        let entry = catalogue
            .by_node
            .iter()
            .find(|(node, _)| *node == row.node)
            .map(|(_, entry)| *entry);
        layout_entries.push(MapSettingsEntryLayout {
            entry,
            widget: widget.clone(),
        });
    }
    let state = crate::storage_live::mapmaker_ui().lock().ok()?;
    let projection = build_map_settings_page(
        &state.snapshot(),
        page.id,
        MapSettingsLayout {
            widget_type: page.widget_type,
            panel_bounds: page.panel_bounds,
            nav: page.nav,
            wrap: page.wrap,
            entries: layout_entries,
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

/// Apply semantic live focus/activation to the owned settings state. Existing
/// callbacks still run through the original caller seam until adoption.
pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder != MAP_SETTINGS_BUILDER
        || !crate::storage_live::mapmaker_ui_enabled()
    {
        return;
    }
    // Input can be the first consumer after the page is pushed. Synchronize
    // from the current live page before resolving generation-bound row IDs.
    let Some(generation) = generation_for_settings_page(current.descriptor.builder, current.slot)
    else {
        return;
    };
    let Some(catalogue) = owned_state(&current.live, &current.cursor_nodes, generation) else {
        return;
    };
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    for command in commands {
        match *command {
            NativeCommand::Focus(index) => {
                focus_at(&mut state, &catalogue, &current.cursor_nodes, index);
            }
            NativeCommand::Select(index) => {
                if let Some(entry) = entry_at(&catalogue, &current.cursor_nodes, index) {
                    activate_entry(&mut state, entry);
                }
            }
            NativeCommand::Accept => {
                if let Some(entry) = state.focused_map_settings_entry() {
                    activate_entry(&mut state, entry);
                }
            }
            NativeCommand::Step(step) => match step {
                tsfp_ui_input::widgets::Step::Up | tsfp_ui_input::widgets::Step::Down => {
                    let entries = &catalogue.entries;
                    let current_index = state
                        .focused_map_settings_entry()
                        .and_then(|focused| entries.iter().position(|entry| *entry == focused));
                    let next = match (current_index, step) {
                        (Some(index), tsfp_ui_input::widgets::Step::Up) => index.checked_sub(1),
                        (Some(index), tsfp_ui_input::widgets::Step::Down) => index.checked_add(1),
                        (None, tsfp_ui_input::widgets::Step::Down) => Some(0),
                        (None, tsfp_ui_input::widgets::Step::Up) => entries.len().checked_sub(1),
                        _ => None,
                    }
                    .filter(|index| *index < entries.len());
                    if let Some(entry) = next.and_then(|index| entries.get(index)).copied() {
                        focus_entry(&mut state, entry);
                    }
                }
                tsfp_ui_input::widgets::Step::Left | tsfp_ui_input::widgets::Step::Right => {
                    adjust_focused_choice(&mut state, step == tsfp_ui_input::widgets::Step::Right);
                }
            },
            NativeCommand::Adjust { row, right } => {
                focus_at(&mut state, &catalogue, &current.cursor_nodes, row);
                adjust_focused_choice(&mut state, right);
            }
            NativeCommand::Back => {
                state.dispatch(MapMakerUiCommand::Back);
            }
            _ => {} // Scroll, ResumeGame, Delete and Join are not settings actions
        }
    }
}

fn entry_at(
    catalogue: &RuntimeCatalogue,
    cursor_nodes: &[u32],
    index: u32,
) -> Option<MapSettingsEntry> {
    let node = *cursor_nodes.get(index as usize)?;
    catalogue
        .by_node
        .iter()
        .find(|(candidate, _)| *candidate == node)
        .map(|(_, entry)| *entry)
}

fn focus_at(
    state: &mut tsfp_mapmaker::ui::MapMakerUiState,
    catalogue: &RuntimeCatalogue,
    cursor_nodes: &[u32],
    index: u32,
) {
    if let Some(entry) = entry_at(catalogue, cursor_nodes, index) {
        focus_entry(state, entry);
    }
}

fn focus_entry(state: &mut tsfp_mapmaker::ui::MapMakerUiState, entry: MapSettingsEntry) {
    if let Some(target) = state.map_settings_entry_target(entry) {
        state.dispatch(MapMakerUiCommand::FocusMapSettingsEntry(target));
    }
}

fn activate_entry(state: &mut tsfp_mapmaker::ui::MapMakerUiState, entry: MapSettingsEntry) {
    if let Some(target) = state.map_settings_entry_target(entry) {
        match entry {
            MapSettingsEntry::Field(_) => {
                state.dispatch(MapMakerUiCommand::FocusMapSettingsEntry(target));
            }
            MapSettingsEntry::Action(_) => {
                state.dispatch(MapMakerUiCommand::ActivateMapSettingsAction(target));
            }
        }
    }
}

fn adjust_focused_choice(state: &mut tsfp_mapmaker::ui::MapMakerUiState, forward: bool) {
    let Some(MapSettingsEntry::Field(field)) = state.focused_map_settings_entry() else {
        return;
    };
    let Some(row) = state.map_settings().iter().find(|row| row.field == field) else {
        return;
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
    .filter(|index| *index < row.choices.len());
    let (Some(index), Some(target)) = (next, state.map_setting_target(field)) else {
        return;
    };
    let choice = row.choices[index];
    state.dispatch(MapMakerUiCommand::SelectMapSetting { target, choice });
}

#[cfg(test)]
mod tests {
    use super::{
        EDITOR_CHILD_SURFACES, MAP_SETTINGS_BUILDER, RuntimeCatalogue, Session, entry_for_label,
        install_owned_state, runtime_catalogue,
    };
    use tsfp_mapmaker::ui::{
        EditorSurface, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate, MapSettingChoiceId,
        MapSettingField, MapSettingRow, MapSettingsAction, MapSettingsEntry as E,
    };
    use tsfp_ui_input::live_page::{LiveOption, LivePage, LiveRow, LiveText};

    fn editor_surface_belongs_in_catalog(surface: EditorSurface) -> bool {
        use EditorSurface as S;
        match surface {
            // The editor menu is the root page and routes through typed menu
            // actions; it is deliberately not an editor child-page target.
            S::MainMenu => false,
            S::MapPreview
            | S::PaletteGrid
            | S::SaveMap
            | S::MapSettings
            | S::EditorSettings
            | S::Controls
            | S::PaletteProperties
            | S::WeaponSets
            | S::BotSets
            | S::MapDescription
            | S::Briefing => EDITOR_CHILD_SURFACES.contains(&surface),
        }
    }

    #[test]
    fn editor_surface_catalog_classifies_every_typed_surface() {
        use EditorSurface as S;
        let all = [
            S::MainMenu,
            S::MapPreview,
            S::PaletteGrid,
            S::SaveMap,
            S::MapSettings,
            S::EditorSettings,
            S::Controls,
            S::PaletteProperties,
            S::WeaponSets,
            S::BotSets,
            S::MapDescription,
            S::Briefing,
        ];

        assert_eq!(EDITOR_CHILD_SURFACES.len(), all.len() - 1);
        for surface in all {
            assert_eq!(
                EDITOR_CHILD_SURFACES.contains(&surface),
                editor_surface_belongs_in_catalog(surface),
                "typed editor surface {surface:?} has incorrect shared-catalog membership"
            );
        }
    }

    fn row(node: u32, label: u32, options: Vec<u32>, selected: u32) -> LiveRow {
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
            selected,
            options: options
                .into_iter()
                .enumerate()
                .map(|(index, id)| LiveOption {
                    id,
                    text: LiveText::Id(id),
                    enabled: true,
                })
                .collect(),
            brightness: 0.0,
            height: 0.0,
            text_align: 0,
            text_x: 0.0,
            text_y: 0.0,
        }
    }

    #[test]
    fn t1714_map_settings_labels_have_typed_row_identities() {
        let labels = [
            (0x0AB9, E::Field(MapSettingField::Theme)),
            (0x03B7, E::Field(MapSettingField::WeaponSet)),
            (0x03B8, E::Field(MapSettingField::BotSet)),
            (0x0AC6, E::Field(MapSettingField::DisableModes)),
            (0x0ABB, E::Field(MapSettingField::Music)),
            (0x0ABC, E::Field(MapSettingField::SpawnSounds)),
            (0x0ABE, E::Field(MapSettingField::Sky)),
            (0x0ABF, E::Field(MapSettingField::IndoorLevel)),
            (0x0AC0, E::Field(MapSettingField::Weather)),
            (0x0B08, E::Action(MapSettingsAction::ViewEditWeaponSets)),
            (0x0B09, E::Action(MapSettingsAction::ViewEditBotSets)),
            (0x0AC5, E::Action(MapSettingsAction::EditMapDescription)),
            (0x03DB, E::Action(MapSettingsAction::Briefing)),
        ];
        for (label, entry) in labels {
            assert_eq!(entry_for_label(label), Some(entry));
        }
        assert_eq!(entry_for_label(0x036B), None); // section heading
        assert_eq!(entry_for_label(0xFFFF), None);
    }

    #[test]
    fn settings_choice_ids_are_runtime_ordinals_not_guest_values() {
        let live = LivePage {
            slot: 2,
            builder: 0x3133E0,
            state: 1,
            cursor_node: 11,
            rows: vec![row(11, 0x0AB9, vec![0xAA, 0xBB], 1)],
            units: (1.0, 1.0),
        };
        let catalogue = runtime_catalogue(&live, &[11]).unwrap();
        assert_eq!(catalogue.entries, [E::Field(MapSettingField::Theme)]);
        assert_eq!(catalogue.rows[0].choices[0].get(), 0);
        assert_eq!(catalogue.rows[0].choices[1].get(), 1);
        assert_eq!(catalogue.rows[0].selected.unwrap().get(), 1);
    }

    #[test]
    fn first_map_settings_click_opens_text_action_from_a_cold_page() {
        for (action, surface) in [
            (MapSettingsAction::Briefing, EditorSurface::Briefing),
            (
                MapSettingsAction::EditMapDescription,
                EditorSurface::MapDescription,
            ),
        ] {
            let mut session = Session::default();
            assert_eq!(session.generation, 0);
            let generation = session
                .generation_for_settings_page(MAP_SETTINGS_BUILDER, 2)
                .expect("the first live Map Settings event initializes its generation");

            let mut state = MapMakerUiState::default();
            let catalogue = RuntimeCatalogue {
                entries: vec![E::Action(action)],
                rows: Vec::new(),
                by_node: Vec::new(),
            };
            install_owned_state(&mut state, &catalogue, generation)
                .expect("cold settings page installs its full typed state");

            let target = state.map_settings_entry_target(E::Action(action)).unwrap();
            assert_eq!(
                state.dispatch(MapMakerUiCommand::ActivateMapSettingsAction(target)),
                MapMakerUiUpdate::MapSettingsActionOpened { action, surface }
            );
            assert_eq!(state.editor_surface(), surface);
        }
    }

    #[test]
    fn keyboard_accept_opens_every_typed_map_settings_action_from_cold_page() {
        use tsfp_ui_input::input::{IntentSet, MenuIntent};

        for (action, surface) in [
            (
                MapSettingsAction::ViewEditWeaponSets,
                EditorSurface::WeaponSets,
            ),
            (MapSettingsAction::ViewEditBotSets, EditorSurface::BotSets),
            (
                MapSettingsAction::EditMapDescription,
                EditorSurface::MapDescription,
            ),
            (MapSettingsAction::Briefing, EditorSurface::Briefing),
        ] {
            let mut session = Session::default();
            let generation = session
                .generation_for_settings_page(MAP_SETTINGS_BUILDER, 2)
                .expect("the first live Map Settings event initializes its generation");
            let mut state = MapMakerUiState::default();
            let catalogue = RuntimeCatalogue {
                entries: vec![E::Field(MapSettingField::Theme), E::Action(action)],
                rows: vec![MapSettingRow {
                    field: MapSettingField::Theme,
                    choices: vec![MapSettingChoiceId::new(0)],
                    selected: Some(MapSettingChoiceId::new(0)),
                }],
                by_node: Vec::new(),
            };
            install_owned_state(&mut state, &catalogue, generation)
                .expect("cold settings page installs its full typed state");

            let mut repeat = [0; 4];
            let down = IntentSet::default().with(MenuIntent::Down);
            assert_eq!(
                tsfp_ui_input::mapmaker_settings::dispatch_frame(
                    &mut state,
                    down,
                    IntentSet::default(),
                    1,
                    &mut repeat,
                ),
                vec![
                    tsfp_mapmaker::ui::MapMakerUiUpdate::MapSettingsEntryFocused(E::Action(action))
                ]
            );

            let accept = IntentSet::default().with(MenuIntent::Accept);
            assert_eq!(
                tsfp_ui_input::mapmaker_settings::dispatch_frame(
                    &mut state,
                    accept,
                    IntentSet::default(),
                    2,
                    &mut repeat,
                ),
                vec![
                    tsfp_mapmaker::ui::MapMakerUiUpdate::MapSettingsActionOpened {
                        action,
                        surface,
                    }
                ]
            );
            assert_eq!(state.editor_surface(), surface);
        }
    }
}
