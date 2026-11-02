// SPDX-License-Identifier: GPL-3.0-or-later
//! Recursive device lock. Recursive because the sampled host provider may reconcile hotplug from
//! inside a GetState callback while the same thread already holds the lock.

use std::sync::{Condvar, Mutex, PoisonError};
use std::thread::{self, ThreadId};

#[derive(Default)]
pub struct DeviceLock {
    owner: Mutex<(Option<ThreadId>, u32)>,
    released: Condvar,
}

impl DeviceLock {
    pub const fn new() -> Self {
        Self {
            owner: Mutex::new((None, 0)),
            released: Condvar::new(),
        }
    }

    pub fn lock(&self) {
        let me = thread::current().id();
        let mut state = self.owner.lock().unwrap_or_else(PoisonError::into_inner);
        while state.0.is_some_and(|owner| owner != me) {
            state = self
                .released
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        *state = (Some(me), state.1 + 1);
    }

    /// Release one level. False when the calling thread does not hold the lock.
    pub fn unlock(&self) -> bool {
        let me = thread::current().id();
        let mut state = self.owner.lock().unwrap_or_else(PoisonError::into_inner);
        if state.0 != Some(me) {
            return false;
        }
        state.1 -= 1;
        if state.1 == 0 {
            state.0 = None;
            self.released.notify_one();
        }
        true
    }

    /// Current recursion depth of the calling thread (0 when not held).
    pub fn depth(&self) -> u32 {
        let state = self.owner.lock().unwrap_or_else(PoisonError::into_inner);
        if state.0 == Some(thread::current().id()) {
            state.1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn recursive_and_exclusive() {
        let lock = Arc::new(DeviceLock::new());
        lock.lock();
        lock.lock();
        assert_eq!(lock.depth(), 2);
        let other = Arc::clone(&lock);
        let foreign = thread::spawn(move || other.unlock()).join().unwrap();
        assert!(!foreign);
        assert!(lock.unlock() && lock.unlock());
        assert!(!lock.unlock());
        let other = Arc::clone(&lock);
        thread::spawn(move || {
            other.lock();
            assert!(other.unlock());
        })
        .join()
        .unwrap();
    }
}
