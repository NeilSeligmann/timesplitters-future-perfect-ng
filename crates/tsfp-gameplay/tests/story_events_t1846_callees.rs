//! T1846 callee models (FABRICATED inputs against MEASURED control flow, docs/t-story-event-callees-t1846.md).
use tsfp_gameplay::story_events::callees_t1846::*;

#[test]
fn throttled_sound_fires_then_waits_ten_ticks() {
    let mut stamp = 0;
    assert_eq!(
        throttled_sound(5, &mut stamp).map(|s| s.sound_id),
        Some(0xCFB)
    );
    assert_eq!(stamp, 15);
    assert!(throttled_sound(14, &mut stamp).is_none());
    assert!(throttled_sound(15, &mut stamp).is_some());
    assert_eq!(stamp, 25);
}

fn entry(flags: u32, group: u8) -> GroupEntry {
    GroupEntry { flags, group }
}

#[test]
fn mixed_flag_groups() {
    let table = [
        entry(0x100, 1),
        entry(0x100, 1),
        entry(0x180, 3),
        entry(0x000, 3),
        entry(0x80, 0),
    ];
    assert!(!group_has_mixed_flags(&table, 0));
    assert!(group_has_mixed_flags(&table, 2));
    // Group 3 qualifies through entry 2 (flags 0x180); entry 3 lacks the 0x180 bits; group 0 never counts.
    assert_eq!(highest_mixed_group(&table), 3);
    assert_eq!(highest_mixed_group(&[entry(0x100, 2)]), 0);
    assert_eq!(highest_mixed_group(&[]), 0);
}

#[test]
fn region_box_reads_two_vectors() {
    let mut words = [0u32; 0xB4 / 4];
    for (i, v) in [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0].iter().enumerate() {
        words[0x10 / 4 + i] = v.to_bits();
    }
    assert_eq!(region_box(&words), ([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]));
    assert!(!region_query_available(false));
}
