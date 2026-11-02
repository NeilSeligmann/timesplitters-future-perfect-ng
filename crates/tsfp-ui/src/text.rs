// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-port-data-text/c2r/data_text
// portreg: replaces=tsfp-ports/c2r/engine_object_2
// portreg: replaces=tsfp-engine-memory/w4m_arena
//! T1892 data/text system: string tables, text bank lookup and loading, language suffix
//! building, script token scanning, text-entry cursor and memory-unit labels. Hand translated
//! from the original disassembly and checked against the original XBE under Unicorn (Tier 1,
//! `tools/small_diff`, `tests/small_diff_text.rs`). Everything the system needs from its
//! neighbours crosses [`TextSys`].
use crate::memory::GuestMemory;
use crate::strmem::GuestStrings;

/// Guest globals of the text system (original addresses).
pub mod globals {
    /// Story text bank: table of pointers, ids from [`super::STORY_FIRST_ID`] on (`0x6B7B08`).
    pub const STORY_BANK: u32 = 0x006b_7b08;
    /// Size in bytes of the story bank allocation.
    pub const STORY_BANK_SIZE: u32 = 0x006b_7b0c;
    /// Common text bank: table of pointers for ids `1..STORY_FIRST_ID` (`0x6B7B10`).
    pub const COMMON_BANK: u32 = 0x006b_7b10;
    /// Size in bytes of the common bank allocation.
    pub const COMMON_BANK_SIZE: u32 = 0x006b_7b14;
    /// Pointer to the "missing string" text returned when a bank is not loaded.
    pub const MISSING_TEXT: u32 = 0x004c_0a44;
    /// Constant empty string used for id 0.
    pub const EMPTY_TEXT: u32 = 0x0047_66d8;
    /// Current language index into [`LANGUAGE_NAMES`], `-1` when none.
    pub const LANGUAGE_INDEX: u32 = 0x004c_0454;
    /// Five language name pointers, `0x4C0A30 .. 0x4C0A44`.
    pub const LANGUAGE_NAMES: u32 = 0x004c_0a30;
    pub const LANGUAGE_COUNT: u32 = 5;
    /// Game mode byte and its flag byte (bit 6 set keeps the common bank loaded).
    pub const GAME_TYPE: u32 = 0x007d_e455;
    pub const GAME_FLAGS: u32 = 0x007d_e458;
    /// Game language/region code (`0x38` is the one the bank logic treats specially).
    pub const GAME_LANGUAGE: u32 = 0x0079_094c;
    /// Output buffer of the case converting copy.
    pub const CASE_BUFFER: u32 = 0x006b_8408;
    /// Decimal-comma flag of the string length helpers.
    pub const COMMA_MODE: u32 = 0x006b_93a8;
    /// Fixed string slots: 0x32 bytes each from `FIXED_SLOTS`, sources `[68904C + 12 * n]`.
    pub const FIXED_SLOTS: u32 = 0x0068_8f80;
    pub const FIXED_SLOT_SOURCES: u32 = 0x0068_904c;
    /// Device query state: mode word and its two result words.
    pub const DEVICE_MODE: u32 = 0x0068_91a8;
    pub const DEVICE_DEFAULT_RESULT: u32 = 0x0068_91c8;
    pub const DEVICE_SECOND_RESULT: u32 = 0x0068_91b8;
    /// Language prefix and suffix tables of the build-string helper (bits 4..5 of `7497A8`).
    pub const LANGUAGE_BITS: u32 = 0x0074_97a8;
    pub const PREFIX_TABLE: u32 = 0x004e_8e28;
    pub const SUFFIX_TABLE_END: u32 = 0x004e_8e40;
    /// Text id tables (`7E7200` first table, `7E7210` second table), `{ptr, count, base}` each.
    pub const ID_TABLE_A: u32 = 0x007e_7200;
    pub const ID_TABLE_B: u32 = 0x007e_7210;
    pub const MODE_BYTE: u32 = 0x007d_e456;
    /// Selection state of the memory unit label table (`4D9690`, stride 20).
    pub const MEMORY_UNIT_PAGE: u32 = 0x007b_9794;
    pub const MEMORY_UNIT_SLOT: u32 = 0x007b_9798;
    pub const MEMORY_UNIT_TABLE: u32 = 0x004d_9690;
    /// Tables of text ids by index (`4D28B8`), by 8-byte record (`537214`), by 0x58 record.
    pub const TEXT_ID_BY_INDEX: u32 = 0x004d_28b8;
    pub const RECORD8_TEXT_IDS: u32 = 0x0053_7214;
    pub const RECORD58_TEXT_IDS: u32 = 0x0052_cf38;
    /// C runtime ctype table pointer and `MB_CUR_MAX`.
    pub const CTYPE_TABLE: u32 = 0x0054_d7f0;
    pub const MB_CUR_MAX: u32 = 0x0054_d7f8;
    /// C runtime `errno` word.
    pub const ERRNO: u32 = 0x0077_29b8;
}

