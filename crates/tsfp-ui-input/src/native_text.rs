// SPDX-License-Identifier: GPL-3.0-or-later
//! T2310 UI-12: the native text field widget. Typed characters edit the Rust-owned
//! [`tsfp_text_field::TextField`] directly; the page's field record in guest memory (buffer `+0x220`, cursor
//! `+0x22C`, flags `+0x228`) is the write-through view that the original page still draws and commits, the same
//! write-through as the cursor record of [`crate::menu_state`]. No pad presses are emulated.
//!
//! Reference oracle (disassembly of the owner XBE, docs/t2197-text-field.md for the field layout):
//! * insertion 0x7DB20 (string mode): key sound 0x7D960, a selected text is cleared first (flag 0x10), the length
//!   limit `+0x230`, shift-and-insert, the width rule when the scale `+0x238` is positive, on success cursor + 1,
//!   the one-shot shift bit cleared (`& ~2`) and the key sound again; then always the selection bit cleared
//!   (`& ~0x10`) and the widget marked dirty (0x746D0: `+0x44 |= 0x20`);
//! * the commit ("Done") 0x7C4F0(widget): blank names (0x7C4D0) stay open, else the buffer is copied to the source
//!   `+0x224`, the widget closes and the page's done handler `+0x1CC` runs. The native Enter calls 0x7C4F0 itself
//!   (the commit and the handlers stay lifted, the UiBindings boundary);
//! * Back is the framework's native back ([`crate::native_page::back`]).
//!
//! The width rule (scale `+0x238` > 0, MEASURED on the live Enter Name page 0x194980) measures the candidate text
//! with the original font metric exactly as 0x7DB20 does: 0x71940(widget, -2) for the field node, 0x3FA10(node font),
//! then 0x405C0(buffer) whose float result comes back through [`GuestUi::call_st0`]; the text must not be wider than
//! `[0x7BA038] * scale`. A host without `call_st0` cannot measure: characters on such a field are rejected as
//! too wide (nothing is inserted without the original's width check).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tsfp_text_field::keyboard::{FieldFlags, reachable_charset};
use tsfp_text_field::{EditError, Event, TextField, TextWidth};

use crate::menu_state::{PageRef, read_f32};
use crate::native_page::{self, GuestUi};

/// Field fields of the page record (the text input widget is the page itself, built by 0x7D090).
pub mod field {
    pub const DIRTY_FLAGS: u32 = 0x44;
    pub const DIRTY: u32 = 0x20;
    pub const ON_DONE: u32 = 0x1CC;
    pub const BUFFER: u32 = 0x220;
    pub const SOURCE: u32 = 0x224;
    pub const FLAGS: u32 = 0x228;
    pub const CURSOR: u32 = 0x22C;
    pub const LIMIT: u32 = 0x230;
    pub const WIDTH_SCALE: u32 = 0x238;
    /// `FLAGS`: the one-shot shift.
    pub const FLAG_SHIFT: u32 = 0x2;
    /// `FLAGS`: the whole text is selected (the next key replaces it).
    pub const FLAG_SELECTED: u32 = 0x10;
    /// `FLAGS`: numeric ring mode (not a name field).
    pub const FLAG_NUMERIC_RING: u32 = 0x20;
    /// `FLAGS`: no dirty mark after navigation or delete.
    pub const FLAG_NO_DIRTY: u32 = 0x100;
    /// The page id of the zero-padded numeric input class (its handler, built by 0x7D1B0).
    pub const NUMERIC_CLASS_ID: u32 = 0x4D_94D4;
}

/// Original functions the field reaches.
pub mod original {
    /// The key click with a random pitch (no arguments).
    pub const KEY_SOUND: u32 = 0x7_D960;
    /// The commit ("Done") of a text input widget (`widget`).
    pub const COMMIT: u32 = 0x7_C4F0;
    /// The field's node of the widget (`widget`, -2) -> node; the font is node `+0x1C`.
    pub const FIND_NODE: u32 = 0x7_1940;
    /// Select the current font (`font`).
    pub const SELECT_FONT: u32 = 0x3_FA10;
    /// Width of a text in the current font (`text`) -> float in ST0.
    pub const MEASURE_WIDTH: u32 = 0x4_05C0;
    /// The runtime x viewport scale (float global).
    pub const VIEWPORT_X_SCALE: u32 = 0x7B_A038;
}

/// The width rule of 0x7DB20 over the guest: the candidate is written to the field buffer and measured with the
/// original font metric.
struct GuestWidth<'a> {
    g: &'a mut dyn GuestUi,
    page: PageRef,
    buffer: u32,
    scale: f32,
    unmeasured: bool,
}

impl TextWidth for GuestWidth<'_> {
    fn fits(&mut self, bytes: &[u8]) -> bool {
        write_cstring(self.g, self.buffer, bytes);
        let node = self
            .g
            .call(original::FIND_NODE, &[self.page.0, 0xFFFF_FFFE])
            .unwrap_or(0);
        let font = self.g.read32(node.wrapping_add(0x1C)).unwrap_or(0);
        self.g.call(original::SELECT_FONT, &[font]);
        let Some(width) = self.g.call_st0(original::MEASURE_WIDTH, &[self.buffer]) else {
            self.unmeasured = true;
            return false;
        };
        let limit = f64::from(read_f32(self.g, original::VIEWPORT_X_SCALE)) * f64::from(self.scale);
        // The original keeps the character unless the width is strictly greater (unordered keeps it).
        !(width > limit)
    }
}

