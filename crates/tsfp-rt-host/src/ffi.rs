// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI of the render runtime (header: `src/host/tsfp_rt_host.h`). Every entry catches panics: unwinding across the C frames of
//! the host is undefined behaviour, so a panic switches the call into a no-op failure instead.
#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_int};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::{Runtime, Stats, parse_args};

/// Frames the renderer may be behind above which the live display stops drawing.
const DEFAULT_SKIP_BEHIND: u64 = 3;

/// The runtime the host created last and has not destroyed (T2136e): lets another part of the process (the Bevy shell) find it.
static SHARE_GPU: std::sync::Mutex<Option<tsfp_wgpu::context::GpuContext>> =
    std::sync::Mutex::new(None);

static CURRENT: AtomicPtr<Runtime> = AtomicPtr::new(std::ptr::null_mut());

/// The live runtime or null. The pointer stays valid until `tsfp_rt_host_destroy`; the shell only uses it while the game thread
/// is running, and the game thread destroys it when it exits.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rt_host_current() -> *mut Runtime {
    CURRENT.load(Ordering::Acquire)
}

/// Ask for a new output size (pixels) from the next present on. Needs a live runtime; returns 0, or -1 for null or zero sizes.
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create` and is not destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_set_output(
    runtime: *mut Runtime,
    width: u32,
    height: u32,
) -> c_int {
    if runtime.is_null() || width == 0 || height == 0 {
        return -1;
    }
    unsafe { &*runtime }.set_output_size(width, height);
    0
}

/// Mirror of `tsfp_rt_host_stats` in the C header.
#[repr(C)]
#[derive(Default)]
pub struct CStats {
    pub batches_sent: u64,
    pub batches_dropped: u64,
    pub stalls: u64,
    pub stall_ns_total: u64,
    pub stall_ns_max: u64,
    pub lines_rendered: u64,
    pub presents: u64,
    pub frames_rendered: u64,
    pub gave_up: u32,
    pub render_error: u32,
}

fn to_c(stats: Stats) -> CStats {
    CStats {
        batches_sent: stats.batches_sent,
        batches_dropped: stats.batches_dropped,
        stalls: stats.stalls,
        stall_ns_total: stats.stall_ns_total,
        stall_ns_max: stats.stall_ns_max,
        lines_rendered: stats.lines_rendered,
        presents: stats.presents,
        frames_rendered: stats.frames_rendered,
        gave_up: stats.gave_up as u32,
        render_error: stats.render_error as u32,
    }
}

/// `WxH` text to a size, None for anything else or a zero side.
pub fn parse_size(text: Option<&str>) -> Option<(u32, u32)> {
    let (w, h) = text?.trim().split_once('x')?;
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    (w > 0 && h > 0).then_some((w, h))
}

/// The live internal size from `TSFP_WGPU_SCALE`: `N` (clamped 1 to 8) times `logical`, or `WxH`; unset or bad means 1x.
pub fn live_output_from_env(text: Option<&str>, logical: (u32, u32)) -> (u32, u32) {
    if let Some(size) = parse_size(text) {
        return size;
    }
    let factor = text
        .and_then(|t| t.trim().parse::<u32>().ok())
        .map_or(1, |n| n.clamp(1, 8));
    (logical.0 * factor, logical.1 * factor)
}

/// Ask for a window (present target) size in pixels from the next present on: the internal image is mapped to it with the sharp
/// bilinear pass, aspect kept, black bars. Returns 0, or -1 for null or zero sizes.
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create` and is not destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_set_window(
    runtime: *mut Runtime,
    width: u32,
    height: u32,
) -> c_int {
    if runtime.is_null() || width == 0 || height == 0 {
        return -1;
    }
    unsafe { &*runtime }.set_window_size(width, height);
    0
}

