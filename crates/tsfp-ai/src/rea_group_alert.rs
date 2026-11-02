// SPDX-License-Identifier: GPL-3.0-or-later
//! Group alert member sweep of original `0x267FF0` (T2385 awareness branch).
//!
//! The original resolves an alert (actor, source record 1 or source record 2)
//! into a group mask, an excluded actor id and a target node, then visits every
//! member of every group whose bit is set (20 groups, record stride `0x60` at
//! `0x7A2D88`). This module is the member sweep only. Evidence: REA/Ghidra
//! static read of `0x267FF0` (MEASURED static, sha256 e99faaeb...f866f).

/// Group flag bit in the actor extension `+0x10` word (alert-capable actor).
pub const ACTOR_ALERTABLE: u32 = 0x0100_0000;
/// Alerted bit in the extension `+0x28` word.
pub const LOCAL_ALERTED: u32 = 0x400;
/// Behavior flag `+0x54` bit 3: ignore the candidate check.
pub const BEHAVIOR_SKIP_CHECK: u32 = 0x8;
/// Behavior flag `+0x54` bit 14: take the raise-0x200 path.
pub const BEHAVIOR_RAISE_FULL: u32 = 0x4000;
/// Number of alert groups in the table.
pub const GROUP_COUNT: u32 = 20;

/// Member state read from one extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberView {
    pub id: u32,
    pub actor_flags: u32,
    pub local_flags: u32,
    /// Extension `+0x14` (bit 0: listens to the group check).
    pub listen_flags: u32,
    pub behavior_flags: u32,
    pub group_mask: u32,
    pub class_selector: i16,
}

/// Where the target node comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetChoice {
    /// Kind 0: the caller supplied node.
    Fixed(u32),
    /// Kinds 1 and 2: each newly alerted member reads the node from its own
    /// linked object (`MemberWorld::linked_object_node`).
    FromMemberObject,
}

/// What the sweep needs from the game state.
pub trait GroupAlertWorld {
    /// Re-read on every iteration, as the original does.
    fn member_count(&self, group: u32) -> usize;
    fn mark_group_alerted(&mut self, group: u32);
    fn member(&self, group: u32, index: usize) -> MemberView;
    fn linked_object_node(&self, group: u32, index: usize) -> u32;
    fn set_member_target(&mut self, group: u32, index: usize, node: u32);
    fn set_member_local_flags(&mut self, group: u32, index: usize, flags: u32);
    fn set_member_group_mask(&mut self, group: u32, index: usize, mask: u32);
    /// `0xD6DD0(class_selector)`.
    fn class_listens(&mut self, selector: i16) -> bool;
    /// `0x15E120(member, 0)`.
    fn candidate_check(&mut self, group: u32, index: usize) -> bool;
    /// `0x59E00(actor, 0x20, 0)`.
    fn actor_busy(&mut self, group: u32, index: usize) -> bool;
    /// `0x17570(member, code, ...)`; `three_args` is false for code 0x200.
    fn raise(&mut self, group: u32, index: usize, code: u32, three_args: bool);
    /// `0x160CF0(actor, groups)`.
    fn leave_groups(&mut self, group: u32, index: usize, groups: u32);
}

/// Sweep all groups in `mask` and alert their members.
pub fn alert_group_members<W: GroupAlertWorld>(
    world: &mut W,
    mask: u32,
    excluded_id: u32,
    target: TargetChoice,
) {
    for group in 0..GROUP_COUNT {
        if mask & (1 << group) == 0 {
            continue;
        }
        world.mark_group_alerted(group);
        let mut index = 0;
        while index < world.member_count(group) {
            alert_member(world, group, index, mask, excluded_id, target);
            index += 1;
        }
    }
}

fn alert_member<W: GroupAlertWorld>(
    world: &mut W,
    group: u32,
    index: usize,
    mask: u32,
    excluded_id: u32,
    target: TargetChoice,
) {
    let member = world.member(group, index);
    if member.id == excluded_id {
        return;
    }
    let alertable = member.actor_flags & ACTOR_ALERTABLE != 0;
    let alerted = member.local_flags & LOCAL_ALERTED != 0;
    if alertable && !alerted {
        let node = match target {
            TargetChoice::Fixed(node) => node,
            TargetChoice::FromMemberObject => world.linked_object_node(group, index),
        };
        world.set_member_target(group, index, node);
        world.set_member_local_flags(group, index, member.local_flags | LOCAL_ALERTED);
        let stale = !mask & member.group_mask;
        if stale != 0 {
            world.leave_groups(group, index, stale);
            world.set_member_group_mask(group, index, member.group_mask & mask);
        }
        return;
    }
    if !alertable || member.behavior_flags & BEHAVIOR_SKIP_CHECK != 0 || !alerted {
        if member.listen_flags & 1 != 0 && world.class_listens(member.class_selector) {
            world.set_member_local_flags(group, index, member.local_flags | LOCAL_ALERTED);
        }
        return;
    }
    if !world.candidate_check(group, index) {
        if !world.actor_busy(group, index) {
            world.raise(group, index, 8, true);
        }
    } else if member.behavior_flags & BEHAVIOR_RAISE_FULL == 0 {
        if !world.actor_busy(group, index) {
            world.raise(group, index, 4, true);
        }
    } else {
        world.raise(group, index, 0x200, false);
    }
}