/// `ERANGE`.
pub const ERANGE: u32 = 0x22;

/// Ids at or above this come from the story bank.
pub const STORY_FIRST_ID: i32 = 0x0d87;
/// Ctype bit for white space in the table at [`globals::CTYPE_TABLE`].
const CTYPE_SPACE: u32 = 0x08;

/// Everything the text system takes from its neighbours. The guest memory is the supertrait.
pub trait TextSys: GuestMemory {
    /// 0003DFA0 heap allocation (`size`, `tag`), returns the guest address.
    fn alloc(&mut self, size: u32, tag: u32) -> u32;
    /// 003C85D4 `sprintf(dst, format, a, b, name)` for the bank path format.
    fn sprintf_path(&mut self, dst: u32, format: u32, args: [u32; 3]);
    /// 00062CA0 size of the named file, negative when absent.
    fn file_size(&mut self, path: u32) -> i32;
    /// 000630C0 already loaded file record for `path` (+0x10 data, +0x14 size), 0 when absent.
    fn find_loaded_file(&mut self, path: u32) -> u32;
    /// 00062D20 reads `size` bytes of the file into `dst`.
    fn read_file(&mut self, path: u32, dst: u32, size: u32);
    /// 000639E0 queues the file for preloading.
    fn preload_file(&mut self, path: u32, flags: u32);
    /// 001CEB00 language specific gate of the bank loader (0 lets the loader skip a bank).
    fn language_gate(&mut self, language: u32) -> u32;
    /// 003C9C4E `_isctype(ch, mask)` used when multibyte characters are possible.
    fn isctype(&mut self, ch: u32, mask: u32) -> u32;
    /// 00046030 builds one text id table (`table`, `selector`, `flag`) from the id range table.
    fn build_id_table(&mut self, table: u32, selector: u32, flag: u32);
    /// 000746D0 marks the widget `this` dirty.
    fn widget_changed(&mut self, this: u32);
}

/// 00027250 game_string_duplicate_aligned: heap copy of `text` in a 16 byte rounded block.
pub fn string_duplicate_aligned<S: TextSys>(s: &mut S, text: u32) -> u32 {
    let length = s.strlen(text);
    let block = s.alloc(length.wrapping_add(0x10) & !0xf, 1);
    s.strcpy(block, text);
    block
}

/// 00031C50 / 00037330 game_string_table_copy_entry_to_fixed_slot: copies string `entry` of the
/// 12 byte source table into the 0x32 byte slot `slot` (zero padded `strncpy`).
pub fn string_table_copy_entry_to_fixed_slot<S: TextSys>(s: &mut S, slot: u32, entry: u32) {
    let source = s.read32(entry.wrapping_mul(12).wrapping_add(globals::FIXED_SLOT_SOURCES));
    let dst = slot.wrapping_mul(0x32).wrapping_add(globals::FIXED_SLOTS);
    s.strncpy(dst, source, 0x32);
}

/// 00032860 game_device_query_c: mode -1 stores the default result through `first`, mode 2
/// stores the second result through `second`, other modes store nothing.
pub fn device_query<S: TextSys>(s: &mut S, first: u32, second: u32) {
    match s.read_i32(globals::DEVICE_MODE) {
        -1 => {
            let value = s.read32(globals::DEVICE_DEFAULT_RESULT);
            s.write32(first, value);
        }
        2 => {
            let value = s.read32(globals::DEVICE_SECOND_RESULT);
            s.write32(second, value);
        }
        _ => {}
    }
}

/// The code a text field is tagged with, from the character found in its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldCode {
    None = 0,
    I = 1,
    R = 2,
    A = 3,
    D = 4,
}

/// 00038200 game_text_field_0x104_first_char_code: `object + 0x104` holds a short code string.
/// Despite the name it searches the whole string, in the order I, R, A, D.
pub fn text_field_code<S: TextSys>(s: &S, object: u32) -> FieldCode {
    let text = object.wrapping_add(0x104);
    if s.read8(text) == 0 {
        return FieldCode::None;
    }
    for (ch, code) in [(b'I', FieldCode::I), (b'R', FieldCode::R), (b'A', FieldCode::A)] {
        if s.strchr(text, ch) != 0 {
            return code;
        }
    }
    if s.strchr(text, b'D') != 0 { FieldCode::D } else { FieldCode::None }
}