/// One key for the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextCommand {
    Char(char),
    Backspace,
    Left,
    Right,
    /// Enter: commit through the original Done path.
    Confirm,
    /// Escape: the page's back path.
    Cancel,
}

/// Why a page's field was not taken over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    NoField,
    NumericRing,
    Charset,
    Invalid(EditError),
}

/// What a command did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOutcome {
    Edited,
    Rejected(EditError),
    /// Enter on a blank name: the original keeps the field open.
    Blank,
    Committed,
    Back(native_page::Outcome),
}

fn read_bytes(g: &dyn GuestUi, va: u32, limit: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    while out.len() <= limit {
        let at = va + out.len() as u32;
        let word = g.read32(at & !3)?;
        let byte = (word >> ((at & 3) * 8)) as u8;
        if byte == 0 {
            return Some(out);
        }
        out.push(byte);
    }
    None
}

fn read8(g: &dyn GuestUi, va: u32) -> Option<u8> {
    let word = g.read32(va & !3)?;
    Some((word >> ((va & 3) * 8)) as u8)
}

/// Writes `bytes` and a terminating NUL at `va` through aligned word read-modify-writes.
fn write_cstring(g: &mut dyn GuestUi, va: u32, bytes: &[u8]) -> bool {
    let mut data = bytes.to_vec();
    data.push(0);
    for (i, byte) in data.into_iter().enumerate() {
        let at = va + i as u32;
        let aligned = at & !3;
        let shift = (at & 3) * 8;
        let Some(word) = g.read32(aligned) else {
            return false;
        };
        let word = (word & !(0xFF << shift)) | (u32::from(byte) << shift);
        if !g.write32(aligned, word) {
            return false;
        }
    }
    true
}

/// The Rust-owned field of one text input page.
#[derive(Clone, Debug)]
pub struct NativeTextField {
    pub page: PageRef,
    buffer: u32,
    scale: f32,
    pub field: TextField,
}

impl NativeTextField {
    /// Takes over the field of `page`: the current text, cursor, limit, selection and the charset of every grid the
    /// field's flags reach (the runtime tables, read through the guest).
    pub fn open(g: &dyn GuestUi, page: PageRef) -> Result<Self, OpenError> {
        let get = |offset: u32| g.read32(page.0 + offset).unwrap_or(0);
        let buffer = get(field::BUFFER);
        if buffer == 0 {
            return Err(OpenError::NoField);
        }
        let flags = get(field::FLAGS);
        if flags & field::FLAG_NUMERIC_RING != 0 {
            return Err(OpenError::NumericRing);
        }
        let scale = read_f32(g, page.0 + field::WIDTH_SCALE);
        let limit = (get(field::LIMIT) as usize).min(255);
        let charset =
            reachable_charset(FieldFlags(flags), |a| read8(g, a)).ok_or(OpenError::Charset)?;
        let text = read_bytes(g, buffer, 255).ok_or(OpenError::NoField)?;
        let mut model = TextField::new(&text, limit, &charset).map_err(OpenError::Invalid)?;
        let cursor = (get(field::CURSOR) as usize).min(text.len());
        for _ in cursor..text.len() {
            let _ = model.apply(Event::Left);
        }
        if flags & field::FLAG_SELECTED != 0 {
            model.select_all();
        }
        Ok(Self {
            page,
            buffer,
            scale,
            field: model,
        })
    }

    /// True when the guest record still shows exactly this model (no other input edited it meanwhile).
    pub fn in_sync(&self, g: &dyn GuestUi) -> bool {
        g.read32(self.page.0 + field::BUFFER) == Some(self.buffer)
            && read_bytes(g, self.buffer, 255).as_deref() == Some(self.field.text())
            && g.read32(self.page.0 + field::CURSOR) == Some(self.field.cursor() as u32)
    }

    fn update_flags(g: &mut dyn GuestUi, page: PageRef, clear: u32) -> u32 {
        let flags = g.read32(page.0 + field::FLAGS).unwrap_or(0) & !clear;
        g.write32(page.0 + field::FLAGS, flags);
        flags
    }

    fn mark_dirty(g: &mut dyn GuestUi, page: PageRef) {
        let dirty = g.read32(page.0 + field::DIRTY_FLAGS).unwrap_or(0) | field::DIRTY;
        g.write32(page.0 + field::DIRTY_FLAGS, dirty);
    }

    fn write_back(&self, g: &mut dyn GuestUi) {
        write_cstring(g, self.buffer, self.field.text());
        g.write32(self.page.0 + field::CURSOR, self.field.cursor() as u32);
    }

