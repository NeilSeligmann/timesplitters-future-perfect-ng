// SPDX-License-Identifier: GPL-3.0-or-later
//! T2394 typed motion-contact attachment and deferred physics events.
//!
//! The scope is the established delivery boundary: typed owner identities, a
//! borrowed frame contact, immediate/deferred order, and the deferred LIFO queue.
//! Geometry, contact admission, damage/effects, body lifetime and the surrounding
//! solver remain separate services. The guest-memory path is unchanged by default.

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnerHandle(pub u32);

/// Opaque address identity carried only across the temporary guest callback
/// adapter. Native handlers use the leased `PhysicsContact` instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContactGuestId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContactIndex(pub usize);

/// The fields whose identity/order is established at the contact delivery edge.
/// The record remains owned by its frame arena through immediate and deferred
/// callbacks; it is not copied into a detached contact snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsContact {
    pub kind: u16,
    pub owner_a: Option<OwnerHandle>,
    pub owner_b: Option<OwnerHandle>,
    /// Some callback ABIs expose no normal alongside the contact pointer. Keep
    /// that absence explicit instead of manufacturing a zero vector.
    pub horizontal_normal: Option<[f32; 2]>,
    /// Deferred rows do not contain the speed result; `None` preserves that
    /// absence instead of fabricating a value for the native contact model.
    pub relative_speed_bits: Option<u32>,
    pub next: Option<ContactIndex>,
    /// The lifted list-link address carried only while a live contact is
    /// imported through the temporary host adapter. Rust-owned contact lists
    /// use `next`; the host mirrors this value to guest `contact+0x34` after a
    /// registered callback mutates it.
    pub guest_next: Option<ContactGuestId>,
    pub guest_identity: Option<ContactGuestId>,
}

#[derive(Clone, Debug, Default)]
pub struct ContactArena {
    contacts: Rc<RefCell<Vec<PhysicsContact>>>,
}

impl ContactArena {
    /// Verification tier: Tier 0, typed lifetime contract.
    pub fn new(contacts: Vec<PhysicsContact>) -> Self {
        Self {
            contacts: Rc::new(RefCell::new(contacts)),
        }
    }

    /// Verification tier: Tier 0, typed lifetime contract.
    pub fn lease(&self, index: ContactIndex) -> Option<ContactLease> {
        (index.0 < self.contacts.borrow().len()).then(|| ContactLease {
            contacts: Rc::clone(&self.contacts),
            index,
        })
    }

    /// Add an imported contact snapshot to this frame-owned arena.
    /// The slot is append-only so previously issued leases remain stable.
    /// Verification tier: Tier 0, FABRICATED host-attachment inputs.
    pub fn push(&self, contact: PhysicsContact) -> ContactLease {
        let mut contacts = self.contacts.borrow_mut();
        let index = ContactIndex(contacts.len());
        contacts.push(contact);
        ContactLease {
            contacts: Rc::clone(&self.contacts),
            index,
        }
    }

    /// Verification tier: Tier 0, structural helper.
    pub fn len(&self) -> usize {
        self.contacts.borrow().len()
    }

    /// Verification tier: Tier 0, structural helper.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An owned reference to a stable contact slot. Holding a lease keeps its
/// Rust-owned frame arena alive across separate enqueue and drain calls. Slots
/// remain stable because the arena has no remove or recycle operation.
#[derive(Clone, Debug)]
pub struct ContactLease {
    contacts: Rc<RefCell<Vec<PhysicsContact>>>,
    index: ContactIndex,
}

impl ContactLease {
    /// Verification tier: Tier 0, typed contact identity helper.
    pub fn index(&self) -> ContactIndex {
        self.index
    }

    /// Return the opaque guest identity for the temporary lifted-callback
    /// adapter, if the contact was imported from guest memory.
    pub fn guest_identity(&self) -> Option<ContactGuestId> {
        self.with(|contact| contact.guest_identity).flatten()
    }

    /// Verification tier: Tier 0, typed contact borrow helper.
    pub fn with<R>(&self, f: impl FnOnce(&PhysicsContact) -> R) -> Option<R> {
        let contacts = self.contacts.borrow();
        contacts.get(self.index.0).map(f)
    }

    /// Verification tier: Tier 0, typed contact borrow helper.
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut PhysicsContact) -> R) -> Option<R> {
        let mut contacts = self.contacts.borrow_mut();
        contacts.get_mut(self.index.0).map(f)
    }
}

#[derive(Clone, Debug)]
pub struct PhysicsEvent {
    /// Event recipient is kept separate from `other`, the row's first word.
    pub recipient: OwnerHandle,
    pub other: Option<OwnerHandle>,
    /// Raw float bits preserve signed zero and NaN payloads across the attachment.
    pub horizontal_normal_bits: [u32; 2],
    pub contact: ContactLease,
}

impl PhysicsEvent {
    /// Verification tier: Tier 0, contact-row construction contract.
    pub fn from_contact(
        recipient: OwnerHandle,
        other: Option<OwnerHandle>,
        horizontal_normal: [f32; 2],
        contact: ContactLease,
        negate_normal: bool,
    ) -> PhysicsEvent {
        let mut bits = horizontal_normal.map(f32::to_bits);
        if negate_normal {
            bits[0] ^= 0x8000_0000;
            bits[1] ^= 0x8000_0000;
        }
        PhysicsEvent {
            recipient,
            other,
            horizontal_normal_bits: bits,
            contact: contact.clone(),
        }
    }
}

#[derive(Clone, Debug)]
struct EventRow {
    other: Option<OwnerHandle>,
    horizontal_normal_bits: [u32; 2],
    contact: ContactLease,
}

impl EventRow {
    fn from_event(event: PhysicsEvent) -> Self {
        Self {
            other: event.other,
            horizontal_normal_bits: event.horizontal_normal_bits,
            contact: event.contact,
        }
    }
}

/// The original queue is allocated before substeps and drained LIFO afterward.
/// The typed form has the same capacity/drop and callback-enqueue order. Its
/// buffers and contact leases are Rust-owned; an event keeps its frame arena alive.
#[derive(Debug)]
pub struct DeferredEvents {
    owners: Option<Vec<Option<OwnerHandle>>>,
    rows: Option<Vec<Option<EventRow>>>,
    capacity: usize,
    count: usize,
}

impl DeferredEvents {
    /// Verification tier: Tier 1 queue append/drain tests cover initialized storage.
    pub fn new(capacity: usize) -> Self {
        Self::with_storage(capacity, true, true)
    }

    /// Construct one of the partial-allocation states accepted by the legacy
    /// service. `drain` returns without clearing either remaining buffer unless
    /// both are present.
    /// Verification tier: Tier 0, partial-allocation contract test.
    pub fn with_storage(capacity: usize, owners: bool, rows: bool) -> Self {
        Self {
            owners: owners.then(|| vec![None; capacity]),
            rows: rows.then(|| vec![None; capacity]),
            capacity,
            count: 0,
        }
    }

    /// Verification tier: Tier 1 queue append/drain tests.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Verification tier: Tier 1 queue append/drain tests.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Verification tier: Tier 1 queue append/drain tests.
    pub fn is_allocated(&self) -> bool {
        self.owners.is_some() && self.rows.is_some()
    }

    /// Append one four-word event row. Capacity pressure drops the event silently.
    /// The typed source has no guest-pointer aliasing; normal/native calls pass the
    /// event fields directly, and count is published only after both buffers write.
    /// Verification tier: Tier 1, 2,500 fabricated 0x16C070 comparisons.
    pub fn enqueue(&mut self, event: PhysicsEvent) -> bool {
        if self.capacity == 0 || self.count >= self.capacity {
            return false;
        }
        let (Some(owners), Some(rows)) = (&mut self.owners, &mut self.rows) else {
            return false;
        };
        let slot = self.count;
        owners[slot] = Some(event.recipient);
        rows[slot] = Some(EventRow::from_event(event));
        self.count += 1;
        true
    }

    /// Verification tier: Tier 1 queue drain comparisons cover row reads.
    pub fn row(&self, slot: usize) -> Option<PhysicsEvent> {
        let recipient = self.owners.as_ref()?.get(slot)?.as_ref().copied()?;
        let row = self.rows.as_ref()?.get(slot)?.as_ref()?;
        Some(PhysicsEvent {
            recipient,
            other: row.other,
            horizontal_normal_bits: row.horizontal_normal_bits,
            contact: row.contact.clone(),
        })
    }

    /// Replace the mutable row payload after a foreign callback. The owner
    /// table slot is stable; only the four row words are callback-visible.
    /// Verification tier: Tier 0 callback-row mutation contract.
    pub fn replace_row(&mut self, slot: usize, event: PhysicsEvent) -> bool {
        let Some(owners) = self.owners.as_ref() else {
            return false;
        };
        if slot >= owners.len() || owners[slot].is_none() {
            return false;
        }
        let Some(rows) = self.rows.as_mut() else {
            return false;
        };
        let Some(row) = rows.get_mut(slot) else {
            return false;
        };
        *row = Some(EventRow::from_event(event));
        true
    }

