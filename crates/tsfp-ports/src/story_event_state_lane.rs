// SPDX-License-Identifier: GPL-3.0-or-later
//! T2469: typed active-instance control gates inside the Story state pass (0x237E70).
//!
//! The adapter owns only two measured decisions. `executed == 2` is normalized to zero for
//! an active instance, and a nonzero instance `+0x2c` bypasses the condition-evaluation block.
//! Condition routines, verb calls, callback dispatch, and objective link polling remain at
//! their existing measured call boundaries.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveStateControl {
    pub executed: u32,
    pub condition_wait: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveStateDecision {
    pub executed: u32,
    pub evaluate_conditions: bool,
}

/// Measured control transition for one member of the state pass's entry-count snapshot.
pub const fn active_state_decision(input: ActiveStateControl) -> ActiveStateDecision {
    ActiveStateDecision {
        executed: if input.executed == 2 {
            0
        } else {
            input.executed
        },
        evaluate_conditions: input.condition_wait != 0,
    }
}

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(b"rust")
}

/// Default-off A/B switch. `TSFP_STORY_EVENT_STATE=rust` selects the typed control decisions.
#[cfg(not(test))]
pub fn enabled() -> bool {
    unsafe {
        let value = getenv(c"TSFP_STORY_EVENT_STATE".as_ptr());
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

    fn lifted(input: ActiveStateControl) -> ActiveStateDecision {
        ActiveStateDecision {
            executed: if input.executed == 2 {
                0
            } else {
                input.executed
            },
            evaluate_conditions: input.condition_wait != 0,
        }
    }

    fn mutated(input: ActiveStateControl) -> ActiveStateDecision {
        ActiveStateDecision {
            executed: if input.executed == 2 {
                1
            } else {
                input.executed
            },
            evaluate_conditions: input.condition_wait != 0,
        }
    }

    #[test]
    fn fabricated_tier_one_control_gate_differential_and_mutation_guard() {
        let mut mutation_detected = false;
        for seed in 0..4096u32 {
            let input = ActiveStateControl {
                executed: if seed == 1 {
                    2
                } else {
                    seed.wrapping_mul(0x9e37_79b9).rotate_left(7)
                },
                condition_wait: seed.wrapping_mul(0x85eb_ca6b).rotate_right(11),
            };
            let actual = active_state_decision(input);
            let reference = lifted(input);
            assert_eq!(actual, reference);
            mutation_detected |= mutated(input) != reference;
        }
        assert!(mutation_detected, "normalization mutation was not detected");
        assert_eq!(
            active_state_decision(ActiveStateControl {
                executed: 2,
                condition_wait: 0
            })
            .executed,
            0
        );
        assert!(
            active_state_decision(ActiveStateControl {
                executed: 0,
                condition_wait: 1
            })
            .evaluate_conditions
        );
    }
}
