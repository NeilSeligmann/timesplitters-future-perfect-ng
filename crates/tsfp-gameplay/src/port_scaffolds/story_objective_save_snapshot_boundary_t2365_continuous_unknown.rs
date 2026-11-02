// SPDX-License-Identifier: GPL-3.0-or-later
//! Generated from `docs/t-story-mode-flow.md#story_objective_save_snapshot_boundary_t2365_continuous_unknown`. Fill the logic; retain the evidence label.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectiveCheckpoint {
    pub saved_state: u32,
    pub saved_aux_state: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectiveSlot {
    /// Slot class comes from the live setup (+0x08); checkpoint restore does not replace it.
    pub class: u32,
    /// Restored from checkpoint state (+0x0b).
    pub state: u32,
    /// Restored from checkpoint auxiliary byte (+0x0a).
    pub auxiliary_state: u32,
    /// Whether this live slot participates in the saved checkpoint set.
    pub present: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PortState {
    pub objectivecheckpoint: Option<ObjectiveCheckpoint>,
    pub objectiveslot: Vec<ObjectiveSlot>,
    /// Active objective counts indexed by class, corresponding to 0x75C134 + class*4.
    pub class_counters: Vec<i32>,
}

/// Candidate lane roots found in the spec. Confirm root ABI before registration.
pub const LANE_ROOT_CANDIDATES: &[u32] = &[
    0x000B0EE0, 0x0017F780, 0x00227C90, 0x00227D60, 0x00227E90, 0x00233F90, 0x00237500, 0x0023EF70,
    0x0025EF80, 0x0025F010, 0x0025F0C0, 0x0075C134, 0x0075C140, 0x0075C150, 0x0075F5D0, 0x0075F5D8,
    0x0078CEC8,
];

pub fn lane_root_selected(va: u32) -> bool {
    LANE_ROOT_CANDIDATES.contains(&va)
}

/// Capture the two checkpoint bytes from a live objective slot.
pub fn capture(slot: &ObjectiveSlot) -> ObjectiveCheckpoint {
    ObjectiveCheckpoint {
        saved_state: slot.state,
        saved_aux_state: slot.auxiliary_state,
    }
}

pub fn set_checkpoint(state: &mut PortState, checkpoint: Option<ObjectiveCheckpoint>) {
    state.objectivecheckpoint = checkpoint;
}

/// Selectively merge saved state into live slots and reconcile active class counts.
/// Unsupported checkpoints are a no-op, matching the lifted fallback boundary.
pub fn port_step(state: &mut PortState) {
    let Some(checkpoint) = state.objectivecheckpoint.as_ref() else {
        return;
    };
    for slot in &mut state.objectiveslot {
        if !slot.present {
            continue;
        }
        let was_active = slot.state == 1;
        let becomes_active = checkpoint.saved_state == 1;
        if was_active != becomes_active {
            if let Some(counter) = state.class_counters.get_mut(slot.class as usize) {
                *counter += if becomes_active { 1 } else { -1 };
            }
        }
        slot.auxiliary_state = checkpoint.saved_aux_state;
        slot.state = checkpoint.saved_state;
    }
}
