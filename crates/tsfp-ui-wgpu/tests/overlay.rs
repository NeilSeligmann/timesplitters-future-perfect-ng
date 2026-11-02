//! T2189 UI-6 slice 15: `overlay::render_page` with text. FABRICATED tree, font tables and string
//! bank (real structure, made-up values); MEASURED on the software adapter (llvmpipe). No game data.
use tsfp_ui_model::font::{FontObject, FontRecord, FontSet, Glyph, KERN_SIZE};
use tsfp_ui_model::frame::{ShapedRun, TextPart};
use tsfp_ui_model::frontend::dump_load_options;
use tsfp_ui_model::strings::{Bank, BankStrings};
use tsfp_ui_model::switch::{Route, UiSwitch};
use tsfp_ui_model::{PageId, TextPlacement, TextShaper, UiModel};
use tsfp_ui_renderer::{FontId, GlyphId, PositionedGlyph, Vec2};
use tsfp_ui_wgpu::overlay::{CoveredShaper, GlyphSource, TextAssets, render_page};
use tsfp_wgpu::context::{GpuContext, Preference};

const TREE: &str = r#"{"schema":1,"pages":{
  "0x100":{"page_id":256,"builder":2952160,"widget_types":[1],"rows":[
    {"row_id":1,"activate_callback":4096,"text_id":1,"font_index":0},
    {"row_id":2,"activate_callback":4096,"text_id":2,"font_index":0}]}},"edges":[]}"#;
const SIZE: (u32, u32) = (640, 480);

fn assets() -> TextAssets {
    let record = FontRecord {
        cell_width: 20.0,
        cell_height: 20.0,
        glyphs: vec![Glyph::default(); 96],
        kerning: vec![0; KERN_SIZE * KERN_SIZE],
        ..FontRecord::default()
    };
    let fonts = FontSet {
        records: [(0, record)].into(),
        objects: vec![FontObject {
            record: 0,
            scale_x: 0.5,
            scale_y: 1.0,
            advance_adjust: 0,
        }],
        ..FontSet::default()
    };
    let mut bank = Vec::new();
    for offset in [12u32, 13, 18] {
        bank.extend_from_slice(&offset.to_le_bytes());
    }
    bank.extend_from_slice(b"\0PLAY\0QUIT\0");
    let strings = BankStrings {
        common: Bank::parse(&bank, 3).unwrap(),
        story: None,
    };
    TextAssets {
        fonts,
        strings,
        font_pages: vec![],
        skin: None,
        skin_note: None,
    }
}

fn rust() -> UiSwitch {
    UiSwitch::parse(None, ["--ui=rust"]).unwrap().0
}

fn lit(rgba: &[u8]) -> usize {
    rgba.chunks_exact(4).filter(|p| p[3] != 0).count()
}

#[test]
fn rows_carry_text_and_the_rest_stays_transparent() {
    let gpu = GpuContext::new(&Preference::Software).expect("software adapter");
    let model = UiModel::from_tree_json(TREE, &dump_load_options()).unwrap();
    let page = PageId(0x100);
    let plain = render_page(&gpu, rust(), &model, page, None, None, SIZE, [0; 4]).unwrap();
    let assets = assets();
    let text = render_page(
        &gpu,
        rust(),
        &model,
        page,
        None,
        Some(&assets),
        SIZE,
        [0; 4],
    )
    .unwrap();
    assert_eq!((plain.route, text.route), (Route::Rust, Route::Rust));
    assert_eq!((plain.text_runs, plain.declined_text), (0, 2));
    assert_eq!(
        (text.text_runs, text.declined_text, text.dropped_glyphs),
        (2, 0, 0)
    );
    let (plain, text) = (plain.rgba.unwrap(), text.rgba.unwrap());
    assert!(lit(&plain) > 0, "panels drawn");
    assert!(
        lit(&plain) < plain.len() / 4,
        "outside the panels stays transparent"
    );
    let changed = plain
        .chunks_exact(4)
        .zip(text.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 50, "glyph pixels drawn over the rows: {changed}");
}

