//! T2312 UI-14: the native HUD drawn through the UI renderer contract (`WgpuUiRenderer`).
//!
//! Input is a [`HudDraw`] (quads, textured sprites by original texture name, text runs in an
//! original font) from `tsfp_ui_model::hud_native::layout`. Textures come from the owner's disc at
//! run time ([`HudTextures::from_disc`], the gun and level paks hold the HUD textures, MEASURED),
//! glyphs from the T2307 font layer ([`TextAssets`]). Nothing is cached on disk or committed.
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use tsfp_format_pak::Pak;
use tsfp_import::source::{self, Platform};
use tsfp_ui_model::anchor::window_viewport;
use tsfp_ui_model::font::FontTableMeasure;
use tsfp_ui_model::hud_native::{HudDraw, HudPoly};
use tsfp_ui_model::shaper::FontTableShaper;
use tsfp_ui_model::{RenderLayout, TextPlacement, TextRef, UiFrame, WidgetId};
use tsfp_ui_renderer::{GlyphKey, Sprite, TextureId, UiRenderer, UvRect, Vec2};
use tsfp_wgpu::frame_layer::{FrameLayer, LayerTarget};

use crate::overlay::{CoveredShaper, GlyphSource, NoText, TextAssets};
use crate::skin_assets::{Image, decode_xbt};
use crate::{UiBackend, WgpuUiRenderer};

/// Renderer ids of the HUD textures (outside the font, stand-in and skin ranges).
pub const HUD_TEXTURE_BASE: u64 = 0x4855_0000;

/// Paks searched for HUD textures, in order. MEASURED on the USA Xbox disc: the HUD textures
/// (`crosshair_scifi`, `newHud_*`, `newAmmo_*`) sit in every Story level pak, the weapon ones also
/// in `pak/gun.pak`; `newHud_powerupCloak` (the rank box panel) only in the Challenge level paks
/// (`pak/challenge/l_65_ch.pak` .. `l_79_ch.pak`, T2312 increment 4). The radar textures (`hudradar*`,
/// `newHud_box`, `newHud_cornerGradient`) sit in the Arcade level paks, `radarspot` only in
/// `pak/arcade/l_103.pak` (T2312 increment 5).
pub const HUD_PAKS: [&str; 6] = [
    "pak/gun.pak",
    "pak/story/l_5_st.pak",
    "pak/story/l_9_st.pak",
    "pak/challenge/l_76_ch.pak",
    "pak/arcade/l_100.pak",
    "pak/arcade/l_103.pak",
];

/// Decoded HUD textures by original texture-table name.
#[derive(Clone, Debug, Default)]
pub struct HudTextures {
    pub images: BTreeMap<String, Image>,
}

impl HudTextures {
    /// Read `names` from the disc's HUD paks. Names not found are left out (the caller reports them).
    pub fn from_disc(disc_path: &Path, names: &[&str]) -> Result<Self, String> {
        let disc = source::open(disc_path, "disc", Platform::Xbox)?;
        let files = disc.list()?;
        let mut images = BTreeMap::new();
        for pak_path in HUD_PAKS {
            if names.iter().all(|n| images.contains_key(*n)) {
                break;
            }
            let Some(entry) = files.iter().find(|f| f.path.eq_ignore_ascii_case(pak_path)) else {
                continue;
            };
            let bytes = disc.read(entry).map_err(|e| format!("{pak_path}: {e}"))?;
            let pak = Pak::parse(&bytes).map_err(|e| format!("{pak_path}: {e}"))?;
            for name in names {
                if images.contains_key(*name) {
                    continue;
                }
                let path = format!("assets/ts/textures/xbox/{name}.xbt");
                if let Some(image) =
                    crate::disc::pak_member(&pak, &path).and_then(|b| decode_xbt(&b))
                {
                    images.insert((*name).to_owned(), image);
                }
            }
        }
        Ok(Self { images })
    }

    fn id(&self, name: &str) -> Option<TextureId> {
        self.images
            .keys()
            .position(|n| n == name)
            .map(|i| TextureId(HUD_TEXTURE_BASE + i as u64))
    }
}

/// What the native HUD draws with: original text assets and HUD textures (either may be absent).
#[derive(Default)]
pub struct HudAssets {
    pub text: Option<TextAssets>,
    pub textures: HudTextures,
}

/// One native HUD frame.
#[derive(Debug)]
pub struct HudOut {
    pub texture: wgpu::Texture,
    pub rgba: Option<Vec<u8>>,
    pub quads: usize,
    pub sprites: usize,
    pub text_runs: usize,
    /// Sprite textures not available (the sprite is left out, never replaced by a stand-in).
    pub missing_textures: Vec<&'static str>,
    pub glyphs: &'static str,
}

const HUD_WIDGET_BASE: u64 = WidgetId::ANON_BASE + 0x4855_0000;

/// A transparent HUD target the shell can sample (Rgba8Unorm, viewable as sRGB).
pub fn new_target(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    (width, height): (u32, u32),
) -> wgpu::Texture {
    crate::overlay::target(
        device,
        queue,
        (width, height),
        &[0, 0, 0, 0].repeat((width * height) as usize),
    )
}

/// Draw `draw` onto a new transparent `size` target on `device` (window pixels; the HUD layout
/// already placed everything). With `readback` the RGBA comes back too (tests, frame comparison).
pub fn render_hud_on(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    draw: &HudDraw,
    assets: &HudAssets,
    (width, height): (u32, u32),
    readback: bool,
) -> Result<HudOut, String> {
    let texture = crate::overlay::target(
        device,
        queue,
        (width, height),
        &[0, 0, 0, 0].repeat((width * height) as usize),
    );
    render_hud_into(device, queue, draw, assets, texture, readback)
}

