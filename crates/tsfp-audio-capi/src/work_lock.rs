// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared Rust-owned exclusion between the refill worker and guest-thread audio rendering.

use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard, TryLockError};

static AUDIO_WORK_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    // C's blocking lock/unlock pair runs on the same guest thread. Keep the Rust guard on that
    // thread so the critical section can span the synchronous host render call safely.
    static C_AUDIO_WORK_GUARD: RefCell<Option<MutexGuard<'static, ()>>> = const { RefCell::new(None) };
}

fn lock_unpoisoned() -> MutexGuard<'static, ()> {
    AUDIO_WORK_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn lock_blocking() -> MutexGuard<'static, ()> {
    lock_unpoisoned()
}

pub(crate) fn try_lock() -> Option<MutexGuard<'static, ()>> {
    match AUDIO_WORK_LOCK.try_lock() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_work_lock() {
    let guard = lock_unpoisoned();
    C_AUDIO_WORK_GUARD.with(|slot| {
        let previous = slot.replace(Some(guard));
        debug_assert!(previous.is_none(), "audio work lock is not reentrant");
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_audio_runtime_work_unlock() {
    C_AUDIO_WORK_GUARD.with(|slot| {
        let guard = slot.take();
        debug_assert!(
            guard.is_some(),
            "audio work lock must be released by its owner"
        );
        drop(guard);
    });
}

#[cfg(test)]
mod tests {
    use super::{dsound_audio_runtime_work_lock, dsound_audio_runtime_work_unlock, try_lock};

    #[test]
    fn rust_work_lock_serializes_guest_and_worker_threads() {
        dsound_audio_runtime_work_lock();
        assert!(
            std::thread::spawn(|| try_lock().is_none())
                .join()
                .expect("worker lock probe thread")
        );
        dsound_audio_runtime_work_unlock();

        let guard = try_lock().expect("lock is released after guest render");
        drop(guard);
    }
}
