//! Native input adapter for T1714's measured Award Flags row labels.

use crate::{
    live_page::{LivePage, LiveText},
    native_page::NativeCommand,
    widgets::Step,
};
use tsfp_mapmaker::award_flags::{AwardFlagAction, AwardFlagsPageState, AwardFlagsUpdate};

pub const BUILDER: u32 = 0x31_FDC0;
pub const ROW_LABELS: [[u32; 2]; 4] = [
    [0x0B55, 0x0B59],
    [0x0B56, 0x0B5A],
    [0x0B57, 0x0B5B],
    [0x0B58, 0x0B5C],
];

/// Match the four rows in T1714's rank order and retain each measured label
/// variant as typed state. Unknown labels or row permutations fail closed.
pub fn measured_actions(live: &LivePage, cursor_nodes: &[u32]) -> Option<[AwardFlagAction; 4]> {
    if live.builder != BUILDER || cursor_nodes.len() != ROW_LABELS.len() {
        return None;
    }
    let mut actions = [AwardFlagAction::Add; 4];
    for (index, (node, labels)) in cursor_nodes.iter().zip(ROW_LABELS).enumerate() {
        let mut matches = live.rows.iter().filter(|row| row.node == *node);
        let row = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        actions[index] = match &row.text {
            LiveText::Id(id) if *id == labels[0] => AwardFlagAction::ViewOrEdit,
            LiveText::Id(id) if *id == labels[1] => AwardFlagAction::Add,
            _ => return None,
        };
    }
    Some(actions)
}

pub fn dispatch(
    state: &mut AwardFlagsPageState,
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<AwardFlagsUpdate> {
    match command {
        NativeCommand::Focus(index) => {
            let target = target_at(state, index as usize)?;
            state.focus(target)
        }
        NativeCommand::Select(index) => {
            let target = target_at(state, index as usize)?;
            state.activate(target)
        }
        NativeCommand::Accept => {
            let target = target_at(state, focused_index?)?;
            state.activate(target)
        }
        NativeCommand::Back => Some(state.back()),
        NativeCommand::Step(Step::Up | Step::Down) => {
            let next = step_target(focused_index, command, wrap)?;
            let target = target_at(state, next)?;
            state.focus(target)
        }
        NativeCommand::Step(Step::Left | Step::Right)
        | NativeCommand::Delete
        | NativeCommand::Shortcut
        | NativeCommand::Join(_)
        | NativeCommand::ResumeGame
        | NativeCommand::Scroll(_)
        | NativeCommand::Adjust { .. } => None,
    }
}

fn target_at(
    state: &AwardFlagsPageState,
    index: usize,
) -> Option<tsfp_mapmaker::award_flags::AwardFlagsTarget> {
    let rank = tsfp_mapmaker::award_flags::AwardRank::at(index)?;
    Some(state.target(rank))
}

fn step_target(index: Option<usize>, command: NativeCommand, wrap: bool) -> Option<usize> {
    let down = matches!(command, NativeCommand::Step(Step::Down));
    match (index, down, wrap) {
        (Some(index), true, _) if index < 3 => Some(index + 1),
        (Some(index), false, _) if index > 0 => Some(index - 1),
        (Some(_), true, true) => Some(0),
        (Some(_), false, true) => Some(3),
        (None, true, _) => Some(0),
        (None, false, _) => Some(3),
        (Some(_), _, _) => None,
    }
}

pub fn dispatch_commands(
    state: &mut AwardFlagsPageState,
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<AwardFlagsUpdate> {
    let mut focused = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, focused, *command, wrap) {
            focused = match update {
                AwardFlagsUpdate::Focused(rank)
                | AwardFlagsUpdate::SelectionRequested { rank, .. } => {
                    tsfp_mapmaker::award_flags::AwardRank::ALL
                        .iter()
                        .position(|candidate| *candidate == rank)
                }
                AwardFlagsUpdate::BackRequested => focused,
            };
            updates.push(update);
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_page::LiveRow;
    use tsfp_mapmaker::award_flags::{AwardFlagsGeneration, AwardRank};

    fn page(labels: &[u32]) -> (LivePage, Vec<u32>) {
        let nodes = [10, 11, 12, 13];
        let rows = nodes
            .iter()
            .zip(labels)
            .map(|(node, label)| LiveRow {
                node: *node,
                node_type: 4,
                id: *label as i32,
                flags: 0,
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
                brightness: 0.0,
                height: 0.0,
                text_align: 0,
                text_x: 0.0,
                text_y: 0.0,
            })
            .collect();
        (
            LivePage {
                slot: 3,
                builder: BUILDER,
                state: 1,
                cursor_node: nodes[0],
                rows,
                units: (6.4, 4.8),
            },
            nodes.to_vec(),
        )
    }

    #[test]
    fn admits_only_the_measured_four_rank_order_and_label_variants() {
        let (live, nodes) = page(&[0x0B55, 0x0B5A, 0x0B57, 0x0B5C]);
        assert_eq!(
            measured_actions(&live, &nodes),
            Some([
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
            ])
        );
        let (bad_order, nodes) = page(&[0x0B56, 0x0B59, 0x0B57, 0x0B5C]);
        assert!(measured_actions(&bad_order, &nodes).is_none());
        let (wrong_builder, nodes) = page(&[0x0B55, 0x0B5A, 0x0B57, 0x0B5C]);
        let mut wrong_builder = wrong_builder;
        wrong_builder.builder = 0x32_78D0;
        assert!(measured_actions(&wrong_builder, &nodes).is_none());
    }

    #[test]
    fn keyboard_and_pointer_commands_emit_typed_rank_requests() {
        let mut state = AwardFlagsPageState::new(
            AwardFlagsGeneration::new(3),
            [
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
            ],
        );
        let updates = dispatch_commands(
            &mut state,
            None,
            &[
                NativeCommand::Step(Step::Down),
                NativeCommand::Accept,
                NativeCommand::Step(Step::Down),
                NativeCommand::Accept,
                NativeCommand::Select(2),
                NativeCommand::Back,
            ],
            false,
        );
        assert_eq!(updates.len(), 6);
        assert!(matches!(
            updates[1],
            AwardFlagsUpdate::SelectionRequested {
                rank: AwardRank::Platinum,
                action: AwardFlagAction::ViewOrEdit
            }
        ));
        assert!(matches!(
            updates[3],
            AwardFlagsUpdate::SelectionRequested {
                rank: AwardRank::Gold,
                action: AwardFlagAction::Add
            }
        ));
        assert!(matches!(
            updates[4],
            AwardFlagsUpdate::SelectionRequested {
                rank: AwardRank::Silver,
                action: AwardFlagAction::ViewOrEdit
            }
        ));
        assert_eq!(updates[5], AwardFlagsUpdate::BackRequested);
    }

    #[test]
    fn invalid_focus_does_not_poison_later_batch_commands() {
        let mut state = AwardFlagsPageState::new(
            AwardFlagsGeneration::new(4),
            [
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
                AwardFlagAction::ViewOrEdit,
                AwardFlagAction::Add,
            ],
        );
        let updates = dispatch_commands(
            &mut state,
            Some(1),
            &[NativeCommand::Focus(u32::MAX), NativeCommand::Accept],
            false,
        );
        assert_eq!(
            updates,
            [AwardFlagsUpdate::SelectionRequested {
                rank: AwardRank::Gold,
                action: AwardFlagAction::Add,
            }]
        );
        assert_eq!(state.focused(), Some(AwardRank::Gold));
    }
}
