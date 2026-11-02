// SPDX-License-Identifier: GPL-3.0-or-later
//! T2377: native, owned contracts for the REA inventory lookup and interaction continuation.
//!
//! The original table layouts and call order are specified in `docs/t1680-rea-interaction.md`
//! and `docs/t2367-rea-inventory.md`. This module deliberately has no guest-memory dependency:
//! callers provide Rust-owned table rows and stable IDs. T2377's opt-in hybrid adapter compiles
//! this same source into the lane; its switch defaults off, so default behavior is unchanged.

/// Original setup/loadout entry count.
pub const ENTRY_COUNT: usize = 70;
/// Ammo record table count. Ammo type zero is a valid stored word but not a definition row.
pub const AMMO_TYPE_COUNT: usize = 44;
/// Ammo rows with definitions reachable through the `0x40` object-definition branch.
pub const FIRST_AMMO_DEFINITION: u32 = 1;
pub const LAST_AMMO_DEFINITION: u32 = 43;

/// Two ammo fields paired with the amount granted by a weapon record.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AmmoPair {
    pub ammo_type: u32,
    pub amount: u32,
}

/// The two record subrecords consumed by pickup grant and definition lookup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponAmmoRecord {
    pub primary: AmmoPair,
    pub secondary: AmmoPair,
}

impl WeaponAmmoRecord {
    /// Return the original pickup pair order, including zero/negative-looking dwords unchanged.
    pub const fn pickup_pairs(self) -> [AmmoPair; 2] {
        [self.primary, self.secondary]
    }
}

/// Consumer-established fields in a 0x54-byte game-object definition view.
///
/// Unnamed words remain in `words`; this is not a claim that the complete original structure is
/// known. Ammo definitions embed this view at row `+0x8c` within a 0xe0-byte ammo row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameObjectDefinition {
    words: [u32; 21],
}

impl GameObjectDefinition {
    pub const WORD_COUNT: usize = 21;

    pub const fn from_words(words: [u32; Self::WORD_COUNT]) -> Self {
        Self { words }
    }

    /// Consumer-established `+0` text/label ID candidate.
    pub const fn label_id(&self) -> u32 {
        self.words[0]
    }

    /// Consumer-established `+4` stored resource/model ID, not the address of this field.
    pub const fn stored_resource_id(&self) -> u32 {
        self.words[1]
    }

    /// Creation payload word at definition `+0x28`; its component meaning remains unresolved.
    pub const fn creation_payload_a(&self) -> u32 {
        self.words[10]
    }

    /// Creation payload word at definition `+0x3c`; its component meaning remains unresolved.
    pub const fn creation_payload_b(&self) -> u32 {
        self.words[15]
    }

    /// `+0x48` is an optional resolver callback. Keep its identity typed, never as a callable
    /// address or guest pointer.
    pub const fn resolver_id(&self) -> Option<u32> {
        match self.words[18] {
            0 => None,
            id => Some(id),
        }
    }

    pub const fn words(&self) -> &[u32; Self::WORD_COUNT] {
        &self.words
    }
}

/// A single ammo row's definition view and separate reserve limit.
///
/// The reserve limit is carried independently because the definition begins at `+0x8c`; its
/// presence here does not assign an unmeasured byte offset within the row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AmmoDefinitionRow {
    pub reserve_limit: i32,
    pub definition: GameObjectDefinition,
}

/// A loaded definition table for one original type code. The table contents are Rust-owned and
/// preserve the source table's valid extent; the original has no local upper clamp on most of
/// these branches, so callers must provide the complete valid table.
#[derive(Clone, Copy, Debug)]
pub struct DefinitionTypeTable<'a> {
    pub type_code: u32,
    pub rows: &'a [Option<GameObjectDefinition>],
}

/// Immutable catalog for the definition-selection part of `0xbe860`.
#[derive(Clone, Copy, Debug)]
pub struct DefinitionCatalog<'a> {
    pub ammo_rows: &'a [Option<AmmoDefinitionRow>],
    pub type_tables: &'a [DefinitionTypeTable<'a>],
    pub fixed_type_800: Option<&'a GameObjectDefinition>,
}

impl<'a> DefinitionCatalog<'a> {
    /// Select a definition by its original type code and signed table index.
    ///
    /// Type `0x40` uses ammo rows `1..=43`; `0x800` is a fixed definition; `0x100` and
    /// `0x1000` share the `0x100` table. Other types use the table explicitly supplied by the
    /// runtime data loader. Invalid indices return `None` instead of manufacturing a pointer.
    pub fn lookup(&self, type_code: u32, index: i32) -> Option<&'a GameObjectDefinition> {
        if index < 0 {
            return None;
        }
        match type_code {
            0x40 => {
                let index = index as u32;
                if !(FIRST_AMMO_DEFINITION..=LAST_AMMO_DEFINITION).contains(&index) {
                    return None;
                }
                self.ammo_rows
                    .get(index as usize)?
                    .as_ref()
                    .map(|row| &row.definition)
            }
            0x800 => self.fixed_type_800,
            0x100 | 0x1000 => self
                .type_tables
                .iter()
                .find(|table| table.type_code == 0x100)?
                .rows
                .get(index as usize)?
                .as_ref(),
            other => self
                .type_tables
                .iter()
                .find(|table| table.type_code == other)?
                .rows
                .get(index as usize)?
                .as_ref(),
        }
    }

    /// Resolve an entry's primary or secondary ammo subrecord to its consumed definition view.
    pub fn ammo_definition_for_entry(
        &self,
        inventory: &InventoryDefinitionView<'_>,
        entry_index: i32,
        hand: AmmoHand,
    ) -> Option<&'a GameObjectDefinition> {
        let ammo_type = match hand {
            AmmoHand::Primary => inventory.primary_ammo_type(entry_index),
            AmmoHand::Secondary => inventory.secondary_ammo_type(entry_index),
        };
        self.lookup(0x40, ammo_type as i32)
    }
}

