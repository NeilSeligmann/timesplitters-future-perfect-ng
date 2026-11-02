// SPDX-License-Identifier: GPL-3.0-or-later
//! The Bevy app: window, event loop, input and audio. It shows the host's latest picture as a letterboxed sprite (linear or nearest, T2036)
//!; the game itself runs on its own thread (main.rs). Bevy 0.16.1, Vulkan only (docs/t-bevy-spike.md).
use crate::events::{gamepad_events, keyboard_events};
use crate::present::{self, Filter};
use crate::shared::{SHARED, lock};
use bevy::app::{PluginGroupBuilder, ScheduleRunnerPlugin};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::{ExitCondition, PrimaryWindow};
use std::sync::atomic::Ordering;

/// Rust-owned session/lobby state admitted to the Bevy runtime. No system selects it yet, so the
/// current lifted session path and default gameplay behavior remain unchanged.
#[derive(Resource, Default)]
#[allow(dead_code)]
pub(crate) struct NativeSessionStateRes(pub tsfp_network::syslink_session::NativeSessionState);

#[derive(Resource)]
struct Screen {
    handle: Handle<Image>,
    size: (u32, u32),
    /// Buffer swapped with the mailbox, so the steady state allocates nothing.
    spare: Vec<u8>,
    /// `--wgpu-live` (T2136e): output size requests and frame hand-off from the render runtime.
    pump: crate::wgpu_live::Pump,
    /// `--wgpu-share` (T2229): GPU texture hand-off, no CPU buffers. `None` on the CPU paths.
    share: Option<ShareView>,
}

/// Main world half of the GPU texture path: the blit state and the strong handle of the current shell texture (dropping it on a
/// resize lets Bevy forget the old texture and its bind group).
struct ShareView {
    share: crate::gpu_share::Share,
    handle: Option<Handle<Image>>,
}

#[derive(Component)]
struct PictureSprite;

/// T2189 slice 15/16: the Rust UI page above the picture (`--ui=rust`), same size as the picture.
#[derive(Component)]
struct UiOverlaySprite;

/// The overlay sprite's image; the render world replaces its `GpuImage` with the page drawn on the shell device.
static UI_OVERLAY_IMAGE: std::sync::OnceLock<bevy::asset::AssetId<Image>> =
    std::sync::OnceLock::new();

fn spawn_ui_overlay(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // T2309/T2313: the page is drawn inside the engine frame by the game's renderer; the sprite is only the debug option
    // `TSFP_UI_COMPOSE=overlay` (the C Vulkan picture keeps the original page draw).
    if !crate::ui_overlay::overlay_sprite() {
        return;
    }
    let (width, height) = crate::ui_overlay::SIZE;
    // Transparent placeholder until the render world adopts the drawn page (and for good when legacy keeps the page).
    let image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    let handle = images.add(image);
    let _ = UI_OVERLAY_IMAGE.set(handle.id());
    commands.spawn((
        UiOverlaySprite,
        Sprite {
            image: handle,
            custom_size: Some(Vec2::ZERO),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));
}

/// The overlay page drawn on the render device: the Bevy-side texture and its sampled view and sampler.
struct DrawnOverlay {
    texture: bevy::render::render_resource::Texture,
    view: bevy::render::render_resource::TextureView,
    sampler: bevy::render::render_resource::Sampler,
}

/// Render world: draw the prepared page once on the shell's own device (no readback) and keep it the `GpuImage` of the overlay
/// image, re-adopting it whenever Bevy prepares its placeholder again.
fn draw_ui_overlay(
    mut gpu_images: ResMut<
        bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>,
    >,
    device: Res<bevy::render::renderer::RenderDevice>,
    queue: Res<bevy::render::renderer::RenderQueue>,
    mut state: Local<Option<(Option<tsfp_ui_model::WidgetId>, Option<DrawnOverlay>)>>,
) {
    let (Some(prepared), Some(&id)) = (crate::ui_overlay::prepared(), UI_OVERLAY_IMAGE.get())
    else {
        return;
    };
    // Wait for the placeholder: an insert before Bevy prepares it would be overwritten.
    let Some(current) = gpu_images.get(id) else {
        return;
    };
    // T2188: redraw whenever the live focus changed (checked every frame).
    let focused = crate::ui_live::focused().or(prepared.focused);
    if state
        .as_ref()
        .is_some_and(|(drawn_focus, _)| *drawn_focus != focused)
    {
        *state = None;
    }
    let (_, drawn) = state.get_or_insert_with(|| {
        let wgpu_queue: &wgpu::Queue = &queue.0;
        let drawn =
            match crate::ui_overlay::draw(prepared, focused, device.wgpu_device(), wgpu_queue) {
                Ok(Some(texture)) => {
                    let view = texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some("tsfp-ui-overlay"),
                        format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                        ..Default::default()
                    });
                    Some(DrawnOverlay {
                        texture: bevy::render::render_resource::Texture::from(texture),
                        view: bevy::render::render_resource::TextureView::from(view),
                        sampler: device.create_sampler(
                            &bevy::render::render_resource::SamplerDescriptor {
                                label: Some("tsfp-ui-overlay"),
                                mag_filter: bevy::render::render_resource::FilterMode::Linear,
                                min_filter: bevy::render::render_resource::FilterMode::Linear,
                                ..default()
                            },
                        ),
                    })
                }
                Ok(None) => None,
                Err(message) => {
                    eprintln!("tsfp-bevy: --ui=rust: overlay not drawn: {message}");
                    None
                }
            };
        (focused, drawn)
    });
    let Some(drawn) = drawn else {
        return;
    };
    if current.texture.id() == drawn.texture.id() {
        return;
    }
    let (width, height) = crate::ui_overlay::SIZE;
    gpu_images.insert(
        id,
        bevy::render::texture::GpuImage {
            texture: drawn.texture.clone(),
            texture_view: drawn.view.clone(),
            texture_format: TextureFormat::Rgba8UnormSrgb,
            sampler: drawn.sampler.clone(),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
        },
    );
}

