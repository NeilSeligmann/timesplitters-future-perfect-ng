//! T1845 task C: the four large level hooks (`story_events::hooks_big`), synthetic worlds only.

use tsfp_gameplay::story_events::hooks_big::*;

fn ident(key: u32) -> u32 {
    key
}

fn alive(character: u32, state: u32) -> PlayerView {
    PlayerView {
        alive: true,
        character,
        state,
    }
}

#[test]
fn level06_sweeps_slot9_and_ends_on_tail_call_for_state_25() {
    let view = Level06View {
        player_count: 2,
        player_flag_570: vec![true, false],
        states: vec![0, 0x25],
        words_1568: vec![0, 0],
        ..Default::default()
    };
    let out = level06_hook(&view);
    assert_eq!(out[0], Effect::SlotFinish { slot: 9, arg: 0.0 });
    assert_eq!(out[1], Effect::CallSub { address: 0x20B480 });
    assert_eq!(out.last(), Some(&Effect::TailCall { address: 0x208810 }));
    assert_eq!(
        out.iter()
            .filter(|e| matches!(e, Effect::SlotFinish { .. }))
            .count(),
        1
    );
}

#[test]
fn level06_cdwd_prop_runs_clear_restart_set_unless_549_without_flag() {
    let mut view = Level06View {
        player_count: 1,
        states: vec![0],
        words_1568: vec![9],
        cdwd_active: true,
        cptb_present: true,
        cptb_key_is_549: true,
        cptb_flag_bit0: false,
        ..Default::default()
    };
    assert!(
        !level06_hook(&view)
            .iter()
            .any(|e| matches!(e, Effect::PropRestart { .. }))
    );
    view.cptb_flag_bit0 = true;
    let out = level06_hook(&view);
    let tail: Vec<_> = out.iter().rev().take(3).rev().cloned().collect();
    assert_eq!(
        tail,
        vec![
            Effect::PropClearFlags {
                prop: LEVEL06_CDWD_KEY,
                bits: 0x4000
            },
            Effect::PropRestart {
                prop: LEVEL06_CDWD_KEY
            },
            Effect::PropSetFlags {
                prop: LEVEL06_CDWD_KEY,
                bits: 2
            },
        ]
    );
}

#[test]
fn level06_hud_binding_paths() {
    let mut view = Level06View {
        local_player: true,
        ..Default::default()
    };
    view.slot4 = SlotFacts {
        above_one: true,
        finished: false,
    };
    assert!(level06_hook(&view).contains(&Effect::BindHud {
        local_word_ff4: true
    }));
    view.local_ff4_matches_first = true;
    assert!(!level06_hook(&view).contains(&Effect::BindHud {
        local_word_ff4: true
    }));
    // slot 2 path always runs the damage over time step.
    view.slot4 = SlotFacts::default();
    view.slot2 = SlotFacts {
        above_one: true,
        finished: false,
    };
    assert!(level06_hook(&view).contains(&Effect::CallSub { address: 0x208AF0 }));
    // several players in mode 0xA skip both paths.
    view.mode_id = 0x0A;
    view.player_count = 2;
    assert!(!level06_hook(&view).contains(&Effect::CallSub { address: 0x208AF0 }));
}

#[test]
fn level0b_loop_promotes_slots_and_sets_the_flag() {
    let view = Level0bView {
        players: vec![
            alive(0x5EB, 0x4C),
            PlayerView {
                alive: false,
                character: 0x5E7,
                state: 0,
            },
        ],
        tags_present: vec![],
        ..Default::default()
    };
    let mut globals = Level0bGlobals {
        handle_e64: -1,
        ..Default::default()
    };
    let out = level0b_hook(&view, &mut globals, &ident);
    assert!(out.contains(&Effect::SlotPromote { slot: 1, arg: 9.0 }));
    assert!(out.contains(&Effect::SlotPromote { slot: 3, arg: 13.0 }));
    assert!(
        !out.contains(&Effect::SlotPromote { slot: 0, arg: 7.0 }),
        "dead player is skipped"
    );
    assert!(
        out.contains(&Effect::CallSub { address: 0x1EC300 }),
        "action state keeps the music running"
    );
    let sets = Level0bView {
        players: vec![alive(0x632, 0)],
        ..Default::default()
    };
    level0b_hook(&sets, &mut globals, &ident);
    assert_eq!(globals.flag_e4, 1);
}

