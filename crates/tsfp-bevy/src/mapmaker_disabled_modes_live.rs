// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach the observed Map Settings Disable Modes checklist to typed Rust state.
//!
//! The page has no distinct published builder contract. Admit it only when it is
//! reached directly from the focused Disable Modes entry and every cursor row
//! is a checkbox. Runtime labels, layout, and existing callbacks remain at the
//! live-page seam until the Map Maker adoption gate passes.

use std::sync::Mutex;

use tsfp_mapmaker::disabled_modes::{
    DisabledModeId, DisabledModeRow, DisabledModesGeneration, DisabledModesPage,
    DisabledModesUpdate, DisabledModesUpdateBatch,
};
use tsfp_mapmaker::disabled_modes_focus::MapMakerDisabledModesFocus;
use tsfp_ui_input::live_page::LivePage;
use tsfp_ui_input::native_page::{GuestUi, NativeCommand};
use tsfp_ui_model::default_style::DefaultPage;
use tsfp_ui_model::default_widgets::WidgetDraw;

const MAP_SETTINGS_BUILDER: u32 = 0x31_33E0;

#[derive(Default)]
struct Session {
    top: Option<(u32, u32)>,
    pending_from_map_settings: bool,
    active_page: Option<(u32, u32)>,
    generation: u64,
    attached: bool,
    row_nodes: Vec<u32>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    top: None,
    pending_from_map_settings: false,
    active_page: None,
    generation: 0,
    attached: false,
    row_nodes: Vec::new(),
});

fn disable_modes_is_focused() -> bool {
    crate::storage_live::mapmaker_ui()
        .lock()
        .ok()
        .is_some_and(|state| state.disable_modes_focused())
}

/// Track the Map Settings -> checklist transition. The checkbox signature is
/// still checked by the projection before any row is admitted.
pub fn observe_top(top: Option<(u32, u32)>) {
    let focused = disable_modes_is_focused();
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    if session.top == top {
        if top.is_some_and(|(builder, _)| builder == MAP_SETTINGS_BUILDER) {
            session.pending_from_map_settings = focused;
            session.active_page = None;
            session.attached = false;
            session.row_nodes.clear();
        }
        return;
    }

    let previous = session.top;
    if top.is_some_and(|(builder, _)| builder == MAP_SETTINGS_BUILDER) {
        session.pending_from_map_settings = focused;
        session.active_page = None;
        session.attached = false;
        session.row_nodes.clear();
    } else if previous.is_some_and(|(builder, _)| builder == MAP_SETTINGS_BUILDER) && focused {
        session.pending_from_map_settings = true;
        session.active_page = top;
        session.generation = session.generation.wrapping_add(1).max(1);
        session.attached = false;
        session.row_nodes.clear();
    } else {
        session.pending_from_map_settings = false;
        session.active_page = None;
        session.attached = false;
        session.row_nodes.clear();
    }
    session.top = top;
}

fn checkbox_rows<'a>(
    live: &'a LivePage,
    cursor_nodes: &[u32],
) -> Option<Vec<&'a tsfp_ui_input::live_page::LiveRow>> {
    if cursor_nodes.is_empty() {
        return None;
    }
    let checkbox_kind = tsfp_ui_input::default_widgets_live::addr::KIND_CHECK_BOX;
    let nodes: Vec<_> = live
        .rows
        .iter()
        .filter(|row| row.kind == checkbox_kind)
        .map(|row| row.node)
        .collect();
    if nodes.as_slice() != cursor_nodes {
        return None;
    }
    Some(
        cursor_nodes
            .iter()
            .map(|node| live.rows.iter().find(|row| row.node == *node))
            .collect::<Option<Vec<_>>>()?,
    )
}

fn capture_rows(
    guest: &dyn GuestUi,
    live: &LivePage,
    cursor_nodes: &[u32],
) -> Option<Vec<DisabledModeRow>> {
    checkbox_rows(live, cursor_nodes)?
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            let value_address = row
                .node
                .checked_add(tsfp_ui_input::default_widgets_live::addr::VALUE)?;
            let checked = guest.read32(value_address)?;
            (checked <= 1).then_some(DisabledModeRow {
                id: DisabledModeId::new(index as u64),
                disabled: checked == 1,
            })
        })
        .collect()
}

fn patch_checkbox_draws(draws: &mut [WidgetDraw], disabled: &[bool]) -> bool {
    let checkbox_count = draws
        .iter()
        .filter(|draw| matches!(draw, WidgetDraw::CheckBox { .. }))
        .count();
    if checkbox_count != disabled.len() {
        return false;
    }
    let mut values = disabled.iter().copied();
    for draw in draws {
        if let WidgetDraw::CheckBox { checked, .. } = draw {
            let Some(value) = values.next() else {
                return false;
            };
            *checked = value;
        }
    }
    values.next().is_none()
}