/// Letterboxed picture size in window units (read by the T2188 pointer mapping).
static PICTURE_SIZE: std::sync::Mutex<(f32, f32)> = std::sync::Mutex::new((0.0, 0.0));

pub fn picture_size() -> Vec2 {
    let (w, h) = *lock(&PICTURE_SIZE);
    Vec2::new(w, h)
}

/// Keeps the overlay the picture's size; shown only while the Rust main menu owns input (T2188, F9 hides it, ui_live.rs).
fn follow_picture_and_toggle(
    picture: Query<&Sprite, (With<PictureSprite>, Without<UiOverlaySprite>)>,
    mut overlay: Query<(&mut Sprite, &mut Visibility), With<UiOverlaySprite>>,
) {
    let Ok(picture) = picture.single() else {
        return;
    };
    if let Some(size) = picture.custom_size {
        *lock(&PICTURE_SIZE) = (size.x, size.y);
    }
    let Ok((mut sprite, mut visibility)) = overlay.single_mut() else {
        return;
    };
    sprite.custom_size = picture.custom_size;
    let want = if crate::ui_live::overlay_visible() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if *visibility != want {
        *visibility = want;
    }
}

fn plugins() -> PluginGroupBuilder {
    // The spike's hand-picked set (no PBR, no UI, no scene), plus gamepad and audio.
    let plugins = PluginGroupBuilder::start::<PluginGroupBuilder>()
        .add(bevy::app::TaskPoolPlugin::default())
        .add(bevy::diagnostic::FrameCountPlugin)
        .add(bevy::time::TimePlugin)
        .add(bevy::transform::TransformPlugin)
        .add(bevy::input::InputPlugin)
        .add(bevy::log::LogPlugin::default())
        .add(bevy::window::WindowPlugin {
            primary_window: Some(Window {
                title: "tsfp (Bevy)".into(),
                resolution: {
                    let (width, height) = present::default_window_size(
                        present::plan(present::options().aspect).window_aspect,
                    );
                    bevy::window::WindowResolution::new(width, height)
                },
                resizable: true,
                desired_maximum_frame_latency: Some(
                    std::num::NonZeroU32::new(
                        if std::env::var("TSFP_PRESENT_QUEUE").as_deref() == Ok("1") {
                            1
                        } else {
                            2
                        },
                    )
                    .unwrap(),
                ),
                // Rule 21: uncapped unless the owner requests VSync. The surface
                // probe selects a supported explicit mode and reports it.
                present_mode: if std::env::var("TSFP_PRESENT_MODE")
                    .is_ok_and(|v| v == "fifo" || v == "vsync")
                {
                    bevy::window::PresentMode::Fifo
                } else {
                    bevy::window::PresentMode::AutoNoVsync
                },
                ..default()
            }),
            // The game decides when to stop: a close request is queued for the C host, which stops cleanly and then ends Bevy.
            exit_condition: ExitCondition::DontExit,
            close_when_requested: false,
        })
        .add(bevy::a11y::AccessibilityPlugin)
        .add(bevy::asset::AssetPlugin {
            file_path: "assets".into(),
            ..default()
        })
        .add(render_plugin())
        .add(match present::options().filter {
            Filter::Nearest => bevy::render::texture::ImagePlugin::default_nearest(),
            Filter::Linear => bevy::render::texture::ImagePlugin::default_linear(),
        })
        .add(bevy::core_pipeline::CorePipelinePlugin)
        .add(bevy::sprite::SpritePlugin::default())
        .add(bevy::audio::AudioPlugin::default());
    let plugins = if std::env::var_os("TSFP_DISABLE_LIVE_GAMEPADS").is_some() {
        plugins
    } else {
        plugins.add(bevy::gilrs::GilrsPlugin)
    };
    plugins.add(bevy::winit::WinitPlugin::<bevy::winit::WakeUp>::default())
}

