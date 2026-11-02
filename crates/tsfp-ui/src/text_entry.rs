// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-port-data-text/c2r/data_text
// portreg: mirrors=tsfp-engine-render/overlay
// portreg: replaces=tsfp-ports/c2r/ui_framework
// portreg: replaces=tsfp-ports/c2r/engine_object
// portreg: replaces=tsfp-ports/c2r/t1937_math
// portreg: mirrors=tsfp-ui/text
//! T1892 data/text, text entry screen part (on-screen keyboard rows, done button state and the
//! saved-name list conversion). Field names are INFERRED from the disassembly.
//! `update_done_button` and `convert_saved_entries` are Tier 1 (golden replay); the two drawing
//! functions are Tier 0 (float/draw calls, covered by a recorded-draw smoke test).
use crate::memory::GuestMemory;
use crate::strmem::GuestStrings;
use crate::text::{self, TextSys};

pub mod globals {
    /// Row (page) index the cursor is on.
    pub const ROW: u32 = 0x0078_9b30;
    /// Selected key row of the keyboard (compared with the `which` of a row label).
    pub const SELECTED_KEY_ROW: u32 = 0x0078_9b48;
    pub const LIMIT: u32 = 0x0078_9b4c;
    pub const LENGTH_B: u32 = 0x0078_9b50;
    pub const LENGTH_A: u32 = 0x0078_9b54;
    pub const ROW_BASE: u32 = 0x0078_9b58;
    pub const ROW_LIMITS: u32 = 0x0078_9b28;
    pub const ENTRY_STATE: u32 = 0x0078_9d40;
    pub const ENTRY_W: u32 = 0x0078_9d38;
    pub const ENTRY_H: u32 = 0x0078_9d3c;
    pub const SIDE: u32 = 0x0074_cf60;
    /// Number text buffer shown in the third key row.
    pub const NUMBER_BUFFER: u32 = 0x0078_a0c8;
    pub const NUMBER_FORMAT: u32 = 0x0047_60fc;
    pub const UI_SCALE: u32 = 0x004c_0460;
    pub const LABELS_A: u32 = 0x0052_45b8;
    pub const LABELS_B: u32 = 0x0052_4618;
    pub const SAVED_ENTRIES: u32 = 0x0075_6f18;
    pub const SAVED_BUFFER_DISABLED: u32 = 0x0075_9eb4;
    pub const SAVED_TARGET: u32 = 0x0075_9eb8;
    pub const SAVED_PENDING: u32 = 0x0075_9ebc;
    pub const SAVED_COUNT: u32 = 0x0075_9d18;
    /// Stand-ins for the original's stack locals of the device query.
    pub const QUERY_FIRST: u32 = 0xfff0_2000;
    pub const QUERY_SECOND: u32 = 0xfff0_2004;
}

/// Float constants read from the constant pool of the original executable.
pub mod constants {
    pub const HALF: u32 = 0x0047_5cd4;
    pub const RESULT_ZERO: u32 = 0x0047_5c78;
    pub const BOX_INSET: u32 = 0x0047_5d10;
    pub const PAD: u32 = 0x0047_5d28;
    pub const ROW_INSET_A: u32 = 0x0047_d80c;
    pub const ROW_INSET_B: u32 = 0x0047_862c;
    pub const ALPHA_SCALE: u32 = 0x0047_85c8;
}

fn float_at<S: GuestMemory>(s: &S, address: u32) -> f32 {
    f32::from_bits(s.read32(address))
}

/// What the text entry screen needs from the UI/render and widget neighbours.
pub trait TextEntrySys: TextSys {
    /// 0001EFB0 filled rectangle `(a, b, c, d, color)`, coordinates as the original pushes them.
    fn fill_rect(&mut self, a: i32, b: i32, c: i32, d: i32, color: u32);
    /// 000405C0 width of the text in UI units.
    fn text_width(&mut self, text: u32) -> f32;
    /// 00075F60 pulsing highlight colours `(first, second)` of the style object for `phase`.
    fn pulse_colors(&mut self, style: u32, phase: u32) -> (u32, u32);
    /// 00041E30 text with two colours: `(x, y, text, first, second)`.
    fn draw_text_two_colors(&mut self, x: f32, y: f32, text: u32, first: u32, second: u32);
    /// 00154FE0 colour `color` scaled by the alpha `alpha`.
    fn modulate_color(&mut self, color: u32, alpha: f32) -> u32;
    /// 00041DC0 plain text `(x, y, text, color)`.
    fn draw_text(&mut self, x: f32, y: f32, text: u32, color: u32);
    /// 003C85D4 `sprintf(dst, format, value)`.
    fn sprintf_one(&mut self, dst: u32, format: u32, value: u32);
    /// 002D1640 entry buffer state flag (feeds bit 5 of the state byte).
    fn entry_flag_1640(&mut self) -> u32;
    /// 002D1700 per-button state test for button `index` (3 and 4 used here).
    fn entry_flag_1700(&mut self, index: u32) -> u32;
    /// 00071940 menu item `index` of `page`, returns the item address (flags at +0x28).
    fn page_item(&mut self, page: u32, index: u32) -> u32;
    /// 00150E60 converts the UTF-8 text `source` to Latin-1 into `dst`.
    fn utf8_to_latin1(&mut self, source: &[u8], dst: u32);
}

