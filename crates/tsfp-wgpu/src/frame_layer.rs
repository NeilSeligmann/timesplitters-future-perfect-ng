// SPDX-License-Identifier: GPL-3.0-or-later
//! T2309 UI-11: layers drawn inside the engine frame.
//!
//! A [`FrameLayer`] records its draws into the renderer's own command encoder at the end of every
//! presented frame: after the scene and its post passes (the whole replayed frame, in the guest's
//! pass order) and before the frame leaves the renderer (window present pass copy, readback ring or
//! shared GPU ring). The Rust UI uses it so its batches are composited with the scene by the game's
//! renderer, on the same device and in the same submission, instead of being a picture laid over the
//! finished frame by the window shell.
//!
//! Contract: the layer's target is `Rgba8Unorm`, single sample, `RENDER_ATTACHMENT | COPY_SRC`,
//! the size of the frame that is shown. The layer must load (never clear) it; it may copy
//! from it (T2313 backdrop pass). The renderer never hands out a guest surface: when the shown image
//! is a guest render target (no window present pass), it is first copied into a private composite
//! texture, so later guest frames that read the back buffer never see the UI.
//!
//! T2313: several layers may be installed, each under a key, drawn in ascending `order` (the HUD
//! under the menu page, for example). [`seal`] is called on the guest thread when the host presents
//! guest frame `present` (tsfp-rt-host `Runtime::present`); a layer latches there what the guest
//! frame decided (for the UI page: whether the original draw was skipped), and [`LayerTarget::present`]
//! names the same guest frame when the render thread composes it, so a layer draws exactly for the
//! guest frames whose decision it latched (no one-frame skip/draw race).
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// What a layer draws with. Device and queue are the renderer's own.
pub struct LayerTarget<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub target: &'a wgpu::Texture,
    /// Presented frames composed so far.
    pub frame: u64,
    /// The guest's own present number of this frame (the `number` passed to [`seal`]), 0 when the
    /// stream does not name its presents.
    pub present: u64,
}

/// A draw pass appended to every presented engine frame.
pub trait FrameLayer: Send {
    /// Record the layer's draws (or nothing). Returns true when it drew.
    fn encode(&mut self, target: LayerTarget<'_>) -> bool;
    /// Release resources retained for the just-submitted command buffer only
    /// after its queue work completes. The compositor calls this immediately
    /// after `Queue::submit`.
    fn submitted(&mut self, _queue: &wgpu::Queue) {}
    /// A display diagnostic may redraw independently without changing UI/HUD cadence.
    fn wants_display(&self) -> bool {
        false
    }
    /// Optional display-thread pass. The host must restore a clean scene before every call.
    /// Only layers explicitly implementing this contract may defer their guest-frame draw.
    fn encode_display(&mut self, _target: LayerTarget<'_>) -> bool {
        false
    }
    /// Guest thread: the host presents guest frame `present` (its draw calls are all recorded).
    fn seal(&mut self, _present: u64) {}
    /// Native view-pass draws at an explicit ordered renderer boundary, before feedback sampling.
    /// Defaults to the existing view-pass implementation for compatible layers. Layers must latch
    /// once per frame so a later grade fallback cannot draw the replacement twice.
    fn encode_view_pass(&mut self, target: LayerTarget<'_>) -> bool {
        self.encode_before_grade(target)
    }
    /// T2312: draws that belong to the guest's view pass, recorded into the scene target right
    /// before the frame's closing colour grade pass (T2140) samples it, so they pass through the
    /// grade as the original's own view-pass draws do. `target.target` is that guest surface:
    /// draw only what replaces an original draw into the same surface. Returns true when it drew.
    /// Frames without a grade pass never call this (the layer falls back to [`FrameLayer::encode`]).
    fn encode_before_grade(&mut self, _target: LayerTarget<'_>) -> bool {
        false
    }
    /// T2336: display thread, the view-pass draws of guest frame `target.present` sampled at display rate, into a copy of
    /// that frame's clean pre-grade surface that the native grade then reads. A layer drawing here leaves its view pass
    /// out of the following [`FrameLayer::encode_display`] of the same update.
    fn encode_display_view_pass(&mut self, _target: LayerTarget<'_>) -> bool {
        false
    }
}

struct Installed {
    key: &'static str,
    order: i32,
    layer: Box<dyn FrameLayer>,
}

static INSTALLED: AtomicBool = AtomicBool::new(false);
static LAYERS: Mutex<Vec<Installed>> = Mutex::new(Vec::new());

fn layers() -> std::sync::MutexGuard<'static, Vec<Installed>> {
    LAYERS.lock().unwrap_or_else(|p| p.into_inner())
}

