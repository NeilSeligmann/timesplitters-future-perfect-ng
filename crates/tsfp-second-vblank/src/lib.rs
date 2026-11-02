//! Configuration and epoch state for the second-vblank host policy.
//!
//! This is the first, offline slice of the port. Guest access, host thread
//! identity and delivery remain in the C implementation until integrated.

mod host_flags;
mod quiescence_state;
mod state_abi;

pub const MAX_BLANKS: u32 = 1_000_000;
pub const DEFAULT_WORKER_HOLD_MS: u32 = 600_000;
pub const OWNER_START: u32 = 0x3801d9;
pub const OWNER_SYSTEM: u32 = 0x37fe1d;
pub const CALLBACK: u32 = 0x22020;
pub const GATE: u32 = 0x156840;
pub const GATE_WAIT: u32 = 0x3800bf;
pub const GETTER: u32 = 0x22030;

#[repr(C)]
pub struct FinalHoldHost {
    pub now: extern "C" fn() -> u64,
    pub shutdown: extern "C" fn() -> bool,
    pub owner_blocked: extern "C" fn(u32) -> bool,
    pub mark_held: extern "C" fn(u32),
    pub idle: extern "C" fn(),
}

#[repr(C)]
pub struct StartupOwnerWaitHost {
    pub now: extern "C" fn() -> u64,
    pub owner_blocked: extern "C" fn(u32) -> bool,
    pub shutdown: extern "C" fn() -> bool,
    pub idle: extern "C" fn(),
}

#[repr(C)]
pub struct WorkerHoldHost {
    pub now: extern "C" fn() -> u64,
    pub shutdown: extern "C" fn() -> bool,
    pub ready: extern "C" fn(u32) -> bool,
    pub mark_held: extern "C" fn(u32),
    pub idle: extern "C" fn(),
}

/// Returns 1 for shutdown and 2 for timeout; the host retains the last verdict.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_worker_hold(
    handle: u32,
    limit: u64,
    host: WorkerHoldHost,
) -> u32 {
    let began = (host.now)();
    let mut held = false;
    loop {
        if (host.shutdown)() {
            return 1;
        }
        if (host.ready)(handle) {
            return 0;
        }
        if !held {
            held = true;
            (host.mark_held)(handle);
        }
        if (host.now)().wrapping_sub(began) >= limit {
            return 2;
        }
        (host.idle)();
    }
}

/// Return the host-side stop reason without unwinding through Rust.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_wait_startup_owner(
    handle: u32,
    limit: u64,
    host: StartupOwnerWaitHost,
) -> u32 {
    let began = (host.now)();
    while !(host.owner_blocked)(handle) {
        if (host.shutdown)() {
            return 1;
        }
        if (host.now)().wrapping_sub(began) >= limit {
            return 2;
        }
        (host.idle)();
    }
    0
}

/// Wait for the owner to block on the worker; return a refusal to the host
/// so that its non-local stop never crosses a Rust stack frame.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_final_hold(
    handle: u32,
    limit: u64,
    host: FinalHoldHost,
) -> u32 {
    let began = (host.now)();
    let mut counted = false;
    loop {
        if (host.shutdown)() {
            return 1;
        }
        if (host.owner_blocked)(handle) {
            return 0;
        }
        if !counted {
            counted = true;
            (host.mark_held)(handle);
        }
        if (host.now)().wrapping_sub(began) >= limit {
            return 2;
        }
        (host.idle)();
    }
}

#[cfg(test)]
mod final_hold_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TICKS: AtomicU32 = AtomicU32::new(0);
    static MARKS: AtomicU32 = AtomicU32::new(0);
    static MODE: AtomicU32 = AtomicU32::new(0);
    extern "C" fn now() -> u64 {
        TICKS.load(Ordering::SeqCst) as u64
    }
    extern "C" fn shutdown() -> bool {
        MODE.load(Ordering::SeqCst) == 1
    }
    extern "C" fn blocked(handle: u32) -> bool {
        handle == 42 && MODE.load(Ordering::SeqCst) == 0 && TICKS.load(Ordering::SeqCst) >= 2
    }
    extern "C" fn mark(handle: u32) {
        assert_eq!(handle, 42);
        MARKS.fetch_add(1, Ordering::SeqCst);
    }
    extern "C" fn idle() {
        TICKS.fetch_add(1, Ordering::SeqCst);
    }
    fn host() -> FinalHoldHost {
        FinalHoldHost {
            now,
            shutdown,
            owner_blocked: blocked,
            mark_held: mark,
            idle,
        }
    }

    #[test]
    fn final_hold_waits_once_or_reports_shutdown_and_timeout() {
        TICKS.store(0, Ordering::SeqCst);
        MARKS.store(0, Ordering::SeqCst);
        MODE.store(0, Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_final_hold(42, 5, host()), 0);
        assert_eq!(MARKS.load(Ordering::SeqCst), 1);
        MODE.store(1, Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_final_hold(42, 5, host()), 1);
        MODE.store(2, Ordering::SeqCst);
        TICKS.store(0, Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_final_hold(42, 2, host()), 2);
        assert_eq!(MARKS.load(Ordering::SeqCst), 2);
        let startup = || StartupOwnerWaitHost {
            now,
            owner_blocked: blocked,
            shutdown,
            idle,
        };
        MODE.store(0, Ordering::SeqCst);
        TICKS.store(0, Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_wait_startup_owner(42, 5, startup()), 0);
        MODE.store(1, Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_wait_startup_owner(42, 5, startup()), 1);
        MODE.store(2, Ordering::SeqCst);
        TICKS.store(0, Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_wait_startup_owner(42, 2, startup()), 2);
        let worker = || WorkerHoldHost {
            now,
            shutdown,
            ready: blocked,
            mark_held: mark,
            idle,
        };
        MODE.store(0, Ordering::SeqCst);
        TICKS.store(0, Ordering::SeqCst);
        let before = MARKS.load(Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_worker_hold(42, 5, worker()), 0);
        assert_eq!(MARKS.load(Ordering::SeqCst), before + 1);
        MODE.store(1, Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_worker_hold(42, 5, worker()), 1);
        MODE.store(2, Ordering::SeqCst);
        TICKS.store(0, Ordering::SeqCst);
        assert_eq!(tsfp_second_vblank_worker_hold(42, 2, worker()), 2);
    }
}

/// Host operations needed by the display-period wait. The clock and scheduler
/// remain host services; the wait and its failure decisions live here.
#[repr(C)]
pub struct DisplayWaitHost {
    pub shutdown: extern "C" fn() -> bool,
    pub virtual_time: extern "C" fn() -> bool,
    pub now: extern "C" fn(bool, *mut u64) -> bool,
    pub mode: extern "C" fn() -> u32,
    pub poll: extern "C" fn(u64, u32, *mut u64) -> u32,
    pub wait: extern "C" fn(bool, u64, u64) -> bool,
}

/// Wait for credit, returning false when shutdown interrupts the wait.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_await_credit(
    now: extern "C" fn() -> u64,
    pending: extern "C" fn() -> bool,
    shutdown: extern "C" fn() -> bool,
    idle: extern "C" fn(),
) -> bool {
    let began = now();
    loop {
        if !pending() || now().wrapping_sub(began) >= 10_000 {
            return true;
        }
        if shutdown() {
            return false;
        }
        idle();
    }
}

#[cfg(test)]
mod credit_wait_tests {
    use super::tsfp_second_vblank_await_credit;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    static TIME: AtomicU64 = AtomicU64::new(0);
    static MODE: AtomicUsize = AtomicUsize::new(0);
    static IDLES: AtomicUsize = AtomicUsize::new(0);
    extern "C" fn now() -> u64 {
        TIME.load(Ordering::SeqCst)
    }
    extern "C" fn pending() -> bool {
        MODE.load(Ordering::SeqCst) != 0
    }
    extern "C" fn shutdown() -> bool {
        MODE.load(Ordering::SeqCst) == 2
    }
    extern "C" fn idle() {
        IDLES.fetch_add(1, Ordering::SeqCst);
        TIME.store(10_000, Ordering::SeqCst);
    }

    #[test]
    fn credit_wait_handles_completion_shutdown_and_timeout() {
        TIME.store(0, Ordering::SeqCst);
        IDLES.store(0, Ordering::SeqCst);
        MODE.store(0, Ordering::SeqCst);
        assert!(tsfp_second_vblank_await_credit(
            now, pending, shutdown, idle
        ));
        MODE.store(2, Ordering::SeqCst);
        assert!(!tsfp_second_vblank_await_credit(
            now, pending, shutdown, idle
        ));
        MODE.store(1, Ordering::SeqCst);
        assert!(tsfp_second_vblank_await_credit(
            now, pending, shutdown, idle
        ));
        assert_eq!(IDLES.load(Ordering::SeqCst), 1);
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_wait_display_period(host: DisplayWaitHost) -> bool {
    loop {
        if (host.shutdown)() {
            return false;
        }
        let virtual_time = (host.virtual_time)();
        let mut now = 0;
        if !(host.now)(virtual_time, &mut now) {
            return false;
        }
        let refresh = if (host.mode)() & 0x0040_0000 != 0 {
            60
        } else {
            50
        };
        let mut wait = 0;
        match (host.poll)(now, refresh, &mut wait) {
            1 => return true,
            0 if wait != 0 && (!virtual_time || now.checked_add(wait).is_some()) => {}
            _ => return false,
        }
        if !(host.wait)(virtual_time, now, wait) {
            return false;
        }
    }
}

/// Start each owner frame wait with a fresh deadline, then propagate the wait result.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_wait_owner_display_period(
    reset: extern "C" fn(),
    wait: extern "C" fn() -> bool,
) -> bool {
    reset();
    wait()
}

#[cfg(test)]
mod owner_display_wait_tests {
    use super::tsfp_second_vblank_wait_owner_display_period;
    use std::sync::atomic::{AtomicU32, Ordering};

