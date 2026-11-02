// SPDX-License-Identifier: GPL-3.0-or-later
//! T2189 UI-6 slice 15/16: the Rust UI main menu page drawn over the game picture (`--ui=rust` or `TSFP_UI=rust`, EXPERIMENTAL).
//!
//! The page comes from the captured menu tree (`TSFP_UI_TREE`, default tmp/ui-tree/tree.json, private, optional since T2315:
//! without it every page is read from its live page record) and its row text from the
//! disc (`TSFP_UI_DISC`: original font tables, original font pages and string banks). Slice 16: [`prepare`] only loads and checks
//! the inputs at start; the render world draws the page once on the shell's own wgpu device (Bevy's `RenderDevice`, which is the
//! T2246 shared device under `--wgpu-share`) through `tsfp_ui_wgpu::overlay::render_page_on` with no readback, and the target
//! texture becomes the `GpuImage` of the overlay sprite (app.rs). No second software device, no CPU hand-off. F9 shows or hides
//! it. Legacy (the default) or a layout-incomplete page draws nothing (the sprite stays transparent).
//! T2242: opt-in ui-display defers post-grade page batches to clean-scene recomposition in the shared-GPU shell.
//! T2188: the page is interactive (ui_live.rs): redrawn on every focus change, shown only while the main menu is the live top page.
//! T2309 UI-11: when the game's wgpu renderer presents the frame (`--wgpu-live`, `--wgpu-share`, every default variant) the
//! page is no longer an overlay sprite: [`install_in_frame`] installs a `tsfp_ui_wgpu::in_frame::PageLayer` as the renderer's
//! frame layer, so the page's batches are drawn by the engine renderer at the end of every frame (after the scene and post,
//! original blend modes), and [`tsfp_ui_page_draw_owned`] tells the C host (src/host/ui_draw_hook.c) to skip the original
//! draw of that page (no double drawing). The sprite is only the debug option `TSFP_UI_COMPOSE=overlay` (T2313); the C Vulkan picture keeps the original page draw.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use tsfp_ui_input::live_menu::LiveSlot as LivePage;
use tsfp_ui_model::frontend::{self, MAIN_MENU_BUILDER};
use tsfp_ui_model::switch::{UiBackend, UiSwitch};
use tsfp_ui_model::{PageId, UiModel, WidgetId};
use tsfp_ui_wgpu::overlay::{self, TextAssets};

/// Native cursor sample from the window event stream, never from guest memory.
static DISPLAY_POINTER: Mutex<Option<[f32; 2]>> = Mutex::new(None);
static DISPLAY_POINTER_READY: AtomicBool = AtomicBool::new(false);
pub fn set_display_pointer(position: Option<[f32; 2]>) {
    let position =
        position.filter(|xy| xy.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
    if let Ok(mut p) = DISPLAY_POINTER.lock() {
        *p = position;
    }
}
pub fn display_pointer() -> Option<[f32; 2]> {
    DISPLAY_POINTER.lock().ok().and_then(|p| *p)
}
pub fn display_pointer_ready(ready: bool) {
    DISPLAY_POINTER_READY.store(ready, Ordering::Release);
}
pub fn display_pointer_active() -> bool {
    tsfp_wgpu::frame_layer::display_rate()
        && DISPLAY_POINTER_READY.load(Ordering::Acquire)
        && display_pointer().is_some()
}

pub const SIZE: (u32, u32) = (640, 480);

static PREPARED: std::sync::OnceLock<Prepared> = std::sync::OnceLock::new();
static ROUTE_SWITCH: OnceLock<UiSwitch> = OnceLock::new();
static LIVE_PAGES: Mutex<[Option<LivePage>; 20]> = Mutex::new([None; 20]);
static INPUT_TOP: Mutex<Option<LivePage>> = Mutex::new(None);
/// T2309: the page is drawn inside the engine frame (frame layer installed).
static IN_FRAME: AtomicBool = AtomicBool::new(false);
/// T2309: the frame layer drew the page on the last frame it was wanted (false after a draw failure or an incomplete layout).
static IN_FRAME_DRAWING: AtomicBool = AtomicBool::new(true);
/// T2313: the original draw of the Rust page was skipped in the guest frame in progress (taken at the frame's seal).
static FRAME_OWNED: AtomicBool = AtomicBool::new(false);
/// T2313: the page fade (`page + 0xF4`, f32 bits) read when the original draw was skipped.
static FRAME_FADE: AtomicU32 = AtomicU32::new(0x3F80_0000);
/// The fade last written to the `TSFP_UI_DRAW_LOG` log (steps of 0.1 are logged; starts at -1, nothing logged).
static FADE_LOGGED: AtomicU32 = AtomicU32::new(0xBF80_0000);
/// T2311: the registry page whose original draw was skipped in this guest frame and its cursor row (`None`: the fixed
/// main-menu model).
#[allow(clippy::type_complexity)]
// T2181: independent footer layer; original pages keep their own clip and chrome.
static FOOTER_CONTENT: Mutex<Option<tsfp_ui_wgpu::in_frame::LayerState>> = Mutex::new(None);
static FRAME_CONTENT: Mutex<
    Option<(
        std::sync::Arc<tsfp_ui_wgpu::in_frame::LiveContent>,
        Option<WidgetId>,
    )>,
> = Mutex::new(None);
/// T2313: the page's frame layer key and order (the HUD and other layers draw under it).
const PAGE_LAYER_KEY: &str = "ui-page";
const PAGE_LAYER_ORDER: i32 = 10;

/// C host route query. A live checked page model is still absent, so the frame
/// observer asks with `layout_complete=0` and original input/drawing continue.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ui_backend_for(page: u64, layout_complete: i32) -> i32 {
    let Some(switch) = ROUTE_SWITCH.get() else {
        return 0;
    };
    i32::from(switch.route(PageId(page), layout_complete != 0).backend() == UiBackend::Rust)
}

