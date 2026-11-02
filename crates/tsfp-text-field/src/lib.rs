// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned byte text field. See docs/t2197-text-field.md for evidence and integration debt.

// portreg: wraps=tsfp-ports/text_field_lane
// portreg: mirrors=tsfp-ports/c2r/ui_framework_2
/// T2197 native name-field slice roots. The keyboard/widget models implement these
/// contracts; numeric modes and guest ABI/callee dispatch remain in the hybrid lane.
/// Registration alone supplies no verification tier or archived original proof.
pub const ROOTS: [u32; 4] = [0x7c1b0, 0x7c410, 0x7c4d0, 0x7db20];

extern crate alloc;

pub mod field_spec;
pub mod keyboard;
pub mod name_entry;
pub mod widget;

/// Editable single-byte text with owned cursor, selection, and cancel state.
///
/// Characters must belong to the caller's runtime charset. Use
/// [`TextField::character_with_width`] when font width limits apply.
///
/// ```
/// use tsfp_text_field::{Event, Outcome, TextField};
/// let mut field = TextField::new(b"AB", 3, b"ABC").unwrap();
/// field.apply(Event::Left).unwrap();
/// field.apply(Event::Character('C')).unwrap();
/// assert_eq!(field.text(), b"ACB");
/// field.apply(Event::Confirm).unwrap();
/// assert_eq!(field.outcome(), Outcome::Confirmed);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextField {
    text: Vec<u8>,
    initial: Vec<u8>,
    cursor: usize,
    limit: usize,
    charset: Vec<u8>,
    replace_selection: bool,
    outcome: Outcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Editing,
    Confirmed,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Left,
    Right,
    Character(char),
    Backspace,
    Confirm,
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditError {
    InvalidCharacter,
    TooLong,
    TooWide,
    Closed,
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self {
            Self::InvalidCharacter => "character is not in the active charset",
            Self::TooLong => "text exceeds the character limit",
            Self::TooWide => "text exceeds the font width limit",
            Self::Closed => "text field is closed",
        };
        f.write_str(reason)
    }
}

impl std::error::Error for EditError {}

/// Supplies the language/flags-selected charset from runtime disc tables.
/// Host character input and pad keys share this exact validation path.
/// Mirrors the optional font-width constraint of 0x7DB20. Implement using runtime
/// font metrics and the field's width/scale; the model owns no renderer or font data.
pub trait TextWidth {
    fn fits(&mut self, bytes: &[u8]) -> bool;
}

pub trait CharacterEvents {
    fn next_character(&mut self) -> Option<char>;
}

