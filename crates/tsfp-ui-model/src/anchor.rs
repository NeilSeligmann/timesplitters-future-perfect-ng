//! T2096 / T2036 slice 4: widescreen 2D anchoring as typed, readable Rust (host free).
//!
//! The native widescreen model of docs/t-widescreen-design.md section 4: 2D content is laid
//! out in a logical space of height 480 and width `480 * A` (A = content aspect). Widgets are
//! authored in the original 640 x 480 space and placed by one of two anchoring classes:
//!
//! - **Edge**: HUD counters, ammo, health, timers, corners, minimap. The widget keeps its
//!   distance to the nearest authored edge (left, right or centre), so it follows the true
//!   edge of the extended width.
//! - **SafeArea**: menus, text entry, pause content, title. The 640 x 480 block is centred in
//!   the wide frame unchanged (x shift `(W - 640) / 2`).
//!
//! Full-width backgrounds (`Stretch`) span the whole logical width.
//!
//! Evidence: COMPUTED and INFERRED (design section 4); the widget family to class mapping is
//! INFERRED, the original 16:9 HUD scale 0.75 equals `ui_scale(16/9)` (COMPUTED). Tier 0 only:
//! synthetic tests, no runtime caller. Connecting it to HUD drawing is T2171 (rule 19 gate).
//! Contract: docs/t2096-widescreen-anchoring.md.

/// Authored width of the original 2D space.
pub const AUTHORED_W: f32 = 640.0;
/// Authored and logical height.
pub const LOGICAL_H: f32 = 480.0;
/// Original content aspect.
pub const ASPECT_4_3: f32 = 4.0 / 3.0;

/// Horizontal anchor of an edge-anchored widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAnchor {
    Left,
    Centre,
    Right,
}

/// Anchoring class of a widget family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchoring {
    Edge(HAnchor),
    SafeArea,
    Stretch,
}

/// Logical 2D frame for a content aspect (aspects narrower than 4:3 are clamped to 4:3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LogicalFrame {
    pub width: f32,
    pub height: f32,
}

impl LogicalFrame {
    pub fn for_aspect(aspect: f32) -> Self {
        let aspect = if aspect.is_finite() {
            aspect.max(ASPECT_4_3)
        } else {
            ASPECT_4_3
        };
        Self {
            width: LOGICAL_H * aspect,
            height: LOGICAL_H,
        }
    }

    /// Left edge of the centred 640 x 480 safe area.
    pub fn safe_area_x(&self) -> f32 {
        (self.width - AUTHORED_W) * 0.5
    }

    /// Place an authored x span `[x, x + w)` by its anchoring class; returns `(x, w)`.
    pub fn place_x(&self, anchoring: Anchoring, x: f32, w: f32) -> (f32, f32) {
        let extra = self.width - AUTHORED_W;
        match anchoring {
            Anchoring::Edge(HAnchor::Left) => (x, w),
            Anchoring::Edge(HAnchor::Right) => (x + extra, w),
            Anchoring::Edge(HAnchor::Centre) | Anchoring::SafeArea => (x + extra * 0.5, w),
            Anchoring::Stretch => (0.0, self.width),
        }
    }
}

/// Scale from logical width to the original 4:3 grid (`(4/3) / A`, 0.75 at 16:9).
pub fn ui_scale(aspect: f32) -> f32 {
    ASPECT_4_3 / LogicalFrame::for_aspect(aspect).width * LOGICAL_H
}

/// Nearest authored edge of a span: the default class for a HUD element without a table entry.
pub fn nearest_edge(x: f32, w: f32) -> HAnchor {
    let centre = x + w * 0.5;
    let third = AUTHORED_W / 3.0;
    if centre < third {
        HAnchor::Left
    } else if centre >= 2.0 * third {
        HAnchor::Right
    } else {
        HAnchor::Centre
    }
}

