//! T2309 UI-11: the Rust UI page drawn inside the engine frame.
//!
//! [`PageLayer`] is a `tsfp_wgpu::frame_layer::FrameLayer`: at the end of every presented frame
//! the game's own renderer (the wgpu `Replayer`) hands it its command encoder and the finished
//! frame (scene and post passes done, in the guest's pass order), and the layer records the
//! page's batches into that same encoder with the original blend modes (alpha, and the additive
//! focused-row fill). So the UI is composited by the engine renderer in the frame's pass order,
//! like the original front end draws its pages after the 3D background, instead of a picture the
//! window shell lays over the frame (the T2189 overlay sprite, TEMPORARY-SHIM removed by T2309).
//!
//! What to draw comes from a [`LayerState`] source the shell supplies every frame (the page owner:
//! `None` while legacy keeps the page, F9 or another page on top). The layer keeps one
//! `WgpuUiRenderer` with the skin textures and glyph pages uploaded, recreated only when the
//! frame size changes.
//!
//! T2313: before the page's fill the layer runs the original fill's back-buffer pass
//! ([`crate::backdrop`], `0x204F0`: the scene inside the box blurred and darkened). The state is
//! latched per guest frame: [`FrameLayer::seal`] (guest thread, when the host presents guest frame
//! N) asks the source once, and the render thread draws present N with exactly that state, so the
//! original page draw's skip decision and the Rust draw belong to the same guest frame. The highlight
//! pulse runs on the page's own clock: guest frames the page was shown (Rust-owned), not on the
//! renderer's presented-frame count.
use crate::backdrop::{BackdropParams, BackdropPass};
use crate::overlay::{CoveredShaper, GlyphSource, NoText, TextAssets, glyph_keys};
use crate::skin_assets::SkinImages;
use crate::WgpuUiRenderer;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tsfp_ui_model::anchor::window_viewport;
use tsfp_ui_model::font::FontTableMeasure;
use tsfp_ui_model::frontend::{self, PageLayoutState};
use tsfp_ui_model::shaper::FontTableShaper;
use tsfp_ui_model::skin::{OriginalSkin, SkinTexture, SkinTextures, skin_page_with_pulses};
use tsfp_ui_model::{PageId, TextRef, TextShaper, UiFrame, UiModel, WidgetId};
use tsfp_ui_model::default_style::{self, DefaultPage, DefaultTextures};
use tsfp_ui_model::bridge::TextPlacement;
use tsfp_ui_model::default_widgets;
use tsfp_ui_model::list_rows::{ListRowKind, ListRows, draw_list_rows};
use tsfp_ui_model::progress_summary::draw_progress_summary;
use tsfp_ui_model::ingame_rows::{draw_objectives, draw_scoreboard_with_fixed_width};
use tsfp_ui_model::list_rows::IngameRow;
use tsfp_ui_renderer::{GlyphKey, TextureId, UiRenderer, Vec2};
use tsfp_wgpu::frame_layer::{FrameLayer, LayerTarget};

/// What the page owner wants drawn this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct LayerState {
    pub focused: Option<WidgetId>,
    /// Page fade (`page + 0xF4` in the original), 1.0 = fully shown.
    pub fade: f32,
    /// T2311: the page to draw instead of the layer's fixed page (built from the live page record
    /// through the shell's page registry). `None` draws the fixed page.
    pub live: Option<Arc<LiveContent>>,
    /// T1916: the pointer in design units (the Map Maker pointer of a default-style page), drawn over the text when the
    /// page has a pointer picture; read every guest frame (the live content only changes with the page record).
    pub pointer: Option<(f32, f32)>,
    /// T1916: the guest's frame counter (`[0x7497A8]`) of this frame: the default style's highlight pulses on it
    /// (`0x72B70`), not on the page's shown-frames clock.
    pub guest_frame: Option<u64>,
}

/// T2311: a page built at run time from the live page record (`tsfp_ui_input::live_page`): its
/// model, page id and the runtime text of its rows (level and profile names).
#[derive(Clone, Debug, PartialEq)]
pub struct LiveContent {
    pub model: UiModel,
    pub page: PageId,
    pub dynamic: BTreeMap<WidgetId, Vec<u8>>,
    /// T2311 increment 3: the Story level rows or Challenge set rows of the page (`list_rows`),
    /// drawn by Rust after the fill and before the focused-row highlight.
    pub list: Option<ListRows>,
    /// T1916 increment 2: the page uses the framework default style (no frame or highlight callback, the Map Maker
    /// pages): drawn by `tsfp_ui_model::default_style` instead of the front-end skin.
    pub default_page: Option<DefaultPage>,
}

/// Renderer state that depends on the frame size.
struct Sized {
    size: (u32, u32),
    renderer: WgpuUiRenderer<'static>,
    skin: Option<SkinTextures>,
    glyphs: GlyphSource,
    registered: HashSet<GlyphKey>,
    /// T2311: list row textures uploaded by table name (None: not on the disc).
    named: HashMap<String, Option<TextureId>>,
}

/// Renderer ids of the list row textures (above the skin's `SKIN_BASE` page).
const NAMED_BASE: u64 = 0x4C52_0000;

