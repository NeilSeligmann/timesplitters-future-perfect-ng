// SPDX-License-Identifier: GPL-3.0-or-later
//! T3415 readable Story reset relocation contract.
//!
//! The original callbacks are 0x234A50 (first player) and 0x234AC0 (second player).
//! They resolve a spawn key, request relocation with flags 1 or 5, then clear three
//! callback-specific context vectors. The second callback has the measured local-player,
//! object and extension guards. This model deliberately does not release camera control.
//!
//! Evidence: MEASURED-static REA in `docs/t1703-rea-objectives.md`, source-matched original
//! XBE evidence retained there. Tier 1 fixtures are fabricated and weak.

use core::sync::atomic::{AtomicBool, Ordering};
use std::collections::BTreeMap;

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpawnKey(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerSlot {
    #[default]
    First,
    Second,
}

impl PlayerSlot {
    pub const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlacementFlags(pub u32);

impl PlacementFlags {
    pub const NORMAL: Self = Self(1);
    pub const PRESERVE_INPUT_DRIVEN: Self = Self(5);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StoryResetEvent {
    pub spawn_key: SpawnKey,
    pub player_slot: PlayerSlot,
    pub placement_flags: PlacementFlags,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpawnPlacement {
    pub spawn_id: u32,
    pub position: [f32; 3],
    pub cell: u32,
    pub orientation: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpawnTable {
    rows: BTreeMap<SpawnKey, SpawnPlacement>,
}

impl SpawnTable {
    pub fn insert(&mut self, key: SpawnKey, row: SpawnPlacement) -> Option<SpawnPlacement> {
        self.rows.insert(key, row)
    }

    pub fn resolve(&self, key: SpawnKey) -> Option<SpawnPlacement> {
        self.rows.get(&key).copied()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerPlacement {
    pub spawn_key: SpawnKey,
    pub spawn_id: u32,
    pub position: [f32; 3],
    pub cell: u32,
    pub orientation: f32,
    pub flags: PlacementFlags,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraContext {
    /// The three callback-owned input/context floats zeroed by the original body.
    pub reset_vectors: [f32; 3],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerSlotState {
    pub object_present: bool,
    pub extension_present: bool,
    pub placement: PlayerPlacement,
    pub camera: CameraContext,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoryResetState {
    pub local_player_count: u32,
    pub slots: [Option<PlayerSlotState>; 2],
    pub spawns: SpawnTable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResetDisposition {
    Applied,
    GuardedOut,
}

/// Apply callback 32 (`0x234A50`). The original has no early player-count guard.
pub fn reset_first_player(
    event: StoryResetEvent,
    state: &mut StoryResetState,
) -> Option<ResetDisposition> {
    if event.player_slot != PlayerSlot::First {
        return None;
    }
    apply(event, state)
}

/// Apply callback 33 (`0x234AC0`), preserving its measured second-player guards.
pub fn reset_second_player(
    event: StoryResetEvent,
    state: &mut StoryResetState,
) -> Option<ResetDisposition> {
    if event.player_slot != PlayerSlot::Second {
        return None;
    }
    let Some(slot) = state.slots[PlayerSlot::Second.index()] else {
        return Some(ResetDisposition::GuardedOut);
    };
    if state.local_player_count <= 1 || !slot.object_present || !slot.extension_present {
        return Some(ResetDisposition::GuardedOut);
    }
    apply(event, state)
}

fn apply(event: StoryResetEvent, state: &mut StoryResetState) -> Option<ResetDisposition> {
    let row = state.spawns.resolve(event.spawn_key)?;
    let slot = state.slots[event.player_slot.index()].as_mut()?;
    slot.placement = PlayerPlacement {
        spawn_key: event.spawn_key,
        spawn_id: row.spawn_id,
        position: row.position,
        cell: row.cell,
        orientation: row.orientation,
        flags: event.placement_flags,
    };
    slot.camera.reset_vectors = [0.0; 3];
    Some(ResetDisposition::Applied)
}

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Default-off A/B switch. The host may opt in explicitly; a missing or ineligible
/// callback leaves its lifted fallback selected.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callback_contract_preserves_guards_relocation_and_vector_clearing() {
        let mut state = StoryResetState {
            local_player_count: 2,
            slots: [
                Some(PlayerSlotState {
                    object_present: true,
                    extension_present: true,
                    placement: PlayerPlacement::default(),
                    camera: CameraContext {
                        reset_vectors: [1.0, -2.0, 3.0],
                    },
                }),
                Some(PlayerSlotState {
                    object_present: true,
                    extension_present: true,
                    placement: PlayerPlacement::default(),
                    camera: CameraContext {
                        reset_vectors: [4.0, 5.0, 6.0],
                    },
                }),
            ],
            spawns: SpawnTable::default(),
        };
        state.spawns.insert(
            SpawnKey(1045),
            SpawnPlacement {
                spawn_id: 7,
                position: [10.0, 20.0, 30.0],
                cell: 4,
                orientation: 1.25,
            },
        );
        state.spawns.insert(
            SpawnKey(1502),
            SpawnPlacement {
                spawn_id: 9,
                position: [-1.0, 2.5, 3.0],
                cell: 8,
                orientation: -0.5,
            },
        );

        assert_eq!(
            reset_first_player(
                StoryResetEvent {
                    spawn_key: SpawnKey(1045),
                    player_slot: PlayerSlot::First,
                    placement_flags: PlacementFlags::NORMAL,
                },
                &mut state,
            ),
            Some(ResetDisposition::Applied)
        );
        assert_eq!(
            state.slots[0].unwrap().placement.position,
            [10.0, 20.0, 30.0]
        );
        assert_eq!(state.slots[0].unwrap().placement.flags, PlacementFlags(1));
        assert_eq!(state.slots[0].unwrap().camera.reset_vectors, [0.0; 3]);
        assert_eq!(
            state.slots[1].unwrap().camera.reset_vectors,
            [4.0, 5.0, 6.0]
        );

        assert_eq!(
            reset_second_player(
                StoryResetEvent {
                    spawn_key: SpawnKey(1502),
                    player_slot: PlayerSlot::Second,
                    placement_flags: PlacementFlags::PRESERVE_INPUT_DRIVEN,
                },
                &mut state,
            ),
            Some(ResetDisposition::Applied)
        );
        assert_eq!(state.slots[1].unwrap().placement.spawn_id, 9);
        assert_eq!(state.slots[1].unwrap().placement.flags, PlacementFlags(5));
        assert_eq!(state.slots[1].unwrap().camera.reset_vectors, [0.0; 3]);

        state.local_player_count = 1;
        let before = state.clone();
        assert_eq!(
            reset_second_player(
                StoryResetEvent {
                    spawn_key: SpawnKey(1502),
                    player_slot: PlayerSlot::Second,
                    placement_flags: PlacementFlags::NORMAL,
                },
                &mut state,
            ),
            Some(ResetDisposition::GuardedOut)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn opt_in_switch_is_closed_by_default_for_non_rust_values() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(!enabled_from(Some(b"true")));
        assert!(enabled_from(Some(b"rust")));
    }
}
