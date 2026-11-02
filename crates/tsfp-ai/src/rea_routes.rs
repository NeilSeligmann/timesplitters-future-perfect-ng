// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned route records and the measured 0x26D690 / 0x26DE30 boundaries.
//!
//! Route records are valid loaded-world data. The original has no bounds checks;
//! this value model therefore indexes valid records directly instead of defining
//! behavior for malformed cursors. Tier 1 differential coverage is in
//! `tests/t2385_rea.rs` for peek selection and cursor/goal advancement.

use crate::navigation_scoring::NodeId;
use crate::rea_ai_types::NodeKey;
pub use crate::rea_route_peek::peek_next_index;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RouteEntry {
    pub kind: u32,
    pub payload_a: u32,
    pub payload_b: u32,
}

/// The kind-13 route control is a full DWORD stored in the next entry's first
/// payload. Keep this distinct from node keys and loaded node IDs at the route
/// boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteControl13(pub u32);

impl RouteControl13 {
    /// Applies the measured wrapping `(cursor + 1) - payload` operation.
    pub const fn next_index(self, cursor: u32) -> u32 {
        cursor.wrapping_add(1).wrapping_sub(self.0)
    }
}

/// A loaded route and its actor-owned cursor. `cursor` is a DWORD because the
/// original arithmetic wraps at 32 bits and the extension field is also used as
/// a float deadline by a different AI state variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteProgram {
    pub entries: Vec<RouteEntry>,
    pub cursor: u32,
}

impl RouteProgram {
    fn entry(&self, index: u32) -> RouteEntry {
        self.entries[index as usize]
    }

    /// 0x26D690: next-entry controls redirect the peek without mutating cursor.
    pub fn peek_next(&self) -> (u32, RouteEntry) {
        let next = self.cursor.wrapping_add(1);
        let entry = self.entry(next);
        let selected = peek_next_index(self.cursor, entry.kind, entry.payload_a);
        (selected, self.entry(selected))
    }

    /// 0x26DE30's cursor-only portion. The selected entry is returned after the
    /// cursor has already been changed, preserving the caller's ordered reads.
    pub fn advance_cursor(&mut self) -> RouteEntry {
        let control_index = self.cursor.wrapping_add(1);
        let control = self.entry(control_index);
        self.cursor = advance_route_index(self.cursor, control.kind, control.payload_a);
        self.entry(self.cursor)
    }
}

/// Cursor selection for the mutating route-step helper. Kept shared with
/// runtime snapshots so the adapter and owner cannot diverge on control rows.
pub const fn advance_route_index(cursor: u32, control_kind: u32, payload_a: u32) -> u32 {
    let next = cursor.wrapping_add(1);
    match control_kind {
        12 => 1,
        13 => RouteControl13(payload_a).next_index(cursor),
        18 => next.wrapping_add(1),
        _ => next,
    }
}

/// Bounded result domain consumed by the `0x26EA00` point-follow stage.
///
/// The original search result is signed. Keep the two measured negative
/// sentinels separate from loaded node IDs, and preserve other negative values
/// without assigning them invented behavior. This is a value contract only;
/// it does not establish route-table bounds or graph traversal safety.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointFollowSearchResult {
    /// `-2`: the measured alert and abort path.
    AlertAbort,
    /// `-1`: consume the cached record at actor extension `+0x67C`.
    CachedRecord,
    /// Nonnegative results are selected primary-node IDs.
    SelectedNode(NodeId),
    /// Other negative values have no bounded meaning in the retained entry.
    OtherNegative(i32),
}

