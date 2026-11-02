// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use std::collections::BTreeMap;
use tsfp_state::GuestMemory;
use tsfp_state::character::*;
use tsfp_state::generated_views::TsfpCharacterView;

#[derive(Clone)]
struct Image(BTreeMap<u32, u8>);
/// Absent bytes read as zero, so equality ignores explicit zero entries.
impl PartialEq for Image {
    fn eq(&self, o: &Self) -> bool {
        let nz = |m: &Image| {
            m.0.iter()
                .filter(|(_, v)| **v != 0)
                .map(|(k, v)| (*k, *v))
                .collect::<Vec<_>>()
        };
        nz(self) == nz(o)
    }
}
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}

const P0: u32 = 0x43a0_0000;
const P1: u32 = 0x43a1_0000;
const P3: u32 = 0x43a4_0000;
const PNEW: u32 = 0x43b0_0000;
const OBJECT: u32 = 0x42b0_0000;

fn image() -> Image {
    let mut m = Image(BTreeMap::new());
    for (slot, ptr) in [(0u32, P0), (1, P1), (3, P3)] {
        m.write32(TABLE + 4 * slot, ptr);
        for i in 0..BODY_WORDS as u32 {
            m.write32(
                ptr + i * 4,
                i.wrapping_mul(0x9e37_79b1) ^ slot ^ 0x1234_5678,
            );
        }
    }
    m.write32(MEMBER_COUNT, 4);
    m
}

fn alias_slot_4(m: &mut Image, ptr: u32) {
    m.write32(TABLE + 4 * 4, ptr);
    m.write32(MEMBER_COUNT, 5);
}

/// Offsets written by lifted code (T2057 attribution table, family char).
fn csv_offsets() -> Vec<u32> {
    let csv = include_str!("../../../docs/data/t2057-write-attribution.csv");
    let mut v: Vec<u32> = csv
        .lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            (c[2] == "char")
                .then(|| u32::from_str_radix(c[3].trim_start_matches("0x"), 16).unwrap())
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn round_trip_is_byte_identical() {
    let m = image();
    let set = CharacterSet::load(&m);
    assert_eq!(set.live(), 4);
    assert_eq!(set.diff(&m), None);
    let mut changed_count = m.clone();
    changed_count.write32(MEMBER_COUNT, 2);
    assert_eq!(set.diff(&changed_count), Some(CharDiff::Count));
    let mut copy = m.clone();
    set.store(&mut copy);
    assert!(copy == m);
    let mut blank = Image(BTreeMap::new());
    set.store(&mut blank);
    assert_eq!(CharacterSet::load(&blank), set);
}

#[test]
fn store_touches_only_declared_extents() {
    let mut m = image();
    m.write32(P0 + BODY_SIZE, 0xfeed_beef);
    m.write32(P3 + BODY_SIZE, 0xcafe_f00d);
    let before = m.clone();
    CharacterSet::load(&m).store(&mut m);
    assert!(m == before);
}

#[test]
fn setters_write_through_and_typed_view_agrees() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    assert!(set.set_state(&mut m, 1, 7));
    assert!(set.set_pending_state(&mut m, 1, -3));
    assert!(set.set_transition_time(&mut m, 1, 0.25));
    assert!(set.set_aim_point(&mut m, 3, [1.0, 2.0, -3.5]));
    assert!(set.set_melee_target(&mut m, 0, P1));
    let goal_flags = CharacterFlagWords {
        low: 0x1234_5678,
        high: 0x9abc_def0,
    };
    let pending_flags = CharacterFlagWords {
        low: 0x55aa_55aa,
        high: 0xaa55_aa55,
    };
    assert!(set.set_goal_flags(&mut m, 1, goal_flags));
    assert!(set.set_pending_flags(&mut m, 1, pending_flags));
    assert!(set.set_field(&mut m, 0, 0x48, -2i16));
    assert!(set.set_field(&mut m, 0, 0x28, 0x80u8));
    assert_eq!(m.read32(P1 + 0x20), 7);
    assert_eq!(m.read32(P1 + 0x9d0), (-3i32) as u32);
    assert_eq!(set.goal_flags(1), Some(goal_flags));
    assert_eq!(set.pending_flags(1), Some(pending_flags));
    assert_eq!(m.read32(P1 + 0xaa8), goal_flags.low);
    assert_eq!(m.read32(P1 + 0xaac), goal_flags.high);
    assert_eq!(m.read32(P1 + 0xab8), pending_flags.low);
    assert_eq!(m.read32(P1 + 0xabc), pending_flags.high);
    assert_eq!(m.read32(P3 + 0xbb8), 2.0f32.to_bits());
    assert_eq!(set.diff(&m), None);
    // the owned typed record equals the typed view over the guest image
    for slot in [0usize, 1, 3] {
        let from_owned = set.owned(slot).unwrap();
        let from_guest = TsfpCharacterView(set.ptrs[slot]).load(&m);
        assert_eq!(from_owned, from_guest, "slot {slot}");
    }
    let o = set.owned(0).unwrap();
    assert_eq!(
        (o.model_selector, o.flags_28, o.melee_target.0),
        (-2, 0x80, P1)
    );
    // a neighbouring byte of a narrow field is left alone
    assert_eq!(m.read8(P0 + 0x29), image().read8(P0 + 0x29));
    assert_eq!(set.slot_of(P3), Some(3));
    assert_eq!(set.slot_of(0), None);
}

#[test]
fn setters_reject_invalid_slots_without_mutation() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();

    assert!(!set.set_word(&mut m, SLOTS, 0x20, 1));
    assert!(!set.set_field(&mut m, SLOTS, 0x20, 1u32));
    assert!(!set.set_state(&mut m, SLOTS, 1));
    assert!(!set.set_goal_flags(&mut m, SLOTS, CharacterFlagWords { low: 1, high: 2 }));
    assert!(!set.set_pending_flags(&mut m, SLOTS, CharacterFlagWords { low: 3, high: 4 }));

    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn pending_flag_handler_receives_raw_pair_before_owned_and_guest_clear() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let pending = CharacterFlagWords {
        low: 0x0116_0000,
        high: 1,
    };
    assert!(set.set_pending_flags(&mut m, 1, pending));
    assert!(set.set_word(&mut m, 1, 0xab4, 0xfeed_beef));
    assert!(set.set_word(&mut m, 1, 0xac0, 0xcafe_f00d));

    let applied = set.apply_pending_flag_pair(&mut m, 1, |flags| {
        (flags.low, flags.high, flags.low ^ flags.high)
    });

    assert_eq!(
        applied,
        Some((pending.low, pending.high, pending.low ^ pending.high))
    );
    assert_eq!(set.pending_flags(1), Some(CharacterFlagWords::default()));
    assert_eq!(m.read32(P1 + 0xab8), 0);
    assert_eq!(m.read32(P1 + 0xabc), 0);
    assert_eq!(m.read32(P1 + 0xab4), 0xfeed_beef);
    assert_eq!(m.read32(P1 + 0xac0), 0xcafe_f00d);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn pending_flag_application_uses_selected_pair_and_clears_owned_words() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let pending = CharacterFlagWords {
        low: 0x0116_0000,
        high: 1,
    };
    assert!(set.set_pending_flags(&mut m, 1, pending));
    assert!(set.set_word(&mut m, 1, 0xab4, 0xfeed_beef));
    assert!(set.set_word(&mut m, 1, 0xac0, 0xcafe_f00d));

    let applied = set.apply_pending_flag_pair_or_default(&mut m, 1, true, |flags| flags);

    assert_eq!(applied, Some(pending));
    assert_eq!(set.pending_flags(1), Some(CharacterFlagWords::default()));
    assert_eq!(m.read32(P1 + 0xab8), 0);
    assert_eq!(m.read32(P1 + 0xabc), 0);
    assert_eq!(m.read32(P1 + 0xab4), 0xfeed_beef);
    assert_eq!(m.read32(P1 + 0xac0), 0xcafe_f00d);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn pending_flag_application_uses_measured_default_pair_when_selected() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let pending = CharacterFlagWords {
        low: 0x2468_ace0,
        high: 0x1357_9bdf,
    };
    assert!(set.set_pending_flags(&mut m, 1, pending));

    let applied = set.apply_pending_flag_pair_or_default(&mut m, 1, false, |flags| flags);

    assert_eq!(applied, Some(CharacterFlagWords::DEFAULT_APPLICATION));
    assert_eq!(set.pending_flags(1), Some(CharacterFlagWords::default()));
    assert_eq!(m.read32(P1 + 0xab8), 0);
    assert_eq!(m.read32(P1 + 0xabc), 0);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn pending_flag_application_uses_measured_mask_and_default_branch() {
    for bit in [0x0100_0000, 0x0010_0000, 0x0004_0000, 0x0002_0000] {
        let mut m = image();
        let mut set = CharacterSet::load(&m);
        let pending = CharacterFlagWords {
            low: bit,
            high: 0x1357_9bdf,
        };
        assert!(set.set_pending_flags(&mut m, 1, pending));

        let applied = set.apply_pending_flag_pair_for_masked_branch(&mut m, 1, |flags| flags);

        assert_eq!(applied, Some(pending), "mask bit {bit:#010x}");
        assert_eq!(set.pending_flags(1), Some(CharacterFlagWords::default()));
        assert_eq!(m.read32(P1 + 0xab8), 0);
        assert_eq!(m.read32(P1 + 0xabc), 0);
        assert_eq!(set.diff(&m), None);
    }

    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let pending = CharacterFlagWords {
        low: 0x0080_0000,
        high: 0x2468_ace0,
    };
    assert!(set.set_pending_flags(&mut m, 1, pending));

    let applied = set.apply_pending_flag_pair_for_masked_branch(&mut m, 1, |flags| flags);

    assert_eq!(applied, Some(CharacterFlagWords::DEFAULT_APPLICATION));
    assert_eq!(set.pending_flags(1), Some(CharacterFlagWords::default()));
    assert_eq!(m.read32(P1 + 0xab8), 0);
    assert_eq!(m.read32(P1 + 0xabc), 0);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn pending_flag_handler_skips_absent_and_invalid_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let mut called = false;

    assert_eq!(
        set.apply_pending_flag_pair(&mut m, 2, |_| {
            called = true;
        }),
        None
    );
    assert_eq!(
        set.apply_pending_flag_pair(&mut m, SLOTS, |_| {
            called = true;
        }),
        None
    );
    assert_eq!(
        set.apply_pending_flag_pair_or_default(&mut m, SLOTS, false, |_| {
            called = true;
        }),
        None
    );
    assert_eq!(
        set.apply_pending_flag_pair_for_masked_branch(&mut m, SLOTS, |_| {
            called = true;
        }),
        None
    );

    assert!(!called);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn tick_state_writer_commits_the_complete_typed_snapshot() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let state = CharacterTickState {
        current: 23,
        pending: -2,
        previous: 17,
        transition_time: 0.625,
    };

    assert!(set.set_tick_state(&mut m, 1, state));
    assert_eq!(set.tick_state(1), Some(state));
    assert_eq!(m.read32(P1 + 0x20), state.current);
    assert_eq!(m.read32(P1 + 0x9d0), state.pending as u32);
    assert_eq!(m.read32(P1 + 0x9d4), state.previous);
    assert_eq!(m.read32(P1 + 0x9d8), state.transition_time.to_bits());
    assert_eq!(set.diff(&m), None);
}

#[test]
fn tick_state_writer_rejects_absent_and_out_of_range_slots_without_mutation() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let state = CharacterTickState {
        current: 23,
        pending: 4,
        previous: 17,
        transition_time: 0.625,
    };

    assert!(!set.set_tick_state(&mut m, 2, state));
    assert!(!set.set_tick_state(&mut m, SLOTS, state));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn behavior_tick_state_writer_round_trips_controller_and_actor_context() {
    let mut m = image();
    m.write8(P1 + TsfpCharacterView::OFFSET_FLAGS_2C + 1, 0xd3);
    let mut set = CharacterSet::load(&m);
    let state = CharacterBehaviorTickState {
        controller: CharacterTickState {
            current: 23,
            pending: -2,
            previous: 17,
            transition_time: 0.625,
        },
        byte_2c: 0x5a,
        word_a90: 0x7654_3210,
    };

    assert!(set.set_behavior_tick_state(&mut m, 1, state));
    assert_eq!(set.behavior_tick_state(1), Some(state));
    assert_eq!(m.read32(P1 + 0x20), state.controller.current);
    assert_eq!(m.read8(P1 + 0x2c), state.byte_2c);
    assert_eq!(m.read8(P1 + 0x2d), 0xd3);
    assert_eq!(m.read32(P1 + 0xa90), state.word_a90);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn behavior_tick_state_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let state = CharacterBehaviorTickState {
        controller: CharacterTickState {
            current: 23,
            pending: 4,
            previous: 17,
            transition_time: 0.625,
        },
        byte_2c: 0x5a,
        word_a90: 0x7654_3210,
    };

    assert_eq!(set.behavior_tick_state(2), None);
    assert_eq!(set.behavior_tick_state(SLOTS), None);
    assert!(!set.set_behavior_tick_state(&mut m, 2, state));
    assert!(!set.set_behavior_tick_state(&mut m, SLOTS, state));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn behavior_flags_writer_preserves_opaque_bits_neighbors_and_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P1);
    for (offset, value) in [(0x2b, 0x11), (0x2d, 0x22), (0x2e, 0x33), (0x2f, 0x44)] {
        m.write8(P1 + offset, value);
    }
    let mut set = CharacterSet::load(&m);
    let flags = CharacterBehaviorFlags { raw: 0xf4 };

    assert!(flags.skip_pending_application());
    assert!(flags.accumulator_control_bit());
    assert_eq!(flags.with_skip_pending_application(false).raw, 0xf0);
    assert_eq!(flags.with_accumulator_control_bit(false).raw, 0x74);
    assert!(set.set_behavior_flags(&mut m, 1, flags));
    assert_eq!(set.behavior_flags(1), Some(flags));
    assert_eq!(set.behavior_flags(4), Some(flags));
    assert_eq!(m.read8(P1 + 0x2c), 0xf4);
    for (offset, value) in [(0x2b, 0x11), (0x2d, 0x22), (0x2e, 0x33), (0x2f, 0x44)] {
        assert_eq!(m.read8(P1 + offset), value);
    }
    assert_eq!(set.diff(&m), None);
}

