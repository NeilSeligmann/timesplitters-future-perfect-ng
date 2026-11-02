// SPDX-License-Identifier: GPL-3.0-or-later
//! The UI text path (T1909): font exchange, text scale globals, inline number parsing and the
//! integer position string draw. Hand translations of the original disassembly; the Tier 1
//! differential against the retail bytes is `tests/hud_bulk_oracle.rs` (`hand_ports`), fed by the
//! bulk case files of `tools/hud_bulk_oracle.py`.
//!
//! Font state (INFERRED names, MEASURED addresses): `0x6B93A8` text mode flag (0: the descriptor
//! chooses the font, else the fixed font 0x4C8C98), `0x6B93B4` current descriptor index,
//! `0x6B93B8` current font record (0x4C8C28 + index * 0x38), `0x6B93BC` current descriptor record
//! (0x4C8DE8 + index * 20; its first dword is the font index). The scale globals `0x7E7264..0x7E7290`
//! are derived from the descriptor, the font and the screen height.

use crate::hud::addr::SCREEN_HEIGHT_INT;
use crate::hud::x86::{div_ss, int_to_f32, mul_ss, read_f32, write_f32};
use crate::memory::GuestMemory;

pub const TEXT_MODE_FLAG: u32 = 0x006b_93a8;
pub const CURRENT_DESCRIPTOR_INDEX: u32 = 0x006b_93b4;
pub const CURRENT_FONT: u32 = 0x006b_93b8;
pub const CURRENT_DESCRIPTOR: u32 = 0x006b_93bc;
const DESCRIPTOR_TABLE: u32 = 0x004c_8de8;
const DESCRIPTOR_STRIDE: u32 = 20;
const FONT_TABLE: u32 = 0x004c_8c28;
const FONT_STRIDE: u32 = 0x38;
const FIXED_FONT: u32 = 0x004c_8c98;
/// Font record field read by the scale update (`+0x18` of the current font, `+0x18` of a table row).
const FONT_RECORD_SCALE_BASE: u32 = 0x004c_8c40;
const FONT_SCALE_FIELD: u32 = 0x18;

const SCALE_A: u32 = 0x004c_0460;
const GLOBAL_SCALE_X: u32 = 0x007e_7270;
const GLOBAL_SCALE_Y: u32 = 0x007e_7280;
const GLOBAL_SCALE_FONT: u32 = 0x007e_7274;
const SCREEN_RATIO: u32 = 0x0047_8390;
const FONT_NORMALISE: u32 = 0x0047_838c;
const OUT_WIDTH_SCALE: u32 = 0x007e_728c;
const OUT_HEIGHT_SCALE: u32 = 0x007e_7290;
const OUT_BASE: u32 = 0x007e_7264;
const OUT_WIDTH: u32 = 0x007e_7268;
const OUT_HEIGHT: u32 = 0x007e_7288;
const OUT_BASE_COPY: u32 = 0x007e_727c;

/// 0x3F8D0 `game_hud_text_scale_globals_update_from_font_table`. cdecl(0). Recomputes the text
/// scale globals from the current descriptor (`[0x6B93BC]`), the current font and the screen height.
pub fn scale_globals_update<M: GuestMemory>(m: &mut M) {
    let descriptor = m.read32(CURRENT_DESCRIPTOR);
    let mut width = mul_ss(read_f32(m, descriptor.wrapping_add(8)), read_f32(m, SCALE_A));
    width = mul_ss(width, read_f32(m, GLOBAL_SCALE_X));
    let mut height = mul_ss(int_to_f32(m.read32(SCREEN_HEIGHT_INT) as i32), read_f32(m, SCREEN_RATIO));
    let flag = m.read32(TEXT_MODE_FLAG);
    write_f32(m, OUT_WIDTH_SCALE, width);
    height = mul_ss(height, read_f32(m, descriptor.wrapping_add(0xc)));
    height = mul_ss(height, read_f32(m, GLOBAL_SCALE_Y));
    write_f32(m, OUT_HEIGHT_SCALE, height);
    if flag != 0 {
        let row = m.read32(descriptor).wrapping_mul(FONT_STRIDE);
        let row_scale = row.wrapping_add(FONT_RECORD_SCALE_BASE);
        let font = m.read32(CURRENT_FONT);
        let mut base = mul_ss(read_f32(m, row_scale), read_f32(m, GLOBAL_SCALE_FONT));
        let ratio = div_ss(read_f32(m, row_scale), read_f32(m, font.wrapping_add(FONT_SCALE_FIELD)));
        let ratio = mul_ss(ratio, read_f32(m, FONT_NORMALISE));
        base = mul_ss(base, height);
        write_f32(m, OUT_BASE, base);
        write_f32(m, OUT_WIDTH, mul_ss(width, ratio));
        write_f32(m, OUT_HEIGHT, mul_ss(height, ratio));
        write_f32(m, OUT_BASE_COPY, mul_ss(base, ratio));
    } else {
        let font = m.read32(CURRENT_FONT);
        write_f32(m, OUT_WIDTH, width);
        write_f32(m, OUT_HEIGHT, height);
        let base = mul_ss(mul_ss(read_f32(m, font.wrapping_add(FONT_SCALE_FIELD)), read_f32(m, GLOBAL_SCALE_FONT)), height);
        write_f32(m, OUT_BASE, base);
        write_f32(m, OUT_BASE_COPY, base);
    }
}