/// Classifies the signed result used by the measured `0x26EA00` route stage.
///
/// Contract source: `docs/t1662-rea-executor.md`, “route-node bounds and
/// sentinels” and “route executor state-commit unknown”.
pub const fn classify_point_follow_search_result(raw: i32) -> PointFollowSearchResult {
    match raw {
        -2 => PointFollowSearchResult::AlertAbort,
        -1 => PointFollowSearchResult::CachedRecord,
        value if value >= 0 => PointFollowSearchResult::SelectedNode(NodeId(value as u32)),
        value => PointFollowSearchResult::OtherNegative(value),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NodeId, PointFollowSearchResult, RouteControl13, advance_route_index,
        classify_point_follow_search_result,
    };

    #[test]
    fn kind13_control_keeps_the_full_dword_and_wraps() {
        let control = RouteControl13(0x1_0001);
        assert_eq!(control.next_index(0), 0xffff_0000);
        assert_eq!(advance_route_index(0, 13, control.0), 0xffff_0000);
    }

    #[test]
    fn point_follow_search_results_keep_signed_sentinels_distinct_from_nodes() {
        assert_eq!(
            classify_point_follow_search_result(-2),
            PointFollowSearchResult::AlertAbort
        );
        assert_eq!(
            classify_point_follow_search_result(-1),
            PointFollowSearchResult::CachedRecord
        );
        assert_eq!(
            classify_point_follow_search_result(0),
            PointFollowSearchResult::SelectedNode(NodeId(0))
        );
        assert_eq!(
            classify_point_follow_search_result(i32::MAX),
            PointFollowSearchResult::SelectedNode(NodeId(i32::MAX as u32))
        );
        assert_eq!(
            classify_point_follow_search_result(i32::MIN),
            PointFollowSearchResult::OtherNegative(i32::MIN)
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteActor {
    pub current_node: NodeId,
    pub goal_node: NodeId,
    pub intermediate_node: Option<NodeId>,
    pub reverse_trace_within: u32,
    pub reverse_trace_other: u32,
}

impl Default for RouteActor {
    fn default() -> Self {
        Self {
            current_node: NodeId(0),
            goal_node: NodeId(0),
            intermediate_node: None,
            reverse_trace_within: 0,
            reverse_trace_other: 0,
        }
    }
}

impl RouteActor {
    /// Refreshes the loaded current-node ID. The measured actor refresh clears
    /// reverse-trace counters only when this ID changes; callers project the
    /// returned change across the typed seam before updating other fields.
    pub fn refresh_current_node(&mut self, current_node: NodeId) -> bool {
        if self.current_node == current_node {
            return false;
        }
        self.current_node = current_node;
        self.reverse_trace_within = 0;
        self.reverse_trace_other = 0;
        true
    }
}

/// World services remain explicit; route state and typed node IDs stay Rust-owned.
pub trait RouteWorld {
    fn resolve_key(&mut self, key: NodeKey) -> Option<NodeId>;
    fn player_current_node(&mut self) -> NodeId;
    fn find_intermediate(&mut self, current: NodeId, target: NodeId) -> i32;
    fn abort_route(&mut self);
}

/// 0x26DE30 route advancement and goal assignment for the documented route
/// record boundary. The 16/17 intermediate search retains its negative-result
/// abort after the cursor has moved; no unconditional cleanup is added.
pub fn advance_route<W: RouteWorld>(
    actor: &mut RouteActor,
    route: &mut RouteProgram,
    world: &mut W,
) -> RouteEntry {
    let selected = route.advance_cursor();
    if matches!(selected.kind, 16 | 17) {
        let (_, next) = route.peek_next();
        let target = world
            .resolve_key(NodeKey(next.payload_b as i32))
            .unwrap_or_else(|| world.player_current_node());
        let intermediate = world.find_intermediate(actor.current_node, target);
        if intermediate < 0 {
            world.abort_route();
            return selected;
        }
        actor.intermediate_node = Some(NodeId(intermediate as u32));
    }

    let key = if selected.kind == 1 {
        selected.payload_b
    } else {
        selected.payload_a
    };
    actor.goal_node = world
        .resolve_key(NodeKey(key as i32))
        .unwrap_or_else(|| world.player_current_node());
    actor.reverse_trace_within = 0;
    actor.reverse_trace_other = 0;
    selected
}
