// SPDX-License-Identifier: GPL-3.0-or-later
//! The per-player HUD message ring (pickup notices, "You killed ...", checkpoint text).
//!
//! Each of four players has 16 entries of 0x16C bytes at block `+0x38`, filled round robin
//! through the cursor at block `+0x30` (docs/t-hud.md, T1711). Kind 0 means unused. An entry
//! fades out between `fade_start` and `end` seconds of age (alpha from [`fade_alpha`]).

use crate::hud::addr::*;
use crate::hud::draw::Draw;
use crate::hud::x86::*;
use crate::memory::GuestMemory;

/// Kinds written by the two producers. Kind 0xA is the plain notice, 0xC the variant of 0x12D760.
pub const KIND_NOTICE: u32 = 0x0a;
pub const KIND_NOTICE_C: u32 = 0x0c;
/// Kinds that survive "clear everything" (selector -1): 4, 5 and 0xD.
pub const KEPT_BY_CLEAR_ALL: [u32; 3] = [4, 5, 0x0d];

/// Address of player `player`'s block, or `None` outside 0..4.
fn block_of(player: i32) -> Option<u32> {
    if (0..HUD_BLOCK_PLAYERS).contains(&player) {
        Some(HUD_BLOCK_BASE.wrapping_add((player as u32).wrapping_mul(HUD_BLOCK_STRIDE)))
    } else {
        None
    }
}

/// Address of message entry `index` of a block.
pub fn entry_address(block: u32, index: u32) -> u32 {
    block
        .wrapping_add(BLOCK_RING_ENTRIES)
        .wrapping_add(index.wrapping_mul(RING_ENTRY_SIZE))
}

/// Shared body of 0x12D6B0 and 0x12D760.
fn push<M: GuestMemory>(m: &mut M, kind: u32, player: i32, text: u32, opaque: u32, delay: f32) {
    let Some(block) = block_of(player) else {
        return;
    };
    let cursor = m.read32(block.wrapping_add(BLOCK_RING_CURSOR));
    let entry = block
        .wrapping_add(BLOCK_RING_ENTRIES)
        .wrapping_add(cursor.wrapping_mul(RING_ENTRY_SIZE));
    let next = cursor.wrapping_add(1);
    m.write32(
        block.wrapping_add(BLOCK_RING_CURSOR),
        if (next as i32) < RING_LEN as i32 { next } else { 0 },
    );
    if entry == 0 {
        return;
    }
    bounded_copy(m, entry.wrapping_add(MSG_TEXT), text, MSG_TEXT_BYTES);
    // 0 - delay, not -delay: a zero delay gives +0.0 as the original's subss does
    write_f32(m, entry.wrapping_add(MSG_AGE), 0.0 - delay);
    let fade = m.read32(CONST_FADE);
    let end = m.read32(CONST_MSG_END);
    m.write32(entry.wrapping_add(MSG_KIND), kind);
    m.write32(entry.wrapping_add(MSG_FADE_START), fade);
    m.write32(entry.wrapping_add(MSG_END), end);
    m.write32(entry.wrapping_add(MSG_EVICT_AFTER), fade);
    m.write32(entry.wrapping_add(MSG_WEIGHT), 1);
    m.write32(entry.wrapping_add(MSG_OPAQUE), opaque);
}

/// 0x12D6B0 `game_hud_player_message_queue_push_text_kind_0xa`. cdecl(4): (player, text, opaque, delay).
/// Players outside 0..4 are ignored. Fields the producer does not write (+0x15C, +0x160, +0x164)
/// keep the previous entry's values.
// portreg: va=0x0012d6b0 tier=1
// portreg: mirrors=tsfp-ports/c2r/ui_hud
pub fn push_notice<M: GuestMemory>(m: &mut M, player: i32, text: u32, opaque: u32, delay: f32) {
    push(m, KIND_NOTICE, player, text, opaque, delay);
}

