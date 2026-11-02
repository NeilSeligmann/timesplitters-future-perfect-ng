// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic admission detector (T3906): a capture may only be called a complete memory stream when every class
//! of guest-memory writer has a witness. The census below is of what the tree really hooks; a class without a hook can
//! never be witnessed, so no stream is admitted until every class gains one. This is a refusal gate, not evidence.
use crate::events;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriterClass {
    /// C `kernel_guest_write_*` helpers (observer, `kernel_call.c`).
    HostHelper,
    /// Recompiled-guest stores via `t2cap_st*` (every CAP_ST*, PUSH32, XMM/MMX store); TSFP_CAPTURE builds only.
    LiftedStore,
    /// `t2cap_memcpy/memmove` (guest copies).
    GuestCopy,
    /// `t2cap_memset` (rep stos fills).
    GuestFill,
    /// `t2cap_atomic_*` (lock xadd / cmpxchg on host pointers).
    GuestAtomic,
    /// Rust ports' `MappedGuest::write*` (`tsfp_hostmem_direct_write`, fenced like the range writers).
    RustMapped,
    /// Raw host-pointer writers outside every hook. Covered by the census guards, not by an op id: `kernel_guest_at()` is
    /// read-only by type outside its accessor (tests/test_t3906_kernel_guest_at.py) and every other raw writer is hooked or
    /// classified non-guest in tools/data/raw_guest_write_allowlist.json (tests/test_t3906_raw_guest_writes.py, shrink-only).
    /// A new raw writer fails those guards in landing verify, so the class is hooked with no stream witness of its own.
    DirectHostPointer,
}

pub const ALL: [WriterClass; 7] = [
    WriterClass::HostHelper,
    WriterClass::LiftedStore,
    WriterClass::GuestCopy,
    WriterClass::GuestFill,
    WriterClass::GuestAtomic,
    WriterClass::RustMapped,
    WriterClass::DirectHostPointer,
];

/// Writer classes that currently have an actual hook feeding the capture stream (tree census, 2026-10-30).
pub const HOOKED: [WriterClass; 7] = [
    WriterClass::HostHelper,
    WriterClass::LiftedStore,
    WriterClass::GuestCopy,
    WriterClass::GuestFill,
    WriterClass::GuestAtomic,
    WriterClass::RustMapped,
    WriterClass::DirectHostPointer,
];

/// The capture operation id of each hooked class, as declared by the producer.
#[derive(Clone, Copy, Debug)]
pub struct ClassOps {
    pub host_write: u32,
    pub lifted_store: u32,
    pub guest_copy: u32,
    pub guest_fill: u32,
    pub guest_atomic: u32,
    pub rust_mapped: u32,
}

impl ClassOps {
    pub fn op(&self, class: WriterClass) -> Option<u32> {
        match class {
            WriterClass::HostHelper => Some(self.host_write),
            WriterClass::LiftedStore => Some(self.lifted_store),
            WriterClass::GuestCopy => Some(self.guest_copy),
            WriterClass::GuestFill => Some(self.guest_fill),
            WriterClass::GuestAtomic => Some(self.guest_atomic),
            WriterClass::RustMapped => Some(self.rust_mapped),
            WriterClass::DirectHostPointer => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The class has no hook in the tree (or the supplied census omits it).
    Unhooked(WriterClass),
    /// The class is hooked but the stream carries no event of it, so the hook is not shown to have run.
    NoWitness(WriterClass),
    /// Two classes share one op id, so witnesses cannot be told apart.
    AmbiguousOps,
}

fn call_ops(events: &[(u32, Vec<u8>)]) -> Vec<u32> {
    events
        .iter()
        .filter(|(kind, _)| *kind == events::CALL_BEGIN)
        .filter_map(|(_, d)| Some(u32::from_le_bytes(d.get(24..28)?.try_into().ok()?)))
        .collect()
}

/// Admit a purported complete stream only if every writer class in `ALL` is hooked per `hooked` and witnessed in
/// `events`. The first refusal in `ALL` order is returned.
pub fn admit_complete(
    events: &[(u32, Vec<u8>)],
    ops: ClassOps,
    hooked: &[WriterClass],
) -> Result<(), Refusal> {
    let ids: Vec<u32> = ALL.iter().filter_map(|c| ops.op(*c)).collect();
    if ids.iter().enumerate().any(|(i, id)| ids[..i].contains(id)) {
        return Err(Refusal::AmbiguousOps);
    }
    let seen = call_ops(events);
    for class in ALL {
        if !hooked.contains(&class) {
            return Err(Refusal::Unhooked(class));
        }
        if class == WriterClass::DirectHostPointer {
            continue; // guard-covered (see the variant), nothing to witness in the stream
        }
        match ops.op(class) {
            Some(op) if seen.contains(&op) => {}
            _ => return Err(Refusal::NoWitness(class)),
        }
    }
    Ok(())
}
