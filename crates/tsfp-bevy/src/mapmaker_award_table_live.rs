// SPDX-License-Identifier: GPL-3.0-or-later
//! Attach Rust-owned focus state to the measured T1714 dynamic Awards table.
//!
//! The original table row handlers still perform their domain effects through
//! the existing callback seam; this adapter records typed row requests.

use std::sync::Mutex;

use tsfp_mapmaker::award_table::{AwardTableGeneration, AwardTablePageState, AwardTableUpdate};
use tsfp_ui_input::{
    live_page::LivePage,
    mapmaker_award_table::{self, BUILDER},
    native_page::NativeCommand,
};
use tsfp_ui_model::model::Page;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    row_nodes: Option<Vec<u32>>,
    state: Option<AwardTablePageState>,
    updates: Vec<AwardTableUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwardTableUpdateBatch {
    pub generation: AwardTableGeneration,
    pub updates: Vec<AwardTableUpdate>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
    row_nodes: None,
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
        session.row_nodes = None;
        session.state = None;
        session.updates.clear();
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

pub fn supported(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    mapmaker_award_table::row_nodes(live, cursor_nodes).is_some()
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32], focused_index: Option<usize>) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let Some(row_nodes) = mapmaker_award_table::row_nodes(live, cursor_nodes) else {
        return false;
    };
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return false;
    }
    if session.row_nodes.as_deref() != Some(row_nodes.as_slice()) {
        session.generation = session.generation.wrapping_add(1).max(1);
        session.row_nodes = Some(row_nodes.clone());
        session.state = Some(AwardTablePageState::new(
            AwardTableGeneration::new(session.generation),
            row_nodes.len() as u32,
        ));
        session.updates.clear();
    }
    if let Some(index) = focused_index
        && let Some(state) = session.state.as_mut()
        && let Some(target) = state.target(index)
    {
        let _ = state.focus(target);
    }
    true
}

/// Preserve all runtime labels/layout and attach page-local Rust interaction
/// identity for each cursor row, including the trailing extra item.
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
    let Some(row_nodes) = mapmaker_award_table::row_nodes(&current.live, &current.cursor_nodes)
    else {
        return;
    };
    let focused_index = current.focused.and_then(|widget| {
        current
            .live
            .cursor_index(widget.0 as usize, &current.cursor_nodes)
            .map(|index| index as usize)
    });
    if !ensure_state(&current.live, &row_nodes, focused_index) {
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
        mapmaker_award_table::dispatch_commands(state, focused_index, commands, wrap)
    };
    session.updates.extend(updates);
}

pub fn take_updates() -> Option<AwardTableUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(AwardTableUpdateBatch {
        generation: AwardTableGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}
