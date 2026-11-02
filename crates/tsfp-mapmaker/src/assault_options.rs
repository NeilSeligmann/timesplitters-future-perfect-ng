//! Rust-owned semantic state for the Assault Options page (T2408).
//!
//! The runtime page still supplies presentation and the effect callbacks. This
//! model owns typed focus and action requests without carrying widget or guest
//! addresses across the UI boundary.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssaultOption {
    PreviewTeam,
    Attackers,
    Defenders,
    StartAssault,
}

impl AssaultOption {
    pub const fn participant_field(self) -> Option<AssaultParticipantField> {
        match self {
            Self::Attackers => Some(AssaultParticipantField::Attackers),
            Self::Defenders => Some(AssaultParticipantField::Defenders),
            Self::PreviewTeam | Self::StartAssault => None,
        }
    }
}

/// One of the page's two live choice controls. Choice meaning and localized
/// text remain at the runtime data seam until their value table is measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssaultParticipantField {
    Attackers,
    Defenders,
}

/// Opaque ordinal identity from the current live choice table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultParticipantChoiceId(u32);

impl AssaultParticipantChoiceId {
    pub const fn new(ordinal: u32) -> Self {
        Self(ordinal)
    }

    pub const fn ordinal(self) -> u32 {
        self.0
    }
}

/// Opaque ID stored on the runtime option entry. Its gameplay meaning is not
/// established by the current REA record, so native callers must not treat it
/// as a participant count or launch value without a separate contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultParticipantRuntimeOptionId(u32);

impl AssaultParticipantRuntimeOptionId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssaultParticipantChoice {
    pub id: AssaultParticipantChoiceId,
    pub runtime_option_id: AssaultParticipantRuntimeOptionId,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultParticipantChoiceTarget {
    generation: AssaultOptionsGeneration,
    field: AssaultParticipantField,
    choice: AssaultParticipantChoiceId,
}

impl AssaultParticipantChoiceTarget {
    pub const fn new(
        generation: AssaultOptionsGeneration,
        field: AssaultParticipantField,
        choice: AssaultParticipantChoiceId,
    ) -> Self {
        Self {
            generation,
            field,
            choice,
        }
    }

    pub const fn field(self) -> AssaultParticipantField {
        self.field
    }

