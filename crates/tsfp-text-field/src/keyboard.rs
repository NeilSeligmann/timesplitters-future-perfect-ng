// SPDX-License-Identifier: GPL-3.0-or-later
//! On-screen keyboard grids: which runtime table a field's flags select (0x7C1B0) and the
//! key bytes read from the owner's image at run time. No table contents are embedded here.
//! Contract and evidence: docs/t2197-text-field.md.

use alloc::vec::Vec;

/// Widget flag word (+0x228 of the original widget). Only the bits this crate reads are named.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldFlags(pub u32);

impl FieldFlags {
    /// One-shot shift: selects the alternate table and is cleared after an inserted key
    /// (0x7DB20 clears it on the success path only).
    pub const SHIFT: u32 = 0x2;
    /// Caps lock: selects the alternate table too, but SHIFT and CAPS together cancel out.
    pub const CAPS: u32 = 0x4;
    /// Symbol table (wins over every other table in its family).
    pub const SYMBOLS: u32 = 0x8;
    /// Replace-selection mark (constructor sets it when the selection argument is non-zero).
    pub const SELECT_ALL: u32 = 0x10;
    /// Numeric ring mode, outside the name-field slice.
    pub const NUMERIC_RING: u32 = 0x20;
    /// Letters-only family (sign bit of the low byte).
    pub const LETTERS_ONLY: u32 = 0x80;
    /// Accented table pair.
    pub const ACCENTS: u32 = 0x1000;

    /// Grid builders 0x7DD10/0x7E070 disable non-ASCII-alphanumeric character keys.
    /// This is a key availability rule, independent of the table selected by 0x7C1B0.
    pub const ALPHANUMERIC_ONLY: u32 = 0x4000;

    /// Whether the original keyboard enables a character. The dedicated Space button is
    /// always enabled, even when the ordinary non-alphanumeric grid keys are disabled.
    pub fn character_enabled(self, character: char) -> bool {
        !self.has(Self::ALPHANUMERIC_ONLY) || character.is_ascii_alphanumeric() || character == ' '
    }

    fn has(self, bit: u32) -> bool {
        self.0 & bit != 0
    }
    /// Shift and caps toggle the alternate table; both together give the base table.
    pub fn alternate_case(self) -> bool {
        (self.has(Self::SHIFT) || self.has(Self::CAPS)) && self.0 & 6 != 6
    }
}

/// Address pair of one runtime table: per-key label pointers and per-key character bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyTable {
    pub labels: u32,
    pub bytes: u32,
}

impl KeyTable {
    const fn new(labels: u32, bytes: u32) -> Self {
        Self { labels, bytes }
    }
    /// Keys in the table: the byte array directly follows the 4-byte label array.
    pub fn key_count(self) -> usize {
        (self.bytes.wrapping_sub(self.labels) / 4) as usize
    }
}

struct Family {
    base: KeyTable,
    alternate: KeyTable,
    symbols: KeyTable,
    accents: KeyTable,
    accents_alternate: KeyTable,
}

const FULL: Family = Family {
    base: KeyTable::new(0x4d_8dd0, 0x4d_8e74),
    alternate: KeyTable::new(0x4d_8ea0, 0x4d_8f44),
    symbols: KeyTable::new(0x4d_8f70, 0x4d_9014),
    accents: KeyTable::new(0x4d_9040, 0x4d_90e4),
    accents_alternate: KeyTable::new(0x4d_9110, 0x4d_91b4),
};
const LETTERS: Family = Family {
    base: KeyTable::new(0x4d_91e0, 0x4d_9254),
    alternate: KeyTable::new(0x4d_9278, 0x4d_92ec),
    symbols: KeyTable::new(0x4d_9310, 0x4d_9384),
    accents: KeyTable::new(0x4d_93a8, 0x4d_941c),
    accents_alternate: KeyTable::new(0x4d_9440, 0x4d_94b4),
};

