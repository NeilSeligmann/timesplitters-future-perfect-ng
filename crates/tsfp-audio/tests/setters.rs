//! Setter cache and SetBufferData conformance (T2046 9b): expectations are read off the C source
//! (`dsound_stream.c` / `dsound_buffer.c` `access_owned`, `dsound_*_scope.c`), Tier 1, not a guest run.

mod common;
use common::{World, put, words_bytes};
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::game_boundary::{
    AudioObject, AudioObjectKind, GuestAddress, Position3, RolloffCurve, SpatialParameters,
    SpatialStateSource,
};
use tsfp_audio::lifecycle::*;
use tsfp_audio::scope::*;
use tsfp_audio::setters::*;

const OUT: u32 = 0x0010_0100;
const DESC: u32 = 0x0010_0200;
const FMT: u32 = 0x0010_0300;
const I3DL2: u32 = 0x0010_0400;
const DATA: u32 = 0x0010_0800;
const SPATIAL_SLOT: u32 = 0x0058_35F0;
const ORDINARY_SLOT: u32 = 0x0058_18E8;

fn stream_format(channels: u16, rate: u32) -> Vec<u8> {
    let block = 36 * channels;
    let mut f = vec![0u8; 20];
    f[0..2].copy_from_slice(&0x69u16.to_le_bytes());
    f[2..4].copy_from_slice(&channels.to_le_bytes());
    f[4..8].copy_from_slice(&rate.to_le_bytes());
    f[8..12].copy_from_slice(&((u32::from(block) * rate) >> 6).to_le_bytes());
    f[12..14].copy_from_slice(&block.to_le_bytes());
    f[14..16].copy_from_slice(&4u16.to_le_bytes());
    f[16..18].copy_from_slice(&2u16.to_le_bytes());
    f[18..20].copy_from_slice(&64u16.to_le_bytes());
    f
}

fn stream_descriptor(flags: u32) -> Vec<u8> {
    words_bytes(&[flags, 3, FMT, 0, 0, 0])
}
fn buffer_descriptor(flags: u32) -> Vec<u8> {
    words_bytes(&[24, flags, 0, FMT, 0, 0])
}

struct Fixture {
    w: World,
    d: Device,
    objs: Objects,
    s: Setters,
}

fn fixture() -> Fixture {
    let mut w = World::new();
    let mut d = Device::new();
    assert_eq!(d.create(&mut w, 0, OUT, 0, false, true).ok(), Some(0));
    Fixture {
        w,
        d,
        objs: Objects::new(),
        s: Setters::new(Policy {
            enabled: true,
            completion: false,
            irql0: true,
        }),
    }
}

impl Fixture {
    fn stream(&mut self, flags: u32) -> u32 {
        let channels = if flags == 0 { 2 } else { 1 };
        put(&mut self.w, DESC, &stream_descriptor(flags));
        put(&mut self.w, FMT, &stream_format(channels, 44100));
        let out = OUT + 8 + 4 * self.objs.live.len() as u32;
        let r = self
            .s
            .create_stream(&mut self.objs, &mut self.d, &mut self.w, DESC, out);
        assert_eq!(r, Ok(0));
        self.w.read_u32(out).unwrap()
    }
    fn buffer(&mut self, flags: u32) -> u32 {
        put(&mut self.w, DESC, &buffer_descriptor(flags));
        put(&mut self.w, FMT, &stream_format(1, 44000));
        let slot = if flags == 0x10 {
            SPATIAL_SLOT
        } else {
            ORDINARY_SLOT
        };
        let iface = self.d.owned_address() + 8;
        let out = slot + 4 * self.objs.live.len() as u32;
        let r = self.s.create_buffer(
            &mut self.objs,
            &mut self.d,
            &mut self.w,
            iface,
            DESC,
            out,
            0,
        );
        assert_eq!(r, Ok(0));
        self.w.read_u32(out).unwrap()
    }
    /// Startup order of a stream: volume, Pause1, FlushEx(0,0,1), Discontinuity.
    fn start(&mut self, st: u32) {
        let (o, d, w) = (&self.objs, &self.d, &self.w);
        assert_eq!(
            self.s
                .stream_volume(o, d, w, st, VOLUME_STARTUP, false)
                .ok(),
            Some(0)
        );
        assert_eq!(
            self.s
                .stream_pause(o, d, w, st, 1, false, &mut |_, _| {})
                .ok(),
            Some(0)
        );
        assert_eq!(self.s.stream_flush_ex(o, d, w, st, 0, 0, 1).ok(), Some(0));
        assert_eq!(self.s.stream_discontinuity(o, d, w, st).ok(), Some(0));
    }
    fn spatial_stream_caches(&mut self, st: u32) {
        put(
            &mut self.w,
            I3DL2,
            &words_bytes(&[0, 0, 0xFFFF_F448, 0, 0, 0, 0, 0, 0]),
        );
        let (o, d, w) = (&self.objs, &self.d, &self.w);
        assert_eq!(self.s.stream_i3dl2(o, d, w, st, I3DL2, 0).ok(), Some(0));
        assert_eq!(
            self.s.stream_min_distance(o, d, w, st, 0x3F80_0000, 0).ok(),
            Some(0)
        );
        assert_eq!(
            self.s.stream_rolloff(o, d, w, st, STREAM_CURVE, 4, 0).ok(),
            Some(0)
        );
    }
    fn spatial_buffer_caches(&mut self, b: u32) {
        put(
            &mut self.w,
            I3DL2,
            &words_bytes(&[0, 0, 0xFFFF_F448, 0, 0, 0, 0, 0, 0]),
        );
        let (o, d, w) = (&self.objs, &self.d, &self.w);
        assert_eq!(self.s.buffer_i3dl2(o, d, w, b, I3DL2, 0).ok(), Some(0));
        assert_eq!(
            self.s.buffer_min_distance(o, d, w, b, 0x3F80_0000, 0).ok(),
            Some(0)
        );
        assert_eq!(
            self.s.buffer_rolloff(o, d, w, b, BUFFER_CURVE, 1, 0).ok(),
            Some(0)
        );
    }
}

