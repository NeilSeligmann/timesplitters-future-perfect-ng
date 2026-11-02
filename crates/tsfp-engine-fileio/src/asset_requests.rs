//! Rust-owned asynchronous asset requests (T1663).
//!
//! Request slots, asynchronous I/O file identities, and destination owners have
//! separate typed identities. Retiring a request revokes its destination lease
//! immediately; the completion queue keeps its byte buffer until completion and
//! only then returns it to the opt-in reusable-buffer pool.

use std::collections::{HashMap, VecDeque};

use crate::async_io::{AsyncIo, CompletionHost, Read, Volume};

pub const REQUEST_SLOT_COUNT: usize = 200;
const REQUEST_TABLE_BASE: u32 = 0x0066_1370;
const REQUEST_SLOT_STRIDE: u32 = 0x44;
const REQUEST_LIST_SENTINEL: u32 = 0x0066_4890;

/// A request's slot and generation. Numeric equality with file handles is not identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssetRequestId {
    pub slot: u16,
    pub generation: u32,
}

/// Generation of the archive/resource owner used to validate a destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AssetEpoch(pub u64);

/// A write target whose owner must remain valid through completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DestinationLease {
    pub address: u32,
    pub owner: u64,
    pub generation: u32,
    pub epoch: AssetEpoch,
}

/// Inputs needed to enqueue a native asset request.
#[derive(Debug)]
pub struct AssetRequestInput {
    pub slot: usize,
    pub file: u32,
    pub file_base: u32,
    pub destination: DestinationLease,
    pub offset: u32,
    pub size: u32,
    pub timestamp: u32,
    pub clock: u32,
    pub close_file_on_retire: bool,
    pub file_handle: u32,
    /// Existing slot word at +0x24; enqueue preserves it until retirement clears its low word.
    pub request_word: u32,
    pub volume: Volume,
    /// Owned source bytes transfer to the completion queue on successful enqueue.
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnqueueErrorKind {
    InvalidSlot,
    SlotBusy,
    CompletionQueueFull,
}

/// An enqueue refusal returns the exact bytes and request metadata to the caller.
#[derive(Debug)]
pub struct EnqueueFailure {
    pub kind: EnqueueErrorKind,
    pub request: AssetRequestInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetRequestStatus {
    Retired,
    Pending,
    Completed,
    Failed,
}

/// The legacy slot words that form the attachment point to existing producers.
/// These values are a compatibility projection; native callers use the typed ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacySlotSnapshot {
    pub file_handle: u32,
    pub close_file: bool,
    pub request_word: u32,
    pub file_base: u32,
    pub reserved_0c: u32,
    pub reserved_10: u32,
    pub file_offset: u32,
    pub aligned_size: u32,
    pub timestamp_delta: u32,
    pub destination: u32,
    pub state: u32,
    pub previous: u32,
    pub next: u32,
}

impl LegacySlotSnapshot {
    fn empty() -> Self {
        Self {
            file_handle: u32::MAX,
            close_file: false,
            request_word: 0,
            file_base: 0,
            reserved_0c: 0,
            reserved_10: 0,
            file_offset: 0,
            aligned_size: 0,
            timestamp_delta: 0,
            destination: 0,
            state: 0,
            previous: 0,
            next: 0,
        }
    }
}

#[derive(Clone, Copy)]
struct ActiveRequest {
    id: AssetRequestId,
    ticket: u32,
    destination: DestinationLease,
    status: AssetRequestStatus,
}

#[derive(Default)]
struct Slot {
    generation: u32,
    request: Option<ActiveRequest>,
    legacy: Option<LegacySlotSnapshot>,
}

struct Ticket {
    id: AssetRequestId,
    destination: Option<DestinationLease>,
    write_succeeded: Option<bool>,
}

/// Host operations needed by this native service. `write_asset` must validate
/// owner/generation/epoch before touching the destination memory.
pub trait AssetRequestHost: CompletionHost {
    fn write_asset(&mut self, destination: DestinationLease, bytes: &[u8]) -> bool;
    fn close_asset_file(&mut self, handle: u32);
}

/// Owns request-slot state and composes with the existing owned `AsyncIo` queue.
pub struct AssetRequests {
    completions: AsyncIo,
    slots: Vec<Slot>,
    queue: VecDeque<u16>,
    tickets: HashMap<u32, Ticket>,
    selected: Option<AssetRequestId>,
    active_count: u32,
    next_ticket: u32,
}

impl Default for AssetRequests {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetRequests {
    pub fn new() -> Self {
        let mut completions = AsyncIo::default();
        completions.set_enabled(true);
        completions.enable_owned_buffer_reuse();
        Self {
            completions,
            slots: (0..REQUEST_SLOT_COUNT).map(|_| Slot::default()).collect(),
            queue: VecDeque::new(),
            tickets: HashMap::new(),
            selected: None,
            active_count: 0,
            next_ticket: 1,
        }
    }