/// [`render_hud_on`] into an existing target (cleared first). The shell keeps one HUD texture so the
/// sprite's cached bind group stays valid.
pub fn render_hud_into(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    draw: &HudDraw,
    assets: &HudAssets,
    texture: wgpu::Texture,
    readback: bool,
) -> Result<HudOut, String> {
    let (width, height) = (texture.width(), texture.height());
    queue.write_texture(
        texture.as_image_copy(),
        &vec![0u8; (width * height * 4) as usize],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    let viewport = window_viewport(width, height).map_err(|e| format!("{e:?}"))?;
    let mut renderer = WgpuUiRenderer::new(device, queue, &texture, UiBackend::Wgpu)
        .map_err(|e| format!("renderer: {e:?}"))?;
    upload_textures(&mut renderer, &assets.textures, |name| {
        draw.sprites.iter().any(|s| s.texture == name)
            || draw.polys.iter().any(|p| p.texture == name)
    })?;
    let scale = Vec2::new(
        viewport.scale() * draw.text_scale[0],
        viewport.scale() * draw.text_scale[1],
    );
    let glyphs = match &assets.text {
        Some(text) => GlyphSource::for_assets(text, scale),
        None => GlyphSource::StandIn,
    };
    let (frame, missing) = shape(draw, assets, &glyphs, scale)?;
    glyphs
        .register(&mut renderer, &crate::overlay::glyph_keys(&frame))
        .map_err(|e| format!("glyph atlas: {e:?}"))?;
    frame
        .draw(&mut renderer, viewport)
        .map_err(|e| format!("draw: {e:?}"))?;
    let rgba = if readback {
        Some(
            renderer
                .read_target_rgba()
                .map_err(|e| format!("readback: {e:?}"))?,
        )
    } else {
        None
    };
    let out = HudOut {
        rgba,
        quads: frame.quads.len(),
        sprites: frame.sprites.len(),
        text_runs: frame.text_runs(),
        missing_textures: missing,
        glyphs: glyphs.name(),
        texture: texture.clone(),
    };
    drop(renderer);
    Ok(HudOut { texture, ..out })
}

/// Upload the HUD textures `wanted` selects under their fixed renderer ids.
fn upload_textures(
    renderer: &mut WgpuUiRenderer<'_>,
    textures: &HudTextures,
    wanted: impl Fn(&str) -> bool,
) -> Result<(), String> {
    for (index, (name, image)) in textures.images.iter().enumerate() {
        if wanted(name) {
            renderer
                .upload_rgba_filtered(
                    TextureId(HUD_TEXTURE_BASE + index as u64),
                    image.width,
                    image.height,
                    &image.rgba,
                )
                .map_err(|e| format!("HUD texture {name}: {e:?}"))?;
        }
    }
    Ok(())
}

/// Shape one [`HudDraw`] into a UI frame (quads, sprites, original-font text) at glyph `scale`.
/// Returns the frame and the sprite textures that are not available (left out, never stood in).
fn shape(
    draw: &HudDraw,
    assets: &HudAssets,
    glyphs: &GlyphSource,
    scale: Vec2,
) -> Result<(UiFrame, Vec<&'static str>), String> {
    let mut sprites = Vec::new();
    let mut missing = Vec::new();
    let mut blends = Vec::new();
    let mut mode = tsfp_ui_renderer::BlendMode::Alpha;
    for sprite in &draw.sprites {
        if assets.textures.id(sprite.texture).is_some() && sprite.blend != mode {
            mode = sprite.blend;
            blends.push((sprites.len(), mode));
        }
        match assets.textures.id(sprite.texture) {
            Some(texture) => sprites.push(Sprite {
                rect_px: if sprite.keep_aspect {
                    let image = &assets.textures.images[sprite.texture];
                    fit(sprite.rect_px, image.width as f32 / image.height as f32)
                } else {
                    sprite.rect_px
                },
                uv: UvRect {
                    min: Vec2::new(sprite.uv[0], sprite.uv[1]),
                    max: Vec2::new(sprite.uv[2], sprite.uv[3]),
                },
                color: sprite.color,
                texture,
            }),
            None => missing.push(sprite.texture),
        }
    }
    // Texts at their own glyph scale (HudText::scale) are shaped per scale and appended with a scaled font id.
    let unit = |s: [f32; 2]| (s[0] - 1.0).abs() < 1e-4 && (s[1] - 1.0).abs() < 1e-4;
    // Additive texts (the original's blend mode 2, T2312 message titles) come last, behind one blend switch.
    let scales_of = |additive: bool| {
        let mut scales: Vec<[f32; 2]> = Vec::new();
        for text in draw
            .texts
            .iter()
            .filter(|t| t.additive == additive && !unit(t.scale))
        {
            if !scales.contains(&text.scale) {
                scales.push(text.scale);
            }
        }
        scales
    };
    let mut frame = shape_texts(
        draw,
        assets,
        glyphs,
        scale,
        |t| !t.additive && unit(t.scale),
        draw.quads.clone(),
    )?;
    for group in scales_of(false) {
        let scaled = Vec2::new(scale.x * group[0], scale.y * group[1]);
        let part = shape_texts(
            draw,
            assets,
            glyphs,
            scaled,
            |t| !t.additive && t.scale == group,
            Vec::new(),
        )?;
        frame.append_text(part, |font| {
            crate::original_font::scaled_font_id(font.0 as u32, group)
        });
    }
    if draw.texts.iter().any(|t| t.additive) {
        frame
            .text_blends
            .push((frame.text_runs(), tsfp_ui_renderer::BlendMode::Additive));
        let part = shape_texts(
            draw,
            assets,
            glyphs,
            scale,
            |t| t.additive && unit(t.scale),
            Vec::new(),
        )?;
        frame.append_text(part, |font| font);
        for group in scales_of(true) {
            let scaled = Vec2::new(scale.x * group[0], scale.y * group[1]);
            let part = shape_texts(
                draw,
                assets,
                glyphs,
                scaled,
                |t| t.additive && t.scale == group,
                Vec::new(),
            )?;
            frame.append_text(part, |font| {
                crate::original_font::scaled_font_id(font.0 as u32, group)
            });
        }
    }
    frame.sprites = sprites;
    frame.sprite_blends = blends;
    let mut mode = tsfp_ui_renderer::BlendMode::Alpha;
    for poly in &draw.polys {
        match assets.textures.id(poly.texture) {
            Some(texture) => {
                if poly.blend != mode {
                    mode = poly.blend;
                    frame.poly_blends.push((frame.polys.len(), mode));
                }
                let [u0, v0, u1, v1] = poly.uv;
                frame.polys.push(tsfp_ui_renderer::Poly {
                    corners_px: poly.corners_px,
                    uv: [
                        Vec2::new(u0, v0),
                        Vec2::new(u1, v0),
                        Vec2::new(u0, v1),
                        Vec2::new(u1, v1),
                    ],
                    color: poly.color,
                    texture,
                })
            }
            None => missing.push(poly.texture),
        }
    }
    Ok((frame, missing))
}

/// Align the original text-top metric inside the typed row rectangle.
fn text_line_rect(
    mut rect: tsfp_ui_renderer::Rect,
    align: tsfp_ui_model::Align,
    line_height: f32,
) -> tsfp_ui_renderer::Rect {
    let free = rect.size.y - line_height;
    rect.origin.y += match align {
        tsfp_ui_model::Align::Start => 0.0,
        tsfp_ui_model::Align::Center => free * 0.5,
        tsfp_ui_model::Align::End => free,
    };
    rect
}

/// Shape the texts of `draw` whose scale `wanted` selects, at glyph `scale`, with `quads` under them.
fn shape_texts(
    draw: &HudDraw,
    assets: &HudAssets,
    glyphs: &GlyphSource,
    scale: Vec2,
    wanted: impl Fn(&tsfp_ui_model::hud_native::HudText) -> bool,
    quads: Vec<tsfp_ui_renderer::Quad>,
) -> Result<UiFrame, String> {
    let mut dynamic = BTreeMap::new();
    let mut clip_bounds = BTreeMap::new();
    let texts: Vec<TextPlacement> = draw
        .texts
        .iter()
        .enumerate()
        .filter(|(_, text)| wanted(text))
        .map(|(index, text)| {
            let widget = WidgetId(HUD_WIDGET_BASE + index as u64);
            dynamic.insert(widget, text.bytes.clone());
            if let Some(clip) = text.clip_px {
                clip_bounds.insert(widget, clip);
            }
            let mut rect_px = text.rect_px;
            if let Some(line_height) = assets
                .text
                .as_ref()
                .and_then(|assets| assets.fonts.text_top_height(text.font))
            {
                let line_height = line_height * scale.y;
                rect_px = text_line_rect(rect_px, text.vertical_align, line_height);
            }
            TextPlacement {
                widget,
                text: TextRef::Dynamic,
                font_index: text.font,
                value_font_index: text.font,
                align: text.align,
                rect_px,
                color: text.color,
                value: None,
            }
        })
        .collect();
    let layout = RenderLayout { quads, texts };
    match &assets.text {
        Some(text) => {
            let measure = FontTableMeasure::new(&text.fonts, &text.strings);
            let mut inner = FontTableShaper::new(&measure);
            inner.px_per_design = scale;
            inner.dynamic = dynamic;
            inner.clip_bounds = clip_bounds;
            let mut shaper = CoveredShaper {
                inner,
                glyphs,
                dropped_glyphs: 0,
            };
            UiFrame::build(&layout, &mut shaper).map_err(|e| format!("shape: {e:?}"))
        }
        None => {
            let mut shaper = CoveredShaper {
                inner: NoText,
                glyphs,
                dropped_glyphs: 0,
            };
            UiFrame::build(&layout, &mut shaper).map_err(|e| format!("shape: {e:?}"))
        }
    }
}

/// Renderer state that depends on the engine frame size.
struct LayerSized {
    size: (u32, u32),
    renderer: WgpuUiRenderer<'static>,
    glyphs: GlyphSource,
    registered: HashSet<GlyphKey>,
}

/// T2312: the native HUD drawn inside the engine frame (T2309 frame layer contract): the game's
/// renderer hands over its encoder and finished frame, the layer records the HUD into it with alpha
/// blending, after the scene and post passes. `source(width, height)` returns this frame's
/// [`HudDraw`] laid out for the frame size (`None` or empty draws nothing).
pub struct HudLayer<F> {
    original_font: bool,
    assets: &'static HudAssets,
    source: F,
    sized: Option<LayerSized>,
    /// Renderer for the view-pass target (the grade's source surface, its own size).
    sized_view: Option<LayerSized>,
    /// Layer frame whose view-pass sprites went in before the grade (else they are drawn with the HUD).
    view_pass_frame: Option<u64>,
    /// T2336: the display update's sample, its view pass already drawn before the display regrade.
    display_rest: Option<HudDraw>,
    failed: bool,
    /// Frames the HUD was drawn on.
    pub drawn: u64,
    slide_logs: u32,
    turn_logs: u32,
    display: DisplayHud,
}

/// Owned post-grade HUD snapshots. View-pass sprites remain in the guest's grade source.
#[derive(Default)]
struct DisplayHud {
    /// Sealed draws; T2336: view-pass sprites lead `sprites` (their count is the third field) so they share its tracks.
    pending: std::collections::VecDeque<(u64, Option<HudDraw>, usize)>,
    view_count: usize,
    present: Option<u64>,
    current: Option<HudDraw>,
    origin: Option<std::time::Instant>,
    clock: tsfp_ui_model::animation_clock::AnimationClock,
    sprites: Vec<tsfp_ui_model::animation_clock::AnimationTrack>,
    texts: Vec<tsfp_ui_model::animation_clock::AnimationTrack>,
    polys: Vec<tsfp_ui_model::animation_clock::AnimationTrack>,
}

impl DisplayHud {
    fn seal(&mut self, present: u64, draw: Option<HudDraw>) {
        if self.pending.len() == 4096 {
            self.pending.pop_front();
        }
        let mut view = 0;
        let draw = draw.map(|mut draw| {
            view = draw.view_pass.len();
            let mut sprites = std::mem::take(&mut draw.view_pass);
            sprites.append(&mut draw.sprites);
            draw.sprites = sprites;
            draw
        });
        self.pending.push_back((present, draw, view));
    }

    fn sample(&mut self, present: u64, now: std::time::Instant) -> Option<HudDraw> {
        use tsfp_ui_model::animation_clock::{AnimationMode, AnimationTrack, AnimationValues};
        if self.present != Some(present) {
            while self.pending.front().is_some_and(|(p, _, _)| *p < present) {
                self.pending.pop_front();
            }
            let (draw, view) = match self.pending.front() {
                Some((p, _, _)) if *p == present => {
                    let (_, draw, view) = self.pending.pop_front().expect("front checked");
                    (draw, view)
                }
                _ => (None, 0),
            };
            self.view_count = view;
            let old = self.current.as_ref();
            if old.is_none() || draw.is_none() || self.present.is_some_and(|p| present <= p) {
                self.clock.reset();
            }
            self.clock.mode = AnimationMode::DisplayRate;
            let origin = *self.origin.get_or_insert(now);
            self.clock
                .publish(now.saturating_duration_since(origin), present as f64 / 60.0);
            if let Some(new) = &draw {
                self.sprites
                    .resize(new.sprites.len(), AnimationTrack::default());
                self.texts
                    .resize(new.texts.len(), AnimationTrack::default());
                for (i, sprite) in new.sprites.iter().enumerate() {
                    let stable = old.and_then(|d| d.sprites.get(i)).is_some_and(|p| {
                        let mut identity = p.clone();
                        identity.color = sprite.color;
                        identity.rect_px.origin = sprite.rect_px.origin;
                        identity == *sprite && slides(p.rect_px, sprite.rect_px)
                    });
                    let value = AnimationValues {
                        fade: sprite.color.0[3],
                        slide: [sprite.rect_px.origin.x, sprite.rect_px.origin.y],
                        ..Default::default()
                    };
                    if stable {
                        self.sprites[i].publish(value);
                    } else {
                        self.sprites[i].snap(value);
                    }
                }
                self.polys
                    .resize(new.polys.len(), AnimationTrack::default());
                for (i, poly) in new.polys.iter().enumerate() {
                    let turned = turned_square(poly);
                    let stable = old.and_then(|d| d.polys.get(i)).is_some_and(|p| {
                        let same = p.element == poly.element
                            && p.texture == poly.texture
                            && p.uv == poly.uv
                            && p.blend == poly.blend;
                        match (turned_square(p), turned) {
                            (Some(before), Some(now)) => same && before.same_pivot(&now),
                            _ => same && p.corners_px == poly.corners_px,
                        }
                    });
                    let value = AnimationValues {
                        fade: poly.color.0[3],
                        radar_sweep: turned.map_or(0.0, |t| t.degrees),
                        ..Default::default()
                    };
                    if stable {
                        self.polys[i].publish(value);
                    } else {
                        self.polys[i].snap(value);
                    }
                }
                for (i, text) in new.texts.iter().enumerate() {
                    let stable = old.and_then(|d| d.texts.get(i)).is_some_and(|p| {
                        let mut identity = p.clone();
                        identity.color = text.color;
                        identity.rect_px.origin = text.rect_px.origin;
                        identity.clip_px = text.clip_px;
                        identity == *text && slides(p.rect_px, text.rect_px)
                    });
                    let value = AnimationValues {
                        fade: text.color.0[3],
                        slide: [text.rect_px.origin.x, text.rect_px.origin.y],
                        ..Default::default()
                    };
                    if stable {
                        self.texts[i].publish(value);
                    } else {
                        self.texts[i].snap(value);
                    }
                }
            }
            self.current = draw;
            self.present = Some(present);
        }
        let mut draw = self.current.clone()?;
        let at = self
            .clock
            .sample(now.saturating_duration_since(self.origin?));
        for (sprite, track) in draw.sprites.iter_mut().zip(&self.sprites) {
            let value = track.sample(at);
            sprite.color.0[3] = value.fade;
            sprite.rect_px.origin = Vec2::new(value.slide[0], value.slide[1]);
        }
        for (poly, track) in draw.polys.iter_mut().zip(&self.polys) {
            let value = track.sample(at);
            poly.color.0[3] = value.fade;
            if let Some(turned) = turned_square(poly) {
                turned.turn(&mut poly.corners_px, value.radar_sweep - turned.degrees);
            }
        }
        for (text, track) in draw.texts.iter_mut().zip(&self.texts) {
            let value = track.sample(at);
            let shift = (
                value.slide[0] - text.rect_px.origin.x,
                value.slide[1] - text.rect_px.origin.y,
            );
            text.color.0[3] = value.fade;
            text.rect_px.origin = Vec2::new(value.slide[0], value.slide[1]);
            if let Some(clip) = &mut text.clip_px {
                clip.origin = Vec2::new(clip.origin.x + shift.0, clip.origin.y + shift.1);
            }
        }
        let rest = draw.sprites.split_off(self.view_count.min(draw.sprites.len()));
        draw.view_pass = std::mem::replace(&mut draw.sprites, rest);
        Some(draw)
    }
}

/// A quad that is a square turned about its centre in per-axis scaled space (the radar sweep
/// `hudradarcentre`: corners `(c, s)`, `(s, -c)`, ... times `(sx, sy)`).
#[derive(Clone, Copy, Debug)]
struct TurnedSquare {
    centre: Vec2,
    scale: Vec2,
    degrees: f32,
}

impl TurnedSquare {
    fn same_pivot(&self, other: &Self) -> bool {
        let near = |a: f32, b: f32| (a - b).abs() <= 1e-2 * a.abs().max(b.abs());
        (self.centre.x - other.centre.x).abs() <= 0.5
            && (self.centre.y - other.centre.y).abs() <= 0.5
            && near(self.scale.x, other.scale.x)
            && near(self.scale.y, other.scale.y)
    }

    /// Turn the corners about the centre by `degrees` in the normalised space.
    fn turn(&self, corners: &mut [Vec2; 4], degrees: f32) {
        let (sin, cos) = degrees.to_radians().sin_cos();
        for corner in corners {
            let x = (corner.x - self.centre.x) / self.scale.x;
            let y = (corner.y - self.centre.y) / self.scale.y;
            *corner = Vec2::new(
                self.centre.x + (x * cos - y * sin) * self.scale.x,
                self.centre.y + (x * sin + y * cos) * self.scale.y,
            );
        }
    }
}

fn turned_square(poly: &HudPoly) -> Option<TurnedSquare> {
    let c = &poly.corners_px;
    let centre = Vec2::new(
        (c[0].x + c[1].x + c[2].x + c[3].x) / 4.0,
        (c[0].y + c[1].y + c[2].y + c[3].y) / 4.0,
    );
    let d = |i: usize| (c[i].x - centre.x, c[i].y - centre.y);
    let (d0, d1, d2, d3) = (d(0), d(1), d(2), d(3));
    let scale = Vec2::new(d0.0.hypot(d1.0), d0.1.hypot(d1.1));
    if !(scale.x > 1e-3 && scale.y > 1e-3) {
        return None;
    }
    let n = |v: (f32, f32)| (v.0 / scale.x, v.1 / scale.y);
    let (n0, n1, n2, n3) = (n(d0), n(d1), n(d2), n(d3));
    let length = n0.0 * n0.0 + n0.1 * n0.1;
    let close =
        |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() <= 1e-3 && (a.1 - b.1).abs() <= 1e-3;
    // Adjacent corners orthogonal and equal, opposite corners mirrored: a turned square.
    let square = (n0.0 * n1.0 + n0.1 * n1.1).abs() <= 1e-3 * length
        && ((n1.0 * n1.0 + n1.1 * n1.1) - length).abs() <= 1e-3 * length
        && close(n2, (-n1.0, -n1.1))
        && close(n3, (-n0.0, -n0.1));
    square.then(|| TurnedSquare {
        centre,
        scale,
        degrees: n0.1.atan2(n0.0).to_degrees(),
    })
}

/// T2242: a same-content item that moved by no more than its own size in one guest frame is
/// a slide (message titles, notices, pickup stack shrink) and interpolates; larger moves are
/// reflows or new placements and snap. Unmoved items always qualify.
fn slides(previous: tsfp_ui_renderer::Rect, current: tsfp_ui_renderer::Rect) -> bool {
    let limit = current.size.x.max(current.size.y);
    (current.origin.x - previous.origin.x).abs() <= limit
        && (current.origin.y - previous.origin.y).abs() <= limit
}

impl<F: FnMut(u32, u32) -> Option<HudDraw> + Send> HudLayer<F> {
    pub fn new(assets: &'static HudAssets, source: F) -> Self {
        Self {
            original_font: false,
            assets,
            source,
            sized: None,
            sized_view: None,
            view_pass_frame: None,
            display_rest: None,
            failed: false,
            drawn: 0,
            slide_logs: 0,
            turn_logs: 0,
            display: DisplayHud::default(),
        }
    }

    /// Original font pages only, independent of the optional outline-font preset.
    pub fn new_original(assets: &'static HudAssets, source: F) -> Self {
        let mut layer = Self::new(assets, source);
        layer.original_font = true;
        layer
    }

    fn sized(
        &mut self,
        target: &LayerTarget<'_>,
        text_scale: [f32; 2],
        view: bool,
    ) -> Result<&mut LayerSized, String> {
        let size = (target.target.width(), target.target.height());
        let reuse = if view { &self.sized_view } else { &self.sized }
            .as_ref()
            .is_some_and(|s| s.size == size);
        if reuse {
            let slot = if view {
                &mut self.sized_view
            } else {
                &mut self.sized
            };
            let sized = slot.as_mut().expect("checked");
            sized
                .renderer
                .retarget(target.target)
                .map_err(|e| format!("retarget: {e:?}"))?;
            return Ok(sized);
        }
        let mut renderer = WgpuUiRenderer::new_owned(target.device, target.queue, target.target)
            .map_err(|e| format!("renderer: {e:?}"))?;
        upload_textures(&mut renderer, &self.assets.textures, |_| true)?;
        let viewport = window_viewport(size.0, size.1).map_err(|e| format!("{e:?}"))?;
        let scale = Vec2::new(
            viewport.scale() * text_scale[0],
            viewport.scale() * text_scale[1],
        );
        let glyphs = match &self.assets.text {
            Some(text) if self.original_font => {
                GlyphSource::Original(crate::original_font::OriginalFont::match_pages(
                    text.fonts.clone(),
                    &text.font_pages,
                    scale,
                ))
            }
            Some(text) => GlyphSource::for_assets(text, scale),
            None => GlyphSource::StandIn,
        };
        let slot = if view {
            &mut self.sized_view
        } else {
            &mut self.sized
        };
        Ok(slot.insert(LayerSized {
            size,
            renderer,
            glyphs,
            registered: HashSet::new(),
        }))
    }

    fn draw(&mut self, target: LayerTarget<'_>, draw: &HudDraw, view: bool) -> Result<(), String> {
        let assets = self.assets;
        let sized = self.sized(&target, draw.text_scale, view)?;
        let viewport = window_viewport(sized.size.0, sized.size.1).map_err(|e| format!("{e:?}"))?;
        let scale = Vec2::new(
            viewport.scale() * draw.text_scale[0],
            viewport.scale() * draw.text_scale[1],
        );
        let (frame, _missing) = shape(draw, assets, &sized.glyphs, scale)?;
        let keys = crate::overlay::glyph_keys(&frame);
        if keys.iter().any(|k| !sized.registered.contains(k)) {
            sized
                .glyphs
                .register(&mut sized.renderer, &keys)
                .map_err(|e| format!("glyph atlas: {e:?}"))?;
            sized.registered.extend(keys);
        }
        let renderer = &mut sized.renderer;
        renderer
            .begin_frame(viewport)
            .map_err(|e| format!("begin: {e:?}"))?;
        let recorded = renderer
            .submit(&frame.batches())
            .and_then(|()| renderer.end_frame_into(target.encoder));
        if let Err(e) = recorded {
            renderer.abort_frame();
            return Err(format!("draw: {e:?}"));
        }
        Ok(())
    }
}

impl<F: FnMut(u32, u32) -> Option<HudDraw> + Send> HudLayer<F> {
    /// Record an owned draw directly, without guest-latch or interpolation handling.
    pub fn record(&mut self, target: LayerTarget<'_>, draw: &HudDraw, view: bool) -> bool {
        if self.failed {
            return false;
        }
        match self.draw(target, draw, view) {
            Ok(()) => true,
            Err(message) => {
                eprintln!("tsfp-ui-wgpu: native HUD layer: {message}; the HUD layer stops drawing");
                self.failed = true;
                false
            }
        }
    }
}

impl<F: FnMut(u32, u32) -> Option<HudDraw> + Send> FrameLayer for HudLayer<F> {
    /// The view-pass sprites (the weapon reticle) into the grade's source surface, once per frame.
    fn encode_before_grade(&mut self, target: LayerTarget<'_>) -> bool {
        if self.failed || self.view_pass_frame == Some(target.frame) {
            return false;
        }
        let Some(draw) = (self.source)(target.target.width(), target.target.height()) else {
            return false;
        };
        if draw.view_pass.is_empty() {
            return false;
        }
        let frame = target.frame;
        let view = HudDraw {
            sprites: draw.view_pass,
            text_scale: draw.text_scale,
            ..HudDraw::default()
        };
        let drew = self.record(target, &view, true);
        if drew {
            self.view_pass_frame = Some(frame);
        }
        drew
    }

    fn encode_display_view_pass(&mut self, target: LayerTarget<'_>) -> bool {
        if self.failed {
            return false;
        }
        let Some(mut draw) = self.display.sample(target.present, std::time::Instant::now()) else {
            return false;
        };
        let view = HudDraw {
            sprites: std::mem::take(&mut draw.view_pass),
            text_scale: draw.text_scale,
            ..HudDraw::default()
        };
        self.display_rest = Some(draw);
        !view.is_empty() && self.record(target, &view, true)
    }

    fn encode_display(&mut self, target: LayerTarget<'_>) -> bool {
        if self.failed {
            return false;
        }
        // Without a display regrade this update the view pass stays the guest-rate one inside the clean scene.
        let Some(mut draw) = self
            .display_rest
            .take()
            .or_else(|| self.display.sample(target.present, std::time::Instant::now()))
        else {
            return false;
        };
        draw.view_pass.clear();
        if draw.is_empty() {
            return false;
        }
        if self.slide_logs < 48 && std::env::var_os("TSFP_UI_DISPLAY_LOG").is_some_and(|v| v == "1")
        {
            // Diagnostic only: turns and texts drawn between their guest-frame values (T2242).
            let sealed_polys = self
                .display
                .current
                .as_ref()
                .map_or(&[][..], |d| &d.polys[..]);
            if self.turn_logs < 24 {
                if let Some((shown, sealed)) = draw
                    .polys
                    .iter()
                    .zip(sealed_polys)
                    .find(|(a, b)| a.corners_px != b.corners_px)
                {
                    if let (Some(a), Some(b)) = (turned_square(shown), turned_square(sealed)) {
                        eprintln!(
                            "tsfp-ui-wgpu: hud display turn present {} {} at {} deg (guest {} deg)",
                            target.present, shown.texture, a.degrees, b.degrees
                        );
                        self.turn_logs += 1;
                    }
                }
            }
            let current = self
                .display
                .current
                .as_ref()
                .map_or(&[][..], |d| &d.texts[..]);
            for (shown, sealed) in draw
                .texts
                .iter()
                .zip(current)
                .filter(|(a, b)| a.rect_px.origin != b.rect_px.origin)
            {
                eprintln!(
                    "tsfp-ui-wgpu: hud display slide present {} text {:?} at {} {} (guest {} {})",
                    target.present,
                    String::from_utf8_lossy(&shown.bytes),
                    shown.rect_px.origin.x,
                    shown.rect_px.origin.y,
                    sealed.rect_px.origin.x,
                    sealed.rect_px.origin.y
                );
                self.slide_logs += 1;
                break;
            }
        }
        let drew = self.record(target, &draw, false);
        self.drawn += u64::from(drew);
        drew
    }

    fn encode(&mut self, target: LayerTarget<'_>) -> bool {
        if self.failed {
            return false;
        }
        let Some(mut draw) = (self.source)(target.target.width(), target.target.height()) else {
            if tsfp_wgpu::frame_layer::display_rate() {
                self.display.seal(target.present, None);
            }
            return false;
        };
        // View-pass sprites already drawn before this frame's grade stay out; without a grade pass
        // (no render targets, `no_grade_pass`) they are drawn here, after the scene.
        let view_pass = std::mem::take(&mut draw.view_pass);
        if self.view_pass_frame != Some(target.frame) {
            let mut sprites = view_pass;
            sprites.append(&mut draw.sprites);
            draw.sprites = sprites;
        } else if tsfp_wgpu::frame_layer::display_rate() {
            // T2336: kept apart for the display regrade, which redraws it under the native grade.
            draw.view_pass = view_pass;
        }
        if tsfp_wgpu::frame_layer::display_rate() {
            let wanted = !draw.is_empty();
            self.display.seal(target.present, Some(draw));
            return wanted;
        }
        if draw.is_empty() {
            return false;
        }
        let drew = self.record(target, &draw, false);
        self.drawn += u64::from(drew);
        drew
    }
}

fn fit(rect: tsfp_ui_renderer::Rect, aspect: f32) -> tsfp_ui_renderer::Rect {
    let (w, h) = (rect.size.x, rect.size.y);
    let size = if w / h > aspect {
        Vec2::new(h * aspect, h)
    } else {
        Vec2::new(w, w / aspect)
    };
    tsfp_ui_renderer::Rect {
        origin: Vec2::new(
            rect.origin.x + (w - size.x) / 2.0,
            rect.origin.y + (h - size.y) / 2.0,
        ),
        size,
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0 on FABRICATED inputs: a quad, a sprite from a made-up texture and stand-in text.
    #[test]
    fn centred_font_line_may_be_taller_than_its_row_and_top_aligned_lines_stay_put() {
        // Tier 0, FABRICATED geometry: the font line exceeds its owning row.
        let row =
            tsfp_ui_renderer::Rect::new(Vec2::new(10.0, 100.0), Vec2::new(60.0, 16.0)).unwrap();
        let aligned = super::text_line_rect(row, tsfp_ui_model::Align::Center, 20.0);
        assert_eq!(aligned.origin.y, 98.0);
        assert_eq!(aligned.origin.y + 10.0, row.origin.y + row.size.y * 0.5);
        assert_eq!(
            super::text_line_rect(row, tsfp_ui_model::Align::Start, 20.0),
            row
        );
        assert_eq!(
            super::text_line_rect(row, tsfp_ui_model::Align::End, 20.0)
                .origin
                .y,
            96.0
        );
    }
    use super::*;
    use tsfp_ui_model::hud_native::{FULL_UV, HudElement, HudSprite, HudText};
    use tsfp_ui_model::model::Align;
    use tsfp_ui_renderer::{Color, Quad, Rect};

    #[test]
    fn display_hud_fades_only_stable_owned_identity_and_hides() {
        let mut state = DisplayHud::default();
        let now = std::time::Instant::now();
        let draw = |alpha, texture| HudDraw {
            sprites: vec![HudSprite {
                element: HudElement::Crosshair,
                texture,
                rect_px: Rect::new(Vec2::new(10.0, 10.0), Vec2::new(8.0, 8.0)).unwrap(),
                color: Color([1.0, 1.0, 1.0, alpha]),
                keep_aspect: false,
                blend: tsfp_ui_renderer::BlendMode::Alpha,
                uv: FULL_UV,
            }],
            ..Default::default()
        };
        state.seal(1, Some(draw(0.0, "first")));
        state.sample(1, now).unwrap();
        state.seal(2, Some(draw(1.0, "first")));
        state
            .sample(2, now + std::time::Duration::from_millis(20))
            .unwrap();
        for _ in 0..240 {
            let at = now + std::time::Duration::from_millis(30);
            assert_eq!(state.sample(2, at).unwrap().sprites[0].color.0[3], 0.5);
        }
        state.seal(3, Some(draw(0.25, "new")));
        assert_eq!(
            state
                .sample(3, now + std::time::Duration::from_millis(40))
                .unwrap()
                .sprites[0]
                .color
                .0[3],
            0.25
        );
        state.seal(4, None);
        assert!(
            state
                .sample(4, now + std::time::Duration::from_millis(60))
                .is_none()
        );
    }

    #[test]
    fn display_hud_slides_moved_text_and_snaps_jumps() {
        // Tier 0, FABRICATED message title: same bytes/font, origin moved per guest frame.
        let mut state = DisplayHud::default();
        let now = std::time::Instant::now();
        let ms = std::time::Duration::from_millis;
        let draw = |x: f32| HudDraw {
            texts: vec![HudText {
                element: HudElement::Messages,
                bytes: b"TITLE".to_vec(),
                font: 3,
                rect_px: Rect::new(Vec2::new(x, 40.0), Vec2::new(50.0, 10.0)).unwrap(),
                clip_px: Some(Rect::new(Vec2::new(x - 5.0, 40.0), Vec2::new(60.0, 10.0)).unwrap()),
                align: Align::Start,
                vertical_align: Align::Start,
                color: Color([1.0, 1.0, 1.0, 1.0]),
                scale: [1.0, 1.0],
                additive: true,
            }],
            ..Default::default()
        };
        state.seal(1, Some(draw(100.0)));
        state.sample(1, now).unwrap();
        state.seal(2, Some(draw(110.0)));
        assert_eq!(
            state.sample(2, now + ms(20)).unwrap().texts[0]
                .rect_px
                .origin
                .x,
            100.0
        );
        let mid = state.sample(2, now + ms(30)).unwrap();
        assert_eq!(
            (
                mid.texts[0].rect_px.origin.x,
                mid.texts[0].clip_px.unwrap().origin.x
            ),
            (105.0, 100.0)
        );
        assert_eq!(
            state.sample(2, now + ms(40)).unwrap().texts[0],
            draw(110.0).texts[0]
        );
        state.seal(3, Some(draw(400.0)));
        assert_eq!(
            state.sample(3, now + ms(50)).unwrap().texts[0],
            draw(400.0).texts[0]
        );
    }

    #[test]
    fn display_hud_turns_the_radar_sweep_along_the_shortest_arc() {
        // Tier 0, FABRICATED sweep built like hud_radar's quad, non-uniform axis scale.
        let ms = std::time::Duration::from_millis;
        let quad = |degrees: f32| {
            let (s, c) = degrees.to_radians().sin_cos();
            let (cx, cy, sx, sy) = (100.0, 50.0, 20.0, 10.0);
            let p = |x: f32, y: f32| Vec2::new(cx + x * sx, cy + y * sy);
            HudDraw {
                polys: vec![HudPoly {
                    element: HudElement::Radar,
                    texture: "hudradarcentre",
                    corners_px: [p(c, s), p(s, -c), p(-s, c), p(-c, -s)],
                    color: Color([1.0, 1.0, 1.0, 0.5]),
                    uv: FULL_UV,
                    blend: tsfp_ui_renderer::BlendMode::Alpha,
                }],
                ..Default::default()
            }
        };
        let angle = |d: &HudDraw| super::turned_square(&d.polys[0]).unwrap().degrees;
        for (from, to, mid) in [(10.0f32, 30.0f32, 20.0f32), (350.0, 10.0, 0.0)] {
            let mut state = DisplayHud::default();
            let now = std::time::Instant::now();
            state.seal(1, Some(quad(from)));
            state.sample(1, now).unwrap();
            state.seal(2, Some(quad(to)));
            state.sample(2, now + ms(20)).unwrap();
            let shown = state.sample(2, now + ms(30)).unwrap();
            let error = (angle(&shown) - mid + 180.0).rem_euclid(360.0) - 180.0;
            assert!(error.abs() < 1e-3, "{from}->{to}: {}", angle(&shown));
            assert_eq!(state.sample(2, now + ms(40)).unwrap().polys, quad(to).polys);
        }
        // A non-square quad (rim arrow shape) is never turned, just drawn as sealed.
        let mut arrow = quad(0.0);
        arrow.polys[0].corners_px[0].x += 3.0;
        assert!(super::turned_square(&arrow.polys[0]).is_none());
    }

    fn gpu() -> Option<tsfp_wgpu::context::GpuContext> {
        tsfp_wgpu::context::GpuContext::new(&tsfp_wgpu::context::Preference::Software).ok()
    }

    /// Real data (owner's disc, `TSFP_UI_DISC`): every pak holding each name of `TSFP_HUD_TEXTURE_CENSUS`
    /// (comma list). Run with `--ignored --nocapture` to pick `HUD_PAKS` entries for new HUD textures.
    #[test]
    #[ignore]
    fn texture_pak_census() {
        let (Ok(disc), Ok(names)) = (
            std::env::var("TSFP_UI_DISC"),
            std::env::var("TSFP_HUD_TEXTURE_CENSUS"),
        ) else {
            eprintln!("TSFP_UI_DISC or TSFP_HUD_TEXTURE_CENSUS unset, skipped");
            return;
        };
        let disc = source::open(Path::new(&disc), "disc", Platform::Xbox).unwrap();
        let files = disc.list().unwrap();
        for entry in files
            .iter()
            .filter(|f| f.path.to_ascii_lowercase().ends_with(".pak"))
        {
            let Ok(bytes) = disc.read(entry) else {
                continue;
            };
            let Ok(pak) = Pak::parse(&bytes) else {
                continue;
            };
            let found: Vec<&str> = names
                .split(',')
                .filter(|n| {
                    crate::disc::pak_member(&pak, &format!("assets/ts/textures/xbox/{n}.xbt"))
                        .is_some()
                })
                .collect();
            if !found.is_empty() {
                println!("{} {:?}", entry.path, found);
            }
        }
    }

    #[test]
    fn draws_quads_sprites_and_text_or_reports_missing() {
        let Some(gpu) = gpu() else {
            eprintln!("no wgpu adapter, skipped");
            return;
        };
        let mut textures = HudTextures::default();
        textures.images.insert(
            "fabricated".to_owned(),
            Image {
                width: 2,
                height: 2,
                rgba: vec![255; 16],
            },
        );
        let assets = HudAssets {
            text: None,
            textures,
        };
        let rect = |x: f32, y: f32| Rect::new(Vec2::new(x, y), Vec2::new(8.0, 8.0)).unwrap();
        let draw = HudDraw {
            quads: vec![Quad {
                rect_px: rect(0.0, 0.0),
                color: Color([1.0, 0.0, 0.0, 1.0]),
            }],
            sprites: vec![
                HudSprite {
                    element: HudElement::Crosshair,
                    texture: "fabricated",
                    rect_px: rect(16.0, 0.0),
                    color: Color([1.0; 4]),
                    keep_aspect: false,
                    blend: tsfp_ui_renderer::BlendMode::Alpha,
                    uv: FULL_UV,
                },
                HudSprite {
                    element: HudElement::Ammo,
                    texture: "absent",
                    rect_px: rect(32.0, 0.0),
                    color: Color([1.0; 4]),
                    keep_aspect: true,
                    blend: tsfp_ui_renderer::BlendMode::Alpha,
                    uv: FULL_UV,
                },
            ],
            texts: vec![HudText {
                clip_px: None,
                additive: false,
                element: HudElement::Ammo,
                bytes: b"16".to_vec(),
                font: 0,
                rect_px: Rect::new(Vec2::new(0.0, 16.0), Vec2::new(40.0, 16.0)).unwrap(),
                align: Align::Start,
                vertical_align: Align::Start,
                color: Color([1.0; 4]),
                scale: [1.0, 1.0],
            }],
            text_scale: [1.0, 1.0],
            view_pass: Vec::new(),
            // A diamond (a square turned 45 degrees) around (50, 30), additive.
            polys: vec![tsfp_ui_model::hud_native::HudPoly {
                element: HudElement::Vitals,
                texture: "fabricated",
                corners_px: [
                    Vec2::new(50.0, 24.0),
                    Vec2::new(56.0, 30.0),
                    Vec2::new(44.0, 30.0),
                    Vec2::new(50.0, 36.0),
                ],
                color: Color([0.0, 1.0, 0.0, 1.0]),
                uv: tsfp_ui_model::hud_native::FULL_UV,
                blend: tsfp_ui_renderer::BlendMode::Additive,
            }],
        };
        let out = render_hud_on(&gpu.device, &gpu.queue, &draw, &assets, (64, 48), true).unwrap();
        assert_eq!((out.quads, out.sprites), (1, 1));
        assert_eq!(out.missing_textures, ["absent"]);
        let rgba = out.rgba.unwrap();
        let px = |x: usize, y: usize| &rgba[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(px(4, 4)[0], 255, "red quad");
        assert_eq!(px(20, 4), [255, 255, 255, 255], "white sprite");
        assert_eq!(px(36, 4)[3], 0, "missing texture draws nothing");
        assert_eq!(px(60, 40)[3], 0, "transparent elsewhere");
        assert_eq!(px(50, 30)[1], 255, "rotated quad centre");
        assert_eq!(
            px(45, 25)[3],
            0,
            "outside the diamond, inside its bounding box"
        );
    }

    #[test]
    fn layer_composites_over_the_engine_frame_and_skips_empty_frames() {
        let Some(gpu) = gpu() else {
            eprintln!("no wgpu adapter, skipped");
            return;
        };
        let mut textures = HudTextures::default();
        textures.images.insert(
            "fabricated".to_owned(),
            Image {
                width: 2,
                height: 2,
                rgba: vec![255; 16],
            },
        );
        let assets: &'static HudAssets = Box::leak(Box::new(HudAssets {
            text: None,
            textures,
        }));
        let rect = Rect::new(Vec2::new(16.0, 0.0), Vec2::new(8.0, 8.0)).unwrap();
        let mut frames = vec![
            HudDraw::default(),
            HudDraw {
                sprites: vec![HudSprite {
                    element: HudElement::Crosshair,
                    texture: "fabricated",
                    rect_px: rect,
                    color: Color([1.0; 4]),
                    keep_aspect: false,
                    blend: tsfp_ui_renderer::BlendMode::Alpha,
                    uv: FULL_UV,
                }],
                ..HudDraw::default()
            },
        ];
        let mut layer = HudLayer::new(assets, move |_, _| frames.pop());
        // The engine frame: opaque blue.
        let scene: Vec<u8> = [0u8, 0, 255, 255].repeat(64 * 48);
        let target = crate::overlay::target(&gpu.device, &gpu.queue, (64, 48), &scene);
        let mut run = |layer: &mut HudLayer<_>| {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let drew = layer.encode(LayerTarget {
                device: &gpu.device,
                queue: &gpu.queue,
                encoder: &mut encoder,
                target: &target,
                frame: 0,
                present: 0,
            });
            gpu.queue.submit([encoder.finish()]);
            drew
        };
        assert!(run(&mut layer), "a HUD frame is drawn");
        assert!(!run(&mut layer), "an empty HUD frame draws nothing");
        assert!(!run(&mut layer), "no published state draws nothing");
        assert_eq!(layer.drawn, 1);
        let rgba = WgpuUiRenderer::new(&gpu.device, &gpu.queue, &target, UiBackend::Wgpu)
            .unwrap()
            .read_target_rgba()
            .unwrap();
        let px = |x: usize, y: usize| &rgba[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(px(20, 4), [255, 255, 255, 255], "HUD sprite over the scene");
        assert_eq!(px(4, 4), [0, 0, 255, 255], "scene kept (load, never clear)");
    }

    #[test]
    fn display_samples_keep_the_view_pass_apart_for_the_display_regrade() {
        // Tier 0, FABRICATED sprites (T2336): the view pass shares the sprite tracks but comes back separated.
        let sprite = |x: f32| HudSprite {
            element: HudElement::Crosshair,
            texture: "fabricated",
            rect_px: Rect::new(Vec2::new(x, 0.0), Vec2::new(8.0, 8.0)).unwrap(),
            color: Color([1.0; 4]),
            keep_aspect: false,
            blend: tsfp_ui_renderer::BlendMode::Alpha,
            uv: FULL_UV,
        };
        let mut display = DisplayHud::default();
        let draw = HudDraw {
            view_pass: vec![sprite(16.0)],
            sprites: vec![sprite(32.0), sprite(48.0)],
            ..HudDraw::default()
        };
        display.seal(7, Some(draw));
        let shown = display.sample(7, std::time::Instant::now()).expect("sealed frame");
        assert_eq!(shown.view_pass.len(), 1);
        assert_eq!(shown.view_pass[0].rect_px.origin.x, 16.0);
        let rest: Vec<f32> = shown.sprites.iter().map(|s| s.rect_px.origin.x).collect();
        assert_eq!(rest, [32.0, 48.0]);
    }

    #[test]
    fn ordered_view_pass_prevents_grade_and_present_duplicates() {
        let Some(gpu) = gpu() else {
            eprintln!("no wgpu adapter, skipped");
            return;
        };
        let mut textures = HudTextures::default();
        textures.images.insert(
            "fabricated".to_owned(),
            Image {
                width: 2,
                height: 2,
                rgba: vec![255; 16],
            },
        );
        let assets: &'static HudAssets = Box::leak(Box::new(HudAssets {
            text: None,
            textures,
        }));
        let sprite = |x: f32| HudSprite {
            element: HudElement::Crosshair,
            texture: "fabricated",
            rect_px: Rect::new(Vec2::new(x, 0.0), Vec2::new(8.0, 8.0)).unwrap(),
            color: Color([1.0; 4]),
            keep_aspect: false,
            blend: tsfp_ui_renderer::BlendMode::Alpha,
            uv: FULL_UV,
        };
        let mut layer = HudLayer::new(assets, move |_, _| {
            Some(HudDraw {
                view_pass: vec![sprite(16.0)],
                sprites: vec![sprite(32.0)],
                ..HudDraw::default()
            })
        });
        let scene: Vec<u8> = [0u8, 0, 255, 255].repeat(64 * 48);
        let surface = crate::overlay::target(&gpu.device, &gpu.queue, (64, 48), &scene);
        let shown = crate::overlay::target(&gpu.device, &gpu.queue, (64, 48), &scene);
        let mut run = |layer: &mut HudLayer<_>, target: &wgpu::Texture, frame: u64, phase: u8| {
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            let t = LayerTarget {
                device: &gpu.device,
                queue: &gpu.queue,
                encoder: &mut encoder,
                target,
                frame,
                present: 0,
            };
            let drew = match phase {
                0 => layer.encode(t),
                1 => layer.encode_before_grade(t),
                _ => layer.encode_view_pass(t),
            };
            gpu.queue.submit([encoder.finish()]);
            drew
        };
        assert!(
            run(&mut layer, &surface, 5, 2),
            "view pass at the original ordered boundary"
        );
        assert!(
            !run(&mut layer, &surface, 5, 2),
            "ordered boundary once per frame"
        );
        assert!(
            !run(&mut layer, &surface, 5, 1),
            "grade fallback must not duplicate ordered draw"
        );
        assert!(run(&mut layer, &shown, 5, 0));
        let read = |t: &wgpu::Texture| {
            WgpuUiRenderer::new(&gpu.device, &gpu.queue, t, UiBackend::Wgpu)
                .unwrap()
                .read_target_rgba()
                .unwrap()
        };
        let px = |rgba: &[u8], x: usize| rgba[x * 4..x * 4 + 4].to_vec();
        let (a, b) = (read(&surface), read(&shown));
        assert_eq!(
            (px(&a, 20), px(&a, 36)),
            (vec![255; 4], vec![0, 0, 255, 255]),
            "surface: view pass only"
        );
        assert_eq!(
            (px(&b, 20), px(&b, 36)),
            (vec![0, 0, 255, 255], vec![255; 4]),
            "shown frame: no second reticle"
        );
        // A frame without a grade pass: the view-pass sprite is drawn with the HUD.
        assert!(run(&mut layer, &shown, 6, 0));
        assert_eq!(px(&read(&shown), 20), vec![255; 4]);
    }
}
