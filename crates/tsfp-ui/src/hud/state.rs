// SPDX-License-Identifier: GPL-3.0-or-later
//! Attachment point 1: what the HUD reads about a player.
//!
//! [`PlayerHudState`] is the *view* the HUD consumes. Today it is derived from guest records
//! ([`PlayerHudState::read`]); a Rust player/inventory system can fill the same struct directly.
//! Every field names its guest source and confidence. Sources come from
//! `tools/data/struct_layouts.json` (TsfpObject, TsfpCharacter, TsfpWeaponRecord) and
//! docs/t-player-inventory.md.

use crate::hud::addr::*;
use crate::hud::messages;
use crate::hud::x86::*;
use crate::memory::GuestMemory;

/// One live message of the ring (kind, text, age, thresholds). Source: HUD block ring entries.
#[derive(Clone, Debug, PartialEq)]
pub struct HudMessage {
    pub kind: u32,
    pub text: String,
    pub age: f32,
    pub fade_start: f32,
    pub end: f32,
    pub entry_address: u32,
}

/// Everything the HUD reads about one local player.
#[derive(Clone, Debug, Default)]
pub struct PlayerHudState {
    /// HUD player slot (0..4); the HUD block is `0x7A6160 + slot * 0x177C`.
    pub slot: u32,
    /// Character object address (`[player record + 0x14]`). MEASURED.
    pub object: u32,
    /// Health: object `+0x1CC` (f32). MEASURED (struct_layouts TsfpObject.health).
    pub health: f32,
    /// Maximum health: object `+0x1D8` (f32). MEASURED.
    pub max_health: f32,
    /// Armour pool: character `+0x78` (f32), character = object `+0x7C`. MEASURED (T1741).
    pub armor: f32,
    /// Armour maximum used by the HUD segments: character `+0x7C` (f32). INFERRED (table 0x4E6F28).
    pub armor_max: f32,
    /// Lit health segments (20): `health >= table[i] * max_health`. Derived, MEASURED rule.
    pub health_segments: [bool; 20],
    /// Lit armour segments (20), characters only.
    pub armor_segments: [bool; 20],
    /// Current weapon entry: character/ext `+0x94`. MEASURED (T1642).
    pub weapon_entry: u32,
    /// Held weapon record index: ext hand B `+0x1D4 + 0x30`. MEASURED (T1642).
    pub weapon_record: u32,
    /// Ammo type 1 of that record: weapon record `+0x14`. INFERRED.
    pub ammo_type: u32,
    /// Clip size 1 of that record: weapon record `+0x18`. INFERRED.
    pub clip_size: u32,
    /// Rounds loaded: hand B `+0x38 + 2 * ammo_type` (u16). MEASURED (T1642).
    pub ammo_loaded: u32,
    /// Reserve rounds: ext `+0x61E + 2 * ammo_type` (u16). MEASURED (T1642).
    pub ammo_reserve: u32,
    /// Live messages of the slot's ring (kind != 0), in ring order. MEASURED layout (T1711).
    pub messages: Vec<HudMessage>,
    /// Letterbox amount 0..1: `0x7490EC`. MEASURED.
    pub letterbox: f32,
    /// Mode byte `0x7DE455` (1 score race, 3 timed...). MEASURED address, meanings INFERRED.
    pub mode_byte: u32,
    /// Remaining time in frames for the timed modes: `0x7DE318`. INFERRED (0x12EAB0 reads it).
    pub time_frames: u32,
    /// Viewport rectangle `[0x7E71C8]`: left, right, top, bottom (`+0, +4, +0x14, +0x18`). INFERRED names.
    pub viewport: [i32; 4],
    /// Weapon name label id: not recovered (no weapon record field is known to hold it). `None`.
    pub weapon_label: Option<u32>,
    /// Crosshair and hit marker: no measured source (0x136F30 is named "aim texture quad when
    /// field 0x94 is zero" only). The frame draws a fixed centre mark and says so. `None`.
    pub crosshair: Option<u32>,
    /// Score: obtained through the mode switch 0x641C0, not a direct field read. `None`.
    pub score: Option<i32>,
}

