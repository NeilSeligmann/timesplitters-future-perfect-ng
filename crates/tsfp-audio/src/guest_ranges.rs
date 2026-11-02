//! Shared alias checks for guest-visible audio descriptors and objects.

/// Compare range endpoints in widened arithmetic, without wrapping guest addresses.
/// Strict comparisons permit adjacent ranges. This preserves the callers' existing
/// predicate even for zero lengths; callers remain responsible for validating sizes.
pub(crate) fn overlaps(a: u32, an: u32, b: u32, bn: u32) -> bool {
    u64::from(a) < u64::from(b) + u64::from(bn) && u64::from(b) < u64::from(a) + u64::from(an)
}

#[cfg(test)]
mod tests {
    use super::overlaps;

    #[test]
    fn endpoints_do_not_wrap_and_touching_ranges_are_disjoint() {
        assert!(!overlaps(0x1000, 4, 0x1004, 4));
        assert!(overlaps(0x1000, 4, 0x1003, 4));
        assert!(overlaps(u32::MAX - 1, 4, u32::MAX, 1));
        assert!(!overlaps(u32::MAX - 1, 4, 0, 4));
    }

    #[test]
    fn preserve_zero_length_predicate_and_symmetry() {
        // This is the inherited predicate, rather than general set intersection:
        // a zero-length range strictly inside another range is considered overlap.
        for (a, an, b, bn, expected) in [
            (12, 0, 10, 4, true),
            (10, 0, 10, 4, false),
            (14, 0, 10, 4, false),
            (12, 0, 12, 0, false),
        ] {
            assert_eq!(overlaps(a, an, b, bn), expected);
            assert_eq!(overlaps(b, bn, a, an), expected);
        }
    }
}
