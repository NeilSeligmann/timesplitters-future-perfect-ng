// SPDX-License-Identifier: GPL-3.0-or-later
//! T3387 Tier 1 weak differential on fabricated states.
//!
//! This compares the typed store's projected status/payload behavior with the
//! retained lifted lifecycle transcription. It does not compare generation
//! identity (which is a native extension) or prove whole-system replay parity.

#[path = "../src/port_scaffolds/story_objective_slot_reuse_generation.rs"]
mod port;

use port::{ObjectiveStatus, StoryObjectiveSlots};

fn lifted_defer(status: u8) -> u8 {
    if matches!(status, 2 | 3) { 1 } else { status }
}

fn lifted_finish(status: u8, payload: u32) -> (u8, u32, bool) {
    let mut status = status;
    if status == 1 {
        status = 2;
    }
    let completed = matches!(status, 2 | 3);
    if completed {
        status = 5;
    }
    // The payload is the adapter's typed completion value; the reviewed lifted
    // completion projection carries it unchanged to the completion boundary.
    (status, payload, completed)
}

#[test]
fn tier1_lifted_lifecycle_projection_3000_fabricated_seeds_weak() {
    let mut seed = 0x3387_5eed_u64;
    for _ in 0..3_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let status = (seed as u8) % 8;
        let mut slots = StoryObjectiveSlots::default();
        let handle = slots.register(
            (seed >> 8) as u32,
            ObjectiveStatus::from_original(status),
            Some((seed >> 16) as u32),
            Some((seed >> 32) as u32),
        );

        let changed = slots.defer(handle).unwrap();
        let expected_status = lifted_defer(status);
        assert_eq!(changed, expected_status != status, "seed={seed:#018x}");
        assert_eq!(
            slots.get(handle).unwrap().status.to_original(),
            expected_status,
            "seed={seed:#018x}"
        );

        let payload = (seed >> 24) as u32;
        let (lifted_status, lifted_payload, completes) = lifted_finish(expected_status, payload);
        let result = slots.complete(handle, payload);
        if completes {
            let dispatch = result.unwrap();
            assert_eq!(dispatch.payload, lifted_payload, "seed={seed:#018x}");
            assert!(matches!(
                slots.get(handle),
                Err(port::SlotError::StaleHandle)
            ));
        } else {
            assert_eq!(result, Err(port::SlotError::NotCompletable));
            assert_eq!(
                slots.get(handle).unwrap().status.to_original(),
                lifted_status,
                "seed={seed:#018x}"
            );
        }
    }
}

#[test]
fn native_lane_is_explicit_and_default_off() {
    assert!(!port::NATIVE_OBJECTIVE_SLOTS_DEFAULT);
    assert!(!port::ObjectiveSlotLane::default().native_enabled());
    assert!(port::ObjectiveSlotLane::from_default_off_switch(true).native_enabled());
}
