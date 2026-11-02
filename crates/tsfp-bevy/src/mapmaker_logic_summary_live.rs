// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: own the Logic Summary page's semantic rows and native input state.
//!
//! T1714 supplies the exact label order; the existing runtime page remains the
//! text, layout, and callback effect seam until the Rule 24 adoption gate passes.

use std::sync::Mutex;

use tsfp_mapmaker::logic_summary::{
    LOGIC_SUMMARY_ACTIONS, LogicSummaryAction, LogicSummaryGeneration, LogicSummaryPageState,
    LogicSummaryUpdate,
};
use tsfp_ui_input::live_page::LivePage;
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_model::{focus::Wrap, model::Page};

pub const BUILDER: u32 = tsfp_ui_input::mapmaker_logic_summary::BUILDER;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    state: Option<LogicSummaryPageState>,
    updates: Vec<LogicSummaryUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogicSummaryUpdateBatch {
    pub generation: LogicSummaryGeneration,
    pub updates: Vec<LogicSummaryUpdate>,
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

fn exact_actions(live: &LivePage, cursor_nodes: &[u32]) -> Option<[LogicSummaryAction; 9]> {
    tsfp_ui_input::mapmaker_logic_summary::exact_actions(live, cursor_nodes)
}

pub fn supported(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    exact_actions(live, cursor_nodes).is_some()
}

fn focus_from_page(
    state: &mut LogicSummaryPageState,
    actions: &[LogicSummaryAction; 9],
    focused_index: Option<usize>,
) {
    if let Some(action) = focused_index.and_then(|index| actions.get(index)).copied()
        && let Some(target) = state.target(action)
    {
        let _ = state.focus(target);
    }
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
    let generation = LogicSummaryGeneration::new(session.generation);
    let state = session
        .state
        .get_or_insert_with(|| LogicSummaryPageState::new(generation));
    focus_from_page(state, &LOGIC_SUMMARY_ACTIONS, focused_index);
    true
}

/// Preserve live localization and geometry after validating the measured
/// semantic row table and syncing focus into the generation-scoped Rust state.
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
        .is_some_and(|page| page.wrap == Wrap::Wrap);
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    let updates = {
        let Some(state) = session.state.as_mut() else {
            return;
        };
        tsfp_ui_input::mapmaker_logic_summary::dispatch_commands(
            state,
            &actions,
            focused_index,
            commands,
            wrap,
        )
    };
    session.updates.extend(updates);
}

pub fn take_updates() -> Option<LogicSummaryUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(LogicSummaryUpdateBatch {
        generation: LogicSummaryGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_input::live_page::{LiveRow, LiveText};
    use tsfp_ui_input::mapmaker_logic_summary::{ROW_LABELS, exact_actions};

    fn live(labels: &[u32]) -> (LivePage, Vec<u32>) {
        let rows: Vec<_> = labels
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
            .collect();
        let nodes = rows.iter().map(|row| row.node).collect();
        (
            LivePage {
                slot: 3,
                builder: BUILDER,
                state: 2,
                cursor_node: 1,
                rows,
                units: (6.4, 4.8),
            },
            nodes,
        )
    }

    #[test]
    fn only_the_measured_logic_summary_row_sequence_is_admitted() {
        let (page, nodes) = live(&ROW_LABELS);
        assert_eq!(exact_actions(&page, &nodes), Some(LOGIC_SUMMARY_ACTIONS));
        let mut changed = ROW_LABELS;
        changed.swap(2, 3);
        let (page, nodes) = live(&changed);
        assert_eq!(exact_actions(&page, &nodes), None);
    }
}