/// Install a frame layer under `key` (replaces the layer of that key). Lower `order` draws first.
pub fn install(key: &'static str, order: i32, layer: Box<dyn FrameLayer>) {
    let mut list = layers();
    list.retain(|l| l.key != key);
    let at = list.partition_point(|l| l.order <= order);
    list.insert(at, Installed { key, order, layer });
    INSTALLED.store(true, Ordering::Release);
}

/// Remove the layer of `key`; with none left, frames are presented untouched again.
pub fn remove(key: &'static str) {
    let mut list = layers();
    list.retain(|l| l.key != key);
    INSTALLED.store(!list.is_empty(), Ordering::Release);
}

/// True when a layer is installed (one atomic load, the per-frame fast path).
pub fn installed() -> bool {
    INSTALLED.load(Ordering::Acquire)
}

/// Guest thread: guest frame `present` is complete. Every installed layer latches its decision.
pub fn seal(present: u64) {
    if !installed() {
        return;
    }
    for installed in layers().iter_mut() {
        installed.layer.seal(present);
    }
}

/// Run the installed layers' view-pass draws in order, before the grade pass. Returns true when any drew.
pub(crate) fn encode_before_grade(target: LayerTarget<'_>) -> bool {
    let mut drew = false;
    for installed in layers().iter_mut() {
        drew |= installed.layer.encode_before_grade(LayerTarget {
            device: target.device,
            queue: target.queue,
            encoder: &mut *target.encoder,
            target: target.target,
            frame: target.frame,
            present: target.present,
        });
    }
    drew
}

/// Run the installed layers in order. Returns true when any drew.
pub(crate) fn encode(target: LayerTarget<'_>) -> bool {
    let mut drew = false;
    for installed in layers().iter_mut() {
        drew |= installed.layer.encode(LayerTarget {
            device: target.device,
            queue: target.queue,
            encoder: &mut *target.encoder,
            target: target.target,
            frame: target.frame,
            present: target.present,
        });
    }
    drew
}

/// Notify layers that their recorded work has entered the queue. Implementors
/// can register completion callbacks without treating command encoding as GPU
/// completion.
pub(crate) fn submitted(queue: &wgpu::Queue) {
    for installed in layers().iter_mut() {
        installed.layer.submitted(queue);
    }
}

/// Encode the explicitly ordered native view-pass boundary on its captured target.
pub(crate) fn encode_view_pass(target: LayerTarget<'_>) -> bool {
    let mut drew = false;
    for installed in layers().iter_mut() {
        drew |= installed.layer.encode_view_pass(LayerTarget {
            device: target.device,
            queue: target.queue,
            encoder: &mut *target.encoder,
            target: target.target,
            frame: target.frame,
            present: target.present,
        });
    }
    drew
}

/// T2242: explicitly activated by the shared-device shell, never by the renderer alone.
static DISPLAY_RATE: AtomicBool = AtomicBool::new(false);
pub fn set_display_rate(enabled: bool) {
    DISPLAY_RATE.store(enabled, Ordering::Release);
}
pub fn display_rate() -> bool {
    DISPLAY_RATE.load(Ordering::Acquire)
}
/// T2326: diagnostic layers can request a clean-scene display update independently.
pub fn needs_display() -> bool {
    display_rate() || layers().iter().any(|layer| layer.layer.wants_display())
}

