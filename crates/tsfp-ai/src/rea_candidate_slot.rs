//! Rust-owned model of the bounded slot selection performed by 0xCFEC0.
//!
//! This helper preserves the observed raw predicates and ordering. Slot names,
//! table ownership, and the meaning of the selector argument remain unknown.

/// Raw words observed at the two tested locations in each 0xCC-byte slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateSlotRaw {
    /// Dword at slot base (`argument + 0x1030 + index * 0xCC`).
    pub base_word: u32,
    /// Dword at slot base + 0x0C.
    pub word_0c: u32,
}

/// Result of the measured selector. `Fallback` corresponds to the raw pointer
/// returned from the first argument's +0xF78 field when no slot is eligible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateSlotSelection {
    Slot(usize),
    Fallback,
}

/// Select from the six slots scanned by original helper 0xCFEC0.
///
/// Both observed slot words must be nonzero. A nonzero `selector` skips that
/// many eligible slots. If the requested ordinal is unavailable, the original
/// restarts at zero and returns the first eligible slot. A zero selector
/// returns the first eligible slot immediately. Raw field and selector
/// semantics are intentionally unnamed.
pub fn select_candidate_slot(
    slots: [CandidateSlotRaw; 6],
    selector: u32,
) -> CandidateSlotSelection {
    let eligible = slots
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| (slot.base_word != 0 && slot.word_0c != 0).then_some(index));

    let eligible: [usize; 6] = {
        let mut indices = [usize::MAX; 6];
        let mut len = 0;
        for index in eligible {
            indices[len] = index;
            len += 1;
        }
        if len == 0 {
            return CandidateSlotSelection::Fallback;
        }
        indices
    };

    let count = eligible
        .iter()
        .take_while(|&&index| index != usize::MAX)
        .count();
    let ordinal = usize::try_from(selector).unwrap_or(usize::MAX);
    CandidateSlotSelection::Slot(if ordinal < count {
        eligible[ordinal]
    } else {
        eligible[0]
    })
}

#[cfg(test)]
mod tests {
    use super::{CandidateSlotRaw, CandidateSlotSelection, select_candidate_slot};

    fn eligible() -> CandidateSlotRaw {
        CandidateSlotRaw {
            base_word: 1,
            word_0c: 1,
        }
    }

    #[test]
    fn selects_requested_eligible_ordinal_and_falls_back_to_first_when_out_of_range() {
        let slots = [
            CandidateSlotRaw::default(),
            eligible(),
            CandidateSlotRaw::default(),
            eligible(),
            CandidateSlotRaw::default(),
            eligible(),
        ];

        assert_eq!(
            select_candidate_slot(slots, 0),
            CandidateSlotSelection::Slot(1)
        );
        assert_eq!(
            select_candidate_slot(slots, 1),
            CandidateSlotSelection::Slot(3)
        );
        assert_eq!(
            select_candidate_slot(slots, 2),
            CandidateSlotSelection::Slot(5)
        );
        assert_eq!(
            select_candidate_slot(slots, 3),
            CandidateSlotSelection::Slot(1)
        );
    }

    #[test]
    fn falls_back_only_when_no_slot_is_eligible() {
        let mut slots = [CandidateSlotRaw::default(); 6];
        assert_eq!(
            select_candidate_slot(slots, 0),
            CandidateSlotSelection::Fallback
        );

        slots[4].base_word = 1;
        assert_eq!(
            select_candidate_slot(slots, 0),
            CandidateSlotSelection::Fallback
        );

        slots[4].word_0c = 1;
        assert_eq!(
            select_candidate_slot(slots, 0),
            CandidateSlotSelection::Slot(4)
        );
    }
}