#[test]
fn scope_parsing_matches_c_validators() {
    let mut w = World::new();
    put(&mut w, DESC, &stream_descriptor(0));
    put(&mut w, FMT, &stream_format(2, 44100));
    let s = stream_scope_snapshot(&w, DESC).expect("stereo scope");
    assert_eq!(
        (s.flags, s.max_packets, s.channels, s.block_align),
        (0, 3, 2, 72)
    );
    assert_eq!(s.average_bytes_per_second, 49612, "(72*44100)>>6, floored");
    assert_eq!(s.descriptor_address, DESC);
    // mono spatial: flags 0x10, block 36
    put(&mut w, DESC, &stream_descriptor(0x10));
    put(&mut w, FMT, &stream_format(1, 44100));
    assert_eq!(
        stream_scope_snapshot(&w, DESC).map(|s| s.block_align),
        Some(36)
    );
    // refusals: wrong flags, packets, rate, channel count for the flags, nonzero reserved words
    let mut d = stream_descriptor(0x10);
    d[0] = 3;
    assert!(
        stream_scope_validate(&d, &stream_format(1, 44100)).is_none(),
        "flags 3"
    );
    let mut d = stream_descriptor(0);
    d[4] = 4;
    assert!(
        stream_scope_validate(&d, &stream_format(2, 44100)).is_none(),
        "4 packets"
    );
    assert!(
        stream_scope_validate(&stream_descriptor(0), &stream_format(2, 22050)).is_none(),
        "rate"
    );
    assert!(
        stream_scope_validate(&stream_descriptor(0), &stream_format(1, 44100)).is_none(),
        "mono for flags 0"
    );
    let mut d = stream_descriptor(0);
    d[12] = 1;
    assert!(
        stream_scope_validate(&d, &stream_format(2, 44100)).is_none(),
        "callback word"
    );
    assert!(
        stream_scope_validate(&stream_descriptor(0)[..23], &stream_format(2, 44100)).is_none(),
        "short"
    );
    // the format may not overlap the descriptor
    put(&mut w, DESC, &words_bytes(&[0, 3, DESC + 8, 0, 0, 0]));
    assert!(
        stream_scope_snapshot(&w, DESC).is_none(),
        "format overlaps descriptor"
    );
    // buffer: 44000 Hz mono, size word 24, buffer bytes 0
    put(&mut w, DESC, &buffer_descriptor(0x10));
    put(&mut w, FMT, &stream_format(1, 44000));
    let b = buffer_scope_snapshot(&w, DESC).expect("buffer scope");
    assert_eq!(
        (
            b.flags,
            b.sample_rate,
            b.block_align,
            b.average_bytes_per_second
        ),
        (0x10, 44000, 36, 24750)
    );
    put(&mut w, DESC, &words_bytes(&[24, 0, 4096, FMT, 0, 0]));
    assert!(
        buffer_scope_snapshot(&w, DESC).is_none(),
        "nonzero buffer bytes"
    );
    put(&mut w, DESC, &words_bytes(&[20, 0, 0, FMT, 0, 0]));
    assert!(
        buffer_scope_snapshot(&w, DESC).is_none(),
        "descriptor size word"
    );
    put(&mut w, DESC, &buffer_descriptor(0));
    put(&mut w, FMT, &stream_format(1, 44100));
    assert!(
        buffer_scope_snapshot(&w, DESC).is_none(),
        "buffers are 44000 Hz"
    );
}