    static STAGE: AtomicU32 = AtomicU32::new(0);
    extern "C" fn reset() {
        STAGE.store(1, Ordering::SeqCst);
    }
    extern "C" fn wait() -> bool {
        STAGE.swap(2, Ordering::SeqCst) == 1
    }
    extern "C" fn fail() -> bool {
        false
    }

    #[test]
    fn resets_before_wait_and_returns_its_result() {
        STAGE.store(0, Ordering::SeqCst);
        assert!(tsfp_second_vblank_wait_owner_display_period(reset, wait));
        assert_eq!(STAGE.load(Ordering::SeqCst), 2);
        assert!(!tsfp_second_vblank_wait_owner_display_period(reset, fail));
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod display_wait_tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static CASE: AtomicU32 = AtomicU32::new(0);
    static POLLS: AtomicU32 = AtomicU32::new(0);
    extern "C" fn shutdown() -> bool {
        CASE.load(Ordering::SeqCst) == 1
    }
    extern "C" fn virtual_time() -> bool {
        true
    }
    extern "C" fn now(_: bool, out: *mut u64) -> bool {
        unsafe {
            *out = if CASE.load(Ordering::SeqCst) == 2 {
                u64::MAX
            } else {
                100
            };
        }
        true
    }
    extern "C" fn mode() -> u32 {
        0x0040_0000
    }
    extern "C" fn poll(_: u64, refresh: u32, out: *mut u64) -> u32 {
        assert_eq!(refresh, 60);
        unsafe {
            *out = 1;
        }
        if POLLS.fetch_add(1, Ordering::SeqCst) == 0 {
            0
        } else {
            1
        }
    }
    extern "C" fn wait(virtual_time: bool, now: u64, duration: u64) -> bool {
        assert!(virtual_time);
        assert_eq!((now, duration), (100, 1));
        true
    }
    fn host() -> DisplayWaitHost {
        DisplayWaitHost {
            shutdown,
            virtual_time,
            now,
            mode,
            poll,
            wait,
        }
    }
    #[test]
    fn waits_for_deadline_and_refuses_shutdown_or_virtual_overflow() {
        CASE.store(0, Ordering::SeqCst);
        POLLS.store(0, Ordering::SeqCst);
        assert!(tsfp_second_vblank_wait_display_period(host()));
        assert_eq!(POLLS.load(Ordering::SeqCst), 2);
        CASE.store(1, Ordering::SeqCst);
        assert!(!tsfp_second_vblank_wait_display_period(host()));
        CASE.store(2, Ordering::SeqCst);
        POLLS.store(0, Ordering::SeqCst);
        assert!(!tsfp_second_vblank_wait_display_period(host()));
    }
}

/// Interpret the gate's guest float words using its ordered comparison.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_gate_closed(progress: u32, one: u32) -> bool {
    f32::from_bits(one) > f32::from_bits(progress)
}

#[cfg(test)]
mod gate_tests {
    use super::tsfp_second_vblank_gate_closed as closed;

    #[test]
    fn ordered_comparison_handles_equal_and_nan() {
        assert!(closed(0.5f32.to_bits(), 1.0f32.to_bits()));
        assert!(!closed(1.0f32.to_bits(), 1.0f32.to_bits()));
        assert!(!closed(f32::NAN.to_bits(), 1.0f32.to_bits()));
        assert!(!closed(0, f32::NAN.to_bits()));
    }
}

/// Check one readable device bookkeeping word before callback delivery.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_bookkeeping_matches(
    index: u32,
    value: u32,
    count: u32,
    consumer: u32,
    widescreen: bool,
) -> bool {
    let expected = match index {
        0 => CALLBACK,
        1 => count,
        2 => consumer,
        3 | 5..=8 => 0,
        4 => {
            if widescreen {
                0x02490104
            } else {
                0x02480104
            }
        }
        _ => return false,
    };
    value == expected
}

/// Read and validate the device words in their original order.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_bookkeeping_valid(
    count: u32,
    consumer: u32,
    widescreen: bool,
    modelled: bool,
    read: extern "C" fn(u32, *mut u32) -> bool,
) -> bool {
    let offsets = [
        0x1db8, 0x1de8, 0x1de4, 0x1dec, 0x1ddc, 0x1d9c, 0x1da8, 0x2448, 0x244c,
    ];
    for (index, offset) in offsets.into_iter().enumerate() {
        if modelled && index == 3 {
            continue;
        }
        let mut value = 0;
        if !read(0x3e3f60 + offset, &mut value)
            || !tsfp_second_vblank_bookkeeping_matches(
                index as u32,
                value,
                count,
                consumer,
                widescreen,
            )
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod bookkeeping_tests {
    use super::tsfp_second_vblank_bookkeeping_matches as matches;

    #[test]
    fn checks_each_word_and_rejects_unknown_indices() {
        let words = [0x22020, 7, 3, 0, 0x02490104, 0, 0, 0, 0];
        for (i, word) in words.into_iter().enumerate() {
            assert!(matches(i as u32, word, 7, 3, true));
            assert!(!matches(i as u32, word ^ 1, 7, 3, true));
        }
        assert!(matches(4, 0x02480104, 7, 3, false));
        assert!(!matches(9, 0, 7, 3, false));
    }
}

/// Identify a live loading worker from the guest thread table.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_worker_record_matches(
    in_use: bool,
    started: bool,
    finished: bool,
    start: u32,
    system: u32,
    context: u32,
) -> bool {
    in_use && started && !finished && start == 0x156cb0 && system == 0x37fe1d && context == 0
}

/// Identify the terminated producer whose control block matches the callback.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_producer_record_matches(
    in_use: bool,
    started: bool,
    finished: bool,
    terminated: bool,
    termination_requested: bool,
    control_base: u32,
    fs: u32,
    system: u32,
    start: u32,
    context: u32,
) -> bool {
    in_use
        && started
        && finished
        && terminated
        && termination_requested
        && control_base == fs
        && system == 0x37fe1d
        && start == 0x156cb0
        && context == 0
}

/// Match the requested guest identity to a live worker row.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_worker_identity_matches(
    handle: u32,
    fs: u32,
    control_base: u32,
    record_matches: bool,
) -> bool {
    handle != 0 && fs != 0 && record_matches && control_base == fs
}

/// Look up a worker only after both components of its identity are present.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_lookup_worker_identity(
    handle: u32,
    fs: u32,
    lookup: extern "C" fn(u32, *mut u32) -> bool,
) -> bool {
    if handle == 0 || fs == 0 {
        return false;
    }
    let mut control_base = 0;
    lookup(handle, &mut control_base) && control_base == fs
}

/// Check the current worker only when a worker handle has been recorded.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_worker_epoch_active(
    handle: u32,
    live: extern "C" fn(u32) -> bool,
) -> bool {
    handle != 0 && live(handle)
}

#[cfg(test)]
mod worker_record_tests {
    use super::tsfp_second_vblank_worker_record_matches as matches;

    #[test]
    fn producer_requires_completed_matching_thread() {
        let matches = super::tsfp_second_vblank_producer_record_matches;
        assert!(matches(
            true, true, true, true, true, 3, 3, 0x37fe1d, 0x156cb0, 0
        ));
        assert!(!matches(
            false, true, true, true, true, 3, 3, 0x37fe1d, 0x156cb0, 0
        ));
        assert!(!matches(
            true, false, true, true, true, 3, 3, 0x37fe1d, 0x156cb0, 0
        ));
        assert!(!matches(
            true, true, false, true, true, 3, 3, 0x37fe1d, 0x156cb0, 0
        ));
        assert!(!matches(
            true, true, true, false, true, 3, 3, 0x37fe1d, 0x156cb0, 0
        ));
        assert!(!matches(
            true, true, true, true, false, 3, 3, 0x37fe1d, 0x156cb0, 0
        ));
        assert!(!matches(
            true, true, true, true, true, 3, 4, 0x37fe1d, 0x156cb0, 0
        ));
        assert!(!matches(true, true, true, true, true, 3, 3, 0, 0x156cb0, 0));
        assert!(!matches(true, true, true, true, true, 3, 3, 0x37fe1d, 0, 0));
        assert!(!matches(
            true, true, true, true, true, 3, 3, 0x37fe1d, 0x156cb0, 1
        ));
    }

    #[test]
    fn worker_epoch_queries_only_a_recorded_handle() {
        extern "C" fn live(handle: u32) -> bool {
            assert_ne!(handle, 0);
            handle == 7
        }
        let active = super::tsfp_second_vblank_worker_epoch_active;
        assert!(!active(0, live));
        assert!(!active(8, live));
        assert!(active(7, live));
    }

