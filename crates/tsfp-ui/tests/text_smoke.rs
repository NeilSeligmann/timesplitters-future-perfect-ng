// SPDX-License-Identifier: GPL-3.0-or-later
//! T1892 Tier 0 smoke tests of the data/text system on sparse memory, including the two
//! drawing functions of the text entry screen (recorded draw calls).
use tsfp_ui::memory::GuestMemory;
use tsfp_ui::strmem::{GuestStrings, SparseMemory};
use tsfp_ui::text::{self, TextSys};
use tsfp_ui::text_entry::{self, KeyRowRect, TextEntrySys};

#[derive(Default)]
struct Sys {
    memory: SparseMemory,
    draws: Vec<String>,
    heap: u32,
}

impl GuestMemory for Sys {
    fn read8(&self, address: u32) -> u8 {
        self.memory.read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.memory.write8(address, value);
    }
}

impl TextSys for Sys {
    fn alloc(&mut self, size: u32, _tag: u32) -> u32 {
        self.heap += size;
        0x0a00_0000 + self.heap
    }
    fn sprintf_path(&mut self, _: u32, _: u32, _: [u32; 3]) {}
    fn file_size(&mut self, _: u32) -> i32 {
        0
    }
    fn find_loaded_file(&mut self, _: u32) -> u32 {
        0
    }
    fn read_file(&mut self, _: u32, _: u32, _: u32) {}
    fn preload_file(&mut self, _: u32, _: u32) {}
    fn language_gate(&mut self, _: u32) -> u32 {
        0
    }
    fn isctype(&mut self, _: u32, _: u32) -> u32 {
        0
    }
    fn build_id_table(&mut self, _: u32, _: u32, _: u32) {}
    fn widget_changed(&mut self, _: u32) {}
}

impl TextEntrySys for Sys {
    fn fill_rect(&mut self, a: i32, b: i32, c: i32, d: i32, color: u32) {
        self.draws.push(format!("rect {a} {b} {c} {d} {color:x}"));
    }
    fn text_width(&mut self, _: u32) -> f32 {
        10.0
    }
    fn pulse_colors(&mut self, _: u32, _: u32) -> (u32, u32) {
        (1, 2)
    }
    fn draw_text_two_colors(&mut self, x: f32, y: f32, _: u32, first: u32, second: u32) {
        self.draws.push(format!("text2 {x} {y} {first:x} {second:x}"));
    }
    fn modulate_color(&mut self, color: u32, _: f32) -> u32 {
        color
    }
    fn draw_text(&mut self, x: f32, y: f32, _: u32, color: u32) {
        self.draws.push(format!("text {x} {y} {color:x}"));
    }
    fn sprintf_one(&mut self, dst: u32, _: u32, value: u32) {
        self.memory.write_cstring(dst, &value.to_string());
    }
    fn entry_flag_1640(&mut self) -> u32 {
        0
    }
    fn entry_flag_1700(&mut self, _: u32) -> u32 {
        0
    }
    fn page_item(&mut self, _: u32, index: u32) -> u32 {
        0x0b00_0000 + index * 0x40
    }
    fn utf8_to_latin1(&mut self, _: &[u8], _: u32) {}
}

fn put_float(sys: &mut Sys, address: u32, value: f32) {
    sys.write32(address, value.to_bits());
}

#[test]
fn ring_cursor_skips_non_digits_and_clears_change_flags() {
    let mut sys = Sys::default();
    let widget = 0x3000;
    let source = 0x4000;
    sys.memory.write_cstring(source, "a1b2");
    sys.write32(widget + 0x220, source);
    sys.write32(widget + 0x228, 0x20 | 0x12);
    sys.write32(widget + 0x22c, 1);
    text::ring_cursor_step_forward(&mut sys, widget);
    assert_eq!(sys.read32(widget + 0x22c), 3);
    assert_eq!(sys.read32(widget + 0x228), 0x20);
    text::ring_cursor_step_back(&mut sys, widget);
    assert_eq!(sys.read32(widget + 0x22c), 1);

    sys.write32(widget + 0x228, 0);
    text::ring_cursor_step_back(&mut sys, widget);
    assert_eq!(sys.read32(widget + 0x22c), 0);
    text::ring_cursor_step_forward(&mut sys, widget);
    assert_eq!(sys.read32(widget + 0x22c), 1);
}

