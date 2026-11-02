//! Rust-owned actor facts published to the native Story world boundary.
//!
//! Handles include a lifecycle generation so delayed actor updates cannot mutate a replacement
//! occupant. The store accepts complete typed facts from native gameplay owners and never reads
//! guest memory.

use std::collections::{BTreeMap, BTreeSet};

use super::{NativeStoryWorld, StoryChildState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoryActorHandle {
    actor_key: u32,
    generation: u64,
}

impl StoryActorHandle {
    pub const fn actor_key(self) -> u32 {
        self.actor_key
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoryActorState {
    pub health: Option<f32>,
    pub tags: BTreeSet<u32>,
    pub tagged_child_states: BTreeMap<u32, StoryChildState>,
    pub settled_prop: bool,
    pub ready: bool,
    pub live: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryActorStoreError {
    GenerationExhausted,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoryActorStore {
    generations: BTreeMap<u32, u64>,
    actors: BTreeMap<u32, (u64, StoryActorState)>,
}

impl StoryActorStore {
    pub fn bind(
        &mut self,
        actor_key: u32,
        state: StoryActorState,
    ) -> Result<StoryActorHandle, StoryActorStoreError> {
        let generation = self
            .generations
            .get(&actor_key)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(StoryActorStoreError::GenerationExhausted)?;
        self.generations.insert(actor_key, generation);
        self.actors.insert(actor_key, (generation, state));
        Ok(StoryActorHandle {
            actor_key,
            generation,
        })
    }

    pub fn update(&mut self, handle: StoryActorHandle, state: StoryActorState) -> bool {
        let Some((generation, current)) = self.actors.get_mut(&handle.actor_key) else {
            return false;
        };
        if *generation != handle.generation {
            return false;
        }
        *current = state;
        true
    }

    pub fn unbind(&mut self, handle: StoryActorHandle) -> Option<StoryActorState> {
        let (generation, _) = self.actors.get(&handle.actor_key)?;
        if *generation != handle.generation {
            return None;
        }
        self.actors
            .remove(&handle.actor_key)
            .map(|(_, state)| state)
    }

    pub fn is_current(&self, handle: StoryActorHandle) -> bool {
        self.actors
            .get(&handle.actor_key)
            .is_some_and(|(generation, _)| *generation == handle.generation)
    }

    /// Remove all active actors while retaining generation counters across mission changes.
    pub fn clear(&mut self) {
        self.actors.clear();
    }

    /// Replace only the actor/object domains owned here, preserving unrelated world inputs.
    pub fn publish_into(&self, world: &mut NativeStoryWorld) {
        world.actor_health.clear();
        world.tags.clear();
        world.tagged_child_states.clear();
        world.settled_props.clear();
        world.ready_objects.clear();
        world.live_objects.clear();

        for (&actor_key, (_, state)) in &self.actors {
            if let Some(health) = state.health {
                world.actor_health.insert(actor_key, health);
            }
            world.tags.extend(state.tags.iter().copied());
            world.tagged_child_states.extend(
                state
                    .tagged_child_states
                    .iter()
                    .map(|(&key, &value)| (key, value)),
            );
            if state.settled_prop {
                world.settled_props.insert(actor_key);
            }
            if state.ready {
                world.ready_objects.insert(actor_key);
            }
            if state.live {
                world.live_objects.insert(actor_key);
            }
        }
    }
}
