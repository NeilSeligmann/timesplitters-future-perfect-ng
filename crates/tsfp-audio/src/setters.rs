//! Startup setter caches and SetBufferData (T2046 slice 9b): the Rust port of the `access_owned` setter
//! half of `dsound_stream.c` and `dsound_buffer.c`. These produce the [`StreamView`] / [`BufferView`] the
//! polled [`crate::adapter::DsAdapter`] reads. Over [`GuestWorld`], no C, no locks (single owner).
//!
//! Evidence class: Tier 1 vs the C source (branch conditions, error text and values copied, tests in
//! tests/setters.rs). Not run in a guest. The C `refuse()` stops are `Err(Refusal)`.
//!
//! Ported: stream I3DL2, MinDistance, MaxDistance, Position, Rolloff, Pause, FlushEx, Discontinuity,
//! Volume (with the routing note), SetFormat (startup family check and the completion pitch/format note),
//! SetFrequency, SetHeadroom, SetMixBins, SetMixBinVolumes (via [`crate::routing::StreamHooks`]), startup
//! GetStatus; buffer I3DL2, MinDistance, Rolloff, MaxDistance, Position, SetBufferData, SetVolume, Pause,
//! SetFrequency, SetLoopRegion (completion hooks via [`BufferHooks`]). The stream HLE frame handlers are in
//! [`crate::frames`]. NOT ported: the buffer frame handlers, the movie stream extension route.

use crate::adapter::{BufferView, Refusal, StreamView};
use crate::lifecycle::{Device, GLOBAL_STATE, GuestWorld, Kind, Object, Objects, SINGLETON};
use crate::routing::{ROUTE_BINS, ROUTE_SLOTS, Routing, StreamHooks, frequency_pitch};
use crate::scope::{
    BufferScope, FORMAT_BYTES, StreamScope, buffer_scope_snapshot, stream_scope_snapshot,
};
use std::collections::HashMap;

pub const STREAM_CURVE: u32 = 0x004B_914C;
pub const STREAM_CURVE_NEAR: u32 = 0x004B_915C;
pub const STREAM_CURVE_FAR: u32 = 0x004B_916C;
pub const BUFFER_CURVE: u32 = 0x0064_BC98;
pub const VOLUME_STARTUP: u32 = 0xFFFF_D8F0;
pub const VOLUME_UPDATE: u32 = 0xFFFF_F37C;
pub const FREQUENCY_STARTUP: u32 = 22042;
pub const FREQUENCY_MIN: u32 = 0xBC;
pub const FREQUENCY_MAX: u32 = 0x2EDEF;
pub const DATA_MAX_BYTES: u32 = 0x0400_0000;
pub const E_FAIL: u32 = 0x8000_4005;
pub const PACKET_LIST_FULL: u32 = 0x8878_0032;
const STREAM_SECONDARY_BYTES: u32 = 60;
const BUFFER_VTABLE_BYTES: u32 = 16;
const I3DL2_EXPECTED: [u32; 9] = [0, 0, 0xFFFF_F448, 0, 0, 0, 0, 0, 0];

/// The explicit policy switches (oracle: `enabled`, `completion`, the IRQL provider).
#[derive(Clone, Copy, Debug, Default)]
pub struct Policy {
    pub enabled: bool,
    pub completion: bool,
    /// The IRQL provider answered and the level is 0.
    pub irql0: bool,
}

/// Completion-model callbacks the buffer setters consult (oracle: the `*_hook` globals). Each returns
/// whether the completion model accepted the change.
pub trait BufferHooks {
    /// A Play started this buffer (oracle `started_query`).
    fn started(&mut self, _buffer: u32, _serial: u64) -> bool {
        false
    }
    fn voice_running(&mut self, _buffer: u32, _serial: u64) -> bool {
        false
    }
    fn volume(&mut self, _buffer: u32, _serial: u64, _volume: i32) -> bool {
        true
    }
    fn pause(&mut self, _buffer: u32, _serial: u64, _mode: u32) -> bool {
        false
    }
    fn frequency(&mut self, _buffer: u32, _serial: u64, _old: u32, _new: u32) -> bool {
        false
    }
    fn loop_region(
        &mut self,
        _buffer: u32,
        _serial: u64,
        _start: u32,
        _len: u32,
        _hz: u32,
    ) -> bool {
        false
    }
}

/// No completion model attached.
pub struct NoHooks;
impl BufferHooks for NoHooks {}

#[derive(Clone, Debug)]
pub struct StreamState {
    pub scope: StreamScope,
    pub serial: u64,
    pub cache_mask: u32,
    pub i3dl2: [u32; 9],
    pub i3dl2_address: u32,
    pub min_distance_bits: u32,
    pub max_distance_seen: bool,
    pub max_distance_bits: u32,
    pub curve_address: u32,
    pub curve_count: u32,
    pub pause_seen: bool,
    pub pause_mode: u32,
    pub flush_seen: bool,
    pub flush_time_low: u32,
    pub flush_time_high: u32,
    pub flush_flags: u32,
    pub discontinuity_seen: bool,
    pub volume_seen: bool,
    pub volume: i32,
    pub format_sets: u32,
    pub source_rate_hz: u32,
    pub format: [u8; FORMAT_BYTES],
    pub position_bits: [u32; 3],
    pub position_sets: u32,
    pub routing: Routing,
    pub frequency_pitch: i32,
    pub frequency_sets: u32,
}

#[derive(Clone, Debug)]
pub struct BufferState {
    pub scope: BufferScope,
    pub serial: u64,
    pub cache_mask: u32,
    pub i3dl2: [u32; 9],
    pub i3dl2_address: u32,
    pub min_distance_bits: u32,
    pub curve_address: u32,
    pub curve_count: u32,
    pub data_address: u32,
    pub data_length: u32,
    pub data_sets: u32,
    pub loop_start: u32,
    pub loop_length: u32,
    pub loop_sets: u32,
    pub volume: i32,
    pub volume_sets: u32,
    pub pause_sets: u32,
    pub frequency: u32,
    pub frequency_sets: u32,
    pub max_distance_bits: u32,
    pub max_distance_sets: u32,
    pub min_distance_sets: u32,
    pub rolloff_sets: u32,
    pub position_bits: [u32; 3],
    pub position_sets: u32,
}

use crate::guest_ranges::overlaps as overlap;

fn le_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// The stream and buffer setter state, keyed by public interface address.
#[derive(Default)]
pub struct Setters {
    pub policy: Policy,
    streams: HashMap<u32, StreamState>,
    buffers: HashMap<u32, BufferState>,
}