/// 0003D7B0 game_text_string_get_by_id_from_bank: guest address of the text for `id`.
/// No story bank loaded gives the missing-text pointer; id 0 the empty text; ids from
/// [`STORY_FIRST_ID`] index the story table, the rest the common table.
pub fn text_by_id<S: TextSys>(s: &S, id: i32) -> u32 {
    let story = s.read32(globals::STORY_BANK);
    if story == 0 {
        return s.read32(globals::MISSING_TEXT);
    }
    if id >= STORY_FIRST_ID {
        let index = id.wrapping_sub(STORY_FIRST_ID) as u32;
        return s.read32(story.wrapping_add(index.wrapping_mul(4)));
    }
    if id == 0 {
        return globals::EMPTY_TEXT;
    }
    let common = s.read32(globals::COMMON_BANK);
    if common == 0 {
        return s.read32(globals::MISSING_TEXT);
    }
    s.read32(common.wrapping_add((id as u32).wrapping_mul(4)))
}

/// 00066470 text of table entry: `text_by_id([4D28B8 + 4 * index])`.
pub fn text_by_index<S: TextSys>(s: &S, index: u32) -> u32 {
    text_from_id_table(s, globals::TEXT_ID_BY_INDEX, 4, index)
}

/// Reads a text id from a guest table whose first word in each record is the id.
/// Guest address arithmetic wraps, as it does in the original 32-bit lookup functions.
fn text_from_id_table<S: TextSys>(s: &S, base: u32, stride: u32, index: u32) -> u32 {
    let address = base.wrapping_add(index.wrapping_mul(stride));
    text_by_id(s, s.read32(address) as i32)
}

/// 002BE6F0 text of the 8 byte record `index` at 537214.
pub fn text_of_record8<S: TextSys>(s: &S, index: u32) -> u32 {
    text_from_id_table(s, globals::RECORD8_TEXT_IDS, 8, index)
}

/// 00311820 text of the 0x58 byte record `index` at 52CF38.
pub fn text_of_record58<S: TextSys>(s: &S, index: u32) -> u32 {
    text_from_id_table(s, globals::RECORD58_TEXT_IDS, 0x58, index)
}

/// 002C7750 text id 0x35D for index 0, else `0x374 + index`.
pub fn text_by_index_0x374<S: TextSys>(s: &S, index: i32) -> u32 {
    text_by_id(s, if index == 0 { 0x35d } else { index.wrapping_add(0x374) })
}

/// 002C7780 text id 0x35D for index 0, else `0x37E + index`.
pub fn text_by_index_0x37e<S: TextSys>(s: &S, index: i32) -> u32 {
    text_by_id(s, if index == 0 { 0x35d } else { index.wrapping_add(0x37e) })
}

const MEMORY_UNIT_ROW_BYTES: u32 = 20;
const MEMORY_UNIT_SLOT_COUNT: i32 = 5;

/// Text of one field of a memory unit label row. The page and slot selectors are
/// deliberately separate: the original validates the slot before indexing by page.
fn memory_unit_label<S: TextSys>(s: &S, row_index: u32, field: MemoryUnitField) -> u32 {
    let address = globals::MEMORY_UNIT_TABLE
        .wrapping_add(row_index.wrapping_mul(MEMORY_UNIT_ROW_BYTES))
        .wrapping_add(field as u32);
    let id = s.read32(address);
    text_by_id(s, id as i32)
}

/// 00082860 label field 0, indexed by the *page* word 7B9794 but only valid while the slot word
/// 7B9798 is in `0..5` (the original checks one and indexes with the other).
pub fn memory_unit_page_label<S: TextSys>(s: &S) -> u32 {
    let slot = s.read_i32(globals::MEMORY_UNIT_SLOT);
    if !(0..MEMORY_UNIT_SLOT_COUNT).contains(&slot) {
        return globals::EMPTY_TEXT;
    }
    memory_unit_label(s, s.read32(globals::MEMORY_UNIT_PAGE), MemoryUnitField::First)
}

/// Which label of a memory unit row (byte offset in the 20 byte row).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryUnitField {
    /// 4D9690
    First = 0,
    /// 4D9698
    Second = 8,
    /// 4D969C
    Third = 12,
    /// 4D96A0
    Fourth = 16,
}

/// 000828C0 / 000828F0 / 00082920 label fields +8, +12, +16 of the row for slot 7B9798.
pub fn memory_unit_slot_label<S: TextSys>(s: &S, field: MemoryUnitField) -> u32 {
    let slot = s.read_i32(globals::MEMORY_UNIT_SLOT);
    if !(0..MEMORY_UNIT_SLOT_COUNT).contains(&slot) {
        return globals::EMPTY_TEXT;
    }
    memory_unit_label(s, slot as u32, field)
}

