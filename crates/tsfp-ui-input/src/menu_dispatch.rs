//! Apply device-independent input events to a Rust menu page instance.
//!
//! `UiInput` owns hit testing and repeat timing. This adapter keeps the page's
//! selected widget in sync and returns handler tokens for the bindings layer.

use crate::{UiEvent, WidgetId as InputWidgetId};
use tsfp_ui_model::{MenuAction, MenuFocus, Page, WidgetId};

pub fn dispatch_menu_event(
    page: &Page,
    focus: &mut MenuFocus,
    event: &UiEvent,
) -> Option<MenuAction> {
    focus.reconcile(page);
    match event {
        UiEvent::Focus(id) | UiEvent::Hover(id) => {
            let id = model_id(*id);
            if !page.widget(id).is_some_and(|widget| widget.is_focusable()) {
                return None;
            }
            if focus.selected == Some(id) {
                return None;
            }
            focus.selected = Some(id);
            Some(MenuAction::Focus(id))
        }
        UiEvent::Activate(id) => {
            let id = model_id(*id);
            let widget = page.widget(id)?;
            if !widget.enabled {
                if focus.selected == Some(id) {
                    return Some(MenuAction::Disabled(id));
                }
                if widget.is_focusable() {
                    focus.selected = Some(id);
                    return Some(MenuAction::Disabled(id));
                }
                return None;
            }
            if !widget.is_focusable() {
                return None;
            }
            focus.selected = Some(id);
            Some(MenuAction::Activate {
                widget: id,
                handler: widget
                    .activate
                    .clone()
                    .or_else(|| page.handlers.select.clone()),
            })
        }
        UiEvent::Back => Some(MenuAction::Back(page.handlers.back.clone())),
        _ => None,
    }
}

fn model_id(id: InputWidgetId) -> WidgetId {
    WidgetId(id.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_model::{HandlerRef, PageId, Widget, WidgetKind};

    #[test]
    fn hover_click_and_keyboard_focus_share_one_selection() {
        let mut page = Page::new(PageId(1), 1);
        page.widgets
            .push(Widget::new(WidgetId(1), WidgetKind::Button));
        page.widgets
            .push(Widget::new(WidgetId(2), WidgetKind::Button));
        page.widgets[1].activate = Some(HandlerRef::Named("chosen".into()));
        let mut focus = MenuFocus::new(&page);
        assert_eq!(
            dispatch_menu_event(&page, &mut focus, &UiEvent::Hover(InputWidgetId(2))),
            Some(MenuAction::Focus(WidgetId(2)))
        );
        assert_eq!(
            dispatch_menu_event(&page, &mut focus, &UiEvent::Focus(InputWidgetId(1))),
            Some(MenuAction::Focus(WidgetId(1)))
        );
        assert_eq!(
            dispatch_menu_event(&page, &mut focus, &UiEvent::Activate(InputWidgetId(2))),
            Some(MenuAction::Activate {
                widget: WidgetId(2),
                handler: Some(HandlerRef::Named("chosen".into()))
            })
        );
        assert_eq!(focus.selected, Some(WidgetId(2)));
    }

    #[test]
    fn stale_and_disabled_targets_cannot_activate() {
        let mut page = Page::new(PageId(1), 1);
        page.widgets
            .push(Widget::new(WidgetId(1), WidgetKind::Button));
        page.widgets
            .push(Widget::new(WidgetId(2), WidgetKind::Button));
        page.widgets[1].enabled = false;
        let mut focus = MenuFocus::new(&page);
        for id in [InputWidgetId(2), InputWidgetId(99)] {
            assert_eq!(
                dispatch_menu_event(&page, &mut focus, &UiEvent::Activate(id)),
                None
            );
            assert_eq!(
                dispatch_menu_event(&page, &mut focus, &UiEvent::Hover(id)),
                None
            );
        }
        assert_eq!(focus.selected, Some(WidgetId(1)));
    }

    #[test]
    fn disabled_focusable_target_is_reported_without_becoming_activation() {
        let mut page = Page::new(PageId(1), 1);
        let mut widget = Widget::new(WidgetId(2), WidgetKind::Button);
        widget.enabled = false;
        widget.focus_when_disabled = true;
        page.widgets.push(widget);
        let mut focus = MenuFocus::new(&page);

        assert_eq!(
            dispatch_menu_event(&page, &mut focus, &UiEvent::Activate(InputWidgetId(2))),
            Some(MenuAction::Disabled(WidgetId(2)))
        );
        assert_eq!(focus.selected, Some(WidgetId(2)));
    }
}
