// SPDX-License-Identifier: GPL-3.0-or-later
//! T2246: a windowless consumer of the T2229 shared frame ring, for measurement on a machine that cannot open a window.
//! `--wgpu-share-bench=SECONDS:WIDTHxHEIGHT` runs the game thread as usual, but instead of the Bevy app a loop at 60 Hz calls
//! the same `Share::step` the render world uses (request the output size, acquire the newest published frame, blit it into the
//! shell texture, release the slot), then asks the guest to quit after SECONDS. Prints the consumer counters on exit.
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::{gpu_share, shared, wi, window_input};

/// What `--wgpu-share-bench` asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bench {
    pub seconds: u64,
    pub size: (u32, u32),
}

/// Parse `SECONDS:WIDTHxHEIGHT`. Pure, unit tested.
pub fn parse(text: &str) -> Result<Bench, String> {
    let bad = || format!("--wgpu-share-bench=SECONDS:WIDTHxHEIGHT expected, got {text:?}");
    let (seconds, size) = text.split_once(':').ok_or_else(bad)?;
    let (width, height) = size.split_once('x').ok_or_else(bad)?;
    let number = |value: &str| value.parse::<u32>().ok().filter(|&n| n > 0);
    Ok(Bench {
        seconds: seconds.parse().ok().filter(|&n| n > 0).ok_or_else(bad)?,
        size: (
            number(width).ok_or_else(bad)?,
            number(height).ok_or_else(bad)?,
        ),
    })
}

/// Take `--wgpu-share-bench=...` out of the arguments.
pub fn extract(arguments: &mut Vec<String>) -> Result<Option<Bench>, String> {
    let Some(at) = arguments
        .iter()
        .position(|argument| argument.starts_with("--wgpu-share-bench="))
    else {
        return Ok(None);
    };
    let text = arguments.remove(at);
    parse(&text["--wgpu-share-bench=".len()..]).map(Some)
}

// Stall diagnosis: when no frame arrives for a while, every thread prints its kernel state and a backtrace (signal handler per thread).
unsafe extern "C" {
    fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
    fn syscall(number: i64, ...) -> i64;
    fn getpid() -> i32;
}

static TAKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static TICKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const SIGUSR2: i32 = 12;
const SYS_TGKILL: i64 = 234;

extern "C" fn dump_thread(_: i32) {
    // Capture first, print after: printing under the stderr lock while capturing can deadlock a stalled thread.
    let name = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_string();
    let trace = std::backtrace::Backtrace::force_capture();
    eprintln!("tsfp-bevy: stall backtrace of thread {name}:\n{trace}");
}

/// Print state, wait channel and syscall of every thread of this process, then the backtrace of the render thread and the main
/// thread (signal handler per thread). Diagnostic only, async-signal-unsafe by design.
fn dump_all_threads() {
    // SAFETY: installs a handler that only captures and prints.
    unsafe { signal(SIGUSR2, dump_thread) };
    let Ok(entries) = std::fs::read_dir("/proc/self/task") else {
        return;
    };
    let mut wanted = Vec::new();
    for entry in entries.flatten() {
        let tid = entry
            .file_name()
            .to_string_lossy()
            .parse::<i64>()
            .unwrap_or(0);
        let read = |file: &str| {
            std::fs::read_to_string(entry.path().join(file))
                .unwrap_or_default()
                .trim()
                .to_string()
        };
        let stat = read("stat");
        let state = stat
            .rsplit_once(')')
            .map_or("?", |(_, tail)| {
                tail.trim().split(' ').next().unwrap_or("?")
            })
            .to_string();
        let comm = read("comm");
        eprintln!(
            "tsfp-bevy: stall thread {tid} comm={comm} state={state} wchan={} syscall={}",
            read("wchan"),
            read("syscall")
        );
        if comm.contains("render") || comm == "tsfp-bevy" {
            wanted.push(tid);
        }
    }
    for tid in wanted {
        // SAFETY: tgkill of our own thread.
        unsafe { syscall(SYS_TGKILL, i64::from(getpid()), tid, i64::from(SIGUSR2)) };
        std::thread::sleep(Duration::from_millis(1500));
    }
}

/// Diagnostic consumer cadence; invalid values preserve the original 60 Hz benchmark.
fn consumer_hz(value: Option<&str>) -> u32 {
    value
        .and_then(|v| v.parse().ok())
        .filter(|hz| (30..=480).contains(hz))
        .unwrap_or(60)
}

