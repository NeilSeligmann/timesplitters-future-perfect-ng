//! Run-time string banks as a [`StringSource`] (T2189 UI-6 slice 15).
//!
//! Contract (`docs/t-ui-data-model.md` section 9, `docs/t-ui-framework.md`): the common bank
//! `data/str_ts_p<x><lang>.bs` holds ids `0..COMMON_ENTRIES`, the story bank
//! `data/str_ts_p<x><lang>_story.bs` the next `STORY_ENTRIES` ids. Each file starts with one
//! little-endian `u32` per entry, an offset from the start of the file (the original adds the bank
//! base, MEASURED), and the entries point at NUL-terminated 8-bit strings in the same file.
//! Entry counts are MEASURED at run time (3463 and 4149); the file reading is INFERRED from the
//! loader, checked against the owner's disc by the private test (no string enters the repository).

use crate::font::StringSource;
use crate::model::TextId;

pub const COMMON_ENTRIES: usize = 3463;
pub const STORY_ENTRIES: usize = 4149;

/// One bank: per entry the string bytes without the terminator (None when out of range).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bank {
    strings: Vec<Option<Vec<u8>>>,
}

impl Bank {
    /// Parses `count` offsets and their strings. An offset into the table itself, past the end,
    /// or a string without a terminator leaves that entry empty instead of failing the bank.
    pub fn parse(bytes: &[u8], count: usize) -> Option<Self> {
        let table = count.checked_mul(4)?;
        if bytes.len() < table {
            return None;
        }
        let strings = bytes[..table]
            .chunks_exact(4)
            .map(|word| {
                let start = u32::from_le_bytes([word[0], word[1], word[2], word[3]]) as usize;
                let tail = bytes.get(start..).filter(|_| start >= table)?;
                let end = tail.iter().position(|&b| b == 0)?;
                Some(tail[..end].to_vec())
            })
            .collect();
        Some(Self { strings })
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    fn get(&self, index: usize) -> Option<&[u8]> {
        self.strings.get(index)?.as_deref()
    }
}

/// The common bank and (optionally) the story bank, addressed by text id.
#[derive(Clone, Debug, Default)]
pub struct BankStrings {
    pub common: Bank,
    pub story: Option<Bank>,
}

impl StringSource for BankStrings {
    fn bytes(&self, id: TextId) -> Option<&[u8]> {
        let index = usize::try_from(id.0).ok()?;
        match index.checked_sub(self.common.len()) {
            None => self.common.get(index),
            Some(story) => self.story.as_ref()?.get(story),
        }
    }
}

/// Bank file paths on the disc for a language letter (`e f g i s`, MEASURED table 0x4C0A30).
pub fn bank_paths(language: char) -> (String, String) {
    (
        format!("data/str_ts_px{language}.bs"),
        format!("data/str_ts_px{language}_story.bs"),
    )
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED banks with the measured structure.
    use super::*;

    fn bank(strings: &[&[u8]]) -> Vec<u8> {
        let mut table = Vec::new();
        let mut pool = Vec::new();
        let base = strings.len() * 4;
        for s in strings {
            table.extend_from_slice(&((base + pool.len()) as u32).to_le_bytes());
            pool.extend_from_slice(s);
            pool.push(0);
        }
        table.extend(pool);
        table
    }

    #[test]
    fn ids_address_the_common_then_the_story_bank() {
        let common = Bank::parse(&bank(&[b"", b"Play"]), 2).unwrap();
        let story = Bank::parse(&bank(&[b"Mission"]), 1).unwrap();
        let strings = BankStrings {
            common,
            story: Some(story),
        };
        assert_eq!(strings.bytes(TextId(1)), Some(&b"Play"[..]));
        assert_eq!(strings.bytes(TextId(0)), Some(&b""[..]));
        assert_eq!(strings.bytes(TextId(2)), Some(&b"Mission"[..]));
        assert_eq!(strings.bytes(TextId(3)), None);
    }

    #[test]
    fn bad_entries_are_empty_and_short_files_fail() {
        let mut bytes = bank(&[b"ok", b"x"]);
        bytes[4..8].copy_from_slice(&2u32.to_le_bytes()); // points into the table
        let parsed = Bank::parse(&bytes, 2).unwrap();
        assert_eq!(parsed.get(0), Some(&b"ok"[..]));
        assert_eq!(parsed.get(1), None);
        assert!(Bank::parse(&bytes[..6], 2).is_none());
        let unterminated = Bank::parse(&bytes[..10], 1).unwrap();
        assert_eq!(unterminated.get(0), None);
    }

    #[test]
    fn paths_carry_the_language_letter() {
        assert_eq!(
            bank_paths('e'),
            (
                "data/str_ts_pxe.bs".into(),
                "data/str_ts_pxe_story.bs".into()
            )
        );
    }
}
