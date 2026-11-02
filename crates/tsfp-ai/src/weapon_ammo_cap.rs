//! Readable ammo-at-cap predicate for the weapon firing gate `0x1B6DB0`.
//!
//! This models the typed decision only. The runtime boundary may delegate the
//! below-cap predicate to the retained `0x1B6C90` service.

/// Inputs read from the selected weapon definition, owner inventory and hand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmmoCapState {
    /// Negative means the state has no definition and skips the cap lookup.
    pub definition_index: i32,
    /// Signed ammo kind used to select 16-bit owner/hand counters.
    pub ammo_kind: u32,
    /// Definition `+0x8C` or `+0x90`, selected by the original flag argument.
    pub cap: i32,
    pub owner_rounds: i16,
    pub state_rounds: i16,
}

/// Whether the selected cap is met, otherwise asking the original fallback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmmoCapDecision {
    AtCap,
    BelowCap,
    NoDefinition,
}

pub fn ammo_cap_decision(state: AmmoCapState) -> AmmoCapDecision {
    if state.definition_index < 0 {
        return AmmoCapDecision::NoDefinition;
    }
    if i32::from(state.owner_rounds) + i32::from(state.state_rounds) >= state.cap {
        AmmoCapDecision::AtCap
    } else {
        AmmoCapDecision::BelowCap
    }
}

/// Selects ammo-definition cap +0x8c for a zero mode flag and +0x90 for any
/// nonzero mode flag, matching the shipped `0x1B6DB0` instruction path.
pub fn selected_cap(default_mode_cap: i32, alternate_mode_cap: i32, mode_flag: u32) -> i32 {
    if mode_flag == 0 {
        default_mode_cap
    } else {
        alternate_mode_cap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent translation of the REA instruction contract. This is a
    // weak reference: it is not an oracle run of the shipped body.
    fn reference(s: AmmoCapState) -> AmmoCapDecision {
        match (
            s.definition_index >= 0,
            (s.owner_rounds as i32).wrapping_add(s.state_rounds as i32) >= s.cap,
        ) {
            (false, _) => AmmoCapDecision::NoDefinition,
            (true, true) => AmmoCapDecision::AtCap,
            (true, false) => AmmoCapDecision::BelowCap,
        }
    }

    #[test]
    fn weak_differential_covers_signed_counters_cap_and_missing_definition() {
        let values = [i16::MIN, -1, 0, 1, 7, i16::MAX];
        for index in [-1, 0, 1, i32::MAX] {
            for owner in values {
                for held in values {
                    for cap in [-32769, -1, 0, 1, 32767, 32768] {
                        let s = AmmoCapState {
                            definition_index: index,
                            ammo_kind: 2,
                            cap,
                            owner_rounds: owner,
                            state_rounds: held,
                        };
                        assert_eq!(ammo_cap_decision(s), reference(s));
                    }
                }
            }
        }
    }

    #[test]
    fn mutation_check_threshold_and_definition_gate_are_observable() {
        let at = AmmoCapState {
            definition_index: 0,
            ammo_kind: 0,
            cap: 8,
            owner_rounds: 5,
            state_rounds: 3,
        };
        assert_eq!(ammo_cap_decision(at), AmmoCapDecision::AtCap);
        let below = AmmoCapState {
            state_rounds: 2,
            ..at
        };
        assert_eq!(ammo_cap_decision(below), AmmoCapDecision::BelowCap);
        let missing = AmmoCapState {
            definition_index: -1,
            ..at
        };
        assert_eq!(ammo_cap_decision(missing), AmmoCapDecision::NoDefinition);
    }

    #[test]
    fn cap_mode_selects_distinct_definition_fields() {
        // Mutation-sensitive checks: reversing the selector or treating a
        // nonzero flag as false changes both cases.
        assert_eq!(selected_cap(17, 29, 0), 17);
        assert_eq!(selected_cap(17, 29, 1), 29);
        assert_eq!(selected_cap(17, 29, u32::MAX), 29);
    }
}
