// SPDX-License-Identifier: GPL-3.0-or-later
//! Caller-bound live rows for the Rust-owned Assault start-point picker.
//!
//! REA has not identified this child's builder, dynamic row producer, or
//! assignment callback. A caller that has those values supplies the builder
//! and explicit row-to-choice identities; this adapter preserves the live row
//! presentation and refuses incomplete, reordered, or stale bindings.

use tsfp_mapmaker::{
    assault_phase_assignment::AssaultPhaseAssignmentUpdate,
    assault_start_point_picker::{
        StartPointChoiceId, StartPointPickerCommand, StartPointPickerGeneration,
        StartPointPickerRow, StartPointPickerState, StartPointPickerTarget, StartPointPickerUpdate,
    },
    ui::MapMakerUiState,
};
use tsfp_ui_input::{
    live_page::{LivePage, LiveRow},
    mapmaker_assault_start_point_picker::dispatch_commands as dispatch_picker_commands,
    native_page::NativeCommand,
};

/// One live visual row paired with the Rust target for the same opaque choice.
/// The borrowed `LiveRow` retains runtime localization, geometry and styling.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedChoiceRow<'a> {
    live_row: &'a LiveRow,
    picker_row: StartPointPickerRow,
    target: StartPointPickerTarget,
}

impl<'a> ProjectedChoiceRow<'a> {
    pub const fn live_row(self) -> &'a LiveRow {
        self.live_row
    }

    pub const fn picker_row(self) -> StartPointPickerRow {
        self.picker_row
    }

    pub const fn target(self) -> StartPointPickerTarget {
        self.target
    }
}

/// Pair the current shared picker state with caller-identified live rows.
///
/// `choice_rows` is an explicit `(live node, opaque choice ID)` mapping in
/// renderer order. It may omit non-choice rows such as headings. The order of
/// its IDs must equal the picker state order; no guest index is treated as a
/// stable choice ID.
pub fn project_shared_rows<'a>(
    ui: &MapMakerUiState,
    live: &'a LivePage,
    expected_builder: u32,
    expected_slot: u32,
    cursor_nodes: &[u32],
    choice_rows: &[(u32, StartPointChoiceId)],
) -> Option<Vec<ProjectedChoiceRow<'a>>> {
    if live.builder != expected_builder || live.slot != expected_slot {
        return None;
    }

    let picker = ui.snapshot().assault_start_point_picker?;
    let choices = picker.choices();
    if choices.len() != choice_rows.len() {
        return None;
    }

    let mut previous_cursor_index = None;
    let mut projected = Vec::with_capacity(choice_rows.len());
    for (index, (node, choice)) in choice_rows.iter().copied().enumerate() {
        if choices.get(index) != Some(&choice)
            || choice_rows[..index]
                .iter()
                .any(|(prior_node, prior_choice)| *prior_node == node || *prior_choice == choice)
        {
            return None;
        }
        let cursor_index = cursor_nodes.iter().position(|cursor| *cursor == node)?;
        if cursor_nodes
            .iter()
            .filter(|cursor| **cursor == node)
            .count()
            != 1
            || previous_cursor_index.is_some_and(|previous| cursor_index <= previous)
        {
            return None;
        }
        previous_cursor_index = Some(cursor_index);

        if live.rows.iter().filter(|row| row.node == node).count() != 1 {
            return None;
        }
        let live_row = live.rows.iter().find(|row| row.node == node)?;
        let picker_row = picker.rows().nth(index)?;
        if picker_row.target().choice() != choice {
            return None;
        }
        projected.push(ProjectedChoiceRow {
            live_row,
            picker_row,
            target: picker.target(choice)?,
        });
    }
    Some(projected)
}

