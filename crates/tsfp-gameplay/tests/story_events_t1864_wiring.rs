//! T1864: the T1856 services wired into the level hooks (`0x1F48A0` through `StripServices`, `0x1E2B80` through the
//! prelude adapters, the impact stack over the shared RefCell memory). The world is FABRICATED, the control flow above
//! it MEASURED (T1856 documents). The real replay reads level ids from the SETU pak names (TSFP_SETU_PAK_DIR, skipped
//! without it, nothing committed).

use std::path::PathBuf;

use tsfp_gameplay::story_events::hook_wiring::{HookHost, HookOutput, HookState};
use tsfp_gameplay::story_events::hooks_audio::{AudioRng, PlayerAudio};
use tsfp_gameplay::story_events::hooks_big::KeyResolver;
use tsfp_gameplay::story_events::hooks_story::{HookEffect, HookView, NpcView};
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

use tsfp_gameplay::story_events::hooks_big::Effect as BigEffect;
use tsfp_gameplay::story_events::strip_update::{Lcg64, Service, StripRng};
use tsfp_gameplay::story_events::svc_objects::{GLOBAL_SLOT_BASE, GLOBAL_SLOT_COUNT};
use tsfp_gameplay::story_events::svc_particles::{PARTICLE_INDEX, PARTICLE_RING};
use tsfp_gameplay::story_events::svc_slot_teardown::{Machine, NoHooks, RNG_STATE};
use tsfp_gameplay::story_events::svc_wiring::{CellMemory, GuestRam, fabricated};
use tsfp_gameplay::story_events::sweep_dynamic_shape::{
    EffectMemory, ImpactServices, ambient_effect, rng as rng_153b40,
};

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

fn services_of(outputs: &[HookOutput], function: u32) -> Vec<&Vec<TraceEvent>> {
    outputs
        .iter()
        .filter_map(|o| match o {
            HookOutput::Service(report) if report.function == function => Some(&report.trace),
            _ => None,
        })
        .collect()
}

fn run_level(level: u32, players: u32, frames: u32, with_services: bool) -> Headless {
    let mut engine = engine(10, level);
    let mut world = Headless::new(level);
    world.engine_facts = true;
    world.players = (0..players).map(|k| player(0x2C + k)).collect();
    if with_services {
        world.services = Some(HookServices::new(players));
    }
    for _ in 0..frames {
        engine.frame_level_hook(1, 0, &mut world);
    }
    world
}

/// Strip scene with a `PITH` object in state 0x20F: the ten particle spawns of `0x1F48A0` run all their services.
fn seed_pith_scene(ram: &mut GuestRam, players: u32) {
    let slot_base = 0x5400_0000u32;
    let object = 0x5500_0000u32;
    let owner = 0x5500_1000u32;
    let table = 0x5500_4000u32;
    let records = 0x5500_6000u32;
    ram.set_word(GLOBAL_SLOT_BASE, slot_base);
    ram.set_word(GLOBAL_SLOT_COUNT, 1);
    let slot = slot_base + players * 0xEA0 + 0x18;
    ram.set_word(slot, 1);
    ram.set_word(slot - 0x18, object);
    ram.set_word(slot + 0xBD8, 0x5500_8000);
    ram.set_word(0x5500_8008, 0x4854_4950);
    ram.set_word(object + 4, owner);
    ram.set_word(object + 0x22C, u32::MAX);
    ram.set_float(object + 0x64, 1.0);
    ram.set_float(object + 0x68, 0.5);
    ram.set_float(object + 0x6C, 2.0);
    // class record at owner + 0xD4: id 0xBF8, animation seconds 8.8 (state 527)
    ram.set_word(owner + 0xD4, 0xBF8);
    ram.set_float(owner + 0xD4 + 8, 8.8);
    ram.set_word(owner + 4, table);
    ram.set_word(table, 1);
    ram.set_word(table - 200 + 0x60, records);
    ram.set_byte(records + 2, 3);
    ram.set_byte(records + 10 + 3, 0x80);
}

#[test]
fn memory_proxy_shares_one_cell_across_all_traits() {
    use tsfp_gameplay::story_events::strip_update::GuestMemory;
    use tsfp_gameplay::story_events::svc_effect_submit::SvcMemory;
    let cell = std::cell::RefCell::new(GuestRam::default());
    let mut strip = CellMemory(&cell);
    let mut effect = CellMemory(&cell);
    GuestMemory::write_u32(&mut strip, 0x1001, 0xA1B2_C3D4);
    assert_eq!(SvcMemory::r8(&effect, 0x1001), 0xD4);
    EffectMemory::write_u8(&mut effect, 0x1004, 0x7F);
    assert_eq!(GuestMemory::read_u32(&strip, 0x1003), 0x7FB2);
    assert_eq!(cell.borrow().word(0x1001), 0x7FB2_C3D4);
}

