//! T1867: the 49 row effect kind table (`0x4DB490`) loaded from the image into `EmberWorld::rows`, and the hook ports'
//! direct `0xEFBD0` / `0x111AE0` recordings routed through `HookServices` (kind table + ported handlers). The image is
//! read at test time from `TSFP_XBE` or `../../build/default.xbe` and the image tests skip when it is absent (synthetic
//! rows are used otherwise); the pak replay needs `TSFP_SETU_PAK_DIR` and skips without it. No game bytes are committed.

use std::path::PathBuf;

use tsfp_gameplay::story_events::hook_wiring::{HookHost, HookOutput, HookState};
use tsfp_gameplay::story_events::hooks_audio::{AudioRng, PlayerAudio};
use tsfp_gameplay::story_events::hooks_big::KeyResolver;
use tsfp_gameplay::story_events::hooks_story::{HookView, NpcView};
use tsfp_gameplay::story_events::level_roles::{
    Level0bEngine, Level06Engine, Level11Engine, PlayerRecord,
};
use tsfp_gameplay::story_events::record::Link;
use tsfp_gameplay::story_events::svc_wiring::{HookServices, TraceEvent};
use tsfp_gameplay::story_events::world::{
    CallbackTarget, IdVariant, PropRequest, SpawnRequest, VoiceRequest, World,
};
use tsfp_gameplay::story_events::{BuildOptions, Engine, ModeContext};

struct Lcg(u32);

impl AudioRng for Lcg {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }
    fn range_excluding(&mut self, low: u32, high: u32, excluded: u32) -> u32 {
        loop {
            let value = low + self.next_u32() % (high - low + 1);
            if value != excluded {
                return value;
            }
        }
    }
}

struct Resolver;
impl KeyResolver for Resolver {
    fn entry_id(&self, key: u32) -> u32 {
        key
    }
}

struct Headless {
    level: u32,
    steps: u32,
    state: HookState,
    rng: Lcg,
    players: Vec<PlayerAudio>,
    others: Vec<[f32; 3]>,
    locals: Vec<[f32; 3]>,
    outputs: Vec<HookOutput>,
    /// FABRICATED engine facts for the wired views of levels 2, 6, 0xB, 0x11 (T1846 batch 8, `level_roles`).
    engine_facts: bool,
    ambience_started: u32,
    services: Option<HookServices>,
}

impl Headless {
    fn new(level: u32) -> Self {
        Headless {
            level,
            steps: 1,
            state: HookState::default(),
            rng: Lcg(1),
            players: Vec::new(),
            others: Vec::new(),
            locals: Vec::new(),
            outputs: Vec::new(),
            engine_facts: false,
            ambience_started: 0,
            services: None,
        }
    }
}

impl HookView for Headless {
    fn slot_finished(&self, _slot: u32) -> bool {
        false
    }
    fn npc(&self, _tag: u32) -> Option<NpcView> {
        None
    }
    fn player_area(&self) -> Option<u32> {
        None
    }
}

impl HookHost for Headless {
    fn state(&mut self) -> &mut HookState {
        &mut self.state
    }
    fn rng(&mut self) -> &mut dyn AudioRng {
        &mut self.rng
    }
    fn level_id(&self) -> u32 {
        self.level
    }
    fn steps(&self) -> u32 {
        self.steps
    }
    fn players_audio(&self) -> Vec<PlayerAudio> {
        self.players.clone()
    }
    fn key_resolver(&self) -> &dyn KeyResolver {
        &Resolver
    }
    fn services(&mut self) -> Option<&mut HookServices> {
        self.services.as_mut()
    }
    fn beep_positions(&self) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
        (self.others.clone(), self.locals.clone())
    }
    fn player_records(&self) -> Option<Vec<PlayerRecord>> {
        self.engine_facts.then(|| {
            vec![
                PlayerRecord {
                    alive: true,
                    character: 1,
                    state: 0x2C,
                    flag_570: false,
                    word_1568: 0
                };
                2
            ]
        })
    }
    fn level06_engine(&self) -> Option<Level06Engine> {
        self.engine_facts.then(Level06Engine::default)
    }
    fn level0b_engine(&self) -> Option<Level0bEngine> {
        self.engine_facts.then(Level0bEngine::default)
    }
    fn level11_engine(&self) -> Option<Level11Engine> {
        self.engine_facts.then(Level11Engine::default)
    }
    fn ombh_count(&self) -> Option<u32> {
        self.engine_facts.then_some(0)
    }
    fn start_ambience(&mut self, _sound_id: u32) -> i32 {
        self.ambience_started += 1;
        7
    }
}

