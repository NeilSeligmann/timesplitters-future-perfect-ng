// SPDX-License-Identifier: GPL-3.0-or-later
//! Mutation sensitivity for cursor advancement and live later-slot observations.
#[path = "../src/port_scaffolds/child_port_task_spec_unk_story_event_link_iteration_order.rs"]
mod generated_port;
use generated_port::{CallbackEffect, EventSlot, StoryEventPool, observe_active_slots};

fn correct(pool: &mut StoryEventPool) -> Vec<(usize, EventSlot)> {
    observe_active_slots(pool, |index, _| {
        (index == 0).then_some(CallbackEffect::SetLiveState { slot: 1, value: 91 })
    })
    .unwrap()
}

fn restarted_cursor(pool: &mut StoryEventPool) -> Vec<(usize, EventSlot)> {
    let mut cursor = generated_port::EventPassCursor::at_pass_entry(pool);
    let mut seen = Vec::new();
    while let Some(index) = cursor.next_slot(pool) {
        let Some(slot) = pool.slots.get(index) else {
            break;
        };
        seen.push((index, slot.clone()));
        if let Some(effect) =
            (index == 0).then_some(CallbackEffect::SetLiveState { slot: 1, value: 91 })
        {
            generated_port::apply_effect(pool, effect).unwrap();
        }
        // Mutation: restart instead of advancing the stable enclosing cursor.
        if cursor.next_position > 1 {
            cursor.next_position = 1;
            break;
        }
    }
    seen
}

fn stale_slot_snapshot(pool: &mut StoryEventPool) -> Vec<(usize, EventSlot)> {
    let at_entry = pool.slots.clone();
    let indices = pool.active_indices.clone();
    let mut seen = Vec::new();
    for index in indices {
        seen.push((index, at_entry[index].clone()));
        if index == 0 {
            pool.slots[1].live_state = 91;
        }
    }
    seen
}

#[test]
fn cursor_and_live_state_mutations_are_killed() {
    let make_pool = || StoryEventPool {
        identity: 1,
        slots: vec![
            EventSlot::default(),
            EventSlot::default(),
            EventSlot::default(),
        ],
        active_indices: vec![0, 1, 2],
    };
    let mut baseline_pool = make_pool();
    let baseline = correct(&mut baseline_pool);
    let mut cursor_pool = make_pool();
    let mut stale_pool = make_pool();
    assert_ne!(restarted_cursor(&mut cursor_pool), baseline);
    assert_ne!(stale_slot_snapshot(&mut stale_pool), baseline);
}