    /// The zero-padded numeric class (page id [`field::NUMERIC_CLASS_ID`]) drops the text before the cursor when it
    /// starts with '0' and nothing is selected, before a digit is placed (0x7DB55..0x7DBF9: 0x7C410 until the
    /// cursor is 0; the original tests only the first byte, kept).
    fn strip_leading_zeros(&mut self, g: &mut dyn GuestUi) {
        let flags = g.read32(self.page.0 + field::FLAGS).unwrap_or(0);
        if flags & field::FLAG_SELECTED != 0
            || g.read32(self.page.0 + crate::menu_state::layout::page::ID)
                != Some(field::NUMERIC_CLASS_ID)
            || self.field.text().first() != Some(&b'0')
        {
            return;
        }
        while self.field.cursor() > 0 {
            if self.field.apply(Event::Backspace).is_err() {
                break;
            }
            Self::update_flags(g, self.page, field::FLAG_SHIFT);
        }
        self.write_back(g);
        if flags & field::FLAG_NO_DIRTY == 0 {
            Self::mark_dirty(g, self.page);
        }
    }

    /// Applies one command (guest thread, frame boundary).
    pub fn apply(&mut self, g: &mut dyn GuestUi, command: TextCommand) -> TextOutcome {
        let page = self.page;
        match command {
            TextCommand::Char(c) => {
                let flags = FieldFlags(g.read32(page.0 + field::FLAGS).unwrap_or(0));
                if !flags.character_enabled(c) || !self.field.accepts(c) {
                    return TextOutcome::Rejected(EditError::InvalidCharacter);
                }
                g.call(original::KEY_SOUND, &[]);
                self.strip_leading_zeros(g);
                let result = if self.scale > 0.0 {
                    let mut width = GuestWidth {
                        g: &mut *g,
                        page,
                        buffer: self.buffer,
                        scale: self.scale,
                        unmeasured: false,
                    };
                    let result = self.field.character_with_width(c, &mut width);
                    if width.unmeasured {
                        self.write_back(g);
                        return TextOutcome::Rejected(EditError::TooWide);
                    }
                    result
                } else {
                    self.field.apply(Event::Character(c))
                };
                self.write_back(g);
                // A width rollback returns before the selection clear and the dirty mark (0x7DCBE).
                if result == Err(EditError::TooWide) {
                    return TextOutcome::Rejected(EditError::TooWide);
                }
                if result.is_ok() {
                    Self::update_flags(g, page, field::FLAG_SHIFT);
                    g.call(original::KEY_SOUND, &[]);
                }
                Self::update_flags(g, page, field::FLAG_SELECTED);
                Self::mark_dirty(g, page);
                match result {
                    Ok(()) => TextOutcome::Edited,
                    Err(e) => TextOutcome::Rejected(e),
                }
            }
            TextCommand::Backspace | TextCommand::Left | TextCommand::Right => {
                // The special keys go through 0x7DA20, which plays the key sound first.
                g.call(original::KEY_SOUND, &[]);
                let event = match command {
                    TextCommand::Backspace => Event::Backspace,
                    TextCommand::Left => Event::Left,
                    _ => Event::Right,
                };
                let result = self.field.apply(event);
                self.write_back(g);
                let flags = Self::update_flags(g, page, field::FLAG_SHIFT | field::FLAG_SELECTED);
                if flags & field::FLAG_NO_DIRTY == 0 {
                    Self::mark_dirty(g, page);
                }
                match result {
                    Ok(()) => TextOutcome::Edited,
                    Err(e) => TextOutcome::Rejected(e),
                }
            }
            TextCommand::Confirm => {
                g.call(original::KEY_SOUND, &[]);
                if !self.field.can_confirm() {
                    return TextOutcome::Blank;
                }
                self.write_back(g);
                if !mapmaker_string_commit(g, page) {
                    g.call(original::COMMIT, &[page.0]);
                }
                TextOutcome::Committed
            }
            TextCommand::Cancel => TextOutcome::Back(native_page::back(g, page)),
        }
    }
}

/// Typed string commit for the three Map Maker-owned text buffers. Other text
/// fields retain their independently owned callback frontier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapTextCommit {
    pub text: Vec<u8>,
    pub previous: Vec<u8>,
    pub keep_open: bool,
}
impl MapTextCommit {
    pub fn changed(&self) -> bool {
        self.text != self.previous
    }
    pub fn blank(&self) -> bool {
        self.text.iter().all(|b| *b == b' ')
    }
}
fn mapmaker_string_commit(g: &mut dyn GuestUi, page: PageRef) -> bool {
    if !crate::mapmaker_handlers::enabled_for(original::COMMIT) {
        return false;
    }
    mapmaker_string_commit_selected(g, page)
}
fn mapmaker_string_commit_selected(g: &mut dyn GuestUi, page: PageRef) -> bool {
    let source = g.read32(page.0 + 0x224).unwrap_or(0);
    if !matches!(source, 0x783CE0 | 0x783E00 | 0x765A88)
        || g.read32(page.0 + 4) == Some(field::NUMERIC_CLASS_ID)
    {
        return false;
    }
    let buffer = g.read32(page.0 + field::BUFFER).unwrap_or(0);
    let limit = if source == 0x765A88 { 31 } else { 223 };
    let Some(text) = read_bytes(g, buffer, limit) else {
        return false;
    };
    let Some(previous) = read_bytes(g, source, limit) else {
        return false;
    };
    let state = MapTextCommit {
        text,
        previous,
        keep_open: g.read32(page.0 + field::FLAGS).unwrap_or(0) & 0x100 != 0,
    };
    if state.blank() {
        return true;
    }
    let context = g.read32(page.0 + 0x2C).unwrap_or(0);
    let parent_row = g.read32(context + 0x198).unwrap_or(0);
    if state.changed() {
        let reset = g.read32(page.0 + 0x234).unwrap_or(0);
        if reset != 0 {
            g.write32(reset, 0);
        }
    }
    write_cstring(g, source, &state.text);
    let handler = g.read32(page.0 + 0x1CC).unwrap_or(0);
    if state.keep_open {
        if handler != 0 && !crate::mapmaker_handlers::dispatch(g, page, handler) {
            g.call(handler, &[page.0]);
        }
    } else {
        if parent_row != 0 {
            let flags = g.read32(parent_row + 0x2C).unwrap_or(0);
            g.write32(
                parent_row + 0x2C,
                if state.changed() {
                    flags | 8
                } else {
                    flags & !8
                },
            );
        }
        g.call(0x78D80, &[page.0]);
        g.call(0x746D0, &[context]);
        if handler != 0 && !crate::mapmaker_handlers::dispatch(g, PageRef(context), handler) {
            g.call(handler, &[context]);
        }
    }
    if std::env::var_os("TSFP_UI_NATIVE_LOG").is_some() {
        eprintln!("[ui-native] mapmaker text-commit=0x7C4F0 source=0x{source:X}");
    }
    true
}

