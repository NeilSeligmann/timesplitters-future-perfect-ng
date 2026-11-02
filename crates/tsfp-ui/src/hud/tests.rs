// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 0 smoke tests of the HUD slice on synthetic guest memory (no game data). The Tier 1
//! differential against the original is `tests/hud_oracle.rs`.

use super::addr::*;
use super::draw::{DrawCall, RecordingDraw};
use super::health;
use super::letterbox;
use super::messages;
use super::primitives;
use super::snapshot::Snapshot;
use super::state::PlayerHudState;
use super::text::{ids, MapText, TextProvider};
use super::widgets;
use super::x86::*;
use crate::memory::GuestMemory;
use std::collections::HashMap;

#[derive(Default)]
struct Mem(HashMap<u32, u8>);

impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

fn mem_with_constants() -> Mem {
    let mut m = Mem::default();
    write_f32(&mut m, CONST_ONE, 1.0);
    write_f32(&mut m, CONST_LETTERBOX_RATE, 0.5);
    write_f32(&mut m, CONST_LETTERBOX_MIN, 0.01);
    write_f32(&mut m, CONST_BAR_SCALE, 60.0);
    write_f32(&mut m, CONST_FADE, 3.0);
    write_f32(&mut m, CONST_MSG_END, 5.0);
    write_f32(&mut m, CONST_HALF, 0.5);
    write_f32(&mut m, CONST_255, 255.0);
    m.write32(FRAME_RATE_INT, 60);
    write_f32(&mut m, FRAME_STEP_F32, 1.0);
    m
}

#[test]
fn letterbox_fades_in_holds_and_fades_out() {
    let mut m = mem_with_constants();
    m.write8(MODE_BYTE, 1);
    letterbox::start_fade_in(&mut m, 2, 0);
    let mut frames = 0;
    while m.read32(LETTERBOX_STATE) == 1 {
        letterbox::update(&mut m);
        frames += 1;
        assert!(frames < 1000);
    }
    assert_eq!(read_f32(&m, LETTERBOX_AMOUNT), 1.0);
    assert_eq!(m.read32(LETTERBOX_STATE), 0);
    m.write32(FRAME_DELTA_INT, 1);
    letterbox::update(&mut m);
    letterbox::update(&mut m);
    assert_eq!(m.read32(LETTERBOX_STATE), u32::MAX, "hold of 2 frames then fade out");
    while m.read32(LETTERBOX_STATE) == u32::MAX {
        letterbox::update(&mut m);
    }
    assert_eq!(read_f32(&m, LETTERBOX_AMOUNT), 0.0);
}

#[test]
fn letterbox_draw_emits_two_bars() {
    let mut m = mem_with_constants();
    m.write32(VIEWPORT_PTR, 0x1000);
    m.write32(0x1004, 639);
    m.write32(0x1014, 0);
    m.write32(0x1018, 479);
    write_f32(&mut m, LETTERBOX_AMOUNT, 0.5);
    let mut d = RecordingDraw::default();
    letterbox::draw(&m, &mut d);
    let fills: Vec<_> = d
        .calls
        .iter()
        .filter(|c| matches!(c, DrawCall::FillRect { .. }))
        .collect();
    assert_eq!(fills.len(), 2);
    assert_eq!(
        *fills[1],
        DrawCall::FillRect { left: 0, top: 0, right: 640, bottom: 31, alpha: 0x80 }
    );
}

#[test]
fn message_ring_round_robin_and_clear() {
    let mut m = mem_with_constants();
    m.0.extend(b"Hello\0".iter().enumerate().map(|(i, b)| (0x2000 + i as u32, *b)));
    for _ in 0..17 {
        messages::push_notice(&mut m, 1, 0x2000, 7, 0.0);
    }
    let block = HUD_BLOCK_BASE + HUD_BLOCK_STRIDE;
    assert_eq!(m.read32(block + BLOCK_RING_CURSOR), 1, "17 pushes wrap the 16 entry ring");
    let entry = messages::entry_address(block, 0);
    assert_eq!(messages::entry_text(&m, entry), "Hello");
    assert_eq!(m.read32(entry + MSG_KIND), messages::KIND_NOTICE);
    messages::push_notice_c(&mut m, 9, 0x2000, 0, 0.0);
    assert_eq!(m.read32(HUD_BLOCK_BASE + 9 * HUD_BLOCK_STRIDE + BLOCK_RING_CURSOR), 0, "bad player ignored");
    m.write32(messages::entry_address(block, 3) + MSG_KIND, 4);
    messages::clear_kind(&mut m, 1, -1);
    assert_eq!(m.read32(messages::entry_address(block, 3) + MSG_KIND), 4, "kind 4 survives clear-all");
    assert_eq!(m.read32(entry + MSG_KIND), 0);
}