/// 0x12D760 kind 0xC variant of [`push_notice`]. cdecl(4): (player, text, opaque, delay).
// portreg: va=0x0012d760 tier=1
pub fn push_notice_c<M: GuestMemory>(m: &mut M, player: i32, text: u32, opaque: u32, delay: f32) {
    push(m, KIND_NOTICE_C, player, text, opaque, delay);
}

/// 0x12D810 `game_hud_player_messages_clear_by_kind_for_player`. cdecl(2): (player, kind). Clears the kind field of
/// every entry whose kind equals `kind`; `kind == -1` clears all entries except 4, 5 and 0xD.
// portreg: va=0x0012d810 tier=1
pub fn clear_kind<M: GuestMemory>(m: &mut M, player: i32, kind: i32) {
    let Some(block) = block_of(player) else {
        return;
    };
    for index in 0..RING_LEN {
        let address = entry_address(block, index).wrapping_add(MSG_KIND);
        let current = m.read32(address);
        let matches = current as i32 == kind
            || (kind == -1 && !KEPT_BY_CLEAR_ALL.contains(&current));
        if matches {
            m.write32(address, 0);
        }
    }
}

/// Opacity of an entry from its age: 1 until `fade_start`, then linear down to 0 at `end`.
/// Not clamped, as in 0x12D8E0 (the entry is removed at `end` by the per-frame update).
pub fn fade_alpha(age: f32, fade_start: f32, end: f32, one: f32) -> f32 {
    if below_or_equal(age, fade_start) {
        one
    } else {
        one - (age - fade_start) / (end - fade_start)
    }
}

/// Text of an entry (up to the terminator, at most 0x140 bytes). Latin-1 widened to chars.
pub fn entry_text<M: GuestMemory>(m: &M, entry: u32) -> String {
    let mut text = String::new();
    for i in 0..MSG_TEXT_BYTES {
        let byte = m.read8(entry.wrapping_add(MSG_TEXT).wrapping_add(i));
        if byte == 0 {
            break;
        }
        text.push(byte as char);
    }
    text
}

/// 0x12D8E0 `game_hud_draw_player_text_with_fade_ratio_from_fields_0x144_0x148_0x150` (the draw part,
/// font selection 0x12D150 is not ported, Tier 0). cdecl(1): (entry). Draws the
/// entry's text at `viewport.x0 + [0x7A6140]`, `viewport.y1 - letterbox * scale + 0x4760CC` in a
/// colour whose alpha follows [`fade_alpha`] (colour byte layout INFERRED: `alpha * 255 | 0x646464`).
/// Divergence from the original: the x87 `fmul`/`ftol` pair is evaluated in `f32` (the original
/// rounds in 80 bit), so the alpha can differ by one unit in rare cases.
// portreg: va=0x0012d8e0 tier=0
pub fn draw_entry<M: GuestMemory, D: Draw>(m: &M, d: &mut D, entry: u32) {
    let one = read_f32(m, CONST_ONE);
    d.blend_mode(1);
    let alpha = fade_alpha(
        read_f32(m, entry.wrapping_add(MSG_AGE)),
        read_f32(m, entry.wrapping_add(MSG_FADE_START)),
        read_f32(m, entry.wrapping_add(MSG_END)),
        one,
    );
    let viewport = m.read32(VIEWPORT_PTR);
    let x = (m.read32(viewport) as i32).wrapping_add(m.read32(0x007a_6140) as i32);
    let lift = read_f32(m, LETTERBOX_AMOUNT) * read_f32(m, CONST_BAR_SCALE);
    let y = truncate_to_i32(
        int_to_f32(m.read32(viewport.wrapping_add(0x18)) as i32) - lift + read_f32(m, 0x0047_60cc),
    );
    let channel = ftol_low32(f64::from(alpha) * f64::from(read_f32(m, CONST_255)));
    d.text(x, y, &entry_text(m, entry), channel | 0x6464_6400);
}
