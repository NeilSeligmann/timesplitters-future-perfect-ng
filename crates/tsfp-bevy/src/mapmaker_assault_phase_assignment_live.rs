// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: native input state for the measured Assault phase start-point page.
//!
//! T1714 supplies the page and action labels. The eventual point-picker and
//! document mutation stay behind the typed request until their caller contract
//! is ported.

use std::{collections::VecDeque, sync::Mutex};

use tsfp_mapmaker::assault_phase_assignment::{
    AssaultPhaseAssignmentAction, AssaultPhaseAssignmentCommand, AssaultPhaseAssignmentGeneration,
    AssaultPhaseAssignmentState, AssaultPhaseAssignmentUpdate,
};
use tsfp_ui_input::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_ui_model::model::{Page, WidgetId};

pub const BUILDER: u32 = 0x31_90C0;
const SELECT_START_POINT_LABEL: u32 = 0x0B38;
const DONE_LABEL: u32 = 0x0B5E;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    last_parent_page: Option<(u32, u32)>,
    generation: u64,
    updates: Vec<AssaultPhaseAssignmentUpdate>,
    pending: VecDeque<AssaultPhaseAssignmentUpdateBatch>,
}

impl Session {
    fn observe_top(&mut self, page: Option<(u32, u32)>, preserve_generation: bool) -> bool {
        if self.active_page == page {
            return false;
        }
        self.seal_updates();
        self.active_page = page;
        if let Some(page) = page {
            if !(preserve_generation && self.last_parent_page == Some(page)) {
                self.generation = self.generation.wrapping_add(1).max(1);
            }
            self.last_parent_page = Some(page);
        }
        true
    }

    fn seal_updates(&mut self) {
        if self.generation != 0 && !self.updates.is_empty() {
            self.pending.push_back(AssaultPhaseAssignmentUpdateBatch {
                generation: AssaultPhaseAssignmentGeneration::new(self.generation),
                updates: std::mem::take(&mut self.updates),
            });
        }
    }