#[test]
fn strip_spawner_runs_the_particle_path_with_traced_services() {
    let mut services = HookServices::new(1);
    seed_pith_scene(&mut services.ram.borrow_mut(), 1);
    let report = services.run_strip_spawner();
    assert_eq!(report.fault, None);
    let count = |wanted: Service| {
        report
            .trace
            .iter()
            .filter(|e| matches!(e, TraceEvent::Strip { service, .. } if *service == wanted))
            .count()
    };
    assert_eq!(count(Service::FindObjectA), 1);
    assert_eq!(count(Service::StartEffect), 1);
    assert_eq!(count(Service::WorldMatrix), 10);
    assert_eq!(count(Service::EmitterOffset), 10);
    assert_eq!(count(Service::TransformVec), 20);
    assert_eq!(count(Service::SpawnParticle), 10);
    assert_eq!(
        count(Service::LookupByKey),
        1,
        "EFAS is not in the key list"
    );
    assert_eq!(count(Service::FindObjectB), 4);
    // The particle ring advanced ten slots and the first slot holds the type word 3.
    let ram = services.ram.borrow();
    assert_eq!(ram.word(PARTICLE_INDEX), 10);
    assert_eq!(ram.word(PARTICLE_RING), 3);
    // `[object + 0x22C]` holds the handle of the sound submit (`StartEffect` result), the owner flag is 1.
    let start = report.trace.iter().find_map(|e| match e {
        TraceEvent::Strip {
            service: Service::StartEffect,
            result,
            ..
        } => Some(*result),
        _ => None,
    });
    assert_eq!(Some(ram.word(0x5500_0000 + 0x22C)), start);
    assert_eq!(ram.word(0x5500_1000 + 0x30), 1);
}

#[test]
fn strip_spawner_trace_is_deterministic_and_rng_is_the_64_bit_lcg() {
    let run = || {
        let mut services = HookServices::new(2);
        seed_pith_scene(&mut services.ram.borrow_mut(), 2);
        services.run_strip_spawner().trace
    };
    assert_eq!(run(), run());
    let mut reference = Lcg64::default();
    let mut services = HookServices::new(1);
    seed_pith_scene(&mut services.ram.borrow_mut(), 1);
    services.run_strip_spawner();
    // 10 spawns draw 3 values each (entry, record, vertex): the state advanced exactly 30 times.
    for _ in 0..30 {
        reference.next();
    }
    assert_eq!(services.rng, reference);
}

#[test]
fn spawn_object_service_runs_c1950_over_recording_leaves() {
    let mut services = HookServices::new(1);
    let position = 0x5900_0000;
    services.ram.borrow_mut().set_float(position, -422.8);
    let args = [0, 8, 5, 0, position, 0, 0, 0, 0, 0];
    let first = services.spawn_object(args);
    let second = services.spawn_object(args);
    assert_eq!(first, fabricated::OBJECT_HEAP);
    assert_eq!(second, fabricated::OBJECT_HEAP + fabricated::OBJECT_SIZE);
    let leaves: Vec<&str> = services
        .trace()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::SpawnLeaf { leaf, .. } => Some(*leaf),
            _ => None,
        })
        .collect();
    assert!(
        leaves.contains(&"pool_get") && leaves.contains(&"init_object"),
        "{leaves:?}"
    );
}

#[test]
fn impact_services_share_the_cell_with_the_effect_memory() {
    let mut services = HookServices::new(1);
    let position = [1.0, 2.0, 3.0];
    let (rng_calls_before, height) = services.impact_services(7.5, |memory, impact| {
        let before = EffectMemoryProbe::calls(memory);
        ambient_effect(memory, impact, position);
        (before, impact.ground_height(position, 0.01, 0.5))
    });
    assert_eq!(rng_calls_before, 0);
    assert_eq!(height, 7.5);
    // the ambient effect drew once from the 64 bit generator in the shared cell
    assert_eq!(services.ram.borrow().word(0x74_97C8), 1);
    // and submitted a request: the scratch record holds the effect id 0xD4B..0xD4E and the position
    let ram = services.ram.borrow();
    let id = ram.word(fabricated::SUBMIT_SCRATCH);
    assert!((0xD4B..=0xD4E).contains(&id), "{id:#x}");
    assert_eq!(
        f32::from_bits(ram.word(fabricated::SUBMIT_SCRATCH + 28)),
        1.0
    );
}

