//! Renderer projection for the Rust-owned Assault phase assignment page.
//!
//! T1714 supplies the measured row order. The caller retains localization and
//! geometry; widget hits resolve to generation-qualified Rust actions.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId, WidgetKind};
use tsfp_mapmaker::assault_phase_assignment::{
    ASSAULT_PHASE_ASSIGNMENT_ACTIONS, AssaultPhaseAssignmentAction, AssaultPhaseAssignmentCommand,
    AssaultPhaseAssignmentTarget, AssaultPhaseAssignmentUpdate,
};
use tsfp_mapmaker::ui::{MapMakerUiSnapshot, MapMakerUiState};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct AssaultPhaseAssignmentWidgetLayout {
    /// `None` leaves headings and other runtime-owned widgets untouched.
    pub action: Option<AssaultPhaseAssignmentAction>,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssaultPhaseAssignmentLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub widgets: Vec<AssaultPhaseAssignmentWidgetLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultPhaseAssignmentBuildError {
    MissingState,
    ActionOrderMismatch,
    DuplicateWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WidgetBinding {
    widget: WidgetId,
    target: AssaultPhaseAssignmentTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssaultPhaseAssignmentPage {
    pub page: Page,
    bindings: Vec<WidgetBinding>,
}

impl AssaultPhaseAssignmentPage {
    pub fn target_for_widget(&self, widget: WidgetId) -> Option<AssaultPhaseAssignmentTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }

    pub fn focus_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> AssaultPhaseAssignmentUpdate {
        let Some(target) = self.target_for_widget(widget) else {
            return AssaultPhaseAssignmentUpdate::Ignored;
        };
        let can_focus = self.page.widgets.iter().any(|candidate| {
            candidate.id == widget
                && candidate.navigable
                && (candidate.enabled || candidate.focus_when_disabled)
        });
        if !can_focus {
            return AssaultPhaseAssignmentUpdate::Ignored;
        }
        state.dispatch_assault_phase_assignment(AssaultPhaseAssignmentCommand::Focus(target))
    }

    pub fn activate_widget(
        &self,
        state: &mut MapMakerUiState,
        widget: WidgetId,
    ) -> AssaultPhaseAssignmentUpdate {
        let Some(target) = self.target_for_widget(widget) else {
            return AssaultPhaseAssignmentUpdate::Ignored;
        };
        if !self
            .page
            .widgets
            .iter()
            .any(|candidate| candidate.id == widget && candidate.enabled)
        {
            return AssaultPhaseAssignmentUpdate::Ignored;
        }
        state.dispatch_assault_phase_assignment(AssaultPhaseAssignmentCommand::Activate(target))
    }
}

pub fn build_assault_phase_assignment_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    page_id: PageId,
    layout: AssaultPhaseAssignmentLayout,
) -> Result<AssaultPhaseAssignmentPage, AssaultPhaseAssignmentBuildError> {
    let assignment = snapshot
        .assault_phase_assignment
        .ok_or(AssaultPhaseAssignmentBuildError::MissingState)?;
    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut actions = Vec::new();
    let mut bindings = Vec::new();
    for item in layout.widgets {
        if page
            .widgets
            .iter()
            .any(|widget| widget.id == item.widget.id)
        {
            return Err(AssaultPhaseAssignmentBuildError::DuplicateWidget);
        }
        if let Some(action) = item.action {
            actions.push(action);
            bindings.push(WidgetBinding {
                widget: item.widget.id,
                target: assignment.target(action),
            });
        }
        page.widgets.push(item.widget);
    }
    if actions != ASSAULT_PHASE_ASSIGNMENT_ACTIONS {
        return Err(AssaultPhaseAssignmentBuildError::ActionOrderMismatch);
    }
    page.initial_focus = assignment.focused().and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.action() == focused)
            .map(|binding| binding.widget)
            .filter(|widget| {
                page.widgets.iter().any(|candidate| {
                    candidate.id == *widget
                        && candidate.navigable
                        && (candidate.enabled || candidate.focus_when_disabled)
                })
            })
    });
    Ok(AssaultPhaseAssignmentPage { page, bindings })
}

#[cfg(test)]
mod tests {
    use super::{
        AssaultPhaseAssignmentBuildError, AssaultPhaseAssignmentLayout,
        AssaultPhaseAssignmentWidgetLayout, build_assault_phase_assignment_page,
    };
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{PageId, TextId, TextRef, Widget, WidgetId, WidgetKind, menu_rect};
    use tsfp_mapmaker::assault_phase_assignment::{
        AssaultPhaseAssignmentAction, AssaultPhaseAssignmentGeneration,
        AssaultPhaseAssignmentState, AssaultPhaseAssignmentUpdate,
    };
    use tsfp_mapmaker::ui::MapMakerUiState;
    use tsfp_ui_renderer::Vec2;

    fn widget(id: u64) -> Widget {
        let mut widget = Widget::new(WidgetId(id), WidgetKind::Button);
        widget.rect = menu_rect(Vec2::new(20.0, id as f32 * 12.0), Vec2::new(220.0, 10.0));
        widget.text = TextRef::Id(TextId(id as u32));
        widget
    }

