//! T1845 batch 5 story hooks (part 1): synthetic checks of `hooks_story`.
use std::collections::HashMap;
use tsfp_gameplay::story_events::hooks_story::*;

#[derive(Default)]
struct View {
    finished: Vec<u32>,
    npcs: HashMap<u32, NpcView>,
    area: Option<u32>,
    mode: u8,
    flags: u32,
    gated: HashMap<u32, NpcView>,
    flag3ec: Option<bool>,
}
impl HookView for View {
    fn slot_finished(&self, slot: u32) -> bool {
        self.finished.contains(&slot)
    }
    fn npc(&self, tag: u32) -> Option<NpcView> {
        self.npcs.get(&tag).copied()
    }
    fn player_area(&self) -> Option<u32> {
        self.area
    }
    fn mode_byte(&self) -> u8 {
        self.mode
    }
    fn game_flags(&self) -> u32 {
        self.flags
    }
    fn npc_gate(&self, tag: u32, _mask: u32) -> Option<NpcView> {
        self.gated.get(&tag).copied()
    }
    fn table_3ec_flag(&self) -> Option<bool> {
        self.flag3ec
    }
}
fn npc(model: u32, alive: bool) -> NpcView {
    NpcView { model, alive }
}

#[test]
fn level12_rules_set_state_and_ammo_only_for_matching_alive_models() {
    let mut view = View::default();
    view.npcs.insert(0x4450_3039, npc(0x694, true));
    view.npcs.insert(0x4450_3037, npc(0x694, true)); // 0x694 is not a DP07 model
    view.npcs.insert(0x4450_3130, npc(0x693, false)); // dead
    let out = level12_hook(&view, None, 0.0);
    assert!(out.contains(&HookEffect::NpcState {
        tag: 0x4450_3039,
        state: 2,
        clear_byte: 0x54C
    }));
    assert!(out.contains(&HookEffect::NpcAmmo {
        tag: 0x4450_3039,
        ammo_type: 1,
        amount: 1000
    }));
    assert!(!out.iter().any(|e| matches!(
        e,
        HookEffect::NpcState {
            tag: 0x4450_3037 | 0x4450_3130,
            ..
        }
    )));
}

#[test]
fn level12_slot_8_needs_area_model_and_value_above_three() {
    let mut view = View {
        area: Some(0x15),
        ..Default::default()
    };
    let finish = HookEffect::SlotFinish { slot: 8, arg: 0 };
    assert!(level12_hook(&view, Some(0x667), 3.5).contains(&finish));
    assert!(!level12_hook(&view, Some(0x667), 3.0).contains(&finish));
    assert!(!level12_hook(&view, Some(0x668), 9.0).contains(&finish));
    view.area = Some(0x16);
    assert!(!level12_hook(&view, Some(0x667), 9.0).contains(&finish));
}

#[test]
fn level0e_phase_machine_finishes_the_vanished_tag_slot() {
    let view = View {
        area: Some(3),
        ..Default::default()
    };
    let mut state = Level0eState::default();
    let seen = Level0eInputs {
        hna1: true,
        ..Default::default()
    };
    assert!(
        !level0e_hook(&view, &mut state, &seen)
            .iter()
            .any(|e| matches!(e, HookEffect::SlotFinish { .. }))
    );
    assert_eq!(state.phase, 1);
    let out = level0e_hook(&view, &mut state, &Level0eInputs::default());
    assert!(out.contains(&HookEffect::SlotFinish { slot: 5, arg: 0 }));
    let both = Level0eInputs {
        hna2: true,
        ..Default::default()
    };
    level0e_hook(&view, &mut state, &both);
    assert_eq!(state.phase, 2);
    assert!(
        level0e_hook(&view, &mut state, &Level0eInputs::default())
            .contains(&HookEffect::SlotFinish { slot: 6, arg: 0 })
    );
}

#[test]
fn level0e_tail_calls_follow_slot_three() {
    let view = View {
        finished: vec![3],
        area: Some(9),
        ..Default::default()
    };
    let mut state = Level0eState::default();
    let out = level0e_hook(
        &view,
        &mut state,
        &Level0eInputs {
            record_flag: true,
            ..Default::default()
        },
    );
    assert!(out.iter().any(|e| matches!(
        e,
        HookEffect::Call {
            callee: 0x1FD350,
            ..
        }
    )));
    let out = level0e_hook(
        &view,
        &mut state,
        &Level0eInputs {
            player_ten: true,
            ..Default::default()
        },
    );
    assert!(out.iter().any(|e| matches!(
        e,
        HookEffect::Call {
            callee: 0x1FD1B0,
            ..
        }
    )));
}

