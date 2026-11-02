//! Readiness gate for weapon object state and pending transitions (0x1B9A30).

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponState {
    pub current: u32,
    pub pending: u32,
}

/// The exact transition probes the original performs, in call order.
pub const TRANSITION_TARGETS: [u32; 3] = [0x21, 0x14, 0x22];

/// State equality gate, followed by the three original transition queries.
/// A true transition query blocks readiness immediately.
pub fn is_ready(state: WeaponState, transition_pending: [bool; 3]) -> bool {
    let stable = match state.current {
        9 | 14 | 19 => state.pending == state.current,
        18 => state.pending == 18,
        _ => true,
    };
    stable && !transition_pending.into_iter().any(|pending| pending)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(s: WeaponState, probes: [bool; 3]) -> bool {
        let stable = if s.current == 9 || s.current == 14 || s.current == 19 {
            s.pending == s.current
        } else if s.current == 18 {
            s.pending == 18
        } else {
            true
        };
        if !stable {
            return false;
        }
        for probe in probes {
            if probe {
                return false;
            }
        }
        true
    }

    #[test]
    fn tier1_weak_differential_against_independent_contract_transliteration() {
        let mut seed = 0x1b9a_3024u32;
        for _ in 0..4000 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let current = match seed & 7 {
                0 => 9,
                1 => 14,
                2 => 19,
                3 => 18,
                _ => seed >> 8,
            };
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let pending = match seed & 3 {
                0 => current,
                1 => 18,
                2 => current.wrapping_add(1),
                _ => seed >> 12,
            };
            let probes = [seed & 1 != 0, seed & 2 != 0, seed & 4 != 0];
            let state = WeaponState { current, pending };
            assert_eq!(is_ready(state, probes), reference(state, probes));
        }
    }

    #[test]
    fn mutation_checks_cover_state_rules_and_all_transition_probes() {
        assert!(!is_ready(
            WeaponState {
                current: 9,
                pending: 8
            },
            [false; 3]
        ));
        assert!(!is_ready(
            WeaponState {
                current: 18,
                pending: 19
            },
            [false; 3]
        ));
        assert!(is_ready(
            WeaponState {
                current: 3,
                pending: 99
            },
            [false; 3]
        ));
        for i in 0..3 {
            let mut probes = [false; 3];
            probes[i] = true;
            assert!(!is_ready(
                WeaponState {
                    current: 1,
                    pending: 2
                },
                probes
            ));
        }
        assert!(is_ready(
            WeaponState {
                current: 9,
                pending: 9
            },
            [false; 3]
        ));
        assert!(is_ready(
            WeaponState {
                current: 18,
                pending: 18
            },
            [false; 3]
        ));
        assert!(is_ready(
            WeaponState {
                current: 2,
                pending: 77
            },
            [false; 3]
        ));
    }
}
