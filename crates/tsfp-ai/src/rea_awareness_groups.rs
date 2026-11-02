// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned group headers and generation-scoped actor membership for REA's
//! twenty-group awareness table. This is native model state; conversion from
//! an original member pointer to a live actor identity remains a host seam.

use alloc::vec::Vec;

use crate::rea_group_alert::{GroupAlertWorld, MemberView};
use crate::{rea_ai_types::GroupMask, rea_tick_state::ReaActorIdentity};

pub const AWARENESS_GROUP_COUNT: usize = 20;
/// Value read from the original group timeout data. Its time unit is unknown.
pub const GROUP_ALERT_TIMEOUT_VALUE: f32 = 15.0;

/// A validated index into the fixed awareness group table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AwarenessGroupId(u8);

impl AwarenessGroupId {
    pub const fn new(value: u8) -> Option<Self> {
        if (value as usize) < AWARENESS_GROUP_COUNT {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Owned fields from one 0x60-byte original group record. Members are Rust
/// actor lifetimes, never retained guest pointers. The actor identity mapping
/// is an inferred native attachment until the runtime lifecycle seam is wired.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AwarenessGroupState {
    alerted: u32,
    alert_state: u32,
    timeout_value: f32,
    members: Vec<ReaActorIdentity>,
}

impl AwarenessGroupState {
    pub const fn alerted(&self) -> u32 {
        self.alerted
    }

    pub const fn alert_state(&self) -> u32 {
        self.alert_state
    }

    pub const fn timeout_value(&self) -> f32 {
        self.timeout_value
    }

    pub fn members(&self) -> &[ReaActorIdentity] {
        &self.members
    }

    pub fn replace_members(&mut self, members: Vec<ReaActorIdentity>) {
        self.members = members;
    }

    pub fn add_member(&mut self, member: ReaActorIdentity) {
        self.members.push(member);
    }

    /// Applies the measured selector-1 group-header writes. The timeout value
    /// is copied from pinned static data; its units and runtime decay are open.
    pub fn mark_alerted(&mut self) {
        self.alerted = 1;
        self.alert_state = 1;
        self.timeout_value = GROUP_ALERT_TIMEOUT_VALUE;
    }
}

/// Rust-owned fixed group table corresponding to 0x7A2D80 (20 records, 0x60
/// stride). Member vectors stay dynamic because the measured traversal does
/// not establish a capacity bound.
#[derive(Clone, Debug, PartialEq)]
pub struct ReaAwarenessGroupTable {
    groups: [AwarenessGroupState; AWARENESS_GROUP_COUNT],
}

impl ReaAwarenessGroupTable {
    pub fn new() -> Self {
        Self {
            groups: core::array::from_fn(|_| AwarenessGroupState::default()),
        }
    }

    pub fn group(&self, id: AwarenessGroupId) -> &AwarenessGroupState {
        &self.groups[id.index()]
    }

    pub fn group_mut(&mut self, id: AwarenessGroupId) -> &mut AwarenessGroupState {
        &mut self.groups[id.index()]
    }

    /// Marks selected group headers in the same ascending order as the native
    /// twenty-bit traversal. Bits above the measured table are ignored.
    pub fn mark_selected_alerted(&mut self, mask: GroupMask) -> usize {
        let mut marked = 0;
        for index in 0..AWARENESS_GROUP_COUNT {
            if mask.0 & (1 << index) == 0 {
                continue;
            }
            self.groups[index].mark_alerted();
            marked += 1;
        }
        marked
    }
}

impl Default for ReaAwarenessGroupTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Typed callbacks needed to run the measured member sweep against Rust-owned
/// group membership. Actor identity is explicit at every recipient boundary;
/// no original extension or member pointer is retained.
pub trait OwnedAwarenessMemberServices {
    fn member_view(&self, actor: ReaActorIdentity) -> MemberView;
    fn linked_object_node(&self, actor: ReaActorIdentity) -> u32;
    fn set_member_target(&mut self, actor: ReaActorIdentity, node: u32);
    fn set_member_local_flags(&mut self, actor: ReaActorIdentity, flags: u32);
    fn set_member_group_mask(&mut self, actor: ReaActorIdentity, mask: u32);
    fn class_listens(&mut self, selector: i16) -> bool;
    fn candidate_check(&mut self, actor: ReaActorIdentity) -> bool;
    fn actor_busy(&mut self, actor: ReaActorIdentity) -> bool;
    fn raise(&mut self, actor: ReaActorIdentity, code: u32, three_args: bool);
    /// The callback may update typed membership before the sweep re-reads its
    /// dynamic member count. The original callback's exact list mutation
    /// behavior remains open.
    fn leave_groups(
        &mut self,
        table: &mut ReaAwarenessGroupTable,
        actor: ReaActorIdentity,
        groups: u32,
    );
}

/// Adapter from the legacy group/index sweep API to generation-keyed Rust
/// membership and typed actor services.
pub struct OwnedAwarenessGroupWorld<'a, S> {
    table: &'a mut ReaAwarenessGroupTable,
    services: &'a mut S,
}

impl<'a, S> OwnedAwarenessGroupWorld<'a, S> {
    pub fn new(table: &'a mut ReaAwarenessGroupTable, services: &'a mut S) -> Self {
        Self { table, services }
    }

