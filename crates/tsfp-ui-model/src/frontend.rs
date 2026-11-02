//! Frontend driver: the end-to-end `--ui=rust` path for one menu page (T2189 UI-6 slice 14).
//! It finds a page in the loaded model, decides whether its layout is complete, builds the
//! `RenderLayout` and hands both to a `PagePresenter`. A page the Rust model cannot fully lay out
//! (unknown, empty, or a rect that does not resolve) is presented with `layout_complete = false`,
//! so the switch routes it `LayoutIncomplete` and the legacy path keeps it. Renderer-agnostic:
//! `WgpuUiRenderer` in the owner preview, `RecordingRenderer`/`CpuRasterRenderer` in tests.

use crate::bridge::RenderLayout;
use crate::frame::TextShaper;
use crate::json::Json;
use crate::loader::{LoadOptions, RectEdges};
use crate::model::{PageId, UiModel, WidgetId};
use crate::present::{PagePresenter, PresentError, Presented};
use tsfp_ui_renderer::{LayoutError, UiRenderer, Viewport};

/// Builder function of the front-end main menu (docs/t-ui-model.md, survey table).
pub const MAIN_MENU_BUILDER: u64 = 0x2D_0BE0;

/// Why a page cannot be owned by the Rust model this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Incomplete {
    UnknownPage,
    NoWidgets,
    Layout(LayoutError),
}

/// The page's layout, or why the legacy path must keep it.
#[derive(Clone, Debug, PartialEq)]
pub enum PageLayoutState {
    Complete(RenderLayout),
    Incomplete(Incomplete),
}

/// Page id of the page built by `builder` in a menu-tree JSON (the `builder` field of each page).
/// `None` when the text does not parse or no page names that builder.
pub fn page_for_builder(tree_json: &str, builder: u64) -> Option<PageId> {
    let tree = Json::parse(tree_json).ok()?;
    tree.get("pages")?
        .as_object()?
        .iter()
        .find(|(_, page)| page.get("builder").and_then(Json::as_u64) == Some(builder))
        .and_then(|(key, page)| {
            page.get("page_id")
                .and_then(Json::as_u64)
                .or_else(|| u64::from_str_radix(key.trim_start_matches("0x"), 16).ok())
        })
        .map(PageId)
}

/// Load options for captured menu-tree dumps: rects from the MEASURED layout cells and the
/// row flag bit 0 ignored (slice 1 findings).
pub fn dump_load_options() -> LoadOptions {
    LoadOptions {
        rect_cells: None,
        rect_edges: Some(RectEdges::DUMP),
        ignore_row_enabled_flag: true,
    }
}

/// Lay out `page` for `viewport` and judge completeness.
pub fn page_layout(
    model: &UiModel,
    page: PageId,
    viewport: Viewport,
    focused: Option<WidgetId>,
) -> PageLayoutState {
    match model.page(page) {
        None => PageLayoutState::Incomplete(Incomplete::UnknownPage),
        Some(p) if p.widgets.is_empty() => PageLayoutState::Incomplete(Incomplete::NoWidgets),
        Some(_) => match model.to_render_batches(page, viewport, focused) {
            Some(Ok(layout)) => PageLayoutState::Complete(layout),
            Some(Err(error)) => PageLayoutState::Incomplete(Incomplete::Layout(error)),
            None => PageLayoutState::Incomplete(Incomplete::UnknownPage),
        },
    }
}

/// One frame of `page`: Rust draws it when the switch and the layout allow, else legacy keeps it.
pub fn present_page<R: UiRenderer, S: TextShaper>(
    presenter: &mut PagePresenter<R>,
    model: &UiModel,
    page: PageId,
    focused: Option<WidgetId>,
    shaper: &mut S,
    viewport: Viewport,
) -> Result<Presented, PresentError<S::Error, R::Error>> {
    let empty = RenderLayout {
        quads: Vec::new(),
        texts: Vec::new(),
    };
    match page_layout(model, page, viewport, focused) {
        PageLayoutState::Complete(layout) => {
            presenter.present(page, true, &layout, shaper, viewport)
        }
        PageLayoutState::Incomplete(_) => presenter.present(page, false, &empty, shaper, viewport),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::present::{Recorded, RecordingRenderer};
    use crate::switch::{Route, UiSwitch};
    use crate::{ShapedRun, TextPart, TextPlacement};
    use tsfp_ui_renderer::{Rect, Vec2};

    // FABRICATED tree: one page with two rows, one empty page. No game data.
    const TREE: &str = r#"{"schema":1,"pages":{
      "0x100":{"page_id":256,"builder":2952160,"widget_types":[1],"rows":[
        {"row_id":1,"activate_callback":4096},
        {"row_id":2,"activate_callback":4096}]},
      "0x200":{"page_id":512,"builder":7,"widget_types":[1],"rows":[]}},"edges":[]}"#;

    struct NoText;
    impl TextShaper for NoText {
        type Error = ();
        fn shape(&mut self, _: &TextPlacement, _: TextPart) -> Result<Option<ShapedRun>, ()> {
            Ok(None)
        }
    }

    fn model() -> UiModel {
        UiModel::from_tree_json(TREE, &LoadOptions::default()).unwrap()
    }
    fn viewport() -> Viewport {
        let size = Vec2::new(640.0, 480.0);
        Viewport::new(size, 1.0, Rect::new(Vec2::new(0.0, 0.0), size).unwrap()).unwrap()
    }
    fn presenter(args: &[&str]) -> PagePresenter<RecordingRenderer> {
        let switch = UiSwitch::parse(None, args.iter().copied()).unwrap().0;
        PagePresenter::new(switch, RecordingRenderer::default())
    }

    #[test]
    fn finds_main_menu_page_by_builder() {
        assert_eq!(
            page_for_builder(TREE, MAIN_MENU_BUILDER),
            Some(PageId(0x100))
        );
        assert_eq!(page_for_builder(TREE, 0xDEAD), None);
        assert_eq!(page_for_builder("not json", MAIN_MENU_BUILDER), None);
    }

    #[test]
    fn rust_route_draws_main_menu() {
        let model = model();
        let mut p = presenter(&["--ui=rust"]);
        let out = present_page(&mut p, &model, PageId(0x100), None, &mut NoText, viewport());
        assert!(matches!(out, Ok(Presented::Rust { .. })));
        assert_eq!(p.log.lines().len(), 1);
        let quads: usize = p
            .renderer
            .calls
            .iter()
            .map(|c| match c {
                Recorded::Quads(q) => q.len(),
                _ => 0,
            })
            .sum();
        assert_eq!(quads, 2, "one quad per focusable row");
    }

    #[test]
    fn incomplete_pages_fall_back_to_legacy() {
        let model = model();
        let mut p = presenter(&["--ui=rust"]);
        for page in [PageId(0x200), PageId(0x999)] {
            let out = present_page(&mut p, &model, page, None, &mut NoText, viewport());
            assert_eq!(out, Ok(Presented::Legacy(Route::LayoutIncomplete)));
        }
        assert!(p.renderer.calls.is_empty(), "legacy routes submit nothing");
        assert_eq!(
            page_layout(&model, PageId(0x200), viewport(), None),
            PageLayoutState::Incomplete(Incomplete::NoWidgets)
        );
    }

    #[test]
    fn legacy_default_draws_nothing() {
        let model = model();
        let mut p = presenter(&[]);
        let out = present_page(&mut p, &model, PageId(0x100), None, &mut NoText, viewport());
        assert_eq!(out, Ok(Presented::Legacy(Route::LegacySelected)));
        assert!(p.renderer.calls.is_empty());
    }
}