/// The numeric ring field (flag [`field::FLAG_NUMERIC_RING`], e.g. a time "MM:SS"): a fixed-length text whose
/// cursor moves only over digits, wrapping around, and a key overwrites the digit under the cursor. The text stays
/// in the guest record (no Rust model: the format around the digits belongs to the page that built it).
///
/// Ports of the ring branches of 0x7C300 (back), 0x7C390 (forward), 0x7C410 (delete) and 0x7DB20 (key). The length
/// helper 0x40650 is a raw byte length in both of its modes (docs/t1447-traversal-helpers.md), computed here.
pub mod ring {
    use super::{GuestUi, PageRef, field, read8};

    /// Ring walks give up after this many positions (the original loops forever on a text without digits).
    const WALK_LIMIT: u32 = 512;

    fn length(g: &dyn GuestUi, buffer: u32) -> i32 {
        let mut n = 0u32;
        while n < 0x1_0000 && read8(g, buffer.wrapping_add(n)).unwrap_or(0) != 0 {
            n += 1;
        }
        n as i32
    }

    fn write8(g: &mut dyn GuestUi, va: u32, byte: u8) {
        let aligned = va & !3;
        let shift = (va & 3) * 8;
        let word = g.read32(aligned).unwrap_or(0);
        g.write32(
            aligned,
            (word & !(0xFF << shift)) | (u32::from(byte) << shift),
        );
    }

    fn is_digit(byte: u8) -> bool {
        (b'0'..=b'9').contains(&byte)
    }

    /// The navigation tail of 0x7C300 / 0x7C390: shift and selection cleared, dirty unless suppressed.
    fn finish_step(g: &mut dyn GuestUi, page: PageRef) {
        let flags = g.read32(page.0 + field::FLAGS).unwrap_or(0)
            & !(field::FLAG_SHIFT | field::FLAG_SELECTED);
        g.write32(page.0 + field::FLAGS, flags);
        if flags & field::FLAG_NO_DIRTY == 0 {
            mark_dirty(g, page);
        }
    }

    fn mark_dirty(g: &mut dyn GuestUi, page: PageRef) {
        let dirty = g.read32(page.0 + field::DIRTY_FLAGS).unwrap_or(0) | field::DIRTY;
        g.write32(page.0 + field::DIRTY_FLAGS, dirty);
    }

    /// Cursor one place back (0x7C300) or forward (0x7C390). A plain field clamps to `[0, length]`; a ring field
    /// wraps modulo the length and keeps going until the cursor is on a digit.
    pub fn step(g: &mut dyn GuestUi, page: PageRef, forward: bool) {
        let get = |g: &dyn GuestUi, offset: u32| g.read32(page.0 + offset).unwrap_or(0);
        if get(g, field::FLAGS) & field::FLAG_NUMERIC_RING == 0 {
            let cursor = get(g, field::CURSOR) as i32;
            if forward {
                if cursor < length(g, get(g, field::BUFFER)) {
                    g.write32(page.0 + field::CURSOR, (cursor + 1) as u32);
                }
            } else if cursor > 0 {
                g.write32(page.0 + field::CURSOR, (cursor - 1) as u32);
            }
        } else {
            for _ in 0..WALK_LIMIT {
                let buffer = get(g, field::BUFFER);
                let len = length(g, buffer);
                if len == 0 {
                    break;
                }
                let cursor = get(g, field::CURSOR) as i32;
                let next = if forward {
                    cursor.wrapping_add(1)
                } else {
                    len.wrapping_add(cursor).wrapping_sub(1)
                };
                let at = next.wrapping_rem(len);
                g.write32(page.0 + field::CURSOR, at as u32);
                if is_digit(read8(g, buffer.wrapping_add(at as u32)).unwrap_or(0)) {
                    break;
                }
            }
        }
        finish_step(g, page);
    }

