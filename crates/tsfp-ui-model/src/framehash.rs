//! Frame-hash scaffold for the `original` preset (T2189 UI-6 slice 12). A page is rendered
//! offscreen by `PagePresenter<CpuRasterRenderer>` and hashed. The hash is checked against a
//! golden table keyed by page and viewport size. The table is empty: original goldens need a
//! capture of the legacy path on the same CPU reference (host run, T2247 recapture), so every
//! page reports `Pending` until then. Pending is not a pass.

use crate::bridge::RenderLayout;
use crate::frame::TextShaper;
use crate::model::PageId;
use crate::present::{PagePresenter, PresentError, Presented};
use crate::raster::{CpuRasterRenderer, RasterError, RasterImage};
use tsfp_ui_renderer::{GlyphAtlas, Viewport};

/// FNV-1a 64 over width, height (little endian) and the RGBA8 bytes. Stable across runs
/// and platforms, so goldens can live in the tracked table.
pub fn frame_hash(image: &RasterImage) -> u64 {
    frame_hash_rgba(image.width, image.height, &image.rgba)
}

/// `frame_hash` over any tightly packed RGBA8 frame, so a GPU readback (`tsfp-ui-wgpu`)
/// hashes the same way as the CPU reference.
pub fn frame_hash_rgba(width: u32, height: u32, rgba: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let header = [width.to_le_bytes(), height.to_le_bytes()];
    for byte in header.iter().flatten().chain(rgba) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// One golden: the hash of a page at a viewport size on the CPU reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Golden {
    pub page: PageId,
    pub width: u32,
    pub height: u32,
    pub hash: u64,
}

/// Original-preset goldens. Empty until a legacy capture exists (see module doc).
pub const ORIGINAL_PRESET_GOLDENS: &[Golden] = &[];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashCheck {
    Match,
    Mismatch {
        expected: u64,
        actual: u64,
    },
    /// No golden for this page and size: unverified, recorded as debt.
    Pending {
        actual: u64,
    },
    /// The page took a legacy route, so the Rust frame was not drawn.
    NotRust,
}

pub fn check(goldens: &[Golden], page: PageId, image: &RasterImage) -> HashCheck {
    check_rgba(goldens, page, image.width, image.height, &image.rgba)
}