/// Project rows for the child page returned by `begin_child_picker_for_request`.
/// Keeping the successful open result as the identity source prevents a caller
/// from accidentally pairing the picker with a different builder or slot.
pub fn project_opened_child_rows<'a>(
    ui: &MapMakerUiState,
    live: &'a LivePage,
    opened: OpenedChildPicker,
    cursor_nodes: &[u32],
    choice_rows: &[(u32, StartPointChoiceId)],
) -> Option<Vec<ProjectedChoiceRow<'a>>> {
    if live.builder != opened.builder || live.slot != opened.slot {
        return None;
    }
    let picker = ui.snapshot().assault_start_point_picker?;
    if picker.generation() != opened.generation {
        return None;
    }
    project_shared_rows(
        ui,
        live,
        opened.builder,
        opened.slot,
        cursor_nodes,
        choice_rows,
    )
}

/// Resolve a focused live cursor node to its position in the picker list.
/// Non-choice rows have no picker index and cannot activate an assignment.
pub fn choice_index_for_node(
    cursor_nodes: &[u32],
    choice_rows: &[(u32, StartPointChoiceId)],
    node: u32,
) -> Option<usize> {
    let cursor_index = cursor_nodes
        .iter()
        .position(|candidate| *candidate == node)?;
    if cursor_nodes
        .iter()
        .filter(|candidate| **candidate == node)
        .count()
        != 1
    {
        return None;
    }
    let choice_index = choice_rows
        .iter()
        .position(|(choice_node, _)| *choice_node == node)?;
    if choice_rows
        .iter()
        .filter(|(choice_node, _)| *choice_node == node)
        .count()
        != 1
        || choice_rows[..choice_index]
            .iter()
            .any(|(_, choice)| *choice == choice_rows[choice_index].1)
    {
        return None;
    }
    Some(choice_index)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildPickerStartError {
    ParentNotRequestingSelection,
    PickerAlreadyOpen,
    ZeroGeneration,
    DuplicateChoiceIdentity,
}

/// Runtime values required to attach a parent request to the child picker.
/// The REA dossier has not identified the child's page identity or choice
/// source, so callers must supply these values from their live page/document
/// projection instead of relying on a guessed builder or guest offset.
#[derive(Clone, Copy, Debug)]
pub struct ChildPickerBinding<'a> {
    pub builder: u32,
    pub slot: u32,
    pub page_generation: u64,
    pub choices: &'a [StartPointChoiceId],
    pub wrap: bool,
}

/// Page identity retained by the caller until the child page is observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenedChildPicker {
    pub builder: u32,
    pub slot: u32,
    pub page_generation: u64,
    pub generation: StartPointPickerGeneration,
}

/// Start the child only for the parent's typed selection request.
///
/// This is the orchestration seam between the Rust-owned parent reducer and
/// its caller-supplied live child binding. The binding remains available to
/// `project_shared_rows` for renderer-order validation; no row identity is
/// inferred here.
pub fn begin_child_picker_for_request(
    ui: &mut MapMakerUiState,
    update: AssaultPhaseAssignmentUpdate,
    binding: ChildPickerBinding<'_>,
) -> Result<OpenedChildPicker, ChildPickerStartError> {
    if update != AssaultPhaseAssignmentUpdate::StartPointSelectionRequested {
        return Err(ChildPickerStartError::ParentNotRequestingSelection);
    }
    if binding.page_generation == 0 {
        return Err(ChildPickerStartError::ZeroGeneration);
    }
    begin_child_picker(
        ui,
        StartPointPickerGeneration::new(binding.page_generation),
        binding.choices,
        binding.wrap,
    )?;
    Ok(OpenedChildPicker {
        builder: binding.builder,
        slot: binding.slot,
        page_generation: binding.page_generation,
        generation: StartPointPickerGeneration::new(binding.page_generation),
    })
}

