//! Rust-owned state transition used by the original 0x153B40 random integer helper.
//!
//! This ports only the measured shared-state recurrence. It does not model state
//! initialization, the caller's x87 integer-to-float conversion, or replay ownership.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReaRngState {
    /// Original global call counter at 0x7497C8.
    pub call_count: u32,
    /// Low dword of the original 64-bit state at 0x4E7950.
    pub state_low: u32,
    /// High dword of the original 64-bit state at 0x4E7954.
    pub state_high: u32,
}

/// Advance the measured 0x153B40 state and return its updated low dword.
///
/// REA shows a 64-bit state multiplied by 0x343FD, then incremented by 0x269EC3,
/// with modulo-2^64 storage. The call counter increments modulo 2^32.
pub fn advance_153b40(state: &mut ReaRngState) -> u32 {
    state.call_count = state.call_count.wrapping_add(1);
    let previous = (u64::from(state.state_high) << 32) | u64::from(state.state_low);
    let next = previous.wrapping_mul(0x0003_43fd).wrapping_add(0x0026_9ec3);
    state.state_low = next as u32;
    state.state_high = (next >> 32) as u32;
    state.state_low
}

/// Convert DFFA0's returned random dword into its emitter scalar.
///
/// The caller interprets the integer as unsigned, adds one to its unit interval,
/// scales by the positive `0xDFFA0` input, then halves the result. `f64` keeps
/// intermediate precision before the stored `f32` value is produced.
pub fn dffa0_randomized_scalar(random_dword: u32, scale: f32) -> f32 {
    let unsigned_unit = f64::from(random_dword) * (1.0 / 4_294_967_296.0);
    (((unsigned_unit + 1.0) * f64::from(scale)) * 0.5) as f32
}

/// Advance the shared recurrence and apply DFFA0's measured caller-side conversion.
pub fn advance_dffa0_scalar(state: &mut ReaRngState, scale: f32) -> f32 {
    dffa0_randomized_scalar(advance_153b40(state), scale)
}

#[cfg(test)]
mod tests {
    use super::{ReaRngState, advance_153b40, advance_dffa0_scalar, dffa0_randomized_scalar};

    #[test]
    fn zero_seed_advances_shared_state_and_returns_low_dword() {
        let mut state = ReaRngState::default();
        assert_eq!(advance_153b40(&mut state), 0x0026_9ec3);
        assert_eq!(
            state,
            ReaRngState {
                call_count: 1,
                state_low: 0x0026_9ec3,
                state_high: 0
            }
        );

        assert_eq!(advance_153b40(&mut state), 0x1e27_8e7a);
        assert_eq!(
            state,
            ReaRngState {
                call_count: 2,
                state_low: 0x1e27_8e7a,
                state_high: 0x7e
            }
        );
    }

    #[test]
    fn recurrence_and_call_counter_wrap_at_their_observed_widths() {
        let mut state = ReaRngState {
            call_count: u32::MAX,
            state_low: u32::MAX,
            state_high: u32::MAX,
        };
        assert_eq!(advance_153b40(&mut state), 0x0023_5ac6);
        assert_eq!(state.call_count, 0);
        assert_eq!(state.state_high, 0);
    }

    #[test]
    fn dffa0_unsigned_conversion_scales_between_half_and_full_input() {
        assert_eq!(dffa0_randomized_scalar(0, 2.0), 1.0);
        assert_eq!(dffa0_randomized_scalar(0x8000_0000, 2.0), 1.5);
        assert_eq!(dffa0_randomized_scalar(u32::MAX, 2.0), 2.0);

        let mut state = ReaRngState::default();
        assert_eq!(
            advance_dffa0_scalar(&mut state, 2.0),
            1.000_589_251_518_249_5
        );
        assert_eq!(state.call_count, 1);
    }
}
