// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures; no retail data. T2138 Retire pilot 2: the persistent PlayerSet owner, its re-base rule
// and the level-load window.
use tsfp_state::GuestMemory;
use tsfp_state::game_state::{RETIRE_AUDIT, RETIRE_OFF, RETIRE_OWN};
use tsfp_state::player::*;

struct Image(Vec<u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        self.0[a as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[a as usize] = v;
    }
}
const REC_A: u32 = 0x10_0000;
const REC_B: u32 = 0x12_0000;
const STATS: u32 = 0x14_0000;
/// Offsets used below are authoritative (not on the pass-through lists).
fn image() -> Image {
    for off in [0x700u32, 0x704, 0x701] {
        assert!(
            PASSTHROUGH_RECORD_OFFSETS
                .binary_search(&(off & !3))
                .is_err()
        );
    }
    for off in [8u32, 0x20] {
        assert!(PASSTHROUGH_STATS_OFFSETS.binary_search(&off).is_err());
    }
    let mut m = Image(vec![0; 0x80_0000]);
    m.write32(RECORD_CELL, REC_A);
    m.write32(CONTROLLED_CELL, REC_A);
    m.write32(STATS_CELL, STATS);
    m.write32(REC_A + 0x700, 0x3f80_0000);
    m.write32(STATS + 8, 0x4000_0000);
    m
}
fn read(r: &PlayerRetire, m: &Image, a: u32, w: u32) -> u32 {
    r.read(m, a, w, |_, _, _| {}, |_, _, _| {})
}

#[test]
fn off_is_plain_guest_access_and_never_initialises() {
    let r = PlayerRetire::new();
    let mut m = image();
    assert_eq!(read(&r, &m, REC_A + 0x700, 4), 0x3f80_0000);
    m.write32(REC_A + 0x700, 5);
    assert_eq!(read(&r, &m, REC_A + 0x700, 4), 5);
    assert!(!r.is_init());
}

#[test]
fn first_use_initialises_from_the_guest_and_own_answers_from_the_owner() {
    let r = PlayerRetire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    assert_eq!(read(&r, &m, REC_A + 0x700, 4), 0x3f80_0000);
    assert!(r.is_init());
    // a store behind the owner is invisible to own-mode readers (that is what the audit measures)
    m.write32(REC_A + 0x700, 9);
    assert_eq!(read(&r, &m, REC_A + 0x700, 4), 0x3f80_0000);
    // sub-dword reads come out of the owned word
    assert_eq!(read(&r, &m, REC_A + 0x701, 1), 0x00);
    assert_eq!(read(&r, &m, REC_A + 0x702, 2), 0x3f80);
    // stats table and addresses outside the extents
    assert_eq!(read(&r, &m, STATS + 8, 4), 0x4000_0000);
    m.write32(0x30_0000, 77);
    assert_eq!(read(&r, &m, 0x30_0000, 4), 77);
    // the record end is exclusive
    m.write32(REC_A + RECORD_SIZE, 123);
    assert_eq!(read(&r, &m, REC_A + RECORD_SIZE, 4), 123);
}

#[test]
fn audit_returns_the_guest_value_counts_drift_and_resyncs_once() {
    let r = PlayerRetire::new();
    r.set_mode(RETIRE_AUDIT);
    let mut m = image();
    assert_eq!(read(&r, &m, REC_A + 0x700, 4), 0x3f80_0000);
    m.write32(REC_A + 0x700, 9);
    let mut seen = Vec::new();
    let v = r.read(
        &m,
        REC_A + 0x700,
        4,
        |_, _, _| {},
        |s, g, o| seen.push((s, g, o)),
    );
    assert_eq!(v, 9, "audit never changes behaviour");
    assert_eq!(seen, vec![(Slot::Record(0x700 / 4), 9, 0x3f80_0000)]);
    assert_eq!(r.drift_count(), 1);
    assert_eq!(read(&r, &m, REC_A + 0x700, 4), 9);
    assert_eq!(r.drift_count(), 1, "reported once");
    // the entry scan finds the other bypassing stores, once each, and resyncs
    m.write32(REC_A + 0x704, 1);
    m.write32(STATS + 0x20, 2);
    let mut slots = Vec::new();
    assert_eq!(r.scan(&m, true, |s, _, _| slots.push(s.log_offset())), 2);
    assert_eq!(slots, vec![0x704, 0x1_0020]);
    assert_eq!(r.scan(&m, true, |_, _, _| {}), 0);
}

