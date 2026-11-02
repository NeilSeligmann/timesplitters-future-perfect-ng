//! One Rust UI page rendered to RGBA for a host overlay (T2189 UI-6 slice 15).
//!
//! Shared by `tsfp-ui-preview` and the Bevy shell (`--ui=rust`): lay the page out
//! (`frontend::page_layout`), shape its rows with `FontTableShaper` over the original font tables
//! and string banks when [`TextAssets`] are given, draw quads and text through
//! `PagePresenter<WgpuUiRenderer>` and read the frame back. A layout-incomplete page or
//! `--ui=legacy` falls back to legacy: nothing is drawn and `rgba` is None.
//!
//! Slice 16: glyph pixels come from the original font pages when the disc gave them (`original_font`, advances from the
//! glyph rects), else from the FABRICATED stand-in face (`standin_font`). Glyphs the source lacks are dropped and counted.
//! [`render_page_on`] draws on any wgpu device and queue (the Bevy shell passes its own render device) and can skip the
//! readback, handing back the target texture instead.

use std::path::Path;

use tsfp_ui_model::anchor::window_viewport;
use tsfp_ui_model::font::{FontSet, FontTableMeasure};
use tsfp_ui_model::framehash::HashCheck;
use tsfp_ui_model::frontend::{self, PageLayoutState};
use tsfp_ui_model::present::PagePresenter;
use tsfp_ui_model::shaper::FontTableShaper;
use tsfp_ui_model::strings::BankStrings;
use tsfp_ui_model::switch::{Route, UiSwitch};
use tsfp_ui_model::{
    PageId, RenderLayout, ShapedRun, TextPart, TextPlacement, TextShaper, UiFrame, UiModel,
    WidgetId,
};
use tsfp_ui_renderer::{Batch, GlyphKey, Vec2, Viewport};
use tsfp_wgpu::context::GpuContext;

use crate::original_font::{OriginalFont, Page};
use crate::present::{WGPU_ORIGINAL_PRESET_GOLDENS, present_and_check_skinned};
use crate::skin_assets::{SkinImages, SkinOverrides};
use crate::{Error, UiBackend, WgpuUiRenderer, standin_font};
use tsfp_ui_model::skin::{OriginalSkin, skin_page};
use tsfp_ui_renderer::Sprite;

/// Original font tables, font page candidates and string banks, read from the owner's disc at run time.
pub struct TextAssets {
    pub fonts: FontSet,
    pub strings: BankStrings,
    /// A8 textures of the bank pak; empty keeps the stand-in face.
    pub font_pages: Vec<Page>,
    /// T2307: frame, fill and highlight textures of the original front end (None keeps the
    /// placeholder panels), and why they are missing.
    pub skin: Option<SkinImages>,
    pub skin_note: Option<String>,
}

impl TextAssets {
    pub fn from_disc(disc: &Path, language: char) -> Result<Self, String> {
        let assets = crate::disc::load(disc, language)?;
        let (skin, skin_note) = match SkinImages::from_disc(disc, &SkinOverrides::from_env()) {
            Ok(images) => (Some(images), None),
            Err(why) => (None, Some(why)),
        };
        Ok(Self {
            fonts: FontSet::load(&assets.image),
            strings: assets.strings,
            font_pages: assets.font_pages,
            skin,
            skin_note,
        })
    }
}

/// Where glyph pixels come from.
pub enum GlyphSource {
    Original(OriginalFont),
    /// T2307: `improved` preset, outline faces fitted into the original glyph boxes.
    #[cfg(feature = "outline-fonts")]
    Outline(crate::outline_font::OutlineFont),
    StandIn,
}

impl GlyphSource {
    /// The original pages when every record with rects matched one, else the stand-in face.
    pub fn for_assets(assets: &TextAssets, window_scale: Vec2) -> Self {
        let font =
            OriginalFont::match_pages(assets.fonts.clone(), &assets.font_pages, window_scale);
        let wanted = assets
            .fonts
            .records
            .values()
            .filter(|r| !r.rects.is_empty())
            .count();
        if wanted > 0 && font.pages.len() == wanted {
            #[cfg(feature = "outline-fonts")]
            if crate::outline_font::selected_from_env() {
                let faces =
                    crate::outline_font::find_faces(&crate::outline_font::font_dir_from_env());
                let original = OriginalFont::match_pages(
                    assets.fonts.clone(),
                    &assets.font_pages,
                    window_scale,
                );
                if let Some(outline) = crate::outline_font::OutlineFont::load(original, &faces) {
                    return GlyphSource::Outline(outline);
                }
            }
            GlyphSource::Original(font)
        } else {
            GlyphSource::StandIn
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            GlyphSource::Original(_) => "original font pages",
            #[cfg(feature = "outline-fonts")]
            GlyphSource::Outline(_) => "outline fonts (improved preset)",
            GlyphSource::StandIn => "stand-in 5x7 face",
        }
    }

