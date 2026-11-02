// SPDX-License-Identifier: GPL-3.0-or-later
//! Mutation sensitivity for zero/nonzero callback result normalization.

#[path = "../src/port_scaffolds/verb_callback_dispatch_boundary_at_0x236099_as_a_typed_rust_borrowed_view_over_the_current_level_arena_event_instance.rs"]
mod port;
use port::{DispatchGate, dispatch_gate};

#[test]
fn nonzero_to_continue_mutant_is_killed() {
    let correct = dispatch_gate(2);
    let mutant = if 2 & 1 == 0 {
        DispatchGate::ContinueVerb
    } else {
        DispatchGate::SharedNoOp
    };
    assert_eq!(correct, DispatchGate::SharedNoOp);
    assert_ne!(mutant, correct);
}

#[test]
fn zero_to_noop_mutant_is_killed() {
    let correct = dispatch_gate(0);
    let mutant = DispatchGate::SharedNoOp;
    assert_eq!(correct, DispatchGate::ContinueVerb);
    assert_ne!(mutant, correct);
}
