//! Native input adapter for the measured Map Maker trigger-selector rows.

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::trigger_selection::{
    TRIGGER_SELECTION_ACTIONS, TriggerSelectionAction, TriggerSelectionPageState,
    TriggerSelectionUpdate,
};

pub const BUILDER: u32 = 0x32_7F90;
pub const STATIC_LABELS: [(u32, TriggerSelectionAction); 4] = [
    (0x0B44, TriggerSelectionAction::AddTrigger),
    (0x0D6B, TriggerSelectionAction::MatchAll),
    (0x0D6C, TriggerSelectionAction::MatchAny),
    (0x0B66, TriggerSelectionAction::Negate),
];

/// Bind measured static controls; dynamic trigger entries stay on the existing
/// live row path and have no invented semantic action.
pub fn exact_actions(
    live: &LivePage,
    cursor_nodes: &[u32],
) -> Option<Vec<Option<TriggerSelectionAction>>> {
    if live.builder != BUILDER || cursor_nodes.len() < STATIC_LABELS.len() {
        return None;
    }
    let mut actions = Vec::with_capacity(cursor_nodes.len());
    let mut seen = [false; STATIC_LABELS.len()];
    for node in cursor_nodes {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let action = match &row.text {
            crate::live_page::LiveText::Id(label) => STATIC_LABELS
                .iter()
                .position(|(candidate, _)| *candidate == *label)
                .map(|index| {
                    seen[index] = true;
                    STATIC_LABELS[index].1
                }),
            _ => None,
        };
        actions.push(action);
    }
    (seen.into_iter().all(|found| found)
        && actions
            .iter()
            .flatten()
            .copied()
            .eq(TRIGGER_SELECTION_ACTIONS))
    .then_some(actions)
}

pub fn dispatch(
    state: &mut TriggerSelectionPageState,
    actions: &[Option<TriggerSelectionAction>],
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<TriggerSelectionUpdate> {
    let action_at = |index: usize| actions.get(index).copied().flatten();
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
    state: &mut TriggerSelectionPageState,
    actions: &[Option<TriggerSelectionAction>],
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<TriggerSelectionUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, actions, focus, *command, wrap) {
            updates.push(update);
        }
        if let Some(action) = state.focused() {
            focus = actions
                .iter()
                .position(|candidate| *candidate == Some(action));
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_page::{LiveRow, LiveText};
    use tsfp_mapmaker::trigger_selection::{TriggerSelectionGeneration, TriggerSelectionUpdate};

    fn live_with_dynamic_row() -> (LivePage, Vec<u32>) {
        let labels = [0x0B44, 0x0D6B, 0x9999, 0x0D6C, 0x0B66];
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
        let nodes = rows.iter().map(|row| row.node).collect();
        (
            LivePage {
                slot: 5,
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
    fn measured_controls_allow_dynamic_trigger_rows_between_them() {
        let (page, nodes) = live_with_dynamic_row();
        let actions = exact_actions(&page, &nodes).unwrap();
        assert_eq!(actions.len(), 5);
        assert_eq!(actions[2], None);
        let mut state = TriggerSelectionPageState::new(TriggerSelectionGeneration::new(3));
        assert_eq!(
            dispatch_commands(
                &mut state,
                &actions,
                Some(1),
                &[NativeCommand::Accept],
                false,
            ),
            [TriggerSelectionUpdate::ActionRequested(
                TriggerSelectionAction::MatchAll
            )]
        );
        assert_eq!(
            dispatch_commands(
                &mut state,
                &actions,
                Some(3),
                &[NativeCommand::Focus(4), NativeCommand::Select(4)],
                false,
            ),
            [
                TriggerSelectionUpdate::Focused(TriggerSelectionAction::Negate),
                TriggerSelectionUpdate::ActionRequested(TriggerSelectionAction::Negate),
            ]
        );
    }

    #[test]
    fn reordered_or_missing_static_rows_are_not_admitted() {
        let (mut page, nodes) = live_with_dynamic_row();
        let mut reordered_nodes = nodes.clone();
        reordered_nodes.swap(1, 3);
        assert!(exact_actions(&page, &reordered_nodes).is_none());
        page.rows.retain(|row| row.node != 5);
        assert!(exact_actions(&page, &nodes).is_none());
    }
}
