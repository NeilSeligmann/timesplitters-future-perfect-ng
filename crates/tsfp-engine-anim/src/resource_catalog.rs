//! Owned clip slots and their legacy ID/policy view.
//!
//! This is an additive native owner. `BindingWorld` still supplies the signed
//! slot observed after a load; this catalog does not infer archive residency,
//! load success, or the original engine's allocation/refcount rules.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_CATALOG_ID: AtomicU64 = AtomicU64::new(1);

fn next_catalog_id() -> u64 {
    NEXT_CATALOG_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .expect("animation resource catalog identity space exhausted")
}

/// Identifies one incarnation of the catalog, separate from object lifetimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceGeneration(u64);

/// Rust archive/resource owner identity scoped to one catalog generation.
/// This is a native ownership token, not an original engine handle or count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceOwnerId {
    catalog_id: u64,
    generation: ResourceGeneration,
    key: u64,
}

/// Stable lookup token for a clip slot until that slot is replaced or the
/// resource generation is reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipSlotHandle {
    generation: ResourceGeneration,
    slot: i16,
    revision: u64,
}

/// A non-cloneable token keeping a slot alive while a Rust consumer uses it.
/// This is a native lifetime guard, not an original-engine reference count.
#[must_use = "an active clip use must be ended before its resource can retire"]
#[derive(Debug, PartialEq, Eq)]
pub struct ClipUse {
    catalog_id: u64,
    use_id: u64,
    handle: ClipSlotHandle,
}

impl ClipSlotHandle {
    pub fn generation(self) -> ResourceGeneration {
        self.generation
    }
    pub fn legacy_slot(self) -> i16 {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogError {
    GenerationExhausted,
    BindingOrderExhausted,
    ForeignOwner,
    StaleOwner,
    OwnersActive,
    SlotMissing,
    SlotOwned,
    SlotInUse,
    SlotRevisionExhausted,
    UseIdExhausted,
    UseCountExhausted,
    ForeignUse,
    StaleUse,
    UsesActive,
}

/// A slot whose last Rust owner released it. The caller receives and drops
/// the payload after the catalog has invalidated handles and removed aliases.
pub struct RetiredSlot<T> {
    pub slot: i16,
    pub clip: Option<T>,
}

struct Slot<T> {
    clip: Option<T>,
    policy: [u32; 2],
    revision: u64,
}

#[derive(Clone, Copy)]
struct IdBinding {
    slot: i16,
    order: u64,
}

/// A read-only view keeps the catalog immutably borrowed while clip data is in
/// use, so slot replacement cannot invalidate a live Rust reference.
pub struct ResourceView<'a, T> {
    pub handle: ClipSlotHandle,
    pub clip: Option<&'a T>,
    pub policy: [u32; 2],
}

/// Owns parsed clip payloads by signed legacy slot, with IDs and the two policy
/// words attached at the same per-slot boundary as the original tables.
/// Instantiate with `tsfp_format_anim::Clip` (or an owned `Arc<Clip>`) for the
/// native animation path.
pub struct AnimationResourceCatalog<T> {
    catalog_id: u64,
    generation: ResourceGeneration,
    ids: BTreeMap<u16, i16>,
    unowned_ids: BTreeMap<u16, IdBinding>,
    owned_ids: BTreeMap<ResourceOwnerId, BTreeMap<u16, IdBinding>>,
    binding_order: u64,
    slots: BTreeMap<i16, Slot<T>>,
    slot_revisions: BTreeMap<i16, u64>,
    owners_by_slot: BTreeMap<i16, BTreeSet<ResourceOwnerId>>,
    slots_by_owner: BTreeMap<ResourceOwnerId, BTreeSet<i16>>,
    next_use_id: u64,
    active_uses: BTreeMap<u64, ClipSlotHandle>,
    active_use_counts: BTreeMap<i16, usize>,
}

impl<T> Default for AnimationResourceCatalog<T> {
    fn default() -> Self {
        Self {
            catalog_id: next_catalog_id(),
            generation: ResourceGeneration(1),
            ids: BTreeMap::new(),
            unowned_ids: BTreeMap::new(),
            owned_ids: BTreeMap::new(),
            binding_order: 0,
            slots: BTreeMap::new(),
            slot_revisions: BTreeMap::new(),
            owners_by_slot: BTreeMap::new(),
            slots_by_owner: BTreeMap::new(),
            next_use_id: 0,
            active_uses: BTreeMap::new(),
            active_use_counts: BTreeMap::new(),
        }
    }
}

impl<T> AnimationResourceCatalog<T> {
    pub fn generation(&self) -> ResourceGeneration {
        self.generation
    }

    /// Create an opaque owner token for a caller-stable archive/resource key.
    /// The token is bound to this catalog and generation, so it cannot release
    /// resources in another catalog or a later generation.
    pub fn owner_id(&self, key: u64) -> ResourceOwnerId {
        ResourceOwnerId {
            catalog_id: self.catalog_id,
            generation: self.generation,
            key,
        }
    }

    pub(crate) fn validate_owner(&self, owner: ResourceOwnerId) -> Result<(), CatalogError> {
        if owner.catalog_id != self.catalog_id {
            return Err(CatalogError::ForeignOwner);
        }
        (owner.generation == self.generation)
            .then_some(())
            .ok_or(CatalogError::StaleOwner)
    }

