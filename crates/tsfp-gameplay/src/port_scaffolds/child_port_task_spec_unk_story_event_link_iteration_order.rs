// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed model of Story event active-list traversal and live slot observation.
//! Evidence is MEASURED-static from the T2368-v4 carrier; Tier 1 is fabricated and weak.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventSlot {
    pub live_state: u32,
    pub link_state: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoryEventPool {
    pub identity: u32,
    pub slots: Vec<EventSlot>,
    pub active_indices: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventPassCursor {
    pub pool_identity: u32,
    pub next_position: usize,
    entry_indices: Vec<usize>,
}

impl EventPassCursor {
    pub fn at_pass_entry(pool: &StoryEventPool) -> Self {
        Self {
            pool_identity: pool.identity,
            next_position: 0,
            entry_indices: pool.active_indices.clone(),
        }
    }

    /// Select one entry and advance before callback effects can mutate live state.
    pub fn next_slot(&mut self, pool: &StoryEventPool) -> Option<usize> {
        if self.pool_identity != pool.identity || self.next_position >= self.entry_indices.len() {
            return None;
        }
        let position = self.next_position;
        self.next_position += 1;
        self.entry_indices.get(position).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallbackEffect {
    SetLiveState { slot: usize, value: u32 },
    SetLinkState { slot: usize, value: u32 },
    Unsupported { effect: u32 },
}

/// Apply only supported callback effects to Rust-owned pool state.
pub fn apply_effect(pool: &mut StoryEventPool, effect: CallbackEffect) -> Result<(), u32> {
    match effect {
        CallbackEffect::SetLiveState { slot, value } => {
            let Some(slot) = pool.slots.get_mut(slot) else {
                return Err(u32::MAX);
            };
            slot.live_state = value;
            Ok(())
        }
        CallbackEffect::SetLinkState { slot, value } => {
            let Some(slot) = pool.slots.get_mut(slot) else {
                return Err(u32::MAX);
            };
            slot.link_state = value;
            Ok(())
        }
        CallbackEffect::Unsupported { effect } => Err(effect),
    }
}

/// Runs the selected entry sequence. Each visitor sees live state and cannot access the cursor.
pub fn observe_active_slots(
    pool: &mut StoryEventPool,
    mut visitor: impl FnMut(usize, &EventSlot) -> Option<CallbackEffect>,
) -> Result<Vec<(usize, EventSlot)>, u32> {
    let mut cursor = EventPassCursor::at_pass_entry(pool);
    let mut observed = Vec::with_capacity(cursor.entry_indices.len());
    while let Some(index) = cursor.next_slot(pool) {
        let Some(slot) = pool.slots.get(index) else {
            return Err(u32::MAX);
        };
        observed.push((index, slot.clone()));
        if let Some(effect) = visitor(index, slot) {
            apply_effect(pool, effect)?;
        }
    }
    Ok(observed)
}
