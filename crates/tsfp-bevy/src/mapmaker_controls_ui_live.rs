// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach native focus and Back input to the Map Maker Controls page.
//! Labels, animated cells, and artwork remain with the existing native draw
//! path; this adapter owns page-local focus and routes Back through typed state.

use std::sync::Mutex;

use tsfp_mapmaker::controls_ui::{
    ControlsPageGeneration, ControlsPageState, ControlsPageUpdate, MEASURED_CONTROLS_ROW_COUNT,
};
use tsfp_ui_input::live_page::LivePage;
use tsfp_ui_input::native_page::NativeCommand;

pub const BUILDER: u32 = 0x31_5720;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    state: Option<ControlsPageState>,
    updates: Vec<ControlsPageUpdate>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
    state: None,
    updates: Vec::new(),
});

pub fn observe_top(top: Option<(u32, u32)>) {
    let page = top.filter(|(builder, _)| *builder == BUILDER);
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    if session.active_page != page {
        session.active_page = page;
        session.state = None;
        session.updates.clear();
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

fn valid_live_rows(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    if live.builder != BUILDER
        || live.rows.len() != MEASURED_CONTROLS_ROW_COUNT
        || cursor_nodes.len() != MEASURED_CONTROLS_ROW_COUNT
    {
        return false;
    }
    let mut seen = Vec::with_capacity(cursor_nodes.len());
    cursor_nodes.iter().all(|node| {
        if seen.contains(node) {
            return false;
        }
        let Some(row) = live.rows.iter().find(|row| row.node == *node) else {
            return false;
        };
        // T1916 measured the page as 18 type-7, kind-1 rows with one option
        // each. Do not admit a different page shape under this interaction map.
        if row.options.len() != 1 {
            return false;
        }
        seen.push(*node);
        true
    })
}

fn ensure_state(current: &crate::ui_pages::Current) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled()
        || !valid_live_rows(&current.live, &current.cursor_nodes)
    {
        return false;
    }
    let owned_surface = crate::storage_live::mapmaker_ui()
        .lock()
        .ok()
        .is_some_and(|state| {
            state.page() == tsfp_mapmaker::ui::MapMakerPage::Editor
                && state.editor_surface() == tsfp_mapmaker::ui::EditorSurface::Controls
        });
    if !owned_surface {
        return false;
    }
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((current.descriptor.builder, current.slot))
        || session.generation == 0
    {
        return false;
    }
    if session.state.is_none() {
        let focused = current
            .focused
            .and_then(|widget| {
                current
                    .live
                    .cursor_index(widget.0 as usize, &current.cursor_nodes)
            })
            .and_then(|index| u8::try_from(index).ok())
            .and_then(tsfp_mapmaker::controls_ui::ControlsRowId::new);
        session.state = ControlsPageState::new(
            ControlsPageGeneration::new(session.generation),
            current.cursor_nodes.len(),
            focused,
        );
    }
    session.state.is_some()
}

pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder != BUILDER || !ensure_state(current) {
        return;
    }
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    let Some(state) = session.state.as_mut() else {
        return;
    };
    let updates =
        tsfp_ui_input::mapmaker_controls_live::dispatch_controls_commands(state, commands);
    let return_to_menu = updates.contains(&ControlsPageUpdate::BackRequested);
    session.updates.extend(updates);
    if return_to_menu && let Ok(mut ui) = crate::storage_live::mapmaker_ui().lock() {
        ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::Back);
    }
}

/// Drain ordered Rust-owned focus and Back outcomes for the typed caller.
pub fn take_updates() -> Option<(ControlsPageGeneration, Vec<ControlsPageUpdate>)> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() {
        return None;
    }
    Some((
        ControlsPageGeneration::new(session.generation),
        std::mem::take(&mut session.updates),
    ))
}
