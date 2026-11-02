// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed view of the original text-entry widget record and the readable edit rules of its
//! name-field (ordinary string) mode: 0x7C410 delete, 0x7C4D0 blank test and the insertion
//! half of 0x7DB20. The hybrid lane (`tsfp-ports` `text_field_lane`) runs these over guest
//! memory behind the per-function A/B switch; host-free callers use [`crate::TextField`].
//! Numeric ring mode (flag 0x20) and the zero-padded numeric widget class are not modelled
//! here: the lane hands those to the retained translation. Evidence: docs/t2197-text-field.md.

use super::keyboard::FieldFlags;

/// Byte offsets of the widget record fields this module reads or writes.
pub mod offset {
    /// Class (vtable) pointer.
    pub const CLASS: u32 = 0x4;
    /// Selected key slot of the on-screen grid.
    pub const GRID_INDEX: u32 = 0x18C;
    /// Pointer to the edited, NUL-terminated byte buffer.
    pub const BUFFER: u32 = 0x220;
    /// Pointer to the caller's source buffer (the done handler's write-back target).
    pub const SOURCE: u32 = 0x224;
    /// Flag word ([`FieldFlags`]).
    pub const FLAGS: u32 = 0x228;
    /// Cursor, a byte index into the buffer.
    pub const CURSOR: u32 = 0x22C;
    /// Maximum text length in bytes, excluding the terminator.
    pub const LIMIT: u32 = 0x230;
    /// Width scale (f32); zero or negative disables the font-width rule.
    pub const WIDTH_SCALE: u32 = 0x238;
}

/// Class pointer of the zero-padded numeric widget (0x7DB20 strips its leading zeros first).
pub const ZERO_PADDED_CLASS: u32 = 0x4D_94D4;
/// Flag bit that suppresses the widget dirty call (0x746D0) after a delete.
pub const SUPPRESS_DIRTY: u32 = 0x100;
/// Byte at 0x475CAC: the 0.0f the width scale is compared against.
pub const WIDTH_SCALE_ZERO: u32 = 0x47_5CAC;
/// Runtime x viewport scale global multiplied by the width scale (0x7DB20).
pub const VIEWPORT_X_SCALE: u32 = 0x7B_A038;

/// Byte and dword access to the memory holding a widget (guest memory in the hybrid lane).
pub trait WidgetMemory {
    fn read8(&self, address: u32) -> u8;
    fn read32(&self, address: u32) -> u32;
    fn write8(&mut self, address: u32, value: u8);
    fn write32(&mut self, address: u32, value: u32);
}

/// Read a dword member from an optional guest node, treating a null node as zero.
/// The lifted UI reads the zero page for null font nodes; guarded lanes must avoid
/// issuing that low-address read themselves.
pub fn read_node_field32(m: &impl WidgetMemory, node: u32, offset: u32) -> u32 {
    if node == 0 {
        0
    } else {
        m.read32(node.wrapping_add(offset))
    }
}

/// A widget record at a guest address. All state lives in the record; this is a typed handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Widget(pub u32);

impl Widget {
    fn field(self, offset: u32) -> u32 {
        self.0.wrapping_add(offset)
    }
    pub fn class(self, m: &impl WidgetMemory) -> u32 {
        m.read32(self.field(offset::CLASS))
    }
    pub fn flags(self, m: &impl WidgetMemory) -> FieldFlags {
        FieldFlags(m.read32(self.field(offset::FLAGS)))
    }
    pub fn set_flags(self, m: &mut impl WidgetMemory, flags: FieldFlags) {
        m.write32(self.field(offset::FLAGS), flags.0)
    }
    pub fn cursor(self, m: &impl WidgetMemory) -> u32 {
        m.read32(self.field(offset::CURSOR))
    }
    pub fn set_cursor(self, m: &mut impl WidgetMemory, cursor: u32) {
        m.write32(self.field(offset::CURSOR), cursor)
    }
    pub fn buffer(self, m: &impl WidgetMemory) -> u32 {
        m.read32(self.field(offset::BUFFER))
    }
    pub fn source(self, m: &impl WidgetMemory) -> u32 {
        m.read32(self.field(offset::SOURCE))
    }
    pub fn limit(self, m: &impl WidgetMemory) -> u32 {
        m.read32(self.field(offset::LIMIT))
    }
    pub fn width_scale(self, m: &impl WidgetMemory) -> f32 {
        f32::from_bits(m.read32(self.field(offset::WIDTH_SCALE)))
    }
    pub fn grid_index(self, m: &impl WidgetMemory) -> u32 {
        m.read32(self.field(offset::GRID_INDEX))
    }
    /// Length of the edited text (strlen of the buffer).
    pub fn text_len(self, m: &impl WidgetMemory) -> u32 {
        string_length(m, self.buffer(m))
    }
    /// Whether the name-field rules apply: no numeric ring and not the zero-padded class.
    pub fn is_name_field(self, m: &impl WidgetMemory) -> bool {
        self.flags(m).0 & FieldFlags::NUMERIC_RING == 0 && self.class(m) != ZERO_PADDED_CLASS
    }
}

