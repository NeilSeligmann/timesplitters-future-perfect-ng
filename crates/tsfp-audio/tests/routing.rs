//! Stream SetFrequency, SetHeadroom, SetMixBins, SetMixBinVolumes, the Volume routing note, SetFormat under
//! the completion model and the stream frame handlers (T2046 slice 9b). Tier 1 against the C source
//! (`dsound_stream.c`, `dsound_stream_routing.h`, `dsound_stream_frequency.c`): expected values are read off
//! the C branches or computed independently in Python from the C formula, not measured by running the C.

mod common;
use common::{World, put, words_bytes as words};
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::frames::*;
use tsfp_audio::lifecycle::*;
use tsfp_audio::routing::*;
use tsfp_audio::setters::*;

const OUT: u32 = 0x0010_0100;
const DESC: u32 = 0x0010_0200;
const FMT: u32 = 0x0010_0300;
const PAIRS: u32 = 0x0010_0600;
const COUNTED: u32 = 0x0010_0500;
const STACK: u32 = 0x0010_0900;
const CW: u16 = 0x027F;

fn format(rate: u32) -> Vec<u8> {
    let mut f = vec![0u8; 20];
    f[0..2].copy_from_slice(&0x69u16.to_le_bytes());
    f[2..4].copy_from_slice(&2u16.to_le_bytes());
    f[4..8].copy_from_slice(&rate.to_le_bytes());
    f[8..12].copy_from_slice(&((72 * rate) >> 6).to_le_bytes());
    f[12..14].copy_from_slice(&72u16.to_le_bytes());
    f[14..16].copy_from_slice(&4u16.to_le_bytes());
    f[16..18].copy_from_slice(&2u16.to_le_bytes());
    f[18..20].copy_from_slice(&64u16.to_le_bytes());
    f
}

#[derive(Default)]
struct Hooks {
    freq: bool,
    route: bool,
    fmt: bool,
    cw: Option<u16>,
    accept: bool,
    freq_calls: Vec<(u32, u32)>,
    route_calls: Vec<Routing>,
    fmt_calls: Vec<(u32, u32)>,
}
impl StreamHooks for Hooks {
    fn frequency_installed(&self) -> bool {
        self.freq
    }
    fn routing_installed(&self) -> bool {
        self.route
    }
    fn format_installed(&self) -> bool {
        self.fmt
    }
    fn control_word(&mut self) -> Option<u16> {
        self.cw
    }
    fn frequency_note(&mut self, _: u32, _: u64, old: u32, new: u32) -> bool {
        self.freq_calls.push((old, new));
        self.accept
    }
    fn routing_note(&mut self, _: u32, _: u64, c: &Routing) -> bool {
        self.route_calls.push(c.clone());
        self.accept
    }
    fn format_note(&mut self, _: u32, _: u64, old: u32, new: u32, _: u32) -> bool {
        self.fmt_calls.push((old, new));
        self.accept
    }
}
fn hooks() -> Hooks {
    Hooks {
        freq: true,
        route: true,
        fmt: true,
        cw: Some(CW),
        accept: true,
        ..Hooks::default()
    }
}

struct Fx {
    w: World,
    d: Device,
    objs: Objects,
    s: Setters,
}
fn fixture(completion: bool) -> Fx {
    let mut w = World::new();
    let mut d = Device::new();
    assert_eq!(d.create(&mut w, 0, OUT, 0, false, true).ok(), Some(0));
    Fx {
        w,
        d,
        objs: Objects::new(),
        s: Setters::new(Policy {
            enabled: true,
            completion,
            irql0: true,
        }),
    }
}
impl Fx {
    fn stream(&mut self, flags: u32) -> u32 {
        put(&mut self.w, DESC, &words(&[flags, 3, FMT, 0, 0, 0]));
        let mut f = format(44100);
        if flags != 0 {
            f[2] = 1;
            f[12] = 36;
            let avg = (36u32 * 44100) >> 6;
            f[8..12].copy_from_slice(&avg.to_le_bytes());
        }
        put(&mut self.w, FMT, &f);
        let out = OUT + 8 + 4 * self.objs.live.len() as u32;
        let r = self
            .s
            .create_stream(&mut self.objs, &mut self.d, &mut self.w, DESC, out);
        assert_eq!(r, Ok(0));
        self.w.read_u32(out).unwrap()
    }
    fn start(&mut self, st: u32) {
        let (o, d, w) = (&self.objs, &self.d, &self.w);
        let v = self.s.stream_volume(o, d, w, st, VOLUME_STARTUP, false);
        assert_eq!(v.ok(), Some(0));
        let p = self.s.stream_pause(o, d, w, st, 1, false, &mut |_, _| {});
        assert_eq!(p.ok(), Some(0));
        assert_eq!(self.s.stream_flush_ex(o, d, w, st, 0, 0, 1).ok(), Some(0));
        assert_eq!(self.s.stream_discontinuity(o, d, w, st).ok(), Some(0));
    }
}