/// 002D1370 game_text_entry_draw_key_row_label_with_highlight (Tier 0). `style` is the register
/// argument EDI (alpha at +0xF4), `label` ESI, `coordinate` the SSE register argument XMM1.
/// Stack arguments: `screen`, `which` (row compared with the selected one), `x`, `y`, `highlight`
/// (the selected row may pulse) and `enabled` (draw in full colour).
#[allow(clippy::too_many_arguments)]
pub fn draw_key_row_label<S: TextEntrySys>(
    s: &mut S,
    style: u32,
    label: u32,
    coordinate: f32,
    screen: u32,
    which: u32,
    x: f32,
    y: f32,
    highlight: u32,
    enabled: u32,
) {
    let scale_pad = float_at(s, globals::UI_SCALE) * float_at(s, constants::PAD);
    let inset = float_at(s, constants::BOX_INSET);
    let x0 = (coordinate - inset) as i32;
    let right = (scale_pad + x) as i32;
    let bottom = (y + inset) as i32;
    s.fill_rect(x as i32, bottom, right, x0, 0x283c_6440);
    let width = s.text_width(label);
    let label_x = x + (scale_pad - width) * float_at(s, constants::HALF);
    if s.read32(globals::SELECTED_KEY_ROW) == which && highlight != 0 {
        let (first, second) = s.pulse_colors(style, screen);
        s.draw_text_two_colors(label_x, y, label, first, second);
        return;
    }
    let alpha = float_at(s, style + 0xf4);
    if enabled != 0 {
        let dim = s.modulate_color(0x140a_0a64, alpha);
        let bright = s.modulate_color(0x7f32_3264, alpha);
        s.draw_text_two_colors(label_x, y, label, bright, dim);
    } else {
        let level = (alpha * float_at(s, constants::ALPHA_SCALE)) as i32 as u32 & 0xff;
        s.draw_text(label_x, y, label, level | 0x6464_6400);
    }
}

/// Layout record the key rows are placed in: floats at +4, +8 and +12.
#[derive(Clone, Copy, Debug)]
pub struct KeyRowRect {
    pub y: f32,
    pub x: f32,
    pub spare: f32,
}

/// 002D14C0 game_text_entry_screen_draw_key_rows (Tier 0): the two key rows and the number box.
/// `this` is the screen object (bit 28 of +0x40 picks the highlight style).
pub fn draw_key_rows<S: TextEntrySys>(s: &mut S, this: u32, screen: u32, rect: KeyRowRect) {
    let scale = float_at(s, globals::UI_SCALE);
    let left = rect.x as i32;
    let inset_a = scale * float_at(s, constants::ROW_INSET_A);
    let inset_b = scale * float_at(s, constants::ROW_INSET_B);
    let pad = scale * float_at(s, constants::PAD);
    let half = float_at(s, constants::HALF);
    let slots = [
        (left as f32 - inset_a) as i32,
        (((rect.x - inset_b) + (rect.x - inset_a)) * half - pad * half) as i32,
        (left as f32 - pad - inset_b) as i32,
    ];
    let highlight = (s.read32(this + 0x40) >> 28) & 1;
    let row = s.read32(globals::ROW);
    let value = s.read32(row.wrapping_mul(4).wrapping_add(globals::ROW_BASE));
    s.sprintf_one(globals::NUMBER_BUFFER, globals::NUMBER_FORMAT, value);
    let first = u32::from(s.read32(globals::SIDE) != 0);
    let marker = u32::from(s.read8(globals::ENTRY_STATE)) & 0x20;
    let label = s.read32(first * 4 + globals::ROW_LIMITS);
    let label_a = label.wrapping_mul(3).wrapping_add(globals::LABELS_A);
    draw_key_row_label(s, this, label_a, rect.spare, screen, first, slots[first as usize] as f32, rect.y, highlight, marker);
    let second = u32::from(s.read32(globals::SIDE) == 0);
    let label = s.read32(second * 4 + globals::ROW_LIMITS);
    let label_b = label.wrapping_mul(3).wrapping_add(globals::LABELS_B);
    draw_key_row_label(s, this, label_b, rect.spare, screen, second, slots[second as usize] as f32, rect.y, highlight, 0);
    draw_key_row_label(s, this, globals::NUMBER_BUFFER, rect.spare, screen, 2, slots[2] as f32, rect.y, highlight, 0);
}