#[test]
fn create_from_descriptor_refuses_out_of_scope_and_keeps_references() {
    let mut f = fixture();
    put(&mut f.w, DESC, &stream_descriptor(0));
    put(&mut f.w, FMT, &stream_format(1, 44100)); // mono for flags 0 is outside the scope
    let r =
        f.s.create_stream(&mut f.objs, &mut f.d, &mut f.w, DESC, OUT + 8);
    assert!(r.is_err());
    assert_eq!((f.d.reference_count(), f.objs.live.len()), (6, 0));
    // a buffer needs the policy and the measured output slot window
    let iface = f.d.owned_address() + 8;
    put(&mut f.w, DESC, &buffer_descriptor(0));
    put(&mut f.w, FMT, &stream_format(1, 44000));
    assert!(
        f.s.create_buffer(&mut f.objs, &mut f.d, &mut f.w, iface, DESC, OUT + 16, 0)
            .is_err(),
        "output outside slots"
    );
    assert!(
        f.s.create_buffer(
            &mut f.objs,
            &mut f.d,
            &mut f.w,
            iface,
            DESC,
            ORDINARY_SLOT + 2,
            0
        )
        .is_err(),
        "unaligned slot"
    );
    assert!(
        f.s.create_buffer(
            &mut f.objs,
            &mut f.d,
            &mut f.w,
            iface,
            DESC,
            ORDINARY_SLOT,
            5
        )
        .is_err(),
        "aggregation"
    );
    f.s.policy.enabled = false;
    assert!(
        f.s.create_buffer(
            &mut f.objs,
            &mut f.d,
            &mut f.w,
            iface,
            DESC,
            ORDINARY_SLOT,
            0
        )
        .is_err(),
        "policy off"
    );
    f.s.policy.enabled = true;
    assert_eq!(f.d.reference_count(), 6);
}

#[test]
fn stereo_stream_startup_sequence_produces_the_adapter_view() {
    let mut f = fixture();
    let st = f.stream(0);
    assert_eq!(f.d.reference_count(), 7);
    let v = f.s.stream_view(&f.objs, &f.d, &f.w, st).unwrap();
    assert!(!v.volume_seen && !v.discontinuity_seen);
    assert_eq!((v.max_packets, v.serial), (3, 1));
    assert_eq!(
        &v.format[..],
        &stream_format(2, 44100)[..],
        "creation format until SetFormat"
    );
    // Pause before the volume is fine for mode 1, but FlushEx before Pause is refused
    {
        let (o, d, w) = (&f.objs, &f.d, &f.w);
        assert!(
            f.s.stream_flush_ex(o, d, w, st, 0, 0, 1).is_err(),
            "FlushEx needs Pause1"
        );
        assert!(
            f.s.stream_discontinuity(o, d, w, st).is_err(),
            "Discontinuity needs Pause and FlushEx"
        );
        assert!(
            f.s.stream_volume(o, d, w, st, 0, false).is_err(),
            "only -10000 at startup"
        );
    }
    f.start(st);
    let v = f.s.stream_view(&f.objs, &f.d, &f.w, st).unwrap();
    assert!(v.volume_seen && v.discontinuity_seen);
    let state = f.s.stream(st).unwrap();
    assert_eq!(
        (state.volume, state.pause_mode, state.flush_flags),
        (-10000, 1, 1)
    );
    // repeated Discontinuity is a no-op
    assert_eq!(
        f.s.stream_discontinuity(&f.objs, &f.d, &f.w, st).ok(),
        Some(0)
    );
    // a stream that is not owned is refused
    assert!(
        f.s.stream_discontinuity(&f.objs, &f.d, &f.w, st + 0x100)
            .is_err()
    );
    // stream setters do not apply to buffers
    let b = f.buffer(0);
    assert!(f.s.stream_discontinuity(&f.objs, &f.d, &f.w, b).is_err());
}