/// Readable port of 0x7C1B0 `game_font_charset_select` (Tier 1, exhaustive over 16 flag bits).
pub fn select_table(flags: FieldFlags) -> KeyTable {
    let family = if flags.has(FieldFlags::LETTERS_ONLY) {
        &LETTERS
    } else {
        &FULL
    };
    if flags.has(FieldFlags::SYMBOLS) {
        family.symbols
    } else if flags.has(FieldFlags::ACCENTS) {
        if flags.alternate_case() {
            family.accents_alternate
        } else {
            family.accents
        }
    } else if flags.alternate_case() {
        family.alternate
    } else {
        family.base
    }
}

/// Every table a field can reach, for loading a complete charset up front.
pub fn all_tables() -> [KeyTable; 10] {
    [
        FULL.base,
        FULL.alternate,
        FULL.symbols,
        FULL.accents,
        FULL.accents_alternate,
        LETTERS.base,
        LETTERS.alternate,
        LETTERS.symbols,
        LETTERS.accents,
        LETTERS.accents_alternate,
    ]
}

/// One keyboard grid read at run time. A zero byte is a non-character key slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyGrid {
    pub table: KeyTable,
    pub keys: Vec<u8>,
}

impl KeyGrid {
    /// Reads `table` through `read` (virtual address to byte, e.g. `XbeImage::byte`).
    pub fn load(table: KeyTable, read: impl Fn(u32) -> Option<u8>) -> Option<Self> {
        let keys = (0..table.key_count() as u32)
            .map(|i| read(table.bytes + i))
            .collect::<Option<Vec<u8>>>()?;
        Some(Self { table, keys })
    }
    /// Character byte of grid slot `index` (0x7DB20 reads `bytes[+0x18C]`).
    pub fn key(&self, index: usize) -> Option<u8> {
        self.keys.get(index).copied().filter(|b| *b != 0)
    }
    /// The distinct characters of this grid, in grid order.
    pub fn charset(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for b in self.keys.iter().copied().filter(|b| *b != 0) {
            if !out.contains(&b) {
                out.push(b);
            }
        }
        out
    }
}

/// Union of the charsets of every table the flags can reach by toggling shift, caps and
/// symbols (the page toggles those bits). Host characters validate against this set.
pub fn reachable_charset(flags: FieldFlags, read: impl Fn(u32) -> Option<u8>) -> Option<Vec<u8>> {
    let toggles = FieldFlags::SHIFT | FieldFlags::CAPS | FieldFlags::SYMBOLS;
    let mut out = Vec::new();
    for bits in [0, FieldFlags::SHIFT, FieldFlags::SYMBOLS] {
        let table = select_table(FieldFlags((flags.0 & !toggles) | bits));
        for b in KeyGrid::load(table, &read)?.charset() {
            if !out.contains(&b) {
                out.push(b);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn table_selection_precedence() {
        assert_eq!(select_table(FieldFlags(0)), FULL.base);
        assert_eq!(select_table(FieldFlags(2)), FULL.alternate);
        assert_eq!(select_table(FieldFlags(6)), FULL.base);
        assert_eq!(select_table(FieldFlags(0x100a)), FULL.symbols);
        assert_eq!(select_table(FieldFlags(0x1004)), FULL.accents_alternate);
        assert_eq!(select_table(FieldFlags(0x80)), LETTERS.base);
        assert_eq!(FULL.base.key_count(), 41);
        assert_eq!(LETTERS.base.key_count(), 29);
    }
    #[test]
    fn grid_reads_through_the_supplied_image() {
        let table = KeyTable::new(0x100, 0x10c);
        let grid = KeyGrid::load(table, |a| Some(if a == 0x10d { 0 } else { a as u8 })).unwrap();
        assert_eq!(grid.keys.len(), 3);
        assert_eq!(grid.key(1), None);
        assert_eq!(grid.charset(), [0x0c, 0x0e]);
        assert!(KeyGrid::load(table, |_| None).is_none());
    }
}