/// Live page-slot observation from the C frame hook; log only changed fields.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ui_page_snapshot(
    slot: u32,
    builder: u32,
    state: u32,
    cursor: u32,
    rows: u32,
) {
    let Ok(mut pages) = LIVE_PAGES.lock() else {
        return;
    };
    let Some(previous) = pages.get_mut(slot as usize) else {
        return;
    };
    if builder == 0 {
        let was_live = previous.take().is_some();
        if was_live && ROUTE_SWITCH.get().is_some() {
            eprintln!("ui-page slot={slot} closed");
        }
        return;
    }
    let page = LivePage {
        builder,
        state,
        cursor,
        rows,
    };
    let changed = *previous != Some(page);
    let changed_builder = previous.is_none_or(|old| old.builder != builder);
    *previous = Some(page);
    if changed && let Some(switch) = ROUTE_SWITCH.get() {
        let route = switch.route(PageId(u64::from(builder)), false);
        if changed_builder {
            let _ = tsfp_ui_backend_for(u64::from(builder), 0);
        }
        eprintln!(
            "ui-page slot={slot} builder=0x{builder:x} route={} state={state} cursor={cursor} rows={rows}",
            route.name()
        );
    }
}

/// T2309: C host query from 0x76810: 0 original, 1 whole page, 2 Controls callback only (T1916).
/// Return 1 when the Rust UI draws the page of
/// `builder` inside the engine frame this frame, so the original draw of that widget tree is skipped. T2311: the top page of
/// the registry (ui_pages.rs) qualifies when every row draw it uses is ported; F9, any other page or a non-top widget keeps
/// the original draw, and only while the frame layer is drawing.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_ui_page_draw_owned(widget: u32, builder: u32, fade_bits: u32) -> i32 {
    // T2311: a page of the registry (crates/tsfp-bevy/src/ui_pages.rs) is drawn from its live page record; the fixed
    // main-menu model is the fallback while the registry has not read a page (no frame service yet).
    let enabled = IN_FRAME.load(Ordering::Acquire)
        && IN_FRAME_DRAWING.load(Ordering::Acquire)
        && !crate::ui_live::user_hidden();
    let registry = crate::ui_pages::draw_owned(widget, builder, enabled);
    // T1916: only the Controls custom callback is replaced. Its surrounding
    // framework frame/chrome remains at the original painter-order boundary.
    let callback_only = registry.as_ref().is_some_and(|c| {
        c.content
            .default_page
            .as_ref()
            .is_some_and(|p| p.widgets.controls.is_some())
    });
    let owned = registry.is_some()
        || (crate::ui_pages::top_builder() == 0
            && prepared().is_some_and(|p| p.fixed)
            && page_draw_owned(builder, crate::ui_live::overlay_visible()));
    if let Ok(mut content) = FRAME_CONTENT.lock()
        && owned
    {
        *content = registry
            .map(|c| crate::ui_extensions::compose(c.descriptor.builder, c.content, c.focused));
    }
    if owned {
        // T2313: this guest frame's original page draw is skipped, so the layer must draw it (latched at seal), with the
        // page's own fade (`page + 0xF4`) of this guest frame.
        let fade = page_fade(fade_bits);
        FRAME_FADE.store(fade.to_bits(), Ordering::Release);
        let logged = f32::from_bits(FADE_LOGGED.load(Ordering::Acquire));
        if (logged - fade).abs() >= 0.1
            && std::env::var_os("TSFP_UI_DRAW_LOG").is_some_and(|v| v == "1")
        {
            FADE_LOGGED.store(fade.to_bits(), Ordering::Release);
            eprintln!("tsfp-bevy: --ui=rust: page fade {fade:.3} (raw {fade_bits:#010x})");
        }
        FRAME_OWNED.store(true, Ordering::Release);
    }
    if enabled && crate::ui_back::enabled() {
        let footer_page = if crate::text_entry_live::footer_page() == Some(widget) {
            Some((builder, tsfp_ui_model::PageId(u64::from(builder))))
        } else {
            crate::ui_pages::current()
                .filter(|c| {
                    c.live.builder == builder && crate::ui_pages::slot_record(c.slot) == widget
                })
                .map(|c| (c.descriptor.builder, c.content.page))
        };
        if let Some((footer_builder, footer_id)) = footer_page {
            let (live, focused) = crate::ui_back::fallback(footer_builder, footer_id);
            if let Ok(mut content) = FOOTER_CONTENT.lock() {
                *content = Some(tsfp_ui_wgpu::in_frame::LayerState {
                    live: Some(live),
                    focused,
                    fade: page_fade(fade_bits),
                    pointer: None,
                    guest_frame: Some(crate::ui_pages::guest_frame()),
                });
            }
        }
    }
    if owned && callback_only {
        2
    } else {
        i32::from(owned)
    }
}

