// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach typed Rust state to the measured Preview/tool menu rows.
//!
//! T1714 gives row identity and the enabled-row handler, not the handler's
//! mutation contract. Existing callbacks remain the temporary effect seam.

use std::sync::Mutex;

use tsfp_mapmaker::tool_menu::{
    ToolMenuChoice, ToolMenuEffect, ToolMenuGeneration, ToolMenuPageState, ToolMenuUpdate,
};
use tsfp_ui_input::{
    live_page::LivePage,
    mapmaker_tool_menu::{self, BUILDER},
    native_page::NativeCommand,
};
use tsfp_ui_model::model::Page;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    signature: Option<Vec<Option<ToolMenuChoice>>>,
    state: Option<ToolMenuPageState>,
    updates: Vec<ToolMenuEffect>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolMenuUpdateBatch {
    pub generation: ToolMenuGeneration,
    pub updates: Vec<ToolMenuEffect>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
    signature: None,
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
        session.signature = None;
        session.state = None;
        session.updates.clear();
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

fn exact_actions(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<Option<ToolMenuChoice>>> {
    mapmaker_tool_menu::exact_actions(live, cursor_nodes)
}

pub fn supported(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    exact_actions(live, cursor_nodes).is_some()
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32], focused_index: Option<usize>) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let Some(actions) = exact_actions(live, cursor_nodes) else {
        return false;
    };
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) {
        return false;
    }
    if session.signature.as_deref() != Some(actions.as_slice()) {
        if session.signature.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
        session.generation = session.generation.max(1);
        session.state = Some(ToolMenuPageState::new(
            ToolMenuGeneration::new(session.generation),
            &actions,
        ));
        session.signature = Some(actions.clone());
        session.updates.clear();
    }
    let Some(state) = session.state.as_mut() else {
        return false;
    };
    if let Some(choice) = focused_index.and_then(|index| actions.get(index).copied().flatten())
        && let Some(target) = state.target(choice)
    {
        let _ = state.focus(target);
    }
    true
}

/// Preserve the live page's labels, layout and callback fields while attaching
/// Rust focus/action state to the measured enabled rows.
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
    let Some(actions) = exact_actions(&current.live, &current.cursor_nodes) else {
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
        mapmaker_tool_menu::dispatch_commands(state, &actions, focused_index, commands, wrap)
    };
    let Some(state) = session.state.as_ref() else {
        return;
    };
    let effects = tsfp_ui_input::mapmaker_tool_menu::qualify_updates(state, updates);
    session.updates.extend(effects);
}

pub fn take_updates() -> Option<ToolMenuUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(ToolMenuUpdateBatch {
        generation: ToolMenuGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}