    pub fn covers(&self, key: GlyphKey) -> bool {
        match self {
            GlyphSource::Original(font) => font.glyph(key).is_some(),
            #[cfg(feature = "outline-fonts")]
            GlyphSource::Outline(font) => font.covers(key),
            GlyphSource::StandIn => standin_font::glyph(key).is_some(),
        }
    }

    pub(crate) fn register(
        &self,
        renderer: &mut WgpuUiRenderer<'_>,
        keys: &[GlyphKey],
    ) -> Result<Vec<GlyphKey>, Error> {
        match self {
            GlyphSource::Original(font) => font.register(renderer, keys),
            #[cfg(feature = "outline-fonts")]
            GlyphSource::Outline(font) => font.register(renderer, keys),
            GlyphSource::StandIn => standin_font::register(renderer, keys),
        }
    }
}

/// Wraps a shaper and drops glyphs the glyph source cannot draw (a run left empty declines).
pub struct CoveredShaper<'g, S> {
    pub inner: S,
    pub glyphs: &'g GlyphSource,
    pub dropped_glyphs: usize,
}

impl<S: TextShaper> TextShaper for CoveredShaper<'_, S> {
    type Error = S::Error;
    fn clip(&self, placement: &TextPlacement, run: &ShapedRun) -> tsfp_ui_renderer::Rect {
        self.inner.clip(placement, run)
    }
    fn shape(
        &mut self,
        placement: &TextPlacement,
        part: TextPart,
    ) -> Result<Option<ShapedRun>, S::Error> {
        let Some(mut run) = self.inner.shape(placement, part)? else {
            return Ok(None);
        };
        let before = run.glyphs.len();
        let (font, raster_height_px) = (run.font, run.raster_height_px);
        run.glyphs.retain(|g| {
            self.glyphs.covers(GlyphKey {
                font,
                glyph: g.id,
                raster_height_px,
            })
        });
        self.dropped_glyphs += before - run.glyphs.len();
        Ok((!run.glyphs.is_empty()).then_some(run))
    }
}

/// Quad-only frames (no text assets).
pub struct NoText;
impl TextShaper for NoText {
    type Error = std::convert::Infallible;
    fn shape(&mut self, _: &TextPlacement, _: TextPart) -> Result<Option<ShapedRun>, Self::Error> {
        Ok(None)
    }
}

/// What one overlay render produced.
#[derive(Debug)]
pub struct OverlayFrame {
    /// RGBA8 of the target, top-left origin; None when legacy keeps the page or no readback was asked.
    pub rgba: Option<Vec<u8>>,
    /// The drawn target (Rgba8Unorm, viewable as Rgba8UnormSrgb) when the page is on the Rust route.
    pub texture: Option<wgpu::Texture>,
    /// Which glyph pixels the text used.
    pub glyphs: &'static str,
    pub route: Route,
    pub check: HashCheck,
    pub quads: usize,
    /// Skin sprites drawn (0 without skin textures).
    pub sprites: usize,
    pub text_rows: usize,
    pub text_runs: usize,
    pub declined_text: usize,
    pub dropped_glyphs: usize,
    pub layout_note: Option<String>,
}

/// Every glyph key of a built frame, deduplicated.
pub(crate) fn glyph_keys(frame: &UiFrame) -> Vec<GlyphKey> {
    let mut keys: Vec<GlyphKey> = frame
        .batches()
        .iter()
        .filter_map(|batch| match batch {
            Batch::Text(run) => Some(run.glyphs.iter().map(|g| GlyphKey {
                font: run.font,
                glyph: g.id,
                raster_height_px: run.raster_height_px,
            })),
            _ => None,
        })
        .flatten()
        .collect();
    keys.sort_by_key(|k| (k.font.0, k.glyph.0, k.raster_height_px));
    keys.dedup();
    keys
}