/// T2313: the frame layer's state for the guest frame being sealed: drawn exactly when the original draw of the page was
/// skipped in that guest frame (one decision for both sides, no frame with both or neither page). Takes the latch.
fn sealed_page_state(
    first_focus: Option<tsfp_ui_model::WidgetId>,
) -> Option<tsfp_ui_wgpu::in_frame::LayerState> {
    if IN_FRAME.load(Ordering::Acquire)
        && !crate::ui_live::user_hidden()
        && let Some(presentation) = crate::storage_live::window_presentation()
    {
        // Owned modal is independent of the guest page tree. Consume the old
        // latch so closing it cannot resurrect a stale suppressed page.
        FRAME_OWNED.store(false, Ordering::Release);
        return Some(tsfp_ui_wgpu::in_frame::LayerState {
            live: Some(presentation.content),
            focused: presentation.focused,
            fade: 1.0,
            pointer: None,
            guest_frame: None,
        });
    }

    if IN_FRAME.load(Ordering::Acquire)
        && !crate::ui_live::user_hidden()
        && let Some(presentation) = crate::storage_live::destination_presentation(0)
    {
        FRAME_OWNED.store(false, Ordering::Release);
        return Some(tsfp_ui_wgpu::in_frame::LayerState {
            live: Some(presentation.content),
            focused: presentation.focused,
            fade: 1.0,
            pointer: None,
            guest_frame: None,
        });
    }

    FRAME_OWNED.swap(false, Ordering::AcqRel).then(|| {
        let fade = f32::from_bits(FRAME_FADE.load(Ordering::Acquire));
        match FRAME_CONTENT.lock().ok().and_then(|c| c.clone()) {
            Some((live, focused)) => {
                let pointer = live
                    .default_page
                    .as_ref()
                    .and_then(|_| crate::ui_pages::pointer());
                let guest_frame = Some(crate::ui_pages::guest_frame());
                tsfp_ui_wgpu::in_frame::LayerState {
                    focused,
                    fade,
                    live: Some(live),
                    pointer,
                    guest_frame,
                }
            }
            None => tsfp_ui_wgpu::in_frame::LayerState {
                focused: crate::ui_live::focused().or(first_focus),
                fade,
                live: None,
                pointer: None,
                guest_frame: None,
            },
        }
    })
}