impl World for Headless {
    fn play_voice(&mut self, _r: &VoiceRequest) -> Option<u32> {
        None
    }
    fn voice_finished(&mut self, _h: u32) -> bool {
        true
    }
    fn run_callback(&mut self, _c: u32, _t: CallbackTarget) {}
    fn set_objective(&mut self, _k: u32, _i: u32) {}
    fn control_prop(&mut self, _r: &PropRequest) {}
    fn prop_settled(&mut self, _k: u32) -> bool {
        true
    }
    fn spawn_object(&mut self, _r: &SpawnRequest) -> Option<u32> {
        None
    }
    fn object_ready(&mut self, _h: u32) -> bool {
        true
    }
    fn destroy_object(&mut self, _h: u32) {}
    fn player_count(&self) -> usize {
        1
    }
    fn player_matches(&self, _p: usize, _v: IdVariant, _i: &[u32]) -> bool {
        true
    }
    fn first_player_flag(&self) -> bool {
        false
    }
    fn tag_exists(&self, _k: u32) -> bool {
        false
    }
    fn poll_link(&mut self, _l: &Link) -> bool {
        false
    }
    fn hook_host(&mut self) -> Option<&mut dyn HookHost> {
        Some(self)
    }
    fn hook_output(&mut self, output: &HookOutput) {
        self.outputs.push(output.clone());
    }
}

use std::collections::BTreeMap;

use tsfp_gameplay::story_events::effect_routing::{
    EffectRequest, RouteResult, RoutedEffect, from_barrier, from_l11, from_roof, from_scene,
    from_spark, from_strike, handler_of, route,
};
use tsfp_gameplay::story_events::hooks_audio::StrikeRequest;
use tsfp_gameplay::story_events::hooks_fx::RoofEffect;
use tsfp_gameplay::story_events::hooks_l06_0b::Level0bBarrier;
use tsfp_gameplay::story_events::hooks_l11;
use tsfp_gameplay::story_events::hooks_scene::{ParticleAnchor, SceneEffect};
use tsfp_gameplay::story_events::hooks_story::SparkEffect;
use tsfp_gameplay::story_events::svc_kind_table::{self, KIND_ROWS};
use tsfp_gameplay::story_events::svc_particle_efbd0::SpawnArgs;

fn engine(mode_id: u8, level_id: u32) -> Engine {
    Engine::new(
        &[],
        BuildOptions {
            context: ModeContext {
                mode_id,
                level_id,
                ..ModeContext::default()
            },
        },
    )
}

fn player(object_state: u32) -> PlayerAudio {
    PlayerAudio {
        active: true,
        object_state,
        ..PlayerAudio::default()
    }
}

/// The test-time image, when present.
fn real_image() -> Option<Vec<u8>> {
    let path = std::env::var_os("TSFP_XBE")
        .map_or_else(|| PathBuf::from("../../build/default.xbe"), PathBuf::from);
    std::fs::read(path).ok()
}

/// Services with the real tables when the image exists, else the synthetic image.
fn services_with_tables(players: u32) -> (HookServices, bool) {
    let mut services = HookServices::new(players);
    if let Some(image) = real_image() {
        assert!(services.load_kind_tables(&image));
        (services, true)
    } else {
        assert!(services.load_kind_tables(&svc_kind_table::synthetic_image()));
        (services, false)
    }
}

fn kind_args(kind: u32) -> SpawnArgs {
    SpawnArgs {
        kind,
        extra: 0,
        position: [1.0, 2.0, 3.0],
        direction: None,
        source: None,
        owner: 0,
        seconds: 0.0,
        sound: false,
    }
}

#[test]
fn synthetic_image_tables_load_into_the_ember_world() {
    let image = svc_kind_table::synthetic_image();
    let rows = svc_kind_table::load_kind_rows(&image).unwrap();
    assert_eq!(rows.len(), KIND_ROWS);
    assert_eq!(
        (rows[10].effect_a, rows[10].effect_b, rows[10].flags),
        (3, -1, 0)
    );
    assert_eq!(
        (rows[11].light_radius, rows[11].radius, rows[11].flags),
        (11.0, 11.5, 1)
    );
    let defined = svc_kind_table::load_effect_defined(&image).unwrap();
    assert_eq!(
        (defined.len(), defined[6], defined[7]),
        (0x400, false, true)
    );
    let mut services = HookServices::new(1);
    assert!(services.load_kind_tables(&image));
    assert!(
        !services.load_kind_tables(&[0u8; 0x400]),
        "an image without the tables changes nothing"
    );
}

