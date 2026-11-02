//! Native input adapter for T1714's mode-filtered kind list.
//!
//! Row text and mode filtering remain supplied by the current page source;
//! keyboard, controller, pointer and Back commands produce typed row intent.

use std::vec::Vec;

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::kind_list::{KindListPageState, KindListTarget, KindListUpdate};

pub const BUILDER: u32 = 0x32_A230;

pub fn row_nodes(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<u32>> {
    if live.builder != BUILDER || cursor_nodes.is_empty() {
        return None;
    }
    cursor_nodes
        .iter()
        .enumerate()
        .all(|(index, node)| {
            live.rows.iter().any(|row| row.node == *node) && !cursor_nodes[..index].contains(node)
        })
        .then(|| cursor_nodes.to_vec())
}

fn target(state: &KindListPageState, index: u32) -> Option<KindListTarget> {
    state.target(index as usize)
}

pub fn dispatch(
    state: &mut KindListPageState,
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<KindListUpdate> {
    let count = state.row_count() as usize;
    match command {
        NativeCommand::Focus(index) => state.focus(target(state, index)?),
        NativeCommand::Select(index) => state.activate(target(state, index)?),
        NativeCommand::Accept => {
            let index = u32::try_from(focused_index?).ok()?;
            state.activate(target(state, index)?)
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
            state.focus(target(state, u32::try_from(next).ok()?)?)
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
    state: &mut KindListPageState,
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<KindListUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, focus, *command, wrap) {
            match update {
                KindListUpdate::Focused(row) | KindListUpdate::SelectionRequested(row) => {
                    focus = Some(row.ordinal() as usize);
                }
                KindListUpdate::BackRequested => {}
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
    use tsfp_mapmaker::kind_list::KindListGeneration;

    fn row(node: u32) -> LiveRow {
        LiveRow {
            node,
            node_type: 4,
            id: node as i32,
            flags: 0,
            text: LiveText::Id(node),
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
        let nodes = [11, 12];
        (
            LivePage {
                slot: 3,
                builder,
                state: 3,
                cursor_node: nodes[0],
                rows: nodes.iter().map(|node| row(*node)).collect(),
                units: (6.4, 4.8),
            },
            nodes.to_vec(),
        )
    }

    #[test]
    fn routes_dynamic_kind_rows_without_inventing_labels() {
        let (page, nodes) = live(BUILDER);
        assert_eq!(row_nodes(&page, &nodes), Some(nodes.clone()));
        assert_eq!(row_nodes(&page, &[12, 11]), Some(vec![12, 11]));
        assert_eq!(row_nodes(&page, &[11, 11]), None);
        let (other, nodes) = live(0x32_8D20);
        assert_eq!(row_nodes(&other, &nodes), None);

        let mut state = KindListPageState::new(KindListGeneration::new(1), 2);
        let updates = dispatch_commands(
            &mut state,
            None,
            &[NativeCommand::Step(Step::Down), NativeCommand::Accept],
            false,
        );
        assert_eq!(updates.len(), 2);
        assert!(matches!(updates[0], KindListUpdate::Focused(row) if row.ordinal() == 0));
        assert!(
            matches!(updates[1], KindListUpdate::SelectionRequested(row) if row.ordinal() == 0)
        );
    }
}
