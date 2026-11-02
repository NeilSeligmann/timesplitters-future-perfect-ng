// SPDX-License-Identifier: GPL-3.0-or-later
//! Text glyph metrics (the hottest unported HUD text function, about 4.0M entries per route).

use crate::memory::GuestMemory;

const CURRENT_FONT: u32 = 0x006b_93b8;
const CURRENT_FONT_OBJECT: u32 = 0x006b_93bc;
const ACCENT_BASE_TABLE: u32 = 0x004c_9040;
const ACCENT_ADJUST_TABLE: u32 = 0x004c_90c0;

fn signed_byte<M: GuestMemory>(m: &M, address: u32) -> i32 {
    m.read8(address) as i8 as i32
}

/// 0x3FAA0 `game_hud_text_glyph_metrics_from_char`. cdecl(1): (text). Width in pixels of the first
/// character, including the kerning against the second (EAX of the original). Bytes with the high
/// bit set are accented: the byte tables 0x4C9040 / 0x4C90C0 give the base glyph and an adjustment
/// record (stride 0x28, float at `-4`). 0xC7 / 0xE7 map to glyphs 0x23 / 0x43, `|` to glyph 0.
/// Font layout (INFERRED): current font `[0x6B93B8]`: `+0xC` adjustment records, `+0x10` glyph
/// table of 12-byte entries (`+0` kern row, `+4` kern column, `+8` advance), `+0x14` kerning
/// matrix (13 columns, 169 entries); font object `[0x6B93BC]+0x10` is the base advance.
// portreg: va=0x0003faa0 tier=1
pub fn glyph_metric<M: GuestMemory>(m: &M, text: u32) -> i32 {
    let font = m.read32(CURRENT_FONT);
    let first = m.read8(text);
    let mut advance = m.read32(m.read32(CURRENT_FONT_OBJECT).wrapping_add(0x10)) as i32;
    let glyph_table = m.read32(font.wrapping_add(0x10));
    let adjust_records = m.read32(font.wrapping_add(0xc));
    let accented = first & 0x80 != 0;
    let mut glyph = i32::from(first);
    if accented {
        glyph = signed_byte(m, ACCENT_BASE_TABLE + u32::from(first & 0x7f));
    }
    glyph = glyph.wrapping_sub(0x20);
    if accented {
        let record = signed_byte(m, ACCENT_ADJUST_TABLE + u32::from(first & 0x7f));
        let address = adjust_records
            .wrapping_add((record.wrapping_mul(5) as u32).wrapping_mul(8))
            .wrapping_sub(4);
        let sum = advance as f32 + f32::from_bits(m.read32(address));
        advance = crate::hud::x86::truncate_to_i32(sum);
    }
    glyph = match first {
        0xc7 => 0x23,
        0xe7 => 0x43,
        0x7c => 0,
        _ if glyph as u32 >= 0x60 => return advance,
        _ => glyph,
    };
    let entry = glyph_table.wrapping_add((glyph as u32).wrapping_mul(12));
    let own = if glyph_table == 0 {
        0
    } else {
        m.read32(entry.wrapping_add(8)) as i32
    };
    advance = advance.wrapping_add(own);
    if glyph_table == 0 {
        return advance;
    }
    let kerning = m.read32(m.read32(CURRENT_FONT).wrapping_add(0x14));
    if kerning == 0 {
        return advance;
    }
    let second = m.read8(text.wrapping_add(1));
    let row = m.read32(entry.wrapping_add(4)) as i32;
    if second == 0 || second == 0x0a {
        return advance;
    }
    let mut next = i32::from(second);
    if second & 0x80 != 0 {
        next = signed_byte(m, ACCENT_BASE_TABLE + u32::from(second & 0x7f));
    }
    next = next.wrapping_sub(0x20);
    if next as u32 >= 0x60 {
        return advance;
    }
    let column_entry = glyph_table.wrapping_add((next as u32).wrapping_mul(12));
    let index = row
        .wrapping_mul(0xd)
        .wrapping_add(m.read32(column_entry) as i32);
    if index < 0 || index >= 0xa9 {
        return advance;
    }
    advance.wrapping_add(m.read32(kerning.wrapping_add((index as u32).wrapping_mul(4))) as i32)
}
