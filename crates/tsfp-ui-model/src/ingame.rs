//! Host-free in-game UI composition. Geometry and text identifiers come from a
//! runtime table; this module contains no retail presentation data.
use std::collections::BTreeMap;

use tsfp_state::snapshots::HudSnapshot;
use tsfp_ui_renderer::{AnchoredRect, Color, LayoutError, Quad, Rect, Viewport};

use crate::letterbox::{Bar, LetterboxBars};
use crate::model::{Align, WidgetId};
use crate::{PageId, RenderLayout, TextId, TextPlacement, TextRef, UiModel};

/// Semantic HUD slots whose visual presentation is supplied at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HudSlot {
    Prompt,
    Player(u8),
    Inventory(u8),
}

/// A runtime-loaded HUD table entry. No text or asset bytes are retained here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudStyle {
    pub rect: AnchoredRect,
    pub color: Color,
    pub text: Option<TextId>,
    pub font_index: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudTable {
    pub styles: BTreeMap<HudSlot, HudStyle>,
}

/// The per-frame visible slots, computed only from a pushed snapshot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HudModel {
    pub visible: Vec<HudSlot>,
}

impl HudModel {
    pub fn from_snapshot(snapshot: &HudSnapshot) -> Self {
        let mut visible = Vec::new();
        if snapshot.prompt_allowed {
            visible.push(HudSlot::Prompt);
        }
        for slot in 0..snapshot.players.min(snapshot.inventory.len() as u32) as usize {
            visible.push(HudSlot::Player(slot as u8));
            if snapshot.inventory[slot].is_some() {
                visible.push(HudSlot::Inventory(slot as u8));
            }
        }
        Self { visible }
    }

    /// Resolve a fabricated or runtime-loaded table through the shared UI renderer.
    /// Missing styles are omitted: no inferred geometry or labels are invented.
    pub fn layout(
        &self,
        table: &HudTable,
        viewport: Viewport,
    ) -> Result<RenderLayout, LayoutError> {
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        for slot in &self.visible {
            let Some(style) = table.styles.get(slot) else {
                continue;
            };
            let rect_px = viewport.resolve(style.rect)?;
            quads.push(Quad {
                rect_px,
                color: style.color,
            });
            if let Some(text) = style.text {
                texts.push(TextPlacement {
                    widget: hud_widget_id(*slot),
                    text: TextRef::Id(text),
                    font_index: style.font_index,
                    value_font_index: style.font_index,
                    align: Align::Start,
                    rect_px,
                    color: style.color,
                    value: None,
                });
            }
        }
        Ok(RenderLayout { quads, texts })
    }
}

fn hud_widget_id(slot: HudSlot) -> WidgetId {
    let index = match slot {
        HudSlot::Prompt => 0,
        HudSlot::Player(player) => 1 + u64::from(player),
        HudSlot::Inventory(player) => 5 + u64::from(player),
    };
    WidgetId(WidgetId::ANON_BASE + index)
}

