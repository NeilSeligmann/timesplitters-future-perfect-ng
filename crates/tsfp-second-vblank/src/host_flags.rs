//! Atomic host-side announcement state for the startup clock.
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

// The C fallback keeps its pthread mutex; the Rust path owns the same critical
// section until the remaining host callbacks have moved across the boundary.
static HOST_LOCK: Mutex<()> = Mutex::new(());
static OWNER_THREAD: Mutex<Option<std::thread::ThreadId>> = Mutex::new(None);
static PRODUCER_THREAD: Mutex<Option<std::thread::ThreadId>> = Mutex::new(None);

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_is_owner_thread() -> bool {
    *OWNER_THREAD.lock().unwrap_or_else(|e| e.into_inner()) == Some(std::thread::current().id())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_is_producer_thread() -> bool {
    *PRODUCER_THREAD.lock().unwrap_or_else(|e| e.into_inner()) == Some(std::thread::current().id())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_owner_is_producer() -> bool {
    let owner = *OWNER_THREAD.lock().unwrap_or_else(|e| e.into_inner());
    let producer = *PRODUCER_THREAD.lock().unwrap_or_else(|e| e.into_inner());
    owner.is_some() && owner == producer
}
thread_local! {
    static HOST_GUARD: RefCell<Option<MutexGuard<'static, ()>>> = const { RefCell::new(None) };
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_lock() {
    HOST_GUARD.with(|slot| {
        let mut guard = slot.borrow_mut();
        assert!(guard.is_none(), "recursive vblank host lock");
        *guard = Some(HOST_LOCK.lock().unwrap_or_else(|e| e.into_inner()));
    });
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_unlock() {
    HOST_GUARD.with(|slot| {
        assert!(
            slot.borrow_mut().take().is_some(),
            "unheld vblank host lock"
        );
    });
}

static OWNER_CLOCK_ANNOUNCED: AtomicBool = AtomicBool::new(false);
static WORKER_BUDGET: AtomicU32 = AtomicU32::new(0);
static OWNER_ID: AtomicU32 = AtomicU32::new(0);
static WORKER_ID: AtomicU32 = AtomicU32::new(0);
static HELD_ID: AtomicU32 = AtomicU32::new(0);
static OWNER_LOCATION: AtomicU32 = AtomicU32::new(0);
static OWNER_BUDGET: AtomicU32 = AtomicU32::new(0);
static INTERACTIVE: AtomicBool = AtomicBool::new(false);
static BOUND: AtomicBool = AtomicBool::new(false);
static REGISTERED: AtomicBool = AtomicBool::new(false);
static FIRST_DELIVERED: AtomicBool = AtomicBool::new(false);
static SECOND_ATTEMPTED: AtomicBool = AtomicBool::new(false);
static SECOND_DELIVERED: AtomicBool = AtomicBool::new(false);
static INFLIGHT: AtomicBool = AtomicBool::new(false);
static REFUSED: AtomicBool = AtomicBool::new(false);
static SECOND_VIA_WORKER: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(false);
static STARTUP_CLOCK: AtomicBool = AtomicBool::new(false);
static POLL_BLANK: AtomicBool = AtomicBool::new(false);
static HOST_EPOCH: AtomicU64 = AtomicU64::new(0);
static WORKER_HELD: AtomicU32 = AtomicU32::new(0);
static WORKER_FINAL_HELD: AtomicU32 = AtomicU32::new(0);
static WORKER_DELIVERED_TOTAL: AtomicU32 = AtomicU32::new(0);
static OWNER_GATE_ENTRIES: AtomicU32 = AtomicU32::new(0);
static POLL_DELIVERED: AtomicU32 = AtomicU32::new(0);
static WORKER_DELIVERED: AtomicU32 = AtomicU32::new(0);
static FRAMES_ADMITTED: AtomicU32 = AtomicU32::new(0);
static OWNER_DELIVERED: AtomicU32 = AtomicU32::new(0);
static CREDITS: AtomicU32 = AtomicU32::new(0);
static OWNER_HANDLE: AtomicU32 = AtomicU32::new(0);
static OWNER_FS: AtomicU32 = AtomicU32::new(0);
static PRODUCER_HANDLE: AtomicU32 = AtomicU32::new(0);
static PRODUCER_FS: AtomicU32 = AtomicU32::new(0);
static STACK_LOW: AtomicU32 = AtomicU32::new(0);
static STACK_HIGH: AtomicU32 = AtomicU32::new(0);
static SHUTDOWN_QUERY: Mutex<Option<extern "C" fn() -> bool>> = Mutex::new(None);
type Invoker = extern "C" fn(u32, u32, u32, *const u32) -> bool;
type Confirm = extern "C" fn(u32, u32) -> bool;
static CALLBACKS: Mutex<(Option<Invoker>, Option<Confirm>)> = Mutex::new((None, None));

// The caller holds the host lock, so both delivery counts belong to one epoch.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_expected_counter(startup: bool) -> u32 {
    if startup {
        1
    } else {
        2u32.wrapping_add(OWNER_DELIVERED.load(Ordering::SeqCst))
            .wrapping_add(WORKER_DELIVERED_TOTAL.load(Ordering::SeqCst))
    }
}

// Clear the old location before the host observes worker activity, as the C
// call hook did. Return the previous location for the subsequent decision.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_owner_call_begin(callee: u32) -> u32 {
    let location = OWNER_LOCATION.load(Ordering::SeqCst);
    if (location == 1 && callee != crate::GATE_WAIT) || location == 2 {
        OWNER_LOCATION.store(0, Ordering::SeqCst);
    }
    location
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_owner_call_finish(
    location: u32,
    callee: u32,
    budgeted: bool,
    active: bool,
) -> u32 {
    let decision = crate::tsfp_second_vblank_owner_call(location, callee, budgeted, active);
    if decision == 1 {
        OWNER_LOCATION.store(2, Ordering::SeqCst);
    }
    decision
}

/// Preserve the location transition before observing the worker's live state.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_owner_call(
    callee: u32,
    budgeted: bool,
    worker_active: extern "C" fn() -> bool,
) -> u32 {
    let location = tsfp_second_vblank_host_owner_call_begin(callee);
    let active = budgeted
        && crate::tsfp_second_vblank_owner_needs_worker_check(location, callee)
        && worker_active();
    tsfp_second_vblank_host_owner_call_finish(location, callee, budgeted, active)
}

// The host holds its vblank lock across these observations and reservation.
fn startup_observations(handle: u32, producer_matches: bool) -> crate::StartupCandidate {
    let load = |v: &AtomicU32| v.load(Ordering::SeqCst);
    let flag = |v: &AtomicBool| v.load(Ordering::SeqCst);
    crate::StartupCandidate {
        worker_budget: load(&WORKER_BUDGET),
        owner_budget: load(&OWNER_BUDGET),
        credits: load(&CREDITS),
        interactive: flag(&INTERACTIVE),
        clock_enabled: flag(&STARTUP_CLOCK),
        enabled: flag(&ENABLED),
        bound: flag(&BOUND),
        registered: flag(&REGISTERED),
        first_delivered: flag(&FIRST_DELIVERED),
        second_attempted: flag(&SECOND_ATTEMPTED),
        second_delivered: flag(&SECOND_DELIVERED),
        inflight: flag(&INFLIGHT),
        refused: flag(&REFUSED),
        producer_matches,
        is_owner: handle == load(&OWNER_HANDLE),
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_startup_candidate(
    handle: u32,
    producer_matches: bool,
) -> bool {
    crate::tsfp_second_vblank_startup_candidate(startup_observations(handle, producer_matches))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_startup_reserve(
    handle: u32,
    producer_matches: bool,
    had_credit: bool,
    owner_blocked: bool,
) -> bool {
    if !crate::tsfp_second_vblank_startup_reservation_allowed(
        startup_observations(handle, producer_matches),
        had_credit,
        owner_blocked,
    ) {
        return false;
    }
    INFLIGHT.store(true, Ordering::SeqCst);
    SECOND_ATTEMPTED.store(true, Ordering::SeqCst);
    CREDITS.store(0, Ordering::SeqCst);
    true
}

// Called with the host lock held; the caller must recheck after reading guest memory.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_owner_clock_candidate(handle: u32, fs: u32) -> u32 {
    let flag = |v: &AtomicBool| v.load(Ordering::SeqCst);
    if WORKER_BUDGET.load(Ordering::SeqCst) != 0
        || !flag(&STARTUP_CLOCK)
        || !flag(&INTERACTIVE)
        || !flag(&ENABLED)
        || !flag(&BOUND)
        || !flag(&REGISTERED)
        || flag(&REFUSED)
        || flag(&INFLIGHT)
        || handle != OWNER_HANDLE.load(Ordering::SeqCst)
        || fs != OWNER_FS.load(Ordering::SeqCst)
    {
        return 0;
    }
    if flag(&SECOND_DELIVERED) {
        2
    } else if flag(&FIRST_DELIVERED)
        && !flag(&SECOND_ATTEMPTED)
        && CREDITS.load(Ordering::SeqCst) == 0
    {
        1
    } else {
        0
    }
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_owner_clock_reserve(
    handle: u32,
    fs: u32,
    startup: bool,
) -> bool {
    if tsfp_second_vblank_host_owner_clock_candidate(handle, fs) != if startup { 1 } else { 2 } {
        return false;
    }
    INFLIGHT.store(true, Ordering::SeqCst);
    if startup {
        SECOND_ATTEMPTED.store(true, Ordering::SeqCst);
    }
    true
}

/// Decide whether frame admission needs guest counters, with the host lock held.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_frame_needs_counters(
    handle: u32,
    fs: u32,
    owner_thread: bool,
) -> bool {
    let flag = |v: &AtomicBool| v.load(Ordering::SeqCst);
    flag(&ENABLED)
        && !flag(&REFUSED)
        && flag(&BOUND)
        && flag(&REGISTERED)
        && !flag(&INFLIGHT)
        && handle == OWNER_HANDLE.load(Ordering::SeqCst)
        && fs == OWNER_FS.load(Ordering::SeqCst)
        && owner_thread
        && flag(&FIRST_DELIVERED)
        && flag(&SECOND_DELIVERED)
        && (OWNER_BUDGET.load(Ordering::SeqCst) != 0
            || (flag(&STARTUP_CLOCK) && WORKER_BUDGET.load(Ordering::SeqCst) == 0))
}

/// Apply frame admission to the live host state under the host vblank lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_frame(
    handle: u32,
    fs: u32,
    owner_thread: bool,
    producer_thread: bool,
    counters_valid: bool,
    counter: u32,
    last: u32,
) -> crate::state_abi::FrameResult {
    let mut p = crate::Policy::default();
    p.owner_budget = OWNER_BUDGET.load(Ordering::SeqCst);
    p.worker_budget = WORKER_BUDGET.load(Ordering::SeqCst);
    p.interactive = INTERACTIVE.load(Ordering::SeqCst);
    p.startup_clock = STARTUP_CLOCK.load(Ordering::SeqCst);
    p.poll_blank = POLL_BLANK.load(Ordering::SeqCst);
    let s = &mut p.state;
    s.enabled = ENABLED.load(Ordering::SeqCst);
    s.refused = REFUSED.load(Ordering::SeqCst);
    s.bound = BOUND.load(Ordering::SeqCst);
    s.registered = REGISTERED.load(Ordering::SeqCst);
    s.inflight = INFLIGHT.load(Ordering::SeqCst);
    s.first_delivered = FIRST_DELIVERED.load(Ordering::SeqCst);
    s.second_delivered = SECOND_DELIVERED.load(Ordering::SeqCst);
    s.second_attempted = SECOND_ATTEMPTED.load(Ordering::SeqCst);
    s.owner_handle = OWNER_HANDLE.load(Ordering::SeqCst);
    s.owner_fs = OWNER_FS.load(Ordering::SeqCst);
    s.producer_handle = PRODUCER_HANDLE.load(Ordering::SeqCst);
    s.producer_fs = PRODUCER_FS.load(Ordering::SeqCst);
    s.credits = CREDITS.load(Ordering::SeqCst).into();
    s.frames_admitted = FRAMES_ADMITTED.load(Ordering::SeqCst);
    s.owner_delivered = OWNER_DELIVERED.load(Ordering::SeqCst);
    let action = p.frame(
        handle,
        fs,
        owner_thread,
        producer_thread,
        counters_valid.then_some((counter, last)),
    );
    INFLIGHT.store(p.state.inflight, Ordering::SeqCst);
    SECOND_ATTEMPTED.store(p.state.second_attempted, Ordering::SeqCst);
    CREDITS.store(p.state.credits as u32, Ordering::SeqCst);
    FRAMES_ADMITTED.store(p.state.frames_admitted, Ordering::SeqCst);
    let (action, producer_handle, producer_fs) = match action {
        crate::FrameAction::Ignored => (0, 0, 0),
        crate::FrameAction::First => (1, 0, 0),
        crate::FrameAction::Second {
            producer_handle,
            producer_fs,
        } => (2, producer_handle, producer_fs),
        crate::FrameAction::Admitted => (3, 0, 0),
        crate::FrameAction::PollBlank => (4, 0, 0),
        crate::FrameAction::RefuseOwner => (5, 0, 0),
        crate::FrameAction::RefuseUnreadable => (6, 0, 0),
        crate::FrameAction::RefuseSpin => (7, 0, 0),
        crate::FrameAction::RefuseCredit => (8, 0, 0),
    };
    crate::state_abi::FrameResult {
        action,
        producer_handle,
        producer_fs,
    }
}

/// Decide whether the owner should keep waiting for the producer's credit under the host lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_await_credit(owner_thread: bool) -> bool {
    let flag = |v: &AtomicBool| v.load(Ordering::SeqCst);
    let late = flag(&ENABLED)
        && flag(&BOUND)
        && flag(&REGISTERED)
        && flag(&FIRST_DELIVERED)
        && !flag(&SECOND_DELIVERED)
        && !flag(&SECOND_ATTEMPTED)
        && !flag(&REFUSED)
        && !flag(&INFLIGHT)
        && CREDITS.load(Ordering::SeqCst) == 0
        && owner_thread;
    let clocked =
        flag(&STARTUP_CLOCK) && flag(&INTERACTIVE) && WORKER_BUDGET.load(Ordering::SeqCst) == 0;
    late && !clocked
}

/// Snapshot of the live host state; the caller holds the host vblank lock.
#[repr(C)]
pub struct HostSnapshot {
    epoch: u64,
    credits: u64,
    owner_handle: u32,
    owner_fs: u32,
    producer_handle: u32,
    producer_fs: u32,
    owner_budget: u32,
    owner_delivered: u32,
    poll_delivered: u32,
    frames_admitted: u32,
    worker_budget: u32,
    worker_delivered: u32,
    worker_delivered_total: u32,
    worker_held: u32,
    worker_final_held: u32,
    owner_gate_entries: u32,
    enabled: bool,
    bound: bool,
    registered: bool,
    first_delivered: bool,
    second_attempted: bool,
    second_delivered: bool,
    inflight: bool,
    refused: bool,
    second_via_worker: bool,
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_snapshot() -> HostSnapshot {
    let load = |v: &AtomicU32| v.load(Ordering::SeqCst);
    let flag = |v: &AtomicBool| v.load(Ordering::SeqCst);
    HostSnapshot {
        epoch: HOST_EPOCH.load(Ordering::SeqCst),
        credits: load(&CREDITS).into(),
        owner_handle: load(&OWNER_HANDLE),
        owner_fs: load(&OWNER_FS),
        producer_handle: load(&PRODUCER_HANDLE),
        producer_fs: load(&PRODUCER_FS),
        owner_budget: load(&OWNER_BUDGET),
        owner_delivered: load(&OWNER_DELIVERED),
        poll_delivered: load(&POLL_DELIVERED),
        frames_admitted: load(&FRAMES_ADMITTED),
        worker_budget: load(&WORKER_BUDGET),
        worker_delivered: load(&WORKER_DELIVERED),
        worker_delivered_total: load(&WORKER_DELIVERED_TOTAL),
        worker_held: load(&WORKER_HELD),
        worker_final_held: load(&WORKER_FINAL_HELD),
        owner_gate_entries: load(&OWNER_GATE_ENTRIES),
        enabled: flag(&ENABLED),
        bound: flag(&BOUND),
        registered: flag(&REGISTERED),
        first_delivered: flag(&FIRST_DELIVERED),
        second_attempted: flag(&SECOND_ATTEMPTED),
        second_delivered: flag(&SECOND_DELIVERED),
        inflight: flag(&INFLIGHT),
        refused: flag(&REFUSED),
        second_via_worker: flag(&SECOND_VIA_WORKER),
    }
}

/// Read a consistent live snapshot using the Rust-owned host lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_snapshot() -> HostSnapshot {
    tsfp_second_vblank_host_lock();
    let snapshot = tsfp_second_vblank_host_snapshot();
    tsfp_second_vblank_host_unlock();
    snapshot
}

/// Validate configuration while the host holds its vblank lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_configure_allowed(
    enabled: bool,
    callbacks_present: bool,
    low: u32,
    high: u32,
) -> bool {
    host_configure_allowed(
        enabled,
        callbacks_present,
        low,
        high,
        INFLIGHT.load(Ordering::SeqCst),
        HOST_EPOCH.load(Ordering::SeqCst),
    )
}

fn host_configure_allowed(
    enabled: bool,
    callbacks: bool,
    low: u32,
    high: u32,
    inflight: bool,
    epoch: u64,
) -> bool {
    !inflight
        && epoch != u64::MAX
        && (!enabled
            || (callbacks && low != 0 && high > low && high - low >= 128 && high & 15 == 0))
}

/// Configure the Rust-owned host state while the caller holds the host lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_configure(
    enabled: bool,
    invoke: Option<Invoker>,
    confirm: Option<Confirm>,
    low: u32,
    high: u32,
) -> bool {
    if !tsfp_second_vblank_host_configure_allowed(
        enabled,
        invoke.is_some() && confirm.is_some(),
        low,
        high,
    ) {
        return false;
    }
    tsfp_second_vblank_host_clear_epoch(true, enabled);
    tsfp_vblank_set_callbacks(invoke, confirm);
    STACK_LOW.store(low, Ordering::SeqCst);
    STACK_HIGH.store(high, Ordering::SeqCst);
    OWNER_BUDGET.store(0, Ordering::SeqCst);
    WORKER_BUDGET.store(0, Ordering::SeqCst);
    POLL_BLANK.store(false, Ordering::SeqCst);
    INTERACTIVE.store(false, Ordering::SeqCst);
    STARTUP_CLOCK.store(false, Ordering::SeqCst);
    tsfp_second_vblank_set_shutdown_query(None);
    crate::state_abi::tsfp_second_vblank_policy_set_worker_hold_ms(600_000);
    tsfp_second_vblank_reset_clock_announcement();
    true
}

