use std::collections::{BTreeMap, BTreeSet};

use tsfp_gameplay::story_events::native_world::{
    StoryGameplayInputs, StoryGameplayInputsStore, StoryGameplayInputsStoreError,
    StoryGameplayOwnerStoreError, StoryPlayerState,
};
use tsfp_input::player::{AssistMask, Look, PlayerInput, SourceKind, Vec2};

fn inputs() -> StoryGameplayInputs {
    StoryGameplayInputs::new(
        Vec::new(),
        BTreeMap::new(),
        BTreeSet::new(),
        BTreeMap::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        Vec::new(),
        None,
    )
}

#[test]
fn publishes_and_invalidates_shared_complete_snapshots_by_session() {
    let writer = StoryGameplayInputsStore::default();
    let reader = writer.clone();

    assert_eq!(reader.snapshot().unwrap(), None);
    let first_session = writer.begin_session().unwrap();
    writer.publish(first_session, inputs()).unwrap();
    assert_eq!(reader.snapshot().unwrap(), Some(inputs()));

    let second_session = writer.begin_session().unwrap();
    assert_eq!(reader.snapshot().unwrap(), None);
    assert_eq!(
        writer.publish(first_session, inputs()),
        Err(StoryGameplayInputsStoreError::StaleSession)
    );
    writer.publish(second_session, inputs()).unwrap();
    assert_eq!(reader.snapshot().unwrap(), Some(inputs()));
}

#[test]
fn publishes_revisioned_snapshots_only_after_a_new_frame() {
    let store = StoryGameplayInputsStore::default();

    let first_session = store.begin_session().unwrap();
    assert_eq!(store.snapshot_after(1).unwrap(), None);
    store.publish(first_session, inputs()).unwrap();
    let (revision, snapshot) = store.snapshot_after(1).unwrap().unwrap();
    assert_eq!(revision, 2);
    assert_eq!(snapshot, inputs());
    assert_eq!(store.snapshot_after(revision).unwrap(), None);

    let second_session = store.begin_session().unwrap();
    assert_eq!(store.snapshot_after(revision).unwrap(), None);
    assert_eq!(
        store.publish(first_session, inputs()),
        Err(StoryGameplayInputsStoreError::StaleSession)
    );
    store.publish(second_session, inputs()).unwrap();
    let (next_revision, snapshot) = store.snapshot_after(revision).unwrap().unwrap();
    assert_eq!(next_revision, 4);
    assert_eq!(snapshot, inputs());
}

#[test]
fn per_step_player_input_is_scoped_to_mission_and_player_generation() {
    let store = StoryGameplayInputsStore::default();
    let session = store.begin_session().unwrap();
    let player = store
        .bind_player(
            session,
            3,
            StoryPlayerState {
                character_id: 9,
                following_link: 0,
                resource_memberships: BTreeSet::new(),
                first_player_flag: true,
                health: 100.0,
                down: false,
            },
        )
        .unwrap();
    let input = PlayerInput {
        step: 42,
        slot: 0,
        source: SourceKind::MouseKeyboard,
        held: 2,
        pressed: 2,
        released: 0,
        move_axis: Vec2 { x: 1.0, y: 0.0 },
        look: Look::Delta {
            yaw_deg: 2.0,
            pitch_deg: -1.0,
        },
        triggers: [0.0; 2],
        menu: Default::default(),
        assists: AssistMask::NONE,
    };

    assert_eq!(store.player_input_for_step(session, player), Ok(Some(None)));
    assert_eq!(
        store.set_player_input_for_step(session, player, Some(input)),
        Ok(true)
    );
    assert_eq!(
        store.player_input_for_step(session, player),
        Ok(Some(Some(input)))
    );

    let next_session = store.begin_session().unwrap();
    assert_eq!(
        store.player_input_for_step(session, player),
        Err(StoryGameplayOwnerStoreError::Inputs(
            StoryGameplayInputsStoreError::StaleSession
        ))
    );
    assert_eq!(store.player_input_for_step(next_session, player), Ok(None));
}

#[test]
fn router_channels_publish_through_explicit_generation_bound_player_bindings() {
    let store = StoryGameplayInputsStore::default();
    let session = store.begin_session().unwrap();
    let first = store
        .bind_player(
            session,
            10,
            StoryPlayerState {
                character_id: 10,
                following_link: 0,
                resource_memberships: BTreeSet::new(),
                first_player_flag: true,
                health: 100.0,
                down: false,
            },
        )
        .unwrap();
    let second = store
        .bind_player(
            session,
            20,
            StoryPlayerState {
                character_id: 20,
                following_link: 0,
                resource_memberships: BTreeSet::new(),
                first_player_flag: false,
                health: 100.0,
                down: false,
            },
        )
        .unwrap();
    assert!(store.bind_player_input_channel(session, 0, second).unwrap());
    assert!(store.bind_player_input_channel(session, 1, first).unwrap());

    let channel_zero = PlayerInput {
        step: 7,
        slot: 1,
        source: SourceKind::Pad(1),
        held: 1,
        pressed: 1,
        released: 0,
        move_axis: Vec2 { x: 0.5, y: 0.0 },
        look: Look::Stick(Vec2 { x: 0.0, y: 0.0 }),
        triggers: [0.0; 2],
        menu: Default::default(),
        assists: AssistMask::NONE,
    };
    let channel_one = PlayerInput {
        step: 7,
        slot: 0,
        ..channel_zero
    };
    store
        .publish_player_inputs_for_step(
            session,
            [Some(channel_zero), Some(channel_one), None, None],
        )
        .unwrap();
    assert_eq!(
        store.player_input_for_step(session, second),
        Ok(Some(Some(channel_zero)))
    );
    assert_eq!(
        store.player_input_for_step(session, first),
        Ok(Some(Some(channel_one)))
    );

    store
        .publish_player_inputs_for_step(session, [Some(channel_zero), None, None, None])
        .unwrap();
    assert_eq!(
        store.player_input_for_step(session, second),
        Ok(Some(Some(channel_zero)))
    );
    assert_eq!(store.player_input_for_step(session, first), Ok(Some(None)));

    let replacement = store
        .bind_player(
            session,
            20,
            StoryPlayerState {
                character_id: 21,
                following_link: 0,
                resource_memberships: BTreeSet::new(),
                first_player_flag: false,
                health: 100.0,
                down: false,
            },
        )
        .unwrap();
    assert_eq!(
        store.player_input_for_step(session, replacement),
        Ok(Some(None))
    );
    store
        .publish_player_inputs_for_step(session, [Some(channel_zero), None, None, None])
        .unwrap();
    assert_eq!(
        store.player_input_for_step(session, replacement),
        Ok(Some(None))
    );
    assert!(
        !store
            .bind_player_input_channel(session, 4, replacement)
            .unwrap()
    );
}
