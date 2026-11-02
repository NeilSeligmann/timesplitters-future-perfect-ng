// SPDX-License-Identifier: GPL-3.0-or-later
//! T2476/T2480/T2481/T2483: typed common Story event-condition predicates at 0x2359A0.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConditionRecord {
    pub kind: u32,
    pub args: [i32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerIdVariant {
    Character,
    Linked,
    Resource,
}

/// Host adapter for the measured player identity lookups. Implementations own access to the
/// current typed player view; this predicate retains no guest pointer or player state.
pub trait ConditionEvaluator {
    fn player_count(&self) -> usize;
    /// Match one player against the condition's four id slots. Implementations must ignore
    /// `-1` slots; retaining this rule at the adapter keeps the guest record out of the model.
    fn player_passes(&mut self, player: usize, variant: PlayerIdVariant, ids: [i32; 4]) -> bool;
    fn first_player_ext_c08_nonzero(&mut self) -> bool;
    /// Read the typed Story slot manager; absent/out-of-range slots are not finished.
    fn slot_status(&self, slot_id: u32) -> Option<u8>;
    /// Query the typed Story world's object-tag index for one tag key.
    fn tag_exists(&self, tag_key: i32) -> bool;
    /// Resolve a tag and read the object's typed child-state field. Missing tags/objects
    /// return `None`; callers do not retain object handles or guest pointers.
    fn tagged_object_child_state(&self, tag_key: i32) -> Option<u32>;
    /// Whether the indexed typed player identity belongs to the class recognized by helper
    /// 0xD6EC0. The adapter owns the class lookup and player lifetime.
    fn player_in_story_id_class(&self, _player: usize) -> bool {
        false
    }
    /// Resolve an already-adapted Story condition table key to entry field +0x18.
    /// `None` declines the Rust predicate so the retained lifted lookup remains authoritative.
    fn condition_table_field_0x18(&self, _key: i32) -> Option<u32> {
        None
    }
    /// Adapt the Story scene flag at original address `0x75BF0C`; `None` declines kind 29.
    fn scene_flag_0x75bf0c(&self) -> Option<u32> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachedPlayerFacts {
    pub count: usize,
    pub identity_matches: [[bool; 3]; 2],
    pub first_ext_c08_nonzero: bool,
    /// Slot manager snapshot indexed by slot id. `None` represents an absent slot.
    pub slot_statuses: [Option<u8>; 6],
}

impl ConditionEvaluator for CachedPlayerFacts {
    fn player_count(&self) -> usize {
        self.count
    }
    fn player_passes(&mut self, player: usize, variant: PlayerIdVariant, _: [i32; 4]) -> bool {
        self.identity_matches[player][match variant {
            PlayerIdVariant::Character => 0,
            PlayerIdVariant::Linked => 1,
            PlayerIdVariant::Resource => 2,
        }]
    }
    fn first_player_ext_c08_nonzero(&mut self) -> bool {
        self.first_ext_c08_nonzero
    }
    fn slot_status(&self, slot_id: u32) -> Option<u8> {
        self.slot_statuses.get(slot_id as usize).copied().flatten()
    }
    fn tag_exists(&self, _: i32) -> bool {
        false
    }
    fn tagged_object_child_state(&self, _: i32) -> Option<u32> {
        None
    }
}

pub const fn is_common_kind(kind: u32) -> bool {
    kind == 0
        || (kind >= 10 && kind <= 27)
        || kind == 2
        || kind == 28
        || kind == 32
        || kind == 33
        || kind == 34
        || kind == 35
        || kind == 36
        || kind == 37
        || kind == 29
}

pub fn evaluate<E: ConditionEvaluator>(record: ConditionRecord, evaluator: &mut E) -> Option<bool> {
    if record.kind == 29 {
        return crate::port_scaffolds::story_kind_29_t2489::port_step(
            &crate::port_scaffolds::story_kind_29_t2489::PortState {
                gas_tag_0x47415330_exists: evaluator.tag_exists(0x47415330),
                scene_flag_0x75bf0c: evaluator.scene_flag_0x75bf0c(),
            },
        );
    }
    if record.kind == 33 {
        return evaluator
            .condition_table_field_0x18(record.args[0])
            .map(crate::port_scaffolds::story_kind_33_table_field_predicate_t2485::table_field_0x18_is_true);
    }
    let ids = record.args;
    let count = evaluator.player_count().min(2);
    let p = |e: &mut E, player, variant| e.player_passes(player, variant, ids);
    let any = |e: &mut E, variant| (0..count).any(|i| p(e, i, variant));
    let all = |e: &mut E, variant| count != 0 && (0..count).all(|i| p(e, i, variant));
    Some(match record.kind {
        0 => true,
        10 => false,
        11 => any(evaluator, PlayerIdVariant::Character),
        12 => all(evaluator, PlayerIdVariant::Character),
        13 => !any(evaluator, PlayerIdVariant::Character),
        14 => any(evaluator, PlayerIdVariant::Linked),
        15 => any(evaluator, PlayerIdVariant::Resource),
        16 => count > 1 && p(evaluator, 1, PlayerIdVariant::Linked),
        17 => all(evaluator, PlayerIdVariant::Linked),
        18 => all(evaluator, PlayerIdVariant::Resource),
        19 => (0..count).any(|i| !p(evaluator, i, PlayerIdVariant::Resource)),
        20 => (0..count).any(|i| !p(evaluator, i, PlayerIdVariant::Linked)),
        21 => !any(evaluator, PlayerIdVariant::Linked),
        22 => !any(evaluator, PlayerIdVariant::Resource),
        23 => count > 0 && p(evaluator, 0, PlayerIdVariant::Character),
        24 => count > 0 && p(evaluator, 0, PlayerIdVariant::Linked),
        25 => count > 0 && !p(evaluator, 0, PlayerIdVariant::Linked),
        26 => count > 0 && p(evaluator, 0, PlayerIdVariant::Resource),
        27 => count > 0 && evaluator.first_player_ext_c08_nonzero(),
        2 => evaluator.slot_status(5) == Some(5),
        28 => evaluator.slot_status(2) == Some(5),
        32 => record.args[0] >= 0 && evaluator.slot_status(record.args[0] as u32) == Some(5),
        34 => !evaluator.tag_exists(record.args[0]),
        35 => evaluator.tag_exists(record.args[0]),
        36 => matches!(
            evaluator.tagged_object_child_state(record.args[0]),
            Some(2 | 3 | 5 | 7)
        ),
        37 => {
            (0..count)
                .filter(|&i| evaluator.player_in_story_id_class(i))
                .count()
                < 2
        }
        _ => return None,
    })
}

/// A/B entry point for callers that have already adapted guest state to typed facts. The off
/// path deliberately declines the value so the caller can invoke the retained lifted evaluator.
pub fn evaluate_selected<E: ConditionEvaluator>(
    record: ConditionRecord,
    evaluator: &mut E,
) -> Option<bool> {
    enabled().then(|| evaluate(record, evaluator)).flatten()
}

pub fn enabled() -> bool {
    #[cfg(not(test))]
    unsafe {
        unsafe extern "C" {
            fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
        }
        let value = getenv(c"TSFP_STORY_EVENT_CONDITION".as_ptr());
        !value.is_null() && core::ffi::CStr::from_ptr(value).to_bytes() == b"rust"
    }
    #[cfg(test)]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Inputs {
        values: [[bool; 3]; 2],
        ext: bool,
    }
    impl ConditionEvaluator for Inputs {
        fn player_count(&self) -> usize {
            2
        }
        fn player_passes(
            &mut self,
            player: usize,
            variant: PlayerIdVariant,
            ids: [i32; 4],
        ) -> bool {
            assert!(ids.iter().any(|id| *id == -1));
            self.values[player][match variant {
                PlayerIdVariant::Character => 0,
                PlayerIdVariant::Linked => 1,
                PlayerIdVariant::Resource => 2,
            }]
        }
        fn first_player_ext_c08_nonzero(&mut self) -> bool {
            self.ext
        }
        fn slot_status(&self, slot_id: u32) -> Option<u8> {
            match slot_id {
                2 => Some(2),
                5 => Some(5),
                _ => None,
            }
        }
        fn tag_exists(&self, _: i32) -> bool {
            false
        }
        fn tagged_object_child_state(&self, _: i32) -> Option<u32> {
            None
        }
    }

    fn lifted(kind: u32, x: Inputs) -> Option<bool> {
        let [a0, a1, a2] = x.values[0];
        let [b0, b1, b2] = x.values[1];
        Some(match kind {
            0 => true,
            10 => false,
            11 => a0 || b0,
            12 => a0 && b0,
            13 => !a0 && !b0,
            14 => a1 || b1,
            15 => a2 || b2,
            16 => b1,
            17 => a1 && b1,
            18 => a2 && b2,
            19 => !a2 || !b2,
            20 => !a1 || !b1,
            21 => !a1 && !b1,
            22 => !a2 && !b2,
            23 => a0,
            24 => a1,
            25 => !a1,
            26 => a2,
            27 => x.ext,
            _ => return None,
        })
    }

    #[derive(Clone, Copy)]
    struct SlotFacts([Option<u8>; 8]);
    impl ConditionEvaluator for SlotFacts {
        fn player_count(&self) -> usize {
            0
        }
        fn player_passes(&mut self, _: usize, _: PlayerIdVariant, _: [i32; 4]) -> bool {
            false
        }
        fn first_player_ext_c08_nonzero(&mut self) -> bool {
            false
        }
        fn slot_status(&self, slot_id: u32) -> Option<u8> {
            self.0.get(slot_id as usize).copied().flatten()
        }
        fn tag_exists(&self, _: i32) -> bool {
            false
        }
        fn tagged_object_child_state(&self, _: i32) -> Option<u32> {
            None
        }
    }

    #[derive(Clone, Copy)]
    struct TagFacts([i32; 4], usize);
    impl ConditionEvaluator for TagFacts {
        fn player_count(&self) -> usize {
            0
        }
        fn player_passes(&mut self, _: usize, _: PlayerIdVariant, _: [i32; 4]) -> bool {
            false
        }
        fn first_player_ext_c08_nonzero(&mut self) -> bool {
            false
        }
        fn slot_status(&self, _: u32) -> Option<u8> {
            None
        }
        fn tag_exists(&self, key: i32) -> bool {
            self.0[..self.1].contains(&key)
        }
        fn tagged_object_child_state(&self, _: i32) -> Option<u32> {
            None
        }
    }

    fn lifted_tag(kind: u32, args: [i32; 4], tags: &[i32]) -> Option<bool> {
        let exists = tags.contains(&args[0]);
        match kind {
            34 => Some(!exists),
            35 => Some(exists),
            _ => None,
        }
    }

    #[derive(Clone, Copy)]
    struct ChildFacts {
        key: i32,
        state: Option<u32>,
    }
    impl ConditionEvaluator for ChildFacts {
        fn player_count(&self) -> usize {
            0
        }
        fn player_passes(&mut self, _: usize, _: PlayerIdVariant, _: [i32; 4]) -> bool {
            false
        }
        fn first_player_ext_c08_nonzero(&mut self) -> bool {
            false
        }
        fn slot_status(&self, _: u32) -> Option<u8> {
            None
        }
        fn tag_exists(&self, key: i32) -> bool {
            key == self.key && self.state.is_some()
        }
        fn tagged_object_child_state(&self, key: i32) -> Option<u32> {
            (key == self.key).then_some(self.state).flatten()
        }
    }

    fn lifted_child_state(state: Option<u32>) -> bool {
        matches!(state, Some(2 | 3 | 5 | 7))
    }

    #[test]
    fn tagged_child_state_matches_lifted_contract_and_detects_mutation() {
        let mut seed = 0x2482_1889u32;
        for case in 0..3000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let key = ((seed >> 8) % 31) as i32 - 15;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let state = if case % 9 == 0 {
                None
            } else {
                Some((seed >> 16) % 10)
            };
            let queried_key = if case % 7 == 0 {
                key.wrapping_add(1)
            } else {
                key
            };
            let record = ConditionRecord {
                kind: 36,
                args: [queried_key, 0, 0, 0],
            };
            let expected_state = (queried_key == key).then_some(state).flatten();
            assert_eq!(
                evaluate(record, &mut ChildFacts { key, state }),
                Some(lifted_child_state(expected_state)),
                "case {case}"
            );
        }
        let record = ConditionRecord {
            kind: 36,
            args: [7, 0, 0, 0],
        };
        let accepted = evaluate(
            record,
            &mut ChildFacts {
                key: 7,
                state: Some(2),
            },
        );
        let mutated = evaluate(
            record,
            &mut ChildFacts {
                key: 7,
                state: Some(4),
            },
        );
        assert_eq!(accepted, Some(true));
        assert_eq!(mutated, Some(false));
        assert_ne!(accepted, mutated);
        assert_eq!(
            evaluate(
                ConditionRecord {
                    kind: 99,
                    args: [0; 4]
                },
                &mut ChildFacts {
                    key: 0,
                    state: None
                }
            ),
            None
        );
    }

    #[test]
    fn tag_existence_predicates_match_lifted_expressions_and_detect_mutation() {
        let mut seed = 0x2481_2359u32;
        for case in 0..3000 {
            let mut tags = [0i32; 4];
            let count = case % 5;
            for tag in &mut tags {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *tag = ((seed >> 8) % 17) as i32 - 8;
            }
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let key = ((seed >> 8) % 21) as i32 - 10;
            let kind = if case % 2 == 0 { 34 } else { 35 };
            let record = ConditionRecord {
                kind,
                args: [key, 0, 0, 0],
            };
            assert_eq!(
                evaluate(record, &mut TagFacts(tags, count)),
                lifted_tag(kind, record.args, &tags[..count]),
                "case {case}"
            );
        }
        let record = ConditionRecord {
            kind: 35,
            args: [7, 0, 0, 0],
        };
        let present = evaluate(record, &mut TagFacts([7, 0, 0, 0], 1));
        let mutated = evaluate(record, &mut TagFacts([0, 0, 0, 0], 0));
        assert_eq!(present, Some(true));
        assert_eq!(mutated, Some(false));
        assert_ne!(present, mutated);
        assert_eq!(
            evaluate(
                ConditionRecord {
                    kind: 38,
                    args: [7, 0, 0, 0]
                },
                &mut TagFacts([7, 0, 0, 0], 1)
            ),
            None
        );
    }

    fn lifted_slot(kind: u32, args: [i32; 4], slots: [Option<u8>; 8]) -> Option<bool> {
        let finished = |id: i32| id >= 0 && slots.get(id as usize).copied().flatten() == Some(5);
        match kind {
            2 => Some(finished(5)),
            28 => Some(finished(2)),
            32 => Some(finished(args[0])),
            _ => None,
        }
    }

    #[test]
    fn slot_status_predicates_match_lifted_expressions_and_detect_mutation() {
        let mut seed = 0x2480_2359u32;
        for case in 0..3000 {
            let mut slots = [None; 8];
            for slot in &mut slots {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *slot = match seed % 5 {
                    0 => None,
                    1 => Some(0),
                    2 => Some(2),
                    3 => Some(4),
                    _ => Some(5),
                };
            }
            let kind = [2, 28, 32][case % 3];
            let selector = ((seed >> 8) as i32 % 10) - 1;
            let record = ConditionRecord {
                kind,
                args: [selector, 0, 0, 0],
            };
            assert_eq!(
                evaluate(record, &mut SlotFacts(slots)),
                lifted_slot(kind, record.args, slots),
                "case {case}"
            );
        }
        let mut slots = [None; 8];
        slots[5] = Some(5);
        let record = ConditionRecord {
            kind: 2,
            args: [0; 4],
        };
        let original = evaluate(record, &mut SlotFacts(slots));
        let mut mutated = slots;
        mutated[5] = Some(4);
        let changed = evaluate(record, &mut SlotFacts(mutated));
        assert_eq!(original, lifted_slot(2, record.args, slots));
        assert_eq!(changed, Some(false));
        assert_ne!(original, changed);
        assert_eq!(
            evaluate(
                ConditionRecord {
                    kind: 32,
                    args: [-1, 0, 0, 0]
                },
                &mut SlotFacts(slots)
            ),
            Some(false)
        );
    }

    #[test]
    fn tier1_fabricated_differential_and_mutation() {
        let mut seed = 0x2476_2359u32;
        for n in 0..3000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = Inputs {
                values: [
                    [seed & 1 != 0, seed & 2 != 0, seed & 4 != 0],
                    [seed & 8 != 0, seed & 16 != 0, seed & 32 != 0],
                ],
                ext: seed & 64 != 0,
            };
            let kind = 10 + (n % 18);
            let record = ConditionRecord {
                kind,
                args: [-1, 2, 3, 4],
            };
            assert_eq!(
                evaluate(record, &mut x.clone()),
                lifted(kind, x),
                "case {n}"
            );
            if n == 0 {
                assert_ne!(
                    evaluate(record, &mut x.clone()),
                    Some(!lifted(kind, x).unwrap())
                );
            }
        }
        assert_eq!(
            evaluate(
                ConditionRecord {
                    kind: 38,
                    args: [0; 4]
                },
                &mut Inputs {
                    values: [[false; 3]; 2],
                    ext: false
                }
            ),
            None
        );
        assert_eq!(
            evaluate(
                ConditionRecord {
                    kind: 27,
                    args: [0; 4]
                },
                &mut CachedPlayerFacts {
                    count: 0,
                    identity_matches: [[false; 3]; 2],
                    first_ext_c08_nonzero: true,
                    slot_statuses: [None; 6],
                }
            ),
            Some(false)
        );
    }

    #[derive(Clone, Copy)]
    struct ClassFacts {
        matches: [bool; 2],
        count: usize,
    }
    impl ConditionEvaluator for ClassFacts {
        fn player_count(&self) -> usize {
            self.count
        }
        fn player_passes(&mut self, _: usize, _: PlayerIdVariant, _: [i32; 4]) -> bool {
            false
        }
        fn first_player_ext_c08_nonzero(&mut self) -> bool {
            false
        }
        fn slot_status(&self, _: u32) -> Option<u8> {
            None
        }
        fn tag_exists(&self, _: i32) -> bool {
            false
        }
        fn tagged_object_child_state(&self, _: i32) -> Option<u32> {
            None
        }
        fn player_in_story_id_class(&self, player: usize) -> bool {
            self.matches[player]
        }
    }

    fn lifted_fewer_than_two(matches: &[bool]) -> bool {
        matches.iter().filter(|&&matches| matches).take(2).count() < 2
    }

    #[test]
    fn player_class_count_matches_lifted_predicate_and_detects_mutation() {
        let mut seed = 0x2483_d6ecu32;
        for case in 0..3000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let facts = ClassFacts {
                matches: [seed & 1 != 0, seed & 2 != 0],
                count: (seed as usize >> 3) % 3,
            };
            let record = ConditionRecord {
                kind: 37,
                args: [0; 4],
            };
            let expected = lifted_fewer_than_two(&facts.matches[..facts.count.min(2)]);
            assert_eq!(
                evaluate(record, &mut { facts }),
                Some(expected),
                "case {case}"
            );
        }
        let record = ConditionRecord {
            kind: 37,
            args: [0; 4],
        };
        let one = ClassFacts {
            matches: [true, false],
            count: 2,
        };
        let two = ClassFacts {
            matches: [true, true],
            count: 2,
        };
        assert_eq!(evaluate(record, &mut { one }), Some(true));
        assert_eq!(evaluate(record, &mut { two }), Some(false));
        // Mutation: suppressing the second successful helper result flips the lifted answer.
        let two_matches = [true, true];
        let mutated_matches = [true, false];
        assert_eq!(lifted_fewer_than_two(&two_matches), false);
        assert_eq!(lifted_fewer_than_two(&mutated_matches), true);
    }
}
