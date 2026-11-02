//! Host-free bridge from the reconstructed pause list to retained UI widgets.
//! Geometry and fonts are supplied by a runtime table; the item decisions and
//! actions come from the readable Rust pause ports.

use std::collections::BTreeMap;

use tsfp_ui_renderer::AnchoredRect;

use crate::model::{HandlerRef, Page, PageId, TextId, TextRef, Widget, WidgetId, WidgetKind};
use crate::pause::{PauseItem, PauseMenu};
use crate::pause_actions::PauseAction;

/// Presentation of one row, loaded from original data or an explicit preset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PauseRowStyle {
    pub rect: AnchoredRect,
    pub font_index: u32,
    pub value_font_index: u32,
}

/// Runtime presentation table. Row index is the ordered position in `PauseMenu`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PauseWidgetTable {
    pub rows: BTreeMap<usize, PauseRowStyle>,
}

/// Data not represented by the generic widget contract. The host may pass these
/// values to the original-compatible value and page-shell implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PauseWidgetData {
    pub action: PauseAction,
    pub initial_state: bool,
    pub metric_bits: u32,
    pub value_callback: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PauseWidgetPage {
    pub page: Page,
    pub data: BTreeMap<WidgetId, PauseWidgetData>,
    /// Preserved page-shell values for the host's original layout pass.
    pub header_metric_bits: u32,
    pub widget_flags: u32,
    pub tail: crate::pause::PauseTail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseWidgetError {
    MissingStyle(usize),
    UnknownHandler(u32),
}

impl PauseWidgetPage {
    /// Produce a generic page with stable row IDs and native action keys. Refuse
    /// incomplete tables rather than silently inventing original geometry.
    pub fn from_menu(
        id: PageId,
        menu: &PauseMenu,
        table: &PauseWidgetTable,
    ) -> Result<Self, PauseWidgetError> {
        let mut page = Page::new(id, 1);
        let mut data = BTreeMap::new();
        for (index, item) in menu.items.iter().enumerate() {
            let style = table
                .rows
                .get(&index)
                .ok_or(PauseWidgetError::MissingStyle(index))?;
            let action = PauseAction::from_handler(item.handler)
                .ok_or(PauseWidgetError::UnknownHandler(item.handler))?;
            let widget = widget(index, item, *style, action);
            data.insert(
                widget.id,
                PauseWidgetData {
                    action,
                    initial_state: item.state,
                    metric_bits: item.metric_bits,
                    value_callback: (item.value != 0).then_some(item.value),
                },
            );
            page.widgets.push(widget);
        }
        Ok(Self {
            page,
            data,
            header_metric_bits: menu.header_metric_bits,
            widget_flags: menu.widget_flags,
            tail: menu.tail,
        })
    }

    /// Resolve a focused widget to its typed Rust handler.
    pub fn action(&self, widget: WidgetId) -> Option<PauseAction> {
        self.data.get(&widget).map(|entry| entry.action)
    }
}

fn widget(index: usize, item: &PauseItem, style: PauseRowStyle, action: PauseAction) -> Widget {
    let mut widget = Widget::new(WidgetId::anon(index), WidgetKind::Button);
    widget.rect = style.rect;
    widget.font_index = style.font_index;
    widget.value_font_index = style.value_font_index;
    widget.text = item
        .text
        .map_or(TextRef::None, |id| TextRef::Id(TextId(id)));
    widget.enabled = item.enabled.unwrap_or(true);
    widget.activate = Some(HandlerRef::Named(format!("pause.{action:?}")));
    widget
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pause::PauseInputs;
    use tsfp_ui_renderer::{AnchorSpace, Edge, Rect, Vec2, Viewport};

    fn style() -> PauseRowStyle {
        PauseRowStyle {
            rect: AnchoredRect {
                space: AnchorSpace::SafeArea4x3,
                horizontal: Edge::Start,
                vertical: Edge::Start,
                offset: Vec2::new(8.0, 12.0),
                size: Vec2::new(80.0, 18.0),
            },
            font_index: 2,
            value_font_index: 3,
        }
    }

    #[test]
    fn all_items_route_through_typed_actions_and_shared_renderer() {
        let menu = PauseMenu::original(&PauseInputs {
            level: 1,
            player_count: 1,
            session_active: true,
            host_side: true,
            ..Default::default()
        });
        let table = PauseWidgetTable {
            rows: (0..menu.items.len()).map(|i| (i, style())).collect(),
        };
        let mapped = PauseWidgetPage::from_menu(PageId(7), &menu, &table).unwrap();
        assert_eq!(mapped.page.widgets.len(), menu.items.len());
        for (i, item) in menu.items.iter().enumerate() {
            let id = WidgetId::anon(i);
            assert_eq!(mapped.action(id), PauseAction::from_handler(item.handler));
            assert_eq!(mapped.page.widgets[i].enabled, item.enabled.unwrap_or(true));
        }
        assert_eq!(
            mapped.data[&WidgetId::anon(3)].value_callback,
            Some(0x1a_0d80)
        );
        let mut model = crate::UiModel::new();
        model.pages.insert(mapped.page.id, mapped.page);
        let viewport = Viewport::new(
            Vec2::new(640.0, 480.0),
            1.0,
            Rect::new(Vec2::new(0.0, 0.0), Vec2::new(640.0, 480.0)).unwrap(),
        )
        .unwrap();
        let layout = model
            .to_render_batches(PageId(7), viewport, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            layout.texts.len(),
            menu.items.iter().filter(|i| i.text.is_some()).count()
        );
    }

    #[test]
    fn incomplete_presentation_fails_closed() {
        let menu = PauseMenu::original(&PauseInputs::default());
        assert_eq!(
            PauseWidgetPage::from_menu(PageId(1), &menu, &PauseWidgetTable::default()),
            Err(PauseWidgetError::MissingStyle(0))
        );
    }
}