/// strlen over widget memory.
pub fn string_length(m: &impl WidgetMemory, address: u32) -> u32 {
    let mut length = 0u32;
    while m.read8(address.wrapping_add(length)) != 0 {
        length = length.wrapping_add(1);
    }
    length
}

/// 0x7C4D0: true for an empty string or one made only of ASCII spaces (a name the done
/// handler must refuse).
pub fn is_blank(m: &impl WidgetMemory, address: u32) -> bool {
    let mut at = address;
    loop {
        match m.read8(at) {
            0 => return true,
            b' ' => at = at.wrapping_add(1),
            _ => return false,
        }
    }
}

/// Moves `count` bytes from `from` to `to` front to back (the original `rep movs`, `to < from`).
fn copy_down(m: &mut impl WidgetMemory, to: u32, from: u32, count: u32) {
    for i in 0..count {
        let byte = m.read8(from.wrapping_add(i));
        m.write8(to.wrapping_add(i), byte);
    }
}

/// 0x7C410 in name-field mode (the dirty call is the caller's): a selected text is cleared
/// whole; otherwise a non-zero cursor steps back (dropping one-shot SHIFT) and the byte under
/// the cursor is removed, the tail and terminator shifting down. An empty text is unchanged.
pub fn delete_at_cursor(m: &mut impl WidgetMemory, widget: Widget) {
    let buffer = widget.buffer(m);
    let flags = widget.flags(m);
    if string_length(m, buffer) == 0 {
        return;
    }
    if flags.0 & FieldFlags::SELECT_ALL != 0 {
        widget.set_cursor(m, 0);
        m.write8(buffer, 0);
        let now = widget.flags(m);
        widget.set_flags(m, FieldFlags(now.0 & !FieldFlags::SELECT_ALL));
        return;
    }
    let length = string_length(m, buffer);
    let cursor = widget.cursor(m);
    if (cursor as i32) > 0 {
        widget.set_cursor(m, cursor - 1);
        widget.set_flags(m, FieldFlags(flags.0 & !FieldFlags::SHIFT));
    }
    let cursor = widget.cursor(m);
    let at = buffer.wrapping_add(cursor);
    copy_down(m, at, at.wrapping_add(1), length.wrapping_sub(cursor));
}

/// Whether a delete must mark the widget dirty (0x746D0) afterwards.
pub fn delete_marks_dirty(m: &impl WidgetMemory, widget: Widget) -> bool {
    widget.flags(m).0 & SUPPRESS_DIRTY == 0
}

/// Result of the insertion step of 0x7DB20.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Insert {
    /// The text was already at the limit; nothing was written.
    Full,
    /// `byte` now sits at the cursor; `tail` bytes (the old tail plus terminator minus one)
    /// moved up. The cursor has not advanced yet: the width rule decides first.
    Placed { tail: u32 },
}

/// The insertion step of 0x7DB20 in name-field mode: a selected text is cleared first, then
/// `byte` is inserted at the cursor when the text is shorter than the limit.
pub fn insert_at_cursor(m: &mut impl WidgetMemory, widget: Widget, byte: u8) -> Insert {
    let buffer = widget.buffer(m);
    if widget.flags(m).0 & FieldFlags::SELECT_ALL != 0 {
        widget.set_cursor(m, 0);
        m.write8(buffer, 0);
    }
    let length = string_length(m, buffer);
    if (length as i32) >= (widget.limit(m) as i32) {
        return Insert::Full;
    }
    let cursor = widget.cursor(m);
    let tail = length.wrapping_sub(cursor);
    // Shift the tail and the terminator up by one, from the back.
    let mut index = tail as i32;
    while index >= 0 {
        let from = buffer.wrapping_add(cursor).wrapping_add(index as u32);
        let value = m.read8(from);
        m.write8(from.wrapping_add(1), value);
        index -= 1;
    }
    m.write8(buffer.wrapping_add(cursor), byte);
    Insert::Placed { tail }
}

/// Undoes a [`Insert::Placed`] whose text failed the width rule (the original moves the
/// tail back down; the byte after the new terminator keeps its stale terminator).
pub fn undo_insert(m: &mut impl WidgetMemory, widget: Widget, tail: u32) {
    let at = widget.buffer(m).wrapping_add(widget.cursor(m));
    copy_down(m, at, at.wrapping_add(1), tail.wrapping_add(1));
}

