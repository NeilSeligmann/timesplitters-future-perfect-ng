//! Typed slice for the candidate-driven scalar adjustment inside `0x18AF40`.
//!
//! Inputs retain the observed field offsets and runtime globals. The semantic
//! names of the candidate coordinates, query coordinate and actor `+0x94`
//! scalar are unknown, so this slice does not assign them game-level meaning.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateAdjustmentRaw {
    /// Candidate record components read at `+0xB4/+0xB8/+0xBC`.
    pub candidate_components_b4_b8_bc: [f32; 3],
    /// Stack-local components read at `+0x14/+0x18/+0x1C`.
    pub local_components_14_18_1c: [f32; 3],
    /// The separately read stack-local scalar at `+0x24`.
    pub local_scalar_24: f32,
    /// Actor field written at `+0x94`.
    pub actor_scalar_94: f32,
    /// Runtime value read from global `0x4765F4`.
    pub normalize_squared_distance_threshold: f32,
    /// Signed integer converted from global `0x4E8CEC`.
    pub rate_divisor_4e8cec: i32,
    /// Signed integer converted from global `0x4E7944`.
    pub signed_rate_4e7944: i32,
    /// Float read from global `0x475CE4`.
    pub angular_scale_475ce4: f32,
    /// Clamp bounds read from `0x47DE9C` and `0x47DEA0` respectively.
    pub clamp_bounds_47de9c_47dea0: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CandidateAdjustmentResult {
    pub squared_distance: f32,
    pub selected_component: f32,
    pub signed_step: f32,
    pub actor_scalar_94: f32,
    /// False when the strict absolute-difference gate jumps around both the
    /// adjustment and clamp block.
    pub adjustment_path_taken: bool,
}

/// Apply the raw candidate adjustment and bound its resulting actor field.
///
/// This follows the observed branch order and strict comparisons. `rsqrtss`
/// is represented as reciprocal `f32::sqrt`; its approximation/rounding is a
/// known difference from the original SIMD instruction, so this remains Tier
/// 0 pending lifted-vs-Rust differential coverage.
pub fn apply_candidate_adjustment(input: CandidateAdjustmentRaw) -> CandidateAdjustmentResult {
    let delta = [
        input.candidate_components_b4_b8_bc[0] - input.local_components_14_18_1c[0],
        input.candidate_components_b4_b8_bc[1] - input.local_components_14_18_1c[1],
        input.candidate_components_b4_b8_bc[2] - input.local_components_14_18_1c[2],
    ];
    let squared_distance = delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2];

    // COMISS + JBE treats unordered values like the taken (<=) branch.
    let mut selected_component = delta[1];
    if !comiss_jbe(squared_distance, input.normalize_squared_distance_threshold) {
        selected_component *= 1.0 / squared_distance.sqrt();
    }

    let difference = selected_component - input.local_scalar_24;
    let absolute_difference = f32::from_bits(difference.to_bits() & 0x7fff_ffff);
    let rate_divisor = input.rate_divisor_4e8cec as f32;
    let signed_rate = input.signed_rate_4e7944 as f32;
    let signed_step = (1.0 / rate_divisor) * signed_rate * input.angular_scale_475ce4;

    let mut actor_scalar_94 = input.actor_scalar_94;
    let mut adjustment_path_taken = false;
    if !comiss_jbe(absolute_difference, signed_step) {
        adjustment_path_taken = true;
        if !comiss_jbe(input.local_scalar_24, selected_component) {
            actor_scalar_94 += signed_step;
        } else if !comiss_jbe(selected_component, input.local_scalar_24) {
            actor_scalar_94 -= signed_step;
        }

        let [lower, upper] = input.clamp_bounds_47de9c_47dea0;
        if !comiss_jbe(actor_scalar_94, upper) {
            actor_scalar_94 = upper;
        } else if !comiss_jbe(lower, actor_scalar_94) {
            actor_scalar_94 = lower;
        }
    }

    CandidateAdjustmentResult {
        squared_distance,
        selected_component,
        signed_step,
        actor_scalar_94,
        adjustment_path_taken,
    }
}

/// COMISS followed by JBE: CF or ZF takes the branch; unordered sets CF.
fn comiss_jbe(lhs: f32, rhs: f32) -> bool {
    lhs.is_nan() || rhs.is_nan() || lhs <= rhs
}

#[cfg(test)]
mod tests {
    use super::{CandidateAdjustmentRaw, apply_candidate_adjustment};

    fn input() -> CandidateAdjustmentRaw {
        CandidateAdjustmentRaw {
            candidate_components_b4_b8_bc: [3.0, 4.0, 0.0],
            local_components_14_18_1c: [0.0; 3],
            local_scalar_24: 0.0,
            actor_scalar_94: 0.52,
            normalize_squared_distance_threshold: 0.0,
            rate_divisor_4e8cec: 60,
            signed_rate_4e7944: 1,
            angular_scale_475ce4: std::f32::consts::FRAC_PI_2,
            clamp_bounds_47de9c_47dea0: [-0.523_598_8, 0.523_598_8],
        }
    }

    #[test]
    fn strict_distance_gate_applies_signed_step_and_upper_clamp() {
        let result = apply_candidate_adjustment(input());
        assert_eq!(result.squared_distance, 25.0);
        assert_eq!(result.selected_component, 0.8);
        assert!(result.adjustment_path_taken);
        // The step is (1/60) * 1 * pi/2; 0.52 - that step is
        // already inside the clamp interval, so the lower value survives.
        assert_eq!(
            result.actor_scalar_94,
            0.52 - (1.0 / 60.0) * std::f32::consts::FRAC_PI_2
        );
    }

    #[test]
    fn zero_difference_skips_both_update_and_clamp() {
        let mut raw = input();
        raw.candidate_components_b4_b8_bc = [0.0, 1.0, 0.0];
        raw.local_scalar_24 = 1.0;
        raw.actor_scalar_94 = 10.0;
        let result = apply_candidate_adjustment(raw);
        assert!(!result.adjustment_path_taken);
        assert_eq!(result.actor_scalar_94, 10.0);
    }

    #[test]
    fn negative_step_and_nan_follow_comiss_branch_order() {
        let mut raw = input();
        raw.signed_rate_4e7944 = -1;
        let negative = apply_candidate_adjustment(raw);
        assert!(negative.signed_step < 0.0);
        assert!(negative.adjustment_path_taken);

        raw.candidate_components_b4_b8_bc[1] = f32::NAN;
        let unordered = apply_candidate_adjustment(raw);
        assert!(!unordered.adjustment_path_taken);
        assert_eq!(unordered.actor_scalar_94, raw.actor_scalar_94);
    }
}
