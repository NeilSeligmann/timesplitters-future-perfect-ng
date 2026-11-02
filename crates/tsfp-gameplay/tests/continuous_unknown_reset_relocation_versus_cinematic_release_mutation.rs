// SPDX-License-Identifier: GPL-3.0-or-later
//! T3415 semantic mutation checks: wrong flag mapping, omitted vector clearing and removed guard.

use tsfp_gameplay::story_events::reset_relocation::*;

fn state() -> StoryResetState {
    let mut state = StoryResetState {
        local_player_count: 2,
        slots: [
            Some(PlayerSlotState {
                object_present: true,
                extension_present: true,
                placement: PlayerPlacement::default(),
                camera: CameraContext {
                    reset_vectors: [1.0, 2.0, 3.0],
                },
            }),
            Some(PlayerSlotState {
                object_present: true,
                extension_present: true,
                placement: PlayerPlacement::default(),
                camera: CameraContext {
                    reset_vectors: [4.0, 5.0, 6.0],
                },
            }),
        ],
        spawns: SpawnTable::default(),
    };
    state.spawns.insert(
        SpawnKey(1045),
        SpawnPlacement {
            spawn_id: 33,
            position: [7.0, 8.0, 9.0],
            cell: 12,
            orientation: 0.75,
        },
    );
    state
}

#[test]
fn semantic_mutants_are_rejected_by_the_contract_assertions() {
    let event = StoryResetEvent {
        spawn_key: SpawnKey(1045),
        player_slot: PlayerSlot::First,
        placement_flags: PlacementFlags::PRESERVE_INPUT_DRIVEN,
    };
    let mut correct = state();
    assert_eq!(
        reset_first_player(event, &mut correct),
        Some(ResetDisposition::Applied)
    );

    // Mutant: discard the definition's 0x800000 bit and always use mode 1.
    let mut wrong_flags = state();
    let wrong_event = StoryResetEvent {
        placement_flags: PlacementFlags::NORMAL,
        ..event
    };
    reset_first_player(wrong_event, &mut wrong_flags);
    assert_ne!(wrong_flags, correct, "flag mutation escaped detection");

    // Mutant: perform the relocation but omit the callback's three vector stores.
    let mut missing_clear = state();
    reset_first_player(event, &mut missing_clear);
    missing_clear.slots[0]
        .as_mut()
        .unwrap()
        .camera
        .reset_vectors = [1.0, 2.0, 3.0];
    assert_ne!(
        missing_clear, correct,
        "vector-clear mutation escaped detection"
    );

    // Mutant: remove the second callback's player-count guard.
    let mut guarded = state();
    guarded.local_player_count = 1;
    let before = guarded.clone();
    let second = StoryResetEvent {
        player_slot: PlayerSlot::Second,
        ..event
    };
    assert_eq!(
        reset_second_player(second, &mut guarded),
        Some(ResetDisposition::GuardedOut)
    );
    let mut no_guard = before.clone();
    let row = no_guard.spawns.resolve(second.spawn_key).unwrap();
    let slot = no_guard.slots[1].as_mut().unwrap();
    slot.placement = PlayerPlacement {
        spawn_key: second.spawn_key,
        spawn_id: row.spawn_id,
        position: row.position,
        cell: row.cell,
        orientation: row.orientation,
        flags: second.placement_flags,
    };
    slot.camera.reset_vectors = [0.0; 3];
    assert_eq!(guarded, before);
    assert_ne!(
        no_guard, guarded,
        "guard-removal mutation escaped detection"
    );
}
