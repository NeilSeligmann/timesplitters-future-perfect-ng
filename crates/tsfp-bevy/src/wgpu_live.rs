// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136e: the opt-in `--wgpu-live` presentation path (docs/t-wgpu-default-presentation.md, option b). The picture comes from the
//! wgpu live renderer of `tsfp-rt-host` (the render thread the C host feeds with `TSFP_WGPU_LIVE=1`) at the size of the window,
//! instead of the C Vulkan renderer's fixed 640x480 frame. The shell asks the runtime for the picture size (`set_output`), takes
//! the newest RGBA8 frame (`frame`, one copy into a reusable buffer), forces alpha opaque (the renderer writes 0 everywhere)
//! and hands the buffer to the Bevy `Image`. CPU hand-off: no device sharing. Default OFF.
//!
//! The runtime is linked as an rlib (T2246), so this evidence path binds its C ABI directly, just like GPU sharing.
use std::ffi::{c_int, c_void};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Largest side of a requested output (the wgpu context raises the texture limit to 8192; a 4K picture is the practical cap).
pub const MAX_SIDE: u32 = 3840;
/// Seconds a changed window size must stay unchanged before the renderer is resized (a drag resize is one reallocation).
const SETTLE: Duration = Duration::from_millis(150);

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// What `--wgpu-live` / `--wgpu-live-solo` asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub on: bool,
    /// Also drop the host's C Vulkan renderer options (`--gpu-live` and the options that need it).
    pub solo: bool,
    /// `--wgpu-share` (T2229): share the wgpu device with Bevy and show the frame as a GPU texture. Implies `on`; falls back to the
    /// CPU hand-off with a stated reason when sharing is not possible.
    pub share: bool,
}

/// Take `--wgpu-live` (also `--wgpu-live=cpu`, the explicit CPU hand-off), `--wgpu-live-solo` and `--wgpu-share` out of the
/// arguments (the shell's, not the C host's).
pub fn extract(arguments: &mut Vec<String>) -> Request {
    let has = |arguments: &[String], name: &str| arguments.iter().any(|argument| argument == name);
    let solo = has(arguments, "--wgpu-live-solo");
    // Explicit CPU evidence capture overrides the launcher's shared GPU default.
    let share = has(arguments, "--wgpu-share") && !has(arguments, "--wgpu-live=cpu");
    let on = solo || share || has(arguments, "--wgpu-live") || has(arguments, "--wgpu-live=cpu");
    arguments.retain(|argument| {
        !matches!(
            argument.as_str(),
            "--wgpu-live" | "--wgpu-live=cpu" | "--wgpu-live-solo" | "--wgpu-share"
        )
    });
    Request { on, solo, share }
}

/// Arguments of the C host that would run a second renderer next to the wgpu one (the C Vulkan live renderer composes the frame
/// that this path does not show). Removed in wgpu mode and reported by the caller.
pub fn strip_c_renderer(arguments: &mut Vec<String>) -> Vec<String> {
    let mut removed = Vec::new();
    arguments.retain(|argument| {
        let drop = matches!(argument.as_str(), "--gpu-live" | "--gpu-live-inferred");
        if drop {
            removed.push(argument.clone());
        }
        !drop
    });
    removed
}

/// Output size for a window of `physical` pixels showing a picture of `aspect`: the largest fitting rectangle, even sides,
/// at most [`MAX_SIDE`], at least 640x480 / 2 so a tiny window does not make a tiny render. Pure, unit tested.
pub fn output_size(physical: (u32, u32), aspect: f32) -> (u32, u32) {
    let (fit_w, fit_h) = crate::present::fit((physical.0 as f32, physical.1 as f32), aspect);
    let even = |value: f32| ((value.round() as u32).max(2) & !1).max(2);
    let (mut width, mut height) = (even(fit_w), even(fit_h));
    if width > MAX_SIDE || height > MAX_SIDE {
        let shrink = (MAX_SIDE as f32 / width as f32).min(MAX_SIDE as f32 / height as f32);
        width = even(width as f32 * shrink);
        height = even(height as f32 * shrink);
    }
    (width.max(320), height.max(240))
}

type CurrentFn = unsafe extern "C" fn() -> *mut c_void;
type SetOutputFn = unsafe extern "C" fn(*mut c_void, u32, u32) -> c_int;
type FrameFn =
    unsafe extern "C" fn(*mut c_void, *mut u32, *mut u32, *mut u64, *mut u8, usize) -> isize;

#[derive(Clone, Copy)]
struct Api {
    current: CurrentFn,
    set_output: SetOutputFn,
    frame: FrameFn,
}

static API: OnceLock<Option<Api>> = OnceLock::new();