/// Begin a new owner gate episode while the host holds its vblank lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_enter_gate() {
    OWNER_GATE_ENTRIES.fetch_add(1, Ordering::SeqCst);
    WORKER_DELIVERED.store(0, Ordering::SeqCst);
}

/// Observe the owner's gate entry in guest read order, then publish its location.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_observe_gate(read: extern "C" fn(u32, *mut u32) -> bool) {
    let mut flag = 1u32;
    let mut running = 0u32;
    let entered =
        read(0x4e7a94, &mut flag) && flag == 0 && read(0x74a9ac, &mut running) && running != 0;
    if entered {
        tsfp_second_vblank_host_lock();
        tsfp_second_vblank_host_enter_gate();
        tsfp_second_vblank_host_unlock();
    }
    OWNER_LOCATION.store(u32::from(entered), Ordering::SeqCst);
}

/// Check whether a worker getter belongs to the protected delivery episode.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_worker_getter_armed(handle: u32) -> bool {
    worker_getter_armed(
        SECOND_DELIVERED.load(Ordering::SeqCst),
        REFUSED.load(Ordering::SeqCst),
        handle,
        PRODUCER_HANDLE.load(Ordering::SeqCst),
    )
}

fn worker_getter_armed(delivered: bool, refused: bool, handle: u32, producer: u32) -> bool {
    delivered && !refused && handle != producer
}

