// SPDX-License-Identifier: GPL-3.0-or-later
//! Owned composition of the measured `0x25FAF0` source pass and its nested
//! selector-1 `0x267FF0` group alert.

use alloc::vec::Vec;

use crate::{
    rea_ai_types::GroupMask,
    rea_awareness_groups::{
        AwarenessGroupId, OwnedAwarenessGroupWorld, OwnedAwarenessMemberServices,
        ReaAwarenessGroupTable,
    },
    rea_awareness_sources::{
        AlertSource, AwarenessSourceRecord, OwnedAwarenessAlertEffects, ReaAwarenessSourceStore,
        process_owned_awareness_sources,
    },
    rea_group_alert::{GroupAlertWorld, TargetChoice, alert_group_members},
    rea_tick_state::ReaActorIdentity,
};

const GROUP_ALERT_MODE: u8 = 0x0a;
const GROUP_ALERT_SUPPRESS_FLAG: u32 = 0x4000_0000;

const fn group_alert_entry_enabled(mode_byte: u8, global_flags: u32) -> bool {
    mode_byte == GROUP_ALERT_MODE && global_flags & GROUP_ALERT_SUPPRESS_FLAG == 0
}

/// Global alert bookkeeping remains an explicit attachment to the owned
/// source and group state.
pub trait AwarenessPipelineBookkeeping {
    /// Current `[0x7DE455]`, sampled when the nested selector-1 call enters.
    fn game_mode_byte(&self) -> u8;
    /// Current `[0x7DE458]`, sampled when the nested selector-1 call enters.
    fn global_flags(&self) -> u32;
    fn set_group_alert_value(&mut self, value: u32);
    fn register_spawn_entry(&mut self, index: usize);
    fn increment_alert_count(&mut self);
}

/// Rust-owned state for the measured awareness source and group sweeps.
///
/// The original's twenty group headers, actor memberships and source records
/// live here; object resolution, actor fields, and callback effects cross the
/// `OwnedAwarenessMemberServices` and `AwarenessPipelineBookkeeping` seams.
/// Actor identity conversion and callback-time membership lifetime remain
/// explicit attachment boundaries, so this is an owned model, not a runtime
/// adoption claim for the whole AI tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OwnedAwarenessState {
    sources: ReaAwarenessSourceStore,
    groups: ReaAwarenessGroupTable,
}

impl OwnedAwarenessState {
    pub fn new(sources: ReaAwarenessSourceStore, groups: ReaAwarenessGroupTable) -> Self {
        Self { sources, groups }
    }

    pub fn from_sources(records: Vec<AwarenessSourceRecord>) -> Self {
        Self::new(
            ReaAwarenessSourceStore::new(records),
            ReaAwarenessGroupTable::new(),
        )
    }

    pub fn sources(&self) -> &ReaAwarenessSourceStore {
        &self.sources
    }

    pub fn groups(&self) -> &ReaAwarenessGroupTable {
        &self.groups
    }

    pub fn set_group_members(&mut self, group: AwarenessGroupId, members: Vec<ReaActorIdentity>) {
        self.groups.group_mut(group).replace_members(members);
    }

    /// Run one owned awareness source pass and its nested selector-1 group
    /// sweeps in the measured order.
    pub fn process_tick<S, B>(
        &mut self,
        incoming: GroupMask,
        services: &mut S,
        bookkeeping: &mut B,
    ) -> usize
    where
        S: OwnedAwarenessMemberServices,
        B: AwarenessPipelineBookkeeping,
    {
        let mut groups = OwnedAwarenessGroupWorld::new(&mut self.groups, services);
        process_owned_awareness_pipeline(incoming, &mut self.sources, &mut groups, bookkeeping)
    }
}

