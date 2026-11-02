//! C ABI for the T2260 frame phase timer; logic lives in tsfp-frame-phase.
use std::ffi::{CStr, c_char};
use std::fs::File;
use std::io::Write;
use std::os::fd::FromRawFd;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::ThreadId;
use tsfp_frame_phase::FrameTimer;

struct State {
    timer: FrameTimer,
    csv: Option<File>,
    owner: Option<ThreadId>,
}
static STATE: Mutex<Option<State>> = Mutex::new(None);
static ENABLED: AtomicBool = AtomicBool::new(false);

#[repr(C)]
struct Timespec {
    sec: i64,
    nsec: i64,
}
unsafe extern "C" {
    fn clock_gettime(clock: i32, out: *mut Timespec) -> i32;
}
const CLOCK_MONOTONIC: i32 = 1;
const CLOCK_THREAD_CPUTIME_ID: i32 = 3;

fn clock_ns(clock: i32) -> u64 {
    let mut now = Timespec { sec: 0, nsec: 0 };
    // SAFETY: `now` is a valid out pointer for the duration of the call.
    unsafe { clock_gettime(clock, &mut now) };
    now.sec as u64 * 1_000_000_000 + now.nsec as u64
}

/// Start writing one CSV row per frame to `path`; false when the file cannot be created.
/// `TSFP_FPT_DRILL=LO:HI` (hex) moves the drill range.
///
/// # Safety
/// `path` must be a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_frame_phase_start(path: *const c_char) -> bool {
    // SAFETY: the caller guarantees a NUL-terminated string.
    let Ok(path) = unsafe { CStr::from_ptr(path) }.to_str() else {
        return false;
    };
    let Ok(mut file) = File::create(path) else {
        return false;
    };
    let mut timer = FrameTimer::new();
    if let Ok(range) = std::env::var("TSFP_FPT_DRILL") {
        let parsed = range.split_once(':').and_then(|(lo, hi)| {
            Some((
                u32::from_str_radix(lo, 16).ok()?,
                u32::from_str_radix(hi, 16).ok()?,
            ))
        });
        if let Some((lo, hi)) = parsed {
            timer.set_drill_range(lo, hi);
        }
    }
    if file.write_all(timer.start_csv().as_bytes()).is_err() {
        return false;
    }
    *STATE.lock().unwrap() = Some(State {
        timer,
        csv: Some(file),
        owner: None,
    });
    ENABLED.store(true, Ordering::Release);
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_phase_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Note the return site at `[esp]`. Only the first thread that reaches a tracked site is followed.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_phase_note(site: u32) {
    let mut guard = STATE.lock().unwrap();
    let Some(state) = guard.as_mut() else { return };
    if !state.timer.is_tracked(site) {
        return;
    }
    let me = std::thread::current().id();
    if *state.owner.get_or_insert(me) != me {
        return;
    }
    state.timer.note(
        site,
        clock_ns(CLOCK_MONOTONIC),
        clock_ns(CLOCK_THREAD_CPUTIME_ID),
    );
    let rows = state.timer.take_csv_rows();
    if let (Some(file), false) = (state.csv.as_mut(), rows.is_empty()) {
        let _ = file.write_all(rows.as_bytes());
    }
}

/// Close the CSV and write the summary report to file descriptor `report_fd` (not closed).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_frame_phase_stop(report_fd: i32) {
    if !ENABLED.swap(false, Ordering::AcqRel) {
        return;
    }
    let Some(state) = STATE.lock().unwrap().take() else {
        return;
    };
    drop(state.csv);
    // SAFETY: the descriptor belongs to the caller; ManuallyDrop keeps it open.
    let mut out = std::mem::ManuallyDrop::new(unsafe { File::from_raw_fd(report_fd) });
    let _ = out.write_all(state.timer.report().as_bytes());
}