impl Setters {
    pub fn new(policy: Policy) -> Self {
        Self {
            policy,
            ..Self::default()
        }
    }
    pub fn stream(&self, public: u32) -> Option<&StreamState> {
        self.streams.get(&public)
    }
    pub fn buffer(&self, public: u32) -> Option<&BufferState> {
        self.buffers.get(&public)
    }

    /// CreateSoundStream: parse the descriptor scope then create the passive header (oracle
    /// `dsound_stream_create`). Scope refusal is `Err`, OOM is `Ok(0x8007000E)`.
    pub fn create_stream<W: GuestWorld>(
        &mut self,
        objs: &mut Objects,
        device: &mut Device,
        w: &mut W,
        descriptor: u32,
        output: u32,
    ) -> Result<u32, Refusal> {
        let scope = stream_scope_snapshot(&*w, descriptor)
            .ok_or(Refusal("descriptor/format outside measured startup scope"))?;
        let status = objs.create(device, w, Kind::Stream, scope.lifecycle(), output)?;
        if status == 0 {
            let public = w
                .read_u32(output)
                .ok_or(Refusal("published output vanished"))?;
            let serial = objs.find(public).map_or(0, |o| o.lease.serial);
            self.streams.insert(public, new_stream(scope, serial));
        }
        Ok(status)
    }

    /// CreateSoundBuffer (device interface, descriptor, output, outer). Oracle `dsound_buffer_create`.
    pub fn create_buffer<W: GuestWorld>(
        &mut self,
        objs: &mut Objects,
        device: &mut Device,
        w: &mut W,
        interface: u32,
        descriptor: u32,
        output: u32,
        outer: u32,
    ) -> Result<u32, Refusal> {
        let scope = buffer_scope_snapshot(&*w, descriptor)
            .ok_or(Refusal("descriptor/format outside measured startup scope"))?;
        self.policy_error(&*w)?;
        if outer != 0 {
            return Err(Refusal("aggregation is unsupported"));
        }
        let owner = w
            .read_u32(SINGLETON)
            .filter(|&d| d != 0 && d <= u32::MAX - 8)
            .ok_or(Refusal("no owned SILENT device"))?;
        let base = if scope.flags == 16 {
            0x0058_35F0
        } else {
            0x0058_18E8
        };
        if interface != owner + 8
            || !(output >= base && output - base < 160 && (output - base) % 4 == 0)
        {
            return Err(Refusal("device/output outside measured startup scope"));
        }
        let status = objs.create(device, w, Kind::Buffer, scope.lifecycle(), output)?;
        if status == 0 {
            let public = w
                .read_u32(output)
                .ok_or(Refusal("published output vanished"))?;
            let serial = objs.find(public).map_or(0, |o| o.lease.serial);
            self.buffers.insert(public, new_buffer(scope, serial));
        }
        Ok(status)
    }

    /// Drop the setter state of objects `Objects::reset_checked` removed.
    pub fn retain_live(&mut self, objs: &Objects) {
        self.streams.retain(|k, _| objs.find(*k).is_some());
        self.buffers.retain(|k, _| objs.find(*k).is_some());
    }

    /// Oracle `policy_error`.
    fn policy_error<W: GuestWorld + ?Sized>(&self, w: &W) -> Result<(), Refusal> {
        if !self.policy.enabled {
            return Err(Refusal("explicit headless policy is disabled"));
        }
        if !self.policy.irql0 {
            return Err(Refusal("only known IRQL0 is supported"));
        }
        if w.read_u32(GLOBAL_STATE) != Some(0) {
            return Err(Refusal("original global audio state must be zero"));
        }
        Ok(())
    }

    /// Oracle `spatial_preflight` / `setter_preflight`: the original global failure precedes object access.
    /// `Ok(Some(E_FAIL))` means the original answers E_FAIL, `Ok(None)` continues.
    fn preflight<W: GuestWorld + ?Sized>(
        &self,
        w: &W,
        needs_completion: bool,
    ) -> Result<Option<u32>, Refusal> {
        if !self.policy.enabled
            || (needs_completion && !self.policy.completion)
            || !self.policy.irql0
        {
            return Err(Refusal("setter needs explicit policy at known IRQL0"));
        }
        let state = w
            .read_u32(GLOBAL_STATE)
            .ok_or(Refusal("global audio state inaccessible"))?;
        Ok((state != 0).then_some(E_FAIL))
    }

