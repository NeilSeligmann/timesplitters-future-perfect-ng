// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed raw-field leaves called from original 0x26EFC0.

use crate::point_tables::owner_has_either_slot;

/// The controller record values consumed by the 0x59D50..0x59DD0 leaf group.
/// Meanings are intentionally not inferred from these offsets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptFieldsRaw {
    pub field_d4: u32,
    pub field_d8: u32,
    pub field_124: u32,
    pub field_12c: u32,
    pub field_17c: u32,
}

/// Observed table-entry value at byte offset +0x18.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScriptTableEntryRaw {
    pub field_18: f32,
}

/// Raw fields read and written by original 0x17570.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptPairFieldsRaw {
    pub field_aa8: u32,
    pub field_aac: u32,
    pub field_10: u32,
    pub field_14: u32,
    pub field_18: u32,
    pub field_1c: u32,
}

/// Apply original 0x17570's guarded raw-pair write.
/// The global byte chooses the `+0x10/+0x14` or `+0x18/+0x1c` gate pair.
pub fn update_pair_17570(
    fields: &mut ScriptPairFieldsRaw,
    global_mode_byte: u8,
    requested: [u32; 2],
) {
    if fields.field_aa8 == 1 && fields.field_aac == 0 {
        let gate = if global_mode_byte == 0x0a {
            fields.field_10
        } else {
            fields.field_18
        };
        if gate & 1 == 0 {
            return;
        }
    }

    fields.field_aa8 = requested[0];
    fields.field_aac = requested[1];
}

/// Read the raw float at table-entry offset +0x18 selected by field +0xD8
/// (original 0x59D50). `None` represents an absent/out-of-range Rust view.
pub fn table_value_d8(fields: ScriptFieldsRaw, table: &[ScriptTableEntryRaw]) -> Option<f32> {
    table
        .get(fields.field_d8 as usize)
        .map(|entry| entry.field_18)
}

/// Compare raw field +0xD4 with the requested value (original 0x59D70).
pub const fn field_d4_matches(fields: ScriptFieldsRaw, requested: u32) -> bool {
    fields.field_d4 == requested
}

/// Compare raw field +0x12C with the requested value (original 0x59D90).
pub const fn field_12c_matches(fields: ScriptFieldsRaw, requested: u32) -> bool {
    fields.field_12c == requested
}

/// Return raw field +0x124 only when +0xD4 equals the request (0x59DB0).
pub const fn field_124_when_d4_matches(fields: ScriptFieldsRaw, requested: u32) -> u32 {
    if field_d4_matches(fields, requested) {
        fields.field_124
    } else {
        0
    }
}

/// Original 0x59DD0 nullable selector: null maps to zero; otherwise return
/// field +0x124 when +0x12C is zero, or field +0x17C when it is nonzero.
pub const fn select_124_or_17c(fields: Option<ScriptFieldsRaw>) -> u32 {
    match fields {
        None => 0,
        Some(fields) if fields.field_12c == 0 => fields.field_124,
        Some(fields) => fields.field_17c,
    }
}

/// Raw selected-object prefix fields read and written by 0x26EFC0.
///
/// The field meanings are not established; names retain their original byte
/// offsets until the actor/world owner is connected.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScriptControllerEntryRaw {
    pub field_20: u32,
    pub field_9d0: i32,
    pub field_9d4: u32,
    pub field_9d8: f32,
    pub field_a90: f32,
    pub field_2c: u8,
    pub field_28: u32,
}

/// Raw local-state values dispatched by the `0x26EFC0` jump table.
/// Semantic state names are not established; variants preserve the observed
/// numeric selectors and intentionally do not assign gameplay meanings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptControllerLocalState {
    State0,
    State1,
    State2,
    State3,
    State4,
    State5,
    State6,
    State7,
    State8,
}

/// Decode the unsigned `+0x20` jump-table selector. Values above 8 take the
/// controller's default exit; states 0..=8 enter their corresponding handler.
pub const fn decode_controller_local_state(raw: u32) -> Option<ScriptControllerLocalState> {
    use ScriptControllerLocalState::*;
    match raw {
        0 => Some(State0),
        1 => Some(State1),
        2 => Some(State2),
        3 => Some(State3),
        4 => Some(State4),
        5 => Some(State5),
        6 => Some(State6),
        7 => Some(State7),
        8 => Some(State8),
        _ => None,
    }
}

/// Explicit global snapshot consumed by one 0x26EFC0 tick. Values are raw
/// float inputs as reconstructed from the global integer conversions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScriptControllerGlobals {
    pub divisor_4e8cec: i32,
    pub scale_4e794c: i32,
}

/// Typed snapshots needed by the bounded state-4 selector scan. Descriptor
/// lookup and table reads happen at the caller boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase4SelectorInputs {
    pub candidate_fields: ScriptFieldsRaw,
    pub selector_pairs: [[u32; 2]; 3],
    pub descriptor_gate: [bool; 3],
}

/// Result of composing the entry transition with the independently bounded
/// state-4 selector and tail. Missing inputs mean the tick took another state
/// path or the corresponding phase snapshot was not available.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptControllerTickInput {
    pub globals: ScriptControllerGlobals,
    pub state4_selector: Option<ScriptPhase4SelectorInputs>,
    pub state4_tail: Option<ScriptPhase4TailInputs>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptControllerTickResult {
    pub state4_selector: Option<ScriptPhase4SelectorPlan>,
    pub state4_tail: Option<ScriptPhase4TailPlan>,
}

/// Apply the measured 0x26EFC0 entry prefix, then optional state-4 selector and
/// terminal snapshots in their observed order. Call resolution, phase dispatch,
/// other states and external table/global ownership remain outside this bounded
/// composition.
pub fn apply_controller_tick_prefix(
    fields: &mut ScriptControllerEntryRaw,
    input: ScriptControllerTickInput,
) -> ScriptControllerTickResult {
    apply_controller_entry_prefix(
        fields,
        input.globals.divisor_4e8cec,
        input.globals.scale_4e794c,
    );
    ScriptControllerTickResult {
        state4_selector: input.state4_selector.map(|snapshot| {
            script_phase4_selector_plan(
                snapshot.candidate_fields,
                snapshot.selector_pairs,
                snapshot.descriptor_gate,
            )
        }),
        state4_tail: input.state4_tail.map(script_phase4_tail_plan),
    }
}

/// Apply the bounded scalar setup prefix observed at 0x26EFC0.
///
/// `divisor_4e8cec` and `scale_4e794c` are explicit snapshots of the two
/// integer globals converted by CVTSI2SS in the original. The 0x15E120 call
/// and object resolution remain caller-owned boundaries.
pub fn apply_controller_entry_prefix(
    fields: &mut ScriptControllerEntryRaw,
    divisor_4e8cec: i32,
    scale_4e794c: i32,
) {
    if fields.field_9d0 >= 0 {
        let requested = fields.field_9d0 as u32;
        if requested != fields.field_20 {
            fields.field_9d8 = 0.0;
            fields.field_9d4 = fields.field_20;
            fields.field_20 = requested;
        }
        fields.field_9d0 = -1;
    }

    if (fields.field_2c as i8) < 0 {
        let increment = (1.0 / divisor_4e8cec as f32) * scale_4e794c as f32;
        fields.field_a90 += increment;
    } else {
        fields.field_a90 = 0.0;
    }
    fields.field_28 |= 0x4000;
}

/// The bounded fast-return outcomes from the entry of 0x15E120, called by
/// 0x26EFC0. `Continue` means the awareness search needs the remaining helper
/// contract; it is not a rejection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptAwarenessFastPath {
    Return(u32),
    Continue,
}

/// Raw controller fields used by the measured 0x15E120 early paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptAwarenessPrefixRaw {
    pub controller_field_0: u32,
    pub controller_field_4: u32,
    pub controller_field_adc: u32,
}

