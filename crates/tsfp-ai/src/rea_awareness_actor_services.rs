// SPDX-License-Identifier: GPL-3.0-or-later
//! Connect the Rust-owned group sweep to generation-owned AI actor state.

use crate::{
    navigation_scoring::NodeId,
    rea_awareness_groups::{OwnedAwarenessMemberServices, ReaAwarenessGroupTable},
    rea_group_alert::MemberView,
    rea_tick_state::{AwarenessRecipientState, ReaActorIdentity, ReaTickActorStore},
};

/// Typed world calls that remain outside actor and awareness state ownership.
/// The original linked-object resolution, candidate checks, and callback-time
/// membership mutation are still explicit seams; this trait does not infer
/// their unresolved pointer-lifetime contracts.
pub trait ReaTickAwarenessWorld {
    fn linked_object_node(&self, actor: ReaActorIdentity) -> u32;
    fn class_listens(&mut self, selector: i16) -> bool;
    fn candidate_check(
        &mut self,
        actor: ReaActorIdentity,
        recipient: &mut AwarenessRecipientState,
    ) -> bool;
    fn actor_busy(
        &mut self,
        actor: ReaActorIdentity,
        recipient: &mut AwarenessRecipientState,
    ) -> bool;
    fn raise(
        &mut self,
        actor: ReaActorIdentity,
        recipient: &mut AwarenessRecipientState,
        code: u32,
        three_args: bool,
    );
    fn leave_groups(
        &mut self,
        groups: &mut ReaAwarenessGroupTable,
        actor: ReaActorIdentity,
        recipient: &mut AwarenessRecipientState,
        removed_groups: u32,
    );
}

/// Adapter that keeps alert-recipient reads and writes in `ReaTickActorStore`
/// while routing only world-dependent calls through `ReaTickAwarenessWorld`.
pub struct ReaTickAwarenessServices<'a, W> {
    actors: &'a mut ReaTickActorStore,
    world: &'a mut W,
}

impl<'a, W> ReaTickAwarenessServices<'a, W> {
    pub fn new(actors: &'a mut ReaTickActorStore, world: &'a mut W) -> Self {
        Self { actors, world }
    }

    fn update_recipient<R>(
        &mut self,
        actor: ReaActorIdentity,
        update: impl FnOnce(&mut W, &mut AwarenessRecipientState) -> R,
    ) -> Option<R> {
        let world = &mut self.world;
        self.actors
            .with_awareness_recipient_mut(actor, |recipient| update(world, recipient))
            .ok()
            .flatten()
    }
}

