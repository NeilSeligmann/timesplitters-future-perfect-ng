//! Pointer-free nearest-position selection from the 0x15A540 AI helper.
//!
//! The original walks several parallel tables and also has mode/callback
//! branches. This module models only the repeated position comparison after
//! callers supply rows in the observed encounter order. Record ownership and
//! the meaning of the raw state word are intentionally left at the boundary.

/// Raw values used by one candidate comparison in 0x15A540.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionCandidateRaw {
    /// Index written to the helper's best-index local when this row wins.
    pub index: u32,
    /// Raw object fields at +0x64/+0x68/+0x6C.
    pub position_64_68_6c: [f32; 3],
    /// Raw row state compared with literal 0x10.
    pub state: u32,
}

/// Select the eligible row with the smallest squared distance to `reference`.
///
/// Rows with state 0x10 are skipped. Strict improvement preserves the first
/// row on ties, matching the observed `COMISS`/`JBE` branch. `None` means no
/// eligible row was supplied; the original's surrounding fallback paths are
/// not represented here.
pub fn select_nearest_position_index(
    candidates: impl IntoIterator<Item = PositionCandidateRaw>,
    reference: [f32; 3],
) -> Option<u32> {
    let mut best_index = None;
    let mut best_distance = f32::MAX;
    for candidate in candidates {
        if candidate.state == 0x10 {
            continue;
        }
        let dx = candidate.position_64_68_6c[0] - reference[0];
        let dy = candidate.position_64_68_6c[1] - reference[1];
        let dz = candidate.position_64_68_6c[2] - reference[2];
        // Preserve the filtered loop's scalar X, Y, Z accumulation order.
        // The separate 0x15A760 block uses X, Z, Y and needs its own variant.
        let distance = (dx * dx + dy * dy) + dz * dz;
        if best_distance > distance {
            best_index = Some(candidate.index);
            best_distance = distance;
        }
    }
    best_index
}

/// Return whether a candidate in the separately observed `0x15A760` scan
/// strictly beats the current best. That block accumulates X² + Z², then Y²;
/// keep it separate from the `0x15A5C7` X/Y/Z selector above. The caller owns
/// eligibility and encounter order because the bounded evidence does not
/// establish the surrounding loop's complete filter contract.
pub fn xzy_candidate_is_closer(
    candidate: [f32; 3],
    reference: [f32; 3],
    best_distance: f32,
) -> bool {
    let dx = candidate[0] - reference[0];
    let dy = candidate[1] - reference[1];
    let dz = candidate[2] - reference[2];
    let distance = dx * dx + dz * dz;
    let distance = distance + dy * dy;
    best_distance > distance
}

#[cfg(test)]
mod tests {
    use super::{PositionCandidateRaw, select_nearest_position_index, xzy_candidate_is_closer};

    fn row(index: u32, position: [f32; 3], state: u32) -> PositionCandidateRaw {
        PositionCandidateRaw {
            index,
            position_64_68_6c: position,
            state,
        }
    }

    #[test]
    fn skips_state_10_and_selects_smallest_squared_distance() {
        let rows = [
            row(4, [0.0, 0.0, 0.0], 0x10),
            row(7, [3.0, 0.0, 0.0], 0),
            row(9, [0.0, 2.0, 0.0], 1),
        ];
        assert_eq!(select_nearest_position_index(rows, [0.0; 3]), Some(9));
    }

    #[test]
    fn preserves_encounter_order_on_equal_distance() {
        let rows = [row(3, [1.0, 0.0, 0.0], 0), row(8, [-1.0, 0.0, 0.0], 0)];
        assert_eq!(select_nearest_position_index(rows, [0.0; 3]), Some(3));
    }

    #[test]
    fn no_eligible_candidates_returns_none() {
        assert_eq!(
            select_nearest_position_index([row(5, [0.0; 3], 0x10)], [0.0; 3]),
            None
        );
    }

    #[test]
    fn unordered_distance_does_not_replace_the_float_max_sentinel() {
        assert_eq!(
            select_nearest_position_index([row(6, [f32::NAN, 0.0, 0.0], 0)], [0.0; 3]),
            None
        );
    }

    #[test]
    fn uses_source_x_then_y_then_z_accumulation_order() {
        // Check a representative input remains accepted by this slice.
        let row = row(11, [2.0, 3.0, 5.0], 0);
        let reference = [1.0, 1.0, 1.0];
        let dx = 1.0_f32;
        let dy = 2.0_f32;
        let dz = 4.0_f32;
        let expected = (dx * dx + dy * dy) + dz * dz;
        let alternate = (dx * dx + dz * dz) + dy * dy;
        assert_eq!(expected, alternate);
        assert_eq!(select_nearest_position_index([row], reference), Some(11));
    }

    #[test]
    fn xzy_block_requires_strict_improvement_and_rejects_unordered_distance() {
        assert!(xzy_candidate_is_closer([1.0, 2.0, 3.0], [0.0; 3], 15.0));
        assert!(!xzy_candidate_is_closer([1.0, 2.0, 3.0], [0.0; 3], 14.0));
        assert!(!xzy_candidate_is_closer(
            [f32::NAN, 0.0, 0.0],
            [0.0; 3],
            f32::MAX
        ));
    }
}
