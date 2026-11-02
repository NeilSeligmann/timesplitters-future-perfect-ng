// SPDX-License-Identifier: GPL-3.0-or-later
//! Attach Rust-owned state and native input to the measured Sample Map chooser.
//!
//! T1714 defines row labels and identifies the first zero-label row as a
//! special return. Selection effects are not specified, so the existing
//! callback remains the TEMPORARY-SHIM effect seam.

use std::sync::Mutex;

use tsfp_mapmaker::sample_map_menu::{
    SampleMapChoice, SampleMapMenuGeneration, SampleMapMenuState, SampleMapMenuUpdate,
};
use tsfp_ui_input::{
    live_page::LivePage,
    mapmaker_sample_map_menu::{self, BUILDER},
    native_page::NativeCommand,
};
use tsfp_ui_model::model::Page;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    signature: Option<Vec<Option<SampleMapChoice>>>,
    state: Option<SampleMapMenuState>,
    updates: Vec<SampleMapMenuUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleMapMenuUpdateBatch {
    pub generation: SampleMapMenuGeneration,
    pub updates: Vec<SampleMapMenuUpdate>,
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

fn exact_actions(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<Option<SampleMapChoice>>> {
    mapmaker_sample_map_menu::exact_actions(live, cursor_nodes)
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
        session.state = Some(SampleMapMenuState::new(
            SampleMapMenuGeneration::new(session.generation),
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

/// Preserve the live layout and localized IDs while attaching typed Rust
/// focus and selection state to the exact measured row table.
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
        mapmaker_sample_map_menu::dispatch_commands(state, &actions, focused_index, commands, wrap)
    };
    session.updates.extend(updates);
}

pub fn take_updates() -> Option<SampleMapMenuUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(SampleMapMenuUpdateBatch {
        generation: SampleMapMenuGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}
