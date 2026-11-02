// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic image (patterned bytes); no retail data.
use tsfp_state::GuestMemory;
use tsfp_state::game_state::LEVEL;
use tsfp_state::missions::*;

#[path = "support/patterned_image.rs"]
mod patterned_image;
use patterned_image::{Image, image};

#[test]
fn cells_round_trip_and_drift() {
    let m = image();
    let c = PickupCells::load(&m);
    let mut copy = Image(m.0.clone());
    c.store(&mut copy);
    assert!(copy.0 == m.0, "store back is byte identical");
    assert_eq!(c.diff(&m), None);
    for a in PTR_CELLS
        .into_iter()
        .chain([MAIN_COUNT, LINK_COUNT, T34_COUNT])
        .chain(AUX_CELLS)
    {
        let mut x = Image(m.0.clone());
        x.write32(a, x.read32(a) ^ 1);
        assert!(c.diff(&x).is_some(), "{a:x}");
    }
}

#[test]
fn setters_write_through() {
    let mut m = image();
    let mut c = PickupCells::load(&m);
    c.set_ptr(&mut m, 0, 0x40_0000);
    c.set_main_count(&mut m, 7);
    c.set_link_count(&mut m, 3);
    c.set_t34_count(&mut m, 2);
    c.set_aux(&mut m, 2, 0x11);
    assert_eq!(m.read32(MAIN_PTR), 0x40_0000);
    assert_eq!(m.read32(MAIN_COUNT), 7);
    assert_eq!(m.read32(0x7b_666c), 0x11);
    assert_eq!(c.diff(&m), None);
    assert_eq!(c.main_row_address(2), 0x40_0040);
    c.reset_9ce00(&mut m);
    assert_eq!((m.read32(MAIN_COUNT), m.read32(LINK_COUNT)), (0, 0));
    assert_eq!(m.read32(T34_COUNT), 2, "9CE00 keeps the t34 count");
    assert_eq!(m.read32(0x7b_666c), 0x11, "9CE00 keeps the group base");
    assert_eq!(c.diff(&m), None);
    m.write32(LEVEL, 5);
    assert!(!c.reset_9a20c(&mut m), "only level 0x64");
    m.write32(LEVEL, 0x64);
    assert!(c.reset_9a20c(&mut m));
    assert_eq!(m.read32(T34_COUNT), 0);
    assert_eq!(m.read32(0x7b_666c), 0);
    assert_eq!(c.diff(&m), None);
    m.write32(CLEAR_GUARD, 1);
    c.clear_pointers_9a260(&mut m);
    assert_ne!(m.read32(MAIN_PTR), 0, "guarded by 6B7AA8");
    m.write32(CLEAR_GUARD, 0);
    c.clear_pointers_9a260(&mut m);
    assert_eq!(c.ptrs[..4], [0; 4]);
    assert_eq!(c.diff(&m), None);
}

#[test]
fn rows_round_trip_every_word() {
    let mut m = image();
    let base = 0x40_0000u32;
    let r = PickupRow::load(&m, base);
    let mut copy = Image(m.0.clone());
    r.store(&mut copy);
    assert!(copy.0 == m.0);
    let mut r = r;
    for w in 0..8 {
        r.set_word(&mut m, w, 0xdead_0000 + w as u32);
        assert_eq!(r.diff(&m), None);
        assert_eq!(m.read32(base + 4 * w as u32), 0xdead_0000 + w as u32);
    }
    r.set_word(&mut m, 2, 0xfffe_0003);
    assert_eq!((r.group(), r.tag()), (3, -2));
    r.set_flags(&mut m, 3);
    assert!(r.usable() && r.active());
    r.set_position(&mut m, [1, 2, 3]);
    assert_eq!(PickupRow::load(&m, base), r);
    assert_eq!(r.position_bits(), [1, 2, 3]);
    let mut x = Image(m.0.clone());
    x.write8(base + 5, 0x77);
    assert_eq!(r.diff(&x), Some(1));
}