struct EffectMemoryProbe;
impl EffectMemoryProbe {
    fn calls(memory: &mut dyn EffectMemory) -> u32 {
        memory.read_u32(0x74_97C8)
    }
}

#[test]
fn rng_153b40_is_64_bit_and_153bd0_is_a_separate_32_bit_lcg() {
    // 0x153B40: 64 bit state at 0x4E7950/54, the same sequence as `Lcg64`.
    let mut ram = GuestRam::default();
    ram.set_word(0x4E_7950, 0xFFFF_FFFF);
    ram.set_word(0x4E_7954, 0x1);
    let mut reference = Lcg64 {
        state: 0x1_FFFF_FFFF,
    };
    for _ in 0..8 {
        assert_eq!(rng_153b40(&mut ram), reference.next());
    }
    // 0x153BD0: 32 bit state at 0x7497CC, 15 bit output scaled to [0, 1).
    let mut ram = GuestRam::default();
    ram.set_word(RNG_STATE, 0x1234_5678);
    let mut state = 0x1234_5678u32;
    let mut hooks = NoHooks;
    for _ in 0..8 {
        state = state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
        let expected = f64::from((state >> 16) & 0x7FFF) * f64::from(f32::from_bits(0x3800_0100));
        let got = Machine {
            memory: &mut ram,
            hooks: &mut hooks,
        }
        .rng_float();
        assert_eq!(got.to_bits(), expected.to_bits());
        assert!((0.0..1.0).contains(&got));
        assert_eq!(
            ram.word(RNG_STATE),
            state,
            "the 32 bit state, not the 64 bit pair"
        );
    }
    assert_eq!(
        ram.word(0x4E_7950),
        0,
        "0x153BD0 never touches the 0x153B40 state"
    );
}

#[test]
fn build_debris_is_the_trait_name_for_0x103a00() {
    // Compile time guard for the T1864 rename (`release_slot` was a misnomer, 0x103A00 builds a debris effect).
    struct Probe(u32);
    impl ImpactServices for Probe {
        fn disturb(&mut self, _: [f32; 3], _: f32, _: u32) {}
        fn spawn_request(&mut self, _: [u32; 12]) {}
        fn build_debris(&mut self, slot: u32) {
            self.0 = slot;
        }
        fn ground_height(&mut self, _: [f32; 3], _: f32, _: f32) -> f32 {
            0.0
        }
    }
    let mut probe = Probe(0);
    probe.build_debris(0x742360);
    assert_eq!(probe.0, 0x742360);
}

#[test]
fn level_4_and_50_prelude_run_through_the_services() {
    for level in [4u32, 0x50] {
        for players in [1u32, 2] {
            let world = run_level(level, players, 30, true);
            let runs = services_of(&world.outputs, 0x1E2B80);
            assert_eq!(runs.len(), 30, "level {level:#x}, one prelude per frame");
            // the two light ramps run every frame and reach the positional sound starter
            for trace in &runs {
                let lights = trace
                    .iter()
                    .filter(|e| matches!(e, TraceEvent::Prelude(tsfp_gameplay::story_events::prelude_1e2b80::PreludeEffect::Light { .. })))
                    .count();
                assert_eq!(lights, 2);
            }
        }
    }
}

#[test]
fn hooks_without_services_keep_the_t1846_behaviour() {
    let world = run_level(4, 1, 5, false);
    assert!(
        !world
            .outputs
            .iter()
            .any(|o| matches!(o, HookOutput::Service(_)))
    );
    assert!(world.outputs.iter().any(|o| matches!(
        o,
        HookOutput::Story(HookEffect::Call {
            callee: 0x1E2B80,
            ..
        })
    )));
}

#[test]
fn level_0b_runs_the_strip_spawner_after_its_call_sub() {
    let world = run_level(0xB, 1, 10, true);
    let strips = services_of(&world.outputs, 0x1F48A0);
    let subs = world
        .outputs
        .iter()
        .filter(|o| matches!(o, HookOutput::Big(BigEffect::CallSub { address: 0x1F48A0 })))
        .count();
    assert_eq!(strips.len(), subs);
    assert!(!strips.is_empty(), "level 0xB reaches 0x1F48A0");
    // no scene yet: the registry scan finds nothing, the four controller lookups still run
    assert!(strips[0].iter().any(|e| matches!(
        e,
        TraceEvent::Strip {
            service: Service::FindObjectB,
            ..
        }
    )));
}

