//! Weak checks for the REA-derived 0x3C848C conversion and turn update slice.

#[test]
fn turn_error_slice_checks_threshold_positive_divisor_and_upper_only_clamp() {
    use tsfp_ai::rea_weapon_turn_step::{rounded_turn_steps, turn_accumulator_delta};
    // WEAK reference: Ghidra REA instruction transcription (FIMUL 60, FISTP,
    // COMISS 0.3, DIVSS, COMISS 0.5, ADDSS); no lifted body/original execution.
    assert_eq!(rounded_turn_steps(1.0, 0.0), 60);
    assert_eq!(rounded_turn_steps(0.25, 0.0), 15);
    assert_eq!(turn_accumulator_delta(0.3, 15), None);
    assert_eq!(turn_accumulator_delta(0.31, 0), None);
    assert_eq!(turn_accumulator_delta(10.0, 15), Some(0.5));
    assert_eq!(turn_accumulator_delta(-10.0, 15), Some(-10.0 / 15.0));
    assert!((turn_accumulator_delta(0.6, 15).unwrap() - 0.04).abs() < 1e-7);
}

#[test]
fn weak_seeded_differential_and_mutation_boundaries() {
    use tsfp_ai::rea_weapon_turn_step::{rounded_turn_steps, turn_accumulator_delta};
    // Independent instruction-order transcription; all inputs are FABRICATED.
    fn rea_instruction_reference(error: f32, rate: f32, adjustment: f32) -> Option<f32> {
        let scaled_x87 = (f64::from(rate) - f64::from(adjustment)) * 60.0;
        let divisor = scaled_x87.round_ties_even() as i32;
        if !(error.abs() > 0.3) || divisor <= 0 {
            return None;
        }
        let quotient = error / divisor as f32;
        Some(if quotient > 0.5 { 0.5 } else { quotient })
    }
    let mut seed = 0x26ce_20u32;
    for _ in 0..3000 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let error = ((seed as i32) as f32 / 131_072.0).clamp(-900.0, 900.0);
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let rate = ((seed as i32) as f32 / 65_536.0).clamp(-300.0, 300.0);
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let adjustment = ((seed as i32) as f32 / 65_536.0).clamp(-300.0, 300.0);
        let divisor = rounded_turn_steps(rate, adjustment);
        let actual = turn_accumulator_delta(error, divisor);
        let expected = rea_instruction_reference(error, rate, adjustment);
        assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
    }

    // Sentinels for changing the threshold, positivity gate, or upper-only cap.
    assert_eq!(turn_accumulator_delta(0.3001, 1), Some(0.3001));
    assert_eq!(turn_accumulator_delta(0.3, 1), None);
    assert_eq!(turn_accumulator_delta(0.31, 0), None);
    assert_eq!(turn_accumulator_delta(1.0, 1), Some(0.5));
    assert_eq!(turn_accumulator_delta(-1.0, 1), Some(-1.0));
}
