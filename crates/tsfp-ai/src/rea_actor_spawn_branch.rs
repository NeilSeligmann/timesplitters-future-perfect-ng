//! Raw terminal branch selection observed in the 0x18AF40 AI routine.
//!
//! These are call-site values, not semantic effect or audio types. The
//! allocation and request services remain explicit integration boundaries.

use crate::rea_candidate_slot::{CandidateSlotRaw, CandidateSlotSelection, select_candidate_slot};
use crate::rea_request_gate::{RawRequestIdGate, route_45820};
use crate::rea_request_pool::RawRequestPool;
use crate::rea_sound_request::SoundRequestRaw;

/// The path chosen by the early 0x18AF40 branch after its two raw field checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorSpawnSpatialPath {
    /// The body joins the path that constructs/transforms local matrix vectors.
    TransformHelpers,
    /// A non-null third argument selects the bypass/scratch path.
    Arg3Bypass,
}

impl ActorSpawnSpatialPath {
    /// Whether this path prepares the transform/query helpers. This is
    /// independent of the later raw stack word tested at 0x18B14D.
    pub const fn uses_transform_helpers(self) -> bool {
        matches!(self, Self::TransformHelpers)
    }
}

/// Select the observed later `0x18B4C8` route from its raw stack word.
///
/// REA stack accounting shows this is function-local `S0+0x28`; it is not the
/// `0x3F133333` float stored at `S0+0x40`, and its runtime value is unresolved.
/// Keep it as explicit caller input instead of deriving it from the earlier
/// arg3/spatial path.
pub const fn uses_type_11_terminal_route(raw_gate_word: u32) -> bool {
    raw_gate_word != 0
}

/// Ordered route after the raw gate load and before the candidate loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorSpawnLoopRoute {
    Type11Terminal,
    TerminalWithoutCandidateScan,
    CandidateScan,
}

/// Apply the observed `0x18B174` gate followed by the independent arg+0x14
/// test at `0x18B17A`. The stack word is explicitly supplied because its
/// runtime contents are unresolved; it is not inferred from the spatial path.
pub const fn actor_spawn_loop_route(
    raw_gate_word: u32,
    arg_14_nonzero: bool,
) -> ActorSpawnLoopRoute {
    if uses_type_11_terminal_route(raw_gate_word) {
        ActorSpawnLoopRoute::Type11Terminal
    } else if arg_14_nonzero {
        ActorSpawnLoopRoute::TerminalWithoutCandidateScan
    } else {
        ActorSpawnLoopRoute::CandidateScan
    }
}

/// Apply the observed COMISS/JNBE/JBE checks against raw arg2 fields `+0x8C`
/// and `+0x90`, followed by the arg3 null guard. The source compares the
/// constant at `0x475DBC` (`0x3A83126F`) against each field. `partial_cmp`
/// preserves the native unordered behavior: JNBE is false and JBE is true for
/// NaN comparisons, so unordered inputs do not spuriously select arg3 bypass.
/// Verification: Tier 0 synthetic smoke test; no live actor argument adapter.
pub fn actor_spawn_spatial_path(
    arg2_field_8c: f32,
    arg2_field_90: f32,
    arg3_is_non_null: bool,
) -> ActorSpawnSpatialPath {
    const RAW_THRESHOLD: f32 = f32::from_bits(0x3A83_126F);

    let first_jnbe =
        RAW_THRESHOLD.partial_cmp(&arg2_field_8c) == Some(core::cmp::Ordering::Greater);
    let second_jbe = matches!(
        RAW_THRESHOLD.partial_cmp(&arg2_field_90),
        Some(core::cmp::Ordering::Less | core::cmp::Ordering::Equal) | None
    );

    if (first_jnbe || !second_jbe) && arg3_is_non_null {
        ActorSpawnSpatialPath::Arg3Bypass
    } else {
        ActorSpawnSpatialPath::TransformHelpers
    }
}

/// Derive the three raw output components written by the `0xEA7A0` row
/// initializer at offsets `+0x1A0..+0x1A8`.
///
/// The ordinary arm adds the incoming component at `+0x64/+0x68/+0x6C` to
/// the corresponding local component at `+0x1C/+0x20/+0x24`. Raw row type
/// `0x2A` first multiplies the incoming component by the observed binary32
/// constant at `0x47CFF4` (`30.0`). Coordinate and gameplay meanings remain
/// unknown. The two branches are kept separate to preserve the measured
/// instruction sequence.
pub fn derive_effect_row_position(row_type: u32, incoming: [f32; 3], local: [f32; 3]) -> [f32; 3] {
    if row_type == 0x2A {
        let scale = f32::from_bits(0x41F0_0000);
        [
            incoming[0] * scale + local[0],
            incoming[1] * scale + local[1],
            incoming[2] * scale + local[2],
        ]
    } else {
        [
            incoming[0] + local[0],
            incoming[1] + local[1],
            incoming[2] + local[2],
        ]
    }
}

