//! T1845 round 3 task I: level 0x11 callees, level 0x1D weather, level 0x16 pieces (synthetic worlds only).
use tsfp_gameplay::story_events::hooks_l11::*;

fn alive(owned: bool, progress: f32) -> TrackedObject {
    TrackedObject {
        key_matches: true,
        health: 10.0,
        ext_flag_8: false,
        owner_has_item: owned,
        owned_by_jbth: owned,
        progress,
    }
}

#[test]
fn tally_finishes_slot_when_all_gone() {
    let mut table = [TallyEntry {
        net_type: 1,
        key: 0,
        state: 2,
    }; TALLY_ENTRIES];
    let out = jbth_tally(&TallyView::default(), &mut table);
    assert_eq!(
        out,
        vec![
            Effect::SlotFinish { slot: 1, arg: 3.0 },
            Effect::SetLocalWordFf4 { value: 0 }
        ]
    );
}

#[test]
fn tally_tracks_then_expires_entries() {
    let mut table = [TallyEntry::default(); TALLY_ENTRIES];
    let mut view = TallyView {
        jbth_exists: true,
        jbth_health: 5.0,
        ..Default::default()
    };
    view.found_keys[0] = Some(0x100);
    jbth_tally(&view, &mut table);
    assert_eq!(table[0].state, 1);
    assert_eq!(table[0].key, 0x100);
    // the object disappears: entry 0 goes to state 2
    jbth_tally(&view, &mut table);
    assert_eq!(table[0].state, 2);
    assert_eq!(table[0].key, 0);
}

#[test]
fn tally_damage_rules() {
    let mut table = [TallyEntry {
        net_type: 1,
        key: 5,
        state: 1,
    }; TALLY_ENTRIES];
    let mut view = TallyView {
        jbth_exists: true,
        jbth_health: 5.0,
        mode_byte: 0,
        ..Default::default()
    };
    for i in 0..3 {
        view.objects[i] = Some(alive(true, 0.95));
    }
    for i in 3..TALLY_ENTRIES {
        view.objects[i] = Some(alive(false, 0.0));
    }
    // no local player: 3 * 0.8 scaled by the mode (0 gives 0.5)
    let out = jbth_tally(&view, &mut table.clone());
    assert_eq!(out.len(), 1);
    match &out[0] {
        Effect::CommitDamage { amount, .. } => assert!((amount - 1.2).abs() < 1e-5),
        other => panic!("{other:?}"),
    }
    view.mode_byte = 2;
    match &jbth_tally(&view, &mut table.clone())[0] {
        Effect::CommitDamage { amount, .. } => assert!((amount - 3.6).abs() < 1e-5),
        other => panic!("{other:?}"),
    }
    // alive local player in state 1: fixed 8.0
    view.local_state = Some(1);
    assert_eq!(
        jbth_tally(&view, &mut table.clone())[0],
        Effect::CommitDamage {
            amount: 8.0,
            source_is_jbth: true
        }
    );
    // state 7 falls back to the tally; progress at the limit does not count
    view.local_state = Some(7);
    view.objects[0] = Some(alive(true, 0.9));
    view.objects[1] = None;
    let mut dead_table = table;
    dead_table[1].key = 0;
    let out = jbth_tally(&view, &mut dead_table);
    assert!(
        out.iter()
            .all(|e| !matches!(e, Effect::CommitDamage { amount, .. } if *amount > 8.0))
    );
    // a dead JBTH fails slot 1
    view.jbth_health = 0.0;
    assert!(jbth_tally(&view, &mut table).contains(&Effect::SlotFail { slot: 1, arg: 0.0 }));
}

#[test]
fn cz_pass_sets_and_clears_bit0() {
    let mut classes = [None; 11];
    classes[0] = Some(0x22A);
    classes[1] = Some(0x228);
    classes[2] = Some(0x229);
    classes[3] = Some(0x1);
    let out = cz_flag_pass(&classes, Some(0x225));
    assert_eq!(
        out[0],
        Effect::SetFlagBit0 {
            tag: CZ_TAGS[0],
            set: false
        }
    );
    assert_eq!(
        out[1],
        Effect::SetFlagBit0 {
            tag: CZ_TAGS[1],
            set: true
        }
    );
    assert_eq!(
        out[2],
        Effect::SetFlagBit0 {
            tag: CZ_TAGS[2],
            set: true
        }
    );
    assert_eq!(out.len(), 4);
    assert_eq!(out[3], Effect::Call3734e0 { tag: TAG_LCP2 });
    assert_eq!(
        cz_flag_pass(&[None; 11], Some(0x227)),
        vec![Effect::Call373500]
    );
    assert!(cz_flag_pass(&[None; 11], None).is_empty());
}

