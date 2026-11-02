// SPDX-License-Identifier: GPL-3.0-or-later
//! T2078 (S3-10 of T2051): Rust-owned object pool header, per-cell tables, update list and records.
//! Design and honest limits: docs/t-s3-object-pool.md. Stage is Own with write-through, as in
//! player.rs and character.rs: the owned words are authoritative for the declared extents and every
//! setter stores the changed bytes into the guest image in the same call, so lifted readers still see
//! the original layout. Names are INFERRED from the lifted code (engine/object), addresses MEASURED.
//!
//! Layout (from the lifted pool code 0xA5C00 alloc, 0xA5AF0 reset, 0x9E430 add, 0x9E4B0 rebuild, 0x9FBA0 push):
//! * header `0x7B54C0..0x7B54E0` (8 words); word 6 (`0x7B54D8`) is the update list length.
//! * five tables of `CELLS` words at stride `0x360` from `0x7B54E0`: member counts, planned counts,
//!   capacities, member pointer arrays (into the arena at `[0x715FB4]`), and a fifth that reset clears.
//!   The lifted code clears and fills `0xD6` cells; `CELLS` is 0xD8 because the CSV shows writes into the
//!   2 gap words of each table.
//! * records: `[0x72F5E8]` points to `[0x72F5E4]` records of `RECORD_SIZE` (0x2E8) bytes, `[0x7B4FAC]` used.
//! * update list: singly linked, head `[0x7B4CC0]`, tail `[0x7B4CB8]`, next pointer at node `+0`.
use super::GuestMemory;
use super::generated_views::{TsfpObjectOwned, TsfpObjectView};
use alloc::vec;
use alloc::vec::Vec;

/// Native lifecycle identity for an object slot. Record indexes and guest
/// addresses are explicit bindings only; neither supplies the generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectId {
    pub slot: u32,
    pub generation: u64,
}

pub const HEADER: u32 = 0x007b_54c0;
pub const HEADER_WORDS: usize = 8;
pub const TABLES_BASE: u32 = 0x007b_54e0;
pub const TABLE_STRIDE: u32 = 0x360;
pub const CELLS: usize = 0xd8;
pub const TABLE_COUNT: usize = 5;
pub const LIST_LEN_INDEX: usize = 6;
pub const RECORD_SIZE: u32 = 0x2e8;
pub const RECORD_WORDS: usize = (RECORD_SIZE / 4) as usize;
/// Raw Object+0x28 field, whose reactivation path ORs `0x61` (T1661).
pub const RECORD_OBJECT_FLAGS_OFFSET: u32 = 0x28;
/// Object+0x28 bit set by the measured A7090 retirement path (T1661).
pub const OBJECT_RETIRED_FLAG: u32 = 0x8000_0000;
/// Unsigned object model id at Object+0x94 (T1743, generated view).
pub const RECORD_MODEL_ID_OFFSET: u32 = TsfpObjectView::OFFSET_MODEL_ID;
/// Common Object+0x28 mask applied by the T1743-measured initializer.
pub const INITIALIZER_OBJECT_FLAGS: u32 = 0x50;
/// Additional Object+0x28 mask for model key 0x553 in the initializer.
pub const MODEL_0553_INITIALIZER_OBJECT_FLAGS: u32 = 0x4002_0000;
/// Object+0x7C contains a pointer to a type-dependent extension. Its pointee is
/// intentionally kept as an opaque ABI address here.
pub const RECORD_EXTENSION_ADDRESS_OFFSET: u32 = 0x7c;
/// Object+0x64/+0x68/+0x6C float position components (T1743).
pub const RECORD_POSITION_OFFSET: u32 = 0x64;
/// Object+0x98/+0x9C/+0xA0 copied into the initializer extension before the
/// measured model-specific position adjustment (T1743).
pub const RECORD_ORIGINAL_POSITION_OFFSETS: [u32; 3] = [0x98, 0x9c, 0xa0];
pub const RECORD_ROTATION_INPUT_OFFSETS: [u32; 3] = [0x70, 0xc0, 0xc8];
pub const RECORD_ACTOR_PARAMETER_OFFSETS: [u32; 2] = [0x1cc, 0x1d8];
/// Maximum record capacity observed from the lifted pool allocator (0xA5C00).
/// This is an ABI-derived bound, not a claim that every level uses that many.
pub const REC_CAP: usize = 0x226;
/// Longest update list chain mirrored.
pub const CHAIN_CAP: usize = 64;
/// Member arena limit in dwords checked by the lifted add function (0x9E430).
pub const ARENA_LIMIT: u32 = 0x14cd;
/// Maximum number of dwords addressable in the pool member arena. The lifted
/// allocator rejects a growth when `arena_used + capacity >= ARENA_LIMIT`.
pub const ARENA_CAP: usize = ARENA_LIMIT as usize;

/// Snapshot of the pool's eight header words. Only word 6 is currently
/// identified as the update-list length; the other words stay opaque ABI data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolHeaderState {
    pub words: [u32; HEADER_WORDS],
}

/// Table indexes.
pub const COUNTS: usize = 0;
pub const PLANNED: usize = 1;
pub const CAPS: usize = 2;
pub const MEMBERS: usize = 3;
pub const AUX: usize = 4;

/// Typed view of the parallel pool tables for one cell. The names are inferred
/// from the lifted add/rebuild writers; `members_ptr` remains a guest ABI
/// address and does not identify an object. `auxiliary` preserves the raw fifth
/// table word without assigning it a role.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolCellState {
    pub count: u32,
    pub planned: u32,
    pub capacity: u32,
    pub members_ptr: u32,
    pub auxiliary: u32,
}

/// Raw member words for one pool cell in its existing insertion order.
/// Entries remain guest ABI values; this snapshot does not assign identity,
/// room ownership, or lifetime semantics to them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PoolCellMembersState {
    pub words: Vec<u32>,
}

/// Snapshot of the pool's scalar ABI words. Field labels are inferred from
/// lifted allocator paths; pointer-like values remain raw guest words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolScalarState {
    pub arena_used: u32,
    pub arena_base: u32,
    pub capacity: u32,
    pub records_base: u32,
    pub aux_base: u32,
    pub pool_flag: u32,
    pub bitmap_base: u32,
    pub chunk_index: u32,
    pub used_records: u32,
    pub first_record: u32,
    pub list_head: u32,
    pub list_tail: u32,
}

/// Snapshot of the update-list endpoints and stored length. Endpoint words
/// remain guest addresses; this view does not confer node or object identity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolUpdateListState {
    pub head: u32,
    pub tail: u32,
    pub len: u32,
}

/// Snapshot of one complete object-pool record. The dwords remain raw ABI
/// state, including words not yet named by the generated object view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolRecordState {
    pub words: [u32; RECORD_WORDS],
}

/// Raw guest address stored at Object+0x7C. REA distinguishes the keyed
/// 0x118-byte A buffer from the larger type-dependent C extension, so this
/// address does not imply a CharacterSet slot, object identity, or lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolRecordExtensionAddress(pub u32);