/// 0003FB90 game_string_copy_with_case_convert_to_buffer: upper cases (a-z and the Latin-1
/// letters E0-FE, including the original's F7 quirk) into the shared buffer and returns it.
/// A null source just empties the buffer.
pub fn string_to_upper_buffer<S: TextSys>(s: &mut S, text: u32) -> u32 {
    let buffer = globals::CASE_BUFFER;
    s.write8(buffer, 0);
    if text == 0 {
        return buffer;
    }
    let length = s.strlen(text);
    s.copy_bytes(buffer, text, length);
    for i in 0..length {
        let ch = s.read8(buffer.wrapping_add(i));
        if (0x61..=0x7a).contains(&ch) || (0xe0..=0xfe).contains(&ch) {
            s.write8(buffer.wrapping_add(i), ch.wrapping_sub(0x20));
        }
    }
    s.write8(buffer.wrapping_add(length), 0);
    buffer
}

/// 0003FC10 game_text_extract_line_until_newline: copies line number `count` (1 based, the last
/// line repeats when the text has fewer) of `text` into `dst`, NUL terminated.
pub fn extract_line<S: TextSys>(s: &mut S, text: u32, count: i32, dst: u32) {
    let mut cursor = 0u32;
    let mut current = text;
    for _ in 0..count.max(0) {
        if current == 0 {
            break;
        }
        if cursor != 0 {
            current = cursor;
        }
        let newline = s.strchr(current, b'\n');
        if newline != 0 {
            let length = newline.wrapping_sub(current);
            s.copy_bytes(dst, current, length);
            s.write8(dst.wrapping_add(length), 0);
            cursor = newline.wrapping_add(1);
        } else {
            cursor = current;
            s.strcpy(dst, current);
        }
    }
}

/// 00040650 game_string_length_with_optional_comma_decimal_scan. With the comma mode on the
/// original calls a decimal digit scanner for every ',' and ignores its result, so both modes
/// are `strlen`.
pub fn string_length<S: TextSys>(s: &S, text: u32) -> u32 {
    s.strlen(text)
}

/// 000406A0 game_string_advance_signed_count_or_unchecked_offset: the address `count` characters
/// into `text`, stopping at the terminator and at non-positive counts when the comma mode is on,
/// plain `text + count` when it is off.
pub fn string_advance<S: TextSys>(s: &S, text: u32, count: i32) -> u32 {
    if s.read32(globals::COMMA_MODE) == 0 {
        return text.wrapping_add(count as u32);
    }
    let (mut at, mut left) = (text, count);
    while s.read8(at) != 0 && left > 0 {
        at = at.wrapping_add(1);
        left -= 1;
    }
    at
}

/// 00046170 game_text_id_tables_build_by_mode: builds both id tables for the current mode
/// (`7DE456 == 0x10` selects 0xFF and flag 0, else 0xFE and flag 1). `arg` is the selector of
/// the first table.
pub fn build_text_id_tables<S: TextSys>(s: &mut S, arg: u32) {
    let sixteen = s.read8(globals::MODE_BYTE) == 0x10;
    let (selector, flag) = if sixteen { (0xff, 0) } else { (0xfe, 1) };
    s.build_id_table(globals::ID_TABLE_A, arg, 1);
    s.build_id_table(globals::ID_TABLE_B, selector, flag);
}

/// 000461C0 game_text_id_tables_lookup_by_id: text index for `id`, -1 when unknown. Table A is
/// `{ptr, count, base}`, table B the fallback with the same layout.
pub fn text_id_table_lookup<S: TextSys>(s: &S, id: i32) -> i32 {
    if id == 0 {
        return -1;
    }
    let a_index = id.wrapping_sub(s.read_i32(globals::ID_TABLE_A + 8));
    if a_index >= 0 && a_index < s.read_i32(globals::ID_TABLE_A + 4) {
        let entry = s.read_i32(s.read32(globals::ID_TABLE_A).wrapping_add((a_index as u32).wrapping_mul(4)));
        if entry != -1 {
            return entry;
        }
    }
    let b_index = id.wrapping_sub(s.read_i32(globals::ID_TABLE_B + 8));
    if b_index >= 0 && b_index < s.read_i32(globals::ID_TABLE_B + 4) {
        return s.read_i32(s.read32(globals::ID_TABLE_B).wrapping_add((b_index as u32).wrapping_mul(4)));
    }
    -1
}

/// Script token scanner state (the "text script" cursor object, 0x10C bytes).
pub mod token {
    /// Current read pointer.
    pub const CURSOR: u32 = 0x00;
    /// End of text pointer.
    pub const END: u32 = 0x04;
    /// Output string of [`super::scan_token_copy`].
    pub const COPY: u32 = 0x08;
    /// Token buffer, `0x80` bytes.
    pub const BUFFER: u32 = 0x88;
    /// Number of bytes in the token buffer, including the terminator.
    pub const LENGTH: u32 = 0x108;
    pub const BUFFER_LIMIT: i32 = 0x7f;
}

