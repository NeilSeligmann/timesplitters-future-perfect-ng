// SPDX-License-Identifier: GPL-3.0-or-later
//! Generation-checked entry to the Rust-owned actor tick admission state.

use crate::rea_tick_state::{
    ActorTickAdmission, ActorTickAdmissionResult, ReaActorIdentity, ReaActorStoreError,
    ReaTickActorStore,
};

impl ReaTickActorStore {
    /// Applies the measured actor prelude to one live Rust-owned generation.
    /// The caller supplies only guest-facing guard inputs and the measured
    /// time-step; admission ordering and timer mutation stay with the owner.
    pub fn run_actor_tick_prelude(
        &mut self,
        identity: ReaActorIdentity,
        admission: ActorTickAdmission,
        time_step: f32,
    ) -> Result<ActorTickAdmissionResult, ReaActorStoreError> {
        self.state_mut(identity)
            .ok_or(ReaActorStoreError::StaleIdentity)
            .map(|state| state.run_actor_tick_prelude(admission, time_step))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::awareness::AwarenessCache;
    use crate::rea_tick_state::{
        ActorTickTimers, BehaviorMode, BehaviorPair, LocalState, ReaTickSnapshot,
        TransitionProgress,
    };

    fn snapshot() -> ReaTickSnapshot {
        ReaTickSnapshot {
            mode: BehaviorMode::Other,
            mode10_active: BehaviorPair::default(),
            other_active: BehaviorPair::default(),
            previous_behavior: BehaviorPair::default(),
            pending_behavior: BehaviorPair::default(),
            local_state: LocalState(0),
            previous_local_state: LocalState(0),
            pending_local_state: None,
            transition_progress: TransitionProgress(0),
            awareness_cache: AwarenessCache::from_raw(0),
        }
    }

    fn admission() -> ActorTickAdmission {
        ActorTickAdmission {
            mode: 0,
            global_tick_blocked: false,
            slot_connected: true,
            slot: 4,
            minimum_slot: 2,
            actor_permission_check_required: false,
            actor_permission_granted: false,
            actor_flag_4_set: false,
        }
    }

    #[test]
    fn prelude_mutates_only_the_named_live_actor_generation() {
        let mut store = ReaTickActorStore::new_empty();
        let identity = store.spawn(7, snapshot()).unwrap();
        store
            .state_mut(identity)
            .unwrap()
            .set_actor_tick_timers(ActorTickTimers {
                primary: 2.0,
                secondary: -1.0,
            });

        assert_eq!(
            store.run_actor_tick_prelude(identity, admission(), 0.5),
            Ok(ActorTickAdmissionResult::Admitted)
        );
        assert_eq!(
            store.state(identity).unwrap().actor_tick_timers(),
            ActorTickTimers {
                primary: 1.5,
                secondary: -1.0,
            }
        );
        assert_eq!(
            store.run_actor_tick_prelude(
                ReaActorIdentity {
                    slot: identity.slot,
                    generation: identity.generation + 1,
                },
                admission(),
                1.0,
            ),
            Err(ReaActorStoreError::StaleIdentity)
        );
    }
}