#[test]
fn level1_flag_slot_and_attach() {
    let view = View {
        area: Some(4),
        ..Default::default()
    };
    let inputs = Level1Inputs {
        db01_model: Some(0x15C),
        sci2_present: true,
        ..Default::default()
    };
    let out = level1_hook(&view, &inputs);
    assert!(out.contains(&HookEffect::ObjectFlag {
        tag: 0x4442_3031,
        mask: 0x10,
        set: false
    }));
    assert!(out.contains(&HookEffect::AttachScript {
        tag: 0x5343_4932,
        vector: [-1.95, 5.26, 2.4]
    }));
    let other = Level1Inputs {
        db01_model: Some(1),
        ..Default::default()
    };
    assert!(
        level1_hook(&view, &other).contains(&HookEffect::ObjectFlag {
            tag: 0x4442_3031,
            mask: 0x10,
            set: true
        })
    );

    let view = View {
        area: Some(0x18),
        ..Default::default()
    };
    let grdn = Level1Inputs {
        grdn_flag: true,
        ..Default::default()
    };
    let out = level1_hook(&view, &grdn);
    assert!(out.contains(&HookEffect::SlotPromote {
        slot: 1,
        value: 0.0
    }));
    let done = View {
        finished: vec![1],
        area: Some(0x18),
        ..Default::default()
    };
    assert!(
        !level1_hook(&done, &grdn)
            .iter()
            .any(|e| matches!(e, HookEffect::SlotPromote { .. }))
    );
}

#[test]
fn level1_jbth_table() {
    let mut view = View::default();
    view.npcs.insert(0x4A42_5448, npc(0x190, true));
    let out = level1_hook(&view, &Level1Inputs::default());
    assert!(out.contains(&HookEffect::NpcState {
        tag: 0x4A42_5448,
        state: 0x35,
        clear_byte: 0x54C
    }));
    assert!(out.contains(&HookEffect::NpcAmmo {
        tag: 0x4A42_5448,
        ammo_type: 9,
        amount: 1000
    }));
}

#[test]
fn ambience_and_ombh_thresholds() {
    assert_eq!(ambience_sound_id(2), 0x10);
    assert_eq!(ambience_sound_id(0x40), 0x1627);
    assert_eq!(ambience_sound_id(7), 0x12);
    assert!(start_ambience(5, 2).is_none());
    assert!(start_ambience(-1, 2).is_some());
    assert_eq!([0, 1, 2, 3].map(ombh_threshold), [7, 0xF, 0x19, 0xA]);
    let view = View {
        mode: 0,
        ..Default::default()
    };
    let inputs = Level2Inputs {
        ambience_handle: 0,
        mode_word: 2,
        ombh_count: 7,
        flag_40: false,
    };
    assert!(level2_hook(&view, &inputs).contains(&HookEffect::SlotFinish { slot: 5, arg: 0 }));
    let few = Level2Inputs {
        ombh_count: 6,
        ..inputs
    };
    assert!(!level2_hook(&view, &few).contains(&HookEffect::SlotFinish { slot: 5, arg: 0 }));
    let flagged = Level2Inputs {
        flag_40: true,
        ..inputs
    };
    assert!(!level2_hook(&view, &flagged).contains(&HookEffect::SlotFinish { slot: 5, arg: 0 }));
}

