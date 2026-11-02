// SPDX-License-Identifier: GPL-3.0-or-later
//! Mutation sensitivity for the Story kind-29 typed predicate.
#[path = "../src/port_scaffolds/story_kind_29_t2489.rs"]
mod rust_port;
use rust_port::PortState;

#[test]
fn mutation_detects_wrong_scene_flag_comparator() {
    let state = PortState {
        gas_tag_0x47415330_exists: false,
        scene_flag_0x75bf0c: Some(2),
    };
    let expected = rust_port::port_step(&state);
    let mutant = state.gas_tag_0x47415330_exists || state.scene_flag_0x75bf0c == Some(2);
    assert_eq!(expected, Some(false));
    assert_ne!(expected, Some(mutant));
}

#[test]
fn unknown_scene_fact_declines_when_tag_is_absent() {
    let state = PortState {
        gas_tag_0x47415330_exists: false,
        scene_flag_0x75bf0c: None,
    };
    assert_eq!(rust_port::port_step(&state), None);
}
