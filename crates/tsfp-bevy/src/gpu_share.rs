// SPDX-License-Identifier: GPL-3.0-or-later
//! T2229: `--wgpu-share`, the wgpu frame as a GPU texture in the Bevy window with no CPU readback (design: docs/t-wgpu-gpu-share.md).
//!
//! The shell creates the wgpu `Instance`/`Adapter`/`Device`/`Queue` itself ([`create_gpu`]), gives clones to the render runtime
//! (`tsfp_rt_host_share_gpu`) and to Bevy (`RenderCreation::Manual`). The runtime copies every finished frame into a ring of
//! published textures on the same queue. Per Update the shell acquires the newest one ([`FrameSource`]), blits it into its own
//! `Rgba8UnormSrgb` texture with alpha forced to 1 (the renderer writes alpha 0 everywhere) and releases the slot; Bevy's sprite
//! pass samples the shell texture (adopted as the `GpuImage` of the picture handle by the render world system in `app.rs`).
//! Same queue, so submission order orders copy, blit and sprite pass without a fence.
//!
//! The render runtime is linked into this binary (the `tsfp-rt-host` rlib, T2246), so shell, Bevy and renderer use ONE copy of wgpu
//! 24.0.5. (T2229 first bound `libtsfp_rt_host.so` with dlsym: two copies of wgpu-core and of its `parking_lot_core` hash table,
//! and a lock contended across the copies never woke its waiter. docs/t-wgpu-renderer.md "T2227/T2229 A/B".) [`layout_words`] is
//! still compared with the runtime's before anything is shared, any difference keeps the CPU path (`--wgpu-live` hand-off).
use crate::wgpu_live::Pump;
use std::ffi::{CString, c_char, c_int, c_void};
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Version of the C ABI below (word 5 of [`layout_words`]).
pub const ABI_VERSION: u64 = 1;
/// The wgpu both sides must be built with (word 0 of [`layout_words`] is its FNV-1a hash).
pub const WGPU_VERSION: &str = "24.0.5";
/// Largest texture side requested from the adapter (what `tsfp-wgpu` requests too).
const MAX_TEXTURE_SIDE: u32 = 8192;

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Words both libraries must agree on: wgpu version hash, sizes of `Device`, `Queue`, `Texture`, alignment of `Device`, ABI version.
pub fn layout_words() -> [u64; 6] {
    [
        fnv(WGPU_VERSION),
        size_of::<wgpu::Device>() as u64,
        size_of::<wgpu::Queue>() as u64,
        size_of::<wgpu::Texture>() as u64,
        align_of::<wgpu::Device>() as u64,
        ABI_VERSION,
    ]
}

// ---------------------------------------------------------------------------------------------------------------------------
// Adapter and device
// ---------------------------------------------------------------------------------------------------------------------------

/// The device the shell owns and shares. All four are cheap `Arc` handles, `Send + Sync`.
#[derive(Clone)]
pub struct SharedGpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
}

/// Preference rank of an adapter: lower is better (discrete, integrated, virtual, other, software).
fn type_rank(kind: wgpu::DeviceType) -> u8 {
    match kind {
        wgpu::DeviceType::DiscreteGpu => 0,
        wgpu::DeviceType::IntegratedGpu => 1,
        wgpu::DeviceType::VirtualGpu => 2,
        wgpu::DeviceType::Other => 3,
        wgpu::DeviceType::Cpu => 4,
    }
}

/// Index of the adapter to open among `(name, type)` pairs: a name containing `wanted` (case blind, `WGPU_ADAPTER_NAME`) wins,
/// else the best device type, the first on a tie. `None` when nothing matches the name or the list is empty. Pure, unit tested.
pub fn pick_adapter(
    adapters: &[(String, wgpu::DeviceType)],
    wanted: Option<&str>,
) -> Option<usize> {
    if let Some(wanted) = wanted
        .map(str::to_lowercase)
        .filter(|wanted| !wanted.is_empty())
    {
        return adapters
            .iter()
            .position(|(name, _)| name.to_lowercase().contains(&wanted));
    }
    (0..adapters.len()).min_by_key(|&index| (type_rank(adapters[index].1), index))
}

/// Open the Vulkan adapter (discrete GPU preferred, `WGPU_ADAPTER_NAME` honoured) with limits for 4096+ textures. `Err` says why
/// no device could be made, which becomes the reason in the startup line.
pub fn create_gpu() -> Result<SharedGpu, String> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        flags: wgpu::InstanceFlags::from_build_config().with_env(),
        ..Default::default()
    });
    let mut adapters = instance.enumerate_adapters(wgpu::Backends::VULKAN);
    let described: Vec<(String, wgpu::DeviceType)> = adapters
        .iter()
        .map(|adapter| {
            let info = adapter.get_info();
            (info.name, info.device_type)
        })
        .collect();
    let wanted = std::env::var("WGPU_ADAPTER_NAME").ok();
    let chosen = pick_adapter(&described, wanted.as_deref()).ok_or_else(|| match &wanted {
        Some(name) => format!(
            "no Vulkan adapter matches WGPU_ADAPTER_NAME={name} (found: {})",
            list(&described)
        ),
        None => "no Vulkan adapter found".to_string(),
    })?;
    let adapter = adapters.swap_remove(chosen);
    let info = adapter.get_info();
    if info.backend != wgpu::Backend::Vulkan {
        return Err(format!(
            "adapter {} is {:?}, not Vulkan",
            info.name, info.backend
        ));
    }
    let supported = adapter.limits();
    let mut limits = wgpu::Limits::default().using_resolution(supported.clone());
    limits.max_texture_dimension_2d = supported
        .max_texture_dimension_2d
        .min(MAX_TEXTURE_SIDE)
        .max(2048);
    let request = |limits: wgpu::Limits| {
        bevy::tasks::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("tsfp-shared"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
    };
    let (device, queue) = request(limits.clone()).or_else(|first| {
        // Same fallback as `tsfp-wgpu`'s own context: downlevel defaults with the raised texture side.
        let fallback = wgpu::Limits {
            max_texture_dimension_2d: limits.max_texture_dimension_2d,
            ..wgpu::Limits::downlevel_defaults()
        };
        request(fallback).map_err(|second| {
            format!(
                "request_device on {}: {first}; downlevel retry: {second}",
                info.name
            )
        })
    })?;
    Ok(SharedGpu {
        instance,
        adapter,
        device,
        queue,
        info,
    })
}

