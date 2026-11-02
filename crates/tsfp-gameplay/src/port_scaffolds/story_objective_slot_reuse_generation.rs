// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned objective slot lifecycle candidate for T3387.
//!
//! The original path exposes byte states and untyped callback/link words. This
//! module keeps those at the adapter edge and uses generation-qualified
//! identities internally. It is not selected by the hybrid runtime by default.

/// Lifted lane remains the default until the subsystem adoption gate passes.
pub const NATIVE_OBJECTIVE_SLOTS_DEFAULT: bool = false;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ObjectiveStatus {
    #[default]
    Free,
    Pending,
    Active,
    Marked,
    Failed,
    Finished,
    Other(u8),
}

impl ObjectiveStatus {
    pub const fn from_original(value: u8) -> Self {
        match value {
            0 => Self::Free,
            1 => Self::Pending,
            2 => Self::Active,
            3 => Self::Marked,
            4 => Self::Failed,
            5 => Self::Finished,
            other => Self::Other(other),
        }
    }

    pub const fn to_original(self) -> u8 {
        match self {
            Self::Free => 0,
            Self::Pending => 1,
            Self::Active => 2,
            Self::Marked => 3,
            Self::Failed => 4,
            Self::Finished => 5,
            Self::Other(other) => other,
        }
    }
}

/// Stable identity for one occupancy of a slot. Generations start at one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectiveHandle {
    pub slot: usize,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectiveSlot {
    pub generation: u64,
    pub definition_key: u32,
    pub status: ObjectiveStatus,
    pub callback_handle: Option<u32>,
    pub link_handle: Option<u32>,
    pub completion_payload: Option<u32>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StoryObjectiveSlots {
    slots: Vec<SlotCell>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SlotCell {
    last_generation: u64,
    contents: Option<ObjectiveSlot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotError {
    StaleHandle,
    NotCompletable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionDispatch {
    pub callback_handle: Option<u32>,
    pub link_handle: Option<u32>,
    pub payload: u32,
}

impl StoryObjectiveSlots {
    /// Occupy the first free slot, preserving its generation history.
    pub fn register(
        &mut self,
        definition_key: u32,
        status: ObjectiveStatus,
        callback_handle: Option<u32>,
        link_handle: Option<u32>,
    ) -> ObjectiveHandle {
        let slot = self
            .slots
            .iter()
            .position(|cell| cell.contents.is_none())
            .unwrap_or_else(|| {
                self.slots.push(SlotCell::default());
                self.slots.len() - 1
            });
        let generation = next_generation(self.slots[slot].last_generation);
        self.slots[slot].last_generation = generation;
        self.slots[slot].contents = Some(ObjectiveSlot {
            generation,
            definition_key,
            status,
            callback_handle,
            link_handle,
            completion_payload: None,
        });
        ObjectiveHandle { slot, generation }
    }

    pub fn get(&self, handle: ObjectiveHandle) -> Result<&ObjectiveSlot, SlotError> {
        self.slots
            .get(handle.slot)
            .and_then(|cell| cell.contents.as_ref())
            .filter(|slot| slot.generation == handle.generation)
            .ok_or(SlotError::StaleHandle)
    }

    pub fn get_mut(&mut self, handle: ObjectiveHandle) -> Result<&mut ObjectiveSlot, SlotError> {
        self.slots
            .get_mut(handle.slot)
            .and_then(|cell| cell.contents.as_mut())
            .filter(|slot| slot.generation == handle.generation)
            .ok_or(SlotError::StaleHandle)
    }

    /// Apply the observed 2/3 -> 1 status transition without inventing other
    /// original state semantics. Returns false for any other current status.
    pub fn defer(&mut self, handle: ObjectiveHandle) -> Result<bool, SlotError> {
        let slot = self.get_mut(handle)?;
        if matches!(
            slot.status,
            ObjectiveStatus::Active | ObjectiveStatus::Marked
        ) {
            slot.status = ObjectiveStatus::Pending;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Complete and retire the exact occupancy. Handles are returned to the
    /// caller for dispatch, then cleared before this index can be reused.
    pub fn complete(
        &mut self,
        handle: ObjectiveHandle,
        payload: u32,
    ) -> Result<CompletionDispatch, SlotError> {
        let slot = self.get_mut(handle)?;
        if !matches!(
            slot.status,
            ObjectiveStatus::Pending | ObjectiveStatus::Active | ObjectiveStatus::Marked
        ) {
            return Err(SlotError::NotCompletable);
        }
        slot.status = ObjectiveStatus::Finished;
        slot.completion_payload = Some(payload);
        let dispatch = CompletionDispatch {
            callback_handle: slot.callback_handle,
            link_handle: slot.link_handle,
            payload,
        };
        // Preserve only generation history in the vacant entry. No stale
        // callback or link can be reached through the next occupancy.
        self.slots[handle.slot].contents = None;
        Ok(dispatch)
    }

    pub fn occupied(&self) -> impl Iterator<Item = (ObjectiveHandle, &ObjectiveSlot)> {
        self.slots.iter().enumerate().filter_map(|(slot, cell)| {
            cell.contents.as_ref().map(|contents| {
                (
                    ObjectiveHandle {
                        slot,
                        generation: contents.generation,
                    },
                    contents,
                )
            })
        })
    }
}

/// Increment with zero reserved for invalid/uninitialized handles.
const fn next_generation(generation: u64) -> u64 {
    let next = generation.wrapping_add(1);
    if next == 0 { 1 } else { next }
}

/// Adapter lane selector. Caller must keep `Lifted` selected until adoption.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ObjectiveSlotLane {
    #[default]
    Lifted,
    Native,
}

impl ObjectiveSlotLane {
    pub const fn from_default_off_switch(native_requested: bool) -> Self {
        if native_requested {
            Self::Native
        } else {
            Self::Lifted
        }
    }

    pub const fn native_enabled(self) -> bool {
        matches!(self, Self::Native)
    }
}