#[test]
fn level0b_stops_the_music_handle_when_nobody_is_in_action() {
    let view = Level0bView {
        players: vec![alive(1, 0)],
        ..Default::default()
    };
    let mut globals = Level0bGlobals {
        handle_e64: 7,
        ..Default::default()
    };
    let out = level0b_hook(&view, &mut globals, &ident);
    assert!(out.contains(&Effect::SoundStop { handle: 0x4FFE64 }));
    assert_eq!(globals.handle_e64, -1);
}

#[test]
fn level0b_local_state_chain() {
    let mut globals = Level0bGlobals {
        handle_e64: -1,
        flag_e4: 1,
        ..Default::default()
    };
    // state 1 with the flag and no OILY finishes slot 0xD.
    let view = Level0bView {
        local_state: Some(1),
        ..Default::default()
    };
    assert!(
        level0b_hook(&view, &mut globals, &ident).contains(&Effect::SlotFinish {
            slot: 0x0D,
            arg: 0.0
        })
    );
    // state 2 and 3 with none of the four tags finish 0x15, any tag blocks it.
    let view = Level0bView {
        local_state: Some(3),
        ..Default::default()
    };
    assert!(
        level0b_hook(&view, &mut globals, &ident).contains(&Effect::SlotFinish {
            slot: 0x15,
            arg: 0.0
        })
    );
    let view = Level0bView {
        local_state: Some(3),
        tags_present: vec![tags::HIDE],
        ..Default::default()
    };
    assert!(
        !level0b_hook(&view, &mut globals, &ident).contains(&Effect::SlotFinish {
            slot: 0x15,
            arg: 0.0
        })
    );
    // state 0x4B with key 0x558 picks activate or finish by [0x75BF0C].
    let view = Level0bView {
        local_state: Some(0x4B),
        first_entry_key: Some(0x558),
        ..Default::default()
    };
    assert!(
        level0b_hook(&view, &mut globals, &ident).contains(&Effect::SlotFinish {
            slot: 0x1F,
            arg: 0.0
        })
    );
    globals.flag_0c = 1;
    assert!(
        level0b_hook(&view, &mut globals, &ident).contains(&Effect::SlotActivate {
            slot: 0x1F,
            arg: 0.0
        })
    );
}

#[test]
fn level0b_tail_sequence_and_wind() {
    let view = Level0bView {
        wind_present: true,
        first_player_1548_is_14: true,
        handle_present: true,
        input_bytes_any: true,
        ..Default::default()
    };
    let mut globals = Level0bGlobals {
        handle_e64: -1,
        volume_5c: 100.0,
        volume_ec: 50.0,
        ..Default::default()
    };
    let out = level0b_hook(&view, &mut globals, &ident);
    assert!(out.contains(&Effect::SoundLoop {
        handle: 0x50025C,
        id: 4,
        volume: 1.0,
        pitch: 1.0,
        flags: 0
    }));
    assert!(out.contains(&Effect::SetObjectFloat {
        tag: tags::WIND,
        offset: 0x6C,
        value: -12.5
    }));
    assert_eq!(out.last(), Some(&Effect::SetHandleValue { value: 0xFFFF }));
}

#[test]
fn level11_slot_pairs_and_activate_all() {
    let view = Level11View {
        players: vec![alive(0x43A, 0), alive(0x45E, 0), alive(0x448, 0)],
        ..Default::default()
    };
    let out = level11_hook(&view, &ident);
    assert!(out.contains(&Effect::SlotFinish { slot: 4, arg: 0.0 }));
    assert!(out.contains(&Effect::SlotFinish { slot: 8, arg: 0.0 }));
    assert_eq!(
        out.iter()
            .filter(|e| matches!(e, Effect::SlotActivate { .. }))
            .count(),
        5
    );
    assert_eq!(out.last(), Some(&Effect::TailCall { address: 0x213C40 }));
}

