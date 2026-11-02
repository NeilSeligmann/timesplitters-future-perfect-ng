//! T2191 Tier 1: typed ammo readout vs `tsfp_ui::hud::state::PlayerHudState::read` (T1642 guest
//! layout), and the `original` ammo layout and crosshair vs the `tsfp_ui::hud::frame` draw calls.
//! Seeded FABRICATED guest images and viewports. The frame stand-in is itself INFERRED/FABRICATED
//! placement, so the layout half proves agreement between the two paths, not original parity.
use std::collections::HashMap;

use tsfp_ui::hud::addr::*;
use tsfp_ui::hud::draw::{DrawCall, RecordingDraw};
use tsfp_ui::hud::frame::draw_frame;
use tsfp_ui::hud::state::PlayerHudState;
use tsfp_ui::hud::text::MapText;
use tsfp_ui::memory::GuestMemory;
use tsfp_ui_model::ammo::{AmmoCounters, AmmoLayout, AmmoReadout, HeldWeapon, WeaponRecord};
use tsfp_ui_model::crosshair::CrosshairStyle;

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

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

const PLAYERS_AT: u32 = 0x0100_0000;
const OBJECT: u32 = 0x0110_0000;
const CHARACTER: u32 = 0x0120_0000;

/// Counters read from the same guest image, as a Rust inventory system would expose them.
struct GuestCounters<'a>(&'a Mem);
impl AmmoCounters for GuestCounters<'_> {
    fn loaded(&self, ammo_type: u32) -> u16 {
        let hand = CHARACTER.wrapping_add(EXT_HAND_B);
        self.0.read16(
            hand.wrapping_add(HAND_LOADED_BY_TYPE)
                .wrapping_add(ammo_type.wrapping_mul(2)),
        )
    }
    fn reserve(&self, ammo_type: u32) -> u16 {
        self.0.read16(
            CHARACTER
                .wrapping_add(EXT_RESERVE_BY_TYPE)
                .wrapping_add(ammo_type.wrapping_mul(2)),
        )
    }
}

fn image(rng: &mut Rng, slot: u32) -> (Mem, u32) {
    let mut m = Mem::default();
    m.write32(CURRENT_PLAYER_PTR, PLAYERS_AT);
    m.write32(
        PLAYERS_AT + slot * PLAYER_RECORD_SIZE + PLAYER_OBJECT,
        OBJECT,
    );
    m.write32(OBJECT + OBJ_CHARACTER, CHARACTER);
    m.write32(CHARACTER + EXT_CURRENT_WEAPON_ENTRY, rng.next() % 4);
    let record_index = match rng.next() % 5 {
        0 => u32::MAX,
        1 => WEAPON_RECORD_COUNT + rng.next() % 4,
        2 => rng.next(),
        _ => rng.next() % WEAPON_RECORD_COUNT,
    };
    m.write32(CHARACTER + EXT_HAND_B + HAND_RECORD_INDEX, record_index);
    for index in 0..WEAPON_RECORD_COUNT {
        let record = WEAPON_RECORDS + index * WEAPON_RECORD_SIZE;
        let ammo_type = if rng.next() % 8 == 0 {
            rng.next()
        } else {
            rng.next() % 12
        };
        m.write32(record + WEAPON_AMMO_TYPE_1, ammo_type);
        m.write32(record + WEAPON_CLIP_SIZE_1, rng.next() % 200);
    }
    for ammo_type in 0..12 {
        let hand = CHARACTER + EXT_HAND_B;
        write16(
            &mut m,
            hand + HAND_LOADED_BY_TYPE + ammo_type * 2,
            rng.next() as u16,
        );
        write16(
            &mut m,
            CHARACTER + EXT_RESERVE_BY_TYPE + ammo_type * 2,
            rng.next() as u16,
        );
    }
    (m, record_index)
}

