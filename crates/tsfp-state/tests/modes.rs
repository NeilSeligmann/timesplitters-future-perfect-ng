// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic image (patterned bytes plus a hand-built row table); no retail data.
use tsfp_state::GuestMemory;
use tsfp_state::game_state::LEVEL;
use tsfp_state::modes::*;

struct Image(Vec<u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        self.0[a as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[a as usize] = v;
    }
}

const REC_A: u32 = 0x4fd9a8;
const REC_B: u32 = 0x4fdec0;

fn image() -> Image {
    let mut m = Image((0..0x80_0000usize).map(|i| (i * 7 + 3) as u8).collect());
    // rows: id 1 (A, B), id 2 (A null, B), id 0x65 (A, B), then the zero marker.
    let rows = [(1, REC_A, REC_B), (2, 0, REC_B), (0x65, REC_A, REC_B)];
    for (i, (id, a, b)) in rows.iter().enumerate() {
        let r = ModeRows::row_address(i as u32);
        m.write32(r, 0xe36);
        m.write32(r + ROW_MODE_ID, *id);
        m.write32(r + ROW_RECORD_A, *a);
        m.write32(r + ROW_RECORD_B, *b);
    }
    m.write32(ModeRows::row_address(3), 0);
    m
}

#[test]
fn row_selection_follows_6a7b0_and_6a810() {
    let m = image();
    assert_eq!(ModeRows::find(&m, 2), Some(1));
    assert_eq!(ModeRows::find(&m, 0x65), Some(2));
    assert_eq!(
        ModeRows::find(&m, 9),
        None,
        "zero marker terminates the scan"
    );
    assert_eq!(
        ModeRows::select(&m, 1, 1, 1),
        Some(REC_A),
        "level <= 0x64 and flag set -> A"
    );
    assert_eq!(
        ModeRows::select(&m, 1, 1, 0),
        Some(REC_B),
        "flag clear -> B"
    );
    assert_eq!(ModeRows::select(&m, 2, 2, 1), None, "null A slot");
    assert_eq!(
        ModeRows::select(&m, 0x65, 0x65, 1),
        Some(REC_B),
        "level > 0x64 -> B"
    );
    assert_eq!(
        ModeRows::select_a(&m, 0x65),
        Some(REC_A),
        "6A810 is always A"
    );
    assert_eq!(ModeRows::select_a(&m, 2), None);
}

#[test]
fn record_round_trip_and_untouched_bytes() {
    let mut m = image();
    let before = m.0.clone();
    let mut r = ModeRecord::load(&m, REC_A);
    r.words[SETUPB4_COUNT] = 7;
    r.words[RULE_BASE] = 0x1234_5670;
    r.store(&mut m);
    let again = ModeRecord::load(&m, REC_A);
    assert_eq!(again, r);
    assert_eq!(r.diff(&m), None);
    for (i, (b, a)) in before.iter().zip(&m.0).enumerate() {
        let inside = (i as u32) >= REC_A && (i as u32) < REC_A + RECORD_SIZE;
        assert!(inside || b == a, "byte {i:#x} outside the record changed");
    }
}

#[test]
fn setters_write_through_and_drift_is_found() {
    let mut m = image();
    let mut r = ModeRecord::load(&m, REC_B);
    r.set_list(&mut m, SETUPB4_COUNT, 3, 0x7a_0000);
    r.set_list(&mut m, RULE_COUNT, 5, 0x7b_0000);
    r.set_word(&mut m, GROUP_BOUND, 9);
    assert_eq!(m.read32(REC_B + 0x12c), 3);
    assert_eq!(m.read32(REC_B + 0x130), 0x7a_0000);
    assert_eq!(m.read32(REC_B + 0x134), 5);
    assert_eq!(m.read32(REC_B + 0x138), 0x7b_0000);
    assert_eq!(m.read32(REC_B + 0xd8), 9);
    assert_eq!(r.setups_b4(), (3, 0x7a_0000));
    assert_eq!(r.rules(), (5, 0x7b_0000));
    assert!(r.is_populated());
    assert_eq!(r.diff(&m), None);
    // every word must be reported when it drifts
    for w in 0..RECORD_WORDS {
        let mut m2 = Image(m.0.clone());
        let a = REC_B + 4 * w as u32;
        m2.write32(a, m2.read32(a) ^ 1);
        assert_eq!(r.diff(&m2), Some(w));
    }
}

#[test]
fn clear_mirrors_23f8da() {
    let mut m = image();
    let mut r = ModeRecord::load(&m, REC_A);
    r.clear(&mut m);
    assert!((0..RECORD_SIZE).all(|i| m.read8(REC_A + i) == 0));
    assert_eq!(
        m.read8(REC_A + RECORD_SIZE),
        ((REC_A + RECORD_SIZE) as usize * 7 + 3) as u8
    );
    assert!(!r.is_populated());
}

#[test]
fn setup_resolves_memoises_and_caches() {
    let mut m = image();
    m.write32(LEVEL, 1);
    m.write32(SELECT_FLAG, 1);
    m.write32(CURRENT_CELL, 0);
    m.write32(MEMO_ARG, 0);
    m.write32(MEMO_MODE, 0);
    m.write8(MODE_CELL, 3);
    let mut s = ModeSetup::load(&m);
    assert_eq!(s.resolve(&mut m, 1), REC_A);
    assert_eq!(
        (
            m.read32(CURRENT_CELL),
            m.read32(MEMO_ARG),
            m.read32(MEMO_MODE)
        ),
        (REC_A, 1, 3)
    );
    assert_eq!(s.record.unwrap().base, REC_A);
    assert_eq!(s.diff(&m), None);
    // memo hit: the flag change is not seen until the argument or mode byte changes
    m.write32(SELECT_FLAG, 0);
    assert_eq!(s.resolve(&mut m, 1), REC_A);
    m.write8(MODE_CELL, 4);
    assert_eq!(s.resolve(&mut m, 1), REC_B);
    // Story mode byte: always slot A
    m.write8(MODE_CELL, STORY_MODE as u8);
    assert_eq!(s.resolve(&mut m, 1), REC_A);
    // unknown id caches null
    assert_eq!(s.resolve(&mut m, 9), 0);
    assert!(s.record.is_none());
    m.write32(CURRENT_CELL, REC_B);
    assert_eq!(s.diff(&m), Some("current"));
    s.set_current(&mut m, REC_A);
    m.write32(REC_A + 0x138, 1);
    assert_eq!(s.diff(&m), Some("record"));
}