#[test]
fn level4_weights_counter_and_slots() {
    assert_eq!(level4_class_weight(0x802, 10, 2), Some((1, 11.0)));
    assert_eq!(level4_class_weight(0x802, 3, 1), Some((1, 21.0)));
    assert_eq!(level4_class_weight(0xAE2, 3, 1), Some((4, 3.0)));
    assert_eq!(level4_class_weight(1, 3, 1), None);
    assert_eq!([0, 1, 3, 4].map(level4_counter_step), [0, 2, 4, 4]);
    let mut view = View::default();
    view.npcs.insert(0x534F_4C59, npc(0xB62, true));
    view.npcs.insert(0x4350_5441, npc(0xB58, true));
    let inputs = Level4Inputs {
        player_health: 10.5,
        cpta_gate_open: true,
        area: 0x3F,
        counter_670: 3,
        ..Default::default()
    };
    let out = level4_hook(&view, &inputs);
    assert!(out.contains(&HookEffect::SlotFinish { slot: 0x17, arg: 0 }));
    assert!(out.contains(&HookEffect::SlotFinish { slot: 0xC, arg: 0 }));
    assert!(out.contains(&HookEffect::NpcState {
        tag: 0x534F_4C59,
        state: 0x30,
        clear_byte: 0x54C
    }));
    assert!(out.contains(&HookEffect::NpcAmmo {
        tag: 0x4350_5441,
        ammo_type: 1,
        amount: 1000
    }));
    let healthy = Level4Inputs {
        player_health: 50.0,
        ..inputs
    };
    assert!(!level4_hook(&view, &healthy).contains(&HookEffect::SlotFinish { slot: 0x17, arg: 0 }));
}

#[test]
fn level5_finish_conditions() {
    let view = View::default();
    let base = Level5Inputs {
        boss_present: true,
        boss_state_present: true,
        bigr_found: true,
        bigr_health: 1.0,
        finish_armed: true,
        ..Default::default()
    };
    assert!(level5_hook(&view, &base).contains(&HookEffect::SlotFinish { slot: 4, arg: 0 }));
    let blocked = Level5Inputs {
        finish_blocked: true,
        ..base
    };
    assert!(!level5_hook(&view, &blocked).contains(&HookEffect::SlotFinish { slot: 4, arg: 0 }));
    let idle = Level5Inputs {
        boss_present: false,
        ..base
    };
    assert!(level5_hook(&view, &idle).iter().any(|e| matches!(
        e,
        HookEffect::Call {
            callee: 0x21C580,
            ..
        }
    )));
}

#[test]
fn level9_work_rule_and_tick() {
    let mut view = View::default();
    view.npcs.insert(0x574F_524B, npc(0x6E1, true));
    let out = level9_hook(&view);
    assert!(out.contains(&HookEffect::NpcAmmo {
        tag: 0x574F_524B,
        ammo_type: 2,
        amount: 1000
    }));
    assert_eq!(
        level9_tick(
            Level9Tick {
                mode: 1,
                value: 0.0,
                sound_started: false
            },
            0
        ),
        None
    );
    let rise = level9_tick(
        Level9Tick {
            mode: 1,
            value: 0.0,
            sound_started: false,
        },
        1,
    )
    .unwrap();
    assert!(
        (rise.state.value - 0.1).abs() < 1e-6 && rise.busy && rise.started_sound == Some(0x378)
    );
    let end = level9_tick(
        Level9Tick {
            mode: 1,
            value: 90.95,
            sound_started: true,
        },
        1,
    )
    .unwrap();
    assert!(
        !end.busy
            && end.state.value == 0.0
            && end.started_sound == Some(0x379)
            && end.stopped_sound == Some(0x378)
    );
    let fall = level9_tick(
        Level9Tick {
            mode: 0,
            value: 0.05,
            sound_started: true,
        },
        1,
    )
    .unwrap();
    assert!(!fall.busy);
    assert!((level9_music_mix(&[113.0, 100.0], 2) - 3.0).abs() < 1e-5);
}

#[test]
fn level1a_and_coverage() {
    let out = level1a_hook(&View::default(), 0, -1, 0x40);
    assert_eq!(out[0], HookEffect::StartAmbience { sound_id: 0x1627 });
    assert!(out.iter().any(|e| matches!(
        e,
        HookEffect::Call {
            callee: 0x218C50,
            ..
        }
    )));
    assert_eq!(COVERED.len(), 8);
}