/// Open the Rust-owned picker after its parent has emitted a selection request.
///
/// The caller supplies identities from its live document projection. The
/// generation is a fresh native page lifetime token; guest node addresses are
/// never used as durable choice identities.
pub fn begin_child_picker(
    ui: &mut MapMakerUiState,
    generation: StartPointPickerGeneration,
    choices: &[StartPointChoiceId],
    wrap: bool,
) -> Result<(), ChildPickerStartError> {
    if generation.get() == 0 {
        return Err(ChildPickerStartError::ZeroGeneration);
    }
    if ui.assault_start_point_picker().is_some() {
        return Err(ChildPickerStartError::PickerAlreadyOpen);
    }
    if !ui
        .assault_phase_assignment()
        .is_some_and(|parent| parent.awaiting_start_point_selection())
    {
        return Err(ChildPickerStartError::ParentNotRequestingSelection);
    }
    let Some(picker) = StartPointPickerState::new(generation, choices, wrap) else {
        return Err(ChildPickerStartError::DuplicateChoiceIdentity);
    };
    ui.set_assault_start_point_picker(Some(picker));
    Ok(())
}

/// Apply the caller's generation-tagged selection request to the Rust-owned
/// parent page. The original document-record lookup/write remains outside.
pub fn complete_child_picker(
    ui: &mut MapMakerUiState,
    generation: StartPointPickerGeneration,
    choice: StartPointChoiceId,
) -> AssaultPhaseAssignmentUpdate {
    ui.complete_assault_start_point_selection(generation, choice)
}

/// Cancel only the matching child-page lifetime; prior parent selection stays.
pub fn cancel_child_picker(
    ui: &mut MapMakerUiState,
    generation: StartPointPickerGeneration,
) -> AssaultPhaseAssignmentUpdate {
    ui.cancel_assault_start_point_selection(generation)
}

/// Dispatch native keyboard/controller commands against the current rendered
/// rows. The shared Rust input reducer owns keyboard/controller semantics;
/// dynamic pointer focus and activation use generation-qualified targets.
pub fn dispatch_native_commands(
    ui: &mut MapMakerUiState,
    live: &LivePage,
    expected_builder: u32,
    expected_slot: u32,
    cursor_nodes: &[u32],
    choice_rows: &[(u32, StartPointChoiceId)],
    commands: &[NativeCommand],
) -> Vec<StartPointPickerUpdate> {
    if project_shared_rows(
        ui,
        live,
        expected_builder,
        expected_slot,
        cursor_nodes,
        choice_rows,
    )
    .is_none()
    {
        return Vec::new();
    }

    ui.with_assault_start_point_picker_mut(|picker| {
        let focused_index = picker.focused().and_then(|choice| {
            picker
                .choices()
                .iter()
                .position(|candidate| *candidate == choice)
        });
        dispatch_picker_commands(picker, focused_index, commands)
    })
    .unwrap_or_default()
}