/// 0x3FA10 `game_text_descriptor_index_exchange_then_update_font_and_metrics`. cdecl(1): (index).
/// Installs descriptor `index`, selects its font and returns the previously installed index.
// portreg: va=0x0003fa10 tier=1
// portreg: mirrors=tsfp-ports/c2r/ui_framework
// portreg: mirrors=tsfp-ui/hud_text
pub fn font_exchange<M: GuestMemory>(m: &mut M, index: u32) -> u32 {
    font_exchange_with(m, index, scale_globals_update)
}

/// [`font_exchange`] with the scale update injected (the differential test replaces it by the
/// recorded call, the original calls 0x3F8D0 as an opaque callee there).
pub fn font_exchange_with<M: GuestMemory>(m: &mut M, index: u32, update: impl FnOnce(&mut M)) -> u32 {
    let flag = m.read32(TEXT_MODE_FLAG);
    let previous = m.read32(CURRENT_DESCRIPTOR_INDEX);
    m.write32(CURRENT_DESCRIPTOR_INDEX, index);
    let descriptor = DESCRIPTOR_TABLE.wrapping_add(index.wrapping_mul(DESCRIPTOR_STRIDE));
    m.write32(CURRENT_DESCRIPTOR, descriptor);
    let font = if flag == 0 {
        FONT_TABLE.wrapping_add(m.read32(descriptor).wrapping_mul(FONT_STRIDE))
    } else {
        FIXED_FONT
    };
    m.write32(CURRENT_FONT, font);
    update(m);
    previous
}

/// 0x3FC90 (unnamed in the name table, called by the layout and render cores). Register
/// argument: `text` in EAX; stack argument `counter` (a pointer, may be 0). Parses the decimal
/// digits that follow the escape byte `text[0]` (INFERRED: the inline code introducer), keeping
/// the last four digits seen, increments `*counter` once per digit, and returns the 4 digit value.
pub fn parse_inline_number<M: GuestMemory>(m: &mut M, text: u32, counter: u32) -> u32 {
    let mut cursor = text.wrapping_add(1);
    let mut digits = [0u32; 4];
    let mut byte = m.read8(cursor);
    while byte != 0 {
        let digit = i32::from(byte as i8) - 0x30;
        if !(0..10).contains(&digit) {
            break;
        }
        digits = [digits[1], digits[2], digits[3], digit as u32];
        cursor = cursor.wrapping_add(1);
        if counter != 0 {
            let seen = m.read32(counter);
            m.write32(counter, seen.wrapping_add(1));
        }
        byte = m.read8(cursor);
    }
    digits.iter().fold(0u32, |sum, d| sum.wrapping_mul(10).wrapping_add(*d))
}

/// The descriptor 0x42490 hands to the render core (stack local in the original; INFERRED layout:
/// `+0` colour, `+0x14` zero).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextDescriptor {
    pub colour: u32,
}

/// The two renderer entries 0x42490 uses.
pub trait TextBackend {
    /// 0x19650: select the render state group, returning the previous one.
    fn state_group(&mut self, group: u32) -> u32;
    /// 0x40FA0 `game_hud_text_render_string_core_with_inline_codes(&mut x, &mut y, 0, text,
    /// 0x40260, &descriptor)`: lays out and draws `text`; advances the float position.
    fn render(&mut self, x: &mut f32, y: &mut f32, text: u32, descriptor: &TextDescriptor);
}

/// Render group 0x0C: the HUD text pass.
pub const TEXT_STATE_GROUP: u32 = 0x0c;

/// 0x42490 `game_hud_text_draw_string_int_pos`. cdecl(4): (x pointer, y pointer, text, colour).
/// Draws `text` at the integer position `*x, *y` in render group 0xC, writes the advanced position
/// back (truncated) and restores the previous group.
// portreg: va=0x00042490 tier=1
pub fn draw_string_int_pos<M: GuestMemory, B: TextBackend>(
    m: &mut M,
    backend: &mut B,
    x_pointer: u32,
    y_pointer: u32,
    text: u32,
    colour: u32,
) {
    let mut x = int_to_f32(m.read32(x_pointer) as i32);
    let mut y = int_to_f32(m.read32(y_pointer) as i32);
    let previous = backend.state_group(TEXT_STATE_GROUP);
    backend.render(&mut x, &mut y, text, &TextDescriptor { colour });
    backend.state_group(previous);
    m.write32(x_pointer, crate::hud::x86::truncate_to_i32(x) as u32);
    m.write32(y_pointer, crate::hud::x86::truncate_to_i32(y) as u32);
}