    /// Delete on a ring field (0x7C410 ring branch): the digit under the cursor becomes '0', then one ring step
    /// back; dirty unless suppressed.
    pub fn delete(g: &mut dyn GuestUi, page: PageRef) {
        let buffer = g.read32(page.0 + field::BUFFER).unwrap_or(0);
        let cursor = g.read32(page.0 + field::CURSOR).unwrap_or(0);
        write8(g, buffer.wrapping_add(cursor), b'0');
        step(g, page, false);
        if g.read32(page.0 + field::FLAGS).unwrap_or(0) & field::FLAG_NO_DIRTY == 0 {
            mark_dirty(g, page);
        }
    }

    /// A key on a ring field (0x7DB20 ring branch, after its key sound): the digit under the cursor is replaced,
    /// capped at '5' on the second-to-last position (the tens of seconds), then one ring step forward; selection
    /// cleared and the widget dirtied.
    pub fn key(g: &mut dyn GuestUi, page: PageRef, key: u8) {
        let buffer = g.read32(page.0 + field::BUFFER).unwrap_or(0);
        let cursor = g.read32(page.0 + field::CURSOR).unwrap_or(0);
        let mut key = key;
        if cursor as i32 == length(g, buffer) - 2 && key as i8 >= b'5' as i8 {
            key = b'5';
        }
        write8(g, buffer.wrapping_add(cursor), key);
        step(g, page, true);
        let flags = g.read32(page.0 + field::FLAGS).unwrap_or(0) & !field::FLAG_SELECTED;
        g.write32(page.0 + field::FLAGS, flags);
        mark_dirty(g, page);
    }
}

/// The queue between the host keyboard and the guest frame hook.
pub type TextQueue = Arc<Mutex<VecDeque<TextCommand>>>;

/// One key on a numeric ring field ([`ring`]): digits overwrite, Backspace zeroes and steps back, Left/Right walk
/// the digits, Enter commits through the original Done path, Escape is the native back. Every key but Escape plays
/// the key sound first, as 0x7DB20 and 0x7DA20 do.
pub fn apply_ring(g: &mut dyn GuestUi, page: PageRef, command: TextCommand) -> TextOutcome {
    if let TextCommand::Char(c) = command
        && !c.is_ascii_digit()
    {
        return TextOutcome::Rejected(EditError::InvalidCharacter);
    }
    if command != TextCommand::Cancel {
        g.call(original::KEY_SOUND, &[]);
    }
    match command {
        TextCommand::Char(c) => ring::key(g, page, c as u8),
        TextCommand::Backspace => ring::delete(g, page),
        TextCommand::Left => ring::step(g, page, false),
        TextCommand::Right => ring::step(g, page, true),
        TextCommand::Confirm => {
            g.call(original::COMMIT, &[page.0]);
            return TextOutcome::Committed;
        }
        TextCommand::Cancel => return TextOutcome::Back(native_page::back(g, page)),
    }
    TextOutcome::Edited
}

/// Page ids of the original text input widgets (the handler the creator passes as the child id to 0x78E50): the
/// string field of 0x7D090 and the numeric field of 0x7D1B0. Any page with one of these ids and a buffer is a text
/// field this module can drive (Enter Name, Map Maker names, numeric and time entries).
pub const TEXT_WIDGET_IDS: [u32; 2] = [0x4D_8D78, field::NUMERIC_CLASS_ID];

/// The text field page among the live page slots: built by one of `builders`, or a text input widget
/// ([`TEXT_WIDGET_IDS`]); its buffer must exist. Highest slot first. The slot index is not the stack order: the
/// Map Maker Briefing/description/name entry (0x7E800) opens in a LOWER slot than the Map Settings page that
/// launched it (slot 1 below slot 2), so a text page must be found past live non-text pages (T3497).
pub fn top_text_page(g: &dyn GuestUi, builders: &[u32]) -> Option<PageRef> {
    (0..native_page::PAGE_SLOTS)
        .rev()
        .map(PageRef::slot)
        .filter(|p| p.live(g))
        .enumerate()
        .find(|(rank, page)| {
            let id = g
                .read32(page.0 + crate::menu_state::layout::page::ID)
                .unwrap_or(0);
            // Builder-listed pages (retained buffers) count only as the top live page, widgets anywhere.
            let known = (*rank == 0 && builders.contains(&page.builder(g)))
                || TEXT_WIDGET_IDS.contains(&id);
            known && g.read32(page.0 + field::BUFFER).unwrap_or(0) != 0
        })
        .map(|(_, page)| page)
}

/// Frame-boundary service: drains `queue` into the field of the top live page built by `builder`, opening (or
/// re-opening after another input changed the guest record) the Rust model first. Commands wait while the page is
/// not ready; Confirm and Cancel end the frame (the page may close).
pub fn service(
    g: &mut dyn GuestUi,
    builder: u32,
    queue: &TextQueue,
    state: &mut Option<NativeTextField>,
    out: &mut Vec<(TextCommand, TextOutcome)>,
) -> Option<OpenError> {
    let page = native_page::find_page(g, builder).map(|(_, page)| page);
    service_on(g, page, queue, state, out)
}

