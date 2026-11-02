//! Raw selector-to-offset port for original controller helper `0x26CE20`.
//!
//! The original helper reads guest tables and delegates selector checks to
//! `0x59D20`. This version receives those table values and the check through
//! typed inputs; it does not read guest memory.

use crate::rea_controller_value_12540::{X87RoundingMode, apply_12540_callsite_2712ec};

/// Selector inputs in the order observed at original table bases
/// `0x520308`, `0x5202F8`, `0x5202E8`, `0x520338`, `0x520328`, `0x520318`,
/// and `0x5203B8`. Each pair is indexed by the original derived 0/1 selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectionOffsetInput {
    pub actor_key: u32,
    pub local_state_204: i32,
    pub table_flag_bit_0: bool,
    pub selectors_by_gate: [[u32; 2]; 7],
}

/// Typed boundary corresponding to the repeated `0x59D20(actor, selector)` calls.
pub trait OwnerSlotProbe {
    fn owner_has_either_slot(&mut self, actor_key: u32, selector: u32) -> bool;
}

/// Raw fields feeding the observed `0x26CE20` -> `0x12540` caller chain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControllerScalarInput {
    pub actor_key: u32,
    pub local_state_204: i32,
    pub table_flag_bit_0: bool,
    pub selectors_by_gate: [[u32; 2]; 7],
    pub context_value_70: f32,
    pub actor_value_9e0: f32,
    pub rounding_mode: X87RoundingMode,
}

/// Evaluate the measured selector priority and return constant from `0x26CE20`.
///
/// The seventh probe is always executed after the first six fail, but its result
/// is ignored before the original returns `180.0`.
pub fn direction_offset_26ce20(
    input: DirectionOffsetInput,
    probe: &mut impl OwnerSlotProbe,
) -> f32 {
    let selector_index = usize::from(input.local_state_204 == -1 || input.table_flag_bit_0);
    let selectors = input.selectors_by_gate.map(|pair| pair[selector_index]);
    let results = [-90.0, 90.0, 180.0, -90.0, 90.0, 180.0];

    for (selector, result) in selectors.iter().zip(results) {
        if probe.owner_has_either_slot(input.actor_key, *selector) {
            return result;
        }
    }

    let _ = probe.owner_has_either_slot(input.actor_key, selectors[6]);
    180.0
}

/// Run the measured scalar dataflow at `0x2712D0..0x2712EC` through its typed
/// selector and floating-point helper boundaries.
///
/// Field labels stay offset-based. The returned `f64` is the original x87
/// result before the caller's observed stores to two f32 stack temporaries.
pub fn controller_scalar_2712ec(
    input: ControllerScalarInput,
    probe: &mut impl OwnerSlotProbe,
) -> Option<f64> {
    let offset = direction_offset_26ce20(
        DirectionOffsetInput {
            actor_key: input.actor_key,
            local_state_204: input.local_state_204,
            table_flag_bit_0: input.table_flag_bit_0,
            selectors_by_gate: input.selectors_by_gate,
        },
        probe,
    );
    let adjusted_value = offset + input.context_value_70;
    let computed_arg1 = input.actor_value_9e0 - adjusted_value;
    apply_12540_callsite_2712ec(computed_arg1, input.rounding_mode)
}

#[cfg(test)]
mod tests {
    use super::{
        ControllerScalarInput, DirectionOffsetInput, OwnerSlotProbe, controller_scalar_2712ec,
        direction_offset_26ce20,
    };
    use crate::rea_controller_value_12540::X87RoundingMode;

    #[derive(Default)]
    struct Probe {
        calls: Vec<(u32, u32)>,
        hit: Option<u32>,
    }

    impl OwnerSlotProbe for Probe {
        fn owner_has_either_slot(&mut self, actor_key: u32, selector: u32) -> bool {
            self.calls.push((actor_key, selector));
            self.hit == Some(selector)
        }
    }

    fn selectors() -> [[u32; 2]; 7] {
        [
            [10, 110],
            [20, 120],
            [30, 130],
            [40, 140],
            [50, 150],
            [60, 160],
            [70, 170],
        ]
    }

    #[test]
    fn selects_the_first_matching_gate_and_stops_probing() {
        let mut probe = Probe {
            hit: Some(20),
            ..Probe::default()
        };
        let offset = direction_offset_26ce20(
            DirectionOffsetInput {
                actor_key: 7,
                local_state_204: 3,
                table_flag_bit_0: false,
                selectors_by_gate: selectors(),
            },
            &mut probe,
        );
        assert_eq!(offset, 90.0);
        assert_eq!(probe.calls, [(7, 10), (7, 20)]);
    }

    #[test]
    fn uses_second_selector_column_and_calls_seventh_probe_on_fallback() {
        let mut probe = Probe::default();
        let offset = direction_offset_26ce20(
            DirectionOffsetInput {
                actor_key: 8,
                local_state_204: 0,
                table_flag_bit_0: true,
                selectors_by_gate: selectors(),
            },
            &mut probe,
        );
        assert_eq!(offset, 180.0);
        assert_eq!(
            probe.calls,
            (110..=170)
                .step_by(10)
                .map(|id| (8, id))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn controller_scalar_composes_the_observed_selector_and_value_calls() {
        let mut probe = Probe::default();
        let result = controller_scalar_2712ec(
            ControllerScalarInput {
                actor_key: 9,
                local_state_204: 2,
                table_flag_bit_0: true,
                selectors_by_gate: selectors(),
                context_value_70: 10.0,
                actor_value_9e0: 200.0,
                rounding_mode: X87RoundingMode::TowardZero,
            },
            &mut probe,
        );
        assert_eq!(result, Some(10.0));
        assert_eq!(probe.calls.len(), 7);
        assert_eq!(probe.calls[0], (9, 110));
    }
}