#[test]
fn pitch_matches_the_c_formula() {
    // values computed in Python from the C formula (float32 reciprocal 0x37AEC33E, 4096*log2, nearest even)
    for (hz, pitch) in [
        (8000, -10588),
        (11025, -8693),
        (22050, -4597),
        (32000, -2396),
        (44100, -501),
        (48000, 0),
        (96000, 4096),
    ] {
        assert_eq!(frequency_pitch(hz, 0x027F), Some(pitch), "{hz}");
    }
    for cw in [0x003F, 0x027F, 0x037F] {
        assert_eq!(frequency_pitch(22050, cw), Some(-4597));
    }
    assert_eq!(frequency_pitch(22050, 0x0000), None, "unknown control word");
    assert_eq!(frequency_pitch(7999, CW), None);
    assert_eq!(frequency_pitch(96001, CW), None);
}

#[test]
fn pitch_has_no_rounding_ties_so_f64_equals_the_c_long_double() {
    let r = f32::from_bits(0x37AE_C33E);
    let mut nearest = f64::MAX;
    for hz in 8000u32..=96000 {
        let n = (f64::from(hz) * f64::from(r)) as f32;
        let x = 4096.0 * f64::from(n).log2();
        nearest = nearest.min(((x - x.floor()) - 0.5).abs());
    }
    assert!(nearest > 1e-6, "closest tie margin {nearest}");
}

#[test]
fn routing_arithmetic_is_modulo_32_and_transactional() {
    let mut r = Routing::default();
    assert_eq!(
        (r.headroom, r.aggregate, r.count),
        (600, 0u32.wrapping_sub(600), 2)
    );
    assert_eq!(r.attenuation(0), 384, "(600<<6)/100");
    assert_eq!(r.attenuation(2), 4095, "slot past count");
    r.set_headroom(0);
    assert_eq!(r.aggregate, 0);
    assert_eq!(r.attenuation(0), 0);
    r.set_volume(0xFFFF_D8F0);
    assert_eq!(r.attenuation(0), 4095, "cap 0FFF");
    // bins: byte identity, duplicates share the last volume, bad bin leaves state untouched
    let before = r.clone();
    assert!(!r.set_bins(&[(0, 5), (6, 7)], false));
    assert_eq!(r, before);
    assert!(!r.set_bins(&[(0, 0); 9], false), "at most 8 routes");
    assert!(r.set_bins(&[(2, 10), (2, 20), (0x102, 30)], false));
    assert_eq!(
        (r.count, r.bins[..3].to_vec(), r.bin_volume[2]),
        (3, vec![2, 2, 2], 30)
    );
    // defaults restore 0/1 and zero their volumes, other entries stay
    r.bin_volume[4] = 9;
    assert!(r.set_bins(&[], true));
    assert_eq!(
        (
            r.count,
            r.bins[0],
            r.bins[1],
            r.bin_volume[0],
            r.bin_volume[4]
        ),
        (2, 0, 1, 0, 9)
    );
    // empty explicit input keeps volumes and sets count 0
    assert!(r.set_bins(&[], false));
    assert_eq!(r.count, 0);
    // volumes use the full DWORD bin
    assert!(!r.set_bin_volumes(&[(32, 1)]));
    assert!(!r.set_bin_volumes(&[(0x100, 1)]));
    assert!(r.set_bin_volumes(&[(31, 7)]));
    assert_eq!(r.bin_volume[31], 7);
}

