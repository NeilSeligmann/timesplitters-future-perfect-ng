//! Typed arithmetic boundary for native `0x12540`.
//!
//! REA measures a three-argument floating-point sequence around the unresolved
//! `0x3C8501` floating-environment helper. This port keeps that helper explicit
//! instead of treating it as an identity function. Evidence label: INFERRED;
//! keep opt-in until the helper contract is validated.

/// Inputs to `0x12540`, retained as raw argument positions because their game
/// meanings and units have not been established.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerValueArgs {
    pub arg1: f32,
    pub arg2: f32,
    pub arg3: f32,
}

/// Adapter for the original floating-environment/status helper at `0x3C8501`.
///
/// Its exceptional-value and status behavior is not ported. Callers must supply
/// an implementation derived from validated evidence before enabling this path.
pub trait QuotientStatusHelper {
    /// Return `None` when the input is outside the helper implementation's
    /// validated domain.
    fn apply_status_path(&mut self, quotient: f64) -> Option<f64>;
}

/// x87 control-word rounding modes used by `FRNDINT`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum X87RoundingMode {
    NearestEven,
    Downward,
    Upward,
    TowardZero,
}

/// Rounding mode encoded by the observed initial word at `0x54D7C4` (`0x003F`).
/// Runtime writes to that global have not been ruled out; callers should pass
/// the mode observed for their execution context.
pub const XBE_INITIAL_ROUNDING_MODE: X87RoundingMode = X87RoundingMode::NearestEven;

/// Value-only finite-input model of the `FRNDINT` return path in `0x3C8501`.
///
/// This preserves the rounded numeric result for an explicitly supplied x87
/// mode. It does not reproduce the helper's floating-point status/error calls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrndintValueOnly {
    pub rounding_mode: X87RoundingMode,
}

impl QuotientStatusHelper for FrndintValueOnly {
    fn apply_status_path(&mut self, quotient: f64) -> Option<f64> {
        if !quotient.is_finite() {
            return None;
        }

        let truncated = quotient.trunc();
        let rounded = match self.rounding_mode {
            X87RoundingMode::TowardZero => truncated,
            X87RoundingMode::Downward if quotient < truncated => truncated - 1.0,
            X87RoundingMode::Upward if quotient > truncated => truncated + 1.0,
            X87RoundingMode::Downward | X87RoundingMode::Upward => truncated,
            X87RoundingMode::NearestEven => {
                if quotient.abs() >= 4_503_599_627_370_496.0 {
                    quotient
                } else {
                    let fraction = quotient - truncated;
                    match fraction.abs().partial_cmp(&0.5) {
                        Some(core::cmp::Ordering::Less) => truncated,
                        Some(core::cmp::Ordering::Greater) => truncated + fraction.signum(),
                        Some(core::cmp::Ordering::Equal) if truncated % 2.0 == 0.0 => truncated,
                        Some(core::cmp::Ordering::Equal) => truncated + fraction.signum(),
                        None => return None,
                    }
                }
            }
        };

        Some(rounded)
    }
}

/// Apply the locally observed `0x12540` arithmetic sequence.
///
/// The denominator is rounded to `f32`, matching the original store at
/// `[esp+0x14]`; division is represented as `f64` for the qword passed to
/// `0x3C8501`. This is a readable arithmetic port with a replaceable unknown
/// helper boundary, not a complete function port or bit-exact x87 model.
pub fn apply_12540_sequence(
    args: ControllerValueArgs,
    helper: &mut impl QuotientStatusHelper,
) -> Option<f64> {
    let denominator = args.arg3 - args.arg2;
    let numerator = args.arg1 as f64 - args.arg2 as f64;
    let quotient = numerator / denominator as f64;
    let processed_quotient = helper.apply_status_path(quotient)?;
    Some(args.arg1 as f64 - processed_quotient * denominator as f64)
}

/// Apply the raw `0x2712EC` call-site argument form of `0x12540`.
///
/// REA measures the third argument as `180` and the second as `-180`. The first
/// argument is the caller's computed scalar; interpreting it as an angle delta
/// remains inferred, so this adapter keeps the callsite address in its name.
pub fn apply_12540_callsite_2712ec(
    computed_arg1: f32,
    rounding_mode: X87RoundingMode,
) -> Option<f64> {
    let args = ControllerValueArgs {
        arg1: computed_arg1,
        arg2: -180.0,
        arg3: 180.0,
    };
    let mut helper = FrndintValueOnly { rounding_mode };
    apply_12540_sequence(args, &mut helper)
}
