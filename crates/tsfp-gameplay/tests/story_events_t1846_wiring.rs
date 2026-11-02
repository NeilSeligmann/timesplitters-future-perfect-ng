//! T1846: the level hook models run through `World::run_level_hook` / `Engine::frame_level_hook` (mode 5 beeper
//! included). The headless world is FABRICATED state against MEASURED hook control flow (T1845 documents); the real
//! replay reads level ids from the SETU pak names (TSFP_SETU_PAK_DIR, skipped without it, nothing committed).

use std::path::PathBuf;

use tsfp_gameplay::story_events::hook_wiring::{HookHost, HookOutput, HookState};
use tsfp_gameplay::story_events::hooks_audio::{AudioEffect, AudioRng, PlayerAudio};
use tsfp_gameplay::story_events::hooks_big::KeyResolver;
use tsfp_gameplay::story_events::hooks_story::{HookEffect, HookView, NpcView};
use tsfp_gameplay::story_events::level_roles::{
    Level0bEngine, Level06Engine, Level11Engine, PlayerRecord,
};
use tsfp_gameplay::story_events::levels::HookDispatch;
use tsfp_gameplay::story_events::record::Link;
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

#[test]
fn max_mix_hook_ramps_through_the_engine() {
    // Level 0x27 (hook 0x222D90, a thunk of 0x222C90): combat state 0x2C gives intensity 0.2.
    let mut engine = engine(10, 0x27);
    let mut world = Headless::new(0x27);
    world.players = vec![player(0x2C)];
    for _ in 0..2 {
        assert_eq!(
            engine.frame_level_hook(1, 0, &mut world),
            HookDispatch::AfterObjective
        );
    }
    let volumes: Vec<f32> = world
        .outputs
        .iter()
        .filter_map(|o| match o {
            HookOutput::Audio(AudioEffect::SetLoop {
                sound_id: 0x3B,
                volume,
                ..
            }) => Some(*volume),
            _ => None,
        })
        .collect();
    assert_eq!(volumes.len(), 2);
    let step = (1.0_f32 / 60.0) * 0.444_444_45;
    assert!(
        (volumes[0] - step).abs() < 1e-7 && (volumes[1] - 2.0 * step).abs() < 1e-7,
        "{volumes:?}"
    );
}

#[test]
fn skipped_frames_run_nothing_and_null_hooks_stay_silent() {
    let mut engine_a = engine(10, 0x27);
    let mut world = Headless::new(0x27);
    assert_eq!(
        engine_a.frame_level_hook(0, 0, &mut world),
        HookDispatch::Skip
    );
    assert!(world.outputs.is_empty());
    // Level 0x64 has a null hook (T1845).
    let mut engine_b = engine(10, 0x64);
    engine_b.frame_level_hook(1, 0, &mut world);
    assert!(world.outputs.is_empty());
}

#[test]
fn state_persists_between_frames_and_periodic_timer_fires() {
    // Level 0x2E (0x222B60): timer starts 0, so the first frame fires sound 0x156F and adds 900.
    let mut engine = engine(10, 0x2E);
    let mut world = Headless::new(0x2E);
    engine.frame_level_hook(1, 0, &mut world);
    assert_eq!(
        world.outputs,
        [HookOutput::Audio(AudioEffect::PlayOnce {
            sound_id: 0x156F
        })]
    );
    engine.frame_level_hook(1, 0, &mut world);
    assert_eq!(world.outputs.len(), 1);
    assert!(world.state.periodic_timer > 899.0);
}

#[test]
fn lightning_level_emits_a_strike_inside_the_disc() {
    let mut engine = engine(10, 0x30);
    let mut world = Headless::new(0x30);
    world.steps = 60;
    let mut strikes = 0;
    for _ in 0..600 {
        engine.frame_level_hook(60, 0, &mut world);
    }
    for output in &world.outputs {
        if let HookOutput::Strike(strike) = output {
            strikes += 1;
            let (dx, dz) = (strike.point[0] - 36.78, strike.point[2] + 30.0);
            assert!((dx * dx + dz * dz).sqrt() <= 70.5);
        }
    }
    assert_eq!(
        strikes, 600,
        "steps 60 makes every roll pass (rand % 60 < 60)"
    );
}

#[test]
fn strip_speed_and_unwired_markers() {
    let mut engine = engine(10, 0x22);
    let mut world = Headless::new(0x22);
    engine.frame_level_hook(1, 0, &mut world);
    assert_eq!(world.outputs, [HookOutput::StripSpeed(None)]);
    // Level 2 needs the OMBH count the host does not give: reported, not skipped.
    let mut engine = engine_for(2);
    let mut world = Headless::new(2);
    engine.frame_level_hook(1, 0, &mut world);
    assert!(matches!(
        world.outputs[0],
        HookOutput::Unwired {
            address: 0x21AD00,
            ..
        }
    ));
}

