// SPDX-License-Identifier: GPL-3.0-or-later
//! Native pool-owner lifetime boundary from T1673.
//!
//! The registry stores typed identities and lease metadata beside the guest image. It never stores
//! owner pointers or guest callback addresses. A client captures an [`OwnedAllocationPermit`]
//! before allocating, publishes only after a successful reclaimable-pool allocation, then consumes
//! a [`ReclaimEvent`] synchronously before
//! returning the old bytes to the pool. Registry mutation finishes before an event is returned, so
//! a client may dispatch it without holding a registry borrow and safely re-enter the registry.
//!
//! This module is opt-in through the default-off `native-owner-lifecycle` Cargo feature. It does
//! not change the legacy callback path or enable game-host routing. Guest-image addresses remain
//! stable allocation locations while the existing memory image is in use.

use crate::contract::{Allocation, HeapKind};
use crate::{pool::PoolHooks, w4m_arena::ArenaHooks};
use tsfp_types::GuestAddr;

/// Monotonic identity for one lifetime of the reclaimable pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PoolEpoch(u64);

impl PoolEpoch {
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Generation-checked identity for a native owner record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnerHandle {
    slot: u32,
    generation: u64,
}

impl OwnerHandle {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Opaque, never-reused value stored in the guest pool header for a native owner callback.
/// It is not a guest pointer; the Rust registry resolves it to a generation-checked owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NativeOwnerToken(u32);

impl NativeOwnerToken {
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Generation-checked identity for one pool allocation lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AllocationId {
    slot: u32,
    generation: u64,
}

impl AllocationId {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

/// Owner identity, callback token, and pool epoch captured before an allocation may reclaim or
/// reset the pool. The permit owns no registry borrow, so an [`OwnerLifecycleHooks`] callback can
/// update the registry while the allocator is running. It is consumed when publication is tried.
#[derive(Debug, PartialEq, Eq)]
pub struct OwnedAllocationPermit {
    owner: OwnerHandle,
    token: NativeOwnerToken,
    epoch: PoolEpoch,
    layout: crate::contract::Layout,
    stride: u32,
}

impl OwnedAllocationPermit {
    pub const fn owner(&self) -> OwnerHandle {
        self.owner
    }

    pub const fn token(&self) -> NativeOwnerToken {
        self.token
    }

    pub const fn epoch(&self) -> PoolEpoch {
        self.epoch
    }

    pub const fn layout(&self) -> crate::contract::Layout {
        self.layout
    }

    pub const fn stride(&self) -> u32 {
        self.stride
    }
}

/// Typed ownership metadata for a stable guest-image allocation.
#[derive(Debug, PartialEq, Eq)]
pub struct BufferLease {
    id: AllocationId,
    owner: OwnerHandle,
    owner_token: Option<NativeOwnerToken>,
    epoch: PoolEpoch,
    address: GuestAddr,
    length: u32,
    stride: u32,
}

impl BufferLease {
    pub const fn id(&self) -> AllocationId {
        self.id
    }

    pub const fn owner(&self) -> OwnerHandle {
        self.owner
    }

    pub const fn owner_token(&self) -> Option<NativeOwnerToken> {
        self.owner_token
    }

    pub const fn epoch(&self) -> PoolEpoch {
        self.epoch
    }

    pub const fn address(&self) -> GuestAddr {
        self.address
    }

    pub const fn length(&self) -> u32 {
        self.length
    }

    pub const fn stride(&self) -> u32 {
        self.stride
    }

    pub const fn element_count(&self) -> u32 {
        self.length / self.stride
    }
}

/// Why a live lease was invalidated. The event's lease is historical and is no longer live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReclaimReason {
    PoolPressure,
    OwnerRelease,
    PoolReset,
    OwnerRetirement,
}

/// Synchronous notification for an owner to invalidate its Rust-side view before reuse/free.
#[derive(Debug, PartialEq, Eq)]
pub struct ReclaimEvent {
    lease: BufferLease,
    reason: ReclaimReason,
}

impl ReclaimEvent {
    pub const fn lease(&self) -> &BufferLease {
        &self.lease
    }