#[test]
fn fade_alpha_is_one_until_fade_start() {
    assert_eq!(messages::fade_alpha(1.0, 3.0, 5.0, 1.0), 1.0);
    assert_eq!(messages::fade_alpha(4.0, 3.0, 5.0, 1.0), 0.5);
}

#[test]
fn health_flags_follow_thresholds() {
    let mut m = mem_with_constants();
    for i in 0..THRESHOLD_COUNT {
        write_f32(&mut m, HEALTH_THRESHOLDS + i * THRESHOLD_STRIDE, (i as f32 + 1.0) / 20.0);
    }
    let object = 0x4000;
    write_f32(&mut m, object + OBJ_HEALTH, 50.0);
    write_f32(&mut m, object + OBJ_MAX_HEALTH, 100.0);
    let block = 0x5000;
    health::flags_init(&mut m, block, object);
    let lit: u32 = (0..20).map(|i| m.read32(block + 4 * i)).sum();
    assert_eq!(lit, 10, "50 percent health lights 10 of 20 segments");
    assert!(!health::flags_differ(&m, block, object));
    write_f32(&mut m, object + OBJ_HEALTH, 100.0);
    assert!(health::flags_differ(&m, block, object));
}

#[test]
fn colour_scale_clamps_and_keeps_the_rgb_bytes() {
    assert_eq!(widgets::scale_colour_low_byte(0x1234_56ff, 0.5), 0x1234_567f);
    assert_eq!(widgets::scale_colour_low_byte(0x1234_56ff, 7.0), 0x1234_56ff);
    assert_eq!(widgets::scale_colour_low_byte(0x1234_56ff, -1.0), 0x1234_5600);
}

#[test]
fn textured_rect_is_skipped_off_screen_or_empty() {
    let mut m = Mem::default();
    m.write32(SCREEN_WIDTH_INT, 640);
    m.write32(SCREEN_HEIGHT_INT, 480);
    let descriptor = 0x6000;
    write_f32(&mut m, descriptor + 0x18, 10.0);
    write_f32(&mut m, descriptor + 0x1c, 20.0);
    write_f32(&mut m, descriptor + 0x20, 32.0);
    write_f32(&mut m, descriptor + 0x24, 32.0);
    let mut d = RecordingDraw::default();
    primitives::draw_textured_rect(&m, &mut d, descriptor);
    assert_eq!(d.calls.len(), 1);
    write_f32(&mut m, descriptor + 0x18, 700.0);
    primitives::draw_textured_rect(&m, &mut d, descriptor);
    assert_eq!(d.calls.len(), 1, "origin right of the screen is not drawn");
}

#[test]
fn snapshot_round_trip_and_state_read() {
    let mut bytes = b"HUDSNAP1".to_vec();
    bytes.extend(1u32.to_le_bytes());
    bytes.extend(0x1000u32.to_le_bytes());
    bytes.extend(4u32.to_le_bytes());
    bytes.extend(0xdead_beefu32.to_le_bytes());
    let snapshot = Snapshot::parse(&bytes).unwrap();
    assert_eq!(snapshot.read32(0x1000), 0xdead_beef);
    assert_eq!(snapshot.missing_reads.get(), 0);
    assert_eq!(snapshot.read32(0x2000), 0);
    assert_eq!(snapshot.missing_reads.get(), 4);
    let state = PlayerHudState::read(&Snapshot::default(), 0);
    assert!(state.messages.is_empty());
}

#[test]
fn text_provider_maps_ids() {
    let mut text = MapText::default();
    text.0.insert(ids::SCORE, "SCORE".to_owned());
    assert_eq!(text.label(ids::SCORE).as_deref(), Some("SCORE"));
    assert_eq!(text.label(ids::RANK), None);
}