#[test]
fn text_lookup_prefers_story_bank_and_falls_back() {
    let mut sys = Sys::default();
    assert_eq!(text::text_by_id(&sys, 5), 0, "no bank: missing text pointer (zero here)");
    sys.write32(text::globals::MISSING_TEXT, 0x1234);
    assert_eq!(text::text_by_id(&sys, 5), 0x1234);
    sys.write32(text::globals::STORY_BANK, 0x8000);
    sys.write32(text::globals::COMMON_BANK, 0x9000);
    sys.write32(0x9000 + 5 * 4, 0xaaaa);
    sys.write32(0x8000 + 4, 0xbbbb);
    assert_eq!(text::text_by_id(&sys, 5), 0xaaaa);
    assert_eq!(text::text_by_id(&sys, text::STORY_FIRST_ID + 1), 0xbbbb);
    assert_eq!(text::text_by_id(&sys, 0), text::globals::EMPTY_TEXT);
}

#[test]
fn text_id_tables_and_memory_unit_selectors() {
    let mut sys = Sys::default();
    sys.write32(text::globals::STORY_BANK, 0x8000);
    sys.write32(text::globals::COMMON_BANK, 0x9000);
    for (base, stride, index, id) in [
        (text::globals::TEXT_ID_BY_INDEX, 4, 2, 7),
        (text::globals::RECORD8_TEXT_IDS, 8, 3, 8),
        (text::globals::RECORD58_TEXT_IDS, 0x58, 4, 9),
    ] {
        sys.write32(base + stride * index, id);
        sys.write32(0x9000 + 4 * id, 0xa000 + id);
    }
    assert_eq!(text::text_by_index(&sys, 2), 0xa007);
    assert_eq!(text::text_of_record8(&sys, 3), 0xa008);
    assert_eq!(text::text_of_record58(&sys, 4), 0xa009);

    sys.write32(text::globals::MEMORY_UNIT_SLOT, 1);
    sys.write32(text::globals::MEMORY_UNIT_PAGE, 3);
    sys.write32(text::globals::MEMORY_UNIT_TABLE + 3 * 20, 7);
    sys.write32(text::globals::MEMORY_UNIT_TABLE + 1 * 20 + 8, 8);
    assert_eq!(text::memory_unit_page_label(&sys), 0xa007);
    assert_eq!(text::memory_unit_slot_label(&sys, text::MemoryUnitField::Second), 0xa008);
    sys.write32(text::globals::MEMORY_UNIT_SLOT, 5);
    assert_eq!(text::memory_unit_page_label(&sys), text::globals::EMPTY_TEXT);
}

#[test]
fn extract_line_returns_the_nth_line() {
    let mut sys = Sys::default();
    sys.write_cstring(0x100, "one\ntwo\nthree");
    text::extract_line(&mut sys, 0x100, 2, 0x400);
    assert_eq!(sys.memory.read_cstring(0x400), "two");
    text::extract_line(&mut sys, 0x100, 9, 0x400);
    assert_eq!(sys.memory.read_cstring(0x400), "three", "the last line repeats");
}

#[test]
fn script_tokens_cover_words_numbers_and_quotes() {
    let mut sys = Sys::default();
    let source = "  # note\nname 12.5e+3 \"a b\"";
    sys.write_cstring(0x100, source);
    let state = 0x800;
    sys.write32(state, 0x100);
    sys.write32(state + 4, 0x100 + source.len() as u32);
    let mut tokens = Vec::new();
    for _ in 0..3 {
        text::scan_next_token(&mut sys, state);
        tokens.push(sys.memory.read_cstring(state + text::token::BUFFER));
    }
    assert_eq!(tokens, ["name", "12.5e+3", "a b"]);
}

