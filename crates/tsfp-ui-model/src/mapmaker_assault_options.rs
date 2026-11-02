//! Rust renderer projection for the Map Maker Assault Options page.
//!
//! The registry still supplies localization and layout. The projection only
//! accepts the measured four-action order and binds those rows to generation
//! checked Rust state.

use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId, WidgetKind};
use tsfp_mapmaker::assault_options::{
    ASSAULT_OPTIONS_ORDER, AssaultOption, AssaultOptionTarget, AssaultOptionsCommand,
    AssaultOptionsState, AssaultOptionsUpdate, AssaultParticipantChoiceId, AssaultParticipantField,
    AssaultParticipantRuntimeOptionId,
};
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub struct AssaultOptionsWidgetLayout {
    /// `None` retains a runtime heading or other non-action widget.
    pub action: Option<AssaultOption>,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssaultOptionsLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub widgets: Vec<AssaultOptionsWidgetLayout>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssaultOptionsBuildError {
    ActionOrderMismatch,
    DuplicateWidget,
    ParticipantChoicesMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WidgetBinding {
    widget: WidgetId,
    target: AssaultOptionTarget,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssaultOptionsPage {
    pub page: Page,
    bindings: Vec<WidgetBinding>,
}

impl AssaultOptionsPage {
    pub fn focus_widget(
        &self,
        state: &mut AssaultOptionsState,
        widget: WidgetId,
    ) -> AssaultOptionsUpdate {
        self.target_for_widget(widget)
            .map_or(AssaultOptionsUpdate::Ignored, |target| {
                state.dispatch(AssaultOptionsCommand::Focus(target))
            })
    }

    pub fn activate_widget(
        &self,
        state: &mut AssaultOptionsState,
        widget: WidgetId,
    ) -> AssaultOptionsUpdate {
        let Some(target) = self.target_for_widget(widget) else {
            return AssaultOptionsUpdate::Ignored;
        };
        let command = if target.option().participant_field().is_some() {
            AssaultOptionsCommand::Focus(target)
        } else {
            AssaultOptionsCommand::Activate(target)
        };
        state.dispatch(command)
    }

    pub fn choose_participant_widget(
        &self,
        state: &mut AssaultOptionsState,
        widget: WidgetId,
        choice: AssaultParticipantChoiceId,
    ) -> AssaultOptionsUpdate {
        let Some(field) = self
            .bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .and_then(|binding| binding.target.option().participant_field())
        else {
            return AssaultOptionsUpdate::Ignored;
        };
        state
            .participant_choice_target(field, choice)
            .map_or(AssaultOptionsUpdate::Ignored, |target| {
                state.dispatch(AssaultOptionsCommand::SelectParticipantChoice(target))
            })
    }

    fn target_for_widget(&self, widget: WidgetId) -> Option<AssaultOptionTarget> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .map(|binding| binding.target)
    }
}

pub fn build_assault_options_page(
    state: &AssaultOptionsState,
    page_id: PageId,
    layout: AssaultOptionsLayout,
) -> Result<AssaultOptionsPage, AssaultOptionsBuildError> {
    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    let mut actions = Vec::new();
    let mut bindings = Vec::new();
    for mut item in layout.widgets {
        if page
            .widgets
            .iter()
            .any(|widget| widget.id == item.widget.id)
        {
            return Err(AssaultOptionsBuildError::DuplicateWidget);
        }
        if let Some(action) = item.action {
            actions.push(action);
            if let Some(field) = action.participant_field() {
                let choices = state.participant_choices(field);
                let selected = state.selected_participant_choice(field);
                if item.widget.kind != WidgetKind::Cycler
                    || choices.len() != item.widget.options.len()
                    || choices
                        .iter()
                        .zip(&item.widget.options)
                        .any(|(choice, option)| choice.enabled != option.enabled)
                {
                    return Err(AssaultOptionsBuildError::ParticipantChoicesMismatch);
                }
                let Some(selected) = selected.map(|choice| choice.ordinal() as usize) else {
                    return Err(AssaultOptionsBuildError::ParticipantChoicesMismatch);
                };
                if selected >= choices.len() {
                    return Err(AssaultOptionsBuildError::ParticipantChoicesMismatch);
                }
                item.widget.selected = selected;
            }
            bindings.push(WidgetBinding {
                widget: item.widget.id,
                target: state.target(action),
            });
        }
        page.widgets.push(item.widget);
    }
    if actions != ASSAULT_OPTIONS_ORDER {
        return Err(AssaultOptionsBuildError::ActionOrderMismatch);
    }
    page.initial_focus = state.focused().and_then(|focused| {
        bindings
            .iter()
            .find(|binding| binding.target.option() == focused)
            .map(|binding| binding.widget)
    });
    Ok(AssaultOptionsPage { page, bindings })
}

#[cfg(test)]
mod tests {
    use super::{
        AssaultOptionsBuildError, AssaultOptionsLayout, AssaultOptionsWidgetLayout,
        build_assault_options_page,
    };
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{
        PageId, TextId, TextRef, UiOption, Widget, WidgetId, WidgetKind, menu_rect,
    };
    use tsfp_mapmaker::assault_options::{
        ASSAULT_OPTIONS_ORDER, AssaultOption, AssaultOptionsCommand, AssaultOptionsGeneration,
        AssaultOptionsState, AssaultOptionsUpdate, AssaultParticipantChoice,
        AssaultParticipantChoiceId, AssaultParticipantField, AssaultParticipantRuntimeOptionId,
    };
    use tsfp_ui_renderer::Vec2;

    fn widget(id: u64) -> Widget {
        let mut widget = Widget::new(WidgetId(id), WidgetKind::Button);
        widget.rect = menu_rect(Vec2::new(20.0, id as f32 * 12.0), Vec2::new(220.0, 10.0));
        widget.text = TextRef::Id(TextId(id as u32));
        widget
    }

    fn layout(actions: &[AssaultOption]) -> AssaultOptionsLayout {
        AssaultOptionsLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            widgets: actions
                .iter()
                .enumerate()
                .map(|(index, action)| {
                    let mut widget = widget(index as u64 + 1);
                    if action.participant_field().is_some() {
                        widget.kind = WidgetKind::Cycler;
                        widget.options = [10, 11]
                            .into_iter()
                            .map(|id| UiOption {
                                id,
                                text: TextRef::Id(TextId(id)),
                                enabled: true,
                            })
                            .collect();
                    }
                    AssaultOptionsWidgetLayout {
                        action: Some(*action),
                        widget,
                    }
                })
                .collect(),
        }
    }

    fn state() -> AssaultOptionsState {
        let mut state = AssaultOptionsState::new(AssaultOptionsGeneration::new(9));
        let choices: Vec<AssaultParticipantChoice> = (0..2)
            .map(|index| AssaultParticipantChoice {
                id: AssaultParticipantChoiceId::new(index),
                runtime_option_id: AssaultParticipantRuntimeOptionId::new(10 + index),
                enabled: true,
            })
            .collect();
        for field in [
            AssaultParticipantField::Attackers,
            AssaultParticipantField::Defenders,
        ] {
            assert!(state.install_participant_choices(
                field,
                choices.clone(),
                AssaultParticipantChoiceId::new(0),
            ));
        }
        state
    }

    #[test]
    fn renderer_projection_preserves_rows_and_dispatches_owned_actions() {
        let mut state = state();
        let target = state.target(AssaultOption::Defenders);
        assert_eq!(
            state.dispatch(AssaultOptionsCommand::Focus(target)),
            AssaultOptionsUpdate::Focused(AssaultOption::Defenders)
        );
        let projection =
            build_assault_options_page(&state, PageId(0x313FE0), layout(&ASSAULT_OPTIONS_ORDER))
                .unwrap();
        assert_eq!(projection.page.widget_type, 1);
        assert_eq!(projection.page.widgets.len(), 4);
        assert_eq!(projection.page.initial_focus, Some(WidgetId(3)));
        assert_eq!(
            projection.activate_widget(&mut state, WidgetId(4)),
            AssaultOptionsUpdate::ActionRequested(AssaultOption::StartAssault)
        );
        assert_eq!(projection.page.widgets[1].kind, WidgetKind::Cycler);
        assert_eq!(projection.page.widgets[1].selected, 0);
        assert_eq!(
            projection.activate_widget(&mut state, WidgetId(2)),
            AssaultOptionsUpdate::Focused(AssaultOption::Attackers)
        );
        assert_eq!(
            projection.choose_participant_widget(
                &mut state,
                WidgetId(2),
                AssaultParticipantChoiceId::new(1),
            ),
            AssaultOptionsUpdate::ParticipantChoiceSelected {
                field: AssaultParticipantField::Attackers,
                choice: AssaultParticipantChoiceId::new(1),
                runtime_option_id: AssaultParticipantRuntimeOptionId::new(11),
            }
        );
        let updated =
            build_assault_options_page(&state, PageId(0x313FE0), layout(&ASSAULT_OPTIONS_ORDER))
                .unwrap();
        assert_eq!(updated.page.widgets[1].selected, 1);
    }

    #[test]
    fn renderer_projection_rejects_reordered_or_missing_actions() {
        let state = state();
        let reordered = [
            AssaultOption::Attackers,
            AssaultOption::PreviewTeam,
            AssaultOption::Defenders,
            AssaultOption::StartAssault,
        ];
        assert_eq!(
            build_assault_options_page(&state, PageId(1), layout(&reordered)),
            Err(AssaultOptionsBuildError::ActionOrderMismatch)
        );
        assert_eq!(
            build_assault_options_page(&state, PageId(1), layout(&ASSAULT_OPTIONS_ORDER[..3])),
            Err(AssaultOptionsBuildError::ActionOrderMismatch)
        );
    }

    #[test]
    fn renderer_projection_rejects_unmatched_participant_choices() {
        let state = state();
        let mut page_layout = layout(&ASSAULT_OPTIONS_ORDER);
        page_layout.widgets[1].widget.options.pop();
        assert_eq!(
            build_assault_options_page(&state, PageId(1), page_layout),
            Err(AssaultOptionsBuildError::ParticipantChoicesMismatch)
        );
    }
}