/// One Rust UI page drawn by the engine renderer at the end of every frame.
pub struct PageLayer<F> {
    model: &'static UiModel,
    page: PageId,
    text: Option<&'static TextAssets>,
    state: F,
    sized: Option<Sized>,
    /// First error, printed once (the frame goes on without the page).
    failed: bool,
    /// Frames the page was drawn on.
    pub drawn: u64,
    /// The last composed frame wanted the page (its state was Some).
    pub last_wanted: bool,
    /// Latched states by guest present number (see [`FrameLayer::seal`]).
    latch: FrameLatch,
    backdrop: Option<BackdropPass>,
    display: DisplayPage,
}

/// A guest frame's state, latched on the guest thread.
#[derive(Clone, Debug)]
struct Latched {
    present: u64,
    state: Option<LayerState>,
    clock: u64,
}

/// Latched frames kept for the render thread. T2315: the live queue is unbounded by default, so the
/// render thread can trail the guest by many frames (a PNG dump or a pipeline compile stalls it);
/// with the old depth of 16 those presents lost their latched state and showed neither the original
/// nor the Rust page (the intermittent blank Player Progress frames of T2311). Frames are dropped
/// from the front only once composed; this bound only caps memory (states are small, `live` is an `Arc`).
const LATCH_DEPTH: usize = 4096;

/// The per guest frame states: sealed on the guest thread, taken by the render thread for the
/// same present number. Also counts the guest frames the page was shown (the pulse clock).
#[derive(Debug, Default)]
struct FrameLatch {
    sealed: VecDeque<Latched>,
    ever_sealed: bool,
    clock: u64,
    /// Highest present evicted unconsumed by the depth bound (0: none).
    evicted_up_to: u64,
    /// Presents composed after their latched state was evicted (drawn with the nearest later state).
    pub missed: u64,
}

impl FrameLatch {
    fn seal(&mut self, present: u64, state: Option<LayerState>) {
        if state.is_some() {
            self.clock += 1;
        }
        self.ever_sealed = true;
        if self.sealed.len() == LATCH_DEPTH {
            if let Some(old) = self.sealed.pop_front() {
                self.evicted_up_to = self.evicted_up_to.max(old.present);
            }
        }
        self.sealed.push_back(Latched {
            present,
            state,
            clock: self.clock,
        });
    }

    /// The state latched for `present`, `None` when the guest sealed none (the original draw kept
    /// the page); `Err(())` before any seal (a host that does not seal: ask the source now).
    fn take(&mut self, present: u64) -> Result<(Option<LayerState>, u64), ()> {
        if !self.ever_sealed {
            return Err(());
        }
        while self.sealed.front().is_some_and(|l| l.present < present) {
            self.sealed.pop_front();
        }
        match self.sealed.front() {
            Some(l) if l.present == present => {
                let latched = self.sealed.pop_front().expect("checked");
                Ok((latched.state, latched.clock))
            }
            // Evicted before it was composed: the nearest later guest frame's decision is the best
            // guess (a page shown in both keeps showing instead of a blank frame).
            Some(l) if present <= self.evicted_up_to => {
                self.missed += 1;
                Ok((l.state.clone(), l.clock))
            }
            _ => Ok((None, self.clock)),
        }
    }
}

/// Retained owned page state: repeated display draws never consume a guest latch again.
#[derive(Default)]
struct DisplayPage {
    present: Option<u64>,
    state: Option<LayerState>,
    clock: u64,
    origin: Option<std::time::Instant>,
    animation: tsfp_ui_model::animation_clock::AnimationClock,
    fade: tsfp_ui_model::animation_clock::AnimationTrack,
    pulses: tsfp_ui_model::animation_clock::AnimationTrack,
    style_pulses: tsfp_ui_model::animation_clock::AnimationTrack,
}

impl DisplayPage {
    fn publish(&mut self, present: u64, state: Option<LayerState>, clock: u64, now: std::time::Instant) {
        use tsfp_ui_model::animation_clock::{AnimationMode, AnimationValues};
        let same_page = match (&self.state, &state) {
            (Some(old), Some(new)) => old.focused == new.focused && match (&old.live, &new.live) {
                (Some(a), Some(b)) => a == b,
                (None, None) => true,
                _ => false,
            },
            _ => false,
        };
        // Presentation identity alone survives a simulation rewind. The typed guest
        // stamp and shown-page clock must also advance before retaining history.
        let same_epoch = self.present.is_none_or(|old| present > old)
            && clock > self.clock
            && match (&self.state, &state) {
                (Some(old), Some(new)) => match (old.guest_frame, new.guest_frame) {
                    (Some(a), Some(b)) => b > a,
                    (None, None) => true,
                    _ => false,
                },
                _ => false,
            };
        let continuous = same_page && same_epoch;
        if !continuous {
            self.animation.reset();
        }
        let origin = *self.origin.get_or_insert(now);
        self.animation.mode = AnimationMode::DisplayRate;
        self.animation.publish(now.saturating_duration_since(origin), present as f64 / 60.0);
        let values = AnimationValues { fade: state.as_ref().map_or(1.0, |s| s.fade), ..Default::default() };
        if continuous { self.fade.publish(values); } else { self.fade.snap(values); }
        let frame = state.as_ref().and_then(|s| s.guest_frame).unwrap_or(clock);
        let pulses = AnimationValues {
            fade: OriginalSkin::pulse(frame),
            pulse: OriginalSkin::text_pulse(frame),
            ..Default::default()
        };
        if continuous { self.pulses.publish(pulses); } else { self.pulses.snap(pulses); }
        let content = state.as_ref().and_then(|s| s.live.as_deref());
        let style_pulses = AnimationValues {
            fade: content.and_then(|c| c.default_page.as_ref())
                .map_or(1.0, |p| p.style.highlight.pulse(frame)),
            pulse: content.and_then(|c| c.list.as_ref()).and_then(|l| l.options.as_ref())
                .map_or(1.0, |o| o.pulse.at(frame)),
            ..Default::default()
        };
        if continuous { self.style_pulses.publish(style_pulses); } else { self.style_pulses.snap(style_pulses); }
        self.present = Some(present);
        self.state = state;
        self.clock = clock;
    }

