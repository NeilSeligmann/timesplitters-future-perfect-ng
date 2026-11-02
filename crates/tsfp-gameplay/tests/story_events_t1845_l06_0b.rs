//! T1845 round 3 task H: callees of the level 6 and level 0xB hooks (synthetic facts only, no game data).
use tsfp_gameplay::story_events::hooks_l06_0b::*;

fn facts(flags: &[u32], items: &[u32], model: u32) -> ObjectFacts {
    ObjectFacts {
        present: true,
        flags: flags.to_vec(),
        items: items.to_vec(),
        model,
    }
}

#[test]
fn switches_finish_slot_8_only_after_all_six_left() {
    let mut tracker = SwitchTracker::default();
    assert!(tracker.step([true; 6]).is_empty());
    assert_eq!(tracker.latches, [1; 6]);
    // Five leave: no finish.
    assert!(
        tracker
            .step([true, false, false, false, false, false])
            .is_empty()
    );
    assert_eq!(tracker.count, 5);
    let out = tracker.step([false; 6]);
    assert_eq!(out, vec![Effect::SlotFinish { slot: 8, arg: 0 }]);
    // Not guarded by a once flag: it repeats (MEASURED).
    assert_eq!(tracker.step([false; 6]).len(), 1);
    // A switch that never appeared never counts.
    let mut never = SwitchTracker::default();
    for _ in 0..3 {
        assert!(never.step([false; 6]).is_empty());
    }
}

#[test]
fn weapon_handover_needs_item_and_flag() {
    let target = Target::Tag(MT_TAGS[0]);
    assert!(weapon_handover(target, &facts(&[0x8000], &[0xAC9], 0)).is_empty());
    assert!(weapon_handover(target, &facts(&[], &[0xAC8], 0)).is_empty());
    let out = weapon_handover(target, &facts(&[0x8000], &[0xACC], 0));
    assert_eq!(out.len(), 3);
    assert_eq!(out[0], Effect::SetState { target, state: 8 });
    assert_eq!(
        out[2],
        Effect::AddAmmo {
            target,
            ammo_type: 3,
            amount: 1000
        }
    );
}

#[test]
fn npc_rules_follow_the_call_order() {
    let mut view = Level06NpcView::default();
    view.mt[1] = facts(&[0x8000], &[0xAC8], 0);
    view.os01 = facts(&[1], &[], 0xA34);
    view.mt07 = facts(&[1], &[], 0xA35);
    view.first_player = facts(&[1, 2], &[0xA3F, 0xA46], 0);
    let out = level06_npc_rules(&view);
    assert_eq!(
        out[0],
        Effect::SetState {
            target: Target::Tag(MT_TAGS[1]),
            state: 8
        }
    );
    let os = out
        .iter()
        .position(|e| {
            *e == Effect::SetState {
                target: Target::Tag(OS01_TAG),
                state: 0x24,
            }
        })
        .unwrap();
    assert!(os > 2);
    assert!(!out.iter().any(|e| matches!(
        e,
        Effect::SetState {
            target: Target::Tag(MT07_TAG),
            ..
        }
    )));
    // First player: 0xA3F with flag 1 then flag 2, then 0xA46.
    let me: Vec<&Effect> = out
        .iter()
        .filter(|e| {
            matches!(
                e,
                Effect::SetState {
                    target: Target::FirstPlayer,
                    ..
                }
            )
        })
        .collect();
    let states: Vec<u32> = me
        .iter()
        .map(|e| {
            if let Effect::SetState { state, .. } = e {
                *state
            } else {
                0
            }
        })
        .collect();
    assert_eq!(states, vec![0x41, 0, 8]);
    assert!(out.contains(&Effect::SetByte {
        target: Target::FirstPlayer,
        offset: 0x58D,
        value: 1
    }));
    assert!(out.contains(&Effect::AddAmmo {
        target: Target::FirstPlayer,
        ammo_type: 3,
        amount: 1000
    }));
    // No first player: nothing more.
    view.first_player = ObjectFacts::default();
    assert!(!level06_npc_rules(&view).iter().any(|e| matches!(
        e,
        Effect::SetState {
            target: Target::FirstPlayer,
            ..
        }
    )));
}

#[test]
fn character_predicate_matches_the_jump_table() {
    for rejected in [0, 1, 4, 6, 7, 8, 0x23] {
        assert!(!level06_character_ok(rejected, 0x600), "{rejected:#x}");
    }
    for accepted in [9, 0x10, 0x22, 0x24, 0xFFFF_FFFF] {
        assert!(level06_character_ok(accepted, 0), "{accepted:#x}");
    }
    assert!(!level06_character_ok(5, 0x50A));
    assert!(level06_character_ok(5, 0x50B));
    assert!(level06_character_ok(5, 0x50E));
    assert!(!level06_character_ok(5, 0x50F));
    assert!(level06_character_ok(5, 0x5E0));
}