    fn take_updates(&mut self) -> Option<AssaultPhaseAssignmentUpdateBatch> {
        if let Some(batch) = self.pending.pop_front() {
            return Some(batch);
        }
        if self.active_page.is_none() || self.updates.is_empty() || self.generation == 0 {
            return None;
        }
        Some(AssaultPhaseAssignmentUpdateBatch {
            generation: AssaultPhaseAssignmentGeneration::new(self.generation),
            updates: std::mem::take(&mut self.updates),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssaultPhaseAssignmentUpdateBatch {
    pub generation: AssaultPhaseAssignmentGeneration,
    pub updates: Vec<AssaultPhaseAssignmentUpdate>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    last_parent_page: None,
    generation: 0,
    updates: Vec::new(),
    pending: VecDeque::new(),
});

pub fn observe_top(top: Option<(u32, u32)>) {
    let page = top.filter(|(builder, _)| *builder == BUILDER);
    let mapmaker_ui_enabled = crate::storage_live::mapmaker_ui_enabled();
    let preserve_generation = page.is_some()
        && mapmaker_ui_enabled
        && crate::storage_live::mapmaker_ui()
            .lock()
            .is_ok_and(|state| {
                state.assault_child_return_pending() || state.assault_start_point_picker().is_some()
            });
    let changed = {
        let Ok(mut session) = SESSION.lock() else {
            return;
        };
        session.observe_top(page, preserve_generation)
    };
    if changed
        && mapmaker_ui_enabled
        && let Ok(mut state) = crate::storage_live::mapmaker_ui().lock()
    {
        state.observe_assault_phase_page_change(page.is_some());
    }
}

fn action_for_label(label: u32) -> Option<AssaultPhaseAssignmentAction> {
    match label {
        SELECT_START_POINT_LABEL => Some(AssaultPhaseAssignmentAction::SelectStartPoint),
        DONE_LABEL => Some(AssaultPhaseAssignmentAction::Done),
        _ => None,
    }
}

fn row_action(live: &LivePage, node: u32) -> Option<Option<AssaultPhaseAssignmentAction>> {
    let row = live.rows.iter().find(|row| row.node == node)?;
    match &row.text {
        tsfp_ui_input::live_page::LiveText::Id(label) => Some(action_for_label(*label)),
        _ => Some(None),
    }
}

fn exact_actions(
    live: &LivePage,
    cursor_nodes: &[u32],
) -> Option<Vec<Option<AssaultPhaseAssignmentAction>>> {
    if live.builder != BUILDER || cursor_nodes.is_empty() {
        return None;
    }
    for (index, node) in cursor_nodes.iter().enumerate() {
        if cursor_nodes[..index].contains(node) || !live.rows.iter().any(|row| row.node == *node) {
            return None;
        }
    }
    let select_nodes = live
        .rows
        .iter()
        .filter(|row| row.text == tsfp_ui_input::live_page::LiveText::Id(SELECT_START_POINT_LABEL))
        .map(|row| row.node)
        .collect::<Vec<_>>();
    let done_nodes = live
        .rows
        .iter()
        .filter(|row| row.text == tsfp_ui_input::live_page::LiveText::Id(DONE_LABEL))
        .map(|row| row.node)
        .collect::<Vec<_>>();
    if select_nodes.len() != 1 || done_nodes.len() != 1 {
        return None;
    }
    let select_index = cursor_nodes
        .iter()
        .position(|node| *node == select_nodes[0])?;
    let done_index = cursor_nodes
        .iter()
        .position(|node| *node == done_nodes[0])?;
    if select_index >= done_index {
        return None;
    }
    cursor_nodes
        .iter()
        .map(|node| row_action(live, *node))
        .collect()
}

pub fn supported(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    exact_actions(live, cursor_nodes).is_some()
}

fn focus_action(
    actions: &[Option<AssaultPhaseAssignmentAction>],
    index: u32,
) -> Option<AssaultPhaseAssignmentAction> {
    actions.get(index as usize).copied().flatten()
}

fn ensure_state(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    wrap: bool,
    focused_index: Option<usize>,
) -> Option<Vec<Option<AssaultPhaseAssignmentAction>>> {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let actions = exact_actions(live, cursor_nodes)?;
    let generation = {
        let session = SESSION.lock().ok()?;
        if session.active_page != Some((live.builder, slot)) || session.generation == 0 {
            return None;
        }
        session.generation
    };
    let Ok(mut ui_state) = crate::storage_live::mapmaker_ui().lock() else {
        return None;
    };
    let page_generation = AssaultPhaseAssignmentGeneration::new(generation);
    let mut state = ui_state
        .assault_phase_assignment()
        .filter(|state| state.generation() == page_generation)
        .unwrap_or_else(|| AssaultPhaseAssignmentState::new(page_generation, wrap));
    let focus_update = focused_index
        .and_then(|index| focus_action(&actions, index as u32))
        .filter(|action| state.focused() != Some(*action))
        .map(|action| state.dispatch(AssaultPhaseAssignmentCommand::Focus(state.target(action))));
    ui_state.set_assault_phase_assignment(Some(state));
    drop(ui_state);
    if let Some(update) = focus_update
        && update != AssaultPhaseAssignmentUpdate::Ignored
        && let Ok(mut session) = SESSION.lock()
        && session.active_page == Some((live.builder, slot))
        && session.generation == generation
    {
        session.updates.push(update);
    }
    Some(actions)
}

/// Keep the runtime's localized text and measured layout while admitting the
/// exact page into the Rust-owned focus/action seam.
pub fn project_owned_page(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    focused: Option<WidgetId>,
    page: Page,
) -> Option<Page> {
    if live.slot != slot {
        return None;
    }
    let focused_index = focused
        .and_then(|widget| live.cursor_index(widget.0 as usize, cursor_nodes))
        .map(|index| index as usize);
    ensure_state(
        live,
        slot,
        cursor_nodes,
        page.wrap == tsfp_ui_model::focus::Wrap::Wrap,
        focused_index,
    )
    .map(|_| page)
}

pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder != BUILDER {
        return;
    }
    let wrap = current
        .content
        .model
        .page(current.content.page)
        .is_some_and(|page| page.wrap == tsfp_ui_model::focus::Wrap::Wrap);
    let focused_index = current.focused.and_then(|widget| {
        current
            .live
            .cursor_index(widget.0 as usize, &current.cursor_nodes)
            .map(|index| index as usize)
    });
    let Some(actions) = ensure_state(
        &current.live,
        current.slot,
        &current.cursor_nodes,
        wrap,
        focused_index,
    ) else {
        return;
    };
    let Ok(mut ui_state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    let Some(mut state) = ui_state.assault_phase_assignment() else {
        return;
    };
    let mut emitted = Vec::new();
    for command in commands {
        let typed = match *command {
            NativeCommand::Focus(index) => focus_action(&actions, index)
                .map(|action| state.target(action))
                .map(AssaultPhaseAssignmentCommand::Focus),
            NativeCommand::Select(index) => focus_action(&actions, index)
                .map(|action| state.target(action))
                .map(AssaultPhaseAssignmentCommand::Activate),
            NativeCommand::Accept => state
                .focused()
                .map(|action| state.target(action))
                .map(AssaultPhaseAssignmentCommand::Activate),
            NativeCommand::Back => Some(AssaultPhaseAssignmentCommand::Back(state.generation())),
            NativeCommand::Step(Step::Up) => Some(AssaultPhaseAssignmentCommand::Move {
                generation: state.generation(),
                delta: -1,
            }),
            NativeCommand::Step(Step::Down) => Some(AssaultPhaseAssignmentCommand::Move {
                generation: state.generation(),
                delta: 1,
            }),
            NativeCommand::Step(Step::Left | Step::Right)
            | NativeCommand::Adjust { .. }
            | NativeCommand::Scroll(_)
            | NativeCommand::Delete
            | NativeCommand::Shortcut
            | NativeCommand::Join(_)
            | NativeCommand::ResumeGame => None,
        };
        if let Some(command) = typed {
            let update = state.dispatch(command);
            if update != AssaultPhaseAssignmentUpdate::Ignored {
                emitted.push(update);
            }
        }
    }
    let generation = state.generation();
    ui_state.set_assault_phase_assignment(Some(state));
    drop(ui_state);
    if let Ok(mut session) = SESSION.lock()
        && session.active_page == Some((BUILDER, current.slot))
        && session.generation == generation.get()
    {
        session.updates.extend(emitted);
    }
}

/// Drain native input outcomes. Outcomes survive closing this page so its
/// caller can process a generation-scoped request after opening a child page.
/// The child chooser and original assignment effect remain typed caller work.
pub fn take_updates() -> Option<AssaultPhaseAssignmentUpdateBatch> {
    SESSION.lock().ok()?.take_updates()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_input::live_page::{LiveRow, LiveText};

    fn row(node: u32, label: u32) -> LiveRow {
        LiveRow {
            node,
            node_type: 0,
            id: 0,
            flags: 0,
            text: LiveText::Id(label),
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
            brightness: 0.0,
            height: 0.0,
            text_align: 0,
            text_x: 0.0,
            text_y: 0.0,
        }
    }

    #[test]
    fn exact_start_point_action_and_done_rows_are_supported() {
        let live = LivePage {
            slot: 3,
            builder: BUILDER,
            state: 2,
            cursor_node: 2,
            rows: vec![
                row(1, 0x0B36),
                row(2, SELECT_START_POINT_LABEL),
                row(3, DONE_LABEL),
            ],
            units: (6.4, 4.8),
        };
        assert!(supported(&live, &[1, 2, 3]));
        assert!(!supported(&live, &[1, 3, 2]));
        assert!(!supported(&live, &[1, 2]));
    }

    #[test]
    fn page_transition_preserves_typed_request_with_its_generation() {
        let mut session = Session {
            active_page: Some((BUILDER, 3)),
            last_parent_page: Some((BUILDER, 3)),
            generation: 8,
            updates: vec![AssaultPhaseAssignmentUpdate::StartPointSelectionRequested],
            pending: VecDeque::new(),
        };

        assert!(session.observe_top(Some((BUILDER + 1, 4)), false));
        let batch = session.take_updates().unwrap();
        assert_eq!(batch.generation.get(), 8);
        assert_eq!(
            batch.updates,
            [AssaultPhaseAssignmentUpdate::StartPointSelectionRequested]
        );
        assert!(session.take_updates().is_none());
    }

    #[test]
    fn queued_page_requests_remain_ordered_across_reopenings() {
        let mut session = Session {
            active_page: Some((BUILDER, 3)),
            last_parent_page: Some((BUILDER, 3)),
            generation: 8,
            updates: vec![AssaultPhaseAssignmentUpdate::StartPointSelectionRequested],
            pending: VecDeque::new(),
        };
        session.observe_top(None, false);
        session.observe_top(Some((BUILDER, 3)), false);
        session
            .updates
            .push(AssaultPhaseAssignmentUpdate::BackRequested);

        let first = session.take_updates().unwrap();
        let second = session.take_updates().unwrap();
        assert_eq!(first.generation.get(), 8);
        assert_eq!(second.generation.get(), 9);
        assert_eq!(
            first.updates,
            [AssaultPhaseAssignmentUpdate::StartPointSelectionRequested]
        );
        assert_eq!(
            second.updates,
            [AssaultPhaseAssignmentUpdate::BackRequested]
        );
    }

    #[test]
    fn returning_from_child_preserves_parent_generation_until_next_reopen() {
        let mut session = Session {
            active_page: None,
            last_parent_page: Some((BUILDER, 3)),
            generation: 8,
            updates: Vec::new(),
            pending: VecDeque::new(),
        };

        assert!(session.observe_top(Some((BUILDER, 3)), true));
        assert_eq!(session.generation, 8);
        assert!(session.observe_top(None, false));
        assert!(session.observe_top(Some((BUILDER, 3)), false));
        assert_eq!(session.generation, 9);
    }

    #[test]
    fn child_picker_round_trip_retains_parent_selection_then_clears_on_exit() {
        use tsfp_mapmaker::{
            assault_phase_assignment::{
                AssaultPhaseAssignmentAction, AssaultPhaseAssignmentCommand,
                AssaultPhaseAssignmentGeneration, AssaultPhaseAssignmentState,
                AssaultPhaseAssignmentUpdate,
            },
            assault_start_point_picker::{
                StartPointChoiceId, StartPointPickerGeneration, StartPointPickerState,
            },
            ui::MapMakerUiState,
        };

        let choice = StartPointChoiceId::new(0xabc);
        let parent_generation = AssaultPhaseAssignmentGeneration::new(8);
        let parent = AssaultPhaseAssignmentState::new(parent_generation, false);
        let mut ui = MapMakerUiState::default();
        ui.set_assault_phase_assignment(Some(parent));
        assert_eq!(
            ui.dispatch_assault_phase_assignment(AssaultPhaseAssignmentCommand::Activate(
                parent.target(AssaultPhaseAssignmentAction::SelectStartPoint),
            )),
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );
        ui.observe_assault_phase_page_change(false);
        ui.set_assault_start_point_picker(Some(
            StartPointPickerState::new(StartPointPickerGeneration::new(12), &[choice], false)
                .unwrap(),
        ));

        let mut session = Session {
            active_page: Some((BUILDER, 3)),
            last_parent_page: Some((BUILDER, 3)),
            generation: 8,
            updates: Vec::new(),
            pending: VecDeque::new(),
        };
        assert!(session.observe_top(None, false));
        assert_eq!(
            ui.complete_assault_start_point_selection(StartPointPickerGeneration::new(12), choice),
            AssaultPhaseAssignmentUpdate::StartPointAssigned(choice)
        );
        assert!(session.observe_top(Some((BUILDER, 3)), ui.assault_child_return_pending()));
        ui.observe_assault_phase_page_change(true);
        assert_eq!(session.generation, 8);
        assert_eq!(
            ui.assault_phase_assignment()
                .unwrap()
                .selected_start_point(),
            Some(choice)
        );

        assert!(session.observe_top(None, false));
        ui.observe_assault_phase_page_change(false);
        assert!(ui.assault_phase_assignment().is_none());
    }
}
