// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic image (patterned bytes); no retail data. Replay evidence is in
// examples/mapedit_check.rs and docs/t-s3-mapmaker.md.
use tsfp_state::GuestMemory;
use tsfp_state::mapedit::*;

#[path = "support/patterned_image.rs"]
mod patterned_image;
use patterned_image::{Image, image};

const OBJ: u32 = 0x7a_0000;

#[test]
fn layout_constants_match_t1712() {
    assert_eq!(OBJECT_WORDS * 4, 0x11950);
    assert_eq!(TABLE_E4.capacity(), 50);
    assert_eq!(TABLE_220.capacity(), 30);
    assert_eq!(TABLE_B4.capacity(), 130);
    assert_eq!(TABLE_8C.capacity(), 100);
    assert_eq!(WEAPON_SLOTS + 4 * WEAPON_SLOT_COUNT, 0x1_1924);
}

#[test]
fn round_trip_is_byte_identical_and_touches_only_the_object() {
    let m = image();
    let o = MapEdit::load(&m, OBJ);
    let mut copy = image();
    copy.write8(OBJ - 1, 0xaa);
    copy.write8(OBJ + OBJECT_SIZE, 0xbb);
    o.store(&mut copy);
    let range = OBJ as usize..(OBJ + OBJECT_SIZE) as usize;
    assert_eq!(&copy.0[range.clone()], &m.0[range]);
    assert_eq!(copy.0[(OBJ - 1) as usize], 0xaa);
    assert_eq!(copy.0[(OBJ + OBJECT_SIZE) as usize], 0xbb);
    assert_eq!(o.diff(&copy), None);
}

#[test]
fn pointer_cell_gates_loading() {
    let mut m = image();
    m.write32(POINTER_CELL, 0);
    assert!(MapEdit::load_current(&m).is_none());
    m.write32(POINTER_CELL, OBJ);
    assert_eq!(MapEdit::load_current(&m).unwrap().base, OBJ);
}

#[test]
fn construct_mirrors_2fe360() {
    let mut m = image();
    let o = MapEdit::construct(&mut m, OBJ, [0x11, 0x22]);
    assert_eq!(m.read32(POINTER_CELL), OBJ);
    assert_eq!(o.diff(&m), None);
    assert_eq!(o.get32(0x210), 0xffff_ffff);
    assert_eq!(o.get32(0x214), 1);
    assert_eq!(o.get32(0x1185c) as i32, -1);
    assert_eq!(o.get32(0x2f9c), 0);
    assert_eq!(m.read32(OBJ + 0x11948), 0x11);
    assert_eq!(m.read32(OBJ + 0x1194c), 0x22);
}

#[test]
fn write_through_setters_and_per_word_drift() {
    let mut m = image();
    let mut o = MapEdit::construct(&mut m, OBJ, [0, 0]);
    o.set_flags(&mut m, 0x120);
    assert!(o.snapshot_requested());
    assert_eq!(m.read32(OBJ + FLAGS), 0x120);
    o.set_change_count(&mut m, 0x1234);
    assert_eq!(m.read32(OBJ + CHANGE_COUNT) & 0xffff, 0x1234);
    o.set_change_key(&mut m, 3, -2);
    assert_eq!(o.change_key(3), -2);
    assert_eq!(m.read8(OBJ + CHANGE_BASE + 12 * 3 + 10), 0xfe);
    o.set8(&mut m, 0x41dc, 7);
    assert_eq!(o.get8(0x41dc), 7);
    o.set_weapon_slot(&mut m, 5, -1);
    assert_eq!(o.weapon_slot(5), -1);
    o.set_count(&mut m, TABLE_B4, 3);
    o.set_element_word(&mut m, TABLE_B4, 2, 4, 0xdead);
    assert_eq!(o.element(TABLE_B4, 2).unwrap()[4], 0xdead);
    assert!(o.element(TABLE_B4, 130).is_none());
    assert_eq!(m.read32(OBJ + B4_BASE + 2 * B4_STRIDE + 16), 0xdead);
    assert_eq!(o.diff(&m), None);
    let before = o.get32(0x41dc);
    o.set8(&mut m, 0x41dd, 9);
    assert_eq!(o.get32(0x41dc) & 0xffff_00ff, before & 0xffff_00ff);
    assert_eq!(o.diff(&m), None);
    for w in 0..OBJECT_WORDS {
        let mut g = m.0.clone();
        g[OBJ as usize + 4 * w] ^= 1;
        assert_eq!(o.diff(&Image(g)), Some(4 * w as u32));
    }
}

#[test]
fn clear_table_zeroes_only_the_body() {
    let mut m = image();
    let mut o = MapEdit::load(&m, OBJ);
    o.set_count(&mut m, TABLE_8C, 5);
    o.clear_table(&mut m, TABLE_8C);
    assert!(o.element(TABLE_8C, 99).unwrap().iter().all(|&w| w == 0));
    assert_eq!(o.count(TABLE_8C), 5, "count left to the caller");
    assert_eq!(o.diff(&m), None);
}