/// The page fade as the original stores it (`page + 0xF4`, f32, 1.0 = fully shown), clamped to 0..1; a non-finite value
/// (a page record without a fade) draws fully shown.
fn page_fade(bits: u32) -> f32 {
    let fade = f32::from_bits(bits);
    if fade.is_finite() {
        fade.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn page_draw_owned(builder: u32, visible: bool) -> bool {
    builder == MAIN_MENU_BUILDER as u32
        && visible
        && IN_FRAME.load(Ordering::Acquire)
        && IN_FRAME_DRAWING.load(Ordering::Acquire)
}

/// T2309: true when the page is drawn inside the engine frame (no overlay sprite).
pub fn in_frame() -> bool {
    IN_FRAME.load(Ordering::Acquire)
}

/// T2313: the T2189 overlay sprite runs only as an explicit debug option (`TSFP_UI_COMPOSE=overlay`, a picture laid over
/// the finished frame for A/B). Without the wgpu renderer (`plain`, the C Vulkan picture) the page keeps its original
/// draw; nothing falls back to the sprite silently any more.
pub fn overlay_sprite() -> bool {
    prepared().is_some()
        && !in_frame()
        && compose_overlay_requested(std::env::var("TSFP_UI_COMPOSE").ok().as_deref())
}

/// How the Rust page reaches the picture: `frame` (default, drawn by the engine renderer) or `overlay` (the T2189 sprite,
/// a debug option for A/B since T2313). `TSFP_UI_COMPOSE`.
pub fn compose_overlay_requested(value: Option<&str>) -> bool {
    value.is_some_and(|v| v.eq_ignore_ascii_case("overlay"))
}

/// The frame layer: the page layer plus the drawing flag the C draw switch reads.
struct TrackedLayer<F> {
    page: tsfp_ui_wgpu::in_frame::PageLayer<F>,
}

impl<F: FnMut() -> Option<tsfp_ui_wgpu::in_frame::LayerState> + Send>
    tsfp_wgpu::frame_layer::FrameLayer for TrackedLayer<F>
{
    fn seal(&mut self, present: u64) {
        tsfp_wgpu::frame_layer::FrameLayer::seal(&mut self.page, present);
    }

    fn encode_display(&mut self, target: tsfp_wgpu::frame_layer::LayerTarget<'_>) -> bool {
        let drew = tsfp_wgpu::frame_layer::FrameLayer::encode_display(&mut self.page, target);
        if self.page.last_wanted {
            IN_FRAME_DRAWING.store(drew, Ordering::Release);
        }
        drew
    }

    fn encode(&mut self, target: tsfp_wgpu::frame_layer::LayerTarget<'_>) -> bool {
        let drew = tsfp_wgpu::frame_layer::FrameLayer::encode(&mut self.page, target);
        if self.page.last_wanted {
            let before = IN_FRAME_DRAWING.swap(drew, Ordering::AcqRel);
            if before != drew {
                eprintln!(
                    "tsfp-bevy: --ui=rust: in-frame page {}",
                    if drew {
                        "drawn again"
                    } else {
                        "NOT drawn, the original draw takes the page back"
                    }
                );
            }
        }
        drew
    }
}

/// T2309: draw the prepared page inside the engine frame (call once the wgpu renderer is chosen, before the game starts).
/// Returns false (the original page draw stays, or the debug overlay sprite) without a prepared page, for `TSFP_UI_COMPOSE=overlay` or a legacy-routed page.
pub fn install_in_frame() -> bool {
    let Some(prepared) = prepared() else {
        return false;
    };
    if compose_overlay_requested(std::env::var("TSFP_UI_COMPOSE").ok().as_deref()) {
        eprintln!(
            "tsfp-bevy: --ui=rust: TSFP_UI_COMPOSE=overlay: the page is the debug overlay sprite laid over the frame (T2313)"
        );
        return false;
    }
    if prepared.switch.route(prepared.page, true).backend() != UiBackend::Rust {
        return false;
    }
    let first_focus = prepared.focused;
    let layer = tsfp_ui_wgpu::in_frame::PageLayer::new(
        &prepared.model,
        prepared.page,
        prepared.text.as_ref(),
        move || sealed_page_state(first_focus),
    );
    tsfp_wgpu::frame_layer::install(
        PAGE_LAYER_KEY,
        PAGE_LAYER_ORDER,
        Box::new(TrackedLayer { page: layer }),
    );
    if crate::ui_back::enabled() {
        let layer = tsfp_ui_wgpu::in_frame::PageLayer::new(
            &prepared.model,
            prepared.page,
            prepared.text.as_ref(),
            || FOOTER_CONTENT.lock().ok().and_then(|mut c| c.take()),
        );
        tsfp_wgpu::frame_layer::install("tsfp-ui-back", PAGE_LAYER_ORDER + 1, Box::new(layer));
    }
    IN_FRAME.store(true, Ordering::Release);
    eprintln!(
        "tsfp-bevy: --ui=rust: Rust UI page {:#x} drawn INSIDE the engine frame by the wgpu renderer (after scene and post, \
         original blend), the original draw of that page is skipped while it shows; F9 gives the page back, \
         TSFP_UI_COMPOSE=overlay for the old sprite",
        prepared.page.0
    );
    true
}

/// T3853: publish the actual top page after native commands, independent of stale C slot snapshots.
pub fn observe_input_top(g: &dyn tsfp_ui_input::native_page::GuestUi) {
    if let Ok(mut top) = INPUT_TOP.lock() {
        *top = input_snapshot(g);
    }
}

fn input_snapshot(g: &dyn tsfp_ui_input::native_page::GuestUi) -> Option<LivePage> {
    let top = tsfp_ui_input::native_page::input_page(g)?;
    (tsfp_ui_input::native_page::popup_identity(g, top) != 0).then(|| LivePage {
        builder: tsfp_ui_input::native_page::popup_identity(g, top),
        state: top.state(g),
        cursor: top.cursor(g),
        rows: top.row_count(g),
    })
}

/// The Rust-owned input identity follows active children, including reused lower slots.
pub fn top_live_page() -> Option<tsfp_ui_input::live_menu::LiveSlot> {
    INPUT_TOP.lock().ok().and_then(|top| *top)
}

/// The overlay inputs loaded at start, if any (read by the app's setup and the render world).
pub fn prepared() -> Option<&'static Prepared> {
    PREPARED.get()
}

/// Shell start: load the requested overlay inputs and keep them for the app. Failures are reported, the game runs without it.
pub fn prepare(request: &Request) {
    let _ = ROUTE_SWITCH.set(request.switch.clone());
    match load(request) {
        Ok(prepared) => {
            eprintln!(
                "tsfp-bevy: --ui=rust: Rust UI overlay prepared: page {:#x}, {}; drawn on the shell device at the first frame",
                prepared.page.0, prepared.text_note
            );
            let _ = PREPARED.set(prepared);
        }
        Err(message) => eprintln!("tsfp-bevy: --ui=rust: no overlay: {message}"),
    }
}

/// What the shell was asked to overlay.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub switch: UiSwitch,
    pub tree: PathBuf,
    pub disc: Option<PathBuf>,
}

