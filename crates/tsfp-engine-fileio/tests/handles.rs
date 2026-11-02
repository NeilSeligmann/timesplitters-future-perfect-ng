//! Spec tests for the open-slot index (T2074 slice 3). FABRICATED inputs, Tier 0/1.
use std::collections::HashSet;
use tsfp_engine_fileio::handles::{FILE_SHARE_READ, FILE_SHARE_WRITE, OpenIndex, share_conflicts};

#[test]
fn claims_lowest_free_slot_and_reclaims_dead_handles() {
    let mut index = OpenIndex::new(4);
    let mut live: HashSet<u32> = HashSet::new();
    let mut reclaimed = Vec::new();
    for handle in [10u32, 11, 12] {
        let slot = index.claim_slot(&mut |h| live.contains(&h), &mut reclaimed).unwrap();
        index.commit(slot, handle, b"a", 1, 7);
        live.insert(handle);
    }
    assert!(reclaimed.is_empty());
    assert_eq!(index.find(11), Some(1));
    live.remove(&10);
    live.remove(&12);
    let slot = index.claim_slot(&mut |h| live.contains(&h), &mut reclaimed).unwrap();
    assert_eq!((slot, reclaimed.clone()), (0, vec![0, 2]));
    assert_eq!(index.find(10), None);
    assert_eq!(index.find(11), Some(1));
}

#[test]
fn full_table_returns_none() {
    let mut index = OpenIndex::new(2);
    let mut reclaimed = Vec::new();
    for handle in [1u32, 2] {
        let slot = index.claim_slot(&mut |_| true, &mut reclaimed).unwrap();
        index.commit(slot, handle, b"x", 1, 7);
    }
    assert!(index.claim_slot(&mut |_| true, &mut reclaimed).is_none());
    index.release(0);
    assert_eq!(index.claim_slot(&mut |_| true, &mut reclaimed), Some(0));
}

#[test]
fn share_rule_counts_conflicts_case_insensitively() {
    // writer that shares nothing, then a reader
    assert!(share_conflicts(2, 0, 1, 7));
    assert!(!share_conflicts(1, FILE_SHARE_READ | FILE_SHARE_WRITE, 1, FILE_SHARE_READ | FILE_SHARE_WRITE));
    assert!(share_conflicts(1, FILE_SHARE_READ, 2, 7)); // held reader denies writers
    let mut index = OpenIndex::new(3);
    assert!(!index.commit(0, 1, b"Save.DAT", 2, 0));
    assert!(index.commit(1, 2, b"save.dat", 1, 7));
    assert!(!index.commit(2, 3, b"other", 1, 7));
    assert_eq!(index.would_conflict(), 1);
}

#[test]
fn check_answers_without_recording() {
    let mut index = OpenIndex::new(4);
    index.commit(0, 0x10, b"\\Device\\Harddisk0\\partition1\\save.bin", 1, FILE_SHARE_READ);
    assert!(index.check(b"\\device\\HARDDISK0\\partition1\\SAVE.bin", 2, 7));
    assert!(!index.check(b"\\Device\\Harddisk0\\partition1\\save.bin", 1, 7));
    assert!(!index.check(b"\\Device\\Harddisk0\\partition1\\other.bin", 2, 7));
    assert_eq!(index.would_conflict(), 0);
    index.release(0);
    assert!(!index.check(b"\\Device\\Harddisk0\\partition1\\save.bin", 2, 7));
}