fn records(m: &Mem) -> Vec<WeaponRecord> {
    (0..WEAPON_RECORD_COUNT)
        .map(|index| {
            let record = WEAPON_RECORDS + index * WEAPON_RECORD_SIZE;
            WeaponRecord {
                ammo_type: m.read32(record + WEAPON_AMMO_TYPE_1),
                clip_size: m.read32(record + WEAPON_CLIP_SIZE_1),
            }
        })
        .collect()
}

#[test]
fn ammo_readout_matches_guest_view() {
    let mut rng = Rng(0x2191_a110);
    let mut armed = 0;
    for seed in 0..4000 {
        let slot = seed % 4;
        let (m, record_index) = image(&mut rng, slot);
        let guest = PlayerHudState::read(&m, slot);
        let held = HeldWeapon {
            entry: m.read32(CHARACTER + EXT_CURRENT_WEAPON_ENTRY),
            record_index,
        };
        let model = AmmoReadout::of(held, &records(&m), &GuestCounters(&m));
        let expected = (
            guest.weapon_entry,
            guest.ammo_type,
            guest.clip_size,
            guest.ammo_loaded,
            guest.ammo_reserve,
        );
        let actual = (
            model.weapon_entry,
            model.ammo_type,
            model.clip_size,
            model.loaded,
            model.reserve,
        );
        assert_eq!(actual, expected, "seed {seed}");
        armed += usize::from(guest.ammo_loaded != 0);
    }
    assert!(armed > 1000, "too few armed cases: {armed}");
}

#[test]
fn ammo_layout_and_crosshair_match_frame_stand_in() {
    let mut rng = Rng(0x2191_c805);
    let mut texts = 0;
    for seed in 0..2000 {
        let left = (rng.next() % 200) as i32;
        let top = (rng.next() % 200) as i32;
        let viewport = [
            left,
            left + 64 + (rng.next() % 1200) as i32,
            top,
            top + 64 + (rng.next() % 900) as i32,
        ];
        let state = PlayerHudState {
            viewport,
            health: 1.0,
            max_health: 1.0,
            weapon_entry: rng.next() % 4,
            ammo_loaded: rng.next() % 1000,
            ammo_reserve: rng.next() % 1000,
            ..PlayerHudState::default()
        };
        let mut draw = RecordingDraw::default();
        draw_frame(
            &Mem::default(),
            &mut draw,
            &MapText(Default::default()),
            &state,
        );
        let mut expected_texts = Vec::new();
        let readout = AmmoReadout {
            visible: state.weapon_entry != 1,
            weapon_entry: state.weapon_entry,
            loaded: state.ammo_loaded,
            reserve: state.ammo_reserve,
            ..AmmoReadout::default()
        };
        if readout.visible {
            let layout = AmmoLayout::original(viewport);
            expected_texts.push((layout.reserve_at, readout.reserve.to_string()));
            expected_texts.push((layout.loaded_at, readout.loaded.to_string()));
            expected_texts.push((
                layout.weapon_label_at,
                format!("weapon {}", readout.weapon_entry),
            ));
        }
        let drawn_texts: Vec<_> = draw
            .calls
            .iter()
            .filter_map(|call| match call {
                DrawCall::Text { x, y, text, .. } => Some(((*x, *y), text.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(drawn_texts, expected_texts, "seed {seed}");
        texts += drawn_texts.len();
        let drawn_arms: Vec<_> = draw
            .calls
            .iter()
            .filter_map(|call| match call {
                DrawCall::FillRect {
                    left,
                    top,
                    right,
                    bottom,
                    alpha: 0xff,
                } => Some((*left, *top, *right, *bottom)),
                _ => None,
            })
            .take(4)
            .collect();
        let style = CrosshairStyle::ORIGINAL;
        let model_arms: Vec<_> = style
            .arms(viewport)
            .iter()
            .map(|arm| (arm.left, arm.top, arm.right, arm.bottom))
            .collect();
        assert_eq!(drawn_arms.len(), 4);
        assert_eq!(drawn_arms, model_arms, "seed {seed}");
    }
    assert!(texts > 3000);
}

fn write16(m: &mut Mem, address: u32, value: u16) {
    m.write8(address, value as u8);
    m.write8(address + 1, (value >> 8) as u8);
}
