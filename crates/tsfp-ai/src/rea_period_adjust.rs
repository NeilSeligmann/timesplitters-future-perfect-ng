//! Bounded raw interval adjustment loop observed inside XBE function 0x5BA40.

/// Repeat the measured add/subtract loop into the inclusive interval `[0, period]`.
///
/// Returns `None` outside the safe finite domain or when float addition/subtraction
/// cannot make progress. This is only the inner scalar loop, not a port of 0x5BA40.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn adjust_period_interval_5ba40(value: f32, period: f32) -> Option<f32> {
    if !value.is_finite() || !period.is_finite() || period < 0.0 {
        return None;
    }
    let mut adjusted = value;
    while adjusted < 0.0 {
        let next = adjusted + period;
        if next == adjusted {
            return None;
        }
        adjusted = next;
    }
    while adjusted > period {
        let next = adjusted - period;
        if next == adjusted {
            return None;
        }
        adjusted = next;
    }
    Some(adjusted)
}

/// Quantize the nonnegative scalar step used after the period adjustment.
///
/// Values whose fraction is below `threshold` use the truncated integer;
/// equal or larger fractions use the next integer, then add `offset`.
/// Verification: Tier 0 (focused smoke tests only; no differential or replay).
pub fn quantize_fraction_threshold_5ba40(value: f32, threshold: f32, offset: f32) -> Option<f32> {
    if !value.is_finite()
        || value < 0.0
        || value >= i32::MAX as f32
        || !threshold.is_finite()
        || !offset.is_finite()
    {
        return None;
    }
    let lower = value.trunc();
    let selected = if value - lower < threshold {
        lower
    } else {
        lower + 1.0
    };
    Some(selected + offset)
}

#[cfg(test)]
mod tests {
    use super::{adjust_period_interval_5ba40, quantize_fraction_threshold_5ba40};

    #[test]
    fn repeats_addition_and_subtraction_to_inclusive_bounds() {
        assert_eq!(adjust_period_interval_5ba40(-0.5, 2.0), Some(1.5));
        assert_eq!(adjust_period_interval_5ba40(5.0, 2.0), Some(1.0));
        assert_eq!(adjust_period_interval_5ba40(0.0, 2.0), Some(0.0));
        assert_eq!(adjust_period_interval_5ba40(2.0, 2.0), Some(2.0));
    }

    #[test]
    fn declines_nonfinite_or_nonprogressing_inputs() {
        assert_eq!(adjust_period_interval_5ba40(f32::NAN, 2.0), None);
        assert_eq!(adjust_period_interval_5ba40(1.0, f32::INFINITY), None);
        assert_eq!(adjust_period_interval_5ba40(1.0, 0.0), None);
    }

    #[test]
    fn quantizes_at_the_measured_fraction_threshold() {
        let threshold = f32::from_bits(0x3f00_0000);
        assert_eq!(
            quantize_fraction_threshold_5ba40(1.49, threshold, 0.25),
            Some(1.25)
        );
        assert_eq!(
            quantize_fraction_threshold_5ba40(1.5, threshold, 0.25),
            Some(2.25)
        );
        assert_eq!(
            quantize_fraction_threshold_5ba40(1.75, threshold, 0.25),
            Some(2.25)
        );
        assert_eq!(
            quantize_fraction_threshold_5ba40(-0.1, threshold, 0.25),
            None
        );
    }
}
