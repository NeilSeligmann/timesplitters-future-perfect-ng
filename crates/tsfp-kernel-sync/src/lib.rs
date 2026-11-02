// SPDX-License-Identifier: GPL-3.0-or-later
//! Scheduler-free K-1 state transitions. See docs/t-contract-kernel-sync.md.
//! Guest pointer validation, execution and asynchronous delivery belong to adapters.
use std::collections::BTreeMap;

pub const RUST_KERNEL_SYNC_ENABLED: bool = cfg!(feature = "rust-kernel-sync");
pub const SUCCESS: u32 = 0;
pub const TIMEOUT: u32 = 0x102;
pub const INVALID_HANDLE: u32 = 0xc0000008;
pub const TYPE_MISMATCH: u32 = 0xc0000024;
pub const NOT_IMPLEMENTED: u32 = 0xc0000002;
pub const MUTANT_NOT_OWNED: u32 = 0xc0000046;
pub const MUTANT_LIMIT: u32 = 0xc0000191;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThreadId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Handle(pub u32);

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Irql {
    pub level: u8,
    pub bad_lowers: u32,
}
impl Irql {
    pub fn raise(&mut self, requested: u32) -> u8 {
        let previous = self.level;
        self.level = self.level.max(requested as u8);
        previous
    }
    pub fn lower(&mut self, requested: u32) {
        let requested = requested as u8;
        if requested > self.level {
            self.bad_lowers = self.bad_lowers.wrapping_add(1);
        } else {
            self.level = requested;
        }
    }
    /// Internal restore is deliberately distinct from checked KfLowerIrql.
    pub fn restore(&mut self, previous: u8) {
        self.level = previous;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Notification,
    Synchronization,
}
/// A handle event's local state after the guest ABI checks.
///
/// Synchronization events consume their signal on a successful wait; notification
/// events keep it until the caller clears them.
///
/// ```
/// use tsfp_kernel_sync::{Event, EventKind};
/// let mut event = Event { kind: EventKind::Synchronization, signaled: false };
/// assert!(!event.try_wait());
/// assert!(!event.set());
/// assert!(event.try_wait());
/// assert!(!event.try_wait());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    pub signaled: bool,
}
impl Event {
    pub fn set(&mut self) -> bool {
        let previous = self.signaled;
        self.signaled = true;
        previous
    }
    pub fn try_wait(&mut self) -> bool {
        let acquired = self.signaled;
        if acquired && self.kind == EventKind::Synchronization {
            self.signaled = false;
        }
        acquired
    }
}