/// Dispatch a pointer target resolved from the currently rendered projection.
/// The target keeps its page generation, so clicks retained across a reopen
/// cannot focus or activate a choice in the new picker.
pub fn dispatch_pointer_target(
    ui: &mut MapMakerUiState,
    target: StartPointPickerTarget,
    activate: bool,
) -> StartPointPickerUpdate {
    ui.with_assault_start_point_picker_mut(|picker| {
        picker.dispatch(if activate {
            StartPointPickerCommand::Activate(target)
        } else {
            StartPointPickerCommand::Focus(target)
        })
    })
    .unwrap_or(StartPointPickerUpdate::Ignored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_mapmaker::assault_phase_assignment::{
        AssaultPhaseAssignmentAction, AssaultPhaseAssignmentCommand,
        AssaultPhaseAssignmentGeneration, AssaultPhaseAssignmentState,
    };
    use tsfp_mapmaker::assault_start_point_picker::{
        StartPointPickerGeneration, StartPointPickerState, StartPointPickerUpdate,
    };
    use tsfp_ui_input::{live_page::LiveText, native_page::NativeCommand, widgets::Step};

    const BUILDER: u32 = 0x91_0042; // Synthetic fixture; REA has not named the child builder.

    fn live_row(node: u32, label: u32) -> LiveRow {
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

    fn setup() -> (
        MapMakerUiState,
        LivePage,
        [u32; 3],
        [(u32, StartPointChoiceId); 2],
    ) {
        let choices = [
            StartPointChoiceId::new(0xabc),
            StartPointChoiceId::new(0xdef),
        ];
        let mut ui = MapMakerUiState::default();
        ui.set_assault_start_point_picker(Some(
            StartPointPickerState::new(StartPointPickerGeneration::new(7), &choices, false)
                .unwrap(),
        ));
        let live = LivePage {
            slot: 4,
            builder: BUILDER,
            state: 0,
            cursor_node: 1,
            rows: vec![
                live_row(11, 0x100),
                live_row(12, 0x101),
                live_row(13, 0x102),
            ],
            units: (6.4, 4.8),
        };
        (ui, live, [11, 12, 13], [(11, choices[0]), (13, choices[1])])
    }

    #[test]
    fn projects_explicit_dynamic_rows_and_preserves_runtime_labels() {
        let (ui, live, cursor_nodes, choices) = setup();
        let rows = project_shared_rows(&ui, &live, BUILDER, 4, &cursor_nodes, &choices).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].live_row().text, LiveText::Id(0x100));
        assert_eq!(rows[1].live_row().text, LiveText::Id(0x102));
        assert_eq!(rows[0].target().choice(), choices[0].1);
        assert_eq!(rows[1].target().choice(), choices[1].1);
        assert_eq!(choice_index_for_node(&cursor_nodes, &choices, 12), None);
        assert_eq!(choice_index_for_node(&cursor_nodes, &choices, 13), Some(1));
    }

    #[test]
    fn rejects_wrong_page_stale_order_and_duplicate_node_bindings() {
        let (ui, live, cursor_nodes, choices) = setup();
        assert!(project_shared_rows(&ui, &live, BUILDER + 1, 4, &cursor_nodes, &choices).is_none());
        assert!(project_shared_rows(&ui, &live, BUILDER, 5, &cursor_nodes, &choices).is_none());

        let reordered = [(13, choices[1].1), (11, choices[0].1)];
        assert!(project_shared_rows(&ui, &live, BUILDER, 4, &cursor_nodes, &reordered).is_none());
        let duplicate = [(11, choices[0].1), (11, choices[1].1)];
        assert!(project_shared_rows(&ui, &live, BUILDER, 4, &cursor_nodes, &duplicate).is_none());

        let duplicate_cursor = [11, 11, 13];
        assert!(project_shared_rows(&ui, &live, BUILDER, 4, &duplicate_cursor, &choices).is_none());
        assert_eq!(choice_index_for_node(&duplicate_cursor, &choices, 11), None);

        let mut duplicate_live = live.clone();
        duplicate_live.rows.push(live_row(11, 0x103));
        assert!(
            project_shared_rows(&ui, &duplicate_live, BUILDER, 4, &cursor_nodes, &choices)
                .is_none()
        );
    }

    #[test]
    fn native_commands_navigate_and_select_the_bound_choice() {
        let (mut ui, live, cursor_nodes, choices) = setup();
        let updates = dispatch_native_commands(
            &mut ui,
            &live,
            BUILDER,
            4,
            &cursor_nodes,
            &choices,
            &[
                NativeCommand::Step(Step::Down),
                NativeCommand::Step(Step::Down),
                NativeCommand::Accept,
            ],
        );
        assert_eq!(
            updates,
            [
                StartPointPickerUpdate::Focused(choices[0].1),
                StartPointPickerUpdate::Focused(choices[1].1),
                StartPointPickerUpdate::SelectionRequested(choices[1].1),
            ]
        );

        let updates = dispatch_native_commands(
            &mut ui,
            &live,
            BUILDER,
            4,
            &cursor_nodes,
            &choices,
            &[NativeCommand::Focus(0), NativeCommand::Select(2)],
        );
        assert!(updates.is_empty());

        let target = project_shared_rows(&ui, &live, BUILDER, 4, &cursor_nodes, &choices).unwrap()
            [0]
        .target();
        assert_eq!(
            dispatch_pointer_target(&mut ui, target, true),
            StartPointPickerUpdate::SelectionRequested(choices[0].1)
        );
    }

    #[test]
    fn keyboard_selection_completes_the_rust_owned_parent_handoff() {
        let (mut ui, live, cursor_nodes, choices) = setup();
        ui.set_assault_start_point_picker(None);
        request_child_picker(&mut ui);

        let generation = StartPointPickerGeneration::new(7);
        let choice_ids = choices.map(|(_, choice)| choice);
        begin_child_picker(&mut ui, generation, &choice_ids, false).unwrap();
        let updates = dispatch_native_commands(
            &mut ui,
            &live,
            BUILDER,
            4,
            &cursor_nodes,
            &choices,
            &[
                NativeCommand::Step(Step::Down),
                NativeCommand::Step(Step::Down),
                NativeCommand::Accept,
            ],
        );
        assert_eq!(
            updates.last(),
            Some(&StartPointPickerUpdate::SelectionRequested(choices[1].1))
        );
        assert_eq!(
            complete_child_picker(&mut ui, generation, choices[1].1),
            AssaultPhaseAssignmentUpdate::StartPointAssigned(choices[1].1)
        );

        ui.observe_assault_phase_page_change(true);
        let parent = ui.assault_phase_assignment().unwrap();
        assert_eq!(parent.selected_start_point(), Some(choices[1].1));
        assert!(!parent.awaiting_start_point_selection());
        assert!(ui.assault_start_point_picker().is_none());
    }

    #[test]
    fn native_dispatch_rejects_unprojected_page_and_pointer_rejects_stale_generation() {
        let (mut ui, live, cursor_nodes, choices) = setup();
        assert!(
            dispatch_native_commands(
                &mut ui,
                &live,
                BUILDER + 1,
                4,
                &cursor_nodes,
                &choices,
                &[NativeCommand::Accept],
            )
            .is_empty()
        );

        let target = project_shared_rows(&ui, &live, BUILDER, 4, &cursor_nodes, &choices).unwrap()
            [0]
        .target();
        ui.set_assault_start_point_picker(Some(
            StartPointPickerState::new(
                StartPointPickerGeneration::new(8),
                &[choices[0].1, choices[1].1],
                false,
            )
            .unwrap(),
        ));
        assert_eq!(
            dispatch_pointer_target(&mut ui, target, true),
            StartPointPickerUpdate::Ignored
        );
    }

    fn request_child_picker(ui: &mut MapMakerUiState) -> AssaultPhaseAssignmentUpdate {
        let mut parent = ui.assault_phase_assignment().unwrap_or_else(|| {
            AssaultPhaseAssignmentState::new(AssaultPhaseAssignmentGeneration::new(3), false)
        });
        let update = parent.dispatch(AssaultPhaseAssignmentCommand::Activate(
            parent.target(AssaultPhaseAssignmentAction::SelectStartPoint),
        ));
        assert_eq!(
            update,
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );
        ui.set_assault_phase_assignment(Some(parent));
        update
    }

    #[test]
    fn caller_binding_opens_only_for_parent_request_and_returns_page_identity() {
        let mut ui = MapMakerUiState::default();
        let choices = [
            StartPointChoiceId::new(0xabc),
            StartPointChoiceId::new(0xdef),
        ];
        let binding = ChildPickerBinding {
            builder: BUILDER,
            slot: 4,
            page_generation: 7,
            choices: &choices,
            wrap: true,
        };
        assert_eq!(
            begin_child_picker_for_request(&mut ui, AssaultPhaseAssignmentUpdate::Ignored, binding,),
            Err(ChildPickerStartError::ParentNotRequestingSelection)
        );
        let request = request_child_picker(&mut ui);
        assert_eq!(
            begin_child_picker_for_request(&mut ui, request, binding),
            Ok(OpenedChildPicker {
                builder: BUILDER,
                slot: 4,
                page_generation: 7,
                generation: StartPointPickerGeneration::new(7),
            })
        );
        assert_eq!(
            ui.assault_start_point_picker()
                .map(|picker| picker.choices()),
            Some(choices.as_slice())
        );
    }

    #[test]
    fn opened_child_identity_qualifies_shared_row_projection() {
        let (ui, live, cursor_nodes, choices) = setup();
        let opened = OpenedChildPicker {
            builder: BUILDER,
            slot: 4,
            page_generation: 7,
            generation: StartPointPickerGeneration::new(7),
        };
        assert_eq!(
            project_opened_child_rows(&ui, &live, opened, &cursor_nodes, &choices)
                .map(|rows| rows.len()),
            Some(2)
        );

        let stale = OpenedChildPicker {
            generation: StartPointPickerGeneration::new(8),
            ..opened
        };
        assert!(project_opened_child_rows(&ui, &live, stale, &cursor_nodes, &choices).is_none());
        let wrong_page = LivePage {
            builder: BUILDER + 1,
            ..live.clone()
        };
        assert!(
            project_opened_child_rows(&ui, &wrong_page, opened, &cursor_nodes, &choices).is_none()
        );
    }

    #[test]
    fn child_picker_lifecycle_requires_parent_request_and_fresh_unique_choices() {
        let first = StartPointChoiceId::new(0xabc);
        let mut ui = MapMakerUiState::default();
        let generation = StartPointPickerGeneration::new(7);
        assert_eq!(
            begin_child_picker(&mut ui, generation, &[first], false),
            Err(ChildPickerStartError::ParentNotRequestingSelection)
        );

        request_child_picker(&mut ui);
        assert_eq!(
            begin_child_picker(&mut ui, StartPointPickerGeneration::new(0), &[first], false),
            Err(ChildPickerStartError::ZeroGeneration)
        );
        assert_eq!(
            begin_child_picker(&mut ui, generation, &[first, first], false),
            Err(ChildPickerStartError::DuplicateChoiceIdentity)
        );
        assert_eq!(
            begin_child_picker(&mut ui, generation, &[first], false),
            Ok(())
        );
        assert_eq!(
            begin_child_picker(&mut ui, StartPointPickerGeneration::new(8), &[first], false),
            Err(ChildPickerStartError::PickerAlreadyOpen)
        );
    }

    #[test]
    fn child_picker_completion_and_cancel_are_generation_scoped() {
        let first = StartPointChoiceId::new(0xabc);
        let second = StartPointChoiceId::new(0xdef);
        let mut ui = MapMakerUiState::default();
        request_child_picker(&mut ui);
        let first_generation = StartPointPickerGeneration::new(7);
        begin_child_picker(&mut ui, first_generation, &[first, second], false).unwrap();

        assert_eq!(
            complete_child_picker(&mut ui, StartPointPickerGeneration::new(6), first),
            AssaultPhaseAssignmentUpdate::Ignored
        );
        assert!(ui.assault_start_point_picker().is_some());
        assert_eq!(
            complete_child_picker(&mut ui, first_generation, second),
            AssaultPhaseAssignmentUpdate::StartPointAssigned(second)
        );
        assert!(ui.assault_start_point_picker().is_none());
        assert_eq!(
            ui.assault_phase_assignment()
                .unwrap()
                .selected_start_point(),
            Some(second)
        );

        request_child_picker(&mut ui);
        let second_generation = StartPointPickerGeneration::new(8);
        begin_child_picker(&mut ui, second_generation, &[first], false).unwrap();
        assert_eq!(
            cancel_child_picker(&mut ui, first_generation),
            AssaultPhaseAssignmentUpdate::Ignored
        );
        assert!(ui.assault_start_point_picker().is_some());
        assert_eq!(
            cancel_child_picker(&mut ui, second_generation),
            AssaultPhaseAssignmentUpdate::StartPointSelectionCancelled
        );
        assert!(ui.assault_start_point_picker().is_none());
        assert_eq!(
            ui.assault_phase_assignment()
                .unwrap()
                .selected_start_point(),
            Some(second)
        );
    }
}