    fn sample_style_pulses(&self, now: std::time::Instant) -> (f32, f32) {
        let stamp = now.saturating_duration_since(self.origin.expect("published page"));
        let values = self.style_pulses.sample(self.animation.sample(stamp));
        (values.fade, values.pulse)
    }

    fn sample_pulses(&self, now: std::time::Instant) -> (f32, f32) {
        let stamp = now.saturating_duration_since(self.origin.expect("published page"));
        let values = self.pulses.sample(self.animation.sample(stamp));
        (values.fade, values.pulse)
    }

    fn sample(&self, now: std::time::Instant) -> Option<LayerState> {
        let mut state = self.state.clone()?;
        let stamp = now.saturating_duration_since(self.origin?);
        state.fade = self.fade.sample(self.animation.sample(stamp)).fade;
        Some(state)
    }
}

impl<F: FnMut() -> Option<LayerState> + Send> PageLayer<F> {
    pub fn new(
        model: &'static UiModel,
        page: PageId,
        text: Option<&'static TextAssets>,
        state: F,
    ) -> Self {
        Self {
            model,
            page,
            text,
            state,
            sized: None,
            failed: false,
            drawn: 0,
            last_wanted: false,
            latch: FrameLatch::default(),
            backdrop: None,
            display: DisplayPage::default(),
        }
    }

    fn sized(&mut self, target: &LayerTarget<'_>) -> Result<&mut Sized, String> {
        let size = (target.target.width(), target.target.height());
        if self.sized.as_ref().is_some_and(|s| s.size == size) {
            let sized = self.sized.as_mut().expect("checked");
            sized
                .renderer
                .retarget(target.target)
                .map_err(|e| format!("retarget: {e:?}"))?;
            return Ok(sized);
        }
        let mut renderer = WgpuUiRenderer::new_owned(target.device, target.queue, target.target)
            .map_err(|e| format!("renderer: {e:?}"))?;
        let skin = match self.text.and_then(|t| t.skin.as_ref()) {
            Some(images) => Some(register_skin(images, &mut renderer)?),
            None => None,
        };
        let viewport = window_viewport(size.0, size.1).map_err(|e| format!("{e:?}"))?;
        let text_scale = if skin.is_some() {
            OriginalSkin::original().text_scale
        } else {
            (1.0, 1.0)
        };
        let scale = Vec2::new(
            viewport.scale() * text_scale.0,
            viewport.scale() * text_scale.1,
        );
        let glyphs = match self.text {
            Some(assets) => GlyphSource::for_assets(assets, scale),
            None => GlyphSource::StandIn,
        };
        Ok(self.sized.insert(Sized {
            size,
            renderer,
            skin,
            glyphs,
            registered: HashSet::new(),
            named: HashMap::new(),
        }))
    }

    /// The state of the frame being composed and its pulse clock: the latched one of this guest
    /// present once frames are sealed, else (no sealing host, tests) the source asked now.
    fn state_for(&mut self, present: u64, frame: u64) -> (Option<LayerState>, u64) {
        match self.latch.take(present) {
            Ok(latched) => latched,
            Err(()) => ((self.state)(), frame),
        }
    }

