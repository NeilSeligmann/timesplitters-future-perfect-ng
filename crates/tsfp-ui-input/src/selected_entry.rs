//! Native input adapter for one T1714 selected-entry label pair.

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::selected_entry::{
    SelectedEntryField, SelectedEntryPageState, SelectedEntryUpdate,
};

pub const BUILDER: u32 = 0x32_8D20;
pub const MESSAGE_NAME_LABEL: u32 = 0x0A2D;
pub const DISPLAY_MESSAGE_LABEL: u32 = 0x0A2E;
const FIELDS: [SelectedEntryField; 2] = [
    SelectedEntryField::MessageName,
    SelectedEntryField::DisplayMessage,
];

/// T1714 records these labels at 0x328F60 and 0x328FA9. Their co-occurrence
/// and relationship are INFERRED; admit only this exact ordered runtime pair.
pub fn exact_fields(live: &LivePage, cursor_nodes: &[u32]) -> Option<[SelectedEntryField; 2]> {
    if live.builder != BUILDER || cursor_nodes.len() != FIELDS.len() {
        return None;
    }
    for (node, expected) in cursor_nodes.iter().zip(FIELDS) {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let crate::live_page::LiveText::Id(label) = &row.text else {
            return None;
        };
        let accepted = match expected {
            SelectedEntryField::MessageName => MESSAGE_NAME_LABEL,
            SelectedEntryField::DisplayMessage => DISPLAY_MESSAGE_LABEL,
        };
        if *label != accepted {
            return None;
        }
    }
    Some(FIELDS)
}

fn field_at(index: usize) -> Option<SelectedEntryField> {
    FIELDS.get(index).copied()
}

pub fn dispatch(
    state: &mut SelectedEntryPageState,
    focused_index: Option<usize>,
    command: NativeCommand,
) -> Option<SelectedEntryUpdate> {
    match command {
        NativeCommand::Focus(index) => state.focus(state.target(field_at(index as usize)?)),
        NativeCommand::Select(index) => state.activate(state.target(field_at(index as usize)?)),
        NativeCommand::Accept => {
            let field = field_at(focused_index?)?;
            state.activate(state.target(field))
        }
        NativeCommand::Back => Some(state.back()),
        NativeCommand::Step(step @ (Step::Up | Step::Down)) => {
            let index = focused_index?;
            let next = match step {
                Step::Up => index.checked_sub(1),
                Step::Down => index.checked_add(1),
                Step::Left | Step::Right => None,
            }?;
            let field = field_at(next)?;
            state.focus(state.target(field))
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
    state: &mut SelectedEntryPageState,
    focused_index: Option<usize>,
    commands: &[NativeCommand],
) -> Vec<SelectedEntryUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, focus, *command) {
            if let SelectedEntryUpdate::Focused(field)
            | SelectedEntryUpdate::ActivationRequested(field) = update
            {
                focus = FIELDS.iter().position(|candidate| *candidate == field);
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
    use tsfp_mapmaker::selected_entry::SelectedEntryGeneration;

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

    fn live(labels: [u32; 2]) -> (LivePage, Vec<u32>) {
        (
            LivePage {
                slot: 3,
                builder: BUILDER,
                state: 2,
                cursor_node: 10,
                rows: vec![row(10, labels[0]), row(11, labels[1])],
                units: (6.4, 4.8),
            },
            vec![10, 11],
        )
    }

    #[test]
    fn admits_only_the_inferred_ordered_label_pair() {
        let (page, nodes) = live([MESSAGE_NAME_LABEL, DISPLAY_MESSAGE_LABEL]);
        assert_eq!(exact_fields(&page, &nodes), Some(FIELDS));
        assert_eq!(exact_fields(&page, &[10]), None);
        let (reordered, nodes) = live([DISPLAY_MESSAGE_LABEL, MESSAGE_NAME_LABEL]);
        assert_eq!(exact_fields(&reordered, &nodes), None);
    }

    #[test]
    fn routes_focus_activate_back_and_keyboard_accept_as_typed_updates() {
        let mut state = SelectedEntryPageState::new(SelectedEntryGeneration::new(9), FIELDS);
        assert_eq!(
            dispatch_commands(
                &mut state,
                None,
                &[NativeCommand::Focus(1), NativeCommand::Accept],
            ),
            [
                SelectedEntryUpdate::Focused(SelectedEntryField::DisplayMessage),
                SelectedEntryUpdate::ActivationRequested(SelectedEntryField::DisplayMessage),
            ]
        );
        assert_eq!(
            dispatch(&mut state, Some(1), NativeCommand::Step(Step::Up)),
            Some(SelectedEntryUpdate::Focused(
                SelectedEntryField::MessageName
            ))
        );
        assert_eq!(
            dispatch(&mut state, None, NativeCommand::Back),
            Some(SelectedEntryUpdate::BackRequested)
        );
    }
}
