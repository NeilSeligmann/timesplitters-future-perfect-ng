//! T1856: effect request submit `0x45820` and its closure, the starter `0x468A0` and the object disturbance
//! `0xE94C0`/`0xE9380`, against emulated-original golden vectors (MEASURED, Unicorn level), plus semantic tests.
mod common;
use common::{Case, parse};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use tsfp_gameplay::story_events::svc_effect_submit::*;
use tsfp_gameplay::story_events::sweep_dynamic_shape::ImpactServices;

const RETS: u32 = 0x179_0000;
const PCOUNT: u32 = 0x179_0400;
const PSLOT: u32 = 0x179_0410;
const REQ: u32 = 0x171_0000;
const SCR: u32 = 0x172_0000;
const POS_A: u32 = 0x179_1000;
const POS_B: u32 = 0x179_1010;
const TOKEN_ADDR: u32 = 0x179_0700;
const ARGS: u32 = 0x179_0800;
const SPARE: u32 = 0x17A_0000;
const OBJ_BASE: u32 = 0x175_0000;
const SUBMIT: u32 = 0x4_5820;

type Log = Vec<(u32, u32, Vec<u32>)>;

fn world(case: &Case) -> SparseSvcMemory {
    let mut memory = SparseSvcMemory::default();
    for (&address, &value) in &case.ins {
        memory.w32(address, value);
    }
    memory
}

#[derive(Default)]
struct State {
    rets: Vec<u32>,
    next: usize,
    players: u32,
    slots: Vec<u32>,
    log: Log,
}

impl State {
    fn script(&mut self) -> u32 {
        let value = self.rets.get(self.next).copied().unwrap_or(0);
        self.next += 1;
        value
    }
}

/// Replays the scripted answers of the logging stubs and records the calls (shared so a submit callback can use it).
#[derive(Clone)]
struct Replay {
    state: Rc<RefCell<State>>,
}

impl Replay {
    fn new(case: &Case) -> Self {
        let state = State {
            rets: (0..32).map(|i| case.input(RETS + 4 * i)).collect(),
            next: 0,
            players: case.input(PCOUNT),
            slots: (0..8).map(|i| case.input(PSLOT + 4 * i)).collect(),
            log: Vec::new(),
        };
        Replay {
            state: Rc::new(RefCell::new(state)),
        }
    }
    fn empty() -> Self {
        Replay {
            state: Rc::new(RefCell::new(State::default())),
        }
    }
    fn log(&self) -> Log {
        self.state.borrow().log.clone()
    }
    fn calls(&self, callee: u32) -> usize {
        self.state
            .borrow()
            .log
            .iter()
            .filter(|c| c.0 == callee)
            .count()
    }
}

impl SoundHw for Replay {
    fn call(&mut self, call: HwCall) -> u32 {
        let mut state = self.state.borrow_mut();
        state.log.push((call.callee, call.reg, call.args));
        state.script()
    }
    fn player_count(&mut self) -> i32 {
        let mut state = self.state.borrow_mut();
        state.log.push((0x1C_DAE0, 0, vec![]));
        state.players as i32
    }
    fn player_slot(&mut self, index: i32) -> i32 {
        let mut state = self.state.borrow_mut();
        state.log.push((0x35_6800, 0, vec![index as u32]));
        state.slots.get(index as usize).copied().unwrap_or(0) as i32
    }
}

/// Which register carries an argument for a callee (thiscall `ecx`, `eax` for the position setter).
fn register_of(callee: u32, regs: &[u32; 5]) -> u32 {
    match callee {
        0x2_6EC0 => regs[0],
        0x2_99C0 | 0x2_9E50 | 0x2_9E40 | 0x2_9DD0 | 0x2_9E60 | 0x2_9EA0 | 0x2_9CC0 => regs[1],
        _ => 0,
    }
}

/// The record pointer of the stubbed submit is a stack address in the original: mask it on both sides.
fn masked(mut log: Log) -> Log {
    for call in log.iter_mut() {
        if call.0 == SUBMIT {
            call.2[0] = 0;
        }
    }
    log
}

fn assert_calls(case: &Case, replay: &Replay, label: &str) {
    let expected: Log = case
        .calls
        .iter()
        .map(|c| (c.callee, register_of(c.callee, &c.regs), c.args.clone()))
        .collect();
    assert_eq!(
        masked(replay.log()),
        masked(expected),
        "{label} hardware calls"
    );
}

