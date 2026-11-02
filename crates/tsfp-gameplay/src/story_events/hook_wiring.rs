//! Wiring of the ported level hook models into the engine (T1846). `0x228870` calls `[level+0x110]` once per frame;
//! [`dispatch_level_hook`] maps the hook address to its port function, feeds it the inputs a [`HookHost`] reads from
//! the game world and collects the results as [`HookOutput`]s for [`World::hook_output`](super::world::World).
//! Control flow of the hook bodies is MEASURED (see the T1845 documents); what the host supplies is the world state.
//! A hook that needs inputs the host does not provide yields [`HookOutput::Unwired`], never a silent skip.

use super::arcade::proximity_beep;
use super::effect_routing::{self, RoutedEffect};
use super::hooks_audio as audio;
use super::hooks_audio::{AudioEffect, AudioRng, PlayerAudio, RandomOneShot};
use super::hooks_big as big;
use super::hooks_story as story;
use super::hooks_story::{HookEffect, HookView};
use super::level_roles::{self, Level0bEngine, Level06Engine, Level11Engine, PlayerRecord};
use super::levels::{HookSource, audio_alias};
use super::svc_wiring::{HookServices, ServiceReport};

/// One result of a hook run.
#[derive(Clone, Debug, PartialEq)]
pub enum HookOutput {
    Audio(AudioEffect),
    Story(HookEffect),
    Big(big::Effect),
    /// `0x220AF0`: a lightning strike at a point (ray cast and ground effect are the world's).
    Strike(audio::StrikeRequest),
    /// `0x1F6620`: the scroll speed store `[0x4FFE50]` (None = store skipped), then the tail call `0x1F2810`.
    StripSpeed(Option<f32>),
    /// `0x20FBD0`: the level 0x16 failure timer expired (slot status 4, flag `0x10000`, match end).
    TimerExpired,
    /// `0x2284D0`: sound `0x1566` submitted.
    Beep,
    /// T1864: a hook callee (`0x1F48A0` strip spawner, `0x1E2B80` level 4 prelude) ran its ported service path over the
    /// host's [`HookServices`]; the report carries the recorded calls (sound hardware, `0xBABA0` callees, ...).
    Service(ServiceReport),
    /// T1867: an effect spawn the hook recorded directly (`0xEFBD0` or `0x111AE0`), run through the host's
    /// [`HookServices`] (kind table row and ported kind handler). Follows the output that recorded it.
    Effect(RoutedEffect),
    /// The hook (address) needs inputs the host did not provide.
    Unwired {
        address: u32,
        reason: &'static str,
    },
}

/// Persistent hook state, the BSS words the hooks keep between frames (all start at 0 like the image).
#[derive(Clone, Debug, Default)]
pub struct HookState {
    pub max_mix: f32,
    pub mean_mix: f32,
    pub outfit: f32,
    pub state_mix: f32,
    pub level_1d: f32,
    pub weight_mix: f32,
    pub class_loop: f32,
    pub periodic_timer: f32,
    pub one_shot: Option<RandomOneShot>,
    pub thunder_timer: f32,
    pub storm_value: f32,
    pub level_0e: story::Level0eState,
    pub level_0b: big::Level0bGlobals,
    /// `[0x75C154]`, the proximity beeper accumulator.
    pub beep_accumulator: f32,
    /// `[0x737C5C]`, the rate limit stamp of `0xE0080` (see `arcade::resolve_effect_sounds`).
    pub throttle_stamp: i32,
    /// `[0x507DB8]`, the level ambience handle (`None` = still -1 from the level init `0x21AD50`). Written back from
    /// [`HookHost::start_ambience`] when a hook emits `StartAmbience`.
    pub ambience_handle: Option<i32>,
}