    #[test]
    fn requires_a_live_matching_worker() {
        assert!(matches(true, true, false, 0x156cb0, 0x37fe1d, 0));
        assert!(!matches(false, true, false, 0x156cb0, 0x37fe1d, 0));
        assert!(!matches(true, false, false, 0x156cb0, 0x37fe1d, 0));
        assert!(!matches(true, true, true, 0x156cb0, 0x37fe1d, 0));
        assert!(!matches(true, true, false, 0, 0x37fe1d, 0));
        assert!(!matches(true, true, false, 0x156cb0, 0, 0));
        assert!(!matches(true, true, false, 0x156cb0, 0x37fe1d, 1));
    }

    #[test]
    fn identity_requires_nonzero_handle_and_matching_control_block() {
        use super::tsfp_second_vblank_worker_identity_matches as identity;
        assert!(identity(1, 0x1000, 0x1000, true));
        assert!(!identity(0, 0x1000, 0x1000, true));
        assert!(!identity(1, 0, 0, true));
        assert!(!identity(1, 0x1000, 0x2000, true));
        assert!(!identity(1, 0x1000, 0x1000, false));
    }

    #[test]
    #[allow(unsafe_code)]
    fn identity_lookup_requires_a_matching_live_row() {
        use super::tsfp_second_vblank_lookup_worker_identity as lookup;
        extern "C" fn row(handle: u32, control: *mut u32) -> bool {
            assert_ne!(handle, 0);
            if handle != 1 {
                return false;
            }
            // SAFETY: the caller supplies a valid output pointer.
            unsafe {
                *control = 0x1000;
            }
            true
        }
        assert!(!lookup(0, 0x1000, row));
        assert!(!lookup(1, 0, row));
        assert!(!lookup(2, 0x1000, row));
        assert!(!lookup(1, 0x2000, row));
        assert!(lookup(1, 0x1000, row));
    }
}

/// Observations made under the host lock before startup-worker admission.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StartupCandidate {
    pub worker_budget: u32,
    pub owner_budget: u32,
    pub credits: u32,
    pub interactive: bool,
    pub clock_enabled: bool,
    pub enabled: bool,
    pub bound: bool,
    pub registered: bool,
    pub first_delivered: bool,
    pub second_attempted: bool,
    pub second_delivered: bool,
    pub inflight: bool,
    pub refused: bool,
    pub producer_matches: bool,
    pub is_owner: bool,
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_startup_candidate(s: StartupCandidate) -> bool {
    let clock_path = s.clock_enabled && s.worker_budget == 0;
    let has_credit = s.credits == 1;
    s.interactive
        && (clock_path || s.owner_budget != 0)
        && (s.clock_enabled || s.worker_budget != 0)
        && s.enabled
        && s.bound
        && s.registered
        && s.first_delivered
        && !s.second_attempted
        && !s.second_delivered
        && !s.inflight
        && !s.refused
        && s.credits <= 1
        && (has_credit || s.clock_enabled)
        && s.producer_matches
        && !s.is_owner
}

/// Recheck the locked observations immediately before reserving startup delivery.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_startup_reservation_allowed(
    s: StartupCandidate,
    had_credit: bool,
    owner_blocked: bool,
) -> bool {
    let clock_path = s.clock_enabled && s.worker_budget == 0;
    let budgeted_path = s.owner_budget != 0 && s.worker_budget != 0;
    s.interactive
        && (clock_path || budgeted_path)
        && s.enabled
        && s.bound
        && s.registered
        && s.first_delivered
        && !s.second_delivered
        && !s.inflight
        && !s.refused
        && !s.second_attempted
        && s.credits == u32::from(had_credit)
        && !s.is_owner
        && owner_blocked
        && (!had_credit || s.producer_matches)
}

/// Check the guest ranges touched by a callback before any writes occur.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_context_aliased(
    stack_low: u32,
    stack_high: u32,
    fs: u32,
    esp: u32,
    startup: bool,
) -> bool {
    fn overlap(a: u32, n: u64, b: u32, m: u64) -> bool {
        (a as u64) < (b as u64).wrapping_add(m) && (b as u64) < (a as u64).wrapping_add(n)
    }
    let bytes = (stack_high as u64).wrapping_sub(stack_low as u64);
    let device = 0x3e3f60;
    let counter = 0x563918;
    overlap(stack_low, bytes, device, 0x2450)
        || overlap(stack_low, bytes, fs, 4096)
        || overlap(stack_low, bytes, esp, 4)
        || overlap(fs, 4096, device, 0x2450)
        || overlap(esp, 4, device, 0x2450)
        || overlap(esp, 4, fs, 4096)
        || overlap(stack_low, bytes, counter, 4)
        || overlap(fs, 4096, counter, 4)
        || if startup {
            false
        } else {
            overlap(stack_low, bytes, 0x3e3f58, 4) || overlap(esp, 4, counter, 4)
        }
}

#[cfg(test)]
mod context_alias_tests {
    use super::tsfp_second_vblank_context_aliased as aliased;

    #[test]
    fn checks_common_and_path_specific_ranges() {
        assert!(!aliased(0x10000, 0x11000, 0x20000, 0x30000, true));
        assert!(aliased(0x10000, 0x11000, 0x10000, 0x30000, true));
        assert!(!aliased(0x3e3f58, 0x3e3f5c, 0x20000, 0x30000, true));
        assert!(aliased(0x3e3f58, 0x3e3f5c, 0x20000, 0x30000, false));
        assert!(!aliased(0x10000, 0x11000, 0x20000, 0x563918, true));
        assert!(aliased(0x10000, 0x11000, 0x20000, 0x563918, false));
    }
}

/// Non-owner calls to the running-flag writers are forbidden while the
/// owner is in the gate or a worker epoch is active. The caller checks the
/// budget and thread identity before applying this decision.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_running_flag_entry(callee: u32) -> bool {
    matches!(callee, 0x155550 | 0x156d80 | 0x156d10 | GATE)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_running_flag_refusal(
    callee: u32,
    owner_in_gate: bool,
    worker_epoch_active: bool,
) -> bool {
    tsfp_second_vblank_running_flag_entry(callee) && (owner_in_gate || worker_epoch_active)
}

/// Query the worker only when the owner is outside the protected gate.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_check_running_flag(
    callee: u32,
    owner_in_gate: bool,
    worker_active: extern "C" fn() -> bool,
) -> bool {
    let active = if owner_in_gate {
        false
    } else {
        worker_active()
    };
    tsfp_second_vblank_running_flag_refusal(callee, owner_in_gate, active)
}

#[cfg(test)]
mod running_flag_tests {
    use super::*;

    #[test]
    fn gate_check_skips_worker_query() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static CALLED: AtomicBool = AtomicBool::new(false);
        extern "C" fn active() -> bool {
            true
        }
        extern "C" fn observed() -> bool {
            CALLED.store(true, Ordering::SeqCst);
            false
        }
        assert!(tsfp_second_vblank_check_running_flag(GATE, true, observed));
        assert!(!CALLED.load(Ordering::SeqCst));
        assert!(tsfp_second_vblank_check_running_flag(GATE, false, active));
        assert!(!tsfp_second_vblank_check_running_flag(
            GETTER, false, active
        ));
    }

    #[test]
    fn writer_entries_refuse_only_during_protected_epochs() {
        for callee in [0x155550, 0x156d80, 0x156d10, GATE] {
            assert!(!tsfp_second_vblank_running_flag_refusal(
                callee, false, false
            ));
            assert!(tsfp_second_vblank_running_flag_refusal(callee, true, false));
            assert!(tsfp_second_vblank_running_flag_refusal(callee, false, true));
        }
        assert!(!tsfp_second_vblank_running_flag_refusal(GETTER, true, true));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerLocation {
    Elsewhere,
    Gate,
    GateWait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerCall {
    GateEntry,
    Continue(OwnerLocation),
    RefuseGateExit,
    RefuseGetter,
}

/// Whether this call requires the host to inspect the worker epoch.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_owner_needs_worker_check(location: u32, callee: u32) -> bool {
    (location == 1 && callee != GATE_WAIT) || (location != 1 && callee == GETTER)
}

/// Decide the owner call-hook transition. Gate entry needs guest memory reads,
/// so its location and episode counter are updated by the caller afterwards.
pub fn owner_call(
    location: OwnerLocation,
    callee: u32,
    budgeted_worker: bool,
    worker_active: bool,
) -> OwnerCall {
    if callee == GATE {
        return OwnerCall::GateEntry;
    }
    if location == OwnerLocation::Gate {
        if callee == GATE_WAIT {
            return OwnerCall::Continue(OwnerLocation::GateWait);
        }
        if budgeted_worker && worker_active {
            return OwnerCall::RefuseGateExit;
        }
        return OwnerCall::Continue(OwnerLocation::Elsewhere);
    }
    if budgeted_worker && callee == GETTER && worker_active {
        return OwnerCall::RefuseGetter;
    }
    OwnerCall::Continue(OwnerLocation::Elsewhere)
}

/// C call-hook decision: 0 = elsewhere, 1 = gate wait, 2 = gate exit
/// refusal, 3 = getter refusal, 4 = gate entry. The host owns guest reads.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_owner_call(
    location: u32,
    callee: u32,
    budgeted_worker: bool,
    worker_active: bool,
) -> u32 {
    let location = match location {
        1 => OwnerLocation::Gate,
        2 => OwnerLocation::GateWait,
        _ => OwnerLocation::Elsewhere,
    };
    match owner_call(location, callee, budgeted_worker, worker_active) {
        OwnerCall::GateEntry => 4,
        OwnerCall::Continue(OwnerLocation::GateWait) => 1,
        OwnerCall::Continue(_) => 0,
        OwnerCall::RefuseGateExit => 2,
        OwnerCall::RefuseGetter => 3,
    }
}