/// `check` over any tightly packed RGBA8 frame (CPU reference or a GPU readback).
pub fn check_rgba(
    goldens: &[Golden],
    page: PageId,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> HashCheck {
    let actual = frame_hash_rgba(width, height, rgba);
    let golden = goldens
        .iter()
        .find(|g| g.page == page && g.width == width && g.height == height);
    match golden {
        None => HashCheck::Pending { actual },
        Some(g) if g.hash == actual => HashCheck::Match,
        Some(g) => HashCheck::Mismatch {
            expected: g.hash,
            actual,
        },
    }
}

/// Present `page` on the CPU reference and check its frame hash against `goldens`.
pub fn present_and_check<A: GlyphAtlas, S: TextShaper>(
    presenter: &mut PagePresenter<CpuRasterRenderer<A>>,
    goldens: &[Golden],
    page: PageId,
    layout_complete: bool,
    layout: &RenderLayout,
    shaper: &mut S,
    viewport: Viewport,
) -> Result<HashCheck, PresentError<S::Error, RasterError>> {
    match presenter.present(page, layout_complete, layout, shaper, viewport)? {
        Presented::Legacy(_) => Ok(HashCheck::NotRust),
        Presented::Rust { .. } => {
            let image = presenter
                .renderer
                .finished()
                .expect("end_frame stores the frame");
            Ok(check(goldens, page, image))
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED layout and font tables: end to end through the production shaper,
    //! the presenter and the CPU reference.
    use super::*;
    use crate::bridge::TextPlacement;
    use crate::font::FontTableMeasure;
    use crate::model::{Align, TextId, TextRef, WidgetId};
    use crate::raster::BoxAtlas;
    use crate::shaper::FontTableShaper;
    use crate::shaper::tests::{Strings, fonts};
    use crate::switch::UiSwitch;
    use tsfp_ui_renderer::{Color, Quad, Rect, Vec2};

    fn viewport() -> Viewport {
        let size = Vec2::new(64.0, 48.0);
        Viewport::new(size, 1.0, Rect::new(Vec2::new(0.0, 0.0), size).unwrap()).unwrap()
    }

    fn layout(text_x: f32) -> RenderLayout {
        let panel = Rect::new(Vec2::new(4.0, 4.0), Vec2::new(40.0, 20.0)).unwrap();
        let text = Rect::new(Vec2::new(text_x, 6.0), Vec2::new(30.0, 10.0)).unwrap();
        RenderLayout {
            quads: vec![Quad {
                rect_px: panel,
                color: Color([0.0, 0.0, 1.0, 0.5]),
            }],
            texts: vec![TextPlacement {
                widget: WidgetId(1),
                text: TextRef::Id(TextId(0)),
                font_index: 0,
                value_font_index: 0,
                align: Align::Start,
                rect_px: text,
                color: Color([1.0; 4]),
                value: None,
            }],
        }
    }

    fn run(switch: UiSwitch, goldens: &[Golden], text_x: f32) -> (HashCheck, Option<RasterImage>) {
        let (fonts, strings) = (fonts(), Strings(vec![b"AB".to_vec()]));
        let measure = FontTableMeasure::new(&fonts, &strings);
        let mut shaper = FontTableShaper::new(&measure);
        let renderer = CpuRasterRenderer::new(BoxAtlas { width_ratio: 0.5 }, [0, 0, 0, 255]);
        let mut presenter = PagePresenter::new(switch, renderer);
        let checked = present_and_check(
            &mut presenter,
            goldens,
            PageId(0x20),
            true,
            &layout(text_x),
            &mut shaper,
            viewport(),
        )
        .unwrap();
        (checked, presenter.renderer.finished().cloned())
    }

    fn rust() -> UiSwitch {
        UiSwitch::parse(None, ["--ui=rust"]).unwrap().0
    }

    #[test]
    fn presenter_frame_renders_offscreen_and_is_deterministic() {
        let (first, image) = run(rust(), ORIGINAL_PRESET_GOLDENS, 6.0);
        let image = image.expect("rust route renders");
        assert_eq!((image.width, image.height), (64, 48));
        // Panel pixel: blue at half alpha over black. Outside: clear colour.
        assert_eq!(image.pixel(40, 20), [0, 0, 128, 255]);
        assert_eq!(image.pixel(50, 30), [0, 0, 0, 255]);
        // Some glyph box pixel is white.
        assert!(image.rgba.chunks(4).any(|p| p == [255; 4]));
        let HashCheck::Pending { actual } = first else {
            panic!("empty goldens must be pending, got {first:?}");
        };
        assert_eq!(actual, frame_hash(&image));
        let (second, _) = run(rust(), ORIGINAL_PRESET_GOLDENS, 6.0);
        assert_eq!(second, first);
    }

    #[test]
    fn golden_match_and_one_pixel_shift_mismatches() {
        let (HashCheck::Pending { actual }, _) = run(rust(), &[], 6.0) else {
            panic!("expected pending");
        };
        let goldens = [Golden {
            page: PageId(0x20),
            width: 64,
            height: 48,
            hash: actual,
        }];
        assert_eq!(run(rust(), &goldens, 6.0).0, HashCheck::Match);
        assert_eq!(
            run(rust(), &goldens, 7.0).0,
            HashCheck::Mismatch {
                expected: actual,
                actual: match run(rust(), &[], 7.0).0 {
                    HashCheck::Pending { actual } => actual,
                    other => panic!("{other:?}"),
                },
            }
        );
        let other_size = [Golden {
            width: 32,
            ..goldens[0]
        }];
        assert!(matches!(
            run(rust(), &other_size, 6.0).0,
            HashCheck::Pending { .. }
        ));
    }

    #[test]
    fn legacy_default_is_not_rust_and_draws_nothing() {
        let (checked, image) = run(UiSwitch::default(), &[], 6.0);
        assert_eq!(checked, HashCheck::NotRust);
        assert!(image.is_none());
    }

    #[test]
    fn hash_covers_size_and_pixels() {
        let image = RasterImage {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        };
        let wide = RasterImage {
            width: 2,
            height: 1,
            rgba: vec![0; 8],
        };
        let tall = RasterImage {
            width: 1,
            height: 2,
            ..wide.clone()
        };
        let lit = RasterImage {
            rgba: vec![0, 0, 0, 1],
            ..image.clone()
        };
        assert_ne!(frame_hash(&wide), frame_hash(&tall));
        assert_ne!(frame_hash(&image), frame_hash(&lit));
    }
}