fn api() -> Option<Api> {
    *API.get_or_init(|| {
        use tsfp_rt_host::ffi;
        // T2229: the runtime is an rlib in this process. Its symbols need not be
        // in the dynamic export table, so dlsym can incorrectly report no runtime.
        // SAFETY: these are the documented C ABI functions; only opaque Runtime
        // pointer types differ from the shell's c_void aliases.
        unsafe {
            Some(Api {
                current: std::mem::transmute::<*const (), CurrentFn>(
                    ffi::tsfp_rt_host_current as *const (),
                ),
                set_output: std::mem::transmute::<*const (), SetOutputFn>(
                    ffi::tsfp_rt_host_set_output as *const (),
                ),
                frame: std::mem::transmute::<*const (), FrameFn>(
                    ffi::tsfp_rt_host_frame as *const (),
                ),
            })
        }
    })
}

/// Counters for the stats line (nanoseconds and counts).
pub static FRAMES: AtomicU64 = AtomicU64::new(0);
pub static COPY_NS: AtomicU64 = AtomicU64::new(0);
pub static ALPHA_NS: AtomicU64 = AtomicU64::new(0);
pub static NONBLACK: AtomicU64 = AtomicU64::new(0);
pub static RESIZES: AtomicU64 = AtomicU64::new(0);
pub static LAST_W: AtomicU64 = AtomicU64::new(0);
pub static LAST_H: AtomicU64 = AtomicU64::new(0);

/// One frame taken from the runtime.
pub struct Taken {
    pub width: u32,
    pub height: u32,
}

/// Per-app state: which size was requested and the settle timer.
pub struct Pump {
    requested: (u32, u32),
    candidate: (u32, u32),
    candidate_since: Instant,
    warned: bool,
}

impl Default for Pump {
    fn default() -> Self {
        Self {
            requested: (0, 0),
            candidate: (0, 0),
            candidate_since: Instant::now(),
            warned: false,
        }
    }
}

/// Make every alpha byte 255. Frame alpha is 0 everywhere in the renderer output (MEASURED, docs/t-wgpu-default-presentation.md).
/// Sampled check (4096 evenly spaced pixels) that a frame has any colour: a diagnostic for a renderer that hands over black.
pub fn has_colour(rgba: &[u8]) -> bool {
    let pixels = rgba.len() / 4;
    let step = (pixels / 4096).max(1);
    (0..pixels)
        .step_by(step)
        .any(|at| rgba[at * 4..at * 4 + 3].iter().any(|&byte| byte != 0))
}

pub fn force_opaque(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
}

impl Pump {
    /// Decide the output size for the current window, with the settle delay. Returns the size to request now, if any.
    pub fn wanted(&mut self, desired: (u32, u32), now: Instant) -> Option<(u32, u32)> {
        if desired == self.requested {
            self.candidate = desired;
            return None;
        }
        if self.requested == (0, 0) {
            self.requested = desired;
            self.candidate = desired;
            return Some(desired);
        }
        if desired != self.candidate {
            self.candidate = desired;
            self.candidate_since = now;
            return None;
        }
        if now.duration_since(self.candidate_since) >= SETTLE {
            self.requested = desired;
            return Some(desired);
        }
        None
    }

    /// One Update: request the window sized output and take the newest frame into `buffer` (resized to the frame). `None` when
    /// there is no runtime yet or no new frame.
    pub fn poll(&mut self, desired: (u32, u32), buffer: &mut Vec<u8>) -> Option<Taken> {
        let Some(api) = api() else {
            if !self.warned {
                self.warned = true;
                eprintln!(
                    "tsfp-bevy: --wgpu-live: the host has no render runtime (build with -DTSFP_RUST_RT_HOST=ON)"
                );
            }
            return None;
        };
        // SAFETY: the runtime pointer is read fresh each call and stays valid until the game thread's exit (the C host destroys it
        // from an atexit handler, after this app has ended).
        let runtime = unsafe { (api.current)() };
        if runtime.is_null() || crate::shared::SHARED.game_done.load(Ordering::Acquire) {
            return None;
        }
        if let Some((width, height)) = self.wanted(desired, Instant::now()) {
            // SAFETY: as above.
            unsafe { (api.set_output)(runtime, width, height) };
            RESIZES.fetch_add(1, Ordering::Relaxed);
        }
        let (mut width, mut height, mut present) = (0u32, 0u32, 0u64);
        // SAFETY: null destination asks for the size of the queued frame (negative needed bytes), nothing is written to it.
        let needed = unsafe {
            (api.frame)(
                runtime,
                &mut width,
                &mut height,
                &mut present,
                std::ptr::null_mut(),
                0,
            )
        };
        if needed >= 0 {
            return None;
        }
        let needed = needed.unsigned_abs();
        if buffer.len() != needed {
            buffer.resize(needed, 0);
        }
        let started = Instant::now();
        // SAFETY: `buffer` has `needed` writable bytes.
        let copied = unsafe {
            (api.frame)(
                runtime,
                &mut width,
                &mut height,
                &mut present,
                buffer.as_mut_ptr(),
                buffer.len(),
            )
        };
        if copied <= 0 {
            return None;
        }
        let copied_at = Instant::now();
        force_opaque(buffer);
        COPY_NS.fetch_add((copied_at - started).as_nanos() as u64, Ordering::Relaxed);
        ALPHA_NS.fetch_add(copied_at.elapsed().as_nanos() as u64, Ordering::Relaxed);
        FRAMES.fetch_add(1, Ordering::Relaxed);
        if has_colour(buffer) {
            NONBLACK.fetch_add(1, Ordering::Relaxed);
        }
        LAST_W.store(u64::from(width), Ordering::Relaxed);
        LAST_H.store(u64::from(height), Ordering::Relaxed);
        crate::fps::game_frame(present);
        Some(Taken { width, height })
    }
}