    /// Get a scratch buffer from the completion service's reusable owned pool.
    pub fn take_owned_buffer(&mut self, capacity: usize) -> Vec<u8> {
        self.completions.take_owned_buffer(capacity)
    }

    pub fn pending_completions(&self) -> u32 {
        self.completions.stats().pending
    }

    pub fn enqueue(
        &mut self,
        host: &impl AssetRequestHost,
        request: AssetRequestInput,
    ) -> Result<AssetRequestId, EnqueueFailure> {
        if request.slot >= REQUEST_SLOT_COUNT {
            return Err(EnqueueFailure {
                kind: EnqueueErrorKind::InvalidSlot,
                request,
            });
        }
        if self.slots[request.slot].request.is_some() {
            return Err(EnqueueFailure {
                kind: EnqueueErrorKind::SlotBusy,
                request,
            });
        }

        let slot_index = request.slot;
        let generation = self.slots[slot_index].generation.wrapping_add(1).max(1);
        let id = AssetRequestId {
            slot: slot_index as u16,
            generation,
        };
        let ticket = self.next_ticket();
        let legacy = LegacySlotSnapshot {
            file_handle: request.file_handle,
            close_file: request.close_file_on_retire,
            request_word: request.request_word,
            file_base: request.file_base,
            reserved_0c: 0,
            reserved_10: 0,
            file_offset: request.file_base.wrapping_add(request.offset),
            aligned_size: request.size.wrapping_add(0x7ff) & 0xffff_f800,
            timestamp_delta: request.clock.wrapping_sub(request.timestamp),
            destination: request.destination.address,
            state: 1,
            previous: REQUEST_LIST_SENTINEL,
            next: self
                .queue
                .front()
                .copied()
                .map(slot_address)
                .unwrap_or(REQUEST_LIST_SENTINEL),
        };
        let mut request = request;
        let read = Read {
            file: request.file,
            event: ticket,
            iosb: 0,
            buffer: ticket,
            requested: request.size,
            volume: request.volume,
            data: std::mem::take(&mut request.bytes),
        };
        if let Err(read) = self.completions.submit(host, read) {
            let mut request = request;
            request.bytes = read.data;
            return Err(EnqueueFailure {
                kind: EnqueueErrorKind::CompletionQueueFull,
                request,
            });
        }

        self.slots[slot_index].generation = generation;
        self.slots[slot_index].legacy = Some(legacy);
        self.slots[slot_index].request = Some(ActiveRequest {
            id,
            ticket,
            destination: request.destination,
            status: AssetRequestStatus::Pending,
        });
        self.queue.push_front(slot_index as u16);
        self.tickets.insert(
            ticket,
            Ticket {
                id,
                destination: Some(request.destination),
                write_succeeded: None,
            },
        );
        self.update_links();
        Ok(id)
    }

    /// Service due reads. Retired requests still drain their owned bytes, but
    /// the completion adapter drops their revoked destination lease.
    pub fn service(&mut self, host: &mut impl AssetRequestHost) -> u32 {
        let mut adapter = CompletionAdapter {
            slots: &mut self.slots,
            tickets: &mut self.tickets,
            host,
        };
        self.completions.service(&mut adapter)
    }

    pub fn complete_next(&mut self, host: &mut impl AssetRequestHost) -> bool {
        let mut adapter = CompletionAdapter {
            slots: &mut self.slots,
            tickets: &mut self.tickets,
            host,
        };
        self.completions.complete_next(&mut adapter)
    }

    /// Retire an exact generation. A stale ID cannot clear a newly reused slot.
    pub fn retire(&mut self, host: &mut impl AssetRequestHost, id: AssetRequestId) -> bool {
        let Some(slot) = self.slots.get_mut(usize::from(id.slot)) else {
            return false;
        };
        let Some(request) = slot.request else {
            return false;
        };
        if request.id != id {
            return false;
        }

        let legacy = slot.legacy.get_or_insert_with(LegacySlotSnapshot::empty);
        if legacy.close_file {
            host.close_asset_file(legacy.file_handle);
        }
        legacy.request_word &= 0xffff_0000;
        legacy.file_handle = u32::MAX;
        legacy.previous = 0;
        legacy.next = 0;
        slot.request = None;
        self.queue.retain(|queued| *queued != id.slot);
        if let Some(ticket) = self.tickets.get_mut(&request.ticket) {
            // This entry stays reserved until the I/O service acknowledges the
            // completion, preventing token reuse while it still owns the bytes.
            ticket.destination = None;
        }
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.active_count = self.active_count.wrapping_sub(1);
        self.update_links();
        true
    }

