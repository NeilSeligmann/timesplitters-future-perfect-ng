//! Renderer projection for the caller supplied Assault participant choices.

use crate::assault_options::{
    AssaultOptionsState, AssaultParticipantChoiceId, AssaultParticipantChoiceTarget,
    AssaultParticipantField, AssaultParticipantRuntimeOptionId,
};

/// Opaque caller ordered choice with enabled, selected and input target state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultParticipantChoiceRow {
    id: AssaultParticipantChoiceId,
    runtime_option_id: AssaultParticipantRuntimeOptionId,
    target: Option<AssaultParticipantChoiceTarget>,
    enabled: bool,
    selected: bool,
}

impl AssaultParticipantChoiceRow {
    pub const fn id(self) -> AssaultParticipantChoiceId {
        self.id
    }

    pub const fn runtime_option_id(self) -> AssaultParticipantRuntimeOptionId {
        self.runtime_option_id
    }

    pub const fn target(self) -> Option<AssaultParticipantChoiceTarget> {
        self.target
    }

    pub const fn enabled(self) -> bool {
        self.enabled
    }

    pub const fn selected(self) -> bool {
        self.selected
    }
}

/// Preserve the installed order and project state without interpreting IDs.
pub fn render_rows(
    state: &AssaultOptionsState,
    field: AssaultParticipantField,
) -> Vec<AssaultParticipantChoiceRow> {
    let selected = state.selected_participant_choice(field);
    state
        .participant_choices(field)
        .iter()
        .map(|choice| AssaultParticipantChoiceRow {
            id: choice.id,
            runtime_option_id: choice.runtime_option_id,
            target: state.participant_choice_target(field, choice.id),
            enabled: choice.enabled,
            selected: selected == Some(choice.id),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assault_options::{
        AssaultOptionsGeneration, AssaultParticipantChoice, AssaultParticipantRuntimeOptionId,
    };

    #[test]
    fn projects_installed_order_and_only_enabled_targets() {
        let mut state = AssaultOptionsState::new(AssaultOptionsGeneration::new(9));
        let first = AssaultParticipantChoiceId::new(5);
        let disabled = AssaultParticipantChoiceId::new(2);
        assert!(state.install_participant_choices(
            AssaultParticipantField::Attackers,
            vec![
                AssaultParticipantChoice {
                    id: first,
                    runtime_option_id: AssaultParticipantRuntimeOptionId::new(17),
                    enabled: true,
                },
                AssaultParticipantChoice {
                    id: disabled,
                    runtime_option_id: AssaultParticipantRuntimeOptionId::new(33),
                    enabled: false,
                },
            ],
            first,
        ));

        let rows = render_rows(&state, AssaultParticipantField::Attackers);
        assert_eq!(
            rows.iter().map(|row| row.id()).collect::<Vec<_>>(),
            [first, disabled]
        );
        assert!(rows[0].selected());
        assert_eq!(rows[0].target().unwrap().choice(), first);
        assert!(rows[1].target().is_none());
        assert!(!rows[1].enabled());
        assert!(!rows[1].selected());
    }
}
