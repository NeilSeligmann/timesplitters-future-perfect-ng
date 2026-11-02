//! Rust-owned text editing seams for Map Description and Briefing pages.

use super::{EditorSurface, EditorSurfaceTarget, MapMakerPage, MapMakerUiState};
use crate::description::{
    DescriptionCommand, DescriptionError, DescriptionGeneration, DescriptionOutcome,
    DescriptionTarget, DescriptionView, MapDescriptionSession,
};

/// Text field plus live editor surface identity for a native description
/// renderer. Both field and surface tokens must accompany returned input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapDescriptionPageView {
    pub surface: EditorSurfaceTarget,
    pub field: DescriptionView,
}

/// Field identity dedicated to Briefing. Keeping this wrapper distinct from
/// `DescriptionTarget` prevents a Map Description input token from being
/// reused on Briefing even when the two editor surfaces share a generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BriefingTextTarget(DescriptionTarget);

impl BriefingTextTarget {
    pub const fn generation(self) -> DescriptionGeneration {
        self.0.generation()
    }
}

/// Renderer-facing field state whose token cannot be used by Map Description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefingTextView {
    pub surface: EditorSurfaceTarget,
    pub target: BriefingTextTarget,
    pub text: String,
    pub caret: usize,
    pub byte_limit: usize,
}

/// A typed native document update requested by Enter. The caller owns the
/// Briefing field mapping and persistence contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefingTextCommit {
    pub target: BriefingTextTarget,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BriefingTextOutcome {
    Changed,
    Unchanged,
    CommitRequested(BriefingTextCommit),
    Cancelled(BriefingTextTarget),
}

impl MapMakerUiState {
    /// Preserve the page-action identity after the action catalogue is cleared
    /// for entry to the native Map Description editor.
    pub fn active_map_description_target(&self) -> Option<EditorSurfaceTarget> {
        (self.page == MapMakerPage::Editor && self.editor_surface == EditorSurface::MapDescription)
            .then_some(self.active_map_description_target)
            .flatten()
    }

    /// Preserve the action identity for the runtime-supplied Briefing page.
    pub fn active_briefing_target(&self) -> Option<EditorSurfaceTarget> {
        (self.page == MapMakerPage::Editor && self.editor_surface == EditorSurface::Briefing)
            .then_some(self.active_briefing_target)
            .flatten()
    }

    pub fn map_description_session(&self) -> Option<&MapDescriptionSession> {
        self.active_map_description_target()
            .and(self.map_description_session.as_ref())
    }

    /// Return the current text/caret projection with its page and field
    /// generations. A renderer can hit-test into `field.caret` and submit a
    /// generation-qualified `DescriptionCommand::PlaceCaret`.
    pub fn map_description_view(&self) -> Option<MapDescriptionPageView> {
        let surface = self.active_map_description_target()?;
        let field = self.map_description_session()?.view();
        Some(MapDescriptionPageView { surface, field })
    }

    /// Install a Rust-owned description session for the active typed surface.
    /// Session identity derives from the page action generation, so rebuilding
    /// the page makes all earlier text commands stale.
    pub fn install_map_description_session(
        &mut self,
        surface: EditorSurfaceTarget,
        initial: &str,
        byte_limit: usize,
    ) -> Result<DescriptionTarget, DescriptionError> {
        if self.active_map_description_target() != Some(surface) {
            return Err(DescriptionError::StaleTarget);
        }
        let generation = DescriptionGeneration::new(surface.generation().get());
        let session = MapDescriptionSession::new(generation, initial, byte_limit)?;
        let target = session.target();
        self.map_description_session = Some(session);
        Ok(target)
    }

    pub fn apply_map_description_command(
        &mut self,
        surface: EditorSurfaceTarget,
        target: DescriptionTarget,
        command: DescriptionCommand,
    ) -> Result<DescriptionOutcome, DescriptionError> {
        if self.active_map_description_target() != Some(surface) {
            return Err(DescriptionError::StaleTarget);
        }
        self.map_description_session
            .as_mut()
            .ok_or(DescriptionError::StaleTarget)?
            .apply(target, command)
    }

    /// Return a separate Rust-owned text session for the active Briefing page.
    /// The caller supplies its initial text and limit; this model does not
    /// assume that Briefing shares Map Description persistence semantics.
    fn briefing_text_session(&self) -> Option<&MapDescriptionSession> {
        self.active_briefing_target()
            .and(self.briefing_text_session.as_ref())
    }

    pub fn briefing_text_view(&self) -> Option<BriefingTextView> {
        let surface = self.active_briefing_target()?;
        let field = self.briefing_text_session()?.view();
        Some(BriefingTextView {
            surface,
            target: BriefingTextTarget(field.target),
            text: field.text,
            caret: field.caret,
            byte_limit: field.byte_limit,
        })
    }

    pub fn install_briefing_text_session(
        &mut self,
        surface: EditorSurfaceTarget,
        initial: &str,
        byte_limit: usize,
    ) -> Result<BriefingTextTarget, DescriptionError> {
        if self.active_briefing_target() != Some(surface) {
            return Err(DescriptionError::StaleTarget);
        }
        let generation = DescriptionGeneration::new(surface.generation().get());
        let session = MapDescriptionSession::new(generation, initial, byte_limit)?;
        let target = BriefingTextTarget(session.target());
        self.briefing_text_session = Some(session);
        Ok(target)
    }

    pub fn apply_briefing_text_command(
        &mut self,
        surface: EditorSurfaceTarget,
        target: BriefingTextTarget,
        command: DescriptionCommand,
    ) -> Result<BriefingTextOutcome, DescriptionError> {
        if self.active_briefing_target() != Some(surface) {
            return Err(DescriptionError::StaleTarget);
        }
        let outcome = self
            .briefing_text_session
            .as_mut()
            .ok_or(DescriptionError::StaleTarget)?
            .apply(target.0, command)?;
        Ok(match outcome {
            DescriptionOutcome::Changed => BriefingTextOutcome::Changed,
            DescriptionOutcome::Unchanged => BriefingTextOutcome::Unchanged,
            DescriptionOutcome::CommitRequested(commit) => {
                BriefingTextOutcome::CommitRequested(BriefingTextCommit {
                    target,
                    text: commit.text,
                })
            }
            DescriptionOutcome::Cancelled(_) => BriefingTextOutcome::Cancelled(target),
        })
    }
}
