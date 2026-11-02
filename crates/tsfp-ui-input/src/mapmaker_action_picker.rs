//! Native input adapter for the T1714 Map Maker Select Action(s) page.

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::action_picker::{ActionPickerChoice, ActionPickerPageState, ActionPickerUpdate};

pub const BUILDER: u32 = 0x32_8350;
pub const HEADER_LABEL: u32 = 0x0D69;
pub const ADD_ACTION_LABEL: u32 = 0x0B45;

/// Recognize the measured page header and one Add New Action row. Other rows
/// retain their visible order and become generation-local opaque row choices;
/// T1714 does not establish their runtime record IDs or callback effects.
pub fn exact_choices(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<ActionPickerChoice>> {
    if live.builder != BUILDER || cursor_nodes.is_empty() {
        return None;
    }
    if !live
        .rows
        .iter()
        .any(|row| row.text == crate::live_page::LiveText::Id(HEADER_LABEL))
    {
        return None;
    }
    let mut seen_nodes = Vec::with_capacity(cursor_nodes.len());
    let mut choices = Vec::with_capacity(cursor_nodes.len());
    let mut next_existing = 0u16;
    let mut add_count = 0;
    for node in cursor_nodes {
        if seen_nodes.contains(node) {
            return None;
        }
        seen_nodes.push(*node);
        let row = live.rows.iter().find(|row| row.node == *node)?;
        if row.text == crate::live_page::LiveText::Id(ADD_ACTION_LABEL) {
            add_count += 1;
            choices.push(ActionPickerChoice::AddNewAction);
        } else {
            choices.push(ActionPickerChoice::ExistingRow(next_existing));
            next_existing = next_existing.checked_add(1)?;
        }
    }
    (add_count == 1).then_some(choices)
}

pub fn dispatch(
    state: &mut ActionPickerPageState,
    choices: &[ActionPickerChoice],
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<ActionPickerUpdate> {
    let choice_at = |index: usize| choices.get(index).copied();
    match command {
        NativeCommand::Focus(index) => choice_at(index as usize)
            .and_then(|choice| state.target(choice))
            .and_then(|target| state.focus(target)),
        NativeCommand::Select(index) => choice_at(index as usize)
            .and_then(|choice| state.target(choice))
            .and_then(|target| state.activate(target)),
        NativeCommand::Accept => focused_index
            .and_then(choice_at)
            .and_then(|choice| state.target(choice))
            .and_then(|target| state.activate(target)),
        NativeCommand::Back => Some(state.back()),
        NativeCommand::Step(Step::Up | Step::Down) => {
            let index = focused_index?;
            let next = match command {
                NativeCommand::Step(Step::Up) if index > 0 => index - 1,
                NativeCommand::Step(Step::Down) if index + 1 < choices.len() => index + 1,
                NativeCommand::Step(Step::Up) if wrap => choices.len() - 1,
                NativeCommand::Step(Step::Down) if wrap => 0,
                NativeCommand::Step(_) => index,
                _ => return None,
            };
            choice_at(next)
                .and_then(|choice| state.target(choice))
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
    state: &mut ActionPickerPageState,
    choices: &[ActionPickerChoice],
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<ActionPickerUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, choices, focus, *command, wrap) {
            updates.push(update);
        }
        if let Some(choice) = state.focused() {
            focus = choices.iter().position(|candidate| *candidate == choice);
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_page::{LiveRow, LiveText};
    use tsfp_mapmaker::action_picker::{ActionPickerGeneration, ActionPickerUpdate};

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

    fn live() -> (LivePage, Vec<u32>) {
        let rows = vec![
            row(1, HEADER_LABEL),
            row(2, 0xF001),
            row(3, ADD_ACTION_LABEL),
        ];
        (
            LivePage {
                slot: 7,
                builder: BUILDER,
                state: 2,
                cursor_node: 2,
                rows,
                units: (6.4, 4.8),
            },
            vec![2, 3],
        )
    }

    #[test]
    fn exact_page_admits_runtime_action_and_add_rows_in_visible_order() {
        let (live, nodes) = live();
        assert_eq!(
            exact_choices(&live, &nodes),
            Some(vec![
                ActionPickerChoice::ExistingRow(0),
                ActionPickerChoice::AddNewAction
            ])
        );
        let choices = exact_choices(&live, &nodes).unwrap();
        let mut state =
            ActionPickerPageState::new(ActionPickerGeneration::new(1), &choices).unwrap();
        assert_eq!(
            dispatch_commands(
                &mut state,
                &choices,
                Some(0),
                &[NativeCommand::Step(Step::Down), NativeCommand::Accept],
                false,
            ),
            [
                ActionPickerUpdate::Focused(ActionPickerChoice::AddNewAction),
                ActionPickerUpdate::SelectionRequested(ActionPickerChoice::AddNewAction)
            ]
        );
    }

    #[test]
    fn rejects_missing_duplicate_or_reordered_static_page_evidence() {
        let (mut wrong_header, nodes) = live();
        assert!(exact_choices(&wrong_header, &nodes).is_some());
        wrong_header.rows[0].text = LiveText::Id(0xAAAA);
        assert!(exact_choices(&wrong_header, &nodes).is_none());
        let (valid_page, _) = live();
        assert!(exact_choices(&valid_page, &[2, 2]).is_none());
        assert!(exact_choices(&valid_page, &[2]).is_none());
        let mut wrong_builder = valid_page;
        wrong_builder.builder += 1;
        assert!(exact_choices(&wrong_builder, &nodes).is_none());
    }
}