    pub const fn reason(&self) -> ReclaimReason {
        self.reason
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    InvalidOwner(OwnerHandle),
    StaleAllocation(AllocationId),
    WrongEpoch {
        requested: PoolEpoch,
        current: PoolEpoch,
    },
    NotReclaimablePool,
    NullAddress,
    ZeroStride,
    LengthNotStrideMultiple {
        length: u32,
        stride: u32,
    },
    AllocationLayoutMismatch,
    AddressAlreadyLeased(GuestAddr),
    ReclaimPending(GuestAddr),
    StaleEvent(AllocationId),
    InvalidOwnerToken(NativeOwnerToken),
    OwnerTokenMismatch,
    AllocationAddressNotFound(GuestAddr),
    SlotSpaceExhausted,
    EpochExhausted,
    OwnerTokenSpaceExhausted,
}

#[derive(Debug)]
struct OwnerSlot {
    generation: u64,
    live: bool,
    exhausted: bool,
    token: Option<NativeOwnerToken>,
}

#[derive(Debug)]
struct AllocationSlot {
    generation: u64,
    lease: Option<BufferLease>,
    exhausted: bool,
}

#[derive(Debug)]
struct PendingReclaim {
    id: AllocationId,
    address: GuestAddr,
}

/// Typed owner and pool-buffer registry. It owns identities, not the client systems' owner data.
#[derive(Debug)]
pub struct OwnerRegistry {
    epoch: PoolEpoch,
    next_owner_token: Option<u32>,
    owners: Vec<OwnerSlot>,
    allocations: Vec<AllocationSlot>,
    pending: Vec<PendingReclaim>,
}

impl Default for OwnerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl OwnerRegistry {
    pub const fn new() -> Self {
        Self {
            epoch: PoolEpoch(1),
            next_owner_token: Some(1),
            owners: Vec::new(),
            allocations: Vec::new(),
            pending: Vec::new(),
        }
    }

    pub const fn epoch(&self) -> PoolEpoch {
        self.epoch
    }

    /// Register one Rust-owned client. Retired generations are never reused.
    pub fn register_owner(&mut self) -> Result<OwnerHandle, RegistryError> {
        let token = self.allocate_owner_token()?;
        if let Some((index, slot)) = self
            .owners
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| !slot.live && !slot.exhausted)
        {
            slot.live = true;
            slot.token = Some(token);
            return Ok(OwnerHandle {
                slot: index as u32,
                generation: slot.generation,
            });
        }

        let index =
            u32::try_from(self.owners.len()).map_err(|_| RegistryError::SlotSpaceExhausted)?;
        self.owners.push(OwnerSlot {
            generation: 1,
            live: true,
            exhausted: false,
            token: Some(token),
        });
        Ok(OwnerHandle {
            slot: index,
            generation: 1,
        })
    }

    pub fn owner_is_live(&self, owner: OwnerHandle) -> bool {
        self.owners
            .get(owner.slot as usize)
            .is_some_and(|slot| slot.live && slot.generation == owner.generation)
    }

    /// Return the opaque callback value for a live owner.
    pub fn owner_token(&self, owner: OwnerHandle) -> Result<NativeOwnerToken, RegistryError> {
        let Some(slot) = self.owners.get(owner.slot as usize) else {
            return Err(RegistryError::InvalidOwner(owner));
        };
        if !slot.live || slot.generation != owner.generation {
            return Err(RegistryError::InvalidOwner(owner));
        }
        slot.token.ok_or(RegistryError::InvalidOwner(owner))
    }

    /// Capture owner identity and the pool epoch before allocation. The returned permit carries
    /// plain values rather than borrowing this registry, so reclaim callbacks may re-enter it.
    pub fn prepare_owned_allocation(
        &self,
        owner: OwnerHandle,
        layout: crate::contract::Layout,
        stride: u32,
    ) -> Result<OwnedAllocationPermit, RegistryError> {
        if stride == 0 {
            return Err(RegistryError::ZeroStride);
        }
        if layout.size() % stride != 0 {
            return Err(RegistryError::LengthNotStrideMultiple {
                length: layout.size(),
                stride,
            });
        }
        Ok(OwnedAllocationPermit {
            owner,
            token: self.owner_token(owner)?,
            epoch: self.epoch,
            layout,
            stride,
        })
    }

    fn allocate_owner_token(&mut self) -> Result<NativeOwnerToken, RegistryError> {
        let token = self
            .next_owner_token
            .ok_or(RegistryError::OwnerTokenSpaceExhausted)?;
        self.next_owner_token = token.checked_add(1);
        Ok(NativeOwnerToken(token))
    }

    /// Publish metadata after allocation succeeds. Callers free the allocation if publication
    /// fails; a failed allocation must never call this method.
    pub fn publish(
        &mut self,
        owner: OwnerHandle,
        allocation: Allocation,
        stride: u32,
    ) -> Result<AllocationId, RegistryError> {
        self.publish_at(owner, self.epoch, allocation, stride)
    }

    /// Publish a successful allocation when no reset or reclaim callback could run between
    /// allocation and publication. Production callers that allocate through the reclaimable pool
    /// should use [`Self::prepare_owned_allocation`] before allocation and
    /// [`Self::publish_prepared_owned_allocation`] afterward.
    pub fn publish_owned(
        &mut self,
        owner: OwnerHandle,
        token: NativeOwnerToken,
        allocation: Allocation,
        stride: u32,
    ) -> Result<AllocationId, RegistryError> {
        self.publish_at_with_token(owner, self.epoch, allocation, stride, Some(token))
    }