    fn draw(&mut self, target: LayerTarget<'_>, state: LayerState, clock: u64, pulses: Option<(f32, f32)>, style_pulses: Option<(f32, f32)>) -> Result<bool, String> {
        let live = state.live.clone();
        let (model, page, text) = match live.as_deref() {
            Some(content) => (&content.model, content.page, self.text),
            None => (self.model, self.page, self.text),
        };
        let frame_number = clock;
        let pulse_frame = state.guest_frame.unwrap_or(clock);
        let (highlight_pulse, text_pulse) = pulses.unwrap_or_else(||
            (OriginalSkin::pulse(pulse_frame), OriginalSkin::text_pulse(pulse_frame)));
        let sized = self.sized(&target)?;
        let viewport = window_viewport(sized.size.0, sized.size.1).map_err(|e| format!("{e:?}"))?;
        let layout_state = frontend::page_layout(model, page, viewport, state.focused);
        let PageLayoutState::Complete(layout) = layout_state else {
            if std::env::var_os("TSFP_UI_DRAW_LOG").is_some() {
                eprintln!("tsfp-ui: in-frame page {page:?} not laid out: {layout_state:?}");
            }
            return Ok(false);
        };
        let skin = OriginalSkin::original();
        let default_page = live.as_deref().and_then(|c| c.default_page.as_ref());
        let skinned = match (&sized.skin, model.page(page)) {
            (Some(ids), Some(model_page)) if default_page.is_some() => ids.get(SkinTexture::Line).and_then(|white| {
                default_style::style_page_with_pulse(
                    &default_page.expect("checked").style,
                    DefaultTextures { white },
                    model_page,
                    &layout,
                    viewport,
                    state.focused,
                    state.fade,
                    style_pulses.map_or_else(|| default_page.expect("checked").style.highlight.pulse(pulse_frame), |p| p.0),
                )
            }),
            (Some(ids), Some(model_page)) => skin_page_with_pulses(
                &skin,
                ids,
                model_page,
                &layout,
                viewport,
                state.focused,
                state.fade,
                highlight_pulse,
                text_pulse,
            ),
            _ => None,
        };
        // The default style has no back-buffer pass (its frame is a plain gradient quad).
        let backdrop = skinned.as_ref().filter(|_| default_page.is_none()).and_then(|s| {
            (skin.backdrop_sigma > 0.0).then(|| BackdropParams {
                rect: s.panel,
                radius: s.panel_radius,
                sigma: skin.backdrop_sigma * viewport.scale(),
                darken: skin.backdrop_darken,
                strength: state.fade,
            })
        });
        let mut skinned = skinned;
        let mut list_dynamic: Vec<(WidgetId, Vec<u8>)> = Vec::new();
        let mut list_text_scale_x = Vec::new();
        let mut list_fixed_spacing = Vec::new();
        if let (Some(s), Some(list)) = (skinned.as_mut(), live.as_deref().and_then(|c| c.list.as_ref())) {
            let named = text.and_then(|t| t.skin.as_ref()).and_then(|i| i.named.clone());
            for name in list_texture_names(list) {
                if sized.named.contains_key(&name) {
                    continue;
                }
                let id = TextureId(NAMED_BASE + sized.named.len() as u64);
                let uploaded = match list.images.iter().find(|i| i.key == name) {
                    // T2315: guest textures (the player icon, 16 x 16 drawn at 2x) are point sampled like the legacy picture shows (INFERRED from the picture, the D3D filter state of 0x81C60 is not decoded).
                    Some(image) => sized.renderer.upload_rgba(id, image.width, image.height, &image.rgba).ok(),
                    None => named.as_ref().and_then(|n| n.image(&name)).and_then(|image| {
                        sized.renderer.upload_rgba_filtered(id, image.width, image.height, &image.rgba).ok()
                    }),
                };
                sized.named.insert(name, uploaded.map(|()| id));
            }
            let ids = &sized.named;
            let lookup = |name: &str| ids.get(name).copied().flatten();
            let mut draw = draw_list_rows(list, &lookup, viewport, state.fade);
            if let Some(summary) = &list.summary {
                // Text widths in design units as drawn (the skin's text scale), as `0x405C0` measures.
                let measure = text.map(|assets| FontTableMeasure::new(&assets.fonts, &assets.strings));
                let width = |t: TextRef, bytes: &[u8], font: u32| -> f32 {
                    let Some(m) = measure.as_ref() else { return 0.0 };
                    let bytes = match t {
                        TextRef::Id(_) => m.text_bytes(t).unwrap_or(&[]),
                        _ => bytes,
                    };
                    m.width_px(font, bytes) * skin.text_scale.0
                };
                draw.append(draw_progress_summary(summary, &lookup, &width, viewport, state.fade));
            }
            if let Some(ingame) = &list.ingame {
                // T2315 increment 4: the in-game scoreboard (`0x2E72E0`) or objectives (`0x2F5B30`) row.
                let measure = text.map(|assets| FontTableMeasure::new(&assets.fonts, &assets.strings));
                let width = |t: TextRef, bytes: &[u8], font: u32| -> f32 {
                    let Some(m) = measure.as_ref() else { return 0.0 };
                    let bytes = match t {
                        TextRef::Id(_) => m.text_bytes(t).unwrap_or(&[]),
                        _ => bytes,
                    };
                    m.width_px(font, bytes) * skin.text_scale.0
                };
                let bytes = |t: TextRef| measure.as_ref().and_then(|m| m.text_bytes(t)).map(<[u8]>::to_vec);
                draw.append(match ingame {
                    IngameRow::Scoreboard(board) => draw_scoreboard_with_fixed_width(board, &width, &|_, bytes, font| {
                        measure.as_ref().and_then(|m| m.fixed_layout(font, bytes)).map_or(0.0, |(_, width)| width * skin.text_scale.0)
                    }, viewport, state.fade),
                    IngameRow::Objectives(page) => draw_objectives(page, &width, &bytes, viewport, state.fade),
                });
            }
            if let Some(options) = &list.options {
                // T2315 increment 3: the Audio/Video rows (slider bars, icons) and page draw, alpha blend.
                let white = sized.skin.as_ref().and_then(|ids| ids.get(SkinTexture::Line));
                let frame = state.guest_frame.unwrap_or(frame_number);
                let option_pulse = style_pulses.map_or_else(|| options.pulse.at(frame), |p| p.1);
                let mut sprites =
                    tsfp_ui_model::option_rows::draw_option_rows_with_pulse(options, &lookup, white, viewport, state.fade, option_pulse);
                if let Some(model_page) = model.page(page) {
                    // Controls: the button picture (`0x2EA080`) and the turn speed bar with the label's width
                    // (`0x405C0`, the row font, upper-cased like the label).
                    let measure = text.map(|assets| FontTableMeasure::new(&assets.fonts, &assets.strings));
                    let pulse = text_pulse;
                    for picture in &options.pictures {
                        // T2316: the picture string through the shaper's inline picture escapes (`0x40FA0`).
                        let Some(m) = measure.as_ref() else { continue };
                        let layout = m.inline_layout(picture.font, &picture.text, picture.size);
                        tsfp_ui_model::option_rows::button_picture(
                            &mut sprites, picture, &layout, &lookup, viewport, state.fade, pulse,
                        );
                    }
                    for slider in &options.text_sliders {
                        let Some(widget) = model_page.widgets.iter().find(|w| w.id == slider.widget) else { continue };
                        let width = measure.as_ref().map_or(0.0, |m| {
                            let bytes: Vec<u8> = match widget.text {
                                TextRef::Dynamic => live.as_deref().and_then(|c| c.dynamic.get(&widget.id)).cloned().unwrap_or_default(),
                                t => m.text_bytes(t).map(<[u8]>::to_vec).unwrap_or_default(),
                            };
                            let bytes = if widget.uppercase { bytes.to_ascii_uppercase() } else { bytes };
                            m.width_px(widget.font_index, &bytes) * skin.text_scale.0
                        });
                        tsfp_ui_model::option_rows::text_slider_with_pulse(
                            &mut sprites, slider, options, width, &lookup, viewport, state.fade, option_pulse,
                        );
                    }
                }
                if let (Some(arrow), Some(model_page)) = (&options.arrow, model.page(page))
                    && let Some(texture) = lookup(&arrow.name)
                {
                    // The value text width as `0x405C0` measures it (upper-cased by `0x40F00`, design units as drawn).
                    let measure = text.map(|assets| FontTableMeasure::new(&assets.fonts, &assets.strings));
                    for values in &options.values {
                        let Some(widget) = model_page.widgets.iter().find(|w| w.id == values.widget) else { continue };
                        let Some(option) = widget.options.get(widget.selected) else { continue };
                        let width = measure.as_ref().map_or(0.0, |m| {
                            let bytes: Vec<u8> = match option.text {
                                TextRef::Dynamic => live.as_deref().and_then(|c| c.dynamic.get(&widget.id)).cloned().unwrap_or_default(),
                                t => m.text_bytes(t).map(<[u8]>::to_vec).unwrap_or_default(),
                            };
                            m.width_px(widget.value_font_index, &bytes.to_ascii_uppercase()) * skin.text_scale.0
                        });
                        tsfp_ui_model::option_rows::value_arrows(
                            &mut sprites,
                            values,
                            width,
                            texture,
                            (arrow.width, arrow.height),
                            viewport,
                            state.fade,
                        );
                    }
                }
                draw.append(tsfp_ui_model::list_rows::ListDraw {
                    blends: vec![(0, tsfp_ui_renderer::BlendMode::Alpha)],
                    sprites,
                    ..Default::default()
                });
            }
            list_dynamic = draw.dynamic.clone();
            list_text_scale_x = draw.text_scale_x.clone();
            list_fixed_spacing = draw.fixed_spacing.clone();
            s.insert_list(draw);
        }
        // T1916 increment 3: the default style's row and widget draws (`default_widgets`), after the highlight.
        if let (Some(s), Some(page), Some(white)) = (
            skinned.as_mut(),
            default_page,
            sized.skin.as_ref().and_then(|ids| ids.get(SkinTexture::Line)),
        ) {
            let named = text.and_then(|t| t.skin.as_ref()).and_then(|i| i.named.clone());
            for name in page.widgets.texture_names() {
                if sized.named.contains_key(&name) {
                    continue;
                }
                let id = TextureId(NAMED_BASE + sized.named.len() as u64);
                let uploaded = match page.widgets.images.iter().find(|i| i.key == name) {
                    // Guest slot textures are point sampled like the list rows' (T2315).
                    Some(image) => sized.renderer.upload_rgba(id, image.width, image.height, &image.rgba).ok(),
                    None => named.as_ref().and_then(|n| n.image(&name)).and_then(|image| {
                        sized.renderer.upload_rgba_filtered(id, image.width, image.height, &image.rgba).ok()
                    }),
                };
                sized.named.insert(name, uploaded.map(|()| id));
            }
            let ids = &sized.named;
            let lookup = |name: &str| ids.get(name).copied().flatten();
            // Text widths in design units as drawn (the skin's text scale), as `0x3FCF0` measures.
            let measure = text.map(|assets| FontTableMeasure::new(&assets.fonts, &assets.strings));
            let width = |t: TextRef, bytes: &[u8], font: u32| -> f32 {
                let Some(m) = measure.as_ref() else { return 0.0 };
                let bytes = match t {
                    TextRef::Id(_) => m.text_bytes(t).unwrap_or(&[]),
                    _ => bytes,
                };
                m.width_px(font, bytes) * skin.text_scale.0
            };
            let bytes_of = |t: TextRef| -> Option<Vec<u8>> { measure.as_ref().and_then(|m| m.text_bytes(t)).map(<[u8]>::to_vec) };
            let draw = default_widgets::draw(&page.widgets, white, &lookup, &width, &bytes_of, viewport, state.fade);
            list_dynamic.extend(draw.dynamic.iter().cloned());
            list_text_scale_x.extend(draw.text_scale_x.iter().copied());
            s.insert_list(draw);
            if !page.widgets.wraps.is_empty() {
                let row_bytes = |p: &TextPlacement| -> Option<Vec<u8>> {
                    match p.text {
                        TextRef::Id(_) => measure.as_ref().and_then(|m| m.text_bytes(p.text)).map(<[u8]>::to_vec),
                        TextRef::Dynamic => live.as_deref().and_then(|c| c.dynamic.get(&p.widget)).cloned(),
                        TextRef::None => None,
                    }
                };
                let measure_bytes = |b: &[u8], font: u32| width(TextRef::Dynamic, b, font);
                default_widgets::split_wrapped(
                    &mut s.layout.texts,
                    &page.widgets.wraps,
                    &row_bytes,
                    &measure_bytes,
                    viewport.scale(),
                    &mut list_dynamic,
                );
            }
        }
        let (layout, sprites, blends) = match skinned {
            Some(s) => (s.layout, s.sprites, s.blends),
            None => (layout, Vec::new(), Vec::new()),
        };
        let mut frame = match text {
            Some(assets) => {
                let measure = FontTableMeasure::new(&assets.fonts, &assets.strings);
                let mut inner = FontTableShaper::new(&measure);
                let text_scale = if sized.skin.is_some() {
                    skin.text_scale
                } else {
                    (1.0, 1.0)
                };
                inner.px_per_design = Vec2::new(
                    viewport.scale() * text_scale.0,
                    viewport.scale() * text_scale.1,
                );
                if let Some(content) = live.as_deref() {
                    inner.dynamic = content.dynamic.clone();
                }
                inner.dynamic.extend(list_dynamic.iter().cloned());
                inner.text_scale_x.extend(list_text_scale_x.iter().copied());
                inner.fixed_spacing.extend(list_fixed_spacing.iter().copied());
                if let Some(p) = model.page(page) {
                    inner.uppercase = p
                        .widgets
                        .iter()
                        .filter(|w| w.uppercase)
                        .map(|w| w.id)
                        .collect();
                }
                build(&layout, inner, &sized.glyphs)?
            }
            None => build(&layout, NoText, &sized.glyphs)?,
        };
        let keys = glyph_keys(&frame);
        if keys.iter().any(|k| !sized.registered.contains(k)) {
            sized
                .glyphs
                .register(&mut sized.renderer, &keys)
                .map_err(|e| format!("glyph atlas: {e:?}"))?;
            sized.registered.extend(keys);
        }
        frame.sprites = sprites;
        frame.sprite_blends = blends;
        if let (Some(pointer), Some(pos)) = (default_page.and_then(|d| d.pointer.as_ref()), state.pointer) {
            let key = pointer.image.key.clone();
            if !sized.named.contains_key(&key) {
                let id = TextureId(NAMED_BASE + sized.named.len() as u64);
                let image = &pointer.image;
                let uploaded = sized.renderer.upload_rgba(id, image.width, image.height, &image.rgba).ok();
                sized.named.insert(key.clone(), uploaded.map(|()| id));
            }
            if let Some(texture) = sized.named.get(&key).copied().flatten() {
                let sprite = default_style::PointerSprite { texture, size: pointer.size, hotspot: pointer.hotspot };
                frame.overlay.extend(default_style::pointer(&sprite, pos, viewport, state.fade));
            }
        }
        // T2315 item 4: the text of a scrolled list is cut to the list view like the original's row viewports.
        frame.text_clip = live
            .as_deref()
            .and_then(|c| c.list.as_ref())
            .and_then(|l| l.clip)
            // T1916 increment 3: a scrolled default-style page cuts its text to the scroll view too.
            .or_else(|| default_page.and_then(|d| d.widgets.clip))
            .and_then(|(top, bottom)| tsfp_ui_model::list_rows::Edges::new(0.0, top, 640.0, bottom).to_px(viewport))
            .or_else(|| model.page(page).and_then(|p| tsfp_ui_model::skin::page_rect(p, viewport, (0.0, 0.0))));
        if let Some(params) = backdrop {
            let pass = self
                .backdrop
                .get_or_insert_with(|| BackdropPass::new(target.device, target.target.format()));
            pass.encode(target.device, target.queue, &mut *target.encoder, target.target, params);
        }
        let sized = self.sized.as_mut().expect("sized above");
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
        Ok(true)
    }
}

