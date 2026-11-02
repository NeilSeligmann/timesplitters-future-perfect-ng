//! Headless object ownership model. See docs/t-contract-kernel-object.md.
use std::collections::VecDeque;

pub const RUST_SELECTED: bool = cfg!(feature = "rust-kernel-object");
pub const INVALID_HANDLE: u32 = 0xc0000008;
pub const INVALID_PARAMETER: u32 = 0xc000000d;
pub const NOT_IMPLEMENTED: u32 = 0xc0000002;
pub const INSUFFICIENT_RESOURCES: u32 = 0xc000009a;
pub const TYPE_MISMATCH: u32 = 0xc0000024;
pub const NOT_OWNED: u32 = 0xc0000046;
pub const ACCESS_VIOLATION: u32 = 0xc0000005;
const BODY_SPAN: u32 = 0x124;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Handle(pub u32);
impl Handle {
    pub fn encode(slot: usize, generation: u16) -> Self {
        assert!(slot < 256 && generation < 16384);
        Self(
            0x00e10000
                | ((slot as u32) << 2)
                | (((generation as u32) & 63) << 10)
                | (((generation as u32) >> 6) << 24),
        )
    }
    fn slot(self) -> usize {
        ((self.0 >> 2) & 255) as usize
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuestAddress(pub u32);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Owner(pub u32);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum Kind {
    #[default]
    None,
    Thread,
    File,
    Event,
    Semaphore,
    Mutant,
    Symlink,
    Other,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ObjectState {
    #[default]
    Plain,
    Event {
        manual: bool,
        signaled: bool,
    },
    Thread {
        open: bool,
        body: GuestAddress,
        body_references: u32,
    },
    File {
        duplicate_root: Handle,
    },
    Mutant {
        owner: Owner,
        depth: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub handle: Handle,
    pub kind: Kind,
    pub owner: Owner,
    pub references: u32,
    pub state: ObjectState,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub bad_close: u32,
    pub references_by_handle: u32,
    pub unchecked_types: u32,
    pub unowned_releases: u32,
    pub messages: Vec<Diagnostic>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnostic {
    MissingFrame,
    InvalidHandle,
    Exhausted,
    Retired,
    Unsupported,
    IgnoredAttributes,
    WriteFailed,
    Created,
    UncheckedType,
    BodyUnavailable,
    Unowned,
}

/// Implementations preserve partial write prefixes on failure, as kernel_call.c does.
pub trait GuestMemory {
    fn read_word(&self, address: GuestAddress) -> Option<u32>;
    fn write_word(&mut self, address: GuestAddress, value: u32) -> bool;
    fn readable(&self, address: GuestAddress, length: u32) -> bool;
}
#[derive(Clone, Copy, Debug)]
pub struct CallFrame {
    pub stack: GuestAddress,
    pub limit: GuestAddress,
    pub ecx: Option<u32>,
}
impl CallFrame {
    pub fn argument(self, memory: &impl GuestMemory, index: usize) -> Option<u32> {
        if self.stack.0 == 0 {
            return None;
        }
        let slot = self.stack.0 as u64 + 4 + 4 * index as u64;
        if slot + 4 > 0x100000000 || (self.limit.0 != 0 && slot + 4 > self.limit.0 as u64) {
            return None;
        }
        memory.read_word(GuestAddress(slot as u32))
    }
}
/// File-body behaviour is supplied by T2210; this crate never constructs file objects.
pub trait FileObjects {
    fn reference(&mut self, table: &mut ObjectTable, object: GuestAddress) -> bool;
    fn dereference(&mut self, table: &mut ObjectTable, object: GuestAddress) -> bool;
    fn reference_by_handle(
        &mut self,
        table: &mut ObjectTable,
        memory: &mut impl GuestMemory,
        handle: Handle,
        object_type: u32,
        output: GuestAddress,
    ) -> Option<u32>;
    fn release(&mut self, handle: Handle);
}
pub struct NoFileBodies;
impl FileObjects for NoFileBodies {
    fn reference(&mut self, _: &mut ObjectTable, _: GuestAddress) -> bool {
        false
    }
    fn dereference(&mut self, _: &mut ObjectTable, _: GuestAddress) -> bool {
        false
    }
    fn reference_by_handle(
        &mut self,
        _: &mut ObjectTable,
        _: &mut impl GuestMemory,
        _: Handle,
        _: u32,
        _: GuestAddress,
    ) -> Option<u32> {
        None
    }
    fn release(&mut self, _: Handle) {}
}
/// Owns 256 guest object slots. Fresh slots precede FIFO reuse, and a stale
/// handle cannot name a later occupant of the same slot.
///
/// ```
/// use tsfp_kernel_object::{Kind, NoFileBodies, ObjectTable, Owner};
/// let mut table = ObjectTable::default();
/// let old = table.create(Kind::Event, Owner(1));
/// assert!(table.release(old, &mut NoFileBodies));
/// assert!(table.find(old).is_none());
/// ```
#[derive(Clone, Debug)]
pub struct ObjectTable {
    entries: [Option<Entry>; 256],
    generations: [u16; 256],
    fresh: usize,
    free: VecDeque<usize>,
    retired: usize,
    pub diagnostics: Diagnostics,
}
impl Default for ObjectTable {
    fn default() -> Self {
        Self {
            entries: std::array::from_fn(|_| None),
            generations: [0; 256],
            fresh: 0,
            free: VecDeque::new(),
            retired: 0,
            diagnostics: Diagnostics::default(),
        }
    }
}
fn overlaps(a: GuestAddress, length: u32, b: GuestAddress, span: u32) -> bool {
    (a.0 as u64) < b.0 as u64 + span as u64 && (b.0 as u64) < a.0 as u64 + length as u64
}
impl ObjectTable {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn entries(&self) -> &[Option<Entry>; 256] {
        &self.entries
    }
    pub fn generations(&self) -> &[u16; 256] {
        &self.generations
    }
    pub fn free_slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.free.iter().copied()
    }
    pub fn fresh_count(&self) -> usize {
        self.fresh
    }
    pub fn retired_count(&self) -> usize {
        self.retired
    }
    pub fn live_count(&self) -> usize {
        self.entries
            .iter()
            .flatten()
            .filter(|e| !matches!(e.state, ObjectState::Thread { open: false, .. }))
            .count()
    }
    fn log(&mut self, d: Diagnostic) {
        self.diagnostics.messages.push(d);
    }
    fn any(&self, handle: Handle) -> Option<&Entry> {
        self.entries[handle.slot()]
            .as_ref()
            .filter(|e| e.handle == handle)
    }
    pub fn find(&self, handle: Handle) -> Option<&Entry> {
        self.any(handle)
            .filter(|e| !matches!(e.state, ObjectState::Thread { open: false, .. }))
    }
    /// Provider-side FILE body reference, sharing the table's authoritative count.
    pub fn file_body_reference(&mut self, handle: Handle) -> bool {
        let Some(entry) = self.entries[handle.slot()].as_mut().filter(|entry| {
            entry.handle == handle && entry.kind == Kind::File && entry.references < u32::MAX
        }) else {
            return false;
        };
        entry.references += 1;
        true
    }
    /// Drop a provider-validated body reference. Returns whether the slot died.
    /// The provider owns body-reference validation and final body destruction.
    pub fn file_body_dereference(&mut self, handle: Handle) -> Option<bool> {
        let entry = self.entries[handle.slot()].as_mut().filter(|entry| {
            entry.handle == handle && entry.kind == Kind::File && entry.references > 0
        })?;
        entry.references -= 1;
        let dead = entry.references == 0;
        if dead {
            self.free_entry(handle.slot());
        }
        Some(dead)
    }
    pub fn create(&mut self, kind: Kind, owner: Owner) -> Handle {
        let slot = if self.fresh < 256 {
            let slot = self.fresh;
            self.fresh += 1;
            slot
        } else if let Some(slot) = self.free.pop_front() {
            slot
        } else {
            self.log(Diagnostic::Exhausted);
            return Handle(0);
        };
        let handle = Handle::encode(slot, self.generations[slot]);
        let state = match kind {
            Kind::Thread => ObjectState::Thread {
                open: true,
                body: GuestAddress(0),
                body_references: 0,
            },
            Kind::File => ObjectState::File {
                duplicate_root: Handle(0),
            },
            Kind::Event => ObjectState::Event {
                manual: true,
                signaled: false,
            },
            Kind::Mutant => ObjectState::Mutant {
                owner: Owner(0),
                depth: 0,
            },
            _ => ObjectState::Plain,
        };
        self.entries[slot] = Some(Entry {
            handle,
            kind,
            owner,
            references: 1,
            state,
        });
        handle
    }
    fn free_entry(&mut self, slot: usize) {
        self.entries[slot] = None;
        self.generations[slot] += 1;
        if self.generations[slot] == 16384 {
            self.retired += 1;
            self.log(Diagnostic::Retired);
        } else {
            self.free.push_back(slot);
        }
    }
    /// Event publication probes the original output bytes before allocating an identity.
    /// The adapter keeps mappings stable; second-write failure rolls back the slot.
    pub fn create_event(
        &mut self,
        manual: bool,
        output: GuestAddress,
        memory: &mut impl GuestMemory,
    ) -> u32 {
        let Some(previous) = (if output.0 == 0 {
            None
        } else {
            memory.read_word(output)
        }) else {
            return INVALID_PARAMETER;
        };
        if self.body_overlap(output, 4) || !memory.write_word(output, previous) {
            return INVALID_PARAMETER;
        }
        let handle = self.create(Kind::Event, Owner(189));
        if handle.0 == 0 {
            return INSUFFICIENT_RESOURCES;
        }
        self.entries[handle.slot()].as_mut().unwrap().state = ObjectState::Event {
            manual,
            signaled: false,
        };
        if !memory.write_word(output, handle.0) {
            self.release(handle, &mut NoFileBodies);
            return INVALID_PARAMETER;
        }
        0
    }
    /// Internal clear is supported by C; it is not an exported NtClearEvent handler.
    pub fn event_set(&mut self, handle: Handle, signaled: bool) -> u32 {
        let Some(entry) = self.find(handle) else {
            return INVALID_HANDLE;
        };
        if entry.kind != Kind::Event {
            return TYPE_MISMATCH;
        }
        if let ObjectState::Event {
            signaled: value, ..
        } = &mut self.entries[handle.slot()].as_mut().unwrap().state
        {
            *value = signaled;
        }
        0
    }
    pub fn event_try_wait(&mut self, handle: Handle) -> Result<bool, u32> {
        let Some(entry) = self.find(handle) else {
            return Err(INVALID_HANDLE);
        };
        if entry.kind != Kind::Event {
            return Err(TYPE_MISMATCH);
        }
        let ObjectState::Event { manual, signaled } =
            &mut self.entries[handle.slot()].as_mut().unwrap().state
        else {
            unreachable!()
        };
        let acquired = *signaled;
        if !*manual {
            *signaled = false;
        }
        Ok(acquired)
    }
    pub fn release(&mut self, handle: Handle, files: &mut impl FileObjects) -> bool {
        if self.find(handle).is_none() {
            return false;
        }
        let entry = self.entries[handle.slot()].as_mut().unwrap();
        if let ObjectState::Thread {
            open,
            body,
            body_references,
        } = &mut entry.state
        {
            *open = false;
            entry.references = *body_references;
            if body.0 == 0 {
                self.free_entry(handle.slot());
            }
        } else if entry.references > 1 {
            entry.references -= 1;
        } else {
            // C invokes a file-provider release only for bound bodies. Provider decides.
            let file = entry.kind == Kind::File;
            self.free_entry(handle.slot());
            if file {
                files.release(handle);
            }
        }
        true
    }
    fn thread_at(&self, body: GuestAddress) -> Option<usize> {
        if body.0 == 0 {
            return None;
        }
        self.entries.iter().position(|entry| matches!(entry.as_ref().map(|e| &e.state),Some(ObjectState::Thread {body:b,..}) if *b==body))
    }
    fn body_overlap(&self, address: GuestAddress, length: u32) -> bool {
        self.entries.iter().flatten().any(|e| matches!(e.state,ObjectState::Thread {body,..} if body.0!=0 && overlaps(address,length,body,BODY_SPAN)))
    }
    pub fn bind_thread_body(
        &mut self,
        handle: Handle,
        owner: Owner,
        body: GuestAddress,
        memory: &impl GuestMemory,
    ) -> bool {
        let valid = self.find(handle).is_some_and(|e| {
            e.owner == owner
                && matches!(
                    e.state,
                    ObjectState::Thread {
                        body: GuestAddress(0),
                        body_references: 0,
                        ..
                    }
                )
        });
        if !valid
            || body.0 == 0
            || body.0 & 3 != 0
            || body.0 as u64 + BODY_SPAN as u64 > 0x100000000
            || !memory.readable(body, BODY_SPAN)
            || self.body_overlap(body, BODY_SPAN)
        {
            return false;
        }
        if let ObjectState::Thread { body: b, .. } =
            &mut self.entries[handle.slot()].as_mut().unwrap().state
        {
            *b = body;
        }
        true
    }
    /// Publish an already provisioned body; output failure removes only the binding.
    pub fn publish_thread_body(
        &mut self,
        handle: Handle,
        owner: Owner,
        body: GuestAddress,
        output: GuestAddress,
        memory: &mut impl GuestMemory,
    ) -> bool {
        if output.0 == 0 || !self.bind_thread_body(handle, owner, body, memory) {
            return false;
        }
        let published = !self.body_overlap(output, 4) && memory.write_word(output, handle.0);
        if !published
            && let ObjectState::Thread { body, .. } =
                &mut self.entries[handle.slot()].as_mut().unwrap().state
        {
            *body = GuestAddress(0);
        }
        published
    }
    pub fn write_optional_thread_id(
        &self,
        output: GuestAddress,
        id: u32,
        memory: &mut impl GuestMemory,
    ) -> bool {
        output.0 != 0 && !self.body_overlap(output, 4) && memory.write_word(output, id)
    }
    pub fn rollback_thread_body(&mut self, handle: Handle, owner: Owner) -> bool {
        if !self.any(handle).is_some_and(|e| {
            e.owner == owner
                && matches!(
                    e.state,
                    ObjectState::Thread {
                        body_references: 0,
                        ..
                    }
                )
        }) {
            return false;
        }
        self.free_entry(handle.slot());
        true
    }
    pub fn referenced_thread_body(&self, body: GuestAddress) -> Option<&Entry> {
        self.thread_at(body).and_then(|slot| self.entries[slot].as_ref()).filter(|e| matches!(e.state, ObjectState::Thread {body_references, ..} if body_references > 0))
    }
    pub fn detach_thread_bodies(&mut self) -> bool {
        if self.entries.iter().flatten().any(|e| matches!(e.state,ObjectState::Thread {body,body_references,..} if body.0!=0 && body_references!=0)) { return false; }
        for slot in 0..256 {
            if self.entries[slot]
                .as_ref()
                .is_some_and(|e| matches!(e.state,ObjectState::Thread {body,..} if body.0!=0))
            {
                self.free_entry(slot);
            }
        }
        true
    }
    /// Controlled oracle fixture: not used by a host adapter or to bypass ABI checks.
    #[cfg(feature = "oracle-fixtures")]
    pub fn seed_reference_counts(&mut self, handle: Handle, references: u32, body_references: u32) {
        if let Some(e) = self.entries[handle.slot()]
            .as_mut()
            .filter(|e| e.handle == handle)
        {
            e.references = references;
            if let ObjectState::Thread {
                body_references: b, ..
            } = &mut e.state
            {
                *b = body_references;
            }
        }
    }
    /// Test-only retirement witness, supplied before any slot has been issued.
    #[cfg(feature = "oracle-fixtures")]
    pub fn seed_retirement_boundary(&mut self) {
        self.reset();
        self.generations[0] = 16383;
    }
    /// Wait-side attachment supplies ownership; no wait policy is implemented here.
    pub fn set_mutant_ownership(&mut self, handle: Handle, owner: Owner, depth: u32) -> bool {
        if self.find(handle).is_none() {
            return false;
        }
        if let ObjectState::Mutant { owner: o, depth: d } =
            &mut self.entries[handle.slot()].as_mut().unwrap().state
        {
            *o = owner;
            *d = depth;
            true
        } else {
            false
        }
    }
    fn fast_reference(
        &mut self,
        object: GuestAddress,
        increment: bool,
        files: &mut impl FileObjects,
    ) {
        if let Some(slot) = self.thread_at(object) {
            let e = self.entries[slot].as_mut().unwrap();
            if let ObjectState::Thread {
                body_references, ..
            } = &mut e.state
            {
                if increment {
                    if *body_references > 0
                        && *body_references < u32::MAX
                        && e.references < u32::MAX
                    {
                        *body_references += 1;
                        e.references += 1;
                    } else {
                        self.log(Diagnostic::BodyUnavailable);
                    }
                } else if *body_references > 0 {
                    *body_references -= 1;
                    e.references = e.references.wrapping_sub(1);
                }
            }
            return;
        }
        if if increment {
            files.reference(self, object)
        } else {
            files.dereference(self, object)
        } {
            return;
        }
        let handle = Handle(object.0);
        if self
            .find(handle)
            .is_some_and(|e| e.kind != Kind::Thread && e.kind != Kind::Event)
        {
            let e = self.entries[handle.slot()].as_mut().unwrap();
            if increment {
                e.references = e.references.wrapping_add(1);
            } else if e.references > 0 {
                e.references -= 1;
            }
        }
    }
    fn reference_by_handle(
        &mut self,
        memory: &mut impl GuestMemory,
        mut handle: Handle,
        object_type: u32,
        output: GuestAddress,
        caller: Owner,
        files: &mut impl FileObjects,
    ) -> u32 {
        let own = handle.0 == 0xfffffffe;
        if own {
            handle = Handle(caller.0);
            if caller.0 == 0
                || !self
                    .any(handle)
                    .is_some_and(|e| matches!(e.state,ObjectState::Thread {body,..} if body.0!=0))
            {
                if output.0 != 0 {
                    self.log(Diagnostic::InvalidHandle);
                }
                return INVALID_HANDLE;
            }
        }
        if self.find(handle).is_some_and(|e| e.kind == Kind::Event) {
            self.log(Diagnostic::Unsupported);
            return NOT_IMPLEMENTED;
        }
        if output.0 == 0 {
            self.log(Diagnostic::WriteFailed);
            return INVALID_PARAMETER;
        }
        let Some(entry) = (if own {
            self.any(handle)
        } else {
            self.find(handle)
        })
        .cloned() else {
            self.log(Diagnostic::InvalidHandle);
            return INVALID_HANDLE;
        };
        if matches!(entry.state,ObjectState::Thread {body,body_references,..} if body.0==0 || body_references==u32::MAX || entry.references==u32::MAX)
        {
            self.log(Diagnostic::BodyUnavailable);
            return NOT_IMPLEMENTED;
        }
        if self.body_overlap(output, 4) {
            return INVALID_PARAMETER;
        }
        if let ObjectState::Thread { body, .. } = entry.state
            && !memory.readable(body, BODY_SPAN)
        {
            return ACCESS_VIOLATION;
        }
        if entry.kind == Kind::File
            && let Some(status) =
                files.reference_by_handle(self, memory, handle, object_type, output)
        {
            return status;
        }
        if object_type != 0 {
            self.diagnostics.unchecked_types = self.diagnostics.unchecked_types.wrapping_add(1);
            self.log(Diagnostic::UncheckedType);
        }
        let published = if let ObjectState::Thread { body, .. } = entry.state {
            body.0
        } else {
            handle.0
        };
        if !memory.write_word(output, published) {
            self.log(Diagnostic::WriteFailed);
            return INVALID_PARAMETER;
        }
        let e = self.entries[handle.slot()].as_mut().unwrap();
        e.references = e.references.wrapping_add(1);
        if let ObjectState::Thread {
            body_references, ..
        } = &mut e.state
        {
            *body_references += 1;
        }
        self.diagnostics.references_by_handle =
            self.diagnostics.references_by_handle.wrapping_add(1);
        0
    }
    pub fn dispatch(
        &mut self,
        ordinal: u32,
        frame: Option<CallFrame>,
        memory: &mut impl GuestMemory,
        caller: Owner,
        files: &mut impl FileObjects,
    ) -> CallOutcome {
        if ![187, 192, 197, 221, 246, 250, 251].contains(&ordinal) {
            return CallOutcome::Unregistered;
        }
        if ordinal == 250 || ordinal == 251 {
            if let Some(ecx) = frame.and_then(|f| f.ecx) {
                self.fast_reference(GuestAddress(ecx), ordinal == 251, files);
            } else {
                self.log(Diagnostic::MissingFrame);
            }
            return CallOutcome::Returned(0);
        }
        let arity = match ordinal {
            187 => 1,
            221 => 2,
            _ => 3,
        };
        let mut args = [0; 3];
        for (index, arg) in args.iter_mut().enumerate().take(arity) {
            let Some(value) = frame.and_then(|f| f.argument(memory, index)) else {
                self.log(Diagnostic::MissingFrame);
                return CallOutcome::Returned(if ordinal == 187 {
                    INVALID_HANDLE
                } else {
                    INVALID_PARAMETER
                });
            };
            *arg = value;
        }
        let result = match ordinal {
            187 => {
                if self.release(Handle(args[0]), files) {
                    0
                } else {
                    self.diagnostics.bad_close = self.diagnostics.bad_close.wrapping_add(1);
                    self.log(Diagnostic::InvalidHandle);
                    INVALID_HANDLE
                }
            }
            192 => self.create_mutant(memory, args, files),
            197 => self.duplicate(memory, args, files),
            221 => self.release_mutant(args, caller),
            246 => self.reference_by_handle(
                memory,
                Handle(args[0]),
                args[1],
                GuestAddress(args[2]),
                caller,
                files,
            ),
            _ => unreachable!(),
        };
        CallOutcome::Returned(result)
    }
    fn create_mutant(
        &mut self,
        memory: &mut impl GuestMemory,
        args: [u32; 3],
        files: &mut impl FileObjects,
    ) -> u32 {
        if args[0] == 0 {
            self.log(Diagnostic::WriteFailed);
            return INVALID_PARAMETER;
        }
        let mut fields = [0; 3];
        if args[1] != 0 {
            for index in [1, 0, 2] {
                let address = args[1].checked_add(index as u32 * 4).unwrap_or(0);
                let Some(word) = memory.read_word(GuestAddress(address)) else {
                    self.log(Diagnostic::MissingFrame);
                    return INVALID_PARAMETER;
                };
                fields[index] = word;
            }
        }
        if fields[1] != 0 {
            self.log(Diagnostic::Unsupported);
            return NOT_IMPLEMENTED;
        }
        if args[1] != 0 && (fields[0] != 0 || fields[2] != 0) {
            self.log(Diagnostic::IgnoredAttributes);
        }
        if args[2] & 255 != 0 {
            self.log(Diagnostic::Unsupported);
            return NOT_IMPLEMENTED;
        }
        let handle = self.create(Kind::Mutant, Owner(192));
        if handle.0 == 0 {
            return INSUFFICIENT_RESOURCES;
        }
        if !memory.write_word(GuestAddress(args[0]), handle.0) {
            self.release(handle, files);
            self.log(Diagnostic::WriteFailed);
            return INVALID_PARAMETER;
        }
        self.log(Diagnostic::Created);
        0
    }
    fn duplicate(
        &mut self,
        memory: &mut impl GuestMemory,
        args: [u32; 3],
        files: &mut impl FileObjects,
    ) -> u32 {
        if args[2] != 2 || args[1] == 0 || args[0] >= 0xfffffffe {
            self.log(Diagnostic::Unsupported);
            return NOT_IMPLEMENTED;
        }
        let Some(entry) = self.find(Handle(args[0])) else {
            self.log(Diagnostic::InvalidHandle);
            return INVALID_HANDLE;
        };
        let ObjectState::File { duplicate_root } = entry.state else {
            self.log(Diagnostic::Unsupported);
            return NOT_IMPLEMENTED;
        };
        let root = if duplicate_root.0 != 0 {
            duplicate_root
        } else {
            entry.handle
        };
        let handle = self.create(Kind::File, Owner(197));
        if handle.0 == 0 {
            return INSUFFICIENT_RESOURCES;
        }
        self.entries[handle.slot()].as_mut().unwrap().state = ObjectState::File {
            duplicate_root: root,
        };
        if !memory.write_word(GuestAddress(args[1]), handle.0) {
            self.release(handle, files);
            self.log(Diagnostic::WriteFailed);
            return INVALID_PARAMETER;
        }
        self.log(Diagnostic::Created);
        0
    }
    fn release_mutant(&mut self, args: [u32; 3], caller: Owner) -> u32 {
        if args[1] != 0 {
            self.log(Diagnostic::Unsupported);
            return NOT_IMPLEMENTED;
        }
        let handle = Handle(args[0]);
        let Some(entry) = self.find(handle) else {
            self.log(Diagnostic::InvalidHandle);
            return INVALID_HANDLE;
        };
        if entry.kind != Kind::Mutant {
            self.log(Diagnostic::Unsupported);
            return TYPE_MISMATCH;
        }
        let ObjectState::Mutant { owner, depth } =
            &mut self.entries[handle.slot()].as_mut().unwrap().state
        else {
            unreachable!()
        };
        if *depth == 0 || *owner != caller {
            self.diagnostics.unowned_releases = self.diagnostics.unowned_releases.wrapping_add(1);
            self.log(Diagnostic::Unowned);
            return NOT_OWNED;
        }
        *depth -= 1;
        if *depth == 0 {
            *owner = Owner(0);
        }
        0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallOutcome {
    Returned(u32),
    Unregistered,
}
