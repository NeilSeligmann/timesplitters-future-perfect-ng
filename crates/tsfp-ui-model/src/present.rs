//! Page presenter: the adapter between the UI switch, the frame driver and any `UiRenderer`
//! (`tsfp-ui-wgpu::WgpuUiRenderer`, `LegacyAdapter`, or `RecordingRenderer` in tests)
//! (T2189 UI-6 slice 11). Only pages routed `Route::Rust` are built and drawn; every other
//! route leaves the page to the legacy builders and draws nothing, so legacy stays default.

use crate::bridge::RenderLayout;
use crate::frame::{TextShaper, UiFrame};
use crate::model::PageId;
use crate::switch::{Route, RouteLog, UiBackend, UiSwitch};
use tsfp_ui_renderer::{Batch, Color, FontId, PositionedGlyph, Quad, UiRenderer, Vec2, Viewport};

/// Result of presenting one page for one frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presented {
    /// The legacy path owns the page (the route says why); nothing was submitted.
    Legacy(Route),
    /// The Rust frame was drawn.
    Rust {
        text_runs: usize,
        skipped_text: usize,
    },
}

#[derive(Debug, PartialEq)]
pub enum PresentError<S, R> {
    Shape(S),
    Render(R),
}

/// Routes, records and draws pages through one renderer.
pub struct PagePresenter<R> {
    pub switch: UiSwitch,
    pub renderer: R,
    pub log: RouteLog,
}

impl<R: UiRenderer> PagePresenter<R> {
    pub fn new(switch: UiSwitch, renderer: R) -> Self {
        Self {
            switch,
            renderer,
            log: RouteLog::new(64),
        }
    }

    /// [`Self::present`] with textured skin sprites under the text (T2307, `skin::skin_page`).
    pub fn present_skinned<S: TextShaper>(
        &mut self,
        page: PageId,
        layout_complete: bool,
        layout: &RenderLayout,
        sprites: &[tsfp_ui_renderer::Sprite],
        shaper: &mut S,
        viewport: Viewport,
    ) -> Result<Presented, PresentError<S::Error, R::Error>> {
        self.present_inner(page, layout_complete, layout, sprites, shaper, viewport)
    }

    /// Present `page` for this frame. `layout_complete` is the checked layout gate of the
    /// page; `layout` is only built into a frame when the route is `Rust`.
    pub fn present<S: TextShaper>(
        &mut self,
        page: PageId,
        layout_complete: bool,
        layout: &RenderLayout,
        shaper: &mut S,
        viewport: Viewport,
    ) -> Result<Presented, PresentError<S::Error, R::Error>> {
        self.present_inner(page, layout_complete, layout, &[], shaper, viewport)
    }

    fn present_inner<S: TextShaper>(
        &mut self,
        page: PageId,
        layout_complete: bool,
        layout: &RenderLayout,
        sprites: &[tsfp_ui_renderer::Sprite],
        shaper: &mut S,
        viewport: Viewport,
    ) -> Result<Presented, PresentError<S::Error, R::Error>> {
        let route = self.switch.route(page, layout_complete);
        self.log.record(page, route);
        if route.backend() != UiBackend::Rust {
            return Ok(Presented::Legacy(route));
        }
        let mut frame = UiFrame::build(layout, shaper).map_err(PresentError::Shape)?;
        frame.sprites = sprites.to_vec();
        frame
            .draw(&mut self.renderer, viewport)
            .map_err(PresentError::Render)?;
        Ok(Presented::Rust {
            text_runs: frame.text_runs(),
            skipped_text: frame.skipped_text,
        })
    }
}