/// Classify a call before performing the host-side guest and thread checks.
/// 0 skips it, 1 handles the owner, and 2 handles a budgeted worker.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_call_role(
    handle: u32,
    owner: u32,
    budgeted_worker: bool,
    startup_clock: bool,
) -> u32 {
    if handle == 0 || (!budgeted_worker && !startup_clock) {
        0
    } else if handle == owner {
        1
    } else if budgeted_worker {
        2
    } else {
        0
    }
}

/// Classify a budgeted worker call: bit 0 checks the running flag writer,
/// bit 1 continues to the getter's worker identity and hold checks.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_worker_call(callee: u32) -> u32 {
    u32::from(tsfp_second_vblank_running_flag_entry(callee)) | (u32::from(callee == GETTER) << 1)
}

#[cfg(test)]
mod owner_call_tests {
    use super::*;

    #[test]
    fn worker_call_classifies_writer_and_getter_entries() {
        assert_eq!(tsfp_second_vblank_worker_call(GATE), 1);
        assert_eq!(tsfp_second_vblank_worker_call(0x155550), 1);
        assert_eq!(tsfp_second_vblank_worker_call(GETTER), 2);
        assert_eq!(tsfp_second_vblank_worker_call(0), 0);
    }

    #[test]
    fn worker_epoch_is_checked_only_for_relevant_owner_calls() {
        assert!(!tsfp_second_vblank_owner_needs_worker_check(1, GATE_WAIT));
        assert!(tsfp_second_vblank_owner_needs_worker_check(1, GETTER));
        assert!(tsfp_second_vblank_owner_needs_worker_check(2, GETTER));
        assert!(tsfp_second_vblank_owner_needs_worker_check(0, GETTER));
        assert!(!tsfp_second_vblank_owner_needs_worker_check(2, GATE_WAIT));
    }

    #[test]
    fn call_role_requires_a_handle_and_an_enabled_mode() {
        assert_eq!(tsfp_second_vblank_call_role(0, 0, true, true), 0);
        assert_eq!(tsfp_second_vblank_call_role(1, 1, false, false), 0);
        assert_eq!(tsfp_second_vblank_call_role(1, 1, false, true), 1);
        assert_eq!(tsfp_second_vblank_call_role(2, 1, false, true), 0);
        assert_eq!(tsfp_second_vblank_call_role(2, 1, true, false), 2);
    }

    #[test]
    fn startup_candidate_c_contract_checks_credit_and_epoch() {
        let mut s = StartupCandidate {
            worker_budget: 1,
            owner_budget: 1,
            credits: 1,
            interactive: true,
            clock_enabled: false,
            enabled: true,
            bound: true,
            registered: true,
            first_delivered: true,
            second_attempted: false,
            second_delivered: false,
            inflight: false,
            refused: false,
            producer_matches: true,
            is_owner: false,
        };
        assert!(tsfp_second_vblank_startup_candidate(s));
        s.producer_matches = false;
        assert!(!tsfp_second_vblank_startup_candidate(s));
        s.producer_matches = true;
        s.credits = 2;
        assert!(!tsfp_second_vblank_startup_candidate(s));
        s.credits = 0;
        s.clock_enabled = true;
        s.worker_budget = 0;
        s.owner_budget = 0;
        assert!(tsfp_second_vblank_startup_candidate(s));
        s.inflight = true;
        assert!(!tsfp_second_vblank_startup_candidate(s));
    }

    #[test]
    fn startup_reservation_rechecks_credit_and_blocked_owner() {
        let mut s = StartupCandidate {
            worker_budget: 1,
            owner_budget: 1,
            credits: 1,
            interactive: true,
            clock_enabled: false,
            enabled: true,
            bound: true,
            registered: true,
            first_delivered: true,
            second_attempted: false,
            second_delivered: false,
            inflight: false,
            refused: false,
            producer_matches: true,
            is_owner: false,
        };
        assert!(tsfp_second_vblank_startup_reservation_allowed(
            s, true, true
        ));
        assert!(!tsfp_second_vblank_startup_reservation_allowed(
            s, true, false
        ));
        s.producer_matches = false;
        assert!(!tsfp_second_vblank_startup_reservation_allowed(
            s, true, true
        ));
        s.credits = 0;
        s.clock_enabled = true;
        s.worker_budget = 0;
        assert!(tsfp_second_vblank_startup_reservation_allowed(
            s, false, true
        ));
        assert!(!tsfp_second_vblank_startup_reservation_allowed(
            s, true, true
        ));
    }

    #[test]
    fn startup_worker_rechecks_credit_and_owner_before_reserving() {
        let mut p = Policy::default();
        p.interactive = true;
        p.startup_clock = true;
        p.state.enabled = true;
        p.state.bound = true;
        p.state.registered = true;
        p.state.first_delivered = true;
        p.state.owner_handle = 1;
        p.state.producer_handle = 2;
        p.state.producer_fs = 3;
        p.state.credits = 1;
        assert!(!p.startup_worker_candidate(2, 3, false));
        assert!(p.startup_worker_candidate(2, 3, true));
        let before = p.clone();
        assert!(!p.reserve_startup_worker(2, 3, true, false, true));
        assert!(!p.reserve_startup_worker(2, 3, true, true, false));
        assert_eq!(p, before);
        assert!(p.reserve_startup_worker(2, 3, true, true, true));
        assert_eq!(p.state.credits, 0);
        assert!(p.state.inflight && p.state.second_attempted);
        assert!(!p.reserve_startup_worker(2, 3, true, true, false));
    }

    #[test]
    fn gate_and_getter_refusals_follow_worker_epoch() {
        assert_eq!(
            owner_call(OwnerLocation::Gate, GATE, true, true),
            OwnerCall::GateEntry
        );
        assert_eq!(
            owner_call(OwnerLocation::Gate, GATE_WAIT, true, true),
            OwnerCall::Continue(OwnerLocation::GateWait)
        );
        assert_eq!(
            owner_call(OwnerLocation::Gate, GETTER, true, true),
            OwnerCall::RefuseGateExit
        );
        assert_eq!(
            owner_call(OwnerLocation::GateWait, GETTER, true, true),
            OwnerCall::RefuseGetter
        );
        for location in [
            OwnerLocation::Gate,
            OwnerLocation::GateWait,
            OwnerLocation::Elsewhere,
        ] {
            assert_eq!(
                owner_call(location, GETTER, false, true),
                OwnerCall::Continue(OwnerLocation::Elsewhere)
            );
            assert_eq!(
                owner_call(location, GETTER, true, false),
                OwnerCall::Continue(OwnerLocation::Elsewhere)
            );
        }
    }

    #[test]
    fn owner_call_c_boundary_preserves_decisions() {
        assert_eq!(tsfp_second_vblank_owner_call(0, GATE, false, false), 4);
        assert_eq!(tsfp_second_vblank_owner_call(1, GATE_WAIT, true, true), 1);
        assert_eq!(tsfp_second_vblank_owner_call(1, GETTER, true, true), 2);
        assert_eq!(tsfp_second_vblank_owner_call(2, GETTER, true, true), 3);
        assert_eq!(tsfp_second_vblank_owner_call(2, GETTER, false, true), 0);
    }

    #[test]
    fn frame_admission_reserves_credits_and_checks_wrapping_counter() {
        let mut p = Policy::default();
        p.state.enabled = true;
        p.state.bound = true;
        p.state.registered = true;
        p.state.owner_handle = 1;
        p.state.owner_fs = 4096;
        assert_eq!(p.frame(1, 4096, true, false, None), FrameAction::First);
        p.state.inflight = false;
        p.state.first_delivered = true;
        p.state.credits = 1;
        p.state.producer_handle = 2;
        p.state.producer_fs = 8192;
        assert_eq!(
            p.frame(1, 4096, true, false, None),
            FrameAction::Second {
                producer_handle: 2,
                producer_fs: 8192
            }
        );
        assert_eq!(p.state.credits, 0);
        assert!(p.state.inflight && p.state.second_attempted);
        p.state.inflight = false;
        p.state.second_delivered = true;
        p.owner_budget = 2;
        assert_eq!(
            p.frame(1, 4096, true, false, None),
            FrameAction::RefuseUnreadable
        );
        assert_eq!(
            p.frame(1, 4096, true, false, Some((0, u32::MAX))),
            FrameAction::Admitted
        );
        assert_eq!(
            p.frame(1, 4096, true, false, Some((u32::MAX, 0))),
            FrameAction::RefuseSpin
        );
        p.poll_blank = true;
        assert_eq!(
            p.frame(1, 4096, true, false, Some((5, 5))),
            FrameAction::PollBlank
        );
        assert!(p.state.inflight);
        assert_eq!(p.state.frames_admitted, 1);
    }
}

/// Result of checking the flip queue after the guest words were read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlipRefusal {
    NotDrained,
    PhantomThreshold,
    Record,
}

