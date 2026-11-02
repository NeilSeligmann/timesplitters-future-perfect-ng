// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned periodic audio refill scheduling for the live SoundHost.
//!
//! The baseline host supplies its clock and PCM sink. The direct Bevy worker samples the shared
//! kernel virtual clock from Rust and delivers PCM through the registered Rust sink. Rust owns the
//! shared audio-work lock that keeps this worker from racing guest-thread DirectSound rendering.
//! Cadence, gating, rendering, and counters stay here.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const REFILL_PERIOD: Duration = Duration::from_millis(5);

type Clock = unsafe extern "C" fn(*mut c_void) -> u64;
type Sink = unsafe extern "C" fn(*mut c_void, *const i16, usize) -> bool;

static PUMP_FRAMES: AtomicU64 = AtomicU64::new(0);
static PUMP_RENDERS: AtomicU64 = AtomicU64::new(0);
static PUMP_FAILURES: AtomicU64 = AtomicU64::new(0);

struct Worker {
    stop: Arc<AtomicBool>,
    join: JoinHandle<()>,
}

static WORKER: Mutex<Option<Worker>> = Mutex::new(None);

fn worker_slot() -> std::sync::MutexGuard<'static, Option<Worker>> {
    WORKER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn should_refill<T>(runtime_active: bool, try_lock: impl FnOnce() -> Option<T>) -> Option<T> {
    if runtime_active { try_lock() } else { None }
}

fn record_pump_result(frames: u64, succeeded: bool) {
    PUMP_FRAMES.fetch_add(frames, Ordering::Relaxed);
    if frames != 0 {
        PUMP_RENDERS.fetch_add(1, Ordering::Relaxed);
    }
    if !succeeded {
        PUMP_FAILURES.fetch_add(1, Ordering::Relaxed);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_start_pump(
    clock: Option<Clock>,
    sink: Option<Sink>,
    context: *mut c_void,
) -> bool {
    let (Some(clock), Some(sink)) = (clock, sink) else {
        return false;
    };
    start_pump_inner(Some(clock), Some(sink), context, false)
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_start_pump_direct() -> bool {
    start_pump_inner(None, None, std::ptr::null_mut(), true)
}

fn start_pump_inner(
    clock: Option<Clock>,
    sink: Option<Sink>,
    context: *mut c_void,
    direct_only: bool,
) -> bool {
    let mut slot = worker_slot();
    if slot.is_some() {
        return false;
    }

    PUMP_FRAMES.store(0, Ordering::Relaxed);
    PUMP_RENDERS.store(0, Ordering::Relaxed);
    PUMP_FAILURES.store(0, Ordering::Relaxed);

    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let context = context as usize;
    let join = thread::Builder::new()
        .name("tsfp-rust-audio-refill".into())
        .spawn(move || {
            let context = context as *mut c_void;
            while !thread_stop.load(Ordering::Acquire) {
                thread::sleep(REFILL_PERIOD);
                if thread_stop.load(Ordering::Acquire) {
                    break;
                }

                let active = super::host_runtime::dsound_audio_runtime_active();
                let Some(_audio_work_guard) = should_refill(active, super::work_lock::try_lock)
                else {
                    continue;
                };

                let mut frames = 0u64;
                // SAFETY: callbacks and context remain valid until stop joins this worker. The
                // Rust work guard spans the full pump and releases before the next iteration.
                let good = if direct_only {
                    // SAFETY: the direct-only worker samples the kernel clock in Rust and has no
                    // host clock or legacy C sink callback by contract.
                    unsafe { super::host_runtime::dsound_audio_runtime_pump_direct(&mut frames) }
                } else {
                    let Some(clock) = clock else {
                        record_pump_result(frames, false);
                        continue;
                    };
                    // SAFETY: the baseline worker's callbacks remain valid until stop joins it.
                    unsafe {
                        super::host_runtime::dsound_audio_runtime_pump(
                            Some(clock),
                            sink,
                            context,
                            &mut frames,
                        )
                    }
                };
                record_pump_result(frames, good);
            }
        });

    let Ok(join) = join else {
        return false;
    };
    *slot = Some(Worker { stop, join });
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_audio_runtime_pump_stats(
    frames: *mut u64,
    renders: *mut u64,
    failures: *mut u64,
) {
    if !frames.is_null() {
        // SAFETY: the optional out pointer is checked and provided by the caller.
        unsafe { *frames = PUMP_FRAMES.load(Ordering::Relaxed) };
    }
    if !renders.is_null() {
        // SAFETY: the optional out pointer is checked and provided by the caller.
        unsafe { *renders = PUMP_RENDERS.load(Ordering::Relaxed) };
    }
    if !failures.is_null() {
        // SAFETY: the optional out pointer is checked and provided by the caller.
        unsafe { *failures = PUMP_FAILURES.load(Ordering::Relaxed) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_stop_pump() {
    let worker = worker_slot().take();
    if let Some(worker) = worker {
        worker.stop.store(true, Ordering::Release);
        let _ = worker.join.join();
    }
}

#[cfg(test)]
mod tests {
    use super::{PUMP_FAILURES, PUMP_FRAMES, PUMP_RENDERS, record_pump_result, should_refill};
    use std::sync::atomic::Ordering;

    #[test]
    fn refill_requires_an_active_runtime_and_the_host_lock() {
        let mut attempted_lock = false;
        assert!(
            should_refill(false, || {
                attempted_lock = true;
                Some(())
            })
            .is_none()
        );
        assert!(!attempted_lock);
        assert!(should_refill(true, || None::<()>).is_none());
        assert!(should_refill(true, || Some(())).is_some());
    }

    #[test]
    fn worker_owns_cumulative_frame_render_and_failure_counts() {
        PUMP_FRAMES.store(0, Ordering::Relaxed);
        PUMP_RENDERS.store(0, Ordering::Relaxed);
        PUMP_FAILURES.store(0, Ordering::Relaxed);
        record_pump_result(128, true);
        record_pump_result(0, true);
        record_pump_result(64, false);
        assert_eq!(PUMP_FRAMES.load(Ordering::Relaxed), 192);
        assert_eq!(PUMP_RENDERS.load(Ordering::Relaxed), 2);
        assert_eq!(PUMP_FAILURES.load(Ordering::Relaxed), 1);
        let (mut frames, mut renders, mut failures) = (0, 0, 0);
        unsafe {
            super::dsound_audio_runtime_pump_stats(&mut frames, &mut renders, &mut failures);
        }
        assert_eq!((frames, renders, failures), (192, 2, 1));
    }
}
