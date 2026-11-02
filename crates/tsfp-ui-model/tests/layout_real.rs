//! Tier 1 check of the layout pass (`layout.rs`) against the dump words of the captured pages
//! (ignored `tmp/ui-tree/layout.json`, written by `python3 -m tools.ui_tree layout RUN_DIR...`, or
//! `TSFP_UI_LAYOUT`). Skips (passes) without a capture. The inputs of a row (start, end, pad,
//! content height) are the cells the builder leaves; the outputs (cursor positions, page position,
//! pixel rectangles) must come out of the pass. Text line heights come from a calibration table.

use std::collections::BTreeMap;
use tsfp_ui_model::json::Json;
use tsfp_ui_model::layout::*;
use tsfp_ui_model::{TextId, TextRef};

#[path = "support/path.rs"]
mod capture_path;
use capture_path::layout_capture as path;

struct Words<'a>(&'a Json);
impl Words<'_> {
    fn u32(&self, offset: u32) -> u32 {
        self.0
            .get(&format!("{offset:#x}"))
            .and_then(Json::as_u64)
            .unwrap_or(0) as u32
    }
    fn f32(&self, offset: u32) -> f32 {
        f32::from_bits(self.u32(offset))
    }
}

/// Line heights (grid units) calibrated from rows whose pad is a round number (docs/t-ui-model.md).
/// Font 18 has one captured row (page 0x524a70, pad 2.0 like its siblings): CALIBRATED from it, so
/// that row is circular evidence, the rest of the page is not.
fn calibrated() -> TableMeasure {
    TableMeasure {
        line_heights: BTreeMap::from([(3, 3.9844), (4, 4.7433), (17, 3.7679), (18, 3.4670)]),
        fallback: f32::NAN,
    }
}

