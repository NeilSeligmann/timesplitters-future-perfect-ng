// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use std::collections::BTreeMap;
use tsfp_state::GuestMemory;
use tsfp_state::game_state::*;
use tsfp_state::inventory::*;
use tsfp_state::player::*;
use tsfp_state::snapshots::*;

#[derive(Clone, PartialEq)]
struct Image(BTreeMap<u32, u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}

const REC: u32 = 0x43dc_a6b0;
const EXT: u32 = 0x43e0_0000;

fn image() -> Image {
    let mut m = Image(BTreeMap::new());
    m.write32(RECORD_CELL, REC);
    m.write32(CONTROLLED_CELL, REC);
    m.write32(REC + VIEWER_STATE_OFFSET, 0x100);
    m.write32(LEVEL, 2);
    m.write32(PLAYERS, 2);
    m.write8(MODE, 0xa);
    m.write32(FLAGS, 0x40);
    m.write32(EXT_POINTER, EXT);
    for (i, cell) in HUD_CELLS.iter().enumerate() {
        m.write32(cell.address, 0x1000 + i as u32);
    }
    for (i, cell) in MENU_CELLS.iter().enumerate() {
        if cell.width == 4 {
            m.write32(cell.address, 0x2000 + i as u32);
        } else {
            m.write8(cell.address, 0x7e);
        }
    }
    m.write32(EXT + EXT_CURRENT, 7);
    m.write32(EXT + EXT_STRIDE + EXT_CURRENT, 9);
    m
}

#[test]
fn hud_matches_direct_reads() {
    let m = image();
    let s = HudSnapshot::build_from_guest(&m, None);
    assert_eq!(s.level, m.read32(LEVEL));
    assert_eq!(s.viewer_record, REC);
    assert_eq!(s.viewer_state, Some(0x100));
    assert!(
        !s.prompt_allowed,
        "level <= 0x64 and state 0x100 vetoes the prompt"
    );
    assert_eq!(s.inventory[0].unwrap().current, 7);
    assert_eq!(s.inventory[1].unwrap().current, 9);
    assert!(s.inventory[2].is_none(), "only `players` slots are read");
    for cell in HUD_CELLS {
        assert_eq!(
            s.cell(cell.name),
            Some(m.read32(cell.address)),
            "{}",
            cell.name
        );
    }
}

#[test]
fn menu_matches_direct_reads() {
    let m = image();
    let s = MenuSnapshot::build_from_guest(&m);
    assert_eq!(s.mode, 0xa);
    for cell in MENU_CELLS {
        let want = if cell.width == 1 {
            u32::from(m.read8(cell.address))
        } else {
            m.read32(cell.address)
        };
        assert_eq!(s.cell(cell.name), Some(want), "{}", cell.name);
    }
    assert_eq!(s.cell("menu_78adae"), Some(0x7e));
}

#[test]
fn prompt_gate_follows_42f90() {
    assert!(prompt_allowed(0, 2, Some(0)));
    assert!(prompt_allowed(0, 2, None), "no controlled object");
    assert!(prompt_allowed(0, 0x65, Some(0x100)), "level above 0x64");
    assert!(!prompt_allowed(0x200, 2, Some(0)));
    assert!(!prompt_allowed(0x8000_0000, 2, Some(0)));
}

#[test]
fn no_player_record_in_menus() {
    let mut m = image();
    m.write32(RECORD_CELL, 0);
    m.write32(CONTROLLED_CELL, 0);
    let s = HudSnapshot::build_from_guest(&m, None);
    assert_eq!((s.viewer_record, s.viewer_state), (0, None));
    assert!(s.prompt_allowed);
}

#[test]
fn mutation_is_visible() {
    let mut m = image();
    let before = HudSnapshot::build_from_guest(&m, None);
    m.write32(HUD_CELLS[3].address, 0xdead);
    assert_ne!(HudSnapshot::build_from_guest(&m, None), before);
    let menu = MenuSnapshot::build_from_guest(&m);
    m.write32(MENU_CELLS[5].address, 0xbeef);
    assert_ne!(MenuSnapshot::build_from_guest(&m), menu);
}
