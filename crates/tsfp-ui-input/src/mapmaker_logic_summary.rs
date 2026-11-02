//! Semantic input adapter for the Map Maker Logic Summary page (T2408).
//!
//! T1714 supplies the measured row label order. Callback identities and the
//! nested editor effects are deliberately kept at the live caller seam.

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::logic_summary::{
    LOGIC_SUMMARY_ACTIONS, LogicSummaryAction, LogicSummaryPageState, LogicSummaryUpdate,
};

pub const BUILDER: u32 = 0x32_9DE0;
pub const ROW_LABELS: [u32; 9] = [
    0x0B37, 0x0B39, 0x0B63, 0x0B64, 0x0B65, 0x0B62, 0x0B63, 0x0B64, 0x0B5E,
];

pub fn exact_actions(live: &LivePage, cursor_nodes: &[u32]) -> Option<[LogicSummaryAction; 9]> {
    if live.builder != BUILDER || cursor_nodes.len() != ROW_LABELS.len() {
        return None;
    }
    for (node, label) in cursor_nodes.iter().zip(ROW_LABELS) {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        if row.text != crate::live_page::LiveText::Id(label) {
            return None;
        }
    }
    Some(LOGIC_SUMMARY_ACTIONS)
}

pub fn dispatch(
    state: &mut LogicSummaryPageState,
    actions: &[LogicSummaryAction; 9],
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<LogicSummaryUpdate> {
    let action_at = |index: usize| actions.get(index).copied();
    match command {
        NativeCommand::Focus(index) => action_at(index as usize)
            .and_then(|action| state.target(action))
            .and_then(|target| state.focus(target)),
        NativeCommand::Select(index) => action_at(index as usize)
            .and_then(|action| state.target(action))
            .and_then(|target| state.activate(target)),
        NativeCommand::Accept => focused_index
            .and_then(action_at)
            .and_then(|action| state.target(action))
            .and_then(|target| state.activate(target)),
        NativeCommand::Back => Some(state.back()),
        NativeCommand::Step(Step::Up | Step::Down) => {
            let index = focused_index?;
            let next = match command {
                NativeCommand::Step(Step::Up) if index > 0 => index - 1,
                NativeCommand::Step(Step::Down) if index + 1 < actions.len() => index + 1,
                NativeCommand::Step(Step::Up) if wrap => actions.len() - 1,
                NativeCommand::Step(Step::Down) if wrap => 0,
                NativeCommand::Step(_) => index,
                _ => return None,
            };
            action_at(next)
                .and_then(|action| state.target(action))
                .and_then(|target| state.focus(target))
        }
        NativeCommand::Step(Step::Left | Step::Right)
        | NativeCommand::Adjust { .. }
        | NativeCommand::Scroll(_)
        | NativeCommand::Delete
        | NativeCommand::Shortcut
        | NativeCommand::Join(_)
        | NativeCommand::ResumeGame => None,
    }
}

pub fn dispatch_commands(
    state: &mut LogicSummaryPageState,
    actions: &[LogicSummaryAction; 9],
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<LogicSummaryUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, actions, focus, *command, wrap) {
            updates.push(update);
        }
        if let Some(action) = state.focused() {
            focus = actions.iter().position(|candidate| *candidate == action);
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_mapmaker::logic_summary::{LogicSummaryGeneration, LogicSummaryUpdate};

    #[test]
    fn keyboard_steps_and_accept_route_to_typed_action() {
        let mut state = LogicSummaryPageState::new(LogicSummaryGeneration::new(4));
        let updates = dispatch_commands(
            &mut state,
            &LOGIC_SUMMARY_ACTIONS,
            Some(1),
            &[NativeCommand::Step(Step::Down), NativeCommand::Accept],
            false,
        );
        assert_eq!(
            updates,
            [
                LogicSummaryUpdate::Focused(LogicSummaryAction::FirstTriggerList),
                LogicSummaryUpdate::ActionRequested(LogicSummaryAction::FirstTriggerList),
            ]
        );
    }

    #[test]
    fn unknown_indices_and_stale_accept_focus_do_not_create_actions() {
        let mut state = LogicSummaryPageState::new(LogicSummaryGeneration::new(4));
        assert_eq!(
            dispatch(
                &mut state,
                &LOGIC_SUMMARY_ACTIONS,
                None,
                NativeCommand::Accept,
                false,
            ),
            None
        );
        assert_eq!(
            dispatch(
                &mut state,
                &LOGIC_SUMMARY_ACTIONS,
                None,
                NativeCommand::Select(9),
                false,
            ),
            None
        );
    }
}
