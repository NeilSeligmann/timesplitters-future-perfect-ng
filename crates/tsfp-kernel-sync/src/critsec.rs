// SPDX-License-Identifier: GPL-3.0-or-later
//! Repository-C critical-section ABI model; no host installation or Xbox parity claim.
use crate::{SUCCESS, ThreadId};

/// Reads and writes stay separate: C tolerates failed accesses and partial writes.
pub trait GuestWords {
    fn read(&mut self, address: u32) -> Option<u32>;
    fn write(&mut self, address: u32, value: u32);
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestAddress(pub u32);
#[derive(Clone, Copy, Debug)]
pub struct CallFrame {
    pub stack: u32,
    pub limit: u32,
}
impl CallFrame {
    fn argument(self, memory: &mut impl GuestWords) -> Option<u32> {
        let address = self.stack.checked_add(4)?;
        if self.stack == 0 || (self.limit != 0 && address.checked_add(4)? > self.limit) {
            return None;
        }
        memory.read(address)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub adopted: u32,
    pub bad_leave: u32,
    pub contended: u32,
    pub recursive_enter: u32,
    pub implausible: u32,
    pub unreadable: u32,
    pub table_full: u32,
    pub reinit_held: u32,
    pub retired: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub address: GuestAddress,
    pub owner: Option<ThreadId>,
    pub recursion: u32,
    pub adopted: bool,
    // A recursive native mutex can retain acquisitions after Initialize clears owner.
    native_owner: Option<ThreadId>,
    native_depth: u32,
    retired: bool,
}
/// Single-use continuation. Adapter parks the caller, then retries after mutex availability.
/// No guest lock-count increment or completed return is permitted while pending.
#[derive(Debug)]
pub struct PendingEnter {
    slot: usize,
    address: GuestAddress,
    caller: ThreadId,
    was_held: bool,
}
#[derive(Debug)]
pub enum CallOutcome {
    Returned(u32),
    Pending(PendingEnter),
    /// 306 has no C handler. Dispatcher fallback is not a TryEnter implementation.
    Unregistered,
}
#[derive(Default)]
pub struct CriticalSections {
    records: Vec<Record>,
    pub diagnostics: Diagnostics,
}
impl CriticalSections {
    pub fn record(&self, address: GuestAddress) -> Option<&Record> {
        self.records
            .iter()
            .find(|r| r.address == address && address.0 != 0)
    }
    pub fn tracked(&self) -> usize {
        self.records.iter().filter(|r| r.address.0 != 0).count()
    }
    fn adopt(
        &mut self,
        address: GuestAddress,
        init: bool,
        memory: &mut impl GuestWords,
    ) -> Option<usize> {
        if address.0 == 0 {
            self.diagnostics.unreadable = self.diagnostics.unreadable.wrapping_add(1);
            return None;
        }
        if let Some(slot) = self.records.iter().position(|r| r.address == address) {
            return Some(slot);
        }
        let Some(header) = memory.read(address.0) else {
            self.diagnostics.unreadable = self.diagnostics.unreadable.wrapping_add(1);
            return None;
        };
        let slot = if let Some(slot) = self
            .records
            .iter()
            .position(|r| r.address.0 == 0 && !r.retired)
        {
            slot
        } else if self.records.len() < 64 {
            self.records.push(Record {
                address: GuestAddress(0),
                owner: None,
                recursion: 0,
                adopted: false,
                native_owner: None,
                native_depth: 0,
                retired: false,
            });
            self.records.len() - 1
        } else {
            self.diagnostics.table_full = self.diagnostics.table_full.wrapping_add(1);
            return None;
        };
        let r = &mut self.records[slot];
        r.address = address;
        r.owner = None;
        r.recursion = 0;
        r.adopted = !init;
        if !init {
            self.diagnostics.adopted = self.diagnostics.adopted.wrapping_add(1);
            if header & 0xff != 1 || (header >> 16) & 0xff != 4 {
                self.diagnostics.implausible = self.diagnostics.implausible.wrapping_add(1);
            }
        }
        Some(slot)
    }
    fn step(address: GuestAddress, delta: u32, memory: &mut impl GuestWords) {
        let address = address.0.wrapping_add(16);
        if let Some(count) = memory.read(address) {
            memory.write(address, count.wrapping_add(delta));
        }
    }
    /// Caller is a nonzero opaque host-thread token, not a guest KTHREAD pointer.
    pub fn call(
        &mut self,
        ordinal: u32,
        frame: Option<CallFrame>,
        caller: ThreadId,
        memory: &mut impl GuestWords,
    ) -> CallOutcome {
        if !matches!(ordinal, 277 | 291 | 294) {
            return CallOutcome::Unregistered;
        }
        assert_ne!(caller.0, 0);
        let Some(address) = frame.and_then(|f| f.argument(memory)).map(GuestAddress) else {
            return CallOutcome::Returned(SUCCESS);
        };
        let Some(slot) = self.adopt(address, ordinal == 291, memory) else {
            return CallOutcome::Returned(SUCCESS);
        };
        let r = &mut self.records[slot];
        match ordinal {
            291 => {
                if r.owner.is_some() {
                    self.diagnostics.reinit_held = self.diagnostics.reinit_held.wrapping_add(1);
                }
                r.owner = None;
                r.recursion = 0;
                r.adopted = false;
                let words = [
                    0x00040001,
                    0,
                    address.0.wrapping_add(8),
                    address.0.wrapping_add(8),
                    u32::MAX,
                    0,
                    0,
                ];
                for (index, word) in words.into_iter().enumerate() {
                    memory.write(address.0.wrapping_add(index as u32 * 4), word);
                }
            }
            294 => {
                if r.owner != Some(caller) {
                    self.diagnostics.bad_leave = self.diagnostics.bad_leave.wrapping_add(1);
                } else {
                    r.recursion = r.recursion.wrapping_sub(1);
                    Self::step(address, u32::MAX, memory);
                    if r.recursion == 0 {
                        r.owner = None;
                        r.native_depth -= 1;
                        if r.native_depth == 0 {
                            r.native_owner = None;
                        }
                    }
                }
            }
            277 => {
                if r.owner == Some(caller) {
                    r.recursion = r.recursion.wrapping_add(1);
                    self.diagnostics.recursive_enter =
                        self.diagnostics.recursive_enter.wrapping_add(1);
                    Self::step(address, 1, memory);
                } else {
                    let ticket = PendingEnter {
                        slot,
                        address,
                        caller,
                        was_held: r.owner.is_some(),
                    };
                    return self.complete_enter(ticket, memory);
                }
            }
            _ => unreachable!(),
        }
        CallOutcome::Returned(SUCCESS)
    }
    /// Models native-mutex acquisition followed by C's table recheck. No FIFO promise.
    pub fn complete_enter(
        &mut self,
        ticket: PendingEnter,
        memory: &mut impl GuestWords,
    ) -> CallOutcome {
        let r = &mut self.records[ticket.slot];
        if r.native_owner.is_some_and(|owner| owner != ticket.caller) {
            return CallOutcome::Pending(ticket);
        }
        // Stale slot after reset: acquire/release native mutex, return without guest writes.
        if r.address != ticket.address {
            return CallOutcome::Returned(SUCCESS);
        }
        r.native_owner = Some(ticket.caller);
        r.native_depth += 1;
        if ticket.was_held {
            self.diagnostics.contended = self.diagnostics.contended.wrapping_add(1);
        }
        r.owner = Some(ticket.caller);
        r.recursion = 1;
        Self::step(ticket.address, 1, memory);
        CallOutcome::Returned(SUCCESS)
    }
    pub fn reset(&mut self) {
        for r in &mut self.records {
            if r.owner.is_some() && !r.retired {
                r.retired = true;
                self.diagnostics.retired = self.diagnostics.retired.wrapping_add(1);
            }
            r.address = GuestAddress(0);
            r.owner = None;
            r.recursion = 0;
            r.adopted = false;
        }
        let retired = self.diagnostics.retired;
        self.diagnostics = Diagnostics {
            retired,
            ..Diagnostics::default()
        };
    }
}