#[test]
fn bytes_at_rejects_unmapped_ranges() {
    let image = svc_kind_table::synthetic_image();
    assert!(svc_kind_table::bytes_at(&image, 0x4D_B000, 4).is_some());
    assert!(svc_kind_table::bytes_at(&image, 0x4D_AFFC, 4).is_none());
    assert!(svc_kind_table::bytes_at(&image, 0x53_AFFD, 4).is_none());
}

#[test]
fn real_image_rows_are_well_formed() {
    let Some(image) = real_image() else {
        eprintln!("skip: no XBE image");
        return;
    };
    let rows = svc_kind_table::load_kind_rows(&image).unwrap();
    assert_eq!(rows.len(), KIND_ROWS);
    assert!(
        rows.iter()
            .all(|r| r.flags <= 1 && r.radius.is_finite() && r.light_radius.is_finite())
    );
    assert!(rows.iter().any(|r| r.flags == 1) && rows.iter().any(|r| r.flags == 0));
    let defined = svc_kind_table::load_effect_defined(&image).unwrap();
    assert!(defined.iter().filter(|d| **d).count() > 100);
    // The static initialiser of `0x4E0FA0` leaves some ids a row names undefined (INFERRED: the loader fills those
    // entries at run time); report them instead of asserting.
    let undefined: Vec<(usize, i32)> = rows
        .iter()
        .enumerate()
        .take(48)
        .flat_map(|(kind, row)| {
            [row.effect_a, row.effect_b]
                .into_iter()
                .map(move |id| (kind, id))
        })
        .filter(|(_, id)| *id != -1 && !defined[*id as usize])
        .collect();
    eprintln!("row effect ids undefined in the static table: {undefined:?}");
}

#[test]
fn every_kind_routes_through_its_handler_and_records() {
    let (mut services, real) = services_with_tables(1);
    let mut report = Vec::new();
    for kind in 0..0x40u32 {
        let routed = route(&mut services, "test", EffectRequest::Kind(kind_args(kind)));
        let RouteResult::Kind(result) = &routed.result else {
            panic!()
        };
        assert!(
            result.slot.is_some(),
            "kind {kind}: the 16 entry pool has room (entries are released or kept)"
        );
        let mut counts = BTreeMap::<&str, u32>::new();
        for event in &routed.trace {
            if let TraceEvent::Ember { service, .. } = event {
                *counts.entry(service).or_default() += 1;
            }
        }
        report.push(format!(
            "kind {kind:#x} {} started {} {counts:?}",
            handler_of(kind),
            result.started
        ));
        if kind == 15 {
            assert_eq!(
                counts.get("emitter_created"),
                Some(&2),
                "ember creates its two emitters"
            );
        }
    }
    eprintln!(
        "tables {}\n{}",
        if real { "real" } else { "synthetic" },
        report.join("\n")
    );
}

#[test]
fn default_path_kinds_trigger_the_row_effects_only_with_loaded_tables() {
    let Some(image) = real_image() else {
        eprintln!("skip: no XBE image");
        return;
    };
    let rows = svc_kind_table::load_kind_rows(&image).unwrap();
    // kind 7 (default path) has effect ids in its row: the loaded tables make `trigger_effect` run, the fabricated
    // all -1 rows never do
    assert!(rows[7].effect_a != -1);
    let count_triggers = |services: &mut HookServices| {
        let routed = route(services, "test", EffectRequest::Kind(kind_args(7)));
        routed
            .trace
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    TraceEvent::Ember {
                        service: "trigger_effect",
                        ..
                    }
                )
            })
            .count()
    };
    let mut fabricated = HookServices::new(1);
    let (mut loaded, _) = services_with_tables(1);
    let (before, after) = (count_triggers(&mut fabricated), count_triggers(&mut loaded));
    assert_eq!(before, 0);
    assert!(after > 0, "the loaded row triggers its effects");
}