#[test]
fn level11_object_ticks_installed_only_when_the_slot_word_is_zero() {
    let view = Level11View {
        htbj: Some(true),
        jani: Some(false),
        hgz1: Some(true),
        hgz2: Some(false),
        hgz_gate: true,
        ..Default::default()
    };
    let out = level11_hook(&view, &ident);
    assert!(out.contains(&Effect::SetStateTick {
        tag: LEVEL11_TAG_HTBJ,
        function: LEVEL11_HTBJ_TICK
    }));
    assert!(
        !out.iter()
            .any(|e| matches!(e, Effect::SetStateTick { tag, .. } if *tag == LEVEL11_TAG_JANI))
    );
    assert!(out.contains(&Effect::SetField19c {
        tag: LEVEL11_TAG_HGZ2,
        function: LEVEL11_HGZ_FIELD19C
    }));
    let ungated = Level11View {
        hgz1: Some(true),
        ..Default::default()
    };
    assert!(
        !level11_hook(&ungated, &ident)
            .iter()
            .any(|e| matches!(e, Effect::SetField19c { tag, .. } if *tag == LEVEL11_TAG_HGZ1))
    );
}

#[test]
fn level11_intensity_table() {
    assert_eq!(level11_state_intensity(1), (0.5, false));
    assert_eq!(level11_state_intensity(2), (0.75, false));
    assert_eq!(level11_state_intensity(3), (1.0, false));
    assert_eq!(level11_state_intensity(8), (0.8, false));
    let finishing: Vec<u32> = (1..=98)
        .filter(|&state| level11_state_intensity(state).1)
        .collect();
    assert_eq!(
        finishing.len(),
        1,
        "exactly one object state finishes slot 0xB"
    );
    assert_eq!(level11_state_intensity(finishing[0]), (1.0, true));
    assert_eq!(level11_state_intensity(0x200), (-0.1, false));
    assert_eq!(level11_state_intensity(0), (-0.1, false));
}

#[test]
fn approach_matches_the_original_helper() {
    let mut value = 0.0;
    assert!(!approach(&mut value, 1.0, 0.25));
    assert_eq!(value, 0.25);
    let mut value = 0.9;
    assert!(approach(&mut value, 1.0, 0.25));
    assert_eq!(value, 1.0);
    let mut value = 0.5;
    assert!(!approach(&mut value, 0.0, 0.25));
    assert_eq!(value, 0.25);
}

#[test]
fn level11_mixer_ramps_intensity_and_plays_the_row_shout() {
    let mut mixer = Level11Mixer::default();
    let mut input = Level11MixerInput {
        steps: 1,
        step_scale: 1.0,
        cutscene_id: -1,
        player_states: vec![3],
        speaker: None,
    };
    let out = level11_mixer_tick(&mut mixer, &input);
    assert!((mixer.intensity - 1.5 / 60.0).abs() < 1e-6);
    assert!(out.shout.is_none());
    input.steps = 0;
    let before = mixer;
    level11_mixer_tick(&mut mixer, &input);
    assert_eq!(mixer, before);
    // A speaker that has lasted four calls and matches row 0 plays its shout once.
    input.steps = 1;
    input.speaker = Some((0x41A, [0.0; 3]));
    let mut shout = None;
    for _ in 0..6 {
        let out = level11_mixer_tick(&mut mixer, &input);
        shout = shout.or(out.shout);
    }
    assert_eq!(shout, Some((0x56F, 0.5)));
    assert_eq!(mixer.shout_row, 1);
}

#[test]
fn level11_cutscene_forces_negative_target() {
    let mut mixer = Level11Mixer {
        intensity: 0.05,
        ..Default::default()
    };
    let input = Level11MixerInput {
        steps: 1,
        step_scale: 1.0,
        cutscene_id: 0x10,
        player_states: vec![3],
        speaker: None,
    };
    level11_mixer_tick(&mut mixer, &input);
    assert!(mixer.intensity < 0.05);
}