/// World facts and per-hook inputs. Everything has a default so a test world only fills what it exercises.
pub trait HookHost: HookView {
    fn state(&mut self) -> &mut HookState;
    fn rng(&mut self) -> &mut dyn AudioRng;
    /// Float frame multiplier `[0x4E794C]`.
    fn multiplier(&self) -> f32 {
        1.0
    }
    /// Integer frame multiplier `[0x4E7944]`.
    fn steps(&self) -> u32 {
        1
    }
    fn level_id(&self) -> u32;
    fn players_audio(&self) -> Vec<PlayerAudio> {
        Vec::new()
    }
    /// Is the one shot handle still playing (`0x45540`)?
    fn sound_playing(&self, _handle: i32) -> bool {
        false
    }
    /// Start a one shot sound for a text id pick (`0x461C0` + `0x45390`), returning the handle.
    fn start_one_shot(&mut self, _pick: u32) -> i32 {
        0
    }
    /// Clock `[0x7DE318]` and the level 0x16 failure timer words (slot, start clock, seconds).
    fn clock(&self) -> i32 {
        0
    }
    fn level_16_timer(&self) -> (i32, i32, f32) {
        (-1, 0, 0.0)
    }
    /// Record word `+0xC` behind `[0x70E164]` (level 0x22 strip speed).
    fn strip_record_word(&self) -> Option<i32> {
        None
    }
    fn level1_inputs(&self) -> story::Level1Inputs {
        story::Level1Inputs::default()
    }
    fn level2_inputs(&self) -> Option<story::Level2Inputs> {
        None
    }
    fn level4_inputs(&self) -> story::Level4Inputs {
        story::Level4Inputs::default()
    }
    fn level5_inputs(&self) -> story::Level5Inputs {
        story::Level5Inputs::default()
    }
    fn level0e_inputs(&self) -> story::Level0eInputs {
        story::Level0eInputs::default()
    }
    /// Level 0x12: model and `+8` value of the player's linked object.
    fn level12_args(&self) -> (Option<u32>, f32) {
        (None, 0.0)
    }
    /// Level 0x1A: PTCZ state, ambience handle, mode word.
    fn level1a_args(&self) -> (u32, i32, u32) {
        (0, -1, 0)
    }
    fn level06_view(&self) -> Option<big::Level06View> {
        None
    }
    fn level0b_view(&self) -> Option<big::Level0bView> {
        None
    }
    fn level11_view(&self) -> Option<big::Level11View> {
        None
    }
    /// The player table (`[0x7B0C48]`, stride 0x1584, count `[0x790950]`) reduced to the fields levels 6, 0xB and 0x11 read.
    fn player_records(&self) -> Option<Vec<PlayerRecord>> {
        None
    }
    /// Engine facts of the level 6 view that neither [`HookView`] nor the player table carry (T1846 batch 8,
    /// `level_roles`).
    fn level06_engine(&self) -> Option<Level06Engine> {
        None
    }
    fn level0b_engine(&self) -> Option<Level0bEngine> {
        None
    }
    fn level11_engine(&self) -> Option<Level11Engine> {
        None
    }
    /// `0x25F370("OMBH", 0)`: how many objects tagged "OMBH" exist (levels 2 and 0x40).
    fn ombh_count(&self) -> Option<u32> {
        None
    }
    /// `0x45820` for the level ambience: returns the handle stored to `[0x507DB8]`.
    fn start_ambience(&mut self, _sound_id: u32) -> i32 {
        0
    }
    fn key_resolver(&self) -> &dyn big::KeyResolver;
    /// T1864: the shared guest memory and service stack. `None` keeps the callees `0x1F48A0` and `0x1E2B80` as plain
    /// recorded calls (the T1846 behaviour); `Some` executes them through the ported T1856 services.
    fn services(&mut self) -> Option<&mut HookServices> {
        None
    }
    /// Mode 5 beeper inputs: other net players, local players (positions) and the `[0x4E794C]` step.
    fn beep_positions(&self) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
        (Vec::new(), Vec::new())
    }
}

fn audio_out(effects: Vec<AudioEffect>) -> Vec<HookOutput> {
    effects.into_iter().map(HookOutput::Audio).collect()
}

fn unwired(address: u32, reason: &'static str) -> Vec<HookOutput> {
    vec![HookOutput::Unwired { address, reason }]
}

fn story_out(host: &mut dyn HookHost, effects: Vec<HookEffect>) -> Vec<HookOutput> {
    let mut out = Vec::new();
    for effect in effects {
        // `0x1D7A70` (level 1 storm) has a port that needs only audio state: run it here.
        if let HookEffect::Call {
            callee: 0x1D7A70, ..
        } = effect
        {
            let players = host.players_audio();
            let multiplier = host.multiplier();
            let mut state = std::mem::take(host.state());
            let effects = audio::storm_hook(
                &players,
                &mut state.storm_value,
                &mut state.thunder_timer,
                multiplier,
                host.rng(),
            );
            *host.state() = state;
            out.extend(audio_out(effects));
            continue;
        }
        if let HookEffect::Call {
            callee: 0x1E2B80, ..
        } = effect
        {
            out.push(HookOutput::Story(effect));
            if let Some(services) = host.services() {
                out.push(HookOutput::Service(services.run_prelude()));
            }
            continue;
        }
        if let HookEffect::StartAmbience { sound_id } = effect {
            let handle = host.start_ambience(sound_id);
            host.state().ambience_handle = Some(handle);
        }
        out.push(HookOutput::Story(effect));
    }
    out
}