#[test]
fn port_records_convert_to_requests() {
    let strike = from_strike(&StrikeRequest {
        point: [1.0, 2.0, 3.0],
    });
    let EffectRequest::Kind(args) = &strike else {
        panic!()
    };
    assert_eq!((args.kind, args.seconds, args.sound), (0xC, 2.0, true));
    let drop = from_l11(&hooks_l11::Effect::SpawnTableEntry {
        kind: 0x25,
        position: [0.0; 3],
    })
    .unwrap();
    assert!(matches!(
        drop,
        EffectRequest::Kind(SpawnArgs { kind: 0x25, .. })
    ));
    let rain = from_l11(&hooks_l11::Effect::SpawnParticle {
        handle_slot: 0x75C060,
        template: 0x78DBA0,
        position: [0.0; 3],
        vector: [1.0, 0.0, 0.0],
    })
    .unwrap();
    assert!(matches!(
        rain,
        EffectRequest::Emitter {
            template: 0x78DBA0,
            ..
        }
    ));
    assert!(from_l11(&hooks_l11::Effect::SetColourWords { value: 0 }).is_none());
    let mut barrier = Level0bBarrier::default();
    let outcome = barrier.step(&[], &[]);
    let emitters: Vec<_> = outcome.effects.iter().filter_map(from_barrier).collect();
    assert_eq!(emitters.len(), 1);
    assert!(matches!(
        emitters[0],
        EffectRequest::Emitter {
            template: 0x78ED40,
            flags: 0x10000,
            ..
        }
    ));
    let bursts = from_roof(&RoofEffect::SecondExpired {
        bursts: [[0.0; 3]; 3],
    });
    assert_eq!(bursts.len(), 3);
    assert!(bursts.iter().all(|b| matches!(
        b,
        EffectRequest::Kind(SpawnArgs {
            kind: 0x16,
            extra: 0x17,
            ..
        })
    )));
    assert!(from_roof(&RoofEffect::FirstExpired).is_empty());
    assert!(from_spark(&SparkEffect::RegisterDraw).is_none());
    let spark = from_spark(&SparkEffect::Spark {
        position: [0.0; 3],
        velocity: [5.0, 0.89, 0.0],
    })
    .unwrap();
    assert!(matches!(
        spark,
        EffectRequest::Emitter {
            template: 0x7B0A80,
            ..
        }
    ));
    assert!(
        from_scene(&SceneEffect::Particle {
            template: 1,
            anchor: ParticleAnchor::Piece(0),
            vector: [0.0; 3]
        })
        .is_none()
    );
    assert!(
        from_scene(&SceneEffect::Particle {
            template: 1,
            anchor: ParticleAnchor::Point([0.0; 3]),
            vector: [0.0; 3]
        })
        .is_some()
    );
}

/// Drive every converted port record through the services and return what each did.
fn run_ports(services: &mut HookServices) -> Vec<RoutedEffect> {
    let mut requests: Vec<(&'static str, EffectRequest)> = vec![
        (
            "0x220AF0",
            from_strike(&StrikeRequest {
                point: [36.0, 40.0, -30.0],
            }),
        ),
        (
            "0x207830",
            from_l11(&hooks_l11::Effect::SpawnTableEntry {
                kind: 0x25,
                position: [17.8, -0.5, -7.03],
            })
            .unwrap(),
        ),
        (
            "0x207A70",
            from_l11(&hooks_l11::Effect::SpawnParticle {
                handle_slot: 0x75C060,
                template: 0x78DBA0,
                position: [0.0, 10.0, 0.0],
                vector: [1.0, 0.0, 0.0],
            })
            .unwrap(),
        ),
    ];
    let mut barrier = Level0bBarrier::default();
    requests.extend(
        barrier
            .step(&[], &[])
            .effects
            .iter()
            .filter_map(from_barrier)
            .map(|r| ("0x1EC300", r)),
    );
    requests.extend(
        from_roof(&RoofEffect::SecondExpired {
            bursts: tsfp_gameplay::story_events::hooks_fx::ROOF_BURSTS,
        })
        .into_iter()
        .map(|r| ("0x1F9DA0", r)),
    );
    requests.extend(
        from_spark(&SparkEffect::Spark {
            position: [0.0, 1.0, 0.0],
            velocity: [5.0, 0.89, 0.0],
        })
        .map(|r| ("0x21C110", r)),
    );
    requests
        .into_iter()
        .map(|(origin, request)| route(services, origin, request))
        .collect()
}