    /// Publish an allocation made with a prior permit. An error leaves the allocation unpublished;
    /// the caller must free it through the same heap contract. A pool reset or owner retirement
    /// during allocation invalidates the permit instead of blessing stale bytes.
    pub fn publish_prepared_owned_allocation(
        &mut self,
        permit: OwnedAllocationPermit,
        allocation: Allocation,
    ) -> Result<AllocationId, RegistryError> {
        if allocation.layout() != permit.layout {
            return Err(RegistryError::AllocationLayoutMismatch);
        }
        self.publish_at_with_token(
            permit.owner,
            permit.epoch,
            allocation,
            permit.stride,
            Some(permit.token),
        )
    }

    /// Publish against the epoch observed before allocation, rejecting a reset that raced it.
    pub fn publish_at(
        &mut self,
        owner: OwnerHandle,
        epoch: PoolEpoch,
        allocation: Allocation,
        stride: u32,
    ) -> Result<AllocationId, RegistryError> {
        self.publish_at_with_token(owner, epoch, allocation, stride, None)
    }

    fn publish_at_with_token(
        &mut self,
        owner: OwnerHandle,
        epoch: PoolEpoch,
        allocation: Allocation,
        stride: u32,
        owner_token: Option<NativeOwnerToken>,
    ) -> Result<AllocationId, RegistryError> {
        if !self.owner_is_live(owner) {
            return Err(RegistryError::InvalidOwner(owner));
        }
        if let Some(token) = owner_token {
            if self.owner_token(owner)? != token {
                return Err(RegistryError::InvalidOwnerToken(token));
            }
        }
        if epoch != self.epoch {
            return Err(RegistryError::WrongEpoch {
                requested: epoch,
                current: self.epoch,
            });
        }
        if allocation.heap() != HeapKind::ReclaimablePool {
            return Err(RegistryError::NotReclaimablePool);
        }
        if allocation.address().0 == 0 {
            return Err(RegistryError::NullAddress);
        }
        if stride == 0 {
            return Err(RegistryError::ZeroStride);
        }
        let length = allocation.layout().size();
        if length % stride != 0 {
            return Err(RegistryError::LengthNotStrideMultiple { length, stride });
        }
        if self
            .pending
            .iter()
            .any(|pending| pending.address == allocation.address())
        {
            return Err(RegistryError::ReclaimPending(allocation.address()));
        }
        if self.allocations.iter().any(|slot| {
            slot.lease
                .as_ref()
                .is_some_and(|lease| lease.address == allocation.address())
        }) {
            return Err(RegistryError::AddressAlreadyLeased(allocation.address()));
        }

        let (index, slot) = if let Some((index, slot)) = self
            .allocations
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.lease.is_none() && !slot.exhausted)
        {
            (index, slot)
        } else {
            let index = u32::try_from(self.allocations.len())
                .map_err(|_| RegistryError::SlotSpaceExhausted)? as usize;
            self.allocations.push(AllocationSlot {
                generation: 1,
                lease: None,
                exhausted: false,
            });
            (index, &mut self.allocations[index])
        };
        let id = AllocationId {
            slot: index as u32,
            generation: slot.generation,
        };
        let lease = BufferLease {
            id,
            owner,
            owner_token,
            epoch,
            address: allocation.address(),
            length,
            stride,
        };
        slot.lease = Some(lease);
        Ok(id)
    }

    /// Invalidate a lease only when both the payload address and opaque owner token match.
    pub fn begin_reclaim_for(
        &mut self,
        address: GuestAddr,
        token: NativeOwnerToken,
    ) -> Result<ReclaimEvent, RegistryError> {
        let Some(slot) = self.allocations.iter().find(|slot| {
            slot.lease
                .as_ref()
                .is_some_and(|lease| lease.address == address)
        }) else {
            return Err(RegistryError::AllocationAddressNotFound(address));
        };
        let lease = slot.lease.as_ref().expect("lease was checked above");
        if lease.owner_token != Some(token) {
            return Err(RegistryError::OwnerTokenMismatch);
        }
        self.begin_reclaim(lease.id)
    }

    pub fn live_lease(&self, id: AllocationId) -> Option<&BufferLease> {
        self.allocations
            .get(id.slot as usize)
            .filter(|slot| slot.generation == id.generation)
            .and_then(|slot| slot.lease.as_ref())
            .filter(|lease| lease.epoch == self.epoch)
    }

    pub fn allocation_is_live(&self, id: AllocationId) -> bool {
        self.live_lease(id).is_some()
    }

    /// Remove identity first; dispatch the returned event before freeing or reusing its address.
    pub fn begin_reclaim(&mut self, id: AllocationId) -> Result<ReclaimEvent, RegistryError> {
        self.take_lease(id, ReclaimReason::PoolPressure)
    }

    /// Remove identity first; dispatch the returned event before explicit pool free.
    pub fn begin_release(&mut self, id: AllocationId) -> Result<ReclaimEvent, RegistryError> {
        self.take_lease(id, ReclaimReason::OwnerRelease)
    }

