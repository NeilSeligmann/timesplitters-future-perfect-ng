//! `PageBuilder`: the recording interface the original page builders run against.
//! The original toolkit makes pages with a handful of calls (push page, add row, add option,
//! pop page). A recorder implementing this trait turns one run of a builder into a `Page`.

use crate::focus::{NavLayout, Wrap};
use crate::model::{
    HandlerRef, HandlerSlot, Page, PageId, Transition, UiModel, UiOption, Widget, WidgetId,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuilderError {
    /// A call needs an open page.
    NoPage,
    /// `record_option` before any row of the current page.
    NoRow,
    /// `record_row` with an id already used on the current page.
    DuplicateWidget(WidgetId),
    /// `finish` with pages still open.
    UnbalancedPages(usize),
    /// Nesting beyond the original build stack.
    TooDeep,
}

/// Maximum nesting of the original build context stack.
pub const MAX_BUILD_DEPTH: usize = 5;

pub trait PageBuilder {
    /// Start a page. Pages nest like the original build context stack.
    fn push_page(&mut self, id: PageId, widget_type: u32) -> Result<(), BuilderError>;
    /// Finish the innermost page and add it to the model. A later recording of the same
    /// page id replaces the earlier one (dynamic pages are rebuilt per visit).
    fn pop_page(&mut self) -> Result<(), BuilderError>;
    fn record_row(&mut self, widget: Widget) -> Result<(), BuilderError>;
    /// Add an option to the most recently recorded row; it becomes a cycler if it was a button.
    fn record_option(&mut self, option: UiOption) -> Result<(), BuilderError>;
    fn set_handler(&mut self, slot: HandlerSlot, handler: HandlerRef) -> Result<(), BuilderError>;
    fn set_navigation(&mut self, nav: NavLayout, wrap: Wrap) -> Result<(), BuilderError>;
    fn set_initial_focus(&mut self, widget: WidgetId) -> Result<(), BuilderError>;
    fn set_transitions(&mut self, enter: Transition, exit: Transition) -> Result<(), BuilderError>;
}

/// Recorder that produces a `UiModel`.
#[derive(Debug, Default)]
pub struct ModelRecorder {
    stack: Vec<Page>,
    model: UiModel,
}

impl ModelRecorder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn finish(self) -> Result<UiModel, BuilderError> {
        if self.stack.is_empty() {
            Ok(self.model)
        } else {
            Err(BuilderError::UnbalancedPages(self.stack.len()))
        }
    }
    fn top(&mut self) -> Result<&mut Page, BuilderError> {
        self.stack.last_mut().ok_or(BuilderError::NoPage)
    }
}

impl PageBuilder for ModelRecorder {
    fn push_page(&mut self, id: PageId, widget_type: u32) -> Result<(), BuilderError> {
        if self.stack.len() >= MAX_BUILD_DEPTH {
            return Err(BuilderError::TooDeep);
        }
        self.stack.push(Page::new(id, widget_type));
        Ok(())
    }
    fn pop_page(&mut self) -> Result<(), BuilderError> {
        let page = self.stack.pop().ok_or(BuilderError::NoPage)?;
        self.model.pages.insert(page.id, page);
        Ok(())
    }
    fn record_row(&mut self, widget: Widget) -> Result<(), BuilderError> {
        let page = self.top()?;
        if page.widget(widget.id).is_some() {
            return Err(BuilderError::DuplicateWidget(widget.id));
        }
        page.widgets.push(widget);
        Ok(())
    }
    fn record_option(&mut self, option: UiOption) -> Result<(), BuilderError> {
        let row = self.top()?.widgets.last_mut().ok_or(BuilderError::NoRow)?;
        row.options.push(option);
        if matches!(
            row.kind,
            crate::model::WidgetKind::Button | crate::model::WidgetKind::Label
        ) {
            row.kind = crate::model::WidgetKind::Cycler;
        }
        Ok(())
    }
    fn set_handler(&mut self, slot: HandlerSlot, handler: HandlerRef) -> Result<(), BuilderError> {
        *self.top()?.handlers.slot_mut(slot) = Some(handler);
        Ok(())
    }
    fn set_navigation(&mut self, nav: NavLayout, wrap: Wrap) -> Result<(), BuilderError> {
        let page = self.top()?;
        page.nav = nav;
        page.wrap = wrap;
        Ok(())
    }
    fn set_initial_focus(&mut self, widget: WidgetId) -> Result<(), BuilderError> {
        self.top()?.initial_focus = Some(widget);
        Ok(())
    }
    fn set_transitions(&mut self, enter: Transition, exit: Transition) -> Result<(), BuilderError> {
        let page = self.top()?;
        page.enter = enter;
        page.exit = exit;
        Ok(())
    }
}