#[test]
fn set_frequency_needs_policy_a_started_stream_and_the_note() {
    let mut f = fixture(true);
    let st = f.stream(0);
    let mut h = hooks();
    let call = |f: &mut Fx, h: &mut Hooks, hz: u32, cw: u16| {
        f.s.stream_frequency(&f.objs, &f.d, &f.w, st, hz, cw, h)
    };
    assert!(call(&mut f, &mut h, 22050, CW).is_err(), "not started");
    f.start(st);
    let mut off = Hooks::default();
    assert!(
        call(&mut f, &mut off, 22050, CW).is_err(),
        "no frequency note installed"
    );
    assert!(
        call(&mut f, &mut h, 22050, 0x1234).is_err(),
        "bad control word"
    );
    assert!(
        call(&mut f, &mut h, 7000, CW).is_err(),
        "rate outside domain"
    );
    h.accept = false;
    assert!(call(&mut f, &mut h, 22050, CW).is_err(), "note refused");
    assert_eq!(
        f.s.stream(st).unwrap().frequency_sets,
        0,
        "refusal leaves state"
    );
    h.accept = true;
    assert_eq!(call(&mut f, &mut h, 22050, CW).ok(), Some(0));
    let s = f.s.stream(st).unwrap();
    assert_eq!(
        (s.source_rate_hz, s.frequency_pitch, s.frequency_sets),
        (22050, -4597, 1)
    );
    assert_eq!(
        h.freq_calls.last(),
        Some(&(44100, 22050)),
        "note sees old and new"
    );
    // hertz 0 means the stream rate, 44100 here
    assert_eq!(call(&mut f, &mut h, 0, CW).ok(), Some(0));
    assert_eq!(f.s.stream(st).unwrap().frequency_pitch, -501);
    // the original global failure precedes object access
    assert!(f.w.write_u32(GLOBAL_STATE, 1));
    assert_eq!(call(&mut f, &mut h, 22050, CW).ok(), Some(E_FAIL));
    assert_eq!(
        call(&mut f, &mut Hooks::default(), 22050, CW).ok(),
        None,
        "stop precedes E_FAIL"
    );
    // completion off is a stop
    f.s.policy.completion = false;
    assert!(f.w.write_u32(GLOBAL_STATE, 0));
    assert!(call(&mut f, &mut h, 22050, CW).is_err());
}

#[test]
fn spatial_streams_take_frequency_but_not_routing() {
    let mut f = fixture(true);
    let st = f.stream(0x10);
    // fully configured mono spatial stream: cache mask 7 plus the startup order
    put(
        &mut f.w,
        0x0010_0400,
        &words(&[0, 0, 0xFFFF_F448, 0, 0, 0, 0, 0, 0]),
    );
    let (o, d, w) = (&f.objs, &f.d, &f.w);
    assert_eq!(f.s.stream_i3dl2(o, d, w, st, 0x0010_0400, 0).ok(), Some(0));
    assert_eq!(
        f.s.stream_min_distance(o, d, w, st, 0x3F80_0000, 0).ok(),
        Some(0)
    );
    assert_eq!(
        f.s.stream_rolloff(o, d, w, st, STREAM_CURVE, 4, 0).ok(),
        Some(0)
    );
    f.start(st);
    let mut h = hooks();
    assert_eq!(
        f.s.stream_frequency(&f.objs, &f.d, &f.w, st, 32000, CW, &mut h)
            .ok(),
        Some(0)
    );
    assert!(
        f.s.stream_headroom(&f.objs, &f.d, &f.w, st, 0, &mut h)
            .is_err()
    );
    assert!(h.route_calls.is_empty(), "rejected before the note");
}

