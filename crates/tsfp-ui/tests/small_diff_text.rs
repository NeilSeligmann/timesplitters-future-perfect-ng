// SPDX-License-Identifier: GPL-3.0-or-later
//! T1892 Tier 1: data/text ports replayed against goldens recorded from the original XBE.
//! `python -m tools.small_diff --system text && cargo test --release -p tsfp-ui --test small_diff_text -- --ignored`
#[path = "../../tsfp-engine-memory/src/golden.rs"]
mod golden;

use golden::{GoldenCase, GoldenWorld};
use tsfp_ui::memory::GuestMemory;
use tsfp_ui::text::{self, TextSys};
use tsfp_ui::text_entry::{self, TextEntrySys};

struct World(GoldenWorld);

impl GuestMemory for World {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value);
    }
}

impl TextSys for World {
    fn alloc(&mut self, size: u32, tag: u32) -> u32 {
        self.0.stub_alloc(size, tag)
    }
    fn sprintf_path(&mut self, dst: u32, format: u32, args: [u32; 3]) {
        self.0.stub_sprintf_path(dst, format, args);
    }
    fn file_size(&mut self, path: u32) -> i32 {
        self.0.stub_file_size(path)
    }
    fn find_loaded_file(&mut self, path: u32) -> u32 {
        self.0.stub_find_loaded(path)
    }
    fn read_file(&mut self, path: u32, dst: u32, size: u32) {
        self.0.stub_read_file(path, dst, size);
    }
    fn preload_file(&mut self, path: u32, flags: u32) {
        self.0.stub_preload(path, flags);
    }
    fn language_gate(&mut self, language: u32) -> u32 {
        self.0.stub_language_gate(language)
    }
    fn isctype(&mut self, ch: u32, mask: u32) -> u32 {
        self.0.stub_isctype(ch, mask)
    }
    fn build_id_table(&mut self, table: u32, selector: u32, flag: u32) {
        self.0.stub_build_id_table(table, selector, flag);
    }
    fn widget_changed(&mut self, this: u32) {
        self.0.stub_widget_changed(this);
    }
}

impl TextEntrySys for World {
    fn fill_rect(&mut self, a: i32, b: i32, c: i32, d: i32, color: u32) {
        self.0.stub_fill_rect([a as u32, b as u32, c as u32, d as u32, color]);
    }
    fn text_width(&mut self, text: u32) -> f32 {
        self.0.stub_text_width(text)
    }
    fn pulse_colors(&mut self, style: u32, phase: u32) -> (u32, u32) {
        self.0.stub_pulse_colors(style, phase)
    }
    fn draw_text_two_colors(&mut self, x: f32, y: f32, text: u32, first: u32, second: u32) {
        self.0.stub_draw_two([x.to_bits(), y.to_bits(), text, first, second]);
    }
    fn modulate_color(&mut self, color: u32, alpha: f32) -> u32 {
        self.0.stub_modulate(color, alpha.to_bits())
    }
    fn draw_text(&mut self, x: f32, y: f32, text: u32, color: u32) {
        self.0.stub_draw_one([x.to_bits(), y.to_bits(), text, color]);
    }
    fn sprintf_one(&mut self, dst: u32, format: u32, value: u32) {
        self.0.stub_sprintf_one(dst, format, value);
    }
    fn entry_flag_1640(&mut self) -> u32 {
        self.0.stub_entry_flag_1640()
    }
    fn entry_flag_1700(&mut self, index: u32) -> u32 {
        self.0.stub_entry_flag_1700(index)
    }
    fn page_item(&mut self, page: u32, index: u32) -> u32 {
        self.0.stub_page_item(page, index)
    }
    fn utf8_to_latin1(&mut self, source: &[u8], dst: u32) {
        self.0.stub_utf8_to_latin1(source, dst);
    }
}

const NAMES: &[&str] = &[
    "string_duplicate_aligned", "string_table_copy_entry_to_fixed_slot", "string_table_wrapper",
    "device_query", "text_field_code", "text_by_id", "text_by_index", "text_of_record8",
    "text_of_record58", "text_by_index_0x374", "text_by_index_0x37e", "memory_unit_page_label",
    "memory_unit_label_2", "memory_unit_label_3", "memory_unit_label_4", "string_to_upper_buffer",
    "extract_line", "string_length", "string_advance", "build_text_id_tables",
    "text_id_table_lookup", "scan_next_token", "scan_token_copy", "ring_cursor_step_back",
    "ring_cursor_step_forward", "cursor_scan_token", "cursor_parse_int",
    "replace_tail_at_first_match", "build_language_string", "load_common_bank",
    "load_story_bank", "preload_bank_files", "convert_saved_entries", "update_done_button", "draw_key_row_label", "draw_key_rows",
];