#[test]
fn legacy_draws_nothing() {
    let gpu = GpuContext::new(&Preference::Software).expect("software adapter");
    let model = UiModel::from_tree_json(TREE, &dump_load_options()).unwrap();
    let legacy = UiSwitch::default();
    let frame = render_page(
        &gpu,
        legacy,
        &model,
        PageId(0x100),
        None,
        None,
        SIZE,
        [0; 4],
    )
    .unwrap();
    assert_eq!(frame.route, Route::LegacySelected);
    assert!(frame.rgba.is_none());
    let missing = render_page(
        &gpu,
        rust(),
        &model,
        PageId(0x999),
        None,
        None,
        SIZE,
        [0; 4],
    )
    .unwrap();
    assert_eq!(missing.route, Route::LayoutIncomplete);
    assert!(missing.rgba.is_none() && missing.layout_note.is_some());
}

struct Fixed(Vec<u32>);
impl TextShaper for Fixed {
    type Error = ();
    fn shape(&mut self, _: &TextPlacement, _: TextPart) -> Result<Option<ShapedRun>, ()> {
        Ok(Some(ShapedRun {
            font: FontId(0),
            raster_height_px: 14,
            glyphs: self
                .0
                .iter()
                .map(|&code| PositionedGlyph {
                    id: GlyphId(code),
                    offset_px: Vec2::new(0.0, 0.0),
                })
                .collect(),
            baseline_px: Vec2::new(0.0, 14.0),
        }))
    }
}

#[test]
fn glyphs_outside_the_stand_in_face_are_dropped_and_counted() {
    let placement = TextPlacement {
        widget: tsfp_ui_model::WidgetId(1),
        text: tsfp_ui_model::TextRef::Id(tsfp_ui_model::TextId(1)),
        font_index: 0,
        value_font_index: 0,
        align: tsfp_ui_model::Align::Start,
        rect_px: tsfp_ui_renderer::Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)).unwrap(),
        color: tsfp_ui_renderer::Color([1.0; 4]),
        value: None,
    };
    let mut shaper = CoveredShaper {
        inner: Fixed(vec![u32::from(b'A'), 0xE9, u32::from(b'B')]),
        glyphs: &GlyphSource::StandIn,
        dropped_glyphs: 0,
    };
    let run = shaper.shape(&placement, TextPart::Label).unwrap().unwrap();
    assert_eq!(run.glyphs.len(), 2);
    assert_eq!(shaper.dropped_glyphs, 1);
    let mut empty = CoveredShaper {
        inner: Fixed(vec![0x01]),
        glyphs: &GlyphSource::StandIn,
        dropped_glyphs: 0,
    };
    assert!(
        empty.shape(&placement, TextPart::Label).unwrap().is_none(),
        "an emptied run declines"
    );
}

/// FABRICATED record with rects and a page lighting the middle of every cell.
fn assets_with_pages() -> TextAssets {
    let mut assets = assets();
    let record = assets.fonts.records.get_mut(&0).unwrap();
    record.rects = (0..tsfp_ui_model::font::RECTS as u16)
        .map(|i| tsfp_ui_model::font::GlyphRect {
            x: (i % 16) * 8,
            y: (i / 16) * 8,
            w: 7,
            h: 7,
            top: 2.0,
        })
        .collect();
    let mut coverage = vec![0u8; 128 * 128];
    for rect in &record.rects {
        for y in 1..6 {
            coverage[(usize::from(rect.y) + y) * 128 + usize::from(rect.x) + 3] = 255;
        }
    }
    assets.font_pages = vec![tsfp_ui_wgpu::original_font::Page {
        width: 128,
        height: 128,
        coverage,
    }];
    assets
}

#[test]
fn matched_pages_replace_the_stand_in_face() {
    let gpu = GpuContext::new(&Preference::Software).expect("software adapter");
    let model = UiModel::from_tree_json(TREE, &dump_load_options()).unwrap();
    let render = |assets: &TextAssets| {
        render_page(
            &gpu,
            rust(),
            &model,
            PageId(0x100),
            None,
            Some(assets),
            SIZE,
            [0; 4],
        )
        .unwrap()
    };
    let original = render(&assets_with_pages());
    assert_eq!(original.glyphs, "original font pages");
    assert_eq!((original.text_runs, original.dropped_glyphs), (2, 0));
    assert!(original.texture.is_some() && original.rgba.is_some());
    assert_eq!(
        render(&assets()).glyphs,
        "stand-in 5x7 face",
        "no pages, no match"
    );
}
