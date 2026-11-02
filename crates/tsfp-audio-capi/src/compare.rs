//! The compare twin: after the C DirectSound handler ran, replay the same call through the Rust lifecycle, setters and stream
//! frames of `tsfp-audio` over a [`Shadow`] world and compare what is observable: the HRESULT/EAX, the device reference
//! word, the object header words in guest memory and the whole setter snapshot (C `dsound_stream_snapshot` /
//! `dsound_buffer_snapshot` against `StreamState` / `BufferState`). C-side addresses and serials differ from the shadow ones,
//! so an alias map (C address to Rust address, C serial to Rust serial) is filled at create time and used for translation.
//!
//! The completion-model hooks are not run twice: the C glue records every answer the C handler got, the twin replays them
//! (and checks the Rust arguments against the recorded ones), so a hook the C never consulted, a hook consulted in a different
//! order or with different arguments is a mismatch of its own.
//!
//! Evidence class: MEASURED on the live hybrid host (see docs/t-audio-replace.md, "Live compare mode"), not xemu.

use std::collections::{BTreeMap, HashMap, VecDeque};
use tsfp_audio::adapter::{GuestMemory, Refusal};
use tsfp_audio::frames::{self, stream_frame};
use tsfp_audio::lifecycle::{Device, GuestWorld, Objects, SINGLETON};
use tsfp_audio::routing::{Routing, StreamHooks};
use tsfp_audio::setters::{BufferHooks, BufferState, Policy, Setters, StreamState};

use crate::world::{DEFAULT_HEAP_BASE, Host, Shadow};

pub const DS_CREATE: u32 = 0x0040_9635;
pub const DS_RELEASE: u32 = 0x0040_6A8A;
pub const B_CREATE: u32 = 0x0040_93C8;
pub const B_SET_DATA: u32 = 0x0040_8C0D;
pub const B_SET_VOLUME: u32 = 0x0040_7A64;
pub const B_PAUSE: u32 = 0x0040_7ABC;
pub const B_SET_FREQUENCY: u32 = 0x0040_84F2;
pub const B_LOOP: u32 = 0x0040_7AD8;
pub const B_I3DL2: u32 = 0x0040_85AF;
pub const B_MINIMUM: u32 = 0x0040_8532;
pub const B_ROLLOFF: u32 = 0x0040_858B;
pub const B_MAX_DISTANCE: u32 = 0x0040_850E;
pub const B_POSITION: u32 = 0x0040_8556;
/// Pseudo addresses of the two stream methods the host reaches through function pointers of xdk_thunk (not dsound_hle_call):
/// the startup Discontinuity and GetStatus (`xdk_thunk_set_stream_virtual_handlers`).
pub const P_DISCONTINUITY: u32 = 0xFFFF_0001;
pub const P_STARTUP_STATUS: u32 = 0xFFFF_0002;
/// Caller of the XMV movie DirectSoundCreate (anything but the startup caller 0x279D5 is a movie caller).
const STARTUP_CREATE_CALLER: u32 = 0x279D5;
const SYNTH_STACK: u32 = 0xF0F0_0000;

pub const NAMES: &[(u32, &str)] = &[
    (DS_CREATE, "DirectSoundCreate"),
    (DS_RELEASE, "IDirectSound_Release"),
    (B_CREATE, "IDirectSound_CreateSoundBuffer"),
    (B_SET_DATA, "IDirectSoundBuffer_SetBufferData"),
    (B_SET_VOLUME, "IDirectSoundBuffer_SetVolume"),
    (B_PAUSE, "IDirectSoundBuffer_Pause"),
    (B_SET_FREQUENCY, "IDirectSoundBuffer_SetFrequency"),
    (B_LOOP, "IDirectSoundBuffer_SetLoopRegion"),
    (B_I3DL2, "IDirectSoundBuffer_SetI3DL2Source"),
    (B_MINIMUM, "IDirectSoundBuffer_SetMinDistance"),
    (B_ROLLOFF, "IDirectSoundBuffer_SetRolloffCurve"),
    (B_MAX_DISTANCE, "IDirectSoundBuffer_SetMaxDistance"),
    (B_POSITION, "IDirectSoundBuffer_SetPosition"),
    (frames::CREATE, "DirectSoundCreateStream"),
    (frames::I3DL2, "IDirectSoundStream_SetI3DL2Source"),
    (frames::MINIMUM, "IDirectSoundStream_SetMinDistance"),
    (frames::MAX_DISTANCE, "IDirectSoundStream_SetMaxDistance"),
    (frames::SET_POSITION, "IDirectSoundStream_SetPosition"),
    (frames::ROLLOFF, "IDirectSoundStream_SetRolloffCurve"),
    (frames::VOLUME, "IDirectSoundStream_SetVolume"),
    (frames::PAUSE, "IDirectSoundStream_Pause"),
    (frames::SET_FORMAT, "IDirectSoundStream_SetFormat"),
    (frames::HEADROOM, "IDirectSoundStream_SetHeadroom"),
    (frames::MIX_VOLUMES, "IDirectSoundStream_SetMixBinVolumes"),
    (frames::MIX_BINS, "IDirectSoundStream_SetMixBins"),
    (frames::FLUSH_EX, "IDirectSoundStream_FlushEx"),
    (frames::SET_FREQUENCY, "IDirectSoundStream_SetFrequency"),
    (
        P_DISCONTINUITY,
        "IDirectSoundStream_Discontinuity (virtual)",
    ),
    (P_STARTUP_STATUS, "IDirectSoundStream_GetStatus (virtual)"),
];