fn is_ascii_digit(ch: u8) -> bool {
    ch.is_ascii_digit()
}
fn is_ascii_letter(ch: u8) -> bool {
    ch.is_ascii_alphabetic()
}

/// Reads the byte at the cursor and advances unless already at the end (the original's
/// `get character` idiom; returns the byte even at the end).
fn take<S: TextSys>(s: &mut S, state: u32) -> u8 {
    let cursor = s.read32(state + token::CURSOR);
    let end = s.read32(state + token::END);
    let ch = s.read8(cursor);
    if cursor < end {
        s.write32(state + token::CURSOR, cursor.wrapping_add(1));
    }
    ch
}

fn push_token_char<S: TextSys>(s: &mut S, state: u32, ch: u8) {
    let length = s.read_i32(state + token::LENGTH);
    if length < token::BUFFER_LIMIT {
        s.write8(state.wrapping_add(token::BUFFER).wrapping_add(length as u32), ch);
        s.write32(state + token::LENGTH, (length + 1) as u32);
    }
}

/// 00063C80 game_text_script_scan_next_token: skips white space and `#` and `//` comments, then
/// scans one token (quoted string, number, or identifier) into the buffer at +0x88, NUL
/// terminated. Returns the state pointer (the original returns it in EAX).
pub fn scan_next_token<S: TextSys>(s: &mut S, state: u32) -> u32 {
    let end = s.read32(state + token::END);
    let mut ch = take(s, state);
    s.write32(state + token::LENGTH, 0);
    while s.read32(state + token::CURSOR) != end && matches!(ch, b' ' | b'\t' | b'\n' | b'\r') {
        ch = take(s, state);
    }
    if ch == b'#' || (ch == b'/' && s.read8(s.read32(state + token::CURSOR)) == ch) {
        loop {
            let c = take(s, state);
            if c == b'\n' || s.read32(state + token::CURSOR) == end {
                break;
            }
        }
        ch = take(s, state);
    }
    if s.read32(state + token::CURSOR) == end {
        return finish_token(s, state);
    }
    if ch == b'"' {
        loop {
            // The original never stops at the end of text without a closing quote (endless
            // loop). The port stops there.
            if s.read32(state + token::CURSOR) == s.read32(state + token::END) {
                return finish_token(s, state);
            }
            let c = take(s, state);
            if c == b'"' {
                return finish_token(s, state);
            }
            push_token_char(s, state, c);
        }
    }
    // The first token character is stored unconditionally and the length becomes 1.
    s.write8(state + token::BUFFER, ch);
    s.write32(state + token::LENGTH, 1);
    if is_ascii_digit(ch) || ch == b'-' || ch == b'.' {
        scan_number(s, state);
    } else if is_ascii_letter(ch) {
        scan_word(s, state, end);
    }
    finish_token(s, state)
}

fn scan_word<S: TextSys>(s: &mut S, state: u32, end: u32) {
    while s.read32(state + token::CURSOR) != end {
        let at = s.read32(state + token::CURSOR);
        let ch = s.read8(at);
        if !(is_ascii_digit(ch) || is_ascii_letter(ch)) {
            return;
        }
        if at < s.read32(state + token::END) {
            s.write32(state + token::CURSOR, at.wrapping_add(1));
        }
        push_token_char(s, state, ch);
    }
}

/// Digits, `.`, `e`, and a sign only directly after an `e`.
fn scan_number<S: TextSys>(s: &mut S, state: u32) {
    let mut after_exponent = false;
    while s.read32(state + token::CURSOR) != s.read32(state + token::END) {
        let at = s.read32(state + token::CURSOR);
        let ch = s.read8(at);
        let accept = is_ascii_digit(ch) || ch == b'e' || ch == b'.' || (after_exponent && (ch == b'+' || ch == b'-'));
        if !accept {
            return;
        }
        after_exponent = ch == b'e';
        if at < s.read32(state + token::END) {
            s.write32(state + token::CURSOR, at.wrapping_add(1));
        }
        push_token_char(s, state, ch);
    }
}

fn finish_token<S: TextSys>(s: &mut S, state: u32) -> u32 {
    let length = s.read_i32(state + token::LENGTH);
    if length < token::BUFFER_LIMIT {
        s.write8(state.wrapping_add(token::BUFFER).wrapping_add(length as u32), 0);
        s.write32(state + token::LENGTH, (length + 1) as u32);
    }
    state
}

/// 00063E70 scan a token, then copy the token buffer (+0x88) to the output string at +0x08.
pub fn scan_token_copy<S: TextSys>(s: &mut S, state: u32) {
    let state = scan_next_token(s, state);
    s.strcpy(state.wrapping_add(token::COPY), state.wrapping_add(token::BUFFER));
}