    /// Publish one completed load observation. The caller passes the signed
    /// ID slot read after the loader call, not the loader's return register.
    /// `clip == None` represents no newly decoded clip: an existing slot's
    /// owned payload remains in place while the observed policy overwrite is
    /// still applied. No success or safe-slot-domain rule is invented here.
    pub fn publish(
        &mut self,
        id: u16,
        slot: i16,
        policy: [u32; 2],
        clip: Option<T>,
    ) -> Result<ClipSlotHandle, CatalogError> {
        self.publish_inner(None, id, slot, policy, clip)
    }

    fn publish_inner(
        &mut self,
        owner: Option<ResourceOwnerId>,
        id: u16,
        slot: i16,
        policy: [u32; 2],
        clip: Option<T>,
    ) -> Result<ClipSlotHandle, CatalogError> {
        if let Some(owner) = owner {
            self.validate_owner(owner)?;
        }
        if clip.is_some() {
            if self.owners_by_slot.contains_key(&slot) {
                return Err(CatalogError::SlotOwned);
            }
            if self.slot_has_active_uses(slot) {
                return Err(CatalogError::SlotInUse);
            }
        }
        let next_binding_order = self
            .binding_order
            .checked_add(1)
            .ok_or(CatalogError::BindingOrderExhausted)?;
        let previous_revision = self.slot_revisions.get(&slot).copied().unwrap_or(0);
        let next_revision = if clip.is_some() {
            previous_revision
                .checked_add(1)
                .ok_or(CatalogError::SlotRevisionExhausted)?
        } else {
            previous_revision
        };

        let revision = {
            let resource = self.slots.entry(slot).or_insert(Slot {
                clip: None,
                policy,
                revision: 0,
            });
            if let Some(clip) = clip {
                resource.clip = Some(clip);
                resource.revision = next_revision;
                self.slot_revisions.insert(slot, next_revision);
            }
            resource.policy = policy;
            resource.revision
        };
        self.binding_order = next_binding_order;
        let binding = IdBinding {
            slot,
            order: next_binding_order,
        };
        if let Some(owner) = owner {
            self.owners_by_slot.entry(slot).or_default().insert(owner);
            self.slots_by_owner.entry(owner).or_default().insert(slot);
            self.owned_ids.entry(owner).or_default().insert(id, binding);
        } else {
            self.unowned_ids.insert(id, binding);
        }
        self.refresh_id(id);

        Ok(ClipSlotHandle {
            generation: self.generation,
            slot,
            revision,
        })
    }

    /// Publish a slot and attach its Rust archive owner in the same call, so
    /// callers cannot accidentally leave a newly published resource
    /// unowned between those steps. The owner identity and lifecycle remain
    /// caller policy; this does not infer original shared-release behavior.
    pub fn publish_for_owner(
        &mut self,
        owner: ResourceOwnerId,
        id: u16,
        slot: i16,
        policy: [u32; 2],
        clip: Option<T>,
    ) -> Result<ClipSlotHandle, CatalogError> {
        self.publish_inner(Some(owner), id, slot, policy, clip)
    }

    fn refresh_id(&mut self, id: u16) {
        let mut latest = self.unowned_ids.get(&id).copied();
        for bindings in self.owned_ids.values() {
            if let Some(candidate) = bindings.get(&id).copied() {
                if latest.is_none_or(|current| candidate.order > current.order) {
                    latest = Some(candidate);
                }
            }
        }
        if let Some(binding) = latest {
            self.ids.insert(id, binding.slot);
        } else {
            self.ids.remove(&id);
        }
    }

    pub fn handle_for_id(&self, id: u16) -> Option<ClipSlotHandle> {
        let slot = *self.ids.get(&id)?;
        let resource = self.slots.get(&slot)?;
        Some(ClipSlotHandle {
            generation: self.generation,
            slot,
            revision: resource.revision,
        })
    }

    pub fn by_id(&self, id: u16) -> Option<ResourceView<'_, T>> {
        self.view(self.handle_for_id(id)?)
    }