impl TextField {
    /// Capacity counts single-byte characters, excludes the NUL, and is clamped to 255
    /// (0x7D090). The caller supplies the charset; no original strings are embedded.
    pub fn new(initial: &[u8], limit: usize, charset: &[u8]) -> Result<Self, EditError> {
        let limit = limit.min(255);
        if initial.len() > limit {
            return Err(EditError::TooLong);
        }
        if initial.iter().any(|b| *b == 0 || !charset.contains(b)) {
            return Err(EditError::InvalidCharacter);
        }
        Ok(Self {
            text: initial.to_vec(),
            initial: initial.to_vec(),
            cursor: initial.len(),
            limit,
            charset: charset.iter().copied().filter(|b| *b != 0).collect(),
            replace_selection: false,
            outcome: Outcome::Editing,
        })
    }
    pub fn text(&self) -> &[u8] {
        &self.text
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn outcome(&self) -> Outcome {
        self.outcome
    }
    /// True when `character` is a single byte in the field's runtime charset.
    pub fn accepts(&self, character: char) -> bool {
        u8::try_from(u32::from(character)).is_ok_and(|b| b != 0 && self.charset.contains(&b))
    }
    /// Marks the entire buffer for replacement by the next character or backspace.
    /// Navigation does not clear this mark; any processed event does.
    pub fn select_all(&mut self) {
        if self.outcome == Outcome::Editing {
            self.replace_selection = true;
        }
    }
    /// Original blank-name predicate (0x7C4D0): only ASCII spaces count as blank.
    pub fn can_confirm(&self) -> bool {
        self.text.iter().any(|b| *b != b' ')
    }
    /// Applies one event without a font-width check.
    ///
    /// Confirm leaves an all-space buffer open. Cancel restores the initial bytes;
    /// either terminal outcome makes later events return [`EditError::Closed`].
    pub fn apply(&mut self, event: Event) -> Result<(), EditError> {
        if self.outcome != Outcome::Editing {
            return Err(EditError::Closed);
        }
        match event {
            Event::Left => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            Event::Right => {
                self.cursor = (self.cursor + 1).min(self.text.len());
            }
            Event::Character(c) => {
                let byte = u8::try_from(u32::from(c)).map_err(|_| EditError::InvalidCharacter)?;
                if byte == 0 || !self.charset.contains(&byte) {
                    return Err(EditError::InvalidCharacter);
                }
                // The original clears a selected buffer before testing capacity (0x7DB20).
                if self.replace_selection {
                    self.text.clear();
                    self.cursor = 0;
                }
                if self.text.len() >= self.limit {
                    self.replace_selection = false;
                    return Err(EditError::TooLong);
                }
                self.text.insert(self.cursor, byte);
                self.cursor += 1;
            }
            Event::Backspace => {
                if self.replace_selection {
                    self.text.clear();
                    self.cursor = 0;
                } else if !self.text.is_empty() {
                    // At cursor zero the original deletes the first byte (0x7C410).
                    self.cursor = self.cursor.saturating_sub(1);
                    self.text.remove(self.cursor);
                }
            }
            Event::Confirm => {
                if self.can_confirm() {
                    self.outcome = Outcome::Confirmed;
                }
            }
            Event::Cancel => {
                self.text.clone_from(&self.initial);
                self.cursor = self.text.len();
                self.outcome = Outcome::Cancelled;
            }
        }
        self.replace_selection = false;
        Ok(())
    }
    /// Pad grid selections already resolve to bytes through the runtime charset.
    pub fn pad_character(
        &mut self,
        byte: u8,
        width: &mut (impl TextWidth + ?Sized),
    ) -> Result<(), EditError> {
        self.character_with_width(char::from(byte), width)
    }
    /// Inserts one character and asks the caller's font metric whether it fits.
    /// A width failure restores ordinary text and cursor state. After select-all,
    /// the original clears the selected bytes before measuring, so failure leaves
    /// the field empty.
    pub fn character_with_width(
        &mut self,
        character: char,
        width: &mut (impl TextWidth + ?Sized),
    ) -> Result<(), EditError> {
        let before = self.clone();
        self.apply(Event::Character(character))?;
        if !width.fits(&self.text) {
            // Selected text was cleared before the attempted insertion in the original.
            if before.replace_selection {
                self.text.clear();
                self.cursor = 0;
            } else {
                self.text = before.text;
                self.cursor = before.cursor;
            }
            return Err(EditError::TooWide);
        }
        Ok(())
    }
    pub fn host_characters(
        &mut self,
        source: &mut impl CharacterEvents,
        width: &mut (impl TextWidth + ?Sized),
    ) -> Vec<Result<(), EditError>> {
        let mut results = Vec::new();
        while let Some(c) = source.next_character() {
            results.push(self.character_with_width(c, width));
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edit_errors_can_cross_standard_error_boundaries() {
        fn propagate() -> Result<(), Box<dyn std::error::Error>> {
            let mut field = TextField::new(b"a", 1, b"a")?;
            field.apply(Event::Character('a'))?;
            Ok(())
        }
        let error = propagate().unwrap_err();
        assert_eq!(error.to_string(), "text exceeds the character limit");
        for reason in [
            EditError::InvalidCharacter,
            EditError::TooLong,
            EditError::TooWide,
            EditError::Closed,
        ] {
            assert!(!reason.to_string().is_empty());
        }
    }
    struct Width(usize);
    impl TextWidth for Width {
        fn fits(&mut self, text: &[u8]) -> bool {
            text.len() <= self.0
        }
    }
    #[test]
    fn insert_limit_selection_and_width() {
        let mut field = TextField::new(b"ab", 3, b"abc ").unwrap();
        field.apply(Event::Left).unwrap();
        field.pad_character(b'c', &mut Width(3)).unwrap();
        assert_eq!(field.text(), b"acb");
        assert_eq!(field.apply(Event::Character('a')), Err(EditError::TooLong));
        field.select_all();
        field.apply(Event::Character(' ')).unwrap();
        field.apply(Event::Confirm).unwrap();
        assert_eq!(field.outcome(), Outcome::Editing);
        field.select_all();
        assert_eq!(
            field.character_with_width('a', &mut Width(0)),
            Err(EditError::TooWide)
        );
        assert_eq!(field.text(), b"");
        field.apply(Event::Cancel).unwrap();
        assert_eq!(field.text(), b"ab");
        assert_eq!(field.apply(Event::Right), Err(EditError::Closed));
    }
    #[test]
    fn charset_and_terminal_confirm() {
        let mut field = TextField::new(b"", 1000, &[b'a', 0xe9]).unwrap();
        assert_eq!(
            field.apply(Event::Character('😀')),
            Err(EditError::InvalidCharacter)
        );
        assert_eq!(
            field.apply(Event::Character('\0')),
            Err(EditError::InvalidCharacter)
        );
        field.apply(Event::Character('é')).unwrap();
        assert_eq!(field.text(), &[0xe9]);
        field.apply(Event::Confirm).unwrap();
        assert_eq!(field.outcome(), Outcome::Confirmed);
    }

    #[test]
    fn navigation_and_cancel_preserve_initial_bytes() {
        let charset: Vec<u8> = (1..=255).collect();
        for length in 0..=32 {
            let initial: Vec<u8> = (0..length).map(|i| 1 + (i * 37 % 255) as u8).collect();
            let mut field = TextField::new(&initial, 255, &charset).unwrap();
            for _ in 0..(length + 3) {
                field.apply(Event::Left).unwrap();
            }
            assert_eq!(field.cursor(), 0);
            for _ in 0..(length + 3) {
                field.apply(Event::Right).unwrap();
            }
            assert_eq!(field.cursor(), length);
            field.apply(Event::Character('A')).unwrap();
            field.apply(Event::Cancel).unwrap();
            assert_eq!(field.text(), initial);
            assert_eq!(field.cursor(), length);
            assert_eq!(field.outcome(), Outcome::Cancelled);
            assert_eq!(field.apply(Event::Backspace), Err(EditError::Closed));
        }
    }

    #[test]
    fn width_rejection_is_transactional_except_selected_replacement() {
        for length in 1..=32 {
            let initial = vec![b'a'; length];
            for cursor in [0, length / 2, length] {
                let mut field = TextField::new(&initial, 255, b"ab").unwrap();
                for _ in cursor..length {
                    field.apply(Event::Left).unwrap();
                }
                assert_eq!(
                    field.character_with_width('b', &mut Width(length)),
                    Err(EditError::TooWide)
                );
                assert_eq!(field.text(), initial);
                assert_eq!(field.cursor(), cursor);
            }
            let mut selected = TextField::new(&initial, 255, b"ab").unwrap();
            selected.select_all();
            assert_eq!(
                selected.character_with_width('b', &mut Width(0)),
                Err(EditError::TooWide)
            );
            assert_eq!(selected.text(), b"");
            assert_eq!(selected.cursor(), 0);
        }
    }
}