fn list(adapters: &[(String, wgpu::DeviceType)]) -> String {
    adapters
        .iter()
        .map(|(name, kind)| format!("{name} ({kind:?})"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One line that names the adapter of the shared device.
pub fn describe(info: &wgpu::AdapterInfo) -> String {
    format!(
        "{} ({:?}, {:?}, {} {})",
        info.name, info.backend, info.device_type, info.driver, info.driver_info
    )
    .trim()
    .to_string()
}

static GPU: OnceLock<SharedGpu> = OnceLock::new();

/// The device made by [`prepare`], for `RenderCreation::Manual` and the blit.
pub fn gpu() -> Option<&'static SharedGpu> {
    GPU.get()
}

// ---------------------------------------------------------------------------------------------------------------------------
// The render runtime ABI (crates/tsfp-rt-host/src/ffi.rs, found with dlsym so a host without it still links)
// ---------------------------------------------------------------------------------------------------------------------------

/// `TsfpSharedFrame` of the C ABI: a published texture the shell may sample until it releases the slot.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawFrame {
    /// `*const wgpu::Texture`, valid until `tsfp_rt_host_shared_release(slot)`.
    pub texture: *const c_void,
    pub width: u32,
    pub height: u32,
    pub present: u64,
    pub slot: u32,
}

type LayoutFn = unsafe extern "C" fn(*mut u64);
type IdentityFn = unsafe extern "C" fn() -> usize;
type ShareFn = unsafe extern "C" fn(*const c_void, *const c_void, *const c_char) -> i32;
type AcquireFn = unsafe extern "C" fn(*mut c_void, *mut RawFrame) -> i32;
type ReleaseFn = unsafe extern "C" fn(*mut c_void, u32);
type CurrentFn = unsafe extern "C" fn() -> *mut c_void;
type SetOutputFn = unsafe extern "C" fn(*mut c_void, u32, u32) -> c_int;

#[derive(Clone, Copy)]
struct RuntimeApi {
    layout: LayoutFn,
    identity: IdentityFn,
    share: ShareFn,
    acquire: AcquireFn,
    release: ReleaseFn,
    current: CurrentFn,
    set_output: SetOutputFn,
}

/// The render runtime is linked into this binary (the `tsfp-rt-host` rlib, T2246), so there is ONE copy of wgpu-core and of its
/// `parking_lot_core` hash table in the process. With the earlier dlsym binding to `libtsfp_rt_host.so` the two copies shared locks
/// they could not wake each other on. The pointers keep the C ABI types of the documented functions.
fn runtime_api() -> Result<RuntimeApi, String> {
    use tsfp_rt_host::ffi;
    // SAFETY: only pointer arguments and results change type; the functions are the documented C ABI of docs/t-wgpu-gpu-share.md.
    unsafe {
        Ok(RuntimeApi {
            layout: ffi::tsfp_rt_host_share_layout,
            identity: ffi::tsfp_rt_host_wgpu_identity,
            share: std::mem::transmute::<*const (), ShareFn>(
                ffi::tsfp_rt_host_share_gpu as *const (),
            ),
            acquire: std::mem::transmute::<*const (), AcquireFn>(
                ffi::tsfp_rt_host_shared_acquire as *const (),
            ),
            release: std::mem::transmute::<*const (), ReleaseFn>(
                ffi::tsfp_rt_host_shared_release as *const (),
            ),
            current: std::mem::transmute::<*const (), CurrentFn>(
                ffi::tsfp_rt_host_current as *const (),
            ),
            set_output: std::mem::transmute::<*const (), SetOutputFn>(
                ffi::tsfp_rt_host_set_output as *const (),
            ),
        })
    }
}

static API: OnceLock<RuntimeApi> = OnceLock::new();

/// Compare the runtime's layout words with ours. Pure, unit tested.
pub fn check_layout(theirs: &[u64; 6], ours: &[u64; 6]) -> Result<(), String> {
    const NAMES: [&str; 6] = [
        "wgpu version",
        "size of Device",
        "size of Queue",
        "size of Texture",
        "alignment of Device",
        "ABI version",
    ];
    match (0..6).find(|&at| theirs[at] != ours[at]) {
        None => Ok(()),
        Some(at) => Err(format!(
            "render runtime and shell disagree on the {} ({} against {})",
            NAMES[at], theirs[at], ours[at]
        )),
    }
}

/// A layout match cannot establish that wgpu's process-global locks and
/// registries are the same instance. Compare code identity before passing any
/// Rust handle to the runtime.
pub fn check_wgpu_identity(runtime: usize, shell: usize) -> Result<(), String> {
    if runtime != 0 && runtime == shell {
        Ok(())
    } else {
        Err("the render runtime and shell contain separate wgpu copies; use the CPU frame handoff until they link one wgpu copy".into())
    }
}

/// Everything that decides the path, once, before the game thread starts: ABI present, layouts equal, device made and handed to the
/// runtime. `Ok(adapter description)` means the GPU texture path is active; `Err(reason)` keeps the CPU hand-off.
pub fn prepare() -> Result<String, String> {
    let api = runtime_api()?;
    let mut theirs = [0u64; 6];
    // SAFETY: `theirs` has the 6 words the ABI writes.
    unsafe { (api.layout)(theirs.as_mut_ptr()) };
    check_layout(&theirs, &layout_words())?;
    // SAFETY: the additive ABI returns a code address and dereferences nothing.
    check_wgpu_identity(
        unsafe { (api.identity)() },
        wgpu::Device::features as *const () as usize,
    )?;
    let gpu = create_gpu()?;
    let name = CString::new(describe(&gpu.info).replace('\0', " ")).unwrap_or_default();
    // SAFETY: both pointers refer to live wgpu 24.0.5 objects, the runtime clones them before returning (layout checked above).
    let status = unsafe {
        (api.share)(
            std::ptr::from_ref(&gpu.device).cast(),
            std::ptr::from_ref(&gpu.queue).cast(),
            name.as_ptr(),
        )
    };
    if status != 0 {
        return Err(format!(
            "the render runtime refused the shared device (status {status})"
        ));
    }
    let description = describe(&gpu.info);
    let _ = API.set(api);
    let _ = GPU.set(gpu);
    Ok(description)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Frame source
// ---------------------------------------------------------------------------------------------------------------------------

/// One published frame, held until [`FrameSource::release`].
pub struct Acquired {
    pub texture: wgpu::Texture,
    pub width: u32,
    pub height: u32,
    pub present: u64,
    pub slot: u32,
}

/// Where published frames come from: the render runtime, or a test double.
pub trait FrameSource: Send + Sync {
    /// False while there is nobody to ask (the runtime is created by the game thread a moment after the window): no request may be
    /// consumed then, or the first window size would be lost and the picture would stay at the internal size.
    fn ready(&self) -> bool {
        true
    }
    /// Ask for a picture of this size (the runtime reallocates its ring).
    fn set_output(&mut self, width: u32, height: u32);
    /// The newest frame not yet acquired, held until released.
    fn acquire(&mut self) -> Option<Acquired>;
    fn release(&mut self, slot: u32);
}

/// The real source: `libtsfp_rt_host.so`.
pub struct RuntimeSource;

impl RuntimeSource {
    fn runtime() -> Option<(RuntimeApi, *mut c_void)> {
        let api = *API.get()?;
        // SAFETY: the runtime pointer is read fresh each call; the C host destroys it at exit after the shell ended.
        let runtime = unsafe { (api.current)() };
        (!runtime.is_null() && !crate::shared::SHARED.game_done.load(Ordering::Acquire))
            .then_some((api, runtime))
    }
}

// SAFETY: only calls the thread safe C functions of the runtime.
unsafe impl Send for RuntimeSource {}
unsafe impl Sync for RuntimeSource {}

impl FrameSource for RuntimeSource {
    fn ready(&self) -> bool {
        Self::runtime().is_some()
    }

    fn set_output(&mut self, width: u32, height: u32) {
        if let Some((api, runtime)) = Self::runtime() {
            // SAFETY: as in `runtime`.
            unsafe { (api.set_output)(runtime, width, height) };
        }
    }

    fn acquire(&mut self) -> Option<Acquired> {
        let (api, runtime) = Self::runtime()?;
        let mut raw = RawFrame {
            texture: std::ptr::null(),
            width: 0,
            height: 0,
            present: 0,
            slot: 0,
        };
        // SAFETY: `raw` is a writable `TsfpSharedFrame`.
        if unsafe { (api.acquire)(runtime, &mut raw) } != 1 || raw.texture.is_null() {
            return None;
        }
        // SAFETY: `texture` is a `*const wgpu::Texture` valid until the slot is released; the clone is an `Arc` handle.
        let texture = unsafe { &*raw.texture.cast::<wgpu::Texture>() }.clone();
        Some(Acquired {
            texture,
            width: raw.width,
            height: raw.height,
            present: raw.present,
            slot: raw.slot,
        })
    }

    fn release(&mut self, slot: u32) {
        if let Some((api, runtime)) = Self::runtime() {
            // SAFETY: `slot` came from `acquire`.
            unsafe { (api.release)(runtime, slot) };
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Blit into the shell's own texture
// ---------------------------------------------------------------------------------------------------------------------------

const BLIT_SHADER: &str = "
@group(0) @binding(0) var source: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let corner = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = textureLoad(source, vec2<i32>(at.xy), 0);
    return vec4<f32>(texel.rgb, 1.0);
}
";

/// The shell's picture texture: sampled by Bevy as `Rgba8UnormSrgb` (the bytes mean what the CPU path's `Image` meant), written
/// through an `Rgba8Unorm` view so the blit copies the renderer's bytes unchanged.
#[derive(Clone)]
pub struct Target {
    pub texture: wgpu::Texture,
    /// Bevy samples this one.
    pub sampled_view: wgpu::TextureView,
    raw_view: wgpu::TextureView,
    pub size: (u32, u32),
    /// Identity of this texture (wgpu 24 has no public texture id; Bevy's wrapper id changes on every wrap).
    pub serial: u64,
}

static TARGET_SERIAL: AtomicU64 = AtomicU64::new(1);

impl Target {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tsfp-shared-picture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        });
        let sampled_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let raw_view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Rgba8Unorm),
            ..Default::default()
        });
        Self {
            texture,
            sampled_view,
            raw_view,
            size: (width, height),
            serial: TARGET_SERIAL.fetch_add(1, Ordering::Relaxed),
        }
    }
}

