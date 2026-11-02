//! Pending weapon-state transition predicate (`0x1B63D0`).

/// Reports whether the weapon is currently in `target` while its pending state
/// still names a different state. This means the requested transition has not
/// yet settled at the target state.
pub const fn is_pending(current: u32, pending: u32, target: u32) -> bool {
    current == target && pending != target
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent transliteration of the measured original instruction
    // contract, used as the Tier 1 reference rather than a lifted execution.
    fn reference(current: u32, pending: u32, target: u32) -> u32 {
        if current == target {
            if pending == target { 0 } else { 1 }
        } else {
            0
        }
    }

    #[test]
    fn tier1_weak_differential_against_instruction_contract() {
        let mut seed = 0x1b63_d024u32;
        for _ in 0..4_000 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let current = seed;
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let pending = seed;
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let target = if seed & 1 == 0 {
                current
            } else {
                seed.rotate_left(11)
            };
            assert_eq!(
                u32::from(is_pending(current, pending, target)),
                reference(current, pending, target)
            );
        }
    }

    #[test]
    fn mutation_checks_cover_each_comparison() {
        assert!(is_pending(0x21, 0x14, 0x21));
        assert!(!is_pending(0x14, 0x21, 0x21));
        assert!(!is_pending(0x21, 0x21, 0x21));
        assert!(is_pending(0x14, 0x22, 0x14));
        assert!(!is_pending(0x22, 0x14, 0x14));
        assert!(!is_pending(0, u32::MAX, 0x22));
    }
}
