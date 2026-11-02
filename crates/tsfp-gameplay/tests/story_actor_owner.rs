use std::collections::{BTreeMap, BTreeSet};

use tsfp_gameplay::story_events::native_world::actor_owner::{StoryActorState, StoryActorStore};
use tsfp_gameplay::story_events::native_world::player_owner::StoryPlayerStore;
use tsfp_gameplay::story_events::native_world::{
    NativeStoryWorld, StoryChildState, StoryGameplayInputsStore, StoryPlayerState,
};

fn actor(health: f32, tag: u32) -> StoryActorState {
    StoryActorState {
        health: Some(health),
        tags: BTreeSet::from([tag]),
        tagged_child_states: BTreeMap::from([(tag, StoryChildState::Word5)]),
        settled_prop: true,
        ready: true,
        live: true,
    }
}

#[test]
fn actor_snapshot_projects_complete_owned_domains_and_preserves_other_world_inputs() {
    let mut actors = StoryActorStore::default();
    let handle = actors.bind(21, actor(75.0, 0x4741_5330)).unwrap();
    let mut world = NativeStoryWorld::new(vec![StoryPlayerState {
        character_id: 7,
        following_link: 0,
        resource_memberships: BTreeSet::new(),
        first_player_flag: true,
        health: 100.0,
        down: false,
    }]);
    world.live_objects.insert(999);

    actors.publish_into(&mut world);

    assert_eq!(handle.actor_key(), 21);
    assert_eq!(world.actor_health, BTreeMap::from([(21, 75.0)]));
    assert_eq!(world.tags, BTreeSet::from([0x4741_5330]));
    assert_eq!(
        world.tagged_child_states,
        BTreeMap::from([(0x4741_5330, StoryChildState::Word5)])
    );
    assert_eq!(world.settled_props, BTreeSet::from([21]));
    assert_eq!(world.ready_objects, BTreeSet::from([21]));
    assert_eq!(world.live_objects, BTreeSet::from([21]));
    assert_eq!(world.players.len(), 1);
}

#[test]
fn replacement_rejects_delayed_updates_and_unbinds() {
    let mut actors = StoryActorStore::default();
    let old = actors.bind(3, actor(10.0, 1)).unwrap();
    let current = actors.bind(3, actor(90.0, 2)).unwrap();

    assert!(current.generation() > old.generation());
    assert!(!actors.update(old, actor(0.0, 3)));
    assert_eq!(actors.unbind(old), None);
    assert!(actors.is_current(current));

    let mut world = NativeStoryWorld::default();
    actors.publish_into(&mut world);
    assert_eq!(world.actor_health, BTreeMap::from([(3, 90.0)]));
    assert_eq!(world.tags, BTreeSet::from([2]));
}

#[test]
fn unbind_removes_only_the_current_entity() {
    let mut actors = StoryActorStore::default();
    let one = actors.bind(1, actor(10.0, 11)).unwrap();
    let two = actors.bind(2, actor(20.0, 22)).unwrap();

    assert_eq!(actors.unbind(one), Some(actor(10.0, 11)));
    assert!(!actors.is_current(one));
    assert!(actors.is_current(two));
    let mut world = NativeStoryWorld::default();
    actors.publish_into(&mut world);
    assert_eq!(world.actor_health, BTreeMap::from([(2, 20.0)]));
    assert_eq!(world.tags, BTreeSet::from([22]));
}

#[test]
fn player_and_actor_owners_publish_as_one_complete_tick_snapshot() {
    let mut players = StoryPlayerStore::default();
    players
        .bind(
            0,
            StoryPlayerState {
                character_id: 7,
                following_link: 3,
                resource_memberships: BTreeSet::from([9]),
                first_player_flag: true,
                health: 80.0,
                down: false,
            },
        )
        .unwrap();
    let mut actors = StoryActorStore::default();
    actors.bind(21, actor(75.0, 0x4741_5330)).unwrap();

    let store = StoryGameplayInputsStore::default();
    let session = store.begin_session().unwrap();
    store
        .publish_from_player_actor_owners(
            session,
            &players,
            &actors,
            BTreeSet::from([4]),
            Vec::new(),
            None,
        )
        .unwrap();

    let published = store.snapshot().unwrap().unwrap();
    assert_eq!(published.players.len(), 1);
    assert_eq!(published.players[0].character_id, 7);
    assert_eq!(published.actor_health, BTreeMap::from([(21, 75.0)]));
    assert_eq!(published.tags, BTreeSet::from([0x4741_5330]));
    assert_eq!(published.finished_voices, BTreeSet::from([4]));
    assert_eq!(published.ready_objects, BTreeSet::from([21]));
    assert_eq!(published.live_objects, BTreeSet::from([21]));
}