/// Keep the hand-off update loop running continuously when the window loses
/// focus. In particular, the HFR route must keep sampling the latest frame;
/// Bevy's default unfocused mode throttles that sampling to 60 Hz.
fn winit_settings() -> bevy::winit::WinitSettings {
    let mut settings = bevy::winit::WinitSettings::game();
    // T3388: a replay runs unfocused under Xvfb or Weston; the default reactive low-power mode
    // capped Bevy (and so the replay input lockstep) at 60 frames per second.
    if std::env::var("TSFP_BEVY_UNFOCUSED_CONTINUOUS").as_deref() == Ok("1")
        || crate::input_recording::replaying()
        || crate::input_recording::fast_forward_switch(
            std::env::var("TSFP_FAST_FORWARD").ok().as_deref(),
        )
    {
        settings.unfocused_mode = bevy::winit::UpdateMode::Continuous;
    }
    settings
}

/// `RenderCreation::Manual` with the device the shell made and shared with the render runtime, else Bevy's own device.
fn render_plugin() -> bevy::render::RenderPlugin {
    use bevy::render::renderer::{
        RenderAdapter, RenderAdapterInfo, RenderDevice, RenderInstance, RenderQueue, WgpuWrapper,
    };
    use bevy::render::settings::{RenderCreation, RenderResources};
    use std::sync::Arc;
    match crate::gpu_share::gpu() {
        Some(gpu) => bevy::render::RenderPlugin {
            render_creation: RenderCreation::Manual(RenderResources(
                RenderDevice::from(gpu.device.clone()),
                RenderQueue(Arc::new(WgpuWrapper::new(gpu.queue.clone()))),
                RenderAdapterInfo(WgpuWrapper::new(gpu.info.clone())),
                RenderAdapter(Arc::new(WgpuWrapper::new(gpu.adapter.clone()))),
                RenderInstance(Arc::new(WgpuWrapper::new(gpu.instance.clone()))),
            )),
            ..default()
        },
        None => bevy::render::RenderPlugin::default(),
    }
}

fn blank(width: u32, height: u32) -> Image {
    Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);
    let handle = images.add(blank(1, 1));
    commands.insert_resource(Screen {
        handle: handle.clone(),
        size: (1, 1),
        spare: Vec::new(),
        pump: Default::default(),
        share: new_share(),
    });
    commands.spawn((
        PictureSprite,
        Sprite {
            image: handle,
            custom_size: Some(Vec2::ZERO),
            ..default()
        },
    ));
}

fn new_share() -> Option<ShareView> {
    let gpu = crate::gpu_share::gpu()?;
    Some(ShareView {
        share: crate::gpu_share::Share::new(Box::new(crate::gpu_share::RuntimeSource), gpu.clone()),
        handle: None,
    })
}

/// GPU texture path: request the window sized picture, blit the newest published frame into the shell texture and (on a new
/// texture) publish it to the render world under a fresh image handle.
fn share_picture(
    screen: &mut Screen,
    images: &mut Assets<Image>,
    physical: (u32, u32),
    sprite: &mut Query<&mut Sprite, With<PictureSprite>>,
) {
    let aspect = present::plan(present::options().aspect).picture_aspect;
    let desired = crate::wgpu_live::output_size(physical, aspect);
    let Some(view) = screen.share.as_mut() else {
        return;
    };
    let started = std::time::Instant::now();
    let stepped = view.share.step(desired, started);
    if stepped.is_some() {
        crate::frame_budget::handoff(started.elapsed());
    }
    let Some(stepped) = stepped else {
        return;
    };
    if !stepped.new_target {
        return;
    }
    // A new texture needs a new image id: the sprite's bind group is cached per id, so a resized texture under the old id would
    // keep sampling the old one. The 1x1 asset behind the handle is replaced in the render world by the shell texture.
    let handle = images.add(blank(1, 1));
    if let Some(target) = view.share.target() {
        *crate::gpu_share::PUBLISHED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(crate::gpu_share::Published {
            id: handle.id(),
            target: target.clone(),
        });
    }
    for mut sprite in sprite.iter_mut() {
        sprite.image = handle.clone();
    }
    view.handle = Some(handle);
    screen.size = stepped.size;
}