/// One recorded renderer call, owning its batch data.
#[derive(Clone, Debug, PartialEq)]
pub enum Recorded {
    Begin(Viewport),
    Quads(Vec<Quad>),
    Text {
        font: FontId,
        raster_height_px: u32,
        glyphs: Vec<PositionedGlyph>,
        baseline_px: Vec2,
        color: Color,
    },
    /// Sprite and clip batches, recorded by kind only.
    Other(&'static str),
    End,
    Abort,
}

/// Host-free `UiRenderer` that records every call, for tests and trace dumps.
/// `fail_submit` makes the next submit fail once (lifecycle tests).
#[derive(Debug, Default)]
pub struct RecordingRenderer {
    pub calls: Vec<Recorded>,
    pub fail_submit: bool,
}

impl UiRenderer for RecordingRenderer {
    type Error = &'static str;
    fn begin_frame(&mut self, viewport: Viewport) -> Result<(), Self::Error> {
        self.calls.push(Recorded::Begin(viewport));
        Ok(())
    }
    fn submit(&mut self, batches: &[Batch<'_>]) -> Result<(), Self::Error> {
        if std::mem::take(&mut self.fail_submit) {
            return Err("submit failed");
        }
        for batch in batches {
            self.calls.push(match batch {
                Batch::Quads(quads) => Recorded::Quads(quads.to_vec()),
                Batch::Text(run) => Recorded::Text {
                    font: run.font,
                    raster_height_px: run.raster_height_px,
                    glyphs: run.glyphs.to_vec(),
                    baseline_px: run.baseline_px,
                    color: run.color,
                },
                Batch::Sprites(_) => Recorded::Other("sprites"),
                Batch::Polys(_) => Recorded::Other("polys"),
                Batch::Clip(_) => Recorded::Other("clip"),
                Batch::Blend(_) => Recorded::Other("blend"),
            });
        }
        Ok(())
    }
    fn end_frame(&mut self) -> Result<(), Self::Error> {
        self.calls.push(Recorded::End);
        Ok(())
    }
    fn abort_frame(&mut self) {
        self.calls.push(Recorded::Abort);
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED layouts and font tables, recording renderer, end to end with the
    //! production shaper.
    use super::*;
    use crate::bridge::TextPlacement;
    use crate::font::FontTableMeasure;
    use crate::model::{Align, TextId, TextRef, WidgetId};
    use crate::shaper::FontTableShaper;
    use crate::shaper::tests::{Strings, fonts};
    use tsfp_ui_renderer::Rect;

    fn viewport() -> Viewport {
        let size = Vec2::new(640.0, 480.0);
        Viewport::new(size, 1.0, Rect::new(Vec2::new(0.0, 0.0), size).unwrap()).unwrap()
    }

    fn layout() -> RenderLayout {
        let rect = Rect::new(Vec2::new(10.0, 20.0), Vec2::new(100.0, 20.0)).unwrap();
        let color = Color([1.0; 4]);
        RenderLayout {
            quads: vec![Quad {
                rect_px: rect,
                color,
            }],
            texts: vec![TextPlacement {
                widget: WidgetId(1),
                text: TextRef::Id(TextId(0)),
                font_index: 0,
                value_font_index: 0,
                align: Align::Start,
                rect_px: rect,
                color,
                value: Some(TextRef::Dynamic),
            }],
        }
    }

    fn rust_switch() -> UiSwitch {
        UiSwitch::parse(None, ["--ui=rust", "--ui-legacy-page=0x10"])
            .unwrap()
            .0
    }

    #[test]
    fn rust_route_draws_quads_then_shaped_label_and_counts_unbound_value() {
        let (fonts, strings) = (fonts(), Strings(vec![b"AB".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let mut presenter = PagePresenter::new(rust_switch(), RecordingRenderer::default());
        let shown = presenter.present(PageId(0x20), true, &layout(), &mut shaper, viewport());
        assert_eq!(
            shown,
            Ok(Presented::Rust {
                text_runs: 1,
                skipped_text: 1
            })
        );
        let calls = &presenter.renderer.calls;
        assert_eq!(calls.len(), 6);
        assert_eq!(calls[0], Recorded::Begin(viewport()));
        assert!(matches!(&calls[1], Recorded::Quads(q) if q.len() == 1));
        let Recorded::Text {
            glyphs,
            baseline_px,
            ..
        } = &calls[3]
        else {
            panic!("expected text, got {:?}", calls[3]);
        };
        assert_eq!(glyphs.len(), 2);
        assert_eq!(glyphs[1].offset_px.x, 10.0);
        assert_eq!(*baseline_px, Vec2::new(10.0, 40.0));
        assert_eq!(calls[5], Recorded::End);
        assert_eq!(presenter.log.entries().len(), 1);
    }

    #[test]
    fn legacy_routes_submit_nothing() {
        let (fonts, strings) = (fonts(), Strings(vec![b"AB".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let mut default = PagePresenter::new(UiSwitch::default(), RecordingRenderer::default());
        let mut rust = PagePresenter::new(rust_switch(), RecordingRenderer::default());
        let cases = [
            (&mut default, PageId(0x20), true, Route::LegacySelected),
            (&mut rust, PageId(0x10), true, Route::LegacyOverride),
        ];
        for (presenter, page, complete, route) in cases {
            let shown = presenter.present(page, complete, &layout(), &mut shaper, viewport());
            assert_eq!(shown, Ok(Presented::Legacy(route)));
            assert!(presenter.renderer.calls.is_empty());
        }
        let shown = rust.present(PageId(0x20), false, &layout(), &mut shaper, viewport());
        assert_eq!(shown, Ok(Presented::Legacy(Route::LayoutIncomplete)));
        assert!(rust.renderer.calls.is_empty());
        assert_eq!(rust.log.entries().len(), 2);
    }

    #[test]
    fn render_error_aborts_and_surfaces() {
        let (fonts, strings) = (fonts(), Strings(vec![b"AB".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let renderer = RecordingRenderer {
            fail_submit: true,
            ..Default::default()
        };
        let mut presenter = PagePresenter::new(rust_switch(), renderer);
        let shown = presenter.present(PageId(0x20), true, &layout(), &mut shaper, viewport());
        assert_eq!(shown, Err(PresentError::Render("submit failed")));
        assert_eq!(
            presenter.renderer.calls,
            [Recorded::Begin(viewport()), Recorded::Abort]
        );
    }
}
