//! Native keyboard, controller, and pointer input for the dynamic Assault
//! phase start-point child list (T2408).

use std::vec::Vec;

use crate::{
    input::IntentSet, native_page::NativeCommand, owned_catalogue::CatalogueAction, widgets::Step,
};
use tsfp_mapmaker::assault_start_point_picker::{
    StartPointPickerCommand, StartPointPickerGeneration, StartPointPickerState,
    StartPointPickerTarget, StartPointPickerUpdate,
};
use tsfp_mapmaker::ui::MapMakerUiState;

/// Pointer events carry the target resolved from the rendered row. A stale
/// hit therefore cannot select a different row after the dynamic list changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Focus(StartPointPickerTarget),
    Activate(StartPointPickerTarget),
    Back(StartPointPickerGeneration),
}

pub fn dispatch_pointer(
    state: &mut StartPointPickerState,
    event: PointerEvent,
) -> StartPointPickerUpdate {
    match event {
        PointerEvent::Focus(target) => state.dispatch(StartPointPickerCommand::Focus(target)),
        PointerEvent::Activate(target) => state.dispatch(StartPointPickerCommand::Activate(target)),
        PointerEvent::Back(generation) => state.dispatch(StartPointPickerCommand::Back(generation)),
    }
}

pub fn dispatch(
    state: &mut StartPointPickerState,
    focused_index: Option<usize>,
    command: NativeCommand,
) -> StartPointPickerUpdate {
    let generation = state.generation();
    match command {
        // These commands carry only a list position. Dynamic pointer hits use
        // dispatch_pointer and a generation-qualified target from `rows()`.
        NativeCommand::Focus(_) | NativeCommand::Select(_) => StartPointPickerUpdate::Ignored,
        NativeCommand::Accept => focused_index
            .and_then(|index| state.target_at(index))
            .map_or(StartPointPickerUpdate::Ignored, |target| {
                state.dispatch(StartPointPickerCommand::Activate(target))
            }),
        NativeCommand::Back => state.dispatch(StartPointPickerCommand::Back(generation)),
        NativeCommand::Step(Step::Up) => state.dispatch(StartPointPickerCommand::Move {
            generation,
            delta: -1,
        }),
        NativeCommand::Step(Step::Down) => state.dispatch(StartPointPickerCommand::Move {
            generation,
            delta: 1,
        }),
        NativeCommand::Step(Step::Left | Step::Right)
        | NativeCommand::Adjust { .. }
        | NativeCommand::Scroll(_)
        | NativeCommand::Delete
        | NativeCommand::Shortcut
        | NativeCommand::Join(_)
        | NativeCommand::ResumeGame => StartPointPickerUpdate::Ignored,
    }
}

pub fn dispatch_commands(
    state: &mut StartPointPickerState,
    focused_index: Option<usize>,
    commands: &[NativeCommand],
) -> Vec<StartPointPickerUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        let update = dispatch(state, focus, *command);
        if update != StartPointPickerUpdate::Ignored {
            updates.push(update);
        }
        focus = state
            .focused()
            .and_then(|choice| state.choices().iter().position(|row| *row == choice));
    }
    updates
}

pub fn dispatch_frame(
    state: &mut StartPointPickerState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<StartPointPickerUpdate> {
    let generation = state.generation();
    let mut updates = Vec::new();
    for action in crate::owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        let update = match action {
            CatalogueAction::Back => state.dispatch(StartPointPickerCommand::Back(generation)),
            CatalogueAction::Step(delta) => {
                state.dispatch(StartPointPickerCommand::Move { generation, delta })
            }
            CatalogueAction::Accept => {
                state
                    .focused()
                    .map_or(StartPointPickerUpdate::Ignored, |choice| {
                        state
                            .target(choice)
                            .map_or(StartPointPickerUpdate::Ignored, |target| {
                                state.dispatch(StartPointPickerCommand::Activate(target))
                            })
                    })
            }
            CatalogueAction::Focus(_) => StartPointPickerUpdate::Ignored,
        };
        if update != StartPointPickerUpdate::Ignored {
            updates.push(update);
        }
    }
    updates
}

/// Route one native command through the picker owned by the shared UI state.
/// The typed selection request is returned to its caller; this adapter does
/// not invent a document assignment effect.
pub fn dispatch_shared(
    state: &mut MapMakerUiState,
    focused_index: Option<usize>,
    command: NativeCommand,
) -> Option<StartPointPickerUpdate> {
    state.with_assault_start_point_picker_mut(|picker| dispatch(picker, focused_index, command))
}

/// Route a renderer-resolved pointer event through the picker owned by the
/// shared Map Maker state. Stale generations are rejected by the reducer.
pub fn dispatch_pointer_shared(
    state: &mut MapMakerUiState,
    event: PointerEvent,
) -> Option<StartPointPickerUpdate> {
    state.with_assault_start_point_picker_mut(|picker| dispatch_pointer(picker, event))
}