    /// Acknowledge owner invalidation after the client synchronously handles the event. Until
    /// acknowledgement, the old address stays quarantined and cannot be published to a new owner.
    pub fn complete_reclaim(&mut self, event: ReclaimEvent) -> Result<(), RegistryError> {
        let id = event.lease.id;
        let address = event.lease.address;
        let Some(index) = self
            .pending
            .iter()
            .position(|pending| pending.id == id && pending.address == address)
        else {
            return Err(RegistryError::StaleEvent(id));
        };
        self.pending.swap_remove(index);
        Ok(())
    }

    /// Invalidate every lease before advancing the pool epoch. Returned events are ordered by slot.
    pub fn reset_pool(&mut self) -> Result<Vec<ReclaimEvent>, RegistryError> {
        let next = self
            .epoch
            .0
            .checked_add(1)
            .ok_or(RegistryError::EpochExhausted)?;
        let mut events = Vec::new();
        for slot in &mut self.allocations {
            if let Some(lease) = slot.lease.take() {
                advance_allocation_slot(slot);
                let event = ReclaimEvent {
                    lease,
                    reason: ReclaimReason::PoolReset,
                };
                self.pending.push(PendingReclaim {
                    id: event.lease.id,
                    address: event.lease.address,
                });
                events.push(event);
            }
        }
        self.epoch = PoolEpoch(next);
        Ok(events)
    }

    /// Invalidate every lease and synchronously dispatch the events before returning.
    ///
    /// Call this before resetting the backing pool. During each callback, every reset address is
    /// still quarantined, so reentrant publication cannot attach a new owner to storage whose
    /// invalidation is still being delivered.
    pub fn reset_pool_with_dispatch<D>(&mut self, mut dispatch: D) -> Result<(), RegistryError>
    where
        D: FnMut(&mut OwnerRegistry, &ReclaimEvent),
    {
        let events = self.reset_pool()?;
        self.dispatch_events(events, &mut dispatch)
    }

    /// Retire an owner and invalidate all its leases before the caller drops owner state.
    pub fn retire_owner(&mut self, owner: OwnerHandle) -> Result<Vec<ReclaimEvent>, RegistryError> {
        let Some(owner_slot) = self.owners.get(owner.slot as usize) else {
            return Err(RegistryError::InvalidOwner(owner));
        };
        if !owner_slot.live || owner_slot.generation != owner.generation {
            return Err(RegistryError::InvalidOwner(owner));
        }

        let mut events = Vec::new();
        for slot in &mut self.allocations {
            if slot
                .lease
                .as_ref()
                .is_some_and(|lease| lease.owner == owner)
            {
                let lease = slot.lease.take().expect("lease was checked above");
                advance_allocation_slot(slot);
                let event = ReclaimEvent {
                    lease,
                    reason: ReclaimReason::OwnerRetirement,
                };
                self.pending.push(PendingReclaim {
                    id: event.lease.id,
                    address: event.lease.address,
                });
                events.push(event);
            }
        }
        let owner_slot = &mut self.owners[owner.slot as usize];
        owner_slot.live = false;
        advance_owner_slot(owner_slot);
        Ok(events)
    }

    /// Retire an owner and synchronously dispatch all invalidations before returning.
    ///
    /// The owner generation is revoked before the first callback. Each former allocation address
    /// remains quarantined until its callback returns, then becomes available for a new lease.
    pub fn retire_owner_with_dispatch<D>(
        &mut self,
        owner: OwnerHandle,
        mut dispatch: D,
    ) -> Result<(), RegistryError>
    where
        D: FnMut(&mut OwnerRegistry, &ReclaimEvent),
    {
        let events = self.retire_owner(owner)?;
        self.dispatch_events(events, &mut dispatch)
    }

    fn dispatch_events<D>(
        &mut self,
        events: Vec<ReclaimEvent>,
        dispatch: &mut D,
    ) -> Result<(), RegistryError>
    where
        D: FnMut(&mut OwnerRegistry, &ReclaimEvent),
    {
        for event in events {
            dispatch(self, &event);
            self.complete_reclaim(event)?;
        }
        Ok(())
    }

