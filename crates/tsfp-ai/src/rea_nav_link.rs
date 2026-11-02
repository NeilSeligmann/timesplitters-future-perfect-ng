// SPDX-License-Identifier: GPL-3.0-or-later
//! Nav link advance of original `0x15D500(actor, requested_node)` (T2385 route executor).
//!
//! The route executor calls this once per tick for an actor that follows a nav
//! graph. It resolves the goal node (an id in `0x1F40..0x20B2` names a spawn
//! record whose key maps to a node), asks the link tracer (`0x1596F0`) for the
//! next hop, copies the hop position to the movement target and, when the
//! tracer reports a node the actor has already reached, commits the link
//! (`0x15A180`). Evidence: REA/Ghidra static read of `0x15D500` (MEASURED
//! static, sha256 e99faaeb...f866f), cross-checked against the lifted c2r body.

/// Node id: index into the node table at `0x715F2C` (stride 32). Negative
/// values are the tracer's sentinels.
pub type NodeId = i32;
/// Guest pointer of a spawn record (`0x9A420` result), 0 when absent.
pub type RecordRef = u32;
/// Position as raw float bits, copied verbatim to the movement target.
pub type PositionBits = [u32; 3];

/// First and one-past-last requested ids that name spawn records.
pub const RECORD_ID_RANGE: core::ops::Range<i32> = 0x1F40..0x20B2;
/// Tracer result: no link, retry without tracing.
pub const LINK_RETRY: NodeId = -2;
/// Tracer result: the actor stands on the goal node.
pub const LINK_ARRIVED: NodeId = -1;
/// Local flag bit cleared at entry in the extension `+0x28` word.
pub const LOCAL_LINK_PENDING: u32 = 0x800;
/// Extension `+0x2C` bit 8: use the tighter hold radius.
pub const FLAG_TIGHT_HOLD: u32 = 0x100;

/// Float constants the original reads from its data segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavLinkConstants {
    /// `0x475D24`, hold radius when `FLAG_TIGHT_HOLD` is set.
    pub hold_radius_tight: f32,
    /// `0x47D568`, hold radius otherwise.
    pub hold_radius_loose: f32,
    /// `0x47DD90`, squared arrival distance to a record position.
    pub arrive_distance_sq: f32,
}

/// Actor state and callee services the advance needs.
pub trait NavLinkWorld {
    fn constants(&self) -> NavLinkConstants;
    // Actor extension fields.
    fn clear_local_flags(&mut self, mask: u32);
    fn local_flags_2c(&self) -> u32;
    /// `+0xACC`: spawn record the actor is holding for, 0 when none.
    fn held_record(&self) -> RecordRef;
    fn set_held_record(&mut self, record: RecordRef);
    /// `+0x684`: goal node.
    fn goal(&self) -> NodeId;
    fn set_goal(&mut self, goal: NodeId);
    /// `+0x9A8` and `+0x9AC`: pending link counters.
    fn pending(&self) -> (u32, u32);
    fn set_pending(&mut self, first: u32, second: u32);
    /// `+0x678`: current node.
    fn current(&self) -> NodeId;
    fn set_current(&mut self, node: NodeId);
    /// `+0x680`: last traced link.
    fn link(&self) -> NodeId;
    fn set_link(&mut self, link: NodeId);
    /// `+0x9B0`: squared distance to the hold point.
    fn hold_distance_sq(&self) -> f32;
    fn set_target(&mut self, position: PositionBits);
    /// Actor position (`+0x64..+0x6C` of the object).
    fn actor_position(&self) -> [f32; 3];
    /// Sets bit 0 and clears bits 1..2, 4 of ext `+0x24` and copies `+0xB14` to `+0xAD0`.
    fn enter_follow_state(&mut self);
    fn decrement_pending_first(&mut self);
    // Tables.
    fn node_position(&self, node: NodeId) -> PositionBits;
    fn node_key(&self, node: NodeId) -> i32;
    fn record_key(&self, record: RecordRef) -> i16;
    fn record_position(&self, record: RecordRef) -> PositionBits;
    fn player_node(&self) -> NodeId;
    fn first_actor_node(&self) -> NodeId;
    // Callee services.
    /// `0x9A420(id)`.
    fn find_record(&mut self, id: i32) -> RecordRef;
    /// `0x9CC40(key)`.
    fn resolve_key(&mut self, key: i16) -> NodeId;
    /// `0x1596F0(actor, &current, trace)`.
    fn trace_link(&mut self, trace: bool) -> NodeId;
    /// `0x160B00(enabled)`.
    fn set_trace_gate(&mut self, enabled: bool);
    /// `0x15D490(actor)`.
    fn abort_link(&mut self);
    /// `0x15A050(actor)`.
    fn refresh_target(&mut self);
    /// `0x9A4C0(node)`.
    fn node_entry_id(&mut self, node: NodeId) -> i32;
    /// `0x15A180(actor, current, link)`, true when the link was taken.
    fn commit_link(&mut self, from: NodeId, link: NodeId) -> bool;
    /// `0x15BE70(actor)`.
    fn finish_step(&mut self);
}

struct PartialRouteRetryAdapter<'a, W> {
    world: &'a mut W,
}

