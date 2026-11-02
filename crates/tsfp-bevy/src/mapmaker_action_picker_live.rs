// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach typed input state to the T1714 action picker rows.
//!
//! Dynamic record identity and action mutations remain at the lifted callback
//! seam; Rust owns page-generation, focus, and native input requests.

use std::sync::Mutex;

use tsfp_mapmaker::action_picker::{
    ActionPickerChoice, ActionPickerGeneration, ActionPickerPageState, ActionPickerUpdate,
};
use tsfp_ui_input::{
    live_page::LivePage,
    mapmaker_action_picker::{self, BUILDER},
    native_page::NativeCommand,
};
use tsfp_ui_model::model::Page;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    state: Option<ActionPickerPageState>,
    updates: Vec<ActionPickerUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionPickerUpdateBatch {
    pub generation: ActionPickerGeneration,
    pub updates: Vec<ActionPickerUpdate>,
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

fn exact_choices(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<ActionPickerChoice>> {
    mapmaker_action_picker::exact_choices(live, cursor_nodes)
}

pub fn supported(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    exact_choices(live, cursor_nodes).is_some()
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32], focused_index: Option<usize>) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let Some(choices) = exact_choices(live, cursor_nodes) else {
        return false;
    };
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return false;
    }
    let generation = ActionPickerGeneration::new(session.generation);
    if session.state.is_none() {
        session.state = ActionPickerPageState::new(generation, &choices);
    }
    let Some(state) = session.state.as_mut() else {
        return false;
    };
    if let Some(choice) = focused_index.and_then(|index| choices.get(index)).copied()
        && let Some(target) = state.target(choice)
    {
        let _ = state.focus(target);
    }
    true
}

/// Keep measured text, layout, and callback projection while a strict page
/// signature seeds the Rust-owned interaction state.
pub fn project_owned_page(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    focused: Option<tsfp_ui_model::model::WidgetId>,
    page: Page,
) -> Option<Page> {
    if live.slot != slot {
        return None;
    }
    let focused_index = focused
        .and_then(|widget| live.cursor_index(widget.0 as usize, cursor_nodes))
        .map(|index| index as usize);
    ensure_state(live, cursor_nodes, focused_index).then_some(page)
}

pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder != BUILDER {
        return;
    }
    let Some(choices) = exact_choices(&current.live, &current.cursor_nodes) else {
        return;
    };
    let focused_index = current.focused.and_then(|widget| {
        current
            .live
            .cursor_index(widget.0 as usize, &current.cursor_nodes)
            .map(|index| index as usize)
    });
    if !ensure_state(&current.live, &current.cursor_nodes, focused_index) {
        return;
    }
    let wrap = current
        .content
        .model
        .page(current.content.page)
        .is_some_and(|page| page.wrap == tsfp_ui_model::focus::Wrap::Wrap);
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    let updates = {
        let Some(state) = session.state.as_mut() else {
            return;
        };
        mapmaker_action_picker::dispatch_commands(state, &choices, focused_index, commands, wrap)
    };
    session.updates.extend(updates);
}

pub fn take_updates() -> Option<ActionPickerUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(ActionPickerUpdateBatch {
        generation: ActionPickerGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}