#[test]
fn routing_setters_edit_a_candidate_and_commit_only_when_the_note_accepts() {
    let mut f = fixture(true);
    let st = f.stream(0);
    let mut h = hooks();
    assert_eq!(
        f.s.stream_headroom(&f.objs, &f.d, &f.w, st, 0, &mut h).ok(),
        Some(0)
    );
    assert_eq!(f.s.stream(st).unwrap().routing.aggregate, 0);
    h.accept = false;
    assert!(
        f.s.stream_headroom(&f.objs, &f.d, &f.w, st, 5, &mut h)
            .is_err()
    );
    assert_eq!(
        f.s.stream(st).unwrap().routing.headroom,
        0,
        "note refusal keeps state"
    );
    h.accept = true;
    // SetMixBins {count 2, ptr}: pairs (bin, volume)
    put(&mut f.w, COUNTED, &words(&[2, PAIRS]));
    put(&mut f.w, PAIRS, &words(&[3, 100, 4, 200]));
    assert_eq!(
        f.s.stream_mix_bins(&f.objs, &f.d, &f.w, st, COUNTED, &mut h)
            .ok(),
        Some(0)
    );
    let r = &f.s.stream(st).unwrap().routing;
    assert_eq!(
        (
            r.count,
            r.bins[0],
            r.bins[1],
            r.bin_volume[3],
            r.bin_volume[4]
        ),
        (2, 3, 4, 100, 200)
    );
    // a bin above 5 is refused for SetMixBins (no DSP/HRTF route) but fine for SetMixBinVolumes below 32
    put(&mut f.w, PAIRS, &words(&[6, 1]));
    put(&mut f.w, COUNTED, &words(&[1, PAIRS]));
    assert!(
        f.s.stream_mix_bins(&f.objs, &f.d, &f.w, st, COUNTED, &mut h)
            .is_err()
    );
    assert_eq!(
        f.s.stream_mix_bin_volumes(&f.objs, &f.d, &f.w, st, COUNTED, &mut h)
            .ok(),
        Some(0)
    );
    assert_eq!(f.s.stream(st).unwrap().routing.bin_volume[6], 1);
    // NULL descriptor restores the defaults for SetMixBins, is refused for SetMixBinVolumes
    assert_eq!(
        f.s.stream_mix_bins(&f.objs, &f.d, &f.w, st, 0, &mut h).ok(),
        Some(0)
    );
    assert_eq!(
        f.s.stream(st).unwrap().routing.bins[..2].to_vec(),
        vec![0, 1]
    );
    assert!(
        f.s.stream_mix_bin_volumes(&f.objs, &f.d, &f.w, st, 0, &mut h)
            .is_err()
    );
    // count past the limit, null pointer, protected state
    put(&mut f.w, COUNTED, &words(&[9, PAIRS]));
    assert!(
        f.s.stream_mix_bins(&f.objs, &f.d, &f.w, st, COUNTED, &mut h)
            .is_err()
    );
    put(&mut f.w, COUNTED, &words(&[1, 0]));
    assert!(
        f.s.stream_mix_bins(&f.objs, &f.d, &f.w, st, COUNTED, &mut h)
            .is_err()
    );
    assert!(
        f.s.stream_mix_bins(&f.objs, &f.d, &f.w, st, GLOBAL_STATE, &mut h)
            .is_err()
    );
    assert!(
        f.s.stream_mix_bins(&f.objs, &f.d, &f.w, st, 0x7000_0000, &mut h)
            .is_err()
    );
    // global audio state nonzero answers E_FAIL before touching the object
    assert!(f.w.write_u32(GLOBAL_STATE, 1));
    assert_eq!(
        f.s.stream_headroom(&f.objs, &f.d, &f.w, st, 1, &mut h).ok(),
        Some(E_FAIL)
    );
    // routing without the completion model or the note is a stop
    assert!(f.w.write_u32(GLOBAL_STATE, 0));
    assert!(
        f.s.stream_headroom(&f.objs, &f.d, &f.w, st, 1, &mut Hooks::default())
            .is_err()
    );
}

#[test]
fn volume_offers_the_aggregate_to_the_routing_note() {
    let mut f = fixture(true);
    let st = f.stream(0);
    let mut h = hooks();
    let v =
        f.s.stream_volume_with(&f.objs, &f.d, &f.w, st, VOLUME_STARTUP, false, &mut h);
    assert_eq!(v.ok(), Some(0));
    // startup silence stores -10000, aggregate = -600 + -10000
    let c = h.route_calls.last().unwrap();
    assert_eq!((c.volume as i32, c.aggregate as i32), (-10000, -10600));
    assert_eq!(f.s.stream(st).unwrap().routing.aggregate as i32, -10600);
    // a refused note rejects the whole SetVolume
    f.start_rest(st);
    h.accept = false;
    let v =
        f.s.stream_volume_with(&f.objs, &f.d, &f.w, st, 0xFFFF_FB6F, false, &mut h);
    assert!(v.is_err());
    assert_eq!(f.s.stream(st).unwrap().volume, -10000);
    h.accept = true;
    let v =
        f.s.stream_volume_with(&f.objs, &f.d, &f.w, st, 0xFFFF_FB6F, false, &mut h);
    assert_eq!(v.ok(), Some(0));
    assert_eq!(f.s.stream(st).unwrap().routing.volume, 0xFFFF_FB6F);
}
impl Fx {
    fn start_rest(&mut self, st: u32) {
        let (o, d, w) = (&self.objs, &self.d, &self.w);
        assert_eq!(
            self.s
                .stream_pause(o, d, w, st, 1, false, &mut |_, _| {})
                .ok(),
            Some(0)
        );
        assert_eq!(self.s.stream_flush_ex(o, d, w, st, 0, 0, 1).ok(), Some(0));
        assert_eq!(self.s.stream_discontinuity(o, d, w, st).ok(), Some(0));
    }
}