/// Raw vector-copy regions populated by `0xEA9C0` before `0xEA7A0` runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RawEffectRowVectorCopies {
    /// Three repeats of EA9C0's second-argument dwords for row `+0x1C..+0x3C`.
    pub fields_1c_to_3c: [u32; 9],
    /// Three repeats of EA9C0's third-argument dwords for row `+0x64..+0x84`.
    pub fields_64_to_84: [u32; 9],
}

/// Preserve the two input triples in the repeated row regions written by
/// `0xEA9C0`. Values remain raw bits; no coordinate names are assigned.
pub const fn copy_effect_row_vectors(
    second_argument: [u32; 3],
    third_argument: [u32; 3],
) -> RawEffectRowVectorCopies {
    RawEffectRowVectorCopies {
        fields_1c_to_3c: [
            second_argument[0],
            second_argument[1],
            second_argument[2],
            second_argument[0],
            second_argument[1],
            second_argument[2],
            second_argument[0],
            second_argument[1],
            second_argument[2],
        ],
        fields_64_to_84: [
            third_argument[0],
            third_argument[1],
            third_argument[2],
            third_argument[0],
            third_argument[1],
            third_argument[2],
            third_argument[0],
            third_argument[1],
            third_argument[2],
        ],
    }
}

/// Rust-owned composition of the measured vector-copy, seed, and position
/// leaves. This is the bounded `0xEA9C0 -> 0xEA7A0` data slice, not a full row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectRowDataSlice {
    pub vectors: RawEffectRowVectorCopies,
    pub seed: RawEffectRowSeed,
    pub derived_components_1a0: [f32; 3],
}

/// Compose the raw `0xEA9C0` vector copies with the `0xEA7A0` seed and derived
/// component calculations. Float inputs are supplied as original raw bits.
pub fn prepare_effect_row_data(
    row_type: u32,
    second_argument: [u32; 3],
    third_argument: [u32; 3],
    row_field_100: u32,
    context_flags_0c: u8,
) -> EffectRowDataSlice {
    let vectors = copy_effect_row_vectors(second_argument, third_argument);
    let local = [
        f32::from_bits(vectors.fields_1c_to_3c[0]),
        f32::from_bits(vectors.fields_1c_to_3c[1]),
        f32::from_bits(vectors.fields_1c_to_3c[2]),
    ];
    let incoming = [
        f32::from_bits(vectors.fields_64_to_84[0]),
        f32::from_bits(vectors.fields_64_to_84[1]),
        f32::from_bits(vectors.fields_64_to_84[2]),
    ];

    EffectRowDataSlice {
        seed: seed_effect_row(vectors.fields_1c_to_3c, row_field_100, context_flags_0c),
        vectors,
        derived_components_1a0: derive_effect_row_position(row_type, incoming, local),
    }
}

/// The two raw fields consumed by the candidate-context branch in `0xEA7A0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RawEffectRowCandidateContext {
    pub field_4: u32,
    pub field_8: i32,
}

/// Raw result of the candidate-context branch before it writes row `+0x19C`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectRowCandidateRoute {
    /// No context was present; the initialized row field remains zero.
    KeepInitializedZero,
    /// The signed context index was not below the global count; use context `+4`.
    UseContextField4 { value: u32 },
    /// The index is in range; pass this table address and ordinal to `0xCFEC0`.
    SelectFromTable {
        table_address: u32,
        selector: u32,
        selection: CandidateSlotSelection,
    },
}

