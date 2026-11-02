// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 weak differential: 3,000 fabricated states against the retained lifted loop contract.
#[path = "../src/port_scaffolds/child_port_task_spec_unk_story_event_link_iteration_order.rs"]
mod generated_port;
use generated_port::{CallbackEffect, EventSlot, StoryEventPool, observe_active_slots};

fn lifted_reference(pool: &mut StoryEventPool, trigger: u32) -> Vec<(usize, EventSlot)> {
    // Retained 0x237E70 contract: entry-count/index snapshot, current record resolved per visit.
    let active_at_entry = pool.active_indices.clone();
    let identity_at_entry = pool.identity;
    let mut seen = Vec::with_capacity(active_at_entry.len());
    for (cursor, slot_index) in active_at_entry.into_iter().enumerate() {
        if pool.identity != identity_at_entry {
            break;
        }
        let Some(slot) = pool.slots.get(slot_index) else {
            break;
        };
        seen.push((slot_index, slot.clone()));
        let target = (slot_index + 1) % pool.slots.len();
        let value = trigger.wrapping_add(cursor as u32).wrapping_add(1);
        pool.slots[target].live_state = value;
    }
    seen
}

fn rust_port(pool: &mut StoryEventPool, trigger: u32) -> Vec<(usize, EventSlot)> {
    let mut cursor = 0u32;
    let slot_count = pool.slots.len();
    observe_active_slots(pool, |slot_index, _current| {
        let target = (slot_index + 1) % slot_count;
        let value = trigger.wrapping_add(cursor).wrapping_add(1);
        cursor = cursor.wrapping_add(1);
        Some(CallbackEffect::SetLiveState {
            slot: target,
            value,
        })
    })
    .expect("fabricated callback effect is supported")
}

#[test]
fn tier1_lifted_differential_3000_seeds_weak() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..3000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let slots = (seed as usize % 8) + 1;
        let mut input = StoryEventPool {
            identity: (seed >> 16) as u32,
            slots: (0..slots)
                .map(|i| EventSlot {
                    live_state: (seed as u32).wrapping_add(i as u32),
                    link_state: ((seed >> 8) as u32).rotate_left(i as u32),
                })
                .collect(),
            active_indices: (0..slots)
                .map(|i| (seed.rotate_left(i as u32) as usize) % slots)
                .collect(),
        };
        let mut expected = input.clone();
        let expected_seen = lifted_reference(&mut expected, seed as u32);
        let actual_seen = rust_port(&mut input, seed as u32);
        assert_eq!(input, expected, "final state seed={seed:#018x}");
        assert_eq!(
            actual_seen, expected_seen,
            "observation order seed={seed:#018x}"
        );
    }
}

#[test]
fn unsupported_callback_effect_is_retained_for_lifted_fallback() {
    let mut pool = StoryEventPool {
        identity: 7,
        slots: vec![EventSlot::default()],
        active_indices: vec![0],
    };
    let result = observe_active_slots(&mut pool, |_, _| {
        Some(CallbackEffect::Unsupported { effect: 0xDEAD })
    });
    assert_eq!(result, Err(0xDEAD));
}