fn check_flip_queue_state(
    queued: u64,
    flips: u64,
    producer: u32,
    threshold: u32,
) -> Result<(), FlipRefusal> {
    if producer != flips as u32 || queued != flips {
        return Err(FlipRefusal::NotDrained);
    }
    if queued == 0 && threshold != 0 {
        return Err(FlipRefusal::PhantomThreshold);
    }
    Ok(())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_check_flip_queue_state(
    queued: u64,
    flips: u64,
    producer: u32,
    threshold: u32,
) -> i32 {
    match check_flip_queue_state(queued, flips, producer, threshold) {
        Ok(()) => -1,
        Err(FlipRefusal::NotDrained) => 1,
        Err(FlipRefusal::PhantomThreshold) => 2,
        Err(FlipRefusal::Record) => unreachable!(),
    }
}

/// Read the queue words in order and obtain the record only after the queue passes.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_check_flip_queue_host(
    queued: u64,
    flips: u64,
    count: u32,
    record: *mut core::ffi::c_void,
    read: extern "C" fn(u32, *mut u32) -> bool,
    check_record: extern "C" fn(*mut core::ffi::c_void, u32, u32, u32) -> i32,
) -> i32 {
    let mut producer = 0;
    let mut threshold = 0;
    if !read(0x3e3f60 + 0x1df4, &mut producer) || !read(0x3e3f60 + 0x1dec, &mut threshold) {
        return 0;
    }
    let state = tsfp_second_vblank_check_flip_queue_state(queued, flips, producer, threshold);
    if state != -1 {
        return state;
    }
    check_record(record, count, producer, threshold)
}

/// Validate the observed queue and callback record without reading guest memory.
/// The C boundary is responsible for reporting unreadable guest words separately.
pub fn check_flip_queue(
    queued: u64,
    flips: u64,
    producer: u32,
    threshold: u32,
    count: u32,
    record_count: u32,
    record_index: u32,
    record_flags: u32,
) -> Result<(), FlipRefusal> {
    check_flip_queue_state(queued, flips, producer, threshold)?;
    if tsfp_second_vblank_check_flip_record(
        count,
        producer,
        threshold,
        record_count,
        record_index,
        record_flags,
    ) != -1
    {
        return Err(FlipRefusal::Record);
    }
    Ok(())
}

/// Check the helper record after the queue state has already passed preflight.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_check_flip_record(
    count: u32,
    producer: u32,
    threshold: u32,
    record_count: u32,
    record_index: u32,
    record_flags: u32,
) -> i32 {
    if record_count != count
        || record_index != producer
        || (record_flags == 2 && threshold != count.wrapping_add(1))
    {
        3
    } else {
        -1
    }
}

/// Validate the guest observations required before publishing a blank.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_delivery_scope_valid(
    current_irql: u32,
    pcr_readable: bool,
    pcr_irql: u32,
    device_readable: bool,
    device: u32,
    expected_device: u32,
    counter_readable: bool,
    counter: u32,
    expected_counter: u32,
) -> bool {
    current_irql == 0
        && pcr_readable
        && pcr_irql == 0
        && device_readable
        && device == expected_device
        && counter_readable
        && counter == expected_counter
}

/// The guest callback must advance its title counter exactly once, including wraparound.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_delivery_advanced(
    readable: bool,
    before: u32,
    after: u32,
) -> bool {
    readable && after == before.wrapping_add(1)
}

/// Probe the callback's guest writes in their original order, stopping on failure.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_delivery_probes(
    fs: u32,
    stack_high: u32,
    count: u32,
    counter: u32,
    read_word: extern "C" fn(u32, *mut u32) -> bool,
    write_word: extern "C" fn(u32, u32) -> bool,
    probe_frame: extern "C" fn(u32) -> bool,
    probe_counter: extern "C" fn(u32) -> bool,
) -> bool {
    let mut head = 0;
    read_word(fs, &mut head)
        && write_word(fs, head)
        && probe_frame(stack_high.wrapping_sub(20))
        && write_word(0x3e3f60 + 0x1de8, count)
        && probe_counter(counter)
}

/// Check device bookkeeping before inspecting the optional flip queue.
/// Returns -2 for bookkeeping refusal, -1 for success, or a flip refusal code.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_delivery_preflight(
    count: u32,
    modelled: bool,
    record: *mut core::ffi::c_void,
    bookkeeping: extern "C" fn(u32, bool) -> bool,
    check_flip: extern "C" fn(u32, *mut core::ffi::c_void) -> i32,
) -> i32 {
    if !bookkeeping(count, modelled) {
        return -2;
    }
    if modelled {
        check_flip(count, record)
    } else {
        -1
    }
}

/// Publish the second callback count only while the delivery remains unpoisoned.
/// Returns 0 on success, 1 for a concurrent refusal, and 2 for a failed write.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_publish_second(
    poisoned: bool,
    coupled: bool,
    count: u32,
    write_word: extern "C" fn(u32, u32) -> bool,
) -> u32 {
    if poisoned {
        return 1;
    }
    let published = if coupled { count } else { 3 };
    if write_word(0x3e3f60 + 0x1de8, published) {
        0
    } else {
        2
    }
}

/// The loading worker may proceed once both owner gate prologue stores are visible.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_gate_prologue_visible(
    stage: u32,
    target: u32,
    one: u32,
) -> bool {
    stage == 0xa && target == one
}

/// Validate each second-event observation in guest-read order.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_second_scope_observation(field: u32, value: u32) -> bool {
    match field {
        0 => value == 0x3d66d,
        1 | 2 | 3 => value == 0,
        4 => value == 0x3e3f60,
        5 => value == 1,
        _ => false,
    }
}

/// Read and validate the second callback's scope in guest observation order.
///
/// # Safety
/// `counter_out` must point to writable storage for a u32.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_second_vblank_read_second_scope(
    esp: u32,
    fs: u32,
    irql: u32,
    counter_out: *mut u32,
    read_word: extern "C" fn(u32, *mut u32) -> bool,
    current_irql: extern "C" fn() -> u32,
    read_pcr_irql: extern "C" fn(u32, *mut u32) -> bool,
    read_counter: extern "C" fn(*mut u32) -> bool,
) -> bool {
    let mut value = 0;
    if !read_word(esp, &mut value)
        || !tsfp_second_vblank_second_scope_observation(0, value)
        || !tsfp_second_vblank_second_scope_observation(1, irql)
        || !tsfp_second_vblank_second_scope_observation(2, current_irql())
        || !read_pcr_irql(fs, &mut value)
        || !tsfp_second_vblank_second_scope_observation(3, value)
        || !read_word(0x3e3f58, &mut value)
        || !tsfp_second_vblank_second_scope_observation(4, value)
        || !read_counter(&mut value)
        || !tsfp_second_vblank_second_scope_observation(5, value)
    {
        return false;
    }
    unsafe {
        *counter_out = value;
    }
    true
}

/// Validate one startup-worker observation in the order the host reads it.
/// Keeping each read separate preserves the original short-circuit and diagnostics.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_startup_scope_observation(
    field: u32,
    readable: bool,
    value: u32,
    counter: u32,
    clock_path: bool,
) -> bool {
    if !readable {
        return false;
    }
    match field {
        0 => value == 0,
        1 => value == 0x156cb6,
        2 => value == 1,
        3 => {
            if clock_path {
                value == 0 || value == counter
            } else {
                value == counter
            }
        }
        4 => value == u32::from(!clock_path),
        5 => {
            if clock_path {
                value <= 1
            } else {
                value == 0
            }
        }
        6 => value == u32::from(!clock_path),
        _ => false,
    }
}

/// Read startup observations in order, returning the first failing field (0-6), or 7.
/// The output retains each attempted read for the host's refusal diagnostic.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_second_vblank_startup_scope_read(
    irql: u32,
    esp: u32,
    clock_path: bool,
    values: *mut u32,
    read: extern "C" fn(u32, *mut u32) -> bool,
    read_counter: extern "C" fn(*mut u32) -> bool,
) -> u32 {
    if !tsfp_second_vblank_startup_scope_observation(0, true, irql, 0, clock_path) {
        return 0;
    }
    // SAFETY: the host supplies six writable words for the duration of this call.
    let values = unsafe { std::slice::from_raw_parts_mut(values, 6) };
    let addresses = [esp, 0, 0x7497d8, 0x74a9b0, 0x74a9ac, 0x4e7a94];
    for i in 0..6 {
        let readable = if i == 1 {
            read_counter(&mut values[i])
        } else {
            read(addresses[i], &mut values[i])
        };
        if !tsfp_second_vblank_startup_scope_observation(
            (i + 1) as u32,
            readable,
            values[i],
            values[1],
            clock_path,
        ) {
            return (i + 1) as u32;
        }
    }
    7
}

#[cfg(test)]
mod delivery_scope_tests {
    use super::tsfp_second_vblank_delivery_scope_valid as valid;

