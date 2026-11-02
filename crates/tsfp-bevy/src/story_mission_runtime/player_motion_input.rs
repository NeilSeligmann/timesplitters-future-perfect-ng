use super::*;
use tsfp_input::player::Vec2;

impl StoryPlayerEngineStateStore {
    /// Resolve the channel against this store's exact engine state and expose its complete
    /// current input sample to native handlers. The caller owns action interpretation and
    /// simulation timing; this does not consume edges or assign the sample to an original tick.
    pub fn step_channel_input_for_step(
        &mut self,
        inputs: &StoryGameplayInputsStore,
        session: StoryGameplayInputSession,
        channel: u8,
        handle: StoryPlayerHandle,
    ) -> Result<Option<tsfp_input::player::PlayerInput>, StoryGameplayOwnerStoreError> {
        if self.session != Some(session) {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        let Some(player) = self.players.get_mut(&handle) else {
            return Ok(None);
        };
        if !attach_story_player_channel_input_for_step(inputs, session, channel, handle, player)? {
            return Ok(None);
        }
        Ok(player.input_for_step())
    }

    /// Resolve the channel against this store's exact engine state and apply native look for the
    /// caller-selected step. The handle owns player selection; the caller still owns cadence,
    /// look configuration, and any broader simulation scheduling.
    pub fn step_channel_look_for_step<P: tsfp_input::look::LookPolicy>(
        &mut self,
        inputs: &StoryGameplayInputsStore,
        session: StoryGameplayInputSession,
        channel: u8,
        handle: StoryPlayerHandle,
        integration: &mut tsfp_input::look::LookIntegration<P>,
        profile: tsfp_input::look::LookProfile,
        allowed: tsfp_input::player::AssistMask,
        config: tsfp_input::look::Integration,
    ) -> Result<bool, StoryGameplayOwnerStoreError> {
        if self.session != Some(session) {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        let Some(player) = self.players.get_mut(&handle) else {
            return Ok(false);
        };
        step_story_player_channel_look_for_step(
            inputs,
            session,
            channel,
            handle,
            player,
            integration,
            profile,
            allowed,
            config,
        )
    }

    /// Resolve the channel sample for this exact player and expose its raw movement axes.
    /// The caller owns axis shaping, fixed-tick cadence, and simulation scheduling.
    pub fn step_channel_move_axis_for_step(
        &mut self,
        inputs: &StoryGameplayInputsStore,
        session: StoryGameplayInputSession,
        channel: u8,
        handle: StoryPlayerHandle,
    ) -> Result<Option<Vec2>, StoryGameplayOwnerStoreError> {
        if self.session != Some(session) {
            return Err(StoryGameplayOwnerStoreError::Inputs(
                tsfp_gameplay::story_events::native_world::StoryGameplayInputsStoreError::StaleSession,
            ));
        }
        let Some(player) = self.players.get_mut(&handle) else {
            return Ok(None);
        };
        if !attach_story_player_channel_input_for_step(inputs, session, channel, handle, player)? {
            return Ok(None);
        }
        Ok(player.move_axis_for_step())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_gameplay::story_events::native_world::StoryPlayerState;
    use tsfp_input::player::{AssistMask, Look, PlayerInput, SourceKind};
    use tsfp_types::TsfpVec3;

    #[test]
    fn story_channel_move_axis_reads_only_the_bound_player_sample() {
        let inputs = StoryGameplayInputsStore::default();
        let session = inputs.begin_session().unwrap();
        let mut states = StoryPlayerEngineStateStore::default();
        states.begin_session(session);
        let handle = states
            .bind_story_player(
                &inputs,
                session,
                3,
                StoryPlayerState {
                    character_id: 0x2a,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: true,
                    health: 1.0,
                    down: false,
                },
                PlayerState::new(
                    TsfpVec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    0.0,
                ),
            )
            .unwrap();
        assert!(
            inputs
                .bind_player_input_channel(session, 2, handle)
                .unwrap()
        );

        let sample = PlayerInput {
            step: 12,
            slot: 7,
            source: SourceKind::Pad(0),
            held: 0,
            pressed: 0,
            released: 0,
            move_axis: Vec2 { x: 0.25, y: -0.75 },
            look: Look::Stick(Vec2 { x: 0.0, y: 0.0 }),
            triggers: [0.0; 2],
            menu: Default::default(),
            assists: AssistMask::PROFILE,
        };
        inputs
            .publish_player_inputs_for_step(session, [None, None, Some(sample), None])
            .unwrap();
        assert_eq!(
            states
                .step_channel_move_axis_for_step(&inputs, session, 2, handle)
                .unwrap(),
            Some(Vec2 { x: 0.25, y: -0.75 })
        );

        inputs
            .publish_player_inputs_for_step(session, [None; 4])
            .unwrap();
        assert_eq!(
            states
                .step_channel_move_axis_for_step(&inputs, session, 2, handle)
                .unwrap(),
            None
        );
        assert_eq!(
            states
                .player_state(session, handle)
                .and_then(PlayerState::input_for_step),
            None
        );
    }

    #[test]
    fn story_channel_input_exposes_complete_bound_sample_and_clears_on_disconnect() {
        let inputs = StoryGameplayInputsStore::default();
        let session = inputs.begin_session().unwrap();
        let mut states = StoryPlayerEngineStateStore::default();
        states.begin_session(session);
        let handle = states
            .bind_story_player(
                &inputs,
                session,
                1,
                StoryPlayerState {
                    character_id: 0x2a,
                    following_link: 0,
                    resource_memberships: Default::default(),
                    first_player_flag: true,
                    health: 1.0,
                    down: false,
                },
                PlayerState::new(
                    TsfpVec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    0.0,
                ),
            )
            .unwrap();
        inputs
            .bind_player_input_channel(session, 1, handle)
            .unwrap();
        let sample = PlayerInput {
            step: 9,
            slot: 2,
            source: SourceKind::Pad(1),
            held: 0x20,
            pressed: 0x20,
            released: 0,
            move_axis: Vec2 { x: 0.5, y: -0.25 },
            look: Look::Stick(Vec2 { x: 0.1, y: -0.2 }),
            triggers: [0.75, 0.0],
            menu: Default::default(),
            assists: AssistMask::PROFILE,
        };
        inputs
            .publish_player_inputs_for_step(session, [None, Some(sample), None, None])
            .unwrap();
        assert_eq!(
            states
                .step_channel_input_for_step(&inputs, session, 1, handle)
                .unwrap(),
            Some(sample)
        );

        inputs
            .publish_player_inputs_for_step(session, [None; 4])
            .unwrap();
        assert_eq!(
            states
                .step_channel_input_for_step(&inputs, session, 1, handle)
                .unwrap(),
            None
        );
    }
}
