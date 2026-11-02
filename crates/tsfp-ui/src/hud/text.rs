// SPDX-License-Identifier: GPL-3.0-or-later
//! Attachment point 3: label id to string.
//!
//! The game looks strings up by id in per-language banks (`tsfp-format-bs`, T-ui-framework 5.1).
//! The HUD only needs "give me the text of id N".

use std::collections::BTreeMap;
use tsfp_format_bs::Bank;

/// Text source of the HUD.
pub trait TextProvider {
    /// The string of a label id, or `None` when the id has no text.
    fn label(&self, id: u32) -> Option<String>;
}

/// A decoded string bank (`tsfp-format-bs`). Ids index the bank directly.
pub struct BankText {
    pub bank: Bank,
}

impl TextProvider for BankText {
    fn label(&self, id: u32) -> Option<String> {
        let bytes = self.bank.string(id as usize).ok()?;
        Some(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Native UI attachment for the original common/story banks. Callers can opt into this provider
/// while the existing `BankText` path remains available as the default reference behavior.
impl TextProvider for crate::native_text::TextBanks {
    fn label(&self, id: u32) -> Option<String> {
        self.lookup(crate::native_text::LabelId::new(id))
            .ok()
            .map(crate::native_text::TextLookup::to_latin1_string)
    }
}

/// An in-memory table, for tests and for the picture tool.
#[derive(Default)]
pub struct MapText(pub BTreeMap<u32, String>);

impl TextProvider for MapText {
    fn label(&self, id: u32) -> Option<String> {
        self.0.get(&id).cloned()
    }
}

/// Label ids the HUD functions look up, MEASURED as constant first arguments of 0x3D7B0 calls
/// (docs/t-hud.md, "Label findings").
pub mod ids {
    pub const SCORE: u32 = 0x164e;
    pub const LIVES: u32 = 0x15fa;
    pub const RANK: u32 = 0x1653;
    pub const TIME: u32 = 0x15fb;
    pub const BAG_TIME: u32 = 0x164d;
    pub const TIME_LEFT: u32 = 0x164f;
    pub const YOU_KILLED: u32 = 0x164a;
}