/// Data needed to resolve inventory entries and their primary/secondary ammo definitions.
///
/// The slices are snapshots owned by the caller. Valid source tables have 70 entries, 66 weapon
/// records and 44 ammo slots; shorter slices are accepted for partial loaded data and fail closed.
#[derive(Clone, Copy, Debug)]
pub struct InventoryDefinitionView<'a> {
    entry_records: &'a [i32],
    records: &'a [Option<WeaponAmmoRecord>],
    ammo_rows: &'a [Option<AmmoDefinitionRow>],
}

impl<'a> InventoryDefinitionView<'a> {
    pub const fn new(
        entry_records: &'a [i32],
        records: &'a [Option<WeaponAmmoRecord>],
        ammo_rows: &'a [Option<AmmoDefinitionRow>],
    ) -> Self {
        Self {
            entry_records,
            records,
            ammo_rows,
        }
    }

    /// Equivalent to the consumed primary-ammo getter: invalid entry/record bounds yield zero.
    pub fn primary_ammo_type(&self, entry_index: i32) -> u32 {
        self.record(entry_index)
            .map_or(0, |record| record.primary.ammo_type)
    }

    /// Equivalent to the consumed secondary-ammo getter: invalid entry/record bounds yield zero.
    pub fn secondary_ammo_type(&self, entry_index: i32) -> u32 {
        self.record(entry_index)
            .map_or(0, |record| record.secondary.ammo_type)
    }

    /// Return the record's primary and secondary pickup pairs in their original order.
    pub fn pickup_pairs(&self, entry_index: i32) -> Option<[AmmoPair; 2]> {
        self.record(entry_index).map(|record| record.pickup_pairs())
    }

    /// Resolve an entry's ammo definition. Type zero and types above 43 have no `0x40` row.
    pub fn ammo_definition(
        &self,
        entry_index: i32,
        hand: AmmoHand,
    ) -> Option<&'a AmmoDefinitionRow> {
        let record = self.record(entry_index)?;
        let ammo_type = match hand {
            AmmoHand::Primary => record.primary.ammo_type,
            AmmoHand::Secondary => record.secondary.ammo_type,
        };
        if !(FIRST_AMMO_DEFINITION..=LAST_AMMO_DEFINITION).contains(&ammo_type) {
            return None;
        }
        self.ammo_rows.get(ammo_type as usize)?.as_ref()
    }

    fn record(&self, entry_index: i32) -> Option<&'a WeaponAmmoRecord> {
        if !(0..ENTRY_COUNT as i32).contains(&entry_index) {
            return None;
        }
        let record_index = *self.entry_records.get(entry_index as usize)?;
        if !(0..66).contains(&record_index) {
            return None;
        }
        self.records.get(record_index as usize)?.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmmoHand {
    Primary,
    Secondary,
}

/// Dispatch the definition's stored `+4` dword through its optional resource resolver.
///
/// A resolver receives the stored value and its return is the resolved ID. With no resolver the
/// stored value itself is returned. This does not return the address of `+4`.
pub fn resolve_resource_id(
    definition: &GameObjectDefinition,
    resolver: &mut dyn ResourceResolver,
) -> u32 {
    match definition.resolver_id() {
        Some(callback) => resolver.resolve(callback, definition.stored_resource_id()),
        None => definition.stored_resource_id(),
    }
}

pub trait ResourceResolver {
    fn resolve(&mut self, callback: u32, stored_resource_id: u32) -> u32;
}

/// Distinct, non-interchangeable identities at the player/interaction boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerRecordId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InteractionTargetId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InteractionCallbackId(pub u32);

/// Captured before publishing the target reference; never recomputed from a later current player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapturedInteractionPlayer {
    pub record: PlayerRecordId,
    pub actor: ActorId,
}

/// Host boundary for the original A0D30 continuation. Implementations own all mutable state.
/// `mark_mission` must read the target's key at call time, after any callback has run.
pub trait InteractionRuntime {
    fn capture_player(&self, player: PlayerId) -> Option<CapturedInteractionPlayer>;
    fn publish_target_reference(&mut self, player: PlayerId, target: InteractionTargetId);
    fn callback_for_target(&mut self, target: InteractionTargetId)
    -> Option<InteractionCallbackId>;
    fn invoke_callback(
        &mut self,
        callback: InteractionCallbackId,
        target: InteractionTargetId,
        captured_actor: ActorId,
    );
    fn set_interacted_flag(&mut self, captured_record: PlayerRecordId);
    fn mark_mission_entries(&mut self, target: InteractionTargetId);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionError {
    MissingPlayerRecord,
}

/// Submit a selected object while preserving the observed capture/send/callback/mark chronology.
///
/// This function is an attachment point after target selection and configured input gates; it
/// does not define the interaction key, target-selection policy, or network internals. A failed
/// player capture has no partial effects. Once captured, sender success does not gate local
/// callback or marking work. The handler is looked up after sending, and mission keys are read by
/// the runtime only after the callback returns.
pub fn submit_interaction<R: InteractionRuntime>(
    runtime: &mut R,
    player: PlayerId,
    target: InteractionTargetId,
) -> Result<(), InteractionError> {
    let captured = runtime
        .capture_player(player)
        .ok_or(InteractionError::MissingPlayerRecord)?;
    runtime.publish_target_reference(player, target);
    if let Some(callback) = runtime.callback_for_target(target) {
        runtime.invoke_callback(callback, target, captured.actor);
    }
    runtime.set_interacted_flag(captured.record);
    runtime.mark_mission_entries(target);
    Ok(())
}
