// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed nearest-qualifying candidate selection from 0x18AF40.
//!
//! REA static evidence measures the filter/order: positive dot product,
//! `0xD1C90` false, and strictly smaller squared distance; equal distances keep
//! the first row. The meaning of the vectors and the owner/lifetime of rows are
//! still unresolved, so this module accepts precomputed metrics and is not yet
//! connected to the live actor/effect path.

/// The already-computed values consumed by the 0x18AF40 candidate ranking.
///
/// `alignment_dot` and `squared_distance` deliberately avoid assigning names
/// to the underlying vector spaces while their original record types remain
/// unknown. `field_114` is kept raw because the meanings of its two rejected
/// values are not known.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttachmentCandidateMetrics {
    pub row_index: usize,
    pub squared_distance: f32,
    pub alignment_dot: f32,
    pub field_114: u32,
}

/// Raw candidate values consumed by the 0x18AF40 ranking loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectorCandidateRaw {
    pub row_index: usize,
    /// Original row floats at +0xA8/+0xAC/+0xB0.
    pub vector_a8_ac_b0: [f32; 3],
    /// Original row floats at +0xB4/+0xB8/+0xBC.
    pub vector_b4_b8_bc: [f32; 3],
    pub field_114: u32,
}

/// Evaluate the measured distance and score terms for one candidate row.
///
/// `reference` and `score_reference` are caller-built vectors. The original scalar
/// SSE accumulation order is retained; labels and coordinate-space meanings remain
/// unassigned.
pub fn measure_selector_candidate(
    row: SelectorCandidateRaw,
    reference: [f32; 3],
    score_reference: [f32; 3],
    score_weights: [f32; 3],
) -> AttachmentCandidateMetrics {
    let delta_0 = row.vector_a8_ac_b0[0] - reference[0];
    let delta_1 = row.vector_a8_ac_b0[1] - reference[1];
    let delta_2 = row.vector_a8_ac_b0[2] - reference[2];
    let mut squared_distance = delta_0 * delta_0;
    squared_distance += delta_1 * delta_1;
    squared_distance += delta_2 * delta_2;

    let term_2 = (row.vector_b4_b8_bc[2] - score_reference[2]) * score_weights[2];
    let term_1 = (row.vector_b4_b8_bc[1] - score_reference[1]) * score_weights[1];
    let term_0 = (row.vector_b4_b8_bc[0] - score_reference[0]) * score_weights[0];
    let mut alignment_dot = term_2 + term_1;
    alignment_dot += term_0;

    AttachmentCandidateMetrics {
        row_index: row.row_index,
        squared_distance,
        alignment_dot,
        field_114: row.field_114,
    }
}

/// Return the original `0xD1C90` result for raw record field `+0x114`.
pub const fn d1c90_result(field_114: u32) -> bool {
    field_114 == 0x10 || field_114 == 0x40000
}

/// Select the nearest candidate that passes the measured 0x18AF40 filters.
///
/// `None` means every row failed a filter or compared at least as far away as
/// the original `f32::MAX` best-distance sentinel. Strict comparison preserves
/// the first table row on ties, matching the original update order.
pub fn select_nearest_qualifying(
    candidates: impl IntoIterator<Item = AttachmentCandidateMetrics>,
) -> Option<usize> {
    let mut best_distance = f32::MAX;
    let mut best_row = None;

    for candidate in candidates {
        if !(candidate.alignment_dot > 0.0) || d1c90_result(candidate.field_114) {
            continue;
        }
        if candidate.squared_distance < best_distance {
            best_distance = candidate.squared_distance;
            best_row = Some(candidate.row_index);
        }
    }

    best_row
}