/// Render world: make the shell texture the `GpuImage` of the picture handle, after Bevy has prepared its own 1x1 placeholder.
fn adopt_shared_texture(
    mut gpu_images: ResMut<
        bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>,
    >,
    device: Res<bevy::render::renderer::RenderDevice>,
    mut sampler: Local<Option<bevy::render::render_resource::Sampler>>,
    mut adopted: Local<
        Option<(
            bevy::asset::AssetId<Image>,
            u64,
            bevy::render::render_resource::TextureId,
        )>,
    >,
) {
    let published = crate::gpu_share::PUBLISHED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(published) = published.as_ref() else {
        return;
    };
    // Compare our own serial: `Texture::from` hands out a fresh Bevy id on every wrap, so re-wrapping each frame would churn.
    // Asset extraction can arrive a frame after PUBLISHED and replace an early
    // import with its placeholder. Check the actual GPU texture identity too.
    if let Some(current) = gpu_images.get(published.id)
        && *adopted == Some((published.id, published.target.serial, current.texture.id()))
    {
        return;
    }
    let texture = bevy::render::render_resource::Texture::from(published.target.texture.clone());
    *adopted = Some((published.id, published.target.serial, texture.id()));
    let sampler = sampler
        .get_or_insert_with(|| {
            let filter = match present::options().filter {
                Filter::Nearest => bevy::render::render_resource::FilterMode::Nearest,
                Filter::Linear => bevy::render::render_resource::FilterMode::Linear,
            };
            device.create_sampler(&bevy::render::render_resource::SamplerDescriptor {
                label: Some("tsfp-shared-picture"),
                mag_filter: filter,
                min_filter: filter,
                ..default()
            })
        })
        .clone();
    let (width, height) = published.target.size;
    gpu_images.insert(
        published.id,
        bevy::render::texture::GpuImage {
            texture,
            texture_view: bevy::render::render_resource::TextureView::from(
                published.target.sampled_view.clone(),
            ),
            texture_format: TextureFormat::Rgba8UnormSrgb,
            sampler,
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
        },
    );
}

/// Probe supported modes once using the live window before Bevy creates its surface.
fn probe_present_mode(
    mut done: Local<bool>,
    windows: Res<bevy::render::view::ExtractedWindows>,
    instance: Res<bevy::render::renderer::RenderInstance>,
    adapter: Res<bevy::render::renderer::RenderAdapter>,
) {
    use std::sync::atomic::Ordering::Relaxed;
    if *done {
        return;
    }
    let Some(window) = windows.windows.values().next() else {
        return;
    };
    *done = true;
    let target = wgpu::SurfaceTargetUnsafe::RawHandle {
        raw_display_handle: window.handle.get_display_handle(),
        raw_window_handle: window.handle.get_window_handle(),
    };
    // SAFETY: the handles in ExtractedWindows are live (Bevy creates its own surface from them right after); this surface only
    // reports capabilities and is dropped at the end of the function, before Bevy configures its own.
    let mode = unsafe { instance.create_surface_unsafe(target) }
        .map(|surface| {
            choose_present_mode(
                &surface.get_capabilities(&adapter).present_modes,
                std::env::var("TSFP_PRESENT_MODE").ok().as_deref(),
            )
        })
        .unwrap_or(2);
    crate::gpu_share::PRESENT_MODE.store(mode, Relaxed);
}

/// Surface mode: Immediate first for uncapped presentation, Mailbox if needed;
/// VSync prefers latest-completed Mailbox so the shared render queue is not
/// serialized on blocking Fifo presents; strict Fifo remains an explicit option.
fn choose_present_mode(supported: &[wgpu::PresentMode], requested: Option<&str>) -> u8 {
    if requested == Some("fifo") {
        return 2;
    }
    if requested == Some("vsync") {
        return if supported.contains(&wgpu::PresentMode::Mailbox) {
            1
        } else {
            2
        };
    }
    if requested == Some("mailbox") && supported.contains(&wgpu::PresentMode::Mailbox) {
        return 1;
    }
    if supported.contains(&wgpu::PresentMode::Immediate) {
        return 3;
    }
    if supported.contains(&wgpu::PresentMode::Mailbox) {
        return 1;
    }
    2
}