#[test]
fn startup_status_writes_bit_zero_for_started_stereo_stream() {
    let mut f = fixture();
    let st = f.stream(0);
    f.start(st);
    let out = OUT + 0x80;
    f.w.write_u32(out, 0xDEAD);
    assert_eq!(
        f.s.stream_startup_status(&f.objs, &f.d, &mut f.w, st, out)
            .ok(),
        Some(0)
    );
    assert_eq!(f.w.read_u32(out), Some(1));
    // the output may not alias the stream header
    assert!(
        f.s.stream_startup_status(&f.objs, &f.d, &mut f.w, st, st)
            .is_err()
    );
    assert!(
        f.s.stream_startup_status(&f.objs, &f.d, &mut f.w, st, 0)
            .is_err(),
        "null output"
    );
}

#[test]
fn spatial_stream_cache_order_is_enforced() {
    let mut f = fixture();
    let st = f.stream(0x10);
    put(
        &mut f.w,
        I3DL2,
        &words_bytes(&[0, 0, 0xFFFF_F448, 0, 0, 0, 0, 0, 0]),
    );
    {
        let (o, d, w) = (&f.objs, &f.d, &f.w);
        assert!(
            f.s.stream_min_distance(o, d, w, st, 0x3F80_0000, 0)
                .is_err(),
            "MinDistance before I3DL2"
        );
        assert!(
            f.s.stream_rolloff(o, d, w, st, STREAM_CURVE, 4, 0).is_err(),
            "curve before MinDistance"
        );
        assert!(
            f.s.stream_i3dl2(o, d, w, st, I3DL2, 1).is_err(),
            "apply must be 0"
        );
        put_wrong_i3dl2(&mut f.w);
    }
    {
        let (o, d, w) = (&f.objs, &f.d, &f.w);
        assert!(
            f.s.stream_i3dl2(o, d, w, st, I3DL2, 0).is_err(),
            "only the exact first cache"
        );
    }
    f.spatial_stream_caches(st);
    assert_eq!(f.s.stream(st).unwrap().cache_mask, 7);
    {
        let (o, d, w) = (&f.objs, &f.d, &f.w);
        assert!(
            f.s.stream_min_distance(o, d, w, st, 0x3F80_0000, 0)
                .is_err(),
            "a cache is set once"
        );
        // after the caches the position and max distance take apply=1 only
        assert_eq!(f.s.stream_position(o, d, w, st, [1, 2, 3], 1).ok(), Some(0));
        assert!(
            f.s.stream_position(o, d, w, st, [1, 2, 3], 0).is_err(),
            "apply0 enters the commit helper"
        );
        assert_eq!(
            f.s.stream_max_distance(o, d, w, st, 0x4120_0000, 1).ok(),
            Some(0)
        );
        // the sound update switches among the three 4 point curves
        assert_eq!(
            f.s.stream_rolloff(o, d, w, st, STREAM_CURVE_FAR, 4, 0).ok(),
            Some(0)
        );
    }
    let s = f.s.stream(st).unwrap();
    assert_eq!((s.position_bits, s.position_sets), ([1, 2, 3], 1));
    assert_eq!(
        (s.max_distance_bits, s.curve_address),
        (0x4120_0000, STREAM_CURVE_FAR)
    );
    // a stereo stream refuses spatial setters
    let ster = f.stream(0);
    assert!(
        f.s.stream_position(&f.objs, &f.d, &f.w, ster, [0; 3], 1)
            .is_err()
    );
}

fn put_wrong_i3dl2(w: &mut World) {
    put(
        w,
        I3DL2,
        &words_bytes(&[1, 0, 0xFFFF_F448, 0, 0, 0, 0, 0, 0]),
    );
}

#[test]
fn global_failure_state_answers_e_fail_before_object_checks() {
    let mut f = fixture();
    let st = f.stream(0x10);
    f.spatial_stream_caches(st);
    f.w.write_u32(GLOBAL_STATE, 1);
    // position/max distance: E_FAIL even for an unowned address (the original checks the global first)
    assert_eq!(
        f.s.stream_position(&f.objs, &f.d, &f.w, 0x1234, [0; 3], 1)
            .ok(),
        Some(E_FAIL)
    );
    assert_eq!(
        f.s.stream_max_distance(&f.objs, &f.d, &f.w, st, 0, 1).ok(),
        Some(E_FAIL)
    );
    // the other setters use policy_error, which refuses (stops the guest)
    assert!(f.s.stream_discontinuity(&f.objs, &f.d, &f.w, st).is_err());
    f.w.write_u32(GLOBAL_STATE, 0);
    f.s.policy.irql0 = false;
    assert!(
        f.s.stream_position(&f.objs, &f.d, &f.w, st, [0; 3], 1)
            .is_err(),
        "unknown IRQL"
    );
}

