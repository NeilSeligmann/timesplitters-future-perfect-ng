//! Renderer projection for the Rust-owned Map Maker dirty-document close prompt.
//!
//! Prompt text, row order, geometry, widget IDs, and Back policy remain
//! caller-supplied because the REA contract does not establish the original
//! prompt rows or callback mapping.

use crate::model::{Page, WidgetId};
use tsfp_mapmaker::{
    document_close::{ClosePromptAction, ClosePromptChoice, ClosePromptChoiceId, ClosePromptPhase},
    ui::{MapMakerPage, MapMakerUiSnapshot},
};

/// One caller-supplied interactive widget bound to a typed close choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosePromptChoiceWidget {
    pub choice: ClosePromptChoice,
    pub widget: WidgetId,
}

/// Runtime page contents and bindings from its measured widgets to opaque
/// Rust choice identities. Order is retained exactly as supplied.
#[derive(Clone, Debug, PartialEq)]
pub struct ClosePromptLayout {
    pub page: Page,
    pub choices: Vec<ClosePromptChoiceWidget>,
    pub back_choice: Option<ClosePromptChoiceId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePromptBuildError {
    WrongPage,
    MissingPrompt,
    PromptNotReady,
    EmptyChoices,
    DuplicateChoice,
    DuplicateWidget,
    MissingWidget,
    NonInteractiveWidget,
    BackChoiceMissing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClosePromptWidgetBinding {
    pub widget: WidgetId,
    pub choice: ClosePromptChoiceId,
    pub action: ClosePromptAction,
}

/// Renderer-facing modal page and its identity-preserving input routes.
#[derive(Clone, Debug, PartialEq)]
pub struct ClosePromptPage {
    pub page: Page,
    bindings: Vec<ClosePromptWidgetBinding>,
    back_choice: Option<ClosePromptChoiceId>,
}

impl ClosePromptPage {
    pub fn bindings(&self) -> &[ClosePromptWidgetBinding] {
        &self.bindings
    }

    pub const fn back_choice(&self) -> Option<ClosePromptChoiceId> {
        self.back_choice
    }

    pub fn binding_for_widget(&self, widget: WidgetId) -> Option<ClosePromptWidgetBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.widget == widget)
            .copied()
    }

    pub fn action_for_widget(&self, widget: WidgetId) -> Option<ClosePromptAction> {
        self.binding_for_widget(widget)
            .map(|binding| binding.action)
    }

    pub fn choice_for_widget(&self, widget: WidgetId) -> Option<ClosePromptChoiceId> {
        self.binding_for_widget(widget)
            .map(|binding| binding.choice)
    }
}