#[test]
fn level06_fade_rises_falls_and_clamps() {
    let mut fade = Level06Fade::default();
    let out = fade.step(true);
    assert!((fade.fade - (1.0 / 60.0) * 1.333_333_4).abs() < 1e-6);
    assert_eq!(out.len(), 3);
    match &out[0] {
        Effect::Sound {
            id, volume, arg5, ..
        } => {
            assert_eq!(*id, 0x11);
            assert_eq!(*volume, fade.fade);
            assert_eq!(*arg5, 50.0);
        }
        other => panic!("{other:?}"),
    }
    match &out[1] {
        Effect::Sound {
            id,
            position,
            handle,
            volume,
            ..
        } => {
            assert_eq!(*id, 0x359);
            assert_eq!(*handle, SoundHandle::Level06 { offset: 0xC });
            assert_eq!(*volume, 1.5);
            assert_eq!(*position, Some([53.47, 0.5, 40.92]));
        }
        other => panic!("{other:?}"),
    }
    fade.step(false);
    fade.step(false);
    assert_eq!(fade.fade, 0.0);
    for _ in 0..100 {
        fade.step(true);
    }
    assert_eq!(fade.fade, 1.0);
}

#[test]
fn callback_20b8c0_is_gated_by_kind_and_resets_members() {
    let mut input = Callback20b8c0Input {
        kind: 0x2E,
        member_states: vec![0x4E0, 1, 0x4E1, 5, 0x4E0, 0x4E0],
        position: [1.0, 2.0, 3.0],
        message_vector: None,
        offset: [10.0, 20.0, 30.0],
        random: 0,
    };
    assert!(callback_20b8c0(&input).is_empty());
    input.kind = CALLBACK_20B8C0_KIND;
    let out = callback_20b8c0(&input);
    // Members 0 and 2 and 4 (only the first five are collected; member 5 is ignored).
    let members: Vec<usize> = out
        .iter()
        .filter_map(|e| {
            if let Effect::SetObjectWord {
                target: Target::Member(i),
                ..
            } = e
            {
                Some(*i)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(members, vec![0, 2, 4]);
    assert!(out.contains(&Effect::SoundRequest {
        id: 0x35F,
        position: [1.0, 2.0, 3.0]
    }));
    assert!(out.contains(&Effect::SetObjectFloat {
        target: Target::SelfObject,
        offset: 0x1A4,
        value: 0.5
    }));
    assert!(out.contains(&Effect::SetObjectFloat {
        target: Target::SelfObject,
        offset: 0xB4,
        value: 11.0
    }));
    input.message_vector = Some([7.0, 8.0, 9.0]);
    input.random = u32::MAX;
    let out = callback_20b8c0(&input);
    assert!(out.contains(&Effect::SetObjectFloat {
        target: Target::SelfObject,
        offset: 0xB8,
        value: 8.0
    }));
    let delay = out
        .iter()
        .find_map(|e| match e {
            Effect::SetObjectFloat {
                offset: 0x1A4,
                value,
                ..
            } => Some(*value),
            _ => None,
        })
        .unwrap();
    assert!((delay - 1.0).abs() < 1e-6);
    assert_eq!(
        out.last(),
        Some(&Effect::SetObjectFloat {
            target: Target::SelfObject,
            offset: 0x1CC,
            value: 0.0
        })
    );
}

#[test]
fn trn3_hook_orders_sound_slot_and_voice() {
    let mut view = Trn3View {
        game_word_7de314: TRN3_WORD,
        mask_bit4: true,
        local_health: 7.0,
        ..Trn3View::default()
    };
    let (out, done) = level0b_trn3(&view);
    assert_eq!(out[0], Effect::SoundFreeSlot { id: 0x3B6 });
    assert_eq!(out[1], Effect::SlotFinish { slot: 0x11, arg: 0 });
    assert_eq!(
        out[2],
        Effect::VoiceLine {
            text_id: 0xFEF,
            argument: 0x140
        }
    );
    assert!(done);
    view.local_health = 7.01;
    assert_eq!(level0b_trn3(&view).0.len(), 2);
    view.local_health = 1.0;
    view.slot_0x11_finished = true;
    view.voice_done = true;
    let (out, done) = level0b_trn3(&view);
    assert_eq!(out, vec![Effect::SlotFinish { slot: 0x11, arg: 0 }]);
    assert!(done);
    view.mask_bit4 = false;
    assert!(level0b_trn3(&view).0.is_empty());
    view.mask_bit4 = true;
    view.game_word_7de314 = TRN3_WORD + 1;
    assert!(level0b_trn3(&view).0.is_empty());
}

#[test]
fn nuke_countdown_windows() {
    let slots = |c: f32| -> Vec<u32> {
        nuke_countdown_slots(c)
            .iter()
            .map(|e| {
                if let Effect::SlotFinish { slot, .. } = e {
                    *slot
                } else {
                    0
                }
            })
            .collect()
    };
    assert_eq!(slots(61.5), vec![0x23]);
    assert!(slots(60.0).is_empty());
    assert_eq!(slots(60.1), vec![0x23]);
    assert!(slots(61.6).is_empty());
    assert_eq!(slots(30.5), vec![0x20]);
    assert_eq!(slots(29.6), vec![0x20]);
    assert!(slots(29.5).is_empty());
    assert_eq!(slots(10.5), vec![0x21]);
    assert!(slots(9.0).is_empty());
    assert_eq!(slots(0.5), vec![0x22]);
    assert!(slots(1.0).is_empty(), "the last upper bound is exclusive");
    assert!(slots(0.0).is_empty());
    assert!(slots(-100.0).is_empty());
}

#[test]
fn fbn_and_pete_rules() {
    assert_eq!(
        level0b_fbn_slot(Some(0x5CD), false, false),
        vec![Effect::SlotFinish { slot: 0x24, arg: 0 }]
    );
    assert_eq!(level0b_fbn_slot(Some(0x5CE), false, false).len(), 1);
    assert!(level0b_fbn_slot(Some(0x5CF), false, false).is_empty());
    assert!(level0b_fbn_slot(Some(0x5CD), true, false).is_empty());
    assert!(level0b_fbn_slot(Some(0x5CD), false, true).is_empty());
    assert!(level0b_fbn_slot(None, false, false).is_empty());

    let target = Target::Tag(PETE_TAG);
    assert!(level0b_pete(false, 0xBEB).is_empty());
    assert_eq!(
        level0b_pete(true, 0xBEC),
        vec![Effect::SetObjectFloat {
            target,
            offset: 0x1CC,
            value: 100.0
        }]
    );
    assert_eq!(
        level0b_pete(true, 0xBED),
        vec![
            Effect::SlotFinish { slot: 0x28, arg: 0 },
            Effect::SetObjectFloat {
                target,
                offset: 0x1CC,
                value: 0.2
            }
        ]
    );
    assert!(level0b_pete(true, 0xBEF).is_empty());
}

#[test]
fn lcg_matches_the_documented_recurrence() {
    let mut lcg = Lcg { state: 1 };
    assert_eq!(lcg.next_u32(), (0x343FDu64 + 0x269EC3) as u32);
    let second = (0x343FDu64 + 0x269EC3)
        .wrapping_mul(0x343FD)
        .wrapping_add(0x269EC3) as u32;
    assert_eq!(lcg.next_u32(), second);
    let mut unit = Lcg { state: 0 };
    let value = unit.next_unit();
    assert!((0.0..1.0).contains(&value));
}

#[test]
fn camera_shake_is_deterministic_and_bounded() {
    let mut lcg = Lcg { state: 12345 };
    let mut shake = CameraShake::init(&mut lcg);
    assert_eq!(shake.channels[0].min_period, 0.5);
    assert_eq!(shake.channels[1].spread, 0.9);
    assert_eq!(shake.channels[2].max_period, 0.05);
    let replay_start = (shake, lcg);
    let mut peak = 0.0f32;
    for _ in 0..600 {
        let out = shake.step(1.0, &mut lcg);
        let (Effect::AddAccumulatorA(a), Effect::AddAccumulatorB(b)) = (&out[0], &out[1]) else {
            panic!()
        };
        for axis in 0..3 {
            assert!((b[axis] + 0.15 * a[axis]).abs() < 1e-6);
            peak = peak.max(a[axis].abs());
        }
        assert!(shake.phase <= 2.0);
        assert!((0.0..=1.0).contains(&shake.intensity));
    }
    // Amplitude bound: channel 0 is 0.03/2, channels 1 and 2 are at most 0.0125/2 and 0.005/2 per axis.
    assert!(peak <= 0.0151 + 0.0063 + 0.0026);
    assert!(peak > 0.0);
    let (mut again, mut lcg2) = replay_start;
    let mut lcg1 = Lcg { state: 12345 };
    let mut first = CameraShake::init(&mut lcg1);
    for _ in 0..50 {
        assert_eq!(first.step(1.0, &mut lcg1), again.step(1.0, &mut lcg2));
    }
    // The phase wraps: after about 1.2 s the sine half wave has finished and the intensity is 0.
    let mut quiet = CameraShake::init(&mut Lcg { state: 1 });
    let mut lcg3 = Lcg { state: 1 };
    let mut saw_zero_after_peak = false;
    let mut saw_peak = false;
    for _ in 0..200 {
        quiet.step(1.0, &mut lcg3);
        if quiet.intensity > 0.9 {
            saw_peak = true;
        }
        if saw_peak && quiet.intensity == 0.0 {
            saw_zero_after_peak = true;
        }
    }
    assert!(saw_peak && saw_zero_after_peak);
}

#[test]
fn barrier_zone_fade_damage_and_drain() {
    let mut barrier = Level0bBarrier::default();
    let normal = ZoneObject {
        kind_is_8: true,
        ..ZoneObject::default()
    };
    let objects = [
        normal,
        ZoneObject {
            type_d5: true,
            ..normal
        },
        ZoneObject {
            is_htip: true,
            ..normal
        },
        ZoneObject {
            is_player_extension: true,
            ..normal
        },
        normal,
    ];
    let players = [BarrierPlayer {
        object_present: true,
        binding: 0,
        health: 0.0,
        state: 0x4C,
    }];
    let out = barrier.step(&objects, &players);
    assert!(matches!(out.effects[0], Effect::CreateEmitter { .. }));
    assert!(out.effects.contains(&Effect::StartBarrierSound));
    assert!(
        out.effects
            .contains(&Effect::SlotActivate { slot: 0x1C, arg: 0 })
    );
    assert!(out.effects.contains(&Effect::EmitterBounds {
        min: BARRIER_BOX.0,
        max: BARRIER_BOX.1
    }));
    let damaged: Vec<usize> = out
        .effects
        .iter()
        .filter_map(|e| {
            if let Effect::Damage {
                victim: Target::Listed(i),
                ..
            } = e
            {
                Some(*i)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(damaged, vec![0, 4]);
    // First sighting: health 15, hud bound, ff4 set.
    assert_eq!(out.players[0].health, 15.0);
    assert!(out.effects.contains(&Effect::BindHud {
        target: Target::Tag(BARRIER_TAGS[0])
    }));
    // Drain while in state 0x4C: health falls by fade * 0.01.
    let before = out.players[0];
    let fade = barrier.fade;
    let out = barrier.step(
        &[],
        &[BarrierPlayer {
            binding: 1,
            ..before
        }],
    );
    let expected = 15.0 - barrier.fade * 0.01;
    assert!((out.players[0].health - expected).abs() < 1e-6);
    assert!(barrier.fade > fade);
    // Exhausted: damage the player.
    let out = barrier.step(
        &[],
        &[BarrierPlayer {
            binding: 1,
            health: 0.0,
            state: 0x4C,
            object_present: true,
        }],
    );
    assert_eq!(out.players[0].health, 0.0);
    assert!(out.effects.iter().any(|e| matches!(
        e,
        Effect::Damage {
            victim: Target::Player(0),
            ..
        }
    )));
    // Other state refills by 0.1 and releases at the ceiling.
    let out = barrier.step(
        &[],
        &[BarrierPlayer {
            binding: 1,
            health: 14.95,
            state: 0,
            object_present: true,
        }],
    );
    assert_eq!(out.players[0].health, 15.0);
    assert!(out.effects.contains(&Effect::SetPlayerFf4 {
        player: 0,
        target: None
    }));
    // Closed (open == 1): emitter off, sound stopped, slot finished, fade falls and no damage once it hits 0.
    barrier.open = 1;
    let out = barrier.step(&objects, &[]);
    assert!(out.effects.contains(&Effect::EmitterEnable(false)));
    assert!(out.effects.contains(&Effect::StopBarrierSound));
    assert!(
        out.effects
            .contains(&Effect::SlotFinish { slot: 0x1C, arg: 0 })
    );
    for _ in 0..200 {
        barrier.step(&[], &[]);
    }
    assert_eq!(barrier.fade, 0.0);
    let out = barrier.step(&objects, &[]);
    assert!(
        !out.effects
            .iter()
            .any(|e| matches!(e, Effect::Damage { .. }))
    );
    // Second player uses the SHUT tag.
    let mut two = Level0bBarrier {
        fade: 0.5,
        ..Level0bBarrier::default()
    };
    let present = BarrierPlayer {
        object_present: true,
        binding: 0,
        ..BarrierPlayer::default()
    };
    let out = two.step(&[], &[present, present]);
    assert!(out.effects.contains(&Effect::BindHud {
        target: Target::Tag(BARRIER_TAGS[1])
    }));
}
