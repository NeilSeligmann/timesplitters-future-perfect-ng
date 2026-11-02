//! Native input routing for the measured Sample Map chooser.

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::sample_map_menu::{
    SAMPLE_MAP_CHOICES, SampleMapChoice, SampleMapMenuState, SampleMapMenuUpdate,
};

pub const BUILDER: u32 = 0x33_7ED0;
pub const ROW_HANDLER: u32 = 0x36_1710;

/// T1714's ten rows in display order. The last two share label id zero but
/// retain separate Rust identities because only the first is known to return.
pub const ROW_LABELS: [u32; 10] = [0x771, 0x7F4, 0x873, 0x879, 0x899, 0x8AE, 0x8F0, 0x93D, 0, 0];

pub fn exact_actions(
    live: &LivePage,
    cursor_nodes: &[u32],
) -> Option<Vec<Option<SampleMapChoice>>> {
    if live.builder != BUILDER
        || cursor_nodes.len() != ROW_LABELS.len()
        || cursor_nodes
            .iter()
            .enumerate()
            .any(|(index, node)| cursor_nodes[..index].contains(node))
    {
        return None;
    }
    let mut actions = Vec::with_capacity(cursor_nodes.len());
    for (index, node) in cursor_nodes.iter().enumerate() {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let crate::live_page::LiveText::Id(label) = &row.text else {
            return None;
        };
        if *label != ROW_LABELS[index] || row.activate != ROW_HANDLER {
            return None;
        }
        actions.push(Some(SAMPLE_MAP_CHOICES[index]));
    }
    Some(actions)
}

pub fn dispatch(
    state: &mut SampleMapMenuState,
    actions: &[Option<SampleMapChoice>],
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<SampleMapMenuUpdate> {
    let target_at = |index: usize| {
        actions
            .get(index)
            .copied()
            .flatten()
            .and_then(|choice| state.target(choice))
    };
    match command {
        NativeCommand::Focus(index) => {
            target_at(index as usize).and_then(|target| state.focus(target))
        }
        NativeCommand::Select(index) => {
            target_at(index as usize).and_then(|target| state.select(target))
        }
        NativeCommand::Accept => focused_index
            .and_then(target_at)
            .and_then(|target| state.select(target)),
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
            target_at(next).and_then(|target| state.focus(target))
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
    state: &mut SampleMapMenuState,
    actions: &[Option<SampleMapChoice>],
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<SampleMapMenuUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, actions, focus, *command, wrap) {
            updates.push(update);
        }
        match command {
            NativeCommand::Focus(index) | NativeCommand::Select(index) => {
                focus = Some(*index as usize);
            }
            NativeCommand::Step(Step::Up) => focus = focus.map(|index| index.saturating_sub(1)),
            NativeCommand::Step(Step::Down) => {
                focus = focus.map(|index| (index + 1).min(actions.len().saturating_sub(1)));
            }
            NativeCommand::Step(Step::Left | Step::Right)
            | NativeCommand::Adjust { .. }
            | NativeCommand::Scroll(_)
            | NativeCommand::Back
            | NativeCommand::Accept
            | NativeCommand::Delete
            | NativeCommand::Shortcut
            | NativeCommand::Join(_)
            | NativeCommand::ResumeGame => {}
        }
        if wrap {
            focus = focus.map(|index| index % actions.len().max(1));
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_page::{LiveRow, LiveText};
    use tsfp_mapmaker::sample_map_menu::SampleMapMenuGeneration;

    fn live_page(labels: &[u32], handler: u32) -> (LivePage, Vec<u32>) {
        let rows = labels
            .iter()
            .enumerate()
            .map(|(index, label)| LiveRow {
                node: index as u32 + 1,
                node_type: 0,
                id: 0,
                flags: 0,
                text: LiveText::Id(*label),
                font: 0,
                value_font: 0,
                align: 0,
                rect: None,
                clip: None,
                value: None,
                activate: handler,
                draw: 0,
                kind: 0,
                selected: 0,
                options: Vec::new(),
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
                cursor_node: 0,
                rows,
                units: (6.4, 4.8),
            },
            nodes,
        )
    }

    #[test]
    fn exact_table_keeps_both_zero_rows_distinct() {
        let (live, nodes) = live_page(&ROW_LABELS, ROW_HANDLER);
        assert_eq!(
            exact_actions(&live, &nodes).unwrap(),
            SAMPLE_MAP_CHOICES.map(Some)
        );
        let mut reordered = ROW_LABELS;
        reordered.swap(0, 1);
        let (wrong_order, nodes) = live_page(&reordered, ROW_HANDLER);
        assert!(exact_actions(&wrong_order, &nodes).is_none());
        let (wrong_handler, nodes) = live_page(&ROW_LABELS, ROW_HANDLER + 1);
        assert!(exact_actions(&wrong_handler, &nodes).is_none());
        let (wrong_page, nodes) = live_page(&ROW_LABELS, ROW_HANDLER);
        let mut wrong_page = wrong_page;
        wrong_page.builder += 1;
        assert!(exact_actions(&wrong_page, &nodes).is_none());
    }

    #[test]
    fn keyboard_accept_pointer_selection_and_back_emit_typed_updates() {
        let (live, nodes) = live_page(&ROW_LABELS, ROW_HANDLER);
        let actions = exact_actions(&live, &nodes).unwrap();
        let mut state = SampleMapMenuState::new(SampleMapMenuGeneration::new(7), &actions);
        assert_eq!(
            dispatch_commands(
                &mut state,
                &actions,
                Some(0),
                &[
                    NativeCommand::Step(Step::Down),
                    NativeCommand::Accept,
                    NativeCommand::Select(8),
                    NativeCommand::Back,
                ],
                false,
            ),
            [
                SampleMapMenuUpdate::Focused(SampleMapChoice::ALittleHead),
                SampleMapMenuUpdate::SelectionRequested(SampleMapChoice::ALittleHead),
                SampleMapMenuUpdate::SelectionRequested(SampleMapChoice::SpecialReturn),
                SampleMapMenuUpdate::BackRequested,
            ]
        );
    }
}