/// Resolve only the fast-return paths observed at 0x15E120 entry.
///
/// `target_kind` is absent when the caller cannot resolve the non-null target
/// record; that case deliberately falls through to the full awareness service.
pub fn script_awareness_fast_path(
    global_7de458: u32,
    controller: ScriptAwarenessPrefixRaw,
    target_kind: Option<u32>,
) -> ScriptAwarenessFastPath {
    if global_7de458 & 0x4000_0000 != 0 || controller.controller_field_4 == 0 {
        return ScriptAwarenessFastPath::Return(0);
    }
    if controller.controller_field_4 == controller.controller_field_0 {
        return ScriptAwarenessFastPath::Return(1);
    }
    match target_kind {
        Some(8) | None => ScriptAwarenessFastPath::Continue,
        Some(_) => ScriptAwarenessFastPath::Return(controller.controller_field_adc),
    }
}

/// At most two raw candidate values retained by the 0x15E120 pair scan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptAwarenessPairCandidates {
    pub values: [u32; 2],
    pub count: usize,
}

/// The bounded raw loop at 0x277FE0, separated from guest globals and row
/// storage. The source byte, count gate, table limit, and candidate predicate
/// are supplied by the caller; their semantic names remain unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptAwarenessLookupOutcome<'a, T> {
    Disabled,
    CountSentinel,
    Matched(&'a T),
    Exhausted,
}

/// Resolve the raw selector at `[0x79094C]` through the bounded dispatch table
/// used by 0x2776F0. `None` covers selector values whose table entries select
/// the zero-return/default arm; no semantic type name is assigned to tags.
/// Verification: Tier 0 smoke test covers a recognized tag, aliases, and the
/// zero/default arms; no live table adapter.
pub fn script_awareness_expected_row_tag(global_selector: u32) -> Option<u32> {
    let Some(index) = global_selector.checked_sub(1) else {
        return None;
    };
    match index {
        0 | 16 => Some(0x4A42_5448),          // handler at 0x27772F
        3 => Some(0x4350_5441),               // handler at 0x277709
        4 | 8 | 13 | 80 => Some(0x5231_3130), // handler at 0x277755
        5 | 21 => Some(0x4143_484E),          // handler at 0x277742
        10 | 17 => Some(0x4854_4950),         // handler at 0x27771C
        _ => None,
    }
}

/// Reproduce the repeated raw `[controller+0x204]` descriptor gate in
/// 0x26EFC0. The all-ones sentinel bypasses descriptor lookup; otherwise bit 0
/// of the descriptor byte at `+0x0C` decides the branch.
/// Verification: Tier 0 smoke test covers the sentinel and raw bit behavior;
/// the live controller/table adapter remains unimplemented.
pub const fn script_descriptor_gate(controller_field_204: u32, descriptor_byte_0c: u8) -> bool {
    controller_field_204 == u32::MAX || descriptor_byte_0c & 1 != 0
}

/// Raw call arguments for the 0x26EFC0 mode-2 route to 0x613D0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptMode2SelectorCallPlan {
    pub table_slot: usize,
    pub selector: u32,
    /// Fixed second cdecl argument observed at the call site.
    pub ordinal: u32,
    /// Fixed third cdecl argument observed at the call site.
    pub mask: u32,
}

/// Prepare the measured 0x613D0 call from caller-supplied table values.
/// Descriptor lookup remains outside this function; it supplies byte `+0x0C`
/// only when `controller_field_204` is not the all-ones sentinel.
/// Verification: Tier 0 synthetic smoke test; no live actor/table adapter.
pub const fn script_mode2_selector_call_plan(
    mode: u32,
    controller_field_204: u32,
    descriptor_byte_0c: u8,
    selector_table: [u32; 2],
) -> Option<ScriptMode2SelectorCallPlan> {
    if mode != 2 {
        return None;
    }
    let table_slot = if script_descriptor_gate(controller_field_204, descriptor_byte_0c) {
        1
    } else {
        0
    };
    Some(ScriptMode2SelectorCallPlan {
        table_slot,
        selector: selector_table[table_slot],
        ordinal: 2,
        mask: 1,
    })
}

/// Raw post-state for the accepted, `EDI == 0` arm of controller state 2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScriptPhase2InitRaw {
    pub controller_field_684: u32,
    pub controller_field_9ac: u32,
    pub controller_field_9a8: u32,
    pub controller_field_b30_bits: u32,
}

/// Model the bounded state-2 stores after its selector checks succeed. A
/// nonnegative `+0xAC8` is copied to `+0x684`; otherwise the caller supplies
/// the raw fallback loaded through the current actor's `+0x7C/+0x678` chain.
/// Descriptor selection and the remaining `0x26F4CF` call setup stay outside.
/// Verification: Tier 0 composition test covers the fallback and nonnegative
/// source branches; no live controller adapter.
pub const fn initialize_script_phase2(
    controller_field_ac8: i32,
    fallback_678: u32,
) -> ScriptPhase2InitRaw {
    let controller_field_684 = if controller_field_ac8 >= 0 {
        controller_field_ac8 as u32
    } else {
        fallback_678
    };
    ScriptPhase2InitRaw {
        controller_field_684,
        controller_field_9ac: 0,
        controller_field_9a8: 0,
        controller_field_b30_bits: 0,
    }
}

/// Which of the two raw 0x271738 selector checks admitted the candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptPhase2SelectorOutcome {
    First,
    Second,
    Rejected,
}

/// Reproduce the accepted/rejected prefix at 0x271738. The descriptor gate
/// selects element 0 or 1 from each caller-supplied table; the raw 0x59DB0
/// result must be nonzero for either candidate to admit the later state path.
/// Verification: Tier 0 composition test covers first, second, and rejection
/// outcomes; no live controller adapter.
pub const fn script_phase2_selector_outcome(
    controller_field_204: u32,
    descriptor_byte_0c: u8,
    candidate_fields: ScriptFieldsRaw,
    first_table: [u32; 2],
    second_table: [u32; 2],
) -> ScriptPhase2SelectorOutcome {
    let slot = if script_descriptor_gate(controller_field_204, descriptor_byte_0c) {
        1
    } else {
        0
    };
    if field_124_when_d4_matches(candidate_fields, first_table[slot]) != 0 {
        ScriptPhase2SelectorOutcome::First
    } else if field_124_when_d4_matches(candidate_fields, second_table[slot]) != 0 {
        ScriptPhase2SelectorOutcome::Second
    } else {
        ScriptPhase2SelectorOutcome::Rejected
    }
}

/// Bounded outcome for the selector gate and `EDI` split in state 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptPhase2Transition {
    Rejected,
    AcceptedAndInitialized {
        matched_by: ScriptPhase2SelectorOutcome,
        writes: ScriptPhase2InitRaw,
    },
    AcceptedOtherEdiArm {
        matched_by: ScriptPhase2SelectorOutcome,
        service_plan: ScriptPhase2OtherEdiPlan,
    },
}

/// The raw `0x17570` argument selected on the accepted nonzero-EDI arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptPhase2ApplyMode {
    LowFlag40000000,
    HighFlag200,
    DefaultFlag4,
}

/// Raw helper-call plan for the accepted nonzero-EDI state-2 arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase2OtherEdiPlan {
    pub apply_mode: ScriptPhase2ApplyMode,
    /// The default arm writes controller `+0xB14` to `+0xAD0` before helpers.
    pub copy_b14_to_ad0: bool,
    /// After the shared `0x17570` call, nonzero EDI conditionally calls `0x158DB0`.
    pub call_158db0_after_apply: bool,
}

