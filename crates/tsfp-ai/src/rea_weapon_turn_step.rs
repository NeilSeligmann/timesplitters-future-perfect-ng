// SPDX-License-Identifier: GPL-3.0-or-later
//! Turn error normalization and accumulator update used by the weapon target controller.

/// The controller converts the scaled x87 turn rate to an integer with FISTP.
/// The caller computes `(turn_rate - record_adjustment) * 60` before entering
/// that conversion helper. Valid gameplay values are finite and in i32 range.
pub fn rounded_turn_steps(turn_rate: f32, record_adjustment: f32) -> i32 {
    ((f64::from(turn_rate) - f64::from(record_adjustment)) * 60.0).round_ties_even() as i32
}

/// Model the signed integer conversion performed by FISTP for finite in-range values.
pub fn fistp_i32(value: f64, x87_control: u16) -> i32 {
    let rounded = match x87_control & 0x0c00 {
        0x0400 => value.floor(),
        0x0800 => value.ceil(),
        0x0c00 => value.trunc(),
        _ => value.round_ties_even(),
    };
    rounded as i32
}

/// Apply the controller's one-sided turn accumulator update.
///
/// Returns `None` when the error threshold is not crossed or the rounded step
/// divisor is nonpositive. The original caps only the positive side at 0.5.
pub fn turn_accumulator_delta(angle_error: f32, steps: i32) -> Option<f32> {
    if angle_error.abs() <= 0.3 || steps <= 0 {
        return None;
    }
    Some((angle_error / steps as f32).min(0.5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_divisor_and_one_sided_cap() {
        assert_eq!(turn_accumulator_delta(0.3, 12), None);
        assert_eq!(turn_accumulator_delta(-0.3, 12), None);
        assert_eq!(turn_accumulator_delta(0.31, 0), None);
        assert_eq!(turn_accumulator_delta(0.31, -1), None);
        assert_eq!(turn_accumulator_delta(12.0, 12), Some(0.5));
        assert_eq!(turn_accumulator_delta(-12.0, 12), Some(-1.0));
        assert_eq!(turn_accumulator_delta(0.6, 12), Some(0.05));
    }

    #[test]
    fn x87_integer_conversion_uses_nearest_even() {
        assert_eq!(rounded_turn_steps(1.0, 0.0), 60);
        assert_eq!(rounded_turn_steps(0.125, 0.0), 8);
        // The f32 adjustment makes this value slightly below 6.5.
        assert_eq!(rounded_turn_steps(0.125, 1.0 / 60.0), 6);
        // This f32 adjustment makes the scaled value slightly below -0.5.
        assert_eq!(rounded_turn_steps(0.0, 0.5 / 60.0), -1);
        assert_eq!(rounded_turn_steps(0.0, 1.5 / 60.0), -2);
        assert_eq!(fistp_i32(6.5, 0), 6);
        assert_eq!(fistp_i32(-0.5, 0), 0);
        assert_eq!(fistp_i32(1.5, 0), 2);
        assert_eq!(fistp_i32(1.9, 0x0400), 1);
        assert_eq!(fistp_i32(1.1, 0x0800), 2);
        assert_eq!(fistp_i32(-1.9, 0x0c00), -1);
    }
}