static BLIT_BINDINGS: AtomicU64 = AtomicU64::new(0);
static BLIT_CACHE_HITS: AtomicU64 = AtomicU64::new(0);

pub struct Blitter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    bindings: Vec<(wgpu::Texture, wgpu::BindGroup)>,
    cache: bool,
}

impl Blitter {
    pub fn new(device: &wgpu::Device) -> Self {
        Self::with_cache(
            device,
            std::env::var("TSFP_SHARED_BLIT_CACHE").as_deref() != Ok("0"),
        )
    }

    fn with_cache(device: &wgpu::Device, cache: bool) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsfp-shared-blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tsfp-shared-blit"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tsfp-shared-blit"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tsfp-shared-blit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            bindings: Vec::new(),
            cache,
        }
    }

    /// Reuse bindings by GPU texture identity, independently of its changing pixels.
    fn binding(&mut self, device: &wgpu::Device, source: &wgpu::Texture) -> wgpu::BindGroup {
        if self.cache {
            if let Some((_, group)) = self.bindings.iter().find(|(texture, _)| texture == source) {
                BLIT_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
                return group.clone();
            }
        }
        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tsfp-shared-blit"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source_view),
            }],
        });
        BLIT_BINDINGS.fetch_add(1, Ordering::Relaxed);
        if self.cache {
            // Three published ring slots plus the optional display work texture.
            // Texture handle identity survives content updates and changes on resize.
            if self.bindings.len() == 4 {
                self.bindings.remove(0);
            }
            self.bindings.push((source.clone(), group.clone()));
        }
        group
    }

    fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source: &wgpu::Texture,
        target: &Target,
    ) {
        let group = self.binding(device, source);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tsfp-shared-blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.raw_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    pub fn blit(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &wgpu::Texture,
        target: &Target,
    ) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tsfp-shared-blit"),
        });
        self.encode(device, &mut encoder, source, target);
        queue.submit(Some(encoder.finish()));
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Per Update state
// ---------------------------------------------------------------------------------------------------------------------------