    fn take_lease(
        &mut self,
        id: AllocationId,
        reason: ReclaimReason,
    ) -> Result<ReclaimEvent, RegistryError> {
        let Some(slot) = self.allocations.get_mut(id.slot as usize) else {
            return Err(RegistryError::StaleAllocation(id));
        };
        if slot.generation != id.generation {
            return Err(RegistryError::StaleAllocation(id));
        }
        let Some(lease) = slot.lease.take() else {
            return Err(RegistryError::StaleAllocation(id));
        };
        advance_allocation_slot(slot);
        let event = ReclaimEvent { lease, reason };
        self.pending.push(PendingReclaim {
            id: event.lease.id,
            address: event.lease.address,
        });
        Ok(event)
    }
}

/// Callback value reserved for the opt-in native owner path. Legacy guest callbacks stay intact.
pub const NATIVE_OWNER_CALLBACK: u32 = u32::MAX;

/// Routes typed owner callbacks before a reclaimable block is freed or its address is reused.
/// The wrapped hooks continue to handle every legacy callback and arena OOM notification.
pub struct OwnerLifecycleHooks<'a, H, D> {
    inner: &'a mut H,
    registry: &'a mut OwnerRegistry,
    dispatch: D,
}

impl<'a, H, D> OwnerLifecycleHooks<'a, H, D> {
    pub fn new(inner: &'a mut H, registry: &'a mut OwnerRegistry, dispatch: D) -> Self {
        Self {
            inner,
            registry,
            dispatch,
        }
    }
}

impl<H: ArenaHooks, D> ArenaHooks for OwnerLifecycleHooks<'_, H, D> {
    fn out_of_memory(&mut self, argument: u32) {
        self.inner.out_of_memory(argument);
    }
}

impl<H, D> PoolHooks for OwnerLifecycleHooks<'_, H, D>
where
    H: PoolHooks + ArenaHooks,
    D: FnMut(&mut OwnerRegistry, &ReclaimEvent),
{
    fn reclaim(&mut self, callback: u32, argument: u32) {
        self.inner.reclaim(callback, argument);
    }

    fn reclaim_block(
        &mut self,
        payload: u32,
        callback: u32,
        argument: u32,
    ) -> Result<(), crate::pool::PoolError> {
        if callback != NATIVE_OWNER_CALLBACK {
            return self.inner.reclaim_block(payload, callback, argument);
        }

        let event = self
            .registry
            .begin_reclaim_for(GuestAddr(payload), NativeOwnerToken(argument))
            .map_err(|_| crate::pool::PoolError::ReclaimHook)?;
        (self.dispatch)(self.registry, &event);
        self.registry
            .complete_reclaim(event)
            .map_err(|_| crate::pool::PoolError::ReclaimHook)
    }
}

/// Failure from the opt-in owned-pool allocation transaction.
#[derive(Debug, PartialEq, Eq)]
pub enum OwnedPoolError {
    Registry(RegistryError),
    Heap(crate::contract::HeapError),
    /// Allocation succeeded, but its owner or epoch changed before publication. The address is
    /// returned so the client can resolve it using the same heap lifecycle; it is never silently
    /// published as a live lease.
    Unpublished {
        allocation: Allocation,
        reason: RegistryError,
    },
}

/// Allocate and publish one native-owned reclaimable buffer through the guest-image heap.
///
/// This composes the pre-allocation permit, `OwnerLifecycleHooks`, the typed `GameHeapContract`
/// call, and post-allocation publication. Reclaim events are synchronously dispatched without a
/// registry borrow, so the client may re-enter the registry. The API is available only through
/// the default-off `native-owner-lifecycle` feature; it does not change the legacy guest callback
/// route or move allocator bytes out of the guest image.
pub fn allocate_owned_with_registry<M, H, D>(
    memory: &mut M,
    inner_hooks: &mut H,
    registry: &mut OwnerRegistry,
    owner: OwnerHandle,
    layout: crate::contract::Layout,
    stride: u32,
    dispatch: D,
) -> Result<Option<(AllocationId, Allocation)>, OwnedPoolError>
where
    M: crate::GuestMemory,
    H: crate::heap::HeapHooks,
    D: FnMut(&mut OwnerRegistry, &ReclaimEvent),
{
    let permit = registry
        .prepare_owned_allocation(owner, layout, stride)
        .map_err(OwnedPoolError::Registry)?;
    let allocation = {
        let token = permit.token();
        let mut hooks = OwnerLifecycleHooks::new(inner_hooks, registry, dispatch);
        let mut heap = crate::heap::GuestImageHeap {
            memory,
            hooks: &mut hooks,
        };
        crate::contract::GameHeapContract::allocate_owned(&mut heap, token, layout)
            .map_err(OwnedPoolError::Heap)?
    };
    let Some(allocation) = allocation else {
        return Ok(None);
    };
    let id = registry
        .publish_prepared_owned_allocation(permit, allocation)
        .map_err(|reason| OwnedPoolError::Unpublished { allocation, reason })?;
    Ok(Some((id, allocation)))
}

fn advance_owner_slot(slot: &mut OwnerSlot) {
    if let Some(next) = slot.generation.checked_add(1) {
        slot.generation = next;
    } else {
        slot.exhausted = true;
    }
}