fn upgrade_present_mode(
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut reported: Local<bool>,
) {
    use std::sync::atomic::Ordering::Relaxed;
    let state = crate::gpu_share::PRESENT_MODE.load(Relaxed);
    if state == 0 || *reported {
        return;
    }
    *reported = true;
    if let Ok(mut window) = window.single_mut() {
        let (mode, name) = match state {
            3 => (bevy::window::PresentMode::Immediate, "Immediate"),
            1 => (bevy::window::PresentMode::Mailbox, "Mailbox"),
            _ => (bevy::window::PresentMode::Fifo, "Fifo"),
        };
        window.present_mode = mode;
        eprintln!(
            "tsfp-bevy: present mode {name} (requested {}; Immediate=uncapped, Mailbox=VSync latest, Fifo=blocking VSync)",
            std::env::var("TSFP_PRESENT_MODE").unwrap_or_else(|_| "uncapped".into())
        );
    }
}

/// Take the newest picture out of the mailbox, upload it, and letterbox the sprite into the window.
fn show_picture(
    mut screen: ResMut<Screen>,
    mut images: ResMut<Assets<Image>>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
    mut sprite: Query<&mut Sprite, With<PictureSprite>>,
) {
    let mut spare = std::mem::take(&mut screen.spare);
    let mut displayed = false;
    let taken = if screen.share.is_some() {
        let physical = window
            .single()
            .map(|window| (window.physical_width(), window.physical_height()))
            .unwrap_or((640, 480));
        share_picture(&mut screen, &mut images, physical, &mut sprite);
        let shared = screen.share.as_ref().and_then(|view| view.handle.as_ref());
        let mailbox = sprite.iter_mut().fold(false, |_, mut s| {
            tsfp_movie_overlay::present(shared, &screen.handle, &mut s.image)
        });
        mailbox.then(|| SHARED.take(&mut spare)).flatten()
    } else if crate::wgpu_live::enabled() {
        // Picture at the window's physical size from the wgpu renderer (CPU hand-off); the mailbox picture of the C sink is ignored.
        let physical = window
            .single()
            .map(|window| (window.physical_width(), window.physical_height()))
            .unwrap_or((640, 480));
        let aspect = present::plan(present::options().aspect).picture_aspect;
        let size = crate::wgpu_live::output_size(physical, aspect);
        let screen = &mut *screen;
        screen
            .pump
            .poll(size, &mut spare)
            .map(|taken| (0, taken.width, taken.height))
    } else {
        SHARED.take(&mut spare)
    };
    if let Some((seq, width, height)) = taken {
        displayed = true;
        if !crate::wgpu_live::enabled() {
            crate::fps::game_frame(seq);
        }
        if (width, height) == screen.size && screen.share.is_none() {
            if let Some(image) = images.get_mut(&screen.handle) {
                spare = image.data.replace(spare).unwrap_or_default();
            }
        } else {
            let mut image = blank(width, height);
            image.data = Some(std::mem::take(&mut spare));
            let _ = images.insert(screen.handle.id(), image);
            screen.size = (width, height);
        }
    }
    if displayed {
        SHARED.counters.bevy_frames.fetch_add(1, Ordering::Relaxed);
    }
    screen.spare = spare;
    if let (Ok(window), Ok(mut sprite)) = (window.single(), sprite.single_mut()) {
        // The buffer is 640x480 in every game mode; the picture aspect is the game's own (4:3, or 16:9 anamorphic).
        let aspect = present::plan(present::options().aspect).picture_aspect;
        let (fit_width, fit_height) = present::fit((window.width(), window.height()), aspect);
        sprite.custom_size = Some(Vec2::new(fit_width, fit_height));
    }
    let mut wi = lock(&SHARED.wi);
    if wi.title_changed {
        wi.title_changed = false;
        if let (Some(text), Ok(mut window)) = (wi.title.clone(), window.single_mut()) {
            window.title = text;
        }
    }
}

fn quit_when_game_is_done(mut exit: EventWriter<AppExit>) {
    if SHARED.game_done.load(Ordering::Acquire) {
        exit.write(AppExit::Success);
    }
}