/// The shell samples the overlay as a sprite, so it needs TEXTURE_BINDING too.
const TARGET_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT
    .union(wgpu::TextureUsages::TEXTURE_BINDING)
    .union(wgpu::TextureUsages::COPY_SRC)
    .union(wgpu::TextureUsages::COPY_DST);

pub(crate) fn target(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    (width, height): (u32, u32),
    initial: &[u8],
) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ui-overlay"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: TARGET_USAGE,
        view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
    });
    queue.write_texture(
        texture.as_image_copy(),
        initial,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    texture
}

fn present<S: TextShaper>(
    renderer: WgpuUiRenderer<'_>,
    switch: UiSwitch,
    page: PageId,
    complete: bool,
    layout: &RenderLayout,
    shaper: &mut CoveredShaper<'_, S>,
    sprites: &[Sprite],
    viewport: Viewport,
    readback: bool,
) -> Result<Presentation, String>
where
    S::Error: std::fmt::Debug,
{
    let mut presenter = PagePresenter::new(switch, renderer);
    // Shaping pass for the atlas: the renderer refuses a frame with an unregistered glyph.
    let frame = UiFrame::build(layout, shaper).map_err(|e| format!("shape: {e:?}"))?;
    let glyphs = shaper.glyphs;
    glyphs
        .register(&mut presenter.renderer, &glyph_keys(&frame))
        .map_err(|e| format!("glyph atlas: {e:?}"))?;
    let (text_runs, declined_text, dropped_glyphs) =
        (frame.text_runs(), frame.skipped_text, shaper.dropped_glyphs);
    let check = present_and_check_skinned(
        &mut presenter,
        WGPU_ORIGINAL_PRESET_GOLDENS,
        page,
        complete,
        layout,
        sprites,
        shaper,
        viewport,
    )
    .map_err(|e| format!("present: {e:?}"))?;
    let route = presenter
        .log
        .entries()
        .last()
        .map(|e| e.route)
        .ok_or("no route recorded")?;
    let rgba = if route == Route::Rust && readback {
        Some(
            presenter
                .renderer
                .read_target_rgba()
                .map_err(|e| format!("readback: {e:?}"))?,
        )
    } else {
        None
    };
    Ok(Presentation {
        rgba,
        route,
        check,
        text_runs,
        declined_text,
        dropped_glyphs,
    })
}

struct Presentation {
    rgba: Option<Vec<u8>>,
    route: Route,
    check: HashCheck,
    text_runs: usize,
    declined_text: usize,
    dropped_glyphs: usize,
}

/// Renders `page` of `model` at `size` (window pixels, design space 640 x 480 scaled) onto a
/// target cleared to `clear` (transparent for an overlay, opaque for a still) and reads it back.
#[allow(clippy::too_many_arguments)]
pub fn render_page(
    gpu: &GpuContext,
    switch: UiSwitch,
    model: &UiModel,
    page: PageId,
    focused: Option<WidgetId>,
    text: Option<&TextAssets>,
    size: (u32, u32),
    clear: [u8; 4],
) -> Result<OverlayFrame, String> {
    render_page_on(
        &gpu.device,
        &gpu.queue,
        switch,
        model,
        page,
        focused,
        text,
        size,
        clear,
        true,
    )
}

/// [`render_page`] drawn over `backdrop` (RGBA8, `size`, top-left origin) instead of a clear
/// colour: the owner's side-by-side check of the page against a legacy picture (T2307).
#[allow(clippy::too_many_arguments)]
pub fn render_page_over(
    gpu: &GpuContext,
    switch: UiSwitch,
    model: &UiModel,
    page: PageId,
    focused: Option<WidgetId>,
    text: Option<&TextAssets>,
    size: (u32, u32),
    backdrop: &[u8],
) -> Result<OverlayFrame, String> {
    if backdrop.len() != (size.0 * size.1 * 4) as usize {
        return Err(format!("backdrop is not {}x{} RGBA", size.0, size.1));
    }
    render_page_init(
        &gpu.device,
        &gpu.queue,
        switch,
        model,
        page,
        focused,
        text,
        size,
        backdrop,
        true,
    )
}