#[test]
fn stream_set_format_accepts_only_the_adpcm_family_of_a_started_stream() {
    let mut f = fixture();
    let st = f.stream(0);
    let fmt = OUT + 0x200;
    put(&mut f.w, fmt, &stream_format(2, 32000));
    assert!(
        f.s.stream_set_format(&f.objs, &f.d, &f.w, st, fmt).is_err(),
        "not started"
    );
    f.start(st);
    assert_eq!(
        f.s.stream_set_format(&f.objs, &f.d, &f.w, st, fmt).ok(),
        Some(0)
    );
    let v = f.s.stream_view(&f.objs, &f.d, &f.w, st).unwrap();
    assert_eq!(
        &v.format[..],
        &stream_format(2, 32000)[..],
        "effective format is the SetFormat one"
    );
    assert_eq!(f.s.stream(st).unwrap().format_sets, 1);
    // average = rate*72/64 floored: 32000*72/64 = 36000
    assert_eq!(
        u32::from_le_bytes(v.format[8..12].try_into().unwrap()),
        36000
    );
    // out of range rate, mono for a stereo stream, bad average, unreadable address
    put(&mut f.w, fmt, &stream_format(2, 7999));
    assert!(f.s.stream_set_format(&f.objs, &f.d, &f.w, st, fmt).is_err());
    put(&mut f.w, fmt, &stream_format(1, 32000));
    assert!(f.s.stream_set_format(&f.objs, &f.d, &f.w, st, fmt).is_err());
    let mut bad = stream_format(2, 32000);
    bad[8] ^= 1;
    put(&mut f.w, fmt, &bad);
    assert!(f.s.stream_set_format(&f.objs, &f.d, &f.w, st, fmt).is_err());
    assert!(
        f.s.stream_set_format(&f.objs, &f.d, &f.w, st, 0x7000_0000)
            .is_err()
    );
    // the format input may not alias the stream header
    assert!(f.s.stream_set_format(&f.objs, &f.d, &f.w, st, st).is_err());
}

#[test]
fn stream_runtime_volume_and_pause_need_the_completion_model() {
    let mut f = fixture();
    let st = f.stream(0);
    f.start(st);
    // without completion, a started stream keeps the startup silence rule
    assert!(
        f.s.stream_volume(&f.objs, &f.d, &f.w, st, 0xFFFF_FB6F, false)
            .is_err()
    );
    assert!(
        f.s.stream_pause(&f.objs, &f.d, &f.w, st, 0, false, &mut |_, _| {})
            .is_err(),
        "resume is completion only"
    );
    f.s.policy.completion = true;
    let mut noted = vec![];
    assert_eq!(
        f.s.stream_volume(&f.objs, &f.d, &f.w, st, 0xFFFF_FB6F, false)
            .ok(),
        Some(0)
    );
    assert_eq!(f.s.stream(st).unwrap().volume, 0xFFFF_FB6Fu32 as i32);
    assert_eq!(
        f.s.stream_pause(&f.objs, &f.d, &f.w, st, 0, true, &mut |a, m| noted
            .push((a, m)))
            .ok(),
        None,
        "runtime pause only mode 1"
    );
    assert_eq!(
        f.s.stream_pause(&f.objs, &f.d, &f.w, st, 0, false, &mut |a, m| noted
            .push((a, m)))
            .ok(),
        Some(0)
    );
    assert_eq!(noted, vec![(st, 0)]);
    // after SetFormat is recorded, SetVolume from the runtime caller needs it
    assert!(
        f.s.stream_volume(&f.objs, &f.d, &f.w, st, 0, true).is_err(),
        "needs a recorded SetFormat"
    );
    // SetFormat under the completion model needs the pitch note (open)
    let fmt = OUT + 0x200;
    put(&mut f.w, fmt, &stream_format(2, 32000));
    assert!(f.s.stream_set_format(&f.objs, &f.d, &f.w, st, fmt).is_err());
}