fn near(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// Captured pages whose placement the layout pass does not model yet (reported as skipped, not as
/// passed). Empty since T2189 slice 8 closed the nested scroll grid of 0x52ddfc.
const KNOWN_GAPS: &[(&str, &str)] = &[];

/// Pages the builder positions explicitly instead of centring (left anchored popups). The anchor
/// is a builder input this test reads from the page cells 0x60/0x64 (INFERRED, the call that sets
/// it is not recovered yet), so only the rows and rectangles are checked against the pass.
const ANCHORED: &[&str] = &["0x4e8d54", "0x524a10", "0x52d464", "0x52ddfc"];

#[derive(Default, Debug)]
struct Stats {
    pages: usize,
    covered: usize,
    skipped: Vec<(String, &'static str)>,
    rows: usize,
    cells_ok: usize,
    rect_exact: usize,
    rect_half_pixel: usize,
    hidden_ok: usize,
    page_ok: usize,
    stale: usize,
    live_rows: usize,
    live_exact: usize,
}

#[test]
fn layout_pass_reproduces_captured_pages() {
    let Ok(text) = std::fs::read_to_string(path()) else {
        eprintln!("no layout capture, skipped");
        return;
    };
    let json = Json::parse(&text).expect("layout.json parses");
    let pages = json.get("pages").and_then(Json::as_object).expect("pages");
    let measure = calibrated();
    let viewport = ScreenRect {
        left: 0.0,
        top: 0.0,
        right: 640.0,
        bottom: 480.0,
    };
    let mut stats = Stats::default();
    for (id, page) in pages {
        stats.pages += 1;
        let pw = Words(page.get("page_words").expect("page words"));
        let rows = page.get("rows").and_then(Json::as_array).expect("rows");
        if rows.is_empty() {
            stats.skipped.push((id.clone(), "no rows"));
            continue;
        }
        if let Some((_, reason)) = KNOWN_GAPS.iter().find(|(gap, _)| gap == id) {
            stats.skipped.push((id.clone(), reason));
            continue;
        }
        let words: Vec<Words> = rows
            .iter()
            .map(|r| Words(r.get("words").expect("row words")))
            .collect();
        let texts: Vec<TextRef> = rows
            .iter()
            .zip(&words)
            .map(|(r, w)| match r.get("text_id").and_then(Json::as_u64) {
                Some(t) => TextRef::Id(TextId(t as u32)),
                None if w.u32(0x20) != 0 => TextRef::Dynamic,
                None => TextRef::None,
            })
            .collect();
        let height = pw.f32(0x68);
        let stale = !near(pw.f32(0x80) * 4.8, pw.f32(0xac), 0.05);
        let window = ScrollWindow {
            top: pw.f32(0x6c),
            offset: pw.f32(0x84),
            clip_top: pw.f32(0xa0),
            clip_bottom: pw.f32(0xa4),
            height: pw.f32(0x70),
        };
        let any_scrolling = words.iter().any(|w| w.u32(0x28) & 0x40 != 0);
        let mut specs = Vec::new();
        let mut rect_flags = Vec::new();
        let mut calibrated_ok = true;
        for (w, t) in words.iter().zip(&texts) {
            let font = w.u32(0x1c);
            let measure_flag = (w.u32(0x28) >> 8) & 4 != 0 && *t != TextRef::None;
            let lh = measure.line_height(font);
            if measure_flag && lh.is_nan() {
                calibrated_ok = false;
            }
            let inner = w.f32(0x74);
            let pad = if measure_flag && !lh.is_nan() {
                inner - lh
            } else {
                inner
            };
            let (start, end, width) = (w.f32(0x5c), w.f32(0x60), w.f32(0x6c));
            let scrolling = w.u32(0x28) & 0x40 != 0;
            specs.push(RowSpec {
                font,
                text: *t,
                flags: FlowFlags {
                    measure_text: measure_flag && !lh.is_nan(),
                    scrolling,
                    advance: w.u32(0xb0) != 0,
                    value_row: false,
                    container: w.u32(0) == 5,
                    nested: w.u32(0x28) & 0x20000 != 0,
                },
                start,
                end,
                width: if near(width, end - start, 1e-4) {
                    0.0
                } else {
                    width
                },
                height: 0.0,
                pad,
                content: w.f32(0xf0),
            });
            rect_flags.push(RectFlags {
                divider: w.u32(0) == 9,
                scrolling,
            });
        }
        if !calibrated_ok {
            stats.skipped.push((id.clone(), "font not calibrated"));
            continue;
        }
        let top = words
            .iter()
            .find(|w| w.u32(0x28) & 0x40 == 0)
            .map_or(if any_scrolling { window.top } else { 2.2 }, |w| {
                w.f32(0x68)
            });
        let title = pw.f32(0x78);
        let fitted = 2.0 * pw.f32(0x64) + height;
        // Titled front end pages use the measured span (6.5, 98.75), others a fitted sum.
        let standard = near(fitted, 105.25, 1e-3);
        // Framed pages (page cell 0x90 = 4.0, popup frame style) pad the bottom like the top and
        // size a scroll window as its top inset plus its height (MEASURED fit on 0x4e8d84, 0x52d464,
        // 0x52ddfc). Menu pages (0x90 = 0.9) keep the 2.0 bottom margin.
        let framed = near(pw.f32(0x90), 4.0, 1e-4);
        let spec = PageSpec {
            width: pw.f32(0x50),
            top,
            // Full screen pages starting at 0 have no bottom margin (0x5237e0, fit).
            bottom: if framed { top } else { top.min(2.0) },
            fixed_height: None,
            window_end: any_scrolling.then_some(pw.f32(0x70)),
            span_start: if standard { 6.5 } else { title },
            span_end: if standard { 98.75 } else { fitted - title },
            anchor: ANCHORED
                .contains(&id.as_str())
                .then(|| (pw.f32(0x60), pw.f32(0x64))),
        };
        let flow = flow_rows(&spec, &specs, &measure);
        if !near(flow.height, height, 1e-3) {
            eprintln!("page {id}: height {} want {height}", flow.height);
            stats.skipped.push((id.clone(), "page height differs"));
            continue;
        }
        let (px, py) = spec.place(flow.height);
        // A page whose cells moved after its rectangles were computed (animation, capture between
        // two frames) is compared at the position its pixel origin records.
        let py = if stale { pw.f32(0xac) / 4.8 } else { py };
        let page = PageRect::from_grid(&Grid::DESIGN, px, py, spec.width, flow.height);
        let clip = page.clip(viewport);
        let rects = screen_rects(
            &Grid::DESIGN,
            &page,
            py,
            clip,
            &flow.rows,
            &rect_flags,
            &window,
        );
        let layout = PageLayout {
            x: px,
            y: py,
            width: spec.width,
            height: flow.height,
            flow,
            rects,
        };
        stats.covered += 1;
        if stale {
            stats.stale += 1;
        }
        let page_ok =
            stale || (near(layout.x, pw.f32(0x60), 1e-3) && near(layout.y, pw.f32(0x64), 1e-3));
        stats.page_ok += usize::from(page_ok);
        for (i, w) in words.iter().enumerate() {
            stats.rows += 1;
            let c = layout.flow.rows[i];
            let ok = near(c.x, w.f32(0x64), 1e-3)
                && near(c.y, w.f32(0x68), 1e-3)
                && near(c.width, w.f32(0x6c), 1e-3)
                && near(c.height, w.f32(0x70), 1e-3)
                && near(c.inner_height, w.f32(0x74), 1e-3);
            stats.cells_ok += usize::from(ok);
            let got = layout.rects[i].rect;
            let want = ScreenRect {
                left: w.f32(0x78),
                top: w.f32(0x7c),
                right: w.f32(0x80),
                bottom: w.f32(0x84),
            };
            let exact = got.left.to_bits() == want.left.to_bits()
                && got.top.to_bits() == want.top.to_bits()
                && got.right.to_bits() == want.right.to_bits()
                && got.bottom.to_bits() == want.bottom.to_bits();
            // Stale pages (cells changed after the rectangles were computed) only match to a pixel
            // and a half; every other page must match bit for bit.
            let tol = if stale { 1.5 } else { 0.0 };
            let close = near(got.left, want.left, tol)
                && near(got.top, want.top, tol)
                && near(got.right, want.right, tol)
                && near(got.bottom, want.bottom, tol);
            if !stale {
                stats.live_rows += 1;
                stats.live_exact += usize::from(exact);
            }
            stats.rect_exact += usize::from(exact);
            stats.rect_half_pixel += usize::from(close);
            let hidden_dump = w.u32(0x2c) & 0x20 != 0;
            stats.hidden_ok += usize::from(hidden_dump == layout.rects[i].hidden);
            if std::env::var("TSFP_LAYOUT_VERBOSE").is_ok() && i < 2 {
                eprintln!("page {id} row {i}: got {got:?} want {want:?}");
            }
            if !close || !ok {
                eprintln!("page {id} row {i}: cells_ok={ok} got {got:?} want {want:?} cells {c:?}");
            }
        }
    }
    eprintln!("{stats:#?}");
    assert!(stats.covered > 0, "no page was covered");
    // Floor of the pages this pass reproduces (slice 8: 42), so a regressed rule that turns a
    // covered page into a skipped one fails instead of shrinking the count silently.
    assert!(
        stats.covered >= 42,
        "covered pages regressed: {}",
        stats.covered
    );
    assert_eq!(
        stats.cells_ok, stats.rows,
        "row cells must match on covered pages"
    );
    assert_eq!(stats.page_ok, stats.covered, "page placement must match");
    assert_eq!(
        stats.live_exact, stats.live_rows,
        "live pages must match bit for bit"
    );
    assert_eq!(
        stats.rect_half_pixel, stats.rows,
        "stale pages must match within 1.5 pixels"
    );
    assert_eq!(stats.hidden_ok, stats.rows, "hidden flags must match");
}