#[test]
fn real_pak_replay_executes_the_service_paths() {
    let Some(dir) = std::env::var_os("TSFP_SETU_PAK_DIR").map(PathBuf::from) else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut ran = 0;
    let mut service_levels = std::collections::BTreeMap::<(u32, u32, u32), Vec<TraceEvent>>::new();
    let mut unwired = std::collections::BTreeSet::new();
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
            world.players = (0..players).map(|k| player(0x2C + k)).collect();
            world.services = Some(HookServices::new(players));
            for _ in 0..120 {
                engine.frame_level_hook(1, 0, &mut world);
            }
            ran += 1;
            for output in &world.outputs {
                match output {
                    HookOutput::Unwired { address, .. } => {
                        unwired.insert((level, *address));
                    }
                    HookOutput::Service(report) => {
                        service_levels
                            .entry((level, players, report.function))
                            .or_default()
                            .extend(report.trace.iter().cloned());
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(ran >= 80, "{ran} runs");
    assert!(unwired.is_empty(), "{unwired:x?}");
    let mut summary = Vec::new();
    for ((level, players, function), trace) in &service_levels {
        let mut strip = std::collections::BTreeMap::<String, u32>::new();
        let (mut hardware, mut destroy, mut leaves, mut ember, mut prelude) = (0, 0, 0, 0, 0);
        for event in trace {
            match event {
                TraceEvent::Strip { service, .. } => {
                    *strip.entry(format!("{service:?}")).or_default() += 1
                }
                TraceEvent::Hardware(_) => hardware += 1,
                TraceEvent::DestroyCallee { .. } => destroy += 1,
                TraceEvent::SpawnLeaf { .. } => leaves += 1,
                TraceEvent::Ember { .. } => ember += 1,
                TraceEvent::Prelude(_) => prelude += 1,
                TraceEvent::Sweep(_) => {}
            }
        }
        summary.push(format!(
            "level {level:#x} players {players} hook {function:#x}: strip {strip:?} prelude {prelude} hw {hardware} destroy {destroy} leaves {leaves} ember {ember}"
        ));
    }
    eprintln!("{}", summary.join("\n"));
    // Which hooks now execute service paths: the prelude of level 4 / 0x50 and the strip spawner of level 0xB.
    let functions: std::collections::BTreeSet<(u32, u32)> =
        service_levels.keys().map(|(l, _, f)| (*l, *f)).collect();
    for expected in [(4, 0x1E2B80), (0x50, 0x1E2B80), (0xB, 0x1F48A0)] {
        if names
            .iter()
            .any(|n| n.contains(&format!("l_{:02}_", expected.0)))
        {
            assert!(
                functions.contains(&expected),
                "{expected:x?} {functions:x?}"
            );
        }
    }
}

#[test]
fn prelude_ember_slot_runs_the_ember_emitter_and_the_sound_submit() {
    use tsfp_gameplay::story_events::prelude_1e2b80::{
        ENABLE_FLAG, PLAYER_PTR, PreludeEffect, SCENE_PTR, ZONE_MODE, scene,
    };
    // zone 62 maps to mode 1 (ember block enabled)
    let zone = ZONE_MODE.iter().position(|m| *m == 1).unwrap() as u32;
    let mut services = HookServices::new(1);
    let scene_base = 0x5600_0000u32;
    {
        let mut ram = services.ram.borrow_mut();
        ram.set_word(SCENE_PTR, scene_base);
        ram.set_word(ENABLE_FLAG, 1);
        let player = ram.word(PLAYER_PTR);
        ram.set_word(player + 0xF54, zone);
        let slot = scene_base; // slot 0: its fields sit at +0x684.. of the scene
        ram.set_word(slot + 0x6A8, 3);
        ram.set_word(slot + 0x6AC, 0);
        ram.set_float(slot + 0x684, 10.0);
        ram.set_float(slot + 0x690, 9.0);
    }
    let mut particles = 0;
    let mut emitted = Vec::new();
    for _ in 0..40 {
        let report = services.run_prelude();
        particles += report
            .trace
            .iter()
            .filter(|e| matches!(e, TraceEvent::Prelude(PreludeEffect::Particle { .. })))
            .count();
        emitted.extend(report.trace);
    }
    assert_eq!(particles, 3, "life 3, one particle per 11 frames");
    assert_eq!(
        services.ember.pool.dirty, 1,
        "the ported 0xEFBD0 allocated pool entries"
    );
    assert!(
        emitted.iter().any(|e| matches!(
            e,
            TraceEvent::Ember {
                service: "settle",
                ..
            }
        )),
        "the ember start tail reached the recorded engine services"
    );
}