/// Every texture name a list draw may ask for.
fn list_texture_names(list: &ListRows) -> Vec<String> {
    let mut out: Vec<String> = list.padlock.iter().chain(list.glow.iter()).cloned().collect();
    out.extend(list.arrows.as_ref().and_then(|a| a.texture.clone()));
    out.extend(list.options.iter().flat_map(tsfp_ui_model::option_rows::texture_names));
    if let Some(icon) = list.summary.as_ref().and_then(|s| s.icon.as_ref()) {
        out.push(icon.texture.clone());
    }
    for row in &list.rows {
        let (icon, medals) = match &row.kind {
            ListRowKind::Level { icon, .. } => (icon.as_ref(), &[][..]),
            ListRowKind::ChallengeSet { icon, medals } => (icon.as_ref(), medals.as_slice()),
        };
        if let Some(icon) = icon {
            out.push(icon.texture.clone());
            out.extend(icon.locked_texture.clone());
        }
        out.extend(medals.iter().filter_map(|m| m.texture.clone()));
    }
    out.sort();
    out.dedup();
    out
}

fn register_skin(
    images: &SkinImages,
    renderer: &mut WgpuUiRenderer<'_>,
) -> Result<SkinTextures, String> {
    images
        .register(renderer)
        .map_err(|e| format!("skin textures: {e:?}"))
}