#[test]
fn dz_tbl_latches_and_flags() {
    let mut view = View::default();
    view.npcs.insert(0x445A_3031, npc(0x15A, true));
    view.npcs.insert(0x445A_3032, npc(0x159, true));
    view.npcs.insert(0x5442_4C32, npc(0x15B, true));
    let mut latches = [0u32; 12];
    latches[2] = 1; // DZ03 present before, absent now
    latches[3] = 0;
    let out = dz_tbl_sync(&view, &mut latches);
    assert_eq!(out.len(), 3);
    assert!(out.contains(&HookEffect::ObjectFlag {
        tag: 0x445A_3031,
        mask: 1,
        set: false
    }));
    assert!(out.contains(&HookEffect::ObjectFlag {
        tag: 0x445A_3032,
        mask: 1,
        set: true
    }));
    assert_eq!(
        (latches[0], latches[1], latches[2], latches[3], latches[11]),
        (1, 1, 2, 0, 1)
    );
}

#[test]
fn level5_handover_and_loop() {
    let mut view = View::default();
    assert!(level5_weapon_handover(&view).is_empty());
    view.gated.insert(0x5231_3130, npc(0x7CB, true));
    let out = level5_weapon_handover(&view);
    assert_eq!(out.len(), 5);
    assert_eq!(
        out[0],
        HookEffect::ControllerCall {
            subject: 0,
            callee: 0x1BA2B0,
            args: vec![0x19, 8]
        }
    );
    view.gated.insert(0x5231_3130, npc(0x7CC, true));
    assert!(level5_weapon_handover(&view).is_empty());
    let HookEffect::ApproachLoop { target, .. } = level5_loop(&view) else {
        panic!()
    };
    assert_eq!(target, 1.0);
    view.flags = 0x200;
    let HookEffect::ApproachLoop { target, sound, .. } = level5_loop(&view) else {
        panic!()
    };
    assert_eq!((target, sound), (0.25, 0x20));
}

#[test]
fn level12_mix_table() {
    let t = |effects: [HookEffect; 2]| {
        let (
            HookEffect::ApproachLoop { target: a, .. },
            HookEffect::ApproachLoop { target: b, .. },
        ) = (&effects[0], &effects[1])
        else {
            panic!()
        };
        (*a, *b)
    };
    assert_eq!(t(level12_mix(1, None)), (1.0, 0.0));
    assert_eq!(t(level12_mix(1, Some(true))), (0.0, 0.0));
    assert_eq!(t(level12_mix(2, None)), (1.0, 0.0));
    assert_eq!(t(level12_mix(17, None)), (0.0, 0.6));
    assert_eq!(t(level12_mix(23, None)), (0.0, 0.0));
    assert_eq!(t(level12_mix(41, None)), (0.0, 0.6));
    assert_eq!(t(level12_mix(0, None)), (0.0, 0.6));
    assert_eq!(t(level12_mix(26, None)), (1.0, 0.0));
}

#[test]
fn level1a_scene_rules() {
    let mut view = View::default();
    view.npcs.insert(0x5054_435A, npc(0xA93, true));
    let out = level1a_scene(&view, 0);
    assert_eq!(
        out,
        vec![
            HookEffect::NpcState {
                tag: 0x5054_435A,
                state: 0x19,
                clear_byte: 0x54C
            },
            HookEffect::NpcAmmo {
                tag: 0x5054_435A,
                ammo_type: 0x12,
                amount: 1000
            },
        ]
    );
    assert!(level1a_scene(&view, 0x19).is_empty());
    view.npcs.insert(0x5242_5331, npc(0xA97, true));
    view.gated.insert(0x5242_5331, npc(0xA97, true));
    assert_eq!(level1a_scene(&view, 0x19).len(), 6);
}

// ---------------------------------------------------------------- round 4

#[test]
fn weapon_handover_runs_each_block_only_for_its_model_and_mask() {
    let rbs1 = 0x5242_5331;
    let sd01 = 0x5344_3031;
    let mut view = View::default();
    assert!(weapon_handover(&view).is_empty());
    view.npcs.insert(rbs1, npc(0xA1C, true));
    view.gated.insert(rbs1, npc(0xA1C, true));
    let out = weapon_handover(&view);
    assert_eq!(
        out.len(),
        6,
        "armed block (4 calls) then alive block (2 calls)"
    );
    assert_eq!(
        out[0],
        HookEffect::ControllerCall {
            subject: SUBJECT_PLAYER0,
            callee: 0x1BA2B0,
            args: vec![0x19, 8]
        }
    );
    // A wrong model blocks it even when the mask test passes.
    view.npcs.insert(rbs1, npc(0xA1D, true));
    view.gated.insert(rbs1, npc(0xA1D, true));
    assert!(weapon_handover(&view).is_empty());
    view.npcs.insert(sd01, npc(0xA03, true));
    view.gated.insert(sd01, npc(0xA03, true));
    let out = weapon_handover(&view);
    assert_eq!(out.len(), 2 + 5 + 2);
    assert!(out.contains(&HookEffect::ControllerCall {
        subject: SUBJECT_PLAYER0,
        callee: 0x1BA2B0,
        args: vec![0x20, 6]
    }));
}

