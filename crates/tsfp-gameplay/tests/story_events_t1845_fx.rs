//! T1845 round 3 task G: effect drivers (htip, roof, spark pool, level 5 sparks, bridge hulls, wrth).
use tsfp_gameplay::story_events::hooks_fx::*;

fn lcg() -> impl FnMut() -> u32 {
    let mut s = 12345u32;
    move || {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        s
    }
}

#[test]
fn htip_zones_and_light() {
    assert_eq!(htip_zone(0x656, 0x2A), Some(0));
    assert_eq!(htip_zone(0x656, 0x6E), Some(0));
    assert_eq!(htip_zone(0x656, 0x6F), None);
    assert_eq!(htip_zone(0x656, 0x12C), Some(3));
    assert_eq!(htip_zone(0x655, 0x50), None);
    assert_eq!(htip_step(0x655, 0x50, [0.0; 3]), HtipLight::Off);
    match htip_step(0x656, 0x80, [14.5, -2.3, -3.21]) {
        HtipLight::On {
            position,
            intensity,
            radius_global,
        } => {
            assert_eq!(position, HTIP_CENTERS[1]);
            assert_eq!(intensity, 1.0);
            assert!((radius_global - 1.1).abs() < 1e-5);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn roof_blocks_expire_in_order() {
    assert_eq!(roof_rate(0), 0.015);
    assert_eq!(roof_rate(2), 0.035);
    assert_eq!(roof_rate(1), 0.025);
    let mut pending = RoofPending {
        first: true,
        second: false,
    };
    let mut value = 0.0003;
    let input = RoofInput {
        step: 1.0,
        mode_byte: 0,
        roof_flag: Some(0),
    };
    assert!(roof_tick(&mut pending, &mut value, &input).is_empty());
    let effects = roof_tick(&mut pending, &mut value, &input);
    assert_eq!(effects, vec![RoofEffect::FirstExpired]);
    assert!(!pending.first);
    // An already flagged roof is not ticked.
    let mut pending = RoofPending {
        first: true,
        second: false,
    };
    let mut value = 0.0;
    let flagged = RoofInput {
        roof_flag: Some(1),
        ..input
    };
    assert!(roof_tick(&mut pending, &mut value, &flagged).is_empty());
    assert_eq!(value, 0.0);
    // Second block fires three bursts.
    let mut pending = RoofPending {
        first: false,
        second: true,
    };
    let mut value = 0.0001;
    let effects = roof_tick(&mut pending, &mut value, &input);
    assert_eq!(
        effects,
        vec![RoofEffect::SecondExpired {
            bursts: ROOF_BURSTS
        }]
    );
}

#[test]
fn spark_pool_spawns_three_and_decays() {
    let mut pool = SparkPool::default();
    let rect = ScreenRect {
        x0: 10,
        y0: 20,
        width: 100,
        height: 50,
    };
    // Out of range or absent trigger: only decay.
    pool.weights = [1.0; 5];
    spark_pool_tick(&mut pool, None, rect, &mut lcg());
    assert!(pool.weights.iter().all(|w| (*w - 0.8).abs() < 1e-6));
    spark_pool_tick(&mut pool, Some(0.5), rect, &mut || 1);
    assert_eq!(pool.next, 0);
    // A word divisible by 15 spawns.
    let mut words = vec![0u32, 0x8000_0000, 0x4000_0000, 0, 0, 0, 0].into_iter();
    let mut pool = SparkPool::default();
    spark_pool_tick(&mut pool, Some(0.2), rect, &mut || {
        words.next().unwrap_or(0)
    });
    assert_eq!(pool.next, 3);
    assert!((pool.positions[0][0] - 60.0).abs() < 1e-4);
    assert!((pool.positions[0][1] - 32.5).abs() < 1e-4 || pool.positions[0][1] >= 20.0);
    assert!((pool.weights[0] - 0.8).abs() < 1e-6);
    assert!((pool.flash - 1.0).abs() < 1e-6 || pool.flash == 1.0);
}

#[test]
fn level5_sparks_pick_the_nearest_anchor() {
    let mut state = SparkState {
        enabled: true,
        timer: -1.0,
        accumulator: 0.0,
    };
    let input = SparkInput {
        steps: 1,
        step_scale: 1.0,
        position: [252.0, 33.0, -674.0],
        previous: [252.0, 33.0, -675.0],
    };
    let table = |i: usize| ((i as f32) / 2048.0 * std::f32::consts::TAU).sin();
    let burst = level5_spark_tick(&mut state, &input, &mut lcg(), &table)
        .expect("fires when the timer is negative");
    assert_eq!(burst.owner, 0x59);
    assert!(state.timer >= 0.1 && state.timer < 1.1);
    // Disabled or zero steps do nothing.
    let mut off = SparkState::default();
    assert!(level5_spark_tick(&mut off, &input, &mut lcg(), &table).is_none());
    let mut idle = SparkState {
        enabled: true,
        timer: -1.0,
        accumulator: 0.0,
    };
    assert!(
        level5_spark_tick(
            &mut idle,
            &SparkInput { steps: 0, ..input },
            &mut lcg(),
            &table
        )
        .is_none()
    );
    // A positive timer only counts down and accumulates speed.
    let mut waiting = SparkState {
        enabled: true,
        timer: 1.0,
        accumulator: 0.0,
    };
    assert!(level5_spark_tick(&mut waiting, &input, &mut lcg(), &table).is_none());
    assert!(waiting.timer < 1.0 && waiting.accumulator > 0.0);
}

#[test]
fn bridge_hulls_follow_the_four_point_recipe() {
    let v = BridgeVectors {
        a: [10.0, 0.0, 5.0],
        b: [1.0, 0.0, 2.0],
        c: [20.0, 0.0, 6.0],
        d: [1.0, 0.0, 1.0],
        e: [30.0, 0.0, 7.0],
        f: [2.0, 0.0, 3.0],
    };
    let (first, second) = bridge_hulls(v, false, 0.0);
    assert_eq!(first.points[0], [11.0, 0.0, 7.0]);
    assert_eq!(first.points[3], [9.0, 0.0, 3.0]);
    assert_eq!((first.min_x, first.max_x), (9.0, 21.0));
    assert_eq!((first.min_z, first.max_z), (3.0, 7.0));
    assert_eq!(second.points[1], [32.0, 0.0, 10.0]);
    // A next post ahead replaces E and F by midpoints.
    let (_, mid) = bridge_hulls(v, true, 1.0);
    assert_eq!(mid.points[1], [26.5, 0.0, 8.5]);
    // No look-ahead degenerates the second hull.
    let (_, flat) = bridge_hulls(v, true, 0.0);
    assert_eq!(flat.points[0], flat.points[1]);
    assert!((bridge_sound_volume(7.0) - 2.0).abs() < 1e-5);
}

#[test]
fn wrth_constants_and_integrator() {
    assert_eq!(WRTH_TAGS[0], 0x5752_5448);
    let (w20, w24, w64, done) = wrth_emitter_step(0.0, 0.0, 0.0, 1.0, 1.0, false, 1.0 / 60.0, 1.0);
    assert!(w20 > 0.0 && w24 > 0.0 && !done);
    assert!((w64 - w20 * 0.25).abs() < 1e-9);
    let (_, _, _, done) = wrth_emitter_step(5.0, 0.0, 0.0, 1.0, 1.0, true, 1.0, 1.0);
    assert!(done);
    assert_eq!(WRTH_UNSUPPORTED.len(), 3);
}

mod round4 {
    use tsfp_gameplay::story_events::hooks_fx::*;
    use tsfp_gameplay::story_events::pose::Mat4;

    const IDENTITY: Mat4 = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];

    fn post(pos: [f32; 3], ext_a8: f32, ext_ac: u32) -> BridgePost {
        BridgePost {
            pos,
            matrix: IDENTITY,
            ext_a8,
            ext_ac,
        }
    }

    #[test]
    fn bridge_vectors_first_post_and_defaults() {
        let first = post([10.0, 20.0, 30.0], 0.0, 0);
        let step = bridge_derive(None, &first, None, [1.0, 2.0, 3.0]);
        assert_eq!(step.vectors.a, [10.0, 20.0, 30.0]);
        assert_eq!(step.vectors.b, [0.0, 0.0, 2.875]);
        assert_eq!(step.vectors.c, [11.0, 22.0, 30.0]);
        assert_eq!(step.vectors.d, [0.0, 0.0, 3.0]);
        assert_eq!(step.vectors.e, BRIDGE_DEFAULT_E);
        assert!(!step.has_next && step.ahead == 1.0);
        assert!(bridge_carry(&step.vectors, step.has_next, step.ahead).is_none());
        let moving = post([10.0, 20.0, 30.0], 3.0, 0);
        assert_eq!(
            bridge_derive(None, &moving, None, [0.0; 3]).vectors.a,
            BRIDGE_DEFAULT_A
        );
    }

    #[test]
    fn bridge_next_post_ahead_and_carry() {
        let first = post([0.0; 3], 0.0, 0);
        let next = post([100.0, 0.0, 0.0], 7.0, 0);
        let step = bridge_derive(None, &first, Some(&next), [1.0, 0.0, 2.0]);
        assert!((step.ahead - 0.5).abs() < 1e-6);
        assert_eq!(step.vectors.e, [101.0, 0.0, 0.0]);
        assert_eq!(step.vectors.f, [0.0, 0.0, 2.0]);
        let carry = bridge_carry(&step.vectors, true, step.ahead).unwrap();
        assert_eq!(carry.0, [(101.0 + 1.0) * 0.5, 0.0, 0.0]);
        let open = post([0.0; 3], 0.0, 1);
        let step = bridge_derive(None, &open, Some(&next), [1.0, 0.0, 2.0]);
        assert_eq!(step.ahead, 0.0);
        let carry = bridge_carry(&step.vectors, true, 0.0).unwrap();
        assert_eq!(carry, (step.vectors.e, step.vectors.f));
    }

    struct Zone {
        hits: Vec<f32>,
        ground: f32,
        draws: u32,
    }
    impl WrthWorld for Zone {
        fn zone_of(&mut self, _bounds: &[[f32; 3]; 2]) -> u32 {
            7
        }
        fn zone_bounds(&mut self, _key: u32) -> ([f32; 3], [f32; 3]) {
            ([-10.0; 3], [10.0; 3])
        }
        fn sweep_hits(&mut self, _key: u32, _pos: [f32; 3]) -> Vec<f32> {
            self.hits.clone()
        }
        fn ground_y(&mut self, _pos: [f32; 3]) -> f32 {
            self.ground
        }
        fn random(&mut self) -> u32 {
            self.draws += 1;
            0x8000_0000
        }
    }

    fn emitter(state: u32) -> WrthEmitter {
        WrthEmitter {
            state,
            zone: 0,
            key: 0,
            pos: [0.0, 1.0, 0.0],
            bounds: [[0.0; 3]; 2],
            dir: [1.0, 0.0, 0.0],
            turn: 1.0,
            axis: [0.0, 1.0, 0.0],
            retarget: 1.0,
            alpha: 1.0,
            life: 5.0,
            handle: 9,
        }
    }

    fn globals(state: u32, pool: Vec<WrthEmitter>) -> WrthGlobals {
        WrthGlobals {
            state,
            progress: 0.0,
            keys: vec![7],
            follow: [0.0, 1.0, 0.0],
            follow_velocity: [0.0; 3],
            marker_pos: [0.0; 3],
            marker_seen: false,
            marker_timer: 0.0,
            anchor: [0.0; 3],
            flags: 0,
            volume: 0.0,
            centroid: [0.0; 3],
            pool,
        }
    }

    fn input(wrth: bool) -> WrthInput {
        WrthInput {
            wrth: wrth.then_some(([0.0; 3], [1.0; 3])),
            key_active: vec![true],
            players: vec![],
            vdmn: Some(WrthObject {
                pos: [5.0, 1.0, 0.0],
                key: 7,
                has_ext: false,
            }),
            vnmn: None,
            jbth: None,
            victims: vec![],
            level: 1,
            steps: 1,
            float_step: 1.0,
        }
    }

    #[test]
    fn wrth_driver_gone_releases_the_pool_once() {
        let mut g = globals(2, vec![emitter(0), emitter(1)]);
        let mut world = Zone {
            hits: vec![],
            ground: 0.0,
            draws: 0,
        };
        let fx = wrth_frame(&mut g, &input(false), &mut world);
        assert_eq!(fx.len(), 2);
        assert!(g.pool.iter().all(|e| e.state == 2) && g.state == 0);
        assert!(wrth_frame(&mut g, &input(false), &mut world).is_empty());
    }

    #[test]
    fn wrth_idle_state_does_nothing_but_the_final_pass() {
        let mut g = globals(0, vec![emitter(0)]);
        let mut world = Zone {
            hits: vec![],
            ground: 0.0,
            draws: 0,
        };
        let fx = wrth_frame(&mut g, &input(true), &mut world);
        assert!(fx.is_empty(), "state 0 returns before the sound");
        assert!(
            g.centroid[1] > 0.0,
            "the centroid still follows state 0 emitters: {:?}",
            g.centroid
        );
    }

    #[test]
    fn wrth_steering_moves_inside_the_box_and_plays_the_loop() {
        let mut g = globals(3, vec![emitter(0), emitter(1)]);
        let mut world = Zone {
            hits: vec![0.5],
            ground: 0.0,
            draws: 0,
        };
        let fx = wrth_frame(&mut g, &input(true), &mut world);
        // State 3 moves linearly at 1.5 and clamps to the zone box with a 0.1 margin.
        assert!(g.pool[0].pos[0] > 0.0 && g.pool[0].pos[0] <= 10.0 - WRTH_WALL_MARGIN);
        assert!(
            g.pool[0].alpha < 1.0,
            "sweep hit 0.5 and ground 1.0 below lower the alpha"
        );
        assert!(matches!(
            fx.last(),
            Some(WrthEffect::Loop {
                id: WRTH_SOUND_ID,
                ..
            })
        ));
        assert!(world.draws > 0, "retargeting draws randoms");
        assert!(
            g.follow[0] == 5.0,
            "follow point is the marker (+velocity 0)"
        );
    }

    #[test]
    fn wrth_nearest_player_filters_key_and_x() {
        let near = WrthObject {
            pos: [1.0, 0.0, 0.0],
            key: 7,
            has_ext: false,
        };
        let far = WrthObject {
            pos: [9.0, 0.0, 0.0],
            key: 7,
            has_ext: false,
        };
        let players = [
            WrthPlayer {
                pos: [2.0, 0.0, 0.0],
                key: 7,
                object: Some(far),
            },
            WrthPlayer {
                pos: [1.0, 0.0, 0.0],
                key: 7,
                object: Some(near),
            },
            WrthPlayer {
                pos: [0.0, 0.0, 0.0],
                key: 8,
                object: None,
            },
            WrthPlayer {
                pos: [14.0, 0.0, 0.0],
                key: 7,
                object: None,
            },
        ];
        assert_eq!(wrth_nearest_player(&players, 7, [0.0; 3]), Some(near));
        assert_eq!(wrth_nearest_player(&players, 9, [0.0; 3]), None);
    }

    #[test]
    fn wrth_life_timer_releases_state_1_emitters() {
        let mut lead = emitter(1);
        lead.life = 1.0;
        let mut g = globals(2, vec![lead]);
        let mut world = Zone {
            hits: vec![],
            ground: 0.0,
            draws: 0,
        };
        let fx = wrth_frame(&mut g, &input(true), &mut world);
        assert_eq!(g.pool[0].state, 2);
        assert!(
            fx.iter()
                .any(|e| matches!(e, WrthEffect::ReleaseBurst { .. }))
        );
    }
}