/// Select the measured helper sequence from `[controller+0xBF0]+0x54` and EDI.
/// Callees and the meaning of their raw mode constants remain external.
/// Verification: Tier 0 composition test covers all three mode selectors;
/// no live controller adapter.
pub const fn script_phase2_other_edi_plan(
    controller_field_54: u32,
    edi_is_nonzero: bool,
) -> ScriptPhase2OtherEdiPlan {
    let (apply_mode, copy_b14_to_ad0) = if controller_field_54 & 0x08 != 0 {
        (ScriptPhase2ApplyMode::LowFlag40000000, false)
    } else if controller_field_54 & 0x4000 != 0 {
        (ScriptPhase2ApplyMode::HighFlag200, false)
    } else {
        (ScriptPhase2ApplyMode::DefaultFlag4, true)
    };
    ScriptPhase2OtherEdiPlan {
        apply_mode,
        copy_b14_to_ad0,
        call_158db0_after_apply: edi_is_nonzero,
    }
}

/// Compose the observed state-2 selector gate with its bounded EDI split.
/// `edi_is_zero` is explicit because the nonzero arm is not yet modeled.
/// Verification: Tier 0 smoke coverage exercises selector order and EDI split;
/// no original differential or live controller adapter.
pub const fn transition_script_phase2(
    controller_field_204: u32,
    descriptor_byte_0c: u8,
    candidate_fields: ScriptFieldsRaw,
    first_table: [u32; 2],
    second_table: [u32; 2],
    edi_is_zero: bool,
    controller_field_54: u32,
    controller_field_ac8: i32,
    fallback_678: u32,
) -> ScriptPhase2Transition {
    let matched_by = script_phase2_selector_outcome(
        controller_field_204,
        descriptor_byte_0c,
        candidate_fields,
        first_table,
        second_table,
    );
    match matched_by {
        ScriptPhase2SelectorOutcome::Rejected => ScriptPhase2Transition::Rejected,
        ScriptPhase2SelectorOutcome::First | ScriptPhase2SelectorOutcome::Second if edi_is_zero => {
            ScriptPhase2Transition::AcceptedAndInitialized {
                matched_by,
                writes: initialize_script_phase2(controller_field_ac8, fallback_678),
            }
        }
        ScriptPhase2SelectorOutcome::First | ScriptPhase2SelectorOutcome::Second => {
            ScriptPhase2Transition::AcceptedOtherEdiArm {
                matched_by,
                service_plan: script_phase2_other_edi_plan(controller_field_54, !edi_is_zero),
            }
        }
    }
}

/// The bounded 0x271541 branch's next animation request and final D4 gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase3Plan {
    /// Raw selector passed to 0x5F460, absent when the first 0x59D20 pair hits.
    pub animation_selector: Option<u32>,
    /// Raw stack float bits pushed around the 0x5F460 call.
    pub animation_parameter_bits: [u32; 2],
    /// True when the later 0x59D70 test admits mode 2.
    pub enters_mode_two: bool,
}

/// Model the bounded state-3 selector sequence. `selectors` are the four
/// already-resolved values from tables 0x520378, 0x520468, 0x520478 and
/// 0x520488, in that order. Candidate +0xD4/+0x12C comparisons reuse 0x59D20;
/// the final +0xD4-only check reuses the observed 0x59D70 predicate.
/// Verification: Tier 0 smoke test covers both pair matches, ordering and the
/// final D4-only gate; no live controller/table adapter.
pub fn script_phase3_plan(
    candidate_fields: ScriptFieldsRaw,
    selectors: [u32; 4],
) -> ScriptPhase3Plan {
    let first_pair_hit = owner_has_either_slot(
        candidate_fields.field_d4,
        candidate_fields.field_12c,
        selectors[0],
    ) || owner_has_either_slot(
        candidate_fields.field_d4,
        candidate_fields.field_12c,
        selectors[1],
    );
    let second_pair_hit = owner_has_either_slot(
        candidate_fields.field_d4,
        candidate_fields.field_12c,
        selectors[2],
    ) || owner_has_either_slot(
        candidate_fields.field_d4,
        candidate_fields.field_12c,
        selectors[3],
    );
    let animation_selector = if first_pair_hit {
        None
    } else if second_pair_hit {
        Some(selectors[1])
    } else {
        Some(selectors[0])
    };
    ScriptPhase3Plan {
        animation_selector,
        animation_parameter_bits: [0x3EA8_F5C3, 0x3F80_0000],
        enters_mode_two: candidate_fields.field_d4 == selectors[0]
            || candidate_fields.field_d4 == selectors[1],
    }
}

/// The bounded ordered selector prefix at the start of controller state 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase4SelectorPlan {
    /// First admitted selector whose 0x59D20 predicate matched, in scan order.
    pub matched_selector: Option<u32>,
    /// Selector passed to 0x5F460 when all three selected values miss.
    pub fallback_animation_selector: Option<u32>,
    /// Subsequent state-4 scan selectors after the initial three checks.
    pub subsequent_scan_selectors: [u32; 9],
    /// True when a later descriptor-selected 0x59D20 scan was entered.
    pub enters_later_scan: bool,
    /// The tail selector selected for 0x5F460 after no later scan match.
    pub later_scan_fallback_selector: Option<u32>,
    /// Raw float bits pushed around the fallback 0x5F460 call.
    pub animation_parameter_bits: [u32; 2],
}

/// Resolve the bounded state-4 prefix over table bases `0x520478`, `0x520468`,
/// and `0x520488`. For each base, the descriptor sentinel/flag gate selects
/// table element 0 or 1 before `0x59D20` checks the two candidate fields. If
/// all three miss, the fallback `0x5F460` selector is element 1 from the first
/// table base, as loaded by the original instruction sequence. The later
/// state-4 join/terminal gates remain outside this selector prefix. Verification:
/// Tier 0 smoke test only.
pub fn script_phase4_selector_plan(
    candidate_fields: ScriptFieldsRaw,
    selector_pairs: [[u32; 2]; 3],
    descriptor_gate: [bool; 3],
) -> ScriptPhase4SelectorPlan {
    let selected = core::array::from_fn::<_, 3, _>(|index| {
        selector_pairs[index][usize::from(descriptor_gate[index])]
    });
    let matched_selector = selected.into_iter().find(|selector| {
        owner_has_either_slot(
            candidate_fields.field_d4,
            candidate_fields.field_12c,
            *selector,
        )
    });

    let later_pairs = [
        [0x5203A8, 0x5203AC],
        [0x520408, 0x52040C],
        [0x520418, 0x52041C],
        [0x520428, 0x52042C],
        [0x5203F8, 0x5203FC],
        [0x520388, 0x52038C],
        [0x520398, 0x52039C],
        [0x5203A8, 0x5203AC],
        [0x5203F8, 0x5203FC],
    ];
    let later_selectors = script_phase4_subsequent_scan_selectors(later_pairs, descriptor_gate[0]);
    ScriptPhase4SelectorPlan {
        matched_selector,
        fallback_animation_selector: matched_selector.is_none().then_some(selector_pairs[0][1]),
        subsequent_scan_selectors: later_selectors,
        enters_later_scan: true,
        later_scan_fallback_selector: Some(if descriptor_gate[0] {
            0x5203FC
        } else {
            0x5203F8
        }),
        animation_parameter_bits: [0x3EA8_F5C3, 0x3F80_0000],
    }
}

/// Select raw values for the later descriptor-gated scan. Order follows
/// addresses 0x5203A8, 0x520408, 0x520418, 0x520428, 0x5203F8,
/// 0x520388, 0x520398, 0x5203A8, 0x5203F8 (observed source order).
/// Each selector is checked with 0x59D20; a nonzero result short-circuits to
/// the shared join before the next selector is read. This remains a raw plan,
/// since EBP table candidate fields and descriptor lookup are external.
pub const fn script_phase4_subsequent_scan_selectors(
    selector_pairs: [[u32; 2]; 9],
    descriptor_slot_one: bool,
) -> [u32; 9] {
    let slot = descriptor_slot_one as usize;
    [
        selector_pairs[0][slot],
        selector_pairs[1][slot],
        selector_pairs[2][slot],
        selector_pairs[3][slot],
        selector_pairs[4][slot],
        selector_pairs[5][slot],
        selector_pairs[6][slot],
        selector_pairs[7][slot],
        selector_pairs[8][slot],
    ]
}