/// Clear per-epoch host state. The caller holds the host vblank lock and has
/// checked that no delivery is in flight and that the epoch can advance.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_clear_epoch(configure: bool, enabled: bool) {
    for flag in [
        &BOUND,
        &REGISTERED,
        &FIRST_DELIVERED,
        &SECOND_ATTEMPTED,
        &SECOND_DELIVERED,
        &INFLIGHT,
        &REFUSED,
        &SECOND_VIA_WORKER,
    ] {
        flag.store(false, Ordering::SeqCst);
    }
    for value in [
        &OWNER_HANDLE,
        &OWNER_FS,
        &PRODUCER_HANDLE,
        &PRODUCER_FS,
        &CREDITS,
        &WORKER_HELD,
        &WORKER_FINAL_HELD,
        &WORKER_DELIVERED_TOTAL,
        &OWNER_GATE_ENTRIES,
        &POLL_DELIVERED,
        &WORKER_DELIVERED,
        &FRAMES_ADMITTED,
        &OWNER_DELIVERED,
        &WORKER_ID,
        &HELD_ID,
        &OWNER_LOCATION,
    ] {
        value.store(0, Ordering::SeqCst);
    }
    if configure {
        OWNER_ID.store(0, Ordering::SeqCst);
    }
    HOST_EPOCH.fetch_add(1, Ordering::SeqCst);
    ENABLED.store(enabled, Ordering::SeqCst);
}