#[test]
fn every_port_effect_runs_through_the_services() {
    for players in [1u32, 2] {
        let (mut services, real) = services_with_tables(players);
        let routed = run_ports(&mut services);
        assert_eq!(routed.len(), 8);
        for effect in &routed {
            match (&effect.request, &effect.result) {
                (EffectRequest::Kind(args), RouteResult::Kind(result)) => {
                    assert!(
                        result.slot.is_some(),
                        "{} kind {:#x}",
                        effect.origin,
                        args.kind
                    );
                }
                (EffectRequest::Emitter { .. }, RouteResult::Emitter(created)) => {
                    assert_eq!(*created, 1, "{}", effect.origin)
                }
                other => panic!("{other:?}"),
            }
            eprintln!(
                "players {players} {} {} {:?} {} events (tables {})",
                effect.origin,
                effect.kind().map_or("emitter".to_string(), |k| format!(
                    "kind {k:#x} {}",
                    handler_of(k)
                )),
                effect.result,
                effect.trace.len(),
                if real { "real" } else { "synthetic" },
            );
        }
    }
}

#[test]
fn strike_hook_output_is_followed_by_its_routed_effect() {
    // level 0x30, step 60: the roll `% 60 < 60` always passes, so every frame strikes
    let mut engine = engine(10, 0x30);
    let mut world = Headless::new(0x30);
    world.steps = 60;
    world.players = vec![player(0x2C)];
    world.services = Some(services_with_tables(1).0);
    for _ in 0..4 {
        engine.frame_level_hook(1, 0, &mut world);
    }
    let strikes = world
        .outputs
        .iter()
        .filter(|o| matches!(o, HookOutput::Strike(_)))
        .count();
    assert!(strikes >= 1, "strike frames");
    let mut pairs = 0;
    for window in world.outputs.windows(2) {
        if let [HookOutput::Strike(_), HookOutput::Effect(effect)] = window {
            assert_eq!(effect.kind(), Some(0xC));
            assert!(matches!(&effect.result, RouteResult::Kind(r) if r.slot.is_some()));
            pairs += 1;
        }
    }
    assert_eq!(
        pairs, strikes,
        "every strike is followed by exactly its routed effect"
    );
    // without services the output list is the T1846 one
    let mut plain = Headless::new(0x30);
    plain.steps = 60;
    let mut engine = self::engine(10, 0x30);
    engine.frame_level_hook(1, 0, &mut plain);
    assert!(
        !plain
            .outputs
            .iter()
            .any(|o| matches!(o, HookOutput::Effect(_)))
    );
}

