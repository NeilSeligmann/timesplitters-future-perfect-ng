//! Native input adapter for the measured Map Maker Awards chooser rows.

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::awards::{AWARDS_CHOICES, AwardsChoice, AwardsPageState, AwardsUpdate};

pub const BUILDER: u32 = 0x32_2BB0;
pub const STATIC_LABELS: [(u32, AwardsChoice); 3] = [
    (0x0B40, AwardsChoice::StoryAwards),
    (0x0B41, AwardsChoice::AssaultAwards),
    (0x0B5E, AwardsChoice::Done),
];

/// Admit only T1714's measured row sequence. Dynamic or reordered layouts stay
/// on the existing live page path without receiving typed Rust actions.
pub fn exact_choices(live: &LivePage, cursor_nodes: &[u32]) -> Option<[AwardsChoice; 3]> {
    if live.builder != BUILDER || cursor_nodes.len() != STATIC_LABELS.len() {
        return None;
    }
    for (node, (expected_label, _)) in cursor_nodes.iter().zip(STATIC_LABELS) {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        if !matches!(&row.text, crate::live_page::LiveText::Id(label) if *label == expected_label) {
            return None;
        }
    }
    Some(AWARDS_CHOICES)
}

pub fn dispatch(
    state: &mut AwardsPageState,
    choices: &[AwardsChoice],
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<AwardsUpdate> {
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
    state: &mut AwardsPageState,
    choices: &[AwardsChoice],
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<AwardsUpdate> {
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
    use tsfp_mapmaker::awards::{AwardsGeneration, AwardsUpdate};

    fn live(labels: [u32; 3]) -> (LivePage, Vec<u32>) {
        let rows = labels
            .into_iter()
            .enumerate()
            .map(|(index, label)| LiveRow {
                node: index as u32 + 1,
                node_type: 0,
                id: 0,
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
                slot: 4,
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
    fn measured_awards_rows_admit_keyboard_and_pointer_requests() {
        let (page, nodes) = live([0x0B40, 0x0B41, 0x0B5E]);
        let choices = exact_choices(&page, &nodes).unwrap();
        let mut state = AwardsPageState::new(AwardsGeneration::new(3));
        assert_eq!(
            dispatch_commands(
                &mut state,
                &choices,
                Some(0),
                &[NativeCommand::Step(Step::Down), NativeCommand::Accept],
                false,
            ),
            [
                AwardsUpdate::Focused(AwardsChoice::AssaultAwards),
                AwardsUpdate::ChoiceRequested(AwardsChoice::AssaultAwards),
            ]
        );
        assert_eq!(
            dispatch_commands(
                &mut state,
                &choices,
                Some(2),
                &[NativeCommand::Focus(2), NativeCommand::Select(2)],
                false,
            ),
            [
                AwardsUpdate::Focused(AwardsChoice::Done),
                AwardsUpdate::ChoiceRequested(AwardsChoice::Done),
            ]
        );
    }

    #[test]
    fn malformed_or_reordered_awards_rows_are_rejected() {
        let (page, nodes) = live([0x0B40, 0x0B5E, 0x0B41]);
        assert!(exact_choices(&page, &nodes).is_none());
        let (page, nodes) = live([0x0B40, 0x0B41, 0x0B5E]);
        assert!(exact_choices(&page, &nodes[..2]).is_none());
        let mut wrong_page = page;
        wrong_page.builder = 0x31_3010;
        assert!(exact_choices(&wrong_page, &nodes).is_none());
    }
}