/// T3390: `TSFP_RENDER=none` (rust_trace `--render none`): no window, swapchain, render world,
/// wgpu device or audio output. Only the input, replay, native UI input and quit systems run, on a
/// ScheduleRunner loop, so a replay needs no GPU and no display.
pub fn render_none_switch(value: Option<&str>) -> bool {
    value == Some("none")
}

pub fn render_none() -> bool {
    static NONE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *NONE.get_or_init(|| render_none_switch(std::env::var("TSFP_RENDER").ok().as_deref()))
}

/// T3394: a headless update has nothing to do until the guest polls input again (replay rows, UI
/// pages and quit all follow guest polls), so block on the progress gate instead of spinning. The
/// 2 ms bound keeps quit and wall-clock systems moving if the guest stalls.
fn wait_for_guest_progress(mut last_poll: Local<u64>) {
    static SPIN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *SPIN.get_or_init(|| std::env::var_os("TSFP_HEADLESS_SPIN").is_some()) {
        return; // diagnostic: the T3390 uncapped loop
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2);
    let mut seen = crate::shared::PROGRESS.current();
    // Only a new guest poll counts: Bevy's own delivery bumps the gate too.
    while crate::wi::host_poll_index() == *last_poll && !SHARED.game_done.load(Ordering::Acquire) {
        let now = std::time::Instant::now();
        if now >= deadline {
            break;
        }
        seen = crate::shared::PROGRESS.wait_past(seen, deadline - now);
    }
    *last_poll = crate::wi::host_poll_index();
    crate::shared::HEADLESS_UPDATES.fetch_add(1, Ordering::Relaxed);
}

fn run_headless() -> AppExit {
    let mut app = App::new();
    app.insert_resource(NativeSessionStateRes::default())
        .add_plugins((
            // T3394: a handful of systems run per update; a small pool keeps idle workers few.
            bevy::app::TaskPoolPlugin {
                task_pool_options: bevy::app::TaskPoolOptions::with_num_threads(2),
            },
            bevy::diagnostic::FrameCountPlugin,
            bevy::time::TimePlugin,
            bevy::input::InputPlugin,
            bevy::log::LogPlugin::default(),
            bevy::window::WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
            },
            ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO),
        ));
    tsfp_input_intake::install(&mut app); // T3820: the one device intake, before every consumer
    if let Some(options) = crate::input_router::options() {
        app.insert_resource(crate::input_router::RouterRes::new(options))
            .add_systems(
                PreUpdate,
                (
                    keyboard_events,
                    crate::input_router::pad_events,
                    crate::input_router::mouse_events,
                    crate::input_router::tick,
                    crate::input_router::rumble_system,
                )
                    .chain()
                    .after(bevy::input::InputSystem),
            );
    } else {
        app.add_systems(
            PreUpdate,
            (keyboard_events, gamepad_events).after(bevy::input::InputSystem),
        );
    }
    app.add_systems(First, wait_for_guest_progress);
    app.add_systems(Update, quit_when_game_is_done);
    app.add_systems(
        PreUpdate,
        crate::text_entry_live::text_entry_tick.after(keyboard_events),
    );
    if crate::ui_overlay::prepared().is_some() {
        app.add_systems(
            PreUpdate,
            crate::ui_live::ui_live_input
                .after(bevy::input::InputSystem)
                .before(keyboard_events)
                .before(gamepad_events)
                .before(crate::input_router::pad_events),
        );
        app.add_systems(
            PreUpdate,
            crate::ui_live::storage_window_input
                .after(keyboard_events)
                .before(gamepad_events)
                .before(crate::input_router::pad_events)
                .before(crate::input_router::mouse_events),
        );
        app.add_systems(
            PreUpdate,
            crate::mapmaker_ui_live::mouse_tick.after(bevy::input::InputSystem),
        );
    }
    eprintln!(
        "tsfp-bevy: --render none (T3390): no window, swapchain, render world, GPU device or audio output"
    );
    let exit = app.run();
    eprintln!(
        "tsfp-bevy: --render none updates {} for {} guest polls (T3394: each update waits for a guest poll)",
        crate::shared::HEADLESS_UPDATES.load(Ordering::Relaxed),
        crate::wi::host_poll_index()
    );
    exit
}