/// [`render_page`] on any device and queue; without `readback` the frame keeps only the target texture.
#[allow(clippy::too_many_arguments)]
pub fn render_page_on(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    switch: UiSwitch,
    model: &UiModel,
    page: PageId,
    focused: Option<WidgetId>,
    text: Option<&TextAssets>,
    (width, height): (u32, u32),
    clear: [u8; 4],
    readback: bool,
) -> Result<OverlayFrame, String> {
    render_page_init(
        device,
        queue,
        switch,
        model,
        page,
        focused,
        text,
        (width, height),
        &clear.repeat((width * height) as usize),
        readback,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_page_init(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    switch: UiSwitch,
    model: &UiModel,
    page: PageId,
    focused: Option<WidgetId>,
    text: Option<&TextAssets>,
    (width, height): (u32, u32),
    initial: &[u8],
    readback: bool,
) -> Result<OverlayFrame, String> {
    let viewport = window_viewport(width, height).map_err(|e| format!("{e:?}"))?;
    let (complete, layout, layout_note) =
        match frontend::page_layout(model, page, viewport, focused) {
            PageLayoutState::Complete(layout) => (true, layout, None),
            PageLayoutState::Incomplete(why) => (
                false,
                RenderLayout {
                    quads: vec![],
                    texts: vec![],
                },
                Some(format!("{why:?}")),
            ),
        };
    let texture = target(device, queue, (width, height), initial);
    let mut renderer = WgpuUiRenderer::new(device, queue, &texture, UiBackend::Wgpu)
        .map_err(|e| format!("renderer: {e:?}"))?;
    // T2307: the original skin replaces the placeholder panels when its textures loaded.
    let mut sprites = Vec::new();
    let mut layout = layout;
    if let (true, Some(images), Some(model_page)) = (
        complete,
        text.and_then(|t| t.skin.as_ref()),
        model.page(page),
    ) {
        let ids = images
            .register(&mut renderer)
            .map_err(|e| format!("skin textures: {e:?}"))?;
        let skin = OriginalSkin::original();
        if let Some(skinned) =
            skin_page(&skin, &ids, model_page, &layout, viewport, focused, 1.0, 0)
        {
            sprites = skinned.sprites;
            layout = skinned.layout;
        }
    }
    // T2307: the skinned page draws its text at the skin's text scale (glyph pages and pens alike).
    let text_scale = if sprites.is_empty() {
        (1.0, 1.0)
    } else {
        OriginalSkin::original().text_scale
    };
    let scale = tsfp_ui_renderer::Vec2::new(
        viewport.scale() * text_scale.0,
        viewport.scale() * text_scale.1,
    );
    let glyphs = match text {
        Some(assets) => GlyphSource::for_assets(assets, scale),
        None => GlyphSource::StandIn,
    };
    let shown = match text {
        Some(assets) => {
            let measure = FontTableMeasure::new(&assets.fonts, &assets.strings);
            let mut inner = FontTableShaper::new(&measure);
            inner.px_per_design = scale;
            // T2307: rows with the original upper-case flag are shaped upper case.
            if let Some(p) = model.page(page) {
                inner.uppercase = p
                    .widgets
                    .iter()
                    .filter(|w| w.uppercase)
                    .map(|w| w.id)
                    .collect();
            }
            let mut shaper = CoveredShaper {
                inner,
                glyphs: &glyphs,
                dropped_glyphs: 0,
            };
            present(
                renderer,
                switch,
                page,
                complete,
                &layout,
                &mut shaper,
                &sprites,
                viewport,
                readback,
            )?
        }
        None => {
            let mut shaper = CoveredShaper {
                inner: NoText,
                glyphs: &glyphs,
                dropped_glyphs: 0,
            };
            present(
                renderer,
                switch,
                page,
                complete,
                &layout,
                &mut shaper,
                &sprites,
                viewport,
                readback,
            )?
        }
    };
    Ok(OverlayFrame {
        texture: (shown.route == Route::Rust).then_some(texture),
        glyphs: glyphs.name(),
        rgba: shown.rgba,
        route: shown.route,
        check: shown.check,
        quads: layout.quads.len(),
        sprites: sprites.len(),
        text_rows: layout.texts.len(),
        text_runs: shown.text_runs,
        declined_text: shown.declined_text,
        dropped_glyphs: shown.dropped_glyphs,
        layout_note,
    })
}

#[cfg(test)]
mod usage_tests {
    #[test]
    fn overlay_target_is_sampleable() {
        assert!(super::TARGET_USAGE.contains(wgpu::TextureUsages::TEXTURE_BINDING));
        assert!(super::TARGET_USAGE.contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
    }
}
