//! Bridge from the model to the `tsfp-ui-renderer` contract: layout output only, no drawing.
//! Quads are ready batches. Text is returned as placements (text id, font index, pixel rect,
//! colour) for the text layout provider, which shapes glyphs and builds `TextRun`s.

use crate::model::*;
use tsfp_ui_renderer::{Batch, Color, LayoutError, Quad, Rect, Viewport};

#[derive(Clone, Debug, PartialEq)]
pub struct TextPlacement {
    pub widget: WidgetId,
    pub text: TextRef,
    pub font_index: u32,
    pub value_font_index: u32,
    pub align: Align,
    pub rect_px: Rect,
    pub color: Color,
    /// Selected option text for cyclers.
    pub value: Option<TextRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderLayout {
    /// Painter order.
    pub quads: Vec<Quad>,
    pub texts: Vec<TextPlacement>,
}

impl RenderLayout {
    /// Batches for `UiRenderer::submit`, borrowing this layout.
    pub fn batches(&self) -> Vec<Batch<'_>> {
        vec![Batch::Quads(&self.quads)]
    }
}

impl UiModel {
    /// Resolve one page to pixel geometry for a viewport. `focused` marks the highlighted widget.
    /// Returns `None` for an unknown page.
    pub fn to_render_batches(
        &self,
        page: PageId,
        viewport: Viewport,
        focused: Option<WidgetId>,
    ) -> Option<Result<RenderLayout, LayoutError>> {
        let page = self.page(page)?;
        Some(layout_page(page, &self.theme, viewport, focused))
    }
}

fn layout_page(
    page: &Page,
    theme: &Theme,
    viewport: Viewport,
    focused: Option<WidgetId>,
) -> Result<RenderLayout, LayoutError> {
    let mut quads = Vec::new();
    let mut texts = Vec::new();
    for w in &page.widgets {
        let rect_px = viewport.resolve(w.rect)?;
        if w.kind == WidgetKind::Panel || w.is_focusable() {
            quads.push(Quad {
                rect_px,
                color: theme.panel,
            });
        }
        if focused == Some(w.id) && w.is_focusable() {
            quads.push(Quad {
                rect_px,
                color: theme.focus,
            });
        }
        if w.text != TextRef::None {
            texts.push(TextPlacement {
                widget: w.id,
                text: w.text,
                font_index: w.font_index,
                value_font_index: w.value_font_index,
                align: w.align,
                rect_px,
                color: if w.enabled {
                    theme.text
                } else {
                    theme.text_disabled
                },
                value: w.options.get(w.selected).map(|o| o.text),
            });
        }
    }
    Ok(RenderLayout { quads, texts })
}
