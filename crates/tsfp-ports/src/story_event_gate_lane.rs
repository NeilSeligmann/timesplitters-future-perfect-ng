// SPDX-License-Identifier: GPL-3.0-or-later
//! T2477: typed Story event run-gate at definition +0x5c / instance +0x28.

/// Gate mode and the executed word at runtime instance +0x34, if lookup found an instance.
/// `None` represents the null gate pointer and preserves the lifted missing-instance path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventRunGate {
    pub mode: u32,
    pub gate_executed: Option<u32>,
}

/// The state-pass makes the mode-2 arm decision before link combination and checks run
/// eligibility after the delay. These decisions are pure views over the same gate state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventRunGateDecision {
    pub arm_ignoring_links: bool,
    pub permits_execution: bool,
}

pub const fn run_gate_decision(input: EventRunGate) -> Option<EventRunGateDecision> {
    let executed = match input.gate_executed {
        Some(value) => value != 0,
        None => false,
    };
    let permits_execution = match input.mode {
        0 | 2 => true,
        // The lifted state pass skips the helper call on a null pointer, so a missing gate
        // never blocks either of these modes.
        1 => match input.gate_executed {
            Some(_) => executed,
            None => true,
        },
        3 => match input.gate_executed {
            Some(_) => !executed,
            None => true,
        },
        _ => return None,
    };
    Some(EventRunGateDecision {
        arm_ignoring_links: input.mode == 2 && executed,
        permits_execution,
    })
}

/// Default-off A/B switch. `TSFP_STORY_EVENT_GATE=rust` selects this typed predicate.
pub fn enabled() -> bool {
    #[cfg(not(test))]
    unsafe {
        unsafe extern "C" {
            fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
        }
        let value = getenv(c"TSFP_STORY_EVENT_GATE".as_ptr());
        !value.is_null() && core::ffi::CStr::from_ptr(value).to_bytes() == b"rust"
    }
    #[cfg(test)]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent transcription of the lifted early-arm check and 0x234270-based run gate.
    fn lifted(input: EventRunGate) -> Option<EventRunGateDecision> {
        if input.mode > 3 {
            return None;
        }
        let present = input.gate_executed.is_some();
        let executed = input.gate_executed.unwrap_or(0) != 0;
        let arm_ignoring_links = input.mode == 2 && present && executed;
        let permits_execution = match input.mode {
            0 | 2 => true,
            1 => !present || executed,
            3 => !present || !executed,
            _ => unreachable!(),
        };
        Some(EventRunGateDecision {
            arm_ignoring_links,
            permits_execution,
        })
    }

    fn mutated(input: EventRunGate) -> Option<EventRunGateDecision> {
        let mut result = run_gate_decision(input)?;
        if input.mode == 1 {
            result.permits_execution = !result.permits_execution;
        }
        Some(result)
    }

    #[test]
    fn tier1_weak_fabricated_differential_3000_cases() {
        let mut mutation_detected = false;
        let mut seed = 0x2477_5c31u32;
        for _ in 0..3000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let input = EventRunGate {
                mode: (seed >> 9) % 6,
                gate_executed: if seed & 4 == 0 {
                    None
                } else {
                    Some(seed.rotate_left(11))
                },
            };
            assert_eq!(run_gate_decision(input), lifted(input));
            mutation_detected |= mutated(input) != lifted(input);
        }
        assert!(mutation_detected, "mode-1 mutation was not detected");
    }

    #[test]
    fn gate_modes_and_missing_instance_contract() {
        for mode in 0..=3 {
            assert_eq!(
                run_gate_decision(EventRunGate {
                    mode,
                    gate_executed: None
                })
                .unwrap()
                .permits_execution,
                true
            );
        }
        assert!(
            run_gate_decision(EventRunGate {
                mode: 2,
                gate_executed: Some(1)
            })
            .unwrap()
            .arm_ignoring_links
        );
        assert!(
            !run_gate_decision(EventRunGate {
                mode: 2,
                gate_executed: Some(0)
            })
            .unwrap()
            .arm_ignoring_links
        );
        assert!(
            !run_gate_decision(EventRunGate {
                mode: 1,
                gate_executed: Some(0)
            })
            .unwrap()
            .permits_execution
        );
        assert!(
            !run_gate_decision(EventRunGate {
                mode: 3,
                gate_executed: Some(1)
            })
            .unwrap()
            .permits_execution
        );
        assert!(
            run_gate_decision(EventRunGate {
                mode: 4,
                gate_executed: None
            })
            .is_none()
        );
    }
}