/// One line for the exit statistics.
pub fn stats_line() -> Option<String> {
    let frames = FRAMES.load(Ordering::Relaxed);
    if !enabled() {
        return None;
    }
    let per = |ns: &AtomicU64| ns.load(Ordering::Relaxed) as f64 / frames.max(1) as f64 / 1e6;
    Some(format!(
        "tsfp-bevy: wgpu-live frames taken {frames} ({} with colour) at {}x{}, resize requests {}, per frame copy {:.2} ms, force opaque {:.2} ms",
        NONBLACK.load(Ordering::Relaxed),
        LAST_W.load(Ordering::Relaxed),
        LAST_H.load(Ordering::Relaxed),
        RESIZES.load(Ordering::Relaxed),
        per(&COPY_NS),
        per(&ALPHA_NS),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_size_fits_the_picture_in_the_window_with_even_sides() {
        assert_eq!(output_size((1280, 960), 4.0 / 3.0), (1280, 960));
        assert_eq!(output_size((1920, 1080), 4.0 / 3.0), (1440, 1080));
        assert_eq!(output_size((1920, 1080), 16.0 / 9.0), (1920, 1080));
        let (w, h) = output_size((7681, 4321), 16.0 / 9.0);
        assert!(w <= MAX_SIDE && h <= MAX_SIDE && w % 2 == 0 && h % 2 == 0);
        assert_eq!(output_size((10, 10), 4.0 / 3.0), (320, 240));
    }

    #[test]
    fn extract_and_strip() {
        let mut arguments: Vec<String> = ["a.xbe", "--wgpu-live", "--gpu-live", "--interactive"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            extract(&mut arguments),
            Request {
                on: true,
                solo: false,
                share: false
            }
        );
        assert_eq!(
            extract(&mut vec!["--wgpu-live-solo".to_string()]),
            Request {
                on: true,
                solo: true,
                share: false
            }
        );
        assert_eq!(
            extract(&mut vec!["--wgpu-share".to_string()]),
            Request {
                on: true,
                solo: false,
                share: true
            }
        );
        let mut cpu = vec!["--wgpu-live=cpu".to_string(), "a.xbe".to_string()];
        assert_eq!(
            extract(&mut cpu),
            Request {
                on: true,
                solo: false,
                share: false
            }
        );
        assert_eq!(cpu, ["a.xbe"], "the shell options never reach the C host");
        for flags in [
            ["--wgpu-share", "--wgpu-live=cpu"],
            ["--wgpu-live=cpu", "--wgpu-share"],
        ] {
            let mut arguments = flags.map(str::to_string).to_vec();
            assert_eq!(
                extract(&mut arguments),
                Request {
                    on: true,
                    solo: false,
                    share: false
                }
            );
            assert!(arguments.is_empty());
        }
        assert_eq!(extract(&mut arguments), Request::default());
        assert_eq!(strip_c_renderer(&mut arguments), ["--gpu-live"]);
        assert_eq!(arguments, ["a.xbe", "--interactive"]);
    }

    #[test]
    fn resize_requests_wait_for_the_window_to_settle() {
        let mut pump = Pump::default();
        let start = Instant::now();
        assert_eq!(
            pump.wanted((1280, 960), start),
            Some((1280, 960)),
            "first size at once"
        );
        assert_eq!(pump.wanted((1280, 960), start), None);
        assert_eq!(
            pump.wanted((1000, 750), start),
            None,
            "a change starts the timer"
        );
        assert_eq!(
            pump.wanted((1002, 750), start + Duration::from_millis(100)),
            None,
            "still dragging"
        );
        assert_eq!(
            pump.wanted((1002, 750), start + Duration::from_millis(200)),
            None,
            "timer restarted by the drag"
        );
        assert_eq!(
            pump.wanted((1002, 750), start + Duration::from_millis(400)),
            Some((1002, 750))
        );
        assert_eq!(
            pump.wanted((1002, 750), start + Duration::from_millis(500)),
            None
        );
    }

    #[test]
    fn force_opaque_sets_alpha_only() {
        let mut pixels = [1, 2, 3, 0, 4, 5, 6, 9];
        force_opaque(&mut pixels);
        assert_eq!(pixels, [1, 2, 3, 255, 4, 5, 6, 255]);
    }
}