#[test]
fn level11_slot_declarations_hide_slot_1_in_multiplayer_mode_a() {
    let solo = level11_slot_declarations(0x0A, 1);
    assert_eq!(solo.len(), 12);
    assert!(solo.iter().any(|s| s.0 == 1));
    let multi = level11_slot_declarations(0x0A, 2);
    assert!(!multi.iter().any(|s| s.0 == 1));
    assert_eq!(multi[0], (0, 0x12D0, [0; 3]));
    assert_eq!(multi.last(), Some(&(0xB, 0, [2; 3])));
}

#[test]
fn strip_speed_rule() {
    assert_eq!(strip_scroll_speed(0x21, Some(0x9E0)), None);
    assert_eq!(strip_scroll_speed(0x22, None), None);
    assert_eq!(strip_scroll_speed(0x22, Some(0x9DE)), Some(4.0));
    assert_eq!(strip_scroll_speed(0x22, Some(0x9DF)), Some(0.0));
    assert_eq!(strip_scroll_speed(0x22, Some(0xA95)), Some(0.0));
    assert_eq!(strip_scroll_speed(0x22, Some(0xA96)), Some(4.0));
}

#[test]
fn round2_fade_and_music_targets() {
    use tsfp_gameplay::story_events::hooks_big::{level0b_music_targets, level06_fade_step};
    let (v, expired) = level06_fade_step(0.0005, 0, 1);
    assert!(expired && v < 0.0);
    let (v, expired) = level06_fade_step(1.0, 2, 1);
    assert!(!expired && (v - (1.0 - 0.058 / 60.0)).abs() < 1e-6);
    assert_eq!(level0b_music_targets(0x200, &[5]), (0, 0));
    assert_eq!(level0b_music_targets(0, &[]), (0, 0));
    assert_eq!(level0b_music_targets(0, &[5]), (35, 0));
    assert_eq!(level0b_music_targets(0, &[9]), (50, 100));
    assert_eq!(level0b_music_targets(0, &[9, 17]), (75, 100));
    assert_eq!(level0b_music_targets(0, &[200]), (100, 0));
}

#[test]
fn hitp_damage_needs_state_area_and_flag() {
    let view = HitpView {
        hitp_state: Some(0x4C),
        player_area: 1,
        flag_f4: true,
    };
    let out = level0b_hitp_damage(&view);
    assert_eq!(
        out[0],
        Effect::ObjectDamage {
            tag: TAG_HITP,
            amount: HITP_DAMAGE
        }
    );
    assert!(out.contains(&Effect::SlotFinish {
        slot: 0xF,
        arg: 0.0
    }));
    assert!(out.contains(&Effect::SlotActivate {
        slot: 0x10,
        arg: 0.0
    }));
    assert!(
        level0b_hitp_damage(&HitpView {
            player_area: 0x4C,
            ..view
        })
        .is_empty()
    );
    assert!(
        level0b_hitp_damage(&HitpView {
            flag_f4: false,
            ..view
        })
        .is_empty()
    );
    assert!(
        level0b_hitp_damage(&HitpView {
            hitp_state: Some(2),
            ..view
        })
        .is_empty()
    );
    assert!(
        level0b_hitp_damage(&HitpView {
            hitp_state: None,
            ..view
        })
        .is_empty()
    );
}

#[test]
fn jum_check_finishes_slot_seven_when_all_are_gone() {
    assert_eq!(
        level0b_jum_check([false; 4]),
        vec![Effect::SlotFinish { slot: 7, arg: 5.0 }]
    );
    assert!(level0b_jum_check([false, true, false, false]).is_empty());
}

#[test]
fn every_call_sub_of_the_hooks_has_a_status() {
    assert_eq!(
        call_sub_status(0x1EA640),
        SubStatus::Ported("hooks_big::level0b_music_targets")
    );
    assert_eq!(
        call_sub_status(0x1ECA60),
        SubStatus::Ported("hooks_big::level0b_hitp_damage")
    );
    assert!(matches!(
        call_sub_status(0x1F48A0),
        SubStatus::Undecoded(_) | SubStatus::Ported(_)
    ));
}