/// Resolve the measured `0xEA7A0` -> `0xCFEC0` candidate-source branch.
///
/// `context` represents a snapshot of row `+0x104`'s `+4/+8` fields. The
/// global table address is opaque: this function does not dereference guest
/// memory. `row_flags_0c` is tested through DH; bit `0x4000` selects ordinal 1,
/// otherwise bit `0x8000` selects ordinal 2, otherwise ordinal 0.
pub fn select_effect_row_candidate_source(
    context: Option<RawEffectRowCandidateContext>,
    row_flags_0c: u32,
    global_count: i32,
    global_table_base: u32,
    slots: [CandidateSlotRaw; 6],
) -> EffectRowCandidateRoute {
    let Some(context) = context else {
        return EffectRowCandidateRoute::KeepInitializedZero;
    };

    if context.field_8 >= global_count {
        return EffectRowCandidateRoute::UseContextField4 {
            value: context.field_4,
        };
    }

    let selector = if row_flags_0c & 0x4000 != 0 {
        1
    } else if row_flags_0c & 0x8000 != 0 {
        2
    } else {
        0
    };
    let table_address =
        global_table_base.wrapping_add((context.field_8 as u32).wrapping_mul(0x1584));
    let selection = select_candidate_slot(slots, selector);

    EffectRowCandidateRoute::SelectFromTable {
        table_address,
        selector,
        selection,
    }
}

/// The final strict distance gate observed in the `0xE91F0` actor-list pass.
///
/// `use_900_limit` is the raw nonzero/zero selector established by preceding
/// checks. The original uses `COMISS` followed by `JBE` to skip on equal,
/// greater, or unordered values, so NaN distances are rejected here.
pub fn actor_row_distance_inside_limit(
    row_components: [f32; 3],
    actor_components: [f32; 3],
    use_900_limit: bool,
) -> bool {
    let delta_middle = actor_components[1] - row_components[1];
    let delta_first = actor_components[0] - row_components[0];
    let delta_last = actor_components[2] - row_components[2];
    let squared_distance =
        (delta_last * delta_last + delta_middle * delta_middle) + delta_first * delta_first;
    let limit = f32::from_bits(if use_900_limit {
        0x4461_0000
    } else {
        0x4361_0000
    });

    limit.partial_cmp(&squared_distance) == Some(core::cmp::Ordering::Greater)
}

/// The subset of raw `0xEA7A0` destination fields initialized from zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RawEffectRowZeroFields {
    pub field_f4: u32,
    pub field_154: u32,
    pub field_178: u32,
    pub field_18c: u32,
    pub field_190: u32,
    pub field_19c: u32,
    pub field_1b4: u32,
    pub field_180: u32,
    pub field_184: u32,
    pub field_188: u32,
}

/// Measured raw row fields written by the `0xEA7A0` initializer.
///
/// The nine input dwords are copied in order from input offsets `+0x1C` through
/// `+0x3C` to row offsets `+0x40` through `+0x60`. Other names retain the
/// destination offsets because their semantic roles are unknown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawEffectRowSeed {
    pub copied_input_words: [u32; 9],
    pub field_14c: u32,
    pub field_18: u16,
    pub field_15c: i32,
    pub field_1b0: i32,
    pub field_1b8: f32,
    pub field_1bc: u32,
    pub zero_fields: RawEffectRowZeroFields,
}

/// Build the bounded row seed observed at `0xEA7A0`.
///
/// `row_field_100` is the raw input dword at `+0x100`; `context_flags_0c` is
/// the raw byte tested at `[0x7B0C7C]+0x0C`. No record or global is read here.
pub fn seed_effect_row(
    copied_input_words: [u32; 9],
    row_field_100: u32,
    context_flags_0c: u8,
) -> RawEffectRowSeed {
    RawEffectRowSeed {
        copied_input_words,
        field_14c: 1,
        field_18: 0,
        field_15c: -1,
        field_1b0: -1,
        field_1b8: 1.0,
        field_1bc: u32::from(row_field_100 == 1 && context_flags_0c & 0x20 != 0),
        zero_fields: RawEffectRowZeroFields::default(),
    }
}

/// The call-site choices made after reading the raw actor word at `+0x94`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorSpawnTerminalPlan {
    /// Selector passed to 0xDFFA0.
    pub dffa0_selector: u32,
    /// Raw float bits passed to 0xDFFA0 as its second call argument.
    pub dffa0_float_argument_bits: u32,
    /// Raw first stack argument passed to 0xEA9C0.
    pub ea9c0_argument_1: u32,
    /// Raw fourth stack float passed to 0xEA9C0; its semantic role is unknown.
    pub ea9c0_fourth_argument_bits: u32,
    /// Whether this arm constructs and submits the separate 0x45390 request.
    pub submit_sound_request: bool,
    /// Raw float bits written to the selected loop slot at `[EBX+index*4+0x84]`.
    pub loop_slot_value_bits: u32,
}

