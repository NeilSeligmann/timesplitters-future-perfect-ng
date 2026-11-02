// SPDX-License-Identifier: GPL-3.0-or-later
use std::sync::Arc;
use tsfp_state::transforms::*;

fn input(owner: TransformOwner, generation: u64, x: f32) -> TickTransform {
    TickTransform {
        id: TransformId {
            owner,
            slot: 0,
            generation,
        },
        sample: TransformSample {
            world: WorldTransform {
                position: [x, 0.0, 0.0],
                orientation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
            bones: Some(BonePalette {
                skeleton: 7,
                revision: x as u64,
                matrices: Arc::from([[x; 16]]),
            }),
        },
        discontinuity: Discontinuity::default(),
    }
}
fn enabled() -> TransformHistory {
    let mut history = TransformHistory::default();
    history.set_enabled(true);
    history
}
#[test]
fn synthetic_ticks_cover_all_owner_namespaces_and_immutable_palettes() {
    let mut history = enabled();
    let owners = [
        TransformOwner::Object,
        TransformOwner::Player,
        TransformOwner::Character,
        TransformOwner::Camera,
    ];
    let first: Vec<_> = owners.map(|owner| input(owner, 0, 1.0)).into();
    let original = first.clone();
    history.capture_tick(10, first).unwrap();
    for value in &original {
        let pair = history.get(value.id).unwrap();
        assert!(pair.discontinuity.spawned);
        assert_eq!(pair.render_inputs(), (&value.sample, &value.sample));
    }
    let second = owners.map(|owner| input(owner, 0, 2.0));
    history.capture_tick(11, second.clone()).unwrap();
    assert_eq!(history.iter().count(), 4);
    for (before, after) in original.iter().zip(second.iter()) {
        let pair = history.get(before.id).unwrap();
        assert_eq!(pair.previous, before.sample);
        assert_eq!(pair.current, after.sample);
        assert!(!pair.discontinuity.any());
        assert_eq!(pair.render_inputs(), (&before.sample, &after.sample));
        assert_eq!(before.sample.bones.as_ref().unwrap().matrices[0], [1.0; 16]);
    }
}
#[test]
fn every_explicit_discontinuity_snaps_and_expires_next_tick() {
    for flags in [
        Discontinuity {
            spawned: true,
            ..Default::default()
        },
        Discontinuity {
            teleported: true,
            ..Default::default()
        },
        Discontinuity {
            cut: true,
            ..Default::default()
        },
        Discontinuity {
            respawned: true,
            ..Default::default()
        },
        Discontinuity {
            spawned: true,
            teleported: true,
            cut: true,
            respawned: true,
        },
    ] {
        let mut history = enabled();
        let before = input(TransformOwner::Player, 1, 0.0);
        history.capture_tick(0, [before.clone()]).unwrap();
        let mut after = input(TransformOwner::Player, 1, 500.0);
        after.discontinuity = flags;
        history.capture_tick(1, [after.clone()]).unwrap();
        let pair = history.get(after.id).unwrap();
        assert_eq!(pair.previous, before.sample);
        assert_eq!(pair.discontinuity, flags);
        assert_eq!(pair.render_inputs(), (&after.sample, &after.sample));
        history
            .capture_tick(2, [input(TransformOwner::Player, 1, 501.0)])
            .unwrap();
        let pair = history.get(after.id).unwrap();
        assert!(!pair.discontinuity.any());
        assert_eq!(pair.previous, after.sample);
    }
}
#[test]
fn retirement_generation_reuse_gaps_and_reset_never_blend_stale_state() {
    let mut history = enabled();
    let first = input(TransformOwner::Character, 1, 1.0);
    history.capture_tick(1, [first.clone()]).unwrap();
    let reused = input(TransformOwner::Character, 2, 8.0);
    history.capture_tick(2, [reused.clone()]).unwrap();
    assert!(history.get(first.id).is_none());
    assert!(history.get(reused.id).unwrap().discontinuity.spawned);
    history
        .capture_tick(4, [input(TransformOwner::Character, 2, 12.0)])
        .unwrap();
    let pair = history.get(reused.id).unwrap();
    assert_eq!(pair.previous, pair.current);
    history.capture_tick(5, []).unwrap();
    assert!(history.get(reused.id).is_none());
    history.capture_tick(6, [reused.clone()]).unwrap();
    assert!(history.get(reused.id).unwrap().discontinuity.spawned);
    history.set_enabled(false);
    assert_eq!(history.tick(), None);
    assert_eq!(history.iter().count(), 0);
    history.set_enabled(true);
    history.capture_tick(0, [first]).unwrap();
}
#[test]
fn disabled_capture_is_lazy_and_failures_are_transactional() {
    let mut history = TransformHistory::default();
    history
        .capture_tick(
            0,
            std::iter::once_with(|| panic!("disabled must not collect")),
        )
        .unwrap();
    assert_eq!(history.tick(), None);
    history.set_enabled(true);
    let first = input(TransformOwner::Camera, 0, 0.0);
    history.capture_tick(7, [first.clone()]).unwrap();
    let saved = history.clone();
    for tick in [6, 7] {
        assert_eq!(
            history.capture_tick(tick, []),
            Err(CaptureError::NonIncreasingTick)
        );
        assert_eq!(history, saved);
    }
    assert_eq!(
        history.capture_tick(8, [first.clone(), first.clone()]),
        Err(CaptureError::DuplicateId(first.id))
    );
    assert_eq!(history, saved);
    history.capture_tick(u64::MAX, [first]).unwrap();
    let saved = history.clone();
    assert_eq!(
        history.capture_tick(0, []),
        Err(CaptureError::NonIncreasingTick)
    );
    assert_eq!(history, saved);
}
