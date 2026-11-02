//! Typed input boundary for the masked selector at original address `0x613D0`.
//!
//! The selector lookup tables behind `0x56FC0` remain caller supplied. This
//! module models only the observed descriptor scan and does not assign
//! semantic names to its masks or returned words.

/// One descriptor entry at `descriptor + 8 + index * 4`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaskedSelectorEntry {
    /// Unsigned word returned when this is the requested matching ordinal.
    pub mapped_value: u16,
    /// Bit mask tested against the caller's low 16 bits.
    pub selector_mask: u16,
}

const _: [(); 4] = [(); core::mem::size_of::<MaskedSelectorEntry>()];

/// Raw descriptor header state used by `0x613D0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaskedSelectorDescriptor<'a> {
    /// Signed word at descriptor offset `+6`.
    pub signed_entry_count: i16,
    /// Entries beginning at descriptor offset `+8`.
    pub entries: &'a [MaskedSelectorEntry],
}

/// Select the one-based `ordinal` entry whose mask intersects `caller_mask`.
///
/// The original routine returns `-1` when selector resolution fails or no
/// requested match occurs. `None` represents that result; a match returns the
/// zero-extended descriptor word. Descriptor bounds are explicit inputs, so a
/// truncated Rust slice safely ends the scan without reading guest memory.
pub fn select_nth_masked_value_613d0(
    descriptor: MaskedSelectorDescriptor<'_>,
    ordinal: i32,
    caller_mask: u32,
) -> Option<u16> {
    if descriptor.signed_entry_count <= 0 || ordinal <= 0 {
        return None;
    }

    let mut remaining = ordinal;
    for entry in descriptor
        .entries
        .iter()
        .take(descriptor.signed_entry_count as usize)
    {
        if entry.selector_mask & caller_mask as u16 == 0 {
            continue;
        }

        remaining -= 1;
        if remaining == 0 {
            return Some(entry.mapped_value);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{MaskedSelectorDescriptor, MaskedSelectorEntry, select_nth_masked_value_613d0};

    #[test]
    fn selects_one_based_matching_mask_ordinal() {
        let entries = [
            MaskedSelectorEntry {
                mapped_value: 10,
                selector_mask: 0x01,
            },
            MaskedSelectorEntry {
                mapped_value: 20,
                selector_mask: 0x04,
            },
            MaskedSelectorEntry {
                mapped_value: 30,
                selector_mask: 0x05,
            },
        ];
        let descriptor = MaskedSelectorDescriptor {
            signed_entry_count: 3,
            entries: &entries,
        };

        assert_eq!(select_nth_masked_value_613d0(descriptor, 1, 0x01), Some(10));
        assert_eq!(select_nth_masked_value_613d0(descriptor, 2, 0x01), Some(30));
        assert_eq!(select_nth_masked_value_613d0(descriptor, 1, 0x04), Some(20));
    }

    #[test]
    fn nonpositive_count_or_ordinal_returns_no_match() {
        let descriptor = MaskedSelectorDescriptor {
            signed_entry_count: -1,
            entries: &[],
        };
        assert_eq!(select_nth_masked_value_613d0(descriptor, 1, 1), None);
        let descriptor = MaskedSelectorDescriptor {
            signed_entry_count: 0,
            entries: &[],
        };
        assert_eq!(select_nth_masked_value_613d0(descriptor, 1, 1), None);
        let descriptor = MaskedSelectorDescriptor {
            signed_entry_count: 1,
            entries: &[],
        };
        assert_eq!(select_nth_masked_value_613d0(descriptor, 0, 1), None);
        assert_eq!(select_nth_masked_value_613d0(descriptor, -1, 1), None);
    }

    #[test]
    fn truncated_entry_slice_ends_the_scan_safely() {
        let entries = [MaskedSelectorEntry {
            mapped_value: 0x1234,
            selector_mask: 0x02,
        }];
        let descriptor = MaskedSelectorDescriptor {
            signed_entry_count: 2,
            entries: &entries,
        };
        assert_eq!(
            select_nth_masked_value_613d0(descriptor, 1, 0x02),
            Some(0x1234)
        );
        assert_eq!(select_nth_masked_value_613d0(descriptor, 2, 0x02), None);
    }
}
