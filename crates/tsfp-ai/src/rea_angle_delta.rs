// SPDX-License-Identifier: GPL-3.0-or-later
//! Safe readable angle-delta helper reconstructed from original 0x12430.

/// Return `to - from` wrapped to the original interval `(-180, 180]` degrees.
///
/// The original repeatedly adds/subtracts 360 degrees. Non-finite values, and
/// finite values too large for `f32` to make progress by 360, return `None`
/// here; those inputs would produce unordered flow or a non-terminating loop in
/// the original helper and are outside the usable gameplay contract.
pub fn wrapped_angle_delta(from: f32, to: f32) -> Option<f32> {
    if !from.is_finite() || !to.is_finite() {
        return None;
    }

    let mut delta = to - from;
    if !delta.is_finite() {
        return None;
    }

    while delta <= -180.0 {
        let next = delta + 360.0;
        if next == delta {
            return None;
        }
        delta = next;
    }
    while delta > 180.0 {
        let next = delta - 360.0;
        if next == delta {
            return None;
        }
        delta = next;
    }

    Some(delta)
}

#[cfg(test)]
mod tests {
    use super::wrapped_angle_delta;

    #[test]
    fn wraps_across_both_boundaries_and_preserves_positive_half_turn() {
        assert_eq!(wrapped_angle_delta(350.0, 10.0), Some(20.0));
        assert_eq!(wrapped_angle_delta(10.0, 350.0), Some(-20.0));
        assert_eq!(wrapped_angle_delta(0.0, -180.0), Some(180.0));
        assert_eq!(wrapped_angle_delta(0.0, 180.0), Some(180.0));
    }

    #[test]
    fn rejects_inputs_that_cannot_make_finite_progress() {
        assert_eq!(wrapped_angle_delta(f32::INFINITY, 0.0), None);
        assert_eq!(wrapped_angle_delta(0.0, f32::NAN), None);
        assert_eq!(wrapped_angle_delta(0.0, f32::MAX), None);
    }
}