impl<W: ReaTickAwarenessWorld> OwnedAwarenessMemberServices for ReaTickAwarenessServices<'_, W> {
    fn member_view(&self, actor: ReaActorIdentity) -> MemberView {
        let recipient = self.actors.awareness_recipient(actor).ok().flatten();
        recipient.map_or(
            MemberView {
                // A missing generation or recipient is ineligible by default.
                // This is native fail-closed policy, not recovered C behavior.
                id: u32::MAX,
                actor_flags: 0,
                local_flags: 0,
                listen_flags: 0,
                behavior_flags: 0,
                group_mask: 0,
                class_selector: 0,
            },
            |recipient| MemberView {
                id: recipient.actor.0,
                actor_flags: recipient.actor_flags,
                local_flags: recipient.local_flags,
                listen_flags: recipient.listen_flags,
                behavior_flags: recipient.behavior_flags,
                group_mask: recipient.group_mask.0,
                class_selector: recipient.class_selector,
            },
        )
    }

    fn linked_object_node(&self, actor: ReaActorIdentity) -> u32 {
        // This query is reached only after an eligible, live recipient view.
        self.world.linked_object_node(actor)
    }

    fn set_member_target(&mut self, actor: ReaActorIdentity, node: u32) {
        let _ = self.update_recipient(actor, |_, recipient| {
            recipient.alert_target = Some(NodeId(node));
        });
    }

    fn set_member_local_flags(&mut self, actor: ReaActorIdentity, flags: u32) {
        let _ = self.update_recipient(actor, |_, recipient| recipient.local_flags = flags);
    }

    fn set_member_group_mask(&mut self, actor: ReaActorIdentity, mask: u32) {
        let _ = self.update_recipient(actor, |_, recipient| recipient.group_mask.0 = mask);
    }

    fn class_listens(&mut self, selector: i16) -> bool {
        self.world.class_listens(selector)
    }

    fn candidate_check(&mut self, actor: ReaActorIdentity) -> bool {
        self.update_recipient(actor, |world, recipient| {
            world.candidate_check(actor, recipient)
        })
        .unwrap_or(false)
    }

    fn actor_busy(&mut self, actor: ReaActorIdentity) -> bool {
        self.update_recipient(actor, |world, recipient| world.actor_busy(actor, recipient))
            .unwrap_or(false)
    }

    fn raise(&mut self, actor: ReaActorIdentity, code: u32, three_args: bool) {
        let _ = self.update_recipient(actor, |world, recipient| {
            world.raise(actor, recipient, code, three_args)
        });
    }

    fn leave_groups(
        &mut self,
        groups: &mut ReaAwarenessGroupTable,
        actor: ReaActorIdentity,
        removed_groups: u32,
    ) {
        let world = &mut self.world;
        let _ = self
            .actors
            .with_awareness_recipient_mut(actor, |recipient| {
                world.leave_groups(groups, actor, recipient, removed_groups)
            });
    }
}

#[cfg(test)]
mod tests {
    use alloc::{vec, vec::Vec};

    use super::*;
    use crate::{
        awareness::AwarenessCache,
        navigation_scoring::ActorId,
        rea_ai_types::GroupMask,
        rea_awareness_groups::{AwarenessGroupId, ReaAwarenessGroupTable},
        rea_awareness_pipeline::{AwarenessPipelineBookkeeping, OwnedAwarenessState},
        rea_awareness_sources::AwarenessSourceRecord,
        rea_group_alert::{ACTOR_ALERTABLE, LOCAL_ALERTED},
        rea_tick_state::{
            BehaviorMode, BehaviorPair, LocalState, ReaTickSnapshot, TransitionProgress,
        },
    };

    struct World;

    impl ReaTickAwarenessWorld for World {
        fn linked_object_node(&self, _actor: ReaActorIdentity) -> u32 {
            42
        }

        fn class_listens(&mut self, _selector: i16) -> bool {
            false
        }

        fn candidate_check(
            &mut self,
            _actor: ReaActorIdentity,
            _recipient: &mut AwarenessRecipientState,
        ) -> bool {
            false
        }

        fn actor_busy(
            &mut self,
            _actor: ReaActorIdentity,
            _recipient: &mut AwarenessRecipientState,
        ) -> bool {
            false
        }

        fn raise(
            &mut self,
            _actor: ReaActorIdentity,
            _recipient: &mut AwarenessRecipientState,
            _code: u32,
            _three_args: bool,
        ) {
        }

        fn leave_groups(
            &mut self,
            _groups: &mut ReaAwarenessGroupTable,
            _actor: ReaActorIdentity,
            _recipient: &mut AwarenessRecipientState,
            _removed_groups: u32,
        ) {
        }
    }

    #[derive(Default)]
    struct Bookkeeping {
        group_value: u32,
        registrations: Vec<usize>,
        alerts: usize,
    }

    impl AwarenessPipelineBookkeeping for Bookkeeping {
        fn game_mode_byte(&self) -> u8 {
            0x0a
        }

        fn global_flags(&self) -> u32 {
            0
        }

        fn set_group_alert_value(&mut self, value: u32) {
            self.group_value = value;
        }

        fn register_spawn_entry(&mut self, index: usize) {
            self.registrations.push(index);
        }

        fn increment_alert_count(&mut self) {
            self.alerts += 1;
        }
    }

