// SPDX-License-Identifier: GPL-3.0-or-later
//! Native lifecycle and ordered room membership producer for T2238.
//! Membership is explicit owner state: guest record addresses and row numbers
//! never become native object identities.
use alloc::vec::Vec;

pub use crate::object_pool::ObjectId;
use crate::object_pool::{MembershipAuditError, ObjectPool, OrderedRoomObjects, REC_CAP};

/// Maximum number of object slots observed in the ObjectPool ABI (0x226).
/// This is deliberately independent of `ObjectPool::REC_CAP`: the owned pool
/// mirror now covers the observed 0x226 record capacity, while this tracks the
/// slot namespace and lifecycle generations rather than record availability.
pub const OBJECT_SLOT_CAP: usize = 0x226;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneMember {
    pub object: ObjectId,
    pub model_asset: u64,
    pub model_key: u16,
    /// Ordered room memberships for one draw identity.
    pub rooms: Vec<u16>,
    pub visible: bool,
    pub deferred: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderComponents {
    pub placement: crate::transforms::WorldTransform,
    pub bounds: [[f32; 3]; 2],
    pub motion_bounds: [[f32; 3]; 2],
    pub scissor: [f32; 4],
    pub scissor_rejected: bool,
    pub model_hidden: bool,
    pub overlay: u8,
    pub discontinuity: bool,
}

/// Owner-ordered render membership, revised only by explicit lifecycle calls.
/// The pool provides current object records and per-room insertion order; this
/// table supplies lifetime generations and native model asset identities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneMembership {
    generations: Vec<u64>,
    active: Vec<bool>,
    members: Vec<SceneMember>,
    render: Vec<Option<RenderComponents>>,
    record_indices: Vec<Option<usize>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MembershipError {
    SlotOutOfRange,
    AlreadyActive,
    NotActive,
    DuplicateRoom,
    MissingRoomMember,
    MissingPoolRecord,
    MissingModelAsset,
    InvalidRoom,
    PoolMirrorIncomplete,
    MissingRenderState,
    PoolIdentityMismatch,
}

impl SceneMembership {
    /// Whether this exact generation is the active scene object in its pool slot.
    pub fn is_active(&self, id: ObjectId) -> bool {
        let slot = id.slot as usize;
        slot < self.active.len() && self.active[slot] && self.generations[slot] == id.generation
    }

    pub fn spawn(&mut self, slot: u32) -> Result<ObjectId, MembershipError> {
        let slot = slot as usize;
        if slot >= OBJECT_SLOT_CAP {
            return Err(MembershipError::SlotOutOfRange);
        }
        if slot >= self.generations.len() {
            self.generations.resize(slot + 1, 0);
            self.active.resize(slot + 1, false);
            self.render.resize(slot + 1, None);
            self.record_indices.resize(slot + 1, None);
        }
        if self.active[slot] {
            return Err(MembershipError::AlreadyActive);
        }
        self.generations[slot] = self.generations[slot]
            .checked_add(1)
            .ok_or(MembershipError::SlotOutOfRange)?;
        self.active[slot] = true;
        self.render[slot] = None;
        self.record_indices[slot] = None;
        Ok(ObjectId {
            slot: slot as u32,
            generation: self.generations[slot],
        })
    }

    pub fn retire(&mut self, id: ObjectId) -> Result<(), MembershipError> {
        if !self.is_active(id) {
            return Err(MembershipError::NotActive);
        }
        let slot = id.slot as usize;
        self.active[slot] = false;
        self.render[slot] = None;
        self.record_indices[slot] = None;
        self.members.retain(|member| member.object != id);
        Ok(())
    }

    /// Attach an object to an ordered room list. The first room establishes
    /// draw order; later rooms extend the existing identity's room set.
    pub fn attach(
        &mut self,
        id: ObjectId,
        model_asset: u64,
        model_key: u16,
        room: u16,
        visible: bool,
        deferred: bool,
    ) -> Result<(), MembershipError> {
        let slot = id.slot as usize;
        if slot >= self.active.len()
            || self.generations[slot] != id.generation
            || !self.active[slot]
        {
            return Err(MembershipError::NotActive);
        }
        if model_asset == 0 {
            return Err(MembershipError::MissingModelAsset);
        }
        if room as usize >= crate::object_pool::CELLS {
            return Err(MembershipError::InvalidRoom);
        }
        if let Some(member) = self.members.iter_mut().find(|member| member.object == id) {
            if member.model_asset != model_asset || member.model_key != model_key {
                return Err(MembershipError::PoolIdentityMismatch);
            }
            if member.rooms.contains(&room) {
                return Err(MembershipError::DuplicateRoom);
            }
            member.rooms.push(room);
            member.visible |= visible;
            member.deferred &= deferred;
            return Ok(());
        }
        self.members.push(SceneMember {
            object: id,
            model_asset,
            model_key,
            rooms: alloc::vec![room],
            visible,
            deferred,
        });
        Ok(())
    }

    pub fn ordered(&self) -> &[SceneMember] {
        &self.members
    }

    /// Native lifecycle-to-record binding. The record index is only a join
    /// key into the owned ObjectPool mirror, never a stable object identity.
    pub fn record_index(&self, id: ObjectId) -> Option<usize> {
        let slot = id.slot as usize;
        (slot < self.active.len() && self.active[slot] && self.generations[slot] == id.generation)
            .then(|| self.record_indices.get(slot).copied().flatten())
            .flatten()
    }

    /// Resolve an explicitly bound pool record to its current active identity.
    /// A recycled slot resolves to its new generation; stale IDs are not kept
    /// in the pool-order projection.
    pub fn object_for_record(&self, record_index: usize) -> Option<ObjectId> {
        self.members.iter().find_map(|member| {
            (self.record_index(member.object) == Some(record_index)).then_some(member.object)
        })
    }

    /// Project the ObjectPool's per-room member insertion order onto current
    /// lifecycle identities. ObjectPool owns the order; this owner only joins
    /// each record through an explicit generation-qualified binding.
    pub fn ordered_pool_rooms(
        &self,
        pool: &ObjectPool,
    ) -> Result<Vec<OrderedRoomObjects>, MembershipAuditError> {
        pool.ordered_room_objects(|record_index| self.object_for_record(record_index))
    }

    pub fn set_render_state(
        &mut self,
        id: ObjectId,
        state: RenderComponents,
    ) -> Result<(), MembershipError> {
        let slot = id.slot as usize;
        if slot >= self.active.len()
            || !self.active[slot]
            || self.generations[slot] != id.generation
        {
            return Err(MembershipError::NotActive);
        }
        if !self.members.iter().any(|member| member.object == id) {
            return Err(MembershipError::MissingPoolRecord);
        }
        self.render[slot] = Some(state);
        Ok(())
    }

    /// Bind the lifecycle identity to a record index supplied by the native
    /// object owner. Index equality is never assumed by collection.
    pub fn bind_record(
        &mut self,
        id: ObjectId,
        record_index: usize,
    ) -> Result<(), MembershipError> {
        let slot = id.slot as usize;
        if slot >= self.active.len()
            || !self.active[slot]
            || self.generations[slot] != id.generation
        {
            return Err(MembershipError::NotActive);
        }
        if self.record_indices[slot].is_some() {
            return Err(MembershipError::PoolIdentityMismatch);
        }
        self.record_indices[slot] = Some(record_index);
        Ok(())
    }

    pub fn render_state(&self, id: ObjectId) -> Option<RenderComponents> {
        let slot = id.slot as usize;
        (slot < self.active.len() && self.active[slot] && self.generations[slot] == id.generation)
            .then(|| self.render.get(slot).copied().flatten())
            .flatten()
    }

    /// Return one complete ordered copy only when the pool mirror contains
    /// every used row. Partial 64-record mirrors must never silently publish
    /// a truncated scene as if it were the whole scene.
    pub fn collect_complete(&self, pool: &ObjectPool) -> Result<Vec<SceneMember>, MembershipError> {
        let used = pool.owned_records();
        if used > REC_CAP || pool.scalars[crate::object_pool::S_USED_RECORDS] as usize > REC_CAP {
            return Err(MembershipError::PoolMirrorIncomplete);
        }
        self.validate_pool(pool)?;
        Ok(self.members.clone())
    }

    /// Validate membership against the current typed pool before scene use.
    /// No guest pointer is converted into an ObjectId or ModelId.
    pub fn validate_pool(&self, pool: &ObjectPool) -> Result<(), MembershipError> {
        for member in &self.members {
            let record_index = self
                .record_indices
                .get(member.object.slot as usize)
                .copied()
                .flatten()
                .filter(|record_index| pool.record_address(*record_index).is_some());
            let Some(record_index) = record_index else {
                return Err(MembershipError::MissingPoolRecord);
            };
            if member.rooms.is_empty()
                || member
                    .rooms
                    .iter()
                    .enumerate()
                    .any(|(index, room)| member.rooms[..index].contains(room))
                || member
                    .rooms
                    .iter()
                    .any(|room| *room as usize >= crate::object_pool::CELLS)
            {
                return Err(MembershipError::InvalidRoom);
            }
            for room in &member.rooms {
                let present = (0..pool.count(*room as usize) as usize).any(|ordinal| {
                    pool.member_record_index(*room as usize, ordinal)
                        .is_some_and(|index| {
                            pool.member_at(*room as usize, ordinal) == pool.record_address(index)
                                && index == record_index
                        })
                });
                if !present {
                    return Err(MembershipError::MissingRoomMember);
                }
            }
            let model = pool
                .record_model_key(record_index)
                .ok_or(MembershipError::MissingPoolRecord)?;
            if model == 0 || model != member.model_key {
                return Err(MembershipError::MissingModelAsset);
            }
            if self.render_state(member.object).is_none() {
                return Err(MembershipError::MissingRenderState);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_slot_reuse_gets_new_generation_and_detaches_members() {
        let mut owner = SceneMembership::default();
        let first = owner.spawn(4).unwrap();
        owner.attach(first, 19, 19, 2, true, false).unwrap();
        owner.retire(first).unwrap();
        let second = owner.spawn(4).unwrap();
        assert_eq!(second.slot, first.slot);
        assert!(second.generation > first.generation);
        assert!(owner.attach(first, 19, 19, 2, true, false).is_err());
        owner.attach(second, 19, 19, 2, true, false).unwrap();
        assert_eq!(owner.ordered().len(), 1);
        assert_eq!(owner.ordered()[0].object, second);
    }

    #[test]
    fn pool_room_order_joins_explicit_ids_and_slot_reuse_changes_generation() {
        use crate::object_pool::{
            CAPS, COUNTS, MEMBERS, S_ARENA_BASE, S_ARENA_USED, S_CAPACITY, S_RECORDS_BASE,
            S_USED_RECORDS,
        };

        let mut pool = ObjectPool::empty_limited(2);
        pool.scalars[S_RECORDS_BASE] = 0x10000;
        pool.scalars[S_CAPACITY] = 2;
        pool.scalars[S_USED_RECORDS] = 2;
        pool.scalars[S_ARENA_BASE] = 0x20000;
        pool.scalars[S_ARENA_USED] = 2;
        pool.tables[COUNTS][3] = 2;
        pool.tables[CAPS][3] = 2;
        pool.tables[MEMBERS][3] = 0x20000;
        pool.arena_len = 2;
        pool.arena[0] = crate::object_pool::rec_addr(0x10000, 1, 0);
        pool.arena[1] = crate::object_pool::rec_addr(0x10000, 0, 0);

        let mut owner = SceneMembership::default();
        let first = owner.spawn(4).unwrap();
        let second = owner.spawn(8).unwrap();
        owner.attach(first, 10, 10, 3, true, false).unwrap();
        owner.attach(second, 11, 11, 3, true, false).unwrap();
        owner.bind_record(first, 1).unwrap();
        owner.bind_record(second, 0).unwrap();

        let ordered = owner.ordered_pool_rooms(&pool).unwrap();
        assert_eq!(ordered[0].room, 3);
        assert_eq!(
            ordered[0]
                .objects
                .iter()
                .map(|entry| (entry.object, entry.record_index))
                .collect::<Vec<_>>(),
            [(first, 1), (second, 0)]
        );

        owner.retire(first).unwrap();
        assert_eq!(
            owner.ordered_pool_rooms(&pool),
            Err(
                crate::object_pool::MembershipAuditError::MissingObjectIdentity {
                    room: 3,
                    record: 1,
                }
            )
        );
        let recycled = owner.spawn(first.slot).unwrap();
        assert!(recycled.generation > first.generation);
        owner.attach(recycled, 10, 10, 3, true, false).unwrap();
        owner.bind_record(recycled, 1).unwrap();
        let ordered = owner.ordered_pool_rooms(&pool).unwrap();
        assert_eq!(ordered[0].objects[0].object, recycled);
        assert_eq!(owner.object_for_record(1), Some(recycled));
    }

    #[test]
    fn room_order_is_insertion_order_and_duplicates_reject() {
        let mut owner = SceneMembership::default();
        let a = owner.spawn(2).unwrap();
        let b = owner.spawn(1).unwrap();
        owner.attach(b, 8, 8, 3, true, false).unwrap();
        owner.attach(a, 7, 7, 3, false, true).unwrap();
        assert_eq!(owner.ordered()[0].object, b);
        assert_eq!(owner.ordered()[1].object, a);
        owner.attach(a, 7, 7, 4, false, true).unwrap();
        assert_eq!(owner.ordered().len(), 2);
        assert_eq!(owner.ordered()[1].rooms, alloc::vec![3, 4]);
        assert_eq!(
            owner.attach(a, 7, 7, 3, false, true),
            Err(MembershipError::DuplicateRoom)
        );
    }

    #[test]
    fn slot_namespace_covers_pool_capacity_and_rejects_out_of_range() {
        let mut owner = SceneMembership::default();
        let id = owner.spawn((OBJECT_SLOT_CAP - 1) as u32).unwrap();
        assert_eq!(id.slot as usize, OBJECT_SLOT_CAP - 1);
        assert_eq!(
            owner.spawn(OBJECT_SLOT_CAP as u32),
            Err(MembershipError::SlotOutOfRange)
        );
    }

    #[test]
    fn complete_collection_rejects_pool_prefix_truncation() {
        let owner = SceneMembership::default();
        let mut pool = ObjectPool::EMPTY;
        pool.scalars[crate::object_pool::S_RECORDS_BASE] = 0x1000;
        pool.scalars[crate::object_pool::S_USED_RECORDS] = (REC_CAP + 1) as u32;
        assert_eq!(
            owner.collect_complete(&mut pool),
            Err(MembershipError::PoolMirrorIncomplete)
        );
    }
}