/// Recompose participating layers on a freshly restored scene at the shell's display cadence.
/// HUD view-pass layers keep their original grade placement until they implement this contract.
pub fn encode_display(mut target: LayerTarget<'_>) -> bool {
    let all = display_rate();
    if !all && !needs_display() {
        return false;
    }
    DISPLAY_UPDATES.fetch_add(1, Ordering::Relaxed);
    let mut list = layers();
    let mut drew = all && encode_display_regrade(&mut target, &mut list);
    for installed in list.iter_mut() {
        if !all && !installed.layer.wants_display() {
            continue;
        }
        let layer_drew = installed.layer.encode_display(LayerTarget {
            device: target.device,
            queue: target.queue,
            encoder: &mut *target.encoder,
            target: target.target,
            frame: target.frame,
            present: target.present,
        });
        drew |= layer_drew;
        if layer_drew {
            match installed.key {
                "ui-page" => {
                    DISPLAY_PAGE_DRAWS.fetch_add(1, Ordering::Relaxed);
                }
                "hud" => {
                    DISPLAY_HUD_DRAWS.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    }
    drew
}

static DISPLAY_UPDATES: AtomicU64 = AtomicU64::new(0);
static DISPLAY_PAGE_DRAWS: AtomicU64 = AtomicU64::new(0);
static DISPLAY_HUD_DRAWS: AtomicU64 = AtomicU64::new(0);
/// Display composition counts, separate from acquired guest frames.
pub fn display_stats() -> (u64, u64, u64) {
    (
        DISPLAY_UPDATES.load(Ordering::Relaxed),
        DISPLAY_PAGE_DRAWS.load(Ordering::Relaxed),
        DISPLAY_HUD_DRAWS.load(Ordering::Relaxed),
    )
}

/// T2336: what the shell needs to redo a guest frame's closing grade at display rate: the grade's source surface
/// copied before any view-pass layer drew into it, the frame's typed grade and its scissor in target pixels.
#[derive(Clone)]
pub struct DisplayGrade {
    /// The renderer's device (the shell shares it; a display target of another device never matches).
    pub device: wgpu::Device,
    pub present: u64,
    pub pregrade: wgpu::Texture,
    pub model: crate::grade::GradeModel,
    pub scissor: (u32, u32, u32, u32),
}

static DISPLAY_GRADES: Mutex<Vec<DisplayGrade>> = Mutex::new(Vec::new());

/// Render thread: latch the native grade of guest frame `grade.present` (the newest few are kept, the shell may lag).
pub(crate) fn latch_display_grade(grade: DisplayGrade) {
    let mut list = DISPLAY_GRADES.lock().unwrap_or_else(|p| p.into_inner());
    list.retain(|g| g.present != grade.present || g.device != grade.device);
    list.push(grade);
    if list.len() > 8 {
        list.remove(0);
    }
}

/// The latched grade of guest frame `present`, if the renderer drew that frame's grade natively.
pub fn display_grade(device: &wgpu::Device, present: u64) -> Option<DisplayGrade> {
    let list = DISPLAY_GRADES.lock().unwrap_or_else(|p| p.into_inner());
    list.iter()
        .rev()
        .find(|g| g.present == present && g.device == *device)
        .cloned()
}

struct Regrade {
    device: wgpu::Device,
    format: wgpu::TextureFormat,
    native: crate::grade::NativeGrade,
    scratch: Option<wgpu::Texture>,
}

static REGRADE: Mutex<Option<Regrade>> = Mutex::new(None);
static DISPLAY_REGRADES: AtomicU64 = AtomicU64::new(0);
static DISPLAY_REGRADE_MISSES: AtomicU64 = AtomicU64::new(0);

/// T2336 counts: display updates that redid the grade with display-rate view-pass draws, and display updates that
/// kept the guest-rate view pass (no native grade latched for the shown frame, or a size mismatch).
pub fn display_regrade_stats() -> (u64, u64) {
    (
        DISPLAY_REGRADES.load(Ordering::Relaxed),
        DISPLAY_REGRADE_MISSES.load(Ordering::Relaxed),
    )
}

/// The pre-grade display composition, original order: clean pre-grade scene, view-pass layers sampled at display rate,
/// native grade into `target` over the grade's scissor. Returns true when it ran (then the layers drew their view pass).
fn encode_display_regrade(target: &mut LayerTarget<'_>, list: &mut [Installed]) -> bool {
    let Some(grade) = display_grade(target.device, target.present) else {
        DISPLAY_REGRADE_MISSES.fetch_add(1, Ordering::Relaxed);
        return false;
    };
    if grade.pregrade.size() != target.target.size() {
        DISPLAY_REGRADE_MISSES.fetch_add(1, Ordering::Relaxed);
        return false;
    }
    let mut regrade = REGRADE.lock().unwrap_or_else(|p| p.into_inner());
    if regrade
        .as_ref()
        .is_none_or(|r| r.device != *target.device || r.format != target.target.format())
    {
        *regrade = Some(Regrade {
            device: target.device.clone(),
            format: target.target.format(),
            native: crate::grade::NativeGrade::new(target.device, target.target.format()),
            scratch: None,
        });
    }
    let state = regrade.as_mut().expect("regrade state");
    if state
        .scratch
        .as_ref()
        .is_none_or(|s| s.size() != grade.pregrade.size())
    {
        state.scratch = Some(target.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tsfp-display-pregrade"),
            size: grade.pregrade.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: grade.pregrade.format(),
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        }));
    }
    let scratch = state.scratch.as_ref().expect("scratch allocated");
    target.encoder.copy_texture_to_texture(
        grade.pregrade.as_image_copy(),
        scratch.as_image_copy(),
        scratch.size(),
    );
    for installed in list.iter_mut() {
        installed.layer.encode_display_view_pass(LayerTarget {
            device: target.device,
            queue: target.queue,
            encoder: &mut *target.encoder,
            target: scratch,
            frame: target.frame,
            present: target.present,
        });
    }
    let scene = scratch.create_view(&Default::default());
    let out = target.target.create_view(&Default::default());
    state.native.encode(
        target.device,
        target.encoder,
        &scene,
        &out,
        &grade.model,
        Some(grade.scissor),
    );
    DISPLAY_REGRADES.fetch_add(1, Ordering::Relaxed);
    true
}
