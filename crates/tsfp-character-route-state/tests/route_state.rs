// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures; no retail data.
use std::collections::BTreeMap;
use tsfp_character_route_state::{
    ActorExtensionBinding, CharacterRouteSelectionState, NextRouteSelectionWord,
    PendingStateInvocation, RouteGoalWord, RouteProgramElapsed, RouteRecordWord,
};
use tsfp_state::{GuestMemory, character::*};

#[derive(Clone, PartialEq)]
struct Image(BTreeMap<u32, u8>);

impl GuestMemory for Image {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

const P0: u32 = 0x43a0_0000;
const P1: u32 = 0x43a1_0000;

fn image() -> Image {
    let mut memory = Image(BTreeMap::new());
    memory.write32(MEMBER_COUNT, 2);
    memory.write32(TABLE, P0);
    memory.write32(TABLE + 4, P1);
    for ptr in [P0, P1] {
        for word in 0..BODY_WORDS as u32 {
            memory.write32(ptr + word * 4, word ^ ptr);
        }
    }
    memory
}

#[test]
fn pending_state_invocation_defers_requests_written_after_entry() {
    let entry = PendingStateInvocation::default().next();
    let request_write = entry;
    let next_entry = entry.next();

    assert!(!entry.accepts_request_written_after(request_write));
    assert!(next_entry.accepts_request_written_after(request_write));
}

#[test]
fn actor_extension_binding_writes_measured_object_and_actor_slot_links() {
    let mut memory = image();
    memory.write32(TABLE + 4, P0);
    let before_left = memory.read32(P0 + 4);
    let before_right = memory.read32(P0 + 12);
    let mut set = CharacterSet::load(&memory);
    let binding = ActorExtensionBinding {
        object: CharacterObjectReference { raw: 0x1234_5000 },
        actor_slot: 7,
    };

    assert!(set.bind_actor_extension(&mut memory, 0, binding));
    assert_eq!(set.object_reference(0), Some(binding.object));
    assert_eq!(set.index_word(0), Some(CharacterIndexWord { raw: 7 }));
    assert_eq!(memory.read32(P0), binding.object.raw);
    assert_eq!(memory.read32(P0 + 8), 7);
    assert_eq!(memory.read32(P0 + 4), before_left);
    assert_eq!(memory.read32(P0 + 12), before_right);
}

#[test]
fn actor_extension_binding_rejects_missing_row_without_mutation() {
    let mut memory = image();
    let mut set = CharacterSet::load(&memory);
    let before = memory.clone();
    assert!(!set.bind_actor_extension(
        &mut memory,
        4,
        ActorExtensionBinding {
            object: CharacterObjectReference { raw: 0x1234_5000 },
            actor_slot: 7,
        },
    ));
    assert!(memory == before);
}

#[test]
fn selected_actor_retirement_fields_clear_measured_words_and_preserve_conditional_state() {
    let mut memory = image();
    memory.write32(TABLE + 4, P0);
    let mut set = CharacterSet::load(&memory);
    let preserved = [0x08, 0x0c, 0x20, 0x674, 0x67c, 0x9bc, 0x9c4, 0xbf8, 0xc08];
    let before: Vec<_> = preserved
        .iter()
        .map(|offset| memory.read32(P0 + *offset))
        .collect();

    assert!(set.clear_selected_actor_retirement_fields(&mut memory, 1));

    for offset in [0x00, 0x04, 0x10, 0x14, 0x18, 0x1c, 0x9c0, 0xbf0, 0xbf4] {
        assert_eq!(memory.read32(P0 + offset), 0, "offset {offset:#x}");
    }
    assert_eq!(memory.read32(P0 + 0x678), u32::MAX);
    for (offset, value) in preserved.iter().zip(before) {
        assert_eq!(memory.read32(P0 + *offset), value, "offset {offset:#x}");
    }
    assert_eq!(set.body_word(0, 0x678), u32::MAX);
    assert_eq!(set.body_word(1, 0x678), u32::MAX);
    assert_eq!(memory.read32(P0 + 0x678), set.body_word(1, 0x678));
}

#[test]
fn selected_actor_retirement_fields_reject_missing_row_without_mutation() {
    let mut memory = image();
    let before = memory.clone();
    let mut set = CharacterSet::load(&memory);

    assert!(!set.clear_selected_actor_retirement_fields(&mut memory, 4));

    assert!(memory == before);
}

#[test]
fn next_selection_writer_updates_owned_aliases_and_preserves_neighbors() {
    let mut memory = image();
    memory.write32(TABLE + 4, P0);
    let left = memory.read32(P0 + 0x67c);
    let right = memory.read32(P0 + 0x684);
    let mut set = CharacterSet::load(&memory);
    let word = NextRouteSelectionWord { raw: 0xdead_beef };

    assert!(set.set_next_route_selection_word(&mut memory, 1, word));
    assert_eq!(set.next_route_selection_word(0), Some(word));
    assert_eq!(set.next_route_selection_word(1), Some(word));
    assert_eq!(memory.read32(P0 + 0x680), word.raw);
    assert_eq!(memory.read32(P0 + 0x67c), left);
    assert_eq!(memory.read32(P0 + 0x684), right);
}

#[test]
fn next_selection_writer_rejects_missing_slots_without_mutation() {
    let mut memory = image();
    let mut set = CharacterSet::load(&memory);
    let before = memory.clone();
    let word = NextRouteSelectionWord { raw: 9 };

    assert!(!set.set_next_route_selection_word(&mut memory, 4, word));
    assert!(memory == before);
    assert_eq!(set.next_route_selection_word(4), None);
}

#[test]
fn route_goal_writer_updates_owned_aliases_and_preserves_neighbors() {
    let mut memory = image();
    memory.write32(TABLE + 4, P0);
    let left = memory.read32(P0 + 0x680);
    let right = memory.read32(P0 + 0x688);
    let mut set = CharacterSet::load(&memory);
    let word = RouteGoalWord { raw: 0xcafe_babe };

    assert!(set.set_route_goal_word(&mut memory, 1, word));
    assert_eq!(set.route_goal_word(0), Some(word));
    assert_eq!(set.route_goal_word(1), Some(word));
    assert_eq!(memory.read32(P0 + 0x684), word.raw);
    assert_eq!(memory.read32(P0 + 0x680), left);
    assert_eq!(memory.read32(P0 + 0x688), right);
}

#[test]
fn route_goal_writer_rejects_missing_slots_without_mutation() {
    let mut memory = image();
    let mut set = CharacterSet::load(&memory);
    let before = memory.clone();
    let word = RouteGoalWord { raw: 9 };

    assert!(!set.set_route_goal_word(&mut memory, 4, word));
    assert!(memory == before);
    assert_eq!(set.route_goal_word(4), None);
}

#[test]
fn route_record_writer_updates_owned_aliases_and_preserves_neighbors() {
    let mut memory = image();
    memory.write32(TABLE + 4, P0);
    let left = memory.read32(P0 + 0x674);
    let next = memory.read32(P0 + 0x680);
    let goal = memory.read32(P0 + 0x684);
    let mut set = CharacterSet::load(&memory);
    let word = RouteRecordWord { raw: 0x7654_3210 };

    assert!(set.set_route_record_word(&mut memory, 1, word));
    assert_eq!(set.route_record_word(0), Some(word));
    assert_eq!(set.route_record_word(1), Some(word));
    assert_eq!(memory.read32(P0 + 0x678), word.raw);
    assert_eq!(memory.read32(P0 + 0x674), left);
    assert_eq!(memory.read32(P0 + 0x680), next);
    assert_eq!(memory.read32(P0 + 0x684), goal);
}

#[test]
fn route_record_writer_rejects_missing_slots_without_mutation() {
    let mut memory = image();
    let mut set = CharacterSet::load(&memory);
    let before = memory.clone();
    let word = RouteRecordWord { raw: 9 };

    assert!(!set.set_route_record_word(&mut memory, 4, word));
    assert!(memory == before);
    assert_eq!(set.route_record_word(4), None);
}

#[test]
fn route_program_elapsed_adds_scaled_delta_and_updates_aliases() {
    let mut memory = image();
    memory.write32(TABLE + 4, P0);
    memory.write32(P0 + 0xb30, 2.5f32.to_bits());
    let neighbor = memory.read32(P0 + 0xb34);
    let mut set = CharacterSet::load(&memory);

    assert_eq!(
        set.advance_route_program_elapsed(&mut memory, 1, 0.25),
        Some(RouteProgramElapsed(2.75))
    );
    assert_eq!(
        set.route_program_elapsed(0),
        Some(RouteProgramElapsed(2.75))
    );
    assert_eq!(memory.read32(P0 + 0xb30), 2.75f32.to_bits());
    assert_eq!(memory.read32(P0 + 0xb34), neighbor);
    let before = memory.clone();
    assert_eq!(set.advance_route_program_elapsed(&mut memory, 4, 1.0), None);
    assert!(memory == before);
    assert_eq!(set.route_program_elapsed(4), None);
}

#[test]
fn route_refresh_commit_resets_reverse_counts_only_when_current_changes() {
    let mut memory = image();
    memory.write32(TABLE + 4, P0);
    for (offset, value) in [(0x678, 7), (0x680, 8), (0x684, 9), (0x9a8, 3), (0x9ac, 4)] {
        memory.write32(P0 + offset, value);
    }
    let neighbor = memory.read32(P0 + 0x9b0);
    let mut set = CharacterSet::load(&memory);

    assert!(set.commit_route_refresh(&mut memory, 1, RouteRecordWord { raw: 7 }, None,));
    assert_eq!(memory.read32(P0 + 0x9a8), 3);
    assert_eq!(memory.read32(P0 + 0x9ac), 4);
    assert_eq!(memory.read32(P0 + 0x680), 8);

    assert!(set.commit_route_refresh(
        &mut memory,
        0,
        RouteRecordWord { raw: 11 },
        Some(NextRouteSelectionWord { raw: 12 }),
    ));
    assert_eq!(set.route_record_word(1), Some(RouteRecordWord { raw: 11 }));
    assert_eq!(
        set.next_route_selection_word(1),
        Some(NextRouteSelectionWord { raw: 12 })
    );
    assert_eq!(memory.read32(P0 + 0x678), 11);
    assert_eq!(memory.read32(P0 + 0x680), 12);
    assert_eq!(memory.read32(P0 + 0x9a8), 0);
    assert_eq!(memory.read32(P0 + 0x9ac), 0);
    assert_eq!(memory.read32(P0 + 0x684), 9);
    assert_eq!(memory.read32(P0 + 0x9b0), neighbor);

    let before = memory.clone();
    assert!(!set.commit_route_refresh(&mut memory, 4, RouteRecordWord { raw: 1 }, None,));
    assert!(memory == before);
}