    pub const fn choice(self) -> AssaultParticipantChoiceId {
        self.choice
    }
}

pub const ASSAULT_OPTIONS_ORDER: [AssaultOption; 4] = [
    AssaultOption::PreviewTeam,
    AssaultOption::Attackers,
    AssaultOption::Defenders,
    AssaultOption::StartAssault,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultOptionsGeneration(u64);

impl AssaultOptionsGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssaultOptionTarget {
    generation: AssaultOptionsGeneration,
    option: AssaultOption,
}

impl AssaultOptionTarget {
    pub const fn new(generation: AssaultOptionsGeneration, option: AssaultOption) -> Self {
        Self { generation, option }
    }

    pub const fn generation(self) -> AssaultOptionsGeneration {
        self.generation
    }

    pub const fn option(self) -> AssaultOption {
        self.option
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultOptionsCommand {
    Focus(AssaultOptionTarget),
    Activate(AssaultOptionTarget),
    SelectParticipantChoice(AssaultParticipantChoiceTarget),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultOptionsUpdate {
    Ignored,
    Focused(AssaultOption),
    ActionRequested(AssaultOption),
    ParticipantChoiceSelected {
        field: AssaultParticipantField,
        choice: AssaultParticipantChoiceId,
        runtime_option_id: AssaultParticipantRuntimeOptionId,
    },
}

/// State for one opening of the Assault Options page. Reopening it requires a
/// new generation so delayed pointer targets cannot activate a later page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssaultOptionsState {
    generation: AssaultOptionsGeneration,
    focused: Option<AssaultOption>,
    attackers: Vec<AssaultParticipantChoice>,
    defenders: Vec<AssaultParticipantChoice>,
    attacker_choice: Option<AssaultParticipantChoiceId>,
    defender_choice: Option<AssaultParticipantChoiceId>,
}

impl AssaultOptionsState {
    pub const fn new(generation: AssaultOptionsGeneration) -> Self {
        Self {
            generation,
            focused: None,
            attackers: Vec::new(),
            defenders: Vec::new(),
            attacker_choice: None,
            defender_choice: None,
        }
    }

    pub const fn generation(&self) -> AssaultOptionsGeneration {
        self.generation
    }

    pub const fn focused(&self) -> Option<AssaultOption> {
        self.focused
    }

    pub const fn target(&self, option: AssaultOption) -> AssaultOptionTarget {
        AssaultOptionTarget::new(self.generation, option)
    }

    fn choices(&self, field: AssaultParticipantField) -> &[AssaultParticipantChoice] {
        match field {
            AssaultParticipantField::Attackers => &self.attackers,
            AssaultParticipantField::Defenders => &self.defenders,
        }
    }

    fn choices_mut(
        &mut self,
        field: AssaultParticipantField,
    ) -> &mut Vec<AssaultParticipantChoice> {
        match field {
            AssaultParticipantField::Attackers => &mut self.attackers,
            AssaultParticipantField::Defenders => &mut self.defenders,
        }
    }

    fn selected(&self, field: AssaultParticipantField) -> Option<AssaultParticipantChoiceId> {
        match field {
            AssaultParticipantField::Attackers => self.attacker_choice,
            AssaultParticipantField::Defenders => self.defender_choice,
        }
    }

    pub fn selected_participant_choice(
        &self,
        field: AssaultParticipantField,
    ) -> Option<AssaultParticipantChoiceId> {
        self.selected(field)
    }

    pub fn selected_runtime_option_id(
        &self,
        field: AssaultParticipantField,
    ) -> Option<AssaultParticipantRuntimeOptionId> {
        let selected = self.selected(field)?;
        self.choices(field)
            .iter()
            .find(|choice| choice.id == selected)
            .map(|choice| choice.runtime_option_id)
    }

    pub fn participant_choices(
        &self,
        field: AssaultParticipantField,
    ) -> &[AssaultParticipantChoice] {
        self.choices(field)
    }

    pub fn install_participant_choices(
        &mut self,
        field: AssaultParticipantField,
        choices: Vec<AssaultParticipantChoice>,
        selected: AssaultParticipantChoiceId,
    ) -> bool {
        if choices.is_empty()
            || choices
                .iter()
                .filter(|choice| choice.id == selected)
                .count()
                != 1
            || choices.iter().enumerate().any(|(index, choice)| {
                choices[..index]
                    .iter()
                    .any(|previous| previous.id == choice.id)
            })
        {
            return false;
        }
        *self.choices_mut(field) = choices;
        match field {
            AssaultParticipantField::Attackers => self.attacker_choice = Some(selected),
            AssaultParticipantField::Defenders => self.defender_choice = Some(selected),
        }
        true
    }

    pub fn participant_choice_target(
        &self,
        field: AssaultParticipantField,
        choice: AssaultParticipantChoiceId,
    ) -> Option<AssaultParticipantChoiceTarget> {
        self.choices(field)
            .iter()
            .any(|entry| entry.id == choice && entry.enabled)
            .then_some(AssaultParticipantChoiceTarget::new(
                self.generation,
                field,
                choice,
            ))
    }

    /// Resolve a left/right step using the live option order. Enabled options
    /// wrap exactly as the native page framework's cycler does.
    pub fn next_participant_choice(
        &self,
        field: AssaultParticipantField,
        forward: bool,
    ) -> Option<AssaultParticipantChoiceTarget> {
        let choices = self.choices(field);
        let current = choices
            .iter()
            .position(|choice| Some(choice.id) == self.selected(field))?;
        for offset in 1..=choices.len() {
            let index = if forward {
                (current + offset) % choices.len()
            } else {
                (current + choices.len() - offset % choices.len()) % choices.len()
            };
            let choice = choices[index];
            if choice.enabled {
                return self.participant_choice_target(field, choice.id);
            }
        }
        None
    }

    pub fn dispatch(&mut self, command: AssaultOptionsCommand) -> AssaultOptionsUpdate {
        let target = match command {
            AssaultOptionsCommand::Focus(target) | AssaultOptionsCommand::Activate(target) => {
                target
            }
            AssaultOptionsCommand::SelectParticipantChoice(target) => {
                if target.generation != self.generation {
                    return AssaultOptionsUpdate::Ignored;
                }
                let Some(choice) = self
                    .choices(target.field)
                    .iter()
                    .find(|choice| choice.id == target.choice && choice.enabled)
                else {
                    return AssaultOptionsUpdate::Ignored;
                };
                let runtime_option_id = choice.runtime_option_id;
                if self.selected(target.field) == Some(target.choice) {
                    return AssaultOptionsUpdate::Ignored;
                }
                match target.field {
                    AssaultParticipantField::Attackers => {
                        self.attacker_choice = Some(target.choice)
                    }
                    AssaultParticipantField::Defenders => {
                        self.defender_choice = Some(target.choice)
                    }
                }
                self.focused = Some(match target.field {
                    AssaultParticipantField::Attackers => AssaultOption::Attackers,
                    AssaultParticipantField::Defenders => AssaultOption::Defenders,
                });
                return AssaultOptionsUpdate::ParticipantChoiceSelected {
                    field: target.field,
                    choice: target.choice,
                    runtime_option_id,
                };
            }
        };
        if target.generation != self.generation {
            return AssaultOptionsUpdate::Ignored;
        }
        match command {
            AssaultOptionsCommand::Focus(target) => {
                self.focused = Some(target.option);
                AssaultOptionsUpdate::Focused(target.option)
            }
            AssaultOptionsCommand::Activate(target) => {
                if target.option.participant_field().is_some() {
                    return AssaultOptionsUpdate::Ignored;
                }
                self.focused = Some(target.option);
                AssaultOptionsUpdate::ActionRequested(target.option)
            }
            AssaultOptionsCommand::SelectParticipantChoice(_) => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_focus_and_activation_are_generation_checked() {
        let generation = AssaultOptionsGeneration::new(7);
        let mut state = AssaultOptionsState::new(generation);
        let target = state.target(AssaultOption::StartAssault);
        assert_eq!(
            state.dispatch(AssaultOptionsCommand::Focus(target)),
            AssaultOptionsUpdate::Focused(AssaultOption::StartAssault)
        );
        assert_eq!(state.focused(), Some(AssaultOption::StartAssault));
        assert_eq!(
            state.dispatch(AssaultOptionsCommand::Activate(target)),
            AssaultOptionsUpdate::ActionRequested(AssaultOption::StartAssault)
        );
        assert_eq!(
            state.dispatch(AssaultOptionsCommand::Activate(AssaultOptionTarget::new(
                AssaultOptionsGeneration::new(6),
                AssaultOption::StartAssault,
            ))),
            AssaultOptionsUpdate::Ignored
        );
        assert_eq!(state.focused(), Some(AssaultOption::StartAssault));
    }

    #[test]
    fn participant_choice_steps_wrap_skip_disabled_and_reject_stale_targets() {
        let generation = AssaultOptionsGeneration::new(4);
        let mut state = AssaultOptionsState::new(generation);
        let choices = [true, false, true]
            .into_iter()
            .enumerate()
            .map(|(index, enabled)| AssaultParticipantChoice {
                id: AssaultParticipantChoiceId::new(index as u32),
                runtime_option_id: AssaultParticipantRuntimeOptionId::new(0x80 + index as u32),
                enabled,
            })
            .collect();
        assert!(state.install_participant_choices(
            AssaultParticipantField::Attackers,
            choices,
            AssaultParticipantChoiceId::new(0),
        ));
        let next = state
            .next_participant_choice(AssaultParticipantField::Attackers, true)
            .expect("enabled successor");
        assert_eq!(next.choice(), AssaultParticipantChoiceId::new(2));
        assert_eq!(
            state.dispatch(AssaultOptionsCommand::SelectParticipantChoice(next)),
            AssaultOptionsUpdate::ParticipantChoiceSelected {
                field: AssaultParticipantField::Attackers,
                choice: AssaultParticipantChoiceId::new(2),
                runtime_option_id: AssaultParticipantRuntimeOptionId::new(0x82),
            }
        );
        let stale = AssaultParticipantChoiceTarget::new(
            AssaultOptionsGeneration::new(3),
            AssaultParticipantField::Attackers,
            AssaultParticipantChoiceId::new(0),
        );
        assert_eq!(
            state.dispatch(AssaultOptionsCommand::SelectParticipantChoice(stale)),
            AssaultOptionsUpdate::Ignored
        );
    }
}