#[test]
fn real_pak_replay_reports_the_effect_kinds_each_level_starts() {
    let Some(dir) = std::env::var_os("TSFP_SETU_PAK_DIR").map(PathBuf::from) else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let (mut ran, mut routed_total) = (0, 0);
    let mut lines = Vec::new();
    for name in names.iter().filter(|n| n.ends_with(".pak")) {
        let stem = name.trim_end_matches(".pak");
        let mut split = stem.split("__");
        let _ = split.next();
        let kind = split.next().unwrap();
        let level: u32 = split
            .next()
            .unwrap()
            .split('_')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let mode_id = match kind {
            "story" => 10,
            "arcade" => 3,
            _ => 5,
        };
        for players in [1u32, 2] {
            let mut engine = engine(mode_id, level);
            let mut world = Headless::new(level);
            world.engine_facts = true;
            world.steps = 60;
            world.players = (0..players).map(|k| player(0x2C + k)).collect();
            world.services = Some(services_with_tables(players).0);
            for _ in 0..120 {
                engine.frame_level_hook(1, 0, &mut world);
            }
            ran += 1;
            let mut kinds = BTreeMap::<u32, (u32, u32, u32, u32)>::new();
            for output in &world.outputs {
                if let HookOutput::Effect(effect) = output {
                    routed_total += 1;
                    let kind = effect.kind().expect("dispatch routes kinds");
                    let RouteResult::Kind(result) = &effect.result else {
                        panic!()
                    };
                    let entry = kinds.entry(kind).or_default();
                    entry.0 += 1;
                    entry.1 += u32::from(result.started);
                    entry.2 += effect.trace.len() as u32;
                    entry.3 += u32::from(result.slot.is_none());
                }
            }
            let summary = if kinds.is_empty() {
                "no effects".to_string()
            } else {
                kinds
                    .iter()
                    .map(|(k, (n, s, t, e))| {
                        format!(
                            "kind {k:#x} x{n} started {s} pool-empty {e} handler [{}] recorded {t}",
                            handler_of(*k)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            lines.push(format!(
                "level {level:#x} ({kind}) players {players}: {summary}"
            ));
        }
    }
    eprintln!("{}", lines.join("\n"));
    assert!(ran >= 80, "{ran} runs");
    assert!(routed_total > 0, "level 0x30 strikes reach the kind table");
}

/// T1868: the pool tick `0xDC7B0` / `0xEFF90` (`HookServices::tick_effects`) starts the waiting entries the hook dispatch
/// allocates. Level 0x30 strikes allocate kind 0xC with seconds 2.0 (timer -2.0): they must start 120 frames later.
#[test]
fn pool_tick_starts_the_waiting_level_0x30_strike() {
    let mut engine = engine(3, 0x30);
    let mut world = Headless::new(0x30);
    world.steps = 60;
    world.players = vec![player(0x2C)];
    world.services = Some(services_with_tables(1).0);
    let mut strike_frames = Vec::new();
    for frame in 0..300 {
        let before = world.outputs.len();
        engine.frame_level_hook(1, 0, &mut world);
        if world.outputs[before..]
            .iter()
            .any(|o| matches!(o, HookOutput::Strike(_)))
        {
            strike_frames.push(frame);
        }
        world.services.as_mut().unwrap().tick_effects(1);
        if frame == 0 {
            world.steps = 0; // one strike only: the later frames roll zero steps
        }
    }
    assert_eq!(strike_frames, vec![0], "one strike at frame 0");
    let services = world.services.as_ref().unwrap();
    let starts: Vec<_> = services
        .trace()
        .into_iter()
        .filter_map(|e| match e {
            TraceEvent::Ember {
                service: "tick_start",
                args,
            } => Some(args),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "the waiting kind 0xC entry starts exactly once: {starts:?}"
    );
    assert_eq!(starts[0][1], 0xC);
}

#[test]
fn real_pak_replay_with_the_pool_tick_reports_started_effects() {
    let Some(dir) = std::env::var_os("TSFP_SETU_PAK_DIR").map(PathBuf::from) else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let (mut ran, mut started_total) = (0, 0);
    let mut lines = Vec::new();
    for name in names.iter().filter(|n| n.ends_with(".pak")) {
        let stem = name.trim_end_matches(".pak");
        let mut split = stem.split("__");
        let _ = split.next();
        let kind = split.next().unwrap();
        let level: u32 = split
            .next()
            .unwrap()
            .split('_')
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let mode_id = match kind {
            "story" => 10,
            "arcade" => 3,
            _ => 5,
        };
        for players in [1u32, 2] {
            let mut engine = engine(mode_id, level);
            let mut world = Headless::new(level);
            world.engine_facts = true;
            world.steps = 60;
            world.players = (0..players).map(|k| player(0x2C + k)).collect();
            world.services = Some(services_with_tables(players).0);
            for frame in 0..1200 {
                engine.frame_level_hook(1, 0, &mut world);
                world.services.as_mut().unwrap().tick_effects(1);
                if frame == 0 {
                    world.steps = 1;
                }
            }
            ran += 1;
            let mut kinds = BTreeMap::<u32, u32>::new();
            let (mut trails, mut sprays) = (0, 0);
            for event in world.services.as_ref().unwrap().trace() {
                if let TraceEvent::Ember { service, args } = event {
                    match service {
                        "tick_start" => *kinds.entry(args[1]).or_default() += 1,
                        "trail" => trails += 1,
                        "spray_burst" => sprays += 1,
                        _ => {}
                    }
                }
            }
            let started = kinds.values().sum::<u32>();
            started_total += started;
            let strikes = world
                .outputs
                .iter()
                .filter(|o| matches!(o, HookOutput::Strike(_)))
                .count() as u32;
            assert!(
                started <= strikes,
                "every started entry is a strike the dispatch allocated"
            );
            if strikes > 0 {
                lines.push(format!("level {level:#x} ({kind}) players {players}: strikes {strikes}, tick starts {kinds:?}, trail records {trails}, sprays {sprays}"));
            }
        }
    }
    eprintln!("{}", lines.join("\n"));
    assert!(ran >= 80, "{ran} runs");
    assert!(
        started_total > 0,
        "the tick starts at least the level 0x30 strikes"
    );
}
