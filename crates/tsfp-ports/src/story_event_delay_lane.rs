// SPDX-License-Identifier: GPL-3.0-or-later
//! T2468: typed Story event-instance delay countdown and dispatch eligibility.
//!
//! This owns only the generic instance `+0x38` delay field. Link-mode polling and objective-slot
//! completion remain separate, and the enclosing state pass retains its original call order.

use crate::memory::GuestMemory;

/// Typed view of the one measured event-instance field owned by this slice. The complete pool
/// record remains partially unresolved; this view intentionally declares no wider size.
pub const OFFSET_DELAY_SECONDS: u32 = 0x38;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventInstanceDelay {
    /// Remaining event delay in seconds, stored at event-instance `+0x38`.
    pub remaining_seconds: f32,
}

impl EventInstanceDelay {
    pub fn load<M: GuestMemory>(memory: &M, event_instance: u32) -> Self {
        Self {
            remaining_seconds: f32::from_bits(
                memory.read32(event_instance.wrapping_add(OFFSET_DELAY_SECONDS)),
            ),
        }
    }

    pub fn store<M: GuestMemory>(&self, memory: &mut M, event_instance: u32) {
        memory.write32(
            event_instance.wrapping_add(OFFSET_DELAY_SECONDS),
            self.remaining_seconds.to_bits(),
        );
    }
}

/// Advance the typed timer by the observed frame delta. The caller computes delta using the
/// lifted game's scale and divisor, preserving that clock boundary and ordering.
pub fn advance_delay(state: &mut EventInstanceDelay, frame_delta: f32) {
    state.remaining_seconds -= frame_delta;
}

/// The state pass proceeds only when the decremented timer compares `<= 0.0`; NaN is ineligible.
pub fn dispatch_eligible(state: &EventInstanceDelay) -> bool {
    state.remaining_seconds <= 0.0
}

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

/// Default-off A/B switch. `TSFP_STORY_EVENT_DELAY=rust` selects the typed transition.
#[cfg(not(test))]
pub fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_EVENT_DELAY".as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
pub fn enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifted_delay_bits(delay_bits: u32, frame_delta: f32) -> (u32, bool) {
        // Mirrors the retained C2R SSE scalar subtract and later x87-compatible eligibility test.
        let delay = f32::from_bits(delay_bits) - frame_delta;
        let delay64 = delay as f64;
        let eligible = !(0.0f64 < delay64 || delay64.is_nan());
        (delay.to_bits(), eligible)
    }

    fn typed_delay_bits(delay_bits: u32, frame_delta: f32) -> (u32, bool) {
        let mut state = EventInstanceDelay {
            remaining_seconds: f32::from_bits(delay_bits),
        };
        advance_delay(&mut state, frame_delta);
        (state.remaining_seconds.to_bits(), dispatch_eligible(&state))
    }

    #[test]
    fn fabricated_tier_one_delay_transition_matches_lifted_body_and_mutation_guard() {
        let mut diverged = false;
        for seed in 0..4096u32 {
            let delay_bits = seed.wrapping_mul(0x9e37_79b9).rotate_left(7);
            let delta_bits = seed.wrapping_mul(0x85eb_ca6b).rotate_right(11);
            let delta = f32::from_bits(delta_bits);
            assert_eq!(
                typed_delay_bits(delay_bits, delta),
                lifted_delay_bits(delay_bits, delta)
            );

            // A deliberate wrong-sign mutation must be detected by this input set.
            let mut mutated = EventInstanceDelay {
                remaining_seconds: f32::from_bits(delay_bits),
            };
            mutated.remaining_seconds += delta;
            diverged |= (
                mutated.remaining_seconds.to_bits(),
                dispatch_eligible(&mutated),
            ) != typed_delay_bits(delay_bits, delta);
        }
        assert!(
            diverged,
            "fabricated cases did not detect the wrong-sign mutation"
        );
    }

    #[test]
    fn eligibility_matches_lifted_order_for_boundaries_and_nan() {
        for (bits, expected) in [
            (0.0f32.to_bits(), true),
            ((-0.0f32).to_bits(), true),
            (f32::MIN_POSITIVE.to_bits(), false),
            ((-f32::MIN_POSITIVE).to_bits(), true),
            (f32::NAN.to_bits(), false),
            (f32::INFINITY.to_bits(), false),
            (f32::NEG_INFINITY.to_bits(), true),
        ] {
            let state = EventInstanceDelay {
                remaining_seconds: f32::from_bits(bits),
            };
            assert_eq!(dispatch_eligible(&state), lifted_delay_bits(bits, 0.0).1);
            assert_eq!(dispatch_eligible(&state), expected);
        }
    }

    #[test]
    fn switch_requires_exact_rust_value() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(!enabled_from(Some(b"true")));
        assert!(enabled_from(Some(b"rust")));
    }
}