    /// Pop the current LIFO slot and publish the decremented count before the
    /// caller crosses an ABI callback boundary. The row remains in its slot
    /// until the frame is finished, so a callback can append into the popped
    /// slot just as the lifted queue does.
    ///
    /// `Some((slot, None))` represents a popped slot whose owner or row storage
    /// is absent; callers skip its callback and continue draining. `None` means
    /// there is no allocated queue or no pending row.
    /// Verification tier: Tier 0, explicit foreign-callback attachment.
    pub fn pop_lifo(&mut self) -> Option<(usize, Option<PhysicsEvent>)> {
        if self.owners.is_none() || self.rows.is_none() || self.count == 0 {
            return None;
        }
        let slot = self.count - 1;
        self.count = slot;
        Some((slot, self.row(slot)))
    }

    /// Release the typed queue after its count reaches zero. A host adapter can
    /// call this after it has finished any lifted callbacks between `pop_lifo`
    /// calls. Verification tier: Tier 0 frame teardown contract.
    pub fn finish_drain(&mut self) -> bool {
        if self.count != 0 {
            return false;
        }
        let released = self.owners.is_some() && self.rows.is_some();
        self.owners = None;
        self.rows = None;
        self.capacity = 0;
        self.count = 0;
        released
    }

    /// Decrement/publish before delivery, resolve rows at the current slot, and
    /// reload count after a callback. Callbacks may append into the popped slot;
    /// skipped rows do not perform that reload.
    /// Verification tier: Tier 1, 2,500 fabricated 0x16C0D0 comparisons; callback
    /// mutation and enqueue are source-derived but not differentially tested.
    pub fn drain<H: OwnerCallbacks>(&mut self, handlers: &mut H) -> DrainReport {
        if self.owners.is_none() || self.rows.is_none() {
            return DrainReport::default();
        }
        let mut delivered = 0;
        let mut skipped = 0;
        while let Some((slot, event)) = self.pop_lifo() {
            let owner = event.as_ref().map(|event| event.recipient);
            let has_other = event
                .as_ref()
                .is_some_and(|event| event.other.is_some_and(|other| other.0 != 0));
            if let Some(owner) =
                owner.filter(|owner| owner.0 != 0 && has_other && handlers.has_callback(*owner))
            {
                handlers.deliver(owner, slot, self);
                delivered += 1;
            } else {
                skipped += 1;
            }
        }
        let released = self.finish_drain();
        DrainReport {
            delivered,
            skipped,
            released,
        }
    }
}

pub trait OwnerCallbacks {
    fn has_callback(&self, owner: OwnerHandle) -> bool;

    /// `slot` is a mutable queue slot, not an immutable event snapshot. The
    /// callback can inspect or replace it and can enqueue more events.
    fn deliver(&mut self, owner: OwnerHandle, slot: usize, queue: &mut DeferredEvents);
}

pub type DeferredOwnerContactHandler<C> =
    Box<dyn FnMut(&mut C, OwnerHandle, usize, &mut DeferredEvents) + 'static>;
pub type ImmediateOwnerContactHandler<C> =
    Box<dyn FnMut(&mut C, ImmediateContactEvent, &mut DeferredEvents) + 'static>;

/// Rust-owned callback table for typed contact recipients. The context and
/// handler registrations live on the native side; guest callback addresses and
/// guest row pointers are not part of this interface. This is the native
/// attachment point for ported physics owners, not a registration of lifted
/// guest callbacks.
#[derive(Default)]
pub struct NativeContactCallbacks<C> {
    context: C,
    handlers: Vec<(
        OwnerHandle,
        Option<DeferredOwnerContactHandler<C>>,
        Option<ImmediateOwnerContactHandler<C>>,
    )>,
}

impl<C> NativeContactCallbacks<C> {
    /// Create an empty native callback table with Rust-owned handler context.
    /// Verification tier: Tier 0; no live owner is registered by the host yet.
    pub fn new(context: C) -> Self {
        Self {
            context,
            handlers: Vec::new(),
        }
    }

    /// Register or replace a typed deferred callback for one owner.
    /// Verification tier: Tier 0 registry contract.
    pub fn register_deferred<H>(&mut self, owner: OwnerHandle, handler: H)
    where
        H: FnMut(&mut C, OwnerHandle, usize, &mut DeferredEvents) + 'static,
    {
        self.entry(owner).1 = Some(Box::new(handler));
    }

    /// Register or replace a typed immediate callback for one owner.
    /// Verification tier: Tier 0 registry contract.
    pub fn register_immediate<H>(&mut self, owner: OwnerHandle, handler: H)
    where
        H: FnMut(&mut C, ImmediateContactEvent, &mut DeferredEvents) + 'static,
    {
        self.entry(owner).2 = Some(Box::new(handler));
    }

    /// Report whether an owner has a Rust-owned immediate callback.
    /// Verification tier: Tier 0 registry contract.
    pub fn has_immediate_callback(&self, owner: OwnerHandle) -> bool {
        self.handlers
            .iter()
            .find(|entry| entry.0 == owner)
            .is_some_and(|entry| entry.2.is_some())
    }

    /// Remove all callback registrations for an owner.
    /// Verification tier: Tier 0 registry contract.
    pub fn unregister(&mut self, owner: OwnerHandle) -> bool {
        let Some(index) = self.handlers.iter().position(|entry| entry.0 == owner) else {
            return false;
        };
        self.handlers.swap_remove(index);
        true
    }

    /// Borrow the Rust-owned callback context for state inspection or updates.
    /// Verification tier: Tier 0 native state access.
    pub fn context(&self) -> &C {
        &self.context
    }

    /// Mutably borrow the Rust-owned callback context.
    /// Verification tier: Tier 0 native state access.
    pub fn context_mut(&mut self) -> &mut C {
        &mut self.context
    }

    fn entry(
        &mut self,
        owner: OwnerHandle,
    ) -> &mut (
        OwnerHandle,
        Option<DeferredOwnerContactHandler<C>>,
        Option<ImmediateOwnerContactHandler<C>>,
    ) {
        let index = self
            .handlers
            .iter()
            .position(|entry| entry.0 == owner)
            .unwrap_or_else(|| {
                self.handlers.push((owner, None, None));
                self.handlers.len() - 1
            });
        &mut self.handlers[index]
    }
}

impl<C> OwnerCallbacks for NativeContactCallbacks<C> {
    fn has_callback(&self, owner: OwnerHandle) -> bool {
        self.handlers
            .iter()
            .find(|entry| entry.0 == owner)
            .is_some_and(|entry| entry.1.is_some())
    }

    fn deliver(&mut self, owner: OwnerHandle, slot: usize, queue: &mut DeferredEvents) {
        let Some(handler) = self
            .handlers
            .iter_mut()
            .find(|entry| entry.0 == owner)
            .and_then(|entry| entry.1.as_mut())
        else {
            return;
        };
        handler(&mut self.context, owner, slot, queue);
    }
}