#[test]
fn engine_facts_wire_levels_2_6_0b_11_and_40() {
    for level in [2u32, 0x40, 6, 0xB, 0x11] {
        let mut engine = engine_for(level);
        let mut world = Headless::new(level);
        world.engine_facts = true;
        world.players = vec![player(0x2C), player(0x30)];
        for _ in 0..3 {
            engine.frame_level_hook(1, 0, &mut world);
        }
        assert!(
            !world
                .outputs
                .iter()
                .any(|o| matches!(o, HookOutput::Unwired { .. })),
            "level {level:#x} still unwired: {:?}",
            world
                .outputs
                .iter()
                .find(|o| matches!(o, HookOutput::Unwired { .. }))
        );
    }
}

#[test]
fn ambience_handle_comes_back_from_the_host_and_stops_the_restart() {
    let mut engine = engine_for(2);
    let mut world = Headless::new(2);
    world.engine_facts = true;
    for _ in 0..4 {
        engine.frame_level_hook(1, 0, &mut world);
    }
    let starts = world
        .outputs
        .iter()
        .filter(|o| matches!(o, HookOutput::Story(HookEffect::StartAmbience { .. })))
        .count();
    assert_eq!(starts, 1, "the handle is stored after the first start");
    assert_eq!(world.ambience_started, 1);
    assert_eq!(world.state.ambience_handle, Some(7));
}

#[test]
fn ported_callees_are_named_not_unsupported() {
    use tsfp_gameplay::story_events::hooks_big::{SubStatus, call_sub_status};
    use tsfp_gameplay::story_events::hooks_story::callee_port;
    assert!(
        callee_port(0x1E2B80).is_some(),
        "the level 4 prelude is a ported callee"
    );
    assert!(matches!(call_sub_status(0x1F2810), SubStatus::Ported(_)));
    assert!(matches!(call_sub_status(0x1F48A0), SubStatus::Ported(_)));
}

fn engine_for(level: u32) -> Engine {
    engine(10, level)
}

#[test]
fn mode5_beeper_runs_before_the_hook_and_keeps_its_accumulator() {
    let mut engine = engine(5, 0x27);
    let mut world = Headless::new(0x27);
    world.others = vec![[0.0, 0.0, 0.0]];
    world.locals = vec![[1.0, 0.0, 0.0]];
    engine.frame_level_hook(1, 0, &mut world);
    // Nearest distance squared 1 is clamped to 4: rate 1.0, accumulator -1/60 + 0.1 and a beep.
    assert_eq!(world.outputs[0], HookOutput::Beep);
    assert!((world.state.beep_accumulator - (0.1 - 1.0 / 60.0)).abs() < 1e-6);
    for _ in 0..4 {
        engine.frame_level_hook(1, 0, &mut world);
    }
    assert_eq!(
        world
            .outputs
            .iter()
            .filter(|o| **o == HookOutput::Beep)
            .count(),
        1,
        "0.0833 lasts five frames"
    );
    // Other modes never beep.
    let mut engine = engine_for(0x27);
    let mut world = Headless::new(0x27);
    world.others = vec![[0.0; 3]];
    world.locals = vec![[1.0, 0.0, 0.0]];
    engine.frame_level_hook(1, 0, &mut world);
    assert!(!world.outputs.contains(&HookOutput::Beep));
}

#[test]
fn hook_only_plan_skips_the_beeper() {
    let mut engine = Engine::new(
        &[],
        BuildOptions {
            context: ModeContext {
                mode_id: 5,
                level_id: 0x27,
                game_flags: 0x40,
                ..ModeContext::default()
            },
        },
    );
    let mut world = Headless::new(0x27);
    world.others = vec![[0.0; 3]];
    world.locals = vec![[1.0, 0.0, 0.0]];
    assert_eq!(
        engine.frame_level_hook(1, 0x40, &mut world),
        HookDispatch::HookOnly
    );
    assert!(!world.outputs.contains(&HookOutput::Beep));
}

#[test]
fn real_pak_levels_all_run_their_hooks() {
    let Some(dir) = std::env::var_os("TSFP_SETU_PAK_DIR").map(PathBuf::from) else {
        eprintln!("skip: TSFP_SETU_PAK_DIR not set");
        return;
    };
    let mut ran = 0;
    let mut unwired = std::collections::BTreeSet::new();
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
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
        let mut engine = engine(mode_id, level);
        let mut world = Headless::new(level);
        world.players = vec![player(0x2C), player(0x30)];
        world.engine_facts = true;
        for _ in 0..120 {
            engine.frame_level_hook(1, 0, &mut world);
        }
        ran += 1;
        for output in &world.outputs {
            if let HookOutput::Unwired { address, .. } = output {
                unwired.insert((level, *address));
            }
        }
    }
    assert!(ran >= 40, "{ran} paks");
    eprintln!("unwired (level, hook): {unwired:x?}");
    // With the engine facts supplied (FABRICATED) every hook of every pak level runs; none stays unwired.
    assert!(unwired.is_empty(), "{unwired:x?}");
}