/// Commits a placed byte: the cursor advances and one-shot SHIFT is released.
pub fn commit_insert(m: &mut impl WidgetMemory, widget: Widget) {
    let cursor = widget.cursor(m);
    widget.set_cursor(m, cursor.wrapping_add(1));
    let flags = widget.flags(m);
    widget.set_flags(m, FieldFlags(flags.0 & !FieldFlags::SHIFT));
}

/// Closing step of 0x7DB20 on every non-rejected path: the selection mark is dropped.
pub fn finish_key(m: &mut impl WidgetMemory, widget: Widget) {
    let flags = widget.flags(m);
    widget.set_flags(m, FieldFlags(flags.0 & !FieldFlags::SELECT_ALL));
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flat([u8; 0x400]);
    impl WidgetMemory for Flat {
        fn read8(&self, a: u32) -> u8 {
            self.0[a as usize]
        }
        fn read32(&self, a: u32) -> u32 {
            u32::from_le_bytes(self.0[a as usize..a as usize + 4].try_into().unwrap())
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0[a as usize] = v
        }
        fn write32(&mut self, a: u32, v: u32) {
            self.0[a as usize..a as usize + 4].copy_from_slice(&v.to_le_bytes())
        }
    }
    const BUF: u32 = 0x300;
    fn widget(text: &[u8], cursor: u32, flags: u32, limit: u32) -> (Flat, Widget) {
        let mut m = Flat([0; 0x400]);
        let w = Widget(0);
        m.write32(offset::BUFFER, BUF);
        m.write32(offset::FLAGS, flags);
        m.write32(offset::CURSOR, cursor);
        m.write32(offset::LIMIT, limit);
        for (i, b) in text.iter().enumerate() {
            m.write8(BUF + i as u32, *b);
        }
        (m, w)
    }
    fn text(m: &Flat) -> &[u8] {
        let len = string_length(m, BUF) as usize;
        &m.0[BUF as usize..BUF as usize + len]
    }

    #[test]
    fn insert_commit_and_limit() {
        let (mut m, w) = widget(b"AC", 1, FieldFlags::SHIFT, 3);
        assert_eq!(
            insert_at_cursor(&mut m, w, b'B'),
            Insert::Placed { tail: 1 }
        );
        commit_insert(&mut m, w);
        assert_eq!(text(&m), b"ABC");
        assert_eq!((w.cursor(&m), w.flags(&m).0), (2, 0));
        assert_eq!(insert_at_cursor(&mut m, w, b'D'), Insert::Full);
        assert_eq!(text(&m), b"ABC");
    }

    #[test]
    fn rejected_insert_restores_text_and_selection_clears() {
        let (mut m, w) = widget(b"AB", 0, 0, 8);
        let Insert::Placed { tail } = insert_at_cursor(&mut m, w, b'X') else {
            panic!()
        };
        undo_insert(&mut m, w, tail);
        assert_eq!((text(&m), w.cursor(&m)), (&b"AB"[..], 0));
        let (mut m, w) = widget(b"AB", 2, FieldFlags::SELECT_ALL, 8);
        insert_at_cursor(&mut m, w, b'Z');
        commit_insert(&mut m, w);
        finish_key(&mut m, w);
        assert_eq!((text(&m), w.cursor(&m), w.flags(&m).0), (&b"Z"[..], 1, 0));
    }

    #[test]
    fn delete_and_blank() {
        let (mut m, w) = widget(b"ABC", 2, FieldFlags::SHIFT | SUPPRESS_DIRTY, 8);
        delete_at_cursor(&mut m, w);
        assert_eq!((text(&m), w.cursor(&m)), (&b"AC"[..], 1));
        assert_eq!(w.flags(&m).0, SUPPRESS_DIRTY);
        assert!(!delete_marks_dirty(&m, w));
        let (mut m, w) = widget(b"ABC", 0, 0, 8);
        delete_at_cursor(&mut m, w);
        assert_eq!(text(&m), b"BC");
        let (mut m, w) = widget(b"AB", 1, FieldFlags::SELECT_ALL, 8);
        delete_at_cursor(&mut m, w);
        assert_eq!((text(&m), w.cursor(&m), w.flags(&m).0), (&b""[..], 0, 0));
        let (m, _) = widget(b"  ", 0, 0, 8);
        assert!(is_blank(&m, BUF));
        assert!(is_blank(&m, BUF + 2));
        let (m, _) = widget(b" a", 0, 0, 8);
        assert!(!is_blank(&m, BUF));
    }

    #[test]
    fn null_node_field_is_zero_without_reading_the_zero_page() {
        let (mut m, _) = widget(b"", 0, 0, 8);
        m.write32(0x1c, 0xdead_beef);
        assert_eq!(read_node_field32(&m, 0, 0x1c), 0);
        assert_eq!(read_node_field32(&m, 4, 0x18), m.read32(0x1c));
    }
}