#[test]
fn level2_hook_now_names_every_callee() {
    let inputs = Level2Inputs {
        ambience_handle: -1,
        mode_word: 0,
        ombh_count: 0,
        flag_40: false,
    };
    let out = level2_hook(&View::default(), &inputs);
    assert!(!out.iter().any(|e| matches!(
        e,
        HookEffect::Unsupported { .. } | HookEffect::TailUnsupported { .. }
    )));
    assert!(out.contains(&HookEffect::TableWordFfff {
        entries: [0xE, 0x14, 0x18]
    }));
    for effect in &out {
        if let HookEffect::Call { callee, port } = effect {
            assert_eq!(callee_port(*callee), Some(*port));
        }
    }
}

#[test]
fn every_registered_port_resolves_and_is_unique_per_address() {
    let addresses: std::collections::BTreeSet<u32> = CALLEE_PORTS.iter().map(|row| row.0).collect();
    assert_eq!(addresses.len(), CALLEE_PORTS.len());
    assert!(CALLEE_PORTS.iter().all(|row| !row.1.is_empty()));
}

#[test]
fn proximity_effect_rumbles_decays_and_jitters_living_players() {
    let mut timer = 3.0;
    let mut rolls = [0.5f32, 0.25].into_iter();
    let out = proximity_effect_step(&mut timer, -1.0, 0xA, &[true, false, true], &mut || {
        rolls.next().unwrap()
    });
    assert!(
        (timer - (3.0 + 1.0 / 60.0)).abs() < 1e-6,
        "the image multiplier is -1.0, so the timer rises; the game writes +1 each frame"
    );
    assert_eq!(
        out[0],
        ProximityEffect::Rumble {
            pad: 0,
            strength: 3.0 / 3.0 * 0.6
        }
    );
    assert!(out.contains(&ProximityEffect::Rumble {
        pad: 1,
        strength: 3.0 * (1.0 / 3.0) * 0.6
    }));
    assert!(out.contains(&ProximityEffect::PlayerJitter {
        player: 2,
        word_fbc: 0.25 * 0.2 + 0.8,
        word_fc0: 0.94
    }));
    assert_eq!(
        out.iter()
            .filter(|e| matches!(e, ProximityEffect::PlayerJitter { .. }))
            .count(),
        2
    );
    let mut idle = 0.0;
    assert_eq!(
        proximity_effect_step(&mut idle, 1.0, 0, &[true], &mut || 0.0),
        vec![ProximityEffect::Rumble {
            pad: 0,
            strength: 0.0
        }]
    );
}

fn fx_input(level: u32) -> FxInput {
    FxInput {
        level_id: level,
        mode_byte: 0,
        local_players: 1,
        player0: [-120.0, 40.0, -240.0],
        camera_xz: [0.0, 0.0],
        shtm: None,
        multiplier: 1.0,
    }
}

#[test]
fn timed_fx_entries_cull_by_level_and_distance() {
    let mut table = default_fx_table();
    let mut zero = || 0.0f32;
    let mut roll = || 1u32;
    // Level 1: types 5 and 6 never run, entry 0 is within 200 units of the player.
    let out = timed_fx_step(&mut table, &fx_input(1), &mut zero, &mut roll);
    assert!(
        out.is_empty(),
        "every entry starts by finishing its pulse (timer 60 > lifetime)"
    );
    assert_eq!(table[0].phase, 0);
    assert_eq!(table[0].timer, 0.0);
    assert_eq!(table[6].phase, 1, "type 5 untouched outside level 2");
    assert_eq!(table[3].timer, 60.0, "entry 3 is more than 200 units away");
}