fn assert_outputs(case: &Case, memory: &SparseSvcMemory, label: &str) {
    for (&address, &value) in &case.outs {
        assert_eq!(memory.r32(address), value, "{label} memory {address:#x}");
    }
}

fn load(text: &'static str, minimum: usize) -> Vec<Case> {
    let cases = parse(text);
    assert!(cases.len() >= minimum, "golden has {} cases", cases.len());
    cases
}

/// A submit callback standing for the stubbed `0x45820`: logs the record words, answers from the script.
type SubmitBox = Box<dyn FnMut(&mut dyn SvcMemory, &mut dyn SoundHw, u32) -> i32>;

fn stub_submit(replay: &Replay) -> SubmitBox {
    let shared = replay.clone();
    Box::new(move |memory, _hw, record| {
        let mut args = vec![record];
        args.extend((0..12).map(|i| memory.r32(record + 4 * i)));
        let mut state = shared.state.borrow_mut();
        state.log.push((SUBMIT, 0, args));
        state.script() as i32
    })
}

#[test]
fn submit_with_stubbed_voice_start() {
    let cases = load(include_str!("data/t1856/effect_submit_45820.golden"), 60);
    let mut seen = BTreeMap::<&str, usize>::new();
    for (n, case) in cases.iter().enumerate() {
        let mut memory = world(case);
        let replay = Replay::new(case);
        let shared = replay.clone();
        let mut start = |m: &mut dyn SvcMemory, request: u32, token: u32| {
            let mut args = vec![request, token];
            args.extend((0..12).map(|i| m.r32(request + 4 * i)));
            let mut state = shared.state.borrow_mut();
            state.log.push((0x2_8240, 0, args));
            state.script() as i32
        };
        let result = submit_effect_with(&mut memory, REQ, &mut start);
        assert_eq!(result as u32, case.eax, "case {n} result");
        assert_calls(case, &replay, &format!("case {n}"));
        assert_outputs(case, &memory, &format!("case {n}"));
        let class = match result {
            -1 if replay.log().is_empty() => "invalid or unknown",
            -1 => "start failed",
            -2 => "ring full",
            -3 => "duplicate",
            _ if replay.log().is_empty() => "queued",
            _ => "started",
        };
        *seen.entry(class).or_default() += 1;
    }
    for class in [
        "invalid or unknown",
        "start failed",
        "duplicate",
        "queued",
        "started",
    ] {
        assert!(
            seen.contains_key(class),
            "branch {class} not covered: {seen:?}"
        );
    }
}

#[test]
fn closure_matches_original() {
    let cases = load(include_str!("data/t1856/effect_submit_closure.golden"), 60);
    let mut seen = BTreeMap::<u32, usize>::new();
    for (n, case) in cases.iter().enumerate() {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        let result = submit_effect(&mut memory, &mut replay, REQ);
        assert_eq!(result as u32, case.eax, "case {n} result");
        assert_calls(case, &replay, &format!("case {n}"));
        assert_outputs(case, &memory, &format!("case {n}"));
        for call in replay.log() {
            *seen.entry(call.0).or_default() += 1;
        }
    }
    for callee in [
        hw::VOICE_BIND,
        hw::VOICE_PARAMS,
        hw::VOICE_SET,
        hw::VOICE_3D_MODE,
        hw::VOICE_3D_CURVE,
        hw::VOICE_POSITION,
        hw::VOICE_VOLUME,
        hw::VOICE_START,
        hw::STREAM_OPEN,
        hw::STREAM_SETUP,
        hw::STREAM_SET_PRIORITY,
        hw::STREAM_SET_POSITIONAL,
        hw::STREAM_SET_POSITION,
        hw::STREAM_COMMIT,
        hw::STREAM_PLAY,
        hw::STREAM_BUSY,
    ] {
        assert!(
            seen.contains_key(&callee),
            "hardware call {callee:#x} never exercised: {seen:?}"
        );
    }
}

