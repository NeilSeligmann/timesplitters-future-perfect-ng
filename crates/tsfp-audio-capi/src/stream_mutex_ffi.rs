// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned serialization for the stream host's synchronous callbacks.
//!
//! The C host retains its pthread mutex. On the Rust audio-host build, C's
//! existing lock/unlock sites enter this mutex instead, so Create and the
//! other stream operations continue to share one lock across the FFI boundary.

use std::cell::RefCell;
use std::sync::{Mutex, MutexGuard};

static STREAM_MUTEX: Mutex<()> = Mutex::new(());
static RESET_MUTEX: Mutex<()> = Mutex::new(());

thread_local! {
    static HELD_STREAM_GUARD: RefCell<Option<MutexGuard<'static, ()>>> = const { RefCell::new(None) };
    static HELD_RESET_GUARD: RefCell<Option<MutexGuard<'static, ()>>> = const { RefCell::new(None) };
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_mutex_lock_rust() {
    let guard = STREAM_MUTEX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    HELD_STREAM_GUARD.with(|held| {
        debug_assert!(held.borrow().is_none(), "stream mutex is not recursive");
        *held.borrow_mut() = Some(guard);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_mutex_unlock_rust() {
    HELD_STREAM_GUARD.with(|held| {
        drop(held.borrow_mut().take());
    });
}

/// Serialize reset independently from stream callbacks, matching the two C mutexes.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_reset_mutex_lock_rust() {
    let guard = RESET_MUTEX
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    HELD_RESET_GUARD.with(|held| {
        debug_assert!(held.borrow().is_none(), "reset mutex is not recursive");
        *held.borrow_mut() = Some(guard);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_reset_mutex_unlock_rust() {
    HELD_RESET_GUARD.with(|held| {
        drop(held.borrow_mut().take());
    });
}

/// Abort callback used by the Rust audio host's device-lease transaction.
///
/// The lease invokes this after a failed prepare while the stream mutex is
/// still held. Keep the callback in Rust so this lock-lifetime edge does not
/// return through a C adapter.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_create_abort_unlock_rust(_userdata: *mut std::ffi::c_void) {
    tsfp_audio_stream_mutex_unlock_rust();
}

#[cfg(test)]
mod tests {
    use super::{
        tsfp_audio_stream_create_abort_unlock_rust as abort_unlock,
        tsfp_audio_stream_mutex_lock_rust as lock, tsfp_audio_stream_mutex_unlock_rust as unlock,
        tsfp_audio_stream_reset_mutex_lock_rust as reset_lock,
        tsfp_audio_stream_reset_mutex_unlock_rust as reset_unlock,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn lock_excludes_other_threads_until_the_callback_scope_finishes() {
        lock();
        let entered = Arc::new(AtomicBool::new(false));
        let thread_entered = Arc::clone(&entered);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).expect("started receiver");
            lock();
            thread_entered.store(true, Ordering::SeqCst);
            unlock();
        });
        started_rx.recv().expect("worker started");
        assert!(!entered.load(Ordering::SeqCst));
        unlock();
        worker.join().expect("stream mutex worker");
        assert!(entered.load(Ordering::SeqCst));
    }

    #[test]
    fn rust_abort_callback_releases_the_create_lock() {
        lock();
        abort_unlock(std::ptr::null_mut());
        lock();
        unlock();
    }

    #[test]
    fn reset_mutex_serializes_resets_without_aliasing_stream_mutex() {
        reset_lock();
        lock();
        unlock();
        let entered = Arc::new(AtomicBool::new(false));
        let thread_entered = Arc::clone(&entered);
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).expect("started receiver");
            reset_lock();
            thread_entered.store(true, Ordering::SeqCst);
            reset_unlock();
        });
        started_rx.recv().expect("worker started");
        assert!(!entered.load(Ordering::SeqCst));
        reset_unlock();
        worker.join().expect("reset worker");
        assert!(entered.load(Ordering::SeqCst));
    }
}
