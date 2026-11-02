//! Native input adapter for T1714's kind-specific Map Maker prompt.

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::kind_picker::{KindPickerChoice, KindPickerPageState, KindPickerUpdate};

pub const BUILDER: u32 = 0x32_8850;
pub const SELECT_OBJECT_LABEL: u32 = 0x0B7C;
pub const SELECT_PICKUP_LABEL: u32 = 0x0B7D;
pub const SELECT_DOOR_LABEL: u32 = 0x0B7E;

/// Admit only the one cursor row whose localized identity T1714 measures.
/// The page's dynamic title and the effects of the existing callback remain
/// caller supplied and are not inferred from this prompt label.
pub fn exact_choice(live: &LivePage, cursor_nodes: &[u32]) -> Option<KindPickerChoice> {
    if live.builder != BUILDER || cursor_nodes.len() != 1 {
        return None;
    }
    let node = cursor_nodes[0];
    let row = live.rows.iter().find(|row| row.node == node)?;
    match &row.text {
        crate::live_page::LiveText::Id(SELECT_OBJECT_LABEL) => {
            Some(KindPickerChoice::SelectObjectFromMap)
        }
        crate::live_page::LiveText::Id(SELECT_PICKUP_LABEL) => {
            Some(KindPickerChoice::SelectPickupFromMap)
        }
        crate::live_page::LiveText::Id(SELECT_DOOR_LABEL) => {
            Some(KindPickerChoice::SelectDoorFromMap)
        }
        _ => None,
    }
}

pub fn dispatch(
    state: &mut KindPickerPageState,
    focused_index: Option<usize>,
    command: NativeCommand,
) -> Option<KindPickerUpdate> {
    let target = state.target();
    match command {
        NativeCommand::Focus(0) => state.focus(target),
        NativeCommand::Select(0) => state.activate(target),
        NativeCommand::Accept if focused_index == Some(0) => state.activate(target),
        NativeCommand::Back => Some(state.back()),
        NativeCommand::Step(Step::Up | Step::Down) if focused_index == Some(0) => {
            state.focus(target)
        }
        NativeCommand::Focus(_)
        | NativeCommand::Select(_)
        | NativeCommand::Accept
        | NativeCommand::Step(_)
        | NativeCommand::Adjust { .. }
        | NativeCommand::Scroll(_)
        | NativeCommand::Delete
        | NativeCommand::Shortcut
        | NativeCommand::Join(_)
        | NativeCommand::ResumeGame => None,
    }
}

pub fn dispatch_commands(
    state: &mut KindPickerPageState,
    focused_index: Option<usize>,
    commands: &[NativeCommand],
) -> Vec<KindPickerUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        if let Some(update) = dispatch(state, focus, *command) {
            updates.push(update);
        }
        if state.is_focused() {
            focus = Some(0);
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_page::{LiveRow, LiveText};
    use tsfp_mapmaker::kind_picker::{KindPickerGeneration, KindPickerUpdate};

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

    fn live(label: u32) -> (LivePage, Vec<u32>) {
        (
            LivePage {
                slot: 4,
                builder: BUILDER,
                state: 2,
                cursor_node: 20,
                rows: vec![row(20, label)],
                units: (6.4, 4.8),
            },
            vec![20],
        )
    }

    #[test]
    fn admits_each_measured_prompt_and_rejects_unmeasured_shapes() {
        for (label, choice) in [
            (SELECT_OBJECT_LABEL, KindPickerChoice::SelectObjectFromMap),
            (SELECT_PICKUP_LABEL, KindPickerChoice::SelectPickupFromMap),
            (SELECT_DOOR_LABEL, KindPickerChoice::SelectDoorFromMap),
        ] {
            let (page, nodes) = live(label);
            assert_eq!(exact_choice(&page, &nodes), Some(choice));
        }
        let (mut page, nodes) = live(SELECT_OBJECT_LABEL);
        page.builder += 1;
        assert_eq!(exact_choice(&page, &nodes), None);
        page.builder = BUILDER;
        page.rows[0].text = LiveText::Id(0xFFFF);
        assert_eq!(exact_choice(&page, &nodes), None);
        assert_eq!(exact_choice(&page, &[20, 20]), None);
    }

    #[test]
    fn routes_keyboard_pointer_and_back_as_typed_updates() {
        let mut state = KindPickerPageState::new(
            KindPickerGeneration::new(3),
            KindPickerChoice::SelectObjectFromMap,
        );
        assert_eq!(
            dispatch_commands(
                &mut state,
                None,
                &[NativeCommand::Focus(0), NativeCommand::Accept],
            ),
            [
                KindPickerUpdate::Focused(KindPickerChoice::SelectObjectFromMap),
                KindPickerUpdate::SelectionRequested(KindPickerChoice::SelectObjectFromMap)
            ]
        );
        assert_eq!(
            dispatch(&mut state, Some(0), NativeCommand::Step(Step::Down)),
            Some(KindPickerUpdate::Focused(
                KindPickerChoice::SelectObjectFromMap
            ))
        );
        assert_eq!(
            dispatch(&mut state, None, NativeCommand::Back),
            Some(KindPickerUpdate::BackRequested)
        );
    }
}
