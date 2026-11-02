//! T3934: the T371 two-consumer quiescence rule for vblank delivery, in readable Rust.
//!
//! Replaces the pure predicate of `src/host/recomp_vblank_quiescence.c` (see docs/vblank-delivery.md "T371 results").
//! The predicate is a function of one snapshot of the guest thread table, never wall time or host thread timing. It only
//! observes: it never pokes the counter and never infers a host thread's guest writes.

pub const GETTER: u32 = 0x22030;
pub const OWNER_RETURN: u32 = 0x153975;
pub const WORKER_RETURN: u32 = 0x156CB6;
pub const OWNER_START: u32 = 0x3801D9;
pub const WORKER_START: u32 = 0x156CB0;
/// The title network library's 50 ms polling thread (T424) and the title's loader thread (T593): neither can reach the
/// getter nor create a reader thread, so `evaluate` skips them in every state.
pub const NON_READER_NET_POLL_START: u32 = 0x3C0FE0;
pub const NON_READER_LOADER_START: u32 = 0x30160;
pub const DEFAULT_NON_READERS: [u32; 2] = [NON_READER_NET_POLL_START, NON_READER_LOADER_START];

/// Thread table capacity of the host (`KERNEL_THREAD_MAX`).
pub const THREAD_MAX: usize = 64;

/// Wait kinds of `kernel_thread_block_state`.
pub const BLOCK_THREAD: u32 = 1;
pub const BLOCK_HOST_TIMER: u32 = 2;
pub const BLOCK_EVENT: u32 = 3;