#[test]
fn buffer_start_order_and_view() {
    let mut f = fixture();
    let b = f.buffer(0);
    put(&mut f.w, DATA, &vec![7u8; 72]);
    {
        let (o, d, w) = (&f.objs, &f.d, &f.w);
        assert!(
            f.s.buffer_volume(o, d, w, b, VOLUME_STARTUP, &mut NoHooks)
                .is_err(),
            "volume before data"
        );
        assert!(f.s.buffer_set_data(o, d, w, b, 0, 72).is_err(), "null data");
        assert!(
            f.s.buffer_set_data(o, d, w, b, DATA, 0).is_err(),
            "zero length"
        );
        assert!(
            f.s.buffer_set_data(o, d, w, b, DATA, DATA_MAX_BYTES + 1)
                .is_err(),
            "over 64 MiB"
        );
        assert!(
            f.s.buffer_set_data(o, d, w, b, 0x7000_0000, 72).is_err(),
            "unreadable"
        );
        assert!(
            f.s.buffer_set_data(o, d, w, b, b - 28, 36).is_err(),
            "aliases the buffer header"
        );
        assert!(
            f.s.buffer_set_data(o, d, w, b, SINGLETON, 4).is_err(),
            "aliases the singleton"
        );
        assert_eq!(f.s.buffer_set_data(o, d, w, b, DATA, 72).ok(), Some(0));
    }
    let v = f.s.buffer_view(&f.objs, &f.d, &f.w, b).unwrap();
    assert_eq!((v.data_address, v.data_length, v.data_sets), (DATA, 72, 1));
    assert_eq!(
        (v.loop_start, v.loop_length),
        (0, 72),
        "a new data pair resets the loop region to the whole data"
    );
    assert_eq!((v.block_align, v.flags, v.serial), (36, 0, 1));
    let (o, d, w) = (&f.objs, &f.d, &f.w);
    assert!(
        f.s.buffer_pause(o, d, w, b, 0, &mut NoHooks).is_err(),
        "Pause before volume"
    );
    assert!(
        f.s.buffer_frequency(o, d, w, b, FREQUENCY_STARTUP, &mut NoHooks)
            .is_err(),
        "frequency before pause"
    );
    assert!(
        f.s.buffer_volume(o, d, w, b, 5, &mut NoHooks).is_err(),
        "only -10000 or -3204 without completion"
    );
    assert!(
        f.s.buffer_volume(o, d, w, b, VOLUME_UPDATE, &mut NoHooks)
            .is_err(),
        "update volume needs a started buffer"
    );
    assert_eq!(
        f.s.buffer_volume(o, d, w, b, VOLUME_STARTUP, &mut NoHooks)
            .ok(),
        Some(0)
    );
    assert!(
        f.s.buffer_pause(o, d, w, b, 1, &mut NoHooks).is_err(),
        "only Pause(0) on a never started buffer"
    );
    assert_eq!(f.s.buffer_pause(o, d, w, b, 0, &mut NoHooks).ok(), Some(0));
    assert!(
        f.s.buffer_frequency(o, d, w, b, 44100, &mut NoHooks)
            .is_err(),
        "only 22042 without completion"
    );
    assert_eq!(
        f.s.buffer_frequency(o, d, w, b, FREQUENCY_STARTUP, &mut NoHooks)
            .ok(),
        Some(0)
    );
    assert_eq!(
        f.s.buffer_volume(o, d, w, b, VOLUME_UPDATE, &mut NoHooks)
            .ok(),
        Some(0),
        "update volume after the whole start"
    );
    let v = f.s.buffer_view(&f.objs, &f.d, &f.w, b).unwrap();
    assert_eq!(
        (
            v.volume,
            v.volume_sets,
            v.pause_sets,
            v.frequency,
            v.frequency_sets
        ),
        (-3204, 2, 1, 22042, 1)
    );
    // same data pair again keeps the loop region, a different one resets it
    assert_eq!(
        f.s.buffer_set_data(&f.objs, &f.d, &f.w, b, DATA, 72).ok(),
        Some(0)
    );
    assert_eq!(f.s.buffer(b).unwrap().data_sets, 2);
}