/// Measured Object position components. Coordinate frame and units remain
/// unresolved; these values do not establish object identity or lifetime.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolRecordPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Original Object position copied by the actor initializer before the
/// model-specific adjustment. Coordinate frame and units remain unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolRecordOriginalPosition {
    pub components: [f32; 3],
}

/// The two measured position triples written by the actor initializer. Keep
/// the current Object position separate from the original-position snapshot;
/// callers provide both without assuming a coordinate frame or unit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolRecordInitializerPosition {
    pub current: PoolRecordPosition,
    pub original: PoolRecordOriginalPosition,
}

/// Object-side values written by the measured model `0x3B3`, `0x482`, or
/// `0x553` actor initializer. The raw flag meanings and position units remain
/// unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolRecordInitializerObjectFields {
    pub object_flags: PoolRecordObjectFlags,
    pub position: PoolRecordInitializerPosition,
}

/// Raw Object rotation inputs in measured offset order (+0x70, +0xC0, +0xC8).
/// Axis labels and units remain unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolRecordRotationInputs {
    pub components: [f32; 3],
}

/// Float values at measured Object offsets +0x1CC and +0x1D8. Their roles
/// remain unresolved; the names retain the addresses without inferring a use.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolRecordActorParameters {
    pub at_1cc: f32,
    pub at_1d8: f32,
}

/// Raw Object+0x28 bits. Individual meanings remain unresolved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolRecordObjectFlags(pub u32);

/// Caller-supplied bundle of separately measured Object fields. This groups
/// storage values for typed updates; it does not assert that one native call
/// writes them together or assign meaning to flags, rotations, or parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PoolRecordMeasuredFields {
    pub object_flags: PoolRecordObjectFlags,
    pub position: PoolRecordInitializerPosition,
    pub rotation_inputs: PoolRecordRotationInputs,
    pub actor_parameters: PoolRecordActorParameters,
}

/// Object+0x28 bits applied by T1661's measured-static character reactivation path.
pub const CHARACTER_REACTIVATION_OBJECT_FLAGS: u32 = 0x61;

impl PoolScalarState {
    fn from_words(words: [u32; SCALARS.len()]) -> Self {
        Self {
            arena_used: words[0],
            arena_base: words[1],
            capacity: words[2],
            records_base: words[3],
            aux_base: words[4],
            pool_flag: words[5],
            bitmap_base: words[6],
            chunk_index: words[7],
            used_records: words[8],
            first_record: words[9],
            list_head: words[10],
            list_tail: words[11],
        }
    }

    fn into_words(self) -> [u32; SCALARS.len()] {
        [
            self.arena_used,
            self.arena_base,
            self.capacity,
            self.records_base,
            self.aux_base,
            self.pool_flag,
            self.bitmap_base,
            self.chunk_index,
            self.used_records,
            self.first_record,
            self.list_head,
            self.list_tail,
        ]
    }
}

/// Scalar globals owned next to the pool: (guest address, INFERRED name).
pub const SCALARS: [(u32, &str); 12] = [
    (0x0072_f298, "arena_used"),
    (0x0071_5fb4, "arena_base"),
    (0x0072_f5e4, "capacity"),
    (0x0072_f5e8, "records_base"),
    (0x0072_f5ec, "aux_base"),
    (0x0072_f5f0, "pool_flag"),
    (0x0072_f5f4, "bitmap_base"),
    (0x0072_f5f8, "chunk_index"),
    (0x007b_4fac, "used_records"),
    (0x007b_975c, "first_record"),
    (0x007b_4cc0, "list_head"),
    (0x007b_4cb8, "list_tail"),
];
pub const S_ARENA_USED: usize = 0;
pub const S_ARENA_BASE: usize = 1;
pub const S_CAPACITY: usize = 2;
pub const S_RECORDS_BASE: usize = 3;
pub const S_USED_RECORDS: usize = 8;
pub const S_LIST_HEAD: usize = 10;
pub const S_LIST_TAIL: usize = 11;

/// First owned location that differs between the owned value and the guest image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolDiff {
    Header(usize),
    Cell(usize, usize),
    Scalar(usize),
    /// Update list node (chain position) whose next pointer differs.
    Chain(usize),
    /// Record index and byte offset of the differing dword.
    Record(usize, u32),
}

/// Byte window over one owned record so the generated view code runs on the owned words.
pub struct RecordMem<'a> {
    pub base: u32,
    pub words: &'a mut [u32; RECORD_WORDS],
}
impl GuestMemory for RecordMem<'_> {
    fn read8(&self, a: u32) -> u8 {
        let o = a.wrapping_sub(self.base);
        if o < RECORD_SIZE {
            self.words[(o / 4) as usize].to_le_bytes()[(o % 4) as usize]
        } else {
            0
        }
    }
    fn write8(&mut self, a: u32, v: u8) {
        let o = a.wrapping_sub(self.base);
        if o < RECORD_SIZE {
            let w = &mut self.words[(o / 4) as usize];
            let mut b = w.to_le_bytes();
            b[(o % 4) as usize] = v;
            *w = u32::from_le_bytes(b);
        }
    }
}

/// Guest address of table cell.
pub const fn cell_addr(table: usize, cell: usize) -> u32 {
    TABLES_BASE + TABLE_STRIDE * table as u32 + 4 * cell as u32
}