/// Convert the signed 0x153B40 result stored to controller `+0xB34` in the
/// state-4 fallback branch. The original x87 path adds `2^32` only for a
/// negative result, multiplies by `2^-32`, then stores binary32. `f64`
/// intermediates retain enough precision for the final binary32 rounding.
pub fn script_phase4_service_result_to_b34(raw_result: i32) -> f32 {
    let unsigned_fraction = if raw_result < 0 {
        raw_result as f64 + 4_294_967_296.0
    } else {
        raw_result as f64
    };
    (unsigned_fraction * (1.0 / 4_294_967_296.0)) as f32
}

/// Tail gate at 0x26FD2A. Inputs retain the observed raw offsets and flags;
/// the selector table is supplied by the caller after descriptor lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptPhase4TailInputs {
    pub b30: f32,
    pub long_distance_metric: f32,
    pub object_flag_54: u8,
    pub controller_flag_28: u32,
    pub descriptor_is_sentinel: bool,
    pub descriptor_flag_bit0: bool,
    pub selectors: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase4TailPlan {
    /// No animation call when a gate rejects; cleanup still runs.
    pub animation_selector: Option<u32>,
    pub animation_parameter_bits: [u32; 2],
    pub call_26d6d0: bool,
    pub long_distance: bool,
    pub cleanup_26dd80: bool,
}

/// Model the ordered `COMISS/JBE` threshold and subsequent raw flag gates.
/// The threshold bytes at 0x4788FC decode to 15.0f. Verification is Tier 0
/// synthetic coverage; descriptor/table and parent-controller adapters remain external.
pub fn script_phase4_tail_plan(input: ScriptPhase4TailInputs) -> ScriptPhase4TailPlan {
    let threshold_passes = input.b30 > 15.0;
    let descriptor_slot_one = input.descriptor_is_sentinel || input.descriptor_flag_bit0;
    let admitted = threshold_passes
        && input.object_flag_54 & 0x20 == 0
        && input.controller_flag_28 & 0x1_0000 == 0;
    let selector = input.selectors[usize::from(descriptor_slot_one)];
    let long_distance = input.long_distance_metric > 90.0;
    ScriptPhase4TailPlan {
        animation_selector: admitted.then_some(selector),
        animation_parameter_bits: [0x3EA8_F5C3, 0x3F80_0000],
        call_26d6d0: admitted,
        long_distance,
        cleanup_26dd80: true,
    }
}

/// Bounded distance-selected animation call from state 5, before later gates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase5DistancePlan {
    pub selector: u32,
    pub metric_exceeded_limit: bool,
    /// Raw float bits pushed in source order around 0x5F460.
    pub animation_parameter_bits: [u32; 2],
}

/// Model the 0x2703CF metric threshold and descriptor-selected 0x5F460 value.
/// The caller supplies the already-computed sign-cleared metric and raw
/// per-slot table values; metric and threshold semantics remain unnamed.
/// Verification: Tier 0 smoke test covers strict comparison, table slot and
/// unordered input; no live controller/table adapter.
pub fn script_phase5_distance_plan(
    sign_cleared_metric: f32,
    limit: f32,
    descriptor_slot_one: bool,
    far_selectors: [u32; 2],
    near_selectors: [u32; 2],
) -> ScriptPhase5DistancePlan {
    let metric_exceeded_limit = sign_cleared_metric > limit;
    let slot = if descriptor_slot_one { 1 } else { 0 };
    let selector = if metric_exceeded_limit {
        far_selectors[slot]
    } else {
        near_selectors[slot]
    };
    ScriptPhase5DistancePlan {
        selector,
        metric_exceeded_limit,
        animation_parameter_bits: [0x3F80_0000, 0],
    }
}

/// Raw state-7 animation request selected after the sign-cleared scalar metric
/// exceeds its global limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase7DistancePlan {
    pub selector: Option<u32>,
    pub metric_exceeded_limit: bool,
    /// Raw float bits pushed around 0x5F460 after the selected table value.
    pub animation_parameter_bits: [u32; 2],
}

/// Model the measured state-7 `0x270971..0x2709E6` distance branch.
/// `limit` and `selectors` are caller snapshots of the global threshold and
/// descriptor-selected `0x5203B8` entries. The original uses COMISS followed
/// by JBE, so NaN follows the non-request route. Metric and table semantics
/// remain unnamed. Verification: Tier 0 unit test only; no parent adapter.
pub fn script_phase7_distance_plan(
    sign_cleared_metric: f32,
    limit: f32,
    descriptor_slot_one: bool,
    selectors: [u32; 2],
) -> ScriptPhase7DistancePlan {
    let metric_exceeded_limit = sign_cleared_metric > limit;
    ScriptPhase7DistancePlan {
        selector: metric_exceeded_limit.then_some(selectors[usize::from(descriptor_slot_one)]),
        metric_exceeded_limit,
        animation_parameter_bits: [0, 0x3F80_0000],
    }
}

/// Rust-owned handler boundary for the state-7 animation request at `0x5F460`.
///
/// The caller supplies snapshots resolved by its controller/table adapter;
/// this function never reads guest memory. A request is dispatched only when
/// the measured ordered comparison selects one. Verification remains Tier 0
/// only when a focused test covers both dispatch and the no-request path.
pub trait ScriptPhase7AnimationHandler {
    fn request_animation(&mut self, selector: u32, parameter_bits: [u32; 2]);
}

/// Apply the bounded state-7 decision to a native Rust handler.
///
/// Object lookup, metric production, descriptor resolution, and ownership of
/// the controller remain with the caller. This completes only the final
/// decision-to-handler edge observed in the state-7 branch.
pub fn dispatch_script_phase7_request<H: ScriptPhase7AnimationHandler>(
    sign_cleared_metric: f32,
    limit: f32,
    descriptor_slot_one: bool,
    selectors: [u32; 2],
    handler: &mut H,
) -> ScriptPhase7DistancePlan {
    let plan =
        script_phase7_distance_plan(sign_cleared_metric, limit, descriptor_slot_one, selectors);
    if let Some(selector) = plan.selector {
        handler.request_animation(selector, plan.animation_parameter_bits);
    }
    plan
}

/// Raw animation requests on the bounded state-8 angle paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptPhase8Route {
    WrappedDeltaOver135 { selector: u32 },
    SignedValueOver30 { selector: u32 },
    CloseValueSelectorMatch { selector: u32 },
}

/// State-8 phase writes observed after the optional close-value request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase8PhaseChange {
    pub previous_phase: u32,
    pub new_phase: u32,
    pub field_9d4: u32,
    pub field_9d8_bits: u32,
    pub field_9d0: u32,
}

/// Raw result of the bounded state-8 angle and terminal phase branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptPhase8Plan {
    pub route: Option<ScriptPhase8Route>,
    pub phase_change: Option<ScriptPhase8PhaseChange>,
    /// Every observed route reaches the shared 0x26DD80 cleanup before return.
    pub reaches_cleanup: bool,
}

