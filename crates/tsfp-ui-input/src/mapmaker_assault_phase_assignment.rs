//! Native keyboard, controller, and pointer input for the Assault phase
//! start-point page (T2408).

use std::vec::{self, Vec};

use crate::{
    input::{IntentSet, MenuIntent},
    owned_catalogue::CatalogueAction,
};
use tsfp_mapmaker::assault_phase_assignment::{
    AssaultPhaseAssignmentAction, AssaultPhaseAssignmentCommand, AssaultPhaseAssignmentGeneration,
    AssaultPhaseAssignmentState, AssaultPhaseAssignmentTarget, AssaultPhaseAssignmentUpdate,
};
use tsfp_mapmaker::ui::MapMakerUiState;

pub fn commands(
    state: &AssaultPhaseAssignmentState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<AssaultPhaseAssignmentCommand> {
    let mut result = Vec::new();
    for action in crate::owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => {
                result.push(AssaultPhaseAssignmentCommand::Back(state.generation()));
            }
            CatalogueAction::Step(delta) => result.push(AssaultPhaseAssignmentCommand::Move {
                generation: state.generation(),
                delta,
            }),
            CatalogueAction::Accept => {
                if let Some(focused) = state.focused() {
                    result.push(AssaultPhaseAssignmentCommand::Activate(
                        state.target(focused),
                    ));
                }
            }
            CatalogueAction::Focus(_) => {}
        }
    }
    result
}

pub fn dispatch_frame(
    state: &mut AssaultPhaseAssignmentState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<AssaultPhaseAssignmentUpdate> {
    commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(AssaultPhaseAssignmentTarget),
    Click(AssaultPhaseAssignmentTarget),
    Back(AssaultPhaseAssignmentGeneration),
}

pub fn pointer_commands(event: PointerEvent) -> Vec<AssaultPhaseAssignmentCommand> {
    match event {
        PointerEvent::Hover(target) => vec![AssaultPhaseAssignmentCommand::Focus(target)],
        PointerEvent::Click(target) => vec![
            AssaultPhaseAssignmentCommand::Focus(target),
            AssaultPhaseAssignmentCommand::Activate(target),
        ],
        PointerEvent::Back(generation) => vec![AssaultPhaseAssignmentCommand::Back(generation)],
    }
}

/// Route a target resolved from the rendered page into the shared Map Maker
/// model. The command retains its render generation for stale-event checks.
pub fn dispatch_pointer_shared(
    state: &mut MapMakerUiState,
    event: PointerEvent,
) -> Vec<AssaultPhaseAssignmentUpdate> {
    pointer_commands(event)
        .into_iter()
        .map(|command| state.dispatch_assault_phase_assignment(command))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_down_and_accept_request_start_point_selection() {
        let mut state = AssaultPhaseAssignmentState::new(
            tsfp_mapmaker::assault_phase_assignment::AssaultPhaseAssignmentGeneration::new(3),
            false,
        );
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert_eq!(
            dispatch_frame(&mut state, down, IntentSet::default(), 100, &mut repeat_at),
            [AssaultPhaseAssignmentUpdate::Focused(
                AssaultPhaseAssignmentAction::SelectStartPoint
            )]
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_frame(&mut state, accept, down, 101, &mut repeat_at),
            [AssaultPhaseAssignmentUpdate::StartPointSelectionRequested]
        );
    }

    #[test]
    fn pointer_click_requests_and_back_are_typed() {
        let state = AssaultPhaseAssignmentState::new(
            tsfp_mapmaker::assault_phase_assignment::AssaultPhaseAssignmentGeneration::new(6),
            false,
        );
        let target = state.target(AssaultPhaseAssignmentAction::SelectStartPoint);
        let click = pointer_commands(PointerEvent::Click(target));
        assert_eq!(click.len(), 2);
        assert!(matches!(click[0], AssaultPhaseAssignmentCommand::Focus(_)));
        assert!(matches!(
            click[1],
            AssaultPhaseAssignmentCommand::Activate(_)
        ));
        assert_eq!(
            pointer_commands(PointerEvent::Back(state.generation())),
            [AssaultPhaseAssignmentCommand::Back(state.generation())]
        );
    }

    #[test]
    fn pointer_from_old_page_generation_is_rejected_by_shared_state() {
        let mut shared = MapMakerUiState::default();
        let stale = AssaultPhaseAssignmentState::new(
            tsfp_mapmaker::assault_phase_assignment::AssaultPhaseAssignmentGeneration::new(6),
            false,
        );
        let stale_target = stale.target(AssaultPhaseAssignmentAction::Done);
        shared.set_assault_phase_assignment(Some(AssaultPhaseAssignmentState::new(
            tsfp_mapmaker::assault_phase_assignment::AssaultPhaseAssignmentGeneration::new(7),
            false,
        )));
        assert_eq!(
            dispatch_pointer_shared(&mut shared, PointerEvent::Click(stale_target)),
            [
                AssaultPhaseAssignmentUpdate::Ignored,
                AssaultPhaseAssignmentUpdate::Ignored
            ]
        );
    }
}
