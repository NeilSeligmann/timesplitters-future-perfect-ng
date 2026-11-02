// SPDX-License-Identifier: GPL-3.0-or-later
//! One HUD frame, composed from the ported element functions and a stand-in layout.
//!
//! HONEST SCOPE (docs/t-hud-slice.md): the letterbox bars (0x1396B0) and the message text
//! (0x12D8E0) are the ported originals. The health, armour, ammo and crosshair elements are NOT
//! ports of 0x14F0B0's element drawers (those draw through textured atlas icons and font code not
//! yet ported): they are a FABRICATED stand-in layout driven by [`PlayerHudState`], so the view
//! shows the attachment-point values in the right screen regions, not the original artwork.

use crate::hud::draw::Draw;
use crate::hud::letterbox;
use crate::hud::messages;
use crate::hud::state::PlayerHudState;
use crate::hud::text::{ids, TextProvider};
use crate::memory::GuestMemory;

/// Draw one frame for `state`.
pub fn draw_frame<M: GuestMemory, D: Draw, T: TextProvider>(
    m: &M,
    d: &mut D,
    text: &T,
    state: &PlayerHudState,
) {
    let [left, right, top, bottom] = state.viewport;
    let width = right - left;
    let height = bottom - top;
    // Ported: messages, then the letterbox over them.
    for message in &state.messages {
        messages::draw_entry(m, d, message.entry_address);
    }
    // Stand-in: health segments, bottom left (20 cells of 6 px, lit cells opaque).
    let base_y = bottom - height / 12;
    d.blend_mode(1);
    d.colour_preset(0);
    // The original shows nothing at full health in the measured frames (INFERRED rule: shown while
    // health is below the maximum; the real trigger is the flash timer in the flag block).
    let show_health = state.health < state.max_health;
    for (index, lit) in state.health_segments.iter().enumerate().filter(|_| show_health) {
        let x = left + width / 20 + index as i32 * 7;
        d.fill_rect(x, base_y, x + 5, base_y + 8, if *lit { 0xc0 } else { 0x30 });
    }
    for (index, lit) in state.armor_segments.iter().enumerate().filter(|_| show_health) {
        let x = left + width / 20 + index as i32 * 7;
        d.fill_rect(x, base_y + 11, x + 5, base_y + 15, if *lit { 0xa0 } else { 0x20 });
    }
    // Stand-in: ammo, bottom right. The original shows reserve left and loaded right of a bullet
    // icon (atlas art, not drawn here), at about 0.81 and 0.91 of the width, 0.88 of the height.
    let ammo_y = top + height * 88 / 100;
    if state.weapon_entry != 1 {
        let reserve = state.ammo_reserve.to_string();
        let loaded = state.ammo_loaded.to_string();
        d.text(left + width * 81 / 100, ammo_y, &reserve, 0xffff_ffff);
        d.text(left + width * 91 / 100, ammo_y, &loaded, 0xffff_ffff);
        let weapon = format!("weapon {}", state.weapon_entry);
        d.text(left + width * 81 / 100, ammo_y - 16, &weapon, 0xc0c0_c0ff);
    }
    // Stand-in: score / time labels from the text provider.
    // Only the timed mode (mode byte 3, INFERRED from 0x12EAB0) shows the TIME label.
    if state.mode_byte == 3 {
        if let Some(label) = text.label(ids::TIME) {
            d.text(left + width / 20, top + height / 12, &label, 0xffff_ffff);
        }
    }
    // Stand-in: fixed centre crosshair (the real source is not recovered).
    d.colour_preset(0);
    let (cx, cy) = (left + width / 2, top + height / 2);
    d.fill_rect(cx - 6, cy, cx - 2, cy + 1, 0xff);
    d.fill_rect(cx + 2, cy, cx + 6, cy + 1, 0xff);
    d.fill_rect(cx, cy - 6, cx + 1, cy - 2, 0xff);
    d.fill_rect(cx, cy + 2, cx + 1, cy + 6, 0xff);
    // Ported: the letterbox bars are drawn last (flat black, preset 1 inside `letterbox::draw`).
    letterbox::draw(m, d);
}