fn checkbox_draw_count(draws: &[WidgetDraw]) -> usize {
    draws
        .iter()
        .filter(|draw| matches!(draw, WidgetDraw::CheckBox { .. }))
        .count()
}

/// The default page must be rebuilt while this checklist is active because
/// its checkbox draws are owned by the Rust model, not the cached guest rows.
pub fn should_refresh(builder: u32, slot: u32) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() || !disable_modes_is_focused() {
        return false;
    }
    SESSION.lock().ok().is_some_and(|session| {
        session.attached
            && session.pending_from_map_settings
            && session.active_page == Some((builder, slot))
    })
}

/// Project the runtime checkbox list through `MapMakerUiState`. Labels and
/// geometry continue to come from the live tables and page record.
pub fn project_owned_page(
    guest: &dyn GuestUi,
    builder: u32,
    slot: u32,
    live: &LivePage,
    cursor_nodes: &[u32],
    page: &mut DefaultPage,
) {
    if !crate::storage_live::mapmaker_ui_enabled() || !disable_modes_is_focused() {
        return;
    }
    let Ok(session) = SESSION.lock() else {
        return;
    };
    if !session.pending_from_map_settings || session.active_page != Some((builder, slot)) {
        return;
    }
    let generation = session.generation;
    let row_nodes = session.row_nodes.clone();
    drop(session);

    let Some(rows) = capture_rows(guest, live, cursor_nodes) else {
        return;
    };
    if !row_nodes.is_empty() && row_nodes.as_slice() != cursor_nodes {
        return;
    }
    if checkbox_draw_count(&page.widgets.draws) != rows.len() {
        return;
    }
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    let modes = state.disabled_modes_mut();
    let owned_generation = DisabledModesGeneration::new(generation);
    if modes.generation() == Some(owned_generation) {
        if !modes
            .rows()
            .iter()
            .map(|row| row.id)
            .eq(rows.iter().map(|row| row.id))
        {
            return;
        }
    } else if modes.install(owned_generation, rows).is_err() {
        return;
    }
    let disabled: Vec<_> = modes.rows().iter().map(|row| row.disabled).collect();
    if !patch_checkbox_draws(&mut page.widgets.draws, &disabled) {
        return;
    }
    drop(state);
    if let Ok(mut session) = SESSION.lock()
        && session.pending_from_map_settings
        && session.active_page == Some((builder, slot))
        && session.generation == generation
    {
        session.attached = true;
        session.row_nodes = cursor_nodes.to_vec();
    }
}

fn focus_index(page: &mut DisabledModesPage, index: u32) -> Option<DisabledModesUpdate> {
    let row = page.rows().get(index as usize)?;
    let target = page.target(row.id)?;
    Some(page.focus(target))
}

fn toggle_index(page: &mut DisabledModesPage, index: u32) -> Option<DisabledModesUpdate> {
    let row = page.rows().get(index as usize)?;
    let target = page.target(row.id)?;
    Some(page.toggle(target))
}

fn apply_native_commands(
    page: &mut DisabledModesPage,
    commands: &[NativeCommand],
) -> Vec<DisabledModesUpdate> {
    let mut updates = Vec::new();
    for command in commands {
        match *command {
            NativeCommand::Focus(index) => {
                if let Some(update) = focus_index(page, index) {
                    updates.push(update);
                }
            }
            NativeCommand::Select(index) => {
                if let Some(update) = toggle_index(page, index) {
                    updates.push(update);
                }
            }
            NativeCommand::Accept => {
                if let Some(id) = page.focused()
                    && let Some(target) = page.target(id)
                {
                    updates.push(page.toggle(target));
                }
            }
            NativeCommand::Step(tsfp_ui_input::widgets::Step::Up)
            | NativeCommand::Step(tsfp_ui_input::widgets::Step::Down) => {
                let down = matches!(
                    *command,
                    NativeCommand::Step(tsfp_ui_input::widgets::Step::Down)
                );
                let rows = page.rows();
                let current = page
                    .focused()
                    .and_then(|id| rows.iter().position(|row| row.id == id));
                let next = match (current, down) {
                    (Some(index), true) => index.checked_add(1),
                    (Some(index), false) => index.checked_sub(1),
                    (None, true) => Some(0),
                    (None, false) => rows.len().checked_sub(1),
                }
                .filter(|index| *index < rows.len());
                if let Some(index) = next
                    && let Some(update) = focus_index(page, index as u32)
                {
                    updates.push(update);
                }
            }
            NativeCommand::Back
            | NativeCommand::Delete
            | NativeCommand::Shortcut
            | NativeCommand::Join(_)
            | NativeCommand::ResumeGame
            | NativeCommand::Step(_)
            | NativeCommand::Scroll(_)
            | NativeCommand::Adjust { .. } => {}
        }
    }
    updates
}

