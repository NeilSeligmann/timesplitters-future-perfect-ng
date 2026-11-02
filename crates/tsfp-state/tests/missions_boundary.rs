// SPDX-License-Identifier: GPL-3.0-or-later
//! T2041: round trip between the tsfp-state missions owner and the tsfp-ports `missions_system` boundary.
//! The boundary source is included by path (tsfp-ports is excluded from the cargo workspace) with the
//! `crate::memory::GuestMemory` it expects mapped to the tsfp-state trait. FABRICATED synthetic image.
use std::collections::BTreeMap;
use tsfp_state::missions::{DrawCursor, LinkRow, PickupCells, PickupRow};

mod memory {
    pub use tsfp_state::GuestMemory;
}
#[allow(dead_code)]
#[path = "../../tsfp-port-wire/src/missions_system.rs"]
mod missions_system;
use missions_system::{GuestTables, PointTables};

#[derive(Default)]
struct Mem(BTreeMap<u32, u8>);
impl tsfp_state::GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}

#[test]
fn boundary_reads_what_the_owner_wrote() {
    let mut m = Mem::default();
    let mut cells = PickupCells::load(&m);
    cells.set_ptr(&mut m, 0, 0x10000);
    cells.set_ptr(&mut m, 1, 0x30000);
    cells.set_main_count(&mut m, 5);
    for i in 0..5u32 {
        let mut row = PickupRow::load(&m, cells.main_row_address(i));
        for w in 0..8 {
            row.set_word(&mut m, w, i * 0x100 + w as u32 * 0x1000_0001);
        }
        row.set_word(&mut m, 0, 10 + 7 * i);
        row.set_word(&mut m, 2, ((i + 3) << 16) | 0xfffe);
    }
    let mut link = LinkRow::load(&m, cells.link_address(2));
    link.set_word(&mut m, 1, 0xabcd);
    let mut cur = DrawCursor::load(&m);
    cur.set_cursor(&mut m, 0x40000);
    cur.advance(&mut m, 3);
    let t = GuestTables(&m);
    assert_eq!(t.main_rows(), 0x10000);
    assert_eq!(t.links(), 0x30000);
    assert_eq!(t.main_count(), 5);
    for i in 0..5u32 {
        let s = PickupRow::load(&m, cells.main_row_address(i));
        let r = t.main_row(i);
        assert_eq!(r.key, s.key());
        assert_eq!(r.flags, s.flags());
        assert_eq!(r.group, s.group());
        assert_eq!(r.tag, s.tag());
        assert_eq!(r.position_bits, s.position_bits());
    }
    assert_eq!(t.main_row(1).group, -2);
    assert_eq!(t.link_word(2, 1), 0xabcd);
    assert_eq!(t.draw_cursor(), 0x4000c);
    assert_eq!(t.find_key(17), Some(1));
    assert_eq!(t.find_key(38), Some(4));
    assert_eq!(t.find_key(11), None);
}