/// Start the runtime. `args` is the live example's argument string (whitespace separated, the same text as `TSFP_WGPU_LIVE_ARGS`),
/// may be null. Returns null on bad arguments or when no adapter is usable (a message goes to stderr).
///
/// # Safety
/// `args` is null or a NUL terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_create(args: *const c_char) -> *mut Runtime {
    let text = if args.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(args) }
            .to_string_lossy()
            .into_owned()
    };
    let words: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    let result = catch_unwind(AssertUnwindSafe(|| {
        parse_args(&words).and_then(|mut options| {
            // Live display (no PNG plan): a renderer that is behind skips frames instead of stalling the guest. A dump plan keeps every
            // present drawn (conformance). `--no-skip` restores back-pressure only.
            if options.live.dump.base.is_none()
                && options.skip_behind.is_none()
                && !words.iter().any(|word| word == "--no-skip")
            {
                options.skip_behind = Some(DEFAULT_SKIP_BEHIND);
            }
            // The guest also omits draws (binary stream) while the renderer is that far behind: less to copy, queue and drop.
            if options.live.dump.base.is_none() && !words.iter().any(|word| word == "--drop-behind")
            {
                options.drop_behind = Some(DEFAULT_SKIP_BEHIND);
            }
            // Pipelines are compiled on worker threads (a first-use driver compile of 300 ms or more must not stall the frame).
            if options.live.dump.base.is_none()
                && !words
                    .iter()
                    .any(|word| word == "--async-pipelines" || word == "--sync-pipelines")
            {
                options.async_pipelines = 2;
            }
            // Live display without a PNG plan or a published frame: nothing reads pixels, so no CPU readback and no hash line.
            if options.live.dump.base.is_none() && !options.publish_frames {
                options.quiet = true;
            }
            // T2095: live internal resolution and window. Not touched when the arguments or a PNG plan already choose (conformance
            // stays at 1x). TSFP_WGPU_SCALE=N (1 to 8) or WxH sets the internal size, default 1x (640x480; higher resolution is opt-in, avoiding an unmeasured
            // raster cost increase). TSFP_WGPU_WINDOW=WxH adds the sharp bilinear present pass to that window size.
            if options.live.dump.base.is_none() && options.live.output.is_none() {
                options.live.output = Some(live_output_from_env(
                    std::env::var("TSFP_WGPU_SCALE").ok().as_deref(),
                    (options.live.width, options.live.height),
                ));
            }
            if options.live.dump.base.is_none() && options.live.window.is_none() {
                options.live.window = parse_size(std::env::var("TSFP_WGPU_WINDOW").ok().as_deref());
            }
            // T2226: opt-in host-boundary experiments; explicit --opt takes precedence.
            for (env, knob) in [
                ("TSFP_WGPU_BATCH_UPLOADS", "batch_uploads"),
                ("TSFP_WGPU_BATCH_TEXTURES", "batch_textures"),
                ("TSFP_WGPU_PREWARM_PIPELINES", "prewarm_pipelines"),
                ("TSFP_WGPU_PIPELINED_READBACK", "pipelined_readback"),
                ("TSFP_WGPU_PERSISTENT_PRESENT", "persistent_present"),
                ("TSFP_WGPU_VSH_MIN_VERTICES", "gpu_vsh_min_vertices"),
            ] {
                if !options.live.knobs.iter().any(|(name, _)| name == knob) {
                    if let Ok(value) = std::env::var(env) {
                        if value != "0" || knob == "gpu_vsh_min_vertices" {
                            options.live.knobs.push((knob.to_string(), value));
                        }
                    }
                }
            }
            if options.stats_every.is_none() && std::env::var_os("TSFP_RT_HOST_STATS").is_some() {
                options.stats_every = Some(std::time::Duration::from_secs(5));
            }
            Runtime::start_with_gpu(options, SHARE_GPU.lock().unwrap().take())
        })
    }));
    match result {
        Ok(Ok(runtime)) => {
            // T2136d: say on stderr which adapter renders (the owner could not tell a GPU from llvmpipe)
            eprintln!("wgpu-live: rt-host adapter {}", runtime.adapter());
            let pointer = Box::into_raw(Box::new(runtime));
            CURRENT.store(pointer, Ordering::Release);
            pointer
        }
        Ok(Err(error)) => {
            eprintln!("rt-host: {error}");
            std::ptr::null_mut()
        }
        Err(_) => {
            eprintln!("rt-host: panic in create");
            std::ptr::null_mut()
        }
    }
}

/// Append stream bytes (any chunking, lines are re-assembled). Returns `len`.
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create` and is not destroyed; `bytes` points at `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_write(
    runtime: *mut Runtime,
    bytes: *const u8,
    len: usize,
) -> isize {
    if runtime.is_null() || bytes.is_null() {
        return -1;
    }
    let (runtime, slice) = unsafe { (&*runtime, std::slice::from_raw_parts(bytes, len)) };
    match catch_unwind(AssertUnwindSafe(|| runtime.write(slice))) {
        Ok(()) => len as isize,
        Err(_) => -1,
    }
}

/// Append one text line (newline added).
///
/// # Safety
/// As `tsfp_rt_host_write`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_send_line(
    runtime: *mut Runtime,
    text: *const u8,
    len: usize,
) -> c_int {
    if runtime.is_null() || text.is_null() {
        return -1;
    }
    let (runtime, slice) = unsafe { (&*runtime, std::slice::from_raw_parts(text, len)) };
    let line = String::from_utf8_lossy(slice).into_owned();
    catch_unwind(AssertUnwindSafe(|| runtime.send_line(&line))).map_or(-1, |()| 0)
}

