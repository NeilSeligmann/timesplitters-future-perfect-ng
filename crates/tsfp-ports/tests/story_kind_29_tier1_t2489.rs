// SPDX-License-Identifier: GPL-3.0-or-later
//! WEAK Tier 1: fabricated typed facts vs independent branch transcription.
#[path = "../src/port_scaffolds/story_kind_29_t2489.rs"]
mod rust_port;
use rust_port::PortState;

fn lifted_body(tag_exists: bool, scene_flag: u32) -> bool {
    // Independent transcription of the retained condition body: tag lookup wins, otherwise
    // compare the scene flag exactly with one.
    if tag_exists { true } else { scene_flag == 1 }
}

#[test]
fn tier1_lifted_differential_3000_seeded_facts_weak() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..3000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let tag = seed & 1 != 0;
        let flag = ((seed >> 1) % 4) as u32;
        let state = PortState {
            gas_tag_0x47415330_exists: tag,
            scene_flag_0x75bf0c: Some(flag),
        };
        assert_eq!(
            rust_port::port_step(&state),
            Some(lifted_body(tag, flag)),
            "seed={seed:#018x}"
        );
    }
}
