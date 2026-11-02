// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED contract evidence; not a lifted differential or route trace.
use tsfp_state::{
    GuestMemory,
    mission_catalog::*,
    missions::*,
    missions_owned::{Missions, REQUIRED_ROOTS},
};
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
    let mut m = Image(vec![0; 0x800000]);
    for (i, cell) in PTR_CELLS[..4].iter().enumerate() {
        m.write32(*cell, 0x10000 + i as u32 * 0x20000);
    }
    m
}
#[test]
fn owned_tables_cells_partial_stores_audit_and_rebase_2000_seeds() {
    let mut m = image();
    let s = Missions::new();
    assert_eq!(s.mode(), 0);
    assert!(!s.store(&mut m, MAIN_COUNT, 1, 4));
    for missing in REQUIRED_ROOTS {
        assert_eq!(s.configure(1, |va| va != missing), Err(missing));
    }
    assert_eq!(s.configure(3, |_| true), Err(u32::MAX));
    s.configure(1, |_| true).unwrap();
    s.initialise(&m);
    let mut rng = 0x2137u32;
    for seed in 0..2000 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        let address = [
            MAIN_COUNT,
            LINK_COUNT,
            T34_COUNT,
            AUX_CELLS[2],
            0x10004,
            0x30004,
            0x50004,
            0x70004,
            VISITED,
        ][seed % 9];
        assert!(s.store(&mut m, address, rng, 4));
        assert_eq!(s.read32(&m, address), rng);
        m.write32(address, rng ^ 0x11223344);
        assert_eq!(
            s.read32(&m, address),
            rng,
            "owner survives mirror corruption"
        );
        assert!(s.store(&mut m, address + 1, 0xabcd, 2));
        // Merge from owner, rather than the corrupt guest dword.
        assert_eq!(s.read32(&m, address), (rng & 0xff0000ff) | 0x00abcd00);
        assert!(s.store(&mut m, address, rng, 4));
        assert_eq!(m.read32(address), rng);
    }
    // Straddling store: last owned byte plus guest-owned neighbour.
    m.write8(VISITED + 32, 0);
    assert!(s.store(&mut m, VISITED + 31, 0xaabbccdd, 4));
    assert_eq!(m.read32(VISITED + 31), 0xaabbccdd);
    // A pointer setter must reload the complete new allocation, never a partial pointer.
    m.write32(0x90000, 0xdeadbeef);
    assert!(s.store(&mut m, MAIN_PTR, 0x90000, 4));
    assert_eq!(s.read32(&m, 0x90000), 0xdeadbeef);
    assert_eq!(s.rebase_count(), 1);
    m.write32(MAIN_PTR, 0); // External free also invalidates the allocation.
    s.initialise(&m);
    assert_eq!(s.classify(0x90000), None);
    assert_eq!(s.rebase_count(), 2);
    s.configure(2, |_| false).unwrap();
    s.initialise(&m);
    m.write32(MAIN_COUNT, 0xabcdef);
    assert_eq!(s.read32(&m, MAIN_COUNT), 0xabcdef);
    assert!(s.drift_count() > 0);
    let mut reports = Vec::new();
    s.audit_entry(&m, 0x9a7a0, |root, previous, slot, guest, owned| {
        reports.push((root, previous, slot, guest, owned))
    });
    assert!(
        reports
            .iter()
            .any(|line| line.0 == 0x9a7a0 && line.1 == 0 && line.3 == 0xabcdef)
    );
    reports.clear();
    s.audit_entry(&m, 0x9af90, |root, previous, slot, guest, owned| {
        reports.push((root, previous, slot, guest, owned))
    });
    assert!(
        reports
            .iter()
            .any(|line| line.0 == 0x9af90 && line.1 == 0x9a7a0)
    );
    assert!(s.store(&mut m, MAIN_COUNT, 7, 4));
    let drift = s.drift_count();
    assert_eq!(s.read32(&m, MAIN_COUNT), 7);
    assert_eq!(s.drift_count(), drift);
    s.configure(0, |_| false).unwrap();
    m.write32(MAIN_COUNT, 9);
    assert_eq!(s.read32(&m, MAIN_COUNT), 9);
}
#[test]
fn level_capacity_and_unused_rows() {
    let mut m = image();
    let s = Missions::new();
    s.configure(1, |_| true).unwrap();
    s.initialise(&m);
    assert!(s.classify(0x50000 + T34_CAPACITY * ROW_SIZE - 1).is_some());
    assert!(s.classify(0x50000 + T34_CAPACITY * ROW_SIZE).is_none());
    m.write32(LEVEL_CELL, 100);
    s.initialise(&m);
    assert!(
        s.classify(0x50000 + T34_CAPACITY_LEVEL100 * ROW_SIZE - 1)
            .is_some()
    );
    // Counts zero do not prevent constructor stores into the allocated capacity.
    assert!(s.store(&mut m, 0x50000 + 0x9000, 17, 4));
    assert_eq!(s.read32(&m, 0x50000 + 0x9000), 17);
}
#[test]
fn catalogue_is_independent_and_retains_unknown_words() {
    let mut m = image();
    m.write32(MODE_ROWS, 1);
    m.write32(MODE_ROWS + 4, 42);
    m.write32(MODE_ROWS + 0x7c, 0x12345678);
    m.write32(MODE_ROWS + 0x80 + 4, 77); // Marker zero: lookup terminates.
    let catalog = MissionCatalog::from_guest(&m);
    m.write32(MODE_ROWS + 4, 99);
    assert_eq!(catalog.find_mode(42).unwrap().mode_id, 42);
    assert!(catalog.find_mode(77).is_none());
    assert_eq!(catalog.raw_row(0).unwrap()[31], 0x12345678);
    assert!(catalog.row(87).is_none());
    assert_eq!(catalog.read8(MODE_ROWS - 1), 0);
}

#[test]
fn catalogue_loader_consumes_owned_xbe_bytes_at_an_independent_address() {
    use tsfp_state::tables::{OwnedTables, TableSpec};
    let mut xbe = vec![0u8; 0x400 + 87 * 0x80];
    xbe[..4].copy_from_slice(b"XBEH");
    for (offset, value) in [
        (0x104, 0x10000u32),
        (0x11c, 1),
        (0x120, 0x10200),
        (0x204, 0x20000),
        (0x208, 87 * 0x80),
        (0x20c, 0x400),
        (0x210, 87 * 0x80),
        (0x400, 1),
        (0x404, 42),
        (0x47c, 0xabcdef),
    ] {
        xbe[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    static SPECS: &[TableSpec] = &[TableSpec {
        name: "mode_rows",
        va: 0x20000,
        size: 87 * 0x80,
        stride: 0x80,
        count: 87,
        sha256: "",
    }];
    // FABRICATED XBE: the existing Tables tests cover hash enforcement; no retail pin is bypassed at runtime.
    let tables = OwnedTables::from_xbe_with(&xbe, SPECS, false).unwrap();
    xbe.fill(0);
    let catalog = MissionCatalog::from_tables(&tables).unwrap();
    drop(tables);
    assert_eq!(catalog.find_mode(42).unwrap().mode_id, 42);
    assert_eq!(catalog.raw_row(0).unwrap()[31], 0xabcdef);
}