/// Level 2 / 0x40 inputs: the host's own, else built by `level_roles::level2_inputs` from the level id, the game
/// flags, the persistent ambience handle and the OMBH count (the only fact only the engine knows).
fn level2_inputs_for(host: &mut dyn HookHost) -> Option<story::Level2Inputs> {
    if let Some(inputs) = host.level2_inputs() {
        return Some(inputs);
    }
    let count = host.ombh_count()?;
    let handle = host.state().ambience_handle.unwrap_or(-1);
    Some(level_roles::level2_inputs(
        &*host,
        host.level_id(),
        handle,
        count,
    ))
}

/// Run the hook at `address` (a table value or thunk target) for one frame. With host services (T1867) the effect
/// spawns the hook recorded directly are also run through the kind table and handlers, as [`HookOutput::Effect`] after
/// the output that recorded them.
pub fn dispatch_level_hook(address: u32, host: &mut dyn HookHost) -> Vec<HookOutput> {
    let outputs = dispatch_recorded(address, host);
    let Some(services) = host.services() else {
        return outputs;
    };
    let mut routed = Vec::with_capacity(outputs.len());
    for output in outputs {
        let request = match &output {
            HookOutput::Strike(strike) => Some(("0x220AF0", effect_routing::from_strike(strike))),
            _ => None,
        };
        routed.push(output);
        if let Some((origin, request)) = request {
            routed.push(HookOutput::Effect(effect_routing::route(
                services, origin, request,
            )));
        }
    }
    routed
}