/// Three-way comparison used twice by the done button logic: `Less` while `used` is below
/// `cap`, `Greater` above it, and exactly at `cap` decided by the row limits.
fn tier<S: TextEntrySys>(s: &S, used: i32, cap: i32, side: u32) -> std::cmp::Ordering {
    use globals::{LENGTH_B, LIMIT, ROW_LIMITS};
    match used.cmp(&cap) {
        std::cmp::Ordering::Equal => {
            let remaining = s.read_i32(LENGTH_B) - s.read_i32(u32::from(side == 0) * 4 + ROW_LIMITS) - 1;
            match remaining.cmp(&0) {
                std::cmp::Ordering::Equal => {
                    let full = s.read_i32(u32::from(side != 0) * 4 + ROW_LIMITS) + 1;
                    full.cmp(&s.read_i32(LIMIT))
                }
                other => other,
            }
        }
        other => other,
    }
}

/// Fields and masks of the text-entry done button. Meanings are inferred from
/// the observed guest writes; the bit positions and item indices are measured.
mod done_button {
    pub const WIDTH: u32 = 0x12;
    pub const HEIGHT: u32 = 0x0d;
    pub const EMPTY_SHIFT: u32 = 5;
    pub const ROOM_BIT: u8 = 0x10;
    pub const BUTTON3_SHIFT: u32 = 3;
    pub const BUTTON4_SHIFT: u32 = 6;
    pub const BUTTON4_BIT: u8 = 0x40;
    pub const DISABLED_BIT: u32 = 1;
    pub const ITEM_OFFSET: u32 = 0x28;
    pub const BUTTON4_ITEM: u32 = 4;
    pub const DONE_ITEM: u32 = 9;
}

/// 002D1770 game_text_entry_update_done_button_enable: derives the entry state byte (bit 5 empty,
/// bit 4 room left, bit 3 and bit 6 button states) and enables page items 4 and 9 accordingly
/// (item flag bit 0 set means disabled). `page` is the menu page. Tier 1.
pub fn update_done_button<S: TextEntrySys>(s: &mut S, page: u32) {
    use globals::{ENTRY_H, ENTRY_STATE, ENTRY_W, LENGTH_A, ROW, ROW_BASE, SIDE};
    use std::cmp::Ordering;
    s.write32(ENTRY_W, done_button::WIDTH);
    s.write32(ENTRY_H, done_button::HEIGHT);
    s.write8(ENTRY_STATE, 0);
    let mut state = (s.entry_flag_1640() << done_button::EMPTY_SHIFT) as u8;
    s.write8(ENTRY_STATE, state);
    let row = s.read32(ROW);
    let used = s.read_i32(LENGTH_A) - s.read_i32(row.wrapping_mul(4).wrapping_add(ROW_BASE));
    let side = s.read32(SIDE);
    // `tier` is Less while there is room, Greater or Equal once the capacity is reached
    let room = tier(s, used, done_button::WIDTH as i32, side) == Ordering::Less;
    state |= u8::from(room) * done_button::ROOM_BIT;
    s.write8(ENTRY_STATE, state);
    state |= (s.entry_flag_1700(3) << done_button::BUTTON3_SHIFT) as u8;
    s.write8(ENTRY_STATE, state);
    let set_disabled = |s: &mut S, index: u32, disable: bool| {
        let item = s.page_item(page, index).wrapping_add(done_button::ITEM_OFFSET);
        let flags = s.read32(item);
        s.write32(item, if disable { flags | done_button::DISABLED_BIT } else { flags & !done_button::DISABLED_BIT });
    };
    let at_cap = state & done_button::ROOM_BIT != 0
        && tier(s, used, done_button::HEIGHT as i32, side) != Ordering::Less;
    if at_cap {
        set_disabled(s, done_button::BUTTON4_ITEM, false);
        state |= (s.entry_flag_1700(done_button::BUTTON4_ITEM) << done_button::BUTTON4_SHIFT) as u8;
        s.write8(ENTRY_STATE, state);
        if state & done_button::BUTTON4_BIT == 0 {
            state &= !done_button::ROOM_BIT;
            s.write8(ENTRY_STATE, state);
        }
    } else {
        set_disabled(s, done_button::BUTTON4_ITEM, true);
    }
    let state = s.read8(ENTRY_STATE);
    set_disabled(s, done_button::DONE_ITEM, state != 0);
}

