//! Rust-owned dirty-document close transaction (T2408).
//!
//! T1689 leaves the original deferred close/save callback lifetime unresolved.
//! Keep only typed document identity and generation in this state; a caller
//! must resolve save results through the matching token and request ID.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MapDocumentId(u64);

impl MapDocumentId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClosePromptToken {
    document: MapDocumentId,
    generation: u64,
}

impl ClosePromptToken {
    pub const fn new(document: MapDocumentId, generation: u64) -> Self {
        Self {
            document,
            generation,
        }
    }

    pub const fn document(self) -> MapDocumentId {
        self.document
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SaveRequestId(u64);

impl SaveRequestId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptAction {
    Save,
    Discard,
    Cancel,
}

/// Opaque identity for one caller-supplied choice row on the close prompt.
/// It is independent of renderer widget IDs and original guest descriptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClosePromptChoiceId(u64);

impl ClosePromptChoiceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A row identity and the typed decision it requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosePromptChoice {
    id: ClosePromptChoiceId,
    action: ClosePromptAction,
}

impl ClosePromptChoice {
    pub const fn new(id: ClosePromptChoiceId, action: ClosePromptAction) -> Self {
        Self { id, action }
    }

    pub const fn id(self) -> ClosePromptChoiceId {
        self.id
    }

    pub const fn action(self) -> ClosePromptAction {
        self.action
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveResult {
    Committed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseDisposition {
    Saved,
    Discarded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptCommand {
    Choose {
        token: ClosePromptToken,
        action: ClosePromptAction,
    },
    SaveCompleted {
        token: ClosePromptToken,
        request: SaveRequestId,
        result: SaveResult,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveRequest {
    pub token: ClosePromptToken,
    pub id: SaveRequestId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptUpdate {
    SaveRequested(SaveRequest),
    Closed {
        token: ClosePromptToken,
        disposition: CloseDisposition,
    },
    Cancelled(ClosePromptToken),
    SaveFailed(ClosePromptToken),
    Ignored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptPhase {
    AwaitingChoice,
    Saving(SaveRequestId),
    Closed(CloseDisposition),
    Cancelled,
}

/// One Rust-owned close prompt. The caller creates a fresh token whenever the
/// document session changes and retains the document until this state closes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosePromptState {
    token: ClosePromptToken,
    phase: ClosePromptPhase,
    next_save_request: u64,
}

impl ClosePromptState {
    pub const fn new(token: ClosePromptToken) -> Self {
        Self {
            token,
            phase: ClosePromptPhase::AwaitingChoice,
            next_save_request: 0,
        }
    }

    pub const fn token(&self) -> ClosePromptToken {
        self.token
    }

    pub const fn phase(&self) -> ClosePromptPhase {
        self.phase
    }

    pub fn dispatch(&mut self, command: ClosePromptCommand) -> ClosePromptUpdate {
        match command {
            ClosePromptCommand::Choose { token, action }
                if token == self.token && self.phase == ClosePromptPhase::AwaitingChoice =>
            {
                match action {
                    ClosePromptAction::Save => {
                        let Some(next_request) = self.next_save_request.checked_add(1) else {
                            return ClosePromptUpdate::Ignored;
                        };
                        self.next_save_request = next_request;
                        let id = SaveRequestId(self.next_save_request);
                        self.phase = ClosePromptPhase::Saving(id);
                        ClosePromptUpdate::SaveRequested(SaveRequest { token, id })
                    }
                    ClosePromptAction::Discard => {
                        self.phase = ClosePromptPhase::Closed(CloseDisposition::Discarded);
                        ClosePromptUpdate::Closed {
                            token,
                            disposition: CloseDisposition::Discarded,
                        }
                    }
                    ClosePromptAction::Cancel => {
                        self.phase = ClosePromptPhase::Cancelled;
                        ClosePromptUpdate::Cancelled(token)
                    }
                }
            }
            ClosePromptCommand::SaveCompleted {
                token,
                request,
                result,
            } if token == self.token && self.phase == ClosePromptPhase::Saving(request) => {
                match result {
                    SaveResult::Committed => {
                        self.phase = ClosePromptPhase::Closed(CloseDisposition::Saved);
                        ClosePromptUpdate::Closed {
                            token,
                            disposition: CloseDisposition::Saved,
                        }
                    }
                    SaveResult::Failed => {
                        self.phase = ClosePromptPhase::AwaitingChoice;
                        ClosePromptUpdate::SaveFailed(token)
                    }
                }
            }
            _ => ClosePromptUpdate::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(document: u64, generation: u64) -> ClosePromptToken {
        ClosePromptToken::new(MapDocumentId::new(document), generation)
    }

    #[test]
    fn save_keeps_document_open_until_matching_commit_receipt() {
        let token = token(7, 11);
        let mut state = ClosePromptState::new(token);
        let ClosePromptUpdate::SaveRequested(request) =
            state.dispatch(ClosePromptCommand::Choose {
                token,
                action: ClosePromptAction::Save,
            })
        else {
            panic!("save should request a typed storage operation");
        };
        assert_eq!(state.phase(), ClosePromptPhase::Saving(request.id));
        assert_eq!(
            state.dispatch(ClosePromptCommand::SaveCompleted {
                token,
                request: request.id,
                result: SaveResult::Committed,
            }),
            ClosePromptUpdate::Closed {
                token,
                disposition: CloseDisposition::Saved,
            }
        );
    }

    #[test]
    fn cancel_retains_the_document_and_finishes_the_prompt() {
        let token = token(2, 3);
        let mut state = ClosePromptState::new(token);
        assert_eq!(
            state.dispatch(ClosePromptCommand::Choose {
                token,
                action: ClosePromptAction::Cancel,
            }),
            ClosePromptUpdate::Cancelled(token)
        );
        assert_eq!(state.phase(), ClosePromptPhase::Cancelled);
        assert_eq!(
            state.dispatch(ClosePromptCommand::Choose {
                token,
                action: ClosePromptAction::Discard,
            }),
            ClosePromptUpdate::Ignored
        );
    }

    #[test]
    fn discard_closes_without_starting_storage() {
        let token = token(4, 5);
        let mut state = ClosePromptState::new(token);
        assert_eq!(
            state.dispatch(ClosePromptCommand::Choose {
                token,
                action: ClosePromptAction::Discard,
            }),
            ClosePromptUpdate::Closed {
                token,
                disposition: CloseDisposition::Discarded,
            }
        );
    }

    #[test]
    fn stale_document_generation_cannot_consume_a_save_receipt() {
        let active_token = token(9, 3);
        let stale = token(9, 2);
        let mut state = ClosePromptState::new(active_token);
        let ClosePromptUpdate::SaveRequested(request) =
            state.dispatch(ClosePromptCommand::Choose {
                token: active_token,
                action: ClosePromptAction::Save,
            })
        else {
            panic!("save should request a typed storage operation");
        };
        assert_eq!(
            state.dispatch(ClosePromptCommand::SaveCompleted {
                token: stale,
                request: request.id,
                result: SaveResult::Committed,
            }),
            ClosePromptUpdate::Ignored
        );
        assert_eq!(state.phase(), ClosePromptPhase::Saving(request.id));
    }

    #[test]
    fn failed_save_allows_retry_and_rejects_the_old_request() {
        let token = token(10, 7);
        let mut state = ClosePromptState::new(token);
        let ClosePromptUpdate::SaveRequested(first) = state.dispatch(ClosePromptCommand::Choose {
            token,
            action: ClosePromptAction::Save,
        }) else {
            panic!("first save should be requested");
        };
        assert_eq!(
            state.dispatch(ClosePromptCommand::SaveCompleted {
                token,
                request: first.id,
                result: SaveResult::Failed,
            }),
            ClosePromptUpdate::SaveFailed(token)
        );
        let ClosePromptUpdate::SaveRequested(retry) = state.dispatch(ClosePromptCommand::Choose {
            token,
            action: ClosePromptAction::Save,
        }) else {
            panic!("failed save should allow retry");
        };
        assert_ne!(first.id, retry.id);
        assert_eq!(
            state.dispatch(ClosePromptCommand::SaveCompleted {
                token,
                request: first.id,
                result: SaveResult::Committed,
            }),
            ClosePromptUpdate::Ignored
        );
        assert_eq!(state.phase(), ClosePromptPhase::Saving(retry.id));
    }
}
