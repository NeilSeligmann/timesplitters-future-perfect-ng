//! Native input adapter for T1714's measured Award Medal Settings labels.

use crate::{
    live_page::{LivePage, LiveText},
    native_page::NativeCommand,
    widgets::Step,
};
use tsfp_mapmaker::award_medal_settings::{
    AwardMedalAction, AwardMedalPageState, AwardMedalUpdate,
};

pub const BUILDER: u32 = 0x32_7A70;
pub const STATIC_LABELS: [(u32, AwardMedalAction); 6] = [
    (0x0B44, AwardMedalAction::AddNewTrigger),
    (0x0B49, AwardMedalAction::MainObjectivesCompleted),
    (0x0B4A, AwardMedalAction::SecondaryObjectivesCompleted),
    (0x0B4B, AwardMedalAction::AwardIfAllCompleted),
    (0x0B4C, AwardMedalAction::AwardIfAnyCompleted),
    (0x0B5E, AwardMedalAction::Done),
];

/// Return a page-local action map. Unrecognized rows are retained as `None`
/// because T1714 does not define the dynamic trigger-entry rows.
pub fn measured_action_rows(
    live: &LivePage,
    cursor_nodes: &[u32],
) -> Option<Vec<Option<AwardMedalAction>>> {
    if live.builder != BUILDER {
        return None;
    }
    let mut actions = vec![None; cursor_nodes.len()];
    let mut previous = None;
    for (label, action) in STATIC_LABELS {
        let found = cursor_nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                live.rows
                    .iter()
                    .find(|row| row.node == *node)
                    .and_then(|row| {
                        matches!(&row.text, LiveText::Id(id) if *id == label).then_some(index)
                    })
            })
            .collect::<Vec<_>>();
        if found.len() != 1 {
            return None;
        }
        let index = found[0];
        if previous.is_some_and(|before| index <= before) {
            return None;
        }
        actions[index] = Some(action);
        previous = Some(index);
    }
    Some(actions)
}

