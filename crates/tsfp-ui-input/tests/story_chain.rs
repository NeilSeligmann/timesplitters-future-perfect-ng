// SPDX-License-Identifier: GPL-3.0-or-later
//! T2431: the Story selection chain end to end over Rust-owned handlers: row callback 0x2CAFE0,
//! difficulty 0x2CAD20, confirm 0x2BEAA0 (`frontend_handlers`), the page tick 0x2D3BF0 and the mode 4
//! branch of 0x2D2C70 (`tsfp_gameplay::modes::story_menu`), then the sequencer 0x65940 filling the mode
//! block. Evidence class: spec test over a shared memory image, not a lifted-original differential.
//!
//! With `TSFP_XBE=<retail default.xbe>` the Story tables are read from the XBE at run time (only pass
//! or fail is printed, nothing is embedded) and row 12 must yield level 0x51. Without it that test
//! skips with a reason; the FABRICATED-table variant always runs and checks the wiring.
use std::collections::HashMap;

use tsfp_gameplay::{
    memory::GuestMemory as ModesMemory,
    modes::{
        ModeHost, story_menu,
        story_sequencer::{
            self, BLOCK_BYTES, CURSOR, CURSOR_TABLE, CursorRow, ENTRY_STRIDE, ENTRY_TABLE, PHASE,
            ROWS,
        },
    },
};
use tsfp_state::xbe::XbeImage;
use tsfp_ui_input::{
    frontend_handlers::dispatch_selected,
    native_page::{GuestUi, PageRef},
};

const PAGE: u32 = 0x0030_0000;
const CONTEXT: u32 = 0x0031_0000;
const BLOCK: u32 = story_menu::MODE_BLOCK;

/// One memory image shared by the UI handlers and the gameplay ports (as the guest memory is).
#[derive(Default)]
struct Image {
    bytes: HashMap<u32, u8>,
    calls: Vec<(u32, Vec<u32>)>,
}
impl Image {
    fn byte(&self, address: u32) -> u8 {
        self.bytes.get(&address).copied().unwrap_or(0)
    }
    fn word(&self, address: u32) -> u32 {
        u32::from_le_bytes(core::array::from_fn(|i| self.byte(address + i as u32)))
    }
    fn put32(&mut self, address: u32, value: u32) {
        for (i, b) in value.to_le_bytes().into_iter().enumerate() {
            self.bytes.insert(address + i as u32, b);
        }
    }
}

struct Ui<'a>(&'a mut Image);
impl GuestUi for Ui<'_> {
    fn read32(&self, va: u32) -> Option<u32> {
        Some(self.0.word(va))
    }
    fn write32(&mut self, va: u32, value: u32) -> bool {
        self.0.put32(va, value);
        true
    }
    fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
        self.0.calls.push((va, args.to_vec()));
        Some(0)
    }
}

struct Modes<'a>(&'a mut Image);
impl ModesMemory for Modes<'_> {
    fn read8(&self, address: u32) -> u8 {
        self.0.byte(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.bytes.insert(address, value);
    }
}
impl ModeHost for Modes<'_> {
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        self.0.calls.push((target, args.to_vec()));
        // The page is not finished (0x70D90 != 0); every other service reports 0 (no session,
        // briefings not skipped).
        u32::from(target == 0x70d90)
    }
    fn call_f32(&mut self, _: u32, _: &[u32]) -> f32 {
        0.0
    }
    fn call_x87_unary(&mut self, _: u32, argument: f32) -> f32 {
        argument
    }
    fn stack_pointer(&self) -> u32 {
        0x0040_0000
    }
}