fn run(case: &GoldenCase, w: &mut World) -> Option<u32> {
    use text::MemoryUnitField::{Fourth, Second, Third};
    let a = |i| case.arg(i);
    Some(match case.name.as_str() {
        "string_duplicate_aligned" => text::string_duplicate_aligned(w, a(0)),
        "string_table_copy_entry_to_fixed_slot" | "string_table_wrapper" => {
            text::string_table_copy_entry_to_fixed_slot(w, a(0), a(1));
            return None;
        }
        "device_query" => {
            text::device_query(w, a(0), a(1));
            return None;
        }
        "text_field_code" => text::text_field_code(w, case.reg("ecx")) as u32,
        "text_by_id" => text::text_by_id(w, a(0) as i32),
        "text_by_index" => text::text_by_index(w, a(0)),
        "text_of_record8" => text::text_of_record8(w, a(0)),
        "text_of_record58" => text::text_of_record58(w, a(0)),
        "text_by_index_0x374" => text::text_by_index_0x374(w, a(0) as i32),
        "text_by_index_0x37e" => text::text_by_index_0x37e(w, a(0) as i32),
        "memory_unit_page_label" => text::memory_unit_page_label(w),
        "memory_unit_label_2" => text::memory_unit_slot_label(w, Second),
        "memory_unit_label_3" => text::memory_unit_slot_label(w, Third),
        "memory_unit_label_4" => text::memory_unit_slot_label(w, Fourth),
        "string_to_upper_buffer" => text::string_to_upper_buffer(w, a(0)),
        "extract_line" => {
            text::extract_line(w, a(0), a(1) as i32, a(2));
            return None;
        }
        "string_length" => text::string_length(w, a(0)),
        "string_advance" => text::string_advance(w, a(0), a(1) as i32),
        "build_text_id_tables" => {
            text::build_text_id_tables(w, a(0));
            return None;
        }
        "text_id_table_lookup" => text::text_id_table_lookup(w, a(0) as i32) as u32,
        "scan_next_token" => text::scan_next_token(w, case.reg("eax")),
        "scan_token_copy" => {
            text::scan_token_copy(w, a(0));
            return None;
        }
        "ring_cursor_step_back" => {
            text::ring_cursor_step_back(w, case.reg("esi"));
            return None;
        }
        "ring_cursor_step_forward" => {
            text::ring_cursor_step_forward(w, case.reg("esi"));
            return None;
        }
        "cursor_scan_token" => text::cursor_scan_token(w, a(0), a(1)) as u32,
        "cursor_parse_int" => text::cursor_parse_int(w, a(0)) as u32,
        "replace_tail_at_first_match" => {
            text::replace_tail_at_first_match(w, a(0), a(1), a(2));
            return None;
        }
        "build_language_string" => {
            text::build_language_string(w, a(0), a(1));
            return None;
        }
        "load_common_bank" => {
            text::load_common_bank(w);
            return None;
        }
        "load_story_bank" => {
            text::load_story_bank(w);
            return None;
        }
        "preload_bank_files" => {
            text::preload_bank_files(w);
            return None;
        }
        "convert_saved_entries" => text_entry::convert_saved_entries(w, a(0)),
        "update_done_button" => {
            text_entry::update_done_button(w, a(0));
            return None;
        }
        "draw_key_row_label" => {
            text_entry::draw_key_row_label(
                w, case.reg("edi"), case.reg("esi"), f32::from_bits(case.reg("xmm1")), a(0), a(1),
                f32::from_bits(a(2)), f32::from_bits(a(3)), a(4), a(5),
            );
            return None;
        }
        "draw_key_rows" => {
            let rect = a(2);
            let float = |w: &World, at: u32| f32::from_bits(w.0.read32(at));
            let rect = text_entry::KeyRowRect { y: float(w, rect + 4), x: float(w, rect + 8), spare: float(w, rect + 12) };
            text_entry::draw_key_rows(w, a(0), a(1), rect);
            return None;
        }
        other => panic!("no replay for {other}"),
    })
}

#[test]
#[ignore = "needs goldens: python -m tools.small_diff --system text"]
fn text_matches_original() {
    golden::replay_all("text", NAMES, World, |w| &mut w.0, run);
}