#[test]
fn voice_start_matches_original() {
    let cases = load(include_str!("data/t1856/effect_submit_28240.golden"), 80);
    let (mut on_voice, mut on_stream, mut failed) = (0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        let result = start_sound(&mut memory, &mut replay, REQ, case.input(TOKEN_ADDR));
        assert_eq!(result as u32, case.eax, "case {n} result");
        assert_calls(case, &replay, &format!("case {n}"));
        assert_outputs(case, &memory, &format!("case {n}"));
        if result != 0 {
            failed += 1;
        } else if replay.calls(hw::STREAM_SETUP) > 0 {
            on_stream += 1;
        } else {
            on_voice += 1;
        }
    }
    assert!(
        on_voice > 5 && on_stream > 5 && failed > 2,
        "{on_voice} {on_stream} {failed}"
    );
}

#[test]
fn small_pure_callees_match_original() {
    let known_cases = load(include_str!("data/t1856/effect_submit_27f80.golden"), 50);
    let mut known_counts = [0; 2];
    for (n, case) in known_cases.iter().enumerate() {
        let memory = world(case);
        let known = sound_known(&memory, case.input(ARGS));
        assert_eq!(u32::from(known), case.eax, "27f80 case {n}");
        known_counts[usize::from(known)] += 1;
    }
    assert!(known_counts.iter().all(|&c| c > 5), "{known_counts:?}");
    for (n, case) in load(include_str!("data/t1856/effect_submit_45710.golden"), 50)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        assert_eq!(allocate_token(&mut memory, REQ), case.eax, "45710 case {n}");
        assert_outputs(case, &memory, "45710");
    }
    let (mut passed, mut blocked) = (0, 0);
    for (n, case) in load(include_str!("data/t1856/effect_submit_45190.golden"), 60)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let result = dedup_request(&mut memory, REQ);
        assert_eq!(u32::from(result), case.eax, "45190 case {n}");
        assert_outputs(case, &memory, "45190");
        if result {
            passed += 1;
        } else {
            blocked += 1;
        }
    }
    assert!(passed > 10 && blocked > 5, "{passed} {blocked}");
    let (mut found, mut full) = (0, 0);
    for (n, case) in load(include_str!("data/t1856/effect_submit_453d0.golden"), 50)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let result = free_queue_slot(&mut memory);
        assert_eq!(result as u32, case.eax, "453d0 case {n}");
        assert_outputs(case, &memory, "453d0");
        if result >= 0 {
            found += 1;
        } else {
            full += 1;
        }
    }
    assert!(found > 10 && full > 2, "{found} {full}");
    let mut queued = 0;
    for (n, case) in load(include_str!("data/t1856/effect_submit_45410.golden"), 50)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let result = queue_request(&mut memory, REQ, case.input(ARGS));
        assert_eq!(result as u32, case.eax, "45410 case {n}");
        assert_outputs(case, &memory, "45410");
        if result >= 0 {
            queued += 1;
        }
    }
    assert!(queued > 10);
    let (mut taken, mut none) = (0, 0);
    for (n, case) in load(include_str!("data/t1856/effect_submit_26b00.golden"), 40)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let handle = take_voice_handle(&mut memory, case.input(ARGS) != 0);
        assert_eq!(handle, case.eax, "26b00 case {n}");
        assert_outputs(case, &memory, "26b00");
        if handle == 0 {
            none += 1;
        } else {
            taken += 1;
        }
    }
    assert!(taken > 5 && none > 1, "{taken} {none}");
}

#[test]
fn listener_range_and_nearest() {
    let (mut near, mut far) = (0, 0);
    for (n, case) in load(include_str!("data/t1856/effect_submit_45310.golden"), 60)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        let radius = f32::from_bits(case.input(ARGS));
        let result = within_listener_range(&mut memory, &mut replay, POS_A, radius, SPARE);
        assert_eq!(u32::from(result), case.eax, "45310 case {n}");
        assert_calls(case, &replay, &format!("45310 case {n}"));
        if result {
            near += 1;
        } else {
            far += 1;
        }
    }
    assert!(near > 5 && far > 5, "{near} {far}");
    for (n, case) in load(include_str!("data/t1856/effect_submit_45200.golden"), 50)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        nearest_listener(&mut memory, &mut replay, POS_A, POS_B);
        assert_calls(case, &replay, &format!("45200 case {n}"));
        assert_outputs(case, &memory, &format!("45200 case {n}"));
    }
}

