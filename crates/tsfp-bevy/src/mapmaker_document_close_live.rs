// SPDX-License-Identifier: GPL-3.0-or-later
//! Caller-bound live-row input for the Rust-owned Map Maker dirty-close prompt.
//!
//! The REA record does not identify the original modal builder, choice rows,
//! or Back callback. The owner supplies those bindings and a page generation;
//! this adapter keeps the runtime's localized rows while sending native input
//! through the typed close transaction.

use tsfp_mapmaker::{
    document_close::{ClosePromptChoice, ClosePromptChoiceId, ClosePromptToken},
    ui::{MapMakerPage, MapMakerUiState},
};
use tsfp_ui_input::{
    input::IntentSet,
    live_page::{LivePage, LiveRow},
    mapmaker_document_close::{ClosePromptInput, ClosePromptInputUpdate},
    native_page::NativeCommand,
};

/// One caller-identified action row paired with its opaque close choice.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedChoiceRow<'a> {
    live_row: &'a LiveRow,
    choice: ClosePromptChoice,
}

impl<'a> ProjectedChoiceRow<'a> {
    pub const fn live_row(self) -> &'a LiveRow {
        self.live_row
    }

    pub const fn choice(self) -> ClosePromptChoice {
        self.choice
    }
}

/// A caller-bound view of one live close prompt page.
pub struct ClosePromptLivePage<'a> {
    builder: u32,
    slot: u32,
    page_generation: u64,
    token: ClosePromptToken,
    rows: Vec<ProjectedChoiceRow<'a>>,
    input: ClosePromptInput,
}

/// Opaque values a runtime caller supplies when it explicitly opens the
/// Rust-owned close prompt. None of these identify the original popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosePromptBinding {
    pub builder: u32,
    pub slot: u32,
    pub page_generation: u64,
    pub token: ClosePromptToken,
}

/// Successful open result; carry this into projection to keep page identity
/// tied to the close transaction that created it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenedClosePrompt {
    binding: ClosePromptBinding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptOpenError {
    ZeroPageGeneration,
    ZeroTokenGeneration,
    NotEditorPage,
    PromptAlreadyActive,
}

/// Failure when opening a caller-bound prompt cannot produce a safe live view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptBindError {
    Open(ClosePromptOpenError),
    ProjectionRejected,
    CancellationRejected,
}

/// Open from an explicit caller request while preserving all runtime-owned
/// identities as supplied. Mode-exit policy remains outside this adapter.
pub fn begin_close_prompt(
    ui: &mut MapMakerUiState,
    binding: ClosePromptBinding,
) -> Result<OpenedClosePrompt, ClosePromptOpenError> {
    if binding.page_generation == 0 {
        return Err(ClosePromptOpenError::ZeroPageGeneration);
    }
    if binding.token.generation() == 0 {
        return Err(ClosePromptOpenError::ZeroTokenGeneration);
    }
    if ui.page() != MapMakerPage::Editor {
        return Err(ClosePromptOpenError::NotEditorPage);
    }
    if !ui.open_document_close_prompt(binding.token) {
        return Err(ClosePromptOpenError::PromptAlreadyActive);
    }
    Ok(OpenedClosePrompt { binding })
}

/// Open and project atomically from the caller's perspective. If the live rows
/// do not satisfy the caller's binding, cancel only the prompt just opened.
pub fn begin_and_project_close_prompt<'a>(
    ui: &mut MapMakerUiState,
    live: &'a LivePage,
    binding: ClosePromptBinding,
    cursor_nodes: &[u32],
    choice_rows: &[(u32, ClosePromptChoice)],
    back_choice: Option<ClosePromptChoiceId>,
    wrap_navigation: bool,
) -> Result<ClosePromptLivePage<'a>, ClosePromptBindError> {
    let opened = begin_close_prompt(ui, binding).map_err(ClosePromptBindError::Open)?;
    if let Some(page) = ClosePromptLivePage::project_opened(
        ui,
        live,
        opened,
        cursor_nodes,
        choice_rows,
        back_choice,
        wrap_navigation,
    ) {
        return Ok(page);
    }
    if ui.cancel_unpresented_document_close_prompt(binding.token) {
        Err(ClosePromptBindError::ProjectionRejected)
    } else {
        Err(ClosePromptBindError::CancellationRejected)
    }
}

impl<'a> ClosePromptLivePage<'a> {
    /// Project using the page identity returned by `begin_close_prompt`.
    pub fn project_opened(
        ui: &MapMakerUiState,
        live: &'a LivePage,
        opened: OpenedClosePrompt,
        cursor_nodes: &[u32],
        choice_rows: &[(u32, ClosePromptChoice)],
        back_choice: Option<ClosePromptChoiceId>,
        wrap_navigation: bool,
    ) -> Option<Self> {
        let binding = opened.binding;
        if live.builder != binding.builder
            || live.slot != binding.slot
            || ui.document_close_prompt_token() != Some(binding.token)
        {
            return None;
        }
        Self::project(
            ui,
            live,
            binding.builder,
            binding.slot,
            binding.page_generation,
            cursor_nodes,
            choice_rows,
            back_choice,
            wrap_navigation,
        )
    }

