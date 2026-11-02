// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach typed input state to the measured trigger selector rows.
//!
//! T1714 does not identify the callbacks' effects, so original mutations stay
//! at the existing live page seam until the subsystem adoption gate passes.

use std::sync::Mutex;

use tsfp_mapmaker::trigger_selection::{
    TriggerSelectionAction, TriggerSelectionGeneration, TriggerSelectionPageState,
    TriggerSelectionUpdate,
};
use tsfp_ui_input::{
    live_page::LivePage,
    mapmaker_trigger_selection::{self, BUILDER},
    native_page::NativeCommand,
};
use tsfp_ui_model::model::Page;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    state: Option<TriggerSelectionPageState>,
    updates: Vec<TriggerSelectionUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerSelectionUpdateBatch {
    pub generation: TriggerSelectionGeneration,
    pub updates: Vec<TriggerSelectionUpdate>,
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

fn exact_actions(
    live: &LivePage,
    cursor_nodes: &[u32],
) -> Option<Vec<Option<TriggerSelectionAction>>> {
    mapmaker_trigger_selection::exact_actions(live, cursor_nodes)
}

pub fn supported(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    exact_actions(live, cursor_nodes).is_some()
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32], focused_index: Option<usize>) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() || exact_actions(live, cursor_nodes).is_none() {
        return false;
    }
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return false;
    }
    let generation = TriggerSelectionGeneration::new(session.generation);
    let state = session
        .state
        .get_or_insert_with(|| TriggerSelectionPageState::new(generation));
    if let Some(action) = focused_index.and_then(|index| {
        exact_actions(live, cursor_nodes)?
            .get(index)
            .copied()
            .flatten()
    }) && let Some(target) = state.target(action)
    {
        let _ = state.focus(target);
    }
    true
}

/// Keep original copy, row values, and geometry while validating the measured
/// static controls and initializing the Rust-owned generation-scoped state.
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
        mapmaker_trigger_selection::dispatch_commands(
            state,
            &actions,
            focused_index,
            commands,
            wrap,
        )
    };
    session.updates.extend(updates);
}

pub fn take_updates() -> Option<TriggerSelectionUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(TriggerSelectionUpdateBatch {
        generation: TriggerSelectionGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_input::live_page::{LiveRow, LiveText};

    #[test]
    fn measured_trigger_selector_rows_are_supported() {
        let labels = [0x0B44, 0x0D6B, 0x7777, 0x0D6C, 0x0B66];
        let rows = labels
            .iter()
            .enumerate()
            .map(|(index, label)| LiveRow {
                node: index as u32 + 1,
                node_type: 0,
                id: 0,
                text: LiveText::Id(*label),
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
                options: Vec::new(),
                flags: 0,
                brightness: 0.0,
                height: 0.0,
                text_align: 0,
                text_x: 0.0,
                text_y: 0.0,
            })
            .collect::<Vec<_>>();
        let nodes = rows.iter().map(|row| row.node).collect::<Vec<_>>();
        let live = LivePage {
            slot: 3,
            builder: BUILDER,
            state: 2,
            cursor_node: 1,
            rows,
            units: (6.4, 4.8),
        };
        assert!(supported(&live, &nodes));
        let mut reordered_nodes = nodes.clone();
        reordered_nodes.swap(1, 3);
        assert!(!supported(&live, &reordered_nodes));
    }
}
