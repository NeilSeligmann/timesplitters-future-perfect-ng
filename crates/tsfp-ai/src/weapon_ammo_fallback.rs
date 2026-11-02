//! Readable typed contract for the below-cap fallback `0x1B6C90`.

/// Snapshot of the definition, mode row and two signed ammo counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmmoFallbackFacts {
    pub definition_index: i32,
    /// First dword of the selected mode row (`< 0` selects the definition row).
    pub mode_row_marker: i32,
    /// Limit selected from the mode row at `+0x24` or `+0x28`.
    pub mode_limit: i32,
    /// Definition fallback limit at `+0x68`.
    pub definition_limit: i32,
    pub owner_rounds: i16,
    pub state_rounds: i16,
}

/// Whether the source's signed sum reaches the selected mode/definition limit.
pub fn ammo_total_covers_cost(facts: AmmoFallbackFacts) -> bool {
    if facts.definition_index < 0 {
        return false;
    }
    let limit = if facts.mode_row_marker >= 0 {
        facts.mode_limit
    } else {
        facts.definition_limit
    };
    i32::from(facts.owner_rounds) + i32::from(facts.state_rounds) >= limit
}

/// Resolve the ammo kind used by the original mode-1 special case.
pub fn selected_ammo_kind(mode: u32, definition_kind: u32, alternate_kind: u32) -> u32 {
    if mode == 1 && alternate_kind != 0 {
        alternate_kind
    } else {
        definition_kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_one_alternate_kind_is_exact_and_requires_nonzero() {
        assert_eq!(selected_ammo_kind(1, 3, 7), 7);
        assert_eq!(selected_ammo_kind(0, 3, 7), 3);
        assert_eq!(selected_ammo_kind(2, 3, 7), 3);
        assert_eq!(selected_ammo_kind(1, 3, 0), 3);
    }

    #[test]
    fn negative_mode_marker_uses_definition_limit_and_signed_sum() {
        let base = AmmoFallbackFacts {
            definition_index: 0,
            mode_row_marker: -1,
            mode_limit: 90,
            definition_limit: 4,
            owner_rounds: 5,
            state_rounds: -1,
        };
        assert!(ammo_total_covers_cost(base));
        assert!(!ammo_total_covers_cost(AmmoFallbackFacts {
            definition_limit: 5,
            ..base
        }));
        assert!(!ammo_total_covers_cost(AmmoFallbackFacts {
            definition_index: -1,
            ..base
        }));
    }
}
