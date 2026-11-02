//! Rust-owned party snapshot and restore rules from T1702.
//!
//! The original contract stores a zero-terminated ID run and one opaque 0xD8-byte extension
//! record per object. This model owns those records and keeps snapshot IDs distinct from live
//! object handles. It also preserves the original restore cursor, which adds the number already
//! restored to each player's 0x14-slot base. See `docs/t1702-rea-rules.md` for evidence and open
//! runtime limits.

pub const PARTY_MODE_BYTE: u8 = 0x0a;
pub const SNAPSHOT_SLOTS_PER_PLAYER: usize = 0x14;
pub const EXTENSION_RECORD_WORDS: usize = 0x36;

/// Generation supplied by the owner of the world snapshot. T1702 does not establish how
/// generations are allocated or compared, so this type carries provenance without inventing it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SnapshotGeneration(pub u64);

/// The saved unsigned type word at object +0x94. It is a terminator-bearing snapshot value, not
/// a live object address or a handle that may be dereferenced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotObjectId(pub u16);

/// Opaque 54-dword copy of an object's extension record. Fields remain raw bits until their
/// pointer/reference meaning has separate evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionSnapshot(pub [u32; EXTENSION_RECORD_WORDS]);

impl ExtensionSnapshot {
    pub fn constructor_args(&self) -> PartyObjectCreateArgs {
        let word = |byte_offset: usize| self.0[byte_offset / 4];
        PartyObjectCreateArgs {
            field_10: word(0x10),
            field_0c: word(0x0c),
            field_14: word(0x14),
            field_1c: word(0x1c),
            field_18: word(0x18),
            field_40: word(0x40),
            field_44: word(0x44),
            field_48: word(0x48),
            field_4c: word(0x4c),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartyObjectSnapshot {
    pub id: SnapshotObjectId,
    pub extension: ExtensionSnapshot,
}

/// Typed names for the nine saved words passed to the original constructor. Its scratch pointer
/// argument is an ABI detail and is intentionally owned by the reconstruction implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartyObjectCreateArgs {
    pub field_10: u32,
    pub field_0c: u32,
    pub field_14: u32,
    pub field_1c: u32,
    pub field_18: u32,
    pub field_40: u32,
    pub field_44: u32,
    pub field_48: u32,
    pub field_4c: u32,
}

/// Flat snapshot slots mirror the original per-player base spacing. A dynamic vector avoids
/// baking the unproven list-capacity invariant into Rust; bytes beyond the documented table
/// extent remain an integration/runtime question.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartySnapshot {
    generation: SnapshotGeneration,
    player_count: usize,
    slots: Vec<Option<PartyObjectSnapshot>>,
}

/// Read-only native inventory boundary for party capture. Each player's slice is already in
/// linked-list order; implementations own how that ordered view is produced.
pub trait PartySnapshotSource {
    fn players_in_link_order(&self) -> &[Vec<PartyObjectSnapshot>];
}

impl PartySnapshotSource for [Vec<PartyObjectSnapshot>] {
    fn players_in_link_order(&self) -> &[Vec<PartyObjectSnapshot>] {
        self
    }
}

impl PartySnapshotSource for Vec<Vec<PartyObjectSnapshot>> {
    fn players_in_link_order(&self) -> &[Vec<PartyObjectSnapshot>] {
        self.as_slice()
    }
}

impl PartySnapshot {
    pub fn empty(generation: SnapshotGeneration, player_count: usize) -> Self {
        Self {
            generation,
            player_count,
            slots: vec![None; player_count.saturating_mul(SNAPSHOT_SLOTS_PER_PLAYER)],
        }
    }

    pub fn generation(&self) -> SnapshotGeneration {
        self.generation
    }

    pub fn player_count(&self) -> usize {
        self.player_count
    }

    /// Copy each player's linked objects in the order supplied by the owned inventory view.
    /// Mode mismatch or an empty party leaves the existing snapshot untouched, matching the
    /// original gates. Existing trailing slots are retained just as the original globals are not
    /// cleared; each new run receives its own zero terminator.
    pub fn capture(&mut self, mode_byte: u8, players: &[Vec<PartyObjectSnapshot>]) -> bool {
        self.capture_from(mode_byte, players)
    }