/// Runs the whole chain for cursor `row` and returns the image after the level phase.
fn run_chain(mut image: Image, row: u32) -> Image {
    image.put32(0x0052_36c4, 1);
    image.put32(0x0076_13e4, CONTEXT);
    image.put32(PAGE + 0x18c, row);

    // 1. Story list row 0x2CAFE0: menu mode 4, cursor = row.
    assert!(dispatch_selected(
        &mut Ui(&mut image),
        PageRef(PAGE),
        0x2cafe0
    ));
    assert_eq!(image.word(story_menu::MENU_MODE), 4);
    assert_eq!(image.word(CURSOR), row);
    assert_eq!(
        image.calls.last().unwrap(),
        &(0x79b70, vec![CONTEXT, 0x5238d0])
    );

    // 2. Difficulty page 0x2CAD20: the third selector.
    image.put32(PAGE + 0x18c, 2);
    assert!(dispatch_selected(
        &mut Ui(&mut image),
        PageRef(PAGE),
        0x2cad20
    ));
    assert_eq!(image.word(0x0078_ad54), 2);
    assert_eq!(
        image.calls.last().unwrap(),
        &(0x79b70, vec![CONTEXT, 0x5238c0])
    );

    // 3. Confirm page 0x2BEAA0 pushes the page whose tick starts the Story.
    assert!(dispatch_selected(
        &mut Ui(&mut image),
        PageRef(PAGE),
        0x2beaa0
    ));
    assert_eq!(
        image.calls.last().unwrap(),
        &(0x79b70, vec![CONTEXT, 0x5236dc])
    );

    // 4. The page tick 0x2D3BF0 hands over to 0x2D38C0 once the match state is 1.
    image.put32(story_menu::MATCH_STATE, 1);
    story_menu::story_page_tick(&mut Modes(&mut image), PAGE);
    assert_eq!(image.calls.last().unwrap(), &(0x2d38c0, vec![]));

    // 5. 0x2D38C0 runs 0x2D2C70 in mode 4: phase 0 and the sequencer on the block.
    assert!(story_menu::setup_mode_4(&mut Modes(&mut image)));
    assert_eq!(
        image.word(CURSOR),
        row,
        "the cursor is the Rust-owned selection"
    );

    // 6. After the pre-level entry the level phase runs (0x65C70 stores phase 1 and calls the
    // sequencer again): the mode block carries the level of the row.
    image.put32(PHASE, 1);
    story_sequencer::story_progression_sequencer(&mut Modes(&mut image), BLOCK);
    image
}

#[test]
fn fabricated_table_chain_wires_selection_to_the_level_block() {
    // FABRICATED: every row's level is 0x30 + row, no entries.
    let mut image = Image::default();
    for row in 0..ROWS {
        let base = CursorRow::address(row);
        // StoryCursorRow::read consumes the low byte of the level word at +6.
        image.bytes.insert(base + 6, 0x30 + row as u8);
        image.put32(base + 0x1c, u32::MAX);
        image.put32(base + 0x20, u32::MAX);
    }
    let image = run_chain(image, 12);
    assert_eq!(image.byte(BLOCK + 0x54), 0x3c);
    assert_eq!(image.word(PHASE), 1);
}

#[test]
fn retail_row_12_selection_yields_level_0x51() {
    let Some(path) = std::env::var_os("TSFP_XBE") else {
        eprintln!("skipped: TSFP_XBE not set (retail Story chain check)");
        return;
    };
    let data = std::fs::read(path).expect("TSFP_XBE is readable");
    let xbe = XbeImage::parse(&data).expect("TSFP_XBE parses");
    let (Some(table), Some(entries)) = (
        xbe.file_bytes(CURSOR_TABLE, ROWS as u32 * 0x44),
        xbe.file_bytes(ENTRY_TABLE, 256 * ENTRY_STRIDE),
    ) else {
        eprintln!(
            "skipped: the Story tables are not file-backed in this XBE (not the retail layout)"
        );
        return;
    };
    let mut image = Image::default();
    for (offset, byte) in table.iter().enumerate() {
        image.bytes.insert(CURSOR_TABLE + offset as u32, *byte);
    }
    for (offset, byte) in entries.iter().enumerate() {
        image.bytes.insert(ENTRY_TABLE + offset as u32, *byte);
    }
    let image = run_chain(image, 12);
    assert_eq!(image.byte(BLOCK + 0x54), 0x51, "row 12 level");
    assert_eq!(image.byte(BLOCK + 0x55), 0x0a);
    assert_eq!(image.byte(BLOCK + 0x56), 2);
    assert!(BLOCK_BYTES > 0x56);
}