#[test]
fn set_format_under_the_completion_model_takes_the_pitch_and_note() {
    let mut f = fixture(true);
    let st = f.stream(0);
    f.start(st);
    put(&mut f.w, FMT + 0x100, &format(32000));
    let at = FMT + 0x100;
    let mut h = hooks();
    h.cw = None;
    assert!(
        f.s.stream_set_format_with(&f.objs, &f.d, &f.w, st, at, &mut h)
            .is_err(),
        "control word unknown"
    );
    h.cw = Some(CW);
    h.accept = false;
    assert!(
        f.s.stream_set_format_with(&f.objs, &f.d, &f.w, st, at, &mut h)
            .is_err(),
        "note refused"
    );
    assert_eq!(f.s.stream(st).unwrap().format_sets, 0);
    h.accept = true;
    assert_eq!(
        f.s.stream_set_format_with(&f.objs, &f.d, &f.w, st, at, &mut h)
            .ok(),
        Some(0)
    );
    let s = f.s.stream(st).unwrap();
    assert_eq!(
        (s.format_sets, s.source_rate_hz, s.frequency_pitch),
        (1, 32000, -2396)
    );
    assert_eq!(h.fmt_calls, vec![(44100, 32000), (44100, 32000)]);
    // the legacy entry keeps denying the completion model (no control word source)
    assert!(f.s.stream_set_format(&f.objs, &f.d, &f.w, st, at).is_err());
}

#[test]
fn set_format_after_frequency_needs_the_note_even_without_completion() {
    let mut f = fixture(true);
    let st = f.stream(0);
    f.start(st);
    let mut h = hooks();
    assert_eq!(
        f.s.stream_frequency(&f.objs, &f.d, &f.w, st, 22050, CW, &mut h)
            .ok(),
        Some(0)
    );
    f.s.policy.completion = false;
    put(&mut f.w, FMT + 0x100, &format(32000));
    h.fmt = false;
    h.cw = Some(CW);
    // frequency_sets != 0 forces the pitch path, but without a format note installation the C has no control word
    assert!(
        f.s.stream_set_format_with(&f.objs, &f.d, &f.w, st, FMT + 0x100, &mut h)
            .is_err()
    );
    h.fmt = true;
    assert_eq!(
        f.s.stream_set_format_with(&f.objs, &f.d, &f.w, st, FMT + 0x100, &mut h)
            .ok(),
        Some(0)
    );
}

#[test]
fn pause_classes_follow_the_c_count() {
    let mut f = fixture(true);
    let st = f.stream(0);
    // class 1 (after SetFormat) needs a recorded SetFormat, class 2 needs a started stream
    let p = |f: &mut Fx, mode: u32, class: u32| {
        f.s.stream_pause_class(&f.objs, &f.d, &f.w, st, mode, class, &mut |_, _| {})
    };
    assert!(p(&mut f, 1, 2).is_err());
    assert!(p(&mut f, 1, 1).is_err(), "no SetFormat yet");
    f.start(st);
    assert_eq!(p(&mut f, 1, 0).ok(), Some(0));
    assert_eq!(p(&mut f, 1, 2).ok(), Some(0));
    assert!(p(&mut f, 0, 2).is_err(), "runtime caller is mode 1 only");
    assert_eq!(p(&mut f, 0, 0).ok(), Some(0), "resume under completion");
    put(&mut f.w, FMT + 0x100, &format(32000));
    let mut h = hooks();
    assert_eq!(
        f.s.stream_set_format_with(&f.objs, &f.d, &f.w, st, FMT + 0x100, &mut h)
            .ok(),
        Some(0)
    );
    assert_eq!(p(&mut f, 1, 1).ok(), Some(0));
}

fn frame(f: &mut Fx, caller: u32, args: &[u32]) {
    let mut v = vec![caller];
    v.extend_from_slice(args);
    put(&mut f.w, STACK, &words(&v));
}