/// Guest address of record `index` + `offset` in a pool at `base`.
pub const fn rec_addr(base: u32, index: usize, offset: u32) -> u32 {
    base.wrapping_add(index as u32 * RECORD_SIZE)
        .wrapping_add(offset)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectPool {
    pub header: [u32; HEADER_WORDS],
    pub tables: [[u32; CELLS]; TABLE_COUNT],
    pub scalars: [u32; SCALARS.len()],
    /// Typed mirror of the member-array arena. `arena_len` is the observed used
    /// prefix; pointers in the MEMBERS table remain guest addresses for ABI
    /// compatibility, while `member_at` resolves them into this owned storage.
    pub arena: [u32; ARENA_CAP],
    pub arena_len: usize,
    /// Update list as walked from the head (node pointers, zero filled after the end).
    pub chain: [u32; CHAIN_CAP],
    /// Next pointer of every chain node at the time of the load.
    pub chain_next: [u32; CHAIN_CAP],
    pub records: Vec<[u32; RECORD_WORDS]>,
    /// Records owned at most (`REC_CAP` by default, a replay with fewer captured records lowers it).
    pub record_limit: usize,
}

/// A validated observation of the pool's ordered room membership. This is a
/// diagnostic/source boundary only: it does not mint lifecycle identities or
/// claim that pool order equals final renderer submission order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MembershipAudit {
    pub rooms: Vec<RoomMembershipAudit>,
    pub total_members: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomMembershipAudit {
    pub room: u16,
    /// Record indexes in the exact insertion order stored by the pool.
    pub records: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MembershipAuditError {
    IncompleteRecords,
    InvalidCellCount { room: usize, count: usize },
    MissingArenaMember { room: usize, ordinal: usize },
    InvalidRecordReference { room: usize, ordinal: usize },
    DuplicateRecord { room: usize, record: usize },
    MissingObjectIdentity { room: usize, record: usize },
}

/// A record's validated position in one authoritative pool room sequence.
/// Cross-room draw order remains the lifecycle owner's responsibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderedObject {
    pub object: ObjectId,
    pub record_index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderedRoomObjects {
    pub room: u16,
    pub objects: Vec<OrderedObject>,
}

impl ObjectPool {
    pub fn empty_limited(limit: usize) -> Self {
        Self {
            header: [0; HEADER_WORDS],
            tables: [[0; CELLS]; TABLE_COUNT],
            scalars: [0; SCALARS.len()],
            arena: [0; ARENA_CAP],
            arena_len: 0,
            chain: [0; CHAIN_CAP],
            chain_next: [0; CHAIN_CAP],
            records: vec![[0; RECORD_WORDS]; limit.min(REC_CAP)],
            record_limit: limit.min(REC_CAP),
        }
    }

    pub const EMPTY: Self = Self {
        header: [0; HEADER_WORDS],
        tables: [[0; CELLS]; TABLE_COUNT],
        scalars: [0; SCALARS.len()],
        arena: [0; ARENA_CAP],
        arena_len: 0,
        chain: [0; CHAIN_CAP],
        chain_next: [0; CHAIN_CAP],
        records: Vec::new(),
        record_limit: REC_CAP,
    };

    /// Mirror: copies header, tables, scalars, chain and the used prefix of the records.
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self::load_limited(m, REC_CAP)
    }

    /// Like `load`, owning at most `limit` records (replays that captured fewer records).
    pub fn load_limited<M: GuestMemory>(m: &M, limit: usize) -> Self {
        let mut s = Self::EMPTY;
        s.record_limit = limit.min(REC_CAP);
        for i in 0..HEADER_WORDS {
            s.header[i] = m.read32(HEADER + 4 * i as u32);
        }
        for t in 0..TABLE_COUNT {
            for c in 0..CELLS {
                s.tables[t][c] = m.read32(cell_addr(t, c));
            }
        }
        for (i, (a, _)) in SCALARS.iter().enumerate() {
            s.scalars[i] = m.read32(*a);
        }
        s.load_arena(m);
        s.load_chain(m);
        s.records.resize(s.record_limit, [0; RECORD_WORDS]);
        s.load_records(m);
        s
    }

    /// Atomically refresh this owned mirror from the post-tick guest image.
    /// The current value is replaced only after the complete bounded mirror
    /// has been read. This does not assign scene lifecycle identities; callers
    /// must join the refreshed pool to their explicit owner state.
    pub fn refresh_from_guest<M: GuestMemory>(&mut self, m: &M) {
        let replacement = Self::load_limited(m, self.record_limit);
        *self = replacement;
    }

    /// Re-reads the chain from the owned head (a hybrid tick after a lifted writer).
    pub fn load_chain<M: GuestMemory>(&mut self, m: &M) {
        self.chain = [0; CHAIN_CAP];
        self.chain_next = [0; CHAIN_CAP];
        let mut node = self.scalars[S_LIST_HEAD];
        for i in 0..CHAIN_CAP {
            if node == 0 {
                break;
            }
            self.chain[i] = node;
            self.chain_next[i] = m.read32(node);
            node = self.chain_next[i];
        }
    }

    pub fn owned_records(&self) -> usize {
        if self.scalars[S_RECORDS_BASE] == 0 {
            return 0;
        }
        (self.scalars[S_USED_RECORDS] as usize)
            .min(self.scalars[S_CAPACITY] as usize)
            .min(self.record_limit)
            .min(self.records.len())
    }

    /// Read-only complete record row from this owned mirror. `None` means the
    /// index is outside the observed allocated prefix; callers must not infer
    /// an object identity from the row number.
    pub fn owned_record(&self, index: usize) -> Option<&[u32; RECORD_WORDS]> {
        (index < self.owned_records()).then(|| &self.records[index])
    }

    /// Resolve an object-pool member reference to an owned record slot. The
    /// member ABI stores guest record addresses, so resolution is accepted only
    /// for exact aligned addresses in the owned record prefix.
    pub fn member_record_index(&self, cell: usize, ordinal: usize) -> Option<usize> {
        let address = self.member_at(cell, ordinal)?;
        let base = self.scalars[S_RECORDS_BASE];
        let offset = address.checked_sub(base)?;
        if offset % RECORD_SIZE != 0 {
            return None;
        }
        let index = (offset / RECORD_SIZE) as usize;
        (index < self.owned_records()).then_some(index)
    }

    /// Validate and copy every non-empty room's ordered membership from the
    /// owned arena. The result intentionally remains record indexes; a scene
    /// owner must join these rows to explicit lifecycle IDs and typed render
    /// components before publishing a SceneSnapshot.
    pub fn audit_membership(&self) -> Result<MembershipAudit, MembershipAuditError> {
        if self.scalars[S_USED_RECORDS] as usize > self.owned_records() {
            return Err(MembershipAuditError::IncompleteRecords);
        }
        let mut rooms = Vec::new();
        let mut total_members = 0usize;
        for room in 0..CELLS {
            let count = self.count(room) as usize;
            if count == 0 {
                continue;
            }
            if count > self.cap(room) as usize {
                return Err(MembershipAuditError::InvalidCellCount { room, count });
            }
            let mut records = Vec::with_capacity(count);
            for ordinal in 0..count {
                let Some(address) = self.member_at(room, ordinal) else {
                    return Err(MembershipAuditError::MissingArenaMember { room, ordinal });
                };
                let Some(index) = self.member_record_index(room, ordinal) else {
                    return Err(MembershipAuditError::InvalidRecordReference { room, ordinal });
                };
                if self.record_address(index) != Some(address) {
                    return Err(MembershipAuditError::InvalidRecordReference { room, ordinal });
                }
                if records.contains(&index) {
                    return Err(MembershipAuditError::DuplicateRecord {
                        room,
                        record: index,
                    });
                }
                records.push(index);
            }
            total_members = total_members.saturating_add(records.len());
            rooms.push(RoomMembershipAudit {
                room: room as u16,
                records,
            });
        }
        Ok(MembershipAudit {
            rooms,
            total_members,
        })
    }

    /// Join the pool's authoritative per-room insertion order to explicitly
    /// supplied lifecycle IDs. The resolver must reject retired/stale IDs;
    /// this method never derives identity from a record index or guest address.
    pub fn ordered_room_objects(
        &self,
        mut identity_for_record: impl FnMut(usize) -> Option<ObjectId>,
    ) -> Result<Vec<OrderedRoomObjects>, MembershipAuditError> {
        let audit = self.audit_membership()?;
        audit
            .rooms
            .into_iter()
            .map(|room| {
                let objects = room
                    .records
                    .into_iter()
                    .map(|record_index| {
                        identity_for_record(record_index)
                            .map(|object| OrderedObject {
                                object,
                                record_index,
                            })
                            .ok_or(MembershipAuditError::MissingObjectIdentity {
                                room: room.room as usize,
                                record: record_index,
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(OrderedRoomObjects {
                    room: room.room,
                    objects,
                })
            })
            .collect()
    }

    /// Guest address for an owned record index, used only to compare the pool's
    /// ordered member ABI with its record arena. It is never a native identity.
    pub fn record_address(&self, index: usize) -> Option<u32> {
        (index < self.owned_records()).then(|| rec_addr(self.scalars[S_RECORDS_BASE], index, 0))
    }

    /// Model key from the generated typed view of an owned record.
    pub fn record_model_key(&self, index: usize) -> Option<u16> {
        let words = self.owned_record(index)?;
        let base = rec_addr(self.scalars[S_RECORDS_BASE], index, 0);
        let mut copy = *words;
        Some(
            TsfpObjectView(base)
                .load(&mut RecordMem {
                    base,
                    words: &mut copy,
                })
                .model_id,
        )
    }

    /// Reads the opaque extension pointer word from an owned record. The raw
    /// address is not joined to CharacterSet or treated as a native identity.
    pub fn record_extension_address(&self, index: usize) -> Option<PoolRecordExtensionAddress> {
        let words = self.owned_record(index)?;
        Some(PoolRecordExtensionAddress(
            words[(RECORD_EXTENSION_ADDRESS_OFFSET / 4) as usize],
        ))
    }

    /// Re-reads the owned record prefix at the owned base.
    pub fn load_records<M: GuestMemory>(&mut self, m: &M) {
        let base = self.scalars[S_RECORDS_BASE];
        for i in 0..self.owned_records() {
            for j in 0..RECORD_WORDS {
                self.records[i][j] = m.read32(rec_addr(base, i, j as u32 * 4));
            }
        }
    }

    /// Re-reads the arena prefix described by the pool's owned base and used
    /// count. Out-of-range guest metadata is clamped to the lifted allocator's
    /// declared limit; it is never used as an unchecked host slice length.
    pub fn load_arena<M: GuestMemory>(&mut self, m: &M) {
        self.arena_len = (self.scalars[S_ARENA_USED] as usize).min(ARENA_CAP);
        let base = self.scalars[S_ARENA_BASE];
        for i in 0..self.arena_len {
            self.arena[i] = m.read32(base.wrapping_add((i as u32).wrapping_mul(4)));
        }
    }

    /// Membership lookup preserving each cell's insertion order. `None`
    /// means the pointer/count is outside the owned arena prefix.
    pub fn member_at(&self, cell: usize, ordinal: usize) -> Option<u32> {
        if cell >= CELLS {
            return None;
        }
        let ptr = self.members_ptr(cell);
        if ptr == 0 || ordinal >= self.count(cell) as usize {
            return None;
        }
        let base = self.scalars[S_ARENA_BASE];
        let offset = ptr.checked_sub(base)?;
        if offset & 3 != 0 {
            return None;
        }
        let index = (offset / 4) as usize + ordinal;
        (index < self.arena_len).then(|| self.arena[index])
    }

    /// Reads the complete counted member prefix for one cell from Rust-owned
    /// arena state. Invalid, unaligned, or out-of-prefix table ranges return
    /// None rather than exposing a partial list.
    pub fn cell_members_state(&self, cell: usize) -> Option<PoolCellMembersState> {
        let count = *self.tables[COUNTS].get(cell)? as usize;
        if count == 0 {
            return Some(PoolCellMembersState::default());
        }
        let ptr = *self.tables[MEMBERS].get(cell)?;
        if ptr == 0 {
            return None;
        }
        let offset = ptr.checked_sub(self.scalars[S_ARENA_BASE])?;
        if offset & 3 != 0 {
            return None;
        }
        let start = (offset / 4) as usize;
        let end = start.checked_add(count)?;
        (end <= self.arena_len).then(|| PoolCellMembersState {
            words: self.arena[start..end].to_vec(),
        })
    }

    /// Write one arena member through the typed owner and guest image.
    pub fn set_member<M: GuestMemory>(
        &mut self,
        m: &mut M,
        cell: usize,
        ordinal: usize,
        object: u32,
    ) -> bool {
        if cell >= CELLS {
            return false;
        }
        let ptr = self.members_ptr(cell);
        if ptr == 0 || ordinal >= self.count(cell) as usize {
            return false;
        }
        let base = self.scalars[S_ARENA_BASE];
        let Some(offset) = ptr.checked_sub(base) else {
            return false;
        };
        if offset & 3 != 0 {
            return false;
        }
        let index = (offset / 4) as usize + ordinal;
        if index >= self.arena_len {
            return false;
        }
        self.arena[index] = object;
        m.write32(ptr.wrapping_add((ordinal as u32).wrapping_mul(4)), object);
        true
    }

    /// Replaces a cell's complete existing member prefix through the owner
    /// and guest image. The list length, pointer, and pool metadata are fixed;
    /// no allocation, identity, or lifecycle policy is changed.
    pub fn set_cell_members_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        cell: usize,
        state: PoolCellMembersState,
    ) -> bool {
        let Some(current) = self.cell_members_state(cell) else {
            return false;
        };
        if current.words.len() != state.words.len() {
            return false;
        }
        for (ordinal, object) in state.words.into_iter().enumerate() {
            if !self.set_member(m, cell, ordinal, object) {
                return false;
            }
        }
        true
    }

    /// Writes everything owned back; no other guest byte is touched.
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for i in 0..HEADER_WORDS {
            m.write32(HEADER + 4 * i as u32, self.header[i]);
        }
        for t in 0..TABLE_COUNT {
            for c in 0..CELLS {
                m.write32(cell_addr(t, c), self.tables[t][c]);
            }
        }
        for (i, (a, _)) in SCALARS.iter().enumerate() {
            m.write32(*a, self.scalars[i]);
        }
        let arena_base = self.scalars[S_ARENA_BASE];
        for i in 0..self.arena_len.min(ARENA_CAP) {
            m.write32(
                arena_base.wrapping_add((i as u32).wrapping_mul(4)),
                self.arena[i],
            );
        }
        for i in 0..CHAIN_CAP {
            if self.chain[i] != 0 {
                m.write32(self.chain[i], self.chain_next[i]);
            }
        }
        let base = self.scalars[S_RECORDS_BASE];
        for i in 0..self.owned_records() {
            for j in 0..RECORD_WORDS {
                m.write32(rec_addr(base, i, j as u32 * 4), self.records[i][j]);
            }
        }
    }

    /// First owned location whose guest bytes differ.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<PoolDiff> {
        for i in 0..HEADER_WORDS {
            if m.read32(HEADER + 4 * i as u32) != self.header[i] {
                return Some(PoolDiff::Header(i));
            }
        }
        for t in 0..TABLE_COUNT {
            for c in 0..CELLS {
                if m.read32(cell_addr(t, c)) != self.tables[t][c] {
                    return Some(PoolDiff::Cell(t, c));
                }
            }
        }
        for (i, (a, _)) in SCALARS.iter().enumerate() {
            if m.read32(*a) != self.scalars[i] {
                return Some(PoolDiff::Scalar(i));
            }
        }
        for i in 0..CHAIN_CAP {
            if self.chain[i] != 0 && m.read32(self.chain[i]) != self.chain_next[i] {
                return Some(PoolDiff::Chain(i));
            }
        }
        let base = self.scalars[S_RECORDS_BASE];
        for i in 0..self.owned_records() {
            for j in 0..RECORD_WORDS {
                if m.read32(rec_addr(base, i, j as u32 * 4)) != self.records[i][j] {
                    return Some(PoolDiff::Record(i, j as u32 * 4));
                }
            }
        }
        None
    }

    pub fn count(&self, cell: usize) -> u32 {
        self.tables[COUNTS][cell]
    }
    pub fn cap(&self, cell: usize) -> u32 {
        self.tables[CAPS][cell]
    }
    pub fn members_ptr(&self, cell: usize) -> u32 {
        self.tables[MEMBERS][cell]
    }
    /// Reads one cell's related table words without inventing count/capacity
    /// constraints or interpreting the member pointer as native identity.
    pub fn cell_state(&self, cell: usize) -> Option<PoolCellState> {
        (cell < CELLS).then(|| PoolCellState {
            count: self.tables[COUNTS][cell],
            planned: self.tables[PLANNED][cell],
            capacity: self.tables[CAPS][cell],
            members_ptr: self.tables[MEMBERS][cell],
            auxiliary: self.tables[AUX][cell],
        })
    }
    pub fn list_len(&self) -> u32 {
        self.header[LIST_LEN_INDEX]
    }
    /// Reads the measured update-list endpoints and header length from owned
    /// state, preserving endpoint values as raw guest addresses.
    pub fn update_list_state(&self) -> PoolUpdateListState {
        PoolUpdateListState {
            head: self.scalars[S_LIST_HEAD],
            tail: self.scalars[S_LIST_TAIL],
            len: self.header[LIST_LEN_INDEX],
        }
    }
    /// Reads the full header without assigning meaning to its unclassified
    /// words. Word 6 remains the measured update-list length.
    pub fn header_state(&self) -> PoolHeaderState {
        PoolHeaderState { words: self.header }
    }
    pub fn scalar(&self, index: usize) -> u32 {
        self.scalars[index]
    }
    /// Reads the complete scalar snapshot without assigning additional
    /// allocator or object-lifetime meaning to its raw words.
    pub fn scalar_state(&self) -> PoolScalarState {
        PoolScalarState::from_words(self.scalars)
    }
    /// Chain length (nodes mirrored, at most `CHAIN_CAP`).
    pub fn chain_len(&self) -> usize {
        self.chain.iter().take_while(|n| **n != 0).count()
    }

    // ---- write-through setters -------------------------------------------------------------

    pub fn set_header<M: GuestMemory>(&mut self, m: &mut M, index: usize, v: u32) -> bool {
        if index >= HEADER_WORDS {
            return false;
        }
        self.header[index] = v;
        m.write32(HEADER + 4 * index as u32, v);
        true
    }
    /// Writes the complete opaque header snapshot through to its original
    /// guest addresses. This does not change pool or list lifecycle policy.
    pub fn set_header_state<M: GuestMemory>(&mut self, m: &mut M, state: PoolHeaderState) {
        self.header = state.words;
        for (index, value) in state.words.into_iter().enumerate() {
            m.write32(HEADER + 4 * index as u32, value);
        }
    }
    /// Writes the update-list endpoints and stored length through to their
    /// original addresses. This does not link, allocate, or retire nodes.
    pub fn set_update_list_state<M: GuestMemory>(&mut self, m: &mut M, state: PoolUpdateListState) {
        self.set_scalar(m, S_LIST_HEAD, state.head);
        self.set_scalar(m, S_LIST_TAIL, state.tail);
        self.set_header(m, LIST_LEN_INDEX, state.len);
    }
    pub fn set_cell<M: GuestMemory>(
        &mut self,
        m: &mut M,
        table: usize,
        cell: usize,
        v: u32,
    ) -> bool {
        if table >= TABLE_COUNT || cell >= CELLS {
            return false;
        }
        self.tables[table][cell] = v;
        m.write32(cell_addr(table, cell), v);
        true
    }
    pub fn set_count<M: GuestMemory>(&mut self, m: &mut M, cell: usize, v: u32) -> bool {
        self.set_cell(m, COUNTS, cell, v)
    }
    pub fn set_planned<M: GuestMemory>(&mut self, m: &mut M, cell: usize, v: u32) -> bool {
        self.set_cell(m, PLANNED, cell, v)
    }
    pub fn set_cap<M: GuestMemory>(&mut self, m: &mut M, cell: usize, v: u32) -> bool {
        self.set_cell(m, CAPS, cell, v)
    }
    pub fn set_members_ptr<M: GuestMemory>(&mut self, m: &mut M, cell: usize, v: u32) -> bool {
        self.set_cell(m, MEMBERS, cell, v)
    }
    /// Writes a fifth-table word directly. Use the ledger leaf's typed view
    /// for the measured retired-count alias at `AUX[214]`; other cells stay raw.
    pub fn set_cell_auxiliary<M: GuestMemory>(
        &mut self,
        m: &mut M,
        cell: usize,
        value: u32,
    ) -> bool {
        self.set_cell(m, AUX, cell, value)
    }
    /// Writes one typed cell state through to all five parallel tables. The
    /// observed words are preserved verbatim; no allocator policy is inferred.
    pub fn set_cell_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        cell: usize,
        state: PoolCellState,
    ) -> bool {
        if cell >= CELLS {
            return false;
        }
        self.set_count(m, cell, state.count);
        self.set_planned(m, cell, state.planned);
        self.set_cap(m, cell, state.capacity);
        self.set_members_ptr(m, cell, state.members_ptr);
        self.set_cell(m, AUX, cell, state.auxiliary);
        true
    }
    pub fn set_scalar<M: GuestMemory>(&mut self, m: &mut M, index: usize, v: u32) -> bool {
        if index >= SCALARS.len() {
            return false;
        }
        self.scalars[index] = v;
        m.write32(SCALARS[index].0, v);
        true
    }

    /// Writes all scalar words through to their original guest addresses.
    /// This is a representation update only; allocator policy is unchanged.
    pub fn set_scalar_state<M: GuestMemory>(&mut self, m: &mut M, state: PoolScalarState) {
        let words = state.into_words();
        self.scalars = words;
        for (index, value) in words.into_iter().enumerate() {
            m.write32(SCALARS[index].0, value);
        }
    }

    /// Clears the five tables like the pool reset (`0xD6` cells each).
    pub fn clear_tables<M: GuestMemory>(&mut self, m: &mut M) {
        for t in 0..TABLE_COUNT {
            for c in 0..0xd6 {
                self.set_cell(m, t, c, 0);
            }
        }
    }

    /// The add function 0x9E430 (`game_object_pool_715fb4_next_free_slot_index`): appends `object` to
    /// the member array of `cell`, growing it inside the arena when full. Mirrors the lifted early exit:
    /// when the arena would overflow (`arena_used + new capacity >= 0x14CD`) the capacity word is already
    /// updated and nothing else changes; returns false. The old member dwords are copied from the guest.
    pub fn add_member<M: GuestMemory>(&mut self, m: &mut M, cell: usize, object: u32) -> bool {
        if cell >= CELLS {
            return false;
        }
        let count = self.tables[COUNTS][cell];
        let cap = self.tables[CAPS][cell];
        if (count as i32) >= (cap as i32) {
            let used = self.scalars[S_ARENA_USED];
            let old = self.tables[MEMBERS][cell];
            let new_cap = (cap.wrapping_add(7) & !7).wrapping_add(8);
            self.set_cap(m, cell, new_cap);
            let total = new_cap.wrapping_add(used);
            if (total as i32) >= ARENA_LIMIT as i32 {
                return false;
            }
            let ptr = self.scalars[S_ARENA_BASE].wrapping_add(used.wrapping_mul(4));
            self.set_members_ptr(m, cell, ptr);
            self.set_scalar(m, S_ARENA_USED, total);
            if ptr != 0 {
                for i in 0..(count & 0x3fff_ffff) {
                    let w = m.read32(old.wrapping_add(i * 4));
                    m.write32(ptr.wrapping_add(i * 4), w);
                    let idx = used.wrapping_add(i) as usize;
                    if idx < ARENA_CAP {
                        self.arena[idx] = w;
                    }
                }
            }
            self.arena_len = (total as usize).min(ARENA_CAP);
        }
        let arr = self.tables[MEMBERS][cell];
        m.write32(arr.wrapping_add(count.wrapping_mul(4)), object);
        if let Some(offset) = arr.checked_sub(self.scalars[S_ARENA_BASE]) {
            let idx = (offset / 4) as usize + count as usize;
            if offset & 3 == 0 && idx < self.arena_len {
                self.arena[idx] = object;
            }
        }
        self.set_count(m, cell, count.wrapping_add(1));
        true
    }

    /// The push function 0x9FBA0 without the bitmap bit (debt): `[node] = head`, `head = node`,
    /// list length + 1. The node's next word is written through into the guest.
    pub fn push_node<M: GuestMemory>(&mut self, m: &mut M, node: u32) {
        let head = self.scalars[S_LIST_HEAD];
        m.write32(node, head);
        self.set_scalar(m, S_LIST_HEAD, node);
        let len = self.header[LIST_LEN_INDEX].wrapping_add(1);
        self.set_header(m, LIST_LEN_INDEX, len);
        self.load_chain(m);
    }

    /// Pool reset of the list (0xA5AF0): head, tail and length cleared.
    pub fn clear_list<M: GuestMemory>(&mut self, m: &mut M) {
        self.set_scalar(m, S_LIST_HEAD, 0);
        self.set_scalar(m, S_LIST_TAIL, 0);
        self.set_header(m, LIST_LEN_INDEX, 0);
        self.chain = [0; CHAIN_CAP];
        self.chain_next = [0; CHAIN_CAP];
    }

    /// Writer: one dword of owned record `index`. False (nothing written) outside the owned prefix.
    pub fn set_record_word<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        offset: u32,
        v: u32,
    ) -> bool {
        if index >= self.owned_records() || offset % 4 != 0 || offset >= RECORD_SIZE {
            return false;
        }
        self.records[index][(offset / 4) as usize] = v;
        m.write32(rec_addr(self.scalars[S_RECORDS_BASE], index, offset), v);
        true
    }

    /// Writes the opaque Object+0x7C extension pointer word through the owned
    /// record. This does not allocate, dereference, or establish pointee
    /// lifetime.
    pub fn set_record_extension_address<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        address: PoolRecordExtensionAddress,
    ) -> bool {
        self.set_record_word(m, index, RECORD_EXTENSION_ADDRESS_OFFSET, address.0)
    }

    /// Reads the measured float position from an owned object record.
    pub fn record_position(&self, index: usize) -> Option<PoolRecordPosition> {
        let words = self.owned_record(index)?;
        Some(PoolRecordPosition {
            x: f32::from_bits(words[(RECORD_POSITION_OFFSET / 4) as usize]),
            y: f32::from_bits(words[((RECORD_POSITION_OFFSET + 4) / 4) as usize]),
            z: f32::from_bits(words[((RECORD_POSITION_OFFSET + 8) / 4) as usize]),
        })
    }

    /// Writes the measured position components through the bounded owned
    /// record and its guest-memory mirror. The writes do not imply a transform
    /// update, coordinate conversion, or object-lifetime transition.
    pub fn set_record_position<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        position: PoolRecordPosition,
    ) -> bool {
        if index >= self.owned_records() {
            return false;
        }
        for (component, value) in [position.x, position.y, position.z].into_iter().enumerate() {
            let offset = RECORD_POSITION_OFFSET + component as u32 * 4;
            if !self.set_record_word(m, index, offset, value.to_bits()) {
                return false;
            }
        }
        true
    }

    /// Applies only the measured model `0x3B3` initializer adjustment to
    /// Object+`0x68`. The caller supplies the adjustment value; Object+`0x64`
    /// and +`0x6C` are preserved. Unsupported model keys, invalid records, and
    /// absent records do not mutate owned state or guest memory. Snapshot any
    /// original position fields needed by the caller before this operation.
    pub fn apply_initializer_3b3_y_adjustment<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        y_adjustment: f32,
    ) -> Option<PoolRecordPosition> {
        if self.record_model_key(index)? != 0x03b3 {
            return None;
        }
        let mut position = self.record_position(index)?;
        position.y -= y_adjustment;
        self.set_record_word(m, index, RECORD_POSITION_OFFSET + 4, position.y.to_bits())
            .then_some(position)
    }

    /// Reads the initializer's original-position copy, distinct from the
    /// potentially adjusted position at Object+0x64/+0x68/+0x6C.
    pub fn record_original_position(&self, index: usize) -> Option<PoolRecordOriginalPosition> {
        let words = self.owned_record(index)?;
        Some(PoolRecordOriginalPosition {
            components: RECORD_ORIGINAL_POSITION_OFFSETS
                .map(|offset| f32::from_bits(words[(offset / 4) as usize])),
        })
    }

    /// Writes the three initializer original-position words through the
    /// bounded record and guest mirror. This does not update the adjusted
    /// Object position or the type-dependent extension.
    pub fn set_record_original_position<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        position: PoolRecordOriginalPosition,
    ) -> bool {
        if index >= self.owned_records() {
            return false;
        }
        for (offset, value) in RECORD_ORIGINAL_POSITION_OFFSETS
            .into_iter()
            .zip(position.components)
        {
            if !self.set_record_word(m, index, offset, value.to_bits()) {
                return false;
            }
        }
        true
    }

    /// Writes the initializer's separately typed current-position and
    /// original-position fields through one bounded ObjectPool operation.
    /// This only stores caller-provided values; it does not infer source
    /// instruction order, perform model adjustment, or establish identity or
    /// lifetime.
    pub fn set_record_initializer_position<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        position: PoolRecordInitializerPosition,
    ) -> bool {
        if index >= self.owned_records() {
            return false;
        }
        self.set_record_position(m, index, position.current)
            && self.set_record_original_position(m, index, position.original)
    }

    /// Applies the measured model `0x3B3` Object position portion of the
    /// initializer as one bounded operation. The caller supplies the input
    /// current and original-position triples and the adjustment value. The
    /// original triple is stored before the adjusted current triple; only the
    /// current Y component is changed. Unsupported model keys and invalid
    /// records leave both owned state and guest memory unchanged.
    pub fn apply_initializer_3b3_position_bundle<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        mut position: PoolRecordInitializerPosition,
        y_adjustment: f32,
    ) -> Option<PoolRecordInitializerPosition> {
        if self.record_model_key(index)? != 0x03b3 {
            return None;
        }

        position.current.y -= y_adjustment;
        if !self.set_record_original_position(m, index, position.original)
            || !self.set_record_position(m, index, position.current)
        {
            return None;
        }
        Some(position)
    }

    /// Applies the measured Object-field portion of the model `0x3B3`
    /// initializer: the common Object+`0x28` flag mask and the bounded
    /// original/current position bundle with its caller-supplied Y
    /// adjustment. Returns the typed values written. It does not initialize
    /// the type-dependent extension or perform CharacterSet membership/
    /// allocation work. Unsupported keys and invalid records leave state
    /// unchanged.
    pub fn apply_initializer_3b3_object_fields<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        position: PoolRecordInitializerPosition,
        y_adjustment: f32,
    ) -> Option<PoolRecordInitializerObjectFields> {
        if self.record_model_key(index)? != 0x03b3 {
            return None;
        }
        if !self.apply_initializer_object_flags(m, index) {
            return None;
        }
        let position =
            self.apply_initializer_3b3_position_bundle(m, index, position, y_adjustment)?;
        let object_flags = self.record_object_flags(index)?;
        Some(PoolRecordInitializerObjectFields {
            object_flags,
            position,
        })
    }

    /// Applies the measured Object-field portion of the model `0x482`
    /// initializer: the common Object+`0x28` flag mask and unchanged current
    /// and original position copies. The `0x3B3` Y adjustment is not used on
    /// this branch. This does not initialize the type-dependent extension or
    /// perform CharacterSet membership/allocation work. Unsupported keys and
    /// invalid records leave state unchanged.
    pub fn apply_initializer_482_object_fields<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        position: PoolRecordInitializerPosition,
    ) -> Option<PoolRecordInitializerObjectFields> {
        if self.record_model_key(index)? != 0x0482 {
            return None;
        }
        if !self.apply_initializer_object_flags(m, index)
            || !self.set_record_initializer_position(m, index, position)
        {
            return None;
        }
        Some(PoolRecordInitializerObjectFields {
            object_flags: self.record_object_flags(index)?,
            position,
        })
    }

    /// Applies the measured Object-field portion of the model `0x553`
    /// initializer: the common Object+`0x28` flag mask and unchanged current
    /// and original position copies. The `0x553`-specific flag bits are
    /// included by `apply_initializer_object_flags`. This does not initialize
    /// the type-dependent extension or perform CharacterSet
    /// membership/allocation work. Unsupported keys and invalid records leave
    /// state unchanged.
    pub fn apply_initializer_553_object_fields<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        position: PoolRecordInitializerPosition,
    ) -> Option<PoolRecordInitializerObjectFields> {
        if self.record_model_key(index)? != 0x0553 {
            return None;
        }
        if !self.apply_initializer_object_flags(m, index)
            || !self.set_record_initializer_position(m, index, position)
        {
            return None;
        }
        Some(PoolRecordInitializerObjectFields {
            object_flags: self.record_object_flags(index)?,
            position,
        })
    }

    /// Writes a caller-supplied bundle of separately measured Object fields
    /// through one bounded owned record and its guest-memory mirror. The
    /// bundle covers Object+0x28 flags, current/original positions, rotation
    /// inputs, and actor-parameter words. It makes no claim about source-call
    /// ordering, a common model branch, transform application, or the
    /// type-dependent extension at Object+0x7C.
    pub fn set_record_measured_fields<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        fields: PoolRecordMeasuredFields,
    ) -> bool {
        if index >= self.owned_records() {
            return false;
        }
        self.set_record_object_flags(m, index, fields.object_flags)
            && self.set_record_original_position(m, index, fields.position.original)
            && self.set_record_position(m, index, fields.position.current)
            && self.set_record_rotation_inputs(m, index, fields.rotation_inputs)
            && self.set_record_actor_parameters(m, index, fields.actor_parameters)
    }

    /// Reads the three measured rotation inputs from an owned object record.
    pub fn record_rotation_inputs(&self, index: usize) -> Option<PoolRecordRotationInputs> {
        let words = self.owned_record(index)?;
        Some(PoolRecordRotationInputs {
            components: RECORD_ROTATION_INPUT_OFFSETS
                .map(|offset| f32::from_bits(words[(offset / 4) as usize])),
        })
    }

    /// Writes the measured rotation input words through the bounded owned
    /// record and guest-memory mirror. This does not perform the separately
    /// observed extension scaling or imply a transform update.
    pub fn set_record_rotation_inputs<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        inputs: PoolRecordRotationInputs,
    ) -> bool {
        if index >= self.owned_records() {
            return false;
        }
        for (offset, value) in RECORD_ROTATION_INPUT_OFFSETS
            .into_iter()
            .zip(inputs.components)
        {
            if !self.set_record_word(m, index, offset, value.to_bits()) {
                return false;
            }
        }
        true
    }

    /// Reads the two measured actor-parameter floats from an owned object
    /// record. Their roles and update cadence remain unresolved.
    pub fn record_actor_parameters(&self, index: usize) -> Option<PoolRecordActorParameters> {
        let words = self.owned_record(index)?;
        Some(PoolRecordActorParameters {
            at_1cc: f32::from_bits(words[(RECORD_ACTOR_PARAMETER_OFFSETS[0] / 4) as usize]),
            at_1d8: f32::from_bits(words[(RECORD_ACTOR_PARAMETER_OFFSETS[1] / 4) as usize]),
        })
    }

    /// Writes the measured parameter floats through the bounded Rust-owned
    /// record and guest-memory mirror. This does not assign behavior to either
    /// field or imply actor identity/lifetime semantics.
    pub fn set_record_actor_parameters<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        parameters: PoolRecordActorParameters,
    ) -> bool {
        if index >= self.owned_records() {
            return false;
        }
        for (offset, value) in RECORD_ACTOR_PARAMETER_OFFSETS
            .into_iter()
            .zip([parameters.at_1cc, parameters.at_1d8])
        {
            if !self.set_record_word(m, index, offset, value.to_bits()) {
                return false;
            }
        }
        true
    }

    /// Reads Object+0x28 from the Rust-owned record without assigning meanings
    /// to its bits.
    pub fn record_object_flags(&self, index: usize) -> Option<PoolRecordObjectFlags> {
        self.owned_record(index)
            .map(|words| PoolRecordObjectFlags(words[(RECORD_OBJECT_FLAGS_OFFSET / 4) as usize]))
    }

    /// Writes raw Object+0x28 flags through the Rust-owned record.
    pub fn set_record_object_flags<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        flags: PoolRecordObjectFlags,
    ) -> bool {
        self.set_record_word(m, index, RECORD_OBJECT_FLAGS_OFFSET, flags.0)
    }

    /// Writes the measured u16 model key at Object+0x94 through the bounded
    /// Rust-owned record and guest mirror, preserving the other halfword.
    pub fn set_record_model_key<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        model_key: u16,
    ) -> bool {
        let Some(words) = self.owned_record(index) else {
            return false;
        };
        let word_index = (RECORD_MODEL_ID_OFFSET / 4) as usize;
        let word = (words[word_index] & 0xffff_0000) | u32::from(model_key);
        self.set_record_word(m, index, RECORD_MODEL_ID_OFFSET, word)
    }

    /// ORs bits into Object+0x28, preserving all other observed bits. T1661's
    /// `0x15C3D0` reactivation path applies mask `0x61` before membership append.
    pub fn add_record_object_flag_bits<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        bits: u32,
    ) -> bool {
        let Some(flags) = self.record_object_flags(index) else {
            return false;
        };
        self.set_record_object_flags(m, index, PoolRecordObjectFlags(flags.0 | bits))
    }

    /// Applies the measured Object+0x28 flag portion of character reactivation.
    /// T1661 identifies mask 0x61 before the conditional CharacterSet append;
    /// the eligibility branch and extension state remain separate operations.
    pub fn apply_character_reactivation_object_flags<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
    ) -> bool {
        self.add_record_object_flag_bits(m, index, CHARACTER_REACTIVATION_OBJECT_FLAGS)
    }

    /// Applies the measured Object+0x28 retirement bit from T1661's A7090
    /// body. Callback ordering, pool accounting, membership unlink, and actor
    /// lifetime remain separate caller-owned operations.
    pub fn mark_record_retired<M: GuestMemory>(&mut self, m: &mut M, index: usize) -> bool {
        self.add_record_object_flag_bits(m, index, OBJECT_RETIRED_FLAG)
    }

    /// Applies the measured Object+0x28 flag portion of the actor initializer:
    /// all model keys receive 0x50, and key 0x553 also receives 0x40020000.
    /// Existing flag bits are preserved; their meanings and the rest of the
    /// initializer remain outside this operation.
    pub fn apply_initializer_object_flags<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
    ) -> bool {
        let Some(model_key) = self.record_model_key(index) else {
            return false;
        };
        let model_flags = if model_key == 0x0553 {
            MODEL_0553_INITIALIZER_OBJECT_FLAGS
        } else {
            0
        };
        self.add_record_object_flag_bits(m, index, INITIALIZER_OBJECT_FLAGS | model_flags)
    }

    /// Reads a complete owned record snapshot. Records beyond the owned used
    /// prefix are not exposed, even if the backing allocation has spare slots.
    pub fn record_state(&self, index: usize) -> Option<PoolRecordState> {
        if index >= self.owned_records() {
            return None;
        }
        Some(PoolRecordState {
            words: *self.records.get(index)?,
        })
    }

    /// Replaces one complete owned record and writes every dword through to
    /// the guest image. This preserves unknown fields verbatim and does not
    /// create object identity or lifecycle semantics.
    pub fn set_record_state<M: GuestMemory>(
        &mut self,
        m: &mut M,
        index: usize,
        state: PoolRecordState,
    ) -> bool {
        if index >= self.owned_records() {
            return false;
        }
        self.records[index] = state.words;
        let base = rec_addr(self.scalars[S_RECORDS_BASE], index, 0);
        for (word, value) in state.words.into_iter().enumerate() {
            m.write32(base.wrapping_add(word as u32 * 4), value);
        }
        true
    }

    /// Typed object prefix decoded from the owned words (not from guest memory).
    pub fn owned(&mut self, index: usize) -> Option<TsfpObjectOwned> {
        (index < self.owned_records()).then(|| {
            let base = rec_addr(self.scalars[S_RECORDS_BASE], index, 0);
            TsfpObjectView(base).load(&RecordMem {
                base,
                words: &mut self.records[index],
            })
        })
    }
}