#[test]
fn timed_fx_waits_then_emits_the_template_points() {
    let mut table = [FxEntry {
        kind: 2,
        phase: 0,
        position: [0.0; 3],
        timer: 0.0,
        direction: [0.0, 1.0, 0.0],
        pulse_scale: 1.0,
        wait_scale: 1.0,
    }];
    let mut input = fx_input(1);
    input.player0 = [0.0; 3];
    input.multiplier = 60.0; // one second per call
    let (mut zero, mut roll) = (|| 0.0f32, || 1u32);
    assert!(timed_fx_step(&mut table, &input, &mut zero, &mut roll).is_empty());
    assert_eq!(table[0].phase, 1, "timer 1.0 passed the wait 0.1");
    input.multiplier = 6.0; // 0.1 s: crosses one interval of 0.1
    let out = timed_fx_step(&mut table, &input, &mut zero, &mut roll);
    assert_eq!(out.len(), 1, "template 2 cycles one point per tick");
    let FxEffect::Particle {
        colour, position, ..
    } = &out[0]
    else {
        panic!("{out:?}")
    };
    assert_eq!(*colour, 0x5F3F_3F7F);
    // direction (0,1,0): helper axis x, basis row 2 = x axis; point (0,0,-2) rotates along row 0 = dir x helper.
    assert!(
        position[1].abs() < 1e-5,
        "points lie in the plane perpendicular to the direction: {position:?}"
    );
    assert!((position[0] * position[0] + position[2] * position[2]).sqrt() - 2.0 < 1e-4);
}

#[test]
fn basis_is_orthonormal() {
    for direction in [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.6, 0.0, 0.8]] {
        let [row0, row1, row2] = basis_from_direction(direction);
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        assert!(
            dot(row0, row1).abs() < 1e-5
                && dot(row1, row2).abs() < 1e-5
                && dot(row0, row2).abs() < 1e-5
        );
        assert!((dot(row2, row2) - 1.0).abs() < 1e-5);
    }
}

#[test]
fn sky_streak_tick_state_machine() {
    let mut streak = SkyStreak::default();
    let mut spawned = 0;
    let out = sky_streak_tick(&mut streak, 60.0, &mut |s| {
        spawned += 1;
        s.state = 2;
        Vec::new()
    });
    assert!(
        out.is_empty() && spawned == 1 && streak.state == 2 && (streak.timer - 1.0).abs() < 1e-6
    );
    streak.state = 1;
    streak.velocity = [3.0, 0.0, 0.0];
    sky_streak_tick(&mut streak, 60.0, &mut |_| panic!());
    assert!((streak.position[0] - 3.0).abs() < 1e-5);
    assert_eq!(streak.state, 1);
    streak.timer = 3.0;
    sky_streak_tick(&mut streak, 60.0, &mut |_| panic!());
    assert_eq!(streak.state, 0, "timer above 3.0");
    streak.state = 3;
    sky_streak_tick(&mut streak, 0.0, &mut |_| panic!());
    assert_eq!(streak.state, 0);
}

#[test]
fn sky_streak_spawn_rejects_hidden_rows_and_far_points() {
    let mut streak = SkyStreak::default();
    let rows = vec![
        SkyRow {
            flag: 0,
            a: [0.0; 3],
            b: [0.0; 3]
        };
        12
    ];
    let mut rng = || 0u32; // row 0, coin 0
    assert!(sky_streak_spawn(&mut streak, &rows, [0.0, 0.0], &[], &mut rng).is_empty());
    assert_eq!(streak.state, 0);
    let mut rows = rows;
    rows[0] = SkyRow {
        flag: 1,
        a: [0.0, 0.0, 30.0],
        b: [10.0, 0.0, 30.0],
    };
    rows[1] = SkyRow {
        flag: 1,
        a: [0.0, 0.0, 40.0],
        b: [10.0, 0.0, 40.0],
    };
    // The point is inside [15, 40] units of the camera but a blocking object sits on it.
    let mut rng = || 0u32;
    let out = sky_streak_spawn(&mut streak, &rows, [0.0, 0.0], &[[0.0, 30.0]], &mut rng);
    assert!(out.is_empty());
}