    fn snapshot() -> ReaTickSnapshot {
        ReaTickSnapshot {
            mode: BehaviorMode::Mode10,
            mode10_active: BehaviorPair::default(),
            other_active: BehaviorPair::default(),
            previous_behavior: BehaviorPair::default(),
            pending_behavior: BehaviorPair::default(),
            local_state: LocalState::default(),
            previous_local_state: LocalState::default(),
            pending_local_state: None,
            transition_progress: TransitionProgress::default(),
            awareness_cache: AwarenessCache::from_raw(0),
        }
    }

    fn source_state() -> OwnedAwarenessState {
        OwnedAwarenessState::from_sources(vec![AwarenessSourceRecord {
            alert_mask: GroupMask(1),
            filter_mask: GroupMask(1),
            target: None,
            state: 0,
            elapsed: 3.0,
        }])
    }

    #[test]
    fn owned_source_group_sweep_updates_generation_owned_recipient() {
        let mut actors = ReaTickActorStore::new_empty();
        let actor = actors.spawn(4, snapshot()).unwrap();
        actors
            .set_awareness_recipient(
                actor,
                AwarenessRecipientState {
                    actor: ActorId(77),
                    actor_flags: ACTOR_ALERTABLE,
                    local_flags: 0,
                    listen_flags: 0,
                    behavior_flags: 0,
                    class_selector: 0,
                    group_mask: GroupMask(1),
                    alert_target: None,
                },
            )
            .unwrap();

        let group = AwarenessGroupId::new(0).unwrap();
        let mut awareness = source_state();
        awareness.set_group_members(group, vec![actor]);
        let mut world = World;
        let mut services = ReaTickAwarenessServices::new(&mut actors, &mut world);
        let mut bookkeeping = Bookkeeping::default();

        assert_eq!(
            awareness.process_tick(GroupMask(1), &mut services, &mut bookkeeping),
            1
        );
        drop(services);
        let recipient = actors.awareness_recipient(actor).unwrap().unwrap();
        assert_eq!(recipient.alert_target, Some(NodeId(42)));
        assert_eq!(recipient.local_flags, LOCAL_ALERTED);
        assert_eq!(recipient.group_mask, GroupMask(1));
        assert_eq!(awareness.sources().records()[0].state, 1);
        assert_eq!(awareness.sources().records()[0].elapsed, 0.0);
        assert_eq!(awareness.groups().group(group).alerted(), 1);
        assert_eq!(awareness.groups().group(group).alert_state(), 1);
        assert_eq!(bookkeeping.group_value, 2);
        assert_eq!(bookkeeping.registrations, vec![0]);
        assert_eq!(bookkeeping.alerts, 1);
    }

    #[test]
    fn stale_member_generation_does_not_write_to_reused_slot() {
        let mut actors = ReaTickActorStore::new_empty();
        let live = actors.spawn(4, snapshot()).unwrap();
        actors
            .set_awareness_recipient(
                live,
                AwarenessRecipientState {
                    actor: ActorId(77),
                    actor_flags: ACTOR_ALERTABLE,
                    local_flags: 0,
                    listen_flags: 0,
                    behavior_flags: 0,
                    class_selector: 0,
                    group_mask: GroupMask(1),
                    alert_target: None,
                },
            )
            .unwrap();
        let stale = ReaActorIdentity {
            slot: live.slot,
            generation: live.generation + 1,
        };

        let group = AwarenessGroupId::new(0).unwrap();
        let mut awareness = source_state();
        awareness.set_group_members(group, vec![stale]);
        let mut world = World;
        let mut services = ReaTickAwarenessServices::new(&mut actors, &mut world);
        let mut bookkeeping = Bookkeeping::default();

        assert_eq!(
            awareness.process_tick(GroupMask(1), &mut services, &mut bookkeeping),
            1
        );
        drop(services);
        assert_eq!(
            actors
                .awareness_recipient(live)
                .unwrap()
                .unwrap()
                .alert_target,
            None
        );
        assert_eq!(
            actors
                .awareness_recipient(live)
                .unwrap()
                .unwrap()
                .local_flags,
            0
        );
    }
}
