//! Attach Rust-owned interaction state to T1714's Award Flags widget.

use std::sync::Mutex;

use tsfp_mapmaker::award_flags::{AwardFlagsGeneration, AwardFlagsPageState, AwardFlagsUpdate};
use tsfp_ui_input::{
    live_page::LivePage,
    mapmaker_award_flags::{self, BUILDER},
    native_page::NativeCommand,
};
use tsfp_ui_model::model::Page;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    row_nodes: Option<Vec<u32>>,
    state: Option<AwardFlagsPageState>,
    updates: Vec<AwardFlagsUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwardFlagsUpdateBatch {
    pub generation: AwardFlagsGeneration,
    pub updates: Vec<AwardFlagsUpdate>,
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
    mapmaker_award_flags::measured_actions(live, cursor_nodes).is_some()
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32], focused_index: Option<usize>) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let Some(actions) = mapmaker_award_flags::measured_actions(live, cursor_nodes) else {
        return false;
    };
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return false;
    }
    if session.row_nodes.as_deref() != Some(cursor_nodes) {
        session.generation = session.generation.wrapping_add(1).max(1);
        session.row_nodes = Some(cursor_nodes.to_vec());
        session.state = Some(AwardFlagsPageState::new(
            AwardFlagsGeneration::new(session.generation),
            actions,
        ));
        session.updates.clear();
    } else if session
        .state
        .is_some_and(|state| state.actions() != actions)
    {
        session.generation = session.generation.wrapping_add(1).max(1);
        session.state = Some(AwardFlagsPageState::new(
            AwardFlagsGeneration::new(session.generation),
            actions,
        ));
        session.updates.clear();
    }
    if let Some(index) = focused_index
        && let Some(state) = session.state.as_mut()
        && let Some(rank) = tsfp_mapmaker::award_flags::AwardRank::at(index)
    {
        let _ = state.focus(state.target(rank));
    }
    true
}

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
    if !ensure_state(&current.live, &current.cursor_nodes, None) {
        return;
    }
    let focused_index = current.focused.and_then(|widget| {
        current
            .live
            .cursor_index(widget.0 as usize, &current.cursor_nodes)
            .map(|index| index as usize)
    });
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
        mapmaker_award_flags::dispatch_commands(state, focused_index, commands, wrap)
    };
    session.updates.extend(updates);
}

pub fn take_updates() -> Option<AwardFlagsUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(AwardFlagsUpdateBatch {
        generation: AwardFlagsGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}