/// Translate the measured `[actor + 0x94] == 0x482` terminal branch into its
/// raw downstream call arguments. No actor memory or guest services are read.
pub const fn actor_spawn_terminal_plan(selector_word_94: u32) -> ActorSpawnTerminalPlan {
    if selector_word_94 == 0x482 {
        ActorSpawnTerminalPlan {
            dffa0_selector: 2,
            dffa0_float_argument_bits: 0x4000_0000,
            ea9c0_argument_1: 30,
            ea9c0_fourth_argument_bits: 0x4040_0000,
            submit_sound_request: true,
            loop_slot_value_bits: 0x3D4C_CCCD,
        }
    } else {
        ActorSpawnTerminalPlan {
            dffa0_selector: 0,
            dffa0_float_argument_bits: 0x4000_0000,
            ea9c0_argument_1: 4,
            ea9c0_fourth_argument_bits: 0x4040_0000,
            submit_sound_request: true,
            loop_slot_value_bits: 0x3D4C_CCCD,
        }
    }
}

/// Rust-owned state prepared for the measured terminal branch of `0x18AF40`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActorSpawnTerminalDecision {
    pub plan: ActorSpawnTerminalPlan,
    /// Raw request record constructed after either measured arm of the
    /// `[actor+0x94] == 0x482` split; this does not claim audible playback.
    pub request: Option<SoundRequestRaw>,
}

impl ActorSpawnTerminalDecision {
    /// Route the optional Rust-owned `0x4C4` request through the existing raw
    /// `0x45820` manager contract. The request ID is the measured `0x4C4`.
    /// `None` means this terminal arm creates no request; `Some` is the raw
    /// manager return, not evidence of audible playback.
    #[allow(clippy::too_many_arguments)]
    pub fn route_request_45820(
        &mut self,
        id_resolves: bool,
        mode_flags: u32,
        table_flags: u32,
        current_mode: u32,
        gate: &mut RawRequestIdGate,
        pool: &mut RawRequestPool,
        process_28240: impl FnMut(&[u32; 12], u32) -> u32,
    ) -> Option<i32> {
        self.request.as_mut().map(|request| {
            route_45820(
                request.words_mut(),
                0x4C4,
                id_resolves,
                mode_flags,
                table_flags,
                current_mode,
                gate,
                pool,
                process_28240,
            )
        })
    }
}

/// Compose the selector decision with the call-site request writes.
///
/// `request_values` are the three local floats copied to request offsets
/// `+0x1c/+0x20/+0x24` on either bounded arm. Their coordinate-space meaning
/// remains unknown and is supplied by the caller. Effect allocation
/// (`0xDFFA0`/`0xEA9C0`) and request routing
/// (`0x45820`) remain separate typed service boundaries.
pub fn actor_spawn_terminal_decision(
    selector_word_94: u32,
    request_values: [f32; 3],
) -> ActorSpawnTerminalDecision {
    let plan = actor_spawn_terminal_plan(selector_word_94);
    let request = plan
        .submit_sound_request
        .then(|| SoundRequestRaw::initialized().with_id_4c4(request_values));
    ActorSpawnTerminalDecision { plan, request }
}

#[cfg(test)]
mod tests {
    use super::{
        ActorSpawnSpatialPath, CandidateSlotRaw, CandidateSlotSelection, EffectRowCandidateRoute,
        RawEffectRowCandidateContext, actor_row_distance_inside_limit, actor_spawn_spatial_path,
        actor_spawn_terminal_decision, copy_effect_row_vectors, derive_effect_row_position,
        prepare_effect_row_data, seed_effect_row, select_effect_row_candidate_source,
    };
    use crate::rea_request_gate::RawRequestIdGate;
    use crate::rea_request_pool::RawRequestPool;

    #[test]
    fn request_4c4_uses_rust_owned_45820_route() {
        let mut decision = actor_spawn_terminal_decision(0x482, [1.0, 2.0, 3.0]);
        let mut gate = RawRequestIdGate::default();
        let mut pool = RawRequestPool::default();
        let mut calls = 0;
        let result =
            decision.route_request_45820(true, 0, 0, 9, &mut gate, &mut pool, |request, flags| {
                calls += 1;
                assert_eq!(request[0], 0x4C4);
                assert_eq!(
                    request[7..10],
                    [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()]
                );
                assert_eq!(flags, 0);
                0
            });

        assert_eq!(result, Some(0));
        assert_eq!(calls, 1);
        assert_eq!(gate.ids[0], 0x4C4);
        assert_eq!(gate.len, 1);
    }