    /// Capture from an owned Rust inventory boundary without exposing guest-memory addresses or
    /// changing the order chosen by the player-list owner.
    pub fn capture_from<S: PartySnapshotSource + ?Sized>(
        &mut self,
        mode_byte: u8,
        source: &S,
    ) -> bool {
        let players = source.players_in_link_order();
        if mode_byte != PARTY_MODE_BYTE || players.is_empty() {
            return false;
        }
        self.player_count = players.len();
        let required = players.len().saturating_mul(SNAPSHOT_SLOTS_PER_PLAYER);
        if self.slots.len() < required {
            self.slots.resize(required, None);
        }
        for (player, objects) in players.iter().enumerate() {
            let base = player.saturating_mul(SNAPSHOT_SLOTS_PER_PLAYER);
            for (offset, object) in objects.iter().enumerate() {
                let index = base.saturating_add(offset);
                if index >= self.slots.len() {
                    self.slots.resize(index.saturating_add(1), None);
                }
                self.slots[index] = Some(object.clone());
            }
            let terminator = base.saturating_add(objects.len());
            if terminator >= self.slots.len() {
                self.slots.resize(terminator.saturating_add(1), None);
            }
            self.slots[terminator] = None;
        }
        true
    }

    pub fn slot(&self, index: usize) -> Option<&PartyObjectSnapshot> {
        self.slots.get(index).and_then(Option::as_ref)
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Reconstruct objects and insert them into each owner's list. The accumulated restore cursor
    /// is intentionally retained from the lifted routine. A missing slot is reported because the
    /// original's read outside the owned snapshot extent has no established safe result.
    pub fn restore<B: PartyRestoreBackend>(
        &self,
        mode_byte: u8,
        backend: &mut B,
    ) -> Result<PartyRestoreReport, PartyRestoreError<B::Error>> {
        if mode_byte != PARTY_MODE_BYTE || self.player_count == 0 {
            return Ok(PartyRestoreReport {
                restored_objects: 0,
            });
        }
        let mut restored = 0usize;
        for player in 0..self.player_count {
            let base = player.saturating_mul(SNAPSHOT_SLOTS_PER_PLAYER);
            let mut index = base.saturating_add(restored);
            loop {
                let Some(saved) = self.slots.get(index) else {
                    return Err(PartyRestoreError::MissingSnapshotSlot { player, index });
                };
                let Some(saved) = saved else { break };
                // The original reads the stored u16 type as a dword sentinel. A linked
                // object with type zero is copied during capture but terminates restoration.
                if saved.id.0 == 0 {
                    break;
                }
                let object = backend
                    .construct(saved.extension.constructor_args())
                    .map_err(|source| PartyRestoreError::Construct {
                        player,
                        index,
                        source,
                    })?;
                backend
                    .replace_extension(object, &saved.extension)
                    .map_err(|source| PartyRestoreError::CopyExtension {
                        player,
                        index,
                        source,
                    })?;
                backend.insert_owned(player, object).map_err(|source| {
                    PartyRestoreError::InsertOwned {
                        player,
                        index,
                        source,
                    }
                })?;
                restored = restored.saturating_add(1);
                index = index.saturating_add(1);
            }
        }
        Ok(PartyRestoreReport {
            restored_objects: restored,
        })
    }
}

/// A handle issued by a live Rust object owner. It is intentionally a backend-associated type,
/// never interchangeable with [`SnapshotObjectId`].
pub trait PartyRestoreBackend {
    type Object: Copy;
    type Error;

    fn construct(&mut self, args: PartyObjectCreateArgs) -> Result<Self::Object, Self::Error>;
    fn replace_extension(
        &mut self,
        object: Self::Object,
        saved: &ExtensionSnapshot,
    ) -> Result<(), Self::Error>;
    fn insert_owned(&mut self, player: usize, object: Self::Object) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartyRestoreReport {
    pub restored_objects: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PartyRestoreError<E> {
    MissingSnapshotSlot {
        player: usize,
        index: usize,
    },
    Construct {
        player: usize,
        index: usize,
        source: E,
    },
    CopyExtension {
        player: usize,
        index: usize,
        source: E,
    },
    InsertOwned {
        player: usize,
        index: usize,
        source: E,
    },
}