    fn state(generation: u64) -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        state.set_assault_phase_assignment(Some(AssaultPhaseAssignmentState::new(
            AssaultPhaseAssignmentGeneration::new(generation),
            false,
        )));
        state
    }

    fn layout(actions: &[Option<AssaultPhaseAssignmentAction>]) -> AssaultPhaseAssignmentLayout {
        AssaultPhaseAssignmentLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            widgets: actions
                .iter()
                .enumerate()
                .map(|(index, action)| AssaultPhaseAssignmentWidgetLayout {
                    action: *action,
                    widget: widget(index as u64 + 1),
                })
                .collect(),
        }
    }

    #[test]
    fn page_binds_measured_order_to_shared_generation_checked_state() {
        let mut state = state(9);
        let page = build_assault_phase_assignment_page(
            &state.snapshot(),
            PageId(7),
            layout(&[
                None,
                Some(AssaultPhaseAssignmentAction::SelectStartPoint),
                Some(AssaultPhaseAssignmentAction::Done),
            ]),
        )
        .unwrap();
        assert_eq!(page.page.initial_focus, None);
        assert_eq!(page.page.widgets.len(), 3);
        assert_eq!(
            page.focus_widget(&mut state, WidgetId(2)),
            AssaultPhaseAssignmentUpdate::Focused(AssaultPhaseAssignmentAction::SelectStartPoint)
        );
        assert_eq!(
            page.activate_widget(&mut state, WidgetId(2)),
            AssaultPhaseAssignmentUpdate::StartPointSelectionRequested
        );
        assert_eq!(
            page.activate_widget(&mut state, WidgetId(3)),
            AssaultPhaseAssignmentUpdate::BackRequested
        );
    }

    #[test]
    fn disabled_start_point_row_cannot_focus_or_activate() {
        let mut state = state(11);
        let mut start = widget(1);
        start.enabled = false;
        let page = build_assault_phase_assignment_page(
            &state.snapshot(),
            PageId(8),
            AssaultPhaseAssignmentLayout {
                widgets: vec![
                    AssaultPhaseAssignmentWidgetLayout {
                        action: Some(AssaultPhaseAssignmentAction::SelectStartPoint),
                        widget: start,
                    },
                    AssaultPhaseAssignmentWidgetLayout {
                        action: Some(AssaultPhaseAssignmentAction::Done),
                        widget: widget(2),
                    },
                ],
                ..layout(&[])
            },
        )
        .unwrap();

        // This fresh interaction state has no focused action yet. The projection
        // must not invent focus for Done while filtering the disabled row.
        assert_eq!(page.page.initial_focus, None);
        assert_eq!(
            page.focus_widget(&mut state, WidgetId(1)),
            AssaultPhaseAssignmentUpdate::Ignored
        );
        assert_eq!(
            page.activate_widget(&mut state, WidgetId(1)),
            AssaultPhaseAssignmentUpdate::Ignored
        );
        assert_eq!(
            page.activate_widget(&mut state, WidgetId(2)),
            AssaultPhaseAssignmentUpdate::BackRequested
        );
    }

    #[test]
    fn projection_rejects_order_duplicates_and_missing_shared_state() {
        let mut state = state(10);
        let snapshot = state.snapshot();
        assert_eq!(
            build_assault_phase_assignment_page(
                &snapshot,
                PageId(1),
                layout(&[
                    Some(AssaultPhaseAssignmentAction::Done),
                    Some(AssaultPhaseAssignmentAction::SelectStartPoint),
                ]),
            ),
            Err(AssaultPhaseAssignmentBuildError::ActionOrderMismatch)
        );
        let duplicate = AssaultPhaseAssignmentLayout {
            widgets: vec![
                AssaultPhaseAssignmentWidgetLayout {
                    action: Some(AssaultPhaseAssignmentAction::SelectStartPoint),
                    widget: widget(2),
                },
                AssaultPhaseAssignmentWidgetLayout {
                    action: Some(AssaultPhaseAssignmentAction::Done),
                    widget: widget(2),
                },
            ],
            ..layout(&[])
        };
        assert_eq!(
            build_assault_phase_assignment_page(&snapshot, PageId(1), duplicate),
            Err(AssaultPhaseAssignmentBuildError::DuplicateWidget)
        );
        state.set_assault_phase_assignment(None);
        assert_eq!(
            build_assault_phase_assignment_page(
                &state.snapshot(),
                PageId(1),
                layout(&[
                    Some(AssaultPhaseAssignmentAction::SelectStartPoint),
                    Some(AssaultPhaseAssignmentAction::Done),
                ]),
            ),
            Err(AssaultPhaseAssignmentBuildError::MissingState)
        );
    }

    #[test]
    fn stale_rendered_target_is_rejected_after_page_generation_changes() {
        let mut state = state(11);
        let page = build_assault_phase_assignment_page(
            &state.snapshot(),
            PageId(1),
            layout(&[
                Some(AssaultPhaseAssignmentAction::SelectStartPoint),
                Some(AssaultPhaseAssignmentAction::Done),
            ]),
        )
        .unwrap();
        state.set_assault_phase_assignment(Some(AssaultPhaseAssignmentState::new(
            AssaultPhaseAssignmentGeneration::new(12),
            false,
        )));
        assert_eq!(
            page.activate_widget(&mut state, WidgetId(1)),
            AssaultPhaseAssignmentUpdate::Ignored
        );
    }
}