    #[test]
    #[allow(unsafe_code)]
    fn startup_scope_reads_in_order_and_stops_at_failure() {
        use super::tsfp_second_vblank_startup_scope_read as check;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn read(address: u32, out: *mut u32) -> bool {
            CALLS.fetch_add(1, Ordering::SeqCst);
            let value = match address {
                0x1000 => 0x156cb6,
                0x7497d8 => 1,
                0x74a9b0 => 1,
                0x74a9ac => 0,
                0x4e7a94 => 1,
                _ => return false,
            };
            // SAFETY: the scope checker passes a writable output word.
            unsafe {
                *out = value;
            }
            true
        }
        extern "C" fn counter(out: *mut u32) -> bool {
            CALLS.fetch_add(1, Ordering::SeqCst);
            // SAFETY: the scope checker passes a writable output word.
            unsafe {
                *out = 1;
            }
            true
        }
        let mut values = [0; 6];
        CALLS.store(0, Ordering::SeqCst);
        assert_eq!(
            unsafe { check(0, 0x1000, false, values.as_mut_ptr(), read, counter) },
            7
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 6);
        CALLS.store(0, Ordering::SeqCst);
        assert_eq!(
            unsafe { check(0, 0x2000, false, values.as_mut_ptr(), read, counter) },
            1
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(
            unsafe { check(1, 0x1000, false, values.as_mut_ptr(), read, counter) },
            0
        );
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn preflight_short_circuits_and_reports_flip_failure() {
        use super::tsfp_second_vblank_delivery_preflight as preflight;
        use std::sync::atomic::{AtomicUsize, Ordering};
        static FLIP_CALLS: AtomicUsize = AtomicUsize::new(0);
        extern "C" fn bookkeeping(count: u32, _: bool) -> bool {
            count != 0
        }
        extern "C" fn flip(_: u32, _: *mut core::ffi::c_void) -> i32 {
            FLIP_CALLS.fetch_add(1, Ordering::SeqCst);
            2
        }
        let record = core::ptr::null_mut();
        assert_eq!(preflight(0, true, record, bookkeeping, flip), -2);
        assert_eq!(preflight(1, false, record, bookkeeping, flip), -1);
        assert_eq!(FLIP_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(preflight(1, true, record, bookkeeping, flip), 2);
        assert_eq!(FLIP_CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn second_publication_preserves_count_and_short_circuits_poison() {
        use super::tsfp_second_vblank_publish_second as publish;
        use std::sync::atomic::{AtomicU32, Ordering};
        static WRITTEN: AtomicU32 = AtomicU32::new(0);
        extern "C" fn write(address: u32, value: u32) -> bool {
            assert_eq!(address, 0x3e3f60 + 0x1de8);
            WRITTEN.store(value, Ordering::SeqCst);
            value != 9
        }
        assert_eq!(publish(true, true, 7, write), 1);
        assert_eq!(WRITTEN.load(Ordering::SeqCst), 0);
        assert_eq!(publish(false, true, 7, write), 0);
        assert_eq!(WRITTEN.load(Ordering::SeqCst), 7);
        assert_eq!(publish(false, false, 7, write), 0);
        assert_eq!(WRITTEN.load(Ordering::SeqCst), 3);
        assert_eq!(publish(false, true, 9, write), 2);
    }

    #[test]
    fn gate_prologue_requires_both_visible_stores() {
        use super::tsfp_second_vblank_gate_prologue_visible as visible;
        assert!(visible(0xa, 0x3f800000, 0x3f800000));
        assert!(!visible(9, 0x3f800000, 0x3f800000));
        assert!(!visible(0xa, 0, 0x3f800000));
    }

    #[test]
    fn second_event_scope_rejects_wrong_observations() {
        use super::tsfp_second_vblank_second_scope_observation as valid;
        for (field, value) in [(0, 0x3d66d), (1, 0), (2, 0), (3, 0), (4, 0x3e3f60), (5, 1)] {
            assert!(valid(field, value));
            assert!(!valid(field, value.wrapping_add(1)));
        }
        assert!(!valid(6, 0));
    }

    #[test]
    fn startup_scope_preserves_clock_and_budgeted_rules() {
        use super::tsfp_second_vblank_startup_scope_observation as check;
        for (field, value) in [
            (0, 0),
            (1, 0x156cb6),
            (2, 1),
            (3, 0),
            (4, 0),
            (5, 1),
            (6, 0),
        ] {
            assert!(check(field, true, value, 1, true));
            assert!(!check(field, false, value, 1, true));
        }
        for (field, value) in [(3, 1), (4, 1), (5, 0), (6, 1)] {
            assert!(check(field, true, value, 1, false));
        }
        assert!(!check(3, true, 0, 1, false));
        assert!(!check(5, true, 2, 1, true));
        assert!(!check(7, true, 0, 1, true));
    }

    #[test]
    fn requires_every_guest_observation() {
        assert!(valid(0, true, 0, true, 4, 4, true, 7, 7));
        assert!(!valid(1, true, 0, true, 4, 4, true, 7, 7));
        assert!(!valid(0, false, 0, true, 4, 4, true, 7, 7));
        assert!(!valid(0, true, 1, true, 4, 4, true, 7, 7));
        assert!(!valid(0, true, 0, false, 4, 4, true, 7, 7));
        assert!(!valid(0, true, 0, true, 5, 4, true, 7, 7));
        assert!(!valid(0, true, 0, true, 4, 4, false, 7, 7));
        assert!(!valid(0, true, 0, true, 4, 4, true, 8, 7));
    }

    #[test]
    fn callback_must_advance_counter_once() {
        use super::tsfp_second_vblank_delivery_advanced as advanced;
        assert!(advanced(true, 7, 8));
        assert!(advanced(true, u32::MAX, 0));
        assert!(!advanced(false, 7, 8));
        assert!(!advanced(true, 7, 7));
        assert!(!advanced(true, 7, 9));
    }
}

/// Stable C boundary for the flip preflight. Values match FLIP_REASON_* in
/// the host, with -1 denoting success. No guest memory crosses this boundary.
#[allow(unsafe_code)] // Exporting the symbol is the only unsafe attribute; the body is safe Rust.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_check_flip_queue(
    queued: u64,
    flips: u64,
    producer: u32,
    threshold: u32,
    count: u32,
    record_count: u32,
    record_index: u32,
    record_flags: u32,
) -> i32 {
    match check_flip_queue(
        queued,
        flips,
        producer,
        threshold,
        count,
        record_count,
        record_index,
        record_flags,
    ) {
        Ok(()) => -1,
        Err(FlipRefusal::NotDrained) => 1,
        Err(FlipRefusal::PhantomThreshold) => 2,
        Err(FlipRefusal::Record) => 3,
    }
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub epoch: u64,
    pub enabled: bool,
    pub inflight: bool,
    pub bound: bool,
    pub registered: bool,
    pub refused: bool,
    pub owner_handle: u32,
    pub owner_fs: u32,
    pub producer_handle: u32,
    pub producer_fs: u32,
    pub credits: u64,
    pub first_delivered: bool,
    pub second_attempted: bool,
    pub second_delivered: bool,
    pub second_via_worker: bool,
    pub owner_delivered: u32,
    pub worker_delivered: u32,
    pub worker_delivered_total: u32,
    pub frames_admitted: u32,
    pub poll_delivered: u32,
    pub worker_held: u32,
    pub worker_final_held: u32,
    pub owner_gate_entries: u32,
}

/// Actions requiring the caller to perform host work outside the policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindOwner {
    Disabled,
    Forward,
    Bound,
    Refuse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registration {
    Ignored,
    Registered,
    RefuseIdentity,
    RefuseDevice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitAction {
    None,
    RecordProducer,
    DeliverOwner,
    DeliverWorker,
    RefuseUnsuccessful,
    RefuseInflight,
    RefuseOwnerBudget,
    RefuseBacklog { describe_later_thread: bool },
}

/// Result of the locked FRAME admission check. Guest reads and callback
/// delivery remain the host's responsibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameAction {
    Ignored,
    First,
    Second {
        producer_handle: u32,
        producer_fs: u32,
    },
    Admitted,
    PollBlank,
    RefuseOwner,
    RefuseUnreadable,
    RefuseSpin,
    RefuseCredit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    pub state: Snapshot,
    pub owner_budget: u32,
    pub worker_budget: u32,
    pub worker_hold_ms: u32,
    pub interactive: bool,
    pub startup_clock: bool,
    pub poll_blank: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            state: Snapshot::default(),
            owner_budget: 0,
            worker_budget: 0,
            worker_hold_ms: DEFAULT_WORKER_HOLD_MS,
            interactive: false,
            startup_clock: false,
            poll_blank: false,
        }
    }
}

impl Policy {
    /// Record host-side milestones after they happen; entering a new gate
    /// starts a fresh worker budget while retaining the run-wide total.
    pub fn record_milestone(&mut self, event: u32) -> bool {
        let s = &mut self.state;
        match event {
            0 => s.poll_delivered = s.poll_delivered.wrapping_add(1),
            1 => s.worker_held = s.worker_held.wrapping_add(1),
            2 => s.worker_final_held = s.worker_final_held.wrapping_add(1),
            3 => {
                s.owner_gate_entries = s.owner_gate_entries.wrapping_add(1);
                s.worker_delivered = 0;
            }
            _ => return false,
        }
        true
    }
    /// Test the locked startup-worker admission before querying the host thread table.
    pub fn startup_worker_candidate(&self, handle: u32, fs: u32, producer_thread: bool) -> bool {
        let s = &self.state;
        let clock_path = self.startup_clock && self.worker_budget == 0;
        let has_credit = s.credits == 1;
        self.interactive
            && (clock_path || self.owner_budget != 0)
            && (self.startup_clock || self.worker_budget != 0)
            && s.enabled
            && s.bound
            && s.registered
            && s.first_delivered
            && !s.second_attempted
            && !s.second_delivered
            && !s.inflight
            && !s.refused
            && s.credits <= 1
            && (has_credit || self.startup_clock)
            && (!has_credit
                || (handle == s.producer_handle && fs == s.producer_fs && producer_thread))
            && handle != s.owner_handle
    }

    /// Recheck the snapshot after waiting for the owner, then consume the credit.
    /// `had_credit` is the value observed at admission, so a changed credit is a race.
    pub fn reserve_startup_worker(
        &mut self,
        handle: u32,
        fs: u32,
        producer_thread: bool,
        owner_blocked: bool,
        had_credit: bool,
    ) -> bool {
        let s = &mut self.state;
        let clock_path = self.startup_clock && self.worker_budget == 0;
        let budgeted_path = self.owner_budget != 0 && self.worker_budget != 0;
        if !self.interactive
            || !(clock_path || budgeted_path)
            || !s.enabled
            || !s.bound
            || !s.registered
            || !s.first_delivered
            || s.second_delivered
            || s.inflight
            || s.refused
            || s.second_attempted
            || s.credits != u64::from(had_credit)
            || handle == s.owner_handle
            || !owner_blocked
            || (had_credit
                && (handle != s.producer_handle || fs != s.producer_fs || !producer_thread))
        {
            return false;
        }
        s.inflight = true;
        s.second_attempted = true;
        s.credits = 0;
        true
    }

    /// Check a FRAME call after the host has waited for any second credit.
    /// `counters` is required only for a later frame; absence means a read
    /// failed. All successful delivery actions reserve the in-flight slot.
    pub fn frame(
        &mut self,
        handle: u32,
        fs: u32,
        owner_thread: bool,
        producer_thread: bool,
        counters: Option<(u32, u32)>,
    ) -> FrameAction {
        let s = &mut self.state;
        if !s.enabled {
            return FrameAction::Ignored;
        }
        if s.refused
            || !s.bound
            || !s.registered
            || s.inflight
            || handle != s.owner_handle
            || fs != s.owner_fs
            || !owner_thread
        {
            return FrameAction::RefuseOwner;
        }
        if !s.first_delivered {
            s.inflight = true;
            return FrameAction::First;
        }
        let clock_only = self.startup_clock && self.worker_budget == 0;
        if clock_only
            && self.interactive
            && !s.second_delivered
            && !s.second_attempted
            && s.credits == 0
        {
            s.frames_admitted = s.frames_admitted.wrapping_add(1);
            return FrameAction::Admitted;
        }
        if s.second_delivered && (self.owner_budget != 0 || clock_only) {
            let Some((counter, last)) = counters else {
                return FrameAction::RefuseUnreadable;
            };
            if counter == last
                && self.poll_blank
                && (self.interactive || s.owner_delivered < self.owner_budget)
            {
                s.inflight = true;
                return FrameAction::PollBlank;
            }
            if counter != last || !clock_only {
                if (counter.wrapping_sub(last) as i32) < 1 {
                    return FrameAction::RefuseSpin;
                }
            }
            s.frames_admitted = s.frames_admitted.wrapping_add(1);
            return FrameAction::Admitted;
        }
        if s.second_attempted || s.credits != 1 || producer_thread {
            return FrameAction::RefuseCredit;
        }
        s.inflight = true;
        s.second_attempted = true;
        s.credits = 0;
        FrameAction::Second {
            producer_handle: s.producer_handle,
            producer_fs: s.producer_fs,
        }
    }

    /// Complete a callback after the host has verified its effects. A refusal
    /// raised during the callback leaves all delivery accounting untouched.
    pub fn complete_delivery(&mut self, worker: bool, startup: bool) -> bool {
        let s = &mut self.state;
        if s.refused {
            return false;
        }
        if startup {
            s.second_delivered = true;
            s.second_via_worker = worker;
        } else if worker {
            s.worker_delivered = s.worker_delivered.wrapping_add(1);
            s.worker_delivered_total = s.worker_delivered_total.wrapping_add(1);
        } else {
            s.owner_delivered = s.owner_delivered.wrapping_add(1);
        }
        s.inflight = false;
        true
    }

    /// Finish either original two-event callback after the host checks it.
    pub fn complete_original_callback(&mut self, first: bool) -> bool {
        let s = &mut self.state;
        if s.refused {
            return false;
        }
        if first {
            s.first_delivered = true;
            s.credits = 0;
        } else {
            s.second_delivered = true;
        }
        s.inflight = false;
        true
    }

    /// A host failure poisons the epoch and releases the in-flight reservation.
    pub fn fail_delivery(&mut self) {
        self.state.inflight = false;
        self.state.refused = true;
    }

    /// `callbacks_present` describes both the invoker and producer query.
    pub fn configure(
        &mut self,
        enabled: bool,
        callbacks_present: bool,
        low: u32,
        high: u32,
    ) -> bool {
        if self.state.inflight
            || self.state.epoch == u64::MAX
            || (enabled
                && (!callbacks_present
                    || low == 0
                    || high <= low
                    || high - low < 128
                    || high & 15 != 0))
        {
            return false;
        }
        let epoch = self.state.epoch + 1;
        *self = Self::default();
        self.state = Snapshot {
            epoch,
            enabled,
            ..Snapshot::default()
        };
        true
    }

    /// Reset preserves configuration, as the C implementation does.
    pub fn reset(&mut self) -> bool {
        if self.state.inflight || self.state.epoch == u64::MAX {
            return false;
        }
        self.state = Snapshot {
            epoch: self.state.epoch + 1,
            enabled: self.state.enabled,
            ..Snapshot::default()
        };
        true
    }

    /// The caller records the host thread identity when `Bound` is returned.
    pub fn bind_owner(&mut self, handle: u32, fs: u32, start: u32, system: u32) -> BindOwner {
        if !self.state.enabled {
            return BindOwner::Disabled;
        }
        if start != OWNER_START || system != OWNER_SYSTEM {
            return BindOwner::Forward;
        }
        if self.state.bound || handle == 0 || fs == 0 || fs & 4095 != 0 {
            return BindOwner::Refuse;
        }
        self.state.bound = true;
        self.state.owner_handle = handle;
        self.state.owner_fs = fs;
        BindOwner::Bound
    }

    /// `device_valid` includes the C path's same-byte write probe; invoke it
    /// only after the identity checks pass and while guest mappings are stable.
    pub fn registration_identity_valid(
        &self,
        callback: u32,
        handle: u32,
        fs: u32,
        owner_thread: bool,
    ) -> bool {
        !self.state.inflight
            && !self.state.registered
            && self.state.bound
            && callback == CALLBACK
            && handle == self.state.owner_handle
            && fs == self.state.owner_fs
            && owner_thread
    }

    pub fn note_registration(
        &mut self,
        callback: u32,
        handle: u32,
        fs: u32,
        owner_thread: bool,
        device_valid: bool,
    ) -> Registration {
        if !self.state.enabled {
            return Registration::Ignored;
        }
        let result = if !self.registration_identity_valid(callback, handle, fs, owner_thread) {
            Registration::RefuseIdentity
        } else if !device_valid {
            Registration::RefuseDevice
        } else {
            self.state.registered = true;
            return Registration::Registered;
        };
        self.state.refused = true;
        result
    }

    /// The caller supplies the host-thread and worker-identity observations;
    /// `RecordProducer` requires it to retain the current host thread identity.
    pub fn note_wait_completed(
        &mut self,
        success: bool,
        handle: u32,
        fs: u32,
        owner_thread: bool,
        worker_identity: bool,
    ) -> WaitAction {
        let startup_clock = self.worker_budget == 0 && self.startup_clock;
        let worker = (self.worker_budget != 0 || startup_clock) && worker_identity;
        let s = &mut self.state;
        if !s.enabled {
            return WaitAction::None;
        }
        let action = if !success {
            WaitAction::RefuseUnsuccessful
        } else if s.inflight {
            WaitAction::RefuseInflight
        } else if s.first_delivered
            && self.owner_budget != 0
            && s.second_delivered
            && !s.refused
            && handle != 0
            && fs != 0
            && handle == s.owner_handle
            && fs == s.owner_fs
            && owner_thread
        {
            if !self.interactive && s.owner_delivered >= self.owner_budget {
                WaitAction::RefuseOwnerBudget
            } else {
                s.inflight = true;
                return WaitAction::DeliverOwner;
            }
        } else if worker
            && s.first_delivered
            && (self.owner_budget != 0 || startup_clock)
            && s.second_delivered
            && !s.refused
            && !owner_thread
            && handle != s.owner_handle
            && (handle != s.producer_handle || s.second_via_worker)
        {
            return WaitAction::DeliverWorker;
        } else if startup_clock
            && self.interactive
            && self.owner_budget == 0
            && s.bound
            && s.registered
            && s.first_delivered
            && !s.refused
            && !s.inflight
            && s.credits == 0
            && handle != 0
            && fs != 0
            && handle == s.owner_handle
            && fs == s.owner_fs
            && owner_thread
        {
            return WaitAction::None;
        } else if s.first_delivered {
            if s.inflight
                || s.second_attempted
                || s.credits != 0
                || handle == 0
                || fs == 0
                || fs & 4095 != 0
                || handle == s.owner_handle
                || fs == s.owner_fs
                || owner_thread
            {
                WaitAction::RefuseBacklog {
                    describe_later_thread: s.second_delivered
                        && self.owner_budget != 0
                        && !owner_thread,
                }
            } else {
                s.credits = 1;
                s.producer_handle = handle;
                s.producer_fs = fs;
                return WaitAction::RecordProducer;
            }
        } else {
            return WaitAction::None;
        };
        s.refused = true;
        action
    }

    pub fn set_owner_waits(&mut self, budget: u32) -> bool {
        if self.state.inflight
            || self.state.epoch == u64::MAX
            || budget > MAX_BLANKS
            || (budget != 0 && !self.state.enabled)
        {
            return false;
        }
        self.owner_budget = budget;
        true
    }

    pub fn set_worker_blanks(&mut self, budget: u32) -> bool {
        if self.state.inflight
            || self.state.epoch == u64::MAX
            || budget > MAX_BLANKS
            || (budget != 0 && (!self.state.enabled || self.owner_budget == 0))
        {
            return false;
        }
        self.worker_budget = budget;
        true
    }

    pub fn set_interactive(&mut self, enabled: bool) -> bool {
        if self.state.inflight || (enabled && !self.state.enabled) {
            return false;
        }
        self.interactive = enabled;
        true
    }

    pub fn set_startup_clock(&mut self, enabled: bool) -> bool {
        if self.state.inflight || (enabled && (!self.interactive || !self.state.enabled)) {
            return false;
        }
        self.startup_clock = enabled;
        true
    }

    pub fn set_poll_blank(&mut self, enabled: bool) -> bool {
        if self.state.inflight
            || self.state.epoch == u64::MAX
            || (enabled && self.owner_budget == 0)
        {
            return false;
        }
        self.poll_blank = enabled;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_entry_resets_episode_budget_and_preserves_run_counters() {
        let mut p = Policy::default();
        p.state.worker_delivered = 3;
        p.state.worker_delivered_total = 9;
        for event in 0..4 {
            assert!(p.record_milestone(event));
        }
        assert_eq!(
            (
                p.state.poll_delivered,
                p.state.worker_held,
                p.state.worker_final_held
            ),
            (1, 1, 1)
        );
        assert_eq!(
            (
                p.state.owner_gate_entries,
                p.state.worker_delivered,
                p.state.worker_delivered_total
            ),
            (1, 0, 9)
        );
        let previous = p.clone();
        assert!(!p.record_milestone(4));
        assert_eq!(p, previous);
    }

    #[test]
    fn delivery_completion_accounts_only_successful_callbacks() {
        let mut p = Policy::default();
        p.state.inflight = true;
        assert!(p.complete_delivery(true, true));
        assert!(p.state.second_delivered && p.state.second_via_worker);
        assert_eq!(p.state.worker_delivered_total, 0);
        p.state.inflight = true;
        assert!(p.complete_delivery(true, false));
        assert_eq!(
            (p.state.worker_delivered, p.state.worker_delivered_total),
            (1, 1)
        );
        p.state.inflight = true;
        p.fail_delivery();
        let before = p.state.clone();
        assert!(!p.complete_delivery(false, false));
        assert_eq!(p.state, before);
    }

    #[test]
    fn original_callback_completion_preserves_poisoned_state() {
        let mut p = Policy::default();
        p.state.inflight = true;
        p.state.credits = 1;
        assert!(p.complete_original_callback(true));
        assert!(p.state.first_delivered && !p.state.inflight);
        assert_eq!(p.state.credits, 0);
        p.state.inflight = true;
        assert!(p.complete_original_callback(false));
        assert!(p.state.second_delivered && !p.state.inflight);
        p.state.inflight = true;
        p.state.refused = true;
        let before = p.state.clone();
        assert!(!p.complete_original_callback(true));
        assert_eq!(p.state, before);
    }

    #[test]
    fn flip_queue_checks_draining_threshold_and_record() {
        assert_eq!(tsfp_second_vblank_check_flip_queue_state(2, 3, 3, 0), 1);
        assert_eq!(tsfp_second_vblank_check_flip_queue_state(0, 0, 0, 1), 2);
        assert_eq!(tsfp_second_vblank_check_flip_queue_state(3, 3, 3, 0), -1);
        assert_eq!(check_flip_queue(0, 0, 0, 0, 7, 7, 0, 0), Ok(()));
        assert_eq!(
            check_flip_queue(2, 3, 3, 0, 7, 7, 3, 0),
            Err(FlipRefusal::NotDrained)
        );
        assert_eq!(
            check_flip_queue(0, 0, 0, 1, 7, 7, 0, 0),
            Err(FlipRefusal::PhantomThreshold)
        );
        assert_eq!(
            check_flip_queue(3, 3, 3, 7, 7, 7, 3, 2),
            Err(FlipRefusal::Record)
        );
        assert_eq!(
            check_flip_queue(3, 3, 3, 0, u32::MAX, u32::MAX, 3, 2),
            Ok(())
        );
        assert_eq!(
            tsfp_second_vblank_check_flip_queue(0, 0, 0, 0, 7, 7, 0, 0),
            -1
        );
        assert_eq!(
            tsfp_second_vblank_check_flip_queue(2, 3, 3, 0, 7, 7, 3, 0),
            1
        );
        assert_eq!(
            tsfp_second_vblank_check_flip_queue(0, 0, 0, 1, 7, 7, 0, 0),
            2
        );
        assert_eq!(
            tsfp_second_vblank_check_flip_queue(3, 3, 3, 7, 7, 7, 3, 2),
            3
        );
    }

    #[test]
    fn configuration_rejections_leave_state_unchanged() {
        let mut p = Policy::default();
        let before = p.clone();
        assert!(!p.configure(true, true, 0x1000, 0x1070));
        assert_eq!(p, before);
        assert!(p.configure(true, true, 0x1000, 0x1080));
        p.state.inflight = true;
        let before = p.clone();
        assert!(!p.reset());
        assert!(!p.configure(false, false, 0, 0));
        assert_eq!(p, before);
    }

    #[test]
    fn dependencies_and_epoch_reset_follow_c_policy() {
        let mut p = Policy::default();
        assert!(!p.set_worker_blanks(1));
        assert!(p.configure(true, true, 0x1000, 0x1080));
        assert!(!p.set_startup_clock(true));
        assert!(!p.set_poll_blank(true));
        assert!(p.set_owner_waits(1));
        assert!(p.set_worker_blanks(MAX_BLANKS));
        assert!(!p.set_worker_blanks(MAX_BLANKS + 1));
        assert!(p.set_interactive(true));
        assert!(p.set_startup_clock(true));
        assert!(p.set_poll_blank(true));
        assert!(p.reset());
        assert_eq!(p.state.epoch, 2);
        assert_eq!(p.worker_budget, MAX_BLANKS);
        assert!(p.startup_clock && p.poll_blank);
        assert!(p.configure(false, false, 0, 0));
        assert_eq!(p.worker_budget, 0);
        assert!(!p.startup_clock && !p.poll_blank);
    }

    #[test]
    fn owner_and_registration_retain_refusal_semantics() {
        let mut p = Policy::default();
        assert_eq!(
            p.bind_owner(1, 0x1000, OWNER_START, OWNER_SYSTEM),
            BindOwner::Disabled
        );
        assert!(p.configure(true, true, 0x1000, 0x1080));
        assert_eq!(p.bind_owner(1, 0x1000, 0, OWNER_SYSTEM), BindOwner::Forward);
        assert_eq!(
            p.bind_owner(1, 0x1001, OWNER_START, OWNER_SYSTEM),
            BindOwner::Refuse
        );
        assert!(!p.state.refused); // bind refusal does not poison the C snapshot
        assert_eq!(
            p.bind_owner(1, 0x1000, OWNER_START, OWNER_SYSTEM),
            BindOwner::Bound
        );
        assert!(!p.registration_identity_valid(CALLBACK, 1, 0x1000, false));
        assert_eq!(
            p.note_registration(CALLBACK, 1, 0x1000, true, false),
            Registration::RefuseDevice
        );
        assert!(p.state.refused);
        assert_eq!(
            p.note_registration(CALLBACK, 1, 0x1000, true, true),
            Registration::Registered
        );
        assert_eq!(
            p.note_registration(CALLBACK, 1, 0x1000, true, true),
            Registration::RefuseIdentity
        );
        assert!(p.reset());
        assert!(!p.state.bound && !p.state.registered && !p.state.refused);
    }

    #[test]
    fn wait_credit_and_delivery_decisions() {
        let mut p = Policy::default();
        assert!(p.configure(true, true, 0x1000, 0x1080));
        assert_eq!(
            p.bind_owner(1, 0x1000, OWNER_START, OWNER_SYSTEM),
            BindOwner::Bound
        );
        assert_eq!(
            p.note_wait_completed(true, 2, 0x2000, false, false),
            WaitAction::None
        );
        p.state.first_delivered = true;
        assert_eq!(
            p.note_wait_completed(true, 2, 0x2000, false, false),
            WaitAction::RecordProducer
        );
        assert_eq!(p.state.credits, 1);
        assert_eq!(
            p.note_wait_completed(true, 2, 0x2000, false, false),
            WaitAction::RefuseBacklog {
                describe_later_thread: false
            }
        );
        assert!(p.state.refused);
        assert!(p.reset());
        assert_eq!(
            p.bind_owner(1, 0x1000, OWNER_START, OWNER_SYSTEM),
            BindOwner::Bound
        );
        assert!(p.set_owner_waits(1));
        p.state.first_delivered = true;
        p.state.second_delivered = true;
        assert_eq!(
            p.note_wait_completed(true, 1, 0x1000, true, false),
            WaitAction::DeliverOwner
        );
        assert_eq!(
            p.note_wait_completed(true, 1, 0x1000, true, false),
            WaitAction::RefuseInflight
        );
        p.state.inflight = false;
        p.state.refused = false;
        p.state.owner_delivered = 1;
        assert_eq!(
            p.note_wait_completed(true, 1, 0x1000, true, false),
            WaitAction::RefuseOwnerBudget
        );
    }
}