pub static FRAMES: AtomicU64 = AtomicU64::new(0);
pub static BLIT_NS: AtomicU64 = AtomicU64::new(0);
pub static RESIZES: AtomicU64 = AtomicU64::new(0);
pub static LAST_W: AtomicU64 = AtomicU64::new(0);
pub static LAST_H: AtomicU64 = AtomicU64::new(0);
pub static LAST_PRESENT: AtomicU64 = AtomicU64::new(0);

/// What one [`Share::step`] produced.
pub struct Stepped {
    /// The target is a new texture (first frame or a resize): the caller makes a new `Image` handle and publishes it.
    pub new_target: bool,
    pub size: (u32, u32),
}

/// The main world half: asks for the window sized output, takes the newest frame, blits it. No Bevy types.
pub struct Share {
    source: Box<dyn FrameSource>,
    gpu: SharedGpu,
    blitter: Blitter,
    pump: Pump,
    target: Option<Target>,
    display_scene: Option<Target>,
    display_work: Option<wgpu::Texture>,
    display_present: u64,
    display_frames: u64,
    pointer: Option<tsfp_ui_wgpu::WgpuUiRenderer<'static>>,
}

impl Share {
    pub fn new(source: Box<dyn FrameSource>, gpu: SharedGpu) -> Self {
        let blitter = Blitter::new(&gpu.device);
        Self {
            source,
            gpu,
            blitter,
            pump: Pump::default(),
            target: None,
            display_scene: None,
            display_work: None,
            display_present: 0,
            display_frames: 0,
            pointer: None,
        }
    }

    pub fn target(&self) -> Option<&Target> {
        self.target.as_ref()
    }

    /// Diagnostic readback of the final composed picture; never used by the live frame pump.
    pub fn capture_rgba(&self) -> Result<(u32, u32, Vec<u8>), tsfp_ui_wgpu::Error> {
        let source = &self
            .target
            .as_ref()
            .ok_or(tsfp_ui_wgpu::Error::Readback)?
            .texture;
        let raw = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tsfp-ui-display-capture"),
            size: source.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(source.as_image_copy(), raw.as_image_copy(), raw.size());
        self.gpu.queue.submit([encoder.finish()]);
        let reader =
            tsfp_ui_wgpu::WgpuUiRenderer::new_owned(&self.gpu.device, &self.gpu.queue, &raw)?;
        Ok((raw.width(), raw.height(), reader.read_target_rgba()?))
    }

    /// One Update: settled size request, acquisition, then optional display redraw.
    /// Original mode returns None without a new frame; display mode holds the last clean scene.
    pub fn step(&mut self, desired: (u32, u32), now: Instant) -> Option<Stepped> {
        self.step_mode(desired, now, tsfp_wgpu::frame_layer::needs_display())
    }