pub fn apply_commands(
    current: &crate::ui_pages::Current,
    commands: &[NativeCommand],
) -> Option<DisabledModesUpdateBatch> {
    if commands.is_empty()
        || !crate::storage_live::mapmaker_ui_enabled()
        || !disable_modes_is_focused()
    {
        return None;
    }
    let Ok(session) = SESSION.lock() else {
        return None;
    };
    if !session.pending_from_map_settings
        || !session.attached
        || session.active_page != Some((current.descriptor.builder, current.slot))
        || session.row_nodes != current.cursor_nodes
    {
        return None;
    }
    let generation = session.generation;
    drop(session);
    if checkbox_rows(&current.live, &current.cursor_nodes).is_none() {
        return None;
    }
    let mut state = crate::storage_live::mapmaker_ui().lock().ok()?;
    if state.disabled_modes().generation() != Some(DisabledModesGeneration::new(generation)) {
        return None;
    }
    Some(DisabledModesUpdateBatch {
        generation: DisabledModesGeneration::new(generation),
        updates: apply_native_commands(state.disabled_modes_mut(), commands),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_model::list_rows::Edges;
    use tsfp_ui_model::skin::PackedColor;

    fn live(rows: Vec<(u32, u32, u32)>) -> LivePage {
        LivePage {
            slot: 1,
            builder: 0x31_33E0,
            state: 0,
            cursor_node: rows.first().map_or(0, |row| row.0),
            rows: rows
                .into_iter()
                .map(|(node, kind, selected)| tsfp_ui_input::live_page::LiveRow {
                    node,
                    node_type: 4,
                    id: 0,
                    flags: 0,
                    text: tsfp_ui_input::live_page::LiveText::Id(node),
                    font: 0,
                    value_font: 0,
                    align: 0,
                    rect: None,
                    clip: None,
                    value: None,
                    activate: 0,
                    draw: 0,
                    kind,
                    selected,
                    options: Vec::new(),
                    brightness: 0.0,
                    height: 0.0,
                    text_align: 0,
                    text_x: 0.0,
                    text_y: 0.0,
                })
                .collect(),
            units: (1.0, 1.0),
        }
    }

    fn checkbox(checked: bool) -> WidgetDraw {
        WidgetDraw::CheckBox {
            area: Edges::new(0.0, 0.0, 1.0, 1.0),
            size: 1.0,
            checked,
            top: PackedColor(0),
            bottom: PackedColor(0),
        }
    }

    #[test]
    fn checkbox_catalogue_fails_closed_on_mixed_rows_or_reordered_cursor() {
        let kind = tsfp_ui_input::default_widgets_live::addr::KIND_CHECK_BOX;
        let page = live(vec![(10, kind, 0), (20, kind, 1)]);
        assert_eq!(checkbox_rows(&page, &[10, 20]).unwrap().len(), 2);
        assert!(checkbox_rows(&page, &[20, 10]).is_none());
        assert!(checkbox_rows(&page, &[10]).is_none());
        let mixed = live(vec![(10, kind, 0), (20, kind + 1, 0)]);
        assert!(checkbox_rows(&mixed, &[10, 20]).is_none());
    }

    #[test]
    fn native_commands_toggle_only_the_typed_checklist() {
        let mut page = DisabledModesPage::default();
        page.install(
            DisabledModesGeneration::new(7),
            vec![
                DisabledModeRow {
                    id: DisabledModeId::new(0),
                    disabled: false,
                },
                DisabledModeRow {
                    id: DisabledModeId::new(1),
                    disabled: true,
                },
            ],
        )
        .unwrap();
        let updates = apply_native_commands(
            &mut page,
            &[
                NativeCommand::Focus(0),
                NativeCommand::Accept,
                NativeCommand::Step(tsfp_ui_input::widgets::Step::Down),
                NativeCommand::Accept,
            ],
        );
        assert_eq!(updates.len(), 4);
        assert_eq!(page.rows()[0].disabled, true);
        assert_eq!(page.rows()[1].disabled, false);
    }

    #[test]
    fn renderer_projection_requires_exact_checkbox_count() {
        let mut draws = vec![checkbox(false), checkbox(true)];
        assert!(patch_checkbox_draws(&mut draws, &[true, false]));
        let checked: Vec<_> = draws
            .iter()
            .filter_map(|draw| match draw {
                WidgetDraw::CheckBox { checked, .. } => Some(*checked),
                _ => None,
            })
            .collect();
        assert_eq!(checked, vec![true, false]);

        assert!(!patch_checkbox_draws(&mut draws, &[false]));
        let still_checked: Vec<_> = draws
            .iter()
            .filter_map(|draw| match draw {
                WidgetDraw::CheckBox { checked, .. } => Some(*checked),
                _ => None,
            })
            .collect();
        assert_eq!(still_checked, vec![true, false]);
    }
}
