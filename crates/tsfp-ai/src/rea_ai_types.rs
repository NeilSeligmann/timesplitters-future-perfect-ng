// SPDX-License-Identifier: GPL-3.0-or-later
//! Small shared REA IDs used by runtime adapters that compile selected owners.

use crate::{movement_approach::Position3, navigation_scoring::NodeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(pub u32);

/// Serialized alert group bits; kept distinct from selectors and node IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroupMask(pub u32);

/// A serialized/source key is not a loaded node index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey(pub i32);

/// Typed actor and target fields shared by the AI model and hybrid runtime.
///
/// The route-link algorithm remains in `rea_links`; this record lives in the
/// shared type module because `ReaTickState` is also compiled into `tsfp-ports`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkActorState {
    pub position: Position3,
    pub current_node: NodeId,
    pub goal_node: NodeId,
    pub movement_target: Position3,
}
