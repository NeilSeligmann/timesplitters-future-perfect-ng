//! Stateful host contract. Host observations and callback delivery remain at
//! the boundary; all policy mutations here are serialized in Rust.
use std::sync::{LazyLock, Mutex};

use crate::{BindOwner, FrameAction, Policy, Registration, Snapshot, WaitAction};

static POLICY: LazyLock<Mutex<Policy>> = LazyLock::new(|| Mutex::new(Policy::default()));

#[repr(C)]
pub struct Configuration {
    pub owner_budget: u32,
    pub worker_budget: u32,
    pub worker_hold_ms: u32,
    pub interactive: bool,
    pub startup_clock: bool,
    pub poll_blank: bool,
}

fn with_policy<T>(f: impl FnOnce(&mut Policy) -> T) -> T {
    // A panic cannot normally cross the C boundary. Recovering poison keeps
    // subsequent callers from panicking across it as well.
    let mut policy = POLICY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut policy)
}

/// Return a consistent copy while holding the policy lock. The returned value
/// is independent of later changes and does not expose Rust-owned storage.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_snapshot() -> Snapshot {
    with_policy(|p| p.state.clone())
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_configuration_snapshot() -> Configuration {
    with_policy(|p| Configuration {
        owner_budget: p.owner_budget,
        worker_budget: p.worker_budget,
        worker_hold_ms: p.worker_hold_ms,
        interactive: p.interactive,
        startup_clock: p.startup_clock,
        poll_blank: p.poll_blank,
    })
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_set_worker_hold_ms(milliseconds: u32) {
    with_policy(|p| p.worker_hold_ms = milliseconds);
}

/// Milestones: successful poll delivery, worker held, final hold, gate entry.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_record_milestone(event: u32) -> bool {
    with_policy(|p| p.record_milestone(event))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_configure(
    enabled: bool,
    callbacks_present: bool,
    low: u32,
    high: u32,
) -> bool {
    with_policy(|p| p.configure(enabled, callbacks_present, low, high))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_reset() -> bool {
    with_policy(Policy::reset)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_set_owner_waits(budget: u32) -> bool {
    with_policy(|p| p.set_owner_waits(budget))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_set_worker_blanks(budget: u32) -> bool {
    with_policy(|p| p.set_worker_blanks(budget))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_set_interactive(enabled: bool) -> bool {
    with_policy(|p| p.set_interactive(enabled))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_set_startup_clock(enabled: bool) -> bool {
    with_policy(|p| p.set_startup_clock(enabled))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_set_poll_blank(enabled: bool) -> bool {
    with_policy(|p| p.set_poll_blank(enabled))
}

/// 0 disabled, 1 forward to first-vblank, 2 bound, 3 refuse.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_bind_owner(
    handle: u32,
    fs: u32,
    start: u32,
    system: u32,
) -> u32 {
    with_policy(|p| match p.bind_owner(handle, fs, start, system) {
        BindOwner::Disabled => 0,
        BindOwner::Forward => 1,
        BindOwner::Bound => 2,
        BindOwner::Refuse => 3,
    })
}

/// Check identity before the host probes the guest callback slot. The host
/// must serialize this check and registration against other policy calls.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_registration_identity_valid(
    callback: u32,
    handle: u32,
    fs: u32,
    owner_thread: bool,
) -> bool {
    with_policy(|p| p.registration_identity_valid(callback, handle, fs, owner_thread))
}

/// 0 ignored, 1 registered, 2 identity refusal, 3 device refusal.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_note_registration(
    callback: u32,
    handle: u32,
    fs: u32,
    owner_thread: bool,
    device_valid: bool,
) -> u32 {
    with_policy(
        |p| match p.note_registration(callback, handle, fs, owner_thread, device_valid) {
            Registration::Ignored => 0,
            Registration::Registered => 1,
            Registration::RefuseIdentity => 2,
            Registration::RefuseDevice => 3,
        },
    )
}

/// Returns an action for the host to perform after the policy lock is released.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_note_wait_completed(
    success: bool,
    handle: u32,
    fs: u32,
    owner_thread: bool,
    worker_identity: bool,
) -> u32 {
    with_policy(|p| {
        match p.note_wait_completed(success, handle, fs, owner_thread, worker_identity) {
            WaitAction::None => 0,
            WaitAction::RecordProducer => 1,
            WaitAction::DeliverOwner => 2,
            WaitAction::DeliverWorker => 3,
            WaitAction::RefuseUnsuccessful => 4,
            WaitAction::RefuseInflight => 5,
            WaitAction::RefuseOwnerBudget => 6,
            WaitAction::RefuseBacklog {
                describe_later_thread: false,
            } => 7,
            WaitAction::RefuseBacklog {
                describe_later_thread: true,
            } => 8,
        }
    })
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_complete_delivery(worker: bool, startup: bool) -> bool {
    with_policy(|p| p.complete_delivery(worker, startup))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_complete_original_callback(first: bool) -> bool {
    with_policy(|p| p.complete_original_callback(first))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_fail_delivery() {
    with_policy(Policy::fail_delivery)
}

/// Frame action and the producer identity needed for a second delivery.
#[repr(C)]
pub struct FrameResult {
    pub action: u32,
    pub producer_handle: u32,
    pub producer_fs: u32,
}

/// Actions: ignored, first, second, admitted, poll blank, owner refusal,
/// unreadable counters, spin refusal, credit refusal (0 through 8).
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_frame(
    handle: u32,
    fs: u32,
    owner_thread: bool,
    producer_thread: bool,
    counters_valid: bool,
    counter: u32,
    last: u32,
) -> FrameResult {
    with_policy(|p| {
        let counters = counters_valid.then_some((counter, last));
        let (action, producer_handle, producer_fs) =
            match p.frame(handle, fs, owner_thread, producer_thread, counters) {
                FrameAction::Ignored => (0, 0, 0),
                FrameAction::First => (1, 0, 0),
                FrameAction::Second {
                    producer_handle,
                    producer_fs,
                } => (2, producer_handle, producer_fs),
                FrameAction::Admitted => (3, 0, 0),
                FrameAction::PollBlank => (4, 0, 0),
                FrameAction::RefuseOwner => (5, 0, 0),
                FrameAction::RefuseUnreadable => (6, 0, 0),
                FrameAction::RefuseSpin => (7, 0, 0),
                FrameAction::RefuseCredit => (8, 0, 0),
            };
        FrameResult {
            action,
            producer_handle,
            producer_fs,
        }
    })
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_startup_worker_candidate(
    handle: u32,
    fs: u32,
    producer_thread: bool,
) -> bool {
    with_policy(|p| p.startup_worker_candidate(handle, fs, producer_thread))
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_policy_reserve_startup_worker(
    handle: u32,
    fs: u32,
    producer_thread: bool,
    owner_blocked: bool,
    had_credit: bool,
) -> bool {
    with_policy(|p| {
        p.reserve_startup_worker(handle, fs, producer_thread, owner_blocked, had_credit)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OWNER_START, OWNER_SYSTEM};

    #[test]
    fn host_contract_preserves_configuration_and_epoch_rules() {
        assert!(tsfp_second_vblank_policy_configure(
            true, true, 0x1000, 0x1100
        ));
        let initial = tsfp_second_vblank_policy_snapshot();
        assert!(initial.enabled);
        assert!(!initial.bound);
        assert!(!tsfp_second_vblank_policy_set_worker_blanks(1));
        assert!(tsfp_second_vblank_policy_set_owner_waits(2));
        assert!(tsfp_second_vblank_policy_set_worker_blanks(1));
        tsfp_second_vblank_policy_set_worker_hold_ms(42);
        let config = tsfp_second_vblank_policy_configuration_snapshot();
        assert_eq!(
            (
                config.owner_budget,
                config.worker_budget,
                config.worker_hold_ms
            ),
            (2, 1, 42)
        );
        assert_eq!(
            tsfp_second_vblank_policy_bind_owner(1, 0x1000, OWNER_START, OWNER_SYSTEM),
            2
        );
        let bound = tsfp_second_vblank_policy_snapshot();
        assert!(bound.bound);
        assert_eq!((bound.owner_handle, bound.owner_fs), (1, 0x1000));
        assert!(!initial.bound);
        assert_eq!(
            tsfp_second_vblank_policy_bind_owner(1, 0x1000, OWNER_START, OWNER_SYSTEM),
            3
        );
        assert!(tsfp_second_vblank_policy_reset());
        let config = tsfp_second_vblank_policy_configuration_snapshot();
        assert_eq!(
            (
                config.owner_budget,
                config.worker_budget,
                config.worker_hold_ms
            ),
            (2, 1, 42)
        );
        assert_eq!(
            tsfp_second_vblank_policy_bind_owner(1, 0x1000, OWNER_START, OWNER_SYSTEM),
            2
        );
        assert!(!tsfp_second_vblank_policy_registration_identity_valid(
            crate::CALLBACK,
            1,
            0x1000,
            false
        ));
        assert!(tsfp_second_vblank_policy_registration_identity_valid(
            crate::CALLBACK,
            1,
            0x1000,
            true
        ));
        assert_eq!(
            tsfp_second_vblank_policy_note_registration(crate::CALLBACK, 1, 0x1000, true, true),
            1
        );
        let frame = tsfp_second_vblank_policy_frame(1, 0x1000, true, false, false, 0, 0);
        assert_eq!(
            (frame.action, frame.producer_handle, frame.producer_fs),
            (1, 0, 0)
        );
        assert!(!tsfp_second_vblank_policy_startup_worker_candidate(
            2, 0x2000, true
        ));
        assert!(!tsfp_second_vblank_policy_reserve_startup_worker(
            2, 0x2000, true, true, false
        ));
        assert_eq!(
            tsfp_second_vblank_policy_note_wait_completed(false, 1, 0x1000, true, false),
            4
        );
        assert!(!tsfp_second_vblank_policy_complete_delivery(false, false));
        tsfp_second_vblank_policy_fail_delivery();
        assert!(tsfp_second_vblank_policy_configure(false, false, 0, 0));
    }
}