/// Takes `--ui=..`, `--ui ..` and `--ui-legacy-page=..` out of the host arguments (the C host does not know them) and reads
/// `TSFP_UI`, `TSFP_UI_TREE`, `TSFP_UI_DISC` through `env`. None when the Rust UI is not selected. Pure, unit tested.
pub fn extract(
    arguments: &mut Vec<String>,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Option<Request>, String> {
    let ui_env = env("TSFP_UI");
    let (switch, rest) = UiSwitch::parse(ui_env.as_deref(), arguments.iter().map(String::as_str))
        .map_err(|e| e.0)?;
    let rest: Vec<String> = rest.into_iter().map(str::to_owned).collect();
    *arguments = rest;
    if switch.backend != UiBackend::Rust {
        return Ok(None);
    }
    Ok(Some(Request {
        switch,
        tree: env("TSFP_UI_TREE")
            .map_or_else(|| PathBuf::from("tmp/ui-tree/tree.json"), PathBuf::from),
        disc: env("TSFP_UI_DISC")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
    }))
}

/// What the overlay draws: the page model, the page, its focused row and the disc text.
pub struct Prepared {
    pub switch: UiSwitch,
    /// T2315: the main-menu model came from a menu tree file (false: empty model, live records only).
    pub fixed: bool,
    pub model: UiModel,
    pub page: PageId,
    pub focused: Option<WidgetId>,
    pub text: Option<TextAssets>,
    pub text_note: &'static str,
}

/// Reads the menu tree (optional since T2315 item 5) and the disc text. Without a tree file the fixed main-menu model is
/// empty ([`Prepared::fixed`] false): every page, the main menu included, is drawn from its live page record by the
/// registry (ui_pages.rs) and the main-menu input model is built from that record (ui_live.rs).
pub fn load(request: &Request) -> Result<Prepared, String> {
    let (model, page, focused, fixed) = match std::fs::read_to_string(&request.tree) {
        Ok(text) => {
            let model = UiModel::from_tree_json(&text, &frontend::dump_load_options())
                .map_err(|e| format!("menu tree {}: {e:?}", request.tree.display()))?;
            let page = frontend::page_for_builder(&text, MAIN_MENU_BUILDER)
                .ok_or("no main menu page in the menu tree")?;
            let focused = model
                .page(page)
                .and_then(|p| p.widgets.iter().find(|w| w.is_focusable()))
                .map(|w| w.id);
            (model, page, focused, true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "tsfp-bevy: --ui=rust: no menu tree at {} (not needed): pages and their text come from the live page records and the disc",
                request.tree.display()
            );
            (UiModel::new(), PageId(MAIN_MENU_BUILDER), None, false)
        }
        Err(e) => return Err(format!("menu tree {}: {e}", request.tree.display())),
    };
    let (text, text_note) = match request
        .disc
        .as_deref()
        .map(|disc| TextAssets::from_disc(Path::new(disc), 'e'))
    {
        Some(Ok(assets)) => (Some(assets), "text from the disc"),
        Some(Err(e)) => {
            eprintln!("tsfp-bevy: --ui=rust: no text ({e})");
            (None, "no text (disc unreadable)")
        }
        None => (None, "no text (TSFP_UI_DISC unset)"),
    };
    Ok(Prepared {
        switch: request.switch.clone(),
        fixed,
        model,
        page,
        focused,
        text,
        text_note,
    })
}

/// Draws the prepared page on `device`/`queue` with `focused` highlighted (render world; T2188: again whenever the live focus
/// changes). `Ok(None)` = legacy keeps the page (reason printed).
pub fn draw(
    prepared: &Prepared,
    focused: Option<WidgetId>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<Option<wgpu::Texture>, String> {
    let frame = overlay::render_page_on(
        device,
        queue,
        prepared.switch.clone(),
        &prepared.model,
        prepared.page,
        focused,
        prepared.text.as_ref(),
        SIZE,
        [0, 0, 0, 0],
        false,
    )?;
    let route = format!("page {:#x} route {}", prepared.page.0, frame.route.name());
    match frame.texture {
        Some(texture) => {
            static LOGGED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !LOGGED.swap(true, std::sync::atomic::Ordering::AcqRel) {
                eprintln!(
                    "tsfp-bevy: --ui=rust: Rust UI overlay drawn on the shell device: {route}, {} panels, {} of {} rows with text, {}, glyphs from the {}, F9 hides it",
                    frame.quads, frame.text_runs, frame.text_rows, prepared.text_note, frame.glyphs
                );
            }
            Ok(Some(texture))
        }
        None => {
            eprintln!(
                "tsfp-bevy: --ui=rust: legacy keeps the menu: {route}{}",
                frame
                    .layout_note
                    .map_or(String::new(), |n| format!(" ({n})"))
            );
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        }
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn page_fade_clamps_and_defaults_to_shown() {
        assert_eq!(page_fade(1.0f32.to_bits()), 1.0);
        assert_eq!(page_fade(0.25f32.to_bits()), 0.25);
        assert_eq!(page_fade((-3.0f32).to_bits()), 0.0);
        assert_eq!(page_fade(7.0f32.to_bits()), 1.0);
        assert_eq!(page_fade(f32::NAN.to_bits()), 1.0);
    }

    #[test]
    fn legacy_by_default_and_ui_flags_never_reach_the_host() {
        let mut arguments = args(&["game.xbe", "--ui=legacy", "--present", "window"]);
        assert_eq!(extract(&mut arguments, env(&[])).unwrap(), None);
        assert_eq!(arguments, args(&["game.xbe", "--present", "window"]));
        let mut arguments = args(&["game.xbe"]);
        assert_eq!(extract(&mut arguments, env(&[])).unwrap(), None);
    }

    #[test]
    fn rust_from_flag_or_env_with_paths() {
        let mut arguments = args(&["--ui", "rust", "--ui-legacy-page=0x10", "--mute"]);
        let request = extract(&mut arguments, env(&[("TSFP_UI_DISC", "d.iso")]))
            .unwrap()
            .unwrap();
        assert_eq!(arguments, args(&["--mute"]));
        assert_eq!(request.tree, PathBuf::from("tmp/ui-tree/tree.json"));
        assert_eq!(request.disc, Some(PathBuf::from("d.iso")));
        assert_eq!(request.switch.backend, UiBackend::Rust);
        let mut arguments = args(&[]);
        let request = extract(
            &mut arguments,
            env(&[("TSFP_UI", "rust"), ("TSFP_UI_TREE", "t.json")]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (request.tree, request.disc),
            (PathBuf::from("t.json"), None)
        );
        // A flag wins over the environment.
        let mut arguments = args(&["--ui=legacy"]);
        assert_eq!(
            extract(&mut arguments, env(&[("TSFP_UI", "rust")])).unwrap(),
            None
        );
    }

    #[test]
    fn a_missing_menu_tree_is_not_an_error() {
        let switch = UiSwitch::parse(None, ["--ui=rust"]).unwrap().0;
        let request = Request {
            switch,
            tree: PathBuf::from("/nonexistent/t2315/tree.json"),
            disc: None,
        };
        let prepared = load(&request).expect("no tree file: live page records only");
        assert!(!prepared.fixed);
        assert_eq!(prepared.page, PageId(MAIN_MENU_BUILDER));
        assert!(prepared.model.page(prepared.page).is_none());
        assert_eq!(prepared.focused, None);
        let directory = Request {
            tree: std::env::temp_dir(),
            ..request
        };
        assert!(
            load(&directory).is_err(),
            "an unreadable tree path (a directory) stays an error"
        );
    }

    #[test]
    fn bad_values_are_errors() {
        assert!(extract(&mut args(&["--ui=fancy"]), env(&[])).is_err());
        assert!(extract(&mut args(&["--ui"]), env(&[])).is_err());
    }

    #[test]
    fn original_page_draw_is_skipped_only_for_the_visible_in_frame_main_menu() {
        let main = MAIN_MENU_BUILDER as u32;
        IN_FRAME.store(false, Ordering::Release);
        assert!(
            !page_draw_owned(main, true),
            "overlay mode keeps the original draw"
        );
        IN_FRAME.store(true, Ordering::Release);
        IN_FRAME_DRAWING.store(true, Ordering::Release);
        assert!(page_draw_owned(main, true));
        assert!(
            !page_draw_owned(main, false),
            "F9 / other page on top: original draw"
        );
        assert!(
            !page_draw_owned(main + 0x10, true),
            "other builders keep the original draw"
        );
        IN_FRAME_DRAWING.store(false, Ordering::Release);
        assert!(
            !page_draw_owned(main, true),
            "layer failed: the original draw takes the page back"
        );
        IN_FRAME.store(false, Ordering::Release);
        IN_FRAME_DRAWING.store(true, Ordering::Release);
        assert!(compose_overlay_requested(Some("overlay")));
        assert!(!compose_overlay_requested(Some("frame")));
        assert!(!compose_overlay_requested(None));
    }

    #[test]
    fn input_snapshot_tracks_child_topology_and_reused_builderless_slots() {
        use std::collections::HashMap;
        use tsfp_ui_input::menu_state::layout::page;
        use tsfp_ui_input::native_page::{GuestUi, PageRef};
        #[derive(Default)]
        struct Memory(HashMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, a: u32) -> Option<u32> {
                self.0.get(&a).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        let mut f = Memory::default();
        let published = |f: &Memory| {
            observe_input_top(f);
            top_live_page()
        };
        let parent = PageRef::slot(3);
        let child = PageRef::slot(1);
        f.0.insert(parent.0, 2);
        f.0.insert(parent.0 + page::BUILDER, 0x31_3010);
        f.0.insert(parent.0 + page::ACTIVE_CHILD, child.0);
        f.0.insert(child.0, 2);
        f.0.insert(child.0 + page::BUILDER, 0x31_33E0);
        f.0.insert(child.0 + page::STATE, 3);
        f.0.insert(child.0 + page::ROW_COUNT, 4);
        f.0.insert(child.0 + page::CURSOR, 2);
        let snapshot = published(&f).unwrap();
        assert_eq!(snapshot.builder, 0x31_33E0);
        assert_eq!((snapshot.state, snapshot.cursor, snapshot.rows), (3, 2, 4));
        f.0.insert(child.0 + page::BUILDER, 0);
        let snapshot = published(&f).unwrap();
        assert_eq!(snapshot.builder, 0x31_3010);
        assert_eq!((snapshot.cursor, snapshot.rows), (2, 4));
        // Slot reuse for a standalone canvas cannot retain the former native page identity.
        f.0.insert(parent.0, 0);
        assert_eq!(
            published(&f),
            None,
            "a standalone canvas does not own menu input"
        );
        f.0.insert(child.0, 0);
        assert_eq!(published(&f), None);
        f.0.insert(parent.0, 2);
        assert_eq!(published(&f).unwrap().builder, 0x31_3010);
        f.0.insert(child.0, 2);
        f.0.insert(child.0 + page::ACTIVE_CHILD, parent.0);
        assert_eq!(published(&f), None, "cycles never retain a stale owner");
    }

    #[test]
    fn gameplay_canvas_does_not_route_escape_or_right_click_to_menu_back() {
        use std::collections::HashMap;
        use tsfp_input::player::bits;
        use tsfp_input::router::{DeviceEvent, MouseButtonId, Router, RouterConfig};
        use tsfp_ui_input::native_page::{GuestUi, PageRef};
        struct Memory(HashMap<u32, u32>);
        impl GuestUi for Memory {
            fn read32(&self, address: u32) -> Option<u32> {
                self.0.get(&address).copied()
            }
            fn write32(&mut self, _: u32, _: u32) -> bool {
                false
            }
            fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
                None
            }
        }
        // A standalone occupied slot has no builder and is not a menu.
        let canvas = PageRef::slot(1);
        let memory = Memory(HashMap::from([(canvas.0, 2)]));
        let menu = input_snapshot(&memory);
        assert!(menu.is_none(), "gameplay must bypass the menu fallback");
        let mut mouse_fallback = tsfp_ui_input::keyboard::MouseFallback::new();
        assert_eq!(mouse_fallback.step(1, true, menu.is_some()), None);
        let mut router = Router::new(RouterConfig {
            mouse_keyboard_slot: Some(0),
            ..Default::default()
        });
        router.feed(DeviceEvent::Key {
            name: "Escape".into(),
            down: true,
        });
        router.feed(DeviceEvent::MouseButton {
            button: MouseButtonId::Right,
            down: true,
        });
        let input = router.tick()[0].unwrap();
        assert_ne!(input.held & bits::START, 0, "Escape pauses");
        assert_ne!(input.held & bits::R3, 0, "right click aims");
        assert_eq!(input.held & bits::B, 0, "neither event triggers melee");
    }

    #[test]
    fn c_page_observer_keeps_typed_slot_state() {
        let switch = UiSwitch::parse(None, ["--ui=rust"]).unwrap().0;
        let _ = ROUTE_SWITCH.set(switch);
        assert_eq!(tsfp_ui_backend_for(0x2D0BE0, 0), 0);
        tsfp_ui_page_snapshot(19, 0x2D0BE0, 3, 2, 4);
        assert_eq!(
            LIVE_PAGES.lock().unwrap()[19],
            Some(LivePage {
                builder: 0x2D0BE0,
                state: 3,
                cursor: 2,
                rows: 4,
            })
        );
        tsfp_ui_page_snapshot(19, 0, 0, 0, 0);
        assert_eq!(LIVE_PAGES.lock().unwrap()[19], None);
    }
}
