// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential scaffold for `docs/t-story-mode-flow.md#story_objective_save_snapshot_boundary_t2365_continuous_unknown`.
//! WEAK EVIDENCE: fabricated deterministic inputs; not an owner-recording replay.
//! Retained behavior transcription versus typed Rust logic; fabricated only.

#[path = "../src/port_scaffolds/story_objective_save_snapshot_boundary_t2365_continuous_unknown.rs"]
mod generated_port;
use generated_port::{ObjectiveCheckpoint, ObjectiveSlot, PortState};

fn lifted_body(state: &PortState) -> PortState {
    let mut out = state.clone();
    if let Some(cp) = &out.objectivecheckpoint {
        for slot in &mut out.objectiveslot {
            if !slot.present {
                continue;
            }
            if (slot.state == 1) != (cp.saved_state == 1) {
                if let Some(count) = out.class_counters.get_mut(slot.class as usize) {
                    *count += if cp.saved_state == 1 { 1 } else { -1 };
                }
            }
            slot.auxiliary_state = cp.saved_aux_state;
            slot.state = cp.saved_state;
        }
    }
    out
}

fn rust_port(state: &PortState) -> PortState {
    let mut out = state.clone();
    generated_port::port_step(&mut out);
    out
}

#[test]
fn tier1_lifted_differential_3000_seeds_weak() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..3000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let n = (seed as usize % 5) + 1;
        let slots = (0..n)
            .map(|i| ObjectiveSlot {
                class: ((seed >> (i * 3)) % 3) as u32,
                state: ((seed >> (i * 5 + 1)) % 5) as u32,
                auxiliary_state: ((seed >> (i * 7 + 2)) % 4) as u32,
                present: (seed >> (i * 9 + 3)) & 1 == 1,
            })
            .collect();
        let input = PortState {
            objectivecheckpoint: if seed & 7 == 0 {
                None
            } else {
                Some(ObjectiveCheckpoint {
                    saved_state: ((seed >> 11) % 5) as u32,
                    saved_aux_state: ((seed >> 14) % 4) as u32,
                })
            },
            objectiveslot: slots,
            class_counters: vec![3, 4, 5],
        };
        assert_eq!(rust_port(&input), lifted_body(&input), "seed={seed:#018x}");
    }
}
