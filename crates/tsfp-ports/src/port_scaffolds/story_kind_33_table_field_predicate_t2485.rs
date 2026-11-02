// SPDX-License-Identifier: GPL-3.0-or-later
//! Generated from `docs/t-story-event-verbs.md#story_kind_33_table_field_predicate_t2485`.
//! Weak Tier 1 evidence is fabricated and does not satisfy the replay adoption gate.

use alloc::vec::Vec;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConditionRecord {
    pub kind: u32,
    pub args: [i32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoryConditionTableEntry {
    pub key: i32,
    pub value_0x18: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PortState {
    pub conditionrecord: Vec<ConditionRecord>,
    pub storyconditiontableentry: Option<StoryConditionTableEntry>,
    pub result: Option<bool>,
}

/// Candidate lane roots found in the spec. Confirm root ABI before registration.
pub const LANE_ROOT_CANDIDATES: &[u32] = &[0x000B7600, 0x002359A0];

pub fn lane_root_selected(va: u32) -> bool {
    LANE_ROOT_CANDIDATES.contains(&va)
}

/// The retained kind-33 branch returns the dword predicate at table-entry +0x18.
pub const fn table_field_0x18_is_true(value: u32) -> bool {
    value != 0
}

/// Resolve only against the already-adapted typed entry. A missing or mismatched
/// entry declines the Rust result so the caller can use the retained lifted path.
pub fn evaluate_key(
    record: &ConditionRecord,
    entry: Option<&StoryConditionTableEntry>,
) -> Option<bool> {
    if record.kind != 33 {
        return None;
    }
    let entry = entry?;
    (entry.key == record.args[0]).then(|| table_field_0x18_is_true(entry.value_0x18))
}

pub fn port_step(state: &mut PortState) {
    state.result = state
        .conditionrecord
        .first()
        .and_then(|record| evaluate_key(record, state.storyconditiontableentry.as_ref()));
}
