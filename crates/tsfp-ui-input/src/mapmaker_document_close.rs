//! Native input for the Rust-owned Map Maker dirty-document close transaction.
//!
//! Callers provide the visible row order and action bindings. The adapter
//! does not infer the original prompt labels, row order, or Back callback.

use std::vec::Vec;

use crate::{
    input::IntentSet, native_page::NativeCommand, owned_catalogue::CatalogueAction, widgets::Step,
};
pub use tsfp_mapmaker::document_close::{ClosePromptChoice, ClosePromptChoiceId};
use tsfp_mapmaker::{
    document_close::{
        ClosePromptAction, ClosePromptCommand, ClosePromptPhase, ClosePromptToken,
        ClosePromptUpdate,
    },
    ui::{MapMakerPage, MapMakerUiState},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptInputError {
    DuplicateChoiceId,
    BackChoiceMissing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptInputUpdate {
    Focused(ClosePromptChoiceId),
    Prompt(ClosePromptUpdate),
    Ignored,
}

/// Input state is scoped to one document generation. `choices` are supplied
/// by the page adapter in its measured order; `back_choice` is an explicit
/// page binding and may be omitted when original Back behavior is unresolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosePromptInput {
    token: ClosePromptToken,
    choices: Vec<ClosePromptChoice>,
    focused: Option<ClosePromptChoiceId>,
    back_choice: Option<ClosePromptChoiceId>,
    wrap_navigation: bool,
}

impl ClosePromptInput {
    pub fn new(
        token: ClosePromptToken,
        choices: &[ClosePromptChoice],
        back_choice: Option<ClosePromptChoiceId>,
        wrap_navigation: bool,
    ) -> Result<Self, ClosePromptInputError> {
        for (index, choice) in choices.iter().enumerate() {
            if choices[..index]
                .iter()
                .any(|prior| prior.id() == choice.id())
            {
                return Err(ClosePromptInputError::DuplicateChoiceId);
            }
        }
        if back_choice.is_some_and(|id| !choices.iter().any(|choice| choice.id() == id)) {
            return Err(ClosePromptInputError::BackChoiceMissing);
        }
        Ok(Self {
            token,
            choices: choices.to_vec(),
            focused: None,
            back_choice,
            wrap_navigation,
        })
    }

    pub const fn focused(&self) -> Option<ClosePromptChoiceId> {
        self.focused
    }

    pub fn dispatch(
        &mut self,
        ui: &mut MapMakerUiState,
        command: NativeCommand,
    ) -> ClosePromptInputUpdate {
        if !self.is_current(ui) {
            return ClosePromptInputUpdate::Ignored;
        }

        match command {
            NativeCommand::Focus(index) => self.focus(index as usize),
            NativeCommand::Select(index) => self
                .choices
                .get(index as usize)
                .map(|choice| choice.id())
                .map_or(ClosePromptInputUpdate::Ignored, |id| {
                    self.activate_choice(ui, id)
                }),
            NativeCommand::Accept => self.focused.map_or(ClosePromptInputUpdate::Ignored, |id| {
                self.activate_choice(ui, id)
            }),
            NativeCommand::Back => self
                .back_choice
                .and_then(|id| self.choice(id))
                .map_or(ClosePromptInputUpdate::Ignored, |choice| {
                    self.activate_choice(ui, choice.id())
                }),
            NativeCommand::Step(Step::Up) => self.move_focus(-1),
            NativeCommand::Step(Step::Down) => self.move_focus(1),
            NativeCommand::Step(Step::Left | Step::Right)
            | NativeCommand::Adjust { .. }
            | NativeCommand::Scroll(_)
            | NativeCommand::Delete
            | NativeCommand::Shortcut
            | NativeCommand::Join(_)
            | NativeCommand::ResumeGame => ClosePromptInputUpdate::Ignored,
        }
    }

    /// Activate the row identity supplied by the renderer/page projection.
    /// This keeps widget IDs out of the input reducer and revalidates the
    /// document generation immediately before applying the action.
    pub fn activate_choice(
        &mut self,
        ui: &mut MapMakerUiState,
        id: ClosePromptChoiceId,
    ) -> ClosePromptInputUpdate {
        if !self.is_current(ui) {
            return ClosePromptInputUpdate::Ignored;
        }
        let Some(choice) = self.choice(id) else {
            return ClosePromptInputUpdate::Ignored;
        };
        self.focused = Some(choice.id());
        self.choose(ui, choice.action())
    }

    pub fn dispatch_frame(
        &mut self,
        ui: &mut MapMakerUiState,
        held: IntentSet,
        before: IntentSet,
        now_ms: u64,
        repeat_at: &mut [u64; 4],
    ) -> Vec<ClosePromptInputUpdate> {
        let actions = crate::owned_catalogue::actions(held, before, now_ms, repeat_at, false);
        let mut updates = Vec::new();
        for action in actions {
            let command = match action {
                CatalogueAction::Step(-1) => Some(NativeCommand::Step(Step::Up)),
                CatalogueAction::Step(1) => Some(NativeCommand::Step(Step::Down)),
                CatalogueAction::Step(_) => None,
                CatalogueAction::Focus(index) => {
                    u32::try_from(index).ok().map(NativeCommand::Focus)
                }
                CatalogueAction::Accept => Some(NativeCommand::Accept),
                CatalogueAction::Back => Some(NativeCommand::Back),
            };
            if let Some(command) = command {
                let update = self.dispatch(ui, command);
                if update != ClosePromptInputUpdate::Ignored {
                    updates.push(update);
                }
            }
        }
        updates
    }

    fn choice(&self, id: ClosePromptChoiceId) -> Option<ClosePromptChoice> {
        self.choices
            .iter()
            .find(|choice| choice.id() == id)
            .copied()
    }

    fn is_current(&self, ui: &MapMakerUiState) -> bool {
        ui.page() == MapMakerPage::Editor
            && ui.document_close_prompt_token() == Some(self.token)
            && ui.document_close_prompt_phase() == Some(ClosePromptPhase::AwaitingChoice)
    }

    fn focus(&mut self, index: usize) -> ClosePromptInputUpdate {
        let Some(choice) = self.choices.get(index) else {
            return ClosePromptInputUpdate::Ignored;
        };
        self.focused = Some(choice.id());
        ClosePromptInputUpdate::Focused(choice.id())
    }

    fn move_focus(&mut self, delta: i32) -> ClosePromptInputUpdate {
        if self.choices.is_empty() {
            return ClosePromptInputUpdate::Ignored;
        }
        let current = self
            .focused
            .and_then(|id| self.choices.iter().position(|choice| choice.id() == id));
        let next = match (current, delta) {
            (None, 1) => Some(0),
            (None, -1) => Some(self.choices.len() - 1),
            (Some(index), 1) if index + 1 < self.choices.len() => Some(index + 1),
            (Some(index), -1) if index > 0 => Some(index - 1),
            (Some(index), 1) if self.wrap_navigation => Some(0),
            (Some(0), -1) if self.wrap_navigation => Some(self.choices.len() - 1),
            _ => None,
        };
        next.map_or(ClosePromptInputUpdate::Ignored, |index| self.focus(index))
    }

    fn choose(
        &self,
        ui: &mut MapMakerUiState,
        action: ClosePromptAction,
    ) -> ClosePromptInputUpdate {
        ClosePromptInputUpdate::Prompt(ui.apply_document_close_prompt(ClosePromptCommand::Choose {
            token: self.token,
            action,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MenuIntent as I;
    use tsfp_mapmaker::document_close::{CloseDisposition, ClosePromptPhase, MapDocumentId};

    fn fixture() -> (MapMakerUiState, ClosePromptInput) {
        let mut ui = MapMakerUiState::default();
        ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::BeginEditor);
        let token = ClosePromptToken::new(MapDocumentId::new(4), 9);
        assert!(ui.open_document_close_prompt(token));
        assert_eq!(
            ui.snapshot()
                .document_close_prompt
                .map(|prompt| prompt.token()),
            Some(token)
        );
        let choices = [
            ClosePromptChoice::new(ClosePromptChoiceId::new(0xa), ClosePromptAction::Discard),
            ClosePromptChoice::new(ClosePromptChoiceId::new(0xb), ClosePromptAction::Save),
            ClosePromptChoice::new(ClosePromptChoiceId::new(0xc), ClosePromptAction::Cancel),
        ];
        let input = ClosePromptInput::new(token, &choices, Some(choices[2].id()), false).unwrap();
        (ui, input)
    }

    #[test]
    fn keyboard_navigation_and_accept_dispatch_the_caller_bound_choice() {
        let (mut ui, mut input) = fixture();
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(I::Down);
        assert_eq!(
            input.dispatch_frame(&mut ui, down, IntentSet::default(), 100, &mut repeat_at),
            [ClosePromptInputUpdate::Focused(ClosePromptChoiceId::new(
                0xa
            ))]
        );
        assert_eq!(
            input.dispatch_frame(&mut ui, down, down, 101, &mut repeat_at),
            []
        );
        assert_eq!(
            input.dispatch_frame(&mut ui, down, IntentSet::default(), 200, &mut repeat_at),
            [ClosePromptInputUpdate::Focused(ClosePromptChoiceId::new(
                0xb
            ))]
        );
        let accept = IntentSet::default().with(I::Accept);
        let update = input.dispatch_frame(&mut ui, accept, down, 201, &mut repeat_at);
        let [ClosePromptInputUpdate::Prompt(ClosePromptUpdate::SaveRequested(request))] =
            update.as_slice()
        else {
            panic!("Accept should dispatch the bound save action");
        };
        assert_eq!(
            request.token,
            ClosePromptToken::new(MapDocumentId::new(4), 9)
        );
        assert!(matches!(
            ui.document_close_prompt_phase(),
            Some(ClosePromptPhase::Saving(_))
        ));
    }

    #[test]
    fn pointer_selection_and_explicit_back_binding_share_the_close_model() {
        let (mut ui, mut input) = fixture();
        assert_eq!(
            input.dispatch(&mut ui, NativeCommand::Select(0)),
            ClosePromptInputUpdate::Prompt(ClosePromptUpdate::Closed {
                token: ClosePromptToken::new(MapDocumentId::new(4), 9),
                disposition: CloseDisposition::Discarded,
            })
        );

        let (mut ui, mut input) = fixture();
        assert_eq!(
            input.dispatch(&mut ui, NativeCommand::Back),
            ClosePromptInputUpdate::Prompt(ClosePromptUpdate::Cancelled(ClosePromptToken::new(
                MapDocumentId::new(4),
                9
            ),))
        );
    }

    #[test]
    fn renderer_choice_identity_activates_the_matching_typed_action() {
        let (mut ui, mut input) = fixture();
        let id = ClosePromptChoiceId::new(0xb);
        let ClosePromptInputUpdate::Prompt(ClosePromptUpdate::SaveRequested(request)) =
            input.activate_choice(&mut ui, id)
        else {
            panic!("the renderer-bound Save choice should request storage");
        };
        assert_eq!(
            request.token,
            ClosePromptToken::new(MapDocumentId::new(4), 9)
        );
        assert_eq!(
            ui.document_close_prompt_phase(),
            Some(ClosePromptPhase::Saving(request.id))
        );
        assert_eq!(
            input.activate_choice(&mut ui, id),
            ClosePromptInputUpdate::Ignored
        );
    }

    #[test]
    fn stale_generation_and_unbound_back_do_not_dispatch_choices() {
        let (_, mut input) = fixture();
        let mut ui = MapMakerUiState::default();
        ui.dispatch(tsfp_mapmaker::ui::MapMakerUiCommand::BeginEditor);
        let other = ClosePromptToken::new(MapDocumentId::new(4), 10);
        assert!(ui.open_document_close_prompt(other));
        assert_eq!(
            input.dispatch(&mut ui, NativeCommand::Back),
            ClosePromptInputUpdate::Ignored
        );

        let choices = [ClosePromptChoice::new(
            ClosePromptChoiceId::new(0x55),
            ClosePromptAction::Cancel,
        )];
        let mut unbound = ClosePromptInput::new(other, &choices, None, false).unwrap();
        assert_eq!(
            unbound.dispatch(&mut ui, NativeCommand::Back),
            ClosePromptInputUpdate::Ignored
        );
    }
}