    fn owned<'a, W: GuestWorld + ?Sized>(
        objs: &'a Objects,
        device: &Device,
        w: &W,
        kind: Kind,
        public: u32,
    ) -> Result<&'a Object, Refusal> {
        let o = objs
            .find(public)
            .filter(|o| o.kind == kind)
            .ok_or(Refusal("object is not owned by passive adapter"))?;
        if !objs.header_intact(w, o) || !device.validate_owned(w, o.lease.internal_address + 8) {
            return Err(Refusal("object ownership/header/generation changed"));
        }
        Ok(o)
    }

    /// Oracle `aliases_headers`: protected state for caller input buffers of one kind.
    fn aliases_headers(
        &self,
        objs: &Objects,
        kind: Kind,
        address: u32,
        bytes: u32,
        internal: u32,
    ) -> bool {
        if overlap(address, bytes, internal, 44)
            || overlap(address, bytes, SINGLETON, 4)
            || overlap(address, bytes, GLOBAL_STATE, 4)
        {
            return true;
        }
        let (vt, vtn) = match kind {
            Kind::Stream => (crate::lifecycle::STREAM_SECONDARY, STREAM_SECONDARY_BYTES),
            Kind::Buffer => (crate::lifecycle::BUFFER_VTABLE, BUFFER_VTABLE_BYTES),
        };
        overlap(address, bytes, vt, vtn)
            || objs.live.iter().filter(|o| o.kind == kind).any(|o| {
                overlap(address, bytes, o.header_address, o.bytes_len())
                    || overlap(address, bytes, o.publication_address, 4)
            })
    }

    /// Oracle buffer `aliases_state` (headers plus the shared curve word) for SetBufferData data.
    fn buffer_aliases_state(
        &self,
        objs: &Objects,
        address: u32,
        bytes: u32,
        internal: u32,
    ) -> bool {
        self.aliases_headers(objs, Kind::Buffer, address, bytes, internal)
            || overlap(address, bytes, BUFFER_CURVE, 4)
    }

    // ----------------------------------------------------------------- streams

    fn stream_ctx<'a, W: GuestWorld + ?Sized>(
        &self,
        objs: &'a Objects,
        device: &Device,
        w: &W,
        stream: u32,
    ) -> Result<(&'a Object, u32), Refusal> {
        let o = Self::owned(objs, device, w, Kind::Stream, stream)?;
        self.policy_error(w)?;
        Ok((o, o.lease.internal_address))
    }

    /// IDirectSoundStream_SetI3DL2Source cache (stream, parameters, apply).
    pub fn stream_i3dl2<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        parameters: u32,
        apply: u32,
    ) -> Result<u32, Refusal> {
        let (_, internal) = self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get(&stream)
            .ok_or(Refusal("no stream state"))?;
        if st.scope.flags != 0x10 || apply != 0 {
            return Err(Refusal("only startup spatial setters/apply0 are supported"));
        }
        let words = w.read_bytes(parameters, 36).map(|b| le_words(&b));
        if st.cache_mask != 0
            || self.aliases_headers(objs, Kind::Stream, parameters, 36, internal)
            || words.as_deref() != Some(&I3DL2_EXPECTED[..])
        {
            return Err(Refusal("only exact first startup I3DL2 cache is supported"));
        }
        let st = self.streams.get_mut(&stream).expect("checked");
        st.i3dl2 = I3DL2_EXPECTED;
        st.i3dl2_address = parameters;
        st.cache_mask = 1;
        Ok(0)
    }

    /// SetMinDistance cache (stream, float bits, apply).
    pub fn stream_min_distance<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        bits: u32,
        apply: u32,
    ) -> Result<u32, Refusal> {
        self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        if st.scope.flags != 0x10 || apply != 0 {
            return Err(Refusal("only startup spatial setters/apply0 are supported"));
        }
        if st.cache_mask != 1 || bits != 0x3F80_0000 {
            return Err(Refusal("only ordered startup minDistance1.0 is supported"));
        }
        st.min_distance_bits = bits;
        st.cache_mask = 3;
        Ok(0)
    }

    /// SetRolloffCurve cache (stream, curve, count, apply): the ordered startup cache (apply 0) or the
    /// sound update's switch between the three 4 point curves (T1199).
    pub fn stream_rolloff<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        curve: u32,
        count: u32,
        apply: u32,
    ) -> Result<u32, Refusal> {
        self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        let switch = st.scope.flags == 0x10
            && st.cache_mask == 7
            && apply == 0
            && count == 4
            && [STREAM_CURVE, STREAM_CURVE_NEAR, STREAM_CURVE_FAR].contains(&curve);
        if switch {
            st.curve_address = curve;
            st.curve_count = count;
            return Ok(0);
        }
        if st.scope.flags != 0x10 || apply != 0 {
            return Err(Refusal("only startup spatial setters/apply0 are supported"));
        }
        if st.cache_mask != 3 || curve != STREAM_CURVE || count != 4 {
            return Err(Refusal("only ordered startup curve VA/count4 is supported"));
        }
        st.curve_address = curve;
        st.curve_count = count;
        st.cache_mask = 7;
        Ok(0)
    }

    /// SetMaxDistance (stream, float bits, apply).
    pub fn stream_max_distance<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        bits: u32,
        apply: u32,
    ) -> Result<u32, Refusal> {
        if let Some(fail) = self.preflight(w, false)? {
            return Ok(fail);
        }
        self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        if st.scope.flags != 0x10 || st.cache_mask != 7 {
            return Err(Refusal(
                "only a fully configured spatial startup stream may record SetMaxDistance",
            ));
        }
        if apply != 1 {
            return Err(Refusal(
                "SetMaxDistance apply=0 enters the unmodeled settings commit helper",
            ));
        }
        st.max_distance_seen = true;
        st.max_distance_bits = bits;
        Ok(0)
    }

    /// SetPosition (stream, x, y, z bits, apply).
    pub fn stream_position<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        xyz: [u32; 3],
        apply: u32,
    ) -> Result<u32, Refusal> {
        if let Some(fail) = self.preflight(w, false)? {
            return Ok(fail);
        }
        self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        if st.scope.flags != 0x10 || st.cache_mask != 7 {
            return Err(Refusal(
                "only a fully configured spatial startup stream may record SetPosition",
            ));
        }
        if apply != 1 {
            return Err(Refusal(
                "SetPosition apply=0 enters the unmodeled settings commit helper",
            ));
        }
        st.position_bits = xyz;
        st.position_sets += 1;
        Ok(0)
    }

    fn stream_shape(st: &StreamState) -> bool {
        (st.scope.flags == 0 && st.cache_mask == 0)
            || (st.scope.flags == 0x10 && st.cache_mask == 7)
    }

    /// Pause (stream, mode). `from_runtime` is the runtime caller (frame handler count 2) which needs the
    /// completion transport and a started stream. `note` receives the pause note (oracle `pause_note`).
    pub fn stream_pause<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        mode: u32,
        from_runtime: bool,
        note: &mut dyn FnMut(u32, u32),
    ) -> Result<u32, Refusal> {
        self.stream_ctx(objs, device, w, stream)?;
        let completion = self.policy.completion;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        if from_runtime && (!completion || !st.discontinuity_seen || mode != 1) {
            return Err(Refusal(
                "runtime Pause caller needs completion transport, a started stream and mode1",
            ));
        }
        let resume_ok = completion && st.discontinuity_seen && (mode == 0 || mode == 1);
        if !Self::stream_shape(st) || (mode != 1 && !resume_ok) {
            return Err(Refusal(
                "only measured stereo or fully configured spatial startup Pause mode1 is supported",
            ));
        }
        if from_runtime && st.format_sets == 0 {
            return Err(Refusal("Pause after SetFormat needs a recorded SetFormat"));
        }
        st.pause_seen = true;
        st.pause_mode = mode;
        note(stream, mode);
        Ok(0)
    }

    /// FlushEx (stream, time low, time high, flags).
    pub fn stream_flush_ex<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        low: u32,
        high: u32,
        flags: u32,
    ) -> Result<u32, Refusal> {
        self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        if !Self::stream_shape(st)
            || !st.pause_seen
            || st.pause_mode != 1
            || low != 0
            || high != 0
            || flags != 1
        {
            return Err(Refusal(
                "only empty paused startup FlushEx time0/flags1 is supported",
            ));
        }
        st.flush_seen = true;
        st.flush_time_low = low;
        st.flush_time_high = high;
        st.flush_flags = flags;
        Ok(0)
    }

    /// Discontinuity (stream).
    pub fn stream_discontinuity<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
    ) -> Result<u32, Refusal> {
        self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        if !Self::stream_shape(st)
            || !st.pause_seen
            || st.pause_mode > 1
            || !st.flush_seen
            || st.flush_time_low != 0
            || st.flush_time_high != 0
            || st.flush_flags != 1
        {
            return Err(Refusal(
                "only empty paused/flushed startup Discontinuity is supported",
            ));
        }
        st.discontinuity_seen = true;
        Ok(0)
    }

    /// SetVolume without a routing hook (the routing note is skipped, as the C does with a NULL note).
    pub fn stream_volume<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        volume: u32,
        after_format: bool,
    ) -> Result<u32, Refusal> {
        self.stream_volume_with(
            objs,
            device,
            w,
            stream,
            volume,
            after_format,
            &mut crate::routing::NoStreamHooks,
        )
    }

    /// SetVolume (stream, volume, after_format) with the routing note: the aggregate volume candidate is offered
    /// to `routing_note` for stereo and spatial streams and kept only if accepted (oracle VOLUME).
    #[allow(clippy::too_many_arguments)]
    pub fn stream_volume_with<W: GuestWorld, H: StreamHooks>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        volume: u32,
        after_format: bool,
        hooks: &mut H,
    ) -> Result<u32, Refusal> {
        self.stream_ctx(objs, device, w, stream)?;
        let completion = self.policy.completion;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        let sc = &st.scope;
        let spatial = sc.flags == 0x10 && st.cache_mask == 7;
        let stereo = sc.flags == 0
            && st.cache_mask == 0
            && sc.max_packets == 3
            && sc.sample_rate == 44100
            && sc.channels == 2
            && sc.block_align == 72
            && sc.average_bytes_per_second == 49612;
        let fresh = !st.pause_seen && !st.flush_seen && !st.discontinuity_seen;
        let paused = stereo
            && st.pause_seen
            && st.pause_mode == 1
            && st.flush_seen
            && st.flush_time_low == 0
            && st.flush_time_high == 0
            && st.flush_flags == 1
            && st.discontinuity_seen;
        let update = completion && (spatial || stereo) && st.discontinuity_seen && st.volume_seen;
        if !update && ((!spatial && !stereo) || (!fresh && !paused) || volume != VOLUME_STARTUP) {
            return Err(Refusal(
                "only fresh or fully paused/flushed/discontinued startup silence is supported",
            ));
        }
        if after_format && st.format_sets == 0 {
            return Err(Refusal(
                "SetVolume after SetFormat needs a recorded SetFormat",
            ));
        }
        let new_volume = if update { volume as i32 } else { -10000 };
        let mut candidate = st.routing.clone();
        candidate.set_volume(new_volume as u32);
        if hooks.routing_installed()
            && (stereo || spatial)
            && !hooks.routing_note(stream, st.serial, &candidate)
        {
            return Err(Refusal("stream PCM volume/routing transaction rejected"));
        }
        st.routing = candidate;
        st.volume_seen = true;
        st.volume = new_volume;
        Ok(0)
    }

    /// SetFormat (stream, format address): the startup ADPCM family check. The completion/pitch note path
    /// (a stream with frequency sets or the completion model on) needs `dsound_stream_frequency_pitch`
    /// and is `Err` here (open).
    pub fn stream_set_format<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        format: u32,
    ) -> Result<u32, Refusal> {
        struct Legacy;
        impl StreamHooks for Legacy {
            fn format_installed(&self) -> bool {
                true
            }
        }
        self.stream_set_format_with(objs, device, w, stream, format, &mut Legacy)
    }

    /// SetFormat with the completion model: when the stream has frequency sets, or the completion model and
    /// `format_note` are on, the guest control word gives the pitch and the note must accept the (old, new)
    /// rate change before the format is recorded (oracle SET_FORMAT).
    pub fn stream_set_format_with<W: GuestWorld, H: StreamHooks>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        format: u32,
        hooks: &mut H,
    ) -> Result<u32, Refusal> {
        let (_, internal) = self.stream_ctx(objs, device, w, stream)?;
        let completion = self.policy.completion;
        let alias = self.aliases_headers(objs, Kind::Stream, format, FORMAT_BYTES as u32, internal)
            || overlap(format, FORMAT_BYTES as u32, STREAM_CURVE, 16);
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        let stereo = st.scope.flags == 0;
        let spatial = st.scope.flags == 0x10 && st.cache_mask == 7;
        if !stereo && !spatial {
            return Err(Refusal(
                "only the stereo startup scope or a configured spatial stream can change format",
            ));
        }
        if !(st.volume_seen && st.discontinuity_seen) {
            return Err(Refusal(
                "only a fully started (volume, Pause1, FlushEx, Discontinuity) stream can change format",
            ));
        }
        if alias {
            return Err(Refusal("SetFormat input aliases protected state"));
        }
        let mut words = [0u8; FORMAT_BYTES];
        let readable = w
            .read_bytes(format, FORMAT_BYTES)
            .map(|b| words.copy_from_slice(&b))
            .is_some();
        let channels: u8 = if stereo { 2 } else { 1 };
        let block = 36 * u32::from(channels);
        let rate = u32::from_le_bytes([words[4], words[5], words[6], words[7]]);
        let average = u32::from_le_bytes([words[8], words[9], words[10], words[11]]);
        let family = words[0] == 0x69
            && words[1] == 0
            && words[2] == channels
            && words[3] == 0
            && (8000..=48000).contains(&rate)
            && u64::from(average) == u64::from(rate) * u64::from(block) / 64
            && u32::from(words[12]) == block
            && words[13] == 0
            && words[14] == 4
            && words[15] == 0
            && words[16] == 2
            && words[17] == 0
            && words[18] == 64
            && words[19] == 0;
        if !readable || !family {
            return Err(Refusal(
                "only an ADPCM format of the stream's channel count (block 36 per channel, 64 samples, average rate*block/64, 8000 to 48000 Hz) at a readable address is supported",
            ));
        }
        let mut format_pitch = st.frequency_pitch;
        if st.frequency_sets != 0 || (completion && hooks.format_installed()) {
            let rejected = Refusal(
                "SetFormat PCM/completion abort/reset rejected ownership, clock, words or guest CW",
            );
            let cw = if hooks.format_installed() {
                hooks.control_word()
            } else {
                None
            };
            format_pitch = cw.and_then(|c| frequency_pitch(rate, c)).ok_or(rejected)?;
            if !hooks.format_note(stream, st.serial, st.source_rate_hz, rate, format) {
                return Err(rejected);
            }
        }
        st.format = words;
        st.frequency_pitch = format_pitch;
        st.format_sets += 1;
        st.source_rate_hz = rate;
        Ok(0)
    }

    /// Pause with the exact C caller class (oracle PAUSE `count`): 0 startup caller, 1 the caller right after
    /// SetFormat (needs a recorded SetFormat), 2 the runtime caller 0x29ED1 (needs completion and a started
    /// stream, mode 1). `stream_pause(.., from_runtime, ..)` above predates this and folds 1 and 2 together.
    #[allow(clippy::too_many_arguments)]
    pub fn stream_pause_class<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        mode: u32,
        class: u32,
        note: &mut dyn FnMut(u32, u32),
    ) -> Result<u32, Refusal> {
        self.stream_ctx(objs, device, w, stream)?;
        let completion = self.policy.completion;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        if class == 2 && (!completion || !st.discontinuity_seen || mode != 1) {
            return Err(Refusal(
                "runtime Pause caller needs completion transport, a started stream and mode1",
            ));
        }
        let resume_ok = completion && st.discontinuity_seen && (mode == 0 || mode == 1);
        if !Self::stream_shape(st) || (mode != 1 && !resume_ok) {
            return Err(Refusal(
                "only measured stereo or fully configured spatial startup Pause mode1 is supported",
            ));
        }
        if class == 1 && st.format_sets == 0 {
            return Err(Refusal("Pause after SetFormat needs a recorded SetFormat"));
        }
        st.pause_seen = true;
        st.pause_mode = mode;
        note(stream, mode);
        Ok(0)
    }

    /// Oracle `frequency_preflight` / `routing_preflight`: policy, completion, the note and IRQL 0 (else a
    /// stop), then the original global failure E_FAIL precedes object access.
    pub(crate) fn hooked_preflight<W: GuestWorld + ?Sized>(
        &self,
        w: &W,
        installed: bool,
    ) -> Result<Option<u32>, Refusal> {
        if !installed {
            return Err(Refusal("needs explicit PCM/completion policy at IRQL0"));
        }
        self.preflight(w, true)
    }

    /// IDirectSoundStream_SetFrequency (stream, hertz, x87 control word). `hertz` 0 means the stream's own
    /// rate (the SetFormat rate once one is recorded).
    #[allow(clippy::too_many_arguments)]
    pub fn stream_frequency<W: GuestWorld, H: StreamHooks>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        hertz: u32,
        control_word: u16,
        hooks: &mut H,
    ) -> Result<u32, Refusal> {
        if let Some(fail) = self.hooked_preflight(w, hooks.frequency_installed())? {
            return Ok(fail);
        }
        self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get_mut(&stream)
            .ok_or(Refusal("no stream state"))?;
        let stereo = st.scope.flags == 0 && st.scope.channels == 2 && st.cache_mask == 0;
        let spatial = st.scope.flags == 0x10 && st.scope.channels == 1 && st.cache_mask == 7;
        if (!stereo && !spatial) || !st.volume_seen || !st.discontinuity_seen {
            return Err(Refusal(
                "frequency needs a started stereo or fully configured mono PCM stream",
            ));
        }
        let mut hertz = hertz;
        if hertz == 0 {
            hertz = st.scope.sample_rate;
            if st.format_sets != 0 {
                hertz =
                    u32::from_le_bytes([st.format[4], st.format[5], st.format[6], st.format[7]]);
            }
        }
        let pitch = frequency_pitch(hertz, control_word).ok_or(Refusal(
            "frequency or guest control word outside measured domain",
        ))?;
        if st.frequency_sets == u32::MAX
            || !hooks.frequency_note(stream, st.serial, st.source_rate_hz, hertz)
        {
            return Err(Refusal("frequency PCM/completion transaction refused"));
        }
        st.source_rate_hz = hertz;
        st.frequency_pitch = pitch;
        st.frequency_sets += 1;
        Ok(0)
    }

    /// SetHeadroom (stream, headroom).
    pub fn stream_headroom<W: GuestWorld, H: StreamHooks>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        headroom: u32,
        hooks: &mut H,
    ) -> Result<u32, Refusal> {
        self.routing_entry(objs, device, w, stream, hooks, |r, _| {
            r.set_headroom(headroom);
            Ok(true)
        })
    }

    /// SetMixBins (stream, descriptor): 0 restores the stereo defaults, else `{count, pointer to count pairs
    /// of (bin, volume)}`, at most 8 routes.
    pub fn stream_mix_bins<W: GuestWorld, H: StreamHooks>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        descriptor: u32,
        hooks: &mut H,
    ) -> Result<u32, Refusal> {
        if descriptor == 0 {
            return self.routing_entry(objs, device, w, stream, hooks, |r, _| {
                Ok(r.set_bins(&[], true))
            });
        }
        self.mix_pairs(objs, device, w, stream, descriptor, hooks, true)
    }

    /// SetMixBinVolumes (stream, descriptor): `{count, pointer}` of (bin < 32, volume), at most 32.
    pub fn stream_mix_bin_volumes<W: GuestWorld, H: StreamHooks>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        descriptor: u32,
        hooks: &mut H,
    ) -> Result<u32, Refusal> {
        self.mix_pairs(objs, device, w, stream, descriptor, hooks, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn mix_pairs<W: GuestWorld, H: StreamHooks>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        descriptor: u32,
        hooks: &mut H,
        bins: bool,
    ) -> Result<u32, Refusal> {
        const BAD_DESC: Refusal = Refusal("mix-bin descriptor inaccessible or aliases owned state");
        const BAD_PAIRS: Refusal =
            Refusal("mix-bin pairs outside checked bounds or aliases owned state");
        self.routing_entry(objs, device, w, stream, hooks, |r, (internal, this)| {
            let alias = |a: u32, n: u32| this.aliases_headers(objs, Kind::Stream, a, n, internal);
            if descriptor == 0 || alias(descriptor, 4) {
                return Err(BAD_DESC);
            }
            let n = w.read_u32(descriptor).ok_or(BAD_DESC)?;
            let mut base = 0;
            if n != 0 {
                if u64::from(descriptor) + 8 > 1 << 32 || alias(descriptor + 4, 4) {
                    return Err(BAD_DESC);
                }
                base = w.read_u32(descriptor + 4).ok_or(BAD_DESC)?;
            }
            let max = if bins { ROUTE_SLOTS } else { ROUTE_BINS } as u32;
            if n > max {
                return Err(BAD_PAIRS);
            }
            let mut pairs = Vec::new();
            if n != 0 {
                if base == 0 || u64::from(base) + u64::from(n) * 8 > 1 << 32 || alias(base, n * 8) {
                    return Err(BAD_PAIRS);
                }
                let bytes = w.read_bytes(base, (n * 8) as usize).ok_or(BAD_PAIRS)?;
                pairs = le_words(&bytes)
                    .chunks_exact(2)
                    .map(|c| (c[0], c[1]))
                    .collect();
            }
            Ok(if bins {
                r.set_bins(&pairs, false)
            } else {
                r.set_bin_volumes(&pairs)
            })
        })
    }

    /// Shared routing transaction (oracle HEADROOM, MIX_BINS, MIX_VOLUMES): preflight, ownership, the
    /// nonspatial stereo admission, the candidate edit, `routing_note`, then commit.
    fn routing_entry<W: GuestWorld, H: StreamHooks>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
        hooks: &mut H,
        edit: impl FnOnce(&mut Routing, (u32, &Self)) -> Result<bool, Refusal>,
    ) -> Result<u32, Refusal> {
        if let Some(fail) = self.hooked_preflight(w, hooks.routing_installed())? {
            return Ok(fail);
        }
        let (_, internal) = self.stream_ctx(objs, device, w, stream)?;
        let st = self
            .streams
            .get(&stream)
            .ok_or(Refusal("no stream state"))?;
        if st.scope.flags != 0 || st.scope.channels != 2 {
            return Err(Refusal(
                "real PCM routing requires admitted nonspatial stereo stream and completion policy",
            ));
        }
        let mut candidate = st.routing.clone();
        let serial = st.serial;
        if !edit(&mut candidate, (internal, &*self))? {
            return Err(Refusal(
                "mix-bin state needs unsupported DSP/HRTF route or invalid index",
            ));
        }
        if !hooks.routing_note(stream, serial, &candidate) {
            return Err(Refusal(
                "PCM routing rejected clock/history/lifetime or inactive renderer",
            ));
        }
        if let Some(st) = self.streams.get_mut(&stream) {
            st.routing = candidate;
        }
        Ok(0)
    }

    /// Startup GetStatus (stream, output): writes 1 (bit 0) for the exact startup or started stream shapes.
    pub fn stream_startup_status<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &mut W,
        stream: u32,
        output: u32,
    ) -> Result<u32, Refusal> {
        let (_, internal) = self.stream_ctx(objs, device, &*w, stream)?;
        let alias = self.aliases_headers(objs, Kind::Stream, output, 4, internal)
            || overlap(output, 4, STREAM_CURVE, 16);
        let st = self
            .streams
            .get(&stream)
            .ok_or(Refusal("no stream state"))?;
        let sc = &st.scope;
        let spatial = sc.flags == 0x10 && st.cache_mask == 7;
        let stereo = sc.flags == 0
            && st.cache_mask == 0
            && sc.sample_rate == 44100
            && sc.channels == 2
            && sc.block_align == 72
            && sc.average_bytes_per_second == 49612;
        let started = st.discontinuity_seen;
        let fresh = !st.pause_seen && !st.flush_seen && !st.discontinuity_seen;
        if (!spatial && !stereo)
            || sc.max_packets != 3
            || !st.volume_seen
            || st.volume != -10000
            || (!fresh && !started)
        {
            return Err(Refusal(
                "only exact spatial or fresh stereo silence startup status or the status of a started stream is supported",
            ));
        }
        let writable = output != 0
            && u64::from(output) + 4 <= 1 << 32
            && !alias
            && w.read_u32(output)
                .is_some_and(|old| w.write_u32(output, old))
            && w.write_u32(output, 1);
        if !writable {
            return Err(Refusal(
                "startup status output is inaccessible or aliases protected state",
            ));
        }
        Ok(0)
    }

    /// The `StreamView` the polled adapter reads (oracle `dsound_stream_get_snapshot` plus the effective format).
    pub fn stream_view<W: GuestWorld + ?Sized>(
        &self,
        objs: &Objects,
        device: &Device,
        w: &W,
        stream: u32,
    ) -> Option<StreamView> {
        Self::owned(objs, device, w, Kind::Stream, stream).ok()?;
        let st = self.streams.get(&stream)?;
        Some(StreamView {
            serial: st.serial,
            max_packets: st.scope.max_packets,
            volume_seen: st.volume_seen,
            discontinuity_seen: st.discontinuity_seen,
            format: if st.format_sets != 0 {
                st.format
            } else {
                st.scope.format
            },
        })
    }

    // ----------------------------------------------------------------- buffers

    fn buffer_ctx<'a, W: GuestWorld + ?Sized>(
        &self,
        objs: &'a Objects,
        device: &Device,
        w: &W,
        buffer: u32,
    ) -> Result<(&'a Object, u32), Refusal> {
        let o = Self::owned(objs, device, w, Kind::Buffer, buffer)?;
        self.policy_error(w)?;
        Ok((o, o.lease.internal_address))
    }

    fn bstate(&mut self, buffer: u32) -> Result<&mut BufferState, Refusal> {
        self.buffers
            .get_mut(&buffer)
            .ok_or(Refusal("no buffer state"))
    }

    /// SetBufferData (buffer, data, length): the host records pointer and length (oracle SET_DATA).
    pub fn buffer_set_data<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        data: u32,
        length: u32,
    ) -> Result<u32, Refusal> {
        let (_, internal) = self.buffer_ctx(objs, device, w, buffer)?;
        let alias = length != 0 && self.buffer_aliases_state(objs, data, length, internal);
        let st = self.bstate(buffer)?;
        let spatial = st.scope.flags == 0x10 && st.cache_mask == 7;
        let ordinary = st.scope.flags == 0 && st.cache_mask == 0;
        if !spatial && !ordinary {
            return Err(Refusal(
                "only fully configured spatial or fresh ordinary startup buffers accept SetBufferData",
            ));
        }
        if data == 0 || length == 0 {
            return Err(Refusal(
                "only non-null data with non-zero length is supported (the original allocates or clears otherwise)",
            ));
        }
        if length > DATA_MAX_BYTES || !w.range_readable(data, length) {
            return Err(Refusal(
                "buffer data range is longer than 64 MiB or not readable guest memory",
            ));
        }
        if alias {
            return Err(Refusal("buffer data aliases owned state"));
        }
        if st.data_address != data || st.data_length != length {
            st.loop_start = 0;
            st.loop_length = length;
        }
        st.data_address = data;
        st.data_length = length;
        st.data_sets += 1;
        Ok(0)
    }

    /// SetVolume (buffer, volume).
    pub fn buffer_volume<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        volume: u32,
        hooks: &mut dyn BufferHooks,
    ) -> Result<u32, Refusal> {
        self.buffer_ctx(objs, device, w, buffer)?;
        let completion = self.policy.completion;
        let st = self.bstate(buffer)?;
        if st.data_sets == 0 {
            return Err(Refusal(
                "SetVolume before SetBufferData is not the measured sound start order",
            ));
        }
        if !completion {
            if volume != VOLUME_STARTUP && volume != VOLUME_UPDATE {
                return Err(Refusal(
                    "only the measured volumes -10000 (start) and -3204 (update) are supported",
                ));
            }
            if volume != VOLUME_STARTUP && (st.frequency_sets == 0 || st.scope.flags != 0) {
                return Err(Refusal(
                    "the update volume -3204 needs a started ordinary buffer (SetBufferData, SetVolume, Pause and SetFrequency recorded)",
                ));
            }
        }
        if completion && !hooks.volume(buffer, st.serial, volume as i32) {
            return Err(Refusal(
                "real buffer PCM rejected volume or its timestamp/history",
            ));
        }
        st.volume = volume as i32;
        st.volume_sets += 1;
        Ok(0)
    }

    /// Pause (buffer, mode).
    pub fn buffer_pause<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        mode: u32,
        hooks: &mut dyn BufferHooks,
    ) -> Result<u32, Refusal> {
        self.buffer_ctx(objs, device, w, buffer)?;
        let st = self.bstate(buffer)?;
        if st.volume_sets == 0 {
            return Err(Refusal(
                "Pause before SetBufferData and SetVolume is not the measured sound start order",
            ));
        }
        if hooks.started(buffer, st.serial) {
            if !hooks.pause(buffer, st.serial, mode) {
                return Err(Refusal(
                    "only Pause(0) of a played buffer, and Pause(1 or 2) of a playing buffer, are modelled",
                ));
            }
            st.pause_sets += 1;
            return Ok(0);
        }
        if mode != 0 {
            return Err(Refusal(
                "only Pause(0) (resume) on a never started buffer is supported",
            ));
        }
        st.pause_sets += 1;
        Ok(0)
    }

    /// SetFrequency (buffer, hertz).
    pub fn buffer_frequency<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        hertz: u32,
        hooks: &mut dyn BufferHooks,
    ) -> Result<u32, Refusal> {
        self.buffer_ctx(objs, device, w, buffer)?;
        let completion = self.policy.completion;
        let st = self.bstate(buffer)?;
        if st.pause_sets == 0 {
            return Err(Refusal(
                "SetFrequency before SetBufferData, SetVolume and Pause is not the measured sound start order",
            ));
        }
        if hertz != FREQUENCY_STARTUP
            && !(completion && (FREQUENCY_MIN..=FREQUENCY_MAX).contains(&hertz))
        {
            return Err(Refusal(
                "only the measured startup frequency 22042 is supported (the completion model takes the title's clamp 0xBC..0x2EDEF)",
            ));
        }
        if hooks.started(buffer, st.serial)
            && !hooks.frequency(buffer, st.serial, st.frequency, hertz)
        {
            return Err(Refusal(
                "SetFrequency on a played buffer the completion model does not know is refused",
            ));
        }
        st.frequency = hertz;
        st.frequency_sets += 1;
        Ok(0)
    }

    /// SetLoopRegion (buffer, start, length). Returns 0x88780032 for a region past the data.
    pub fn buffer_loop_region<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        start: u32,
        length: u32,
        hooks: &mut dyn BufferHooks,
    ) -> Result<u32, Refusal> {
        if let Some(fail) = self.preflight(w, true)? {
            return Ok(fail);
        }
        self.buffer_ctx(objs, device, w, buffer)?;
        let completion = self.policy.completion;
        let st = self.bstate(buffer)?;
        let shape_ok = st.scope.flags == 0 || (st.scope.flags == 0x10 && st.cache_mask == 7);
        if !completion || !shape_ok || st.data_sets == 0 {
            return Err(Refusal(
                "loop regions need passive completion and ordinary buffer data",
            ));
        }
        let running = hooks.started(buffer, st.serial) && hooks.voice_running(buffer, st.serial);
        let end = u64::from(start) + u64::from(length);
        if end > u64::from(u32::MAX) || start > st.data_length {
            return Err(Refusal(
                "wrapped or out-of-range loop start is not supported",
            ));
        }
        if length != 0 && end > u64::from(st.data_length) {
            return Ok(PACKET_LIST_FULL);
        }
        let len = if length != 0 {
            length
        } else {
            st.data_length - start
        };
        if len == 0 || start % 36 != 0 || len % 36 != 0 {
            return Err(Refusal(
                "only nonempty 36-byte ADPCM block loop regions are supported",
            ));
        }
        if running && !hooks.loop_region(buffer, st.serial, start, len, st.frequency) {
            return Err(Refusal(
                "the completion model refused the live loop region of the running voice",
            ));
        }
        st.loop_start = start;
        st.loop_length = len;
        st.loop_sets += 1;
        Ok(0)
    }

    /// SetI3DL2Source cache (buffer, parameters, apply 0).
    pub fn buffer_i3dl2<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        parameters: u32,
        apply: u32,
    ) -> Result<u32, Refusal> {
        let (_, internal) = self.buffer_ctx(objs, device, w, buffer)?;
        let alias = self.aliases_headers(objs, Kind::Buffer, parameters, 36, internal);
        let st = self.bstate(buffer)?;
        if st.scope.flags != 0x10 || apply != 0 {
            return Err(Refusal("only startup spatial setters/apply0 are supported"));
        }
        let words = w.read_bytes(parameters, 36).map(|b| le_words(&b));
        if st.cache_mask != 0 || alias || words.as_deref() != Some(&I3DL2_EXPECTED[..]) {
            return Err(Refusal("only exact first startup I3DL2 cache is supported"));
        }
        st.i3dl2 = I3DL2_EXPECTED;
        st.i3dl2_address = parameters;
        st.cache_mask = 1;
        Ok(0)
    }

    /// SetMinDistance (buffer, bits, apply): apply 1 is the sound start's re-apply (T1173), apply 0 the ordered cache.
    pub fn buffer_min_distance<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        bits: u32,
        apply: u32,
    ) -> Result<u32, Refusal> {
        self.buffer_ctx(objs, device, w, buffer)?;
        let st = self.bstate(buffer)?;
        if apply == 1 {
            if st.scope.flags != 0x10 || st.cache_mask != 7 || st.data_sets == 0 {
                return Err(Refusal(
                    "only the measured apply=1 update on a configured spatial buffer with recorded data is supported",
                ));
            }
            st.min_distance_bits = bits;
            st.min_distance_sets += 1;
            return Ok(0);
        }
        if st.scope.flags != 0x10 || apply != 0 {
            return Err(Refusal("only startup spatial setters/apply0 are supported"));
        }
        if st.cache_mask != 1 || bits != 0x3F80_0000 {
            return Err(Refusal("only ordered startup minDistance1.0 is supported"));
        }
        st.min_distance_bits = bits;
        st.cache_mask = 3;
        Ok(0)
    }

    /// SetRolloffCurve (buffer, curve, count, apply).
    pub fn buffer_rolloff<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        curve: u32,
        count: u32,
        apply: u32,
    ) -> Result<u32, Refusal> {
        self.buffer_ctx(objs, device, w, buffer)?;
        let st = self.bstate(buffer)?;
        if apply == 1 {
            if st.scope.flags != 0x10 || st.cache_mask != 7 || st.data_sets == 0 {
                return Err(Refusal(
                    "only the measured apply=1 update on a configured spatial buffer with recorded data is supported",
                ));
            }
            st.curve_address = curve;
            st.curve_count = count;
            st.rolloff_sets += 1;
            return Ok(0);
        }
        if st.scope.flags != 0x10 || apply != 0 {
            return Err(Refusal("only startup spatial setters/apply0 are supported"));
        }
        if st.cache_mask != 3 || curve != BUFFER_CURVE || count != 1 {
            return Err(Refusal("only ordered startup curve VA/count1 is supported"));
        }
        st.curve_address = curve;
        st.curve_count = count;
        st.cache_mask = 7;
        Ok(0)
    }

    /// SetMaxDistance (buffer, bits, apply 1).
    pub fn buffer_max_distance<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        bits: u32,
        apply: u32,
    ) -> Result<u32, Refusal> {
        if let Some(fail) = self.preflight(w, false)? {
            return Ok(fail);
        }
        self.buffer_ctx(objs, device, w, buffer)?;
        let st = self.bstate(buffer)?;
        if st.scope.flags != 0x10 || st.cache_mask != 7 || st.data_sets == 0 || apply != 1 {
            return Err(Refusal(
                "only the measured apply=1 update on a configured spatial buffer with recorded data is supported",
            ));
        }
        st.max_distance_bits = bits;
        st.max_distance_sets += 1;
        Ok(0)
    }

    /// SetPosition (buffer, x, y, z bits, apply 1).
    pub fn buffer_position<W: GuestWorld>(
        &mut self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
        xyz: [u32; 3],
        apply: u32,
    ) -> Result<u32, Refusal> {
        if let Some(fail) = self.preflight(w, false)? {
            return Ok(fail);
        }
        self.buffer_ctx(objs, device, w, buffer)?;
        let st = self.bstate(buffer)?;
        if st.scope.flags != 0x10 || st.cache_mask != 7 || st.data_sets == 0 {
            return Err(Refusal(
                "only a configured spatial buffer with recorded data is supported",
            ));
        }
        if apply != 1 {
            return Err(Refusal(
                "SetPosition apply=0 enters the unmodeled settings/APU commit",
            ));
        }
        st.position_bits = xyz;
        st.position_sets += 1;
        Ok(0)
    }

    /// The `BufferView` the polled adapter reads (oracle `dsound_buffer_snapshot`).
    pub fn buffer_view<W: GuestWorld + ?Sized>(
        &self,
        objs: &Objects,
        device: &Device,
        w: &W,
        buffer: u32,
    ) -> Option<BufferView> {
        Self::owned(objs, device, w, Kind::Buffer, buffer).ok()?;
        let st = self.buffers.get(&buffer)?;
        Some(BufferView {
            serial: st.serial,
            flags: st.scope.flags,
            cache_mask: st.cache_mask,
            block_align: u32::from(st.scope.block_align),
            frequency: st.frequency,
            volume: st.volume,
            data_address: st.data_address,
            data_length: st.data_length,
            data_sets: st.data_sets,
            volume_sets: st.volume_sets,
            pause_sets: st.pause_sets,
            frequency_sets: st.frequency_sets,
            loop_sets: st.loop_sets,
            loop_start: st.loop_start,
            loop_length: st.loop_length,
        })
    }
}

