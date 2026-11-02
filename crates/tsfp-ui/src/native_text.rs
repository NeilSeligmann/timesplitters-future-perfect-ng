// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned view of the original common and story text banks.
//!
//! This model owns parsed bank bytes and keeps the original lookup split explicit. It is not
//! wired into the hybrid host yet; the existing guest-memory path remains the default until the
//! loader and UI attachment points are migrated and checked together.

use tsfp_format_bs::Bank;

/// First original label ID stored in the story bank (`0x3D7B0`).
pub const STORY_FIRST_ID: u32 = 0x0d87;

/// Typed label key. The original accepts a signed integer, but valid bank IDs are non-negative.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LabelId(u32);

impl LabelId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Which original bank supplied a text string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankKind {
    Common,
    Story,
}

/// A safe lookup result preserving the original special cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextLookup<'a> {
    /// The game's runtime-provided missing-text string. It is supplied by the caller so no game
    /// text needs to be copied into this crate.
    Missing(&'a [u8]),
    /// ID zero returns the original constant empty string when the story bank is loaded.
    Empty,
    /// A bank-backed byte string, excluding its NUL terminator.
    BankText { bank: BankKind, bytes: &'a [u8] },
}

impl<'a> TextLookup<'a> {
    /// Original bank bytes for display or comparison.
    pub const fn bytes(self) -> &'a [u8] {
        match self {
            Self::Missing(bytes) | Self::BankText { bytes, .. } => bytes,
            Self::Empty => b"",
        }
    }

    /// Decode the game's single-byte text as Latin-1. `String::from_utf8_lossy` is not suitable:
    /// every byte from `0x80` through `0xFF` maps to the same-numbered Unicode scalar here.
    pub fn to_latin1_string(self) -> String {
        self.bytes().iter().copied().map(char::from).collect()
    }
}

/// Owned parsed bank pair for the current language.
///
/// The story bank's presence is checked before ID zero or the common bank, matching the original
/// `0x3D7B0` order. The format's entry counts remain loader inputs, as in the original bank-file
/// contract.
#[derive(Clone, Debug)]
pub struct TextBanks {
    common: Option<Bank>,
    story: Option<Bank>,
    missing_text: Vec<u8>,
}

impl TextBanks {
    pub fn new(common: Option<Bank>, story: Option<Bank>, missing_text: Vec<u8>) -> Self {
        Self {
            common,
            story,
            missing_text: without_nul_terminator(missing_text),
        }
    }

    /// Read a label from its owning bank. Invalid IDs are rejected by the Rust model;
    /// valid original IDs retain the exact common/story split and missing-bank behavior.
    pub fn lookup(&self, id: LabelId) -> Result<TextLookup<'_>, InvalidLabelId> {
        let Some(story) = self.story.as_ref() else {
            return Ok(TextLookup::Missing(&self.missing_text));
        };

        if id.0 == 0 {
            return Ok(TextLookup::Empty);
        }

        let (bank_kind, bank, index) = if id.0 < STORY_FIRST_ID {
            let Some(common) = self.common.as_ref() else {
                return Ok(TextLookup::Missing(&self.missing_text));
            };
            (BankKind::Common, common, id.0 as usize)
        } else {
            (BankKind::Story, story, (id.0 - STORY_FIRST_ID) as usize)
        };

        let bytes = bank
            .string(index)
            .map_err(|_| InvalidLabelId { raw: id.0 })?;
        Ok(TextLookup::BankText {
            bank: bank_kind,
            bytes,
        })
    }

    pub fn common(&self) -> Option<&Bank> {
        self.common.as_ref()
    }

    pub fn story(&self) -> Option<&Bank> {
        self.story.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidLabelId {
    pub raw: u32,
}

fn without_nul_terminator(mut bytes: Vec<u8>) -> Vec<u8> {
    if let Some(end) = bytes.iter().position(|byte| *byte == 0) {
        bytes.truncate(end);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::{BankKind, LabelId, STORY_FIRST_ID, TextBanks, TextLookup};
    use crate::hud::text::TextProvider;
    use tsfp_format_bs::Bank;

    fn bank(text: &[u8]) -> Bank {
        // One offset-table entry followed by a NUL-terminated byte string.
        Bank {
            offsets: vec![4],
            pool: [text, &[0]].concat(),
        }
    }

    #[test]
    fn preserves_lookup_order_and_latin1_bytes() {
        let banks = TextBanks::new(
            Some(bank(&[b'C', 0xe9])),
            Some(bank(&[b'S', 0xf1])),
            b"missing\0ignored".to_vec(),
        );

        assert_eq!(banks.lookup(LabelId::new(0)).unwrap(), TextLookup::Empty);
        assert_eq!(
            banks.lookup(LabelId::new(1)).unwrap(),
            TextLookup::BankText {
                bank: BankKind::Common,
                bytes: &[b'C', 0xe9],
            }
        );
        assert_eq!(banks.label(1).as_deref(), Some("Cé"));
        let story = banks.lookup(LabelId::new(STORY_FIRST_ID)).unwrap();
        assert_eq!(
            story,
            TextLookup::BankText {
                bank: BankKind::Story,
                bytes: &[b'S', 0xf1],
            }
        );
        assert_eq!(story.to_latin1_string(), "Sñ");

        let no_story = TextBanks::new(Some(bank(b"common")), None, b"not loaded\0".to_vec());
        assert_eq!(
            no_story.lookup(LabelId::new(0)).unwrap(),
            TextLookup::Missing(b"not loaded")
        );

        let no_common = TextBanks::new(None, Some(bank(b"story")), b"missing".to_vec());
        assert_eq!(
            no_common.lookup(LabelId::new(1)).unwrap(),
            TextLookup::Missing(b"missing")
        );
        assert_eq!(
            no_common.lookup(LabelId::new(STORY_FIRST_ID)).unwrap(),
            TextLookup::BankText {
                bank: BankKind::Story,
                bytes: b"story",
            }
        );
    }
}
