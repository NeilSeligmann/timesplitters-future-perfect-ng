//! Native keyboard, controller, and pointer input for Rust-owned Assault phase rows.
//!
//! Row identity is the typed phase kind plus document node ID. The reducer mutates
//! `AssaultPhases` directly and keeps the original callback and guest memory out of
//! the native input path.

use crate::{input::IntentSet, owned_catalogue::CatalogueAction};
use tsfp_mapmaker::modes::{AssaultPhaseKind, AssaultPhaseToggleResult, AssaultPhases};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultPhaseGeneration(u64);

impl AssaultPhaseGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseId {
    pub kind: AssaultPhaseKind,
    pub node_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseRow {
    pub id: AssaultPhaseId,
    /// Typed replacement for the original widget's select-enabled flag.
    pub widget_selectable: bool,
    /// Typed replacement for the selected phase descriptor's enabled field.
    pub descriptor_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultPhaseTarget {
    generation: AssaultPhaseGeneration,
    id: AssaultPhaseId,
}

impl AssaultPhaseTarget {
    pub const fn id(self) -> AssaultPhaseId {
        self.id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseCommand {
    Focus(AssaultPhaseTarget),
    Move {
        generation: AssaultPhaseGeneration,
        delta: i32,
    },
    Activate(AssaultPhaseTarget),
    Back(AssaultPhaseGeneration),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseUpdate {
    Ignored,
    Focused(AssaultPhaseId),
    SelectionChanged {
        id: AssaultPhaseId,
        before: u32,
        after: u32,
    },
    BackRequested,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseBuildError {
    DuplicateRow,
}

/// State for one opening of an Assault phase editor surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssaultPhasePageState {
    generation: AssaultPhaseGeneration,
    rows: Vec<AssaultPhaseRow>,
    focused: Option<AssaultPhaseId>,
}

impl AssaultPhasePageState {
    pub fn new(
        generation: AssaultPhaseGeneration,
        rows: Vec<AssaultPhaseRow>,
    ) -> Result<Self, AssaultPhaseBuildError> {
        for (index, row) in rows.iter().enumerate() {
            if rows[..index].iter().any(|prior| prior.id == row.id) {
                return Err(AssaultPhaseBuildError::DuplicateRow);
            }
        }
        Ok(Self {
            generation,
            rows,
            focused: None,
        })
    }

    pub fn rows(&self) -> &[AssaultPhaseRow] {
        &self.rows
    }

    pub const fn focused(&self) -> Option<AssaultPhaseId> {
        self.focused
    }

    pub fn target(&self, id: AssaultPhaseId) -> Option<AssaultPhaseTarget> {
        self.rows
            .iter()
            .any(|row| row.id == id)
            .then_some(AssaultPhaseTarget {
                generation: self.generation,
                id,
            })
    }

    pub fn dispatch(
        &mut self,
        phases: &mut AssaultPhases,
        command: AssaultPhaseCommand,
    ) -> AssaultPhaseUpdate {
        match command {
            AssaultPhaseCommand::Focus(target) if self.admits(target) => {
                self.focused = Some(target.id);
                AssaultPhaseUpdate::Focused(target.id)
            }
            AssaultPhaseCommand::Move { generation, delta }
                if generation == self.generation && !self.rows.is_empty() =>
            {
                let current = self
                    .focused
                    .and_then(|focused| self.rows.iter().position(|row| row.id == focused));
                let next = match current {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta < 0 => Some(self.rows.len() - 1),
                    None => Some(0),
                };
                let Some(next) = next.filter(|&index| index < self.rows.len()) else {
                    return AssaultPhaseUpdate::Ignored;
                };
                self.focused = Some(self.rows[next].id);
                AssaultPhaseUpdate::Focused(self.rows[next].id)
            }
            AssaultPhaseCommand::Activate(target) if self.admits(target) => {
                let Some(row) = self.rows.iter().find(|row| row.id == target.id) else {
                    return AssaultPhaseUpdate::Ignored;
                };
                match phases.toggle(
                    row.id.kind,
                    row.id.node_id,
                    row.widget_selectable,
                    row.descriptor_enabled,
                ) {
                    AssaultPhaseToggleResult::Changed { before, after } => {
                        AssaultPhaseUpdate::SelectionChanged {
                            id: row.id,
                            before,
                            after,
                        }
                    }
                    AssaultPhaseToggleResult::Disabled
                    | AssaultPhaseToggleResult::Missing
                    | AssaultPhaseToggleResult::Ambiguous => AssaultPhaseUpdate::Ignored,
                }
            }
            AssaultPhaseCommand::Back(generation) if generation == self.generation => {
                AssaultPhaseUpdate::BackRequested
            }
            _ => AssaultPhaseUpdate::Ignored,
        }
    }

    fn admits(&self, target: AssaultPhaseTarget) -> bool {
        target.generation == self.generation && self.rows.iter().any(|row| row.id == target.id)
    }
}

/// Convert one native keyboard/controller frame to typed page commands.
pub fn commands(
    state: &AssaultPhasePageState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<AssaultPhaseCommand> {
    let mut result = Vec::new();
    for action in crate::owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(AssaultPhaseCommand::Back(state.generation)),
            CatalogueAction::Step(delta) => result.push(AssaultPhaseCommand::Move {
                generation: state.generation,
                delta,
            }),
            CatalogueAction::Accept => {
                if let Some(focused) = state.focused {
                    if let Some(target) = state.target(focused) {
                        result.push(AssaultPhaseCommand::Activate(target));
                    }
                }
            }
            CatalogueAction::Focus(_) => {}
        }
    }
    result
}

pub fn dispatch_frame(
    state: &mut AssaultPhasePageState,
    phases: &mut AssaultPhases,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<AssaultPhaseUpdate> {
    commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(phases, command))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(AssaultPhaseId),
    Click(AssaultPhaseId),
    Back,
}

pub fn pointer_commands(
    state: &AssaultPhasePageState,
    event: PointerEvent,
) -> Vec<AssaultPhaseCommand> {
    match event {
        PointerEvent::Back => vec![AssaultPhaseCommand::Back(state.generation)],
        PointerEvent::Hover(id) | PointerEvent::Click(id) => {
            let Some(target) = state.target(id) else {
                return Vec::new();
            };
            let mut result = Vec::with_capacity(2);
            if state.focused != Some(id) {
                result.push(AssaultPhaseCommand::Focus(target));
            }
            if matches!(event, PointerEvent::Click(_)) {
                result.push(AssaultPhaseCommand::Activate(target));
            }
            result
        }
    }
}

pub fn dispatch_pointer(
    state: &mut AssaultPhasePageState,
    phases: &mut AssaultPhases,
    event: PointerEvent,
) -> Vec<AssaultPhaseUpdate> {
    pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(phases, command))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        AssaultPhaseBuildError, AssaultPhaseGeneration, AssaultPhaseId, AssaultPhasePageState,
        AssaultPhaseRow, AssaultPhaseUpdate, PointerEvent, dispatch_frame, dispatch_pointer,
    };
    use crate::input::{IntentSet, MenuIntent as I};
    use tsfp_mapmaker::modes::{
        AssaultPhaseKind, AssaultPhaseSelection, AssaultPhaseToggleResult, AssaultPhases,
    };

    fn row(node_id: u32) -> AssaultPhaseRow {
        AssaultPhaseRow {
            id: AssaultPhaseId {
                kind: AssaultPhaseKind::Assault,
                node_id,
            },
            widget_selectable: true,
            descriptor_enabled: true,
        }
    }

    fn phases(node_id: u32, selected_word: u32) -> AssaultPhases {
        AssaultPhases {
            phases: vec![AssaultPhaseSelection {
                kind: AssaultPhaseKind::Assault,
                node_id,
                selected_word,
            }],
        }
    }

    #[test]
    fn native_phase_activation_preserves_wrapping_raw_word() {
        let generation = AssaultPhaseGeneration::new(4);
        let id = row(0x31).id;
        let mut page = AssaultPhasePageState::new(generation, vec![row(0x31)]).unwrap();
        let mut phases = phases(0x31, 2);
        let updates = dispatch_pointer(&mut page, &mut phases, PointerEvent::Click(id));
        assert_eq!(phases.phases[0].selected_word, u32::MAX);
        assert_eq!(updates.len(), 2);
        assert!(matches!(
            phases.toggle(AssaultPhaseKind::Assault, 0x31, true, true),
            AssaultPhaseToggleResult::Changed {
                before: u32::MAX,
                after: 2
            }
        ));
    }

    #[test]
    fn disabled_phase_row_cannot_change_owned_state() {
        let generation = AssaultPhaseGeneration::new(7);
        let mut disabled = row(0x44);
        disabled.descriptor_enabled = false;
        let id = disabled.id;
        let mut page = AssaultPhasePageState::new(generation, vec![disabled]).unwrap();
        let mut phases = phases(0x44, 1);
        let updates = dispatch_pointer(&mut page, &mut phases, PointerEvent::Click(id));
        assert_eq!(phases.phases[0].selected_word, 1);
        assert_eq!(
            updates,
            vec![AssaultPhaseUpdate::Focused(id), AssaultPhaseUpdate::Ignored]
        );

        let mut disabled = row(0x44);
        disabled.widget_selectable = false;
        let mut page = AssaultPhasePageState::new(generation, vec![disabled]).unwrap();
        let updates = dispatch_pointer(&mut page, &mut phases, PointerEvent::Click(id));
        assert_eq!(phases.phases[0].selected_word, 1);
        assert_eq!(
            updates,
            vec![AssaultPhaseUpdate::Focused(id), AssaultPhaseUpdate::Ignored]
        );
    }

    #[test]
    fn native_keyboard_moves_focus_and_accepts_phase() {
        let generation = AssaultPhaseGeneration::new(8);
        let mut page = AssaultPhasePageState::new(generation, vec![row(0x51), row(0x52)]).unwrap();
        let mut phases = phases(0x51, 0);
        let mut repeat = [0; 4];
        let down = IntentSet::default().with(I::Down);
        let moved = dispatch_frame(
            &mut page,
            &mut phases,
            down,
            IntentSet::default(),
            10,
            &mut repeat,
        );
        assert_eq!(page.focused(), Some(row(0x51).id));
        assert_eq!(moved.len(), 1);
        let accept = IntentSet::default().with(I::Accept);
        let selected = dispatch_frame(&mut page, &mut phases, accept, down, 11, &mut repeat);
        assert_eq!(phases.phases[0].selected_word, 1);
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn stale_targets_and_duplicate_rows_are_rejected() {
        let old = AssaultPhaseGeneration::new(1);
        let new = AssaultPhaseGeneration::new(2);
        let mut page = AssaultPhasePageState::new(new, vec![row(0x66)]).unwrap();
        let target = super::AssaultPhaseTarget {
            generation: old,
            id: row(0x66).id,
        };
        let mut phase_data = phases(0x66, 0);
        assert_eq!(
            page.dispatch(
                &mut phase_data,
                super::AssaultPhaseCommand::Activate(target)
            ),
            super::AssaultPhaseUpdate::Ignored
        );
        assert_eq!(phase_data.phases[0].selected_word, 0);
        assert_eq!(
            AssaultPhasePageState::new(new, vec![row(0x66), row(0x66)]).unwrap_err(),
            AssaultPhaseBuildError::DuplicateRow
        );

        let mut duplicate_phases = phases(0x66, 0);
        duplicate_phases.phases.push(duplicate_phases.phases[0]);
        let mut page = AssaultPhasePageState::new(new, vec![row(0x66)]).unwrap();
        let updates = dispatch_pointer(
            &mut page,
            &mut duplicate_phases,
            PointerEvent::Click(row(0x66).id),
        );
        assert_eq!(duplicate_phases.phases[0].selected_word, 0);
        assert_eq!(duplicate_phases.phases[1].selected_word, 0);
        assert_eq!(
            updates,
            vec![
                AssaultPhaseUpdate::Focused(row(0x66).id),
                AssaultPhaseUpdate::Ignored
            ]
        );
    }
}