/// Build the renderer viewport for a physical window. The 480-unit height sets
/// one uniform pixel scale; menus use the centred 4:3 safe area while HUD
/// elements can select the full window. Narrow windows fit the authored width.
pub fn window_viewport(
    width: u32,
    height: u32,
) -> Result<tsfp_ui_renderer::Viewport, tsfp_ui_renderer::LayoutError> {
    use tsfp_ui_renderer::{Rect, Vec2, Viewport};
    let size = Vec2::new(width as f32, height as f32);
    let scale = (size.y / LOGICAL_H).min(size.x / AUTHORED_W);
    let safe_width = AUTHORED_W * scale;
    let safe_height = LOGICAL_H * scale;
    let safe = Rect::new(
        Vec2::new((size.x - safe_width) * 0.5, (size.y - safe_height) * 0.5),
        Vec2::new(safe_width, safe_height),
    )?;
    Viewport::new(size, scale, safe)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A16_9: f32 = 16.0 / 9.0;
    const A21_9: f32 = 64.0 / 27.0;

    #[test]
    fn four_three_is_identity() {
        let f = LogicalFrame::for_aspect(ASPECT_4_3);
        assert_eq!(f.width, 640.0);
        for a in [
            Anchoring::Edge(HAnchor::Left),
            Anchoring::Edge(HAnchor::Centre),
            Anchoring::Edge(HAnchor::Right),
            Anchoring::SafeArea,
        ] {
            assert_eq!(f.place_x(a, 100.0, 40.0), (100.0, 40.0));
        }
        assert_eq!(f.safe_area_x(), 0.0);
    }

    #[test]
    fn widths_and_scale() {
        assert!((LogicalFrame::for_aspect(A16_9).width - 853.333).abs() < 1e-2);
        assert!((LogicalFrame::for_aspect(A21_9).width - 1137.777).abs() < 1e-2);
        assert!((ui_scale(A16_9) - 0.75).abs() < 1e-6);
        assert!((ui_scale(A21_9) - 0.5625).abs() < 1e-6);
        assert_eq!(ui_scale(1.0), 1.0);
        assert_eq!(LogicalFrame::for_aspect(f32::NAN).width, 640.0);
    }

    #[test]
    fn right_edge_keeps_margin() {
        let f = LogicalFrame::for_aspect(A21_9);
        let (x, w) = f.place_x(Anchoring::Edge(HAnchor::Right), 580.0, 40.0);
        assert!((f.width - (x + w) - 20.0).abs() < 1e-3);
        let (x, _) = f.place_x(Anchoring::Edge(HAnchor::Left), 20.0, 40.0);
        assert_eq!(x, 20.0);
    }

    #[test]
    fn safe_area_is_centred() {
        let f = LogicalFrame::for_aspect(A16_9);
        let (x, w) = f.place_x(Anchoring::SafeArea, 0.0, AUTHORED_W);
        assert!((x - f.safe_area_x()).abs() < 1e-4);
        assert!((x - (f.width - (x + w))).abs() < 1e-3);
        let (cx, _) = f.place_x(Anchoring::Edge(HAnchor::Centre), 300.0, 40.0);
        assert!((cx + 20.0 - f.width * 0.5).abs() < 1e-3);
        assert_eq!(f.place_x(Anchoring::Stretch, 50.0, 10.0), (0.0, f.width));
    }

    #[test]
    fn nearest_edge_thirds() {
        assert_eq!(nearest_edge(10.0, 40.0), HAnchor::Left);
        assert_eq!(nearest_edge(300.0, 40.0), HAnchor::Centre);
        assert_eq!(nearest_edge(600.0, 30.0), HAnchor::Right);
    }

    #[test]
    fn window_viewport_centres_safe_area_and_keeps_hud_edges() {
        use tsfp_ui_renderer::{AnchorSpace, AnchoredRect, Edge, Vec2};
        for (width, height, scale) in [
            (640, 480, 1.0),
            (1920, 1080, 2.25),
            (2520, 1080, 2.25),
            (3840, 1080, 2.25),
        ] {
            let view = window_viewport(width, height).unwrap();
            assert_eq!(view.scale(), scale);
            let safe = view.safe_area();
            assert_eq!(safe.size.x, 640.0 * scale);
            assert_eq!(safe.origin.x * 2.0 + safe.size.x, width as f32);
            let hud = view
                .resolve(AnchoredRect {
                    space: AnchorSpace::Window,
                    horizontal: Edge::End,
                    vertical: Edge::End,
                    offset: Vec2::new(-10.0, -10.0),
                    size: Vec2::new(20.0, 20.0),
                })
                .unwrap();
            assert_eq!(width as f32 - hud.origin.x - hud.size.x, 10.0 * scale);
        }
        let narrow = window_viewport(600, 600).unwrap();
        assert_eq!(narrow.scale(), 0.9375);
        assert_eq!(narrow.safe_area().origin.y, 75.0);
        assert!(window_viewport(0, 480).is_err());
    }
}