pub fn run() -> AppExit {
    if render_none() {
        return run_headless();
    }
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(NativeSessionStateRes::default())
        .insert_resource(winit_settings())
        .add_plugins(plugins())
        .add_plugins(crate::audio::AudioBridgePlugin)
        .add_systems(Startup, (setup, spawn_ui_overlay));
    crate::native_render_prep::install(&mut app);
    tsfp_input_intake::install(&mut app); // T3820: the one device intake, before every consumer
    if let Some(options) = crate::input_router::options() {
        // T2160: the router replaces the plain gamepad path (keyboard_events stays: close, focus, keys for the hotkeys).
        app.insert_resource(crate::input_router::RouterRes::new(options))
            .add_systems(
                PreUpdate,
                (
                    keyboard_events,
                    crate::input_router::pad_events,
                    crate::input_router::mouse_events,
                    crate::input_router::tick,
                    crate::input_router::rumble_system,
                )
                    .chain()
                    .after(bevy::input::InputSystem),
            );
    } else {
        app.add_systems(
            PreUpdate,
            (keyboard_events, gamepad_events).after(bevy::input::InputSystem),
        );
    }
    app.add_systems(Update, (show_picture, quit_when_game_is_done));
    // T2169: host keyboard typing into the original text pages (closed-loop pad taps).
    app.add_systems(
        PreUpdate,
        crate::text_entry_live::text_entry_tick.after(keyboard_events),
    );
    if crate::ui_overlay::prepared().is_some() {
        use bevy::render::{Render, RenderApp, RenderSet};
        app.add_systems(Update, follow_picture_and_toggle.after(show_picture));
        // T2188: the Rust page routes devices before the plain pad path (which holds back pad events while it owns input).
        app.add_systems(
            PreUpdate,
            crate::ui_live::ui_live_input
                .after(bevy::input::InputSystem)
                .before(keyboard_events)
                .before(gamepad_events)
                .before(crate::input_router::pad_events),
        );
        app.add_systems(
            PreUpdate,
            crate::ui_live::storage_window_input
                .after(keyboard_events)
                .before(gamepad_events)
                .before(crate::input_router::pad_events)
                .before(crate::input_router::mouse_events),
        );
        app.add_systems(
            PreUpdate,
            crate::mapmaker_ui_live::mouse_tick.after(bevy::input::InputSystem),
        ); // T1916
        if crate::ui_overlay::overlay_sprite()
            && let Some(render) = app.get_sub_app_mut(RenderApp)
        {
            render.add_systems(Render, draw_ui_overlay.in_set(RenderSet::Prepare));
        }
    }
    if crate::gpu_share::gpu().is_some() {
        use bevy::render::{Render, RenderApp, RenderSet};

        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render.add_systems(
                Render,
                adopt_shared_texture.in_set(RenderSet::PrepareResources),
            );
        }
    }
    // T2325: same-frame extraction is a diagnostic control until measured
    // latency improves. Keep the previous pipeline by default.
    if std::env::var("TSFP_PIPELINED_PRESENT").as_deref() != Ok("0") {
        app.add_plugins(bevy::render::pipelined_rendering::PipelinedRenderingPlugin);
    }
    crate::present_latency::install(&mut app);
    crate::frame_budget::install(&mut app);
    app.add_systems(Update, upgrade_present_mode);
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        render.add_systems(
            bevy::render::Render,
            probe_present_mode.before(bevy::render::view::window::create_surfaces),
        );
        render.add_systems(
            bevy::render::Render,
            crate::fps::acquired
                .after(bevy::render::RenderSet::Prepare)
                .before(bevy::render::RenderSet::Render),
        );
        render.add_systems(
            bevy::render::Render,
            crate::fps::presented
                .after(bevy::render::RenderSet::Render)
                .before(bevy::render::RenderSet::Cleanup),
        );
    }
    app.run()
}