#[test]
fn repel_moves_pairs_to_seven_units_apart() {
    let mut positions = [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [100.0, 0.0, 0.0]];
    repel_close_objects(&mut positions);
    assert!((positions[1][0] - positions[0][0] - REPEL_SEPARATION).abs() < 1e-4);
    assert_eq!(positions[2][0], 100.0);
    let mut apart = [[0.0; 3], [8.0, 0.0, 0.0]];
    repel_close_objects(&mut apart);
    assert_eq!(
        apart[1][0], 8.0,
        "7.0 squared is 49, farther pairs are left alone"
    );
}

#[test]
fn spark_bursts_count_to_25_and_finish_slot_nine() {
    let mut record = SparkRecord {
        flag: 1,
        timer: 0.0,
        z: 0.0,
        count: 24,
    };
    let mut rng = || 0x8000_0000u32;
    let out = spark_burst_step(&mut record, true, 1.0, &mut rng);
    assert!(out.contains(&SparkEffect::RegisterDraw) && out.contains(&SparkEffect::BurstsDone));
    assert_eq!(record.count, 25);
    assert_eq!(record.flag, 0);
    assert_eq!(record.z, 10.0);
    // coin = 0 (even) mirrors x and flips the velocity.
    let mut record = SparkRecord {
        flag: 1,
        timer: 0.0,
        z: 0.0,
        count: 0,
    };
    let mut even = || 2u32;
    let out = spark_burst_step(&mut record, false, 1.0, &mut even);
    let SparkEffect::Spark { position, velocity } = &out[0] else {
        panic!()
    };
    assert_eq!(velocity[0], 5.0);
    assert!((position[0] - (SPARK_MIRROR - SPARK_TABLE[2][0])).abs() < 1e-4);
}

#[test]
fn level9_targets_follow_class_cases() {
    assert_eq!(level9_class_case(3), 0);
    assert_eq!(level9_class_case(8), 1);
    assert_eq!(level9_class_case(9), 2);
    assert_eq!(level9_class_case(0x39), 2);
    assert_eq!(level9_music_targets(&[(3, 0.0)], false), (0.0, 0.0));
    assert_eq!(
        level9_music_targets(&[(3, 0.0), (9, 0.0)], true),
        (0.5, 0.5)
    );
    let (mix, rest) = level9_music_targets(&[(8, 100.0)], true);
    assert!(
        (mix - 1.0).abs() < 1e-6 && rest.abs() < 1e-6,
        "health 100 gives the full term"
    );
}

#[test]
fn level0e_ambient_loops_fade_by_class_lists() {
    let mut values = [0.0; 4];
    let out = level0e_ambient_loops(&mut values, &[1], false);
    assert_eq!(out.map(|row| row.0), [0x4F, 0x4C, 0x4D, 0x4E]);
    assert!(
        (out[0].1 - 0.025).abs() < 1e-6,
        "class 1 is in list A, the value rises by the step"
    );
    let mut values = [0.5; 4];
    let out = level0e_ambient_loops(&mut values, &[1], true);
    assert!(
        (out[2].1 - 0.475).abs() < 1e-6,
        "slot 3 finished nulls list C"
    );
}

#[test]
fn ember_spawn_fires_only_for_listed_camera_classes() {
    let mut timer = 0.0;
    let mut rolls = || 0.5f32;
    assert!(ember_spawn(&mut timer, Some(0x10), 1, &mut rolls).is_none());
    assert!(ember_spawn(&mut timer, None, 1, &mut rolls).is_none());
    let position =
        ember_spawn(&mut timer, Some(0x48), 1, &mut rolls).expect("timer below 0 spawns");
    assert_eq!(position, [-391.39, -49.6, -160.8]);
    assert!((timer - 0.45).abs() < 1e-6);
    assert!(
        ember_spawn(&mut timer, Some(0x48), 1, &mut rolls).is_none(),
        "timer back above 0"
    );
}

#[test]
fn boss_aim_points_along_the_forward_vector() {
    let aim = boss_aim_vectors(0.0, 0.0, [10.0, 20.0, 30.0]);
    assert_eq!(aim.mode, 3);
    assert!((aim.forward[0] - 1.0).abs() < 1e-3 && aim.forward[1].abs() < 1e-3);
    assert!((aim.target[0] - 14.5).abs() < 1e-2 && (aim.target[1] - 21.0).abs() < 1e-2);
    let steep = boss_aim_vectors(-200.0, 0.0, [0.0; 3]);
    assert!(steep.forward[1] < -0.99, "pitch is floored at -85 degrees");
}
