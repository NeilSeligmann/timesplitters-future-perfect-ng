// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed evaluation of the measured profile unlock requirement cases (T3420).
//!
//! The provider owns the selected profile's requirement words and record-38
//! views. This module does not resolve which original Q block supplies those
//! words; that source/slot authority remains an explicit attachment decision.
//! Missing typed data fails closed. The behavior below reuses the bounded
//! static cases in `docs/t-save-layout.md` and `docs/t1666-rea-profile.md`.

/// Owned projection of one record-38 descriptor's tag and required bit IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record38Requirements<'a> {
    pub tag: u32,
    /// Empty means the original signed count was non-positive, which accepts.
    pub bit_ids: &'a [i32],
}

/// Typed source for the words and descriptor rows consumed by the checker.
/// `difficulty_word` and `summary_word` index one owned Q view; the provider
/// decides whether that view is slot-local or aggregate. `selector_fallback`
/// receives the already-computed word index from the measured H formula.
pub trait UnlockRequirementData {
    fn has_active_profile(&self) -> bool;
    fn difficulty_word(&self, difficulty: i32, word_index: i32) -> Option<u32>;
    fn summary_word(&self, word_index: i32) -> Option<u32>;
    fn selector_fallback_word(&self, word_index: i32) -> Option<u32>;
    fn record38(&self, index: u32) -> Option<Record38Requirements<'_>>;
}

/// Decoded, owned-data form of the five-dword descriptor's measured cases.
/// Type 2 and type 3 both use `SelectorBit`; their decoded bit IDs differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnlockRequirement<'a> {
    /// Original type 0: always false, including under unlock-all.
    Disabled,
    /// Original type 1: true after the active-profile gate.
    Always,
    /// Original types 2/3: H(bit_index, word_index, 0, selector).
    SelectorBit {
        bit_index: i32,
        word_index: i32,
        selector: i32,
    },
    /// Original type 4: all listed bits for one record-38 row.
    RecordGroup { record_index: u32, difficulty: i32 },
    /// Original type 5: one bit in a difficulty block.
    DifficultyBit { bit_index: i32, difficulty: i32 },
    /// Original type 6: at least `minimum` matching rows among records 0..7.
    MinimumGroups { minimum: i32, difficulty: i32 },
    /// Original type 7: every listed record; `None` and an empty list accept.
    AllGroups {
        record_indices: Option<&'a [u32]>,
        difficulty: i32,
    },
    /// Original type 8: every matching tag among records 7..16.
    MatchingTaggedGroups { tag: u32, difficulty: i32 },
    /// An unrecognized nonzero original type. Unlock-all accepts it.
    Unsupported(u32),
}

impl UnlockRequirement<'_> {
    const fn original_type(self) -> u32 {
        match self {
            Self::Disabled => 0,
            Self::Always => 1,
            Self::SelectorBit { .. } => 2,
            Self::RecordGroup { .. } => 4,
            Self::DifficultyBit { .. } => 5,
            Self::MinimumGroups { .. } => 6,
            Self::AllGroups { .. } => 7,
            Self::MatchingTaggedGroups { .. } => 8,
            Self::Unsupported(kind) => kind,
        }
    }
}

/// Default-off opt-in for the typed common requirement evaluator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UnlockRequirementPolicy {
    enabled: bool,
}

impl UnlockRequirementPolicy {
    pub const fn new() -> Self {
        Self { enabled: false }
    }

    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Returns `None` while disabled and a decision when explicitly enabled.
    pub fn evaluate(
        self,
        data: &impl UnlockRequirementData,
        unlock_all: bool,
        requirement: UnlockRequirement<'_>,
    ) -> Option<bool> {
        self.enabled
            .then(|| requirement_met(data, unlock_all, requirement))
    }
}

/// Evaluate one decoded requirement after the original active-profile gate.
/// The type-2/type-3 descriptor conversion and wrapper-specific overrides live
/// at their caller; this function models the common 0x1CD3E0 switch only.
fn requirement_met(
    data: &impl UnlockRequirementData,
    unlock_all: bool,
    requirement: UnlockRequirement<'_>,
) -> bool {
    if !data.has_active_profile() {
        return false;
    }
    if requirement.original_type() != 0 && unlock_all {
        return true;
    }

    match requirement {
        UnlockRequirement::Disabled | UnlockRequirement::Unsupported(_) => false,
        UnlockRequirement::Always => true,
        UnlockRequirement::SelectorBit {
            bit_index,
            word_index,
            selector,
        } => selector_bit_met(data, bit_index, word_index, selector),
        UnlockRequirement::RecordGroup {
            record_index,
            difficulty,
        } => record_group_met(data, record_index, difficulty),
        UnlockRequirement::DifficultyBit {
            bit_index,
            difficulty,
        } => difficulty_bit_met(data, bit_index, difficulty),
        UnlockRequirement::MinimumGroups {
            minimum,
            difficulty,
        } => {
            let mut matched = 0i32;
            for record_index in 0..7 {
                if record_group_met(data, record_index, difficulty) {
                    matched += 1;
                }
            }
            matched >= minimum
        }
        UnlockRequirement::AllGroups {
            record_indices,
            difficulty,
        } => record_indices.is_none_or(|indices| {
            indices
                .iter()
                .all(|&record| record_group_met(data, record, difficulty))
        }),
        UnlockRequirement::MatchingTaggedGroups { tag, difficulty } => {
            for record_index in 7..16 {
                let Some(record) = data.record38(record_index) else {
                    return false;
                };
                if record.tag == tag && !record_group_met(data, record_index, difficulty) {
                    return false;
                }
            }
            true
        }
    }
}