pub fn name_of(va: u32) -> &'static str {
    NAMES
        .iter()
        .find(|(v, _)| *v == va)
        .map_or("(not modelled)", |(_, n)| n)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Device,
    Stream,
    Buffer,
    Other,
}

fn family(va: u32) -> Family {
    match va {
        DS_CREATE | DS_RELEASE => Family::Device,
        B_CREATE | B_SET_DATA | B_SET_VOLUME | B_PAUSE | B_SET_FREQUENCY | B_LOOP | B_I3DL2
        | B_MINIMUM | B_ROLLOFF | B_MAX_DISTANCE | B_POSITION => Family::Buffer,
        frames::CREATE
        | frames::I3DL2
        | frames::MINIMUM
        | frames::MAX_DISTANCE
        | frames::SET_POSITION
        | frames::ROLLOFF
        | frames::VOLUME
        | frames::PAUSE
        | frames::SET_FORMAT
        | frames::HEADROOM
        | frames::MIX_VOLUMES
        | frames::MIX_BINS
        | frames::FLUSH_EX
        | frames::SET_FREQUENCY
        | P_DISCONTINUITY
        | P_STARTUP_STATUS => Family::Stream,
        _ => Family::Other,
    }
}

/// One recorded completion-model hook answer (mirrors the C `audcmp_rec`).
#[derive(Clone, Debug)]
pub struct Rec {
    pub kind: u32,
    pub a: [u32; 5],
    pub serial: u64,
    pub result: bool,
    pub blob: [u32; 40],
}
pub const REC_ROUTING: u32 = 1;
pub const REC_FREQUENCY: u32 = 2;
pub const REC_CONTROL_WORD: u32 = 3;
pub const REC_FORMAT: u32 = 4;
pub const REC_STREAM_PAUSE: u32 = 5;
pub const REC_STARTED: u32 = 6;
pub const REC_BUFFER_PAUSE: u32 = 7;
pub const REC_BUFFER_FREQUENCY: u32 = 8;
pub const REC_VOICE_RUNNING: u32 = 9;
pub const REC_LOOP: u32 = 10;

#[derive(Clone, Copy, Debug, Default)]
pub struct Config {
    pub streams_enabled: bool,
    pub buffers_enabled: bool,
    pub completion: bool,
    pub frequency_hook: bool,
    pub routing_hook: bool,
    pub format_hook: bool,
    pub movie_calls: bool,
}

/// Deliberate deviation of the Rust twin, only with the `fault-inject` feature (detection power check).
#[cfg(feature = "fault-inject")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Flip the low bit of the Rust return value of every 5th modelled call.
    Ret,
    /// Flip the first body word of the Rust setter snapshot.
    State,
    /// Flip the Rust device reference count.
    Refcount,
    /// Flip the first Rust header word.
    Header,
    /// Answer every recorded hook the other way round.
    Hook,
}

#[derive(Default, Clone, Debug)]
pub struct VaStat {
    pub calls: u64,
    pub compared: u64,
    pub checks: u64,
    pub mismatches: u64,
    pub refusals: u64,
    pub foreign: u64,
    pub unmodelled: u64,
}

/// Replays the recorded hook answers and checks the Rust arguments against the recorded ones.
struct Replay<'a> {
    recs: VecDeque<Rec>,
    alias: &'a HashMap<u32, u32>,
    serial_alias: &'a HashMap<u64, u64>,
    cfg: Config,
    problems: Vec<String>,
    consumed: Vec<u32>,
    #[cfg(feature = "fault-inject")]
    flip: bool,
}

impl Replay<'_> {
    fn pop(&mut self, kind: u32, what: &str) -> Option<Rec> {
        match self.recs.front() {
            Some(r) if r.kind == kind => {
                self.consumed.push(kind);
                self.recs.pop_front()
            }
            Some(r) => {
                self.problems.push(format!(
                    "hook order: rust asked {what}, C recorded kind {}",
                    r.kind
                ));
                None
            }
            None => {
                self.problems
                    .push(format!("hook order: rust asked {what}, C recorded none"));
                None
            }
        }
    }
    fn same_object(&mut self, what: &str, c: u32, r: u32, cserial: u64, rserial: u64) {
        if self.alias.get(&c).copied().unwrap_or(c) != r {
            self.problems
                .push(format!("{what}: object C {c:#x} vs rust {r:#x}"));
        }
        if self.serial_alias.get(&cserial).copied().unwrap_or(cserial) != rserial {
            self.problems
                .push(format!("{what}: serial C {cserial} vs rust {rserial}"));
        }
    }
    fn answer(&self, r: &Rec) -> bool {
        #[cfg(feature = "fault-inject")]
        if self.flip {
            return !r.result;
        }
        r.result
    }
    fn args(&mut self, what: &str, r: &Rec, want: &[u32]) {
        if r.a[..want.len()] != *want {
            self.problems.push(format!(
                "{what}: args C {:?} vs rust {:?}",
                &r.a[..want.len()],
                want
            ));
        }
    }
}

