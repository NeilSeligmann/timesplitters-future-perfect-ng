//! T1845 task A: the audio and ambience level hooks (`hooks_audio`). Expected values come from the decoded
//! constants and jump tables (docs/t-story-event-hooks-audio.md).

use tsfp_gameplay::story_events::hooks_audio::*;
use tsfp_gameplay::story_events::levels::LEVEL_HOOKS;

struct Lcg(u32);
impl AudioRng for Lcg {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        self.0
    }
    fn range_excluding(&mut self, low: u32, high: u32, excluded: u32) -> u32 {
        loop {
            let pick = low + self.next_u32() % (high - low + 1);
            if pick != excluded {
                return pick;
            }
        }
    }
}

fn player() -> PlayerAudio {
    PlayerAudio {
        active: true,
        ..Default::default()
    }
}

fn volume(effect: &AudioEffect) -> f32 {
    match effect {
        AudioEffect::SetLoop { volume, .. } => *volume,
        other => panic!("{other:?}"),
    }
}

#[test]
fn approach_snaps_and_steps() {
    let mut value = 0.0;
    assert!(!approach(&mut value, 1.0, 0.25));
    assert_eq!(value, 0.25);
    assert!(approach(&mut value, 0.3, 0.25));
    assert_eq!(value, 0.3);
    assert!(!approach(&mut value, 0.0, 0.1));
    assert!((value - 0.2).abs() < 1e-6);
}

#[test]
fn every_audio_hook_of_the_table_is_classified() {
    let mut classified = 0;
    for &(_, a, b) in LEVEL_HOOKS {
        for address in [a, b] {
            if address != 0 && audio_hook_kind(address).is_some() {
                classified += 1;
            }
        }
    }
    assert!(classified >= 30, "{classified}");
    assert_eq!(audio_hook_kind(0x222D90), Some(AudioHookKind::MaxMix));
    assert_eq!(audio_hook_kind(0x389B0), Some(AudioHookKind::Nothing));
    assert_eq!(audio_hook_kind(0x1DDC90), None);
}

#[test]
fn max_mix_uses_the_maximum_and_the_jump_table() {
    assert_eq!(max_mix_intensity(0x28), 0.2);
    assert_eq!(max_mix_intensity(0x29), 0.0);
    assert_eq!(max_mix_intensity(0x2D), 0.4);
    assert_eq!(max_mix_intensity(0x2E), 0.7);
    assert_eq!(max_mix_intensity(0x30), 0.7);
    assert_eq!(max_mix_intensity(0x2F), 0.4);
    assert_eq!(max_mix_intensity(0x34), 1.0);
    assert_eq!(max_mix_intensity(0x35), 0.0);
    let mut low = player();
    low.object_state = 0x28;
    let mut high = player();
    high.object_state = 0x31;
    let mut inactive = high;
    inactive.active = false;
    let mut value = 0.0;
    let effects = max_mix_hook(&[low, high, inactive], &mut value, 1.0);
    assert!((value - (1.0 / 60.0) * 0.444_444_45).abs() < 1e-7);
    assert_eq!(volume(&effects[0]), value);
    // Run until the maximum is reached.
    for _ in 0..200 {
        max_mix_hook(&[low, high], &mut value, 1.0);
    }
    assert_eq!(value, 1.0);
}

#[test]
fn mean_crossfade_snaps_to_the_mean_and_complements() {
    let mut a = player();
    a.actor_state = 6; // 0.95
    let mut b = player();
    b.actor_state = 18; // 0.95
    assert_eq!(mean_mix_intensity(12), 0.65);
    assert_eq!(mean_mix_intensity(19), 0.65);
    assert_eq!(mean_mix_intensity(13), 0.0);
    assert_eq!(mean_mix_intensity(2), 1.0);
    assert_eq!(mean_mix_intensity(100), 1.0);
    let mut value = 0.949_999_9;
    let effects = mean_crossfade_hook(&[a, b], &mut value);
    assert!((value - 0.95).abs() < 1e-5);
    assert!((volume(&effects[0]) - value).abs() < 1e-7);
    assert!((volume(&effects[1]) - (1.0 - value)).abs() < 1e-7);
    // A far value moves by 5 percent of the gap.
    let mut value = 0.0;
    mean_crossfade_hook(&[a], &mut value);
    assert!((value - 0.0475).abs() < 1e-5);
}