fn difficulty_bit_met(data: &impl UnlockRequirementData, bit_index: i32, difficulty: i32) -> bool {
    let word_index = bit_index >> 5;
    let mask = 1u32 << (bit_index as u32 & 31);
    if difficulty == -1 {
        (1..=4).any(|selected| {
            data.difficulty_word(selected, word_index)
                .is_some_and(|word| word & mask != 0)
        })
    } else {
        data.difficulty_word(difficulty, word_index)
            .is_some_and(|word| word & mask != 0)
    }
}

fn selector_bit_met(
    data: &impl UnlockRequirementData,
    bit_index: i32,
    word_index: i32,
    selector: i32,
) -> bool {
    let mask = 1u32 << (bit_index as u32 & 31);
    if data
        .summary_word(word_index)
        .is_some_and(|word| word & mask != 0)
    {
        return true;
    }
    let fallback_index = word_index.wrapping_add(selector.wrapping_add(255).wrapping_mul(3));
    data.selector_fallback_word(fallback_index)
        .is_some_and(|word| word & mask != 0)
}

fn record_group_met(data: &impl UnlockRequirementData, record_index: u32, difficulty: i32) -> bool {
    data.record38(record_index).is_some_and(|record| {
        let mut met = true;
        for &bit in record.bit_ids {
            // The measured helper visits every bit even after a miss.
            met &= difficulty_bit_met(data, bit, difficulty);
        }
        met
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use core::cell::Cell;

    #[derive(Default)]
    struct FakeRecord {
        tag: u32,
        bits: Vec<i32>,
    }

    #[derive(Default)]
    struct FakeData {
        active: bool,
        difficulty_reads: Cell<usize>,
        difficulty_words: Vec<(i32, i32, u32)>,
        summary_words: Vec<(i32, u32)>,
        fallback_words: Vec<(i32, u32)>,
        records: Vec<FakeRecord>,
    }

    impl UnlockRequirementData for FakeData {
        fn has_active_profile(&self) -> bool {
            self.active
        }

        fn difficulty_word(&self, difficulty: i32, word_index: i32) -> Option<u32> {
            self.difficulty_reads
                .set(self.difficulty_reads.get().wrapping_add(1));
            self.difficulty_words
                .iter()
                .find(|&&(d, w, _)| d == difficulty && w == word_index)
                .map(|&(_, _, word)| word)
        }

        fn summary_word(&self, word_index: i32) -> Option<u32> {
            self.summary_words
                .iter()
                .find(|&&(index, _)| index == word_index)
                .map(|&(_, word)| word)
        }

        fn selector_fallback_word(&self, word_index: i32) -> Option<u32> {
            self.fallback_words
                .iter()
                .find(|&&(index, _)| index == word_index)
                .map(|&(_, word)| word)
        }

        fn record38(&self, index: u32) -> Option<Record38Requirements<'_>> {
            self.records
                .get(index as usize)
                .map(|record| Record38Requirements {
                    tag: record.tag,
                    bit_ids: &record.bits,
                })
        }
    }

    fn active() -> FakeData {
        FakeData {
            active: true,
            ..FakeData::default()
        }
    }

    fn evaluate(
        data: &impl UnlockRequirementData,
        unlock_all: bool,
        requirement: UnlockRequirement<'_>,
    ) -> bool {
        let mut policy = UnlockRequirementPolicy::new();
        policy.set_enabled(true);
        policy
            .evaluate(data, unlock_all, requirement)
            .expect("test explicitly enables the policy")
    }

    #[test]
    fn policy_is_default_off_and_can_be_enabled_explicitly() {
        let data = active();
        let mut policy = UnlockRequirementPolicy::default();
        assert!(!policy.is_enabled());
        assert_eq!(
            policy.evaluate(&data, false, UnlockRequirement::Always),
            None
        );

        policy.set_enabled(true);
        assert_eq!(
            policy.evaluate(&data, false, UnlockRequirement::Always),
            Some(true)
        );
    }

    fn set_bit(data: &mut FakeData, difficulty: i32, bit: i32) {
        let word_index = bit >> 5;
        let mask = 1u32 << (bit as u32 & 31);
        if let Some((_, _, word)) = data
            .difficulty_words
            .iter_mut()
            .find(|entry| entry.0 == difficulty && entry.1 == word_index)
        {
            *word |= mask;
        } else {
            data.difficulty_words.push((difficulty, word_index, mask));
        }
    }

    #[test]
    fn active_profile_and_unlock_all_precede_requirement_cases() {
        let inactive = FakeData::default();
        assert!(!evaluate(&inactive, true, UnlockRequirement::Always));

        let data = active();
        assert!(!evaluate(&data, true, UnlockRequirement::Disabled));
        assert!(evaluate(&data, true, UnlockRequirement::Unsupported(9)));
        assert!(!evaluate(&data, false, UnlockRequirement::Unsupported(9)));
    }

    #[test]
    fn difficulty_bit_uses_selected_block_or_any_of_one_through_four() {
        let mut data = active();
        set_bit(&mut data, 2, 35);
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::DifficultyBit {
                bit_index: 35,
                difficulty: 2,
            }
        ));
        assert!(!evaluate(
            &data,
            false,
            UnlockRequirement::DifficultyBit {
                bit_index: 35,
                difficulty: 1,
            }
        ));

        set_bit(&mut data, 4, 35);
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::DifficultyBit {
                bit_index: 35,
                difficulty: -1,
            }
        ));
    }

    #[test]
    fn selector_bit_checks_summary_then_the_measured_fallback_word() {
        let mut data = active();
        data.fallback_words.push((770, 1 << 1));
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::SelectorBit {
                bit_index: 33,
                word_index: 2,
                selector: 1,
            }
        ));

        data.summary_words.push((2, 1 << 1));
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::SelectorBit {
                bit_index: 33,
                word_index: 2,
                selector: 1,
            }
        ));
    }

    #[test]
    fn record_groups_and_signed_minimum_count_follow_measured_ranges() {
        let mut data = active();
        for bit in 0..7 {
            data.records.push(FakeRecord {
                tag: 0,
                bits: alloc::vec![bit],
            });
        }
        set_bit(&mut data, 3, 0);
        set_bit(&mut data, 3, 1);
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::RecordGroup {
                record_index: 0,
                difficulty: 3,
            }
        ));
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::MinimumGroups {
                minimum: 2,
                difficulty: 3,
            }
        ));
        assert!(!evaluate(
            &data,
            false,
            UnlockRequirement::MinimumGroups {
                minimum: 3,
                difficulty: 3,
            }
        ));
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::MinimumGroups {
                minimum: 0,
                difficulty: 3,
            }
        ));
    }

    #[test]
    fn record_group_visits_every_bit_after_a_missing_requirement() {
        let mut data = active();
        data.records.push(FakeRecord {
            tag: 0,
            bits: alloc::vec![0, 1, 2],
        });
        set_bit(&mut data, 1, 1);

        assert!(!evaluate(
            &data,
            false,
            UnlockRequirement::RecordGroup {
                record_index: 0,
                difficulty: 1,
            }
        ));
        assert_eq!(data.difficulty_reads.get(), 3);
    }

    #[test]
    fn all_groups_accepts_null_or_empty_and_requires_each_supplied_row() {
        let mut data = active();
        data.records.extend([
            FakeRecord {
                tag: 0,
                bits: alloc::vec![0],
            },
            FakeRecord {
                tag: 0,
                bits: alloc::vec![1],
            },
            FakeRecord {
                tag: 0,
                bits: alloc::vec![2],
            },
        ]);
        set_bit(&mut data, 1, 0);
        set_bit(&mut data, 1, 1);
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::AllGroups {
                record_indices: None,
                difficulty: 1,
            }
        ));
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::AllGroups {
                record_indices: Some(&[]),
                difficulty: 1,
            }
        ));
        assert!(!evaluate(
            &data,
            false,
            UnlockRequirement::AllGroups {
                record_indices: Some(&[0, 2]),
                difficulty: 1,
            }
        ));
    }

    #[test]
    fn tagged_groups_check_only_matching_records_in_seven_through_fifteen() {
        let mut data = active();
        data.records.resize_with(16, FakeRecord::default);
        data.records[7] = FakeRecord {
            tag: 4,
            bits: alloc::vec![0],
        };
        data.records[8] = FakeRecord {
            tag: 5,
            bits: alloc::vec![2],
        };
        data.records[9] = FakeRecord {
            tag: 4,
            bits: alloc::vec![1],
        };
        set_bit(&mut data, 1, 0);
        set_bit(&mut data, 1, 1);
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::MatchingTaggedGroups {
                tag: 4,
                difficulty: 1,
            }
        ));
        assert!(evaluate(
            &data,
            false,
            UnlockRequirement::MatchingTaggedGroups {
                tag: 99,
                difficulty: 1,
            }
        ));

        data.records[9].bits[0] = 2;
        assert!(!evaluate(
            &data,
            false,
            UnlockRequirement::MatchingTaggedGroups {
                tag: 4,
                difficulty: 1,
            }
        ));
    }
}