fn build<S: TextShaper>(
    layout: &tsfp_ui_model::RenderLayout,
    inner: S,
    glyphs: &GlyphSource,
) -> Result<UiFrame, String>
where
    S::Error: std::fmt::Debug,
{
    let mut shaper = CoveredShaper {
        inner,
        glyphs,
        dropped_glyphs: 0,
    };
    UiFrame::build(layout, &mut shaper).map_err(|e| format!("shape: {e:?}"))
}

impl<F: FnMut() -> Option<LayerState> + Send> FrameLayer for PageLayer<F> {
    fn seal(&mut self, present: u64) {
        let state = (self.state)();
        if std::env::var_os("TSFP_UI_LATCH_LOG").is_some() {
            eprintln!("tsfp-ui: latch seal present {present} wanted {}", state.is_some());
        }
        self.latch.seal(present, state);
    }

    fn encode_display(&mut self, target: LayerTarget<'_>) -> bool {
        let now = std::time::Instant::now();
        if self.display.present != Some(target.present) {
            let (state, clock) = self.state_for(target.present, target.frame);
            self.display.publish(target.present, state, clock, now);
        }
        self.last_wanted = self.display.state.is_some();
        let Some(state) = self.display.sample(now) else { return false; };
        let clock = self.display.clock;
        let pulses = self.display.sample_pulses(now);
        let style_pulses = self.display.sample_style_pulses(now);
        if self.drawn < 64 && std::env::var_os("TSFP_UI_DISPLAY_LOG").is_some_and(|v| v == "1") {
            eprintln!("tsfp-ui: display sample present={} display_frame={} fade={:.6} highlight={:.6} text={:.6} default={:.6} options={:.6}", target.present, target.frame, state.fade, pulses.0, pulses.1, style_pulses.0, style_pulses.1);
        }
        match self.draw(target, state, clock, Some(pulses), Some(style_pulses)) {
            Ok(drew) => { self.drawn += u64::from(drew); drew }
            Err(message) => {
                if !self.failed { eprintln!("tsfp-ui: display page not drawn: {message}"); self.failed = true; }
                false
            }
        }
    }