#[test]
fn tick_2106a0_by_class() {
    assert!(tick_2106a0(false, 0x1FE).is_empty());
    assert_eq!(
        tick_2106a0(true, 0x17F),
        vec![Effect::SetScriptState {
            state: 0x3E,
            clear_byte_offset: Some(0x54C)
        }]
    );
    assert_eq!(tick_2106a0(true, 0x1FE).len(), 2);
    assert_eq!(
        tick_2106a0(true, 0x1FE)[1],
        Effect::AddAmmo {
            ammo_type: 9,
            amount: 1000
        }
    );
    assert!(tick_2106a0(true, 0x999).is_empty());
}

#[test]
fn tick_210770_paths() {
    assert!(tick_210770(0x1, true, true, 0, true).is_empty());
    let out = tick_210770(0x233, true, true, 7, true);
    assert_eq!(
        out[0],
        Effect::SetScriptState {
            state: 0x3F,
            clear_byte_offset: Some(0x54C)
        }
    );
    assert!(matches!(out[1], Effect::SpawnTimedClone { seconds, .. } if seconds == 10.0));
    assert_eq!(
        out[2],
        Effect::SetScriptState {
            state: 0,
            clear_byte_offset: Some(0x58B)
        }
    );
}

#[test]
fn composite_ticks() {
    let out = tick_2102d0(Some(ZGH1), [1.0, 2.0, 3.0]);
    assert_eq!(
        out,
        vec![Effect::CompositeInit {
            key: COMPOSITE_KEY_1,
            kind: 1,
            resource: ResourceSlot::Offset18,
            position: [1.0, 1.5, 3.0]
        }]
    );
    let out = tick_2102d0(Some(ZGH2), [1.0, 2.0, 3.0]);
    assert!(
        matches!(out[0], Effect::CompositeInit { kind: 7, position: [_, y, _], .. } if y == 2.5)
    );
    assert!(tick_2102d0(None, [0.0; 3]).is_empty());
    // teardown with a slot found
    let t = Teardown {
        marker: Some(ZGH1),
        state_table_is_4: true,
        composite_slot: 3,
        has_parts: true,
    };
    let out = teardown_2103c0(&t);
    assert!(out.contains(&Effect::FreeCompositeSlot { slot: 3 }));
    assert_eq!(out.last(), Some(&Effect::ActivateParts));
    let t = Teardown {
        marker: None,
        state_table_is_4: false,
        composite_slot: -1,
        has_parts: true,
    };
    assert_eq!(teardown_2103c0(&t), vec![Effect::TailCall351260]);
    assert!(
        teardown_2103c0(&Teardown {
            marker: None,
            state_table_is_4: false,
            composite_slot: -1,
            has_parts: false
        })
        .is_empty()
    );
}

#[test]
fn particle_templates_and_init() {
    let [a, b] = particle_templates();
    assert_eq!(a.calls.len(), 9);
    assert_eq!(
        a.calls[0],
        TemplateCall::DrawInit {
            record: 0x78DAC0,
            kind: 0x329
        }
    );
    assert_eq!(
        b.calls[0],
        TemplateCall::DrawInit {
            record: 0x78D940,
            kind: 0x30A
        }
    );
    assert_eq!(a.handle_global, 0x78DA10);
    assert_eq!(b.handle_global, 0x78DA0C);
    let init = level11_init();
    assert_eq!(init.state_record_words[1], -1);
    assert_eq!(init.state_record_words[0], 0);
    assert_eq!(init.global_76c284, 2);
}

fn table() -> Vec<f32> {
    (0..SIN_TABLE_SIZE)
        .map(|i| (i as f32 * std::f32::consts::TAU / 2048.0).sin())
        .collect()
}

#[test]
fn rng_is_the_documented_lcg() {
    let mut rng = Rng { state: 1, calls: 0 };
    let first = rng.next();
    assert_eq!(first, (0x343FDu64 + 0x269EC3) as u32);
    assert_eq!(rng.calls, 1);
}

#[test]
fn rain_spawns_and_integrates() {
    let sine = table();
    let mut rng = Rng {
        state: 12345,
        calls: 0,
    };
    let mut state = RainState::default();
    // steps large enough that the roll always passes
    let out = rain_step(&mut state, &mut rng, &sine, 1000, [true, true]);
    assert!(state.live[0] && state.live[1]);
    assert!(matches!(
        out[0],
        Effect::SpawnParticle {
            template: 0x78DBA0,
            vector: [1.0, 0.0, 0.0],
            ..
        }
    ));
    assert_eq!(rng.calls, 6); // roll + angle + heading per slot
    let before = state.position[0];
    rain_step(&mut state, &mut rng, &sine, 0, [true, true]);
    assert_eq!(state.position[0], before); // zero steps integrate nothing
    let out = rain_step(&mut state, &mut rng, &sine, 1, [true, false]);
    assert!(out.iter().any(|e| matches!(
        e,
        Effect::MoveParticle {
            handle_slot: 0x75C060,
            ..
        }
    )));
    assert!(
        !state.live[1]
            || out
                .iter()
                .any(|e| matches!(e, Effect::SpawnParticle { .. }))
    );
    // the spawn point lies on the 500 radius quarter disc offset by (-20, -2.5)
    let mut rng = Rng {
        state: 99,
        calls: 0,
    };
    let mut state = RainState::default();
    let out = rain_step(&mut state, &mut rng, &sine, 1000, [false, false]);
    let p = match out[0] {
        Effect::SpawnParticle { position, .. } => position,
        _ => panic!(),
    };
    let r = ((p[0] + 20.0).powi(2) + (p[2] + 2.5).powi(2)).sqrt();
    assert!((r - 500.0).abs() < 1.0, "{r}");
    assert_eq!(p[1], 182.5);
}