/// Keeps the headless schedule runner type referenced for a future windowless mode.
#[allow(dead_code)]
type Headless = ScheduleRunnerPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_share::{PUBLISHED, Published, Target, create_gpu};
    use bevy::render::render_asset::RenderAssets;

    #[test]
    fn render_none_is_an_explicit_switch() {
        assert!(render_none_switch(Some("none")));
        assert!(!render_none_switch(Some("gpu")));
        assert!(!render_none_switch(None));
    }

    #[test]
    fn unfocused_continuous_mode_is_opt_in() {
        let settings = winit_settings();
        if std::env::var("TSFP_BEVY_UNFOCUSED_CONTINUOUS").as_deref() == Ok("1")
            || crate::input_recording::fast_forward_switch(
                std::env::var("TSFP_FAST_FORWARD").ok().as_deref(),
            )
        {
            assert_eq!(settings.unfocused_mode, bevy::winit::UpdateMode::Continuous);
        } else {
            assert!(matches!(
                settings.unfocused_mode,
                bevy::winit::UpdateMode::Reactive { .. }
            ));
        }
    }
    use bevy::render::renderer::RenderDevice;
    use bevy::render::texture::GpuImage;

    #[test]
    fn uncapped_presentation_and_explicit_vsync_respect_surface_support() {
        use wgpu::PresentMode::{Fifo, Immediate, Mailbox};
        assert_eq!(choose_present_mode(&[Fifo, Mailbox, Immediate], None), 3);
        assert_eq!(choose_present_mode(&[Fifo, Mailbox], None), 1);
        assert_eq!(choose_present_mode(&[Fifo], None), 2);
        assert_eq!(
            choose_present_mode(&[Fifo, Mailbox, Immediate], Some("fifo")),
            2
        );
        assert_eq!(
            choose_present_mode(&[Fifo, Mailbox, Immediate], Some("vsync")),
            1
        );
        assert_eq!(
            choose_present_mode(&[Fifo, Mailbox, Immediate], Some("mailbox")),
            1
        );
        assert_eq!(choose_present_mode(&[Fifo, Immediate], Some("mailbox")), 3);
        assert_eq!(choose_present_mode(&[Fifo, Immediate], Some("vsync")), 2);
    }

    /// The render world half of the texture import, headless: the shell texture becomes the `GpuImage` of its image id, a repeat
    /// does nothing, and a resize (new id, new texture) is adopted under the new id.
    #[test]
    fn the_shell_texture_is_adopted_as_the_gpu_image_of_its_handle() {
        let gpu = match create_gpu() {
            Ok(gpu) => gpu,
            Err(error) => {
                assert!(
                    std::env::var_os("TSFP_REQUIRE_GPU").is_none(),
                    "TSFP_REQUIRE_GPU set but no device: {error}"
                );
                eprintln!("SKIP: no Vulkan device: {error}");
                return;
            }
        };
        let mut world = World::new();
        world.insert_resource(RenderAssets::<GpuImage>::default());
        world.insert_resource(RenderDevice::from(gpu.device.clone()));
        let id = |n: u128| bevy::asset::AssetId::<Image>::Uuid {
            uuid: bevy::asset::uuid::Uuid::from_u128(n),
        };
        let target = Target::new(&gpu.device, 64, 48);
        *PUBLISHED.lock().unwrap() = Some(Published {
            id: id(1),
            target: target.clone(),
        });
        let adopt = world.register_system(adopt_shared_texture);
        world.run_system(adopt).unwrap();
        {
            let images = world.resource::<RenderAssets<GpuImage>>();
            let image = images.get(id(1)).expect("adopted");
            assert_eq!((image.size.width, image.size.height), (64, 48));
            assert_eq!(image.texture_format, TextureFormat::Rgba8UnormSrgb);
        }
        let first = world
            .resource::<RenderAssets<GpuImage>>()
            .get(id(1))
            .unwrap()
            .texture
            .id();
        world.run_system(adopt).unwrap();
        assert_eq!(
            world
                .resource::<RenderAssets<GpuImage>>()
                .get(id(1))
                .unwrap()
                .texture
                .id(),
            first,
            "same texture, not re-adopted"
        );
        // A late PrepareAssets placeholder must not permanently replace the
        // shared texture, even when the published serial/asset id are unchanged.
        let placeholder = Target::new(&gpu.device, 1, 1);
        let mut image = world
            .resource::<RenderAssets<GpuImage>>()
            .get(id(1))
            .unwrap()
            .clone();
        image.texture = bevy::render::render_resource::Texture::from(placeholder.texture);
        image.size.width = 1;
        image.size.height = 1;
        world
            .resource_mut::<RenderAssets<GpuImage>>()
            .insert(id(1), image);
        world.run_system(adopt).unwrap();
        assert_eq!(
            world
                .resource::<RenderAssets<GpuImage>>()
                .get(id(1))
                .unwrap()
                .size
                .width,
            64
        );

        *PUBLISHED.lock().unwrap() = Some(Published {
            id: id(2),
            target: Target::new(&gpu.device, 128, 96),
        });
        world.run_system(adopt).unwrap();
        let images = world.resource::<RenderAssets<GpuImage>>();
        assert_eq!(
            images
                .get(id(2))
                .map(|image| (image.size.width, image.size.height)),
            Some((128, 96))
        );
        *PUBLISHED.lock().unwrap() = None;
    }
}
