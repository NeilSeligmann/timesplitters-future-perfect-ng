//! Native input adapter for T1714's dynamic Awards table.
//!
//! The table rows and trailing extra item stay in runtime order. Their
//! meanings and callback effects are not established by T1714, so activation
//! emits an opaque page-local row request.

use std::vec::Vec;

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::award_table::{AwardTablePageState, AwardTableUpdate};

pub const BUILDER: u32 = 0x32_78D0;

/// Return cursor row identities in their original order, including the
/// measured trailing extra item. No labels or row meanings are inferred.
pub fn row_nodes(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<u32>> {
    if live.builder != BUILDER || cursor_nodes.is_empty() {
        return None;
    }
    let mut seen = std::collections::BTreeSet::new();
    cursor_nodes
        .iter()
        .all(|node| seen.insert(*node) && live.rows.iter().any(|row| row.node == *node))
        .then(|| cursor_nodes.to_vec())
}

fn target(
    state: &AwardTablePageState,
    index: u32,
) -> Option<tsfp_mapmaker::award_table::AwardTableTarget> {
    state.target(index as usize)
}

pub fn dispatch(
    state: &mut AwardTablePageState,
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<AwardTableUpdate> {
    let count = state.row_count() as usize;
    match command {
        NativeCommand::Focus(index) => {
            let target = target(state, index)?;
            state.focus(target)
        }
        NativeCommand::Select(index) => {
            let target = target(state, index)?;
            state.activate(target)
        }
        NativeCommand::Accept => {
            let index = u32::try_from(focused_index?).ok()?;
            let target = target(state, index)?;
            state.activate(target)
        }
        NativeCommand::Back => Some(state.back()),
        NativeCommand::Step(Step::Up | Step::Down) if count > 0 => {
            let down = matches!(command, NativeCommand::Step(Step::Down));
            let next = match (focused_index, down, wrap) {
                (Some(index), true, _) if index + 1 < count => index + 1,
                (Some(index), false, _) if index > 0 => index - 1,
                (Some(_), true, true) => 0,
                (Some(_), false, true) => count - 1,
                (Some(_), _, _) => return None,
                (None, true, _) => 0,
                (None, false, _) => count - 1,
            };
            let index = u32::try_from(next).ok()?;
            let target = target(state, index)?;
            state.focus(target)
        }
        NativeCommand::Step(Step::Up | Step::Down) => None,
        NativeCommand::Step(Step::Left | Step::Right)
        | NativeCommand::Delete
        | NativeCommand::Shortcut
        | NativeCommand::Join(_)
        | NativeCommand::ResumeGame
        | NativeCommand::Scroll(_)
        | NativeCommand::Adjust { .. } => None,
    }
}

pub fn dispatch_commands(
    state: &mut AwardTablePageState,
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<AwardTableUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, focus, *command, wrap) {
            match update {
                AwardTableUpdate::Focused(row) | AwardTableUpdate::SelectionRequested(row) => {
                    focus = Some(row.ordinal() as usize);
                }
                AwardTableUpdate::BackRequested => {}
            }
            updates.push(update);
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_page::{LiveRow, LiveText};
    use tsfp_mapmaker::award_table::AwardTableGeneration;

    fn row(node: u32, id: i32) -> LiveRow {
        LiveRow {
            node,
            node_type: 4,
            id,
            flags: 0,
            text: LiveText::Id(id as u32),
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

    fn live(builder: u32) -> (LivePage, Vec<u32>) {
        let nodes = [10, 11, 12];
        (
            LivePage {
                slot: 3,
                builder,
                state: 3,
                cursor_node: nodes[0],
                rows: nodes
                    .iter()
                    .enumerate()
                    .map(|(index, node)| row(*node, index as i32))
                    .collect(),
                units: (6.4, 4.8),
            },
            nodes.to_vec(),
        )
    }

    #[test]
    fn admits_live_order_without_guessing_dynamic_rows_or_trailing_item() {
        let (page, nodes) = live(BUILDER);
        assert_eq!(row_nodes(&page, &nodes), Some(nodes));
        assert!(row_nodes(&page, &[]).is_none());
        let (other, nodes) = live(0x32_8D20);
        assert!(row_nodes(&other, &nodes).is_none());
    }

    #[test]
    fn rejects_duplicate_live_nodes_before_assigning_page_local_ordinals() {
        let (page, nodes) = live(BUILDER);
        assert!(row_nodes(&page, &[nodes[0], nodes[0], nodes[2]]).is_none());
    }

    #[test]
    fn keyboard_and_pointer_commands_emit_typed_row_requests() {
        let mut state = AwardTablePageState::new(AwardTableGeneration::new(4), 3);
        let updates = dispatch_commands(
            &mut state,
            None,
            &[
                NativeCommand::Focus(2),
                NativeCommand::Accept,
                NativeCommand::Step(Step::Up),
                NativeCommand::Back,
            ],
            false,
        );
        assert_eq!(updates.len(), 4);
        assert!(matches!(updates[0], AwardTableUpdate::Focused(row) if row.ordinal() == 2));
        assert!(
            matches!(updates[1], AwardTableUpdate::SelectionRequested(row) if row.ordinal() == 2)
        );
        assert!(matches!(updates[2], AwardTableUpdate::Focused(row) if row.ordinal() == 1));
        assert_eq!(updates[3], AwardTableUpdate::BackRequested);
    }
}
