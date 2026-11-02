// SPDX-License-Identifier: GPL-3.0-or-later
//! A name-entry page's field: the owned [`TextField`], the keyboard flags and the runtime key
//! grids, built from a [`FieldSpec`] decoded from the original builder. The pad path selects a
//! grid slot; host characters go through the same field (extension point [`CharacterEvents`]).

use super::field_spec::FieldSpec;
use super::keyboard::{FieldFlags, KeyGrid, all_tables, reachable_charset, select_table};
use super::{CharacterEvents, EditError, Event, Outcome, TextField, TextWidth};

/// The original width rule of 0x7DB20: with a positive field scale, the font-measured text
/// width must not exceed `viewport_x * scale` (`[0x7BA038]` times +0x238). `measure` is the
/// runtime font metric (0x405C0 on the widget's font); the field owns no font data.
pub struct ScaledWidth<F: FnMut(&[u8]) -> f32> {
    pub scale: f32,
    pub viewport_x: f32,
    pub measure: F,
}

impl<F: FnMut(&[u8]) -> f32> TextWidth for ScaledWidth<F> {
    fn fits(&mut self, bytes: &[u8]) -> bool {
        // A non-positive (or NaN) scale skips the measurement, as the original's COMISS does.
        if self.scale.is_nan() || self.scale <= 0.0 {
            return true;
        }
        let width = (self.measure)(bytes);
        // fcmp(width, limit) == 1 rolls back; NaN compares unordered and keeps the character.
        f64::from(width).partial_cmp(&(f64::from(self.viewport_x) * f64::from(self.scale)))
            != Some(std::cmp::Ordering::Greater)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NameEntry {
    pub spec: FieldSpec,
    pub field: TextField,
    pub flags: FieldFlags,
    grids: Vec<KeyGrid>,
}

impl NameEntry {
    /// Builds the field with every reachable grid read through `read`. `initial_shift` is the
    /// constructor's toggle of the shift bit when the global at 0x7BB1E0 holds an entry
    /// (runtime state the caller reads). The charset is the union of the reachable grids.
    pub fn new(
        spec: FieldSpec,
        initial: &[u8],
        initial_shift: bool,
        read: impl Fn(u32) -> Option<u8>,
    ) -> Result<Self, EditError> {
        let mut flags = spec.flags;
        if spec.select_all {
            flags.0 |= FieldFlags::SELECT_ALL;
        }
        if initial_shift {
            flags.0 = (flags.0 ^ FieldFlags::SHIFT) & !FieldFlags::SYMBOLS;
        }
        let grids = all_tables()
            .into_iter()
            .map(|t| KeyGrid::load(t, &read))
            .collect::<Option<Vec<_>>>()
            .ok_or(EditError::InvalidCharacter)?;
        let charset = reachable_charset(flags, &read).ok_or(EditError::InvalidCharacter)?;
        let mut field = TextField::new(initial, spec.limit as usize, &charset)?;
        if spec.select_all && !initial.is_empty() {
            field.select_all();
        }
        Ok(Self {
            spec,
            field,
            flags,
            grids,
        })
    }
    /// The grid the current flags select.
    pub fn grid(&self) -> &KeyGrid {
        let table = select_table(self.flags);
        self.grids
            .iter()
            .find(|g| g.table == table)
            .unwrap_or(&self.grids[0])
    }
    /// Pad press on grid slot `index`: insert its byte with the width rule, then release a
    /// one-shot shift on success (0x7DB20 success path clears bit 2; rollback keeps it).
    pub fn press_key(
        &mut self,
        index: usize,
        width: &mut (impl TextWidth + ?Sized),
    ) -> Result<(), EditError> {
        let byte = self.grid().key(index).ok_or(EditError::InvalidCharacter)?;
        if !self.flags.character_enabled(char::from(byte)) {
            return Err(EditError::InvalidCharacter);
        }
        self.field.pad_character(byte, width)?;
        self.flags.0 &= !FieldFlags::SHIFT;
        Ok(())
    }
    /// Toggle keyboard modifier bits (shift, caps, symbols) as the page's keys do.
    pub fn toggle(&mut self, bits: u32) {
        self.flags.0 ^= bits & (FieldFlags::SHIFT | FieldFlags::CAPS | FieldFlags::SYMBOLS);
    }
    /// Extension point: host characters (keyboard, IME) share the field's charset and width.
    pub fn host_characters(
        &mut self,
        source: &mut impl CharacterEvents,
        width: &mut (impl TextWidth + ?Sized),
    ) -> Vec<Result<(), EditError>> {
        let mut results = Vec::new();
        while let Some(c) = source.next_character() {
            results.push(if self.flags.character_enabled(c) {
                self.field.character_with_width(c, width)
            } else {
                Err(EditError::InvalidCharacter)
            });
        }
        results
    }
    pub fn apply(&mut self, event: Event) -> Result<(), EditError> {
        if let Event::Character(c) = event {
            if !self.flags.character_enabled(c) {
                return Err(EditError::InvalidCharacter);
            }
        }
        self.field.apply(event)
    }
    /// Handler the page calls for the field's terminal outcome: done on confirm, cancel on
    /// cancel (handler slots +0x1CC / +0x1D0, names INFERRED).
    pub fn outcome_handler(&self) -> Option<u32> {
        match self.field.outcome() {
            Outcome::Editing => None,
            Outcome::Confirmed => Some(self.spec.done),
            Outcome::Cancelled => Some(self.spec.cancel),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FieldFlags;
    use super::*;

    fn spec(limit: u32, scale: f32) -> FieldSpec {
        FieldSpec {
            call_site: 0,
            source: None,
            limit,
            width_scale: scale,
            select_all: false,
            flags: FieldFlags(0),
            done: 0xd0,
            cancel: 0xc0,
        }
    }
    /// Synthetic image: every key byte is 'a' + slot % 26, uppercase in alternate tables.
    fn read(a: u32) -> Option<u8> {
        let table = all_tables()
            .into_iter()
            .find(|t| a >= t.bytes && a < t.bytes + t.key_count() as u32)?;
        let slot = (a - table.bytes) as u8 % 26;
        let upper = table == select_table(FieldFlags(FieldFlags::SHIFT))
            || table == select_table(FieldFlags(FieldFlags::SHIFT | FieldFlags::LETTERS_ONLY));
        Some(if upper { b'A' + slot } else { b'a' + slot })
    }

    #[test]
    fn shift_is_one_shot_and_handlers_follow_outcome() {
        let mut entry = NameEntry::new(spec(4, 0.0), b"", false, |a| read(a).or(Some(0))).unwrap();
        let mut w = ScaledWidth {
            scale: 0.0,
            viewport_x: 1.0,
            measure: |_: &[u8]| f32::MAX,
        };
        entry.toggle(FieldFlags::SHIFT);
        entry.press_key(1, &mut w).unwrap();
        entry.press_key(1, &mut w).unwrap();
        assert_eq!(entry.field.text(), b"Bb");
        assert_eq!(entry.outcome_handler(), None);
        entry.apply(Event::Confirm).unwrap();
        assert_eq!(entry.outcome_handler(), Some(0xd0));
    }

    #[test]
    fn width_threshold_keeps_the_original_product_precision() {
        // Two f32 operands fit exactly in an f64 product. Rounding their product to f32
        // first can admit a character just beyond the original x87 threshold.
        let viewport_x = f32::from_bits(1.0f32.to_bits() + 1);
        let scale = 1.5f32;
        let mut width = ScaledWidth { scale, viewport_x, measure: |_: &[u8]| viewport_x * scale };
        assert!(!width.fits(b"a"));
    }

    #[test]
    fn width_rule_uses_viewport_times_scale() {
        let mut entry = NameEntry::new(spec(10, 2.0), b"", false, |a| read(a).or(Some(0))).unwrap();
        let mut w = ScaledWidth {
            scale: 2.0,
            viewport_x: 1.5,
            measure: |t: &[u8]| t.len() as f32,
        };
        for _ in 0..3 {
            entry.press_key(0, &mut w).unwrap();
        }
        entry.toggle(FieldFlags::SHIFT);
        assert_eq!(entry.press_key(0, &mut w), Err(EditError::TooWide));
        // Rollback keeps the one-shot shift armed.
        assert_eq!(entry.flags.0 & FieldFlags::SHIFT, FieldFlags::SHIFT);
        entry.apply(Event::Cancel).unwrap();
        assert_eq!(entry.outcome_handler(), Some(0xc0));
    }
}