    #[test]
    fn spatial_branch_matches_ordered_threshold_and_arg3_guard() {
        let below = f32::from_bits(0x3A83_126E);
        let above = f32::from_bits(0x3A83_1270);
        let unordered = f32::NAN;

        assert_eq!(
            actor_spawn_spatial_path(below, above, true),
            ActorSpawnSpatialPath::Arg3Bypass
        );
        assert_eq!(
            actor_spawn_spatial_path(above, below, true),
            ActorSpawnSpatialPath::Arg3Bypass
        );
        assert_eq!(
            actor_spawn_spatial_path(above, above, true),
            ActorSpawnSpatialPath::TransformHelpers
        );
        assert_eq!(
            actor_spawn_spatial_path(below, below, false),
            ActorSpawnSpatialPath::TransformHelpers
        );
        assert_eq!(
            actor_spawn_spatial_path(unordered, above, true),
            ActorSpawnSpatialPath::TransformHelpers
        );
    }

    #[test]
    fn non_equal_selector_arm_also_routes_request_4c4() {
        let mut decision = actor_spawn_terminal_decision(0x481, [1.0, 2.0, 3.0]);
        assert_eq!(decision.plan.dffa0_selector, 0);
        assert_eq!(decision.plan.ea9c0_argument_1, 4);
        assert_eq!(decision.plan.ea9c0_fourth_argument_bits, 0x4040_0000);
        assert_eq!(decision.plan.loop_slot_value_bits, 0x3D4C_CCCD);
        assert!(decision.plan.submit_sound_request);
        assert_eq!(decision.request.unwrap().words()[0], 0x4C4);
        let mut gate = RawRequestIdGate::default();
        let mut pool = RawRequestPool::default();
        let result =
            decision.route_request_45820(true, 0, 0, 9, &mut gate, &mut pool, |request, _| {
                assert_eq!(request[0], 0x4C4);
                0
            });
        assert_eq!(result, Some(0));
        assert_eq!(gate.ids[0], 0x4C4);
        assert_eq!(gate.len, 1);
    }

    #[test]
    fn spatial_path_is_independent_of_the_later_raw_gate_word() {
        assert_eq!(
            ActorSpawnSpatialPath::TransformHelpers.uses_transform_helpers(),
            true
        );
        assert!(!ActorSpawnSpatialPath::Arg3Bypass.uses_transform_helpers());
        assert!(!super::uses_type_11_terminal_route(0));
        assert!(super::uses_type_11_terminal_route(2));
        assert_eq!(
            super::actor_spawn_loop_route(2, false),
            super::ActorSpawnLoopRoute::Type11Terminal
        );
        assert_eq!(
            super::actor_spawn_loop_route(0, true),
            super::ActorSpawnLoopRoute::TerminalWithoutCandidateScan
        );
        assert_eq!(
            super::actor_spawn_loop_route(0, false),
            super::ActorSpawnLoopRoute::CandidateScan
        );
    }

    #[test]
    fn effect_row_position_uses_plain_component_addition_for_other_types() {
        assert_eq!(
            derive_effect_row_position(4, [1.0, -2.0, 3.0], [4.0, 5.0, 6.0]),
            [5.0, 3.0, 9.0]
        );
    }

    #[test]
    fn effect_row_type_2a_scales_incoming_components_by_thirty_before_addition() {
        assert_eq!(
            derive_effect_row_position(0x2A, [1.5, -2.0, 0.25], [0.5, 1.0, -0.25]),
            [45.5, -59.0, 7.25]
        );
    }