#[test]
fn cursor_scan_and_parse_int() {
    let mut sys = Sys::default();
    sys.write32(text::globals::CTYPE_TABLE, 0x4000);
    sys.write8(0x4000 + 32 * 2, 8); // ctype table: the space character is white space
    sys.write_cstring(0x100, "  -42 rest");
    sys.write32(0x20, 0x100);
    assert_eq!(text::cursor_parse_int(&mut sys, 0x20), -42);
    assert_eq!(text::cursor_scan_token(&mut sys, 0x20, 0x300), 4);
    assert_eq!(sys.memory.read_cstring(0x300), "rest");
    assert_eq!(text::cursor_scan_token(&mut sys, 0x20, 0x300), -1);
}

#[test]
fn key_row_label_draws_box_then_text() {
    let mut sys = Sys::default();
    put_float(&mut sys, text_entry::globals::UI_SCALE, 2.0);
    put_float(&mut sys, text_entry::constants::PAD, 10.0);
    put_float(&mut sys, text_entry::constants::HALF, 0.5);
    put_float(&mut sys, text_entry::constants::BOX_INSET, 1.0);
    put_float(&mut sys, 0x0f4 + 0x2000, 1.0);
    sys.write32(text_entry::globals::SELECTED_KEY_ROW, 1);
    text_entry::draw_key_row_label(&mut sys, 0x2000, 0x500, 50.0, 7, 1, 100.0, 40.0, 1, 0);
    assert_eq!(sys.draws[0], format!("rect 100 41 120 49 {:x}", 0x283c_6440u32));
    assert_eq!(sys.draws[1], "text2 105 40 1 2", "selected highlight row pulses, label is centred");
    sys.draws.clear();
    text_entry::draw_key_row_label(&mut sys, 0x2000, 0x500, 50.0, 7, 0, 100.0, 40.0, 1, 0);
    assert!(sys.draws[1].starts_with("text 105 40 "));
}

#[test]
fn key_rows_draw_two_rows_and_the_number_box() {
    let mut sys = Sys::default();
    put_float(&mut sys, text_entry::globals::UI_SCALE, 1.0);
    put_float(&mut sys, text_entry::constants::PAD, 4.0);
    put_float(&mut sys, text_entry::constants::HALF, 0.5);
    text_entry::draw_key_rows(&mut sys, 0x2000, 3, KeyRowRect { y: 5.0, x: 60.0, spare: 1.0 });
    let rects = sys.draws.iter().filter(|d| d.starts_with("rect")).count();
    assert_eq!(rects, 3, "two key rows and the number box");
    assert_eq!(sys.memory.read_cstring(text_entry::globals::NUMBER_BUFFER), "0");
}

#[test]
fn done_button_updates_menu_flags_from_entry_capacity() {
    let mut sys = Sys::default();
    let button4_flags = 0x0b00_0000 + 4 * 0x40 + 0x28;
    let done_flags = 0x0b00_0000 + 9 * 0x40 + 0x28;
    sys.write32(button4_flags, 0x80);
    sys.write32(done_flags, 0x80);
    text_entry::update_done_button(&mut sys, 0);
    assert_eq!(sys.read8(text_entry::globals::ENTRY_STATE), 0x10);
    assert_eq!(sys.read32(button4_flags), 0x81);
    assert_eq!(sys.read32(done_flags), 0x81);

    sys.write32(text_entry::globals::LENGTH_A, 19);
    text_entry::update_done_button(&mut sys, 0);
    assert_eq!(sys.read8(text_entry::globals::ENTRY_STATE), 0);
    assert_eq!(sys.read32(button4_flags), 0x81);
    assert_eq!(sys.read32(done_flags), 0x80);
}