/// Reset the current epoch unless a delivery is in flight or the epoch is exhausted.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_reset() -> bool {
    tsfp_second_vblank_host_lock();
    let allowed = host_reset_allowed(
        INFLIGHT.load(Ordering::SeqCst),
        HOST_EPOCH.load(Ordering::SeqCst),
    );
    if allowed {
        tsfp_second_vblank_host_clear_epoch(false, ENABLED.load(Ordering::SeqCst));
    }
    tsfp_second_vblank_host_unlock();
    allowed
}

fn host_reset_allowed(inflight: bool, epoch: u64) -> bool {
    !inflight && epoch != u64::MAX
}

#[cfg(test)]
#[test]
fn reset_rejects_inflight_delivery_and_exhausted_epoch() {
    assert!(host_reset_allowed(false, u64::MAX - 1));
    assert!(!host_reset_allowed(true, 0));
    assert!(!host_reset_allowed(false, u64::MAX));
}

/// Check the worker delivery budget while the host vblank lock is held.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_worker_exhausted() -> bool {
    worker_exhausted(
        INTERACTIVE.load(Ordering::SeqCst),
        WORKER_DELIVERED.load(Ordering::SeqCst),
        WORKER_BUDGET.load(Ordering::SeqCst),
    )
}

fn worker_exhausted(interactive: bool, delivered: u32, budget: u32) -> bool {
    !interactive && delivered >= budget
}

/// Reserve the worker callback after its hold, under the host vblank lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_reserve_worker(handle: u32, startup: bool) -> bool {
    if REFUSED.load(Ordering::SeqCst) || INFLIGHT.load(Ordering::SeqCst) != startup {
        return false;
    }
    INFLIGHT.store(true, Ordering::SeqCst);
    WORKER_ID.store(handle, Ordering::SeqCst);
    true
}

/// Complete a verified callback under the host vblank lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_complete_original(first: bool) -> bool {
    let mut p = crate::Policy::default();
    p.state.refused = REFUSED.load(Ordering::SeqCst);
    if !p.complete_original_callback(first) {
        return false;
    }
    if first {
        FIRST_DELIVERED.store(true, Ordering::SeqCst);
        CREDITS.store(0, Ordering::SeqCst);
    } else {
        SECOND_DELIVERED.store(true, Ordering::SeqCst);
    }
    INFLIGHT.store(false, Ordering::SeqCst);
    true
}

/// Complete a verified callback under the host vblank lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_complete_delivery(worker: bool, startup: bool) -> bool {
    let mut p = crate::Policy::default();
    p.state.refused = REFUSED.load(Ordering::SeqCst);
    p.state.inflight = INFLIGHT.load(Ordering::SeqCst);
    p.state.second_delivered = SECOND_DELIVERED.load(Ordering::SeqCst);
    p.state.second_via_worker = SECOND_VIA_WORKER.load(Ordering::SeqCst);
    p.state.worker_delivered = WORKER_DELIVERED.load(Ordering::SeqCst);
    p.state.worker_delivered_total = WORKER_DELIVERED_TOTAL.load(Ordering::SeqCst);
    p.state.owner_delivered = OWNER_DELIVERED.load(Ordering::SeqCst);
    if !p.complete_delivery(worker, startup) {
        return false;
    }
    SECOND_DELIVERED.store(p.state.second_delivered, Ordering::SeqCst);
    SECOND_VIA_WORKER.store(p.state.second_via_worker, Ordering::SeqCst);
    WORKER_DELIVERED.store(p.state.worker_delivered, Ordering::SeqCst);
    WORKER_DELIVERED_TOTAL.store(p.state.worker_delivered_total, Ordering::SeqCst);
    OWNER_DELIVERED.store(p.state.owner_delivered, Ordering::SeqCst);
    INFLIGHT.store(p.state.inflight, Ordering::SeqCst);
    true
}

/// Decide and commit a wait completion while the host holds its vblank lock.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_wait_completed(
    success: bool,
    handle: u32,
    fs: u32,
    owner_thread: bool,
    worker_identity: bool,
) -> u32 {
    let mut p = crate::Policy::default();
    p.owner_budget = OWNER_BUDGET.load(Ordering::SeqCst);
    p.worker_budget = WORKER_BUDGET.load(Ordering::SeqCst);
    p.interactive = INTERACTIVE.load(Ordering::SeqCst);
    p.startup_clock = STARTUP_CLOCK.load(Ordering::SeqCst);
    let s = &mut p.state;
    s.enabled = ENABLED.load(Ordering::SeqCst);
    s.bound = BOUND.load(Ordering::SeqCst);
    s.registered = REGISTERED.load(Ordering::SeqCst);
    s.first_delivered = FIRST_DELIVERED.load(Ordering::SeqCst);
    s.second_delivered = SECOND_DELIVERED.load(Ordering::SeqCst);
    s.second_attempted = SECOND_ATTEMPTED.load(Ordering::SeqCst);
    s.second_via_worker = SECOND_VIA_WORKER.load(Ordering::SeqCst);
    s.inflight = INFLIGHT.load(Ordering::SeqCst);
    s.refused = REFUSED.load(Ordering::SeqCst);
    s.owner_handle = OWNER_HANDLE.load(Ordering::SeqCst);
    s.owner_fs = OWNER_FS.load(Ordering::SeqCst);
    s.producer_handle = PRODUCER_HANDLE.load(Ordering::SeqCst);
    s.credits = u64::from(CREDITS.load(Ordering::SeqCst));
    s.owner_delivered = OWNER_DELIVERED.load(Ordering::SeqCst);
    let action = p.note_wait_completed(success, handle, fs, owner_thread, worker_identity);
    INFLIGHT.store(p.state.inflight, Ordering::SeqCst);
    REFUSED.store(p.state.refused, Ordering::SeqCst);
    CREDITS.store(p.state.credits as u32, Ordering::SeqCst);
    PRODUCER_HANDLE.store(p.state.producer_handle, Ordering::SeqCst);
    if action == crate::WaitAction::RecordProducer {
        PRODUCER_FS.store(p.state.producer_fs, Ordering::SeqCst);
        *PRODUCER_THREAD.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(std::thread::current().id());
    }
    match action {
        crate::WaitAction::None => 0,
        crate::WaitAction::RecordProducer => 1,
        crate::WaitAction::DeliverOwner => 2,
        crate::WaitAction::DeliverWorker => 3,
        crate::WaitAction::RefuseUnsuccessful => 4,
        crate::WaitAction::RefuseInflight => 5,
        crate::WaitAction::RefuseOwnerBudget => 6,
        crate::WaitAction::RefuseBacklog {
            describe_later_thread: false,
        } => 7,
        crate::WaitAction::RefuseBacklog {
            describe_later_thread: true,
        } => 8,
    }
}

/// Called while the host lock protects the registration and guest slot probe.
/// Returns 0 when disabled, 1 when the slot must be probed, or 2 on refusal.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_registration_begin(
    callback: u32,
    handle: u32,
    fs: u32,
    owner_thread: bool,
) -> u32 {
    let action = registration_action(
        ENABLED.load(Ordering::SeqCst),
        INFLIGHT.load(Ordering::SeqCst),
        REGISTERED.load(Ordering::SeqCst),
        BOUND.load(Ordering::SeqCst),
        callback == crate::CALLBACK,
        handle == OWNER_HANDLE.load(Ordering::SeqCst),
        fs == OWNER_FS.load(Ordering::SeqCst),
        owner_thread,
    );
    if action == 2 {
        REFUSED.store(true, Ordering::SeqCst);
    }
    action
}