/// Text widget fields shared by the two cursor movement functions.
mod cursor_widget {
    pub const TEXT: u32 = 0x220;
    pub const FLAGS: u32 = 0x228;
    pub const POSITION: u32 = 0x22c;
    pub const DIGIT_RING: u32 = 0x20;
    pub const CHANGE_FLAGS: u32 = 0x12;
    pub const NO_CHANGE_CALLBACK: u32 = 0x100;
}

/// 0007C300 game_text_input_ring_cursor_step_back_wrapping. `this` is the text widget: text
/// pointer at +0x220, flags +0x228 (0x20 = ring mode over digits only), cursor +0x22c.
/// An empty ring text divides by zero in the original; the port leaves the widget untouched.
pub fn ring_cursor_step_back<S: TextSys>(s: &mut S, this: u32) {
    let (text, flags, cursor) = (
        this + cursor_widget::TEXT,
        this + cursor_widget::FLAGS,
        this + cursor_widget::POSITION,
    );
    if s.read32(flags) & cursor_widget::DIGIT_RING != 0 {
        loop {
            let length = string_length(s, s.read32(text)) as i32;
            if length == 0 {
                return;
            }
            let next = (length.wrapping_add(s.read_i32(cursor)).wrapping_sub(1)).wrapping_rem(length);
            s.write32(cursor, next as u32);
            let ch = s.read8(s.read32(text).wrapping_add(next as u32));
            if is_ascii_digit(ch) {
                break;
            }
        }
    } else if s.read_i32(cursor) > 0 {
        let previous = s.read32(cursor) - 1;
        s.write32(cursor, previous);
    }
    finish_cursor_move(s, this);
}

/// 0007C390 game_text_input_ring_cursor_step_forward_wrapping (see the step back variant).
pub fn ring_cursor_step_forward<S: TextSys>(s: &mut S, this: u32) {
    let (text, flags, cursor) = (
        this + cursor_widget::TEXT,
        this + cursor_widget::FLAGS,
        this + cursor_widget::POSITION,
    );
    if s.read32(flags) & cursor_widget::DIGIT_RING != 0 {
        loop {
            let length = string_length(s, s.read32(text)) as i32;
            if length == 0 {
                return;
            }
            let next = s.read_i32(cursor).wrapping_add(1).wrapping_rem(length);
            s.write32(cursor, next as u32);
            let ch = s.read8(s.read32(text).wrapping_add(next as u32));
            if is_ascii_digit(ch) {
                break;
            }
        }
    } else if s.read_i32(cursor) < string_length(s, s.read32(text)) as i32 {
        let next = s.read32(cursor).wrapping_add(1);
        s.write32(cursor, next);
    }
    finish_cursor_move(s, this);
}

fn finish_cursor_move<S: TextSys>(s: &mut S, this: u32) {
    let address = this + cursor_widget::FLAGS;
    let flags = s.read32(address) & !cursor_widget::CHANGE_FLAGS;
    s.write32(address, flags);
    if flags & cursor_widget::NO_CHANGE_CALLBACK == 0 {
        s.widget_changed(this);
    }
}

fn skip_spaces<S: TextSys>(s: &mut S, cursor_ptr: u32) {
    loop {
        let ch = u32::from(s.read8(s.read32(cursor_ptr)));
        let is_space = if s.read_i32(globals::MB_CUR_MAX) > 1 {
            s.isctype(ch, CTYPE_SPACE) != 0
        } else {
            let table = s.read32(globals::CTYPE_TABLE);
            u32::from(s.read8(table.wrapping_add(ch * 2))) & CTYPE_SPACE != 0
        };
        if !is_space {
            return;
        }
        let at = s.read32(cursor_ptr);
        s.write32(cursor_ptr, at.wrapping_add(1));
    }
}