/// In-game retained pages share the frontend model, input router and renderer.
/// A page is referenced by its runtime descriptor-derived id, never a guest address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InGamePageKind {
    Pause,
    Dialog,
    MapMaker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InGamePage {
    pub id: PageId,
    pub kind: InGamePageKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InGameStack {
    pages: Vec<InGamePage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    UnknownPage(PageId),
}

impl InGameStack {
    pub fn open(&mut self, model: &UiModel, page: InGamePage) -> Result<(), OpenError> {
        if model.page(page.id).is_none() {
            return Err(OpenError::UnknownPage(page.id));
        }
        self.pages.push(page);
        Ok(())
    }

    pub fn close(&mut self) -> Option<InGamePage> {
        self.pages.pop()
    }

    pub fn top(&self) -> Option<InGamePage> {
        self.pages.last().copied()
    }

    pub fn layout_top(
        &self,
        model: &UiModel,
        viewport: Viewport,
    ) -> Option<Result<RenderLayout, LayoutError>> {
        let page = self.top()?;
        model.to_render_batches(page.id, viewport, model.page(page.id)?.start_focus())
    }
}

/// HUD first, then the active page in painter order. Text shaping remains a
/// caller-provided renderer service, as for frontend menus.
pub fn compose(hud: RenderLayout, overlay: Option<RenderLayout>) -> RenderLayout {
    let mut hud = hud;
    if let Some(overlay) = overlay {
        hud.quads.extend(overlay.quads);
        hud.texts.extend(overlay.texts);
    }
    hud
}

/// Append owned letterbox bars after the HUD and before the active page.
/// Bars use exclusive right and bottom edges; empty geometry is omitted.
pub fn compose_letterbox(
    mut hud: RenderLayout,
    bars: Option<LetterboxBars>,
    color: Color,
    overlay: Option<RenderLayout>,
) -> RenderLayout {
    if let Some(bars) = bars {
        for bar in [bars.bottom, bars.top] {
            if let Some(quad) = bar_quad(bar, color) {
                hud.quads.push(quad);
            }
        }
    }
    compose(hud, overlay)
}

fn bar_quad(bar: Bar, color: Color) -> Option<Quad> {
    let width = i64::from(bar.right) - i64::from(bar.left);
    let height = i64::from(bar.bottom) - i64::from(bar.top);
    if width <= 0 || height <= 0 {
        return None;
    }
    let rect_px = Rect::new(
        tsfp_ui_renderer::Vec2::new(bar.left as f32, bar.top as f32),
        tsfp_ui_renderer::Vec2::new(width as f32, height as f32),
    )
    .ok()?;
    Some(Quad { rect_px, color })
}

/// Resolve a rectangle for callers that need hit testing in physical pixels.
pub fn resolve_slot(
    table: &HudTable,
    slot: HudSlot,
    viewport: Viewport,
) -> Option<Result<Rect, LayoutError>> {
    table
        .styles
        .get(&slot)
        .map(|style| viewport.resolve(style.rect))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_ui_renderer::{AnchorSpace, Edge, Vec2};

    fn viewport() -> Viewport {
        Viewport::new(
            Vec2::new(1280.0, 720.0),
            1.5,
            Rect::new(Vec2::new(160.0, 0.0), Vec2::new(960.0, 720.0)).unwrap(),
        )
        .unwrap()
    }

    fn style() -> HudStyle {
        HudStyle {
            rect: AnchoredRect {
                space: AnchorSpace::Window,
                horizontal: Edge::End,
                vertical: Edge::Start,
                offset: Vec2::new(-8.0, 8.0),
                size: Vec2::new(40.0, 20.0),
            },
            color: Color([1.0; 4]),
            text: Some(TextId(7)),
            font_index: 2,
        }
    }

    #[test]
    fn fabricated_hud_table_resolves_window_edge_and_omits_missing_slots() {
        let mut table = HudTable::default();
        table.styles.insert(HudSlot::Prompt, style());
        let hud = HudModel {
            visible: vec![HudSlot::Prompt, HudSlot::Player(0)],
        };
        let layout = hud.layout(&table, viewport()).unwrap();
        assert_eq!(layout.quads.len(), 1);
        assert_eq!(layout.texts.len(), 1);
        assert_eq!(layout.quads[0].rect_px.origin, Vec2::new(1208.0, 12.0));
        assert_eq!(layout.texts[0].text, TextRef::Id(TextId(7)));
    }

    #[test]
    fn snapshot_controls_semantic_slots_without_guest_reads() {
        let snapshot = HudSnapshot {
            level: 1,
            players: 6,
            mode: 0,
            flags: 0,
            tick_divisor: 1,
            tick_scale_bits: 0,
            viewer_record: 0,
            controlled: 0,
            viewer_state: None,
            prompt_allowed: false,
            pool_list_len: None,
            inventory: [None; 4],
            cells: [0; tsfp_state::snapshots::HUD_CELLS.len()],
        };
        assert_eq!(
            HudModel::from_snapshot(&snapshot).visible,
            vec![
                HudSlot::Player(0),
                HudSlot::Player(1),
                HudSlot::Player(2),
                HudSlot::Player(3)
            ]
        );
        let mut snapshot = snapshot;
        snapshot.prompt_allowed = true;
        assert_eq!(
            HudModel::from_snapshot(&snapshot).visible[0],
            HudSlot::Prompt
        );
    }

    #[test]
    fn overlay_stack_uses_existing_page_and_preserves_painter_order() {
        let mut model = UiModel::new();
        model
            .pages
            .insert(PageId(3), crate::Page::new(PageId(3), 1));
        let mut stack = InGameStack::default();
        assert_eq!(
            stack.open(
                &model,
                InGamePage {
                    id: PageId(4),
                    kind: InGamePageKind::Pause
                }
            ),
            Err(OpenError::UnknownPage(PageId(4)))
        );
        assert_eq!(stack.top(), None);
        stack
            .open(
                &model,
                InGamePage {
                    id: PageId(3),
                    kind: InGamePageKind::Pause,
                },
            )
            .unwrap();
        assert!(stack.layout_top(&model, viewport()).unwrap().is_ok());
        let front = RenderLayout {
            quads: vec![Quad {
                rect_px: Rect::new(Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0)).unwrap(),
                color: Color([1.0; 4]),
            }],
            texts: vec![],
        };
        let back = front.clone();
        assert_eq!(compose(back, Some(front)).quads.len(), 2);
        assert_eq!(stack.close().unwrap().kind, InGamePageKind::Pause);
    }

    #[test]
    fn letterbox_uses_bottom_then_top_before_overlay() {
        let color = Color([0.0, 0.0, 0.0, 0.5]);
        let bar = |top, bottom| Bar {
            left: 10,
            top,
            right: 110,
            bottom,
        };
        let empty = RenderLayout {
            quads: vec![],
            texts: vec![],
        };
        let overlay = RenderLayout {
            quads: vec![Quad {
                rect_px: Rect::new(Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0)).unwrap(),
                color,
            }],
            texts: vec![],
        };
        let result = compose_letterbox(
            empty,
            Some(LetterboxBars {
                bottom: bar(80, 100),
                top: bar(0, 20),
            }),
            color,
            Some(overlay),
        );
        assert_eq!(result.quads.len(), 3);
        assert_eq!(result.quads[0].rect_px.origin, Vec2::new(10.0, 80.0));
        assert_eq!(result.quads[0].rect_px.size, Vec2::new(100.0, 20.0));
        assert_eq!(result.quads[1].rect_px.origin, Vec2::new(10.0, 0.0));
        assert_eq!(result.quads[2].rect_px.origin, Vec2::new(1.0, 1.0));
    }
}
