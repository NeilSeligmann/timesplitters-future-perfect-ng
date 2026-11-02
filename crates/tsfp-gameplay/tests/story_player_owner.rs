use std::collections::BTreeSet;

use tsfp_gameplay::story_events::native_world::StoryPlayerState;
use tsfp_gameplay::story_events::native_world::player_owner::StoryPlayerStore;
use tsfp_input::player::{AssistMask, Look, PlayerInput, SourceKind, Vec2};

fn player(character_id: u32, health: f32) -> StoryPlayerState {
    StoryPlayerState {
        character_id,
        following_link: 0,
        resource_memberships: BTreeSet::new(),
        first_player_flag: false,
        health,
        down: health <= 0.0,
    }
}

#[test]
fn player_snapshot_is_ordered_by_native_slot_and_uses_latest_state() {
    let mut players = StoryPlayerStore::default();
    let second = players.bind(4, player(44, 50.0)).unwrap();
    let first = players.bind(1, player(11, 100.0)).unwrap();

    assert!(players.update(second, player(44, 25.0)));
    assert_eq!(
        players.snapshot(),
        vec![player(11, 100.0), player(44, 25.0)]
    );
    assert_eq!(first.slot(), 1);
}

#[test]
fn rebinding_rejects_stale_updates_and_stale_unbinds() {
    let mut players = StoryPlayerStore::default();
    let old = players.bind(0, player(7, 10.0)).unwrap();
    let current = players.bind(0, player(8, 80.0)).unwrap();

    assert!(current.generation() > old.generation());
    assert!(!players.is_current(old));
    assert!(!players.update(old, player(7, 0.0)));
    assert_eq!(players.unbind(old), None);
    assert!(players.is_current(current));
    assert_eq!(players.snapshot(), vec![player(8, 80.0)]);
}

#[test]
fn current_unbind_removes_only_that_player() {
    let mut players = StoryPlayerStore::default();
    let one = players.bind(0, player(7, 10.0)).unwrap();
    let two = players.bind(1, player(8, 80.0)).unwrap();

    assert_eq!(players.unbind(one), Some(player(7, 10.0)));
    assert!(!players.is_current(one));
    assert!(players.is_current(two));
    assert_eq!(players.snapshot(), vec![player(8, 80.0)]);
}

#[test]
fn handle_identity_is_slot_and_generation() {
    let mut players = StoryPlayerStore::default();
    let handle = players.bind(9, player(99, 99.0)).unwrap();
    assert_eq!((handle.slot(), handle.generation()), (9, 1));
}

fn input(step: u64, slot: u8) -> PlayerInput {
    PlayerInput {
        step,
        slot,
        source: SourceKind::Pad(u32::from(slot)),
        held: 1,
        pressed: 1,
        released: 0,
        move_axis: Vec2 { x: 0.25, y: -0.5 },
        look: Look::Stick(Vec2 { x: 0.0, y: 0.75 }),
        triggers: [0.0, 1.0],
        menu: Default::default(),
        assists: AssistMask::PROFILE,
    }
}

#[test]
fn per_step_input_is_attached_to_explicit_player_lifecycle() {
    let mut players = StoryPlayerStore::default();
    let stale = players.bind(7, player(70, 100.0)).unwrap();
    let current = players.bind(7, player(71, 90.0)).unwrap();
    let frame = input(31, 2);

    assert!(!players.set_input_for_step(stale, Some(frame)));
    assert_eq!(players.input_for_step(stale), None);
    assert_eq!(players.input_for_step(current), Some(None));
    assert!(players.set_input_for_step(current, Some(frame)));
    assert_eq!(players.input_for_step(current), Some(Some(frame)));

    assert!(players.set_input_for_step(current, None));
    assert_eq!(players.input_for_step(current), Some(None));
}