#[test]
fn rebase_follows_the_pointer_cell_and_reloads_the_new_body() {
    let r = PlayerRetire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    r.ensure_init(&m);
    // level load: lifted code allocates a new record, initialises it, then points the cell at it
    m.write32(REC_B + 0x700, 0x4040_0000);
    m.write32(RECORD_CELL, REC_B);
    m.write32(CONTROLLED_CELL, REC_B);
    let mut events = Vec::new();
    let v = r.read(
        &m,
        REC_B + 0x700,
        4,
        |c, old, new| events.push((c, old, new)),
        |_, _, _| {},
    );
    assert_eq!(v, 0x4040_0000);
    assert_eq!(
        events,
        vec![(PTR_RECORD, REC_A, REC_B), (PTR_CONTROLLED, REC_A, REC_B)]
    );
    assert_eq!(r.rebase_count(), 2);
    // the old record is no longer owned
    m.write32(REC_A + 0x700, 77);
    assert_eq!(read(&r, &m, REC_A + 0x700, 4), 77);
    // the stats pointer follows too, and a null record stops ownership (menus)
    m.write32(RECORD_CELL, 0);
    assert_eq!(
        read(&r, &m, REC_B + 0x700, 4),
        0x4040_0000,
        "null record: plain guest read of the old address"
    );
    assert_eq!(r.locate(REC_B + 0x700), None);
}

#[test]
fn owner_is_not_resynced_by_a_reallocation_at_the_same_address() {
    // Level load that reuses the address: the pointer cell does not change, so the re-base rule cannot see it. The
    // body is re-initialised behind the owner: own mode serves the stale word, audit counts the drift. This is the
    // documented limit of the rule (docs/t-s3-retire-player.md, "level-load window").
    let r = PlayerRetire::new();
    r.set_mode(RETIRE_AUDIT);
    let mut m = image();
    r.ensure_init(&m);
    for off in (0..RECORD_SIZE).step_by(4) {
        m.write32(REC_A + off, 0);
    }
    assert_eq!(r.rebase_count(), 0);
    assert_eq!(r.scan(&m, true, |_, _, _| {}), 1);
}

#[test]
fn passthrough_offsets_answer_from_the_guest_in_own_mode() {
    let r = PlayerRetire::new();
    r.set_mode(RETIRE_OWN);
    let m = image();
    r.ensure_init(&m);
    for off in PASSTHROUGH_RECORD_OFFSETS {
        assert!(!r.authoritative(Slot::Record((*off / 4) as usize)));
    }
    assert!(r.authoritative(Slot::Record(0)));
    assert!(r.authoritative(Slot::Stats(0)));
    r.set_mode(RETIRE_OFF);
}

#[test]
fn pass_through_words_are_reported_but_not_counted_and_served_from_the_guest() {
    let r = PlayerRetire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    r.ensure_init(&m);
    let off = PASSTHROUGH_RECORD_OFFSETS[0];
    assert!(!r.authoritative(Slot::Record((off / 4) as usize)));
    m.write32(REC_A + off, 0x1234);
    assert_eq!(
        read(&r, &m, REC_A + off, 4),
        0x1234,
        "own mode serves the guest word"
    );
    r.set_mode(RETIRE_AUDIT);
    let mut seen = 0;
    assert_eq!(r.scan(&m, true, |_, _, _| seen += 1), 0, "not counted");
    assert_eq!(seen, 1, "but reported");
    assert_eq!(r.drift_count(), 0);
    // the sorted lists are sorted and aligned
    for list in [PASSTHROUGH_RECORD_OFFSETS, PASSTHROUGH_STATS_OFFSETS] {
        assert!(list.windows(2).all(|w| w[0] < w[1]) && list.iter().all(|o| o % 4 == 0));
    }
    assert!(PASSTHROUGH_RECORD_OFFSETS.iter().all(|o| *o < RECORD_SIZE));
    assert!(PASSTHROUGH_STATS_OFFSETS.iter().all(|o| *o < STATS_SIZE));
}

