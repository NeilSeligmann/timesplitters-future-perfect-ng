// SPDX-License-Identifier: GPL-3.0-or-later
//! T2431 spec tests for the Story progression sequencer (`0x65940`).
//!
//! The retail checks read the cursor and entry tables from the XBE named by `TSFP_XBE` at run time
//! (nothing is embedded or printed, only pass or fail); without it they are skipped and say why.
//! The synthetic checks use a FABRICATED table and always run. Evidence class: spec test, the
//! lifted-original differential is `crates/tsfp-c-diff` `modes_t1` (`t1_00065940`, weak).
use std::collections::HashMap;
use tsfp_gameplay::memory::GuestMemory as ModesMemory;
use tsfp_gameplay::modes::{
    ModeHost,
    story_sequencer::{
        BLOCK_BYTES, CURSOR, CURSOR_TABLE, CursorRow, ENTRY_STRIDE, ENTRY_TABLE, PHASE, Phase,
        ROWS, StoryCursorRow, StoryIntroDescriptor, StorySetupInput, entry_mode, plan_story_setup,
        story_progression_sequencer,
    },
};
use tsfp_state::{
    GuestMemory as StateMemory,
    story_flow::{ACTIVE_SETUP, STAGING_SETUP, SessionSetup, copy_setup},
    xbe::XbeImage,
};

const BLOCK: u32 = 0x0078_ad58;

fn setup_input(identifier: u32, stage: Phase, flags: u32) -> StorySetupInput {
    StorySetupInput {
        current_identifier: identifier,
        cursor_ordinal: 4,
        setup_stage: stage,
        mode_flags: flags,
        row: StoryCursorRow {
            level_word: 0x0042,
            player_levels: [0x21, 0x22],
            pre_id: 3,
            post_id: 7,
        },
        descriptor: Some(StoryIntroDescriptor {
            identifier: 0x18,
            reserved: [0xa5; 0x23],
        }),
    }
}

#[test]
fn typed_setup_plan_preserves_guard_order_and_sentinels() {
    let plan = plan_story_setup(setup_input(0x6a, Phase::PostLevel, 0x0480_0000));
    assert_eq!(plan.identifier, Some(0x65)); // first flag wins over second and identifier 0x6a

    let plan = plan_story_setup(setup_input(0x6a, Phase::PostLevel, 0x0400_0000));
    assert_eq!(plan.identifier, Some(0x66));

    let plan = plan_story_setup(setup_input(0x68, Phase::PostLevel, 0));
    assert_eq!(plan.identifier, Some(0x65));

    let mut input = setup_input(0x6a, Phase::PostLevel, 0);
    input.row.post_id = u32::MAX;
    let plan = plan_story_setup(input);
    assert_eq!(plan.next_cursor, Some(5));
    assert_eq!(plan.next_stage, Some(Phase::PreLevel));
    assert_eq!(plan.identifier, None);

    let plan = plan_story_setup(setup_input(0x6a, Phase::PostLevel, 0));
    assert_eq!(plan.identifier, Some(0x18));
    assert_eq!(plan.active_type, Some(10));
    assert_eq!(plan.selector, Some(2));
    assert_eq!(plan.flags, Some(0x2000_0000));
    assert_eq!(plan.movie_index, Some(7));

    let plan = plan_story_setup(setup_input(0, Phase::Level, 0));
    assert_eq!(plan.identifier, Some(0x42));
    assert_eq!(plan.flags, Some(0x0008_0008));
}

/// Guest image: optional retail bytes below a writable overlay. Callee results are fixed.
struct Image<'a> {
    base: Option<&'a XbeImage<'a>>,
    overlay: HashMap<u32, u8>,
    calls: Vec<u32>,
}

impl Image<'_> {
    fn byte(&self, address: u32) -> u8 {
        self.overlay
            .get(&address)
            .copied()
            .or_else(|| {
                self.base
                    .and_then(|x| x.file_bytes(address, 1))
                    .map(|b| b[0])
            })
            .unwrap_or(0)
    }
    fn word(&self, address: u32) -> u32 {
        u32::from_le_bytes(core::array::from_fn(|i| self.byte(address + i as u32)))
    }
    fn put32(&mut self, address: u32, value: u32) {
        for (i, b) in value.to_le_bytes().into_iter().enumerate() {
            self.overlay.insert(address + i as u32, b);
        }
    }
}
impl ModesMemory for Image<'_> {
    fn read8(&self, address: u32) -> u8 {
        self.byte(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.overlay.insert(address, value);
    }
}
impl StateMemory for Image<'_> {
    fn read8(&self, address: u32) -> u8 {
        self.byte(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.overlay.insert(address, value);
    }
}
impl ModeHost for Image<'_> {
    fn call(&mut self, target: u32, _: &[u32]) -> u32 {
        self.calls.push(target);
        0
    }
    fn call_f32(&mut self, _: u32, _: &[u32]) -> f32 {
        0.0
    }
    fn call_x87_unary(&mut self, _: u32, argument: f32) -> f32 {
        argument
    }
    fn stack_pointer(&self) -> u32 {
        0x0030_0000
    }
}