#[test]
fn links_and_neighbours() {
    let mut m = image();
    let base = 0x40_1000;
    let mut l = LinkRow::load(&m, base);
    l.set_word(&mut m, 0, 0);
    l.set_word(&mut m, 1, 4);
    l.set_word(&mut m, 2, 9);
    assert_eq!(l.other_end(4), Some(9));
    assert_eq!(l.other_end(9), Some(4));
    l.set_word(&mut m, 0, 2);
    assert_eq!(l.other_end(9), None, "flag 2 blocks the reverse direction");
    assert_eq!(l.other_end(4), Some(9));
    assert_eq!(l.other_end(5), None);
    for w in 0..5 {
        let mut x = Image(m.0.clone());
        x.write32(base + 4 * w, 0x1234_5678);
        assert_eq!(l.diff(&x), Some(w as usize));
    }
}

#[test]
fn visited_mark_clear_and_extent() {
    let mut m = image();
    let mut v = Visited::load(&m);
    assert_eq!(v.diff(&m), None);
    v.clear(&mut m);
    assert!((0..VISITED_SIZE).all(|i| m.read8(VISITED + i as u32) == 0));
    let past = m.read8(VISITED + VISITED_SIZE as u32);
    assert!(v.mark(&mut m, 5, 1));
    assert!(v.is_visited(5) && !v.is_visited(6));
    assert!(!v.mark(&mut m, VISITED_SIZE, 1), "past the cleared extent");
    assert_eq!(m.read8(VISITED + VISITED_SIZE as u32), past);
    assert_eq!(v.diff(&m), None);
    m.write8(VISITED + 31, 9);
    assert_eq!(v.diff(&m), Some(31));
}

#[test]
fn setup_and_rule_records() {
    let mut m = image();
    let b4 = SetupB4::address(0x50_0000, 3);
    assert_eq!(b4, 0x50_0000 + 3 * 0xb4);
    let mut s = SetupB4::load(&m, b4);
    let mut copy = Image(m.0.clone());
    s.store(&mut copy);
    assert!(copy.0 == m.0);
    s.set_word(&mut m, 0, 2);
    s.set_word(&mut m, 3, 0x8001);
    s.set_word(&mut m, 0xa4 / 4, 5);
    s.set_word(&mut m, 0xb0 / 4, 0x1_003c);
    assert_eq!(
        (s.kind(), s.index(), s.rule_index(), s.flags()),
        (2, -0x7fff, 5, 0x3c)
    );
    for w in 0..45 {
        let mut x = Image(m.0.clone());
        x.write32(b4 + 4 * w, x.read32(b4 + 4 * w) ^ 1);
        assert_eq!(s.diff(&x), Some(w as usize));
    }
    let ra = RuleRecord::address(0x60_0000, 2);
    assert_eq!(ra, 0x60_0000 + 0x440);
    let mut r = RuleRecord::load(&m, ra);
    r.set_word(&mut m, 0x50 / 4, 2);
    r.set_word(&mut m, 0x54 / 4, 0x1234);
    r.set_word(&mut m, 0x88 / 4, 1);
    r.set_word(&mut m, 0x8c / 4, 0x5678);
    r.set_word(&mut m, 0x7c / 4, 0x0403_0201);
    r.set_word(&mut m, 0x21c / 4, 0);
    assert_eq!(r.conditions(), (2, 0x1234));
    assert_eq!(r.actions(), (1, 0x5678));
    assert_eq!((r.condition_mode(0), r.condition_mode(3)), (1, 4));
    assert!(!r.event_rule());
    r.set_word(&mut m, 0x21c / 4, 1);
    assert!(r.event_rule());
    assert_eq!(RuleRecord::load(&m, ra), r);
    for w in 0..136 {
        let mut x = Image(m.0.clone());
        x.write32(ra + 4 * w, x.read32(ra + 4 * w) ^ 1);
        assert_eq!(r.diff(&x), Some(w as usize));
    }
}

#[test]
fn draw_cursor_round_trip() {
    let mut m = image();
    let mut d = DrawCursor::load(&m);
    let mut copy = Image(m.0.clone());
    d.store(&mut copy);
    assert!(copy.0 == m.0);
    d.set_cursor(&mut m, 0x1000);
    assert_eq!(m.read32(0x55_f974), 0x1000);
    assert_eq!(d.advance(&mut m, 3), 0x1000);
    assert_eq!(m.read32(0x55_f974), 0x100c);
    assert_eq!(d.diff(&m), None);
    for w in 0..4 {
        let mut x = Image(m.0.clone());
        x.write32(0x55_f96c + 4 * w, 0x77);
        assert_eq!(d.diff(&x), Some(w as usize));
    }
}