#[test]
fn outfit_predicates_follow_the_decoded_ranges() {
    let mut p = player();
    p.class_index = 5;
    p.skin_id = 0x446;
    assert!(outfit_predicate_220e60(&p));
    p.skin_id = 0x445;
    assert!(!outfit_predicate_220e60(&p));
    p.class_index = 11;
    p.skin_id = 0x432;
    assert!(!outfit_predicate_220e60(&p));
    p.skin_id = 0x434;
    assert!(outfit_predicate_220e60(&p));
    p.class_index = 13;
    assert!(outfit_predicate_220e60(&p));
    p.class_index = 26;
    p.skin_id = 0x466;
    assert!(!outfit_predicate_220e60(&p));
    p.class_index = 30;
    assert!(outfit_predicate_220e60(&p));
    p.class_index = 15;
    p.skin_id = 0x567;
    assert!(outfit_predicate_207f70(&p));
    p.class_index = 8;
    assert!(outfit_predicate_207f70(&p));
    p.class_index = 9;
    assert!(!outfit_predicate_207f70(&p));
    assert!(state_predicate_207440(&p));
    p.object_state = 0x29;
    assert!(!state_predicate_207440(&p));
}

#[test]
fn ramp_hooks_clamp_and_complement() {
    let mut pass = player();
    pass.class_index = 13; // k = 8 for 0x220E60
    let mut fail = player();
    fail.class_index = 0;
    let mut value = 0.7;
    let effects = outfit_ambience_hook(&[pass], &mut value, 60.0);
    assert_eq!(value, 0.75); // clamped
    assert_eq!(volume(&effects[0]), 0.25);
    outfit_ambience_hook(&[pass, fail], &mut value, 60.0);
    assert_eq!(value, 0.0);

    let mut value = 0.5;
    let mut ok = player();
    ok.object_state = 0;
    let effects = state_crossfade_hook(&[ok], &mut value, 15.0);
    assert!((value - 0.7).abs() < 1e-6);
    assert!((volume(&effects[0]) + volume(&effects[1]) - 1.0).abs() < 1e-6);
    let mut value = 0.5;
    let mut bad = player();
    bad.object_state = 0x29;
    state_crossfade_hook(&[bad], &mut value, 60.0);
    assert_eq!(value, 0.0);

    let mut value = 0.0;
    let mut fine = player();
    fine.class_index = 8;
    level_1d_crossfade_hook(&[fine], &mut value, 45.0);
    assert!((value - 1.0).abs() < 1e-6);
}

#[test]
fn weights_of_226350() {
    let mut p = player();
    p.flag_ff = true;
    assert_eq!(player_weight_226350(&p), 1.0);
    p.flag_ff = false;
    p.class_index = 2;
    p.position = [0.0, 13.0, 0.0];
    assert_eq!(player_weight_226350(&p), 0.5);
    p.position[1] = 100.0;
    assert_eq!(player_weight_226350(&p), 1.0);
    p.class_index = 26;
    assert_eq!(player_weight_226350(&p), 1.0);
    p.class_index = 3;
    assert_eq!(player_weight_226350(&p), 0.0);
    p.class_index = 100;
    assert_eq!(player_weight_226350(&p), 0.0);
    // Ellipse: standing on a focus.
    p.class_index = 7;
    p.position = WEIGHT_FOCI[0];
    let near = player_weight_226350(&p);
    assert!(near > 0.99);
    p.position = [200.0, 0.0, 0.0];
    assert_eq!(player_weight_226350(&p), 1.0 - 1.0);
}

#[test]
fn weight_crossfade_effects() {
    let mut p = player();
    p.flag_ff = true;
    let mut value = 0.0;
    let effects = weight_crossfade_hook(&[p], &mut value, 1.0);
    assert!((value - 0.05).abs() < 1e-6);
    assert!((volume(&effects[0]) - 0.94).abs() < 1e-6);
    assert!((volume(&effects[1]) - 0.04).abs() < 1e-6);
}

#[test]
fn class_list_semantics() {
    assert_eq!(class_list_weight(&LEVEL_3B_LIST, 99, 1.0), 0.5);
    assert_eq!(class_list_weight(&[2, 3, -0x64], 3, 0.25), 1.0);
    assert_eq!(class_list_weight(&[2, 3, -0x64], 9, 0.25), 0.25);
    assert_eq!(class_list_weight(&[-0x64], 3, 0.25), 0.25);
    assert_eq!(class_list_weight(&[2, -0x65, 3, -0x64], 9, 0.25), 0.5);
    let mut value = 0.0;
    let effects = class_list_hook(&[player()], &LEVEL_3B_LIST, 1.0, &mut value);
    assert_eq!(value, 0.025);
    assert_eq!(volume(&effects[0]), 0.025);
}