    fn group_id(group: u32) -> Option<AwarenessGroupId> {
        u8::try_from(group).ok().and_then(AwarenessGroupId::new)
    }

    fn actor(&self, group: u32, index: usize) -> ReaActorIdentity {
        let id = Self::group_id(group).expect("group alert index must fit its fixed table");
        *self
            .table
            .group(id)
            .members()
            .get(index)
            .expect("group alert index must name an owned member")
    }
}

impl<S: OwnedAwarenessMemberServices> GroupAlertWorld for OwnedAwarenessGroupWorld<'_, S> {
    fn member_count(&self, group: u32) -> usize {
        Self::group_id(group)
            .map(|id| self.table.group(id).members().len())
            .unwrap_or(0)
    }

    fn mark_group_alerted(&mut self, group: u32) {
        if let Some(id) = Self::group_id(group) {
            self.table.group_mut(id).mark_alerted();
        }
    }

    fn member(&self, group: u32, index: usize) -> MemberView {
        let actor = self.actor(group, index);
        self.services.member_view(actor)
    }

    fn linked_object_node(&self, group: u32, index: usize) -> u32 {
        let actor = self.actor(group, index);
        self.services.linked_object_node(actor)
    }

    fn set_member_target(&mut self, group: u32, index: usize, node: u32) {
        let actor = self.actor(group, index);
        self.services.set_member_target(actor, node);
    }

    fn set_member_local_flags(&mut self, group: u32, index: usize, flags: u32) {
        let actor = self.actor(group, index);
        self.services.set_member_local_flags(actor, flags);
    }

    fn set_member_group_mask(&mut self, group: u32, index: usize, mask: u32) {
        let actor = self.actor(group, index);
        self.services.set_member_group_mask(actor, mask);
    }

    fn class_listens(&mut self, selector: i16) -> bool {
        self.services.class_listens(selector)
    }

    fn candidate_check(&mut self, group: u32, index: usize) -> bool {
        let actor = self.actor(group, index);
        self.services.candidate_check(actor)
    }

    fn actor_busy(&mut self, group: u32, index: usize) -> bool {
        let actor = self.actor(group, index);
        self.services.actor_busy(actor)
    }

    fn raise(&mut self, group: u32, index: usize, code: u32, three_args: bool) {
        let actor = self.actor(group, index);
        self.services.raise(actor, code, three_args);
    }

