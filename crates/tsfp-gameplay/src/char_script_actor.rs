//! Rust-owned per-actor storage boundary for the 0x26EFC0 script controller.
//!
//! `ReaActorIdentity` comes from the AI actor lifecycle; helper services cross
//! the `ScriptTickWorld` trait. This store contains typed controller state and
//! does not read or retain guest addresses.

use std::vec::Vec;
use tsfp_ai::rea_tick_state::ReaActorIdentity;

use crate::char_tick_script_pose::{ScriptTickCtx, ScriptTickWorld};

#[derive(Clone, Debug, PartialEq)]
pub struct ScriptActorStore<T> {
    actors: Vec<(ReaActorIdentity, T)>,
}

impl<T> Default for ScriptActorStore<T> {
    fn default() -> Self {
        Self { actors: Vec::new() }
    }
}

impl<T> ScriptActorStore<T> {
    /// Number of live script contexts in this store.
    /// Verification: Tier 0 (compile-only convenience accessor).
    pub fn len(&self) -> usize {
        self.actors.len()
    }

    /// Whether the actor store contains no script contexts.
    /// Verification: Tier 0 (compile-only convenience accessor).
    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    /// Attach typed state to the current identity for an actor slot.
    /// Returns the state unchanged when any generation already occupies it.
    /// Verification: Tier 0 (focused storage lifecycle smoke test only).
    pub fn insert(&mut self, identity: ReaActorIdentity, state: T) -> Result<(), T> {
        if self
            .actors
            .iter()
            .any(|(current, _)| current.slot == identity.slot)
        {
            return Err(state);
        }
        self.actors.push((identity, state));
        Ok(())
    }

    /// Return state for the exact slot generation.
    /// Verification: Tier 0 (focused storage lifecycle smoke test only).
    pub fn get(&self, identity: ReaActorIdentity) -> Option<&T> {
        self.actors
            .iter()
            .find(|(current, _)| *current == identity)
            .map(|(_, state)| state)
    }

    /// Return mutable state for the exact slot generation.
    /// Verification: Tier 0 (compile-only; no focused mutable-access test).
    pub fn get_mut(&mut self, identity: ReaActorIdentity) -> Option<&mut T> {
        self.actors
            .iter_mut()
            .find(|(current, _)| *current == identity)
            .map(|(_, state)| state)
    }

    /// Retire the exact actor generation and return its owned state.
    /// Verification: Tier 0 (focused storage lifecycle smoke test only).
    pub fn retire(&mut self, identity: ReaActorIdentity) -> Option<T> {
        let index = self
            .actors
            .iter()
            .position(|(current, _)| *current == identity)?;
        Some(self.actors.swap_remove(index).1)
    }
}

impl ScriptActorStore<ScriptTickCtx> {
    /// Run the joined native controller against an explicit typed world edge.
    /// Verification: Tier 0 attachment smoke only; no live adapter or replay.
    pub fn tick(&mut self, identity: ReaActorIdentity, world: &mut impl ScriptTickWorld) -> bool {
        let Some(state) = self.get_mut(identity) else {
            return false;
        };
        crate::char_tick_script_pose::tick(state, world);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ScriptActorStore;
    use tsfp_ai::rea_tick_state::ReaActorIdentity;

    #[test]
    fn actor_generation_owns_one_typed_state_until_retirement() {
        let first = ReaActorIdentity {
            slot: 12,
            generation: 1,
        };
        let reused = ReaActorIdentity {
            slot: 12,
            generation: 2,
        };
        let mut store = ScriptActorStore::<u32>::default();

        assert_eq!(store.insert(first, 17_u32), Ok(()));
        assert_eq!(store.get(first), Some(&17));
        assert_eq!(store.insert(reused, 18), Err(18));
        assert_eq!(store.get(reused), None);
        assert_eq!(store.retire(first), Some(17));
        assert_eq!(store.insert(reused, 18), Ok(()));
        assert_eq!(store.get(first), None);
        assert_eq!(store.get(reused), Some(&18));
    }
}