#[allow(clippy::too_many_arguments)]
fn registration_action(
    enabled: bool,
    inflight: bool,
    registered: bool,
    bound: bool,
    callback_ok: bool,
    handle_ok: bool,
    fs_ok: bool,
    owner_thread: bool,
) -> u32 {
    if !enabled {
        0
    } else if inflight
        || registered
        || !bound
        || !callback_ok
        || !handle_ok
        || !fs_ok
        || !owner_thread
    {
        2
    } else {
        1
    }
}

/// Complete the registration only after the host has probed the guest slot.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_registration_finish(valid_slot: bool) {
    if valid_slot {
        REGISTERED.store(true, Ordering::SeqCst);
    } else {
        REFUSED.store(true, Ordering::SeqCst);
    }
}

/// Called under the host vblank lock, which also protects the native thread identity.
/// 0: disabled, 1: forward only, 2: bound, 3: invalid binding.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_bind_host_owner(
    handle: u32,
    fs: u32,
    start: u32,
    system: u32,
) -> u32 {
    let action = bind_owner_action(
        ENABLED.load(Ordering::SeqCst),
        BOUND.load(Ordering::SeqCst),
        handle,
        fs,
        start,
        system,
    );
    if action != 2 {
        return action;
    }
    BOUND.store(true, Ordering::SeqCst);
    OWNER_HANDLE.store(handle, Ordering::SeqCst);
    OWNER_FS.store(fs, Ordering::SeqCst);
    OWNER_ID.store(handle, Ordering::SeqCst);
    *OWNER_THREAD.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::thread::current().id());
    2
}

fn bind_owner_action(
    enabled: bool,
    bound: bool,
    handle: u32,
    fs: u32,
    start: u32,
    system: u32,
) -> u32 {
    if !enabled {
        0
    } else if start != 0x3801d9 || system != 0x37fe1d {
        1
    } else if bound || handle == 0 || fs == 0 || fs & 4095 != 0 {
        3
    } else {
        2
    }
}

/// Called during host startup, before guest threads can mutate vblank state.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_startup_set_interactive(enabled: bool) -> bool {
    if !interactive_allowed(
        enabled,
        INFLIGHT.load(Ordering::SeqCst),
        ENABLED.load(Ordering::SeqCst),
    ) {
        return false;
    }
    INTERACTIVE.store(enabled, Ordering::SeqCst);
    true
}

fn interactive_allowed(requested: bool, inflight: bool, enabled: bool) -> bool {
    !inflight && (!requested || enabled)
}

/// Configure the startup clock before guest threads begin running.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_startup_set_clock(enabled: bool) -> bool {
    if !startup_clock_allowed(
        enabled,
        INFLIGHT.load(Ordering::SeqCst),
        INTERACTIVE.load(Ordering::SeqCst),
        ENABLED.load(Ordering::SeqCst),
    ) {
        return false;
    }
    STARTUP_CLOCK.store(enabled, Ordering::SeqCst);
    true
}

fn startup_clock_allowed(
    requested: bool,
    inflight: bool,
    interactive: bool,
    enabled: bool,
) -> bool {
    !inflight && (!requested || (interactive && enabled))
}

const MAX_STARTUP_BUDGET: u32 = 1_000_000;

fn startup_budget_allowed(budget: u32, inflight: bool, epoch: u64, ready: bool) -> bool {
    !inflight && epoch != u64::MAX && budget <= MAX_STARTUP_BUDGET && (budget == 0 || ready)
}