    pub fn by_slot(&self, slot: i16) -> Option<ResourceView<'_, T>> {
        let resource = self.slots.get(&slot)?;
        self.view(ClipSlotHandle {
            generation: self.generation,
            slot,
            revision: resource.revision,
        })
    }

    /// Stale handles fail after resource replacement or generation reset.
    pub fn view(&self, handle: ClipSlotHandle) -> Option<ResourceView<'_, T>> {
        if handle.generation != self.generation {
            return None;
        }
        let resource = self.slots.get(&handle.slot)?;
        if resource.revision != handle.revision {
            return None;
        }
        Some(ResourceView {
            handle,
            clip: resource.clip.as_ref(),
            policy: resource.policy,
        })
    }

    /// Keep a slot alive across Rust-owned use (for example, an active blend).
    /// The token must be returned with `end_use`; it does not model the
    /// original engine's unresolved shared-resource lifetime semantics.
    pub fn begin_use(&mut self, handle: ClipSlotHandle) -> Result<ClipUse, CatalogError> {
        let Some(resource) = self.view(handle).and_then(|_| self.slots.get(&handle.slot)) else {
            return Err(CatalogError::SlotMissing);
        };
        if !self.owners_by_slot.contains_key(&handle.slot)
            && resource.revision.checked_add(1).is_none()
        {
            return Err(CatalogError::SlotRevisionExhausted);
        }
        let next_use_id = self
            .next_use_id
            .checked_add(1)
            .ok_or(CatalogError::UseIdExhausted)?;
        let next_count = self
            .active_use_counts
            .get(&handle.slot)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(CatalogError::UseCountExhausted)?;

        self.next_use_id = next_use_id;
        self.active_uses.insert(next_use_id, handle);
        self.active_use_counts.insert(handle.slot, next_count);
        Ok(ClipUse {
            catalog_id: self.catalog_id,
            use_id: next_use_id,
            handle,
        })
    }

    /// End one active use. If its slot has no archive owner and this was the
    /// final use, return the retired payload to the caller for destruction.
    /// The token is borrowed so a failed validation leaves it available for a
    /// retry against the catalog that created it.
    pub fn end_use(&mut self, usage: &ClipUse) -> Result<Option<RetiredSlot<T>>, CatalogError> {
        if usage.catalog_id != self.catalog_id {
            return Err(CatalogError::ForeignUse);
        }
        if usage.handle.generation != self.generation
            || self.active_uses.get(&usage.use_id) != Some(&usage.handle)
        {
            return Err(CatalogError::StaleUse);
        }
        let count = *self
            .active_use_counts
            .get(&usage.handle.slot)
            .expect("active-use token has a slot count");
        let final_use = count == 1;
        let ownerless = !self.owners_by_slot.contains_key(&usage.handle.slot);
        if final_use && ownerless {
            if let Some(resource) = self.slots.get(&usage.handle.slot) {
                resource
                    .revision
                    .checked_add(1)
                    .ok_or(CatalogError::SlotRevisionExhausted)?;
            }
        }

        self.active_uses.remove(&usage.use_id);
        if final_use {
            self.active_use_counts.remove(&usage.handle.slot);
        } else {
            self.active_use_counts.insert(usage.handle.slot, count - 1);
        }
        if final_use && ownerless {
            let clip = self.retire_unowned_slot(usage.handle.slot)?;
            return Ok(Some(RetiredSlot {
                slot: usage.handle.slot,
                clip,
            }));
        }
        Ok(None)
    }

    fn slot_has_active_uses(&self, slot: i16) -> bool {
        self.active_use_counts
            .get(&slot)
            .is_some_and(|count| *count > 0)
    }

    /// Attach a Rust resource owner to an existing catalog slot. Repeating
    /// the same attachment is idempotent. This policy is opt-in and does not
    /// claim to reconstruct original archive sharing semantics.
    pub fn attach_owner(
        &mut self,
        owner: ResourceOwnerId,
        slot: i16,
    ) -> Result<bool, CatalogError> {
        self.validate_owner(owner)?;
        if !self.slots.contains_key(&slot) {
            return Err(CatalogError::SlotMissing);
        }
        let inserted = self.owners_by_slot.entry(slot).or_default().insert(owner);
        self.slots_by_owner.entry(owner).or_default().insert(slot);
        Ok(inserted)
    }

    /// Release every slot held by one Rust owner. Slots with other owners
    /// remain live; slots whose last owner releases are invalidated and
    /// returned for payload destruction. Revision overflow is checked before
    /// any ownership or catalog state changes.
    pub fn release_owner(
        &mut self,
        owner: ResourceOwnerId,
    ) -> Result<Vec<RetiredSlot<T>>, CatalogError> {
        self.validate_owner(owner)?;
        let Some(slots) = self.slots_by_owner.get(&owner).cloned() else {
            return Ok(Vec::new());
        };

        for slot in &slots {
            let last_owner = self
                .owners_by_slot
                .get(slot)
                .is_some_and(|owners| owners.len() == 1 && owners.contains(&owner));
            if last_owner {
                if let Some(resource) = self.slots.get(slot) {
                    resource
                        .revision
                        .checked_add(1)
                        .ok_or(CatalogError::SlotRevisionExhausted)?;
                }
            }
        }

        let binding_ids: Vec<_> = self
            .owned_ids
            .remove(&owner)
            .into_iter()
            .flat_map(|bindings| bindings.into_keys())
            .collect();
        for id in binding_ids {
            self.refresh_id(id);
        }
        self.slots_by_owner.remove(&owner);
        let mut retired = Vec::new();
        for slot in slots {
            let owners = self
                .owners_by_slot
                .get_mut(&slot)
                .expect("owner-to-slot index must have a reverse entry");
            owners.remove(&owner);
            if owners.is_empty() {
                self.owners_by_slot.remove(&slot);
                if !self.slot_has_active_uses(slot) {
                    let resource = self.retire_unowned_slot(slot)?;
                    retired.push(RetiredSlot {
                        slot,
                        clip: resource,
                    });
                }
            }
        }
        Ok(retired)
    }

    /// Retire one Rust-owned slot after its resource owner has decided that
    /// the slot is no longer resident. All IDs pointing at the slot are
    /// removed and the owned clip is returned to the caller. This operation
    /// does not infer original shared-owner/refcount semantics; the caller
    /// must decide when slot retirement is appropriate.
    pub fn retire_slot(&mut self, slot: i16) -> Result<Option<T>, CatalogError> {
        if self.owners_by_slot.contains_key(&slot) {
            return Err(CatalogError::SlotOwned);
        }
        if self.slot_has_active_uses(slot) {
            return Err(CatalogError::SlotInUse);
        }
        self.retire_unowned_slot(slot)
    }

    fn retire_unowned_slot(&mut self, slot: i16) -> Result<Option<T>, CatalogError> {
        let Some(resource) = self.slots.get(&slot) else {
            return Ok(None);
        };
        let next_revision = resource
            .revision
            .checked_add(1)
            .ok_or(CatalogError::SlotRevisionExhausted)?;

        let resource = self
            .slots
            .remove(&slot)
            .expect("slot was checked before retirement");
        self.slot_revisions.insert(slot, next_revision);
        let removed_ids: Vec<_> = self
            .unowned_ids
            .iter()
            .filter_map(|(id, binding)| (binding.slot == slot).then_some(*id))
            .collect();
        self.unowned_ids.retain(|_, binding| binding.slot != slot);
        for id in removed_ids {
            self.refresh_id(id);
        }
        self.ids.retain(|_, mapped_slot| *mapped_slot != slot);
        Ok(resource.clip)
    }

    /// Begin a new archive/resource lifetime. Old handles are rejected, and no
    /// clip or ID mapping crosses into the new generation. Owners must release
    /// first so this reset cannot silently drop a live owner's resource.
    pub fn begin_generation(&mut self) -> Result<ResourceGeneration, CatalogError> {
        if !self.slots_by_owner.is_empty() {
            return Err(CatalogError::OwnersActive);
        }
        if !self.active_uses.is_empty() {
            return Err(CatalogError::UsesActive);
        }
        let next = self
            .generation
            .0
            .checked_add(1)
            .ok_or(CatalogError::GenerationExhausted)?;
        self.generation = ResourceGeneration(next);
        self.ids.clear();
        self.unowned_ids.clear();
        self.owned_ids.clear();
        self.binding_order = 0;
        self.slots.clear();
        self.slot_revisions.clear();
        self.owners_by_slot.clear();
        self.slots_by_owner.clear();
        self.active_uses.clear();
        self.active_use_counts.clear();
        Ok(self.generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn owner<T>(catalog: &AnimationResourceCatalog<T>, key: u64) -> ResourceOwnerId {
        catalog.owner_id(key)
    }

    #[test]
    fn ids_share_slot_payload_and_last_policy_wins_per_slot() {
        let mut catalog = AnimationResourceCatalog::default();
        let first = catalog.publish(12, 4, [1, 2], Some("clip-a")).unwrap();
        let shared = catalog.publish(13, 4, [3, 4], None).unwrap();

        let a = catalog.by_id(12).unwrap();
        let b = catalog.by_id(13).unwrap();
        assert_eq!(a.handle, shared);
        assert_eq!(b.handle, shared);
        assert_eq!(a.clip, Some(&"clip-a"));
        assert_eq!(b.clip, Some(&"clip-a"));
        assert_eq!(a.policy, [3, 4]);
        assert_eq!(b.policy, [3, 4]);
        assert!(catalog.view(first).is_some());
    }

    #[test]
    fn replacement_invalidates_old_slot_handle_but_moves_ids_with_the_slot() {
        let mut catalog = AnimationResourceCatalog::default();
        let old = catalog.publish(12, -1, [5, 6], Some("old")).unwrap();
        let current = catalog.publish(12, -1, [7, 8], Some("new")).unwrap();

        assert!(catalog.view(old).is_none());
        assert_eq!(current.legacy_slot(), -1);
        let view = catalog.by_id(12).unwrap();
        assert_eq!(view.clip, Some(&"new"));
        assert_eq!(view.policy, [7, 8]);
    }

    #[test]
    fn new_slot_without_decoded_clip_keeps_an_explicit_empty_view() {
        let mut catalog = AnimationResourceCatalog::<&str>::default();
        catalog.publish(22, 7, [9, 10], None).unwrap();
        let view = catalog.by_id(22).unwrap();
        assert_eq!(view.clip, None);
        assert_eq!(view.policy, [9, 10]);
    }

    #[test]
    fn new_generation_drops_ids_and_invalidates_all_old_handles() {
        let mut catalog = AnimationResourceCatalog::default();
        let old = catalog.publish(12, 4, [1, 2], Some("clip")).unwrap();
        let before = catalog.generation();
        let after = catalog.begin_generation().unwrap();

        assert_ne!(before, after);
        assert!(catalog.view(old).is_none());
        assert!(catalog.by_id(12).is_none());
    }

    #[test]
    fn retiring_shared_slot_removes_all_aliases_and_returns_owned_clip() {
        let mut catalog = AnimationResourceCatalog::default();
        let handle = catalog.publish(12, 4, [1, 2], Some("clip")).unwrap();
        catalog.publish(13, 4, [3, 4], None).unwrap();
        catalog.publish(22, 9, [5, 6], Some("other")).unwrap();

        assert_eq!(catalog.retire_slot(4).unwrap(), Some("clip"));
        assert!(catalog.view(handle).is_none());
        assert!(catalog.by_id(12).is_none());
        assert!(catalog.by_id(13).is_none());
        assert_eq!(catalog.by_id(22).unwrap().clip, Some(&"other"));
        assert_eq!(catalog.retire_slot(4).unwrap(), None);
    }

    #[test]
    fn reused_slot_does_not_revalidate_a_retired_handle() {
        let mut catalog = AnimationResourceCatalog::default();
        let retired = catalog.publish(12, 4, [1, 2], Some("old")).unwrap();
        assert_eq!(catalog.retire_slot(4).unwrap(), Some("old"));

        let current = catalog.publish(13, 4, [3, 4], Some("new")).unwrap();
        assert_ne!(retired, current);
        assert!(catalog.view(retired).is_none());
        assert_eq!(catalog.view(current).unwrap().clip, Some(&"new"));
    }

    #[test]
    fn retirement_transfers_clip_drop_to_caller_and_generation_reset_drops_owned_clips() {
        struct DropProbe(Rc<Cell<usize>>);

        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Rc::new(Cell::new(0));
        let mut catalog = AnimationResourceCatalog::default();
        catalog
            .publish(12, 4, [1, 2], Some(DropProbe(Rc::clone(&drops))))
            .unwrap();

        let retired = catalog.retire_slot(4).unwrap().unwrap();
        assert_eq!(drops.get(), 0);
        drop(retired);
        assert_eq!(drops.get(), 1);

        catalog
            .publish(13, 5, [3, 4], Some(DropProbe(Rc::clone(&drops))))
            .unwrap();
        catalog.begin_generation().unwrap();
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn shared_rust_owners_keep_slot_live_until_the_last_owner_releases() {
        let mut catalog = AnimationResourceCatalog::default();
        let handle = catalog.publish(12, 4, [1, 2], Some("clip")).unwrap();
        let archive_a = owner(&catalog, 10);
        let archive_b = owner(&catalog, 20);

        assert!(catalog.attach_owner(archive_a, 4).unwrap());
        assert!(!catalog.attach_owner(archive_a, 4).unwrap());
        assert!(catalog.attach_owner(archive_b, 4).unwrap());
        assert!(matches!(
            catalog.retire_slot(4),
            Err(CatalogError::SlotOwned)
        ));

        assert!(catalog.release_owner(archive_a).unwrap().is_empty());
        assert_eq!(catalog.view(handle).unwrap().clip, Some(&"clip"));

        let retired = catalog.release_owner(archive_b).unwrap();
        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0].slot, 4);
        assert_eq!(retired[0].clip, Some("clip"));
        assert!(catalog.view(handle).is_none());
        assert!(catalog.by_id(12).is_none());
        assert!(catalog.release_owner(archive_b).unwrap().is_empty());
    }

    #[test]
    fn releasing_newer_owner_restores_older_id_alias() {
        let mut catalog = AnimationResourceCatalog::default();
        let older = owner(&catalog, 31);
        let newer = owner(&catalog, 32);
        let older_handle = catalog
            .publish_for_owner(older, 12, 4, [1, 2], Some("older"))
            .unwrap();
        catalog
            .publish_for_owner(newer, 12, 7, [3, 4], Some("newer"))
            .unwrap();
        assert_eq!(catalog.by_id(12).unwrap().clip, Some(&"newer"));

        let retired = catalog.release_owner(newer).unwrap();

        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0].slot, 7);
        let restored = catalog.by_id(12).unwrap();
        assert_eq!(restored.handle, older_handle);
        assert_eq!(restored.clip, Some(&"older"));
        assert_eq!(restored.policy, [1, 2]);
        assert!(catalog.by_slot(7).is_none());
    }

    #[test]
    fn binding_order_exhaustion_leaves_catalog_unchanged() {
        let mut catalog = AnimationResourceCatalog::<&str>::default();
        let owner = owner(&catalog, 33);
        catalog.binding_order = u64::MAX;

        assert_eq!(
            catalog.publish_for_owner(owner, 12, 4, [1, 2], Some("clip")),
            Err(CatalogError::BindingOrderExhausted)
        );
        assert!(catalog.by_id(12).is_none());
        assert!(catalog.by_slot(4).is_none());
        assert!(!catalog.slots_by_owner.contains_key(&owner));
        assert!(!catalog.owners_by_slot.contains_key(&4));
    }

    #[test]
    fn owner_release_retires_only_its_unshared_slots() {
        let mut catalog = AnimationResourceCatalog::default();
        let a_only = catalog.publish(1, 3, [0, 0], Some("a-only")).unwrap();
        let shared = catalog.publish(2, 4, [0, 0], Some("shared")).unwrap();
        let owner_a = owner(&catalog, 1);
        let owner_b = owner(&catalog, 2);
        catalog.attach_owner(owner_a, 3).unwrap();
        catalog.attach_owner(owner_a, 4).unwrap();
        catalog.attach_owner(owner_b, 4).unwrap();

        let retired = catalog.release_owner(owner_a).unwrap();
        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0].slot, 3);
        assert_eq!(retired[0].clip, Some("a-only"));
        assert!(catalog.view(a_only).is_none());
        assert_eq!(catalog.view(shared).unwrap().clip, Some(&"shared"));
        assert_eq!(catalog.retire_slot(4), Err(CatalogError::SlotOwned));

        let final_release = catalog.release_owner(owner_b).unwrap();
        assert_eq!(final_release.len(), 1);
        assert_eq!(final_release[0].slot, 4);
    }

    #[test]
    fn owner_attachment_requires_a_published_slot_and_generation_reset_requires_release() {
        let mut catalog = AnimationResourceCatalog::<&str>::default();
        let owner = owner(&catalog, 7);
        assert_eq!(
            catalog.attach_owner(owner, 8),
            Err(CatalogError::SlotMissing)
        );

        catalog.publish(2, 8, [0, 0], None).unwrap();
        catalog.attach_owner(owner, 8).unwrap();
        assert_eq!(catalog.begin_generation(), Err(CatalogError::OwnersActive));
        assert_eq!(catalog.release_owner(owner).unwrap().len(), 1);
        catalog.begin_generation().unwrap();
        assert!(matches!(
            catalog.release_owner(owner),
            Err(CatalogError::StaleOwner)
        ));
    }

    #[test]
    fn owner_bound_publication_cannot_be_retired_before_owner_release() {
        let mut catalog = AnimationResourceCatalog::default();
        let owner = owner(&catalog, 42);
        let handle = catalog
            .publish_for_owner(owner, 12, 4, [5, 6], Some("clip"))
            .unwrap();

        assert!(matches!(
            catalog.retire_slot(4),
            Err(CatalogError::SlotOwned)
        ));
        assert_eq!(catalog.view(handle).unwrap().clip, Some(&"clip"));
        let retired = catalog.release_owner(owner).unwrap();
        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0].clip, Some("clip"));
        assert!(catalog.view(handle).is_none());
    }

    #[test]
    fn active_use_defers_last_owner_retirement_until_use_ends() {
        let mut catalog = AnimationResourceCatalog::default();
        let owner = owner(&catalog, 7);
        let handle = catalog
            .publish_for_owner(owner, 12, 4, [5, 6], Some("blend clip"))
            .unwrap();
        let use_token = catalog.begin_use(handle).unwrap();

        assert!(catalog.release_owner(owner).unwrap().is_empty());
        assert!(catalog.by_id(12).is_none());
        assert_eq!(catalog.view(handle).unwrap().clip, Some(&"blend clip"));
        assert_eq!(catalog.begin_generation(), Err(CatalogError::UsesActive));

        let retired = catalog.end_use(&use_token).unwrap().unwrap();
        assert_eq!(retired.slot, 4);
        assert_eq!(retired.clip, Some("blend clip"));
        assert!(catalog.view(handle).is_none());
        assert!(catalog.by_slot(4).is_none());
    }

    #[test]
    fn foreign_catalog_rejection_keeps_use_token_retryable() {
        let mut source = AnimationResourceCatalog::default();
        let handle = source.publish(12, 4, [5, 6], Some("clip")).unwrap();
        let use_token = source.begin_use(handle).unwrap();
        let mut foreign = AnimationResourceCatalog::<&str>::default();

        assert!(matches!(
            foreign.end_use(&use_token),
            Err(CatalogError::ForeignUse)
        ));
        let retired = source.end_use(&use_token).unwrap().unwrap();
        assert_eq!(retired.clip, Some("clip"));
        assert!(source.active_uses.is_empty());
    }

    #[test]
    fn revision_exhaustion_preserves_owner_and_blocks_unreleasable_use() {
        let mut catalog = AnimationResourceCatalog::default();
        catalog.publish(12, 4, [5, 6], Some("clip")).unwrap();
        catalog.slots.get_mut(&4).unwrap().revision = u64::MAX;
        let handle = catalog.by_slot(4).unwrap().handle;
        assert_eq!(
            catalog.begin_use(handle),
            Err(CatalogError::SlotRevisionExhausted)
        );
        assert!(catalog.active_uses.is_empty());

        let owner = owner(&catalog, 13);
        catalog.attach_owner(owner, 4).unwrap();
        let use_token = catalog.begin_use(handle).unwrap();
        assert!(matches!(
            catalog.release_owner(owner),
            Err(CatalogError::SlotRevisionExhausted)
        ));
        assert_eq!(catalog.by_id(12).unwrap().clip, Some(&"clip"));
        assert!(catalog.active_uses.contains_key(&use_token.use_id));

        catalog.slots.get_mut(&4).unwrap().revision = u64::MAX - 1;
        assert!(catalog.end_use(&use_token).unwrap().is_none());
        let retired = catalog.release_owner(owner).unwrap();
        assert_eq!(retired[0].clip, Some("clip"));
    }

    #[test]
    fn shared_owners_and_multiple_uses_keep_slot_live_until_both_are_gone() {
        let mut catalog = AnimationResourceCatalog::default();
        let archive_a = owner(&catalog, 7);
        let archive_b = owner(&catalog, 8);
        let handle = catalog
            .publish_for_owner(archive_a, 12, 4, [5, 6], Some("shared blend clip"))
            .unwrap();
        catalog.attach_owner(archive_b, 4).unwrap();
        let first_use = catalog.begin_use(handle).unwrap();
        let second_use = catalog.begin_use(handle).unwrap();

        assert!(catalog.release_owner(archive_a).unwrap().is_empty());
        assert!(catalog.release_owner(archive_b).unwrap().is_empty());
        assert!(catalog.end_use(&first_use).unwrap().is_none());
        assert_eq!(
            catalog.view(handle).unwrap().clip,
            Some(&"shared blend clip")
        );

        let retired = catalog.end_use(&second_use).unwrap().unwrap();
        assert_eq!(retired.slot, 4);
        assert_eq!(retired.clip, Some("shared blend clip"));
        assert!(catalog.view(handle).is_none());
    }

    #[test]
    fn active_use_rejects_replacement_and_direct_retirement() {
        let mut catalog = AnimationResourceCatalog::default();
        let handle = catalog.publish(12, 4, [1, 2], Some("current")).unwrap();
        let use_token = catalog.begin_use(handle).unwrap();

        assert!(matches!(
            catalog.publish(13, 4, [3, 4], Some("replacement")),
            Err(CatalogError::SlotInUse)
        ));
        assert_eq!(catalog.retire_slot(4), Err(CatalogError::SlotInUse));
        assert_eq!(catalog.by_id(12).unwrap().clip, Some(&"current"));

        let retired = catalog.end_use(&use_token).unwrap().unwrap();
        assert_eq!(retired.slot, 4);
        assert_eq!(retired.clip, Some("current"));
    }

    #[test]
    fn owned_slot_rejects_clip_replacement_without_mutating_existing_state() {
        let mut catalog = AnimationResourceCatalog::default();
        let owner = owner(&catalog, 42);
        let current = catalog
            .publish_for_owner(owner, 12, 4, [5, 6], Some("current"))
            .unwrap();

        assert_eq!(
            catalog.publish(13, 4, [7, 8], Some("replacement")),
            Err(CatalogError::SlotOwned)
        );
        assert!(catalog.view(current).is_some());
        assert!(catalog.by_id(13).is_none());
        assert_eq!(catalog.by_id(12).unwrap().clip, Some(&"current"));

        let observed = catalog.publish(14, 4, [9, 10], None).unwrap();
        assert_eq!(observed, current);
        let view = catalog.by_id(14).unwrap();
        assert_eq!(view.clip, Some(&"current"));
        assert_eq!(view.policy, [9, 10]);

        let retired = catalog.release_owner(owner).unwrap();
        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0].clip, Some("current"));
    }

    #[test]
    fn generation_reset_requires_owner_release_and_preserves_state_on_rejection() {
        let mut catalog = AnimationResourceCatalog::default();
        let owner = owner(&catalog, 42);
        let handle = catalog
            .publish_for_owner(owner, 12, 4, [5, 6], Some("clip"))
            .unwrap();
        let before = catalog.generation();

        assert_eq!(catalog.begin_generation(), Err(CatalogError::OwnersActive));
        assert_eq!(catalog.generation(), before);
        assert_eq!(catalog.view(handle).unwrap().clip, Some(&"clip"));
        assert_eq!(catalog.by_id(12).unwrap().policy, [5, 6]);

        assert_eq!(catalog.release_owner(owner).unwrap()[0].clip, Some("clip"));
        let after = catalog.begin_generation().unwrap();
        assert_ne!(before, after);
        assert!(catalog.view(handle).is_none());
        assert!(catalog.by_id(12).is_none());
    }

    #[test]
    fn owner_tokens_are_generation_scoped_and_stale_tokens_cannot_mutate_new_state() {
        let mut catalog = AnimationResourceCatalog::default();
        let old_owner = owner(&catalog, 42);
        catalog
            .publish_for_owner(old_owner, 12, 4, [5, 6], Some("old"))
            .unwrap();
        assert_eq!(
            catalog.release_owner(old_owner).unwrap()[0].clip,
            Some("old")
        );

        let next_generation = catalog.begin_generation().unwrap();
        assert_eq!(catalog.generation(), next_generation);
        let current_owner = catalog.owner_id(42);
        assert_ne!(old_owner, current_owner);
        let current_handle = catalog
            .publish_for_owner(current_owner, 13, 7, [7, 8], Some("current"))
            .unwrap();
        catalog.publish(14, 8, [9, 10], Some("unowned")).unwrap();
        let order_before = catalog.binding_order;

        assert_eq!(
            catalog.publish_for_owner(old_owner, 15, 9, [11, 12], Some("stale")),
            Err(CatalogError::StaleOwner)
        );
        assert_eq!(
            catalog.attach_owner(old_owner, 8),
            Err(CatalogError::StaleOwner)
        );
        assert!(matches!(
            catalog.release_owner(old_owner),
            Err(CatalogError::StaleOwner)
        ));

        assert_eq!(catalog.binding_order, order_before);
        assert!(catalog.by_id(15).is_none());
        assert!(catalog.by_slot(9).is_none());
        assert!(catalog.owners_by_slot[&7].contains(&current_owner));
        assert_eq!(catalog.view(current_handle).unwrap().clip, Some(&"current"));
        assert!(catalog.owners_by_slot.get(&8).is_none());
    }

    #[test]
    fn owner_tokens_are_bound_to_the_catalog_that_created_them() {
        let source = AnimationResourceCatalog::<&str>::default();
        let owner = source.owner_id(42);
        let mut target = AnimationResourceCatalog::default();
        target.publish(12, 4, [1, 2], Some("target")).unwrap();
        let target_handle = target.handle_for_id(12).unwrap();
        let binding_order = target.binding_order;

        assert_eq!(
            target.attach_owner(owner, 4),
            Err(CatalogError::ForeignOwner)
        );
        assert_eq!(
            target.publish_for_owner(owner, 13, 5, [3, 4], Some("foreign")),
            Err(CatalogError::ForeignOwner)
        );
        assert!(matches!(
            target.release_owner(owner),
            Err(CatalogError::ForeignOwner)
        ));

        assert_eq!(target.binding_order, binding_order);
        assert_eq!(target.view(target_handle).unwrap().clip, Some(&"target"));
        assert!(target.by_id(13).is_none());
        assert!(target.by_slot(5).is_none());
        assert!(target.owners_by_slot.get(&4).is_none());
    }

    #[test]
    fn owner_release_is_atomic_when_any_slot_revision_is_exhausted() {
        let mut catalog = AnimationResourceCatalog::default();
        let owner = owner(&catalog, 42);
        let first = catalog
            .publish_for_owner(owner, 12, 3, [1, 2], Some("first"))
            .unwrap();
        catalog
            .publish_for_owner(owner, 13, 4, [3, 4], Some("second"))
            .unwrap();
        catalog.slots.get_mut(&4).unwrap().revision = u64::MAX;
        catalog.slot_revisions.insert(4, u64::MAX);
        let second = catalog.handle_for_id(13).unwrap();

        assert!(matches!(
            catalog.release_owner(owner),
            Err(CatalogError::SlotRevisionExhausted)
        ));
        assert_eq!(catalog.view(first).unwrap().clip, Some(&"first"));
        assert_eq!(catalog.view(second).unwrap().clip, Some(&"second"));
        assert!(catalog.slots_by_owner[&owner].contains(&3));
        assert!(catalog.slots_by_owner[&owner].contains(&4));
        assert!(catalog.owners_by_slot[&3].contains(&owner));
        assert!(catalog.owners_by_slot[&4].contains(&owner));
    }

    #[test]
    fn owner_release_matches_small_exhaustive_shared_slot_memberships() {
        let release_orders = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];

        for first_mask in 1u8..8 {
            for second_mask in 1u8..8 {
                for order in release_orders {
                    let masks = [first_mask, second_mask];
                    let mut catalog = AnimationResourceCatalog::default();
                    let owners = [owner(&catalog, 1), owner(&catalog, 2), owner(&catalog, 3)];
                    catalog.publish(12, 3, [1, 2], Some("first")).unwrap();
                    catalog.publish(13, 4, [3, 4], Some("second")).unwrap();
                    for (slot_index, slot) in [3, 4].into_iter().enumerate() {
                        for owner_index in 0..owners.len() {
                            if masks[slot_index] & (1 << owner_index) != 0 {
                                catalog.attach_owner(owners[owner_index], slot).unwrap();
                            }
                        }
                    }

                    let mut remaining = masks;
                    for owner_index in order {
                        for slot_index in 0..remaining.len() {
                            remaining[slot_index] &= !(1 << owner_index);
                        }
                        let expected_retired: Vec<i16> = [3, 4]
                            .into_iter()
                            .enumerate()
                            .filter_map(|(slot_index, slot)| {
                                (masks[slot_index] & (1 << owner_index) != 0
                                    && remaining[slot_index] == 0)
                                    .then_some(slot)
                            })
                            .collect();
                        let actual_retired: Vec<i16> = catalog
                            .release_owner(owners[owner_index])
                            .unwrap()
                            .into_iter()
                            .map(|retired| retired.slot)
                            .collect();
                        assert_eq!(actual_retired, expected_retired);

                        for (slot_index, slot) in [3, 4].into_iter().enumerate() {
                            assert_eq!(
                                catalog.by_slot(slot).is_some(),
                                remaining[slot_index] != 0,
                                "masks={masks:?}, order={order:?}, after owner {owner_index}"
                            );
                        }
                    }
                    assert!(catalog.slots_by_owner.is_empty());
                    assert!(catalog.owners_by_slot.is_empty());
                }
            }
        }
    }

    #[test]
    fn failed_owner_bound_publication_leaves_no_slot_or_owner_entry() {
        let mut catalog = AnimationResourceCatalog::default();
        let owner = owner(&catalog, 42);
        catalog.slot_revisions.insert(4, u64::MAX);

        assert!(matches!(
            catalog.publish_for_owner(owner, 12, 4, [5, 6], Some("clip")),
            Err(CatalogError::SlotRevisionExhausted)
        ));
        assert!(catalog.slots.is_empty());
        assert!(catalog.ids.is_empty());
        assert!(catalog.slots_by_owner.is_empty());
        assert!(catalog.owners_by_slot.is_empty());
        assert!(catalog.release_owner(owner).unwrap().is_empty());
    }
}
