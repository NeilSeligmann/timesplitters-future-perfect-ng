// SPDX-License-Identifier: GPL-3.0-or-later
//! Attach typed Rust input state to T1714's kind-specific map prompt.
//!
//! The original prompt text and layout stay projected from the active page.
//! Selecting a map object remains a request to the existing callback seam.

use std::sync::Mutex;

use tsfp_mapmaker::kind_picker::{
    KindPickerChoice, KindPickerGeneration, KindPickerPageState, KindPickerUpdate,
};
use tsfp_ui_input::{
    live_page::LivePage,
    mapmaker_kind_picker::{self, BUILDER},
    native_page::NativeCommand,
};
use tsfp_ui_model::model::Page;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    state: Option<KindPickerPageState>,
    updates: Vec<KindPickerUpdate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindPickerUpdateBatch {
    pub generation: KindPickerGeneration,
    pub updates: Vec<KindPickerUpdate>,
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
    mapmaker_kind_picker::exact_choice(live, cursor_nodes).is_some()
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32], focused_index: Option<usize>) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let Some(choice) = mapmaker_kind_picker::exact_choice(live, cursor_nodes) else {
        return false;
    };
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return false;
    }
    if session
        .state
        .as_ref()
        .is_some_and(|state| state.choice() != choice)
    {
        session.generation = session.generation.wrapping_add(1).max(1);
        session.updates.clear();
    }
    let generation = KindPickerGeneration::new(session.generation);
    if session.state.is_none() || session.state.is_some_and(|state| state.choice() != choice) {
        session.state = Some(KindPickerPageState::new(generation, choice));
    }
    let Some(state) = session.state.as_mut() else {
        return false;
    };
    if focused_index == Some(0) {
        let target = state.target();
        let _ = state.focus(target);
    }
    true
}

/// Retain the measured text/layout projection while attaching page-local state.
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
    let Some(choice) = mapmaker_kind_picker::exact_choice(&current.live, &current.cursor_nodes)
    else {
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
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    let updates = {
        let Some(state) = session.state.as_mut() else {
            return;
        };
        if state.choice() != choice {
            return;
        }
        mapmaker_kind_picker::dispatch_commands(state, focused_index, commands)
    };
    session.updates.extend(updates);
}

pub fn take_updates() -> Option<KindPickerUpdateBatch> {
    let mut session = SESSION.lock().ok()?;
    if session.active_page.is_none() || session.updates.is_empty() || session.generation == 0 {
        return None;
    }
    Some(KindPickerUpdateBatch {
        generation: KindPickerGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}
