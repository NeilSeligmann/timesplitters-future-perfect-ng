//! Cleanup for a caller-opened dirty-close transaction that could not be
//! attached to a safe live page projection.

use crate::{
    document_close::{
        ClosePromptAction, ClosePromptCommand, ClosePromptPhase, ClosePromptToken,
        ClosePromptUpdate,
    },
    ui::MapMakerUiState,
};

impl MapMakerUiState {
    /// Cancel a close prompt that a caller opened but could not safely present.
    /// The token and phase checks ensure this cannot cancel a newer document
    /// request or an in-flight save.
    pub fn cancel_unpresented_document_close_prompt(&mut self, token: ClosePromptToken) -> bool {
        if self.document_close_prompt_token() != Some(token)
            || self.document_close_prompt_phase() != Some(ClosePromptPhase::AwaitingChoice)
        {
            return false;
        }
        matches!(
            self.apply_document_close_prompt(ClosePromptCommand::Choose {
                token,
                action: ClosePromptAction::Cancel,
            }),
            ClosePromptUpdate::Cancelled(cancelled) if cancelled == token
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        document_close::{
            ClosePromptAction, ClosePromptCommand, ClosePromptPhase, ClosePromptToken,
            MapDocumentId, SaveRequest, SaveResult,
        },
        ui::{MapMakerUiCommand, MapMakerUiState},
    };

    #[test]
    fn only_matching_unpresented_prompt_can_be_cancelled() {
        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::BeginEditor);
        let token = ClosePromptToken::new(MapDocumentId::new(31), 8);
        assert!(state.open_document_close_prompt(token));

        let stale = ClosePromptToken::new(MapDocumentId::new(31), 7);
        assert!(!state.cancel_unpresented_document_close_prompt(stale));
        assert_eq!(
            state.document_close_prompt_phase(),
            Some(ClosePromptPhase::AwaitingChoice)
        );
        assert!(state.cancel_unpresented_document_close_prompt(token));
        assert_eq!(
            state.document_close_prompt_phase(),
            Some(ClosePromptPhase::Cancelled)
        );
        assert!(
            state.open_document_close_prompt(ClosePromptToken::new(MapDocumentId::new(32), 1,))
        );
    }

    #[test]
    fn in_flight_save_cannot_be_cancelled_as_unpresented() {
        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::BeginEditor);
        let token = ClosePromptToken::new(MapDocumentId::new(41), 2);
        assert!(state.open_document_close_prompt(token));
        assert!(matches!(
            state.apply_document_close_prompt(ClosePromptCommand::Choose {
                token,
                action: ClosePromptAction::Save,
            }),
            crate::document_close::ClosePromptUpdate::SaveRequested(SaveRequest { .. })
        ));

        assert!(!state.cancel_unpresented_document_close_prompt(token));
        assert!(matches!(
            state.document_close_prompt_phase(),
            Some(ClosePromptPhase::Saving(_))
        ));
        assert_eq!(
            state.apply_document_close_prompt(ClosePromptCommand::SaveCompleted {
                token,
                request: match state.document_close_prompt_phase() {
                    Some(ClosePromptPhase::Saving(request)) => request,
                    _ => unreachable!(),
                },
                result: SaveResult::Failed,
            }),
            crate::document_close::ClosePromptUpdate::SaveFailed(token)
        );
    }
}