fn dispatch_recorded(address: u32, host: &mut dyn HookHost) -> Vec<HookOutput> {
    let address = audio_alias(address);
    let players = host.players_audio();
    let multiplier = host.multiplier();
    match address {
        0x389B0 => Vec::new(),
        0x222C90 => audio_out(audio::max_mix_hook(
            &players,
            &mut host.state().max_mix,
            multiplier,
        )),
        0x225840 => audio_out(audio::mean_crossfade_hook(
            &players,
            &mut host.state().mean_mix,
        )),
        0x220F90 => audio_out(audio::outfit_ambience_hook(
            &players,
            &mut host.state().outfit,
            multiplier,
        )),
        0x207450 => audio_out(audio::state_crossfade_hook(
            &players,
            &mut host.state().state_mix,
            multiplier,
        )),
        0x2084A0 => audio_out(audio::level_1d_crossfade_hook(
            &players,
            &mut host.state().level_1d,
            multiplier,
        )),
        0x2264F0 => audio_out(audio::weight_crossfade_hook(
            &players,
            &mut host.state().weight_mix,
            multiplier,
        )),
        0x225C50 => audio_out(audio::class_list_hook(
            &players,
            &audio::LEVEL_3B_LIST,
            1.0,
            &mut host.state().class_loop,
        )),
        0x222B60 => audio_out(audio::periodic_sound_hook(
            &mut host.state().periodic_timer,
            multiplier,
        )),
        0x1D7920 => {
            let mut timer = host.state().thunder_timer;
            let effects = audio::level_3d_hook(&mut timer, multiplier, host.rng());
            host.state().thunder_timer = timer;
            audio_out(effects)
        }
        0x21AF90 => {
            let mut one_shot = host.state().one_shot.unwrap_or(RandomOneShot {
                handle: -1,
                last_pick: 0,
                timer: 0.0,
            });
            let playing = host.sound_playing(one_shot.handle);
            // The start closure cannot borrow the host (the rng is borrowed), so the handle is filled in afterwards.
            let pick = audio::random_one_shot_hook(
                &mut one_shot,
                |_| playing,
                multiplier,
                host.rng(),
                &mut |_| -1,
            );
            if let Some(pick) = pick {
                one_shot.handle = host.start_one_shot(pick);
            }
            host.state().one_shot = Some(one_shot);
            Vec::new()
        }
        0x220AF0 => {
            let roll_source = host.rng().next_u32();
            if !audio::lightning_roll(roll_source, host.steps()) {
                return Vec::new();
            }
            let (angle, radius) = (host.rng().unit(), host.rng().unit());
            vec![HookOutput::Strike(audio::lightning_point(angle, radius))]
        }
        0x20FBD0 => {
            let (slot, start, seconds) = host.level_16_timer();
            let mut out = vec![HookOutput::Audio(audio::level_16_base_loop())];
            if audio::level_16_timeout(slot, start, seconds, host.clock()) {
                out.push(HookOutput::TimerExpired);
            }
            out
        }
        0x1F6620 => vec![HookOutput::StripSpeed(big::strip_scroll_speed(
            host.level_id(),
            host.strip_record_word(),
        ))],
        0x1DDC90 => {
            let inputs = host.level1_inputs();
            let effects = story::level1_hook(&*host, &inputs);
            story_out(host, effects)
        }
        0x21AD00 => match level2_inputs_for(host) {
            Some(inputs) => {
                let effects = story::level2_hook(&*host, &inputs);
                story_out(host, effects)
            }
            None => unwired(
                address,
                "level 2 / 0x40: the OMBH object count (0x25F370) is not provided",
            ),
        },
        0x1E40D0 => {
            let inputs = host.level4_inputs();
            let effects = story::level4_hook(&*host, &inputs);
            story_out(host, effects)
        }
        0x220600 => {
            let inputs = host.level5_inputs();
            let effects = story::level5_hook(&*host, &inputs);
            story_out(host, effects)
        }
        0x224160 => {
            let effects = story::level9_hook(&*host);
            story_out(host, effects)
        }
        0x205BC0 => {
            let inputs = host.level0e_inputs();
            let mut state = host.state().level_0e;
            let effects = story::level0e_hook(&*host, &mut state, &inputs);
            host.state().level_0e = state;
            story_out(host, effects)
        }
        0x1FA9C0 => {
            let (model, value) = host.level12_args();
            let effects = story::level12_hook(&*host, model, value);
            story_out(host, effects)
        }
        0x221C70 => {
            let (ptcz, handle, mode) = host.level1a_args();
            let effects = story::level1a_hook(&*host, ptcz, handle, mode);
            story_out(host, effects)
        }
        0x20B630 => {
            let view = host.level06_view().or_else(|| {
                let (records, engine) = (host.player_records()?, host.level06_engine()?);
                Some(level_roles::level06_view(&*host, &records, &engine))
            });
            match view {
                Some(view) => big::level06_hook(&view)
                    .into_iter()
                    .map(HookOutput::Big)
                    .collect(),
                None => unwired(
                    address,
                    "level 6: player table or engine facts (slot status, CDWD, CPTB) not provided",
                ),
            }
        }
        0x1F5A10 => match host.level0b_view().or_else(|| {
            let (records, engine) = (host.player_records()?, host.level0b_engine()?);
            Some(level_roles::level0b_view(&*host, &records, &engine))
        }) {
            Some(view) => {
                let mut globals = host.state().level_0b;
                let effects = big::level0b_hook(&view, &mut globals, host.key_resolver());
                host.state().level_0b = globals;
                let strip = effects
                    .iter()
                    .any(|e| matches!(e, big::Effect::CallSub { address: 0x1F48A0 }));
                let mut out: Vec<HookOutput> = effects.into_iter().map(HookOutput::Big).collect();
                if strip {
                    if let Some(services) = host.services() {
                        out.push(HookOutput::Service(services.run_strip_spawner()));
                    }
                }
                out
            }
            None => unwired(
                address,
                "level 0xB: player table or engine facts (entry key, GAS0, camera) not provided",
            ),
        },
        0x213F50 => match host.level11_view().or_else(|| {
            let (records, engine) = (host.player_records()?, host.level11_engine()?);
            Some(level_roles::level11_view(&*host, &records, &engine))
        }) {
            Some(view) => big::level11_hook(&view, host.key_resolver())
                .into_iter()
                .map(HookOutput::Big)
                .collect(),
            None => unwired(
                address,
                "level 0x11: player table or engine facts (tick words, slot 1) not provided",
            ),
        },
        _ => unwired(address, "hook has no port"),
    }
}

/// Mode 5 step of `0x228870` (`0x2284D0`, between the slot poll and the level hook): run the proximity beeper.
pub fn run_mode5_beeper(host: &mut dyn HookHost, step: f32) -> Option<HookOutput> {
    let (others, locals) = host.beep_positions();
    let (next, beep) = proximity_beep(&others, &locals, host.state().beep_accumulator, step);
    host.state().beep_accumulator = next;
    beep.then_some(HookOutput::Beep)
}

/// Does the table hold a hook for this source (convenience for callers deciding whether to build a host)?
pub fn has_hook(source: HookSource) -> bool {
    matches!(source, HookSource::Static(_))
}
