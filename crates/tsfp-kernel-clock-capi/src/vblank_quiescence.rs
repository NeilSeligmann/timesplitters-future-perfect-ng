//! C ABI for the T371 vblank quiescence predicate; the rule lives in tsfp-vblank-quiescence.
use std::ffi::{CStr, c_char};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tsfp_vblank_quiescence as rule;
use tsfp_vblank_quiescence::{Reason, State, ThreadView, Verdict};

static ENABLED: AtomicBool = AtomicBool::new(false);
static GETTER_CHECKS: AtomicU32 = AtomicU32::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_quiescence_configure(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_quiescence_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_quiescence_note_getter_check() {
    GETTER_CHECKS.fetch_add(1, Ordering::Relaxed);
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_quiescence_getter_checks() -> u32 {
    GETTER_CHECKS.load(Ordering::Relaxed)
}

#[repr(C)]
pub struct CVerdict {
    holds: bool,
    reason: u32,
    handle: u32,
    start_routine: u32,
    examined: u32,
}

unsafe fn slice<'a, T>(ptr: *const T, count: u32) -> &'a [T] {
    if ptr.is_null() || count == 0 {
        &[]
    } else {
        // SAFETY: the caller passes `count` valid elements.
        unsafe { std::slice::from_raw_parts(ptr, count as usize) }
    }
}

/// Evaluate the quiescence predicate over a snapshot of `count` thread views.
///
/// # Safety
/// Every pointer is null or points to the stated number of elements; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_vblank_quiescence_evaluate(
    threads: *const ThreadView,
    count: u32,
    deliverer: u32,
    non_readers: *const u32,
    non_reader_count: u32,
    parked: *const u32,
    parked_count: u32,
    out: *mut CVerdict,
) {
    // SAFETY: forwarded from the caller's contract.
    let verdict: Verdict = unsafe {
        rule::evaluate(
            slice(threads, count),
            deliverer,
            slice(non_readers, non_reader_count),
            slice(parked, parked_count),
        )
    };
    // SAFETY: `out` is writable per the contract.
    unsafe {
        *out = CVerdict {
            holds: verdict.holds,
            reason: verdict.reason as u32,
            handle: verdict.handle,
            start_routine: verdict.start_routine,
            examined: verdict.examined,
        }
    };
}

/// # Safety
/// `thread` must point to one valid view.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_vblank_quiescence_classify(thread: *const ThreadView) -> u32 {
    // SAFETY: valid per the contract.
    rule::classify(unsafe { &*thread }) as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_quiescence_is_registered_reader(
    return_address: u32,
    start_routine: u32,
) -> bool {
    rule::is_registered_reader(return_address, start_routine)
}

/// Default non-reader start routines; the count is stored in `*count`.
///
/// # Safety
/// `count` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_vblank_quiescence_default_non_readers(count: *mut u32) -> *const u32 {
    // SAFETY: writable per the contract.
    unsafe { *count = rule::DEFAULT_NON_READERS.len() as u32 };
    rule::DEFAULT_NON_READERS.as_ptr()
}

const STATE_NAMES: [&CStr; 6] = [
    c"unstarted",
    c"terminated",
    c"host-stopped",
    c"blocked",
    c"host-timer",
    c"runnable",
];
const REASON_NAMES: [&CStr; 6] = [
    c"holds",
    c"delivering thread has no live started record",
    c"another guest thread is runnable and could sample the counter",
    c"another guest thread sleeps to a host deadline",
    c"another guest thread ended by a host stop, not a guest exit",
    c"another guest thread is blocked on a thread that is not quiescent",
];

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_quiescence_state_name(state: u32) -> *const c_char {
    STATE_NAMES
        .get(state as usize)
        .copied()
        .unwrap_or(c"invalid")
        .as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_quiescence_reason_name(reason: u32) -> *const c_char {
    REASON_NAMES
        .get(reason as usize)
        .copied()
        .unwrap_or(c"invalid")
        .as_ptr()
}

// Keep the Rust enums and the C name tables in step.
const _: () = {
    assert!(State::Runnable as usize + 1 == STATE_NAMES.len());
    assert!(Reason::BlockedOnActive as usize + 1 == REASON_NAMES.len());
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_configuration_and_getter_counter_are_rust_owned() {
        let previous = tsfp_vblank_quiescence_enabled();
        tsfp_vblank_quiescence_configure(true);
        assert!(tsfp_vblank_quiescence_enabled());
        tsfp_vblank_quiescence_configure(false);
        assert!(!tsfp_vblank_quiescence_enabled());
        tsfp_vblank_quiescence_configure(previous);

        let before = tsfp_vblank_quiescence_getter_checks();
        tsfp_vblank_quiescence_note_getter_check();
        assert_eq!(
            tsfp_vblank_quiescence_getter_checks(),
            before.wrapping_add(1)
        );
    }
}
