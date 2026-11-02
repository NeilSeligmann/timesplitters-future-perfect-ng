//! T1846 batch 7 callee models (FABRICATED inputs against MEASURED control flow, docs/t-story-event-callees-t1846.md).
use tsfp_gameplay::story_events::arcade::{EffectSound, effect_sounds, resolve_effect_sounds};
use tsfp_gameplay::story_events::callees_t1846::*;

#[test]
fn rescale_uses_axis_product_or_length() {
    let mut v = [3.0, 0.0, 4.0];
    rescale_vector(&mut v, [0.0, 2.0, 0.0], [0.0, 0.0, 5.0]);
    // divisor |2 * 5| = 10
    assert_eq!(v, [0.3, 0.0, 0.4]);
    let mut w = [3.0, 0.0, 4.0];
    rescale_vector(&mut w, [0.0; 3], [1.0, 0.0, 1.0]);
    // b.z == 0 is false here (z is 1) and b.x is 1: unit length path
    assert!((w[0] - 0.6).abs() < 1e-6 && (w[2] - 0.8).abs() < 1e-6);
    let mut u = [3.0, 0.0, 4.0];
    rescale_vector(&mut u, [0.0, 2.0, 0.0], [3.0, 0.0, 0.0]);
    // b.x != 0 and b.z == 0: divisor |2 * 3| = 6
    assert!((u[0] - 0.5).abs() < 1e-6);
}

#[test]
fn group_record_lookup_and_max_key() {
    assert_eq!(lookup_group_record(&[(7, 2), (9, 0)], 9), Some(0));
    assert_eq!(lookup_group_record(&[(7, 2)], 8), None);
    let record = |kind| GroupRecord {
        region_index: 1,
        word_4c: 5,
        word_50: 1,
        word_88: 1,
        kind_byte: kind,
    };
    let of = |key: u32| match key {
        1 => Some(record(1)),
        2 => Some(record(0)),
        3 => Some(GroupRecord {
            region_index: 0,
            ..record(1)
        }),
        _ => None,
    };
    let region = |index: u32| [4u32, 9][index as usize];
    // key 1 -> region 1 (9); key 2 rejected (kind 0); key 3 -> region 0 (4); key 4 has no record.
    assert_eq!(max_region_key(4, &of, &region), 9);
    assert_eq!(max_region_key(0, &of, &region), 0);
}

#[test]
fn flag_kind_groups_and_array_size() {
    let table = [
        GroupEntry {
            flags: 0x80,
            group: 1,
        },
        GroupEntry {
            flags: 0x100,
            group: 1,
        },
        GroupEntry {
            flags: 0x80,
            group: 2,
        },
        GroupEntry {
            flags: 0x180,
            group: 3,
        },
        GroupEntry {
            flags: 0x00,
            group: 3,
        },
    ];
    assert!(group_has_both_flag_kinds(&table, 1));
    assert!(!group_has_both_flag_kinds(&table, 2));
    // 0x180 carries both bits on one entry
    assert!(group_has_both_flag_kinds(&table, 3));
    assert_eq!(group_array_size(&table, 7), 7);
    assert_eq!(group_array_size(&table, 0), 3);
}

#[test]
fn member_list_skips_negative_words_and_finds_regions() {
    let objects = |n: i32| 100 + n as u32;
    let list = build_member_list(
        &[(0xA0, 2), (0xB0, -1), (0xC0, 0)],
        &[0xC0, 0xA0],
        &objects,
        77,
    );
    assert_eq!(list.word_8, 77);
    assert_eq!(list.count, 3);
    assert_eq!(list.slots[1], None);
    assert_eq!(
        list.slots[0],
        Some(MemberSlot {
            object: 103,
            member: 0xA0,
            region: 1
        })
    );
    assert_eq!(
        list.slots[2],
        Some(MemberSlot {
            object: 101,
            member: 0xC0,
            region: 0
        })
    );
    assert_eq!(list.objects, vec![103, 101]);
    let unknown = build_member_list(&[(0x1, 0)], &[], &objects, 0);
    assert_eq!(unknown.slots[0].unwrap().region, -1);
}

#[test]
fn typed_row_search() {
    // v = 5: type 0 matches id 2 ((5-1)/2); v = 6: type 1 matches id 3.
    assert_eq!(find_typed_row(&[2, 5], 0, 2), Some(1));
    assert_eq!(find_typed_row(&[2, 5], 0, 1), None);
    assert_eq!(find_typed_row(&[5, 6], 1, 3), Some(1));
    assert_eq!(find_typed_row(&[5, 6], 2, 3), None);
}

#[test]
fn region_hits_stop_storing_when_ring_full() {
    let hits = region_hits_fixture();
    let out = process_region_hits(&hits, 2);
    assert_eq!(out.cleared, vec![1, 2, 3, 4]);
    assert_eq!(out.stored, vec![1, 3]);
    assert_eq!(process_region_hits(&hits, 8).stored, vec![1, 3, 4]);
    assert_eq!(
        region_query_block([5.0, 6.0, 7.0, 8.0, 9.0, 10.0]),
        [5.0, f32::from_bits(0xFF7F_FFFF), 7.0, 8.0, 9.0, 10.0]
    );
}

fn region_hits_fixture() -> Vec<u16> {
    // low bit set means "store"; entry = value >> 1
    vec![0b0011, 0b0100, 0b0111, 0b1001]
}

#[test]
fn sweep_objects_ors_member_hits() {
    let object = |flags_28, flags_20, target| SweepObject {
        flags_28,
        flags_20,
        target,
    };
    let head = object(0x20, 0, 10);
    let members = [
        object(0x40, 0, 11),
        object(0x40, 0x4, 12),
        object(0x140, 0, 13),
        object(0x8000_0040, 0, 14),
        object(0x40, 0, 0),
    ];
    let mut seen = Vec::new();
    let mut sweep = |target: u32| {
        seen.push(target);
        if target == 10 {
            0xF0
        } else {
            u32::from(target == 11)
        }
    };
    // skip mask 4 rejects member 12, bit 0x100 rejects 13, sign bit rejects 14, null target rejects the last.
    assert_eq!(sweep_objects(1, 3, head, &members, 4, &mut sweep), 0xF1);
    assert_eq!(seen, vec![10, 11]);
    assert_eq!(sweep_objects(4, 3, head, &members, 4, &mut |_| 1), 0);
    assert_eq!(sweep_objects(0, 3, object(0, 0, 1), &[], 0, &mut |_| 1), 0);
}

#[test]
fn throttled_effect_sound_resolves_with_rate_limit() {
    let mut stamp = 0;
    let first = resolve_effect_sounds(effect_sounds(0x28, 1.0, 0), 5, &mut stamp);
    assert!(matches!(first.as_slice(), [EffectSound::Throttled(s)] if s.sound_id == 0xCFB));
    assert_eq!(stamp, 15);
    assert!(resolve_effect_sounds(effect_sounds(0x2F, 1.0, 0), 6, &mut stamp).is_empty());
    let plain = resolve_effect_sounds(effect_sounds(0xD, 1.0, 0), 6, &mut stamp);
    assert_eq!(plain.len(), 1);
}