impl PlayerHudState {
    /// Read the view for local player `slot` from guest memory.
    pub fn read<M: GuestMemory>(m: &M, slot: u32) -> Self {
        let record = m
            .read32(CURRENT_PLAYER_PTR)
            .wrapping_add(slot.wrapping_mul(PLAYER_RECORD_SIZE));
        let object = m.read32(record.wrapping_add(PLAYER_OBJECT));
        let character = m.read32(object.wrapping_add(OBJ_CHARACTER));
        let health = read_f32(m, object.wrapping_add(OBJ_HEALTH));
        let max_health = read_f32(m, object.wrapping_add(OBJ_MAX_HEALTH));
        let armor = read_f32(m, character.wrapping_add(CHAR_ARMOR));
        let armor_max = read_f32(m, character.wrapping_add(CHAR_ARMOR_MAX));
        let mut health_segments = [false; 20];
        let mut armor_segments = [false; 20];
        for index in 0..THRESHOLD_COUNT {
            let health_threshold =
                read_f32(m, HEALTH_THRESHOLDS + index * THRESHOLD_STRIDE) * max_health;
            health_segments[index as usize] = !below(health, health_threshold);
            let armor_threshold =
                read_f32(m, ARMOR_THRESHOLDS + index * THRESHOLD_STRIDE) * armor_max;
            armor_segments[index as usize] =
                m.read32(object.wrapping_add(OBJ_CLASS)) == CLASS_CHARACTER
                    && !below(armor, armor_threshold);
        }
        let hand = character.wrapping_add(EXT_HAND_B);
        let weapon_record = m.read32(hand.wrapping_add(HAND_RECORD_INDEX));
        // Hands carry no record (index -1) and no ammo; the table has 0x42 records.
        let holds_record = weapon_record < WEAPON_RECORD_COUNT;
        let record_address = WEAPON_RECORDS.wrapping_add(weapon_record.wrapping_mul(WEAPON_RECORD_SIZE));
        let (ammo_type, clip_size) = if holds_record {
            (
                m.read32(record_address.wrapping_add(WEAPON_AMMO_TYPE_1)),
                m.read32(record_address.wrapping_add(WEAPON_CLIP_SIZE_1)),
            )
        } else {
            (0, 0)
        };
        let loaded_at = hand
            .wrapping_add(HAND_LOADED_BY_TYPE)
            .wrapping_add(ammo_type.wrapping_mul(2));
        let reserve_at = character
            .wrapping_add(EXT_RESERVE_BY_TYPE)
            .wrapping_add(ammo_type.wrapping_mul(2));
        let viewport = m.read32(VIEWPORT_PTR);
        let block = HUD_BLOCK_BASE.wrapping_add(slot.wrapping_mul(HUD_BLOCK_STRIDE));
        let mut live = Vec::new();
        for index in 0..RING_LEN {
            let entry = messages::entry_address(block, index);
            let kind = m.read32(entry.wrapping_add(MSG_KIND));
            if kind != 0 {
                live.push(HudMessage {
                    kind,
                    text: messages::entry_text(m, entry),
                    age: read_f32(m, entry.wrapping_add(MSG_AGE)),
                    fade_start: read_f32(m, entry.wrapping_add(MSG_FADE_START)),
                    end: read_f32(m, entry.wrapping_add(MSG_END)),
                    entry_address: entry,
                });
            }
        }
        PlayerHudState {
            slot,
            object,
            health,
            max_health,
            armor,
            armor_max,
            health_segments,
            armor_segments,
            weapon_entry: m.read32(character.wrapping_add(EXT_CURRENT_WEAPON_ENTRY)),
            weapon_record,
            ammo_type,
            clip_size,
            ammo_loaded: if holds_record { u32::from(m.read16(loaded_at)) } else { 0 },
            ammo_reserve: if holds_record { u32::from(m.read16(reserve_at)) } else { 0 },
            messages: live,
            letterbox: read_f32(m, LETTERBOX_AMOUNT),
            mode_byte: u32::from(m.read8(MODE_BYTE)),
            time_frames: m.read32(0x007d_e318),
            viewport: [
                m.read32(viewport) as i32,
                m.read32(viewport.wrapping_add(4)) as i32,
                m.read32(viewport.wrapping_add(0x14)) as i32,
                m.read32(viewport.wrapping_add(0x18)) as i32,
            ],
            weapon_label: None,
            crosshair: None,
            score: None,
        }
    }
}