/// 001B2B80 game_text_entry_buffers_utf8_to_latin1_convert_with_device_query_c: when a saved
/// entry list is pending and the device is queryable, converts up to 30 entries (stride 0x170)
/// of the saved table into the target table (name, text and details Latin-1 strings, flag
/// words), adds a "..." row when truncated, and a placeholder row (text 0x1C35) when empty.
/// The float `[475C78]` or `[475CD4]` is stored through `out_float`. Returns 0, 1 or 2. Tier 1.
pub fn convert_saved_entries<S: TextEntrySys>(s: &mut S, out_float: u32) -> u32 {
    use globals::{
        QUERY_FIRST, QUERY_SECOND, SAVED_BUFFER_DISABLED, SAVED_COUNT, SAVED_ENTRIES, SAVED_PENDING,
        SAVED_TARGET,
    };
    let finish_zero = |s: &mut S| {
        let value = s.read32(constants::RESULT_ZERO);
        s.write32(out_float, value);
        0
    };
    if s.read8(SAVED_BUFFER_DISABLED) != 0 || s.read32(SAVED_PENDING) == 0 {
        return finish_zero(s);
    }
    s.write32(QUERY_FIRST, 0);
    text::device_query(s, QUERY_FIRST, QUERY_SECOND);
    let mode = s.read_i32(text::globals::DEVICE_MODE);
    let target = s.read32(SAVED_TARGET);
    let placeholder = |s: &mut S| {
        s.write32(target + 0x20, 1);
        s.write32(target + 0x28, 1);
        let text_ptr = text::text_by_id(s, 0x1c35);
        s.strcpy(target + 0x2c, text_ptr);
    };
    if mode == -1 {
        placeholder(s);
        s.write32(target + 0x6c, 1);
        s.write32(target + 0x68, 0);
        s.write32(SAVED_PENDING, 0);
        return 1;
    }
    if mode != 2 {
        let value = s.read32(constants::HALF);
        s.write32(out_float, value);
        return 2;
    }
    let mut count = s.read32(QUERY_SECOND) as i32;
    let mut truncated = false;
    if count >= 0x20 {
        count = 0x1e;
        truncated = true;
    }
    s.write32(target + 0x20, count as u32);
    s.write32(SAVED_COUNT, count as u32);
    if count == 0 {
        placeholder(s);
        s.write8(target + 0x70, 0);
        s.write32(target + 0x6c, 1);
        s.write32(target + 0x68, 0);
        s.write32(SAVED_PENDING, 0);
        return finish_zero(s);
    }
    for index in 0..count as u32 {
        let row = target + index * 0x170;
        let src = SAVED_ENTRIES + index * 0x170;
        s.write32(row + 0x28, 1);
        let name = read_padded(s, src + 0x20, 0x32);
        s.utf8_to_latin1(&name, row + 0x2c);
        let short = read_padded(s, src, 0x20);
        s.utf8_to_latin1(&short, row + 0x70);
        let details = read_padded(s, src + 0x41, 0xff);
        s.utf8_to_latin1(&details, row + 0x90);
        for (from, to) in [(0x168, 0x60), (0x16c, 0x64), (0x140, 0x190), (0x144, 0x194)] {
            let value = s.read32(src + from);
            s.write32(row + to, value);
        }
        let bits = u32::from(s.read8(src + 0x160));
        s.write32(row + 0x6c, ((bits & 1) << 1) | ((bits & 2) << 1) | ((bits & 4) << 1));
        s.write32(row + 0x68, 0);
        let flags = s.read32(src + 0x148);
        let mut map = 0u32;
        for (mask, value) in [
            (0x01, 0x40),
            (0x02, 0x20),
            (0x04, 0x08),
            (0x08, 0x8000),
            (0x10, 0x1_0000),
            (0x20, 0x2_0000),
            (0x40, 0x800),
            (0x200, 0x04),
        ] {
            if flags & mask != 0 {
                map |= value;
            }
        }
        if bits & 4 != 0 {
            map |= 0x10;
        }
        s.write32(row + 0x68, map);
    }
    if truncated {
        s.write32(target + 0x20, count as u32 + 1);
        s.write32(target + count as u32 * 0x170 + 0x28, 2);
        s.write32(target + count as u32 * 0x170 + 0x2c, 0x002e_2e2e);
    }
    s.write32(SAVED_PENDING, 0);
    finish_zero(s)
}

/// `count` bytes from `address` as `strncpy` into a zero padded buffer would hold them.
fn read_padded<S: GuestMemory>(s: &S, address: u32, count: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(count as usize);
    let mut ended = false;
    for i in 0..count {
        let byte = if ended { 0 } else { s.read8(address.wrapping_add(i)) };
        ended |= byte == 0;
        bytes.push(byte);
    }
    bytes
}