    #[test]
    fn effect_row_seed_copies_raw_words_and_initializes_measured_defaults() {
        let seed = seed_effect_row([1, 2, 3, 4, 5, 6, 7, 8, 9], 0, 0);

        assert_eq!(seed.copied_input_words, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(seed.field_14c, 1);
        assert_eq!(seed.field_18, 0);
        assert_eq!(seed.field_15c, -1);
        assert_eq!(seed.field_1b0, -1);
        assert_eq!(seed.field_1b8, 1.0);
        assert_eq!(seed.field_1bc, 0);
        assert_eq!(seed.zero_fields, Default::default());
    }

    #[test]
    fn effect_row_seed_sets_field_1bc_only_for_exact_mode_and_flag() {
        assert_eq!(seed_effect_row([0; 9], 1, 0x20).field_1bc, 1);
        assert_eq!(seed_effect_row([0; 9], 2, 0x20).field_1bc, 0);
        assert_eq!(seed_effect_row([0; 9], 1, 0x00).field_1bc, 0);
    }

    #[test]
    fn effect_row_vector_copies_repeat_each_input_triple_three_times() {
        let copies = copy_effect_row_vectors([1, 2, 3], [4, 5, 6]);

        assert_eq!(copies.fields_1c_to_3c, [1, 2, 3, 1, 2, 3, 1, 2, 3]);
        assert_eq!(copies.fields_64_to_84, [4, 5, 6, 4, 5, 6, 4, 5, 6]);
    }

    #[test]
    fn effect_row_data_slice_composes_vector_copies_seed_and_position() {
        let data = prepare_effect_row_data(
            4,
            [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()],
            [4.0f32.to_bits(), 5.0f32.to_bits(), 6.0f32.to_bits()],
            1,
            0x20,
        );

        assert_eq!(data.seed.copied_input_words, data.vectors.fields_1c_to_3c);
        assert_eq!(data.seed.field_1bc, 1);
        assert_eq!(data.derived_components_1a0, [5.0, 7.0, 9.0]);
    }

    #[test]
    fn effect_row_data_slice_uses_raw_type_2a_scale_branch() {
        let data = prepare_effect_row_data(
            0x2A,
            [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()],
            [4.0f32.to_bits(), 5.0f32.to_bits(), 6.0f32.to_bits()],
            0,
            0,
        );

        assert_eq!(data.derived_components_1a0, [121.0, 152.0, 183.0]);
    }

    #[test]
    fn effect_row_candidate_route_keeps_zero_without_context_and_uses_context_field_at_count() {
        let slots = [CandidateSlotRaw::default(); 6];
        assert_eq!(
            select_effect_row_candidate_source(None, 0, 2, 0x1000, slots),
            EffectRowCandidateRoute::KeepInitializedZero
        );
        assert_eq!(
            select_effect_row_candidate_source(
                Some(RawEffectRowCandidateContext {
                    field_4: 0x1234,
                    field_8: 2,
                }),
                0,
                2,
                0x1000,
                slots,
            ),
            EffectRowCandidateRoute::UseContextField4 { value: 0x1234 }
        );
    }

    #[test]
    fn effect_row_candidate_route_uses_signed_index_and_flag_selector_priority() {
        let slots = [
            CandidateSlotRaw::default(),
            CandidateSlotRaw {
                base_word: 1,
                word_0c: 1,
            },
            CandidateSlotRaw::default(),
            CandidateSlotRaw {
                base_word: 1,
                word_0c: 1,
            },
            CandidateSlotRaw::default(),
            CandidateSlotRaw {
                base_word: 1,
                word_0c: 1,
            },
        ];
        let route = select_effect_row_candidate_source(
            Some(RawEffectRowCandidateContext {
                field_4: 0,
                field_8: 1,
            }),
            0xC000,
            2,
            0x1000,
            slots,
        );
        assert_eq!(
            route,
            EffectRowCandidateRoute::SelectFromTable {
                table_address: 0x2584,
                selector: 1,
                selection: CandidateSlotSelection::Slot(3),
            }
        );

        let negative_index = select_effect_row_candidate_source(
            Some(RawEffectRowCandidateContext {
                field_4: 0,
                field_8: -1,
            }),
            0x8000,
            2,
            0x1000,
            slots,
        );
        assert!(matches!(
            negative_index,
            EffectRowCandidateRoute::SelectFromTable { selector: 2, .. }
        ));
    }

    #[test]
    fn actor_row_distance_gate_uses_the_selected_strict_limit() {
        assert!(actor_row_distance_inside_limit(
            [0.0; 3],
            [20.0, 0.0, 0.0],
            true
        ));
        assert!(!actor_row_distance_inside_limit(
            [0.0; 3],
            [20.0, 0.0, 0.0],
            false
        ));
        assert!(!actor_row_distance_inside_limit(
            [0.0; 3],
            [15.0, 0.0, 0.0],
            false
        ));
        assert!(actor_row_distance_inside_limit(
            [0.0; 3],
            [14.0, 0.0, 0.0],
            false
        ));
    }

    #[test]
    fn actor_row_distance_gate_rejects_unordered_float_comparisons() {
        assert!(!actor_row_distance_inside_limit(
            [f32::NAN, 0.0, 0.0],
            [0.0; 3],
            true
        ));
    }
}
