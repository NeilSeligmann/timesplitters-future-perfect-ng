//! Rust-owned focus and activation requests for one selected-entry row pair.
//!
//! The title, values, page drawing, and mutation callback remain on the
//! existing TEMPORARY-SHIM seam until their original contracts are measured.

use std::sync::Mutex;

use tsfp_mapmaker::selected_entry::{
    SelectedEntryField, SelectedEntryGeneration, SelectedEntryPageState, SelectedEntryUpdate,
};
use tsfp_ui_input::{
    live_page::LivePage,
    native_page::NativeCommand,
    selected_entry::{self, BUILDER},
};

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    fields: Option<[SelectedEntryField; 2]>,
    state: Option<SelectedEntryPageState>,
    updates: Vec<SelectedEntryUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedEntryUpdateBatch {
    pub generation: SelectedEntryGeneration,
    pub updates: Vec<SelectedEntryUpdate>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
    fields: None,
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
        session.fields = None;
        session.state = None;
        session.updates.clear();
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

pub fn supported(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    selected_entry::exact_fields(live, cursor_nodes).is_some()
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32], focused_index: Option<usize>) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let Some(fields) = selected_entry::exact_fields(live, cursor_nodes) else {
        return false;
    };
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return false;
    }
    if session.fields != Some(fields) {
        session.generation = session.generation.wrapping_add(1).max(1);
        session.fields = Some(fields);
        session.state = None;
        session.updates.clear();
    }
    let generation = SelectedEntryGeneration::new(session.generation);
    if session.state.is_none() {
        session.state = Some(SelectedEntryPageState::new(generation, fields));
    }
    if let Some(field) = focused_index.and_then(|index| fields.get(index).copied())
        && let Some(state) = session.state.as_mut()
    {
        let _ = state.focus(state.target(field));
    }
    true
}

pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder != BUILDER {
        return;
    }
    let focused_index = current.focused.and_then(|widget| {
        current
            .live
            .cursor_index(widget.0 as usize, &current.cursor_nodes)
            .map(|index| index as usize)
    });
    if !ensure_state(&current.live, &current.cursor_nodes, focused_index) {
        return;
    }
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    let updates = {
        let Some(state) = session.state.as_mut() else {
            return;
        };
        selected_entry::dispatch_commands(state, focused_index, commands)
    };
    session.updates.extend(updates);
}

/// Typed request seam for a future Rust-owned value handler.
pub fn take_updates() -> Option<SelectedEntryUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(SelectedEntryUpdateBatch {
        generation: SelectedEntryGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}
