// SPDX-License-Identifier: GPL-3.0-or-later
//! Small semantic mutation checks for objective registry boundary and transition invariants.

extern crate alloc;

#[path = "../src/port_scaffolds/story_objective_registry_admission_slice.rs"]
mod port;
use port::{
    AllocationContract, ObjectiveDefinition, ObjectiveRegistry, ObjectiveStatus, RegisterError,
    StoryContext,
};

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
fn def(id: u32) -> ObjectiveDefinition {
    ObjectiveDefinition {
        id,
        kind_by_submode: [1, 1, 1],
        predicate_mask: 0x100,
        status: ObjectiveStatus::Active,
        dependencies: alloc::vec![],
    }
}

#[test]
fn capacity_boundary_mutant_is_killed() {
    let mut registry = ObjectiveRegistry::from_allocation(Some(AllocationContract {
        record_slots: 2,
        creation_order_slots: 2,
    }))
    .unwrap();
    let before = registry.clone();
    assert_eq!(
        registry.register(&[def(2)], context()),
        Err(RegisterError::RecordOutOfBounds)
    );
    assert_eq!(registry, before);
    // The mutant `id > capacity` would accept id == capacity and diverge from the contract.
    let mut mutant = ObjectiveRegistry::from_allocation(Some(AllocationContract {
        record_slots: 3,
        creation_order_slots: 3,
    }))
    .unwrap();
    mutant.records[2] = Some(port::ObjectiveRecord {
        id: 2,
        kind: 1,
        status: ObjectiveStatus::Active,
        dependencies: vec![],
    });
    assert_ne!(mutant, before);
}

#[test]
fn counter_and_order_mutants_are_killed() {
    let mut correct = ObjectiveRegistry::from_allocation(Some(AllocationContract {
        record_slots: 3,
        creation_order_slots: 3,
    }))
    .unwrap();
    correct.register(&[def(1)], context()).unwrap();
    let mut missing_counter = correct.clone();
    missing_counter.kind_counts[1] = 0;
    assert!(!missing_counter.invariants_hold());
    let mut reordered = correct.clone();
    reordered.creation_order[0] = 0;
    assert!(!reordered.invariants_hold());
}

#[test]
fn omitted_status_transition_mutant_is_killed() {
    let mut correct = ObjectiveRegistry::from_allocation(Some(AllocationContract {
        record_slots: 3,
        creation_order_slots: 3,
    }))
    .unwrap();
    correct.register(&[def(1)], context()).unwrap();
    assert!(correct.transition(1, ObjectiveStatus::Inactive));
    let mut mutant = correct.clone();
    mutant.records[1].as_mut().unwrap().status = ObjectiveStatus::Active;
    assert_ne!(mutant, correct);
}