/// [`service`] on a given text page (`None`: no text page is live, the queue and the model are dropped).
pub fn service_on(
    g: &mut dyn GuestUi,
    page: Option<PageRef>,
    queue: &TextQueue,
    state: &mut Option<NativeTextField>,
    out: &mut Vec<(TextCommand, TextOutcome)>,
) -> Option<OpenError> {
    let Ok(mut pending) = queue.lock() else {
        return None;
    };
    let Some(page) = page else {
        *state = None;
        pending.clear();
        return None;
    };
    if pending.is_empty() {
        return None;
    }
    if !matches!(page.state(g), 2 | 3)
        || g.call(native_page::original::PAGE_READY, &[page.0])
            .unwrap_or(0)
            == 0
    {
        return None;
    }
    if g.read32(page.0 + field::FLAGS).unwrap_or(0) & field::FLAG_NUMERIC_RING != 0 {
        *state = None;
        while let Some(command) = pending.pop_front() {
            let outcome = apply_ring(g, page, command);
            out.push((command, outcome));
            if matches!(outcome, TextOutcome::Committed | TextOutcome::Back(_)) {
                pending.clear();
                break;
            }
        }
        return None;
    }
    if state
        .as_ref()
        .is_none_or(|s| s.page != page || !s.in_sync(g))
    {
        match NativeTextField::open(g, page) {
            Ok(field) => *state = Some(field),
            Err(error) => {
                pending.clear();
                *state = None;
                return Some(error);
            }
        }
    }
    let Some(field) = state.as_mut() else {
        return None;
    };
    while let Some(command) = pending.pop_front() {
        let outcome = field.apply(g, command);
        out.push((command, outcome));
        if matches!(outcome, TextOutcome::Committed | TextOutcome::Back(_)) {
            pending.clear();
            *state = None;
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_state::layout::page;
    use std::collections::HashMap;
    use tsfp_text_field::keyboard::{FieldFlags, select_table};

    #[derive(Default)]
    struct Fake {
        mem: HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
        /// Width the font metric returns per character (None: the host has no float call).
        char_width: Option<f64>,
    }
    impl GuestUi for Fake {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(*self.mem.get(&va).unwrap_or(&0))
        }
        fn write32(&mut self, va: u32, value: u32) -> bool {
            self.mem.insert(va, value);
            true
        }
        fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((va, args.to_vec()));
            Some(1)
        }
        fn call_st0(&mut self, va: u32, args: &[u32]) -> Option<f64> {
            self.calls.push((va, args.to_vec()));
            let length = read_bytes(self, args[0], 255)?.len() as f64;
            self.char_width.map(|w| w * length)
        }
    }

    const BUILDER: u32 = 0x19_4980;
    const BUFFER: u32 = 0x10_0000;

    fn poke_bytes(f: &mut Fake, va: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            let at = va + i as u32;
            let word = f.read32(at & !3).unwrap();
            let shift = (at & 3) * 8;
            f.mem.insert(
                at & !3,
                (word & !(0xFF << shift)) | (u32::from(*b) << shift),
            );
        }
    }

    #[test]
    fn text_page_below_a_live_non_text_page_is_still_found() {
        let (mut f, p) = page_with_field();
        // The real 0x7E800 page is recognised by its widget id (0x4D8D78 at page::ID), not by a builder.
        f.mem.insert(p.0 + page::ID, 0x4D_8D78);
        assert_eq!(top_text_page(&f, &[]), Some(p));
        // Map Settings (non-text) in a higher slot, as when Briefing is clicked.
        let above = PageRef::slot(5);
        f.mem.insert(above.0, 0x1234);
        f.mem.insert(above.0 + page::BUILDER, 0x31_33E0);
        f.mem.insert(above.0 + page::STATE, 3);
        assert_eq!(top_text_page(&f, &[]), Some(p));
    }

    #[test]
    fn mapmaker_commit_owns_bytes_and_parent_return_contract() {
        let mut f = Fake::default();
        let p = PageRef(0x1000);
        let ctx = 0x2000;
        let row = 0x3000;
        f.mem.insert(p.0 + field::BUFFER, BUFFER);
        f.mem.insert(p.0 + 0x224, 0x783CE0);
        f.mem.insert(p.0 + 0x2C, ctx);
        f.mem.insert(ctx + 0x198, row);
        f.mem.insert(row + 0x2C, 0x40);
        f.mem.insert(p.0 + 0x234, 0x4000);
        f.mem.insert(0x4000, 99);
        poke_bytes(&mut f, BUFFER, b"new\0");
        poke_bytes(&mut f, 0x783CE0, b"old\0");
        assert!(mapmaker_string_commit_selected(&mut f, p));
        assert_eq!(read_bytes(&f, 0x783CE0, 223), Some(b"new".to_vec()));
        assert_eq!(f.mem[&0x4000], 0);
        assert_eq!(f.mem[&(row + 0x2C)], 0x48);
        assert_eq!(f.calls, [(0x78D80, vec![p.0]), (0x746D0, vec![ctx])]);
        f.calls.clear();
        assert!(mapmaker_string_commit_selected(&mut f, p));
        assert_eq!(f.mem[&(row + 0x2C)], 0x40);
        poke_bytes(&mut f, BUFFER, b"   \0");
        f.calls.clear();
        assert!(mapmaker_string_commit_selected(&mut f, p));
        assert!(f.calls.is_empty());
        f.mem.insert(p.0 + 0x224, 0x5000);
        assert!(!mapmaker_string_commit_selected(&mut f, p));
    }

    /// A text page in slot 4 with "Ab" (cursor 2), limit 4, and grids whose bytes are space, a..z, then upper case
    /// letters from A as far as the table has keys (every reachable table gets the same fabricated grid).
    fn page_with_field() -> (Fake, PageRef) {
        let mut f = Fake::default();
        let p = PageRef::slot(4);
        f.mem.insert(p.0, 0x4D_8D78);
        f.mem.insert(p.0 + page::BUILDER, BUILDER);
        f.mem.insert(p.0 + page::STATE, 3);
        f.mem.insert(p.0 + field::BUFFER, BUFFER);
        f.mem.insert(p.0 + field::LIMIT, 4);
        f.mem.insert(p.0 + field::CURSOR, 2);
        f.mem.insert(p.0 + field::FLAGS, field::FLAG_SHIFT);
        poke_bytes(&mut f, BUFFER, b"Ab\0");
        let keys: Vec<u8> = [b' ']
            .into_iter()
            .chain(b'a'..=b'z')
            .chain(b'A'..=b'Z')
            .collect();
        for bits in [0, FieldFlags::SHIFT, FieldFlags::SYMBOLS] {
            let table = select_table(FieldFlags(bits));
            for i in 0..table.key_count() {
                let byte = keys.get(i).copied().unwrap_or(0);
                poke_bytes(&mut f, table.bytes + i as u32, &[byte]);
            }
        }
        (f, p)
    }

    fn text(f: &Fake) -> Vec<u8> {
        read_bytes(f, BUFFER, 255).unwrap()
    }

    #[test]
    fn persona_disables_non_alphanumeric_keys_without_edit_or_sound() {
        let (mut f, p) = page_with_field();
        f.mem
            .insert(p.0 + field::FLAGS, FieldFlags::ALPHANUMERIC_ONLY);
        let symbols = select_table(FieldFlags(FieldFlags::SYMBOLS));
        poke_bytes(&mut f, symbols.bytes + 38, &[0xe9, b'!']);
        let mut field = NativeTextField::open(&f, p).unwrap();
        for c in ['é', '!'] {
            assert!(
                field.field.accepts(c),
                "runtime charset includes disabled key"
            );
            assert_eq!(
                field.apply(&mut f, TextCommand::Char(c)),
                TextOutcome::Rejected(EditError::InvalidCharacter)
            );
            assert_eq!(text(&f), b"Ab");
            assert!(f.calls.is_empty(), "disabled keys do not activate");
        }
        assert_eq!(
            field.apply(&mut f, TextCommand::Char('c')),
            TextOutcome::Edited
        );
        assert_eq!(text(&f), b"Abc");
        assert_eq!(f.mem[&(p.0 + field::FLAGS)], FieldFlags::ALPHANUMERIC_ONLY);
        assert_eq!(
            field.apply(&mut f, TextCommand::Char(' ')),
            TextOutcome::Edited
        );
        assert_eq!(text(&f), b"Abc ", "dedicated Space remains enabled");
    }

    #[test]
    fn typing_edits_the_rust_model_and_writes_through() {
        let (mut f, p) = page_with_field();
        let mut field = NativeTextField::open(&f, p).unwrap();
        assert_eq!(field.field.text(), b"Ab");
        assert_eq!(
            field.apply(&mut f, TextCommand::Char('c')),
            TextOutcome::Edited
        );
        assert_eq!(text(&f), b"Abc");
        assert_eq!(f.mem[&(p.0 + field::CURSOR)], 3);
        assert_eq!(
            f.mem[&(p.0 + field::FLAGS)],
            0,
            "shift released, selection cleared"
        );
        assert_eq!(f.mem[&(p.0 + field::DIRTY_FLAGS)], field::DIRTY);
        assert_eq!(
            f.calls,
            vec![(original::KEY_SOUND, vec![]), (original::KEY_SOUND, vec![])]
        );
        field.apply(&mut f, TextCommand::Left);
        field.apply(&mut f, TextCommand::Backspace);
        assert_eq!(text(&f), b"Ac");
        assert_eq!(f.mem[&(p.0 + field::CURSOR)], 1);
        assert!(field.in_sync(&f));
        field.apply(&mut f, TextCommand::Char('B'));
        field.apply(&mut f, TextCommand::Char('z'));
        assert_eq!(
            field.apply(&mut f, TextCommand::Char('q')),
            TextOutcome::Rejected(EditError::TooLong)
        );
        assert_eq!(text(&f), b"ABzc");
        assert_eq!(
            field.apply(&mut f, TextCommand::Char('#')),
            TextOutcome::Rejected(EditError::InvalidCharacter)
        );
    }

    #[test]
    fn enter_commits_through_the_original_unless_blank() {
        let (mut f, p) = page_with_field();
        poke_bytes(&mut f, BUFFER, b"  \0");
        let mut field = NativeTextField::open(&f, p).unwrap();
        assert_eq!(
            field.apply(&mut f, TextCommand::Confirm),
            TextOutcome::Blank
        );
        assert_eq!(
            f.calls,
            vec![(original::KEY_SOUND, vec![])],
            "0x7DA20 ticks before the blank test"
        );
        field.apply(&mut f, TextCommand::Char('x'));
        f.calls.clear();
        assert_eq!(
            field.apply(&mut f, TextCommand::Confirm),
            TextOutcome::Committed
        );
        assert_eq!(
            f.calls,
            vec![(original::KEY_SOUND, vec![]), (original::COMMIT, vec![p.0])]
        );
    }

    #[test]
    fn service_resyncs_after_another_input() {
        let (mut f, _) = page_with_field();
        let queue = TextQueue::default();
        let mut state = None;
        let mut out = Vec::new();
        queue
            .lock()
            .unwrap()
            .extend([TextCommand::Char('d'), TextCommand::Char('e')]);
        assert_eq!(service(&mut f, BUILDER, &queue, &mut state, &mut out), None);
        assert_eq!(text(&f), b"Abde");
        // The pad edits the guest field: the model follows it before the next key.
        poke_bytes(&mut f, BUFFER, b"C\0");
        let p = PageRef::slot(4);
        f.mem.insert(p.0 + field::CURSOR, 1);
        queue.lock().unwrap().push_back(TextCommand::Backspace);
        service(&mut f, BUILDER, &queue, &mut state, &mut out);
        assert_eq!(text(&f), b"");
    }

    #[test]
    fn width_rule_measures_with_the_original_font_metric() {
        let (mut f, p) = page_with_field();
        f.mem.insert(p.0 + field::WIDTH_SCALE, 2f32.to_bits());
        f.mem.insert(original::VIEWPORT_X_SCALE, 10f32.to_bits());
        // Limit 20: three characters of width 7 do not fit, two do.
        f.char_width = Some(7.0);
        f.mem.insert(p.0 + field::CURSOR, 0);
        poke_bytes(&mut f, BUFFER, b"A\0");
        let mut field = NativeTextField::open(&f, p).unwrap();
        assert_eq!(
            field.apply(&mut f, TextCommand::Char('b')),
            TextOutcome::Edited
        );
        assert_eq!(text(&f), b"bA");
        assert!(f.calls.contains(&(original::MEASURE_WIDTH, vec![BUFFER])));
        f.mem.insert(p.0 + field::DIRTY_FLAGS, 0);
        assert_eq!(
            field.apply(&mut f, TextCommand::Char('c')),
            TextOutcome::Rejected(EditError::TooWide)
        );
        assert_eq!(text(&f), b"bA", "rolled back");
        assert_eq!(
            f.mem[&(p.0 + field::DIRTY_FLAGS)],
            0,
            "no dirty mark after a width rollback"
        );
        // No float call on this host: nothing is inserted unmeasured.
        f.char_width = None;
        assert_eq!(
            field.apply(&mut f, TextCommand::Backspace),
            TextOutcome::Edited
        );
        assert_eq!(
            field.apply(&mut f, TextCommand::Char('d')),
            TextOutcome::Rejected(EditError::TooWide)
        );
        assert_eq!(text(&f), b"A");
    }

    #[test]
    fn ring_field_walks_and_overwrites_digits() {
        let (mut f, p) = page_with_field();
        poke_bytes(&mut f, BUFFER, b"12:34\0");
        f.mem.insert(p.0 + field::FLAGS, field::FLAG_NUMERIC_RING);
        f.mem.insert(p.0 + field::CURSOR, 1);
        ring::step(&mut f, p, true);
        assert_eq!(f.mem[&(p.0 + field::CURSOR)], 3, "the colon is skipped");
        ring::key(&mut f, p, b'9');
        assert_eq!(text(&f), b"12:54", "tens of seconds capped at 5");
        assert_eq!(f.mem[&(p.0 + field::CURSOR)], 4);
        ring::step(&mut f, p, true);
        assert_eq!(f.mem[&(p.0 + field::CURSOR)], 0, "wraps");
        ring::delete(&mut f, p);
        assert_eq!(text(&f), b"02:54");
        assert_eq!(
            f.mem[&(p.0 + field::CURSOR)],
            4,
            "back wraps to the last digit"
        );
        assert_eq!(f.mem[&(p.0 + field::DIRTY_FLAGS)], field::DIRTY);
        assert_eq!(
            apply_ring(&mut f, p, TextCommand::Char('x')),
            TextOutcome::Rejected(EditError::InvalidCharacter)
        );
        f.calls.clear();
        assert_eq!(
            apply_ring(&mut f, p, TextCommand::Char('7')),
            TextOutcome::Edited
        );
        assert_eq!(text(&f), b"02:57");
        assert_eq!(
            f.calls,
            vec![(original::KEY_SOUND, vec![])],
            "one key sound in ring mode"
        );
    }

    #[test]
    fn numeric_class_drops_a_leading_zero_prefix() {
        let (mut f, p) = page_with_field();
        f.mem.insert(p.0 + page::ID, field::NUMERIC_CLASS_ID);
        poke_bytes(&mut f, BUFFER, b"0\0");
        f.mem.insert(p.0 + field::CURSOR, 1);
        f.mem.insert(p.0 + field::FLAGS, 0);
        let keys: Vec<u8> = (b'0'..=b'9').collect();
        for bits in [0, FieldFlags::SHIFT, FieldFlags::SYMBOLS] {
            let table = select_table(FieldFlags(bits));
            for i in 0..table.key_count() {
                poke_bytes(&mut f, table.bytes + i as u32, &[keys[i % keys.len()]]);
            }
        }
        let mut field = NativeTextField::open(&f, p).unwrap();
        assert_eq!(
            field.apply(&mut f, TextCommand::Char('7')),
            TextOutcome::Edited
        );
        assert_eq!(text(&f), b"7");
    }
}
