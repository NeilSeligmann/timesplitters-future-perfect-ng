// SPDX-License-Identifier: GPL-3.0-or-later
//! T2492: Rust-owned Story objective registry contract for `0x227E90`/`0x227750`.
//!
//! The lifted writer has no observed capacity check. Callers must provide an explicit allocation
//! contract; this lane refuses registration when that contract is absent or inconsistent. The
//! guest adapter stays default-off until the allocation and natural replay gates are satisfied.

use alloc::vec::Vec;

pub const NO_KIND: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ObjectiveStatus {
    #[default]
    Pending,
    Active,
    Inactive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectiveDefinition {
    pub id: u32,
    pub kind_by_submode: [u32; 3],
    pub predicate_mask: u32,
    pub status: ObjectiveStatus,
    pub dependencies: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectiveRecord {
    pub id: u32,
    pub kind: u8,
    pub status: ObjectiveStatus,
    pub dependencies: Vec<u32>,
}

/// Allocation extents supplied by the owner that creates the actual runtime tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocationContract {
    pub record_slots: usize,
    pub creation_order_slots: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectiveRegistry {
    pub records: Vec<Option<ObjectiveRecord>>,
    pub creation_order: Vec<u32>,
    pub kind_counts: [u32; 3],
    pub dependency_lists: Vec<Vec<u32>>,
    pub capacity: usize,
    pub creation_order_capacity: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterError {
    MissingAllocationContract,
    InconsistentAllocation,
    RecordOutOfBounds,
    CreationOrderFull,
    InvalidKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoryContext {
    pub game_flags: u32,
    pub mode_id: u8,
    pub submode: u8,
    pub difficulty: u8,
    pub players: u32,
    pub network: bool,
    pub descriptor_word: u32,
}

impl StoryContext {
    pub fn allows(&self, mask: u32) -> bool {
        if self.game_flags & (0x40 | 0x0800_0000) != 0 {
            return self.descriptor_word & mask != 0;
        }
        if mask & 0x8000_0000 != 0 && self.network {
            return false;
        }
        if mask & 0x0060_0000 != 0 {
            let one = mask & 0x0020_0000 != 0 && self.players == 1;
            let two = mask & 0x0040_0000 != 0 && self.players == 2;
            if !(one || two) {
                return false;
            }
        }
        let mode_bit = match self.mode_id {
            1 => 2,
            2 => 4,
            3 => 1,
            4 | 9 => 8,
            5 => 0x40,
            6 => 0x20,
            7 => 0x10,
            10 => {
                let difficulty_bits = mask & 0x3800;
                if difficulty_bits != 0 {
                    let bit = match self.difficulty {
                        0 => 0x800,
                        1 => 0x1000,
                        2 => 0x2000,
                        _ => 0,
                    };
                    if bit == 0 || difficulty_bits & bit == 0 {
                        return false;
                    }
                }
                match self.submode {
                    0 => 0x100,
                    1 => 0x200,
                    2 => 0x400,
                    _ => 0,
                }
            }
            _ => 0,
        };
        mode_bit != 0 && mask & mode_bit != 0
    }
}

impl ObjectiveRegistry {
    pub fn from_allocation(contract: Option<AllocationContract>) -> Result<Self, RegisterError> {
        let contract = contract.ok_or(RegisterError::MissingAllocationContract)?;
        if contract.record_slots == 0 || contract.creation_order_slots == 0 {
            return Err(RegisterError::InconsistentAllocation);
        }
        Ok(Self {
            records: alloc::vec![None; contract.record_slots],
            creation_order: Vec::with_capacity(contract.creation_order_slots),
            kind_counts: [0; 3],
            dependency_lists: alloc::vec![Vec::new(); contract.record_slots],
            capacity: contract.record_slots,
            creation_order_capacity: contract.creation_order_slots,
        })
    }

    /// Equivalent boundary to the `0x227E90` SETU walk and its `0x227750` row insertion.
    /// Filtering precedes writes; each accepted row commits record, order, then counter once.
    pub fn register(
        &mut self,
        definitions: &[ObjectiveDefinition],
        context: StoryContext,
    ) -> Result<usize, RegisterError> {
        let mut admitted = 0;
        for definition in definitions {
            if !context.allows(definition.predicate_mask) {
                continue;
            }
            let Some(&raw_kind) = definition.kind_by_submode.get(context.submode as usize) else {
                continue;
            };
            if raw_kind == NO_KIND || !context.allows(0x700) {
                continue;
            }
            let kind = u8::try_from(raw_kind).map_err(|_| RegisterError::InvalidKind)?;
            if kind >= 3 {
                return Err(RegisterError::InvalidKind);
            }
            let id = definition.id as usize;
            if id >= self.capacity || id >= self.records.len() {
                return Err(RegisterError::RecordOutOfBounds);
            }
            if self.creation_order.len() >= self.creation_order_capacity {
                return Err(RegisterError::CreationOrderFull);
            }
            let mut dependencies = Vec::new();
            for &dependency in &definition.dependencies {
                if dependency == u32::MAX {
                    break;
                }
                if !dependencies.contains(&dependency) {
                    dependencies.push(dependency);
                }
            }
            // Match observed insertion ordering: record, creation-order append, counter increment.
            self.records[id] = Some(ObjectiveRecord {
                id: definition.id,
                kind,
                status: definition.status,
                dependencies: dependencies.clone(),
            });
            self.dependency_lists[id] = dependencies;
            self.creation_order.push(definition.id);
            self.kind_counts[kind as usize] = self.kind_counts[kind as usize].saturating_add(1);
            admitted += 1;
        }
        debug_assert!(self.invariants_hold());
        Ok(admitted)
    }

    pub fn transition(&mut self, id: u32, status: ObjectiveStatus) -> bool {
        let Some(Some(record)) = self.records.get_mut(id as usize) else {
            return false;
        };
        record.status = status;
        true
    }

    pub fn invariants_hold(&self) -> bool {
        if self.capacity > self.records.len()
            || self.capacity > self.dependency_lists.len()
            || self.creation_order.len() > self.creation_order_capacity
        {
            return false;
        }
        let mut counts = [0u32; 3];
        for &id in &self.creation_order {
            let Some(Some(record)) = self.records.get(id as usize) else {
                return false;
            };
            if record.id != id || record.kind >= 3 {
                return false;
            }
            counts[record.kind as usize] = counts[record.kind as usize].saturating_add(1);
        }
        counts == self.kind_counts
    }
}

/// A/B entry point for callers at the Story registration boundary. `None` selects the lifted path.
pub fn register_selected(
    registry: &mut ObjectiveRegistry,
    definitions: &[ObjectiveDefinition],
    context: StoryContext,
) -> Result<Option<usize>, RegisterError> {
    if !enabled() {
        return Ok(None);
    }
    registry.register(definitions, context).map(Some)
}

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

/// Guest adoption remains opt-in and is intentionally inert until allocation evidence is supplied.
#[cfg(not(test))]
pub fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_OBJECTIVE_REGISTRY".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
pub fn enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> StoryContext {
        StoryContext {
            game_flags: 0,
            mode_id: 10,
            submode: 0,
            difficulty: 0,
            players: 1,
            network: false,
            descriptor_word: 0,
        }
    }

    fn definition(id: u32) -> ObjectiveDefinition {
        ObjectiveDefinition {
            id,
            kind_by_submode: [0, 1, 2],
            predicate_mask: 0x900,
            status: ObjectiveStatus::Active,
            dependencies: alloc::vec![4, 4, u32::MAX, 8],
        }
    }

    #[test]
    fn capacity_is_allocation_derived_and_missing_contract_fails_closed() {
        assert_eq!(
            ObjectiveRegistry::from_allocation(None),
            Err(RegisterError::MissingAllocationContract)
        );
        let registry = ObjectiveRegistry::from_allocation(Some(AllocationContract {
            record_slots: 5,
            creation_order_slots: 4,
        }))
        .unwrap();
        assert_eq!(registry.capacity, 5);
        assert_eq!(registry.creation_order_capacity, 4);
        assert_eq!(registry.records.len(), 5);
    }

    #[test]
    fn filtered_admission_preserves_order_and_counter_invariants() {
        let mut registry = ObjectiveRegistry::from_allocation(Some(AllocationContract {
            record_slots: 4,
            creation_order_slots: 4,
        }))
        .unwrap();
        let mut filtered = definition(0);
        filtered.predicate_mask = 0x200;
        assert_eq!(
            registry.register(&[definition(0), definition(1), filtered], context()),
            Ok(2)
        );
        assert_eq!(registry.creation_order, [0, 1]);
        assert_eq!(registry.kind_counts, [2, 0, 0]);
        assert_eq!(registry.dependency_lists[0], [4]);
        assert!(registry.invariants_hold());
    }

    #[test]
    fn record_capacity_boundary_is_rejected_without_partial_append() {
        let mut registry = ObjectiveRegistry::from_allocation(Some(AllocationContract {
            record_slots: 3,
            creation_order_slots: 4,
        }))
        .unwrap();
        let before = registry.clone();
        assert_eq!(
            registry.register(&[definition(3)], context()),
            Err(RegisterError::RecordOutOfBounds)
        );
        assert_eq!(registry, before);
    }

    #[test]
    fn status_transition_is_typed_and_does_not_change_counts() {
        let mut registry = ObjectiveRegistry::from_allocation(Some(AllocationContract {
            record_slots: 3,
            creation_order_slots: 3,
        }))
        .unwrap();
        registry.register(&[definition(1)], context()).unwrap();
        assert!(registry.transition(1, ObjectiveStatus::Inactive));
        assert_eq!(
            registry.records[1].as_ref().unwrap().status,
            ObjectiveStatus::Inactive
        );
        assert_eq!(registry.kind_counts, [1, 0, 0]);
        assert!(!registry.transition(9, ObjectiveStatus::Inactive));
    }

    #[test]
    fn switch_requires_explicit_rust_value() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }
}