/// T2122: where one guest store lands in the pool, decoded without a full mirror (the record array stays
/// out of the per-store path, as `PlayerSet::load_cells` does for the player record).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolSlot {
    Header(usize),
    Cell { table: usize, cell: usize },
    Scalar(usize),
    Arena { index: usize },
    Record { index: usize, offset: u32 },
}

impl PoolSlot {
    /// Guest address of the dword the slot names.
    pub fn address<M: GuestMemory>(self, m: &M) -> u32 {
        match self {
            PoolSlot::Header(i) => HEADER + 4 * i as u32,
            PoolSlot::Cell { table, cell } => cell_addr(table, cell),
            PoolSlot::Scalar(i) => SCALARS[i].0,
            PoolSlot::Arena { index } => m
                .read32(SCALARS[S_ARENA_BASE].0)
                .wrapping_add(index as u32 * 4),
            PoolSlot::Record { index, offset } => {
                rec_addr(m.read32(SCALARS[S_RECORDS_BASE].0), index, offset)
            }
        }
    }
}

impl ObjectPool {
    /// Decodes the dword containing `address`: header word, table cell, scalar global, or a dword of an owned
    /// record (`index < min([0x7B4FAC], REC_CAP)` at `[0x72F5E8]`). None for anything else (stays a raw store).
    pub fn classify<M: GuestMemory>(m: &M, address: u32) -> Option<PoolSlot> {
        let a = address & !3;
        if (HEADER..HEADER + 4 * HEADER_WORDS as u32).contains(&a) {
            return Some(PoolSlot::Header(((a - HEADER) / 4) as usize));
        }
        let tables_end = TABLES_BASE + TABLE_STRIDE * TABLE_COUNT as u32;
        if (TABLES_BASE..tables_end).contains(&a) {
            let off = a - TABLES_BASE;
            return Some(PoolSlot::Cell {
                table: (off / TABLE_STRIDE) as usize,
                cell: ((off % TABLE_STRIDE) / 4) as usize,
            });
        }
        if let Some(i) = SCALARS.iter().position(|(s, _)| *s == a) {
            return Some(PoolSlot::Scalar(i));
        }
        let arena = m.read32(SCALARS[S_ARENA_BASE].0);
        let arena_len = (m.read32(SCALARS[S_ARENA_USED].0) as usize).min(ARENA_CAP);
        if arena != 0 && a >= arena {
            let off = a - arena;
            let index = (off / 4) as usize;
            if off % 4 == 0 && index < arena_len {
                return Some(PoolSlot::Arena { index });
            }
        }
        let base = m.read32(SCALARS[S_RECORDS_BASE].0);
        if base != 0 && a >= base {
            let off = a - base;
            let index = (off / RECORD_SIZE) as usize;
            let used = (m.read32(SCALARS[S_USED_RECORDS].0) as usize).min(REC_CAP);
            if index < used {
                return Some(PoolSlot::Record {
                    index,
                    offset: off % RECORD_SIZE,
                });
            }
        }
        None
    }

    /// Write-through of one dword of the slot (owned word and guest word are the same value). The transient
    /// mirror is not kept: the guest image stays authoritative, exactly as in the other wired setters.
    pub fn write_through<M: GuestMemory>(m: &mut M, slot: PoolSlot, v: u32) {
        let a = slot.address(m);
        m.write32(a, v);
    }
}