/// Model the bounded state-8 branches at 0x271382..0x271540.
///
/// `descriptor_slot` is the already-resolved 0/1 slot selected by the raw
/// +0x204 descriptor gate. Float comparisons intentionally use ordered `>`:
/// the original COMISS followed by JBE sends unordered NaNs through the same
/// arms as values not greater than the threshold. Table values remain raw.
/// Verification: Tier 0 unit tests only; no original differential or runtime
/// attachment is claimed.
pub fn script_phase8_plan(
    wrapped_delta: f32,
    secondary_value: f32,
    stack_selector: u32,
    edi: u32,
    current_phase: u32,
    descriptor_slot: usize,
    over_135_selectors: [u32; 2],
    positive_over_30_selectors: [u32; 2],
    nonpositive_over_30_selectors: [u32; 2],
    close_value_selectors: [u32; 2],
) -> ScriptPhase8Plan {
    let slot = descriptor_slot.min(1);
    let route = if wrapped_delta.abs() > 135.0 {
        Some(ScriptPhase8Route::WrappedDeltaOver135 {
            selector: over_135_selectors[slot],
        })
    } else if secondary_value.abs() > 30.0 {
        let selectors = if secondary_value > 0.0 {
            positive_over_30_selectors
        } else {
            nonpositive_over_30_selectors
        };
        Some(ScriptPhase8Route::SignedValueOver30 {
            selector: selectors[slot],
        })
    } else if stack_selector == edi {
        Some(ScriptPhase8Route::CloseValueSelectorMatch {
            selector: close_value_selectors[slot],
        })
    } else {
        None
    };

    // The >135 and >30 branches return before these phase writes.
    let phase_change = match route {
        Some(ScriptPhase8Route::WrappedDeltaOver135 { .. })
        | Some(ScriptPhase8Route::SignedValueOver30 { .. }) => None,
        _ if current_phase != edi => Some(ScriptPhase8PhaseChange {
            previous_phase: current_phase,
            new_phase: edi,
            field_9d4: current_phase,
            field_9d8_bits: 0,
            field_9d0: edi,
        }),
        _ => None,
    };

    ScriptPhase8Plan {
        route,
        phase_change,
        reaches_cleanup: true,
    }
}

/// Reproduce the observed 0x277FE0 gates and ordered table scan. The source
/// starts at row index 1, returns no row when the raw count equals 1, and stops
/// at the supplied table limit. `matches` models the 0x2776F0 call for each
/// row's raw +0xBF0 field without assigning that field a semantic type.
/// Verification: Tier 0 smoke test covers both gates, ordered scan, and limit;
/// no live table adapter.
pub fn scan_script_awareness_rows<'a, T>(
    mode_byte: u8,
    raw_count: u32,
    table_limit: usize,
    rows: &'a [T],
    mut matches: impl FnMut(&T) -> bool,
) -> ScriptAwarenessLookupOutcome<'a, T> {
    if mode_byte != 0x0A {
        return ScriptAwarenessLookupOutcome::Disabled;
    }
    if raw_count == 1 {
        return ScriptAwarenessLookupOutcome::CountSentinel;
    }
    for row in rows.iter().take(table_limit).skip(1) {
        if matches(row) {
            return ScriptAwarenessLookupOutcome::Matched(row);
        }
    }
    ScriptAwarenessLookupOutcome::Exhausted
}