#[test]
fn spatial_buffer_caches_then_apply1_updates() {
    let mut f = fixture();
    let b = f.buffer(0x10);
    {
        let (o, d, w) = (&f.objs, &f.d, &f.w);
        assert!(
            f.s.buffer_set_data(o, d, w, b, DATA, 72).is_err(),
            "spatial data needs all three caches"
        );
        assert!(
            f.s.buffer_rolloff(o, d, w, b, BUFFER_CURVE, 1, 0).is_err(),
            "curve before the earlier caches"
        );
        assert!(
            f.s.buffer_min_distance(o, d, w, b, 0x3F80_0000, 1).is_err(),
            "apply 1 needs caches and data"
        );
    }
    f.spatial_buffer_caches(b);
    let (o, d, w) = (&f.objs, &f.d, &f.w);
    assert!(
        f.s.buffer_position(o, d, w, b, [1, 2, 3], 1).is_err(),
        "position needs recorded data"
    );
    assert_eq!(f.s.buffer_set_data(o, d, w, b, DATA, 36).ok(), Some(0));
    assert_eq!(f.s.buffer_position(o, d, w, b, [1, 2, 3], 1).ok(), Some(0));
    assert!(f.s.buffer_position(o, d, w, b, [1, 2, 3], 0).is_err());
    assert_eq!(
        f.s.buffer_max_distance(o, d, w, b, 0x4120_0000, 1).ok(),
        Some(0)
    );
    assert_eq!(
        f.s.buffer_min_distance(o, d, w, b, 0x4000_0000, 1).ok(),
        Some(0),
        "apply1 stores the raw value"
    );
    assert_eq!(
        f.s.buffer_rolloff(o, d, w, b, 0x1234, 2, 1).ok(),
        Some(0),
        "apply1 stores the raw curve"
    );
    let s = f.s.buffer(b).unwrap();
    assert_eq!(
        (s.position_bits, s.max_distance_bits, s.min_distance_bits),
        ([1, 2, 3], 0x4120_0000, 0x4000_0000)
    );
    assert_eq!(
        (
            s.curve_address,
            s.curve_count,
            s.min_distance_sets,
            s.rolloff_sets
        ),
        (0x1234, 2, 1, 1)
    );
    assert_eq!(
        f.s.buffer_view(&f.objs, &f.d, &f.w, b).unwrap().cache_mask,
        7
    );
    // E_FAIL precedes object checks for position
    f.w.write_u32(GLOBAL_STATE, 9);
    assert_eq!(
        f.s.buffer_position(&f.objs, &f.d, &f.w, 0x99, [0; 3], 1)
            .ok(),
        Some(E_FAIL)
    );
}

#[test]
fn owned_spatial_projection_keeps_bits_and_rejects_recycled_object_identity() {
    let mut f = fixture();
    let stream = f.stream(0x10);
    f.spatial_stream_caches(stream);
    let position = [0x3f80_0000, 0xc000_0000, 0x7fc0_1234];
    let (o, d, w) = (&f.objs, &f.d, &f.w);
    assert_eq!(f.s.stream_position(o, d, w, stream, position, 1), Ok(0));
    assert_eq!(
        f.s.stream_max_distance(o, d, w, stream, 0x4120_0000, 1),
        Ok(0)
    );
    let serial = f.s.stream(stream).unwrap().serial;
    assert_eq!(
        f.s.spatial_state(
            AudioObjectKind::Stream,
            AudioObject {
                address: GuestAddress(stream),
                serial
            },
        ),
        Some(SpatialParameters {
            position: Some(Position3 { xyz_bits: position }),
            min_distance_bits: Some(0x3f80_0000),
            max_distance_bits: Some(0x4120_0000),
            rolloff: Some(RolloffCurve::StreamDefault),
            i3dl2_source_words: Some([0, 0, 0xffff_f448, 0, 0, 0, 0, 0, 0]),
        })
    );
    assert_eq!(
        f.s.spatial_state(
            AudioObjectKind::Stream,
            AudioObject {
                address: GuestAddress(stream),
                serial: serial + 1
            },
        ),
        None,
        "reused addresses must not expose the previous lease's spatial state"
    );
}

struct Hooks {
    started: bool,
    running: bool,
    calls: Vec<String>,
    accept: bool,
}
impl BufferHooks for Hooks {
    fn started(&mut self, _: u32, _: u64) -> bool {
        self.started
    }
    fn voice_running(&mut self, _: u32, _: u64) -> bool {
        self.running
    }
    fn volume(&mut self, _: u32, _: u64, v: i32) -> bool {
        self.calls.push(format!("volume {v}"));
        self.accept
    }
    fn pause(&mut self, _: u32, _: u64, m: u32) -> bool {
        self.calls.push(format!("pause {m}"));
        self.accept
    }
    fn frequency(&mut self, _: u32, _: u64, o: u32, n: u32) -> bool {
        self.calls.push(format!("frequency {o}->{n}"));
        self.accept
    }
    fn loop_region(&mut self, _: u32, _: u64, s: u32, l: u32, hz: u32) -> bool {
        self.calls.push(format!("loop {s} {l} @{hz}"));
        self.accept
    }
}