/// Append binary trace records (`tsfp_engine_render::trace_wire`, whole records only). `presents` of them are frame ends: a frame
/// end hands everything queued over to the render thread at once. Text lines written before stay ahead of the records.
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create` and is not destroyed; `bytes` points at `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_send_binary(
    runtime: *mut Runtime,
    bytes: *const u8,
    len: usize,
    presents: u32,
) -> c_int {
    if runtime.is_null() || bytes.is_null() {
        return -1;
    }
    let (runtime, slice) = unsafe { (&*runtime, std::slice::from_raw_parts(bytes, len)) };
    catch_unwind(AssertUnwindSafe(|| {
        runtime.write_binary(slice, u64::from(presents))
    }))
    .map_or(-1, |()| 0)
}

/// The host presented frame `number`: queue the `# present N` / `present` lines and hand everything over.
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create` and is not destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_present(runtime: *mut Runtime, number: u64) -> c_int {
    if runtime.is_null() {
        return -1;
    }
    let runtime = unsafe { &*runtime };
    catch_unwind(AssertUnwindSafe(|| runtime.present(number))).map_or(-1, |()| 0)
}

/// Hand every complete queued line to the render thread.
///
/// # Safety
/// As `tsfp_rt_host_present`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_flush(runtime: *mut Runtime) -> c_int {
    if runtime.is_null() {
        return -1;
    }
    let runtime = unsafe { &*runtime };
    catch_unwind(AssertUnwindSafe(|| runtime.flush())).map_or(-1, |()| 0)
}

/// Take the newest rendered frame (RGBA8, rows top to bottom, no padding; needs `--publish`). Returns the byte count copied, 0 when
/// there is no new frame, or `-needed` when `capacity` is too small (the frame stays queued). `width`, `height` and `present` are
/// always filled when a frame exists.
///
/// # Safety
/// `runtime` as above, `width`, `height`, `present` point at one value each, `dst` at `capacity` writable bytes (may be null to ask the size).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_frame(
    runtime: *mut Runtime,
    width: *mut u32,
    height: *mut u32,
    present: *mut u64,
    dst: *mut u8,
    capacity: usize,
) -> isize {
    if runtime.is_null() || width.is_null() || height.is_null() || present.is_null() {
        return -1;
    }
    let runtime = unsafe { &*runtime };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (w, h) = runtime.frame_size()?;
        let needed = w as usize * h as usize * 4;
        if dst.is_null() || capacity < needed {
            unsafe {
                width.write(w);
                height.write(h);
            }
            return Some(-(needed as isize));
        }
        let frame = runtime.take_frame()?;
        let bytes = frame.rgba.len();
        if bytes > capacity {
            return Some(-(bytes as isize));
        }
        unsafe {
            width.write(frame.width);
            height.write(frame.height);
            present.write(frame.present);
            std::ptr::copy_nonoverlapping(frame.rgba.as_ptr(), dst, bytes);
        }
        Some(bytes as isize)
    }));
    match result {
        Ok(Some(count)) => count,
        Ok(None) => 0,
        Err(_) => -1,
    }
}

/// 1 while the live renderer runs, 0 once it was switched off (queue memory cap, stall limit, render thread gone).
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create` and is not destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_alive(runtime: *mut Runtime) -> c_int {
    if runtime.is_null() {
        return 0;
    }
    let runtime = unsafe { &*runtime };
    catch_unwind(AssertUnwindSafe(|| c_int::from(runtime.alive()))).unwrap_or(0)
}

/// 1 while the renderer is behind and the guest should omit the draws of the next frames (T2136d), else 0.
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create` and is not destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_pressure(runtime: *mut Runtime) -> c_int {
    if runtime.is_null() {
        return 0;
    }
    let runtime = unsafe { &*runtime };
    catch_unwind(AssertUnwindSafe(|| c_int::from(runtime.pressure()))).unwrap_or(0)
}

/// T2292: completed frames handed over and not yet replayed by the render thread, 0 for a null runtime.
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create` and is not destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_frames_behind(runtime: *mut Runtime) -> u64 {
    if runtime.is_null() {
        return 0;
    }
    let runtime = unsafe { &*runtime };
    catch_unwind(AssertUnwindSafe(|| runtime.frames_behind())).unwrap_or(0)
}

/// Copy the counters out.
///
/// # Safety
/// `runtime` as above, `out` points at one `CStats`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_stats(runtime: *mut Runtime, out: *mut CStats) -> c_int {
    if runtime.is_null() || out.is_null() {
        return -1;
    }
    let runtime = unsafe { &*runtime };
    match catch_unwind(AssertUnwindSafe(|| runtime.stats())) {
        Ok(stats) => {
            unsafe { out.write(to_c(stats)) };
            0
        }
        Err(_) => -1,
    }
}