#[test]
fn cancel_and_park() {
    for (n, case) in load(include_str!("data/t1856/effect_submit_45460.golden"), 50)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        cancel_request(&mut memory, &mut replay, case.input(ARGS));
        assert_calls(case, &replay, &format!("45460 case {n}"));
        assert_outputs(case, &memory, &format!("45460 case {n}"));
    }
    let (mut parked, mut cancelled) = (0, 0);
    for (n, case) in load(include_str!("data/t1856/effect_submit_46bc0.golden"), 60)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        park_handle(
            &mut memory,
            &mut replay,
            case.input(ARGS),
            SCR,
            case.input(ARGS + 4),
        );
        assert_calls(case, &replay, &format!("46bc0 case {n}"));
        assert_outputs(case, &memory, &format!("46bc0 case {n}"));
        if replay.calls(hw::VOICE_START) > 0 {
            parked += 1;
        } else {
            cancelled += 1;
        }
    }
    assert!(parked > 3 && cancelled > 3, "{parked} {cancelled}");
}

#[test]
fn held_sound_matches_original() {
    let (mut retrigger, mut fresh, mut refused) = (0, 0, 0);
    for (n, case) in load(include_str!("data/t1856/effect_submit_46d80.golden"), 70)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        let sound = HeldSound {
            id: case.input(ARGS),
            volume: case.input(ARGS + 4),
            start_value: case.input(ARGS + 8),
            gain: case.input(ARGS + 12),
            position: case.input(ARGS + 16),
            range_flag: case.input(ARGS + 20),
        };
        let mut submit = stub_submit(&replay);
        let result = start_held_sound_with(&mut memory, &mut replay, sound, SPARE, &mut submit);
        assert_eq!(result as u32, case.eax, "46d80 case {n} result");
        assert_calls(case, &replay, &format!("46d80 case {n}"));
        assert_outputs(case, &memory, &format!("46d80 case {n}"));
        if replay.calls(SUBMIT) > 0 {
            fresh += 1;
        } else if (sound.id as i32) < 0 {
            refused += 1;
        } else {
            retrigger += 1;
        }
    }
    assert!(
        retrigger > 5 && fresh > 5 && refused > 1,
        "{retrigger} {fresh} {refused}"
    );
}

#[test]
fn starter_matches_original() {
    let mut outcomes = BTreeMap::<&str, usize>::new();
    for (n, case) in load(include_str!("data/t1856/effect_submit_468a0.golden"), 100)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        let args = StarterArgs {
            handle_ptr: case.input(ARGS),
            position: case.input(ARGS + 4),
            id: case.input(ARGS + 8) as i32,
            gain: case.input(ARGS + 12),
            radius: case.input(ARGS + 16),
            start_value: case.input(ARGS + 20),
            flags: case.input(ARGS + 24),
        };
        let before = memory.r32(args.handle_ptr);
        let mut submit = stub_submit(&replay);
        let result = sound_starter_with(&mut memory, &mut replay, args, SPARE, &mut submit);
        assert_eq!(result, case.eax, "468a0 case {n} result");
        assert_calls(case, &replay, &format!("468a0 case {n}"));
        assert_outputs(case, &memory, &format!("468a0 case {n}"));
        let class = if args.id < 0 {
            "negative id"
        } else if result == 0 && (before as i32) < 0 {
            "stopped nothing"
        } else if result == 0 && replay.calls(hw::VOICE_START) > 0 {
            "parked"
        } else if result == 0 {
            "cancelled"
        } else if replay.calls(SUBMIT) > 0 {
            "started"
        } else {
            "updated"
        };
        *outcomes.entry(class).or_default() += 1;
    }
    for class in [
        "negative id",
        "stopped nothing",
        "parked",
        "cancelled",
        "started",
        "updated",
    ] {
        assert!(
            outcomes.contains_key(class),
            "starter outcome {class} not covered: {outcomes:?}"
        );
    }
}

fn marked(case: &Case, memory: &SparseSvcMemory) -> bool {
    (0..4).any(|i| {
        let address = OBJ_BASE + 0x1000 * i + 0xB60;
        memory.r32(address) != case.input(address)
    })
}

