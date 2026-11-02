// SPDX-License-Identifier: GPL-3.0-or-later
//! T2433: native, readable owner of three small engine/object frontier leaves (rule 22).
//!
//! No guest-memory dependency: callers hand in plain values and closures, the runtime adapter
//! `tsfp-ports/src/rea_object_runtime.rs` reads and writes the live image at its boundary.
//!
//! | VA | role |
//! |---|---|
//! | `0xB0770` | object field `+0x90` is stamped with the current frame counter, or the previous one |
//! | `0x160F50` | object is type 8 and its `+0x7C` extension tag at `[[ext+0xBF0]+8]` is `VICE` or `VICO` |
//! | `0x162520` | entry flag of the `0x7A2820` array from a record bit mask and a 20-slot table |
//!
//! Evidence class: INFERRED names (docs/t1261-naming-*), ABI and semantics MEASURED against the
//! lifted bodies by the Tier 1 differential `crates/tsfp-gameplay/tests/rea_object_flags_t2433.rs`
//! (weak: fabricated memory, register-level transliteration as reference).

/// Tag `VICE` (little-endian dword `56 49 43 45`).
pub const TAG_VICE: u32 = 0x4543_4956;
/// Tag `VICO` (little-endian dword `56 49 43 4F`).
pub const TAG_VICO: u32 = 0x4f43_4956;
/// Object type value that carries the extension tag.
pub const TAGGED_OBJECT_TYPE: u32 = 8;

/// The frame counter pair that stamps object field `+0x90`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameStamp {
    /// `[0x7DE318]`: the current frame counter. Zero means no frame is running.
    pub current: u32,
    /// `[0x4E7944]`: how far back the previous frame counter is.
    pub previous_delta: u32,
}

impl FrameStamp {
    /// True when `stamp` is non-zero and equals the current counter or the previous one.
    /// With no running frame (`current == 0`) nothing is recent.
    pub fn is_recent(&self, stamp: u32) -> bool {
        self.current != 0
            && stamp != 0
            && (stamp == self.current || stamp == self.current.wrapping_sub(self.previous_delta))
    }
}

/// True for the `VICE` and `VICO` extension tags.
pub fn is_vice_or_vico_tag(tag: u32) -> bool {
    tag == TAG_VICE || tag == TAG_VICO
}

/// Number of slot bits a record mask is tested against (four groups of five).
pub const SLOT_COUNT: u32 = 20;

/// Which dword of a slot row decides a hit. Rows are `0x60` bytes apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotWord {
    /// Row offset `+0`: the first table of the family (`0x7A2DE4` for the first row group).
    First,
    /// Row offset `+4`: the second table (`0x7A2DE8`).
    Second,
}

/// One member of the entry-flag family. Members differ only in these three fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryFlagRule {
    /// Offset of the 32-bit slot mask inside the `0x98`-byte record.
    pub mask_offset: u32,
    /// Which row dword is tested.
    pub slot_word: SlotWord,
    /// Value stored in the entry flag on a hit.
    pub hit_value: u32,
}

/// `0x162450` (a thread entry reached through a data pointer, not lane-eligible).
pub const RULE_162450: EntryFlagRule = EntryFlagRule {
    mask_offset: 0x70,
    slot_word: SlotWord::First,
    hit_value: 1,
};
/// `0x162520`: same as `0x162450` against the second table.
pub const RULE_162520: EntryFlagRule = EntryFlagRule {
    mask_offset: 0x70,
    slot_word: SlotWord::Second,
    hit_value: 1,
};
/// `0x163220` (thread entry, not lane-eligible): mask at `+0x38`, stores 3.
pub const RULE_163220: EntryFlagRule = EntryFlagRule {
    mask_offset: 0x38,
    slot_word: SlotWord::First,
    hit_value: 3,
};

impl EntryFlagRule {
    /// The entry flag after the rule: `hit_value` when any set mask bit `s` (`s < 20`) has a
    /// non-zero slot word, otherwise 0. `slot_word(s)` reads the rule's dword of slot row `s`.
    pub fn flag(&self, mask: u32, mut slot_word: impl FnMut(u32) -> u32) -> u32 {
        let mut flag = 0;
        for slot in 0..SLOT_COUNT {
            if mask & (1 << slot) != 0 && slot_word(slot) != 0 {
                flag = self.hit_value;
            }
        }
        flag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_stamp_matches_current_or_previous_only() {
        let stamp = FrameStamp {
            current: 0x1000,
            previous_delta: 0x10,
        };
        assert!(stamp.is_recent(0x1000));
        assert!(stamp.is_recent(0x0ff0));
        assert!(!stamp.is_recent(0x0fe0));
        assert!(!stamp.is_recent(0));
        assert!(
            !FrameStamp {
                current: 0,
                previous_delta: 0
            }
            .is_recent(0)
        );
        assert!(
            !FrameStamp {
                current: 0,
                previous_delta: 0x10
            }
            .is_recent(0xfffffff0)
        );
    }

    #[test]
    fn vice_and_vico_tags_are_ascii() {
        assert_eq!(&TAG_VICE.to_le_bytes(), b"VICE");
        assert_eq!(&TAG_VICO.to_le_bytes(), b"VICO");
        assert!(is_vice_or_vico_tag(TAG_VICE) && is_vice_or_vico_tag(TAG_VICO));
        assert!(!is_vice_or_vico_tag(0) && !is_vice_or_vico_tag(TAG_VICE ^ 1));
    }

    #[test]
    fn entry_flag_needs_a_set_bit_and_a_populated_slot() {
        let populated = |slot: u32| u32::from(slot == 7);
        assert_eq!(RULE_162520.flag(1 << 7, populated), 1);
        assert_eq!(RULE_163220.flag(1 << 7, populated), 3);
        assert_eq!(RULE_162520.flag(1 << 6, populated), 0);
        assert_eq!(RULE_162520.flag(0, populated), 0);
        // bits at or above the slot count are ignored
        assert_eq!(RULE_162520.flag(0xfff0_0000, |_| 1), 0);
        assert_eq!(RULE_162520.flag(1 << 19, |_| 1), 1);
    }
}
