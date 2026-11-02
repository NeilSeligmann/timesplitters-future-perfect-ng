//! Rust-owned Story player facts published to the native Story mission boundary.
//!
//! The owner accepts typed updates from native gameplay systems; it never reads guest memory.
//! Rebinding a slot mints a new identity so delayed updates from a disconnected player cannot
//! overwrite the replacement player's Story state.

use std::collections::BTreeMap;

use tsfp_input::player::PlayerInput;

use super::StoryPlayerState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoryPlayerHandle {
    slot: u32,
    generation: u64,
}

impl StoryPlayerHandle {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoryPlayerStoreError {
    GenerationExhausted,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoryPlayerStore {
    generations: BTreeMap<u32, u64>,
    players: BTreeMap<u32, (u64, StoryPlayerState, Option<PlayerInput>)>,
    input_channels: BTreeMap<u8, StoryPlayerHandle>,
}

impl StoryPlayerStore {
    /// Bind a native player to a slot and return its new lifecycle identity.
    pub fn bind(
        &mut self,
        slot: u32,
        state: StoryPlayerState,
    ) -> Result<StoryPlayerHandle, StoryPlayerStoreError> {
        let generation = self
            .generations
            .get(&slot)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(StoryPlayerStoreError::GenerationExhausted)?;
        self.generations.insert(slot, generation);
        self.input_channels.retain(|_, handle| handle.slot != slot);
        self.players.insert(slot, (generation, state, None));
        Ok(StoryPlayerHandle { slot, generation })
    }

    /// Remove a player only when the supplied lifecycle identity is still current.
    pub fn unbind(&mut self, handle: StoryPlayerHandle) -> Option<StoryPlayerState> {
        let (generation, _, _) = self.players.get(&handle.slot)?;
        if *generation != handle.generation {
            return None;
        }
        self.input_channels.retain(|_, current| *current != handle);
        self.players.remove(&handle.slot).map(|(_, state, _)| state)
    }

    /// Bind one Bevy input-router channel to an explicitly selected native Story player.
    /// The router channel is the array index, not `PlayerInput::slot` and not an original
    /// guest player index. A player may own at most one channel at a time.
    pub fn bind_input_channel(&mut self, channel: u8, handle: StoryPlayerHandle) -> bool {
        if channel >= 4 || !self.is_current(handle) {
            return false;
        }
        self.input_channels
            .retain(|current_channel, current_handle| {
                *current_channel != channel && *current_handle != handle
            });
        self.input_channels.insert(channel, handle);
        true
    }

    /// Publish one router sample at the caller's simulation-step boundary. Missing channels
    /// clear their bound player's value; stale bindings are ignored. The caller supplies the
    /// player/channel association through `bind_input_channel`.
    pub fn publish_input_channels_for_step(&mut self, inputs: [Option<PlayerInput>; 4]) {
        for (_, _, input) in self.players.values_mut() {
            *input = None;
        }
        for (channel, input) in inputs.into_iter().enumerate() {
            let Some(input) = input else { continue };
            let Some(handle) = self.input_channels.get(&(channel as u8)).copied() else {
                continue;
            };
            let Some((generation, _, current)) = self.players.get_mut(&handle.slot) else {
                continue;
            };
            if *generation == handle.generation {
                *current = Some(input);
            }
        }
    }

    /// Replace one current player's complete typed Story state.
    pub fn update(&mut self, handle: StoryPlayerHandle, state: StoryPlayerState) -> bool {
        let Some((generation, current, _)) = self.players.get_mut(&handle.slot) else {
            return false;
        };
        if *generation != handle.generation {
            return false;
        }
        *current = state;
        true
    }

    /// Attach the caller-selected native input to this player's current Rust simulation step.
    /// The lifecycle handle is the explicit player mapping; `PlayerInput::slot` remains source
    /// metadata and is not guessed to be a guest player index. Publish `None` to clear an
    /// inactive or disconnected player's input at the next step boundary.
    pub fn set_input_for_step(
        &mut self,
        handle: StoryPlayerHandle,
        input: Option<PlayerInput>,
    ) -> bool {
        let Some((generation, _, current)) = self.players.get_mut(&handle.slot) else {
            return false;
        };
        if *generation != handle.generation {
            return false;
        }
        *current = input;
        true
    }

    /// Read the input attached to a still-current player. `Some(None)` means the player exists
    /// and has no input for this step; outer `None` means the handle is stale or unbound.
    pub fn input_for_step(&self, handle: StoryPlayerHandle) -> Option<Option<PlayerInput>> {
        let (generation, _, input) = self.players.get(&handle.slot)?;
        (*generation == handle.generation).then_some(*input)
    }

    /// Resolve one explicitly bound router channel to its current player identity and input.
    /// `None` means the channel has no current player binding; the returned input may itself be
    /// `None` when the live player has no sample for this step. `PlayerInput::slot` remains
    /// source metadata and is never used for routing.
    pub fn input_for_channel(
        &self,
        channel: u8,
    ) -> Option<(StoryPlayerHandle, Option<PlayerInput>)> {
        let handle = self.input_channels.get(&channel).copied()?;
        self.input_for_step(handle).map(|input| (handle, input))
    }

    /// Produce player inputs in slot order for one complete Story world publication.
    pub fn snapshot(&self) -> Vec<StoryPlayerState> {
        self.players
            .values()
            .map(|(_, state, _)| state.clone())
            .collect()
    }

    pub fn is_current(&self, handle: StoryPlayerHandle) -> bool {
        self.players
            .get(&handle.slot)
            .is_some_and(|(generation, _, _)| *generation == handle.generation)
    }

    /// Remove all active players while retaining generation counters across mission changes.
    pub fn clear(&mut self) {
        self.players.clear();
        self.input_channels.clear();
    }
}