    fn step_mode(&mut self, desired: (u32, u32), now: Instant, display: bool) -> Option<Stepped> {
        if !self.source.ready() {
            return None;
        }
        if let Some((width, height)) = self.pump.wanted(desired, now) {
            self.source.set_output(width, height);
            crate::wgpu_live::RESIZES.fetch_add(1, Ordering::Relaxed);
        }
        let mut new_target = false;
        if let Some(frame) = self.source.acquire() {
            let started = Instant::now();
            let size = (frame.width, frame.height);
            new_target = self
                .target
                .as_ref()
                .is_none_or(|target| target.size != size);
            if new_target {
                self.target = Some(Target::new(&self.gpu.device, size.0, size.1));
                if display {
                    self.display_scene = Some(Target::new(&self.gpu.device, size.0, size.1));
                    self.display_work =
                        Some(self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                            label: Some("tsfp-ui-display-work"),
                            size: frame.texture.size(),
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            usage: wgpu::TextureUsages::COPY_DST
                                | wgpu::TextureUsages::COPY_SRC
                                | wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        }));
                }
                RESIZES.fetch_add(1, Ordering::Relaxed);
            }
            let destination = if display {
                &self.display_scene
            } else {
                &self.target
            };
            if let Some(target) = destination {
                self.blitter
                    .blit(&self.gpu.device, &self.gpu.queue, &frame.texture, target);
            }
            crate::fps::game_frame(frame.present);
            crate::present_latency::handoff(frame.present);
            self.display_present = frame.present;
            self.source.release(frame.slot);
            BLIT_NS.fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            FRAMES.fetch_add(1, Ordering::Relaxed);
            LAST_W.store(u64::from(size.0), Ordering::Relaxed);
            LAST_H.store(u64::from(size.1), Ordering::Relaxed);
            LAST_PRESENT.store(frame.present, Ordering::Relaxed);
        } else if !display {
            return None;
        }
        let target = self.target.as_ref()?;
        if display {
            let clean = self.display_scene.as_ref()?;
            let work = self.display_work.as_ref()?;
            let mut encoder =
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("tsfp-ui-display-compose"),
                    });
            // sRGB/unorm are copy-compatible; this preserves the original bytes, no gamma transform.
            encoder.copy_texture_to_texture(
                clean.texture.as_image_copy(),
                work.as_image_copy(),
                work.size(),
            );
            tsfp_wgpu::frame_layer::encode_display(tsfp_wgpu::frame_layer::LayerTarget {
                device: &self.gpu.device,
                queue: &self.gpu.queue,
                encoder: &mut encoder,
                target: work,
                frame: self.display_frames,
                present: self.display_present,
            });
            if let Some(position) = crate::ui_overlay::display_pointer() {
                use tsfp_ui_renderer::{Batch, Rect, UiRenderer, Vec2, Viewport};
                let result = (|| {
                    let renderer = match &mut self.pointer {
                        Some(renderer) => {
                            renderer.retarget(work)?;
                            renderer
                        }
                        slot @ None => slot.insert(tsfp_ui_wgpu::WgpuUiRenderer::new_owned(
                            &self.gpu.device,
                            &self.gpu.queue,
                            work,
                        )?),
                    };
                    let size = Vec2::new(work.width() as f32, work.height() as f32);
                    let viewport =
                        Viewport::new(size, 1.0, Rect::new(Vec2::default(), size).unwrap())
                            .unwrap();
                    let quads = pointer_quads(position, size);
                    renderer.begin_frame(viewport)?;
                    if let Err(error) = renderer
                        .submit(&[Batch::Quads(&quads)])
                        .and_then(|()| renderer.end_frame_into(&mut encoder))
                    {
                        renderer.abort_frame();
                        return Err(error);
                    }
                    Ok::<(), tsfp_ui_wgpu::Error>(())
                })();
                crate::ui_overlay::display_pointer_ready(result.is_ok());
            }
            // Keep scene restore, UI and final opaque blit in one ordered submission.
            self.blitter
                .encode(&self.gpu.device, &mut encoder, work, target);
            self.gpu.queue.submit([encoder.finish()]);
            self.display_frames += 1;
        }
        Some(Stepped {
            new_target,
            size: target.size,
        })
    }
}

/// Opt-in native arrow, built in window pixels from the latest normalized cursor sample.
/// FABRICATED chrome, not an original asset; geometry never advances guest state.
fn pointer_quads(position: [f32; 2], size: tsfp_ui_renderer::Vec2) -> Vec<tsfp_ui_renderer::Quad> {
    use tsfp_ui_renderer::{Color, Quad, Rect, Vec2};
    if !position
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    {
        return Vec::new();
    }
    let origin = Vec2::new(position[0] * size.x, position[1] * size.y);
    let mut quads = Vec::new();
    for (row, width) in [
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 5.0, 3.0, 3.0, 3.0, 3.0,
    ]
    .into_iter()
    .enumerate()
    {
        let p = Vec2::new(origin.x, origin.y + row as f32);
        quads.push(Quad {
            rect_px: Rect::new(p, Vec2::new(width, 1.0)).unwrap(),
            color: Color([0.0, 0.0, 0.0, 1.0]),
        });
        if width > 2.0 {
            quads.push(Quad {
                rect_px: Rect::new(Vec2::new(p.x + 1.0, p.y), Vec2::new(width - 2.0, 1.0)).unwrap(),
                color: Color([1.0; 4]),
            });
        }
    }
    quads
}

/// The texture the render world adopts as the `GpuImage` of the picture handle (set by the main world, read by the render world).
pub struct Published {
    pub id: bevy::asset::AssetId<bevy::prelude::Image>,
    pub target: Target,
}

pub static PUBLISHED: Mutex<Option<Published>> = Mutex::new(None);

/// Surface policy: 0 = unprobed, 1 = Mailbox, 2 = Fifo, 3 = Immediate.
pub static PRESENT_MODE: AtomicU8 = AtomicU8::new(0);

