// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic semantic mutation checks for the T3387 occupancy contract.

#[path = "../src/port_scaffolds/story_objective_slot_reuse_generation.rs"]
mod port;

use port::{ObjectiveStatus, StoryObjectiveSlots};

#[test]
fn generation_cleanup_and_transition_mutants_are_detected() {
    // Mutant 1: reuse the old generation. The contract requires new identity.
    let mut slots = StoryObjectiveSlots::default();
    let first = slots.register(10, ObjectiveStatus::Pending, Some(0x11), Some(0x21));
    slots.complete(first, 7).unwrap();
    let reused = slots.register(11, ObjectiveStatus::Pending, Some(0x12), Some(0x22));
    let no_generation_increment_mutant = first.generation;
    assert_ne!(reused.generation, no_generation_increment_mutant);
    assert!(matches!(
        slots.get(first),
        Err(port::SlotError::StaleHandle)
    ));

    // Mutant 2: retain callback/link words after completion. The returned
    // dispatch is available for the caller, while no occupied slot retains it.
    let dispatch = slots.complete(reused, 9).unwrap();
    assert_eq!(dispatch.callback_handle, Some(0x12));
    assert_eq!(dispatch.link_handle, Some(0x22));
    let retain_handles_mutant = slots
        .occupied()
        .any(|(_, slot)| slot.callback_handle == Some(0x12) || slot.link_handle == Some(0x22));
    assert!(!retain_handles_mutant);

    // Mutant 3: permit an unsupported transition from failed to completion.
    let failed = slots.register(12, ObjectiveStatus::Failed, Some(0x13), None);
    let allow_failed_completion_mutant = slots.complete(failed, 10).is_ok();
    assert!(!allow_failed_completion_mutant);
    assert_eq!(
        slots.get(failed).unwrap().status,
        ObjectiveStatus::Failed,
        "rejected transition must leave the occupied state intact"
    );
}
