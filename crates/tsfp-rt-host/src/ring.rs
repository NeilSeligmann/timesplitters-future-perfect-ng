// SPDX-License-Identifier: GPL-3.0-or-later
//! Reusable, nonblocking batch slots. Publication is release/acquire; a full queue
//! returns immediately. Neither producer nor consumer waits for the other.
use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::*};

struct Slot {
    bytes: Vec<u8>,
    binary: bool,
}
pub(crate) struct Ring {
    slots: Vec<UnsafeCell<Slot>>,
    head: AtomicUsize,
    tail: AtomicUsize,
    writing: AtomicBool,
    reading: AtomicBool,
    closed: AtomicBool,
}
// SAFETY: writer exclusivity is enforced by `writing`, reader by `reading`.
// A slot is written only before head publication, read only before tail release.
unsafe impl Sync for Ring {}
impl Ring {
    pub(crate) fn new(slots: usize, capacity: usize) -> Arc<Self> {
        // Power-of-two indexing stays valid when monotonically increasing cursors wrap.
        let slots = slots.max(2).next_power_of_two();
        Arc::new(Self {
            slots: (0..slots)
                .map(|_| {
                    UnsafeCell::new(Slot {
                        bytes: Vec::with_capacity(capacity),
                        binary: true,
                    })
                })
                .collect(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            writing: AtomicBool::new(false),
            reading: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        })
    }
    pub(crate) fn close(&self) {
        self.closed.store(true, Release);
    }
    pub(crate) fn drained(&self) -> bool {
        self.closed.load(Acquire) && self.head.load(Acquire) == self.tail.load(Acquire)
    }
    pub(crate) fn push(&self, bytes: &[u8], binary: bool) -> bool {
        if self
            .writing
            .compare_exchange(false, true, Acquire, Relaxed)
            .is_err()
        {
            return false;
        }
        let head = self.head.load(Relaxed);
        let room = !self.closed.load(Acquire)
            && head.wrapping_sub(self.tail.load(Acquire)) < self.slots.len();
        if room {
            // SAFETY: this writer owns the unpublished slot; the reader has released it.
            let slot = unsafe { &mut *self.slots[head % self.slots.len()].get() };
            slot.bytes.clear();
            slot.bytes.extend_from_slice(bytes); // one memcpy; capacity reused after warmup
            slot.binary = binary;
            self.head.store(head.wrapping_add(1), Release);
        }
        self.writing.store(false, Release);
        room
    }
    pub(crate) fn pop(self: &Arc<Self>) -> Option<Read> {
        if self
            .reading
            .compare_exchange(false, true, Acquire, Relaxed)
            .is_err()
        {
            return None;
        }
        let tail = self.tail.load(Relaxed);
        if tail == self.head.load(Acquire) {
            self.reading.store(false, Release);
            return None;
        }
        Some(Read {
            ring: self.clone(),
            tail,
        })
    }
}
pub(crate) struct Read {
    ring: Arc<Ring>,
    tail: usize,
}
impl Read {
    pub(crate) fn bytes(&self) -> &[u8] {
        // SAFETY: this read owns the published slot until Drop releases tail.
        unsafe { &(*self.ring.slots[self.tail % self.ring.slots.len()].get()).bytes }
    }
    pub(crate) fn binary(&self) -> bool {
        unsafe { (*self.ring.slots[self.tail % self.ring.slots.len()].get()).binary }
    }
}
impl Drop for Read {
    fn drop(&mut self) {
        self.ring.tail.store(self.tail.wrapping_add(1), Release);
        self.ring.reading.store(false, Release);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_and_held_slots_never_overwrite() {
        let ring = Ring::new(2, 32);
        assert!(ring.push(b"first", true));
        let held = ring.pop().unwrap();
        assert!(ring.push(b"second", false));
        assert!(!ring.push(b"overflow", true));
        assert!(ring.pop().is_none());
        assert_eq!(held.bytes(), b"first");
        drop(held);
        assert!(ring.push(b"third", true));
        let next = ring.pop().unwrap();
        assert!(!next.binary());
        assert_eq!(next.bytes(), b"second");
        drop(next);
        ring.close();
        assert!(!ring.drained());
        assert_eq!(ring.pop().unwrap().bytes(), b"third");
        assert!(ring.drained());
    }
    #[test]
    fn concurrent_ordered_reuse() {
        let ring = Ring::new(8, 8);
        let writer = ring.clone();
        let thread = std::thread::spawn(move || {
            for n in 0u64..100_000 {
                while !writer.push(&n.to_le_bytes(), true) {
                    std::thread::yield_now();
                }
            }
            writer.close();
        });
        for n in 0u64..100_000 {
            let item = loop {
                if let Some(item) = ring.pop() {
                    break item;
                }
                std::thread::yield_now();
            };
            assert_eq!(item.bytes(), n.to_le_bytes());
        }
        thread.join().unwrap();
        assert!(ring.drained());
    }
}