/// Startup configuration runs before guest threads can change these fields.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_startup_set_owner_waits(budget: u32) -> bool {
    if !startup_budget_allowed(
        budget,
        INFLIGHT.load(Ordering::SeqCst),
        HOST_EPOCH.load(Ordering::SeqCst),
        ENABLED.load(Ordering::SeqCst),
    ) {
        return false;
    }
    OWNER_BUDGET.store(budget, Ordering::SeqCst);
    true
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_startup_set_worker_blanks(budget: u32) -> bool {
    let ready = ENABLED.load(Ordering::SeqCst) && OWNER_BUDGET.load(Ordering::SeqCst) != 0;
    if !startup_budget_allowed(
        budget,
        INFLIGHT.load(Ordering::SeqCst),
        HOST_EPOCH.load(Ordering::SeqCst),
        ready,
    ) {
        return false;
    }
    WORKER_BUDGET.store(budget, Ordering::SeqCst);
    true
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_startup_set_poll_blank(enabled: bool) -> bool {
    if INFLIGHT.load(Ordering::SeqCst)
        || HOST_EPOCH.load(Ordering::SeqCst) == u64::MAX
        || (enabled && OWNER_BUDGET.load(Ordering::SeqCst) == 0)
    {
        return false;
    }
    POLL_BLANK.store(enabled, Ordering::SeqCst);
    true
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_set_callbacks(invoke: Option<Invoker>, confirm: Option<Confirm>) {
    *CALLBACKS.lock().unwrap_or_else(|e| e.into_inner()) = (invoke, confirm);
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_invoke(
    address: u32,
    low: u32,
    high: u32,
    payload: *const u32,
) -> bool {
    let invoke = CALLBACKS.lock().unwrap_or_else(|e| e.into_inner()).0;
    invoke.is_some_and(|f| f(address, low, high, payload))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_vblank_confirm(handle: u32, fs: u32) -> bool {
    let confirm = CALLBACKS.lock().unwrap_or_else(|e| e.into_inner()).1;
    confirm.is_some_and(|f| f(handle, fs))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_epoch_get() -> u64 {
    HOST_EPOCH.load(Ordering::SeqCst)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_epoch_set(epoch: u64) {
    HOST_EPOCH.store(epoch, Ordering::SeqCst);
}

/// Called under the host vblank lock, which serializes configuration changes.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_host_set_configuration(kind: u32, value: u32) -> bool {
    let mut policy = crate::Policy::default();
    policy.state.inflight = INFLIGHT.load(Ordering::SeqCst);
    policy.state.enabled = ENABLED.load(Ordering::SeqCst);
    policy.state.epoch = HOST_EPOCH.load(Ordering::SeqCst);
    policy.owner_budget = OWNER_BUDGET.load(Ordering::SeqCst);
    policy.interactive = INTERACTIVE.load(Ordering::SeqCst);
    let accepted = match kind {
        0 => policy.set_owner_waits(value),
        1 => policy.set_interactive(value != 0),
        2 => policy.set_startup_clock(value != 0),
        3 => policy.set_poll_blank(value != 0),
        4 => policy.set_worker_blanks(value),
        _ => false,
    };
    if accepted {
        match kind {
            0 => OWNER_BUDGET.store(policy.owner_budget, Ordering::SeqCst),
            1 => INTERACTIVE.store(policy.interactive, Ordering::SeqCst),
            2 => STARTUP_CLOCK.store(policy.startup_clock, Ordering::SeqCst),
            3 => POLL_BLANK.store(policy.poll_blank, Ordering::SeqCst),
            4 => WORKER_BUDGET.store(policy.worker_budget, Ordering::SeqCst),
            _ => unreachable!(),
        }
    }
    accepted
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_set_shutdown_query(query: Option<extern "C" fn() -> bool>) {
    *SHUTDOWN_QUERY.lock().unwrap_or_else(|e| e.into_inner()) = query;
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_shutdown_requested() -> bool {
    let query = *SHUTDOWN_QUERY.lock().unwrap_or_else(|e| e.into_inner());
    query.is_some_and(|callback| callback())
}

macro_rules! atomic_accessors {
    ($get:ident, $set:ident, $state:ident) => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn $get() -> u32 {
            $state.load(Ordering::SeqCst)
        }
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn $set(value: u32) {
            $state.store(value, Ordering::SeqCst);
        }
    };
}

atomic_accessors!(tsfp_vblank_owner_id_get, tsfp_vblank_owner_id_set, OWNER_ID);
atomic_accessors!(tsfp_vblank_credits_get, tsfp_vblank_credits_set, CREDITS);
atomic_accessors!(
    tsfp_vblank_stack_low_get,
    tsfp_vblank_stack_low_set,
    STACK_LOW
);
atomic_accessors!(
    tsfp_vblank_stack_high_get,
    tsfp_vblank_stack_high_set,
    STACK_HIGH
);
atomic_accessors!(
    tsfp_vblank_owner_handle_get,
    tsfp_vblank_owner_handle_set,
    OWNER_HANDLE
);
atomic_accessors!(tsfp_vblank_owner_fs_get, tsfp_vblank_owner_fs_set, OWNER_FS);
atomic_accessors!(
    tsfp_vblank_producer_handle_get,
    tsfp_vblank_producer_handle_set,
    PRODUCER_HANDLE
);
atomic_accessors!(
    tsfp_vblank_producer_fs_get,
    tsfp_vblank_producer_fs_set,
    PRODUCER_FS
);
atomic_accessors!(
    tsfp_vblank_worker_delivered_get,
    tsfp_vblank_worker_delivered_set,
    WORKER_DELIVERED
);
atomic_accessors!(
    tsfp_vblank_frames_admitted_get,
    tsfp_vblank_frames_admitted_set,
    FRAMES_ADMITTED
);
atomic_accessors!(
    tsfp_vblank_owner_delivered_get,
    tsfp_vblank_owner_delivered_set,
    OWNER_DELIVERED
);
atomic_accessors!(
    tsfp_vblank_worker_delivered_total_get,
    tsfp_vblank_worker_delivered_total_set,
    WORKER_DELIVERED_TOTAL
);
atomic_accessors!(
    tsfp_vblank_owner_gate_entries_get,
    tsfp_vblank_owner_gate_entries_set,
    OWNER_GATE_ENTRIES
);
atomic_accessors!(
    tsfp_vblank_poll_delivered_get,
    tsfp_vblank_poll_delivered_set,
    POLL_DELIVERED
);
atomic_accessors!(
    tsfp_vblank_worker_held_get,
    tsfp_vblank_worker_held_set,
    WORKER_HELD
);
atomic_accessors!(
    tsfp_vblank_worker_final_held_get,
    tsfp_vblank_worker_final_held_set,
    WORKER_FINAL_HELD
);
atomic_accessors!(
    tsfp_vblank_worker_id_get,
    tsfp_vblank_worker_id_set,
    WORKER_ID
);
atomic_accessors!(tsfp_vblank_held_id_get, tsfp_vblank_held_id_set, HELD_ID);
atomic_accessors!(
    tsfp_vblank_owner_location_get,
    tsfp_vblank_owner_location_set,
    OWNER_LOCATION
);
atomic_accessors!(
    tsfp_vblank_owner_budget_get,
    tsfp_vblank_owner_budget_set,
    OWNER_BUDGET
);

macro_rules! bool_accessors {
    ($get:ident, $set:ident, $state:ident) => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn $get() -> bool {
            $state.load(Ordering::SeqCst)
        }
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn $set(value: bool) {
            $state.store(value, Ordering::SeqCst);
        }
    };
}

bool_accessors!(tsfp_vblank_bound_get, tsfp_vblank_bound_set, BOUND);
bool_accessors!(
    tsfp_vblank_registered_get,
    tsfp_vblank_registered_set,
    REGISTERED
);
bool_accessors!(
    tsfp_vblank_first_delivered_get,
    tsfp_vblank_first_delivered_set,
    FIRST_DELIVERED
);
bool_accessors!(
    tsfp_vblank_second_attempted_get,
    tsfp_vblank_second_attempted_set,
    SECOND_ATTEMPTED
);
bool_accessors!(
    tsfp_vblank_second_delivered_get,
    tsfp_vblank_second_delivered_set,
    SECOND_DELIVERED
);
bool_accessors!(tsfp_vblank_inflight_get, tsfp_vblank_inflight_set, INFLIGHT);
bool_accessors!(tsfp_vblank_refused_get, tsfp_vblank_refused_set, REFUSED);
bool_accessors!(
    tsfp_vblank_second_via_worker_get,
    tsfp_vblank_second_via_worker_set,
    SECOND_VIA_WORKER
);
bool_accessors!(tsfp_vblank_enabled_get, tsfp_vblank_enabled_set, ENABLED);
bool_accessors!(
    tsfp_vblank_interactive_get,
    tsfp_vblank_interactive_set,
    INTERACTIVE
);
bool_accessors!(
    tsfp_vblank_startup_clock_get,
    tsfp_vblank_startup_clock_set,
    STARTUP_CLOCK
);
bool_accessors!(
    tsfp_vblank_poll_blank_get,
    tsfp_vblank_poll_blank_set,
    POLL_BLANK
);

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_worker_budget_get() -> u32 {
    WORKER_BUDGET.load(Ordering::SeqCst)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_worker_budget_set(value: u32) {
    WORKER_BUDGET.store(value, Ordering::SeqCst);
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_reset_clock_announcement() {
    OWNER_CLOCK_ANNOUNCED.store(false, Ordering::SeqCst);
}

/// Returns whether the announcement had already been made.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_mark_clock_announced() -> bool {
    OWNER_CLOCK_ANNOUNCED.swap(true, Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    #[test]
    fn owner_thread_identity_distinguishes_threads() {
        *super::OWNER_THREAD.lock().unwrap() = Some(std::thread::current().id());
        assert!(super::tsfp_second_vblank_is_owner_thread());
        assert!(
            !std::thread::spawn(|| super::tsfp_second_vblank_is_owner_thread())
                .join()
                .unwrap()
        );
        *super::PRODUCER_THREAD.lock().unwrap() = Some(std::thread::current().id());
        assert!(super::tsfp_second_vblank_is_producer_thread());
        assert!(super::tsfp_second_vblank_owner_is_producer());
        std::thread::spawn(|| {
            *super::PRODUCER_THREAD.lock().unwrap() = Some(std::thread::current().id());
            assert!(super::tsfp_second_vblank_is_producer_thread());
        })
        .join()
        .unwrap();
        assert!(!super::tsfp_second_vblank_is_producer_thread());
        assert!(!super::tsfp_second_vblank_owner_is_producer());
    }
    #[test]
    fn host_lock_hands_off_between_threads_and_snapshot_releases_it() {
        use super::{
            tsfp_second_vblank_host_lock, tsfp_second_vblank_host_unlock,
            tsfp_second_vblank_snapshot,
        };
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        tsfp_second_vblank_host_lock();
        let worker = std::thread::spawn(move || {
            ready_tx.send(()).unwrap();
            tsfp_second_vblank_host_lock();
            tsfp_second_vblank_host_unlock();
        });
        ready_rx.recv().unwrap();
        tsfp_second_vblank_host_unlock();
        worker.join().unwrap();
        let _ = tsfp_second_vblank_snapshot();
        tsfp_second_vblank_host_lock();
        tsfp_second_vblank_host_unlock();
    }

    #[test]
    fn registration_checks_identity_and_slot_before_committing() {
        use super::registration_action;
        assert_eq!(
            registration_action(false, true, true, false, false, false, false, false),
            0
        );
        assert_eq!(
            registration_action(true, false, false, true, true, true, true, true),
            1
        );
        assert_eq!(
            registration_action(true, false, false, true, true, true, true, false),
            2
        );
        assert_eq!(
            registration_action(true, true, false, true, true, true, true, true),
            2
        );
        assert_eq!(
            registration_action(true, false, true, true, true, true, true, true),
            2
        );
    }
    #[test]
    fn owner_binding_checks_identity_and_alignment() {
        use super::bind_owner_action;
        let (start, system) = (0x3801d9, 0x37fe1d);
        assert_eq!(bind_owner_action(false, false, 1, 0x1000, start, system), 0);
        assert_eq!(bind_owner_action(true, false, 1, 0x1000, 0, system), 1);
        assert_eq!(bind_owner_action(true, false, 1, 0x1000, start, system), 2);
        assert_eq!(bind_owner_action(true, true, 1, 0x1000, start, system), 3);
        assert_eq!(bind_owner_action(true, false, 0, 0x1000, start, system), 3);
        assert_eq!(bind_owner_action(true, false, 1, 0, start, system), 3);
        assert_eq!(bind_owner_action(true, false, 1, 0x1001, start, system), 3);
    }
    use super::*;

    #[test]
    fn host_epoch_preserves_all_bits() {
        tsfp_second_vblank_host_epoch_set(u64::MAX);
        assert_eq!(tsfp_second_vblank_host_epoch_get(), u64::MAX);
        tsfp_second_vblank_host_epoch_set(0);
        assert_eq!(tsfp_second_vblank_host_epoch_get(), 0);
    }

    #[test]
    fn worker_hold_counts_are_independent_and_resettable() {
        tsfp_vblank_worker_held_set(u32::MAX);
        tsfp_vblank_worker_final_held_set(7);
        assert_eq!(tsfp_vblank_worker_held_get(), u32::MAX);
        assert_eq!(tsfp_vblank_worker_final_held_get(), 7);
        tsfp_vblank_worker_held_set(0);
        tsfp_vblank_worker_final_held_set(0);
        assert_eq!(tsfp_vblank_worker_held_get(), 0);
        assert_eq!(tsfp_vblank_worker_final_held_get(), 0);
    }

    #[test]
    fn run_counters_are_independent_and_resettable() {
        tsfp_vblank_owner_delivered_set(u32::MAX);
        assert_eq!(tsfp_vblank_owner_delivered_get(), u32::MAX);
        tsfp_vblank_owner_delivered_set(0);
        assert_eq!(tsfp_vblank_owner_delivered_get(), 0);
        tsfp_vblank_worker_delivered_set(4);
        tsfp_vblank_frames_admitted_set(5);
        assert_eq!(tsfp_vblank_worker_delivered_get(), 4);
        assert_eq!(tsfp_vblank_frames_admitted_get(), 5);
        tsfp_vblank_worker_delivered_set(0);
        tsfp_vblank_frames_admitted_set(0);
        assert_eq!(tsfp_vblank_worker_delivered_get(), 0);
        assert_eq!(tsfp_vblank_frames_admitted_get(), 0);
        tsfp_vblank_worker_delivered_total_set(u32::MAX);
        tsfp_vblank_owner_gate_entries_set(2);
        tsfp_vblank_poll_delivered_set(3);
        assert_eq!(tsfp_vblank_worker_delivered_total_get(), u32::MAX);
        assert_eq!(tsfp_vblank_owner_gate_entries_get(), 2);
        assert_eq!(tsfp_vblank_poll_delivered_get(), 3);
        tsfp_vblank_worker_delivered_total_set(0);
        tsfp_vblank_owner_gate_entries_set(0);
        tsfp_vblank_poll_delivered_set(0);
        assert_eq!(tsfp_vblank_worker_delivered_total_get(), 0);
        assert_eq!(tsfp_vblank_owner_gate_entries_get(), 0);
        assert_eq!(tsfp_vblank_poll_delivered_get(), 0);
    }

    #[test]
    fn epoch_flags_can_be_set_and_cleared() {
        tsfp_vblank_bound_set(true);
        assert!(tsfp_vblank_bound_get());
        tsfp_vblank_registered_set(true);
        assert!(tsfp_vblank_registered_get());
        tsfp_vblank_first_delivered_set(true);
        assert!(tsfp_vblank_first_delivered_get());
        tsfp_vblank_second_attempted_set(true);
        assert!(tsfp_vblank_second_attempted_get());
        tsfp_vblank_second_delivered_set(true);
        assert!(tsfp_vblank_second_delivered_get());
        tsfp_vblank_inflight_set(true);
        assert!(tsfp_vblank_inflight_get());
        tsfp_vblank_refused_set(true);
        assert!(tsfp_vblank_refused_get());
        tsfp_vblank_second_via_worker_set(true);
        assert!(tsfp_vblank_second_via_worker_get());
        tsfp_vblank_bound_set(false);
        assert!(!tsfp_vblank_bound_get());
        tsfp_vblank_registered_set(false);
        assert!(!tsfp_vblank_registered_get());
        tsfp_vblank_first_delivered_set(false);
        assert!(!tsfp_vblank_first_delivered_get());
        tsfp_vblank_second_attempted_set(false);
        assert!(!tsfp_vblank_second_attempted_get());
        tsfp_vblank_second_delivered_set(false);
        assert!(!tsfp_vblank_second_delivered_get());
        tsfp_vblank_inflight_set(false);
        assert!(!tsfp_vblank_inflight_get());
        tsfp_vblank_refused_set(false);
        assert!(!tsfp_vblank_refused_get());
        tsfp_vblank_second_via_worker_set(false);
        assert!(!tsfp_vblank_second_via_worker_get());
    }

    #[test]
    fn enabled_state_can_be_set_and_cleared() {
        tsfp_vblank_enabled_set(true);
        assert!(tsfp_vblank_enabled_get());
        tsfp_vblank_enabled_set(false);
        assert!(!tsfp_vblank_enabled_get());
    }

    #[test]
    fn worker_getter_requires_delivery_without_refusal_or_producer() {
        assert!(worker_getter_armed(true, false, 2, 1));
        assert!(!worker_getter_armed(false, false, 2, 1));
        assert!(!worker_getter_armed(true, true, 2, 1));
        assert!(!worker_getter_armed(true, false, 1, 1));
    }

    #[test]
    fn host_gate_entry_resets_only_episode_deliveries() {
        OWNER_GATE_ENTRIES.store(u32::MAX, Ordering::SeqCst);
        WORKER_DELIVERED.store(3, Ordering::SeqCst);
        WORKER_DELIVERED_TOTAL.store(7, Ordering::SeqCst);
        tsfp_second_vblank_host_enter_gate();
        assert_eq!(OWNER_GATE_ENTRIES.load(Ordering::SeqCst), 0);
        assert_eq!(WORKER_DELIVERED.load(Ordering::SeqCst), 0);
        assert_eq!(WORKER_DELIVERED_TOTAL.load(Ordering::SeqCst), 7);
        WORKER_DELIVERED_TOTAL.store(0, Ordering::SeqCst);
    }

    #[test]
    fn host_configuration_checks_callbacks_stack_and_epoch() {
        assert!(host_configure_allowed(false, false, 0, 0, false, 0));
        assert!(host_configure_allowed(true, true, 0x1000, 0x1080, false, 0));
        assert!(!host_configure_allowed(
            true, false, 0x1000, 0x1080, false, 0
        ));
        assert!(!host_configure_allowed(true, true, 0, 128, false, 0));
        assert!(!host_configure_allowed(
            true, true, 0x1000, 0x1070, false, 0
        ));
        assert!(!host_configure_allowed(
            true, true, 0x1000, 0x1081, false, 0
        ));
        assert!(!host_configure_allowed(false, false, 0, 0, true, 0));
        assert!(!host_configure_allowed(false, false, 0, 0, false, u64::MAX));
    }

    #[test]
    fn interactive_startup_requires_enabled_and_no_inflight_callback() {
        assert!(interactive_allowed(false, false, false));
        assert!(interactive_allowed(true, false, true));
        assert!(!interactive_allowed(true, false, false));
        assert!(!interactive_allowed(false, true, true));
        assert!(!interactive_allowed(true, true, true));
    }

    #[test]
    fn startup_clock_requires_interactive_enabled_and_no_inflight_callback() {
        assert!(startup_clock_allowed(false, false, false, false));
        assert!(startup_clock_allowed(true, false, true, true));
        assert!(!startup_clock_allowed(true, false, false, true));
        assert!(!startup_clock_allowed(true, false, true, false));
        assert!(!startup_clock_allowed(false, true, true, true));
    }

    #[test]
    fn startup_budget_checks_limits_epoch_and_dependencies() {
        assert!(startup_budget_allowed(0, false, 0, false));
        assert!(startup_budget_allowed(MAX_STARTUP_BUDGET, false, 0, true));
        assert!(!startup_budget_allowed(
            MAX_STARTUP_BUDGET + 1,
            false,
            0,
            true
        ));
        assert!(!startup_budget_allowed(1, false, 0, false));
        assert!(!startup_budget_allowed(0, true, 0, true));
        assert!(!startup_budget_allowed(0, false, u64::MAX, true));
    }

    #[test]
    fn shutdown_query_can_be_installed_and_cleared() {
        extern "C" fn closed() -> bool {
            true
        }
        tsfp_second_vblank_set_shutdown_query(Some(closed));
        assert!(tsfp_second_vblank_shutdown_requested());
        tsfp_second_vblank_set_shutdown_query(None);
        assert!(!tsfp_second_vblank_shutdown_requested());
    }

    #[test]
    fn callbacks_forward_arguments_and_allow_reconfiguration() {
        extern "C" fn invoke(address: u32, low: u32, high: u32, payload: *const u32) -> bool {
            tsfp_vblank_set_callbacks(None, None);
            address == 1 && low == 2 && high == 3 && payload.is_null()
        }
        extern "C" fn confirm(handle: u32, fs: u32) -> bool {
            handle == 4 && fs == 5
        }
        tsfp_vblank_set_callbacks(Some(invoke), Some(confirm));
        assert!(tsfp_vblank_confirm(4, 5));
        assert!(!tsfp_vblank_confirm(5, 4));
        assert!(tsfp_vblank_invoke(1, 2, 3, std::ptr::null()));
        assert!(!tsfp_vblank_confirm(4, 5));
        assert!(!tsfp_vblank_invoke(1, 2, 3, std::ptr::null()));
    }

    #[test]
    fn host_configuration_fields_are_independent() {
        tsfp_vblank_stack_low_set(0x1000);
        tsfp_vblank_stack_high_set(u32::MAX);
        tsfp_vblank_owner_budget_set(u32::MAX);
        tsfp_vblank_interactive_set(true);
        tsfp_vblank_startup_clock_set(false);
        tsfp_vblank_poll_blank_set(true);
        assert_eq!(tsfp_vblank_owner_budget_get(), u32::MAX);
        assert!(tsfp_vblank_interactive_get());
        assert!(!tsfp_vblank_startup_clock_get());
        assert!(tsfp_vblank_poll_blank_get());
        assert_eq!(tsfp_vblank_stack_low_get(), 0x1000);
        assert_eq!(tsfp_vblank_stack_high_get(), u32::MAX);
    }

    #[test]
    fn thread_identity_and_location_are_independent() {
        tsfp_vblank_owner_id_set(11);
        tsfp_vblank_worker_id_set(22);
        tsfp_vblank_held_id_set(33);
        tsfp_vblank_owner_location_set(u32::MAX);
        assert_eq!(tsfp_vblank_owner_id_get(), 11);
        assert_eq!(tsfp_vblank_worker_id_get(), 22);
        assert_eq!(tsfp_vblank_held_id_get(), 33);
        assert_eq!(tsfp_vblank_owner_location_get(), u32::MAX);
    }

    #[test]
    fn worker_reservation_requires_expected_inflight_state() {
        REFUSED.store(false, Ordering::SeqCst);
        INFLIGHT.store(false, Ordering::SeqCst);
        assert!(!tsfp_second_vblank_host_reserve_worker(7, true));
        assert!(tsfp_second_vblank_host_reserve_worker(7, false));
        assert_eq!(WORKER_ID.load(Ordering::SeqCst), 7);
        assert!(!tsfp_second_vblank_host_reserve_worker(8, false));
        assert!(tsfp_second_vblank_host_reserve_worker(8, true));
        REFUSED.store(true, Ordering::SeqCst);
        assert!(!tsfp_second_vblank_host_reserve_worker(9, true));
        REFUSED.store(false, Ordering::SeqCst);
        INFLIGHT.store(false, Ordering::SeqCst);
    }

    #[test]
    fn worker_budget_exhaustion_respects_interactive_mode_and_boundary() {
        assert!(!worker_exhausted(false, 1, 2));
        assert!(worker_exhausted(false, 2, 2));
        assert!(worker_exhausted(false, 0, 0));
        assert!(!worker_exhausted(true, u32::MAX, 0));
    }

    #[test]
    fn worker_budget_preserves_full_value_and_reset() {
        tsfp_second_vblank_worker_budget_set(u32::MAX);
        assert_eq!(tsfp_second_vblank_worker_budget_get(), u32::MAX);
        tsfp_second_vblank_worker_budget_set(0);
        assert_eq!(tsfp_second_vblank_worker_budget_get(), 0);
    }

    #[test]
    fn announcement_occurs_once_per_configuration() {
        tsfp_second_vblank_reset_clock_announcement();
        assert!(!tsfp_second_vblank_mark_clock_announced());
        assert!(tsfp_second_vblank_mark_clock_announced());
        tsfp_second_vblank_reset_clock_announcement();
        assert!(!tsfp_second_vblank_mark_clock_announced());
    }
}