/// Route the native keyboard/controller frame directly into shared Rust state.
pub fn dispatch_frame_shared(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Option<Vec<StartPointPickerUpdate>> {
    state.with_assault_start_point_picker_mut(|picker| {
        dispatch_frame(picker, held, before, now_ms, repeat_at)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MenuIntent as I;
    use tsfp_mapmaker::assault_start_point_picker::{
        StartPointChoiceId, StartPointPickerGeneration,
    };
    use tsfp_mapmaker::ui::MapMakerUiState;

    fn state() -> StartPointPickerState {
        StartPointPickerState::new(
            StartPointPickerGeneration::new(7),
            &[
                StartPointChoiceId::new(0xabc),
                StartPointChoiceId::new(0xdef),
            ],
            false,
        )
        .unwrap()
    }

    #[test]
    fn keyboard_down_and_accept_select_the_focused_opaque_row() {
        let second = StartPointChoiceId::new(0xdef);
        let mut state = state();
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(I::Down);
        assert_eq!(
            dispatch_frame(&mut state, down, IntentSet::default(), 100, &mut repeat_at),
            [StartPointPickerUpdate::Focused(StartPointChoiceId::new(
                0xabc
            ))]
        );
        assert_eq!(
            dispatch_frame(&mut state, down, down, 101, &mut repeat_at),
            []
        );
        let released = IntentSet::default();
        dispatch_frame(&mut state, released, down, 102, &mut repeat_at);
        assert_eq!(
            dispatch_frame(&mut state, down, released, 200, &mut repeat_at),
            [StartPointPickerUpdate::Focused(second)]
        );
        let accept = IntentSet::default().with(I::Accept);
        assert_eq!(
            dispatch_frame(&mut state, accept, down, 201, &mut repeat_at),
            [StartPointPickerUpdate::SelectionRequested(second)]
        );
    }

    #[test]
    fn pointer_focus_and_click_use_the_same_opaque_target() {
        let second = StartPointChoiceId::new(0xdef);
        let mut state = state();
        let target = state.target(second).unwrap();
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Focus(target)),
            StartPointPickerUpdate::Focused(second)
        );
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Activate(target)),
            StartPointPickerUpdate::SelectionRequested(second)
        );
    }

    #[test]
    fn keyboard_and_pointer_dispatch_mutate_the_shared_ui_owned_picker() {
        let first = StartPointChoiceId::new(0xabc);
        let second = StartPointChoiceId::new(0xdef);
        let mut ui = MapMakerUiState::default();
        ui.set_assault_start_point_picker(Some(state()));

        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(I::Down);
        assert_eq!(
            dispatch_frame_shared(&mut ui, down, IntentSet::default(), 100, &mut repeat_at),
            Some(vec![StartPointPickerUpdate::Focused(first)])
        );
        let pointer_target = ui
            .assault_start_point_picker()
            .unwrap()
            .target_at(1)
            .unwrap();
        assert_eq!(
            dispatch_pointer_shared(&mut ui, PointerEvent::Activate(pointer_target)),
            Some(StartPointPickerUpdate::SelectionRequested(second))
        );
        assert_eq!(
            ui.assault_start_point_picker().unwrap().focused(),
            Some(second)
        );
        assert_eq!(
            ui.snapshot().assault_start_point_picker.unwrap().focused(),
            Some(second)
        );
    }

    #[test]
    fn shared_picker_dispatch_is_absent_when_the_page_is_not_active() {
        let mut ui = MapMakerUiState::default();
        assert_eq!(dispatch_shared(&mut ui, None, NativeCommand::Back), None);
    }

    #[test]
    fn pointer_hits_from_a_replaced_dynamic_list_are_rejected() {
        let stale = state().target_at(1).unwrap();
        let replacement_choice = StartPointChoiceId::new(0x123);
        let replacement = StartPointPickerState::new(
            StartPointPickerGeneration::new(8),
            &[replacement_choice],
            false,
        )
        .unwrap();
        let mut ui = MapMakerUiState::default();
        ui.set_assault_start_point_picker(Some(replacement));

        assert_eq!(
            dispatch_pointer_shared(&mut ui, PointerEvent::Activate(stale)),
            Some(StartPointPickerUpdate::Ignored)
        );
        assert_eq!(ui.assault_start_point_picker().unwrap().focused(), None);

        let current = ui
            .assault_start_point_picker()
            .unwrap()
            .rows()
            .next()
            .unwrap()
            .target();
        assert_eq!(
            dispatch_pointer_shared(&mut ui, PointerEvent::Activate(current)),
            Some(StartPointPickerUpdate::SelectionRequested(
                replacement_choice
            ))
        );
    }
}