    fn leave_groups(&mut self, group: u32, index: usize, groups: u32) {
        let actor = self.actor(group, index);
        self.services.leave_groups(self.table, actor, groups);
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    fn group(index: u8) -> AwarenessGroupId {
        AwarenessGroupId::new(index).expect("valid group index")
    }

    #[test]
    fn selected_groups_own_measured_header_values_in_table_order() {
        let mut table = ReaAwarenessGroupTable::new();
        let marked = table.mark_selected_alerted(GroupMask((1 << 0) | (1 << 3) | (1 << 19)));

        assert_eq!(marked, 3);
        for index in [0, 3, 19] {
            let selected = table.group(group(index));
            assert_eq!(selected.alerted(), 1);
            assert_eq!(selected.alert_state(), 1);
            assert_eq!(selected.timeout_value(), GROUP_ALERT_TIMEOUT_VALUE);
        }
        let skipped = table.group(group(2));
        assert_eq!(skipped.alerted(), 0);
        assert_eq!(skipped.alert_state(), 0);
        assert_eq!(skipped.timeout_value(), 0.0);
    }

    #[test]
    fn membership_owns_generation_scoped_actor_identities() {
        let mut table = ReaAwarenessGroupTable::new();
        let first = ReaActorIdentity {
            slot: 7,
            generation: 1,
        };
        let replacement = ReaActorIdentity {
            slot: 7,
            generation: 2,
        };
        table.group_mut(group(4)).add_member(first);
        table.group_mut(group(4)).add_member(replacement);

        assert_eq!(table.group(group(4)).members(), &[first, replacement]);
    }

    #[test]
    fn replacing_members_replaces_only_the_selected_group() {
        let mut table = ReaAwarenessGroupTable::new();
        let actor = ReaActorIdentity {
            slot: 2,
            generation: 5,
        };
        table.group_mut(group(1)).add_member(actor);
        table.group_mut(group(1)).replace_members(vec![actor]);

        assert_eq!(table.group(group(1)).members(), &[actor]);
        assert!(table.group(group(2)).members().is_empty());
    }

    #[test]
    fn group_id_rejects_values_outside_the_measured_table() {
        assert!(AwarenessGroupId::new(19).is_some());
        assert!(AwarenessGroupId::new(20).is_none());
    }

    #[derive(Default)]
    struct MemberServices {
        members: Vec<(ReaActorIdentity, MemberView)>,
        targets: Vec<(ReaActorIdentity, u32)>,
        local_flags: Vec<(ReaActorIdentity, u32)>,
    }

    impl OwnedAwarenessMemberServices for MemberServices {
        fn member_view(&self, actor: ReaActorIdentity) -> MemberView {
            self.members
                .iter()
                .find_map(|(identity, view)| (*identity == actor).then_some(*view))
                .expect("member view exists for each owned identity")
        }

        fn linked_object_node(&self, _actor: ReaActorIdentity) -> u32 {
            0
        }

        fn set_member_target(&mut self, actor: ReaActorIdentity, node: u32) {
            self.targets.push((actor, node));
        }

        fn set_member_local_flags(&mut self, actor: ReaActorIdentity, flags: u32) {
            self.local_flags.push((actor, flags));
        }

        fn set_member_group_mask(&mut self, _actor: ReaActorIdentity, _mask: u32) {}
        fn class_listens(&mut self, _selector: i16) -> bool {
            false
        }
        fn candidate_check(&mut self, _actor: ReaActorIdentity) -> bool {
            false
        }
        fn actor_busy(&mut self, _actor: ReaActorIdentity) -> bool {
            false
        }
        fn raise(&mut self, _actor: ReaActorIdentity, _code: u32, _three_args: bool) {}
        fn leave_groups(
            &mut self,
            _table: &mut ReaAwarenessGroupTable,
            _actor: ReaActorIdentity,
            _groups: u32,
        ) {
        }
    }

    #[test]
    fn owned_group_sweep_keeps_reused_slots_generation_distinct() {
        use crate::rea_group_alert::{ACTOR_ALERTABLE, TargetChoice, alert_group_members};

        let selected = group(4);
        let mask = 1 << selected.index();
        let first = ReaActorIdentity {
            slot: 7,
            generation: 1,
        };
        let replacement = ReaActorIdentity {
            slot: 7,
            generation: 2,
        };
        let mut table = ReaAwarenessGroupTable::new();
        table
            .group_mut(selected)
            .replace_members(vec![first, replacement]);
        let make_view = |id| MemberView {
            id,
            actor_flags: ACTOR_ALERTABLE,
            local_flags: 0,
            listen_flags: 0,
            behavior_flags: 0,
            group_mask: mask,
            class_selector: 0,
        };
        let mut services = MemberServices {
            members: vec![(first, make_view(11)), (replacement, make_view(12))],
            ..MemberServices::default()
        };

        alert_group_members(
            &mut OwnedAwarenessGroupWorld::new(&mut table, &mut services),
            mask,
            u32::MAX,
            TargetChoice::Fixed(0x1234),
        );

        let state = table.group(selected);
        assert_eq!(state.alerted(), 1);
        assert_eq!(state.alert_state(), 1);
        assert_eq!(state.timeout_value(), GROUP_ALERT_TIMEOUT_VALUE);
        assert_eq!(
            services.targets,
            vec![(first, 0x1234), (replacement, 0x1234)]
        );
        assert_eq!(
            services.local_flags,
            vec![
                (first, crate::rea_group_alert::LOCAL_ALERTED),
                (replacement, crate::rea_group_alert::LOCAL_ALERTED)
            ]
        );
    }
}