/// Measure and rank caller-supplied raw rows as the pure inner selection slice
/// of 0x18AF40. Table traversal and row ownership stay at the edge.
pub fn select_nearest_raw_candidates(
    candidates: impl IntoIterator<Item = SelectorCandidateRaw>,
    reference: [f32; 3],
    score_reference: [f32; 3],
    score_weights: [f32; 3],
) -> Option<usize> {
    select_nearest_qualifying(
        candidates
            .into_iter()
            .map(|row| measure_selector_candidate(row, reference, score_reference, score_weights)),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        AttachmentCandidateMetrics as Candidate, SelectorCandidateRaw, d1c90_result,
        measure_selector_candidate, select_nearest_qualifying, select_nearest_raw_candidates,
    };

    #[test]
    fn d1c90_compares_only_the_two_measured_raw_values() {
        assert!(d1c90_result(0x10));
        assert!(d1c90_result(0x40000));
        assert!(!d1c90_result(0));
        assert!(!d1c90_result(0x11));
        assert!(!d1c90_result(0x40001));
    }

    #[test]
    fn rejects_nonpositive_alignment_and_state_helper_matches() {
        let rows = [
            Candidate {
                row_index: 0,
                squared_distance: 0.1,
                alignment_dot: 0.0,
                field_114: 0,
            },
            Candidate {
                row_index: 1,
                squared_distance: 0.2,
                alignment_dot: 1.0,
                field_114: 0x10,
            },
            Candidate {
                row_index: 2,
                squared_distance: 0.3,
                alignment_dot: 1.0,
                field_114: 0,
            },
        ];

        assert_eq!(select_nearest_qualifying(rows), Some(2));
    }

    #[test]
    fn strict_distance_update_keeps_the_first_row_on_ties() {
        let rows = [
            Candidate {
                row_index: 7,
                squared_distance: 4.0,
                alignment_dot: 0.25,
                field_114: 0,
            },
            Candidate {
                row_index: 9,
                squared_distance: 4.0,
                alignment_dot: 1.0,
                field_114: 0,
            },
            Candidate {
                row_index: 3,
                squared_distance: 2.0,
                alignment_dot: 0.5,
                field_114: 0,
            },
        ];

        assert_eq!(select_nearest_qualifying(rows), Some(3));
        assert_eq!(select_nearest_qualifying(rows.into_iter().take(2)), Some(7));
    }

    #[test]
    fn unordered_and_sentinel_or_larger_distances_do_not_win() {
        let rows = [
            Candidate {
                row_index: 1,
                squared_distance: 1.0,
                alignment_dot: f32::NAN,
                field_114: 0,
            },
            Candidate {
                row_index: 2,
                squared_distance: f32::NAN,
                alignment_dot: 1.0,
                field_114: 0,
            },
            Candidate {
                row_index: 3,
                squared_distance: f32::MAX,
                alignment_dot: 1.0,
                field_114: 0,
            },
        ];

        assert_eq!(select_nearest_qualifying(rows), None);
    }

    #[test]
    fn raw_candidate_vectors_produce_ordered_distance_and_score_metrics() {
        let row = SelectorCandidateRaw {
            row_index: 7,
            vector_a8_ac_b0: [3.0, 4.0, 12.0],
            vector_b4_b8_bc: [5.0, 8.0, 13.0],
            field_114: 0x20,
        };
        let metrics =
            measure_selector_candidate(row, [0.0, 0.0, 0.0], [1.0, 2.0, 3.0], [1.0, 2.0, 3.0]);

        assert_eq!(metrics.row_index, 7);
        assert_eq!(metrics.squared_distance, 169.0);
        assert_eq!(metrics.alignment_dot, 46.0);
        assert_eq!(metrics.field_114, 0x20);
    }

    #[test]
    fn raw_rows_select_nearest_positive_unfiltered_candidate() {
        let rows = [
            SelectorCandidateRaw {
                row_index: 0,
                vector_a8_ac_b0: [0.5, 0.0, 0.0],
                vector_b4_b8_bc: [1.0, 1.0, 1.0],
                field_114: 0x10,
            },
            SelectorCandidateRaw {
                row_index: 1,
                vector_a8_ac_b0: [0.25, 0.0, 0.0],
                vector_b4_b8_bc: [0.0, 0.0, 0.0],
                field_114: 0,
            },
            SelectorCandidateRaw {
                row_index: 2,
                vector_a8_ac_b0: [3.0, 0.0, 0.0],
                vector_b4_b8_bc: [1.0, 1.0, 1.0],
                field_114: 0,
            },
            SelectorCandidateRaw {
                row_index: 3,
                vector_a8_ac_b0: [1.0, 0.0, 0.0],
                vector_b4_b8_bc: [2.0, 2.0, 2.0],
                field_114: 0,
            },
        ];
        assert_eq!(
            select_nearest_raw_candidates(rows, [0.0; 3], [0.0; 3], [1.0; 3]),
            Some(3)
        );
    }
}