/// Scan candidate values in source order, skip the raw excluded value, and
/// retain at most two values whose 0x12FA0 pair check is nonzero.
///
/// Candidate extraction from the `[0x7356D8] + 8` table (count
/// `[0x790950]`, stride `0xEA0`) and the pair-table owner are caller boundaries.
pub fn collect_script_awareness_pair_candidates(
    candidates: impl IntoIterator<Item = u32>,
    pair_anchor: u32,
    excluded_value: u32,
    mut pair_matches: impl FnMut(u32, u32) -> bool,
) -> ScriptAwarenessPairCandidates {
    let mut result = ScriptAwarenessPairCandidates::default();
    for value in candidates {
        if result.count == result.values.len() {
            break;
        }
        if value != excluded_value && pair_matches(pair_anchor, value) {
            result.values[result.count] = value;
            result.count += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        ScriptAwarenessFastPath, ScriptAwarenessLookupOutcome, ScriptAwarenessPairCandidates,
        ScriptAwarenessPrefixRaw, ScriptControllerEntryRaw, ScriptFieldsRaw, ScriptPairFieldsRaw,
        ScriptPhase2ApplyMode, ScriptPhase2SelectorOutcome, ScriptPhase2Transition,
        ScriptPhase4TailInputs, ScriptPhase8PhaseChange, ScriptPhase8Route, ScriptTableEntryRaw,
        apply_controller_entry_prefix, collect_script_awareness_pair_candidates, field_12c_matches,
        field_124_when_d4_matches, field_d4_matches, scan_script_awareness_rows,
        script_awareness_expected_row_tag, script_awareness_fast_path, script_descriptor_gate,
        script_mode2_selector_call_plan, script_phase2_other_edi_plan, script_phase3_plan,
        script_phase4_selector_plan, script_phase4_service_result_to_b34,
        script_phase4_subsequent_scan_selectors, script_phase4_tail_plan,
        script_phase5_distance_plan, script_phase7_distance_plan, script_phase8_plan,
        select_124_or_17c, table_value_d8, transition_script_phase2, update_pair_17570,
    };

    #[test]
    fn controller_dispatch_decodes_only_the_nine_unsigned_state_indices() {
        use super::ScriptControllerLocalState as State;

        let expected = [
            State::State0,
            State::State1,
            State::State2,
            State::State3,
            State::State4,
            State::State5,
            State::State6,
            State::State7,
            State::State8,
        ];
        for raw in 0..=8 {
            assert_eq!(
                super::decode_controller_local_state(raw),
                Some(expected[raw as usize])
            );
        }
        assert_eq!(super::decode_controller_local_state(9), None);
        assert_eq!(super::decode_controller_local_state(u32::MAX), None);
    }

    #[test]
    fn descriptor_gate_matches_26efc0_setup_sentinel_and_flag_bit() {
        assert!(script_descriptor_gate(u32::MAX, 0));
        assert!(script_descriptor_gate(u32::MAX, 1));
        assert!(!script_descriptor_gate(0, 0));
        assert!(script_descriptor_gate(0, 1));
        assert!(!script_descriptor_gate(7, 0x02));
        assert!(script_descriptor_gate(7, 0x03));
    }

    #[test]
    fn controller_tick_composes_entry_transition_and_selected_state4_tail() {
        let mut fields = ScriptControllerEntryRaw {
            field_20: 3,
            field_9d0: 4,
            field_9d4: 0,
            field_9d8: 2.0,
            field_a90: 9.0,
            field_2c: 0x80,
            field_28: 0,
        };
        let controller_flag_28 = fields.field_28;
        let result = super::apply_controller_tick_prefix(
            &mut fields,
            super::ScriptControllerTickInput {
                globals: super::ScriptControllerGlobals {
                    divisor_4e8cec: 60,
                    scale_4e794c: 1,
                },
                state4_selector: Some(super::ScriptPhase4SelectorInputs {
                    candidate_fields: ScriptFieldsRaw::default(),
                    selector_pairs: [[10, 11], [20, 21], [30, 31]],
                    descriptor_gate: [false, true, false],
                }),
                state4_tail: Some(ScriptPhase4TailInputs {
                    b30: 16.0,
                    long_distance_metric: 90.0,
                    object_flag_54: 0,
                    controller_flag_28,
                    descriptor_is_sentinel: false,
                    descriptor_flag_bit0: true,
                    selectors: [10, 11],
                }),
            },
        );
        assert_eq!(fields.field_20, 4);
        assert_eq!(fields.field_9d4, 3);
        assert_eq!(fields.field_9d8, 0.0);
        assert_eq!(fields.field_a90, 9.0 + (1.0 / 60.0));
        assert_eq!(fields.field_28, 0x4000);
        assert_eq!(
            result.state4_selector.unwrap().fallback_animation_selector,
            Some(11)
        );
        assert_eq!(result.state4_tail.unwrap().animation_selector, Some(11));
    }

    #[test]
    fn controller_timer_uses_signed_byte_and_clears_for_nonnegative_values() {
        let mut fields = ScriptControllerEntryRaw {
            field_9d0: -1,
            field_a90: 7.0,
            field_2c: 0x7f,
            ..ScriptControllerEntryRaw::default()
        };
        super::apply_controller_entry_prefix(&mut fields, 60, 1);
        assert_eq!(fields.field_a90, 0.0);

        fields.field_a90 = 7.0;
        fields.field_2c = 0x80;
        super::apply_controller_entry_prefix(&mut fields, 60, 1);
        assert_eq!(fields.field_a90, 7.0 + (1.0 / 60.0));
    }

    #[test]
    fn mode2_selector_plan_preserves_table_choice_and_613d0_arguments() {
        assert_eq!(
            script_mode2_selector_call_plan(1, 7, 1, [0x111, 0x222]),
            None
        );
        assert_eq!(
            script_mode2_selector_call_plan(2, 7, 0, [0x111, 0x222]),
            Some(super::ScriptMode2SelectorCallPlan {
                table_slot: 0,
                selector: 0x111,
                ordinal: 2,
                mask: 1,
            })
        );
        assert_eq!(
            script_mode2_selector_call_plan(2, u32::MAX, 0, [0x111, 0x222]),
            Some(super::ScriptMode2SelectorCallPlan {
                table_slot: 1,
                selector: 0x222,
                ordinal: 2,
                mask: 1,
            })
        );
        assert_eq!(
            script_mode2_selector_call_plan(2, 7, 0x02, [0x111, 0x222]).map(|plan| plan.table_slot),
            Some(0)
        );
        assert_eq!(
            script_mode2_selector_call_plan(2, 7, 0x01, [0x111, 0x222]).map(|plan| plan.table_slot),
            Some(1)
        );
    }

    #[test]
    fn phase2_state_slice_preserves_ordered_selectors_writes_and_other_edi_plan() {
        let first = transition_script_phase2(
            7,
            0,
            ScriptFieldsRaw {
                field_d4: 10,
                field_124: 1,
                ..ScriptFieldsRaw::default()
            },
            [10, 11],
            [20, 21],
            true,
            0,
            -1,
            0xAABB_CCDD,
        );
        assert_eq!(
            first,
            ScriptPhase2Transition::AcceptedAndInitialized {
                matched_by: ScriptPhase2SelectorOutcome::First,
                writes: super::ScriptPhase2InitRaw {
                    controller_field_684: 0xAABB_CCDD,
                    controller_field_9ac: 0,
                    controller_field_9a8: 0,
                    controller_field_b30_bits: 0,
                },
            }
        );

        let nonnegative = transition_script_phase2(
            u32::MAX,
            0,
            ScriptFieldsRaw {
                field_d4: 11,
                field_124: 1,
                ..ScriptFieldsRaw::default()
            },
            [10, 11],
            [20, 21],
            true,
            0,
            5,
            0xAABB_CCDD,
        );
        assert_eq!(
            nonnegative,
            ScriptPhase2Transition::AcceptedAndInitialized {
                matched_by: ScriptPhase2SelectorOutcome::First,
                writes: super::ScriptPhase2InitRaw {
                    controller_field_684: 5,
                    controller_field_9ac: 0,
                    controller_field_9a8: 0,
                    controller_field_b30_bits: 0,
                },
            }
        );

        let second = transition_script_phase2(
            7,
            0,
            ScriptFieldsRaw {
                field_d4: 20,
                field_124: 2,
                ..ScriptFieldsRaw::default()
            },
            [10, 11],
            [20, 21],
            false,
            0x08,
            3,
            4,
        );
        assert_eq!(
            second,
            ScriptPhase2Transition::AcceptedOtherEdiArm {
                matched_by: ScriptPhase2SelectorOutcome::Second,
                service_plan: super::ScriptPhase2OtherEdiPlan {
                    apply_mode: ScriptPhase2ApplyMode::LowFlag40000000,
                    copy_b14_to_ad0: false,
                    call_158db0_after_apply: true,
                },
            }
        );

        assert_eq!(
            script_phase2_other_edi_plan(0x4000, true),
            super::ScriptPhase2OtherEdiPlan {
                apply_mode: ScriptPhase2ApplyMode::HighFlag200,
                copy_b14_to_ad0: false,
                call_158db0_after_apply: true,
            }
        );
        assert_eq!(
            script_phase2_other_edi_plan(0, true),
            super::ScriptPhase2OtherEdiPlan {
                apply_mode: ScriptPhase2ApplyMode::DefaultFlag4,
                copy_b14_to_ad0: true,
                call_158db0_after_apply: true,
            }
        );
        assert!(!script_phase2_other_edi_plan(0, false).call_158db0_after_apply);

        let rejected = transition_script_phase2(
            7,
            0,
            ScriptFieldsRaw {
                field_d4: 99,
                field_124: 1,
                ..ScriptFieldsRaw::default()
            },
            [10, 11],
            [20, 21],
            true,
            0,
            0,
            0,
        );
        assert_eq!(rejected, ScriptPhase2Transition::Rejected);
    }

    #[test]
    fn phase3_plan_preserves_pair_order_selector_choice_and_d4_gate() {
        let selectors = [10, 20, 30, 40];
        let first_d4_hit = script_phase3_plan(
            ScriptFieldsRaw {
                field_d4: 10,
                field_12c: 90,
                ..ScriptFieldsRaw::default()
            },
            selectors,
        );
        assert_eq!(first_d4_hit.animation_selector, None);
        assert!(first_d4_hit.enters_mode_two);

        let first_12c_hit = script_phase3_plan(
            ScriptFieldsRaw {
                field_d4: 50,
                field_12c: 10,
                ..ScriptFieldsRaw::default()
            },
            selectors,
        );
        assert_eq!(first_12c_hit.animation_selector, None);
        assert!(!first_12c_hit.enters_mode_two);

        let second_pair_hit = script_phase3_plan(
            ScriptFieldsRaw {
                field_d4: 50,
                field_12c: 30,
                ..ScriptFieldsRaw::default()
            },
            selectors,
        );
        assert_eq!(second_pair_hit.animation_selector, Some(20));
        assert!(!second_pair_hit.enters_mode_two);

        let no_pair_hit = script_phase3_plan(
            ScriptFieldsRaw {
                field_d4: 50,
                field_12c: 90,
                ..ScriptFieldsRaw::default()
            },
            selectors,
        );
        assert_eq!(no_pair_hit.animation_selector, Some(10));
        assert!(!no_pair_hit.enters_mode_two);
        assert_eq!(
            no_pair_hit.animation_parameter_bits,
            [0x3EA8_F5C3, 0x3F80_0000]
        );
    }

    #[test]
    fn phase5_distance_plan_uses_strict_limit_and_descriptor_slot() {
        let far = script_phase5_distance_plan(6.0, 5.0, true, [10, 11], [20, 21]);
        assert_eq!(far.selector, 11);
        assert!(far.metric_exceeded_limit);
        assert_eq!(far.animation_parameter_bits, [0x3F80_0000, 0]);

        let equal = script_phase5_distance_plan(5.0, 5.0, false, [10, 11], [20, 21]);
        assert_eq!(equal.selector, 20);
        assert!(!equal.metric_exceeded_limit);

        let unordered = script_phase5_distance_plan(f32::NAN, 5.0, true, [10, 11], [20, 21]);
        assert_eq!(unordered.selector, 21);
        assert!(!unordered.metric_exceeded_limit);
    }

    #[test]
    fn phase7_distance_plan_requests_only_above_limit() {
        let above = script_phase7_distance_plan(6.0, 5.0, true, [10, 11]);
        assert_eq!(above.selector, Some(11));
        assert!(above.metric_exceeded_limit);
        assert_eq!(above.animation_parameter_bits, [0, 0x3F80_0000]);

        let equal = script_phase7_distance_plan(5.0, 5.0, false, [10, 11]);
        assert_eq!(equal.selector, None);
        assert!(!equal.metric_exceeded_limit);

        let unordered = script_phase7_distance_plan(f32::NAN, 5.0, true, [10, 11]);
        assert_eq!(unordered.selector, None);
        assert!(!unordered.metric_exceeded_limit);
    }

    #[test]
    fn phase4_selector_plan_preserves_order_descriptor_gate_and_fallback() {
        let selectors = [[10, 11], [20, 21], [30, 31]];
        let second_hit = script_phase4_selector_plan(
            ScriptFieldsRaw {
                field_d4: 21,
                field_12c: 31,
                ..ScriptFieldsRaw::default()
            },
            selectors,
            [false, true, false],
        );
        assert_eq!(second_hit.matched_selector, Some(21));
        assert_eq!(second_hit.fallback_animation_selector, None);

        let all_miss_uses_first_table_alternate_for_fallback = script_phase4_selector_plan(
            ScriptFieldsRaw {
                field_d4: 99,
                field_12c: 99,
                ..ScriptFieldsRaw::default()
            },
            selectors,
            [false, false, false],
        );
        assert_eq!(
            all_miss_uses_first_table_alternate_for_fallback.matched_selector,
            None
        );
        assert_eq!(
            all_miss_uses_first_table_alternate_for_fallback.fallback_animation_selector,
            Some(11)
        );
        assert_eq!(
            all_miss_uses_first_table_alternate_for_fallback.animation_parameter_bits,
            [0x3EA8_F5C3, 0x3F80_0000]
        );
    }

    #[test]
    fn phase4_tail_gates_are_ordered_and_cleanup_is_unconditional() {
        let base = ScriptPhase4TailInputs {
            b30: 15.1,
            long_distance_metric: 91.0,
            object_flag_54: 0,
            controller_flag_28: 0,
            descriptor_is_sentinel: false,
            descriptor_flag_bit0: false,
            selectors: [40, 41],
        };
        assert_eq!(script_phase4_tail_plan(base).animation_selector, Some(40));
        assert_eq!(
            script_phase4_tail_plan(ScriptPhase4TailInputs { b30: 15.0, ..base })
                .animation_selector,
            None
        );
        assert_eq!(
            script_phase4_tail_plan(ScriptPhase4TailInputs {
                b30: f32::NAN,
                ..base
            })
            .animation_selector,
            None
        );
        assert_eq!(
            script_phase4_tail_plan(ScriptPhase4TailInputs {
                object_flag_54: 0x20,
                ..base
            })
            .animation_selector,
            None
        );
        assert_eq!(
            script_phase4_tail_plan(ScriptPhase4TailInputs {
                controller_flag_28: 0x1_0000,
                ..base
            })
            .animation_selector,
            None
        );
        let alternate = script_phase4_tail_plan(ScriptPhase4TailInputs {
            descriptor_flag_bit0: true,
            ..base
        });
        assert_eq!(alternate.animation_selector, Some(41));
        let not_long = script_phase4_tail_plan(ScriptPhase4TailInputs {
            long_distance_metric: 90.0,
            ..base
        });
        assert!(!not_long.long_distance);
        let long = script_phase4_tail_plan(base);
        assert!(long.long_distance);
        assert!(alternate.call_26d6d0 && alternate.cleanup_26dd80);
        assert_eq!(
            alternate.animation_parameter_bits,
            [0x3EA8_F5C3, 0x3F80_0000]
        );
        let rejected = script_phase4_tail_plan(ScriptPhase4TailInputs {
            controller_flag_28: 0x1_0000,
            ..base
        });
        assert!(!rejected.call_26d6d0);
        assert!(rejected.cleanup_26dd80);
    }

    #[test]
    fn phase4_later_selector_scan_preserves_nine_slot_order() {
        let pairs = core::array::from_fn(|i| [100 + i as u32, 200 + i as u32]);
        assert_eq!(
            script_phase4_subsequent_scan_selectors(pairs, false),
            [100, 101, 102, 103, 104, 105, 106, 107, 108]
        );
        assert_eq!(
            script_phase4_subsequent_scan_selectors(pairs, true),
            [200, 201, 202, 203, 204, 205, 206, 207, 208]
        );
    }

    #[test]
    fn phase4_service_result_to_b34_matches_signed_wrap_and_scale() {
        assert_eq!(script_phase4_service_result_to_b34(0), 0.0);
        assert_eq!(
            script_phase4_service_result_to_b34(1).to_bits(),
            0x2F80_0000
        );
        assert_eq!(script_phase4_service_result_to_b34(i32::MIN), 0.5);
        assert_eq!(
            script_phase4_service_result_to_b34(-1).to_bits(),
            1.0f32.to_bits()
        );
        assert_eq!(
            script_phase4_service_result_to_b34(i32::MAX).to_bits(),
            0x3F00_0000
        );
    }

    #[test]
    fn awareness_lookup_preserves_tag_dispatch_gates_order_and_limit() {
        assert_eq!(script_awareness_expected_row_tag(1), Some(0x4A42_5448));
        assert_eq!(script_awareness_expected_row_tag(17), Some(0x4A42_5448));
        assert_eq!(script_awareness_expected_row_tag(4), Some(0x4350_5441));
        assert_eq!(script_awareness_expected_row_tag(0), None);
        assert_eq!(script_awareness_expected_row_tag(2), None);

        let rows = [(0, false), (1, false), (2, true), (3, true)];
        assert!(matches!(
            scan_script_awareness_rows(0, 4, 4, &rows, |row| row.1),
            ScriptAwarenessLookupOutcome::Disabled
        ));
        assert!(matches!(
            scan_script_awareness_rows(0x0A, 1, 4, &rows, |row| row.1),
            ScriptAwarenessLookupOutcome::CountSentinel
        ));
        assert_eq!(
            scan_script_awareness_rows(0x0A, 4, 3, &rows, |row| row.1),
            ScriptAwarenessLookupOutcome::Matched(&rows[2])
        );
        assert_eq!(
            scan_script_awareness_rows(0x0A, 4, 2, &rows, |row| row.1),
            ScriptAwarenessLookupOutcome::Exhausted
        );
    }

    #[test]
    fn awareness_pair_scan_keeps_order_skips_excluded_and_caps_at_two() {
        let mut checked = Vec::new();
        let candidates = collect_script_awareness_pair_candidates(
            [4, 9, 7, 7, 10],
            3,
            4,
            |anchor, candidate| {
                checked.push((anchor, candidate));
                candidate != 10
            },
        );
        assert_eq!(
            candidates,
            ScriptAwarenessPairCandidates {
                values: [9, 7],
                count: 2,
            }
        );
        assert_eq!(checked, [(3, 9), (3, 7)]);
    }

    #[test]
    fn awareness_fast_path_preserves_measured_raw_returns_and_complex_boundary() {
        let controller = ScriptAwarenessPrefixRaw {
            controller_field_0: 0x100,
            controller_field_4: 0x200,
            controller_field_adc: 7,
        };
        assert_eq!(
            script_awareness_fast_path(0x4000_0000, controller, Some(8)),
            ScriptAwarenessFastPath::Return(0)
        );
        assert_eq!(
            script_awareness_fast_path(
                0,
                ScriptAwarenessPrefixRaw {
                    controller_field_4: 0,
                    ..controller
                },
                Some(8)
            ),
            ScriptAwarenessFastPath::Return(0)
        );
        assert_eq!(
            script_awareness_fast_path(
                0,
                ScriptAwarenessPrefixRaw {
                    controller_field_4: 0x100,
                    ..controller
                },
                Some(3)
            ),
            ScriptAwarenessFastPath::Return(1)
        );
        assert_eq!(
            script_awareness_fast_path(0, controller, Some(3)),
            ScriptAwarenessFastPath::Return(7)
        );
        assert_eq!(
            script_awareness_fast_path(0, controller, Some(8)),
            ScriptAwarenessFastPath::Continue
        );
        assert_eq!(
            script_awareness_fast_path(0, controller, None),
            ScriptAwarenessFastPath::Continue
        );
    }

    #[test]
    fn controller_entry_prefix_consumes_changed_request_and_accumulates_negative_byte_delta() {
        let mut fields = ScriptControllerEntryRaw {
            field_20: 7,
            field_9d0: 9,
            field_9d4: 1,
            field_9d8: 2.0,
            field_a90: 0.5,
            field_2c: 0x80,
            field_28: 0x20,
        };

        apply_controller_entry_prefix(&mut fields, 60, -3);

        assert_eq!(fields.field_20, 9);
        assert_eq!(fields.field_9d0, -1);
        assert_eq!(fields.field_9d4, 7);
        assert_eq!(fields.field_9d8, 0.0);
        assert!((fields.field_a90 - 0.45).abs() < 0.000_001);
        assert_eq!(fields.field_28, 0x4020);
    }

    #[test]
    fn controller_entry_prefix_equal_request_only_consumes_the_pending_value() {
        let mut fields = ScriptControllerEntryRaw {
            field_20: 4,
            field_9d0: 4,
            field_9d4: 3,
            field_9d8: 1.25,
            field_a90: 8.0,
            field_2c: 0x7f,
            field_28: 0,
        };

        apply_controller_entry_prefix(&mut fields, 60, -3);

        assert_eq!(fields.field_20, 4);
        assert_eq!(fields.field_9d0, -1);
        assert_eq!(fields.field_9d4, 3);
        assert_eq!(fields.field_9d8, 1.25);
        assert_eq!(fields.field_a90, 0.0);
        assert_eq!(fields.field_28, 0x4000);
    }

    #[test]
    fn controller_entry_prefix_updates_on_any_raw_difference() {
        let mut fields = ScriptControllerEntryRaw {
            field_20: 9,
            field_9d0: 4,
            field_9d4: 3,
            field_9d8: 1.25,
            field_a90: 0.0,
            field_2c: 0,
            field_28: 0,
        };

        apply_controller_entry_prefix(&mut fields, 60, 1);

        assert_eq!(fields.field_20, 4);
        assert_eq!(fields.field_9d0, -1);
        assert_eq!(fields.field_9d4, 9);
        assert_eq!(fields.field_9d8, 0.0);
    }

    #[test]
    fn controller_entry_prefix_uses_raw_equality_for_the_current_value() {
        let mut fields = ScriptControllerEntryRaw {
            field_20: 0xffff_fffe,
            field_9d0: 1,
            field_9d4: 0,
            field_9d8: 2.0,
            field_a90: 0.0,
            field_2c: 0,
            field_28: 0,
        };

        apply_controller_entry_prefix(&mut fields, 60, 1);

        assert_eq!(fields.field_20, 1);
        assert_eq!(fields.field_9d0, -1);
        assert_eq!(fields.field_9d4, 0xffff_fffe);
        assert_eq!(fields.field_9d8, 0.0);
    }

    #[test]
    fn controller_entry_prefix_leaves_negative_pending_value_unconsumed() {
        let mut fields = ScriptControllerEntryRaw {
            field_20: 4,
            field_9d0: -2,
            field_9d4: 3,
            field_9d8: 1.25,
            field_a90: 8.0,
            field_2c: 0x80,
            field_28: 0,
        };

        apply_controller_entry_prefix(&mut fields, 60, 30);

        assert_eq!(fields.field_9d0, -2);
        assert_eq!(fields.field_9d4, 3);
        assert_eq!(fields.field_9d8, 1.25);
        assert_eq!(fields.field_a90, 8.5);
        assert_eq!(fields.field_28, 0x4000);
    }

    fn sample() -> ScriptFieldsRaw {
        ScriptFieldsRaw {
            field_d4: 11,
            field_d8: 1,
            field_124: 0x124,
            field_12c: 22,
            field_17c: 0x17c,
        }
    }

    #[test]
    fn raw_field_predicates_and_conditional_value() {
        let fields = sample();
        assert!(field_d4_matches(fields, 11));
        assert!(!field_d4_matches(fields, 12));
        assert!(field_12c_matches(fields, 22));
        assert_eq!(field_124_when_d4_matches(fields, 11), 0x124);
        assert_eq!(field_124_when_d4_matches(fields, 12), 0);
    }

    #[test]
    fn indexed_table_and_nullable_field_selection() {
        let fields = sample();
        assert_eq!(
            table_value_d8(
                fields,
                &[
                    ScriptTableEntryRaw { field_18: 1.0 },
                    ScriptTableEntryRaw { field_18: 2.5 }
                ]
            ),
            Some(2.5)
        );
        assert_eq!(
            table_value_d8(fields, &[ScriptTableEntryRaw { field_18: 1.0 }]),
            None
        );
        assert_eq!(select_124_or_17c(None), 0);
        assert_eq!(select_124_or_17c(Some(fields)), 0x17c);
        assert_eq!(
            select_124_or_17c(Some(ScriptFieldsRaw {
                field_12c: 0,
                ..fields
            })),
            0x124
        );
    }

    #[test]
    fn pair_update_obeys_mode_selected_low_bit_gate() {
        let mut gated = ScriptPairFieldsRaw {
            field_aa8: 1,
            field_aac: 0,
            field_10: 1,
            field_14: 77,
            field_18: 0,
            field_1c: 88,
        };
        update_pair_17570(&mut gated, 0x0a, [2, 3]);
        assert_eq!(gated.field_aa8, 2);
        assert_eq!(gated.field_aac, 3);

        let mut held = ScriptPairFieldsRaw {
            field_aa8: 1,
            field_aac: 0,
            field_10: 0,
            field_14: 77,
            field_18: 1,
            field_1c: 88,
        };
        update_pair_17570(&mut held, 0x0a, [2, 3]);
        assert_eq!(held.field_aa8, 1);
        assert_eq!(held.field_aac, 0);

        update_pair_17570(&mut held, 0x09, [4, 5]);
        assert_eq!(held.field_aa8, 4);
        assert_eq!(held.field_aac, 5);
    }

    #[test]
    fn phase8_angle_route_has_priority_and_uses_descriptor_slot() {
        let plan = script_phase8_plan(
            136.0,
            40.0,
            8,
            8,
            3,
            1,
            [10, 11],
            [20, 21],
            [30, 31],
            [40, 41],
        );
        assert_eq!(
            plan.route,
            Some(ScriptPhase8Route::WrappedDeltaOver135 { selector: 11 })
        );
        assert_eq!(plan.phase_change, None);
        assert!(plan.reaches_cleanup);
    }

    #[test]
    fn phase8_signed_threshold_selects_by_sign_and_skips_phase_writes() {
        let positive = script_phase8_plan(
            135.0,
            31.0,
            4,
            4,
            2,
            0,
            [10, 11],
            [20, 21],
            [30, 31],
            [40, 41],
        );
        assert_eq!(
            positive.route,
            Some(ScriptPhase8Route::SignedValueOver30 { selector: 20 })
        );
        assert_eq!(positive.phase_change, None);

        let negative = script_phase8_plan(
            0.0,
            -31.0,
            4,
            4,
            2,
            1,
            [10, 11],
            [20, 21],
            [30, 31],
            [40, 41],
        );
        assert_eq!(
            negative.route,
            Some(ScriptPhase8Route::SignedValueOver30 { selector: 31 })
        );
        assert_eq!(negative.phase_change, None);
    }

    #[test]
    fn phase8_close_gate_and_unordered_float_follow_jbe_paths() {
        let close = script_phase8_plan(
            0.0,
            f32::NAN,
            8,
            8,
            5,
            0,
            [10, 11],
            [20, 21],
            [30, 31],
            [40, 41],
        );
        assert_eq!(
            close.route,
            Some(ScriptPhase8Route::CloseValueSelectorMatch { selector: 40 })
        );
        assert_eq!(
            close.phase_change,
            Some(ScriptPhase8PhaseChange {
                previous_phase: 5,
                new_phase: 8,
                field_9d4: 5,
                field_9d8_bits: 0,
                field_9d0: 8,
            })
        );

        let phase_only = script_phase8_plan(
            f32::NAN,
            0.0,
            7,
            8,
            5,
            0,
            [10, 11],
            [20, 21],
            [30, 31],
            [40, 41],
        );
        assert_eq!(phase_only.route, None);
        assert_eq!(phase_only.phase_change, close.phase_change);
    }
}