#[test]
fn buffer_completion_hooks_and_loop_regions() {
    let mut f = fixture();
    let b = f.buffer(0);
    put(&mut f.w, DATA, &vec![1u8; 144]);
    let mut h = Hooks {
        started: false,
        running: false,
        calls: vec![],
        accept: true,
    };
    assert!(
        f.s.buffer_loop_region(&f.objs, &f.d, &f.w, b, 0, 0, &mut h)
            .is_err(),
        "loop needs the completion model"
    );
    f.s.policy.completion = true;
    assert!(
        f.s.buffer_loop_region(&f.objs, &f.d, &f.w, b, 0, 0, &mut h)
            .is_err(),
        "loop needs recorded data"
    );
    let (o, d, w) = (&f.objs, &f.d, &f.w);
    assert_eq!(f.s.buffer_set_data(o, d, w, b, DATA, 144).ok(), Some(0));
    // completion lifts the volume list and the frequency clamp
    assert_eq!(
        f.s.buffer_volume(o, d, w, b, 0xFFFF_F000, &mut h).ok(),
        Some(0)
    );
    assert_eq!(f.s.buffer_pause(o, d, w, b, 0, &mut h).ok(), Some(0));
    assert_eq!(
        f.s.buffer_frequency(o, d, w, b, 0xBB, &mut h)
            .ok()
            .is_none(),
        true,
        "below the clamp"
    );
    assert_eq!(
        f.s.buffer_frequency(o, d, w, b, 44100, &mut h).ok(),
        Some(0)
    );
    assert_eq!(h.calls, vec!["volume -4096".to_string()]);
    // loop regions: 36 byte blocks only, whole-data default, past-the-end is 0x88780032
    assert_eq!(
        f.s.buffer_loop_region(o, d, w, b, 36, 72, &mut h).ok(),
        Some(0)
    );
    let s = f.s.buffer(b).unwrap();
    assert_eq!((s.loop_start, s.loop_length, s.loop_sets), (36, 72, 1));
    assert_eq!(
        f.s.buffer_loop_region(o, d, w, b, 36, 0, &mut h).ok(),
        Some(0),
        "length 0 means to the end"
    );
    assert_eq!(f.s.buffer(b).unwrap().loop_length, 108);
    assert_eq!(
        f.s.buffer_loop_region(o, d, w, b, 0, 180, &mut h).ok(),
        Some(PACKET_LIST_FULL)
    );
    assert!(
        f.s.buffer_loop_region(o, d, w, b, 10, 36, &mut h).is_err(),
        "unaligned start"
    );
    assert!(
        f.s.buffer_loop_region(o, d, w, b, 200, 0, &mut h).is_err(),
        "start past the data"
    );
    assert_eq!(
        f.s.buffer(b).unwrap().loop_sets,
        2,
        "refusals leave the record alone"
    );
    // a running voice consults the hook and a refusal leaves the old region
    h.started = true;
    h.running = true;
    h.accept = false;
    assert!(f.s.buffer_loop_region(o, d, w, b, 0, 36, &mut h).is_err());
    assert_eq!(f.s.buffer(b).unwrap().loop_start, 36);
    h.accept = true;
    assert_eq!(
        f.s.buffer_loop_region(o, d, w, b, 0, 36, &mut h).ok(),
        Some(0)
    );
    assert_eq!(h.calls.last().unwrap(), "loop 0 36 @44100");
    // pause/frequency on a played buffer go through the hooks
    assert_eq!(f.s.buffer_pause(o, d, w, b, 1, &mut h).ok(), Some(0));
    h.accept = false;
    assert!(f.s.buffer_pause(o, d, w, b, 2, &mut h).is_err());
    assert!(f.s.buffer_frequency(o, d, w, b, 30000, &mut h).is_err());
    assert_eq!(f.s.buffer(b).unwrap().frequency, 44100);
}

#[test]
fn views_follow_ownership_and_reset() {
    let mut f = fixture();
    let st = f.stream(0);
    let b = f.buffer(0);
    assert!(f.s.stream_view(&f.objs, &f.d, &f.w, st).is_some());
    assert!(
        f.s.buffer_view(&f.objs, &f.d, &f.w, st).is_none(),
        "wrong kind"
    );
    // tampering with the header word revokes the view (generation check)
    f.w.write_u32(st + 8, 99);
    assert!(f.s.stream_view(&f.objs, &f.d, &f.w, st).is_none());
    assert!(f.s.stream_discontinuity(&f.objs, &f.d, &f.w, st).is_err());
    f.w.write_u32(st + 8, 1);
    assert!(f.s.stream_view(&f.objs, &f.d, &f.w, st).is_some());
    assert!(f.objs.reset_checked(&mut f.d, &mut f.w));
    f.s.retain_live(&f.objs);
    assert!(f.s.stream(st).is_none() && f.s.buffer(b).is_none());
    assert_eq!((f.d.reference_count(), f.d.lease_count()), (6, 0));
    assert!(f.s.stream_view(&f.objs, &f.d, &f.w, st).is_none());
}
