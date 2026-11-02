// SPDX-License-Identifier: GPL-3.0-or-later
//! Semantic mutation check for the measured kind-33 field predicate.

extern crate alloc;

#[path = "../src/port_scaffolds/story_kind_33_table_field_predicate_t2485.rs"]
mod generated_port;

#[test]
fn port_contract_mutants_are_killed() {
    let mut killed = false;
    for value in 0..4096u32 {
        let original = generated_port::table_field_0x18_is_true(value);
        let inverted_mutant = value == 0;
        if original != inverted_mutant {
            killed = true;
            break;
        }
    }
    assert!(killed, "the inverted zero/nonzero mutant must be rejected");
}
