//! Host-independent menu focus and action routing for the retained Rust page model.
//! The host translates device buttons into `MenuInput`; bindings consume the returned
//! action. Timing, button repeats and parity with the original menu remain host work.

use crate::focus::Dir;
use crate::model::{HandlerRef, Page, WidgetId};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuInput {
    Move(Dir),
    Accept,
    Back,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuAction {
    Focus(WidgetId),
    Activate {
        widget: WidgetId,
        handler: Option<HandlerRef>,
    },
    /// The focused widget accepts focus but is disabled. Typed page bindings may use this
    /// request for a disabled-row help action; generic activation handlers must not run it.
    Disabled(WidgetId),
    Back(Option<HandlerRef>),
}

/// Focus belongs to a page instance, never to the shared page definition.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuFocus {
    pub selected: Option<WidgetId>,
}

impl MenuFocus {
    pub fn new(page: &Page) -> Self {
        Self {
            selected: page.start_focus(),
        }
    }

    /// Reconcile focus after a binding changes widget availability.
    pub fn reconcile(&mut self, page: &Page) -> Option<WidgetId> {
        if self
            .selected
            .and_then(|id| page.widget(id))
            .is_some_and(|widget| widget.can_retain_focus())
        {
            return self.selected;
        }
        self.selected = page.start_focus();
        self.selected
    }

    pub fn input(&mut self, page: &Page, input: MenuInput) -> Option<MenuAction> {
        self.reconcile(page);
        match input {
            MenuInput::Move(dir) => {
                let from = self.selected?;
                let graph = page.focus_graph();
                let mut to = graph.step(from, dir)?;
                let mut visited = BTreeSet::new();
                while visited.insert(to) {
                    let widget = page.widget(to)?;
                    if widget.is_focusable() && !widget.directional_skip {
                        self.selected = Some(to);
                        return Some(MenuAction::Focus(to));
                    }
                    to = graph.step(to, dir)?;
                }
                None
            }
            MenuInput::Accept => {
                let widget = page.widget(self.selected?)?;
                if widget.enabled {
                    Some(MenuAction::Activate {
                        widget: widget.id,
                        handler: widget
                            .activate
                            .clone()
                            .or_else(|| page.handlers.select.clone()),
                    })
                } else {
                    Some(MenuAction::Disabled(widget.id))
                }
            }
            MenuInput::Back => Some(MenuAction::Back(page.handlers.back.clone())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PageId, Widget, WidgetKind};

    fn page() -> Page {
        let mut page = Page::new(PageId(1), 1);
        for id in 1..=3 {
            page.widgets
                .push(Widget::new(WidgetId(id), WidgetKind::Button));
        }
        page.widgets[1].activate = Some(HandlerRef::Named("second".into()));
        page.handlers.select = Some(HandlerRef::Named("default".into()));
        page.handlers.back = Some(HandlerRef::Named("back".into()));
        page
    }

    #[test]
    fn focus_moves_and_actions_resolve_from_the_current_page() {
        let page = page();
        let mut focus = MenuFocus::new(&page);
        assert_eq!(focus.selected, Some(WidgetId(1)));
        assert_eq!(
            focus.input(&page, MenuInput::Move(Dir::Down)),
            Some(MenuAction::Focus(WidgetId(2)))
        );
        assert_eq!(
            focus.input(&page, MenuInput::Accept),
            Some(MenuAction::Activate {
                widget: WidgetId(2),
                handler: Some(HandlerRef::Named("second".into()))
            })
        );
        assert_eq!(
            focus.input(&page, MenuInput::Back),
            Some(MenuAction::Back(Some(HandlerRef::Named("back".into()))))
        );
    }

    #[test]
    fn fabricated_disabled_focus_emits_help_request_not_activation() {
        let mut page = page();
        page.widgets[1].enabled = false;
        page.widgets[1].focus_when_disabled = true;
        let mut focus = MenuFocus::new(&page);
        focus.selected = Some(WidgetId(2));

        assert_eq!(
            focus.input(&page, MenuInput::Accept),
            Some(MenuAction::Disabled(WidgetId(2)))
        );
    }

    #[test]
    fn disabled_focus_reconciles_and_explicit_bad_link_is_ignored() {
        let mut page = page();
        let mut focus = MenuFocus::new(&page);
        page.widgets[0].enabled = false;
        // Disabled-but-navigable rows keep focus (can_retain_focus, T2447/T3404).
        assert_eq!(focus.reconcile(&page), Some(WidgetId(1)));
        focus.selected = Some(WidgetId(2));
        page.links.push(crate::model::FocusLink {
            from: WidgetId(2),
            dir: Dir::Down,
            to: WidgetId(999),
        });
        assert_eq!(focus.input(&page, MenuInput::Move(Dir::Down)), None);
        assert_eq!(focus.selected, Some(WidgetId(2)));
    }
}