#[test]
fn periodic_sound_every_900() {
    let mut timer = 0.01;
    assert!(periodic_sound_hook(&mut timer, 1.0).is_empty() || timer > 0.0);
    let mut timer = 0.0;
    let effects = periodic_sound_hook(&mut timer, 1.0);
    assert_eq!(effects, vec![AudioEffect::PlayOnce { sound_id: 0x156F }]);
    assert!((timer - (900.0 - 1.0 / 60.0)).abs() < 1e-3);
}

#[test]
fn random_one_shot_state_machine() {
    let mut rng = Lcg(7);
    let mut state = RandomOneShot {
        handle: -1,
        last_pick: 0x1B11,
        timer: 0.01,
    };
    let mut started = Vec::new();
    let mut start = |pick: u32| {
        started.push(pick);
        5
    };
    assert_eq!(
        random_one_shot_hook(&mut state, |_| true, 0.0, &mut rng, &mut start),
        None
    );
    let pick = random_one_shot_hook(&mut state, |_| true, 1.0, &mut rng, &mut start).expect("pick");
    assert!((ONE_SHOT_PICK_RANGE.0..=ONE_SHOT_PICK_RANGE.1).contains(&pick));
    assert_ne!(pick, 0x1B11);
    assert_eq!(state.handle, 5);
    assert!((20.0..60.0).contains(&state.timer));
    assert_eq!(
        random_one_shot_hook(&mut state, |_| true, 1.0, &mut rng, &mut start),
        None
    );
    assert_eq!(state.handle, 5);
    random_one_shot_hook(&mut state, |_| false, 1.0, &mut rng, &mut start);
    assert_eq!(state.handle, -1);
}

#[test]
fn thunder_and_storm() {
    assert_eq!(storm_intensity(0x15), 1.0);
    assert_eq!(storm_intensity(0x16), 1.0);
    assert_eq!(storm_intensity(0x17), 0.0);
    assert_eq!(storm_intensity(0x18), 1.0);
    assert_eq!(storm_intensity(0x2D), 0.5);
    assert_eq!(storm_intensity(0x2E), 0.0);
    assert_eq!(storm_intensity(0x15 + 23), 1.0);
    assert_eq!(storm_intensity(0x15 + 24), 0.5);
    let mut rng = Lcg(1);
    let mut timer = 0.0;
    let effects = level_3d_hook(&mut timer, 1.0, &mut rng);
    assert_eq!(effects.len(), 1);
    match &effects[0] {
        AudioEffect::PlayRelative {
            sound_id,
            strength,
            offset,
        } => {
            assert!((0x163..=0x17A).contains(sound_id));
            assert!(*strength > 0.9 && *strength < 1.8);
            assert_eq!(offset[1], 0.0);
            assert!((offset[0].hypot(offset[2]) - 250.0).abs() < 0.01);
        }
        other => panic!("{other:?}"),
    }
    assert!((10.0..50.0).contains(&timer));
    // Intensity above 0.9 never plays.
    let mut timer = 0.0;
    assert!(thunder_hook(&mut timer, 1.0, 1.0, &mut rng).is_empty());
    let mut p = player();
    p.class_2a0 = 0x15;
    let mut value = 0.0;
    let mut timer = 5.0;
    let effects = storm_hook(&[p], &mut value, &mut timer, 1.0, &mut rng);
    assert_eq!(value, 0.03);
    assert!((volume(&effects[0]) - 0.06).abs() < 1e-7);
}

#[test]
fn lightning_and_level_16() {
    assert!(lightning_roll(59, 60));
    assert!(!lightning_roll(61, 1) || 61 % 60 < 1);
    assert!(lightning_roll(120, 1));
    assert!(!lightning_roll(121, 1));
    assert!(!lightning_roll(0, 0));
    let strike = lightning_point(0.0, 1.0);
    assert!((strike.point[0] - (36.78 + 70.0)).abs() < 1e-3);
    assert_eq!(strike.point[1], 40.0);
    assert!((strike.point[2] + 30.0).abs() < 1e-3);
    assert!(level_16_timeout(3, 100, 2.0, 221));
    assert!(!level_16_timeout(3, 100, 2.0, 220));
    assert!(!level_16_timeout(-1, 0, 0.0, 1000));
    assert_eq!(volume(&level_16_base_loop()), 1.3);
}
