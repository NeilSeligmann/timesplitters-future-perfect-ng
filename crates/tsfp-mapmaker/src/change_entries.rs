//! Rust-owned state for deleting the active Map Maker change-table entry.
//!
//! The original callback compacts the table and clears its active selection
//! when that selected entry is deleted. Entry payloads remain opaque until a
//! separately measured contract defines their fields.

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChangeEntryGeneration(u64);

impl ChangeEntryGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChangeEntryId {
    generation: ChangeEntryGeneration,
    serial: u64,
}

impl ChangeEntryId {
    pub const fn generation(self) -> ChangeEntryGeneration {
        self.generation
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeEntry<K, T> {
    pub id: ChangeEntryId,
    pub kind: K,
    pub payload: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeEntryError {
    StaleGeneration,
    MissingEntry,
    NoActiveSelection,
    EntryIsNotActive,
    ActiveEntryMissing,
    IdentitySpaceExhausted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletedChangeEntry<K, T> {
    pub index_before_compaction: usize,
    pub entry: ChangeEntry<K, T>,
    pub remaining_total: usize,
    pub remaining_of_kind: usize,
}

/// Owned change-table rows and their active identity for one editor generation.
///
/// The table preserves payload order while compaction moves rows. UI commands
/// hold `ChangeEntryId`, never a slot index, so a delayed command cannot delete
/// a different row after compaction or after the table is replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeEntryTable<K, T> {
    generation: ChangeEntryGeneration,
    next_serial: u64,
    entries: Vec<ChangeEntry<K, T>>,
    active: Option<ChangeEntryId>,
}

impl<K: Copy + Eq, T> ChangeEntryTable<K, T> {
    pub fn new(generation: ChangeEntryGeneration) -> Self {
        Self {
            generation,
            next_serial: 1,
            entries: Vec::new(),
            active: None,
        }
    }

    pub const fn generation(&self) -> ChangeEntryGeneration {
        self.generation
    }

    pub fn entries(&self) -> &[ChangeEntry<K, T>] {
        &self.entries
    }

    pub const fn active(&self) -> Option<ChangeEntryId> {
        self.active
    }

    pub fn count_of_kind(&self, kind: K) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.kind == kind)
            .count()
    }

    pub fn insert(&mut self, kind: K, payload: T) -> Result<ChangeEntryId, ChangeEntryError> {
        let serial = self.next_serial;
        self.next_serial = serial
            .checked_add(1)
            .ok_or(ChangeEntryError::IdentitySpaceExhausted)?;
        let id = ChangeEntryId {
            generation: self.generation,
            serial,
        };
        self.entries.push(ChangeEntry { id, kind, payload });
        Ok(id)
    }

    pub fn select(&mut self, id: ChangeEntryId) -> Result<(), ChangeEntryError> {
        self.require_generation(id)?;
        if !self.entries.iter().any(|entry| entry.id == id) {
            return Err(ChangeEntryError::MissingEntry);
        }
        self.active = Some(id);
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.active = None;
    }

    /// Delete the active row through its generation-qualified identity.
    ///
    /// The measured callbacks compact later records and clear the selected
    /// index when it names the deleted row. They do not select the row that
    /// shifts into that slot, so this method leaves `active` empty.
    pub fn delete_active(
        &mut self,
        id: ChangeEntryId,
    ) -> Result<DeletedChangeEntry<K, T>, ChangeEntryError> {
        self.require_generation(id)?;
        let Some(active) = self.active else {
            return Err(ChangeEntryError::NoActiveSelection);
        };
        if active != id {
            return Err(ChangeEntryError::EntryIsNotActive);
        }
        let index = self
            .entries
            .iter()
            .position(|entry| entry.id == id)
            .ok_or(ChangeEntryError::ActiveEntryMissing)?;
        let entry = self.entries.remove(index);
        self.active = None;
        let remaining_total = self.entries.len();
        let remaining_of_kind = self.count_of_kind(entry.kind);
        Ok(DeletedChangeEntry {
            index_before_compaction: index,
            entry,
            remaining_total,
            remaining_of_kind,
        })
    }

    fn require_generation(&self, id: ChangeEntryId) -> Result<(), ChangeEntryError> {
        if id.generation != self.generation {
            Err(ChangeEntryError::StaleGeneration)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ChangeEntryError, ChangeEntryGeneration, ChangeEntryId, ChangeEntryTable};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        Terrain,
        Object,
    }

    fn table() -> (ChangeEntryTable<Kind, u32>, [ChangeEntryId; 4]) {
        let mut table = ChangeEntryTable::new(ChangeEntryGeneration::new(7));
        let ids = [
            table.insert(Kind::Terrain, 10).unwrap(),
            table.insert(Kind::Object, 20).unwrap(),
            table.insert(Kind::Terrain, 30).unwrap(),
            table.insert(Kind::Object, 40).unwrap(),
        ];
        (table, ids)
    }

    #[test]
    fn deleting_first_middle_and_final_rows_compacts_without_selecting_a_neighbor() {
        for (index, expected) in [
            (0, vec![20, 30, 40]),
            (1, vec![10, 30, 40]),
            (3, vec![10, 20, 30]),
        ] {
            let (mut table, ids) = table();
            let id = ids[index];
            table.select(id).unwrap();

            let deleted = table.delete_active(id).unwrap();

            assert_eq!(deleted.index_before_compaction, index);
            assert_eq!(deleted.entry.payload, [10, 20, 30, 40][index]);
            assert_eq!(
                table
                    .entries()
                    .iter()
                    .map(|entry| entry.payload)
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(table.active(), None);
        }
    }

    #[test]
    fn deleting_the_only_row_leaves_an_empty_unselected_table() {
        let mut table = ChangeEntryTable::new(ChangeEntryGeneration::new(1));
        let id = table.insert(Kind::Object, 99).unwrap();
        table.select(id).unwrap();

        let deleted = table.delete_active(id).unwrap();

        assert_eq!(deleted.remaining_total, 0);
        assert_eq!(deleted.remaining_of_kind, 0);
        assert!(table.entries().is_empty());
        assert_eq!(table.active(), None);
    }

    #[test]
    fn stale_and_non_active_commands_cannot_delete_a_compacted_row() {
        let (mut table, ids) = table();
        table.select(ids[1]).unwrap();

        assert_eq!(
            table.delete_active(ids[0]),
            Err(ChangeEntryError::EntryIsNotActive)
        );
        assert_eq!(
            table.delete_active(ChangeEntryId {
                generation: ChangeEntryGeneration::new(8),
                serial: 2,
            }),
            Err(ChangeEntryError::StaleGeneration)
        );
        assert_eq!(table.entries().len(), 4);
        assert_eq!(table.active(), Some(ids[1]));
    }

    #[test]
    fn deletion_updates_type_counts_without_reinterpreting_payload() {
        let (mut table, ids) = table();
        assert_eq!(table.count_of_kind(Kind::Terrain), 2);
        assert_eq!(table.count_of_kind(Kind::Object), 2);
        table.select(ids[1]).unwrap();

        let deleted = table.delete_active(ids[1]).unwrap();

        assert_eq!(deleted.entry.payload, 20);
        assert_eq!(deleted.remaining_of_kind, 1);
        assert_eq!(table.count_of_kind(Kind::Terrain), 2);
        assert_eq!(table.count_of_kind(Kind::Object), 1);
    }
}