/// Flush, wait for the render thread to finish (PNG files written), print one summary line to stderr and free the runtime.
///
/// # Safety
/// `runtime` came from `tsfp_rt_host_create`; it must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_destroy(runtime: *mut Runtime) {
    if runtime.is_null() {
        return;
    }
    let _ = CURRENT.compare_exchange(
        runtime,
        std::ptr::null_mut(),
        Ordering::AcqRel,
        Ordering::Relaxed,
    );
    let runtime = unsafe { Box::from_raw(runtime) };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let s = runtime.finish();
        eprintln!(
            "rt-host: done: {} lines, {} batches sent, {} dropped; {}",
            s.lines_rendered,
            s.batches_sent,
            s.batches_dropped,
            s.summary()
        );
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_env_forms() {
        assert_eq!(live_output_from_env(None, (640, 480)), (640, 480));
        assert_eq!(live_output_from_env(Some("1"), (640, 480)), (640, 480));
        assert_eq!(live_output_from_env(Some("3"), (640, 480)), (1920, 1440));
        assert_eq!(live_output_from_env(Some("99"), (640, 480)), (5120, 3840));
        assert_eq!(
            live_output_from_env(Some("1920x1080"), (640, 480)),
            (1920, 1080)
        );
        assert_eq!(live_output_from_env(Some("junk"), (640, 480)), (640, 480));
        assert_eq!(parse_size(Some("0x5")), None);
    }
}

/// Layout guard for the shell's pinned wgpu handles.
/// # Safety
/// `out` points to six writable u64 words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_share_layout(out: *mut u64) {
    if out.is_null() {
        return;
    }
    let hash = b"24.0.5"
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
    let words = [
        hash,
        size_of::<wgpu::Device>() as u64,
        size_of::<wgpu::Queue>() as u64,
        size_of::<wgpu::Texture>() as u64,
        align_of::<wgpu::Device>() as u64,
        1,
    ];
    unsafe {
        std::ptr::copy_nonoverlapping(words.as_ptr(), out, 6);
    }
}

/// Address of code owned by this runtime's wgpu copy. A shell using the same
/// statically linked wgpu crate sees this exact address; a separate cdylib
/// has its own copy and must use the CPU frame handoff instead.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rt_host_wgpu_identity() -> usize {
    wgpu::Device::features as *const () as usize
}

/// Clone the shell device before create. Calls to create/share/destroy must be serialized by the caller.
/// # Safety
/// Handles have the matching layout and originate from the same wgpu/toolchain; name is null or NUL terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_share_gpu(
    device: *const wgpu::Device,
    queue: *const wgpu::Queue,
    name: *const c_char,
) -> c_int {
    if device.is_null() || queue.is_null() || !CURRENT.load(Ordering::Acquire).is_null() {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let name = if name.is_null() {
            "shared".into()
        } else {
            unsafe { CStr::from_ptr(name) }
                .to_string_lossy()
                .into_owned()
        };
        let info = tsfp_wgpu::context::AdapterReport {
            name,
            backend: "shared".into(),
            device_type: "shared".into(),
            driver: String::new(),
        };
        *SHARE_GPU.lock().unwrap() = Some(tsfp_wgpu::context::GpuContext::from_parts(
            unsafe { &*device }.clone(),
            unsafe { &*queue }.clone(),
            info,
        ));
        0
    }))
    .unwrap_or(-1)
}

#[repr(C)]
pub struct TsfpSharedFrame {
    pub texture: *const wgpu::Texture,
    pub width: u32,
    pub height: u32,
    pub present: u64,
    pub slot: u32,
}

/// # Safety
/// Runtime is alive; out is writable. Returned texture remains valid until release, which follows consumer queue submission.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_shared_acquire(
    runtime: *mut Runtime,
    out: *mut TsfpSharedFrame,
) -> c_int {
    if runtime.is_null() || out.is_null() {
        return -1;
    }
    catch_unwind(AssertUnwindSafe(|| {
        let frames = unsafe { &*runtime }.shared.gpu_frames.lock().unwrap();
        let Some(ring) = frames.as_ref() else {
            return -1;
        };
        let mut ring = ring.lock().unwrap();
        let Some((texture, present, slot)) = ring.acquire() else {
            return 0;
        };
        unsafe {
            out.write(TsfpSharedFrame {
                texture,
                width: texture.width(),
                height: texture.height(),
                present,
                slot,
            });
        }
        1
    }))
    .unwrap_or(-1)
}

/// # Safety
/// Runtime remains alive and slot was acquired from it; release once, after submitting consumer work.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rt_host_shared_release(runtime: *mut Runtime, slot: u32) {
    if runtime.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(ring) = unsafe { &*runtime }
            .shared
            .gpu_frames
            .lock()
            .unwrap()
            .as_ref()
        {
            ring.lock().unwrap().release(slot);
        }
    }));
}
