//! Recursive transaction exclusion without recursive mutable references.
//!
//! Foreign `run_locked` callbacks hold only the gate. Each ordinary operation
//! acquires that gate and then the data mutex; a callback may therefore call an
//! ordinary operation safely, while another thread cannot observe the middle of
//! its transaction. A data guard must never be held across such a callback.
use crate::runtime::Runtime;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::{Condvar, LockResult, Mutex, MutexGuard, PoisonError, TryLockError};
use std::thread::{self, ThreadId};

#[derive(Default)]
struct Ownership { owner: Option<ThreadId>, depth: usize }

#[derive(Default)]
struct Gate { ownership: Mutex<Ownership>, changed: Condvar }

pub struct Transaction<'a> { gate: &'a Gate, not_send: PhantomData<Rc<()>> }
impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        let mut ownership = self.gate.ownership.lock().unwrap_or_else(PoisonError::into_inner);
        debug_assert_eq!(ownership.owner, Some(thread::current().id()));
        debug_assert!(ownership.depth > 0);
        ownership.depth -= 1;
        if ownership.depth == 0 {
            ownership.owner = None;
            self.gate.changed.notify_one();
        }
    }
}
impl Gate {
    fn acquire(&self) -> Transaction<'_> {
        let current = thread::current().id();
        let mut ownership = self.ownership.lock().unwrap_or_else(PoisonError::into_inner);
        while ownership.owner.is_some_and(|owner| owner != current) {
            ownership = self.changed.wait(ownership).unwrap_or_else(PoisonError::into_inner);
        }
        ownership.owner = Some(current);
        ownership.depth = ownership.depth.checked_add(1).expect("transaction nesting overflow");
        Transaction { gate: self, not_send: PhantomData }
    }
    fn try_acquire(&self) -> Option<Transaction<'_>> {
        let mut ownership = match self.ownership.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => return None,
        };
        let current = thread::current().id();
        if ownership.owner.is_some_and(|owner| owner != current) { return None; }
        ownership.owner = Some(current);
        ownership.depth = ownership.depth.checked_add(1).expect("transaction nesting overflow");
        Some(Transaction { gate: self, not_send: PhantomData })
    }
}

pub struct RuntimeCell { gate: Gate, data: Mutex<Runtime> }
impl RuntimeCell {
    pub fn new(runtime: Runtime) -> Self { Self { gate: Gate::default(), data: Mutex::new(runtime) } }
    pub fn transaction(&self) -> Transaction<'_> { self.gate.acquire() }
    pub fn try_transaction(&self) -> Option<Transaction<'_>> { self.gate.try_acquire() }
    pub fn lock(&self) -> LockResult<RuntimeGuard<'_>> {
        let transaction = self.transaction();
        match self.data.lock() {
            Ok(data) => Ok(RuntimeGuard { data, transaction }),
            Err(error) => Err(PoisonError::new(RuntimeGuard { data: error.into_inner(), transaction })),
        }
    }
}

pub struct RuntimeGuard<'a> {
    // Drop order matters: release the data before allowing the next transaction.
    data: MutexGuard<'a, Runtime>,
    #[allow(dead_code)]
    transaction: Transaction<'a>,
}
impl Deref for RuntimeGuard<'_> { type Target = Runtime; fn deref(&self) -> &Runtime { &self.data } }
impl DerefMut for RuntimeGuard<'_> { fn deref_mut(&mut self) -> &mut Runtime { &mut self.data } }

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};

    #[test]
    fn same_thread_transaction_can_call_an_ordinary_operation() {
        let cell = RuntimeCell::new(Runtime::default());
        let outer = cell.transaction();
        // The nonblocking probe detects accidental nonrecursive gates immediately.
        let inner = cell.try_transaction().expect("same-thread transaction must be recursive");
        cell.lock().unwrap().missing_empty = true;
        drop(inner);
        assert!(cell.lock().unwrap().missing_empty);
        drop(outer);
        assert!(cell.try_transaction().is_some());
    }

    #[test]
    fn other_threads_cannot_enter_until_outermost_transaction_ends() {
        let cell = Arc::new(RuntimeCell::new(Runtime::default()));
        let outer = cell.transaction();
        let inner = cell.transaction();
        let (requests, receiver) = mpsc::channel::<()>();
        let (results, replies) = mpsc::channel();
        let worker_cell = Arc::clone(&cell);
        let worker = thread::spawn(move || {
            for _ in receiver {
                results.send(worker_cell.try_transaction().is_some()).unwrap();
            }
        });
        requests.send(()).unwrap(); assert!(!replies.recv().unwrap());
        drop(inner);
        requests.send(()).unwrap(); assert!(!replies.recv().unwrap());
        drop(outer);
        requests.send(()).unwrap(); assert!(replies.recv().unwrap());
        drop(requests); worker.join().unwrap();
    }
}
