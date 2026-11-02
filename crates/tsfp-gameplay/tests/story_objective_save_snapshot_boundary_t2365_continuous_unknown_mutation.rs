// SPDX-License-Identifier: GPL-3.0-or-later
//! Mutation-test stub for `docs/t-story-mode-flow.md#story_objective_save_snapshot_boundary_t2365_continuous_unknown`. Replace each named mutant with a real mutation.

#[path = "../src/port_scaffolds/story_objective_save_snapshot_boundary_t2365_continuous_unknown.rs"]
mod generated_port;

#[test]
fn port_contract_mutants_are_killed() {
    use generated_port::*;
    let input = PortState {
        objectivecheckpoint: Some(ObjectiveCheckpoint {
            saved_state: 1,
            saved_aux_state: 2,
        }),
        objectiveslot: vec![ObjectiveSlot {
            class: 1,
            state: 0,
            auxiliary_state: 9,
            present: true,
        }],
        class_counters: vec![0, 4],
    };
    let mut expected = input.clone();
    port_step(&mut expected);
    let mut no_counter = input.clone();
    no_counter.objectivecheckpoint.as_ref();
    no_counter.objectiveslot[0].state = 1;
    no_counter.objectiveslot[0].auxiliary_state = 2;
    assert_ne!(expected, no_counter, "counter transition mutant survives");
    let mut no_aux = input.clone();
    port_step(&mut no_aux);
    no_aux.objectiveslot[0].auxiliary_state = 9;
    assert_ne!(expected, no_aux, "auxiliary byte omission mutant survives");
    let mut unsupported = input;
    unsupported.objectivecheckpoint = None;
    let before = unsupported.clone();
    port_step(&mut unsupported);
    assert_eq!(
        unsupported, before,
        "unsupported checkpoint must be a no-op"
    );
}