/// Consume the ring at 60 Hz for the requested time, then stop the guest.
pub fn run(bench: Bench) {
    let Some(gpu) = gpu_share::gpu() else {
        eprintln!(
            "tsfp-bevy: --wgpu-share-bench: the GPU texture path is not active, nothing to measure (see the path= line above)"
        );
        wi::request_quit(window_input::RawEvent::Quit);
        return;
    };
    let mut share = gpu_share::Share::new(Box::new(gpu_share::RuntimeSource), gpu.clone());
    let started = Instant::now();
    let deadline = started + Duration::from_secs(bench.seconds);
    let hz = consumer_hz(std::env::var("TSFP_UI_DISPLAY_HZ").ok().as_deref());
    let tick = Duration::from_secs_f64(1.0 / f64::from(hz));
    let (mut ticks, mut frames, mut resizes) = (0u64, 0u64, 0u64);
    let mut step_ns: Vec<u64> = Vec::new();
    let mut next = started;
    // Watchdog thread: the consumer loop below beats TICKS; when it stops (a hang in the shared device path), dump every thread once.
    std::thread::Builder::new()
        .name("share-bench-watchdog".into())
        .spawn(|| {
            let (mut seen, mut since) = (u64::MAX, Instant::now());
            loop {
                std::thread::sleep(Duration::from_millis(500));
                // The frame count freezing while the consumer keeps ticking means the renderer stopped publishing.
                let now = TAKEN.load(Ordering::Relaxed);
                if now != seen {
                    (seen, since) = (now, Instant::now());
                } else if now > 0 && since.elapsed() > Duration::from_secs(3) {
                    eprintln!("tsfp-bevy: wgpu-share-bench: no new frame for 3 s after {now} frames (consumer tick {}), dumping threads", TICKS.load(Ordering::Relaxed));
                    dump_all_threads();
                    return;
                }
            }
        })
        .expect("spawn the watchdog");
    while Instant::now() < deadline && !shared::SHARED.game_done.load(Ordering::Acquire) {
        let before = Instant::now();
        if let Some(stepped) = share.step(bench.size, before) {
            frames += 1;
            TAKEN.store(gpu_share::FRAMES.load(Ordering::Relaxed), Ordering::Relaxed);
            resizes += u64::from(stepped.new_target);
            step_ns.push(before.elapsed().as_nanos() as u64);
        }
        gpu.device.poll(wgpu::Maintain::Poll);
        ticks += 1;
        TICKS.store(ticks, Ordering::Relaxed);
        next += tick;
        if let Some(wait) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        } else {
            next = Instant::now();
        }
    }
    step_ns.sort_unstable();
    let at = |fraction: f64| {
        step_ns
            .get(((step_ns.len() as f64 * fraction) as usize).min(step_ns.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0) as f64
            / 1e6
    };
    eprintln!(
        "tsfp-bevy: wgpu-share-bench {}x{}: {:.1} s, consumer ticks {ticks} ({hz} Hz), composed updates {frames} ({:.1}/s), new textures {resizes}, \
         consumer step p50 {:.3} / p99 {:.3} / max {:.3} ms",
        bench.size.0,
        bench.size.1,
        started.elapsed().as_secs_f64(),
        frames as f64 / started.elapsed().as_secs_f64().max(0.001),
        at(0.5),
        at(0.99),
        at(1.0),
    );
    if let Some(path) = std::env::var_os("TSFP_UI_DISPLAY_CAPTURE") {
        match share.capture_rgba() {
            Ok((width, height, rgba)) => {
                let png = tsfp_wgpu::png::encode_rgba(width, height, &rgba);
                match std::fs::write(&path, png) {
                    Ok(()) => eprintln!(
                        "tsfp-bevy: display capture {}x{} hash={:016x} present={}",
                        width,
                        height,
                        tsfp_wgpu::png::image_hash(width, height, &rgba),
                        gpu_share::LAST_PRESENT.load(Ordering::Relaxed)
                    ),
                    Err(error) => eprintln!("tsfp-bevy: display capture write failed: {error}"),
                }
            }
            Err(error) => eprintln!("tsfp-bevy: display capture failed: {error:?}"),
        }
    }
    let (updates, pages, hud) = tsfp_wgpu::frame_layer::display_stats();
    let (regrades, regrade_misses) = tsfp_wgpu::frame_layer::display_regrade_stats();
    eprintln!(
        "tsfp-bevy: UI display stats updates={updates} page_draws={pages} hud_draws={hud} regrades={regrades} regrade_misses={regrade_misses} acquired_guest_frames={}",
        gpu_share::FRAMES.load(Ordering::Relaxed)
    );
    wi::request_quit(window_input::RawEvent::Quit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_consumer_rate_is_bounded() {
        for rate in [120, 144, 240] {
            assert_eq!(consumer_hz(Some(&rate.to_string())), rate);
        }
        for value in [None, Some("0"), Some("481"), Some("nan")] {
            assert_eq!(consumer_hz(value), 60);
        }
    }

    #[test]
    fn parses_seconds_and_size() {
        assert_eq!(
            parse("100:1280x960"),
            Ok(Bench {
                seconds: 100,
                size: (1280, 960)
            })
        );
    }

    #[test]
    fn rejects_malformed_values() {
        for text in [
            "",
            "100",
            "100:1280",
            "0:1280x960",
            "5:0x960",
            "x:1x1",
            "5:1280X960",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn extract_removes_the_argument() {
        let mut arguments = vec![
            "a.xbe".to_string(),
            "--wgpu-share-bench=3:640x480".to_string(),
        ];
        assert_eq!(
            extract(&mut arguments),
            Ok(Some(Bench {
                seconds: 3,
                size: (640, 480)
            }))
        );
        assert_eq!(arguments, vec!["a.xbe".to_string()]);
    }
}
