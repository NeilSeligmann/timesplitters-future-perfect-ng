//! Owned layout adapter from a Rust UI page to the device input router.

use crate::layout::{Dir, HitTarget, Rect, UiLayout, WidgetId};
use std::collections::BTreeSet;
use tsfp_ui_model::{Page, focus::Dir as ModelDir};

#[derive(Clone, Copy, Debug)]
struct Target {
    id: WidgetId,
    rect: Rect,
    enabled: bool,
    focusable: bool,
    directional_skip: bool,
}

impl HitTarget for Target {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn rect(&self) -> Rect {
        self.rect
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn focusable(&self) -> bool {
        self.focusable
    }
    fn directionally_navigable(&self) -> bool {
        self.focusable && !self.directional_skip
    }
}

/// Snapshot of a page at a given window size. Coordinates use the same 640x480
/// design space as `DesignMapping`, including extra width on wide windows.
pub struct ModelLayout {
    targets: Vec<Target>,
    links: Vec<(WidgetId, Dir, WidgetId)>,
}

impl ModelLayout {
    pub fn from_page(page: &Page, window_width: f32, window_height: f32) -> Option<Self> {
        if !window_width.is_finite()
            || !window_height.is_finite()
            || window_width <= 0.0
            || window_height <= 0.0
        {
            return None;
        }
        let scale = window_height / 480.0;
        let design_width = window_width / scale;
        let safe_left = (design_width - 640.0) * 0.5;
        let targets = page
            .widgets
            .iter()
            .map(|w| {
                let r = w.bounds.unwrap_or(w.rect); // T2315: the visible part of a scrolled row
                let (left, width) = match r.space {
                    tsfp_ui_renderer::AnchorSpace::SafeArea4x3 => (safe_left, 640.0),
                    tsfp_ui_renderer::AnchorSpace::Window => (0.0, design_width),
                };
                let fraction = |edge| match edge {
                    tsfp_ui_renderer::Edge::Start => 0.0,
                    tsfp_ui_renderer::Edge::Center => 0.5,
                    tsfp_ui_renderer::Edge::End => 1.0,
                };
                Target {
                    id: WidgetId(w.id.0),
                    rect: Rect::new(
                        left + (width - r.size.x) * fraction(r.horizontal) + r.offset.x,
                        (480.0 - r.size.y) * fraction(r.vertical) + r.offset.y,
                        r.size.x,
                        r.size.y,
                    ),
                    enabled: w.enabled,
                    focusable: w.is_focusable(),
                    directional_skip: w.directional_skip,
                }
            })
            .collect();
        let graph = page.focus_graph();
        let mut links = Vec::new();
        for &from in graph.focusable() {
            for (dir, model_dir) in [
                (Dir::Up, ModelDir::Up),
                (Dir::Down, ModelDir::Down),
                (Dir::Left, ModelDir::Left),
                (Dir::Right, ModelDir::Right),
            ] {
                let mut to = graph.step(from, model_dir);
                let mut visited = BTreeSet::new();
                while let Some(candidate) = to {
                    if !visited.insert(candidate) {
                        to = None;
                        break;
                    }
                    let skipped = page
                        .widget(candidate)
                        .is_some_and(|widget| widget.is_focusable() && widget.directional_skip);
                    if !skipped {
                        break;
                    }
                    to = graph.step(candidate, model_dir);
                }
                if let Some(to) = to {
                    links.push((WidgetId(from.0), dir, WidgetId(to.0)));
                }
            }
        }
        Some(Self { targets, links })
    }
}

impl UiLayout for ModelLayout {
    fn visit(&self, f: &mut dyn FnMut(&dyn HitTarget)) {
        for target in &self.targets {
            f(target);
        }
    }
    fn neighbour(&self, from: WidgetId, dir: Dir) -> Option<WidgetId> {
        self.links
            .iter()
            .find(|(a, d, _)| *a == from && *d == dir)
            .map(|(_, _, b)| *b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::hit_test;
    use tsfp_ui_model::{PageId, Widget, WidgetId as ModelWidgetId, WidgetKind, menu_rect};
    use tsfp_ui_renderer::Vec2;

    #[test]
    fn model_ids_and_widescreen_rects_survive_input_snapshot() {
        let mut page = Page::new(PageId(1), 1);
        for (id, y) in [(ModelWidgetId(9), 10.0), (ModelWidgetId::patch(2), 40.0)] {
            let mut widget = Widget::new(id, WidgetKind::Button);
            widget.rect = menu_rect(Vec2::new(20.0, y), Vec2::new(100.0, 20.0));
            page.widgets.push(widget);
        }
        let layout = ModelLayout::from_page(&page, 1280.0, 720.0).unwrap();
        assert_eq!(hit_test(&layout, 130.0, 15.0), Some(WidgetId(9)));
        assert_eq!(
            hit_test(&layout, 130.0, 45.0),
            Some(WidgetId(ModelWidgetId::patch(2).0))
        );
        assert_eq!(
            layout.neighbour(WidgetId(9), Dir::Down),
            Some(WidgetId(ModelWidgetId::patch(2).0))
        );
        assert!(ModelLayout::from_page(&page, 0.0, 720.0).is_none());
    }

    #[test]
    fn directional_input_skips_rows_but_keeps_them_focusable_and_hit_testable() {
        use crate::layout::{navigate, snapshot};
        use tsfp_ui_model::{NavLayout, Wrap};

        let mut page = Page::new(PageId(2), 1);
        page.nav = NavLayout::Grid(1);
        page.wrap = Wrap::Clamp;
        for (id, y) in [(1, 10.0), (2, 40.0), (3, 70.0)] {
            let mut widget = Widget::new(ModelWidgetId(id), WidgetKind::Button);
            widget.rect = menu_rect(Vec2::new(20.0, y), Vec2::new(100.0, 20.0));
            if id == 2 {
                widget.directional_skip = true;
            }
            page.widgets.push(widget);
        }

        let layout = ModelLayout::from_page(&page, 640.0, 480.0).unwrap();
        assert_eq!(layout.neighbour(WidgetId(1), Dir::Down), Some(WidgetId(3)));
        let snaps = snapshot(&layout);
        assert_eq!(
            navigate(&layout, &snaps, WidgetId(1), Dir::Down, false),
            Some(WidgetId(3))
        );
        assert_eq!(
            navigate(&layout, &snaps, WidgetId(2), Dir::Down, false),
            Some(WidgetId(3))
        );
        assert_eq!(hit_test(&layout, 30.0, 45.0), Some(WidgetId(2)));

        // A disabled row that opts into focus remains a valid movement destination.
        let mut disabled_page = Page::new(PageId(3), 1);
        disabled_page.nav = NavLayout::Grid(1);
        disabled_page.wrap = Wrap::Clamp;
        disabled_page
            .widgets
            .push(Widget::new(ModelWidgetId(4), WidgetKind::Button));
        let mut disabled = Widget::new(ModelWidgetId(5), WidgetKind::Button);
        disabled.enabled = false;
        disabled.focus_when_disabled = true;
        disabled_page.widgets.push(disabled);
        let disabled_layout = ModelLayout::from_page(&disabled_page, 640.0, 480.0).unwrap();
        assert_eq!(
            navigate(
                &disabled_layout,
                &snapshot(&disabled_layout),
                WidgetId(4),
                Dir::Down,
                false
            ),
            Some(WidgetId(5))
        );

        let mut all_skipped = Page::new(PageId(4), 1);
        all_skipped.nav = NavLayout::Grid(1);
        all_skipped.wrap = Wrap::Wrap;
        for id in [6, 7] {
            let mut widget = Widget::new(ModelWidgetId(id), WidgetKind::Button);
            widget.directional_skip = true;
            all_skipped.widgets.push(widget);
        }
        let all_skipped_layout = ModelLayout::from_page(&all_skipped, 640.0, 480.0).unwrap();
        assert_eq!(
            navigate(
                &all_skipped_layout,
                &snapshot(&all_skipped_layout),
                WidgetId(6),
                Dir::Down,
                true
            ),
            None
        );
    }
}