fn run_with<F: FnOnce(&mut Image<'_>)>(base: Option<&XbeImage<'_>>, setup: F) -> Image<'static> {
    // The image borrows nothing past this call: results are copied out of the overlay.
    let mut image = Image {
        base,
        overlay: HashMap::new(),
        calls: vec![],
    };
    setup(&mut image);
    story_progression_sequencer(&mut image, BLOCK);
    Image {
        base: None,
        overlay: image.overlay,
        calls: image.calls,
    }
}

fn retail<R>(check: impl FnOnce(&XbeImage<'_>) -> R) -> Option<R> {
    let Some(path) = std::env::var_os("TSFP_XBE") else {
        eprintln!("skipped: TSFP_XBE not set (retail Story table check)");
        return None;
    };
    let data = std::fs::read(path).expect("TSFP_XBE is readable");
    let image = XbeImage::parse(&data).expect("TSFP_XBE parses");
    let table = image.file_bytes(CURSOR_TABLE, ROWS as u32 * 0x44);
    let entries = image.file_bytes(ENTRY_TABLE, 256 * ENTRY_STRIDE);
    if table.is_none() || entries.is_none() {
        eprintln!(
            "skipped: the Story tables are not file-backed in this XBE (not the retail layout)"
        );
        return None;
    }
    Some(check(&image))
}

#[test]
fn retail_row_12_yields_level_0x51() {
    retail(|xbe| {
        let image = run_with(Some(xbe), |m| {
            m.put32(CURSOR, 12);
            m.put32(PHASE, 1);
            m.overlay.insert(0x0052_36c4, 1);
        });
        assert_eq!(image.byte(BLOCK + 0x54), 0x51);
        assert_eq!(image.byte(BLOCK + 0x55), 0x0a);
        assert_eq!(image.byte(BLOCK + 0x56), 2);
    });
}

#[test]
fn retail_row_0_pre_id_gives_mode_0x18() {
    retail(|xbe| {
        let image = run_with(Some(xbe), |m| {
            m.put32(CURSOR, 0);
            m.put32(PHASE, 0);
            m.overlay.insert(0x0052_36c4, 1);
        });
        let probe = Image {
            base: Some(xbe),
            overlay: HashMap::new(),
            calls: vec![],
        };
        let pre = CursorRow::read(&probe, 0).pre_id;
        assert_eq!(pre, 0, "row 0 pre-level entry id");
        assert_eq!(entry_mode(&probe, pre), 0x18);
        assert_eq!(image.byte(BLOCK + 0x54), 0x18);
        assert_eq!(
            image.byte(BLOCK + 0x9b),
            0,
            "the entry id is recorded in the block"
        );
        assert_eq!(image.word(PHASE), 0, "an intro entry keeps phase 0");
    });
}

/// FABRICATED table: every row has level `row + 1` and no entries except row 0 (pre id 5).
fn synthetic(m: &mut Image<'_>) {
    for row in 0..ROWS {
        let base = CursorRow::address(row);
        m.overlay.insert(base + 6, row as u8 + 1);
        m.put32(base + 0x20, if row == 0 { 5 } else { u32::MAX });
        m.put32(base + 0x24, u32::MAX);
    }
    m.overlay.insert(ENTRY_TABLE + 5 * ENTRY_STRIDE, 0x18);
    m.overlay.insert(0x0052_36c4, 1);
}

#[test]
fn synthetic_cursor_walks_through_intro_level_and_advance() {
    // phase 0 on row 0: intro entry block (mode byte of entry 5), phase stays 0
    let image = run_with(None, |m| {
        synthetic(m);
        m.put32(CURSOR, 0);
        m.put32(PHASE, 0);
    });
    assert_eq!(image.byte(BLOCK + 0x54), 0x18);
    assert_eq!(image.word(PHASE), 0);
    // phase 0 on row 3 (no pre id): level block, phase 1
    let image = run_with(None, |m| {
        synthetic(m);
        m.put32(CURSOR, 3);
        m.put32(PHASE, 0);
    });
    assert_eq!(image.byte(BLOCK + 0x54), 4);
    assert_eq!(image.word(PHASE), 1);
}

#[test]
fn advancing_to_next_level_keeps_setup_paired_with_selected_cursor() {
    let mut image = Image {
        base: None,
        overlay: HashMap::new(),
        calls: vec![],
    };
    synthetic(&mut image);
    image.put32(CURSOR, 0);
    image.put32(PHASE, 3);

    let output = story_progression_sequencer(&mut image, BLOCK)
        .expect("advance should stage the next Story level");

    assert_eq!(output.campaign_ordinal().get(), 1);
    assert_eq!(output.setup().bytes()[0x54], 2);
    assert_eq!(image.word(CURSOR), 1);
    assert_eq!(image.word(PHASE), 1);
}

#[test]
fn level_block_feeds_the_0x658d0_copy() {
    // The block the sequencer builds is what 0x658D0 stages for 0x670C0.
    let mut image = run_with(None, |m| {
        synthetic(m);
        m.put32(CURSOR, 12);
        m.put32(PHASE, 1);
    });
    copy_setup(&mut image, BLOCK, STAGING_SETUP);
    let staged = SessionSetup::load(&image, STAGING_SETUP);
    assert_eq!(staged.level_id(), 13);
    assert_eq!(staged.player_count(), 1);
    assert_eq!(staged.byte(0x55), 0x0a);
    let _ = (ACTIVE_SETUP, BLOCK_BYTES);
}
