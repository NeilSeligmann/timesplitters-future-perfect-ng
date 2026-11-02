// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures; no retail data. T2135 Retire pilot: persistence and mirror consistency.
use tsfp_state::GuestMemory;
use tsfp_state::game_state::*;

struct Image(Vec<u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        self.0[a as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0[a as usize] = v;
    }
}
fn image() -> Image {
    let mut m = Image(vec![0; 0x80_0000]);
    m.write32(LEVEL, 2);
    m.write32(PLAYERS, 1);
    m.write32(EXTRA_PLAYERS, 0);
    m.write8(MODE, 3);
    m.write32(FLAGS, 0x1122_3344);
    m.write32(MATCH, 9);
    m
}

#[test]
fn off_is_plain_guest_access() {
    let r = Retire::new();
    let mut m = image();
    assert_eq!(r.level(&m), 2);
    r.write(&mut m, Cell::Level, 7, 7);
    assert_eq!(m.read32(LEVEL), 7);
    assert!(!r.is_init(), "off never initialises the owner");
    assert_eq!(r.level(&m), 7);
    assert_eq!(r.check(&m), None);
}

#[test]
fn first_use_initialises_from_guest_and_persists() {
    let r = Retire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    assert_eq!(r.level(&m), 2); // init from guest
    assert!(r.is_init());
    // a guest write that bypasses the owner is NOT seen: the owner is persistent (authority)
    m.write32(LEVEL, 99);
    assert_eq!(r.level(&m), 2);
    assert_eq!(r.check(&m), Some(Cell::Level));
    // a writer updates owner and mirror
    r.write(&mut m, Cell::Level, 5, 5);
    assert_eq!((r.level(&m), m.read32(LEVEL)), (5, 5));
    assert_eq!(r.check(&m), None);
    r.assert_consistent(&m);
}

#[test]
fn writer_before_any_read_captures_old_values_first() {
    let r = Retire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    r.write(&mut m, Cell::Match, 4, 4);
    assert_eq!(r.owned(Cell::Level), 2);
    assert_eq!(r.owned(Cell::Match), 4);
    assert_eq!(r.owned(Cell::Mode), 3);
    assert_eq!(r.owned(Cell::Flags), 0x1122_3344);
}

#[test]
fn flags_low8_keeps_owned_upper_bytes() {
    let r = Retire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    r.write_flags_low8(&mut m, 0x99, 0x99);
    assert_eq!(r.owned(Cell::Flags), 0x1122_3399);
    assert_eq!(m.read32(FLAGS), 0x1122_3399);
    assert_eq!(r.check(&m), None);
}

#[test]
fn audit_returns_guest_and_counts_drift() {
    let r = Retire::new();
    r.set_mode(RETIRE_AUDIT);
    let mut m = image();
    assert_eq!(r.level(&m), 2);
    m.write32(LEVEL, 11); // unwired writer
    let mut seen = Vec::new();
    let v = r.read(&m, Cell::Level, |c, g, o| seen.push((c, g, o)));
    assert_eq!(v, 11, "audit never changes what the reader sees");
    assert_eq!(seen, vec![(Cell::Level, 11, 2)]);
    assert_eq!(r.drift_count(), 1);
}

#[test]
fn owned_only_corruption_changes_owned_readers_not_the_mirror() {
    // the mutation control: owned value differs from the mirror
    let r = Retire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    r.write(&mut m, Cell::Players, 2, 3);
    assert_eq!(m.read32(PLAYERS), 2);
    assert_eq!(r.read(&m, Cell::Players, |_, _, _| {}), 3);
    assert_eq!(r.check(&m), Some(Cell::Players));
}

#[test]
#[should_panic(expected = "GameState owner drift on level")]
fn debug_hook_panics_on_drift() {
    let r = Retire::new();
    r.set_mode(RETIRE_AUDIT);
    let mut m = image();
    r.ensure_init(&m);
    m.write32(LEVEL, 1);
    r.assert_consistent(&m);
}

#[test]
fn reset_reinitialises() {
    let r = Retire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    assert_eq!(r.level(&m), 2);
    m.write32(LEVEL, 8);
    r.reset();
    assert_eq!(r.level(&m), 8);
}

#[test]
fn byte_cell_mode_roundtrip() {
    let r = Retire::new();
    r.set_mode(RETIRE_OWN);
    let mut m = image();
    r.write(&mut m, Cell::Mode, 10, 10);
    assert_eq!(
        (m.read8(MODE), r.read(&m, Cell::Mode, |_, _, _| {})),
        (10, 10)
    );
}