#[test]
fn unaligned_reads_respect_each_words_authority_and_extent_boundary() {
    let r = PlayerRetire::new();
    let mut m = image();
    let offset = *PASSTHROUGH_RECORD_OFFSETS
        .iter()
        .find(|off| {
            **off >= 4
                && PASSTHROUGH_RECORD_OFFSETS
                    .binary_search(&(**off - 4))
                    .is_err()
        })
        .unwrap();
    m.write32(REC_A + offset - 4, 0x1122_3344);
    m.write32(REC_A + offset, 0x5566_7788);
    r.set_mode(RETIRE_OWN);
    r.ensure_init(&m);
    m.write32(REC_A + offset - 4, 0);
    m.write32(REC_A + offset, 0xaabb_ccdd);
    // Last byte from the authoritative predecessor, next three from pass-through.
    assert_eq!(read(&r, &m, REC_A + offset - 1, 4), 0xbbcc_dd11);
    m.write32(REC_A + RECORD_SIZE, 0x1234_5678);
    r.set_word(Slot::Record((RECORD_SIZE / 4 - 1) as usize), 0x9900_0000);
    assert!(r.authoritative(Slot::Record((RECORD_SIZE / 4 - 1) as usize)));
    assert_eq!(read(&r, &m, REC_A + RECORD_SIZE - 1, 4), 0x3456_7899);
    r.set_mode(RETIRE_AUDIT);
    assert_eq!(
        read(&r, &m, REC_A + offset - 1, 4),
        m.read32(REC_A + offset - 1)
    );
    assert_eq!(r.drift_count(), 1); // one authoritative word, not one event per byte
    assert_eq!(
        read(&r, &m, REC_A + offset - 1, 4),
        m.read32(REC_A + offset - 1)
    );
    assert_eq!(r.drift_count(), 1);
}

#[test]
fn completed_load_boundary_handles_same_address_and_preserves_diagnostics() {
    let r = PlayerRetire::new();
    let mut m = image();
    r.reload_after_level_init(&m);
    assert!(!r.is_init(), "off boundary is inert");
    for mode in [RETIRE_OWN, RETIRE_AUDIT] {
        r.reset();
        r.set_mode(mode);
        r.ensure_init(&m);
        r.note_drift();
        m.write32(REC_A + 0x700, 42);
        m.write32(STATS + 8, 99);
        r.reload_after_level_init(&m);
        assert_eq!(r.drift_count(), 1);
        assert_eq!(r.rebase_count(), 0);
        assert_eq!(r.word(Slot::Record(0x700 / 4)), 42);
        assert_eq!(r.word(Slot::Stats(2)), 99);
        assert_eq!(r.scan(&m, false, |_, _, _| {}), 0);
        // Null transition and reuse of a different initialized body.
        m.write32(RECORD_CELL, 0);
        m.write32(CONTROLLED_CELL, 0);
        r.reload_after_level_init(&m);
        assert_eq!(r.locate(REC_A + 0x700), None);
        m.write32(REC_B + 0x700, 77);
        m.write32(RECORD_CELL, REC_B);
        m.write32(CONTROLLED_CELL, REC_B);
        r.reload_after_level_init(&m);
        assert_eq!(read(&r, &m, REC_B + 0x700, 4), 77);
        m.write32(RECORD_CELL, REC_A);
        m.write32(CONTROLLED_CELL, REC_A);
    }
}
