// SPDX-License-Identifier: GPL-3.0-or-later
//! Serialized thread ABI, with storage and execution supplied at typed boundaries.
use crate::{NOT_IMPLEMENTED, SUCCESS, Thread, ThreadId, entry_frame, stack_size};
use tsfp_kernel_object::{GuestAddress, GuestMemory, Handle, Kind, ObjectTable, Owner};

pub use tsfp_kernel_object::{ACCESS_VIOLATION, INVALID_HANDLE, INVALID_PARAMETER, TYPE_MISMATCH};
pub const NO_MEMORY: u32 = 0xc0000017;

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub stack: GuestAddress,
    pub limit: GuestAddress,
}
impl Frame {
    pub fn argument(self, memory: &impl GuestMemory, index: u32) -> Option<u32> {
        if self.stack.0 == 0 {
            return None;
        }
        let address = u64::from(self.stack.0) + 4 + 4 * u64::from(index);
        if address + 4 > 0x100000000 || (self.limit.0 != 0 && address + 4 > u64::from(self.limit.0))
        {
            return None;
        }
        memory.read_word(GuestAddress(address as u32))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreateRequest {
    pub stack_bytes: u32,
    pub tls_bytes: u32,
    pub start: u32,
    pub context: u32,
    pub system: u32,
    pub suspended: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Storage {
    pub stack_low: GuestAddress,
    pub stack_high: GuestAddress,
    pub control: GuestAddress,
    pub tls: GuestAddress,
    pub monitor: GuestAddress,
}
/// Provider owns mapped allocation, guard protection and rollback of partial allocation.
/// `provision` returns private storage with control/TLS/monitor initialized, or cleans up.
/// No implementation is installed in the game host by this crate.
pub trait ThreadStorage {
    fn provision(
        &mut self,
        request: CreateRequest,
        memory: &mut impl GuestMemory,
    ) -> Result<Storage, u32>;
    fn release(&mut self, storage: Storage);
}
#[derive(Clone, Debug)]
pub struct Record {
    pub handle: Handle,
    pub state: Thread,
    pub request: CreateRequest,
    pub storage: Storage,
    pub entry: u32,
    pub esp: GuestAddress,
    pub frame_ready: bool,
    pub priority_set: bool,
    pub termination_requested: bool,
    pub exit_eax: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentThread {
    pub slot: usize,
    pub handle: Handle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Launch(Handle),
    Terminate { handle: Handle, status: u32 },
}
/// Immutable entry data handed to an execution provider after publication.
/// The provider must serialize this snapshot with launch acknowledgement and reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Launch {
    pub handle: Handle,
    pub entry: u32,
    pub esp: GuestAddress,
    pub fs_base: GuestAddress,
    pub start: u32,
    pub context: u32,
    pub stack_low: GuestAddress,
    pub stack_high: GuestAddress,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Returned(u32),
    Pending(Action),
    Unregistered,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    MissingArguments,
    InvalidHandle,
    WrongKind,
    BadOutput,
    MissingRecord,
    Finished,
    RunningSuspend,
    SuspendLimit,
    FrameMissing,
    NoExecution,
    PriorityMissing,
    PriorityRecorded,
    TerminationRefused,
    TerminationRequested,
    TerminationReturned,
    Reset,
    Exhausted,
    ProvisionFailed,
    PublishFailed,
    OptionalIdFailed,
    CreatedSuspended,
    RequestedNotRunning,
    YieldRefused,
    ExitPublicationFailed,
}
#[derive(Default, Debug)]
pub struct Threads {
    pub records: Vec<Option<Record>>,
    pub current: Option<CurrentThread>,
    pub resetting: bool,
    pub unhonoured_suspends: u32,
    pub ignored_priorities: u32,
    pub diagnostics: Vec<Diagnostic>,
}
impl Threads {
    /// A pending request is inert until an execution provider accepts it.
    pub fn launch_snapshot(&self, handle: Handle) -> Option<Launch> {
        let record = self.records.iter().flatten().find(|r| r.handle == handle)?;
        if !record.frame_ready
            || record.state.started
            || record.state.finished
            || record.state.suspend_count != 0
        {
            return None;
        }
        Some(Launch {
            handle,
            entry: record.entry,
            esp: record.esp,
            fs_base: record.storage.control,
            start: record.request.start,
            context: record.request.context,
            stack_low: record.storage.stack_low,
            stack_high: record.storage.stack_high,
        })
    }
    /// Call only after a provider has accepted the launch under the adapter lock.
    pub fn acknowledge_launch(&mut self, handle: Handle) -> bool {
        let Some(record) = self
            .records
            .iter_mut()
            .flatten()
            .find(|r| r.handle == handle)
        else {
            return false;
        };
        record.frame_ready && record.state.acknowledge_launch()
    }
    pub fn current_identity(&self) -> ThreadId {
        ThreadId(self.current.map_or(0, |c| c.handle.0))
    }
    fn log(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
    fn valid_current(&self) -> Option<usize> {
        let current = self.current?;
        self.records
            .get(current.slot)?
            .as_ref()
            .filter(|r| r.handle == current.handle && r.state.started && !r.state.finished)
            .map(|_| current.slot)
    }
    pub fn create(
        &mut self,
        frame: Option<Frame>,
        memory: &mut impl GuestMemory,
        objects: &mut ObjectTable,
        provider: &mut impl ThreadStorage,
    ) -> Outcome {
        let arg = |index| frame.and_then(|f| f.argument(memory, index));
        let Some(output) = arg(0).filter(|v| *v != 0) else {
            self.log(Diagnostic::MissingArguments);
            return Outcome::Returned(INVALID_PARAMETER);
        };
        let id_out = arg(4).unwrap_or(0);
        let request = CreateRequest {
            stack_bytes: arg(2).unwrap_or(0),
            tls_bytes: arg(3).unwrap_or(0),
            start: arg(5).unwrap_or(0),
            context: arg(6).unwrap_or(0),
            suspended: arg(7).unwrap_or(0) != 0,
            system: arg(9).unwrap_or(0),
        };
        if self.resetting {
            self.log(Diagnostic::Reset);
            return Outcome::Returned(NOT_IMPLEMENTED);
        }
        if self.records.len() >= 64 {
            self.log(Diagnostic::Exhausted);
            return Outcome::Returned(NO_MEMORY);
        }
        let slot = self.records.len();
        let handle = objects.create(Kind::Thread, Owner(slot as u32));
        if handle.0 == 0 {
            self.log(Diagnostic::Exhausted);
            return Outcome::Returned(NO_MEMORY);
        }
        self.records.push(None);
        let storage = match provider.provision(request, memory) {
            Ok(storage) => storage,
            Err(status) => {
                assert!(objects.rollback_thread_body(handle, Owner(slot as u32)));
                self.records.pop();
                self.log(Diagnostic::ProvisionFailed);
                return Outcome::Returned(status);
            }
        };
        let entry = entry_frame(request.system, request.start, request.context);
        let count = entry.argument_count + 1;
        let bytes = count as u32 * 4;
        let mut esp = GuestAddress(0);
        let mut ready = storage.stack_high.0 > storage.stack_low.0
            && storage.stack_high.0 - storage.stack_low.0 > bytes;
        if ready {
            let base = storage.stack_high.0 - bytes;
            for (index, value) in std::iter::once(0xdeadf00d)
                .chain(entry.arguments.into_iter().take(entry.argument_count))
                .enumerate()
            {
                if !memory.write_word(GuestAddress(base + index as u32 * 4), value) {
                    ready = false;
                    break;
                }
            }
            if ready {
                esp = GuestAddress(base);
            }
        }
        if !ready {
            self.log(Diagnostic::FrameMissing);
        }
        if !objects.publish_thread_body(
            handle,
            Owner(slot as u32),
            GuestAddress(storage.control.0.wrapping_add(0x800)),
            GuestAddress(output),
            memory,
        ) {
            assert!(objects.rollback_thread_body(handle, Owner(slot as u32)));
            provider.release(storage);
            self.records.pop();
            self.log(Diagnostic::PublishFailed);
            return Outcome::Returned(INVALID_PARAMETER);
        }
        if id_out != 0 && !objects.write_optional_thread_id(GuestAddress(id_out), handle.0, memory)
        {
            self.log(Diagnostic::OptionalIdFailed);
        }
        self.records[slot] = Some(Record {
            handle,
            state: Thread::new(ThreadId(handle.0), request.suspended),
            request,
            storage,
            entry: entry.entry,
            esp,
            frame_ready: ready,
            priority_set: false,
            termination_requested: false,
            exit_eax: 0,
        });
        if ready {
            self.log(if request.suspended {
                Diagnostic::CreatedSuspended
            } else {
                Diagnostic::NoExecution
            });
        }
        self.log(Diagnostic::RequestedNotRunning);
        Outcome::Returned(SUCCESS)
    }
    /// Execution presence is explicit. Default false is the headless/no-host path.
    /// Pending launch never sets started; an adapter must acknowledge actual launch.
    pub fn call(
        &mut self,
        ordinal: u32,
        frame: Option<Frame>,
        memory: &mut impl GuestMemory,
        objects: &ObjectTable,
        irql: u8,
        execution_present: bool,
    ) -> Outcome {
        if ordinal == 238 {
            if irql != 0 {
                self.log(Diagnostic::YieldRefused);
            }
            return Outcome::Returned(if irql == 0 {
                0x40000024
            } else {
                NOT_IMPLEMENTED
            });
        }
        if ordinal == 258 {
            let Some(status) = frame.and_then(|f| f.argument(memory, 0)) else {
                self.log(Diagnostic::MissingArguments);
                return Outcome::Returned(INVALID_PARAMETER);
            };
            let Some(slot) = self.valid_current().filter(|_| execution_present) else {
                self.log(Diagnostic::TerminationRefused);
                return Outcome::Returned(NOT_IMPLEMENTED);
            };
            let record = self.records[slot].as_mut().unwrap();
            record.termination_requested = true;
            record.exit_eax = status;
            let handle = record.handle;
            self.log(Diagnostic::TerminationRequested);
            return Outcome::Pending(Action::Terminate { handle, status });
        }
        if !matches!(ordinal, 143 | 144 | 224 | 231) {
            return Outcome::Unregistered;
        }
        let args = frame.and_then(|f| Some((f.argument(memory, 0)?, f.argument(memory, 1)?)));
        let Some((target, value)) = args else {
            self.log(Diagnostic::MissingArguments);
            return Outcome::Returned(if ordinal == 143 || ordinal == 144 {
                0
            } else {
                INVALID_PARAMETER
            });
        };
        if ordinal == 143 || ordinal == 144 {
            let slot = objects
                .referenced_thread_body(GuestAddress(target))
                .and_then(|entry| {
                    self.records
                        .iter()
                        .enumerate()
                        .find(|(slot, record)| {
                            record.as_ref().is_some_and(|r| {
                                r.handle == entry.handle
                                    && entry.owner.0 == *slot as u32
                                    && r.storage.control.0.wrapping_add(0x800) == target
                            })
                        })
                        .map(|(slot, _)| slot)
                });
            let Some(slot) = slot else {
                self.log(Diagnostic::PriorityMissing);
                return Outcome::Returned(0);
            };
            let record = self.records[slot].as_mut().unwrap();
            let previous = if ordinal == 143 {
                record.priority_set = true;
                record.state.priority(value as i32) as u32
            } else {
                u32::from(record.state.disable_boost(value))
            };
            self.ignored_priorities = self.ignored_priorities.wrapping_add(1);
            self.log(Diagnostic::PriorityRecorded);
            return Outcome::Returned(previous);
        }
        let Some(entry) = objects.find(Handle(target)) else {
            self.log(Diagnostic::InvalidHandle);
            return Outcome::Returned(INVALID_HANDLE);
        };
        if entry.kind != Kind::Thread {
            self.log(Diagnostic::WrongKind);
            return Outcome::Returned(TYPE_MISMATCH);
        }
        if value != 0 && !memory.readable(GuestAddress(value), 4) {
            self.log(Diagnostic::BadOutput);
            return Outcome::Returned(ACCESS_VIOLATION);
        }
        let Some(slot) = self
            .records
            .iter()
            .position(|r| r.as_ref().is_some_and(|r| r.handle.0 == target))
        else {
            self.log(Diagnostic::MissingRecord);
            return Outcome::Returned(INVALID_HANDLE);
        };
        let record = self.records[slot].as_mut().unwrap();
        let previous;
        let mut action = None;
        if ordinal == 231 {
            match record.state.suspend() {
                Ok(count) => previous = count,
                Err(status) => {
                    let d = if status == NOT_IMPLEMENTED {
                        self.unhonoured_suspends = self.unhonoured_suspends.wrapping_add(1);
                        Diagnostic::RunningSuspend
                    } else if status == 0xc000004b {
                        Diagnostic::Finished
                    } else {
                        Diagnostic::SuspendLimit
                    };
                    self.log(d);
                    return Outcome::Returned(status);
                }
            }
        } else {
            let resume = record.state.resume();
            previous = resume.previous;
            if record.state.finished {
                self.log(Diagnostic::Finished);
            } else if resume.launch_requested {
                if !record.frame_ready {
                    self.log(Diagnostic::FrameMissing);
                } else if execution_present {
                    action = Some(Action::Launch(record.handle));
                } else {
                    self.log(Diagnostic::NoExecution);
                    self.log(Diagnostic::RequestedNotRunning);
                }
            }
        }
        if value != 0 && !memory.write_word(GuestAddress(value), previous) {
            self.log(Diagnostic::BadOutput);
        }
        action.map_or(Outcome::Returned(SUCCESS), Outcome::Pending)
    }
    /// Called only after the execution provider has returned to its launch boundary.
    /// Confirmation is separate from a request; body status precedes its signal byte.
    /// This transition never delivers a wakeup or schedules another thread.
    pub fn complete_entry(
        &mut self,
        current: CurrentThread,
        confirmed: bool,
        memory: &mut impl ThreadMemory,
    ) -> bool {
        let Some(Some(record)) = self.records.get_mut(current.slot) else {
            return false;
        };
        if record.handle != current.handle || !record.state.started || record.state.finished {
            return false;
        }
        let requested = record.termination_requested;
        let body = record.storage.control.0.wrapping_add(0x800);
        let published = requested
            && confirmed
            && memory.write_word(GuestAddress(body.wrapping_add(0x120)), record.exit_eax)
            && memory.write_byte(GuestAddress(body.wrapping_add(4)), 1);
        record.state.finished = true;
        record.state.terminated = published;
        record.state.exit_status = requested.then_some(record.exit_eax);
        if self.current == Some(current) {
            self.current = None;
        }
        if requested && confirmed && !published {
            self.log(Diagnostic::ExitPublicationFailed);
        }
        true
    }
    pub fn termination_hook_returned(&mut self, current: CurrentThread) -> Outcome {
        if let Some(Some(record)) = self.records.get_mut(current.slot)
            && record.handle == current.handle
        {
            record.termination_requested = false;
            record.exit_eax = 0;
        }
        self.log(Diagnostic::TerminationReturned);
        Outcome::Returned(NOT_IMPLEMENTED)
    }
}

/// Rounded TLS allocation size, independent of the requested TLS end offset.
pub fn tls_allocation_size(requested: u32) -> Option<u32> {
    requested.max(0x40).checked_add(0xfff).map(|v| v & !0xfff)
}
pub fn stack_region_size(requested: u32) -> u32 {
    stack_size(requested) + 0x20000
}
pub fn publish_tls_end(
    memory: &mut impl GuestMemory,
    control: GuestAddress,
    tls: GuestAddress,
    bytes: u32,
) -> bool {
    if control.0 == 0 || tls.0 == 0 {
        return false;
    }
    tls.0
        .checked_add(bytes)
        .is_some_and(|end| memory.write_word(GuestAddress(control.0.wrapping_add(4)), end))
}
/// Byte stores preserve neighbours through the guest provider, including partial failures.
pub trait ThreadMemory: GuestMemory {
    fn write_byte(&mut self, address: GuestAddress, value: u8) -> bool;
}
pub fn initialize_control(
    memory: &mut impl ThreadMemory,
    storage: Storage,
    monitor_bytes: u32,
    callback: u32,
) -> bool {
    if storage.control.0 == 0
        || storage.tls.0 == 0
        || storage.monitor.0 == 0
        || monitor_bytes < 0x28
    {
        return false;
    }
    let at = |base: GuestAddress, offset: u32| GuestAddress(base.0.wrapping_add(offset));
    for offset in (0..monitor_bytes).step_by(4) {
        if !memory.write_word(at(storage.monitor, offset), 0) {
            return false;
        }
    }
    if callback != 0 && !memory.write_word(at(storage.monitor, 0x14), callback) {
        return false;
    }
    for (offset, value) in [
        (0, u32::MAX),
        (0x20, storage.control.0.wrapping_add(0x28)),
        (0x24, 0),
        (0x28, storage.control.0.wrapping_add(0x800)),
        (0x828, storage.tls.0),
    ] {
        if !memory.write_word(at(storage.control, offset), value) {
            return false;
        }
        if offset == 0x24 && !memory.write_byte(at(storage.control, 0x24), 0) {
            return false;
        }
    }
    memory.write_byte(at(storage.control, 0x804), 0)
        && memory.write_word(at(storage.control, 0x920), 0x103)
        && memory.write_word(at(storage.control, 0x278), storage.monitor.0)
}