    fn encode(&mut self, target: LayerTarget<'_>) -> bool {
        if tsfp_wgpu::frame_layer::display_rate() {
            // The shared ring receives the clean post-grade scene. The shell owns this draw.
            // Keep the latch for the shell's selected present (latest-wins may skip frames).
            self.last_wanted = self.latch.sealed.iter().find(|s| s.present == target.present)
                .is_some_and(|s| s.state.is_some());
            return self.last_wanted;
        }
        let present = target.present;
        let (state, clock) = self.state_for(target.present, target.frame);
        if std::env::var_os("TSFP_UI_LATCH_LOG").is_some() {
            eprintln!(
                "tsfp-ui: latch encode present {present} frame {} wanted {} fade {:?} missed {}",
                target.frame,
                state.is_some(),
                state.as_ref().map(|s| s.fade),
                self.latch.missed
            );
        }
        self.last_wanted = state.is_some();
        let Some(state) = state else {
            return false;
        };
        match self.draw(target, state, clock, None, None) {
            Ok(drew) => {
                self.drawn += u64::from(drew);
                drew
            }
            Err(message) => {
                if !self.failed {
                    self.failed = true;
                    eprintln!("tsfp-ui: in-frame page not drawn: {message}");
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(fade: f32) -> Option<LayerState> {
        Some(LayerState { focused: None, fade, live: None, pointer: None, guest_frame: None })
    }

    #[test]
    fn display_pulses_follow_guest_phase_at_each_refresh_rate_and_snap() {
        use std::time::Duration;
        let now = std::time::Instant::now();
        let mut page = DisplayPage::default();
        let mut state = shown(1.0).unwrap();
        state.guest_frame = Some(59); // wrap the original one-second sine cycle
        page.publish(1, Some(state.clone()), 1, now);
        state.guest_frame = Some(60);
        let at = now + Duration::from_secs_f64(1.0 / 60.0);
        page.publish(2, Some(state.clone()), 2, at);
        for rate in [60.0, 120.0, 144.0, 240.0] {
            let sample_at = at + Duration::from_secs_f64(1.0 / rate);
            let alpha = page.animation.sample(sample_at.duration_since(now)).alpha;
            let expected = (
                OriginalSkin::pulse(59) + (OriginalSkin::pulse(60) - OriginalSkin::pulse(59)) * alpha,
                OriginalSkin::text_pulse(59) + (OriginalSkin::text_pulse(60) - OriginalSkin::text_pulse(59)) * alpha,
            );
            for _ in 0..240 { assert_eq!(page.sample_pulses(sample_at), expected); }
        }
        let later = at + Duration::from_secs(1);
        assert_eq!(page.sample_pulses(later), (OriginalSkin::pulse(60), OriginalSkin::text_pulse(60)));
        state.focused = Some(WidgetId(42));
        state.guest_frame = Some(15);
        page.publish(3, Some(state), 3, later);
        assert_eq!(page.sample_pulses(later), (OriginalSkin::pulse(15), OriginalSkin::text_pulse(15)));
        page.publish(4, None, 3, later);
        assert!(page.sample(later).is_none());
    }

    #[test]
    fn rebuilt_equal_live_content_keeps_fade_history() {
        let now = std::time::Instant::now();
        let content = LiveContent { model: UiModel::default(), page: PageId(7), dynamic: BTreeMap::new(), list: None, default_page: None };
        let mut page = DisplayPage::default();
        let mut first = shown(0.0).unwrap();
        first.live = Some(Arc::new(content.clone()));
        page.publish(1, Some(first.clone()), 1, now);
        first.live = Some(Arc::new(content)); // guest importer rebuilt an equal typed snapshot
        first.fade = 1.0;
        page.publish(2, Some(first), 2, now + std::time::Duration::from_millis(20));
        assert_eq!(page.sample(now + std::time::Duration::from_millis(30)).unwrap().fade, 0.5);
    }

    #[test]
    fn display_page_retains_fade_and_snaps_discontinuities() {
        let now = std::time::Instant::now();
        let mut page = DisplayPage::default();
        page.publish(1, shown(0.0), 1, now);
        page.publish(2, shown(1.0), 2, now + std::time::Duration::from_millis(20));
        let at = now + std::time::Duration::from_millis(30);
        for _ in 0..240 {
            assert_eq!(page.sample(at).unwrap().fade, 0.5);
            assert_eq!(page.clock, 2);
        }
        assert_eq!(page.sample(now + std::time::Duration::from_secs(1)).unwrap().fade, 1.0);
        let mut changed = shown(0.25).unwrap();
        changed.focused = Some(WidgetId(42));
        page.publish(3, Some(changed), 3, at);
        assert_eq!(page.sample(at).unwrap().fade, 0.25);
        page.publish(4, None, 3, at);
        assert!(page.sample(at).is_none());
        page.publish(5, shown(0.75), 4, at);
        assert_eq!(page.sample(at).unwrap().fade, 0.75);
    }

    #[test]
    fn display_page_snaps_epoch_changes_then_resumes_interpolation() {
        use std::time::Duration;
        let now = std::time::Instant::now();
        // Independent epoch boundaries: present rewind, shown-clock reset,
        // guest pause/rewind, and appearance/disappearance of a guest stamp.
        for (present, clock, old_guest, new_guest) in [
            (1, 3, Some(10), Some(11)),
            (3, 1, Some(10), Some(11)),
            (3, 3, Some(10), Some(10)),
            (3, 3, Some(10), Some(9)),
            (3, 3, None, Some(11)),
            (3, 3, Some(10), None),
        ] {
            let mut page = DisplayPage::default();
            let mut state = shown(0.0).unwrap();
            state.guest_frame = old_guest;
            page.publish(2, Some(state.clone()), 2, now);
            state.fade = 1.0;
            state.guest_frame = new_guest;
            page.publish(present, Some(state.clone()), clock, now + Duration::from_millis(20));
            assert_eq!(page.sample(now + Duration::from_millis(20)).unwrap().fade, 1.0);
            state.fade = 0.0;
            state.guest_frame = new_guest.map(|g| g + 1);
            page.publish(present + 1, Some(state), clock + 1, now + Duration::from_millis(40));
            assert_eq!(page.sample(now + Duration::from_millis(50)).unwrap().fade, 0.5);
        }
    }

    #[test]
    fn latch_draws_exactly_the_sealed_guest_frames() {
        let mut latch = FrameLatch::default();
        assert!(latch.take(1).is_err(), "no sealing host: ask the source");
        latch.seal(1, shown(0.5));
        latch.seal(2, None);
        latch.seal(3, shown(1.0));
        let (one, clock_one) = latch.take(1).unwrap();
        assert_eq!(one.as_ref().unwrap().fade, 0.5);
        assert_eq!(clock_one, 1);
        assert!(latch.take(2).unwrap().0.is_none(), "frame 2 kept the original draw");
        // Present 3 composed: earlier frames are dropped, its own state drawn with clock 2.
        let (three, clock_three) = latch.take(3).unwrap();
        assert_eq!(three.unwrap().fade, 1.0);
        assert_eq!(clock_three, 2);
        // A frame never sealed (or composed twice) draws nothing.
        assert!(latch.take(3).unwrap().0.is_none());
        assert!(latch.take(9).unwrap().0.is_none());
    }

    #[test]
    fn latch_skips_frames_the_renderer_dropped() {
        let mut latch = FrameLatch::default();
        for present in 1..=40 {
            latch.seal(present, shown(1.0));
        }
        assert_eq!(latch.sealed.len(), 40);
        assert_eq!(latch.take(30).unwrap().1, 30);
        assert_eq!(latch.sealed.len(), 10);
        assert!(latch.take(10).unwrap().0.is_none(), "composed past: nothing");
    }

    #[test]
    fn latch_keeps_frames_while_the_renderer_trails_far_behind() {
        // T2315: the render thread stalled (PNG dump) while the guest sealed 40 more frames.
        let mut latch = FrameLatch::default();
        for present in 1..=40 {
            latch.seal(present, shown(1.0));
        }
        assert!(latch.take(5).unwrap().0.is_some(), "a trailing present keeps its state");
        // Beyond the depth bound: the nearest later state, counted as a miss.
        let total = LATCH_DEPTH as u64 + 100;
        for present in 41..=total {
            latch.seal(present, shown(1.0));
        }
        assert!(latch.take(20).unwrap().0.is_some(), "evicted: nearest later state, not blank");
        assert_eq!(latch.missed, 1);
        assert!(latch.take(total).unwrap().0.is_some());
    }
}