    /// Bind typed choices to explicit live nodes in cursor order.
    ///
    /// `expected_builder`, `expected_slot`, `page_generation`, row choices,
    /// Back behavior, and wrap policy are caller supplied. This function does
    /// not identify the original popup or infer its action semantics.
    pub fn project(
        ui: &MapMakerUiState,
        live: &'a LivePage,
        expected_builder: u32,
        expected_slot: u32,
        page_generation: u64,
        cursor_nodes: &[u32],
        choice_rows: &[(u32, ClosePromptChoice)],
        back_choice: Option<ClosePromptChoiceId>,
        wrap_navigation: bool,
    ) -> Option<Self> {
        if ui.page() != MapMakerPage::Editor
            || page_generation == 0
            || live.builder != expected_builder
            || live.slot != expected_slot
            || choice_rows.is_empty()
        {
            return None;
        }
        let token = ui.document_close_prompt_token()?;
        if ui.document_close_prompt_phase()
            != Some(tsfp_mapmaker::document_close::ClosePromptPhase::AwaitingChoice)
        {
            return None;
        }

        let choices: Vec<_> = choice_rows.iter().map(|(_, choice)| *choice).collect();
        let mut previous_cursor_index = None;
        let mut rows = Vec::with_capacity(choice_rows.len());
        for (index, (node, choice)) in choice_rows.iter().copied().enumerate() {
            if choice_rows[..index]
                .iter()
                .any(|(prior_node, prior_choice)| {
                    *prior_node == node || prior_choice.id() == choice.id()
                })
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
                || live.rows.iter().filter(|row| row.node == node).count() != 1
            {
                return None;
            }
            previous_cursor_index = Some(cursor_index);
            rows.push(ProjectedChoiceRow {
                live_row: live.rows.iter().find(|row| row.node == node)?,
                choice,
            });
        }

        let input = ClosePromptInput::new(token, &choices, back_choice, wrap_navigation).ok()?;
        Some(Self {
            builder: live.builder,
            slot: live.slot,
            page_generation,
            token,
            rows,
            input,
        })
    }

    pub fn rows(&self) -> &[ProjectedChoiceRow<'a>] {
        &self.rows
    }

    pub const fn token(&self) -> ClosePromptToken {
        self.token
    }

    /// Apply a native command only while the caller still observes this page.
    pub fn dispatch(
        &mut self,
        ui: &mut MapMakerUiState,
        current_builder: u32,
        current_slot: u32,
        current_page_generation: u64,
        command: NativeCommand,
    ) -> ClosePromptInputUpdate {
        if !self.is_current(ui, current_builder, current_slot, current_page_generation) {
            return ClosePromptInputUpdate::Ignored;
        }
        self.input.dispatch(ui, command)
    }

    /// Route keyboard/controller intents through the same generation-checked reducer.
    pub fn dispatch_frame(
        &mut self,
        ui: &mut MapMakerUiState,
        current_builder: u32,
        current_slot: u32,
        current_page_generation: u64,
        held: IntentSet,
        before: IntentSet,
        now_ms: u64,
        repeat_at: &mut [u64; 4],
    ) -> Vec<ClosePromptInputUpdate> {
        if !self.is_current(ui, current_builder, current_slot, current_page_generation) {
            return Vec::new();
        }
        self.input
            .dispatch_frame(ui, held, before, now_ms, repeat_at)
    }

    /// Resolve pointer focus/activation from the caller's hit-tested live node.
    pub fn dispatch_pointer(
        &mut self,
        ui: &mut MapMakerUiState,
        current_builder: u32,
        current_slot: u32,
        current_page_generation: u64,
        node: u32,
        activate: bool,
    ) -> ClosePromptInputUpdate {
        if !self.is_current(ui, current_builder, current_slot, current_page_generation) {
            return ClosePromptInputUpdate::Ignored;
        }
        let Some(index) = self.rows.iter().position(|row| row.live_row.node == node) else {
            return ClosePromptInputUpdate::Ignored;
        };
        let Ok(index) = u32::try_from(index) else {
            return ClosePromptInputUpdate::Ignored;
        };
        self.input.dispatch(
            ui,
            if activate {
                NativeCommand::Select(index)
            } else {
                NativeCommand::Focus(index)
            },
        )
    }