    pub fn status(&self, id: AssetRequestId) -> AssetRequestStatus {
        self.slots
            .get(usize::from(id.slot))
            .and_then(|slot| slot.request)
            .filter(|request| request.id == id)
            .map_or(AssetRequestStatus::Retired, |request| request.status)
    }

    pub fn legacy_slot(&self, slot: usize) -> Option<LegacySlotSnapshot> {
        self.slots.get(slot).and_then(|entry| entry.legacy)
    }

    pub fn legacy_head(&self) -> u32 {
        self.queue
            .front()
            .copied()
            .map(slot_address)
            .unwrap_or(REQUEST_LIST_SENTINEL)
    }

    pub fn legacy_tail(&self) -> u32 {
        self.queue
            .back()
            .copied()
            .map(slot_address)
            .unwrap_or(REQUEST_LIST_SENTINEL)
    }

    pub fn set_legacy_counters(&mut self, active_count: u32, selected: Option<AssetRequestId>) {
        self.active_count = active_count;
        self.selected = selected;
    }

    pub fn legacy_active_count(&self) -> u32 {
        self.active_count
    }

    pub fn legacy_selected(&self) -> Option<AssetRequestId> {
        self.selected
    }

    fn next_ticket(&mut self) -> u32 {
        loop {
            let candidate = self.next_ticket.max(1);
            self.next_ticket = candidate.wrapping_add(1).max(1);
            if !self.tickets.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    fn update_links(&mut self) {
        for (position, &slot_index) in self.queue.iter().enumerate() {
            let previous = if position == 0 {
                REQUEST_LIST_SENTINEL
            } else {
                slot_address(self.queue[position - 1])
            };
            let next = self
                .queue
                .get(position + 1)
                .copied()
                .map(slot_address)
                .unwrap_or(REQUEST_LIST_SENTINEL);
            if let Some(legacy) = self.slots[usize::from(slot_index)].legacy.as_mut() {
                legacy.previous = previous;
                legacy.next = next;
            }
        }
    }
}

fn slot_address(slot: u16) -> u32 {
    REQUEST_TABLE_BASE + u32::from(slot) * REQUEST_SLOT_STRIDE
}

struct CompletionAdapter<'a, H> {
    slots: &'a mut [Slot],
    tickets: &'a mut HashMap<u32, Ticket>,
    host: &'a mut H,
}

impl<H: AssetRequestHost> CompletionHost for CompletionAdapter<'_, H> {
    fn now(&self) -> u64 {
        self.host.now()
    }
    fn advance_to(&mut self, ticks: u64) {
        self.host.advance_to(ticks);
    }
    fn identity(&self, file: u32) -> u32 {
        self.host.identity(file)
    }
    fn identity_live(&self, identity: u32) -> bool {
        self.host.identity_live(identity)
    }
    fn write_buffer(&mut self, ticket_id: u32, bytes: &[u8]) -> bool {
        let Some(ticket) = self.tickets.get_mut(&ticket_id) else {
            return true;
        };
        let Some(destination) = ticket.destination else {
            return true;
        };
        let slot = &self.slots[usize::from(ticket.id.slot)];
        if slot
            .request
            .map(|request| (request.id, request.destination))
            != Some((ticket.id, destination))
        {
            return true;
        }
        let succeeded = self.host.write_asset(destination, bytes);
        ticket.write_succeeded = Some(succeeded);
        succeeded
    }
    fn write_iosb(&mut self, address: u32, status: u32, information: u32) {
        self.host.write_iosb(address, status, information);
    }
    fn signal_event(&mut self, ticket_id: u32) {
        let Some(ticket) = self.tickets.remove(&ticket_id) else {
            return;
        };
        if ticket.destination.is_none() {
            return;
        }
        let slot = &mut self.slots[usize::from(ticket.id.slot)];
        if let Some(request) = slot
            .request
            .as_mut()
            .filter(|request| request.id == ticket.id)
        {
            request.status = if ticket.write_succeeded.unwrap_or(true) {
                AssetRequestStatus::Completed
            } else {
                AssetRequestStatus::Failed
            };
        }
    }
    fn event_signalled(&self, event: u32) -> bool {
        self.host.event_signalled(event)
    }
}

impl<H: AssetRequestHost> AssetRequestHost for CompletionAdapter<'_, H> {
    fn write_asset(&mut self, destination: DestinationLease, bytes: &[u8]) -> bool {
        self.host.write_asset(destination, bytes)
    }
    fn close_asset_file(&mut self, handle: u32) {
        self.host.close_asset_file(handle);
    }
}