impl StreamHooks for Replay<'_> {
    fn frequency_installed(&self) -> bool {
        self.cfg.frequency_hook
    }
    fn routing_installed(&self) -> bool {
        self.cfg.routing_hook
    }
    fn format_installed(&self) -> bool {
        self.cfg.format_hook
    }
    fn control_word(&mut self) -> Option<u16> {
        let r = self.pop(REC_CONTROL_WORD, "control_word")?;
        r.result.then_some(r.a[0] as u16)
    }
    fn frequency_note(&mut self, stream: u32, serial: u64, old: u32, new: u32) -> bool {
        match self.pop(REC_FREQUENCY, "frequency_note") {
            Some(r) => {
                self.same_object("frequency_note", r.a[0], stream, r.serial, serial);
                self.args(
                    "frequency_note",
                    &Rec {
                        a: [r.a[1], r.a[2], 0, 0, 0],
                        ..r.clone()
                    },
                    &[old, new],
                );
                self.answer(&r)
            }
            None => false,
        }
    }
    fn routing_note(&mut self, stream: u32, serial: u64, c: &Routing) -> bool {
        match self.pop(REC_ROUTING, "routing_note") {
            Some(r) => {
                self.same_object("routing_note", r.a[0], stream, r.serial, serial);
                let want = routing_words(c);
                if r.blob[..want.len()] != want[..] {
                    self.problems.push(format!(
                        "routing_note: candidate C {:?} vs rust {:?}",
                        &r.blob[..want.len()],
                        want
                    ));
                }
                self.answer(&r)
            }
            None => false,
        }
    }
    fn format_note(&mut self, stream: u32, serial: u64, old: u32, new: u32, format: u32) -> bool {
        match self.pop(REC_FORMAT, "format_note") {
            Some(r) => {
                self.same_object("format_note", r.a[0], stream, r.serial, serial);
                self.args(
                    "format_note",
                    &Rec {
                        a: [r.a[1], r.a[2], r.a[3], 0, 0],
                        ..r.clone()
                    },
                    &[old, new, format],
                );
                self.answer(&r)
            }
            None => false,
        }
    }
}

impl BufferHooks for Replay<'_> {
    fn started(&mut self, buffer: u32, serial: u64) -> bool {
        match self.pop(REC_STARTED, "started") {
            Some(r) => {
                self.same_object("started", r.a[0], buffer, r.serial, serial);
                self.answer(&r)
            }
            None => false,
        }
    }
    fn voice_running(&mut self, buffer: u32, serial: u64) -> bool {
        match self.pop(REC_VOICE_RUNNING, "voice_running") {
            Some(r) => {
                self.same_object("voice_running", r.a[0], buffer, r.serial, serial);
                self.answer(&r)
            }
            None => false,
        }
    }
    fn pause(&mut self, buffer: u32, serial: u64, mode: u32) -> bool {
        match self.pop(REC_BUFFER_PAUSE, "buffer pause") {
            Some(r) => {
                self.same_object("buffer pause", r.a[0], buffer, r.serial, serial);
                self.args(
                    "buffer pause",
                    &Rec {
                        a: [r.a[1], 0, 0, 0, 0],
                        ..r.clone()
                    },
                    &[mode],
                );
                self.answer(&r)
            }
            None => false,
        }
    }
    fn frequency(&mut self, buffer: u32, serial: u64, old: u32, new: u32) -> bool {
        match self.pop(REC_BUFFER_FREQUENCY, "buffer frequency") {
            Some(r) => {
                self.same_object("buffer frequency", r.a[0], buffer, r.serial, serial);
                self.args(
                    "buffer frequency",
                    &Rec {
                        a: [r.a[1], r.a[2], 0, 0, 0],
                        ..r.clone()
                    },
                    &[old, new],
                );
                self.answer(&r)
            }
            None => false,
        }
    }
    fn loop_region(&mut self, buffer: u32, serial: u64, start: u32, len: u32, hz: u32) -> bool {
        match self.pop(REC_LOOP, "loop region") {
            Some(r) => {
                self.same_object("loop region", r.a[0], buffer, r.serial, serial);
                self.args(
                    "loop region",
                    &Rec {
                        a: [r.a[1], r.a[2], r.a[3], 0, 0],
                        ..r.clone()
                    },
                    &[start, len, hz],
                );
                self.answer(&r)
            }
            None => false,
        }
    }
}

fn pack_bytes(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks(4)
        .map(|c| {
            c.iter()
                .enumerate()
                .fold(0u32, |a, (i, b)| a | u32::from(*b) << (8 * i))
        })
        .collect()
}

fn routing_words(r: &Routing) -> Vec<u32> {
    let mut v = vec![r.volume, r.headroom, r.aggregate, r.count];
    v.extend(pack_bytes(&r.bins));
    v.extend_from_slice(&r.bin_volume);
    v
}