pub fn dispatch(
    state: &mut AwardMedalPageState,
    actions: &[Option<AwardMedalAction>],
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<AwardMedalUpdate> {
    let action_at = |index: usize| actions.get(index).copied().flatten();
    let activate_at = |state: &mut AwardMedalPageState, index: usize| {
        if let Some(action) = action_at(index) {
            state.activate(state.target(action))
        } else {
            state.clear_focus();
            None
        }
    };
    match command {
        NativeCommand::Focus(index) => dispatch_pointer(state, actions, index as usize, false),
        NativeCommand::Select(index) => dispatch_pointer(state, actions, index as usize, true),
        NativeCommand::Accept => {
            if let Some(index) = focused_index {
                activate_at(state, index)
            } else {
                state.clear_focus();
                None
            }
        }
        NativeCommand::Back => Some(state.back()),
        NativeCommand::Step(Step::Up | Step::Down) => {
            let index = focused_index?;
            focus_at(
                state,
                action_at(step_target(index, actions.len(), command, wrap)?),
            )
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

fn focus_at(
    state: &mut AwardMedalPageState,
    action: Option<AwardMedalAction>,
) -> Option<AwardMedalUpdate> {
    if let Some(action) = action {
        state.focus(state.target(action))
    } else {
        state.clear_focus();
        None
    }
}

fn step_target(index: usize, len: usize, command: NativeCommand, wrap: bool) -> Option<usize> {
    match command {
        NativeCommand::Step(Step::Up) if index > 0 => Some(index - 1),
        NativeCommand::Step(Step::Down) if index + 1 < len => Some(index + 1),
        NativeCommand::Step(Step::Up) if wrap => Some(len.saturating_sub(1)),
        NativeCommand::Step(Step::Down) if wrap => Some(0),
        NativeCommand::Step(Step::Up | Step::Down) => Some(index),
        _ => None,
    }
}

pub fn dispatch_commands(
    state: &mut AwardMedalPageState,
    actions: &[Option<AwardMedalAction>],
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<AwardMedalUpdate> {
    let mut updates = Vec::new();
    let mut focused = focused_index;
    for command in commands {
        let next_focused = match command {
            NativeCommand::Focus(index) | NativeCommand::Select(index) => Some(*index as usize),
            NativeCommand::Step(Step::Up | Step::Down) => {
                focused.and_then(|index| step_target(index, actions.len(), *command, wrap))
            }
            _ => focused,
        };
        if let Some(update) = dispatch(state, actions, focused, *command, wrap) {
            updates.push(update);
        }
        focused = next_focused;
    }
    updates
}

/// Dispatch a pointer event only when the focused row is one of the measured
/// static actions. Dynamic trigger rows remain owned by their caller seam.
pub fn dispatch_pointer(
    state: &mut AwardMedalPageState,
    actions: &[Option<AwardMedalAction>],
    row_index: usize,
    activate: bool,
) -> Option<AwardMedalUpdate> {
    let Some(action) = actions.get(row_index).copied().flatten() else {
        state.clear_focus();
        return None;
    };
    let target = state.target(action);
    if activate {
        state.activate(target)
    } else {
        state.focus(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_page::LiveRow;
    use tsfp_mapmaker::award_medal_settings::{AwardMedalGeneration, AwardMedalUpdate};

    fn page(labels: &[u32]) -> (LivePage, Vec<u32>) {
        let rows = labels
            .iter()
            .enumerate()
            .map(|(i, label)| LiveRow {
                node: i as u32 + 1,
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
                slot: 1,
                builder: BUILDER,
                state: 2,
                cursor_node: 0,
                rows,
                units: (1.0, 1.0),
            },
            nodes,
        )
    }

    #[test]
    fn measured_static_actions_admit_dynamic_rows_and_native_activation() {
        let (live, nodes) = page(&[0x0B44, 0x0B77, 0x0B49, 0x0B4A, 0x0B4B, 0x0B4C, 0x0B5E]);
        let actions = measured_action_rows(&live, &nodes).unwrap();
        assert_eq!(actions[1], None);
        let mut state = AwardMedalPageState::new(AwardMedalGeneration::new(2));
        assert_eq!(
            dispatch_commands(
                &mut state,
                &actions,
                Some(0),
                &[NativeCommand::Step(Step::Down), NativeCommand::Accept],
                false
            ),
            []
        );
        assert_eq!(state.focused(), None);
        assert_eq!(
            dispatch_commands(
                &mut state,
                &actions,
                Some(1),
                &[NativeCommand::Step(Step::Down), NativeCommand::Accept],
                false
            ),
            [
                AwardMedalUpdate::Focused(AwardMedalAction::MainObjectivesCompleted),
                AwardMedalUpdate::ActivationRequested(AwardMedalAction::MainObjectivesCompleted),
            ]
        );
        assert_eq!(
            dispatch(&mut state, &actions, Some(1), NativeCommand::Accept, false),
            None
        );
        assert_eq!(
            dispatch(
                &mut state,
                &actions,
                Some(6),
                NativeCommand::Select(6),
                false
            ),
            Some(AwardMedalUpdate::ActivationRequested(
                AwardMedalAction::Done
            ))
        );
    }

    #[test]
    fn incomplete_duplicate_or_reordered_static_labels_fail_closed() {
        let (live, nodes) = page(&[0x0B44, 0x0B49, 0x0B4A, 0x0B4C, 0x0B4B, 0x0B5E]);
        assert!(measured_action_rows(&live, &nodes).is_none());
        let (live, nodes) = page(&[0x0B44, 0x0B49, 0x0B4A, 0x0B4B, 0x0B4C, 0x0B5E, 0x0B49]);
        assert!(measured_action_rows(&live, &nodes).is_none());
    }

    #[test]
    fn pointer_focus_and_activation_accept_only_measured_static_rows() {
        let (live, nodes) = page(&[0x0B44, 0x0B77, 0x0B49, 0x0B4A, 0x0B4B, 0x0B4C, 0x0B5E]);
        let actions = measured_action_rows(&live, &nodes).unwrap();
        let mut state = AwardMedalPageState::new(AwardMedalGeneration::new(9));
        assert_eq!(dispatch_pointer(&mut state, &actions, 1, true), None);
        assert_eq!(state.focused(), None);
        assert_eq!(
            dispatch_pointer(&mut state, &actions, 2, false),
            Some(AwardMedalUpdate::Focused(
                AwardMedalAction::MainObjectivesCompleted
            ))
        );
        assert_eq!(
            dispatch_pointer(&mut state, &actions, 2, true),
            Some(AwardMedalUpdate::ActivationRequested(
                AwardMedalAction::MainObjectivesCompleted
            ))
        );
    }
}