impl<C> ContactDeliveryHandlers for NativeContactCallbacks<C> {
    fn deliver_immediate(&mut self, event: ImmediateContactEvent, queue: &mut DeferredEvents) {
        let Some(handler) = self
            .handlers
            .iter_mut()
            .find(|entry| entry.0 == event.recipient)
            .and_then(|entry| entry.2.as_mut())
        else {
            return;
        };
        handler(&mut self.context, event, queue);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrainReport {
    pub delivered: usize,
    pub skipped: usize,
    pub released: bool,
}

/// Exact immediate/deferred ordering attachments from the selected contact pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactDelivery {
    Enqueue {
        recipient: OwnerHandle,
        other: OwnerHandle,
    },
    Immediate {
        recipient: OwnerHandle,
    },
}

/// Direct contact callback data resolved by the contact pass. The contact lease
/// keeps its arena alive for the callback, and speed stays in its original bit
/// representation as required by the caller ABI.
#[derive(Clone, Debug)]
pub struct ImmediateContactEvent {
    pub recipient: OwnerHandle,
    pub contact: ContactLease,
    pub relative_speed_bits: u32,
}

impl ImmediateContactEvent {
    /// The original contact pass dispatches each immediate callback to one of
    /// the contact's two endpoint owners. Reject stale or malformed adapter
    /// events before they can reach a Rust-owned handler.
    pub fn recipient_is_contact_endpoint(&self) -> bool {
        self.contact
            .with(|contact| {
                contact.owner_a == Some(self.recipient) || contact.owner_b == Some(self.recipient)
            })
            .unwrap_or(false)
    }
}

/// Native handlers for both immediate contact callbacks and deferred owner
/// callbacks. `OwnerCallbacks` resolves the deferred owner `+0x170` slot;
/// `deliver_immediate` handles the body `+0x1EC` callback edge.
pub trait ContactDeliveryHandlers: OwnerCallbacks {
    fn deliver_immediate(&mut self, event: ImmediateContactEvent, queue: &mut DeferredEvents);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactDispatchError {
    MissingContact,
    MissingContactOwner,
    MissingContactNormal,
    MissingContactSpeed,
    StateDoesNotMatchContact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactFrameStartError {
    PendingDeferredEvents,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContactDispatchReport {
    pub enabled: bool,
    pub immediate_callbacks: usize,
    pub deferred_queued: usize,
    pub deferred_dropped: usize,
}

/// Result of walking the typed contact list through its delivery callbacks.
/// `output` records the current node only after its callback returns, matching
/// the measured output-list publication edge.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContactPassReport {
    pub processed: usize,
    pub output: Vec<ContactIndex>,
    pub immediate_callbacks: usize,
    pub deferred_queued: usize,
    pub deferred_dropped: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactPassError {
    MissingContact(ContactIndex),
    Dispatch {
        contact: ContactIndex,
        source: ContactDispatchError,
    },
}

/// Opt-in owner for the typed contact-delivery seam. Construction is disabled
/// by default. The hybrid guest-memory path remains the caller's default until
/// an integration explicitly enables this service.
#[derive(Debug)]
pub struct ContactDeliverySystem {
    enabled: bool,
    deferred: DeferredEvents,
}

impl ContactDeliverySystem {
    /// Create an inactive system. This keeps existing lifted callers unchanged.
    /// Verification tier: Tier 0, default-off switch contract.
    pub fn new(capacity: usize) -> Self {
        Self {
            enabled: false,
            deferred: DeferredEvents::new(capacity),
        }
    }

    /// Explicitly opt into the typed delivery path.
    /// Verification tier: Tier 0, opt-in switch contract.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Verification tier: Tier 0, switch state query.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Number of rows currently owned by the native queue.
    /// Verification tier: Tier 0, queue-state query.
    pub fn deferred_count(&self) -> usize {
        self.deferred.count()
    }

    /// Append an already typed deferred event through the host adapter seam.
    /// The operation is inactive while this system's opt-in switch is off.
    /// Verification tier: Tier 0 adapter boundary; payloads are fabricated until
    /// the lifted contact pass is connected.
    pub fn enqueue_deferred(&mut self, event: PhysicsEvent) -> bool {
        self.enabled && self.deferred.enqueue(event)
    }

    /// Pop the current LIFO row and publish the lower count before the caller
    /// invokes a foreign owner callback. No borrow is held across that callback.
    /// Verification tier: Tier 0 foreign-callback boundary.
    pub fn pop_deferred_lifo(&mut self) -> Option<(usize, Option<PhysicsEvent>)> {
        self.deferred.pop_lifo()
    }

    /// Return a typed view of an allocated queue slot, including the popped
    /// slot while its callback is running. The host adapter uses this to mirror
    /// Rust callback mutations back to the temporary guest queue arrays.
    /// Verification tier: Tier 0 host callback attachment.
    pub fn deferred_event(&self, slot: usize) -> Option<PhysicsEvent> {
        self.deferred.row(slot)
    }

    /// Deliver one already-popped row through a registered native owner
    /// callback. `false` leaves the caller free to invoke the lifted callback.
    /// Verification tier: Tier 0 typed callback attachment; no live owner is
    /// registered by default.
    pub fn deliver_deferred_callback<H: OwnerCallbacks>(
        &mut self,
        owner: OwnerHandle,
        slot: usize,
        handlers: &mut H,
    ) -> bool {
        if !self.enabled || !handlers.has_callback(owner) {
            return false;
        }
        handlers.deliver(owner, slot, &mut self.deferred);
        true
    }

    /// Import mutation performed by a foreign callback into the typed row that
    /// remains in the popped slot. The recipient owner stays in its owner slot.
    /// Verification tier: Tier 0 foreign-callback boundary.
    pub fn replace_deferred_row(&mut self, slot: usize, event: PhysicsEvent) -> bool {
        self.deferred.replace_row(slot, event)
    }

    /// Release queue storage after the adapter observes count zero. A callback
    /// that appended another row must be drained before this succeeds.
    /// Verification tier: Tier 0 frame teardown boundary.
    pub fn finish_deferred_drain(&mut self) -> bool {
        self.deferred.finish_drain()
    }

    /// Prepare the queue storage for the next physics frame. The original
    /// caller allocates its deferred arrays before contact substeps and releases
    /// them after draining; a native owner must therefore explicitly begin each
    /// frame. Refuse to replace storage while any deferred row remains pending.
    /// Verification tier: Tier 0 frame lifecycle contract.
    pub fn begin_frame(&mut self, capacity: usize) -> Result<(), ContactFrameStartError> {
        if self.deferred.count() != 0 {
            return Err(ContactFrameStartError::PendingDeferredEvents);
        }
        self.deferred = DeferredEvents::new(capacity);
        Ok(())
    }

    /// Deliver a single already-resolved contact. Pruning, material selection,
    /// callback predicates and effects stay with their existing services. The
    /// caller must provide state matching the typed contact record.
    /// Verification tier: Tier 0, typed dispatch contract; no live route yet.
    pub fn dispatch<H: ContactDeliveryHandlers>(
        &mut self,
        state: ContactDeliveryState,
        contact: ContactLease,
        handlers: &mut H,
    ) -> Result<ContactDispatchReport, ContactDispatchError> {
        let mut report = ContactDispatchReport {
            enabled: self.enabled,
            ..ContactDispatchReport::default()
        };
        if !self.enabled {
            return Ok(report);
        }

        let Some((kind, owner_a, owner_b, normal, speed_bits)) = contact.with(|record| {
            (
                record.kind,
                record.owner_a,
                record.owner_b,
                record.horizontal_normal,
                record.relative_speed_bits,
            )
        }) else {
            return Err(ContactDispatchError::MissingContact);
        };
        if state.kind != kind || owner_a != Some(state.owner_a) || owner_b != Some(state.owner_b) {
            return Err(ContactDispatchError::StateDoesNotMatchContact);
        }

        let actions = contact_delivery_order(state);
        if actions
            .iter()
            .flatten()
            .any(|action| matches!(action, ContactDelivery::Enqueue { .. }))
            && normal.is_none()
        {
            return Err(ContactDispatchError::MissingContactNormal);
        }
        // The deferred 0x16C070 payload does not contain 0x1667C0's relative
        // speed. Only an immediate 0x1EC callback consumes that value. Check
        // before mutating the queue so an incomplete immediate event cannot
        // leave a partial deferred append behind.
        if actions
            .iter()
            .flatten()
            .any(|action| matches!(action, ContactDelivery::Immediate { .. }))
            && speed_bits.is_none()
        {
            return Err(ContactDispatchError::MissingContactSpeed);
        }

        for action in actions.into_iter().flatten() {
            match action {
                ContactDelivery::Enqueue { recipient, other } => {
                    let event = PhysicsEvent::from_contact(
                        recipient,
                        Some(other),
                        normal.expect("deferred actions were checked for a contact normal"),
                        contact.clone(),
                        true,
                    );
                    if self.deferred.enqueue(event) {
                        report.deferred_queued += 1;
                    } else {
                        report.deferred_dropped += 1;
                    }
                }
                ContactDelivery::Immediate { recipient } => {
                    handlers.deliver_immediate(
                        ImmediateContactEvent {
                            recipient,
                            contact: contact.clone(),
                            relative_speed_bits: speed_bits
                                .expect("immediate actions were checked for speed above"),
                        },
                        &mut self.deferred,
                    );
                    report.immediate_callbacks += 1;
                }
            }
        }
        Ok(report)
    }

    /// Build the callback state from the leased contact and already typed body
    /// metadata, then dispatch without asking the caller to repeat contact kind
    /// or owner identity. The host adapter still owns resolving bodies and their
    /// callback predicates.
    /// Verification tier: Tier 0 typed attachment; inputs are fabricated until
    /// the lifted contact pass is connected.
    pub fn dispatch_contact<H: ContactDeliveryHandlers>(
        &mut self,
        contact: ContactLease,
        body_a: ContactDeliveryBody,
        body_b: ContactDeliveryBody,
        handlers: &mut H,
    ) -> Result<ContactDispatchReport, ContactDispatchError> {
        if !self.enabled {
            return Ok(ContactDispatchReport::default());
        }
        let state = match contact
            .with(|record| ContactDeliveryState::from_contact(record, body_a, body_b))
        {
            None => return Err(ContactDispatchError::MissingContact),
            Some(Err(error)) => return Err(error),
            Some(Ok(state)) => state,
        };
        self.dispatch(state, contact, handlers)
    }

    /// Walk a Rust-owned linked contact list, reading each node's successor
    /// after endpoint delivery. The retained T1675 pass dossier and detailed
    /// physics-world account place the `+0x34` read after callbacks; a summary
    /// paragraph in the latter document conflicts and is treated as stale.
    /// Callback mutation of this node can therefore redirect traversal, but
    /// this does not establish safe mutation or retirement of later nodes. The
    /// caller supplies already-typed endpoint metadata; geometry, pruning,
    /// materials, and body ownership remain neighboring services.
    ///
    /// A nonpositive signed capacity processes no contacts. Cycles or a longer
    /// chain stop at the signed capacity, as the source pass does. This is an
    /// opt-in Rust attachment; it is not connected to lifted `0x170690` yet.
    /// Verification tier: Tier 0, FABRICATED contact lists and handlers.
    pub fn dispatch_chain<H, F>(
        &mut self,
        arena: &ContactArena,
        head: Option<ContactIndex>,
        signed_capacity: i32,
        mut endpoint_metadata: F,
        handlers: &mut H,
    ) -> Result<ContactPassReport, ContactPassError>
    where
        H: ContactDeliveryHandlers,
        F: FnMut(&PhysicsContact) -> (ContactDeliveryBody, ContactDeliveryBody),
    {
        let mut report = ContactPassReport::default();
        if !self.enabled || signed_capacity <= 0 {
            return Ok(report);
        }

        let mut current = head;
        while let Some(index) = current {
            if report.processed >= signed_capacity as usize {
                break;
            }
            let contact = arena
                .lease(index)
                .ok_or(ContactPassError::MissingContact(index))?;
            let (body_a, body_b) = contact
                .with(|record| endpoint_metadata(record))
                .ok_or(ContactPassError::MissingContact(index))?;
            let delivery = self
                .dispatch_contact(contact.clone(), body_a, body_b, handlers)
                .map_err(|source| ContactPassError::Dispatch {
                    contact: index,
                    source,
                })?;
            // T1675's measured-static instruction order reads +0x34 after the
            // endpoint callbacks. Preserve callback-time edits to this typed
            // link when choosing the next node.
            let next = contact
                .with(|record| record.next)
                .ok_or(ContactPassError::MissingContact(index))?;

            report.output.push(index);
            report.processed += 1;
            report.immediate_callbacks += delivery.immediate_callbacks;
            report.deferred_queued += delivery.deferred_queued;
            report.deferred_dropped += delivery.deferred_dropped;

            current = next;
        }
        Ok(report)
    }

    /// Deliver one contact callback through the typed registry while retaining
    /// this system's deferred queue for callback-time appends. Used by the
    /// default-off host ABI bridge when the lifted contact pass reaches a
    /// registered owner callback.
    /// Verification tier: Tier 0 attachment; guest-derived event parsing is
    /// covered by the host adapter tests.
    pub fn dispatch_immediate<H: ContactDeliveryHandlers>(
        &mut self,
        event: ImmediateContactEvent,
        handlers: &mut H,
    ) -> bool {
        if !self.enabled || !event.recipient_is_contact_endpoint() {
            return false;
        }
        handlers.deliver_immediate(event, &mut self.deferred);
        true
    }

    /// Drain after solver work, preserving the observed LIFO and callback-
    /// enqueue behavior. Draining is allowed after disabling so queued state is
    /// never abandoned by an A/B switch change.
    /// Verification tier: Tier 1 for the queue core; callback mutation remains
    /// fabricated until a natural owner recording exercises the seam.
    pub fn drain<H: ContactDeliveryHandlers>(&mut self, handlers: &mut H) -> DrainReport {
        self.deferred.drain(handlers)
    }
}

/// Lifecycle errors for an explicitly owned Rust contact frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactFrameError {
    AlreadyActive,
    NotActive,
    PendingDeferredEvents,
    Dispatch(ContactPassError),
}

/// Rust-owned contacts and their deferred delivery queue for one physics frame.
///
/// The owner keeps the source contact arena alive until the deferred queue is
/// drained. A finished frame may start another arena; outstanding `ContactLease`
/// values still refer to the old arena, so recycling a slot cannot silently
/// change a retained Rust event. This is the native lifetime model. It does not
/// assert that the original guest contact pool has generation checks.
#[derive(Debug)]
pub struct ContactDeliveryFrame {
    arena: ContactArena,
    delivery: ContactDeliverySystem,
    active: bool,
}

impl ContactDeliveryFrame {
    /// Start an explicitly native frame with typed contacts and queue capacity.
    /// The host switch remains default-off; constructing this owner is opt-in.
    /// Verification tier: Tier 0, FABRICATED lifecycle inputs.
    pub fn new(contacts: Vec<PhysicsContact>, queue_capacity: usize) -> Self {
        let mut delivery = ContactDeliverySystem::new(queue_capacity);
        delivery.set_enabled(true);
        Self {
            arena: ContactArena::new(contacts),
            delivery,
            active: true,
        }
    }

    /// Borrow the current frame's typed contact arena.
    pub fn arena(&self) -> &ContactArena {
        &self.arena
    }

    /// Retain one contact snapshot in this frame's append-only arena.
    /// Repeated imports intentionally get separate slots: the guest callback
    /// bridge may observe mutation or address reuse between callback edges, and
    /// no generation contract is established for those guest addresses.
    /// Verification tier: Tier 0, FABRICATED host-attachment inputs.
    pub fn retain_contact(
        &mut self,
        contact: PhysicsContact,
    ) -> Result<ContactLease, ContactFrameError> {
        if !self.active {
            return Err(ContactFrameError::NotActive);
        }
        Ok(self.arena.push(contact))
    }

    /// Access the typed callback queue while the host adapts the lifted ABI.
    /// Contact lifetime remains owned by this frame's arena.
    pub fn delivery(&self) -> &ContactDeliverySystem {
        &self.delivery
    }

    /// Mutably access the typed callback queue while the host adapts the lifted ABI.
    pub fn delivery_mut(&mut self) -> &mut ContactDeliverySystem {
        &mut self.delivery
    }

    /// Dispatch the typed list while the frame owns its contact arena.
    /// Verification tier: Tier 0, FABRICATED inputs; no host route is connected.
    pub fn dispatch_chain<H, F>(
        &mut self,
        head: Option<ContactIndex>,
        signed_capacity: i32,
        endpoint_metadata: F,
        handlers: &mut H,
    ) -> Result<ContactPassReport, ContactFrameError>
    where
        H: ContactDeliveryHandlers,
        F: FnMut(&PhysicsContact) -> (ContactDeliveryBody, ContactDeliveryBody),
    {
        if !self.active {
            return Err(ContactFrameError::NotActive);
        }
        self.delivery
            .dispatch_chain(
                &self.arena,
                head,
                signed_capacity,
                endpoint_metadata,
                handlers,
            )
            .map_err(ContactFrameError::Dispatch)
    }

    /// Drain deferred owners before closing this frame.
    /// Verification tier: Tier 0 attachment; native callbacks remain unconnected.
    pub fn drain<H: ContactDeliveryHandlers>(
        &mut self,
        handlers: &mut H,
    ) -> Result<DrainReport, ContactFrameError> {
        if !self.active {
            return Err(ContactFrameError::NotActive);
        }
        Ok(self.delivery.drain(handlers))
    }

    /// Close this frame only after every deferred callback row has been drained.
    pub fn finish(&mut self) -> Result<(), ContactFrameError> {
        if !self.active {
            return Err(ContactFrameError::NotActive);
        }
        if self.delivery.deferred_count() != 0 {
            return Err(ContactFrameError::PendingDeferredEvents);
        }
        self.active = false;
        Ok(())
    }

    /// Begin the next typed frame after the prior queue has drained and closed.
    pub fn start_next_frame(
        &mut self,
        contacts: Vec<PhysicsContact>,
        queue_capacity: usize,
    ) -> Result<(), ContactFrameError> {
        if self.active {
            return Err(ContactFrameError::AlreadyActive);
        }
        self.delivery
            .begin_frame(queue_capacity)
            .map_err(|_| ContactFrameError::PendingDeferredEvents)?;
        self.arena = ContactArena::new(contacts);
        self.active = true;
        Ok(())
    }
}

/// Inputs here are already-resolved contracts, not raw guest handles or an
/// assertion that the source's predicates have been observed in a runtime trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactDeliveryState {
    pub kind: u16,
    pub owner_a: OwnerHandle,
    pub owner_b: OwnerHandle,
    pub body_a_callback_owner: Option<OwnerHandle>,
    pub body_b_callback_owner: Option<OwnerHandle>,
    pub body_a_can_defer: bool,
    pub body_b_can_defer: bool,
    pub body_a_has_physics_handle: bool,
    pub body_b_has_physics_handle: bool,
}

/// Already typed callback/defer metadata for one contact endpoint. This does
/// not expose guest object offsets or claim to own the neighbouring body model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContactDeliveryBody {
    pub callback_owner: Option<OwnerHandle>,
    pub can_defer: bool,
    pub has_physics_handle: bool,
}

impl ContactDeliveryState {
    /// Derive contact identity from the typed record while accepting only the
    /// endpoint predicates supplied by the typed physics body owner.
    /// Verification tier: Tier 0 contract helper.
    pub fn from_contact(
        contact: &PhysicsContact,
        body_a: ContactDeliveryBody,
        body_b: ContactDeliveryBody,
    ) -> Result<Self, ContactDispatchError> {
        let (Some(owner_a), Some(owner_b)) = (contact.owner_a, contact.owner_b) else {
            return Err(ContactDispatchError::MissingContactOwner);
        };
        Ok(Self {
            kind: contact.kind,
            owner_a,
            owner_b,
            body_a_callback_owner: body_a.callback_owner,
            body_b_callback_owner: body_b.callback_owner,
            body_a_can_defer: body_a.can_defer,
            body_b_can_defer: body_b.can_defer,
            body_a_has_physics_handle: body_a.has_physics_handle,
            body_b_has_physics_handle: body_b.has_physics_handle,
        })
    }
}

/// Return only the established callback/enqueue order for one contact. Material,
/// activation, pruning, effects and the output-list tail stay with their services.
/// Verification tier: Tier 0 contract tests; the full 0x170690 controller is not ported.
pub fn contact_delivery_order(state: ContactDeliveryState) -> [Option<ContactDelivery>; 2] {
    let mut actions = [None, None];
    let mut len = 0;
    match state.kind {
        0 if !state.body_b_has_physics_handle && state.body_b_can_defer => {
            actions[len] = Some(ContactDelivery::Enqueue {
                recipient: state.owner_b,
                other: state.owner_a,
            });
            len += 1;
        }
        1 => {
            if let Some(recipient) = state.body_b_callback_owner {
                actions[len] = Some(ContactDelivery::Immediate { recipient });
                len += 1;
            }
        }
        3 if !state.body_a_has_physics_handle && state.body_a_can_defer => {
            actions[len] = Some(ContactDelivery::Enqueue {
                recipient: state.owner_a,
                other: state.owner_b,
            });
            len += 1;
        }
        _ => {}
    }
    if let Some(recipient) = state.body_a_callback_owner {
        if len < actions.len() {
            actions[len] = Some(ContactDelivery::Immediate { recipient });
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena() -> ContactArena {
        ContactArena::new(vec![PhysicsContact {
            kind: 0,
            owner_a: Some(OwnerHandle(10)),
            owner_b: Some(OwnerHandle(20)),
            horizontal_normal: Some([-0.0, f32::from_bits(0x7fc0_1234)]),
            relative_speed_bits: Some(0x3f80_0000),
            next: None,
            guest_next: None,
            guest_identity: None,
        }])
    }

    #[derive(Default)]
    struct Recorder {
        order: Vec<(OwnerHandle, usize)>,
        append_once: bool,
    }

    #[derive(Default)]
    struct DeliveryRecorder {
        order: Vec<(u8, OwnerHandle, u32)>,
        append_from_immediate: bool,
        mutate_contact_on_immediate: bool,
        observed_contact_kind: Option<u16>,
    }

    struct NativeRecorder {
        order: Vec<(u8, OwnerHandle)>,
        append_from_immediate: Option<PhysicsEvent>,
    }

    struct ContactLinkMutator {
        visited: Vec<ContactIndex>,
        redirect_from: ContactIndex,
        redirect_to: ContactIndex,
    }

    fn native_immediate(
        context: &mut NativeRecorder,
        event: ImmediateContactEvent,
        queue: &mut DeferredEvents,
    ) {
        context.order.push((0, event.recipient));
        if let Some(event) = context.append_from_immediate.take() {
            assert!(queue.enqueue(event));
        }
    }

    fn native_deferred(
        context: &mut NativeRecorder,
        owner: OwnerHandle,
        slot: usize,
        queue: &mut DeferredEvents,
    ) {
        assert_eq!(queue.row(slot).map(|event| event.recipient), Some(owner));
        context.order.push((1, owner));
    }

    fn redirect_contact_link(
        context: &mut ContactLinkMutator,
        event: ImmediateContactEvent,
        _queue: &mut DeferredEvents,
    ) {
        let index = event.contact.index();
        context.visited.push(index);
        if index == context.redirect_from {
            assert_eq!(
                event
                    .contact
                    .with_mut(|contact| contact.next = Some(context.redirect_to)),
                Some(())
            );
        }
    }

    impl OwnerCallbacks for DeliveryRecorder {
        fn has_callback(&self, owner: OwnerHandle) -> bool {
            owner == OwnerHandle(20)
        }

        fn deliver(&mut self, owner: OwnerHandle, slot: usize, queue: &mut DeferredEvents) {
            let event = queue.row(slot).unwrap();
            self.order.push((1, owner, event.other.unwrap().0));
            self.observed_contact_kind = event.contact.with(|contact| contact.kind);
        }
    }

    impl ContactDeliveryHandlers for DeliveryRecorder {
        fn deliver_immediate(&mut self, event: ImmediateContactEvent, queue: &mut DeferredEvents) {
            self.order
                .push((0, event.recipient, event.relative_speed_bits));
            if self.mutate_contact_on_immediate {
                self.mutate_contact_on_immediate = false;
                assert_eq!(event.contact.with_mut(|contact| contact.kind = 3), Some(()));
            }
            if self.append_from_immediate {
                self.append_from_immediate = false;
                let row = PhysicsEvent::from_contact(
                    OwnerHandle(20),
                    Some(OwnerHandle(10)),
                    [1.0, -2.0],
                    event.contact,
                    true,
                );
                assert!(queue.enqueue(row));
            }
        }
    }

    impl OwnerCallbacks for Recorder {
        fn has_callback(&self, _: OwnerHandle) -> bool {
            true
        }

        fn deliver(&mut self, owner: OwnerHandle, slot: usize, queue: &mut DeferredEvents) {
            self.order.push((owner, slot));
            let event = queue.row(slot).unwrap();
            if self.append_once {
                self.append_once = false;
                assert!(queue.enqueue(event));
            }
        }
    }

    #[test]
    fn queue_is_lifo_and_callback_append_reuses_the_popped_slot() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let first = PhysicsEvent::from_contact(
            OwnerHandle(20),
            Some(OwnerHandle(10)),
            [-0.0, f32::from_bits(0x7fc0_1234)],
            contact,
            true,
        );
        let second = PhysicsEvent {
            recipient: OwnerHandle(30),
            other: first.other,
            horizontal_normal_bits: first.horizontal_normal_bits,
            contact: first.contact.clone(),
        };
        assert_eq!(first.horizontal_normal_bits, [0, 0xffc0_1234]);
        let mut queue = DeferredEvents::new(3);
        assert!(queue.enqueue(first));
        assert!(queue.enqueue(second));
        let mut handlers = Recorder {
            append_once: true,
            ..Recorder::default()
        };
        let report = queue.drain(&mut handlers);
        assert_eq!(
            handlers.order,
            [
                (OwnerHandle(30), 1),
                (OwnerHandle(30), 1),
                (OwnerHandle(20), 0)
            ]
        );
        assert_eq!(
            report,
            DrainReport {
                delivered: 3,
                skipped: 0,
                released: true
            }
        );
        assert!(!queue.is_allocated());
    }

    #[test]
    fn explicit_pop_and_finish_allow_an_external_callback_to_reenter() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let mut queue = DeferredEvents::new(3);
        let event = |recipient| PhysicsEvent {
            recipient: OwnerHandle(recipient),
            other: Some(OwnerHandle(10)),
            horizontal_normal_bits: [0x3f80_0000, 0xbf80_0000],
            contact: contact.clone(),
        };
        assert!(queue.enqueue(event(20)));
        assert!(queue.enqueue(event(30)));

        let (popped_slot, popped) = queue.pop_lifo().unwrap();
        assert_eq!(popped_slot, 1);
        assert_eq!(popped.unwrap().recipient, OwnerHandle(30));
        assert_eq!(queue.count(), 1);

        // Model the host crossing into a lifted callback after count publication.
        // A reentrant append replaces the slot whose callback is currently running.
        assert!(queue.enqueue(event(40)));
        let (reentrant_slot, reentrant) = queue.pop_lifo().unwrap();
        assert_eq!(reentrant_slot, 1);
        assert_eq!(reentrant.unwrap().recipient, OwnerHandle(40));
        let (older_slot, older) = queue.pop_lifo().unwrap();
        assert_eq!(older_slot, 0);
        assert_eq!(older.unwrap().recipient, OwnerHandle(20));
        assert!(queue.pop_lifo().is_none());
        assert!(queue.finish_drain());
        assert!(!queue.is_allocated());
    }

    #[test]
    fn deferred_contact_lease_preserves_guest_identity_without_fabricated_speed() {
        let contact = |address: u32, normal: [u32; 2]| {
            ContactArena::new(vec![PhysicsContact {
                kind: 3,
                owner_a: Some(OwnerHandle(10)),
                owner_b: Some(OwnerHandle(20)),
                horizontal_normal: Some(normal.map(f32::from_bits)),
                relative_speed_bits: None,
                next: None,
                guest_next: None,
                guest_identity: Some(ContactGuestId(address)),
            }])
            .lease(ContactIndex(0))
            .unwrap()
        };
        let original = contact(0x1234, [0xbf80_0000, 0x8000_0000]);
        assert_eq!(original.guest_identity(), Some(ContactGuestId(0x1234)));
        assert!(
            original
                .with(|record| record.relative_speed_bits)
                .unwrap()
                .is_none()
        );

        let mut queue = DeferredEvents::new(1);
        assert!(queue.enqueue(PhysicsEvent {
            recipient: OwnerHandle(20),
            other: Some(OwnerHandle(10)),
            horizontal_normal_bits: [0xbf80_0000, 0x8000_0000],
            contact: original,
        }));
        let (slot, popped) = queue.pop_lifo().unwrap();
        assert_eq!(
            popped.unwrap().contact.guest_identity(),
            Some(ContactGuestId(0x1234))
        );

        // Callback edits replace the typed lease together with the copied row
        // words while the recipient owner table remains stable.
        assert!(queue.replace_row(
            slot,
            PhysicsEvent {
                recipient: OwnerHandle(20),
                other: Some(OwnerHandle(11)),
                horizontal_normal_bits: [0x3f00_0000, 0xbf00_0000],
                contact: contact(0x5678, [0xbf00_0000, 0x3f00_0000]),
            }
        ));
        let replaced = queue.row(slot).unwrap();
        assert_eq!(replaced.other, Some(OwnerHandle(11)));
        assert_eq!(replaced.horizontal_normal_bits, [0x3f00_0000, 0xbf00_0000]);
        assert_eq!(
            replaced.contact.guest_identity(),
            Some(ContactGuestId(0x5678))
        );
        assert!(
            replaced
                .contact
                .with(|record| record.relative_speed_bits)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn invalid_owner_or_other_skips_and_empty_allocated_queue_releases() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let mut queue = DeferredEvents::new(2);
        assert!(queue.enqueue(PhysicsEvent {
            recipient: OwnerHandle(4),
            other: None,
            horizontal_normal_bits: [0; 2],
            contact,
        }));
        let mut handlers = Recorder::default();
        let report = queue.drain(&mut handlers);
        assert_eq!(
            report,
            DrainReport {
                delivered: 0,
                skipped: 1,
                released: true
            }
        );
        let empty = DeferredEvents::new(0);
        let mut empty = empty;
        assert!(empty.drain(&mut handlers).released);

        let mut partial = DeferredEvents::with_storage(2, true, false);
        let partial_report = partial.drain(&mut handlers);
        assert!(!partial_report.released);
        assert_eq!(partial.capacity(), 2);
    }

    #[test]
    fn contact_branch_actions_preserve_b_then_a_and_enqueue_before_a() {
        let kind_one = contact_delivery_order(ContactDeliveryState {
            kind: 1,
            owner_a: OwnerHandle(1),
            owner_b: OwnerHandle(2),
            body_a_callback_owner: Some(OwnerHandle(11)),
            body_b_callback_owner: Some(OwnerHandle(22)),
            body_a_can_defer: false,
            body_b_can_defer: false,
            body_a_has_physics_handle: true,
            body_b_has_physics_handle: true,
        });
        assert_eq!(
            kind_one,
            [
                Some(ContactDelivery::Immediate {
                    recipient: OwnerHandle(22)
                }),
                Some(ContactDelivery::Immediate {
                    recipient: OwnerHandle(11)
                }),
            ]
        );
        let kind_zero = contact_delivery_order(ContactDeliveryState {
            kind: 0,
            owner_a: OwnerHandle(1),
            owner_b: OwnerHandle(2),
            body_a_callback_owner: Some(OwnerHandle(11)),
            body_b_callback_owner: None,
            body_a_can_defer: false,
            body_b_can_defer: true,
            body_a_has_physics_handle: true,
            body_b_has_physics_handle: false,
        });
        assert_eq!(
            kind_zero,
            [
                Some(ContactDelivery::Enqueue {
                    recipient: OwnerHandle(2),
                    other: OwnerHandle(1)
                }),
                Some(ContactDelivery::Immediate {
                    recipient: OwnerHandle(11)
                }),
            ]
        );
        let kind_three = contact_delivery_order(ContactDeliveryState {
            kind: 3,
            owner_a: OwnerHandle(1),
            owner_b: OwnerHandle(2),
            body_a_callback_owner: Some(OwnerHandle(11)),
            body_b_callback_owner: None,
            body_a_can_defer: true,
            body_b_can_defer: false,
            body_a_has_physics_handle: false,
            body_b_has_physics_handle: true,
        });
        assert_eq!(
            kind_three,
            [
                Some(ContactDelivery::Enqueue {
                    recipient: OwnerHandle(1),
                    other: OwnerHandle(2)
                }),
                Some(ContactDelivery::Immediate {
                    recipient: OwnerHandle(11)
                })
            ]
        );
    }

    #[test]
    fn delivery_system_defaults_off_then_runs_direct_before_deferred_callback() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let state = ContactDeliveryState {
            kind: 0,
            owner_a: OwnerHandle(10),
            owner_b: OwnerHandle(20),
            body_a_callback_owner: Some(OwnerHandle(11)),
            body_b_callback_owner: None,
            body_a_can_defer: false,
            body_b_can_defer: true,
            body_a_has_physics_handle: true,
            body_b_has_physics_handle: false,
        };
        let mut system = ContactDeliverySystem::new(2);
        let mut handlers = DeliveryRecorder::default();

        let off = system
            .dispatch(state, contact.clone(), &mut handlers)
            .unwrap();
        assert!(!system.is_enabled());
        assert!(!off.enabled);
        assert_eq!(system.deferred_count(), 0);
        assert!(handlers.order.is_empty());

        system.set_enabled(true);
        let active = system.dispatch(state, contact, &mut handlers).unwrap();
        assert!(active.enabled);
        assert_eq!(active.immediate_callbacks, 1);
        assert_eq!(active.deferred_queued, 1);
        assert_eq!(active.deferred_dropped, 0);
        assert_eq!(system.deferred_count(), 1);
        assert_eq!(handlers.order, [(0, OwnerHandle(11), 0x3f80_0000)]);

        let drained = system.drain(&mut handlers);
        assert_eq!(drained.delivered, 1);
        assert_eq!(
            handlers.order,
            [(0, OwnerHandle(11), 0x3f80_0000), (1, OwnerHandle(20), 10),]
        );
        assert!(drained.released);
        assert_eq!(system.deferred_count(), 0);
    }

    #[test]
    fn native_immediate_delivery_rejects_a_non_endpoint_recipient() {
        let arena = arena();
        let event = ImmediateContactEvent {
            recipient: OwnerHandle(99),
            contact: arena.lease(ContactIndex(0)).unwrap(),
            relative_speed_bits: 0x3f80_0000,
        };
        let mut system = ContactDeliverySystem::new(1);
        system.set_enabled(true);
        let mut handlers = DeliveryRecorder::default();

        assert!(!system.dispatch_immediate(event, &mut handlers));
        assert!(handlers.order.is_empty());
        assert_eq!(system.deferred_count(), 0);
    }

    #[test]
    fn native_callback_registry_delivers_typed_immediate_and_lifo_deferred_events() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let state = ContactDeliveryState {
            kind: 0,
            owner_a: OwnerHandle(10),
            owner_b: OwnerHandle(20),
            body_a_callback_owner: Some(OwnerHandle(11)),
            body_b_callback_owner: None,
            body_a_can_defer: false,
            body_b_can_defer: true,
            body_a_has_physics_handle: true,
            body_b_has_physics_handle: false,
        };
        let appended = PhysicsEvent::from_contact(
            OwnerHandle(30),
            Some(OwnerHandle(10)),
            [1.0, 2.0],
            contact.clone(),
            false,
        );
        let mut handlers = NativeContactCallbacks::new(NativeRecorder {
            order: Vec::new(),
            append_from_immediate: Some(appended),
        });
        handlers.register_immediate(OwnerHandle(11), native_immediate);
        handlers.register_deferred(OwnerHandle(20), native_deferred);
        handlers.register_deferred(OwnerHandle(30), native_deferred);

        let mut system = ContactDeliverySystem::new(2);
        system.set_enabled(true);
        let report = system.dispatch(state, contact, &mut handlers).unwrap();
        assert_eq!(report.immediate_callbacks, 1);
        assert_eq!(report.deferred_queued, 1);
        assert_eq!(system.deferred_count(), 2);

        let drained = system.drain(&mut handlers);
        assert_eq!(drained.delivered, 2);
        assert!(drained.released);
        assert_eq!(
            handlers.context().order,
            [
                (0, OwnerHandle(11)),
                (1, OwnerHandle(30)),
                (1, OwnerHandle(20)),
            ]
        );
        assert!(handlers.unregister(OwnerHandle(20)));
        assert!(!handlers.has_callback(OwnerHandle(20)));
        assert!(!handlers.unregister(OwnerHandle(20)));
    }