/// The word sequence `stream_snapshot` of the C glue produces, body first, identity tail excluded.
fn stream_body(s: &StreamState) -> Vec<(&'static str, u32)> {
    let mut v: Vec<(&'static str, u32)> = vec![("cache_mask", s.cache_mask)];
    for w in s.i3dl2 {
        v.push(("i3dl2", w));
    }
    v.extend([
        ("i3dl2_address", s.i3dl2_address),
        ("min_distance_bits", s.min_distance_bits),
        ("max_distance_seen", u32::from(s.max_distance_seen)),
        ("max_distance_bits", s.max_distance_bits),
        ("curve_address", s.curve_address),
        ("curve_count", s.curve_count),
        ("pause_seen", u32::from(s.pause_seen)),
        ("pause_mode", s.pause_mode),
        ("flush_seen", u32::from(s.flush_seen)),
        ("flush_time_low", s.flush_time_low),
        ("flush_time_high", s.flush_time_high),
        ("flush_flags", s.flush_flags),
        ("discontinuity_seen", u32::from(s.discontinuity_seen)),
        ("volume_seen", u32::from(s.volume_seen)),
        ("volume", s.volume as u32),
        ("format_sets", s.format_sets),
        ("source_rate_hz", s.source_rate_hz),
        ("frequency_sets", s.frequency_sets),
        ("frequency_pitch", s.frequency_pitch as u32),
    ]);
    // The C keeps the recorded SetFormat bytes (zero until a SetFormat), the Rust pre-fills the scope format and reads it only
    // through `format_sets != 0`: compare the recorded view so the representation difference is not reported as a mismatch.
    let recorded = if s.format_sets != 0 {
        s.format
    } else {
        [0; 20]
    };
    for w in pack_bytes(&recorded) {
        v.push(("format", w));
    }
    let r = routing_words(&s.routing);
    for (i, w) in r.into_iter().enumerate() {
        v.push((
            match i {
                0 => "routing.volume",
                1 => "routing.headroom",
                2 => "routing.aggregate",
                3 => "routing.count",
                4 | 5 => "routing.bins",
                _ => "routing.bin_volume",
            },
            w,
        ));
    }
    for w in s.position_bits {
        v.push(("position_bits", w));
    }
    v.push(("position_sets", s.position_sets));
    v
}

fn buffer_body(s: &BufferState) -> Vec<(&'static str, u32)> {
    let mut v: Vec<(&'static str, u32)> = vec![("cache_mask", s.cache_mask)];
    for w in s.i3dl2 {
        v.push(("i3dl2", w));
    }
    v.extend([
        ("i3dl2_address", s.i3dl2_address),
        ("min_distance_bits", s.min_distance_bits),
        ("curve_address", s.curve_address),
        ("curve_count", s.curve_count),
        ("data_address", s.data_address),
        ("data_length", s.data_length),
        ("data_sets", s.data_sets),
        ("loop_start", s.loop_start),
        ("loop_length", s.loop_length),
        ("loop_sets", s.loop_sets),
        ("volume", s.volume as u32),
        ("volume_sets", s.volume_sets),
        ("pause_sets", s.pause_sets),
        ("frequency", s.frequency),
        ("frequency_sets", s.frequency_sets),
        ("max_distance_bits", s.max_distance_bits),
        ("max_distance_sets", s.max_distance_sets),
        ("min_distance_sets", s.min_distance_sets),
        ("rolloff_sets", s.rolloff_sets),
    ]);
    for w in s.position_bits {
        v.push(("position_bits", w));
    }
    v.push(("position_sets", s.position_sets));
    v
}

/// The word sequence the C glue's `stream_snapshot` produces, built from the Rust model (test stand-in for the C side).
pub fn stream_snapshot_words(setters: &Setters, objs: &Objects, public: u32) -> Option<Vec<u32>> {
    let (s, o) = (setters.stream(public)?, objs.find(public)?);
    let mut v: Vec<u32> = stream_body(s).into_iter().map(|(_, w)| w).collect();
    v.extend([
        o.lease.serial as u32,
        (o.lease.serial >> 32) as u32,
        o.public_address,
    ]);
    v.extend_from_slice(&o.header);
    Some(v)
}

/// Buffer twin of [`stream_snapshot_words`].
pub fn buffer_snapshot_words(setters: &Setters, objs: &Objects, public: u32) -> Option<Vec<u32>> {
    let (s, o) = (setters.buffer(public)?, objs.find(public)?);
    let mut v: Vec<u32> = buffer_body(s).into_iter().map(|(_, w)| w).collect();
    v.extend([
        o.lease.serial as u32,
        (o.lease.serial >> 32) as u32,
        o.public_address,
        o.header_address,
    ]);
    v.extend_from_slice(&o.header);
    Some(v)
}

pub struct Twin<H: Host> {
    pub w: Shadow<H>,
    pub cfg: Config,
    device: Device,
    objs: Objects,
    setters: Setters,
    alias: HashMap<u32, u32>,
    serial_alias: HashMap<u64, u64>,
    pub stats: BTreeMap<u32, VaStat>,
    pub log: Vec<String>,
    pub log_limit: usize,
    key_counts: HashMap<String, u32>,
    /// (C internal, Rust internal) device addresses.
    device_alias: Option<(u32, u32)>,
    /// C device reference minus Rust device reference: the leases of streams this twin does not model (movie streams).
    delta: u32,
    pub rebaselines: u64,
    /// Hook answers replayed by record kind (index 0: stream pause notes).
    pub hook_counts: [u64; 12],
    pub total_mismatches: u64,
    /// Rust return value of the call being observed, set by `compare_value` (mode 1 forwards it to the guest).
    pub last_answer: Option<u32>,
    pub hook_problems: u64,
    pub snapshot_missing: u64,
    pub records_dropped: u64,
    calls_modelled: u64,
    #[cfg(feature = "fault-inject")]
    pub fault: Option<Fault>,
}

enum Outcome {
    Value(u32),
    Refused(&'static str),
}

impl<H: Host> Twin<H> {
    pub fn new(host: H, cfg: Config) -> Self {
        Self::with_heap_base(host, cfg, DEFAULT_HEAP_BASE)
    }
    pub fn with_heap_base(host: H, cfg: Config, heap_base: u32) -> Self {
        let mut w = Shadow::new(host, heap_base);
        // Pre-state of the one title global the C lifecycle writes and the Rust lifecycle reads before writing it.
        w.seed(SINGLETON, &[0; 4]);
        w.map_zero(SYNTH_STACK, 0x100);
        let mut device = Device::new();
        device.movie_calls = cfg.movie_calls;
        Self {
            w,
            cfg,
            device,
            objs: Objects::new(),
            setters: Setters::new(Policy {
                enabled: cfg.streams_enabled && cfg.buffers_enabled,
                completion: cfg.completion,
                irql0: false,
            }),
            alias: HashMap::new(),
            serial_alias: HashMap::new(),
            stats: BTreeMap::new(),
            log: Vec::new(),
            log_limit: 400,
            key_counts: HashMap::new(),
            device_alias: None,
            delta: 0,
            rebaselines: 0,
            hook_counts: [0; 12],
            total_mismatches: 0,
            last_answer: None,
            hook_problems: 0,
            snapshot_missing: 0,
            records_dropped: 0,
            calls_modelled: 0,
            #[cfg(feature = "fault-inject")]
            fault: None,
        }
    }

    fn tr(&self, c: u32) -> u32 {
        self.alias.get(&c).copied().unwrap_or(c)
    }
    fn trs(&self, c: u64) -> u64 {
        self.serial_alias.get(&c).copied().unwrap_or(c)
    }

    /// Print a line unless this class (`key`) already printed three, or the log is full.
    fn emit(&mut self, key: String, line: String, per_key: u32) {
        let n = self.key_counts.entry(key).or_insert(0);
        *n += 1;
        if *n <= per_key && self.log.len() < self.log_limit {
            eprintln!("{line}");
            self.log.push(line);
        }
    }

    fn mismatch(&mut self, va: u32, args: &[u32], what: String) {
        self.total_mismatches += 1;
        self.stats.entry(va).or_default().mismatches += 1;
        let key = format!("m|{va:x}|{}", what.split(':').next().unwrap_or(""));
        let line = format!(
            "audio-cmp: MISMATCH #{} {} ({va:#010x}) args {:x?}: {what}",
            self.total_mismatches,
            name_of(va),
            args
        );
        self.emit(key, line, 3);
    }

    /// A stream the C created but this twin does not model (movie extension) took a device lease: recompute the foreign
    /// share of the C reference word. Not visible otherwise, the C has no lease count API.
    fn rebaseline_delta(&mut self) {
        let Some((c_internal, _)) = self.device_alias else {
            return;
        };
        if let Some(b) = self.w.read_real(c_internal + 4, 4) {
            let c_ref = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            self.delta = c_ref.wrapping_sub(self.device.reference_count());
            self.rebaselines += 1;
        }
    }

    fn check(&mut self, va: u32) {
        self.stats.entry(va).or_default().checks += 1;
    }

    /// Observe one handler call. `stack` is the guest stack at call entry (return address, then the arguments),
    /// `result` the C handler's EAX, `recs` the hook answers the C handler consulted.
    pub fn observe(&mut self, va: u32, stack: &[u32], result: u32, recs: &[Rec], dropped: u32) {
        self.last_answer = None;
        self.stats.entry(va).or_default().calls += 1;
        let fam = family(va);
        if fam == Family::Other {
            self.stats.entry(va).or_default().unmodelled += 1;
            return;
        }
        self.records_dropped += u64::from(dropped);
        let mut args = [0u32; 6];
        for (i, a) in args.iter_mut().enumerate() {
            *a = stack.get(1 + i).copied().unwrap_or(0);
        }
        let ret = stack.first().copied().unwrap_or(0);
        self.calls_modelled += 1;
        self.setters.policy.irql0 = self.w.host.irql0();
        match fam {
            Family::Device => self.device_call(va, ret, &args, result),
            Family::Stream => self.object_call(va, ret, stack, &args, result, recs, true),
            Family::Buffer => self.object_call(va, ret, stack, &args, result, recs, false),
            Family::Other => {}
        }
    }

    fn compare_value(&mut self, va: u32, args: &[u32], c: u32, outcome: Outcome) -> Option<u32> {
        match outcome {
            Outcome::Value(r) => {
                #[cfg(feature = "fault-inject")]
                let r = if self.fault == Some(Fault::Ret) && self.calls_modelled % 5 == 0 {
                    r ^ 1
                } else {
                    r
                };
                self.check(va);
                if r != c {
                    self.mismatch(va, args, format!("return C {c:#x} vs rust {r:#x}"));
                }
                self.last_answer = Some(r);
                Some(r)
            }
            Outcome::Refused(reason) => {
                self.stats.entry(va).or_default().refusals += 1;
                let line = format!(
                    "audio-cmp: RUST REFUSES {} ({va:#010x}) args {args:x?}: {reason} (C answered {c:#x})",
                    name_of(va)
                );
                self.emit(format!("r|{va:x}|{reason}"), line, 1);
                None
            }
        }
    }

    fn outcome(r: Result<u32, Refusal>) -> Outcome {
        match r {
            Ok(v) => Outcome::Value(v),
            Err(Refusal(why)) => Outcome::Refused(why),
        }
    }

    /// Device reference word and device header: C real memory against the Rust overlay.
    fn compare_device(&mut self, va: u32, args: &[u32]) {
        let Some((c_internal, r_internal)) = self.device_alias else {
            return;
        };
        let c_words = self.w.read_real(c_internal, 44);
        let r_words = self.w.read_bytes(r_internal, 44);
        let (Some(c), Some(r)) = (c_words, r_words) else {
            self.mismatch(va, args, "device header unreadable".into());
            return;
        };
        let to_words = |b: &[u8]| -> Vec<u32> {
            b.chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        };
        #[allow(unused_mut)]
        let (cw, mut rw) = (to_words(&c), to_words(&r));
        // the foreign (movie stream) leases are part of the C count only
        rw[1] = rw[1].wrapping_add(self.delta);
        #[cfg(feature = "fault-inject")]
        if self.fault == Some(Fault::Refcount) {
            rw[1] ^= 1;
        }
        #[cfg(feature = "fault-inject")]
        if self.fault == Some(Fault::Header) {
            rw[0] ^= 1;
        }
        self.check(va);
        let translated: Vec<u32> = cw.iter().map(|w| self.tr(*w)).collect();
        if translated != rw {
            let first = translated
                .iter()
                .zip(&rw)
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            self.mismatch(
                va,
                args,
                format!(
                    "device header word {first}: C {:#x} (translated {:#x}) vs rust {:#x}",
                    cw[first], translated[first], rw[first]
                ),
            );
        }
        self.check(va);
        if cw[1] != self.device.reference_count().wrapping_add(self.delta) {
            self.mismatch(
                va,
                args,
                format!(
                    "device reference word C {} vs rust model {} + foreign {}",
                    cw[1],
                    self.device.reference_count(),
                    self.delta
                ),
            );
        }
    }

    fn device_call(&mut self, va: u32, ret: u32, args: &[u32; 6], result: u32) {
        let outcome = if va == DS_CREATE {
            let movie = ret != STARTUP_CREATE_CALLER;
            let ready = self.w.host.codec_ready();
            Self::outcome(
                self.device
                    .create(&mut self.w, args[0], args[1], args[2], movie, ready),
            )
        } else {
            let iface = self.tr(args[0]);
            let delta = self.delta;
            match self.device.release_movie(&mut self.w, iface) {
                Ok(v) => Outcome::Value(v.wrapping_add(delta)),
                Err(Refusal(why)) => Outcome::Refused(why),
            }
        };
        let ok = self.compare_value(va, args, result, outcome);
        if ok.is_none() {
            return;
        }
        if va == DS_CREATE && result == 0 {
            let c_pub = self
                .w
                .read_real(args[1], 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            let r_pub = self.w.read_u32(args[1]);
            if let (Some(c), Some(r)) = (c_pub, r_pub) {
                // public pointer is internal + 8, the header also holds internal + 16
                self.device_alias = Some((c.wrapping_sub(8), r.wrapping_sub(8)));
                for off in [0u32, 8, 16] {
                    self.alias.insert(
                        c.wrapping_sub(8).wrapping_add(off),
                        r.wrapping_sub(8).wrapping_add(off),
                    );
                }
            }
        }
        self.compare_device(va, args);
        self.stats.entry(va).or_default().compared += 1;
    }

    fn object_call(
        &mut self,
        va: u32,
        _ret: u32,
        stack: &[u32],
        args: &[u32; 6],
        result: u32,
        recs: &[Rec],
        stream: bool,
    ) {
        let is_create = va == frames::CREATE && stream || va == B_CREATE && !stream;
        // Translate the object argument. Unknown object: owned by a C extension (movie stream) or created before
        // the twin saw it, so there is nothing to compare.
        let mut tstack: Vec<u32> = stack.to_vec();
        if tstack.len() < 7 {
            tstack.resize(7, 0);
        }
        if !(is_create && stream) {
            let c_obj = args[0];
            let known = self.alias.contains_key(&c_obj);
            if !known {
                self.stats.entry(va).or_default().foreign += 1;
                return;
            }
            tstack[1] = self.tr(c_obj);
        }
        let mut words = Vec::with_capacity(tstack.len() * 4);
        for w in &tstack {
            words.extend_from_slice(&w.to_le_bytes());
        }
        self.w.write_bytes(SYNTH_STACK, &words);

        let mut pause_notes: Vec<(u32, u32)> = Vec::new();
        let mut replay = Replay {
            recs: recs
                .iter()
                .filter(|r| r.kind != REC_STREAM_PAUSE)
                .cloned()
                .collect(),
            alias: &self.alias,
            serial_alias: &self.serial_alias,
            cfg: self.cfg,
            problems: Vec::new(),
            consumed: Vec::new(),
            #[cfg(feature = "fault-inject")]
            flip: self.fault == Some(Fault::Hook),
        };
        let outcome = if stream {
            let mut note = |s: u32, m: u32| pause_notes.push((s, m));
            let a = tstack[1..].to_vec();
            if va == P_DISCONTINUITY {
                Self::outcome(self.setters.stream_discontinuity(
                    &self.objs,
                    &self.device,
                    &self.w,
                    a[0],
                ))
            } else if va == P_STARTUP_STATUS {
                Self::outcome(self.setters.stream_startup_status(
                    &self.objs,
                    &self.device,
                    &mut self.w,
                    a[0],
                    a[1],
                ))
            } else {
                Self::outcome(stream_frame(
                    &mut self.setters,
                    &mut self.objs,
                    &mut self.device,
                    &mut self.w,
                    &mut replay,
                    va,
                    SYNTH_STACK,
                    &mut note,
                ))
            }
        } else {
            let a = tstack[1..].to_vec();
            let (s, o, d, w) = (
                &mut self.setters,
                &mut self.objs,
                &mut self.device,
                &mut self.w,
            );
            Self::outcome(match va {
                B_CREATE => s.create_buffer(o, d, w, a[0], a[1], a[2], a[3]),
                B_SET_DATA => s.buffer_set_data(o, d, &*w, a[0], a[1], a[2]),
                B_SET_VOLUME => s.buffer_volume(o, d, &*w, a[0], a[1], &mut replay),
                B_PAUSE => s.buffer_pause(o, d, &*w, a[0], a[1], &mut replay),
                B_SET_FREQUENCY => s.buffer_frequency(o, d, &*w, a[0], a[1], &mut replay),
                B_LOOP => s.buffer_loop_region(o, d, &*w, a[0], a[1], a[2], &mut replay),
                B_I3DL2 => s.buffer_i3dl2(o, d, &*w, a[0], a[1], a[2]),
                B_MINIMUM => s.buffer_min_distance(o, d, &*w, a[0], a[1], a[2]),
                B_ROLLOFF => s.buffer_rolloff(o, d, &*w, a[0], a[1], a[2], a[3]),
                B_MAX_DISTANCE => s.buffer_max_distance(o, d, &*w, a[0], a[1], a[2]),
                _ => s.buffer_position(o, d, &*w, a[0], [a[1], a[2], a[3]], a[4]),
            })
        };
        let leftover: Vec<u32> = replay.recs.iter().map(|r| r.kind).collect();
        let mut problems = std::mem::take(&mut replay.problems);
        for kind in std::mem::take(&mut replay.consumed) {
            self.hook_counts[(kind as usize).min(11)] += 1;
        }
        self.hook_counts[0] += pause_notes.len() as u64;
        drop(replay);

        // A stream CREATE the Rust model refuses while the C answered success and has no stream of its own at the published
        // pointer was answered by the movie stream extension (T392): not ours, nothing to compare.
        if is_create && stream && result == 0 && matches!(outcome, Outcome::Refused(_)) {
            let c_out = self
                .w
                .read_real(args[1], 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .unwrap_or(0);
            if self.w.host.stream_snapshot(c_out).is_none() {
                self.stats.entry(va).or_default().foreign += 1;
                self.rebaseline_delta();
                return;
            }
        }

        let value = self.compare_value(va, &args[..], result, outcome);
        if value.is_none() {
            return;
        }
        if !leftover.is_empty() {
            problems.push(format!(
                "hook records the C consulted but rust did not: kinds {leftover:?}"
            ));
        }
        // Pause notes: compare with the recorded ones.
        let c_notes: Vec<(u32, u32)> = recs
            .iter()
            .filter(|r| r.kind == REC_STREAM_PAUSE)
            .map(|r| (self.tr(r.a[0]), r.a[1]))
            .collect();
        self.check(va);
        if c_notes != pause_notes {
            problems.push(format!(
                "stream pause notes C {c_notes:x?} vs rust {pause_notes:x?}"
            ));
        }
        for p in problems {
            self.hook_problems += 1;
            self.mismatch(va, &args[..], p);
        }

        let c_obj = if is_create {
            self.w
                .read_real(args[if stream { 1 } else { 2 }], 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .unwrap_or(0)
        } else {
            args[0]
        };
        let r_out = if is_create {
            self.w
                .read_u32(args[if stream { 1 } else { 2 }])
                .unwrap_or(0)
        } else {
            self.tr(args[0])
        };
        if is_create && result == 0 {
            // Alias the new object and its lease serial, using the C snapshot as the source of the C identity.
            let snap = if stream {
                self.w.host.stream_snapshot(c_obj)
            } else {
                self.w.host.buffer_snapshot(c_obj)
            };
            match snap {
                None => {
                    self.stats.entry(va).or_default().foreign += 1;
                    self.snapshot_missing += 1;
                    self.rebaseline_delta();
                    return;
                }
                Some(c_words) => {
                    let Some(o) = self.objs.find(r_out).cloned() else {
                        self.mismatch(
                            va,
                            &args[..],
                            format!("rust published {r_out:#x} but has no such object"),
                        );
                        return;
                    };
                    let tail = c_words.len() - if stream { 13 } else { 13 };
                    let c_serial = u64::from(c_words[tail]) | u64::from(c_words[tail + 1]) << 32;
                    self.serial_alias.insert(c_serial, o.lease.serial);
                    self.alias.insert(c_obj, r_out);
                    if !stream {
                        self.alias.insert(c_words[tail + 3], o.header_address);
                    }
                    // the C header shares the C internal device address words, aliased since DirectSoundCreate
                }
            }
        }
        self.compare_object(va, &args[..], c_obj, r_out, stream);
        self.stats.entry(va).or_default().compared += 1;
    }

    /// Snapshot, header words in guest memory and device reference word of one object after a call.
    fn compare_object(&mut self, va: u32, args: &[u32], c_obj: u32, r_obj: u32, stream: bool) {
        let snap = if stream {
            self.w.host.stream_snapshot(c_obj)
        } else {
            self.w.host.buffer_snapshot(c_obj)
        };
        let Some(c_words) = snap else {
            self.snapshot_missing += 1;
            return;
        };
        let Some(o) = self.objs.find(r_obj).cloned() else {
            self.mismatch(
                va,
                args,
                format!("rust has no object {r_obj:#x} (C {c_obj:#x})"),
            );
            return;
        };
        #[allow(unused_mut)]
        let mut body: Vec<(&'static str, u32)> = if stream {
            self.setters
                .stream(r_obj)
                .map(stream_body)
                .unwrap_or_default()
        } else {
            self.setters
                .buffer(r_obj)
                .map(buffer_body)
                .unwrap_or_default()
        };
        if body.is_empty() {
            self.mismatch(
                va,
                args,
                format!("rust setter state missing for {r_obj:#x}"),
            );
            return;
        }
        #[cfg(feature = "fault-inject")]
        if self.fault == Some(Fault::State) && self.calls_modelled % 5 == 0 {
            body[0].1 ^= 1;
        }
        let tail_len = if stream { 13 } else { 13 };
        if c_words.len() != body.len() + tail_len {
            self.mismatch(
                va,
                args,
                format!(
                    "snapshot length C {} vs rust {}",
                    c_words.len(),
                    body.len() + tail_len
                ),
            );
            return;
        }
        self.check(va);
        for (i, (name, rv)) in body.iter().enumerate() {
            if c_words[i] != *rv {
                self.mismatch(
                    va,
                    args,
                    format!(
                        "snapshot {name} (word {i}): C {:#x} vs rust {rv:#x}",
                        c_words[i]
                    ),
                );
                break;
            }
        }
        // identity tail: serial, public, [header address], header words
        let t = body.len();
        let c_serial = u64::from(c_words[t]) | u64::from(c_words[t + 1]) << 32;
        self.check(va);
        if self.trs(c_serial) != o.lease.serial {
            self.mismatch(
                va,
                args,
                format!("lease serial C {c_serial} vs rust {}", o.lease.serial),
            );
        }
        let mut header_start = t + 3;
        if !stream {
            header_start += 1;
            self.check(va);
            if self.tr(c_words[t + 3]) != o.header_address {
                self.mismatch(
                    va,
                    args,
                    format!(
                        "header address C {:#x} vs rust {:#x}",
                        c_words[t + 3],
                        o.header_address
                    ),
                );
            }
        }
        let c_header: Vec<u32> = c_words[header_start..]
            .iter()
            .map(|w| self.tr(*w))
            .collect();
        #[allow(unused_mut)]
        let mut r_header: Vec<u32> = o.header.clone();
        #[cfg(feature = "fault-inject")]
        if self.fault == Some(Fault::Header) && self.calls_modelled % 5 == 0 {
            r_header[0] ^= 1;
        }
        self.check(va);
        if c_header != r_header {
            self.mismatch(
                va,
                args,
                format!("snapshot header C {c_header:x?} vs rust {r_header:x?}"),
            );
        }
        // The same header as it sits in guest memory: C real memory against the Rust overlay.
        let n = o.bytes_len() as usize;
        let c_hdr_addr = c_obj.wrapping_sub(r_obj.wrapping_sub(o.header_address));
        let c_mem = self.w.read_real(c_hdr_addr, n);
        let r_mem = self.w.read_bytes(o.header_address, n);
        self.check(va);
        match (c_mem, r_mem) {
            (Some(c), Some(r)) => {
                let cw = pack_bytes(&c)
                    .into_iter()
                    .map(|w| self.tr(w))
                    .collect::<Vec<_>>();
                if cw != pack_bytes(&r) {
                    self.mismatch(
                        va,
                        args,
                        format!(
                            "guest header memory C {cw:x?} vs rust {:x?}",
                            pack_bytes(&r)
                        ),
                    );
                }
            }
            _ => self.mismatch(va, args, "header memory unreadable".into()),
        }
    }

    /// Per-VA table and totals, one line each.
    pub fn summary(&self) -> Vec<String> {
        let mut out = vec![format!(
            "audio-cmp: ---- summary: modelled calls {}, mismatches {} (hook problems {}), snapshots missing {}, hook records dropped {}, foreign-lease rebaselines {} (delta {})",
            self.calls_modelled,
            self.total_mismatches,
            self.hook_problems,
            self.snapshot_missing,
            self.records_dropped,
            self.rebaselines,
            self.delta
        )];
        out.push(format!(
            "audio-cmp: hook answers replayed [pause-note, routing, frequency, control-word, format, -, started, buffer-pause, buffer-frequency, voice-running, loop, -]: {:?}",
            self.hook_counts
        ));
        for (va, s) in &self.stats {
            out.push(format!(
                "audio-cmp: {va:#010x} {:<40} calls {:>6} compared {:>6} checks {:>7} mismatch {:>4} refusal {:>4} foreign {:>4} unmodelled {:>5}",
                name_of(*va),
                s.calls,
                s.compared,
                s.checks,
                s.mismatches,
                s.refusals,
                s.foreign,
                s.unmodelled
            ));
        }
        out
    }
}