#[test]
fn room_drop_rules() {
    let mut rng = Rng { state: 7, calls: 0 };
    // far camera, hit, big step count: spawns at 0.1 above the point
    let out = room_drop_step(&mut rng, 1000, true, [200.0, 0.0, 0.0]);
    assert_eq!(out.len(), 1);
    match &out[0] {
        Effect::SpawnTableEntry { kind, position } => {
            assert_eq!(*kind, 0x25);
            assert!((position[1] - -0.5).abs() < 1e-6);
        }
        other => panic!("{other:?}"),
    }
    // consumed 1 gate + 3 offsets
    assert_eq!(rng.calls, 4);
    // too close or no hit: nothing
    assert!(room_drop_step(&mut rng, 1000, true, [17.8, -0.6, -7.03]).is_empty());
    assert!(room_drop_step(&mut rng, 1000, false, [200.0, 0.0, 0.0]).is_empty());
    // zero steps never passes the gate and consumes one number
    let mut rng = Rng { state: 7, calls: 0 };
    assert!(room_drop_step(&mut rng, 0, true, [200.0, 0.0, 0.0]).is_empty());
    assert_eq!(rng.calls, 1);
    assert!(level1d_runs_weather(3) && !level1d_runs_weather(10));
}

#[test]
fn level16_emitters_branches() {
    let triple = level16_emitters(&Level16Emitters {
        volume: 0.7,
        target: 1.0,
        ..Default::default()
    });
    assert_eq!(triple.len(), 5); // base, approach, three emitters
    assert!(matches!(&triple[1], Effect::Approach { slot: 0x44C, step, .. } if *step == 0.1));
    match &triple[4] {
        Effect::Sound(s) => {
            assert_eq!(
                (s.id, s.handle, s.pitch, s.range, s.volume),
                (0x630, 0x45C, 1.0, 9.0, 0.7)
            );
            assert_eq!(s.position, Some([-121.43, 3.0, 19.0]));
        }
        other => panic!("{other:?}"),
    }
    let pair = level16_emitters(&Level16Emitters {
        record_flag_7: true,
        ..Default::default()
    });
    assert_eq!(pair.len(), 3);
    match &pair[1] {
        Effect::Sound(s) => assert_eq!(
            (s.id, s.handle, s.pitch, s.range, s.volume),
            (0x645, 0x464, 1.05, 18.0, 1.3)
        ),
        other => panic!("{other:?}"),
    }
    // a playing 0x645 handle keeps the pair
    assert_eq!(
        level16_emitters(&Level16Emitters {
            handle_468_active: true,
            ..Default::default()
        })
        .len(),
        3
    );
}

#[test]
fn level16_colour_and_scene() {
    assert_eq!(level16_colour_word(1.0) & 0x00FF_FFFF, 0x0010_1000);
    assert_eq!(level16_colour_word(1.0) >> 24, 0x10);
    assert_eq!(level16_colour_word(0.0) >> 24, 63 + 0x10);
    let out = level16_scene(&Level16Scene {
        player: true,
        owns_735_and_flag1: true,
        ..Default::default()
    });
    assert_eq!(
        out[0],
        Effect::SetScriptState {
            state: 8,
            clear_byte_offset: Some(0x54C)
        }
    );
    assert_eq!(
        out[1],
        Effect::AddAmmo {
            ammo_type: 1,
            amount: 1000
        }
    );
    let out = level16_scene(&Level16Scene {
        crw1_class_714: true,
        crw1_flags: 4,
        ..Default::default()
    });
    assert_eq!(
        out.last(),
        Some(&Effect::CallWithTag {
            address: 0x18AC60,
            tag: 0x4E4E_4833
        })
    );
    let out = level16_scene(&Level16Scene {
        czsc_class_776: true,
        czsc_flag2: true,
        ..Default::default()
    });
    assert!(out.contains(&Effect::AddAmmo {
        ammo_type: 0x21,
        amount: 0x14
    }));
    assert!(level16_scene(&Level16Scene::default()).is_empty());
}