    #[test]
    fn dispatch_contact_derives_kind_and_owners_from_the_lease() {
        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 1,
            owner_a: Some(OwnerHandle(10)),
            owner_b: Some(OwnerHandle(20)),
            horizontal_normal: Some([1.0, 0.0]),
            relative_speed_bits: Some(0x4120_0000),
            next: None,
            guest_next: None,
            guest_identity: None,
        }]);
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let mut system = ContactDeliverySystem::new(1);
        system.set_enabled(true);
        let mut handlers = DeliveryRecorder::default();

        let report = system
            .dispatch_contact(
                contact,
                ContactDeliveryBody {
                    callback_owner: Some(OwnerHandle(11)),
                    has_physics_handle: true,
                    ..ContactDeliveryBody::default()
                },
                ContactDeliveryBody {
                    callback_owner: Some(OwnerHandle(22)),
                    has_physics_handle: true,
                    ..ContactDeliveryBody::default()
                },
                &mut handlers,
            )
            .unwrap();

        assert_eq!(report.immediate_callbacks, 2);
        assert_eq!(
            handlers.order,
            [
                (0, OwnerHandle(22), 0x4120_0000),
                (0, OwnerHandle(11), 0x4120_0000)
            ]
        );
    }

    #[test]
    fn dispatch_contact_rejects_unresolved_contact_owners() {
        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 1,
            owner_a: Some(OwnerHandle(10)),
            owner_b: None,
            horizontal_normal: Some([0.0; 2]),
            relative_speed_bits: Some(0),
            next: None,
            guest_next: None,
            guest_identity: None,
        }]);
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let mut system = ContactDeliverySystem::new(0);
        system.set_enabled(true);
        let mut handlers = DeliveryRecorder::default();

        assert_eq!(
            system.dispatch_contact(
                contact,
                ContactDeliveryBody::default(),
                ContactDeliveryBody::default(),
                &mut handlers,
            ),
            Err(ContactDispatchError::MissingContactOwner)
        );
        assert!(handlers.order.is_empty());
    }

    #[test]
    fn contact_pass_reads_next_link_after_callback_mutation() {
        let arena = ContactArena::new(
            [0, 1, 2]
                .into_iter()
                .map(|index| PhysicsContact {
                    kind: 1,
                    owner_a: Some(OwnerHandle(10 + index)),
                    owner_b: Some(OwnerHandle(20 + index)),
                    horizontal_normal: Some([1.0, 0.0]),
                    relative_speed_bits: Some(0x3f80_0000),
                    next: (index < 2).then_some(ContactIndex(index as usize + 1)),
                    guest_next: None,
                    guest_identity: None,
                })
                .collect(),
        );
        let mut handlers = NativeContactCallbacks::new(ContactLinkMutator {
            visited: Vec::new(),
            redirect_from: ContactIndex(0),
            redirect_to: ContactIndex(2),
        });
        handlers.register_immediate(OwnerHandle(99), redirect_contact_link);
        let mut system = ContactDeliverySystem::new(0);
        system.set_enabled(true);

        let report = system
            .dispatch_chain(
                &arena,
                Some(ContactIndex(0)),
                8,
                |_| {
                    (
                        ContactDeliveryBody {
                            callback_owner: Some(OwnerHandle(99)),
                            has_physics_handle: true,
                            ..ContactDeliveryBody::default()
                        },
                        ContactDeliveryBody::default(),
                    )
                },
                &mut handlers,
            )
            .unwrap();

        assert_eq!(report.output, [ContactIndex(0), ContactIndex(2)]);
        assert_eq!(report.processed, 2);
        assert_eq!(report.immediate_callbacks, 2);
        assert_eq!(
            handlers.context().visited,
            [ContactIndex(0), ContactIndex(2)]
        );
    }

    #[test]
    fn contact_pass_respects_signed_capacity_even_for_a_cycle() {
        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 2,
            owner_a: Some(OwnerHandle(10)),
            owner_b: Some(OwnerHandle(20)),
            horizontal_normal: Some([0.0; 2]),
            relative_speed_bits: None,
            next: Some(ContactIndex(0)),
            guest_next: None,
            guest_identity: None,
        }]);
        let mut system = ContactDeliverySystem::new(0);
        system.set_enabled(true);
        let mut handlers = DeliveryRecorder::default();
        let report = system
            .dispatch_chain(
                &arena,
                Some(ContactIndex(0)),
                2,
                |_record| {
                    (
                        ContactDeliveryBody {
                            has_physics_handle: true,
                            ..ContactDeliveryBody::default()
                        },
                        ContactDeliveryBody {
                            has_physics_handle: true,
                            ..ContactDeliveryBody::default()
                        },
                    )
                },
                &mut handlers,
            )
            .unwrap();

        assert_eq!(report.output, [ContactIndex(0), ContactIndex(0)]);
        assert_eq!(report.processed, 2);
        assert!(handlers.order.is_empty());
    }

    #[test]
    fn delivery_system_restarts_queue_each_frame_and_preserves_pending_rows() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let state = ContactDeliveryState {
            kind: 0,
            owner_a: OwnerHandle(10),
            owner_b: OwnerHandle(20),
            body_a_callback_owner: None,
            body_b_callback_owner: None,
            body_a_can_defer: false,
            body_b_can_defer: true,
            body_a_has_physics_handle: true,
            body_b_has_physics_handle: false,
        };
        let mut system = ContactDeliverySystem::new(1);
        system.set_enabled(true);
        let mut handlers = DeliveryRecorder::default();

        assert_eq!(
            system
                .dispatch(state, contact.clone(), &mut handlers)
                .unwrap()
                .deferred_queued,
            1
        );
        assert_eq!(
            system.begin_frame(1),
            Err(ContactFrameStartError::PendingDeferredEvents)
        );
        assert_eq!(system.deferred_count(), 1);
        assert_eq!(system.drain(&mut handlers).delivered, 1);

        assert_eq!(system.begin_frame(1), Ok(()));
        assert_eq!(
            system
                .dispatch(state, contact, &mut handlers)
                .unwrap()
                .deferred_queued,
            1
        );
        assert_eq!(system.drain(&mut handlers).delivered, 1);
    }

    #[test]
    fn owned_contact_frame_drains_before_replacing_its_arena() {
        let original_arena = arena();
        let mut frame = ContactDeliveryFrame::new(original_arena.contacts.borrow().clone(), 1);
        let old_contact = frame.arena().lease(ContactIndex(0)).unwrap();
        let mut handlers = DeliveryRecorder::default();

        let report = frame
            .dispatch_chain(
                Some(ContactIndex(0)),
                1,
                |_| {
                    (
                        ContactDeliveryBody {
                            has_physics_handle: true,
                            ..ContactDeliveryBody::default()
                        },
                        ContactDeliveryBody {
                            can_defer: true,
                            ..ContactDeliveryBody::default()
                        },
                    )
                },
                &mut handlers,
            )
            .unwrap();
        assert_eq!(report.deferred_queued, 1);
        assert_eq!(
            frame.finish(),
            Err(ContactFrameError::PendingDeferredEvents)
        );

        assert_eq!(frame.drain(&mut handlers).unwrap().delivered, 1);
        assert_eq!(frame.finish(), Ok(()));
        assert_eq!(
            frame.dispatch_chain(
                None,
                1,
                |_| (Default::default(), Default::default()),
                &mut handlers
            ),
            Err(ContactFrameError::NotActive)
        );

        frame
            .start_next_frame(
                vec![PhysicsContact {
                    kind: 3,
                    owner_a: Some(OwnerHandle(30)),
                    owner_b: Some(OwnerHandle(40)),
                    horizontal_normal: Some([0.0; 2]),
                    relative_speed_bits: None,
                    next: None,
                    guest_next: None,
                    guest_identity: None,
                }],
                2,
            )
            .unwrap();
        let new_contact = frame.arena().lease(ContactIndex(0)).unwrap();
        assert_eq!(old_contact.with(|contact| contact.kind), Some(0));
        assert_eq!(new_contact.with(|contact| contact.kind), Some(3));
    }

    #[test]
    fn owned_contact_frame_drains_rows_appended_by_an_immediate_callback() {
        let mut frame = ContactDeliveryFrame::new(
            vec![PhysicsContact {
                kind: 1,
                owner_a: Some(OwnerHandle(10)),
                owner_b: Some(OwnerHandle(20)),
                horizontal_normal: Some([1.0, 0.0]),
                relative_speed_bits: Some(0x3f80_0000),
                next: None,
                guest_next: None,
                guest_identity: None,
            }],
            1,
        );
        let mut handlers = DeliveryRecorder {
            append_from_immediate: true,
            ..DeliveryRecorder::default()
        };

        let report = frame
            .dispatch_chain(
                Some(ContactIndex(0)),
                1,
                |_| {
                    (
                        ContactDeliveryBody {
                            callback_owner: Some(OwnerHandle(11)),
                            has_physics_handle: true,
                            ..ContactDeliveryBody::default()
                        },
                        ContactDeliveryBody {
                            has_physics_handle: true,
                            ..ContactDeliveryBody::default()
                        },
                    )
                },
                &mut handlers,
            )
            .unwrap();

        assert_eq!(report.immediate_callbacks, 1);
        assert_eq!(handlers.order, [(0, OwnerHandle(11), 0x3f80_0000)]);
        assert_eq!(
            frame.finish(),
            Err(ContactFrameError::PendingDeferredEvents)
        );
        assert_eq!(frame.drain(&mut handlers).unwrap().delivered, 1);
        assert_eq!(frame.finish(), Ok(()));
        assert_eq!(
            handlers.order,
            [
                (0, OwnerHandle(11), 0x3f80_0000),
                (1, OwnerHandle(20), OwnerHandle(10).0)
            ]
        );
    }

    #[test]
    fn owned_contact_frame_bounds_a_cyclic_list_by_signed_capacity() {
        let mut contact = arena().contacts.borrow()[0].clone();
        contact.next = Some(ContactIndex(0));
        let mut frame = ContactDeliveryFrame::new(vec![contact], 0);
        let mut handlers = DeliveryRecorder::default();

        let empty = frame
            .dispatch_chain(
                Some(ContactIndex(0)),
                0,
                |_| (Default::default(), Default::default()),
                &mut handlers,
            )
            .unwrap();
        assert_eq!(empty.processed, 0);
        assert!(empty.output.is_empty());

        let bounded = frame
            .dispatch_chain(
                Some(ContactIndex(0)),
                3,
                |_| (Default::default(), Default::default()),
                &mut handlers,
            )
            .unwrap();
        assert_eq!(bounded.processed, 3);
        assert_eq!(bounded.output, [ContactIndex(0); 3]);
        assert_eq!(bounded.immediate_callbacks, 0);
        assert_eq!(bounded.deferred_queued, 0);
        assert_eq!(bounded.deferred_dropped, 0);
    }

    #[test]
    fn deferred_contact_lease_keeps_its_arena_alive() {
        let mut queue = DeferredEvents::new(1);
        {
            let arena = arena();
            let contact = arena.lease(ContactIndex(0)).unwrap();
            let event = PhysicsEvent {
                recipient: OwnerHandle(20),
                other: Some(OwnerHandle(10)),
                horizontal_normal_bits: [0x8000_0000, 0],
                contact,
            };
            assert!(queue.enqueue(event));
        }

        let event = queue.row(0).expect("deferred row keeps the contact record");
        assert_eq!(
            event.contact.with(|contact| contact.owner_a),
            Some(Some(OwnerHandle(10)))
        );
    }

    #[test]
    fn delivery_system_rejects_mismatched_typed_contact_state() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let mut system = ContactDeliverySystem::new(1);
        system.set_enabled(true);
        let state = ContactDeliveryState {
            kind: 3,
            owner_a: OwnerHandle(10),
            owner_b: OwnerHandle(20),
            body_a_callback_owner: None,
            body_b_callback_owner: None,
            body_a_can_defer: false,
            body_b_can_defer: false,
            body_a_has_physics_handle: false,
            body_b_has_physics_handle: false,
        };
        assert_eq!(
            system.dispatch(state, contact, &mut DeliveryRecorder::default()),
            Err(ContactDispatchError::StateDoesNotMatchContact)
        );
        assert_eq!(system.deferred_count(), 0);
    }

    #[test]
    fn direct_callback_can_append_a_typed_deferred_row() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let state = ContactDeliveryState {
            kind: 0,
            owner_a: OwnerHandle(10),
            owner_b: OwnerHandle(20),
            body_a_callback_owner: Some(OwnerHandle(11)),
            body_b_callback_owner: None,
            body_a_can_defer: false,
            body_b_can_defer: true,
            body_a_has_physics_handle: true,
            body_b_has_physics_handle: false,
        };
        let mut system = ContactDeliverySystem::new(3);
        system.set_enabled(true);
        let mut handlers = DeliveryRecorder {
            append_from_immediate: true,
            ..DeliveryRecorder::default()
        };
        let report = system.dispatch(state, contact, &mut handlers).unwrap();
        assert_eq!(report.deferred_queued, 1);
        assert_eq!(system.deferred_count(), 2);
        assert_eq!(handlers.order, [(0, OwnerHandle(11), 0x3f80_0000)]);

        let drained = system.drain(&mut handlers);
        assert_eq!(drained.delivered, 2);
        assert_eq!(handlers.order.len(), 3);
        assert_eq!(handlers.order[1].0, 1);
        assert_eq!(handlers.order[2].0, 1);
    }

    #[test]
    fn immediate_contact_mutation_is_visible_through_deferred_lease() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let state = ContactDeliveryState {
            kind: 0,
            owner_a: OwnerHandle(10),
            owner_b: OwnerHandle(20),
            body_a_callback_owner: Some(OwnerHandle(11)),
            body_b_callback_owner: None,
            body_a_can_defer: false,
            body_b_can_defer: true,
            body_a_has_physics_handle: true,
            body_b_has_physics_handle: false,
        };
        let mut system = ContactDeliverySystem::new(1);
        system.set_enabled(true);
        let mut handlers = DeliveryRecorder {
            mutate_contact_on_immediate: true,
            ..DeliveryRecorder::default()
        };

        let dispatched = system.dispatch(state, contact, &mut handlers).unwrap();
        assert_eq!(dispatched.deferred_queued, 1);
        assert_eq!(system.drain(&mut handlers).delivered, 1);
        assert_eq!(handlers.observed_contact_kind, Some(3));
    }

    #[test]
    fn deferred_only_contact_does_not_require_unmeasured_speed() {
        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 0,
            owner_a: Some(OwnerHandle(10)),
            owner_b: Some(OwnerHandle(20)),
            horizontal_normal: Some([0.0, 1.0]),
            relative_speed_bits: None,
            next: None,
            guest_next: None,
            guest_identity: Some(ContactGuestId(0x1234)),
        }]);
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let state = ContactDeliveryState {
            kind: 0,
            owner_a: OwnerHandle(10),
            owner_b: OwnerHandle(20),
            body_a_callback_owner: None,
            body_b_callback_owner: None,
            body_a_can_defer: false,
            body_b_can_defer: true,
            body_a_has_physics_handle: true,
            body_b_has_physics_handle: false,
        };
        let mut system = ContactDeliverySystem::new(1);
        system.set_enabled(true);

        let report = system
            .dispatch(state, contact, &mut DeliveryRecorder::default())
            .unwrap();

        assert_eq!(report.deferred_queued, 1);
        assert_eq!(report.immediate_callbacks, 0);
        assert_eq!(system.deferred_count(), 1);
    }

    #[test]
    fn host_queue_steps_keep_default_off_and_callback_reentry_order() {
        let arena = arena();
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let event = |recipient, other, contact: ContactLease| PhysicsEvent {
            recipient: OwnerHandle(recipient),
            other: Some(OwnerHandle(other)),
            horizontal_normal_bits: [0x3f80_0000, 0xbf00_0000],
            contact,
        };
        let mut system = ContactDeliverySystem::new(2);

        assert!(!system.enqueue_deferred(event(20, 10, contact.clone())));
        assert_eq!(system.deferred_count(), 0);
        system.set_enabled(true);
        assert!(system.enqueue_deferred(event(20, 10, contact.clone())));
        let (first_slot, first) = system.pop_deferred_lifo().unwrap();
        assert_eq!(first_slot, 0);
        assert_eq!(first.unwrap().recipient, OwnerHandle(20));
        assert_eq!(system.deferred_count(), 0);

        // This represents a guest callback appending while the popped row is
        // still borrowed by the foreign side. It reuses slot zero, then appears
        // as the next LIFO item without a Rust mutable borrow spanning the call.
        assert!(system.enqueue_deferred(event(30, 11, contact.clone())));
        assert_eq!(
            system.pop_deferred_lifo().unwrap().1.unwrap().recipient,
            OwnerHandle(30)
        );
        assert!(system.finish_deferred_drain());
        assert_eq!(system.deferred_count(), 0);
        assert!(system.pop_deferred_lifo().is_none());
    }

    #[test]
    fn missing_speed_rejects_immediate_dispatch_before_deferred_append() {
        let arena = ContactArena::new(vec![PhysicsContact {
            kind: 3,
            owner_a: Some(OwnerHandle(10)),
            owner_b: Some(OwnerHandle(20)),
            horizontal_normal: Some([0.0, 1.0]),
            relative_speed_bits: None,
            next: None,
            guest_next: None,
            guest_identity: None,
        }]);
        let contact = arena.lease(ContactIndex(0)).unwrap();
        let state = ContactDeliveryState {
            kind: 3,
            owner_a: OwnerHandle(10),
            owner_b: OwnerHandle(20),
            body_a_callback_owner: Some(OwnerHandle(11)),
            body_b_callback_owner: None,
            body_a_can_defer: true,
            body_b_can_defer: false,
            body_a_has_physics_handle: false,
            body_b_has_physics_handle: true,
        };
        let mut system = ContactDeliverySystem::new(1);
        system.set_enabled(true);

        assert_eq!(
            system.dispatch(state, contact, &mut DeliveryRecorder::default()),
            Err(ContactDispatchError::MissingContactSpeed)
        );
        assert_eq!(system.deferred_count(), 0);
    }
}