fn new_stream(scope: StreamScope, serial: u64) -> StreamState {
    StreamState {
        serial,
        cache_mask: 0,
        i3dl2: [0; 9],
        i3dl2_address: 0,
        min_distance_bits: 0,
        max_distance_seen: false,
        max_distance_bits: 0,
        curve_address: 0,
        curve_count: 0,
        pause_seen: false,
        pause_mode: 0,
        flush_seen: false,
        flush_time_low: 0,
        flush_time_high: 0,
        flush_flags: 0,
        discontinuity_seen: false,
        volume_seen: false,
        volume: 0,
        format_sets: 0,
        source_rate_hz: scope.sample_rate,
        format: scope.format,
        position_bits: [0; 3],
        position_sets: 0,
        routing: Routing::default(),
        frequency_pitch: 0,
        frequency_sets: 0,
        scope,
    }
}

fn new_buffer(scope: BufferScope, serial: u64) -> BufferState {
    BufferState {
        scope,
        serial,
        cache_mask: 0,
        i3dl2: [0; 9],
        i3dl2_address: 0,
        min_distance_bits: 0,
        curve_address: 0,
        curve_count: 0,
        data_address: 0,
        data_length: 0,
        data_sets: 0,
        loop_start: 0,
        loop_length: 0,
        loop_sets: 0,
        volume: 0,
        volume_sets: 0,
        pause_sets: 0,
        frequency: 0,
        frequency_sets: 0,
        max_distance_bits: 0,
        max_distance_sets: 0,
        min_distance_sets: 0,
        rolloff_sets: 0,
        position_bits: [0; 3],
        position_sets: 0,
    }
}