#[test]
fn behavior_flags_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let flags = CharacterBehaviorFlags { raw: 0x84 };

    assert_eq!(set.behavior_flags(2), None);
    assert_eq!(set.behavior_flags(SLOTS), None);
    assert!(!set.set_behavior_flags(&mut m, 2, flags));
    assert!(!set.set_behavior_flags(&mut m, SLOTS, flags));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn flag_controlled_accumulator_writer_preserves_float_bits_and_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P1);
    let before_previous = m.read32(P1 + 0xa8c);
    let before_next = m.read32(P1 + 0xa94);
    let mut set = CharacterSet::load(&m);
    let accumulator = CharacterFlagControlledAccumulator {
        value: f32::from_bits(0x7fc1_2345),
    };

    assert!(set.set_flag_controlled_accumulator(&mut m, 1, accumulator));
    assert_eq!(
        set.flag_controlled_accumulator(1).unwrap().value.to_bits(),
        0x7fc1_2345
    );
    assert_eq!(
        set.flag_controlled_accumulator(4).unwrap().value.to_bits(),
        0x7fc1_2345
    );
    assert_eq!(m.read32(P1 + 0xa90), 0x7fc1_2345);
    assert_eq!(m.read32(P1 + 0xa8c), before_previous);
    assert_eq!(m.read32(P1 + 0xa94), before_next);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn flag_controlled_accumulator_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let accumulator = CharacterFlagControlledAccumulator { value: 1.25 };

    assert_eq!(set.flag_controlled_accumulator(2), None);
    assert_eq!(set.flag_controlled_accumulator(SLOTS), None);
    assert!(!set.set_flag_controlled_accumulator(&mut m, 2, accumulator));
    assert!(!set.set_flag_controlled_accumulator(&mut m, SLOTS, accumulator));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn object_reference_writer_preserves_raw_link_and_shared_body_aliases() {
    let mut m = image();
    // Duplicate table cells share the same Rust-owned body and guest address.
    alias_slot_4(&mut m, P0);
    let adjacent = m.read32(P0 + 4);
    let mut set = CharacterSet::load(&m);
    let reference = CharacterObjectReference { raw: 0x1234_5678 };

    assert!(set.set_object_reference(&mut m, 4, reference));
    assert_eq!(set.object_reference(0), Some(reference));
    assert_eq!(set.object_reference(4), Some(reference));
    assert_eq!(m.read32(P0), reference.raw);
    assert_eq!(m.read32(P0 + 4), adjacent);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn object_reference_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let reference = CharacterObjectReference { raw: 0x8765_4321 };

    assert!(!set.set_object_reference(&mut m, 2, reference));
    assert!(!set.set_object_reference(&mut m, SLOTS, reference));
    assert_eq!(set.object_reference(2), None);
    assert_eq!(set.object_reference(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn target_word_writer_owns_and_writes_all_pointer_like_fields() {
    let mut m = image();
    // Exercise the existing alias rule: duplicate table cells share one body.
    alias_slot_4(&mut m, P0);
    let mut set = CharacterSet::load(&m);
    let words = CharacterTargetWords {
        melee_target: 0x1122_3344,
        alert_target: 0,
        threatening_object: 0x5566_7788,
        pickup_target: 0x99aa_bbcc,
    };

    assert!(set.set_target_words(&mut m, 4, words));
    assert_eq!(set.target_words(0), Some(words));
    assert_eq!(set.target_words(4), Some(words));
    for (offset, value) in [
        (0x4, words.melee_target),
        (0xac8, words.alert_target),
        (0xaec, words.threatening_object),
        (0xaf0, words.pickup_target),
    ] {
        assert_eq!(m.read32(P0 + offset), value, "offset {offset:#x}");
        assert_eq!(set.body_word(0, offset), value, "owned offset {offset:#x}");
        assert_eq!(
            set.body_word(4, offset),
            value,
            "aliased offset {offset:#x}"
        );
    }
    assert_eq!(set.diff(&m), None);
}

#[test]
fn target_word_writer_rejects_absent_and_out_of_range_slots_without_mutation() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let words = CharacterTargetWords {
        melee_target: 1,
        alert_target: 2,
        threatening_object: 3,
        pickup_target: 4,
    };

    assert!(!set.set_target_words(&mut m, 2, words));
    assert!(!set.set_target_words(&mut m, SLOTS, words));
    assert_eq!(set.target_words(2), None);
    assert_eq!(set.target_words(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn class_mask_word_writer_round_trips_all_four_declared_words() {
    let mut m = image();
    let before_low_neighbor = m.read32(P1 + 0x0c);
    let before_high_neighbor = m.read32(P1 + 0x20);
    let mut set = CharacterSet::load(&m);
    let words = CharacterClassMaskWords {
        class_low: 0x0123_4567,
        class_high: 0x89ab_cdef,
        alternate_class_low: 0xfedc_ba98,
        alternate_class_high: 0x7654_3210,
    };

    assert!(set.set_class_mask_words(&mut m, 1, words));
    assert_eq!(set.class_mask_words(1), Some(words));
    assert_eq!(m.read32(P1 + 0x10), words.class_low);
    assert_eq!(m.read32(P1 + 0x14), words.class_high);
    assert_eq!(m.read32(P1 + 0x18), words.alternate_class_low);
    assert_eq!(m.read32(P1 + 0x1c), words.alternate_class_high);
    assert_eq!(m.read32(P1 + 0x0c), before_low_neighbor);
    assert_eq!(m.read32(P1 + 0x20), before_high_neighbor);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn class_mask_word_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before = m.clone();
    let words = CharacterClassMaskWords {
        class_low: 1,
        class_high: 2,
        alternate_class_low: 3,
        alternate_class_high: 4,
    };

    assert_eq!(set.class_mask_words(2), None);
    assert_eq!(set.class_mask_words(SLOTS), None);
    assert!(!set.set_class_mask_words(&mut m, 2, words));
    assert!(!set.set_class_mask_words(&mut m, SLOTS, words));
    assert_eq!(set.class_mask_words(2), None);
    assert_eq!(set.class_mask_words(SLOTS), None);
    assert_eq!(set.diff(&m), None);
    assert!(m == before);
}

#[test]
fn measured_retirement_word_clear_zeros_only_four_extension_words() {
    let mut m = image();
    let before_low_neighbor = m.read32(P1 + 0x0c);
    let before_high_neighbor = m.read32(P1 + 0x20);
    let mut set = CharacterSet::load(&m);
    let words = CharacterClassMaskWords {
        class_low: 0x11,
        class_high: 0x22,
        alternate_class_low: 0x33,
        alternate_class_high: 0x44,
    };

    assert!(set.set_class_mask_words(&mut m, 1, words));
    assert!(set.clear_measured_retirement_words(&mut m, 1));
    assert_eq!(
        set.class_mask_words(1),
        Some(CharacterClassMaskWords::default())
    );
    for offset in [0x10, 0x14, 0x18, 0x1c] {
        assert_eq!(m.read32(P1 + offset), 0);
    }
    assert_eq!(m.read32(P1 + 0x0c), before_low_neighbor);
    assert_eq!(m.read32(P1 + 0x20), before_high_neighbor);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn measured_retirement_word_clear_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();

    assert!(!set.clear_measured_retirement_words(&mut m, 2));
    assert!(!set.clear_measured_retirement_words(&mut m, SLOTS));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn class_label_bytes_writer_round_trips_aliases_and_preserves_neighbor_bytes() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before = [m.read8(P0 + 0x6e), m.read8(P0 + 0x6f)];
    let mut set = CharacterSet::load(&m);
    let label = CharacterClassLabelBytes {
        bytes: core::array::from_fn(|i| (0x30 + i) as u8),
    };

    assert!(set.set_class_label_bytes(&mut m, 4, label));
    assert_eq!(set.class_label_bytes(0), Some(label));
    assert_eq!(set.class_label_bytes(4), Some(label));
    for (i, byte) in label.bytes.into_iter().enumerate() {
        assert_eq!(m.read8(P0 + 0x50 + i as u32), byte);
    }
    assert_eq!([m.read8(P0 + 0x6e), m.read8(P0 + 0x6f)], before);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn class_label_bytes_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let label = CharacterClassLabelBytes {
        bytes: [0xa5; 0x1e],
    };

    assert!(!set.set_class_label_bytes(&mut m, 2, label));
    assert!(!set.set_class_label_bytes(&mut m, SLOTS, label));
    assert_eq!(set.class_label_bytes(2), None);
    assert_eq!(set.class_label_bytes(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn readiness_flags_writer_round_trips_aliases_and_preserves_neighbors() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before = [m.read32(P0 + 0xc68), m.read32(P0 + 0xc70)];
    let mut set = CharacterSet::load(&m);
    let flags = CharacterReadinessFlags { raw: 0x89ab_cdef };

    assert!(set.set_readiness_flags(&mut m, 4, flags));
    assert_eq!(set.readiness_flags(0), Some(flags));
    assert_eq!(set.readiness_flags(4), Some(flags));
    assert_eq!(m.read32(P0 + 0xc6c), flags.raw);
    assert_eq!([m.read32(P0 + 0xc68), m.read32(P0 + 0xc70)], before);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn readiness_flags_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let flags = CharacterReadinessFlags { raw: 0x1234_5678 };

    assert!(!set.set_readiness_flags(&mut m, 2, flags));
    assert!(!set.set_readiness_flags(&mut m, SLOTS, flags));
    assert_eq!(set.readiness_flags(2), None);
    assert_eq!(set.readiness_flags(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn selector_0x112_gate_writer_preserves_other_bits_and_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    m.write32(P0 + 0xc6c, 0x89ab_cded);
    let before_neighbors = [m.read32(P0 + 0xc68), m.read32(P0 + 0xc70)];
    let mut set = CharacterSet::load(&m);

    assert_eq!(set.selector_0x112_gate(0), Some(false));
    assert!(set.set_selector_0x112_gate(&mut m, 4, true));
    assert_eq!(set.selector_0x112_gate(0), Some(true));
    assert_eq!(set.readiness_flags(4).unwrap().raw, 0x89ab_cdef);
    assert_eq!(m.read32(P0 + 0xc6c), 0x89ab_cdef);
    assert_eq!(
        before_neighbors,
        [m.read32(P0 + 0xc68), m.read32(P0 + 0xc70)]
    );

    assert!(set.set_selector_0x112_gate(&mut m, 0, false));
    assert_eq!(set.readiness_flags(4).unwrap().raw, 0x89ab_cded);
    assert_eq!(m.read32(P0 + 0xc6c), 0x89ab_cded);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn selector_0x112_gate_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();

    assert_eq!(set.selector_0x112_gate(2), None);
    assert_eq!(set.selector_0x112_gate(SLOTS), None);
    assert!(!set.set_selector_0x112_gate(&mut m, 2, true));
    assert!(!set.set_selector_0x112_gate(&mut m, SLOTS, true));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn loadout_weapon_word_writer_round_trips_aliases_and_preserves_neighbors() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before = [m.read32(P0 + 0x90), m.read32(P0 + 0x98)];
    let mut set = CharacterSet::load(&m);
    let word = CharacterLoadoutWeaponWord { raw: 0x7654_3210 };

    assert!(set.set_loadout_weapon_word(&mut m, 4, word));
    assert_eq!(set.loadout_weapon_word(0), Some(word));
    assert_eq!(set.loadout_weapon_word(4), Some(word));
    assert_eq!(m.read32(P0 + 0x94), word.raw);
    assert_eq!([m.read32(P0 + 0x90), m.read32(P0 + 0x98)], before);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn loadout_weapon_word_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let word = CharacterLoadoutWeaponWord { raw: 0x1234_5678 };

    assert!(!set.set_loadout_weapon_word(&mut m, 2, word));
    assert!(!set.set_loadout_weapon_word(&mut m, SLOTS, word));
    assert_eq!(set.loadout_weapon_word(2), None);
    assert_eq!(set.loadout_weapon_word(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn damage_history_writer_round_trips_and_preserves_shared_pointer_aliases() {
    let mut m = image();
    m.write32(TABLE, P1);
    let before_neighbor = m.read16(P1 + 0xb6e);
    let mut set = CharacterSet::load(&m);
    let state = CharacterDamageHistoryState {
        seconds_since_damage: 12.5,
        last_health_damage: 30.25,
        threat_stamp: 0x1234_5678,
        threat_id: 0x9abc_def0,
        melee_stamp: 0x0bad_cafe,
        melee_hit_count: 0xbeef,
    };

    assert!(set.set_damage_history_state(&mut m, 1, state));
    assert_eq!(set.damage_history_state(0), Some(state));
    assert_eq!(set.damage_history_state(1), Some(state));
    assert_eq!(m.read32(P1 + 0xb44), state.seconds_since_damage.to_bits());
    assert_eq!(m.read32(P1 + 0xb48), state.last_health_damage.to_bits());
    assert_eq!(m.read32(P1 + 0xb60), state.threat_stamp);
    assert_eq!(m.read32(P1 + 0xb64), state.threat_id);
    assert_eq!(m.read32(P1 + 0xb68), state.melee_stamp);
    assert_eq!(m.read16(P1 + 0xb6c), state.melee_hit_count);
    assert_eq!(m.read16(P1 + 0xb6e), before_neighbor);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn damage_history_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let state = CharacterDamageHistoryState {
        seconds_since_damage: 1.0,
        last_health_damage: 2.0,
        threat_stamp: 3,
        threat_id: 4,
        melee_stamp: 5,
        melee_hit_count: 6,
    };

    assert_eq!(set.damage_history_state(2), None);
    assert_eq!(set.damage_history_state(SLOTS), None);
    assert!(!set.set_damage_history_state(&mut m, 2, state));
    assert!(!set.set_damage_history_state(&mut m, SLOTS, state));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn typed_body_state_round_trips_complete_body_and_shared_pointer_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let mut set = CharacterSet::load(&m);
    let untouched = set.body_state(3);
    let updated = CharacterBodyState {
        words: core::array::from_fn(|word| 0xd200_0000 | word as u32),
    };

    assert!(set.set_body_state(&mut m, 4, updated.clone()));
    assert_eq!(set.body_state(0), Some(updated.clone()));
    assert_eq!(set.body_state(4), Some(updated.clone()));
    assert_eq!(set.body_state(3), untouched);
    for (word, value) in updated.words.iter().copied().enumerate() {
        assert_eq!(m.read32(P0.wrapping_add(word as u32 * 4)), value);
    }
    assert_eq!(set.diff(&m), None);
}

#[test]
fn measured_extension_word_2c_bit_clear_is_owned_bounded_and_alias_safe() {
    let mut m = image();
    alias_slot_4(&mut m, P1);
    let record = P1 + 0x2c;
    let initial = 0xa5c3_71f5;
    m.write32(record, initial);
    let left = m.read32(record - 4);
    let right = m.read32(record + 4);
    let mut set = CharacterSet::load(&m);

    assert_eq!(
        set.extension_word_2c(1),
        Some(CharacterExtensionWord2C { word: initial })
    );
    assert_eq!(set.take_extension_word_2c_bit2(&mut m, 1), Some(true));
    let expected = initial & !4;
    assert_eq!(
        set.extension_word_2c(1),
        Some(CharacterExtensionWord2C { word: expected })
    );
    assert_eq!(set.extension_word_2c(4), set.extension_word_2c(1));
    assert_eq!(m.read32(record), expected);
    assert_eq!(m.read32(record - 4), left);
    assert_eq!(m.read32(record + 4), right);
    assert!(set.clear_extension_word_2c_bit2(&mut m, 4));
    assert_eq!(m.read32(record), expected);
    assert_eq!(set.take_extension_word_2c_bit2(&mut m, 4), Some(false));
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(set.extension_word_2c(2), None);
    assert_eq!(set.extension_word_2c(SLOTS), None);
    assert_eq!(set.take_extension_word_2c_bit2(&mut m, 2), None);
    assert_eq!(set.take_extension_word_2c_bit2(&mut m, SLOTS), None);
    assert!(!set.clear_extension_word_2c_bit2(&mut m, 2));
    assert!(!set.clear_extension_word_2c_bit2(&mut m, SLOTS));
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_field_34_branches_are_opaque_bounded_and_alias_safe() {
    let mut m = image();
    alias_slot_4(&mut m, P1);
    let neighbors = [P0, P1, P3].map(|ptr| (ptr, m.read32(ptr + 0x30), m.read32(ptr + 0x38)));
    let mut set = CharacterSet::load(&m);

    assert_eq!(
        set.apply_measured_initializer_field_34(&mut m, 1, 0x3b3, 0),
        Some(CharacterInitializerField34(6))
    );
    assert_eq!(
        set.initializer_field_34(4),
        Some(CharacterInitializerField34(6))
    );
    assert_eq!(
        set.apply_measured_initializer_field_34(&mut m, 4, 0x553, 0xff),
        Some(CharacterInitializerField34(0xa))
    );
    assert_eq!(
        set.initializer_field_34(1),
        Some(CharacterInitializerField34(0xa))
    );
    assert_eq!(
        set.apply_measured_initializer_field_34(&mut m, 3, 0x482, 0x50),
        Some(CharacterInitializerField34(0))
    );
    assert_eq!(
        set.apply_measured_initializer_field_34(&mut m, 0, 0x482, 0x42),
        Some(CharacterInitializerField34(4))
    );
    assert_eq!(
        set.apply_measured_initializer_field_34(&mut m, 0, 0x482, 0x99),
        Some(CharacterInitializerField34(3))
    );

    for (ptr, left, right) in neighbors {
        assert_eq!(m.read32(ptr + 0x30), left);
        assert_eq!(m.read32(ptr + 0x38), right);
    }
    for slot in [0, 1, 3, 4] {
        assert_eq!(
            set.initializer_field_34(slot),
            Some(CharacterInitializerField34(match slot {
                0 => 3,
                3 => 0,
                _ => 0xa,
            }))
        );
    }
    assert_eq!(m.read32(P0 + 0x34), 3);
    assert_eq!(m.read32(P1 + 0x34), 0xa);
    assert_eq!(m.read32(P3 + 0x34), 0);
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(
        set.apply_measured_initializer_field_34(&mut m, 1, 0x999, 0x42),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_field_34(&mut m, 2, 0x3b3, 0),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_field_34(&mut m, SLOTS, 0x553, 0),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_model_state_is_opaque_bounded_and_alias_safe() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let neighbors = [P0, P1, P3].map(|ptr| (ptr, m.read32(ptr + 4)));
    let mut set = CharacterSet::load(&m);

    assert_eq!(
        set.apply_measured_initializer_model_state(&mut m, 0, 0x0553),
        Some(CharacterInitializerModelState(2))
    );
    assert_eq!(
        set.initializer_model_state(4),
        Some(CharacterInitializerModelState(2))
    );
    assert_eq!(
        set.apply_measured_initializer_model_state(&mut m, 1, 0x0482),
        Some(CharacterInitializerModelState(1))
    );
    assert_eq!(
        set.apply_measured_initializer_model_state(&mut m, 3, 0x03b3),
        Some(CharacterInitializerModelState(0))
    );
    assert_eq!(m.read32(P0), 2);
    assert_eq!(m.read32(P1), 1);
    assert_eq!(m.read32(P3), 0);
    for (ptr, right) in neighbors {
        assert_eq!(m.read32(ptr + 4), right);
    }
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(
        set.apply_measured_initializer_model_state(&mut m, 0, 0x0999),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_model_state(&mut m, 2, 0x0553),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_model_state(&mut m, SLOTS, 0x0553),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_position_copies_and_adjusts_only_the_typed_triple() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let neighbors = [P0, P1, P3].map(|ptr| (ptr, m.read32(ptr + 0x10)));
    let mut set = CharacterSet::load(&m);
    let position = CharacterInitializerPosition {
        x: 12.25,
        y: 18.5,
        z: -7.0,
    };

    assert_eq!(
        set.apply_measured_initializer_position(&mut m, 0, 0x0553, position, 0.5),
        Some(position)
    );
    assert_eq!(
        set.apply_measured_initializer_position(&mut m, 1, 0x0482, position, 0.5),
        Some(position)
    );
    let adjusted = CharacterInitializerPosition {
        y: 18.0,
        ..position
    };
    assert_eq!(
        set.apply_measured_initializer_position(&mut m, 3, 0x03b3, position, 0.5),
        Some(adjusted)
    );
    assert_eq!(set.initializer_position(4), Some(position));
    assert_eq!(set.initializer_source_position(4), Some(position));

    for (ptr, expected) in [(P0, position), (P1, position), (P3, adjusted)] {
        assert_eq!(m.read32(ptr + 4), expected.x.to_bits());
        assert_eq!(m.read32(ptr + 8), expected.y.to_bits());
        assert_eq!(m.read32(ptr + 0x0c), expected.z.to_bits());
    }
    for (ptr, expected) in [(P0, position), (P1, position), (P3, position)] {
        assert_eq!(m.read32(ptr + 0x98), expected.x.to_bits());
        assert_eq!(m.read32(ptr + 0x9c), expected.y.to_bits());
        assert_eq!(m.read32(ptr + 0xa0), expected.z.to_bits());
    }
    for (ptr, right) in neighbors {
        assert_eq!(m.read32(ptr + 0x10), right);
    }
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(
        set.apply_measured_initializer_position(&mut m, 0, 0x0999, position, 0.5),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_position(&mut m, 2, 0x0553, position, 0.5),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_position(&mut m, SLOTS, 0x0553, position, 0.5),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_parameters_follow_model_branches_and_preserve_neighbors() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let neighbors = [P0, P1, P3].map(|ptr| (ptr, m.read32(ptr + 0x64)));
    let mut set = CharacterSet::load(&m);
    let sources = CharacterInitializerParameterSources {
        field_4c: 1.25,
        field_5c: 2.5,
        field_60: 3.75,
        field_5c_increment_for_553: 0.5,
        field_60_replacement_for_553: 4.5,
    };
    let model_553 = CharacterInitializerParameters {
        field_4c: 1.25,
        field_5c: 3.0,
        field_60: 4.5,
    };
    let model_482 = CharacterInitializerParameters {
        field_4c: 1.25,
        field_5c: 2.5,
        field_60: 3.75,
    };
    let model_3b3 = CharacterInitializerParameters {
        field_4c: 1.25,
        field_5c: 2.5,
        field_60: 0.0,
    };

    assert_eq!(
        set.apply_measured_initializer_parameters(&mut m, 0, 0x0553, sources),
        Some(model_553)
    );
    assert_eq!(set.initializer_parameters(4), Some(model_553));
    assert_eq!(
        set.apply_measured_initializer_parameters(&mut m, 1, 0x0482, sources),
        Some(model_482)
    );
    assert_eq!(
        set.apply_measured_initializer_parameters(&mut m, 3, 0x03b3, sources),
        Some(model_3b3)
    );

    for (ptr, expected) in [(P0, model_553), (P1, model_482), (P3, model_3b3)] {
        assert_eq!(m.read32(ptr + 0x4c), expected.field_4c.to_bits());
        assert_eq!(m.read32(ptr + 0x5c), expected.field_5c.to_bits());
        assert_eq!(m.read32(ptr + 0x60), expected.field_60.to_bits());
    }
    for (ptr, right) in neighbors {
        assert_eq!(m.read32(ptr + 0x64), right);
    }
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(
        set.apply_measured_initializer_parameters(&mut m, 0, 0x0999, sources),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_parameters(&mut m, 2, 0x0553, sources),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_parameters(&mut m, SLOTS, 0x0553, sources),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_random_outputs_follow_model_branches_and_alias_safely() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let neighbors = [P0, P1, P3].map(|ptr| {
        (
            ptr,
            [0x70, 0x78, 0x8c, 0x98].map(|offset| m.read32(ptr + offset)),
        )
    });
    let mut set = CharacterSet::load(&m);
    let samples = CharacterInitializerRandomSamples {
        field_74_sample: 1.25,
        field_90_94_sample: -2.5,
    };
    let expected = CharacterInitializerRandomOutputs {
        field_74: 1.25,
        field_90: -2.5,
        field_94: -2.5,
    };

    assert_eq!(
        set.apply_measured_initializer_random_outputs(&mut m, 0, 0x03b3, 0, samples),
        Some(expected)
    );
    assert_eq!(set.initializer_random_outputs(4), Some(expected));
    assert_eq!(
        set.apply_measured_initializer_random_outputs(&mut m, 1, 0x0482, 0x42, samples),
        Some(expected)
    );
    for ptr in [P0, P1] {
        assert_eq!(m.read32(ptr + 0x74), expected.field_74.to_bits());
        assert_eq!(m.read32(ptr + 0x90), expected.field_90.to_bits());
        assert_eq!(m.read32(ptr + 0x94), expected.field_94.to_bits());
    }
    for (ptr, words) in neighbors {
        for (offset, word) in [0x70, 0x78, 0x8c, 0x98].into_iter().zip(words) {
            assert_eq!(m.read32(ptr + offset), word);
        }
    }
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    for (slot, key, k3) in [
        (3, 0x0482, 0x41),
        (3, 0x0553, 0x42),
        (2, 0x03b3, 0),
        (SLOTS, 0x03b3, 0),
    ] {
        assert_eq!(
            set.apply_measured_initializer_random_outputs(&mut m, slot, key, k3, samples),
            None
        );
    }
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_fields_compose_random_outputs_without_partial_rejection() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let input = CharacterInitializerMeasuredInputs {
        branch: CharacterInitializerBranchInput {
            model_key: 0x0482,
            descriptor_k3: 0x42,
            ..CharacterInitializerBranchInput::default()
        },
        ..CharacterInitializerMeasuredInputs::default()
    };
    let samples = CharacterInitializerRandomSamples {
        field_74_sample: 1.25,
        field_90_94_sample: -2.5,
    };
    let (fields, random) = set
        .apply_measured_initializer_fields_with_random_outputs(&mut m, 0, input, samples)
        .expect("measured random branch");
    assert_eq!(fields.model_state, set.initializer_model_state(0).unwrap());
    assert_eq!(random.field_74, 1.25);
    assert_eq!(random.field_90, -2.5);
    assert_eq!(random.field_94, -2.5);
    assert_eq!(m.read32(P0 + 0x74), 1.25f32.to_bits());
    assert_eq!(m.read32(P0 + 0x90), (-2.5f32).to_bits());
    assert_eq!(m.read32(P0 + 0x94), (-2.5f32).to_bits());
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    let unsupported = CharacterInitializerMeasuredInputs {
        branch: CharacterInitializerBranchInput {
            model_key: 0x0553,
            ..input.branch
        },
        ..input
    };
    assert!(
        set.apply_measured_initializer_fields_with_random_outputs(&mut m, 0, unsupported, samples)
            .is_none()
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_model_482_default_outputs_are_typed_bounded_and_alias_safe() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let mut set = CharacterSet::load(&m);
    let sources = CharacterInitializerModel482Sources {
        field_64_source: 3.25,
        field_a8_adjustment: 1.75,
    };
    let expected = CharacterInitializerModel482Outputs {
        field_64: 3.25,
        field_a8: -1.75,
    };
    let neighbor_words = [0x60, 0x68, 0xa4, 0xac].map(|offset| (offset, m.read32(P0 + offset)));
    let other_records = [(P1, 0x64), (P1, 0xa8), (P3, 0x64), (P3, 0xa8)]
        .map(|(ptr, offset)| (ptr, offset, m.read32(ptr + offset)));

    assert_eq!(
        set.apply_measured_initializer_model_482_default_outputs(&mut m, 0, sources),
        Some(expected)
    );
    assert_eq!(set.initializer_model_482_outputs(4), Some(expected));
    assert_eq!(m.read32(P0 + 0x64), expected.field_64.to_bits());
    assert_eq!(m.read32(P0 + 0xa8), expected.field_a8.to_bits());
    for (offset, value) in neighbor_words {
        assert_eq!(m.read32(P0 + offset), value);
    }
    for (ptr, offset, value) in other_records {
        assert_eq!(m.read32(ptr + offset), value);
    }
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(
        set.apply_measured_initializer_model_482_default_outputs(&mut m, 2, sources),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_model_482_default_outputs(&mut m, SLOTS, sources),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_rotations_scale_typed_fields_alias_safely() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let neighbors = [P0, P1, P3].map(|ptr| (ptr, m.read32(ptr + 0x24), m.read32(ptr + 0x34)));
    let p1_rotation_before = [0x28, 0x2c, 0x30].map(|offset| m.read32(P1 + offset));
    let mut set = CharacterSet::load(&m);
    let inputs = CharacterInitializerRotationInputs {
        object_c0: 1.25,
        object_70: -2.5,
        object_c8: 0.75,
    };
    let expected = CharacterInitializerRotations {
        field_28: 0.625,
        field_2c: -1.25,
        field_30: 0.375,
    };

    assert_eq!(
        set.apply_measured_initializer_rotations(&mut m, 0, inputs, 0.5),
        Some(expected)
    );
    assert_eq!(set.initializer_rotations(4), Some(expected));
    assert_eq!(m.read32(P0 + 0x28), expected.field_28.to_bits());
    assert_eq!(m.read32(P0 + 0x2c), expected.field_2c.to_bits());
    assert_eq!(m.read32(P0 + 0x30), expected.field_30.to_bits());
    for (ptr, left, right) in neighbors {
        assert_eq!(m.read32(ptr + 0x24), left);
        assert_eq!(m.read32(ptr + 0x34), right);
    }
    assert_eq!(
        [0x28, 0x2c, 0x30].map(|offset| m.read32(P1 + offset)),
        p1_rotation_before
    );
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(
        set.apply_measured_initializer_rotations(&mut m, 2, inputs, 0.5),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_rotations(&mut m, SLOTS, inputs, 0.5),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_fields_compose_the_bounded_typed_slice() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    for offset in (0..0x118).step_by(4) {
        m.write32(P0 + offset, 0xa5a5_5a5a);
    }
    m.write32(P0 + 0x34, 0xdead_beef);
    m.write32(P0 + 0x38, 0x20);
    m.write32(P0 + 0x3c, 0xcafe_babe);
    let neighbors = [P1, P3].map(|ptr| {
        (
            ptr,
            [0x00, 0x04, 0x28, 0x34, 0x38, 0x3c, 0x4c, 0x60].map(|offset| m.read32(ptr + offset)),
        )
    });
    let mut set = CharacterSet::load(&m);
    let input = CharacterInitializerMeasuredInputs {
        branch: CharacterInitializerBranchInput {
            model_key: 0x0482,
            descriptor_k2: 0x47,
            descriptor_k3: 0x42,
            descriptor_k28: Some(0x1357_9bdf),
        },
        object_position: CharacterInitializerPosition {
            x: 1.5,
            y: 2.5,
            z: 3.5,
        },
        position_y_adjustment: 0.5,
        rotation_inputs: CharacterInitializerRotationInputs {
            object_c0: 1.25,
            object_70: -2.5,
            object_c8: 0.75,
        },
        rotation_scale: 0.5,
        parameter_sources: CharacterInitializerParameterSources {
            field_4c: 0.25,
            field_5c: 0.5,
            field_60: 0.75,
            field_5c_increment_for_553: 0.125,
            field_60_replacement_for_553: 0.875,
        },
    };
    let expected = CharacterInitializerMeasuredFields {
        model_state: CharacterInitializerModelState(1),
        position: input.object_position,
        rotations: CharacterInitializerRotations {
            field_28: 0.625,
            field_2c: -1.25,
            field_30: 0.375,
        },
        parameters: CharacterInitializerParameters {
            field_4c: 0.25,
            field_5c: 0.5,
            field_60: 0.75,
        },
        words: CharacterInitializerMeasuredWords {
            field_34: CharacterInitializerField34(4),
            // Prefix composition starts +0x38 at zero, then applies the
            // measured OR bits (bit 3 from descriptor K+2 == 0x47).
            field_38: CharacterInitializerField38(0x0d),
            field_3c: Some(CharacterInitializerField3C(0x1357_9bdf)),
        },
    };
    let default_sources = CharacterInitializerModel482Sources {
        field_64_source: 1.75,
        field_a8_adjustment: 0.25,
    };
    let defaults = CharacterInitializerModel482Outputs {
        field_64: 1.75,
        field_a8: -0.25,
    };

    assert_eq!(set.zero_measured_initializer_prefix(&mut m, 0), Some(0x118));
    assert_eq!(
        set.apply_measured_initializer_fields_with_482_defaults(&mut m, 0, input, default_sources),
        Some((expected, defaults))
    );
    for offset in (0..0x118).step_by(4) {
        let expected_word = match offset {
            0x00 => expected.model_state.0,
            0x04 => expected.position.x.to_bits(),
            0x08 => expected.position.y.to_bits(),
            0x0c => expected.position.z.to_bits(),
            0x28 => expected.rotations.field_28.to_bits(),
            0x2c => expected.rotations.field_2c.to_bits(),
            0x30 => expected.rotations.field_30.to_bits(),
            0x34 => expected.words.field_34.0,
            0x38 => expected.words.field_38.0,
            0x3c => expected.words.field_3c.unwrap().0,
            0x4c => expected.parameters.field_4c.to_bits(),
            0x5c => expected.parameters.field_5c.to_bits(),
            0x60 => expected.parameters.field_60.to_bits(),
            0x64 => defaults.field_64.to_bits(),
            0xa8 => defaults.field_a8.to_bits(),
            0x98 => input.object_position.x.to_bits(),
            0x9c => input.object_position.y.to_bits(),
            0xa0 => input.object_position.z.to_bits(),
            _ => 0,
        };
        assert_eq!(m.read32(P0 + offset), expected_word, "offset {offset:#x}");
    }
    assert_eq!(set.initializer_model_state(4), Some(expected.model_state));
    assert_eq!(set.initializer_position(4), Some(expected.position));
    assert_eq!(set.initializer_rotations(4), Some(expected.rotations));
    assert_eq!(set.initializer_parameters(4), Some(expected.parameters));
    assert_eq!(set.initializer_field_34(4), Some(expected.words.field_34));
    assert_eq!(set.initializer_field_38(4), Some(expected.words.field_38));
    assert_eq!(set.initializer_field_3c(4), expected.words.field_3c);
    assert_eq!(set.diff(&m), None);
    for (ptr, before) in neighbors {
        assert_eq!(
            [0x00, 0x04, 0x28, 0x34, 0x38, 0x3c, 0x4c, 0x60].map(|offset| m.read32(ptr + offset)),
            before
        );
    }

    assert_eq!(set.initializer_model_482_outputs(4), Some(defaults));
    assert_eq!(m.read32(P0 + 0x64), defaults.field_64.to_bits());
    assert_eq!(m.read32(P0 + 0xa8), defaults.field_a8.to_bits());
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    let unsupported = CharacterInitializerMeasuredInputs {
        branch: CharacterInitializerBranchInput {
            model_key: 0x0999,
            ..input.branch
        },
        ..input
    };
    assert_eq!(
        set.apply_measured_initializer_fields(&mut m, 0, unsupported),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_fields(&mut m, SLOTS, input),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);

    let non_482 = CharacterInitializerMeasuredInputs {
        branch: CharacterInitializerBranchInput {
            model_key: 0x03b3,
            ..input.branch
        },
        ..input
    };
    assert_eq!(
        set.apply_measured_initializer_fields_with_482_defaults(
            &mut m,
            0,
            non_482,
            default_sources,
        ),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_prefix_then_fields_supports_model_553() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    for offset in (0..0x118).step_by(4) {
        m.write32(P0 + offset, 0xa5a5_5a5a);
    }
    let mut set = CharacterSet::load(&m);
    let input = CharacterInitializerMeasuredInputs {
        branch: CharacterInitializerBranchInput {
            model_key: 0x0553,
            descriptor_k2: 0x47,
            descriptor_k3: 0,
            descriptor_k28: None,
        },
        object_position: CharacterInitializerPosition {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
        position_y_adjustment: 9.0,
        rotation_inputs: CharacterInitializerRotationInputs {
            object_c0: 2.0,
            object_70: 4.0,
            object_c8: 6.0,
        },
        rotation_scale: 0.5,
        parameter_sources: CharacterInitializerParameterSources {
            field_4c: 7.0,
            field_5c: 8.0,
            field_60: 9.0,
            field_5c_increment_for_553: 0.25,
            field_60_replacement_for_553: 10.0,
        },
    };
    let (zeroed, fields) = set
        .initialize_measured_extension_prefix_then_fields(&mut m, 0, input)
        .unwrap();

    assert_eq!(zeroed, 0x118);
    assert_eq!(fields.model_state, CharacterInitializerModelState(2));
    assert_eq!(fields.position, input.object_position);
    assert_eq!(fields.parameters.field_5c, 8.25);
    assert_eq!(fields.parameters.field_60, 10.0);
    assert_eq!(fields.words.field_34, CharacterInitializerField34(0x0a));
    assert_eq!(fields.words.field_38, CharacterInitializerField38(0x08));
    assert_eq!(fields.words.field_3c, None);
    for offset in (0..0x118).step_by(4) {
        assert_eq!(set.body_word(0, offset), m.read32(P0 + offset));
    }
    assert_eq!(set.body_word(4, 0x34), 0x0a);
    assert_eq!(m.read32(P0 + 0x04), 1.0f32.to_bits());
    assert_eq!(m.read32(P0 + 0x08), 2.0f32.to_bits());
    assert_eq!(m.read32(P0 + 0x0c), 3.0f32.to_bits());
    assert_eq!(m.read32(P0 + 0x5c), 8.25f32.to_bits());
    assert_eq!(m.read32(P0 + 0x60), 10.0f32.to_bits());
    assert_eq!(m.read32(P0 + 0x118), 0x511f_101e);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn measured_initializer_allocation_prefix_zeroes_owned_aliases_and_guest_memory() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    for offset in (0..INITIALIZER_ZEROED_BYTES).step_by(4) {
        m.write32(P0 + offset, 0xa500_0000 | offset);
    }
    m.write32(P0 + INITIALIZER_ZEROED_BYTES, 0xfeed_beef);
    let neighbor = m.read32(P1);
    let mut set = CharacterSet::load(&m);

    assert_eq!(
        set.zero_measured_initializer_prefix(&mut m, 0),
        Some(INITIALIZER_ZEROED_BYTES)
    );
    for offset in (0..INITIALIZER_ZEROED_BYTES).step_by(4) {
        assert_eq!(set.body_word(0, offset), 0);
        assert_eq!(set.body_word(4, offset), 0);
        assert_eq!(m.read32(P0 + offset), 0);
    }
    assert_eq!(m.read32(P0 + INITIALIZER_ZEROED_BYTES), 0xfeed_beef);
    assert_eq!(m.read32(P1), neighbor);
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(set.zero_measured_initializer_prefix(&mut m, 5), None);
    assert_eq!(set.zero_measured_initializer_prefix(&mut m, SLOTS), None);
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_prefix_composes_random_outputs() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    for offset in (0..0x118).step_by(4) {
        m.write32(P0 + offset, 0xa5a5_5a5a);
    }
    let mut set = CharacterSet::load(&m);
    let input = CharacterInitializerMeasuredInputs {
        branch: CharacterInitializerBranchInput {
            model_key: 0x03b3,
            descriptor_k2: 0,
            descriptor_k3: 0,
            descriptor_k28: None,
        },
        object_position: CharacterInitializerPosition {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
        position_y_adjustment: 0.5,
        rotation_inputs: CharacterInitializerRotationInputs {
            object_c0: 2.0,
            object_70: 4.0,
            object_c8: 6.0,
        },
        rotation_scale: 0.5,
        parameter_sources: CharacterInitializerParameterSources {
            field_4c: 7.0,
            field_5c: 8.0,
            field_60: 9.0,
            field_5c_increment_for_553: 0.0,
            field_60_replacement_for_553: 0.0,
        },
    };
    let samples = CharacterInitializerRandomSamples {
        field_74_sample: 10.0,
        field_90_94_sample: 11.0,
    };
    let zeroed = set.zero_measured_initializer_prefix(&mut m, 0).unwrap();
    let (fields, random) = set
        .apply_measured_initializer_fields_with_random_outputs(&mut m, 0, input, samples)
        .unwrap();
    assert_eq!(zeroed, 0x118);
    assert_eq!(fields.model_state, CharacterInitializerModelState(0));
    assert_eq!(random.field_74, 10.0);
    assert_eq!(random.field_90, 11.0);
    assert_eq!(random.field_94, 11.0);
    for offset in (0..0x118).step_by(4) {
        assert_eq!(m.read32(P0 + offset), set.body_word(0, offset));
    }
    assert_eq!(set.initializer_random_outputs(4), Some(random));
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    let unsupported = CharacterInitializerMeasuredInputs {
        branch: CharacterInitializerBranchInput {
            model_key: 0x0999,
            ..input.branch
        },
        ..input
    };
    assert_eq!(
        set.apply_measured_initializer_fields_with_random_outputs(&mut m, 0, unsupported, samples),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_field_38_ors_are_opaque_bounded_and_alias_safe() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let neighbors = [P0, P1, P3].map(|ptr| (ptr, m.read32(ptr + 0x34), m.read32(ptr + 0x3c)));
    let p1_initial = m.read32(P1 + 0x38);
    let p3_initial = m.read32(P3 + 0x38);
    m.write32(P0 + 0x38, 0x20);
    let mut set = CharacterSet::load(&m);

    assert_eq!(
        set.apply_measured_initializer_field_38(&mut m, 0, 0x3b3, 0x47),
        Some(CharacterInitializerField38(0x28))
    );
    assert_eq!(
        set.initializer_field_38(0),
        Some(CharacterInitializerField38(0x28))
    );
    assert_eq!(
        set.initializer_field_38(4),
        Some(CharacterInitializerField38(0x28))
    );
    assert_eq!(m.read32(P0 + 0x38), 0x28);
    assert_eq!(m.read32(P1 + 0x38), p1_initial);
    assert_eq!(
        set.apply_measured_initializer_field_38(&mut m, 3, 0x482, 0x46),
        Some(CharacterInitializerField38(p3_initial | 5))
    );
    assert_eq!(
        set.apply_measured_initializer_field_38(&mut m, 0, 0x482, 0x47),
        Some(CharacterInitializerField38(0x2d))
    );
    assert_eq!(
        set.apply_measured_initializer_field_38(&mut m, 0, 0x553, 0x46),
        Some(CharacterInitializerField38(0x2d))
    );

    for (ptr, left, right) in neighbors {
        assert_eq!(m.read32(ptr + 0x34), left);
        assert_eq!(m.read32(ptr + 0x3c), right);
    }
    assert_eq!(m.read32(P0 + 0x38), 0x2d);
    assert_eq!(m.read32(P1 + 0x38), p1_initial);
    assert_eq!(m.read32(P3 + 0x38), p3_initial | 5);
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(set.initializer_field_38(2), None);
    assert_eq!(set.initializer_field_38(SLOTS), None);
    assert_eq!(
        set.apply_measured_initializer_field_38(&mut m, 2, 0x482, 0x47),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_field_38(&mut m, SLOTS, 0x482, 0x47),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_words_compose_distinct_overwrite_and_or_branches() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    m.write32(P0 + 0x34, 0xdead_beef);
    m.write32(P0 + 0x38, 0x20);
    m.write32(P0 + 0x3c, 0xcafe_babe);
    let neighbors = [P1, P3].map(|ptr| {
        (
            ptr,
            m.read32(ptr + 0x34),
            m.read32(ptr + 0x38),
            m.read32(ptr + 0x3c),
        )
    });
    let mut set = CharacterSet::load(&m);

    assert_eq!(
        set.apply_measured_initializer_words(
            &mut m,
            0,
            CharacterInitializerBranchInput {
                model_key: 0x0482,
                descriptor_k2: 0x47,
                descriptor_k3: 0x42,
                descriptor_k28: Some(0x1357_9bdf),
            },
        ),
        Some(CharacterInitializerMeasuredWords {
            field_34: CharacterInitializerField34(4),
            field_38: CharacterInitializerField38(0x2d),
            field_3c: Some(CharacterInitializerField3C(0x1357_9bdf)),
        })
    );
    assert_eq!(
        set.initializer_field_34(4),
        Some(CharacterInitializerField34(4))
    );
    assert_eq!(
        set.initializer_field_38(4),
        Some(CharacterInitializerField38(0x2d))
    );
    assert_eq!(
        set.initializer_field_3c(4),
        Some(CharacterInitializerField3C(0x1357_9bdf))
    );
    assert_eq!(m.read32(P0 + 0x34), 4);
    assert_eq!(m.read32(P0 + 0x38), 0x2d);
    assert_eq!(m.read32(P0 + 0x3c), 0x1357_9bdf);
    for (ptr, field_34, field_38, field_3c) in neighbors {
        assert_eq!(m.read32(ptr + 0x34), field_34);
        assert_eq!(m.read32(ptr + 0x38), field_38);
        assert_eq!(m.read32(ptr + 0x3c), field_3c);
    }
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    let unsupported = CharacterInitializerBranchInput {
        model_key: 0x0999,
        descriptor_k2: 0x47,
        descriptor_k3: 0x42,
        descriptor_k28: Some(0x1357_9bdf),
    };
    assert_eq!(
        set.apply_measured_initializer_words(&mut m, 0, unsupported),
        None
    );
    assert_eq!(
        set.apply_measured_initializer_words(&mut m, SLOTS, unsupported),
        None
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn measured_initializer_skips_descriptor_copy_when_key_is_null() {
    let mut m = image();
    m.write32(P0 + 0x3c, 0xa5a5_5a5a);
    let mut set = CharacterSet::load(&m);
    let field_38_before = set.initializer_field_38(0).unwrap();

    assert_eq!(
        set.apply_measured_initializer_words(
            &mut m,
            0,
            CharacterInitializerBranchInput {
                model_key: 0x03b3,
                descriptor_k2: 0,
                descriptor_k3: 0,
                descriptor_k28: None,
            },
        ),
        Some(CharacterInitializerMeasuredWords {
            field_34: CharacterInitializerField34(6),
            field_38: field_38_before,
            field_3c: None,
        })
    );
    assert_eq!(
        set.initializer_field_3c(0),
        Some(CharacterInitializerField3C(0xa5a5_5a5a))
    );
    assert_eq!(m.read32(P0 + 0x3c), 0xa5a5_5a5a);
    assert_eq!(m.read32(P0 + 0x38), field_38_before.0);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn aim_point_writer_round_trips_owned_aliases_and_preserves_neighbors() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before = m.read32(P0 + 0xbb0);
    let mut set = CharacterSet::load(&m);
    let point = CharacterAimPoint {
        coordinates: [1.25, -2.5, 3.75],
    };

    assert!(set.set_aim_point_state(&mut m, 4, point));
    assert_eq!(set.aim_point(0), Some(point));
    assert_eq!(set.aim_point(4), Some(point));
    assert_eq!(m.read32(P0 + 0xbb4), 1.25_f32.to_bits());
    assert_eq!(m.read32(P0 + 0xbb8), (-2.5_f32).to_bits());
    assert_eq!(m.read32(P0 + 0xbbc), 3.75_f32.to_bits());
    assert_eq!(m.read32(P0 + 0xbb0), before);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn aim_point_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let point = CharacterAimPoint {
        coordinates: [4.0, 5.0, 6.0],
    };

    assert!(!set.set_aim_point_state(&mut m, 2, point));
    assert!(!set.set_aim_point_state(&mut m, SLOTS, point));
    assert_eq!(set.aim_point(2), None);
    assert_eq!(set.aim_point(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn aim_height_writer_round_trips_owned_aliases_and_preserves_neighbors() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0xbbc);
    let before_right = m.read32(P0 + 0xbc4);
    let mut set = CharacterSet::load(&m);
    let height = CharacterAimHeight { height: -17.25 };

    assert!(set.set_aim_height_state(&mut m, 4, height));
    assert_eq!(set.aim_height(0), Some(height));
    assert_eq!(set.aim_height(4), Some(height));
    assert_eq!(m.read32(P0 + 0xbc0), (-17.25_f32).to_bits());
    assert_eq!(m.read32(P0 + 0xbbc), before_left);
    assert_eq!(m.read32(P0 + 0xbc4), before_right);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn aim_height_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let height = CharacterAimHeight { height: 9.5 };

    assert!(!set.set_aim_height_state(&mut m, 2, height));
    assert!(!set.set_aim_height_state(&mut m, SLOTS, height));
    assert_eq!(set.aim_height(2), None);
    assert_eq!(set.aim_height(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn aim_scale_writer_round_trips_owned_aliases_and_preserves_neighbors() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0x9f8);
    let before_right = m.read32(P0 + 0xa00);
    let mut set = CharacterSet::load(&m);
    let scale = CharacterAimScale { scale: -2.75 };

    assert!(set.set_aim_scale_state(&mut m, 4, scale));
    assert_eq!(set.aim_scale(0), Some(scale));
    assert_eq!(set.aim_scale(4), Some(scale));
    assert_eq!(m.read32(P0 + 0x9fc), (-2.75_f32).to_bits());
    assert_eq!(m.read32(P0 + 0x9f8), before_left);
    assert_eq!(m.read32(P0 + 0xa00), before_right);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn aim_scale_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let scale = CharacterAimScale { scale: 0.5 };

    assert!(!set.set_aim_scale_state(&mut m, 2, scale));
    assert!(!set.set_aim_scale_state(&mut m, SLOTS, scale));
    assert_eq!(set.aim_scale(2), None);
    assert_eq!(set.aim_scale(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn selector_words_writer_round_trips_signed_values_through_owned_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0x44);
    let before_right = m.read32(P0 + 0x4c);
    let mut set = CharacterSet::load(&m);
    let selectors = CharacterSelectorWords {
        type_model_selector: -0x1234,
        secondary_selector: 0x2345,
    };

    assert!(set.set_selector_words(&mut m, 4, selectors));
    assert_eq!(set.selector_words(0), Some(selectors));
    assert_eq!(set.selector_words(4), Some(selectors));
    assert_eq!(m.read16(P0 + 0x48), -0x1234_i16 as u16);
    assert_eq!(m.read16(P0 + 0x4a), 0x2345);
    assert_eq!(m.read32(P0 + 0x44), before_left);
    assert_eq!(m.read32(P0 + 0x4c), before_right);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn selector_words_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let selectors = CharacterSelectorWords {
        type_model_selector: -7,
        secondary_selector: 9,
    };

    assert!(!set.set_selector_words(&mut m, 2, selectors));
    assert!(!set.set_selector_words(&mut m, SLOTS, selectors));
    assert_eq!(set.selector_words(2), None);
    assert_eq!(set.selector_words(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn cached_target_position_writer_round_trips_owned_aliases_and_preserves_neighbors() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0x2c);
    let before_right = m.read32(P0 + 0x3c);
    let mut set = CharacterSet::load(&m);
    let position = CharacterCachedTargetPosition {
        coordinates: [-11.5, 0.25, 73.0],
    };

    assert!(set.set_cached_target_position_state(&mut m, 4, position));
    assert_eq!(set.cached_target_position(0), Some(position));
    assert_eq!(set.cached_target_position(4), Some(position));
    assert_eq!(m.read32(P0 + 0x30), (-11.5_f32).to_bits());
    assert_eq!(m.read32(P0 + 0x34), 0.25_f32.to_bits());
    assert_eq!(m.read32(P0 + 0x38), 73.0_f32.to_bits());
    assert_eq!(m.read32(P0 + 0x2c), before_left);
    assert_eq!(m.read32(P0 + 0x3c), before_right);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn cached_target_position_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let position = CharacterCachedTargetPosition {
        coordinates: [4.0, 5.0, 6.0],
    };

    assert!(!set.set_cached_target_position_state(&mut m, 2, position));
    assert!(!set.set_cached_target_position_state(&mut m, SLOTS, position));
    assert_eq!(set.cached_target_position(2), None);
    assert_eq!(set.cached_target_position(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn route_goal_writer_round_trips_owned_aliases_and_preserves_neighbors() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0x674);
    let before_gap_a = m.read32(P0 + 0x67c);
    let before_gap_b = m.read32(P0 + 0x680);
    let before_right = m.read32(P0 + 0x688);
    let mut set = CharacterSet::load(&m);
    let state = CharacterRouteGoalState {
        route_id: 0x1234_5678,
        goal_id: 0x90ab_cdef,
    };

    assert!(set.set_route_goal_state(&mut m, 4, state));
    assert_eq!(set.route_goal_state(0), Some(state));
    assert_eq!(set.route_goal_state(4), Some(state));
    assert_eq!(m.read32(P0 + 0x678), state.route_id);
    assert_eq!(m.read32(P0 + 0x684), state.goal_id);
    assert_eq!(m.read32(P0 + 0x674), before_left);
    assert_eq!(m.read32(P0 + 0x67c), before_gap_a);
    assert_eq!(m.read32(P0 + 0x680), before_gap_b);
    assert_eq!(m.read32(P0 + 0x688), before_right);
}

#[test]
fn route_goal_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let state = CharacterRouteGoalState {
        route_id: 4,
        goal_id: 5,
    };

    assert!(!set.set_route_goal_state(&mut m, 2, state));
    assert!(!set.set_route_goal_state(&mut m, SLOTS, state));
    assert_eq!(set.route_goal_state(2), None);
    assert_eq!(set.route_goal_state(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn route_query_preclear_writer_round_trips_raw_words_through_owned_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0x9a4);
    let before_right = m.read32(P0 + 0x9b0);
    let mut set = CharacterSet::load(&m);
    let words = CharacterRouteQueryPreclearWords {
        at_9a8: 0x1234_5678,
        at_9ac: 0x90ab_cdef,
    };

    assert!(set.set_route_query_preclear_words(&mut m, 4, words));
    assert_eq!(set.route_query_preclear_words(0), Some(words));
    assert_eq!(set.route_query_preclear_words(4), Some(words));
    assert_eq!(m.read32(P0 + 0x9a8), words.at_9a8);
    assert_eq!(m.read32(P0 + 0x9ac), words.at_9ac);
    assert_eq!(m.read32(P0 + 0x9a4), before_left);
    assert_eq!(m.read32(P0 + 0x9b0), before_right);
}

#[test]
fn route_query_preclear_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let words = CharacterRouteQueryPreclearWords {
        at_9a8: 7,
        at_9ac: 9,
    };

    assert_eq!(set.route_query_preclear_words(2), None);
    assert_eq!(set.route_query_preclear_words(SLOTS), None);
    assert!(!set.set_route_query_preclear_words(&mut m, 2, words));
    assert!(!set.set_route_query_preclear_words(&mut m, SLOTS, words));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn route_query_preclear_clear_zeros_both_owned_alias_words() {
    let mut m = image();
    m.write32(P0 + 0x9a8, 0x1234_5678);
    m.write32(P0 + 0x9ac, 0x90ab_cdef);
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0x9a4);
    let before_right = m.read32(P0 + 0x9b0);
    let mut set = CharacterSet::load(&m);

    assert!(set.clear_route_query_preclear_words(&mut m, 4));
    assert_eq!(
        set.route_query_preclear_words(0),
        Some(CharacterRouteQueryPreclearWords::default())
    );
    assert_eq!(
        set.route_query_preclear_words(4),
        Some(CharacterRouteQueryPreclearWords::default())
    );
    assert_eq!(m.read32(P0 + 0x9a8), 0);
    assert_eq!(m.read32(P0 + 0x9ac), 0);
    assert_eq!(m.read32(P0 + 0x9a4), before_left);
    assert_eq!(m.read32(P0 + 0x9b0), before_right);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn route_query_preclear_clear_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();

    assert!(!set.clear_route_query_preclear_words(&mut m, 2));
    assert!(!set.clear_route_query_preclear_words(&mut m, SLOTS));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn prepare_route_query_sets_raw_ids_then_clears_preclear_words() {
    let mut m = image();
    m.write32(P0 + 0x9a8, 0x1234_5678);
    m.write32(P0 + 0x9ac, 0x90ab_cdef);
    alias_slot_4(&mut m, P0);
    let mut set = CharacterSet::load(&m);
    let route = CharacterRouteGoalState {
        route_id: 0x1020_3040,
        goal_id: 0x5060_7080,
    };

    assert!(set.prepare_route_query(&mut m, 4, route));
    assert_eq!(set.route_goal_state(0), Some(route));
    assert_eq!(set.route_goal_state(4), Some(route));
    assert_eq!(
        set.route_query_preclear_words(0),
        Some(CharacterRouteQueryPreclearWords::default())
    );
    assert_eq!(
        set.route_query_preclear_words(4),
        Some(CharacterRouteQueryPreclearWords::default())
    );
    assert_eq!(m.read32(P0 + 0x678), route.route_id);
    assert_eq!(m.read32(P0 + 0x684), route.goal_id);
    assert_eq!(m.read32(P0 + 0x9a8), 0);
    assert_eq!(m.read32(P0 + 0x9ac), 0);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn prepare_route_query_rejects_absent_and_out_of_range_without_mutation() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let route = CharacterRouteGoalState {
        route_id: 7,
        goal_id: 9,
    };

    assert!(!set.prepare_route_query(&mut m, 2, route));
    assert!(!set.prepare_route_query(&mut m, SLOTS, route));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn callback_record_reference_writer_round_trips_raw_pointer_through_owned_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0xbec);
    let before_right = m.read32(P0 + 0xbf4);
    let mut set = CharacterSet::load(&m);
    let reference = CharacterCallbackRecordReference { raw: 0x7654_3210 };

    assert!(set.set_callback_record_reference(&mut m, 4, reference));
    assert_eq!(set.callback_record_reference(0), Some(reference));
    assert_eq!(set.callback_record_reference(4), Some(reference));
    assert_eq!(m.read32(P0 + 0xbf0), reference.raw);
    assert_eq!(m.read32(P0 + 0xbec), before_left);
    assert_eq!(m.read32(P0 + 0xbf4), before_right);
}

#[test]
fn callback_record_reference_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let reference = CharacterCallbackRecordReference { raw: 0x1234_5678 };

    assert_eq!(set.callback_record_reference(2), None);
    assert_eq!(set.callback_record_reference(SLOTS), None);
    assert!(!set.set_callback_record_reference(&mut m, 2, reference));
    assert!(!set.set_callback_record_reference(&mut m, SLOTS, reference));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn typed_body_state_rejects_absent_and_out_of_range_slots_without_mutation() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let owned_before = set.clone();
    let guest_before = m.clone();
    let invalid = CharacterBodyState {
        words: [0xfeed_beef; BODY_WORDS],
    };

    assert_eq!(set.body_state(2), None);
    assert_eq!(set.body_state(SLOTS), None);
    assert!(!set.set_body_state(&mut m, 2, invalid.clone()));
    assert!(!set.set_body_state(&mut m, SLOTS, invalid));
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}

#[test]
fn pending_state_request_commits_from_rust_owned_character_state() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    assert!(set.set_state(&mut m, 1, 4));
    assert!(set.set_pending_state(&mut m, 1, 7));
    assert!(set.set_field(&mut m, 1, 0x9d4, 2u32));
    assert!(set.set_transition_time(&mut m, 1, 1.25));

    assert_eq!(
        set.apply_pending_state_request(&mut m, 1),
        Some(tsfp_state::character::CharacterTickEffect::StateChanged { from: 4, to: 7 })
    );
    assert_eq!(
        set.tick_state(1),
        Some(CharacterTickState {
            current: 7,
            pending: -1,
            previous: 4,
            transition_time: 0.0,
        })
    );
    assert_eq!(m.read32(P1 + 0x20), 7);
    assert_eq!(m.read32(P1 + 0x9d0), (-1i32) as u32);
    assert_eq!(m.read32(P1 + 0x9d4), 4);
    assert_eq!(m.read32(P1 + 0x9d8), 0.0f32.to_bits());
    assert_eq!(set.diff(&m), None);

    // A negative sentinel is consumed without changing current/previous state
    // or the elapsed transition value.
    assert!(set.set_pending_state(&mut m, 1, -3));
    assert!(set.set_field(&mut m, 1, 0x9d4, 11u32));
    assert!(set.set_transition_time(&mut m, 1, 0.5));
    assert_eq!(
        set.apply_pending_state_request(&mut m, 1),
        Some(tsfp_state::character::CharacterTickEffect::RequestCleared { current: 7 })
    );
    assert_eq!(
        set.tick_state(1),
        Some(CharacterTickState {
            current: 7,
            pending: -1,
            previous: 11,
            transition_time: 0.5,
        })
    );
    assert_eq!(set.diff(&m), None);
    // Requesting the already-current state only clears pending. It preserves
    // previous state and transition time just like a negative sentinel.
    assert!(set.set_pending_state(&mut m, 1, 7));
    assert!(set.set_field(&mut m, 1, 0x9d4, 13u32));
    assert!(set.set_transition_time(&mut m, 1, 0.75));
    assert_eq!(
        set.apply_pending_state_request(&mut m, 1),
        Some(tsfp_state::character::CharacterTickEffect::RequestCleared { current: 7 })
    );
    assert_eq!(
        set.tick_state(1),
        Some(CharacterTickState {
            current: 7,
            pending: -1,
            previous: 13,
            transition_time: 0.75,
        })
    );
    assert_eq!(set.diff(&m), None);
    assert_eq!(set.apply_pending_state_request(&mut m, 2), None);
    assert_eq!(set.tick_state(SLOTS), None);
}

#[test]
fn pending_state_request_skip_bit_preserves_owned_and_guest_state() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let initial = CharacterTickState {
        current: 4,
        pending: 7,
        previous: 2,
        transition_time: 1.25,
    };
    assert!(set.set_tick_state(&mut m, 1, initial));
    // Keep unrelated control bits opaque while enabling measured bit 2.
    assert!(set.set_behavior_flags(&mut m, 1, CharacterBehaviorFlags { raw: 0xa5 }));
    let owned_before = set.clone();
    let guest_before = m.clone();

    assert_eq!(
        set.apply_pending_state_request(&mut m, 1),
        Some(tsfp_state::character::CharacterTickEffect::PendingApplicationSkipped)
    );
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn missing_slot_writes_nothing_and_shared_pointer_stays_coherent() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let snapshot = m.clone();
    assert!(!set.set_word(&mut m, 2, 0x20, 1));
    assert!(!set.set_state(&mut m, 2, 1));
    assert_eq!(set.goal_flags(2), None);
    assert!(!set.set_goal_flags(&mut m, 2, CharacterFlagWords { low: 3, high: 4 }));
    assert!(m == snapshot);
    // two cells pointing at one body: a write through one is visible through the other
    m.write32(TABLE + 8, P0);
    set = CharacterSet::load(&m);
    assert!(set.set_state(&mut m, 2, 99));
    let flags = CharacterFlagWords {
        low: 0x8000_0001,
        high: 0x4000_0002,
    };
    assert!(set.set_goal_flags(&mut m, 2, flags));
    assert_eq!(set.body_word(0, 0x20), 99);
    assert_eq!(set.goal_flags(0), Some(flags));
    assert_eq!(set.goal_flags(2), Some(flags));
    assert_eq!(set.diff(&m), None);
}

#[test]
fn counted_extension_append_and_swap_remove_match_t1660() {
    let mut m = image();
    m.write32(OBJECT + OBJECT_EXTENSION_OFFSET, PNEW);
    for i in 0..BODY_WORDS as u32 {
        m.write32(PNEW + 4 * i, 0x9000_0000 | i);
    }
    let mut set = CharacterSet::load(&m);
    assert_eq!(set.append_object_extension(&mut m, 0), None);
    assert_eq!(set.append_object_extension(&mut m, OBJECT), Some(4));
    assert_eq!(m.read32(TABLE + 16), PNEW);
    assert_eq!(m.read32(MEMBER_COUNT), 5);
    assert_eq!(set.body_word(4, 0x20), 0x9000_0008);
    assert_eq!(set.live(), 5);
    assert_eq!(set.diff(&m), None);

    m.write32(OBJECT + OBJECT_EXTENSION_OFFSET, P1);
    assert!(set.remove_object_extension(&mut m, OBJECT));
    assert_eq!(m.read32(TABLE + 4), PNEW); // last active pointer moved into the removed cell
    assert_eq!(m.read32(TABLE + 16), PNEW); // inactive tail is left as found
    assert_eq!(m.read32(MEMBER_COUNT), 4);
    assert_eq!(set.ptrs[1], PNEW);
    assert_eq!(set.ptrs[4], 0);
    assert_eq!(set.diff(&m), None);

    // Removing the last active member decrements the count but leaves its guest cell stale.
    m.write32(OBJECT + OBJECT_EXTENSION_OFFSET, P3);
    assert!(set.remove_object_extension(&mut m, OBJECT));
    assert_eq!(m.read32(TABLE + 12), P3);
    assert_eq!(m.read32(MEMBER_COUNT), 3);
    assert_eq!(set.ptrs[3], 0);
    assert_eq!(CharacterSet::classify(&m, P3 + 0x20), None);
    assert_eq!(set.diff(&m), None);
    assert_eq!(CharacterSet::load(&m), set);
}

#[test]
fn drift_fields_equal_the_layout_file() {
    // Parse the TsfpCharacter block of the layout file: offsets and names must equal FIELDS.
    let json = include_str!("../../../tools/data/struct_layouts.json");
    let start = json.find("\"name\": \"TsfpCharacter\"").unwrap();
    let block = &json[start..];
    let block = &block[..block.find("]},").unwrap()];
    let mut parsed = Vec::new();
    for line in block.lines().filter(|l| l.contains("\"offset\"")) {
        let grab = |key: &str| {
            let at = line.find(key).unwrap() + key.len();
            let rest = &line[at..];
            rest[..rest.find('"').unwrap()].to_owned()
        };
        parsed.push((
            u32::from_str_radix(grab("\"offset\": \"0x").as_str(), 16).unwrap(),
            grab("\"name\": \""),
        ));
    }
    assert_eq!(parsed.len(), FIELDS.len());
    for ((off, _, name), (poff, pname)) in FIELDS.iter().zip(&parsed) {
        assert_eq!((*off, *name), (*poff, pname.as_str()));
    }
    assert!(FIELDS.iter().all(|(o, s, _)| o + s <= BODY_SIZE));
    // the T2057 written offsets are all inside the body and 4-aligned
    let w = csv_offsets();
    assert_eq!(w.len(), 217, "character written dwords");
    assert!(w.iter().all(|o| o % 4 == 0 && *o < BODY_SIZE));
    // declared fields that lifted code writes (by dword), and written dwords outside the declared fields
    let declared = |o: u32| FIELDS.iter().any(|(f, s, _)| o >= *f & !3 && o < f + s);
    let undeclared = w.iter().filter(|o| !declared(**o)).count();
    assert!(
        undeclared > 100,
        "most written dwords are not named yet: {undeclared}"
    );
}

#[test]
fn mutation_per_owned_field_is_detected() {
    let m = image();
    let base = CharacterSet::load(&m);
    // every declared field, flipped alone in slot 1, is reported as that dword
    for (off, _, name) in FIELDS {
        let mut s = base.clone();
        s.bodies[1][(off / 4) as usize] ^= 1;
        assert_eq!(s.diff(&m), Some(CharDiff::Body(1, off & !3)), "{name}");
    }
    // every dword written by lifted code (217), slot 3
    for off in csv_offsets() {
        let mut s = base.clone();
        s.bodies[3][(off / 4) as usize] ^= 1;
        assert_eq!(s.diff(&m), Some(CharDiff::Body(3, off)), "+{off:#x}");
    }
    // every table cell
    for slot in 0..base.live() {
        let mut s = base.clone();
        s.ptrs[slot] ^= 0x1000;
        assert_eq!(s.diff(&m), Some(CharDiff::Cell(slot)));
    }
}

#[test]
fn spawn_defs_round_trip_and_write_through() {
    let base = 0x4400_0000;
    let mut m = Image(BTreeMap::new());
    for i in 0..5u32 {
        for j in 0..DEF_WORDS as u32 {
            m.write32(base + i * DEF_STRIDE + j * 4, (i << 16) | j);
        }
    }
    let mut defs = SpawnDefs::load(&m, base, 5);
    assert_eq!(defs.diff(&m), None);
    let mut copy = Image(BTreeMap::new());
    defs.store(&mut copy);
    assert_eq!(SpawnDefs::load(&copy, base, 5), defs);
    // named fields decode from the owned words (flags +0, timer +0xC, spawn_point +0x10)
    let d = defs.owned(2).unwrap();
    assert_eq!((d.flags, d.spawn_point), (2 << 16, (2 << 16) | 4));
    assert!(defs.set_word(&mut m, 2, 0x10, 77));
    assert_eq!(m.read32(base + 2 * DEF_STRIDE + 0x10), 77);
    assert_eq!(defs.owned(2).unwrap().spawn_point, 77);
    assert_eq!(defs.diff(&m), None);
    assert!(!defs.set_word(&mut m, 5, 0, 1)); // beyond count
    assert!(!defs.set_word(&mut m, 0, DEF_STRIDE, 1)); // beyond stride
    // every dword mutation of every definition is detected
    for i in 0..5 {
        for j in 0..DEF_WORDS {
            let mut s = defs.clone();
            s.defs[i][j] ^= 1;
            assert_eq!(s.diff(&m), Some((i, j as u32 * 4)));
        }
    }
    // no list: nothing loaded, nothing written
    let empty = SpawnDefs::load(&m, 0, 9);
    assert_eq!(empty.diff(&m), None);
}

#[test]
fn spawn_defs_typed_field_writer_updates_owned_bytes_and_guest_image() {
    let base = 0x4400_0000;
    let mut m = Image(BTreeMap::new());
    for i in 0..3u32 {
        for j in 0..DEF_WORDS as u32 {
            m.write32(base + i * DEF_STRIDE + j * 4, (i << 16) | j);
        }
    }
    let mut defs = SpawnDefs::load(&m, base, 3);
    let address = base + 2 * DEF_STRIDE + 0x16;
    let before_low_neighbor = m.read8(address - 1);
    let before_high_neighbor = m.read8(address + 2);

    assert!(defs.set_field(&mut m, 2, 0x16, -7i16));

    assert_eq!(
        i16::from_le_bytes([m.read8(address), m.read8(address + 1)]),
        -7
    );
    assert_eq!(m.read8(address - 1), before_low_neighbor);
    assert_eq!(m.read8(address + 2), before_high_neighbor);
    assert_eq!(defs.owned(2).unwrap().model_slot, -7);
    assert_eq!(defs.diff(&m), None);
}

#[test]
fn spawn_defs_typed_field_writer_rejects_unowned_ranges_without_mutation() {
    let base = 0x4400_0000;
    let mut m = Image(BTreeMap::new());
    for j in 0..DEF_WORDS as u32 {
        m.write32(base + j * 4, j);
    }
    let mut defs = SpawnDefs::load(&m, base, 1);
    let before_defs = defs.clone();
    let before_memory = m.clone();

    assert!(!defs.set_field::<u16, _>(&mut m, 1, 0, 7));
    assert!(!defs.set_field::<u16, _>(&mut m, 0, DEF_STRIDE - 1, 7));
    assert!(!defs.set_field::<u32, _>(&mut m, 0, u32::MAX, 7));
    assert_eq!(defs, before_defs);
    assert!(m == before_memory);

    let mut empty = SpawnDefs::load(&m, 0, 1);
    let before_empty = empty.clone();
    assert!(!empty.set_field(&mut m, 0, 0, 7u32));
    assert_eq!(empty, before_empty);
    assert!(m == before_memory);
}

#[test]
fn signed_index_word_writer_round_trips_through_owned_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 4);
    let before_right = m.read32(P0 + 12);
    let mut set = CharacterSet::load(&m);
    let index = CharacterSignedIndexWord { raw: -17 };

    assert!(set.set_signed_index_word(&mut m, 4, index));
    assert_eq!(set.signed_index_word(0), Some(index));
    assert_eq!(set.signed_index_word(4), Some(index));
    assert_eq!(m.read32(P0 + 8), (-17_i32) as u32);
    assert_eq!(m.read32(P0 + 4), before_left);
    assert_eq!(m.read32(P0 + 12), before_right);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn signed_index_word_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let index = CharacterSignedIndexWord { raw: -1 };

    assert!(!set.set_signed_index_word(&mut m, 2, index));
    assert!(!set.set_signed_index_word(&mut m, SLOTS, index));
    assert_eq!(set.signed_index_word(2), None);
    assert_eq!(set.signed_index_word(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn timer_words_writer_round_trips_through_owned_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0xb2c);
    let before_right = m.read32(P0 + 0xb38);
    let mut set = CharacterSet::load(&m);
    let timers = CharacterTimerWords {
        at_b30: 0x1234_5678,
        at_b34: 0x90ab_cdef,
    };

    assert!(set.set_timer_words_state(&mut m, 4, timers));
    assert_eq!(set.timer_words(0), Some(timers));
    assert_eq!(set.timer_words(4), Some(timers));
    assert_eq!(m.read32(P0 + 0xb30), timers.at_b30);
    assert_eq!(m.read32(P0 + 0xb34), timers.at_b34);
    assert_eq!(m.read32(P0 + 0xb2c), before_left);
    assert_eq!(m.read32(P0 + 0xb38), before_right);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn timer_words_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let timers = CharacterTimerWords {
        at_b30: 1,
        at_b34: 2,
    };

    assert!(!set.set_timer_words_state(&mut m, 2, timers));
    assert!(!set.set_timer_words_state(&mut m, SLOTS, timers));
    assert_eq!(set.timer_words(2), None);
    assert_eq!(set.timer_words(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn contact_words_writer_round_trips_through_owned_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let neighbor_offsets = [0xb64, 0xb70];
    let before = neighbor_offsets.map(|offset| m.read32(P0 + offset));
    let mut set = CharacterSet::load(&m);
    let words = CharacterContactWords {
        at_b68: 0x1234_5678,
        at_b6c: 0x90ab_cdef,
    };

    assert!(set.set_contact_words_state(&mut m, 4, words));
    assert_eq!(set.contact_words(0), Some(words));
    assert_eq!(set.contact_words(4), Some(words));
    assert_eq!(m.read32(P0 + 0xb68), words.at_b68);
    assert_eq!(m.read32(P0 + 0xb6c), words.at_b6c);
    assert_eq!(neighbor_offsets.map(|offset| m.read32(P0 + offset)), before);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn contact_words_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let words = CharacterContactWords {
        at_b68: 1,
        at_b6c: 2,
    };

    assert!(!set.set_contact_words_state(&mut m, 2, words));
    assert!(!set.set_contact_words_state(&mut m, SLOTS, words));
    assert_eq!(set.contact_words(2), None);
    assert_eq!(set.contact_words(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn contact_tick_marker_updates_only_the_measured_tick_word() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let counter_before = m.read32(P0 + 0xb6c);
    let neighbors_before = [m.read32(P0 + 0xb64), m.read32(P0 + 0xb70)];
    let mut set = CharacterSet::load(&m);
    let tick = CharacterContactTick(0xdead_beef);

    assert!(set.set_contact_tick_marker(&mut m, 4, tick));
    assert_eq!(set.contact_tick_marker(0), Some(tick));
    assert_eq!(set.contact_tick_marker(4), Some(tick));
    assert_eq!(m.read32(P0 + 0xb68), tick.0);
    assert_eq!(m.read32(P0 + 0xb6c), counter_before);
    assert_eq!(
        [m.read32(P0 + 0xb64), m.read32(P0 + 0xb70)],
        neighbors_before
    );
    assert_eq!(set.diff(&m), None);
}

#[test]
fn contact_tick_marker_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let tick = CharacterContactTick(0x1234_5678);

    assert!(!set.set_contact_tick_marker(&mut m, 2, tick));
    assert!(!set.set_contact_tick_marker(&mut m, SLOTS, tick));
    assert_eq!(set.contact_tick_marker(2), None);
    assert_eq!(set.contact_tick_marker(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn contact_counter_clear_only_clears_the_measured_low_halfword() {
    let mut m = image();
    m.write32(P0 + 0xb6c, 0xa55a_1234);
    alias_slot_4(&mut m, P0);
    let tick_before = m.read32(P0 + 0xb68);
    let right_neighbor = m.read32(P0 + 0xb70);
    let mut set = CharacterSet::load(&m);

    assert_eq!(
        set.contact_counter(0),
        Some(CharacterContactCounter(0x1234))
    );
    assert!(set.apply_contact_counter_clear_on_branch(&mut m, 0, false));
    assert_eq!(
        set.contact_counter(0),
        Some(CharacterContactCounter(0x1234))
    );
    assert_eq!(m.read32(P0 + 0xb6c), 0xa55a_1234);

    assert!(set.apply_contact_counter_clear_on_branch(&mut m, 4, true));
    assert_eq!(set.contact_counter(0), Some(CharacterContactCounter(0)));
    assert_eq!(set.contact_counter(4), Some(CharacterContactCounter(0)));
    assert_eq!(m.read32(P0 + 0xb6c), 0xa55a_0000);
    assert_eq!(m.read32(P0 + 0xb68), tick_before);
    assert_eq!(m.read32(P0 + 0xb70), right_neighbor);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn contact_counter_clear_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();

    assert!(!set.apply_contact_counter_clear_on_branch(&mut m, 2, true));
    assert!(!set.apply_contact_counter_clear_on_branch(&mut m, SLOTS, false));
    assert_eq!(set.contact_counter(2), None);
    assert_eq!(set.contact_counter(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn contact_window_update_applies_measured_branch_then_tick_write() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let mut set = CharacterSet::load(&m);
    let no_clear = CharacterContactWindowUpdate {
        tick: CharacterContactTick(0x1020_3040),
        clear_counter: false,
    };
    assert!(set.set_contact_words_state(
        &mut m,
        0,
        CharacterContactWords {
            at_b68: 0x0102_0304,
            at_b6c: 0xa55a_1234,
        },
    ));

    assert!(set.apply_contact_window_update(&mut m, 4, no_clear));
    assert_eq!(set.contact_tick_marker(0), Some(no_clear.tick));
    assert_eq!(set.contact_tick_marker(4), Some(no_clear.tick));
    assert_eq!(m.read32(P0 + 0xb68), no_clear.tick.0);
    assert_eq!(m.read32(P0 + 0xb6c), 0xa55a_1234);

    let clear = CharacterContactWindowUpdate {
        tick: CharacterContactTick(0x5566_7788),
        clear_counter: true,
    };
    assert!(set.apply_contact_window_update(&mut m, 0, clear));
    assert_eq!(
        set.contact_words(4),
        Some(CharacterContactWords {
            at_b68: clear.tick.0,
            at_b6c: 0xa55a_0000,
        })
    );
    assert_eq!(m.read32(P0 + 0xb68), clear.tick.0);
    assert_eq!(m.read32(P0 + 0xb6c), 0xa55a_0000);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn contact_window_update_rejects_absent_slot_without_partial_write() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();

    assert!(!set.apply_contact_window_update(
        &mut m,
        2,
        CharacterContactWindowUpdate {
            tick: CharacterContactTick(0x1234_5678),
            clear_counter: true,
        },
    ));
    assert!(!set.apply_contact_window_update(
        &mut m,
        SLOTS,
        CharacterContactWindowUpdate {
            tick: CharacterContactTick(0x1234_5678),
            clear_counter: false,
        },
    ));
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn contact_window_then_candidate_preserves_measured_order_and_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    m.write32(P0 + 0xb6c, 0xaaaa_0009);
    m.write32(P1 + 0xb6c, 0x0008_bbbb);
    let mut set = CharacterSet::load(&m);

    // The measured counter clear happens before the candidate comparison.
    // With the supplied branch selected, the zeroed attacker count is
    // suppressed against the target's existing accepted count.
    assert_eq!(
        set.apply_contact_window_then_candidate(
            &mut m,
            4,
            1,
            CharacterContactWindowUpdate {
                tick: CharacterContactTick(0x1234_5678),
                clear_counter: true,
            },
        ),
        Some(false)
    );
    assert_eq!(
        set.contact_tick_marker(0),
        Some(CharacterContactTick(0x1234_5678))
    );
    assert_eq!(set.contact_counter(0), Some(CharacterContactCounter(0)));
    assert_eq!(
        set.accepted_contact_counter(1),
        Some(CharacterAcceptedContactCounter(8))
    );
    assert_eq!(m.read32(P1 + 0xb6c), 0x0008_bbbb);

    // Without the clear, a greater attacker count is recorded on the target.
    m.write32(P0 + 0xb6c, 0xaaaa_0009);
    set.load_bodies(&m);
    assert_eq!(
        set.apply_contact_window_then_candidate(
            &mut m,
            0,
            1,
            CharacterContactWindowUpdate {
                tick: CharacterContactTick(0x8765_4321),
                clear_counter: false,
            },
        ),
        Some(true)
    );
    assert_eq!(
        set.contact_tick_marker(4),
        Some(CharacterContactTick(0x8765_4321))
    );
    assert_eq!(
        set.accepted_contact_counter(1),
        Some(CharacterAcceptedContactCounter(9))
    );
    assert_eq!(m.read32(P1 + 0xb6c), 0x0009_bbbb);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn contact_window_then_candidate_rejects_missing_target_without_partial_write() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();

    assert_eq!(
        set.apply_contact_window_then_candidate(
            &mut m,
            0,
            SLOTS,
            CharacterContactWindowUpdate {
                tick: CharacterContactTick(0x1234_5678),
                clear_counter: true,
            },
        ),
        None
    );
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn melee_contact_candidate_requires_a_greater_counter_and_records_it() {
    let mut m = image();
    alias_slot_4(&mut m, P1);
    m.write32(P0 + 0xb6c, 0xaaaa_0008);
    m.write32(P1 + 0xb6c, 0x0008_1234);
    let mut set = CharacterSet::load(&m);

    assert_eq!(
        set.consider_melee_contact_candidate(&mut m, 0, 1),
        Some(false)
    );
    assert_eq!(m.read32(P1 + 0xb6c), 0x0008_1234);
    assert_eq!(
        set.accepted_contact_counter(1),
        Some(CharacterAcceptedContactCounter(8))
    );

    m.write32(P0 + 0xb6c, 0xaaaa_0009);
    set.load_bodies(&m);
    assert_eq!(
        set.consider_melee_contact_candidate(&mut m, 0, 4),
        Some(true)
    );
    assert_eq!(
        set.accepted_contact_counter(1),
        Some(CharacterAcceptedContactCounter(9))
    );
    assert_eq!(
        set.accepted_contact_counter(4),
        Some(CharacterAcceptedContactCounter(9))
    );
    assert_eq!(m.read32(P1 + 0xb6c), 0x0009_1234);
    assert_eq!(set.contact_counter(0), Some(CharacterContactCounter(9)));
    assert_eq!(set.diff(&m), None);
}

#[test]
fn melee_contact_candidate_rejects_missing_actor_bodies() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();

    assert_eq!(set.consider_melee_contact_candidate(&mut m, 2, 1), None);
    assert_eq!(set.consider_melee_contact_candidate(&mut m, 0, SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn nav_point_words_writer_round_trips_through_owned_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let before_left = m.read32(P0 + 0xaf4);
    let before_right = m.read32(P0 + 0xb04);
    let mut set = CharacterSet::load(&m);
    let words = CharacterNavPointWords {
        at_af8: 0x1234_5678,
        at_afc: 0x90ab_cdef,
        at_b00: 0x0bad_cafe,
    };

    assert!(set.set_nav_point_words_state(&mut m, 4, words));
    assert_eq!(set.nav_point_words(0), Some(words));
    assert_eq!(set.nav_point_words(4), Some(words));
    assert_eq!(m.read32(P0 + 0xaf8), words.at_af8);
    assert_eq!(m.read32(P0 + 0xafc), words.at_afc);
    assert_eq!(m.read32(P0 + 0xb00), words.at_b00);
    assert_eq!(m.read32(P0 + 0xaf4), before_left);
    assert_eq!(m.read32(P0 + 0xb04), before_right);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn nav_point_words_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let words = CharacterNavPointWords {
        at_af8: 1,
        at_afc: 2,
        at_b00: 3,
    };

    assert!(!set.set_nav_point_words_state(&mut m, 2, words));
    assert!(!set.set_nav_point_words_state(&mut m, SLOTS, words));
    assert_eq!(set.nav_point_words(2), None);
    assert_eq!(set.nav_point_words(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn turn_thresholds_writer_round_trips_through_owned_aliases() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let neighbor_offsets = [0xb14, 0xb1c, 0xb24, 0xb2c];
    let before = neighbor_offsets.map(|offset| m.read32(P0 + offset));
    let mut set = CharacterSet::load(&m);
    let thresholds = CharacterTurnThresholds {
        threshold: 1.25,
        delta: -0.375,
    };

    assert!(set.set_turn_thresholds_state(&mut m, 4, thresholds));
    assert_eq!(set.turn_thresholds(0), Some(thresholds));
    assert_eq!(set.turn_thresholds(4), Some(thresholds));
    assert_eq!(m.read32(P0 + 0xb18), thresholds.threshold.to_bits());
    assert_eq!(m.read32(P0 + 0xb28), thresholds.delta.to_bits());
    assert_eq!(neighbor_offsets.map(|offset| m.read32(P0 + offset)), before);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn turn_thresholds_writer_rejects_absent_and_out_of_range_slots() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let before_set = set.clone();
    let before_memory = m.clone();
    let thresholds = CharacterTurnThresholds {
        threshold: 0.5,
        delta: 0.125,
    };

    assert!(!set.set_turn_thresholds_state(&mut m, 2, thresholds));
    assert!(!set.set_turn_thresholds_state(&mut m, SLOTS, thresholds));
    assert_eq!(set.turn_thresholds(2), None);
    assert_eq!(set.turn_thresholds(SLOTS), None);
    assert_eq!(set, before_set);
    assert!(m == before_memory);
}

#[test]
fn turn_thresholds_writer_preserves_float_payload_bits() {
    let mut m = image();
    let mut set = CharacterSet::load(&m);
    let thresholds = CharacterTurnThresholds {
        threshold: f32::from_bits(0x7fc1_2345),
        delta: f32::from_bits(0x8000_0000),
    };

    assert!(set.set_turn_thresholds_state(&mut m, 0, thresholds));
    assert_eq!(m.read32(P0 + 0xb18), 0x7fc1_2345);
    assert_eq!(m.read32(P0 + 0xb28), 0x8000_0000);
    assert_eq!(
        set.turn_thresholds(0).unwrap().threshold.to_bits(),
        0x7fc1_2345
    );
    assert_eq!(set.turn_thresholds(0).unwrap().delta.to_bits(), 0x8000_0000);
    assert_eq!(set.diff(&m), None);
}

#[test]
fn character_index_word_round_trips_as_signed_alias_aware_state() {
    let mut m = image();
    alias_slot_4(&mut m, P0);
    let mut set = CharacterSet::load(&m);
    let prior_melee = m.read32(P0 + 4);
    let prior_class_low = m.read32(P0 + 0x10);

    assert!(set.set_index_word(&mut m, 0, CharacterIndexWord { raw: -17 }));
    assert_eq!(set.index_word(0), Some(CharacterIndexWord { raw: -17 }));
    assert_eq!(set.index_word(4), Some(CharacterIndexWord { raw: -17 }));
    assert_eq!(m.read32(P0 + 8), (-17i32) as u32);
    assert_eq!(m.read32(P0 + 4), prior_melee);
    assert_eq!(m.read32(P0 + 0x10), prior_class_low);
    assert_eq!(set.diff(&m), None);

    let owned_before = set.clone();
    let guest_before = m.clone();
    assert_eq!(set.index_word(2), None);
    assert_eq!(set.index_word(SLOTS), None);
    assert!(!set.set_index_word(&mut m, 2, CharacterIndexWord { raw: 3 }));
    assert!(!set.set_index_word(&mut m, SLOTS, CharacterIndexWord { raw: 3 }));
    assert_eq!(set, owned_before);
    assert!(m == guest_before);
}