/// Validate a close prompt and preserve its caller-owned page data. The
/// builder does not assign text, focus order, wrap behavior, or Back action.
pub fn build_close_prompt_page(
    snapshot: &MapMakerUiSnapshot<'_>,
    layout: ClosePromptLayout,
) -> Result<ClosePromptPage, ClosePromptBuildError> {
    if snapshot.page != MapMakerPage::Editor {
        return Err(ClosePromptBuildError::WrongPage);
    }
    let Some(prompt) = snapshot.document_close_prompt else {
        return Err(ClosePromptBuildError::MissingPrompt);
    };
    if prompt.phase() != ClosePromptPhase::AwaitingChoice {
        return Err(ClosePromptBuildError::PromptNotReady);
    }
    if layout.choices.is_empty() {
        return Err(ClosePromptBuildError::EmptyChoices);
    }

    let mut bindings = Vec::with_capacity(layout.choices.len());
    for (index, row) in layout.choices.iter().enumerate() {
        if layout.choices[..index]
            .iter()
            .any(|prior| prior.choice.id() == row.choice.id())
        {
            return Err(ClosePromptBuildError::DuplicateChoice);
        }
        if layout.choices[..index]
            .iter()
            .any(|prior| prior.widget == row.widget)
        {
            return Err(ClosePromptBuildError::DuplicateWidget);
        }
        let mut matching_widgets = layout
            .page
            .widgets
            .iter()
            .filter(|widget| widget.id == row.widget);
        let Some(widget) = matching_widgets.next() else {
            return Err(ClosePromptBuildError::MissingWidget);
        };
        if matching_widgets.next().is_some() {
            return Err(ClosePromptBuildError::DuplicateWidget);
        }
        if !widget.is_focusable() {
            return Err(ClosePromptBuildError::NonInteractiveWidget);
        }
        bindings.push(ClosePromptWidgetBinding {
            widget: row.widget,
            choice: row.choice.id(),
            action: row.choice.action(),
        });
    }
    if layout
        .back_choice
        .is_some_and(|id| !bindings.iter().any(|binding| binding.choice == id))
    {
        return Err(ClosePromptBuildError::BackChoiceMissing);
    }

    Ok(ClosePromptPage {
        page: layout.page,
        bindings,
        back_choice: layout.back_choice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PageId, Widget, WidgetKind};
    use tsfp_mapmaker::{
        document_close::{ClosePromptToken, MapDocumentId},
        ui::MapMakerUiCommand,
    };

    fn owned_fixture() -> (tsfp_mapmaker::ui::MapMakerUiState, ClosePromptLayout) {
        let mut ui = tsfp_mapmaker::ui::MapMakerUiState::default();
        ui.dispatch(MapMakerUiCommand::BeginEditor);
        assert!(ui.open_document_close_prompt(ClosePromptToken::new(MapDocumentId::new(7), 3,)));

        let save_widget = WidgetId(100);
        let cancel_widget = WidgetId(101);
        let choices = vec![
            ClosePromptChoiceWidget {
                choice: ClosePromptChoice::new(
                    ClosePromptChoiceId::new(10),
                    ClosePromptAction::Save,
                ),
                widget: save_widget,
            },
            ClosePromptChoiceWidget {
                choice: ClosePromptChoice::new(
                    ClosePromptChoiceId::new(11),
                    ClosePromptAction::Cancel,
                ),
                widget: cancel_widget,
            },
        ];
        let mut page = Page::new(PageId(88), 4);
        page.widgets
            .push(Widget::new(save_widget, WidgetKind::Button));
        page.widgets
            .push(Widget::new(cancel_widget, WidgetKind::Button));
        (
            ui,
            ClosePromptLayout {
                page,
                choices,
                back_choice: Some(ClosePromptChoiceId::new(11)),
            },
        )
    }

    #[test]
    fn preserves_caller_page_and_binds_widget_to_choice() {
        let (ui, layout) = owned_fixture();
        let page = build_close_prompt_page(&ui.snapshot(), layout).unwrap();
        assert_eq!(page.page.id, PageId(88));
        assert_eq!(page.bindings().len(), 2);
        assert_eq!(
            page.action_for_widget(WidgetId(100)),
            Some(ClosePromptAction::Save)
        );
        assert_eq!(
            page.choice_for_widget(WidgetId(101)),
            Some(ClosePromptChoiceId::new(11))
        );
        assert_eq!(page.back_choice(), Some(ClosePromptChoiceId::new(11)));
    }

    #[test]
    fn rejects_stale_or_ambiguous_choice_bindings() {
        let (ui, mut layout) = owned_fixture();
        layout.choices[1].choice = layout.choices[0].choice;
        assert_eq!(
            build_close_prompt_page(&ui.snapshot(), layout),
            Err(ClosePromptBuildError::DuplicateChoice)
        );

        let (ui, mut layout) = owned_fixture();
        layout.choices[1].widget = WidgetId(999);
        assert_eq!(
            build_close_prompt_page(&ui.snapshot(), layout),
            Err(ClosePromptBuildError::MissingWidget)
        );
    }

    #[test]
    fn rejects_noninteractive_prompt_rows_and_unbound_back_actions() {
        let (ui, mut layout) = owned_fixture();
        layout.page.widgets[0] = Widget::new(WidgetId(100), WidgetKind::Label);
        layout.page.widgets[0].navigable = false;
        assert_eq!(
            build_close_prompt_page(&ui.snapshot(), layout),
            Err(ClosePromptBuildError::NonInteractiveWidget)
        );

        let (ui, mut layout) = owned_fixture();
        layout.back_choice = Some(ClosePromptChoiceId::new(55));
        assert_eq!(
            build_close_prompt_page(&ui.snapshot(), layout),
            Err(ClosePromptBuildError::BackChoiceMissing)
        );
    }
}