/// Ke events are caller-owned; import this view afresh after every guest write.
/// Unknown dispatcher types must retain their signal state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatcherEvent {
    pub object_type: u8,
    pub signal_state: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeWait {
    Satisfied,
    Unhonoured,
}
impl DispatcherEvent {
    pub fn set(&mut self) -> u32 {
        let previous = u32::from(self.signal_state != 0);
        self.signal_state = 1;
        previous
    }
    /// C returns success even on clear events. Keep the fidelity gap visible.
    pub fn wait(&mut self) -> KeWait {
        if self.signal_state == 0 {
            return KeWait::Unhonoured;
        }
        if self.object_type == 1 {
            self.signal_state = 0;
        }
        KeWait::Satisfied
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Mutant {
    pub owner: Option<ThreadId>,
    pub recursion: u32,
}
impl Mutant {
    pub fn acquire(&mut self, caller: ThreadId) -> (u32, bool) {
        if self.recursion == 0 {
            self.owner = Some(caller);
            self.recursion = 1;
            (SUCCESS, true)
        } else if self.owner != Some(caller) {
            (SUCCESS, false)
        } else if self.recursion >= 0x10000 {
            (MUTANT_LIMIT, false)
        } else {
            self.recursion += 1;
            (SUCCESS, true)
        }
    }
    pub fn release(&mut self, caller: ThreadId) -> (u32, Option<u32>) {
        if self.recursion == 0 || self.owner != Some(caller) {
            return (MUTANT_NOT_OWNED, None);
        }
        let previous = self.recursion;
        self.recursion -= 1;
        if self.recursion == 0 {
            self.owner = None;
        }
        (SUCCESS, Some(previous))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timeout {
    Infinite,
    Poll,
    Relative(u64),
    Absolute(i64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    UnsupportedScope,
    WouldBlock,
    TerminalHostFailure,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitOutcome {
    Status(u32),
    Refused(Refusal),
    PendingThread(Handle),
}
#[derive(Clone, Debug)]
pub enum Object {
    Event(Event),
    Mutant(Mutant),
    Thread(Thread),
    Semaphore,
}
/// Semaphore is a classified unsupported object, not an invented counting model.
#[derive(Default)]
pub struct SyncObjects {
    objects: BTreeMap<Handle, Object>,
}
impl SyncObjects {
    /// Adapter owns allocation, close/refcounts, pointer probing and handle generation.
    pub fn insert(&mut self, handle: Handle, object: Object) -> Option<Object> {
        self.objects.insert(handle, object)
    }
    pub fn remove(&mut self, handle: Handle) -> Option<Object> {
        self.objects.remove(&handle)
    }
    pub fn get(&self, handle: Handle) -> Option<&Object> {
        self.objects.get(&handle)
    }
    pub fn wait(
        &mut self,
        handle: Handle,
        caller: ThreadId,
        irql: u8,
        user_mode: bool,
        alertable: bool,
        timeout: Timeout,
    ) -> WaitOutcome {
        if irql != 0 || !user_mode || alertable {
            return WaitOutcome::Refused(Refusal::UnsupportedScope);
        }
        let Some(object) = self.objects.get_mut(&handle) else {
            // C validates unsupported finite thread timeouts before invalid handles.
            return if matches!(timeout, Timeout::Infinite | Timeout::Poll) {
                WaitOutcome::Status(INVALID_HANDLE)
            } else {
                WaitOutcome::Refused(Refusal::UnsupportedScope)
            };
        };
        let acquired = match object {
            Object::Event(event) => event.try_wait(),
            Object::Mutant(mutant) => {
                let (status, acquired) = mutant.acquire(caller);
                if status != SUCCESS {
                    return WaitOutcome::Status(status);
                }
                acquired
            }
            Object::Thread(thread) => {
                if !matches!(timeout, Timeout::Infinite | Timeout::Poll)
                    || !thread.started
                    || thread.suspend_count != 0
                    || thread.id == caller
                {
                    return WaitOutcome::Refused(Refusal::UnsupportedScope);
                }
                if thread.terminated {
                    return WaitOutcome::Status(SUCCESS);
                }
                if thread.finished {
                    return WaitOutcome::Refused(Refusal::TerminalHostFailure);
                }
                return if timeout == Timeout::Poll {
                    WaitOutcome::Status(TIMEOUT)
                } else {
                    WaitOutcome::PendingThread(handle)
                };
            }
            Object::Semaphore => return WaitOutcome::Refused(Refusal::UnsupportedScope),
        };
        if acquired {
            WaitOutcome::Status(SUCCESS)
        } else if timeout == Timeout::Poll {
            WaitOutcome::Status(TIMEOUT)
        } else {
            WaitOutcome::Refused(Refusal::WouldBlock)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Thread {
    pub id: ThreadId,
    pub suspend_count: u32,
    pub started: bool,
    pub finished: bool,
    pub terminated: bool,
    pub exit_status: Option<u32>,
    pub base_priority: i32,
    pub boost_disabled: bool,
    pub irql: Irql,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resume {
    pub previous: u32,
    pub launch_requested: bool,
}
impl Thread {
    /// Provisioning/launch is separate: absence of host ops must never imply started.
    pub fn new(id: ThreadId, suspended: bool) -> Self {
        Self {
            id,
            suspend_count: u32::from(suspended),
            started: false,
            finished: false,
            terminated: false,
            exit_status: None,
            base_priority: 0,
            boost_disabled: false,
            irql: Irql::default(),
        }
    }
    pub fn suspend(&mut self) -> Result<u32, u32> {
        if self.finished {
            return Err(0xc000004b);
        }
        if self.started {
            return Err(NOT_IMPLEMENTED);
        }
        if self.suspend_count >= 127 {
            return Err(0xc000004a);
        }
        let previous = self.suspend_count;
        self.suspend_count += 1;
        Ok(previous)
    }
    pub fn resume(&mut self) -> Resume {
        let previous = self.suspend_count;
        if !self.finished && previous != 0 {
            self.suspend_count -= 1;
        }
        Resume {
            previous,
            launch_requested: !self.finished
                && previous != 0
                && self.suspend_count == 0
                && !self.started,
        }
    }
    pub fn acknowledge_launch(&mut self) -> bool {
        if self.finished || self.started || self.suspend_count != 0 {
            return false;
        }
        self.started = true;
        true
    }
    pub fn terminate(&mut self, status: u32) {
        self.finished = true;
        self.terminated = true;
        self.exit_status = Some(status);
    }
    /// Unexpected entry return is host failure, never a successful termination.
    pub fn returned(&mut self) {
        self.finished = true;
    }
    pub fn priority(&mut self, increment: i32) -> i32 {
        std::mem::replace(&mut self.base_priority, increment)
    }
    pub fn disable_boost(&mut self, value: u32) -> bool {
        std::mem::replace(&mut self.boost_disabled, value as u8 != 0)
    }
}

pub fn stack_size(requested: u32) -> u32 {
    requested.clamp(0x10000, 0x800000).div_ceil(0x1000) * 0x1000
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryFrame {
    pub entry: u32,
    pub arguments: [u32; 2],
    pub argument_count: usize,
}
pub fn entry_frame(system: u32, start: u32, context: u32) -> EntryFrame {
    if system == 0 {
        EntryFrame {
            entry: start,
            arguments: [context, 0],
            argument_count: 1,
        }
    } else {
        EntryFrame {
            entry: system,
            arguments: [start, context],
            argument_count: 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CriticalSection {
    pub owner: Option<ThreadId>,
    pub recursion: u32,
    pub lock_count: u32,
    /// C reinitialization clears bookkeeping but leaves a held host mutex locked.
    pub stranded: bool,
    stranded_owner: Option<ThreadId>,
    pub bad_leaves: u32,
}
impl Default for CriticalSection {
    fn default() -> Self {
        Self {
            owner: None,
            recursion: 0,
            lock_count: u32::MAX,
            stranded: false,
            stranded_owner: None,
            bad_leaves: 0,
        }
    }
}
impl CriticalSection {
    /// False asks the adapter to block/acquire; never pretend contention succeeded.
    pub fn enter(&mut self, caller: ThreadId) -> bool {
        if (self.stranded && self.stranded_owner != Some(caller))
            || self.owner.is_some_and(|owner| owner != caller)
        {
            return false;
        }
        self.owner = Some(caller);
        self.recursion = self.recursion.wrapping_add(1);
        self.lock_count = self.lock_count.wrapping_add(1);
        true
    }
    pub fn leave(&mut self, caller: ThreadId) {
        if self.owner != Some(caller) {
            self.bad_leaves = self.bad_leaves.wrapping_add(1);
            return;
        }
        self.recursion -= 1;
        self.lock_count = self.lock_count.wrapping_sub(1);
        if self.recursion == 0 {
            self.owner = None;
        }
    }
    pub fn initialize(&mut self) {
        if self.owner.is_some() && !self.stranded {
            self.stranded_owner = self.owner;
        }
        self.stranded |= self.owner.is_some();
        self.owner = None;
        self.recursion = 0;
        self.lock_count = u32::MAX;
    }
}

/// Host extension point: no clock ownership, scheduler, vblank synthesis or callbacks here.
/// Adapter must serialize delivery on the owning thread at PASSIVE_LEVEL, not in a signal handler.
pub trait SafepointDelivery {
    fn service_pending(&mut self, caller: ThreadId);
}
pub fn safepoint(thread: &Thread, delivery: &mut impl SafepointDelivery) -> bool {
    if !thread.started || thread.finished || thread.irql.level != 0 {
        return false;
    }
    delivery.service_pending(thread.id);
    true
}

pub mod critsec;

pub mod wait;

pub mod thread;

pub mod event;