/// One line for the exit statistics.
pub fn stats_line() -> Option<String> {
    gpu()?;
    let frames = FRAMES.load(Ordering::Relaxed);
    Some(format!(
        "tsfp-bevy: wgpu-share frames blitted {frames} (last guest present {}) at {}x{}, texture (re)allocations {}, size requests {}, per frame blit encode+submit {:.3} ms (no CPU readback), blit bindings {} cache hits {}",
        LAST_PRESENT.load(Ordering::Relaxed),
        LAST_W.load(Ordering::Relaxed),
        LAST_H.load(Ordering::Relaxed),
        RESIZES.load(Ordering::Relaxed),
        crate::wgpu_live::RESIZES.load(Ordering::Relaxed),
        BLIT_NS.load(Ordering::Relaxed) as f64 / frames.max(1) as f64 / 1e6,
        BLIT_BINDINGS.load(Ordering::Relaxed),
        BLIT_CACHE_HITS.load(Ordering::Relaxed),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Arc;

    fn named(items: &[(&str, wgpu::DeviceType)]) -> Vec<(String, wgpu::DeviceType)> {
        items
            .iter()
            .map(|(name, kind)| (name.to_string(), *kind))
            .collect()
    }

    #[test]
    fn native_pointer_samples_window_coordinates_without_guest_steps() {
        let size = tsfp_ui_renderer::Vec2::new(1920.0, 1080.0);
        let first = pointer_quads([0.25, 0.5], size);
        let moved = pointer_quads([0.75, 0.5], size);
        assert_eq!(first[0].rect_px.origin.x, 480.0);
        assert_eq!(moved[0].rect_px.origin.x, 1440.0);
        assert_eq!(first[0].rect_px.origin.y, moved[0].rect_px.origin.y);
        assert_eq!(first.len(), moved.len());
        assert!(pointer_quads([f32::NAN, 0.5], size).is_empty());
        assert!(pointer_quads([-0.1, 0.5], size).is_empty());
    }

    #[test]
    fn adapter_choice_prefers_the_discrete_gpu_and_honours_the_name() {
        use wgpu::DeviceType::*;
        let all = named(&[
            ("llvmpipe (LLVM 19)", Cpu),
            ("AMD Radeon Graphics (RADV GFX1201)", DiscreteGpu),
            ("Intel iGPU", IntegratedGpu),
        ]);
        assert_eq!(pick_adapter(&all, None), Some(1));
        assert_eq!(
            pick_adapter(&all, Some("intel")),
            Some(2),
            "name wins over the type, case blind"
        );
        assert_eq!(pick_adapter(&all, Some("LLVMPIPE")), Some(0));
        assert_eq!(
            pick_adapter(&all, Some("nvidia")),
            None,
            "a name that matches nothing is an error, not a silent other GPU"
        );
        assert_eq!(
            pick_adapter(&all, Some("")),
            Some(1),
            "an empty name means no preference"
        );
        assert_eq!(pick_adapter(&[], None), None);
        let two = named(&[("a", DiscreteGpu), ("b", DiscreteGpu)]);
        assert_eq!(pick_adapter(&two, None), Some(0), "first on a tie");
    }

    #[test]
    fn the_linked_runtime_reports_the_layout_of_this_process() {
        // One copy of wgpu in the process (T2246): the runtime's six words must equal ours exactly.
        let api = runtime_api().expect("the runtime is linked into the shell");
        let mut theirs = [0u64; 6];
        // SAFETY: six writable words.
        unsafe { (api.layout)(theirs.as_mut_ptr()) };
        assert_eq!(check_layout(&theirs, &layout_words()), Ok(()));
        // SAFETY: the ABI returns a code address and dereferences nothing.
        let runtime_copy = unsafe { (api.identity)() };
        assert_eq!(
            check_wgpu_identity(runtime_copy, wgpu::Device::features as *const () as usize),
            Ok(()),
            "one wgpu copy: the identity guard of T2267 accepts it"
        );
        assert!(gpu().is_none(), "nothing is shared before prepare()");
    }

    #[test]
    fn layout_check_names_the_first_difference() {
        let ours = layout_words();
        assert_eq!(check_layout(&ours, &ours), Ok(()));
        let mut theirs = ours;
        theirs[3] += 8;
        let error = check_layout(&theirs, &ours).unwrap_err();
        assert!(error.contains("size of Texture"), "{error}");
        assert!(
            layout_words().iter().all(|&word| word != 0),
            "sizes and the hash are non zero"
        );
    }

    #[test]
    fn duplicate_wgpu_copy_is_rejected_before_handle_handoff() {
        let shell = wgpu::Device::features as *const () as usize;
        assert!(check_wgpu_identity(shell, shell).is_ok());
        assert!(
            check_wgpu_identity(shell.wrapping_add(1), shell)
                .unwrap_err()
                .contains("separate wgpu copies")
        );
        assert!(check_wgpu_identity(0, shell).is_err());
    }

    /// Opens the best adapter, or skips (prints SKIP) on a machine without Vulkan unless `TSFP_REQUIRE_GPU=1`.
    fn gpu_or_skip() -> Option<SharedGpu> {
        match create_gpu() {
            Ok(gpu) => {
                eprintln!("test adapter: {}", describe(&gpu.info));
                Some(gpu)
            }
            Err(error) => {
                assert!(
                    std::env::var_os("TSFP_REQUIRE_GPU").is_none(),
                    "TSFP_REQUIRE_GPU set but no device: {error}"
                );
                eprintln!("SKIP: no Vulkan device: {error}");
                None
            }
        }
    }

    fn make_source_texture(gpu: &SharedGpu, width: u32, height: u32, rgba: &[u8]) -> wgpu::Texture {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test-published"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            texture.as_image_copy(),
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        texture
    }

    /// Read a `COPY_SRC` Rgba texture back as tightly packed rows (test only, this is the readback the shell avoids).
    fn read_back(gpu: &SharedGpu, target: &Target) -> Vec<u8> {
        let (width, height) = target.size;
        let padded = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test-readback"),
            size: u64::from(padded * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(Some(encoder.finish()));
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| result.expect("map"));
        gpu.device.poll(wgpu::Maintain::Wait);
        let mapped = slice.get_mapped_range();
        let mut out = Vec::new();
        for row in 0..height {
            let start = (row * padded) as usize;
            out.extend_from_slice(&mapped[start..start + (width * 4) as usize]);
        }
        out
    }

    #[test]
    fn blit_copies_colour_unchanged_and_forces_alpha_opaque() {
        let Some(gpu) = gpu_or_skip() else { return };
        let (width, height) = (8u32, 4u32);
        // Alpha 0 everywhere like the renderer output, colour from a gradient that includes values a gamma mistake would change.
        let mut pixels = Vec::new();
        for index in 0..width * height {
            pixels.extend_from_slice(&[
                (index * 8) as u8,
                (255 - index * 7) as u8,
                (index * 3 + 40) as u8,
                0,
            ]);
        }
        let source = make_source_texture(&gpu, width, height, &pixels);
        let target = Target::new(&gpu.device, width, height);
        Blitter::new(&gpu.device).blit(&gpu.device, &gpu.queue, &source, &target);
        let got = read_back(&gpu, &target);
        assert_eq!(got.len(), pixels.len());
        for (index, (got, want)) in got.chunks_exact(4).zip(pixels.chunks_exact(4)).enumerate() {
            assert_eq!(
                &got[..3],
                &want[..3],
                "pixel {index}: colour must pass through byte for byte"
            );
            assert_eq!(got[3], 255, "pixel {index}: alpha forced opaque");
        }
    }

    #[test]
    fn cached_blit_follows_texture_identity_contents_and_resize() {
        let Some(gpu) = gpu_or_skip() else { return };
        let mut blitter = Blitter::with_cache(&gpu.device, true);
        // More sources than the ring/cache, including same-sized distinct textures.
        for turn in 0..12u8 {
            let width = if turn < 6 { 8 } else { 16 };
            let pixels = [turn, 51, 103, 17].repeat((width * 8) as usize);
            let source = make_source_texture(&gpu, width, 8, &pixels);
            let target = Target::new(&gpu.device, width, 8);
            for _ in 0..2 {
                blitter.blit(&gpu.device, &gpu.queue, &source, &target);
            }
            let actual = read_back(&gpu, &target);
            assert!(actual.chunks_exact(4).all(|p| p == [turn, 51, 103, 255]));
            // Cached bindings must observe new texels, not hold an old snapshot.
            let changed = [turn + 1, 52, 104, 0].repeat((width * 8) as usize);
            gpu.queue.write_texture(
                source.as_image_copy(),
                &changed,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(8),
                },
                source.size(),
            );
            blitter.blit(&gpu.device, &gpu.queue, &source, &target);
            assert!(
                read_back(&gpu, &target)
                    .chunks_exact(4)
                    .all(|p| p == [turn + 1, 52, 104, 255])
            );
        }
    }

    /// Same device/workload A/B, with a GPU wait every eight submissions to bound queue depth.
    /// CPU submission percentiles and throughput including GPU completion are separate.
    #[test]
    #[ignore]
    fn bench_blit_cache_budget() {
        let Some(gpu) = gpu_or_skip() else { return };
        for (width, height) in [(960, 720), (1280, 960), (1920, 1440), (2880, 2160)] {
            let pixels = vec![64; (width * height * 4) as usize];
            let sources: Vec<_> = (0..3)
                .map(|_| make_source_texture(&gpu, width, height, &pixels))
                .collect();
            let target = Target::new(&gpu.device, width, height);
            // Repeat/reverse order so first-use compilation isn't mistaken for a cache gain.
            for cached in [false, true, true, false] {
                let mut blitter = Blitter::with_cache(&gpu.device, cached);
                for n in 0..24 {
                    blitter.blit(&gpu.device, &gpu.queue, &sources[n % 3], &target);
                }
                gpu.device.poll(wgpu::Maintain::Wait);
                let mut samples = Vec::new();
                let started = Instant::now();
                for n in 0..6000 {
                    let before = Instant::now();
                    blitter.blit(&gpu.device, &gpu.queue, &sources[n % 3], &target);
                    samples.push(before.elapsed().as_nanos() as u64);
                    if n % 8 == 7 {
                        gpu.device.poll(wgpu::Maintain::Wait);
                    }
                }
                gpu.device.poll(wgpu::Maintain::Wait);
                let total = started.elapsed().as_secs_f64();
                samples.sort_unstable();
                eprintln!(
                    "blit-budget {width}x{height} cached={cached} CPU p50={:.4} p99={:.4} max={:.4}ms completed={:.1}/s end_to_end={:.4}ms/frame",
                    samples[2999] as f64 / 1e6,
                    samples[5939] as f64 / 1e6,
                    samples[5999] as f64 / 1e6,
                    6000.0 / total,
                    total * 1e3 / 6000.0
                );
            }
        }
    }

    /// `cargo test --release -- --ignored --nocapture bench_blit_against_readback`: per frame cost on this machine's adapter of the
    /// old path (render target to buffer, map, wait, row unpad, force alpha) and the new one (blit, no wait). Not an assertion.
    #[test]
    #[ignore]
    fn bench_blit_against_readback() {
        let Some(gpu) = gpu_or_skip() else { return };
        let mut blitter = Blitter::new(&gpu.device);
        for (width, height) in [(1280u32, 960u32), (1920, 1440), (2880, 2160)] {
            let pixels = vec![0x40u8; (width * height * 4) as usize];
            let source = make_source_texture(&gpu, width, height, &pixels);
            let target = Target::new(&gpu.device, width, height);
            let frames = 60u32;
            // New path: encode + submit only (the GPU runs behind the CPU), one wait at the end to count the GPU time too.
            let started = Instant::now();
            for _ in 0..frames {
                blitter.blit(&gpu.device, &gpu.queue, &source, &target);
            }
            let submit = started.elapsed();
            gpu.device.poll(wgpu::Maintain::Wait);
            let total = started.elapsed();
            // Old path: readback with a wait per frame plus the alpha pass.
            let started = Instant::now();
            let mut sink = 0u64;
            for _ in 0..frames {
                let mut rgba = read_back(&gpu, &target);
                crate::wgpu_live::force_opaque(&mut rgba);
                sink += u64::from(rgba[0]);
            }
            let readback = started.elapsed();
            eprintln!(
                "bench {width}x{height}: blit CPU encode+submit {:.3} ms/frame, blit incl. GPU {:.3} ms/frame, readback+unpad+alpha {:.3} ms/frame (sink {sink})",
                submit.as_secs_f64() * 1e3 / f64::from(frames),
                total.as_secs_f64() * 1e3 / f64::from(frames),
                readback.as_secs_f64() * 1e3 / f64::from(frames),
            );
        }
    }

    /// A source that publishes whatever the test queued, the way the runtime ring does.
    struct Mock {
        ready: Arc<std::sync::atomic::AtomicBool>,
        frames: VecDeque<Acquired>,
        released: Arc<Mutex<Vec<u32>>>,
        sizes: Arc<Mutex<Vec<(u32, u32)>>>,
    }

    impl FrameSource for Mock {
        fn ready(&self) -> bool {
            self.ready.load(Ordering::Relaxed)
        }
        fn set_output(&mut self, width: u32, height: u32) {
            self.sizes.lock().unwrap().push((width, height));
        }
        fn acquire(&mut self) -> Option<Acquired> {
            self.frames.pop_front()
        }
        fn release(&mut self, slot: u32) {
            self.released.lock().unwrap().push(slot);
        }
    }

    #[test]
    fn display_updates_restore_clean_scene_without_new_guest_frames() {
        let Some(gpu) = gpu_or_skip() else { return };
        let released = Arc::new(Mutex::new(Vec::new()));
        let pixels = [50, 100, 150, 0].repeat(16);
        let frame = Acquired {
            texture: make_source_texture(&gpu, 4, 4, &pixels),
            width: 4,
            height: 4,
            present: 7,
            slot: 2,
        };
        let mock = Mock {
            ready: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            frames: VecDeque::from([frame]),
            released: released.clone(),
            sizes: Arc::new(Mutex::new(Vec::new())),
        };
        let mut share = Share::new(Box::new(mock), gpu.clone());
        let now = Instant::now();
        assert!(share.step_mode((4, 4), now, true).unwrap().new_target);
        let serial = share.target().unwrap().serial;
        let expected = [50, 100, 150, 255].repeat(16);
        for rate in [120u64, 144, 240] {
            // Simulate a prior composite with unrelated pixels. Every display redraw must restore
            // from the retained scene, including when no new producer frame can be acquired.
            let work = share.display_work.as_ref().unwrap();
            gpu.queue.write_texture(
                work.as_image_copy(),
                &[255u8; 64],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(16),
                    rows_per_image: Some(4),
                },
                work.size(),
            );
            let step = share
                .step_mode(
                    (4, 4),
                    now + std::time::Duration::from_secs_f64(1.0 / rate as f64),
                    true,
                )
                .unwrap();
            assert!(!step.new_target);
            assert_eq!(share.target().unwrap().serial, serial);
            assert_eq!(read_back(&gpu, share.target().unwrap()), expected);
        }
        assert_eq!(*released.lock().unwrap(), [2]);
        assert_eq!(share.display_present, 7);
        assert_eq!(share.display_frames, 4);
    }

    #[test]
    fn step_blits_releases_every_slot_and_reallocates_only_on_a_size_change() {
        let Some(gpu) = gpu_or_skip() else { return };
        let released = Arc::new(Mutex::new(Vec::new()));
        let sizes = Arc::new(Mutex::new(Vec::new()));
        let frame = |width: u32, height: u32, slot: u32, value: u8| {
            let pixels: Vec<u8> = (0..width * height)
                .flat_map(|_| [value, 10, 20, 0])
                .collect();
            Acquired {
                texture: make_source_texture(&gpu, width, height, &pixels),
                width,
                height,
                present: u64::from(slot),
                slot,
            }
        };
        let mock = Mock {
            ready: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            frames: VecDeque::from([
                frame(16, 8, 0, 100),
                frame(16, 8, 1, 200),
                frame(32, 16, 2, 50),
            ]),
            released: released.clone(),
            sizes: sizes.clone(),
        };
        let mut share = Share::new(Box::new(mock), gpu.clone());
        let now = Instant::now();
        let first = share.step((16, 8), now).expect("first frame");
        assert!(first.new_target);
        assert_eq!(
            read_back(&gpu, share.target().unwrap())[..4],
            [100, 10, 20, 255]
        );
        let second = share.step((16, 8), now).expect("second frame");
        assert!(!second.new_target, "same size reuses the texture");
        assert_eq!(
            read_back(&gpu, share.target().unwrap())[..4],
            [200, 10, 20, 255]
        );
        let third = share.step((16, 8), now).expect("third frame");
        assert!(
            third.new_target && third.size == (32, 16),
            "a bigger frame makes a new texture"
        );
        assert!(share.step((16, 8), now).is_none(), "no frame, no work");
        assert_eq!(
            *released.lock().unwrap(),
            [0, 1, 2],
            "every acquired slot is released after its blit"
        );
        assert_eq!(
            *sizes.lock().unwrap(),
            [(16, 8)],
            "the size is requested once, repeats are not"
        );
    }

    #[test]
    fn the_first_size_request_waits_for_the_runtime() {
        let Some(gpu) = gpu_or_skip() else { return };
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sizes = Arc::new(Mutex::new(Vec::new()));
        let mock = Mock {
            ready: ready.clone(),
            frames: VecDeque::new(),
            released: Arc::new(Mutex::new(Vec::new())),
            sizes: sizes.clone(),
        };
        let mut share = Share::new(Box::new(mock), gpu);
        let now = Instant::now();
        assert!(share.step((1280, 960), now).is_none());
        assert!(
            sizes.lock().unwrap().is_empty(),
            "nobody to ask yet, the request must not be spent"
        );
        ready.store(true, Ordering::Relaxed);
        assert!(share.step((1280, 960), now).is_none(), "no frame yet");
        assert_eq!(
            *sizes.lock().unwrap(),
            [(1280, 960)],
            "asked as soon as the runtime exists"
        );
    }
}