impl<W: NavLinkWorld> crate::rea_partial_route_retry::PartialRouteRetryWorld
    for PartialRouteRetryAdapter<'_, W>
{
    fn search_route(&mut self, trace: bool) -> i32 {
        let result = self.world.trace_link(trace);
        self.world.set_link(result);
        result
    }

    fn set_search_switch(&mut self, enabled: bool) {
        self.world.set_trace_gate(enabled);
    }

    fn clear_reverse_trace_counters(&mut self) {
        self.world.set_pending(0, 0);
    }

    fn stop_actor(&mut self) {
        self.world.abort_link();
    }
}

fn squared_distance(actor: [f32; 3], target: PositionBits) -> f32 {
    let dx = actor[0] - f32::from_bits(target[0]);
    let dy = actor[1] - f32::from_bits(target[1]);
    let dz = actor[2] - f32::from_bits(target[2]);
    dx * dx + dy * dy + dz * dz
}

/// Returns the original's result: true when the actor reached its hold point
/// or goal this tick (the caller stops walking), false otherwise.
pub fn advance_nav_link<W: NavLinkWorld>(world: &mut W, requested: i32) -> bool {
    world.clear_local_flags(LOCAL_LINK_PENDING);
    let mut held: RecordRef = 0;
    let mut goal = requested;
    if RECORD_ID_RANGE.contains(&requested) {
        held = world.find_record(requested);
        goal = if held == 0 {
            world.player_node()
        } else {
            let key = world.record_key(held);
            world.resolve_key(key)
        };
    }

    let holding = world.held_record();
    let pending = world.pending();
    let keep_tracing = holding == 0
        || holding == held
        || (world.goal() == goal && (pending.0 != 0 || pending.1 != 0));
    if !keep_tracing {
        return hold_at_current(world);
    }

    retarget_goal(world, held, goal);
    let search = {
        let mut retry_world = PartialRouteRetryAdapter { world };
        crate::rea_partial_route_retry::search_with_one_replan(&mut retry_world)
    };
    // Lifted 0x15D500 returns from the retry branch before projecting its result
    // into target position or committing the link, even when the retry succeeds.
    if search.retried {
        return false;
    }
    let link = match search.outcome {
        crate::rea_partial_route_retry::PartialRouteOutcome::Arrived => LINK_ARRIVED,
        crate::rea_partial_route_retry::PartialRouteOutcome::NextNode(node) => node.id.0 as i32,
        crate::rea_partial_route_retry::PartialRouteOutcome::OtherSearchResult(raw) => raw,
        crate::rea_partial_route_retry::PartialRouteOutcome::NoRouteAfterRetry => return false,
    };
    if link >= 0 {
        world.set_target(world.node_position(link));
        world.refresh_target();
    }
    if world.link() == LINK_ARRIVED {
        if let Some(result) = arrive(world, held) {
            return result;
        }
    }
    world.enter_follow_state();
    let (current, link) = (world.current(), world.link());
    if world.commit_link(current, link) {
        world.set_held_record(0);
        world.set_current(link);
        world.decrement_pending_first();
    }
    world.finish_step();
    false
}

/// Moves the stored goal to `goal` unless it already belongs to the same record.
fn retarget_goal<W: NavLinkWorld>(world: &mut W, held: RecordRef, goal: NodeId) {
    let stored = world.goal();
    if stored == goal {
        return;
    }
    if held != 0 && world.find_record(stored) == held {
        return;
    }
    let new_goal = if goal < 0 {
        world.first_actor_node()
    } else {
        goal
    };
    world.set_goal(new_goal);
    world.set_pending(0, 0);
}

/// Tracer reports the actor is on its goal. `None` means the goal does not
/// match and the caller continues with the ordinary link commit.
fn arrive<W: NavLinkWorld>(world: &mut W, held: RecordRef) -> Option<bool> {
    let current = world.current();
    let at_goal = current == world.goal()
        || (held != 0 && world.node_entry_id(current) == i32::from(world.record_key(held)));
    if !at_goal {
        return None;
    }
    if held != 0 && i32::from(world.record_key(held)) as u32 == world.node_key(current) as u32 {
        let position = world.record_position(held);
        world.set_target(position);
        if squared_distance(world.actor_position(), position)
            <= world.constants().arrive_distance_sq
        {
            world.set_held_record(held);
            return Some(true);
        }
        world.enter_follow_state();
        return Some(false);
    }
    world.set_held_record(0);
    Some(true)
}

/// Another record owns the actor and the goal is not shared: hold at the
/// current node until the hold point is reached.
fn hold_at_current<W: NavLinkWorld>(world: &mut W) -> bool {
    let constants = world.constants();
    let radius = if world.local_flags_2c() & FLAG_TIGHT_HOLD == 0 {
        constants.hold_radius_loose
    } else {
        constants.hold_radius_tight
    };
    let current = world.current();
    // `!(distance > radius^2)` also holds for NaN, as in the original.
    if !(world.hold_distance_sq() > radius * radius) {
        world.set_held_record(0);
        return false;
    }
    world.set_target(world.node_position(current));
    world.enter_follow_state();
    false
}