#[test]
fn disturbance_matches_original() {
    let mut hit_cases = 0;
    for (n, case) in load(include_str!("data/t1856/effect_submit_e94c0.golden"), 60)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let radius = f32::from_bits(case.input(ARGS));
        disturb_nearby(&mut memory, POS_A, radius, case.input(ARGS + 4));
        assert_outputs(case, &memory, &format!("e94c0 case {n}"));
        if marked(case, &memory) {
            hit_cases += 1;
        }
    }
    assert!(hit_cases > 5, "{hit_cases}");
    let mut with_shape = 0;
    for (n, case) in load(include_str!("data/t1856/effect_submit_e9380.golden"), 60)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let shape = case.input(ARGS);
        let radius = f32::from_bits(case.input(ARGS + 4));
        disturb_objects(&mut memory, POS_A, shape, radius, case.input(ARGS + 8));
        assert_outputs(case, &memory, &format!("e9380 case {n}"));
        if shape != 0 && marked(case, &memory) {
            with_shape += 1;
        }
    }
    assert!(with_shape > 3, "{with_shape}");
}

struct CountingInner {
    released: Vec<u32>,
}

impl ImpactServices for CountingInner {
    fn disturb(&mut self, _position: [f32; 3], _radius: f32, _flags: u32) {
        panic!("inner disturb must not be used");
    }
    fn spawn_request(&mut self, _record: [u32; 12]) {
        panic!("inner spawn must not be used");
    }
    fn build_debris(&mut self, slot: u32) {
        self.released.push(slot);
    }
    fn ground_height(&mut self, _point: [f32; 3], _y_bias: f32, _half_extent: f32) -> f32 {
        7.5
    }
}

#[test]
fn impact_services_adapter_equals_direct_calls() {
    // spawn_request: the golden closure cases with the request words fed through the adapter (scratch == REQ)
    for (n, case) in load(include_str!("data/t1856/effect_submit_closure.golden"), 60)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        let mut inner = CountingInner { released: vec![] };
        let mut record = [0u32; 12];
        for (i, word) in record.iter_mut().enumerate() {
            *word = case.input(REQ + 4 * i as u32);
        }
        let mut services = EffectSubmitServices {
            memory: &mut memory,
            hw: &mut replay,
            inner: &mut inner,
            scratch: REQ,
            submitted: Vec::new(),
        };
        services.spawn_request(record);
        services.build_debris(0x1234);
        assert_eq!(services.ground_height([0.0; 3], 0.01, 0.5), 7.5);
        assert_eq!(
            services.submitted,
            vec![case.eax as i32],
            "adapter case {n}"
        );
        assert_eq!(inner.released, vec![0x1234]);
        assert_calls(case, &replay, &format!("adapter case {n}"));
        assert_outputs(case, &memory, &format!("adapter case {n}"));
    }
    // disturb: the e94c0 cases through the adapter (position copied to scratch + 0x40)
    for (n, case) in load(include_str!("data/t1856/effect_submit_e94c0.golden"), 60)
        .iter()
        .enumerate()
    {
        let mut memory = world(case);
        let mut replay = Replay::new(case);
        let mut inner = CountingInner { released: vec![] };
        let mut services = EffectSubmitServices {
            memory: &mut memory,
            hw: &mut replay,
            inner: &mut inner,
            scratch: SPARE,
            submitted: Vec::new(),
        };
        let radius = f32::from_bits(case.input(ARGS));
        services.disturb(case.vec3(POS_A), radius, case.input(ARGS + 4));
        assert_outputs(case, &memory, &format!("adapter disturb case {n}"));
    }
}

#[test]
fn semantic_values() {
    let mut memory = SparseSvcMemory::default();
    // out of range ids never touch the tables or the hardware
    for id in [u32::MAX, 0x1649, 0x8000_0000] {
        memory.w32(REQ, id);
        let mut replay = Replay::empty();
        assert_eq!(submit_effect(&mut memory, &mut replay, REQ), -1);
        assert!(replay.log().is_empty());
    }
    // the token counter wraps to 1 above 0x3FFFFFF and the returned token is the old value
    memory.w32(0x4C_A9B4, 0x170_0000);
    memory.w32(0x4C_A9B0, 0x3FF_FFFF);
    memory.w32(REQ, 1);
    assert_eq!(allocate_token(&mut memory, REQ), 0x3FF_FFFF);
    assert_eq!(memory.r32(0x4C_A9B0), 1);
    assert_eq!(allocate_token(&mut memory, REQ), 1);
    assert_eq!(memory.r32(0x4C_A9B0), 2);
    // class bits: owner set gives the queue bit
    memory.w32(REQ + 4, 0x55);
    assert_eq!(allocate_token(&mut memory, REQ) & 0x2000_0000, 0x2000_0000);
}