/// Run the source-record pass with its measured nested selector-1 group sweep.
///
/// The source store and the `GroupAlertWorld` implementation are supplied as
/// Rust state. `GroupAlertWorld` callbacks remain the typed edges to linked
/// object resolution, membership notification, candidate checks and behavior
/// dispatch. Each nested selector-1 call applies `0x267FF0`'s measured
/// mode/global-flag guard at its own entry. The caller of this source pass
/// remains responsible for the guard on the outer selector-0 or selector-2
/// call.
pub fn process_owned_awareness_pipeline<W, B>(
    incoming: GroupMask,
    sources: &mut ReaAwarenessSourceStore,
    groups: &mut W,
    bookkeeping: &mut B,
) -> usize
where
    W: GroupAlertWorld,
    B: AwarenessPipelineBookkeeping,
{
    let mut effects = PipelineEffects {
        groups,
        bookkeeping,
    };
    process_owned_awareness_sources(incoming, sources, &mut effects)
}

struct PipelineEffects<'a, W, B> {
    groups: &'a mut W,
    bookkeeping: &'a mut B,
}

impl<W: GroupAlertWorld, B: AwarenessPipelineBookkeeping> OwnedAwarenessAlertEffects
    for PipelineEffects<'_, W, B>
{
    fn set_group_alert_value(&mut self, value: u32) {
        self.bookkeeping.set_group_alert_value(value);
    }

    fn dispatch_alert(&mut self, _sources: &mut ReaAwarenessSourceStore, source: AlertSource) {
        if !group_alert_entry_enabled(
            self.bookkeeping.game_mode_byte(),
            self.bookkeeping.global_flags(),
        ) {
            return;
        }
        match source {
            AlertSource::RecordOne { mask, .. } => {
                alert_group_members(
                    self.groups,
                    mask.0,
                    u32::MAX,
                    TargetChoice::FromMemberObject,
                );
            }
            _ => unreachable!("the owned source pass dispatches selector 1 records only"),
        }
    }

    fn register_spawn_entry(&mut self, index: usize) {
        self.bookkeeping.register_spawn_entry(index);
    }

    fn increment_alert_count(&mut self) {
        self.bookkeeping.increment_alert_count();
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, vec::Vec};

    use super::*;
    use crate::{
        navigation_scoring::ObjectId,
        rea_awareness_groups::OwnedAwarenessMemberServices,
        rea_awareness_sources::AwarenessSourceRecord,
        rea_group_alert::{ACTOR_ALERTABLE, MemberView},
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Event {
        Reset(u32),
        GroupAlerted(u32),
        LinkedNode(u32),
        Target(u32, u32),
        LocalFlags(u32, u32),
        LeaveGroups(u32, u32),
        GroupMask(u32, u32),
        MemberLinkedNode(ReaActorIdentity),
        MemberTarget(ReaActorIdentity, u32),
        MemberLocalFlags(ReaActorIdentity, u32),
        MemberLeaveGroups(ReaActorIdentity, u32),
        MemberGroupMask(ReaActorIdentity, u32),
        Register(usize),
        Count,
    }

    #[derive(Clone)]
    struct Log(Rc<RefCell<Vec<Event>>>);

    struct Groups {
        log: Log,
        member: MemberView,
    }

    impl GroupAlertWorld for Groups {
        fn member_count(&self, group: u32) -> usize {
            if group == 0 { 1 } else { 0 }
        }

        fn mark_group_alerted(&mut self, group: u32) {
            self.log.0.borrow_mut().push(Event::GroupAlerted(group));
        }

        fn member(&self, _group: u32, _index: usize) -> MemberView {
            self.member
        }

        fn linked_object_node(&self, group: u32, _index: usize) -> u32 {
            self.log.0.borrow_mut().push(Event::LinkedNode(group));
            42
        }

        fn set_member_target(&mut self, group: u32, _index: usize, node: u32) {
            self.log.0.borrow_mut().push(Event::Target(group, node));
        }

        fn set_member_local_flags(&mut self, group: u32, _index: usize, flags: u32) {
            self.log
                .0
                .borrow_mut()
                .push(Event::LocalFlags(group, flags));
        }

        fn set_member_group_mask(&mut self, group: u32, _index: usize, mask: u32) {
            self.log.0.borrow_mut().push(Event::GroupMask(group, mask));
        }

        fn class_listens(&mut self, _selector: i16) -> bool {
            false
        }

        fn candidate_check(&mut self, _group: u32, _index: usize) -> bool {
            false
        }

        fn actor_busy(&mut self, _group: u32, _index: usize) -> bool {
            false
        }

        fn raise(&mut self, _group: u32, _index: usize, _code: u32, _three_args: bool) {}

        fn leave_groups(&mut self, group: u32, _index: usize, removed: u32) {
            self.log
                .0
                .borrow_mut()
                .push(Event::LeaveGroups(group, removed));
        }
    }

    struct Bookkeeping {
        log: Log,
        mode: u8,
        flags: u32,
    }

    impl AwarenessPipelineBookkeeping for Bookkeeping {
        fn game_mode_byte(&self) -> u8 {
            self.mode
        }

        fn global_flags(&self) -> u32 {
            self.flags
        }

        fn set_group_alert_value(&mut self, value: u32) {
            self.log.0.borrow_mut().push(Event::Reset(value));
        }

        fn register_spawn_entry(&mut self, index: usize) {
            self.log.0.borrow_mut().push(Event::Register(index));
        }

        fn increment_alert_count(&mut self) {
            self.log.0.borrow_mut().push(Event::Count);
        }
    }

    struct OwnedMembers {
        log: Log,
        actor: ReaActorIdentity,
        member: MemberView,
    }

    impl OwnedAwarenessMemberServices for OwnedMembers {
        fn member_view(&self, actor: ReaActorIdentity) -> MemberView {
            assert_eq!(actor, self.actor);
            self.member
        }

        fn linked_object_node(&self, actor: ReaActorIdentity) -> u32 {
            assert_eq!(actor, self.actor);
            self.log.0.borrow_mut().push(Event::MemberLinkedNode(actor));
            42
        }

        fn set_member_target(&mut self, actor: ReaActorIdentity, node: u32) {
            assert_eq!(actor, self.actor);
            self.log
                .0
                .borrow_mut()
                .push(Event::MemberTarget(actor, node));
        }

        fn set_member_local_flags(&mut self, actor: ReaActorIdentity, flags: u32) {
            assert_eq!(actor, self.actor);
            self.log
                .0
                .borrow_mut()
                .push(Event::MemberLocalFlags(actor, flags));
        }

        fn set_member_group_mask(&mut self, actor: ReaActorIdentity, mask: u32) {
            assert_eq!(actor, self.actor);
            self.log
                .0
                .borrow_mut()
                .push(Event::MemberGroupMask(actor, mask));
        }

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
            actor: ReaActorIdentity,
            groups: u32,
        ) {
            assert_eq!(actor, self.actor);
            self.log
                .0
                .borrow_mut()
                .push(Event::MemberLeaveGroups(actor, groups));
        }
    }

    #[test]
    fn owned_awareness_state_composes_source_and_generation_keyed_group_passes() {
        let log = Log(Rc::new(RefCell::new(Vec::new())));
        let actor = ReaActorIdentity {
            slot: 7,
            generation: 4,
        };
        let group = AwarenessGroupId::new(0).unwrap();
        let mut group_table = ReaAwarenessGroupTable::new();
        group_table.group_mut(group).add_member(actor);
        let mut state = OwnedAwarenessState::new(
            ReaAwarenessSourceStore::new(vec![AwarenessSourceRecord {
                alert_mask: GroupMask(1),
                filter_mask: GroupMask(1),
                target: Some(ObjectId(77)),
                state: 0,
                elapsed: 8.0,
            }]),
            group_table,
        );
        let mut services = OwnedMembers {
            log: log.clone(),
            actor,
            member: MemberView {
                id: u32::from(actor.slot),
                actor_flags: ACTOR_ALERTABLE,
                local_flags: 0,
                listen_flags: 0,
                behavior_flags: 0,
                group_mask: 3,
                class_selector: 0,
            },
        };
        let mut bookkeeping = Bookkeeping {
            log: log.clone(),
            mode: GROUP_ALERT_MODE,
            flags: 0,
        };

        assert_eq!(
            state.process_tick(GroupMask(1), &mut services, &mut bookkeeping),
            1
        );
        assert_eq!(
            state.sources().records()[0],
            AwarenessSourceRecord {
                alert_mask: GroupMask(1),
                filter_mask: GroupMask(1),
                target: None,
                state: 1,
                elapsed: 0.0,
            }
        );
        let alerted = state.groups().group(group);
        assert_eq!(alerted.alerted(), 1);
        assert_eq!(alerted.alert_state(), 1);
        assert_eq!(
            alerted.timeout_value(),
            crate::rea_awareness_groups::GROUP_ALERT_TIMEOUT_VALUE
        );
        assert_eq!(
            *log.0.borrow(),
            vec![
                Event::Reset(2),
                Event::MemberLinkedNode(actor),
                Event::MemberTarget(actor, 42),
                Event::MemberLocalFlags(actor, crate::rea_group_alert::LOCAL_ALERTED),
                Event::MemberLeaveGroups(actor, 2),
                Event::MemberGroupMask(actor, 1),
                Event::Register(0),
                Event::Count,
            ]
        );
    }

    #[test]
    fn source_record_dispatches_owned_group_alert_before_registration_and_count() {
        let log = Log(Rc::new(RefCell::new(Vec::new())));
        let mut sources = ReaAwarenessSourceStore::new(vec![AwarenessSourceRecord {
            alert_mask: GroupMask(1),
            filter_mask: GroupMask(1),
            target: Some(ObjectId(77)),
            state: 0,
            elapsed: 8.0,
        }]);
        let mut groups = Groups {
            log: log.clone(),
            member: MemberView {
                id: 7,
                actor_flags: ACTOR_ALERTABLE,
                local_flags: 0,
                listen_flags: 0,
                behavior_flags: 0,
                group_mask: 3,
                class_selector: 0,
            },
        };
        let mut bookkeeping = Bookkeeping {
            log: log.clone(),
            mode: GROUP_ALERT_MODE,
            flags: 0,
        };

        assert_eq!(
            process_owned_awareness_pipeline(
                GroupMask(1),
                &mut sources,
                &mut groups,
                &mut bookkeeping,
            ),
            1
        );

        assert_eq!(
            sources.records()[0],
            AwarenessSourceRecord {
                alert_mask: GroupMask(1),
                filter_mask: GroupMask(1),
                target: None,
                state: 1,
                elapsed: 0.0,
            }
        );
        assert_eq!(
            *log.0.borrow(),
            vec![
                Event::Reset(2),
                Event::GroupAlerted(0),
                Event::LinkedNode(0),
                Event::Target(0, 42),
                Event::LocalFlags(0, crate::rea_group_alert::LOCAL_ALERTED),
                Event::LeaveGroups(0, 2),
                Event::GroupMask(0, 1),
                Event::Register(0),
                Event::Count,
            ]
        );
    }

    #[test]
    fn nested_group_alert_guard_skips_only_group_effects() {
        for (mode, flags) in [
            (GROUP_ALERT_MODE - 1, 0),
            (GROUP_ALERT_MODE, GROUP_ALERT_SUPPRESS_FLAG),
        ] {
            let log = Log(Rc::new(RefCell::new(Vec::new())));
            let mut sources = ReaAwarenessSourceStore::new(vec![AwarenessSourceRecord {
                alert_mask: GroupMask(1),
                filter_mask: GroupMask(1),
                target: Some(ObjectId(77)),
                state: 0,
                elapsed: 8.0,
            }]);
            let mut groups = Groups {
                log: log.clone(),
                member: MemberView {
                    id: 7,
                    actor_flags: ACTOR_ALERTABLE,
                    local_flags: 0,
                    listen_flags: 0,
                    behavior_flags: 0,
                    group_mask: 3,
                    class_selector: 0,
                },
            };
            let mut bookkeeping = Bookkeeping {
                log: log.clone(),
                mode,
                flags,
            };

            assert_eq!(
                process_owned_awareness_pipeline(
                    GroupMask(1),
                    &mut sources,
                    &mut groups,
                    &mut bookkeeping,
                ),
                1
            );
            assert_eq!(
                sources.records()[0],
                AwarenessSourceRecord {
                    alert_mask: GroupMask(1),
                    filter_mask: GroupMask(1),
                    target: None,
                    state: 1,
                    elapsed: 0.0,
                }
            );
            assert_eq!(
                *log.0.borrow(),
                vec![Event::Reset(2), Event::Register(0), Event::Count]
            );
        }
    }
}
