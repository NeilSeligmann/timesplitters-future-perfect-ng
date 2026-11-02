// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach typed Rust Editor Settings rows to the live renderer page.
//! Runtime text, option presentation, and geometry remain inputs; the existing
//! page callbacks are the temporary effect seam until Rule 24 adoption.

use std::sync::Mutex;

use tsfp_mapmaker::modes::EditorKind;
use tsfp_mapmaker::ui::{
    EditorConfirmationChoice, EditorSettingChoiceId, EditorSettingField, EditorSettingRow,
    EditorSettingsEditEffect, EditorSettingsEditPhase, EditorSettingsGeneration, EditorSurface,
    EditorSurfaceGeneration, MapMakerPage, MapMakerUiCommand,
};
use tsfp_ui_input::live_page::{LivePage, LiveText};
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_model::mapmaker_editor_settings::{
    EditorSettingsLayout, EditorSettingsWidgetLayout, build_editor_kind_confirmation_page,
    build_editor_settings_page,
};
use tsfp_ui_model::model::{Page, WidgetId};

const EDITOR_SETTINGS_BUILDER: u32 = 0x31_2270;
const MAPMAKER_CONFIRM_BUILDER: u32 = 0x7_A080;
const CLEAR_MAP_YES_HANDLER: u32 = 0x31_2120;
const CLEAR_MAP_NO_HANDLER: u32 = 0x36_1710;
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
const REQUIRED_FIELDS: &[EditorSettingField] = &[
    EditorSettingField::GridIntensity,
    EditorSettingField::Music,
    EditorSettingField::Sounds,
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

pub fn observe_top(top: Option<(u32, u32)>) {
    let page = top.filter(|(builder, _)| *builder == EDITOR_SETTINGS_BUILDER);
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

fn field_for_label(label: u32) -> Option<EditorSettingField> {
    Some(match label {
        0x0AB5 => EditorSettingField::GridIntensity,
        0x0ABB => EditorSettingField::Music,
        0x0AB7 => EditorSettingField::Sounds,
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeCatalogue {
    rows: Vec<EditorSettingRow>,
    by_node: Vec<(u32, EditorSettingField)>,
    mode: RuntimeEditorKindRow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeEditorKindRow {
    node: u32,
    current: EditorKind,
    choices: Vec<EditorKind>,
}

fn runtime_catalogue(live: &LivePage, cursor_nodes: &[u32]) -> Option<RuntimeCatalogue> {
    let mut rows = Vec::with_capacity(REQUIRED_FIELDS.len());
    let mut by_node = Vec::with_capacity(REQUIRED_FIELDS.len());
    let mut mode = None;
    for node in cursor_nodes {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        if matches!(&row.text, LiveText::Id(0x0B01)) {
            if mode.is_some()
                || row.options.len() != 2
                || !matches!(&row.options[0].text, LiveText::Id(0x0AFF))
                || !matches!(&row.options[1].text, LiveText::Id(0x0B00))
                || row.selected >= 2
            {
                return None;
            }
            let choices = vec![EditorKind(0), EditorKind(1)];
            mode = Some(RuntimeEditorKindRow {
                node: *node,
                current: choices[row.selected as usize],
                choices,
            });
            continue;
        }
        let Some(field) = row.text_field() else {
            continue;
        };
        if rows
            .iter()
            .any(|candidate: &EditorSettingRow| candidate.field == field)
        {
            return None;
        }
        let choices: Vec<_> = (0..row.options.len())
            .map(|index| EditorSettingChoiceId::new(index as u64))
            .collect();
        let selected = choices.get(row.selected as usize).copied();
        rows.push(EditorSettingRow {
            field,
            choices,
            selected,
        });
        by_node.push((*node, field));
    }
    let fields: Vec<_> = rows.iter().map(|row| row.field).collect();
    (fields == REQUIRED_FIELDS).then_some(RuntimeCatalogue {
        rows,
        by_node,
        mode: mode?,
    })
}

trait EditorSettingsRowField {
    fn text_field(&self) -> Option<EditorSettingField>;
}

impl EditorSettingsRowField for tsfp_ui_input::live_page::LiveRow {
    fn text_field(&self) -> Option<EditorSettingField> {
        match &self.text {
            LiveText::Id(label) => field_for_label(*label),
            LiveText::None | LiveText::Bytes(_) => None,
        }
    }
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
            .install_editor_surface_catalog(surface_generation, EDITOR_SURFACES.to_vec())
            .ok()?;
    }
    if state.editor_surface() != EditorSurface::EditorSettings {
        let target = state.editor_surface_target(EditorSurface::EditorSettings)?;
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(target));
    }
    let settings_generation = EditorSettingsGeneration::new(generation);
    state
        .install_editor_settings(settings_generation, catalogue.rows.clone())
        .ok()?;
    let needs_edit = state
        .editor_settings_edit()
        .is_none_or(|edit| edit.phase() == EditorSettingsEditPhase::Finished);
    if needs_edit
        && !state.begin_editor_settings_edit(
            catalogue.mode.current,
            false,
            catalogue.mode.choices.clone(),
        )
    {
        return None;
    }
    Some(catalogue)
}

/// Build the Editor Settings page from Rust-owned typed rows while preserving
/// caller-provided localized text, option display, layout, and framework data.
pub fn project_owned_editor_settings_page(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    page: Page,
) -> Option<Page> {
    if live.builder != EDITOR_SETTINGS_BUILDER || !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let (generation, active_page) = SESSION
        .lock()
        .ok()
        .map(|session| (session.generation, session.active_page))?;
    if generation == 0 || active_page != Some((EDITOR_SETTINGS_BUILDER, slot)) {
        return None;
    }
    let catalogue = owned_state(live, cursor_nodes, generation)?;
    if page.widgets.len() != live.rows.len() {
        return None;
    }
    let mode_widget = live
        .rows
        .iter()
        .position(|row| row.node == catalogue.mode.node)
        .and_then(|index| page.widgets.get(index))
        .map(|widget| widget.id)?;
    let widgets = live
        .rows
        .iter()
        .zip(page.widgets.iter())
        .map(|(row, widget)| {
            let field = catalogue
                .by_node
                .iter()
                .find(|(node, _)| *node == row.node)
                .map(|(_, field)| *field);
            EditorSettingsWidgetLayout {
                field,
                widget: widget.clone(),
            }
        })
        .collect();
    let state = crate::storage_live::mapmaker_ui().lock().ok()?;
    let projection = build_editor_settings_page(
        &state.snapshot(),
        page.id,
        EditorSettingsLayout {
            widget_type: page.widget_type,
            panel_bounds: page.panel_bounds,
            nav: page.nav,
            wrap: page.wrap,
            mode_widget: Some(mode_widget),
            widgets,
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

/// Match only the live two-choice editor-kind confirmation registered by
/// `popup_descriptor`. The handler pair is the T1688/T2333 Yes/No contract.
fn confirmation_widgets(live: &LivePage, cursor_nodes: &[u32]) -> Option<(WidgetId, WidgetId)> {
    if live.state != 2 || cursor_nodes.len() != 2 {
        return None;
    }
    let mut yes = None;
    let mut no = None;
    for node in cursor_nodes {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let index = live
            .rows
            .iter()
            .position(|candidate| candidate.node == *node)?;
        match row.activate {
            CLEAR_MAP_YES_HANDLER if yes.is_none() => {
                yes = Some(tsfp_ui_input::live_page::widget_id(index));
            }
            CLEAR_MAP_NO_HANDLER if no.is_none() => {
                no = Some(tsfp_ui_input::live_page::widget_id(index));
            }
            _ => return None,
        }
    }
    Some((no?, yes?))
}

/// Keep the generic runtime prompt's text, layout and widgets, while binding
/// its measured Yes/No actions to the active Rust mode-change transaction.
pub fn project_owned_editor_kind_confirmation_page(
    live: &LivePage,
    cursor_nodes: &[u32],
    page: Page,
) -> Option<Page> {
    if live.builder != 0 || !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let (cancel_widget, accept_widget) = confirmation_widgets(live, cursor_nodes)?;
    let state = crate::storage_live::mapmaker_ui().lock().ok()?;
    if state.editor_settings_edit()?.phase() != EditorSettingsEditPhase::AwaitingClearConfirmation {
        return None;
    }
    let projection =
        build_editor_kind_confirmation_page(&state.snapshot(), page, cancel_widget, accept_widget)
            .ok()?;
    Some(projection.page)
}

fn apply_confirmation_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder != MAPMAKER_CONFIRM_BUILDER
        || !crate::storage_live::mapmaker_ui_enabled()
    {
        return;
    }
    let Some((cancel_widget, accept_widget)) =
        confirmation_widgets(&current.live, &current.cursor_nodes)
    else {
        return;
    };
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    if state
        .editor_settings_edit()
        .is_none_or(|edit| edit.phase() != EditorSettingsEditPhase::AwaitingClearConfirmation)
    {
        return;
    }
    let Some(page) = current
        .content
        .model
        .pages
        .get(&current.content.page)
        .cloned()
    else {
        return;
    };
    let Ok(projection) =
        build_editor_kind_confirmation_page(&state.snapshot(), page, cancel_widget, accept_widget)
    else {
        return;
    };
    for command in commands {
        let widget = match command {
            NativeCommand::Select(index) => {
                Some(tsfp_ui_input::live_page::widget_id(*index as usize))
            }
            NativeCommand::Accept => current.focused,
            NativeCommand::Back => Some(cancel_widget),
            NativeCommand::Focus(_)
            | NativeCommand::Step(_)
            | NativeCommand::Adjust { .. }
            | NativeCommand::Scroll(_)
            | NativeCommand::Delete
            | NativeCommand::Shortcut
            | NativeCommand::Join(_)
            | NativeCommand::ResumeGame => None,
        };
        let Some(widget) = widget else {
            continue;
        };
        if matches!(
            projection.choice_for_widget(widget),
            Some(EditorConfirmationChoice::Cancel | EditorConfirmationChoice::Accept)
        ) {
            let updates = projection.activate_widget(&mut state, widget);
            if updates.iter().any(|update| {
                matches!(
                    update,
                    tsfp_mapmaker::ui::MapMakerUiUpdate::EditorSettingsEditEffect(
                        EditorSettingsEditEffect::CloseSettings
                    )
                )
            }) {
                state.dispatch(MapMakerUiCommand::Back);
            }
        }
    }
}

pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    apply_confirmation_commands(current, commands);
    if current.descriptor.builder != EDITOR_SETTINGS_BUILDER
        || !crate::storage_live::mapmaker_ui_enabled()
    {
        return;
    }
    let (generation, active_page) = match SESSION.lock() {
        Ok(session) => (session.generation, session.active_page),
        Err(_) => return,
    };
    if generation == 0 || active_page != Some((EDITOR_SETTINGS_BUILDER, current.slot)) {
        return;
    }
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
                focus_at(&mut state, &catalogue, &current.cursor_nodes, index);
            }
            NativeCommand::Accept => {
                if let Some(field) = state.focused_editor_setting()
                    && let Some(target) = state.editor_setting_target(field)
                {
                    state.dispatch(MapMakerUiCommand::FocusEditorSetting(target));
                }
            }
            NativeCommand::Step(step) => match step {
                tsfp_ui_input::widgets::Step::Up | tsfp_ui_input::widgets::Step::Down => {
                    focus_next(&mut state, &catalogue, step);
                }
                tsfp_ui_input::widgets::Step::Left | tsfp_ui_input::widgets::Step::Right => {
                    let index = current.focused.and_then(|widget| {
                        current
                            .live
                            .cursor_index(widget.0 as usize, &current.cursor_nodes)
                    });
                    adjust_focused_choice(
                        &mut state,
                        &catalogue,
                        &current.cursor_nodes,
                        index,
                        step == tsfp_ui_input::widgets::Step::Right,
                    );
                }
            },
            NativeCommand::Adjust { row, right } => {
                focus_at(&mut state, &catalogue, &current.cursor_nodes, row);
                adjust_focused_choice(
                    &mut state,
                    &catalogue,
                    &current.cursor_nodes,
                    Some(row),
                    right,
                );
            }
            NativeCommand::Back => {
                let requesting_confirmation = state
                    .leave_editor_settings_edit()
                    .contains(&EditorSettingsEditEffect::RequestClearMapConfirmation);
                if !requesting_confirmation {
                    state.dispatch(MapMakerUiCommand::Back);
                }
            }
            _ => {} // Scroll, ResumeGame, Delete and Join are not settings actions
        }
    }
}

fn field_at(
    catalogue: &RuntimeCatalogue,
    cursor_nodes: &[u32],
    index: u32,
) -> Option<EditorSettingField> {
    let node = *cursor_nodes.get(index as usize)?;
    catalogue
        .by_node
        .iter()
        .find(|(candidate, _)| *candidate == node)
        .map(|(_, field)| *field)
}

fn focus_at(
    state: &mut tsfp_mapmaker::ui::MapMakerUiState,
    catalogue: &RuntimeCatalogue,
    cursor_nodes: &[u32],
    index: u32,
) {
    if let Some(field) = field_at(catalogue, cursor_nodes, index)
        && let Some(target) = state.editor_setting_target(field)
    {
        state.dispatch(MapMakerUiCommand::FocusEditorSetting(target));
    }
}

fn focus_next(
    state: &mut tsfp_mapmaker::ui::MapMakerUiState,
    catalogue: &RuntimeCatalogue,
    step: tsfp_ui_input::widgets::Step,
) {
    let rows = state.editor_settings();
    let current = state
        .focused_editor_setting()
        .and_then(|field| rows.iter().position(|row| row.field == field));
    let next = match (current, step) {
        (Some(index), tsfp_ui_input::widgets::Step::Up) => index.checked_sub(1),
        (Some(index), tsfp_ui_input::widgets::Step::Down) => index.checked_add(1),
        (None, tsfp_ui_input::widgets::Step::Down) => Some(0),
        (None, tsfp_ui_input::widgets::Step::Up) => rows.len().checked_sub(1),
        _ => None,
    }
    .filter(|index| *index < rows.len());
    if let Some(field) = next.and_then(|index| rows.get(index)).map(|row| row.field)
        && catalogue
            .by_node
            .iter()
            .any(|(_, candidate)| *candidate == field)
        && let Some(target) = state.editor_setting_target(field)
    {
        state.dispatch(MapMakerUiCommand::FocusEditorSetting(target));
    }
}

fn adjust_focused_choice(
    state: &mut tsfp_mapmaker::ui::MapMakerUiState,
    catalogue: &RuntimeCatalogue,
    cursor_nodes: &[u32],
    row_index: Option<u32>,
    forward: bool,
) {
    if row_index
        .and_then(|index| cursor_nodes.get(index as usize))
        .is_some_and(|node| *node == catalogue.mode.node)
    {
        let selected = state
            .editor_settings_edit()
            .map(|edit| edit.requested())
            .unwrap_or(catalogue.mode.current);
        let current = catalogue
            .mode
            .choices
            .iter()
            .position(|choice| *choice == selected);
        let next = match (current, forward) {
            (Some(index), true) => index.checked_add(1),
            (Some(index), false) => index.checked_sub(1),
            _ => None,
        }
        .filter(|index| *index < catalogue.mode.choices.len());
        if let Some(kind) = next
            .and_then(|index| catalogue.mode.choices.get(index))
            .copied()
        {
            state.request_editor_kind(kind);
        }
        return;
    }
    let Some(field) = state.focused_editor_setting() else {
        return;
    };
    let Some(row) = state
        .editor_settings()
        .iter()
        .find(|row| row.field == field)
    else {
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
    let Some(choice) = next.and_then(|index| row.choices.get(index)).copied() else {
        return;
    };
    let Some(target) = state.editor_setting_target(field) else {
        return;
    };
    state.dispatch(MapMakerUiCommand::SelectEditorSetting { target, choice });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_input::live_page::{LiveOption, LiveRow};

    fn row(node: u32, label: u32, options: &[u32], selected: u32) -> LiveRow {
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
                .iter()
                .map(|id| LiveOption {
                    id: *id,
                    text: LiveText::Id(*id),
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
    fn t1714_editor_settings_labels_bind_only_the_owned_choice_rows() {
        assert_eq!(
            field_for_label(0x0AB5),
            Some(EditorSettingField::GridIntensity)
        );
        assert_eq!(field_for_label(0x0ABB), Some(EditorSettingField::Music));
        assert_eq!(field_for_label(0x0AB7), Some(EditorSettingField::Sounds));
        assert_eq!(field_for_label(0x0B01), None); // Mode is a separate transaction.
        assert_eq!(field_for_label(0x1A44), None); // Xbox Live is out of scope.
    }

    #[test]
    fn runtime_choices_are_opaque_ordinals_in_measured_field_order() {
        let live = LivePage {
            slot: 1,
            builder: EDITOR_SETTINGS_BUILDER,
            state: 1,
            cursor_node: 10,
            rows: vec![
                row(9, 0x0B01, &[0x0AFF, 0x0B00], 1),
                row(10, 0x0AB5, &[0x10, 0x20, 0x30], 2),
                row(11, 0x0ABB, &[0x40, 0x50], 1),
                row(12, 0x0AB7, &[0x60, 0x70], 0),
            ],
            units: (1.0, 1.0),
        };
        let catalogue = runtime_catalogue(&live, &[9, 10, 11, 12]).unwrap();
        assert_eq!(
            catalogue
                .rows
                .iter()
                .map(|row| row.field)
                .collect::<Vec<_>>(),
            REQUIRED_FIELDS.to_vec()
        );
        assert_eq!(catalogue.rows[0].choices[2].get(), 2);
        assert_eq!(catalogue.rows[0].selected.unwrap().get(), 2);
        assert_eq!(catalogue.rows[1].selected.unwrap().get(), 1);
        assert_eq!(catalogue.mode.node, 9);
        assert_eq!(catalogue.mode.current, EditorKind(1));
        assert_eq!(catalogue.mode.choices, [EditorKind(0), EditorKind(1)]);
    }

    #[test]
    fn mode_row_fails_closed_when_live_choice_order_is_unrecognized() {
        let live = LivePage {
            slot: 1,
            builder: EDITOR_SETTINGS_BUILDER,
            state: 1,
            cursor_node: 9,
            rows: vec![
                row(9, 0x0B01, &[0x0B00, 0x0AFF], 0),
                row(10, 0x0AB5, &[0x10], 0),
                row(11, 0x0ABB, &[0x20], 0),
                row(12, 0x0AB7, &[0x30], 0),
            ],
            units: (1.0, 1.0),
        };
        assert!(runtime_catalogue(&live, &[9, 10, 11, 12]).is_none());
    }

    #[test]
    fn clear_map_confirmation_binds_only_the_measured_yes_no_popup() {
        let mut live = LivePage {
            slot: 1,
            builder: 0,
            state: 2,
            cursor_node: 10,
            rows: vec![row(10, 0x0AE1, &[], 0), row(11, 0x0AE2, &[], 0)],
            units: (1.0, 1.0),
        };
        live.rows[0].activate = CLEAR_MAP_YES_HANDLER;
        live.rows[1].activate = CLEAR_MAP_NO_HANDLER;
        assert_eq!(
            confirmation_widgets(&live, &[10, 11]),
            Some((WidgetId(1), WidgetId(0)))
        );
        live.rows[1].activate = 0x31_2B60;
        assert_eq!(confirmation_widgets(&live, &[10, 11]), None);
        live.rows[1].activate = CLEAR_MAP_NO_HANDLER;
        assert_eq!(confirmation_widgets(&live, &[10]), None);
        live.state = 1;
        assert_eq!(confirmation_widgets(&live, &[10, 11]), None);
    }
}
