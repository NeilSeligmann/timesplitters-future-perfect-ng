//! Attach generation-scoped Rust state to the measured Award Medal Settings rows.
//! Trigger contents and medal effects stay at the existing typed seam.

use std::sync::Mutex;

use tsfp_mapmaker::award_medal_settings::{
    AwardMedalGeneration, AwardMedalPageState, AwardMedalUpdate,
};
use tsfp_ui_input::{
    live_page::LivePage,
    mapmaker_award_medal::{self, BUILDER},
    native_page::NativeCommand,
};
use tsfp_ui_model::model::Page;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    state: Option<AwardMedalPageState>,
    updates: Vec<AwardMedalUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwardMedalUpdateBatch {
    pub generation: AwardMedalGeneration,
    pub updates: Vec<AwardMedalUpdate>,
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

pub fn supported(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    mapmaker_award_medal::measured_action_rows(live, cursor_nodes).is_some()
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32], focused_index: Option<usize>) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled()
        || mapmaker_award_medal::measured_action_rows(live, cursor_nodes).is_none()
    {
        return false;
    }
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return false;
    }
    let generation = AwardMedalGeneration::new(session.generation);
    let state = session
        .state
        .get_or_insert_with(|| AwardMedalPageState::new(generation));
    if let Some(action) = focused_index.and_then(|index| {
        mapmaker_award_medal::measured_action_rows(live, cursor_nodes)?
            .get(index)
            .copied()
            .flatten()
    }) {
        let _ = state.focus(state.target(action));
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
    let Some(actions) =
        mapmaker_award_medal::measured_action_rows(&current.live, &current.cursor_nodes)
    else {
        return;
    };
    let focused_index = current
        .focused
        .and_then(|widget| {
            current
                .live
                .cursor_index(widget.0 as usize, &current.cursor_nodes)
        })
        .map(|index| index as usize);
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
        mapmaker_award_medal::dispatch_commands(state, &actions, focused_index, commands, wrap)
    };
    session.updates.extend(updates);
}

pub fn take_updates() -> Option<AwardMedalUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(AwardMedalUpdateBatch {
        generation: AwardMedalGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}