fn is_c_space(ch: u8) -> bool {
    matches!(ch, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// Skips white space and returns the white-space delimited token at `*cursor_ptr`, advancing
/// the cursor past it. `None` when only the terminator is left (`sscanf` returned EOF).
fn next_cursor_token<S: TextSys>(s: &mut S, cursor_ptr: u32) -> Option<Vec<u8>> {
    skip_spaces(s, cursor_ptr);
    let mut at = s.read32(cursor_ptr);
    if s.read8(at) == 0 {
        return None;
    }
    let mut bytes = Vec::new();
    while s.read8(at) != 0 && !is_c_space(s.read8(at)) {
        bytes.push(s.read8(at));
        at = at.wrapping_add(1);
    }
    s.write32(cursor_ptr, at);
    Some(bytes)
}

/// 00150F60 game_text_cursor_skip_space_and_scan_token: skips white space at `*cursor_ptr`, scans
/// one token (`sscanf("%s")`, the format at 475D88) into `out` and advances the cursor past it.
/// Returns the token length or -1 when nothing was scanned.
pub fn cursor_scan_token<S: TextSys>(s: &mut S, cursor_ptr: u32, out: u32) -> i32 {
    let Some(token) = next_cursor_token(s, cursor_ptr) else { return -1 };
    for (i, byte) in token.iter().enumerate() {
        s.write8(out.wrapping_add(i as u32), *byte);
    }
    s.write8(out.wrapping_add(token.len() as u32), 0);
    token.len() as i32
}

/// 00150FE0 game_text_cursor_scan_token_and_parse_int: scans a token and parses it as a decimal
/// `strtol`, returning -1 when there is no token or no digits at its start.
pub fn cursor_parse_int<S: TextSys>(s: &mut S, cursor_ptr: u32) -> i32 {
    match next_cursor_token(s, cursor_ptr) {
        Some(token) => match strtol_base10(&token) {
            (_, 0, _) => -1,
            (value, _, overflow) => {
                if overflow {
                    s.write32(globals::ERRNO, ERANGE);
                }
                value
            }
        },
        None => -1,
    }
}

/// `strtol(text, &end, 10)` returning (value, bytes consumed, overflowed); 0 consumed when no
/// digits. Overflow clamps to the i32 range (the caller sets `errno = ERANGE`).
pub fn strtol_base10(text: &[u8]) -> (i32, usize, bool) {
    let mut at = 0;
    while at < text.len() && is_c_space(text[at]) {
        at += 1;
    }
    let negative = at < text.len() && text[at] == b'-';
    if at < text.len() && (text[at] == b'-' || text[at] == b'+') {
        at += 1;
    }
    let start = at;
    let mut value: i64 = 0;
    while at < text.len() && is_ascii_digit(text[at]) {
        value = (value * 10 + i64::from(text[at] - b'0')).min(i64::from(u32::MAX) + 1);
        at += 1;
    }
    if at == start {
        return (0, 0, false);
    }
    let value = if negative { -value } else { value };
    let clamped = value.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    (clamped as i32, at, clamped != value)
}

/// 00151160 game_string_replace_tail_at_first_substring_match: overwrites `haystack` from the
/// first occurrence of `needle` with `replacement` (a copy of the tail), when it occurs.
pub fn replace_tail_at_first_match<S: TextSys>(s: &mut S, haystack: u32, needle: u32, replacement: u32) {
    let found = s.strstr(haystack, needle);
    if found != 0 {
        s.strcpy(found, replacement);
    }
}

/// 00195810 game_text_build_string_from_arg_with_language_prefix_and_suffix (text, dst): `dst =
/// prefix + text + suffix` where prefix is `PREFIX_TABLE[(lang >> 4) & 3]` and suffix
/// `*(SUFFIX_TABLE_END - 4 * ((lang >> 4) & 3))` for `lang = [7497A8]`.
pub fn build_language_string<S: TextSys>(s: &mut S, text: u32, dst: u32) {
    let language = ((s.read_i32(globals::LANGUAGE_BITS) >> 4) & 3) as u32;
    let prefix = s.read32(globals::PREFIX_TABLE + language * 4);
    let suffix = s.read32(globals::SUFFIX_TABLE_END - language * 4);
    s.strcpy(dst, prefix);
    let end = dst.wrapping_add(s.strlen(dst));
    s.strcpy(end, text);
    let end = dst.wrapping_add(s.strlen(dst));
    s.strcpy(end, suffix);
}

/// Path format strings of the bank loader: `sprintf(buf, FORMAT, ARG_A, ARG_B, language)`.
pub mod bank_paths {
    pub const COMMON_FORMAT: u32 = 0x0047_81b4;
    pub const STORY_FORMAT: u32 = 0x0047_81d4;
    pub const ARG_A: u32 = 0x0047_81cc;
    pub const ARG_B: u32 = 0x0047_81d0;
    /// Stand-in for the original's 0x80 byte stack buffer holding the formatted path.
    pub const SCRATCH: u32 = 0xfff0_1000;
}

fn format_path<S: TextSys>(s: &mut S, format: u32, language_name: u32) -> u32 {
    s.sprintf_path(bank_paths::SCRATCH, format, [bank_paths::ARG_A, bank_paths::ARG_B, language_name]);
    bank_paths::SCRATCH
}

/// Largest bank file over all languages, rounded up to 16.
fn largest_bank_size<S: TextSys>(s: &mut S, format: u32) -> u32 {
    let mut largest = 0i32;
    for language in 0..globals::LANGUAGE_COUNT {
        let name = s.read32(globals::LANGUAGE_NAMES + language * 4);
        let path = format_path(s, format, name);
        largest = largest.max(s.file_size(path));
    }
    (largest as u32).wrapping_add(15) & !0xf
}

/// Loads the bank for the current language into `bank` (copy of a loaded file, else read) and
/// rebases its pointer table (`table_bytes` bytes) onto `bank`.
fn load_and_rebase<S: TextSys>(s: &mut S, format: u32, bank: u32, table_bytes: u32) {
    let index = s.read_i32(globals::LANGUAGE_INDEX);
    if index == -1 {
        return;
    }
    let name = s.read32(globals::LANGUAGE_NAMES.wrapping_add((index as u32).wrapping_mul(4)));
    let path = format_path(s, format, name);
    let loaded = s.find_loaded_file(path);
    if loaded != 0 {
        let (size, data) = (s.read32(loaded + 0x14), s.read32(loaded + 0x10));
        s.copy_bytes(bank, data, size);
    } else {
        let size = s.file_size(path) as u32;
        s.read_file(path, bank, size);
    }
    for offset in (0..table_bytes).step_by(4) {
        let entry = s.read32(bank.wrapping_add(offset)).wrapping_add(bank);
        s.write32(bank.wrapping_add(offset), entry);
    }
}

/// 0003DB80 game_text_bank_path_format: (re)loads the common text bank for the current
/// language. In mode 0xA a loaded bank is dropped when the language gate rejects the language
/// (flag 0x40 clear, language not 0x38), an unloaded one stays unloaded in that case, and
/// otherwise the bank is (re)allocated at the largest language size and loaded.
pub fn load_common_bank<S: TextSys>(s: &mut S) {
    let language = s.read32(globals::GAME_LANGUAGE);
    let mode_a = s.read8(globals::GAME_TYPE) == 0x0a;
    let mut allocate = true;
    if s.read32(globals::COMMON_BANK) != 0 {
        if !mode_a {
            allocate = false;
        } else if language != 0x38 {
            if s.language_gate(language) != 0 || s.read8(globals::GAME_FLAGS) & 0x40 != 0 {
                allocate = false;
            } else {
                s.write32(globals::COMMON_BANK, 0);
                return;
            }
        }
    }
    if allocate {
        let gated_out = mode_a
            && s.language_gate(language) == 0
            && s.read8(globals::GAME_FLAGS) & 0x40 == 0
            && language != 0x38;
        if gated_out {
            return;
        }
        let size = largest_bank_size(s, bank_paths::COMMON_FORMAT);
        s.write32(globals::COMMON_BANK_SIZE, size);
        let bank = s.alloc(size, 1);
        s.write32(globals::COMMON_BANK, bank);
    }
    let bank = s.read32(globals::COMMON_BANK);
    load_and_rebase(s, bank_paths::COMMON_FORMAT, bank, 0x361c);
}

/// 0003DD20 game_text_bank_story_path_format: allocates the story bank once (largest language
/// size, tag 2) and loads/rebases `0x40D4` bytes of pointers for the current language.
pub fn load_story_bank<S: TextSys>(s: &mut S) {
    if s.read32(globals::STORY_BANK) == 0 {
        let size = largest_bank_size(s, bank_paths::STORY_FORMAT);
        s.write32(globals::STORY_BANK_SIZE, size);
        let bank = s.alloc(size, 2);
        s.write32(globals::STORY_BANK, bank);
    }
    let bank = s.read32(globals::STORY_BANK);
    load_and_rebase(s, bank_paths::STORY_FORMAT, bank, 0x40d4);
}

/// 0003DE30 game_text_bank_files_preload_from_path_table: queues every language's story bank
/// file for preload and, unless mode 0xA gates the language out, the common bank files too.
pub fn preload_bank_files<S: TextSys>(s: &mut S) {
    for language in 0..globals::LANGUAGE_COUNT {
        let name = s.read32(globals::LANGUAGE_NAMES + language * 4);
        let path = s.alloc(0x80, 1);
        s.sprintf_path(path, bank_paths::STORY_FORMAT, [bank_paths::ARG_A, bank_paths::ARG_B, name]);
        s.preload_file(path, 0);
    }
    let language = s.read32(globals::GAME_LANGUAGE);
    let skip_common = s.read8(globals::GAME_TYPE) == 0x0a
        && s.language_gate(language) == 0
        && s.read8(globals::GAME_FLAGS) & 0x40 == 0
        && language != 0x38;
    if skip_common {
        return;
    }
    for language in 0..globals::LANGUAGE_COUNT {
        let name = s.read32(globals::LANGUAGE_NAMES + language * 4);
        let path = s.alloc(0x80, 1);
        s.sprintf_path(path, bank_paths::COMMON_FORMAT, [bank_paths::ARG_A, bank_paths::ARG_B, name]);
        s.preload_file(path, 0);
    }
}