/// The fields of one guest thread record the rule reads; the C host fills it from its thread table.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ThreadView {
    pub handle: u32,
    pub start_routine: u32,
    pub block_target: u32,
    pub block_state: u32,
    pub suspend_count: u32,
    pub in_use: bool,
    pub terminated: bool,
    pub finished: bool,
    pub started: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum State {
    /// Created or suspended, cannot run until another thread resumes it.
    Unstarted = 0,
    /// Guest termination confirmed (PsTerminateSystemThread).
    Terminated,
    /// Finished by a host stop or fault, never confirmed as a guest exit.
    HostStopped,
    /// Untimed wait on a thread's termination or an event.
    Blocked,
    /// Sleeping to a host deadline, wakes by wall time.
    HostTimer,
    /// Started, not finished, in no recorded wait.
    Runnable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Reason {
    Holds = 0,
    /// The delivering thread has no live started record.
    DelivererUnknown,
    /// Another thread runs and could sample the counter.
    Runnable,
    /// Another thread wakes at a host deadline.
    HostTimer,
    /// Another thread ended by a host stop, not a guest exit.
    HostStopped,
    /// Blocked on a thread that is itself not quiescent.
    BlockedOnActive,
}

impl State {
    pub fn name(self) -> &'static str {
        [
            "unstarted",
            "terminated",
            "host-stopped",
            "blocked",
            "host-timer",
            "runnable",
        ][self as usize]
    }
}
impl Reason {
    pub fn name(self) -> &'static str {
        match self {
            Reason::Holds => "holds",
            Reason::DelivererUnknown => "delivering thread has no live started record",
            Reason::Runnable => "another guest thread is runnable and could sample the counter",
            Reason::HostTimer => "another guest thread sleeps to a host deadline",
            Reason::HostStopped => "another guest thread ended by a host stop, not a guest exit",
            Reason::BlockedOnActive => {
                "another guest thread is blocked on a thread that is not quiescent"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub holds: bool,
    pub reason: Reason,
    /// The first refusing thread (0 when holds) and what it was started at.
    pub handle: u32,
    pub start_routine: u32,
    /// Threads examined besides the deliverer, a guard against an empty pass.
    pub examined: u32,
}

pub fn classify(thread: &ThreadView) -> State {
    if thread.terminated {
        State::Terminated
    } else if thread.finished {
        State::HostStopped
    } else if !thread.started || thread.suspend_count != 0 {
        State::Unstarted
    } else {
        match thread.block_state {
            BLOCK_THREAD | BLOCK_EVENT => State::Blocked,
            BLOCK_HOST_TIMER => State::HostTimer,
            _ => State::Runnable,
        }
    }
}

/// A thread reaching the getter must be a registered reader at its registered return address.
pub fn is_registered_reader(return_address: u32, start_routine: u32) -> bool {
    (return_address == OWNER_RETURN && start_routine == OWNER_START)
        || (return_address == WORKER_RETURN && start_routine == WORKER_START)
}

/// The predicate over one consistent snapshot. Every thread other than `deliverer` and other than a thread whose start
/// routine is in `non_readers` must be terminated, unstarted, or blocked on `deliverer` or a parked thread.
/// `parked_handles` are threads the caller proves parked by location while they are runnable (T592).
pub fn evaluate(
    threads: &[ThreadView],
    deliverer: u32,
    non_readers: &[u32],
    parked_handles: &[u32],
) -> Verdict {
    let mut verdict = Verdict {
        holds: false,
        reason: Reason::DelivererUnknown,
        handle: 0,
        start_routine: 0,
        examined: 0,
    };
    let threads = &threads[..threads.len().min(THREAD_MAX)];
    let self_runnable = threads
        .iter()
        .find(|t| t.in_use && t.handle == deliverer)
        .is_some_and(|t| classify(t) == State::Runnable);
    if !self_runnable {
        return verdict;
    }
    let skipped = |t: &ThreadView| {
        !t.in_use || t.handle == deliverer || non_readers.contains(&t.start_routine)
    };
    // Parked: unstarted, or blocked on the deliverer or on a parked thread (least fixed point, a wait cycle never enters
    // it). A thread blocked on a terminated target wakes at a host timing moment, so it is not parked.
    let mut parked: Vec<bool> = threads
        .iter()
        .map(|t| {
            t.in_use
                && t.handle != deliverer
                && parked_handles.contains(&t.handle)
                && classify(t) == State::Runnable
        })
        .collect();
    loop {
        let mut changed = false;
        for i in 0..threads.len() {
            if parked[i] || skipped(&threads[i]) {
                continue;
            }
            let now = match classify(&threads[i]) {
                State::Unstarted => true,
                State::Blocked => {
                    let target = threads[i].block_target;
                    target == deliverer
                        || threads
                            .iter()
                            .zip(&parked)
                            .any(|(t, &p)| p && t.handle == target)
                }
                _ => false,
            };
            if now {
                parked[i] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    verdict.holds = true;
    verdict.reason = Reason::Holds;
    for (thread, &is_parked) in threads.iter().zip(&parked) {
        if skipped(thread) {
            continue;
        }
        verdict.examined += 1;
        let state = classify(thread);
        // keep counting, the first refusing thread stays the one named
        if state == State::Terminated || is_parked || !verdict.holds {
            continue;
        }
        verdict.holds = false;
        verdict.handle = thread.handle;
        verdict.start_routine = thread.start_routine;
        verdict.reason = match state {
            State::Runnable => Reason::Runnable,
            State::HostTimer => Reason::HostTimer,
            State::HostStopped => Reason::HostStopped,
            _ => Reason::BlockedOnActive,
        };
    }
    verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(handle: u32, start: u32) -> ThreadView {
        ThreadView {
            handle,
            start_routine: start,
            in_use: true,
            started: true,
            ..ThreadView::default()
        }
    }
    fn blocked_on(handle: u32, target: u32) -> ThreadView {
        ThreadView {
            block_state: BLOCK_THREAD,
            block_target: target,
            ..thread(handle, 0x1000)
        }
    }

    #[test]
    fn classification_order() {
        let mut t = thread(1, 0);
        assert_eq!(classify(&t), State::Runnable);
        t.block_state = BLOCK_EVENT;
        assert_eq!(classify(&t), State::Blocked);
        t.block_state = BLOCK_HOST_TIMER;
        assert_eq!(classify(&t), State::HostTimer);
        t.suspend_count = 1;
        assert_eq!(classify(&t), State::Unstarted);
        t.finished = true;
        assert_eq!(classify(&t), State::HostStopped);
        t.terminated = true;
        assert_eq!(classify(&t), State::Terminated);
    }

    #[test]
    fn unknown_or_idle_deliverer_refuses() {
        let v = evaluate(&[thread(1, 0)], 2, &DEFAULT_NON_READERS, &[]);
        assert_eq!((v.holds, v.reason), (false, Reason::DelivererUnknown));
        let mut d = thread(1, 0);
        d.block_state = BLOCK_HOST_TIMER;
        assert!(!evaluate(&[d], 1, &DEFAULT_NON_READERS, &[]).holds);
    }

    #[test]
    fn runnable_other_refuses_and_is_named() {
        let v = evaluate(
            &[thread(1, OWNER_START), thread(2, WORKER_START)],
            1,
            &DEFAULT_NON_READERS,
            &[],
        );
        assert_eq!(
            (v.holds, v.reason, v.handle, v.start_routine),
            (false, Reason::Runnable, 2, WORKER_START)
        );
        assert_eq!(v.examined, 1);
    }

    #[test]
    fn non_readers_are_skipped_in_every_state() {
        let mut net = thread(3, NON_READER_NET_POLL_START);
        net.block_state = BLOCK_HOST_TIMER;
        let v = evaluate(
            &[
                thread(1, OWNER_START),
                net,
                thread(4, NON_READER_LOADER_START),
            ],
            1,
            &DEFAULT_NON_READERS,
            &[],
        );
        assert!(v.holds);
        assert_eq!(v.examined, 0);
    }

    #[test]
    fn blocked_chain_parks_to_a_fixed_point_and_cycles_do_not() {
        let d = thread(1, OWNER_START);
        let held = evaluate(&[d, blocked_on(2, 1), blocked_on(3, 2)], 1, &[], &[]);
        assert!(held.holds);
        let cycle = evaluate(&[d, blocked_on(2, 3), blocked_on(3, 2)], 1, &[], &[]);
        assert_eq!(
            (cycle.holds, cycle.reason),
            (false, Reason::BlockedOnActive)
        );
        let mut gone = thread(3, 0x2000);
        gone.terminated = true;
        let on_dead = evaluate(&[d, blocked_on(2, 3), gone], 1, &[], &[]);
        assert_eq!(on_dead.reason, Reason::BlockedOnActive);
    }

    #[test]
    fn caller_proven_parked_runnable_thread_counts_as_parked() {
        let v = evaluate(
            &[thread(1, OWNER_START), thread(2, WORKER_START)],
            1,
            &[],
            &[2],
        );
        assert!(v.holds);
        assert_eq!(v.examined, 1);
    }

    #[test]
    fn reader_census() {
        assert!(is_registered_reader(OWNER_RETURN, OWNER_START));
        assert!(is_registered_reader(WORKER_RETURN, WORKER_START));
        assert!(!is_registered_reader(OWNER_RETURN, WORKER_START));
    }
}