    fn is_current(
        &self,
        ui: &MapMakerUiState,
        builder: u32,
        slot: u32,
        page_generation: u64,
    ) -> bool {
        builder == self.builder
            && slot == self.slot
            && page_generation == self.page_generation
            && ui.document_close_prompt_token() == Some(self.token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_mapmaker::document_close::{
        CloseDisposition, ClosePromptAction, ClosePromptChoiceId, ClosePromptUpdate, MapDocumentId,
    };
    use tsfp_mapmaker::ui::MapMakerUiCommand;
    use tsfp_ui_input::MenuIntent;
    use tsfp_ui_input::live_page::LiveText;
    use tsfp_ui_input::mapmaker_document_close::ClosePromptInputUpdate;
    use tsfp_ui_input::native_page::NativeCommand;

    const BUILDER: u32 = 0x91_1088; // Synthetic test fixture; no original popup builder is asserted.
    const SLOT: u32 = 5;
    const PAGE_GENERATION: u64 = 17;

    fn live_page(builder: u32, slot: u32) -> (LivePage, Vec<u32>) {
        let rows: Vec<_> = (10..13)
            .map(|node| LiveRow {
                node,
                node_type: 0,
                id: node as i32,
                flags: 0,
                text: LiveText::Id(node),
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
            })
            .collect();
        let cursor_nodes = rows.iter().map(|row| row.node).collect();
        (
            LivePage {
                slot,
                builder,
                state: 2,
                cursor_node: 10,
                rows,
                units: (6.4, 4.8),
            },
            cursor_nodes,
        )
    }

    fn fixture() -> (MapMakerUiState, ClosePromptToken, [ClosePromptChoice; 3]) {
        let mut ui = MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::BeginEditor);
        let token = ClosePromptToken::new(MapDocumentId::new(4), 9);
        assert!(ui.open_document_close_prompt(token));
        let choices = [
            ClosePromptChoice::new(ClosePromptChoiceId::new(10), ClosePromptAction::Save),
            ClosePromptChoice::new(ClosePromptChoiceId::new(11), ClosePromptAction::Discard),
            ClosePromptChoice::new(ClosePromptChoiceId::new(12), ClosePromptAction::Cancel),
        ];
        (ui, token, choices)
    }

    fn project<'a>(
        ui: &MapMakerUiState,
        live: &'a LivePage,
        cursor_nodes: &[u32],
        choices: &[ClosePromptChoice; 3],
    ) -> ClosePromptLivePage<'a> {
        ClosePromptLivePage::project(
            ui,
            live,
            BUILDER,
            SLOT,
            PAGE_GENERATION,
            cursor_nodes,
            &[(10, choices[0]), (11, choices[1]), (12, choices[2])],
            Some(choices[2].id()),
            false,
        )
        .unwrap()
    }

    #[test]
    fn explicit_open_result_qualifies_the_close_page_projection() {
        let mut ui = MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::BeginEditor);
        let token = ClosePromptToken::new(MapDocumentId::new(41), 7);
        let opened = begin_close_prompt(
            &mut ui,
            ClosePromptBinding {
                builder: BUILDER,
                slot: SLOT,
                page_generation: PAGE_GENERATION,
                token,
            },
        )
        .unwrap();
        let (live, cursor_nodes) = live_page(BUILDER, SLOT);
        let choices = [
            ClosePromptChoice::new(ClosePromptChoiceId::new(20), ClosePromptAction::Save),
            ClosePromptChoice::new(ClosePromptChoiceId::new(21), ClosePromptAction::Discard),
            ClosePromptChoice::new(ClosePromptChoiceId::new(22), ClosePromptAction::Cancel),
        ];
        let page = ClosePromptLivePage::project_opened(
            &ui,
            &live,
            opened,
            &cursor_nodes,
            &[(10, choices[0]), (11, choices[1]), (12, choices[2])],
            Some(choices[2].id()),
            false,
        )
        .unwrap();
        assert_eq!(page.token(), token);
        assert_eq!(page.rows()[1].live_row().node, 11);
        let (wrong_page, wrong_cursor) = live_page(BUILDER + 1, SLOT);
        assert!(
            ClosePromptLivePage::project_opened(
                &ui,
                &wrong_page,
                opened,
                &wrong_cursor,
                &[(10, choices[0]), (11, choices[1]), (12, choices[2])],
                Some(choices[2].id()),
                false,
            )
            .is_none()
        );
    }

    #[test]
    fn failed_atomic_projection_cancels_just_opened_transaction() {
        let mut ui = MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::BeginEditor);
        let token = ClosePromptToken::new(MapDocumentId::new(52), 3);
        let (live, cursor_nodes) = live_page(BUILDER + 1, SLOT);
        let choices = [
            ClosePromptChoice::new(ClosePromptChoiceId::new(30), ClosePromptAction::Save),
            ClosePromptChoice::new(ClosePromptChoiceId::new(31), ClosePromptAction::Discard),
            ClosePromptChoice::new(ClosePromptChoiceId::new(32), ClosePromptAction::Cancel),
        ];
        assert!(matches!(
            begin_and_project_close_prompt(
                &mut ui,
                &live,
                ClosePromptBinding {
                    builder: BUILDER,
                    slot: SLOT,
                    page_generation: PAGE_GENERATION,
                    token,
                },
                &cursor_nodes,
                &[(10, choices[0]), (11, choices[1]), (12, choices[2])],
                Some(choices[2].id()),
                false,
            ),
            Err(ClosePromptBindError::ProjectionRejected)
        ));
        assert_eq!(ui.document_close_prompt_token(), Some(token));
        assert_eq!(
            ui.document_close_prompt_phase(),
            Some(tsfp_mapmaker::document_close::ClosePromptPhase::Cancelled)
        );
    }

    #[test]
    fn keyboard_and_pointer_share_caller_bound_live_choices() {
        let (mut ui, token, choices) = fixture();
        let (live, cursor_nodes) = live_page(BUILDER, SLOT);
        let mut page = project(&ui, &live, &cursor_nodes, &choices);
        assert_eq!(page.rows()[1].live_row().text, LiveText::Id(11));

        let down = IntentSet::default().with(MenuIntent::Down);
        assert!(matches!(
            page.dispatch_frame(
                &mut ui,
                BUILDER,
                SLOT,
                PAGE_GENERATION,
                down,
                IntentSet::default(),
                100,
                &mut [0; 4],
            )
            .as_slice(),
            [ClosePromptInputUpdate::Focused(id)] if *id == choices[0].id()
        ));
        assert_eq!(
            page.dispatch_pointer(&mut ui, BUILDER, SLOT, PAGE_GENERATION, 12, true),
            ClosePromptInputUpdate::Prompt(ClosePromptUpdate::Cancelled(token))
        );

        let (mut ui, token, choices) = fixture();
        let (live, cursor_nodes) = live_page(BUILDER, SLOT);
        let mut page = project(&ui, &live, &cursor_nodes, &choices);
        assert!(matches!(
            page.dispatch_pointer(&mut ui, BUILDER, SLOT, PAGE_GENERATION, 10, true),
            ClosePromptInputUpdate::Prompt(ClosePromptUpdate::SaveRequested(request))
                if request.token == token
        ));
    }

    #[test]
    fn stale_page_and_ambiguous_rows_are_rejected() {
        let (mut ui, _, choices) = fixture();
        let (live, cursor_nodes) = live_page(BUILDER, SLOT);
        let mut page = project(&ui, &live, &cursor_nodes, &choices);
        assert_eq!(
            page.dispatch(
                &mut ui,
                BUILDER,
                SLOT,
                PAGE_GENERATION + 1,
                NativeCommand::Accept
            ),
            ClosePromptInputUpdate::Ignored
        );
        assert_eq!(
            page.dispatch_pointer(&mut ui, BUILDER, SLOT, PAGE_GENERATION, 99, true),
            ClosePromptInputUpdate::Ignored
        );

        assert!(
            ClosePromptLivePage::project(
                &ui,
                &live,
                BUILDER + 1,
                SLOT,
                PAGE_GENERATION,
                &cursor_nodes,
                &[(10, choices[0]), (11, choices[1]), (12, choices[2])],
                Some(choices[2].id()),
                false,
            )
            .is_none()
        );
        assert!(
            ClosePromptLivePage::project(
                &ui,
                &live,
                BUILDER,
                SLOT,
                PAGE_GENERATION,
                &cursor_nodes,
                &[(10, choices[0]), (10, choices[1]), (12, choices[2])],
                Some(choices[2].id()),
                false,
            )
            .is_none()
        );
        assert!(
            ClosePromptLivePage::project(
                &ui,
                &live,
                BUILDER,
                SLOT,
                PAGE_GENERATION,
                &[10, 12, 11],
                &[(10, choices[0]), (11, choices[1]), (12, choices[2])],
                Some(choices[2].id()),
                false,
            )
            .is_none()
        );
    }

    #[test]
    fn discard_is_a_typed_close_effect_not_a_synthetic_input_replay() {
        let (mut ui, token, choices) = fixture();
        let (live, cursor_nodes) = live_page(BUILDER, SLOT);
        let mut page = project(&ui, &live, &cursor_nodes, &choices);
        assert_eq!(
            page.dispatch_pointer(&mut ui, BUILDER, SLOT, PAGE_GENERATION, 11, true),
            ClosePromptInputUpdate::Prompt(ClosePromptUpdate::Closed {
                token,
                disposition: CloseDisposition::Discarded,
            })
        );
    }
}