#[test]
fn frame_handlers_admit_only_the_measured_callers() {
    let mut f = fixture(false);
    let mut h = Hooks::default();
    // CreateSoundStream from the measured caller, then from a stray one
    put(&mut f.w, DESC, &words(&[0, 3, FMT, 0, 0, 0]));
    put(&mut f.w, FMT, &format(44100));
    let out = OUT + 8;
    let call = |f: &mut Fx, h: &mut Hooks, entry: u32| {
        stream_frame(
            &mut f.s,
            &mut f.objs,
            &mut f.d,
            &mut f.w,
            h,
            entry,
            STACK,
            &mut |_, _| {},
        )
    };
    frame(&mut f, 0x29944, &[DESC, out]);
    assert!(
        call(&mut f, &mut h, CREATE).is_err(),
        "stray caller is a stop"
    );
    frame(&mut f, 0x29943, &[DESC, out]);
    assert_eq!(call(&mut f, &mut h, CREATE).ok(), Some(0));
    let st = f.w.read_u32(out).unwrap();
    // SetVolume from its two callers, the second needs a recorded SetFormat
    frame(&mut f, 0x29A67, &[st, VOLUME_STARTUP]);
    assert!(call(&mut f, &mut h, VOLUME).is_err());
    frame(&mut f, 0x29F68, &[st, VOLUME_STARTUP]);
    assert_eq!(call(&mut f, &mut h, VOLUME).ok(), Some(0));
    // Pause: the completion-only callers are stops until the completion model is on
    frame(&mut f, 0x29F13, &[st, 1]);
    assert!(call(&mut f, &mut h, PAUSE).is_err());
    frame(&mut f, 0x29B5A, &[st, 1]);
    assert_eq!(call(&mut f, &mut h, PAUSE).ok(), Some(0));
    f.s.policy.completion = true;
    frame(&mut f, 0x29F13, &[st, 1]);
    assert_eq!(call(&mut f, &mut h, PAUSE).ok(), Some(0));
    // FlushEx and unknown entries
    frame(&mut f, 0x29B69, &[st, 0, 0, 1]);
    assert_eq!(call(&mut f, &mut h, FLUSH_EX).ok(), Some(0));
    assert!(call(&mut f, &mut h, 0x0040_1234).is_err());
    // an unreadable stack is a stop, not a panic
    assert!(
        stream_frame(
            &mut f.s,
            &mut f.objs,
            &mut f.d,
            &mut f.w,
            &mut h,
            VOLUME,
            0x7000_0000,
            &mut |_, _| {}
        )
        .is_err()
    );
}

#[test]
fn frame_handlers_for_frequency_and_routing() {
    let mut f = fixture(true);
    let st = f.stream(0);
    f.start(st);
    let mut h = hooks();
    let call = |f: &mut Fx, h: &mut Hooks, entry: u32| {
        stream_frame(
            &mut f.s,
            &mut f.objs,
            &mut f.d,
            &mut f.w,
            h,
            entry,
            STACK,
            &mut |_, _| {},
        )
    };
    frame(&mut f, 0x29B36, &[st, 22050]);
    assert!(call(&mut f, &mut h, SET_FREQUENCY).is_err(), "only 0x29B35");
    frame(&mut f, 0x29B35, &[st, 22050]);
    assert_eq!(call(&mut f, &mut h, SET_FREQUENCY).ok(), Some(0));
    h.cw = None;
    assert!(
        call(&mut f, &mut h, SET_FREQUENCY).is_err(),
        "no control word"
    );
    h.cw = Some(CW);
    // routing wrappers have no caller restriction
    frame(&mut f, 0xDEAD_0000, &[st, 7]);
    assert_eq!(call(&mut f, &mut h, HEADROOM).ok(), Some(0));
    assert_eq!(f.s.stream(st).unwrap().routing.headroom, 7);
    frame(&mut f, 0xDEAD_0000, &[st, 0]);
    assert_eq!(call(&mut f, &mut h, MIX_BINS).ok(), Some(0));
    assert!(call(&mut f, &mut h, MIX_VOLUMES).is_err());
}

#[test]
fn caller_table_is_data() {
    let by = |e: u32| STREAM_FRAMES.iter().find(|s| s.entry == e).unwrap();
    assert!(admit(by(PAUSE), PAUSE_RUNTIME_CALLER, true));
    assert!(!admit(by(PAUSE), PAUSE_RUNTIME_CALLER, false));
    assert!(admit(by(MAX_DISTANCE), 0x29D86, false));
    assert!(!admit(by(MAX_DISTANCE), 0x29943, true));
    assert_eq!(by(SET_POSITION).args, 5);
}