fn advance_allocation_slot(slot: &mut AllocationSlot) {
    if let Some(next) = slot.generation.checked_add(1) {
        slot.generation = next;
    } else {
        slot.exhausted = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Layout;

    fn allocation(address: u32, length: u32) -> Allocation {
        allocation_from(address, length, HeapKind::ReclaimablePool)
    }

    fn allocation_from(address: u32, length: u32, heap: HeapKind) -> Allocation {
        Allocation::from_parts(
            GuestAddr(address),
            heap,
            Layout::new(length, 16).expect("valid layout"),
        )
    }

    #[test]
    fn reclaim_invalidates_before_dispatch_and_address_reuse_gets_a_new_identity() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let old = registry
            .publish(owner, allocation(0x1000, 0x180), 0x18)
            .unwrap();

        let event = registry.begin_reclaim(old).unwrap();
        assert_eq!(event.reason(), ReclaimReason::PoolPressure);
        assert_eq!(event.lease().id(), old);
        assert!(!registry.allocation_is_live(old));
        assert_eq!(
            registry.publish(owner, allocation(0x1000, 0x180), 0x18),
            Err(RegistryError::ReclaimPending(GuestAddr(0x1000)))
        );
        registry.complete_reclaim(event).unwrap();

        let replacement = registry
            .publish(owner, allocation(0x1000, 0x180), 0x18)
            .unwrap();
        assert_ne!(old, replacement);
        assert!(registry.allocation_is_live(replacement));
        assert_eq!(
            registry.begin_reclaim(old),
            Err(RegistryError::StaleAllocation(old))
        );
    }

    #[test]
    fn reset_and_owner_retirement_invalidate_before_returning_events() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let first = registry
            .publish(owner, allocation(0x2000, 0x80), 0x10)
            .unwrap();
        let old_epoch = registry.epoch();

        let reset_events = registry.reset_pool().unwrap();
        assert_eq!(reset_events.len(), 1);
        assert_eq!(reset_events[0].reason(), ReclaimReason::PoolReset);
        assert_eq!(registry.epoch().get(), old_epoch.get() + 1);
        assert!(!registry.allocation_is_live(first));
        assert!(registry.owner_is_live(owner));
        assert_eq!(
            registry.publish_at(owner, old_epoch, allocation(0x2000, 0x80), 0x10),
            Err(RegistryError::WrongEpoch {
                requested: old_epoch,
                current: registry.epoch(),
            })
        );
        for event in reset_events {
            registry.complete_reclaim(event).unwrap();
        }

        let second = registry
            .publish(owner, allocation(0x2000, 0x80), 0x10)
            .unwrap();
        let retirement = registry.retire_owner(owner).unwrap();
        assert_eq!(retirement.len(), 1);
        assert_eq!(retirement[0].lease().id(), second);
        assert_eq!(retirement[0].reason(), ReclaimReason::OwnerRetirement);
        assert!(!registry.owner_is_live(owner));
        assert!(!registry.allocation_is_live(second));
        for event in retirement {
            registry.complete_reclaim(event).unwrap();
        }

        let next_owner = registry.register_owner().unwrap();
        assert_ne!(owner, next_owner);
    }

    #[test]
    fn exhausted_generations_retire_slots_instead_of_reissuing_stale_ids() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        registry.owners[owner.slot as usize].generation = u64::MAX;
        let max_owner = OwnerHandle {
            slot: owner.slot,
            generation: u64::MAX,
        };
        let events = registry.retire_owner(max_owner).unwrap();
        assert!(events.is_empty());
        assert!(registry.owners[owner.slot as usize].exhausted);

        let next_owner = registry.register_owner().unwrap();
        assert_ne!(max_owner.slot, next_owner.slot);
        assert!(!registry.owner_is_live(max_owner));

        let allocation_id = registry
            .publish(next_owner, allocation(0x2800, 0x80), 0x10)
            .unwrap();
        registry.allocations[allocation_id.slot as usize].generation = u64::MAX;
        let max_allocation = AllocationId {
            slot: allocation_id.slot,
            generation: u64::MAX,
        };
        let event = registry.begin_reclaim(max_allocation).unwrap();
        registry.complete_reclaim(event).unwrap();
        assert!(registry.allocations[max_allocation.slot as usize].exhausted);

        let replacement = registry
            .publish(next_owner, allocation(0x2800, 0x80), 0x10)
            .unwrap();
        assert_ne!(max_allocation.slot, replacement.slot);
        assert_ne!(max_allocation, replacement);
        assert_eq!(
            registry.begin_reclaim(max_allocation),
            Err(RegistryError::StaleAllocation(max_allocation))
        );
    }

    #[test]
    fn owned_allocation_permit_captures_token_epoch_and_layout_before_allocation() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let layout = Layout::new(0x80, 16).unwrap();
        let permit = registry
            .prepare_owned_allocation(owner, layout, 0x10)
            .unwrap();
        assert_eq!(permit.owner(), owner);
        assert_eq!(permit.token(), registry.owner_token(owner).unwrap());
        assert_eq!(permit.epoch(), registry.epoch());
        assert_eq!(permit.layout(), layout);
        assert_eq!(permit.stride(), 0x10);

        let allocation =
            Allocation::from_parts(GuestAddr(0x2B00), HeapKind::ReclaimablePool, layout);
        let id = registry
            .publish_prepared_owned_allocation(permit, allocation)
            .unwrap();
        let lease = registry.live_lease(id).unwrap();
        assert_eq!(lease.owner(), owner);
        assert_eq!(
            lease.owner_token(),
            Some(registry.owner_token(owner).unwrap())
        );
        assert_eq!(lease.epoch(), registry.epoch());
        assert_eq!(lease.address(), GuestAddr(0x2B00));
        assert_eq!(lease.length(), 0x80);
        assert_eq!(lease.stride(), 0x10);
    }

    #[test]
    fn owned_allocation_permit_rejects_pool_reset_during_allocation() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let layout = Layout::new(0x80, 16).unwrap();
        let permit = registry
            .prepare_owned_allocation(owner, layout, 0x10)
            .unwrap();
        let requested_epoch = permit.epoch();

        // A reclaim callback can reset the registry while the heap allocation is in flight.
        registry.reset_pool_with_dispatch(|_, _| {}).unwrap();
        let allocation =
            Allocation::from_parts(GuestAddr(0x2C00), HeapKind::ReclaimablePool, layout);
        assert_eq!(
            registry.publish_prepared_owned_allocation(permit, allocation),
            Err(RegistryError::WrongEpoch {
                requested: requested_epoch,
                current: registry.epoch(),
            })
        );
        assert!(registry.allocations.is_empty());
    }

    #[test]
    fn owned_allocation_permit_rejects_retired_owner_and_wrong_layout() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let layout = Layout::new(0x80, 16).unwrap();
        let wrong_layout = Layout::new(0x90, 16).unwrap();
        let wrong = registry
            .prepare_owned_allocation(owner, layout, 0x10)
            .unwrap();
        assert_eq!(
            registry.publish_prepared_owned_allocation(
                wrong,
                Allocation::from_parts(GuestAddr(0x2D00), HeapKind::ReclaimablePool, wrong_layout,),
            ),
            Err(RegistryError::AllocationLayoutMismatch)
        );

        let permit = registry
            .prepare_owned_allocation(owner, layout, 0x10)
            .unwrap();
        registry
            .retire_owner_with_dispatch(owner, |_, _| {})
            .unwrap();
        assert_eq!(
            registry.publish_prepared_owned_allocation(
                permit,
                Allocation::from_parts(GuestAddr(0x2E00), HeapKind::ReclaimablePool, layout),
            ),
            Err(RegistryError::InvalidOwner(owner))
        );
    }

    #[test]
    fn reset_dispatch_is_synchronous_and_quarantines_all_old_addresses() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let first = registry
            .publish(owner, allocation(0x2800, 0x80), 0x10)
            .unwrap();
        let second = registry
            .publish(owner, allocation(0x2900, 0x80), 0x10)
            .unwrap();
        let old_epoch = registry.epoch();
        let mut seen = Vec::new();

        registry
            .reset_pool_with_dispatch(|registry, event| {
                assert_eq!(event.reason(), ReclaimReason::PoolReset);
                assert_eq!(registry.epoch().get(), old_epoch.get() + 1);
                assert!(!registry.allocation_is_live(first));
                assert!(!registry.allocation_is_live(second));
                assert_eq!(
                    registry.publish(
                        owner,
                        allocation(event.lease().address().0, event.lease().length()),
                        event.lease().stride(),
                    ),
                    Err(RegistryError::ReclaimPending(event.lease().address()))
                );
                seen.push(event.lease().id());
            })
            .unwrap();

        assert_eq!(seen.len(), 2);
        assert!(seen.contains(&first));
        assert!(seen.contains(&second));
        for address in [0x2800, 0x2900] {
            assert!(
                registry
                    .publish(owner, allocation(address, 0x80), 0x10)
                    .is_ok()
            );
        }
    }

    #[test]
    fn owner_retirement_dispatch_revokes_owner_and_releases_quarantine_afterward() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let lease = registry
            .publish(owner, allocation(0x2A00, 0x80), 0x10)
            .unwrap();
        let mut replacement_owner = None;

        registry
            .retire_owner_with_dispatch(owner, |registry, event| {
                assert_eq!(event.reason(), ReclaimReason::OwnerRetirement);
                assert_eq!(event.lease().id(), lease);
                assert!(!registry.owner_is_live(owner));
                assert!(!registry.allocation_is_live(lease));
                let next =
                    *replacement_owner.get_or_insert_with(|| registry.register_owner().unwrap());
                assert_eq!(
                    registry.publish(next, allocation(0x2A00, 0x80), 0x10),
                    Err(RegistryError::ReclaimPending(GuestAddr(0x2A00)))
                );
            })
            .unwrap();

        let next = replacement_owner.unwrap();
        let new_lease = registry
            .publish(next, allocation(0x2A00, 0x80), 0x10)
            .unwrap();
        assert_ne!(lease, new_lease);
        assert!(registry.allocation_is_live(new_lease));
    }

    #[test]
    fn publication_rejects_invalid_owner_heap_address_and_stride() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let stale = OwnerHandle {
            generation: owner.generation() + 1,
            ..owner
        };
        assert_eq!(
            registry.publish(stale, allocation(0x3000, 0x80), 0x10),
            Err(RegistryError::InvalidOwner(stale))
        );
        assert_eq!(
            registry.publish(
                owner,
                allocation_from(0x3000, 0x80, HeapKind::RegionB),
                0x10,
            ),
            Err(RegistryError::NotReclaimablePool)
        );
        assert_eq!(
            registry.publish(owner, allocation(0, 0x80), 0x10),
            Err(RegistryError::NullAddress)
        );
        assert_eq!(
            registry.publish(owner, allocation(0x3000, 0x80), 0),
            Err(RegistryError::ZeroStride)
        );
        assert_eq!(
            registry.publish(owner, allocation(0x3000, 0x81), 0x10),
            Err(RegistryError::LengthNotStrideMultiple {
                length: 0x81,
                stride: 0x10,
            })
        );
        let live = registry
            .publish(owner, allocation(0x4000, 0x80), 0x10)
            .unwrap();
        assert_eq!(
            registry.publish(owner, allocation(0x4000, 0x80), 0x10),
            Err(RegistryError::AddressAlreadyLeased(GuestAddr(0x4000)))
        );
        assert!(registry.live_lease(live).is_some());
    }

    #[test]
    fn many_reuse_cycles_never_revive_stale_allocation_ids() {
        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let mut stale = Vec::new();
        for index in 0..3_000u32 {
            let address = 0x4000 + (index % 8) * 0x100;
            let lease = registry
                .publish(owner, allocation(address, 0x40), 0x10)
                .unwrap();
            stale.push(lease);
            let event = registry.begin_reclaim(lease).unwrap();
            assert_eq!(event.lease().address().0, address);
            registry.complete_reclaim(event).unwrap();
        }
        for id in stale {
            assert!(!registry.allocation_is_live(id));
        }
    }

    #[test]
    fn native_pool_hook_dispatches_after_invalidation_and_before_address_reuse() {
        #[derive(Default)]
        struct Hooks;
        impl ArenaHooks for Hooks {
            fn out_of_memory(&mut self, _: u32) {}
        }
        impl PoolHooks for Hooks {
            fn reclaim(&mut self, _: u32, _: u32) {
                panic!("native token must not reach the legacy callback");
            }
        }

        let mut registry = OwnerRegistry::new();
        let owner = registry.register_owner().unwrap();
        let token = registry.owner_token(owner).unwrap();
        let old = registry
            .publish_owned(owner, token, allocation(0x5000, 0x80), 0x10)
            .unwrap();
        let mut inner = Hooks;
        let mut dispatched = false;
        {
            let mut hooks = OwnerLifecycleHooks::new(
                &mut inner,
                &mut registry,
                |registry: &mut OwnerRegistry, event: &ReclaimEvent| {
                    assert_eq!(event.reason(), ReclaimReason::PoolPressure);
                    assert_eq!(event.lease().id(), old);
                    assert!(!registry.allocation_is_live(old));
                    assert_eq!(
                        registry.publish_owned(owner, token, allocation(0x5000, 0x80), 0x10),
                        Err(RegistryError::ReclaimPending(GuestAddr(0x5000)))
                    );
                    dispatched = true;
                },
            );
            assert_eq!(
                PoolHooks::reclaim_block(
                    &mut hooks,
                    0x5000,
                    NATIVE_OWNER_CALLBACK,
                    token.get().wrapping_add(1),
                ),
                Err(crate::pool::PoolError::ReclaimHook)
            );
        }
        assert!(registry.allocation_is_live(old));
        {
            let mut hooks = OwnerLifecycleHooks::new(
                &mut inner,
                &mut registry,
                |registry: &mut OwnerRegistry, event: &ReclaimEvent| {
                    assert_eq!(event.reason(), ReclaimReason::PoolPressure);
                    assert_eq!(event.lease().id(), old);
                    assert!(!registry.allocation_is_live(old));
                    assert_eq!(
                        registry.publish_owned(owner, token, allocation(0x5000, 0x80), 0x10),
                        Err(RegistryError::ReclaimPending(GuestAddr(0x5000)))
                    );
                    dispatched = true;
                },
            );
            PoolHooks::reclaim_block(&mut hooks, 0x5000, NATIVE_OWNER_CALLBACK, token.get())
                .unwrap();
        }
        assert!(dispatched);
        let current = registry
            .publish_owned(owner, token, allocation(0x5000, 0x80), 0x10)
            .unwrap();
        assert_ne!(old, current);
    }
}
