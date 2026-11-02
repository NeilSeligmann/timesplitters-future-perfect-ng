// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-handler access to an owned stream: identity, sidecar reads and the measured startup policy of each handler,
//! run under the device ownership lock and the stream lock (`dsound_device_with_owned_interface`).
use crate::config::{self, Config};
use crate::ffi::*;
use crate::*;
use std::ffi::{c_char, c_void};
use std::ptr::{null, null_mut};

pub(crate) struct Access {
    address: u32,
    entry: u32,
    argument: u32,
    apply: u32,
    count: u32,
    position_z: u32,
    identity: Lease,
    snapshot: *mut Snapshot,
    error: *const c_char,
    observer: bool,
}

impl Access {
    fn new(entry: u32, address: u32) -> Self {
        Self {
            address,
            entry,
            argument: 0,
            apply: 0,
            count: 0,
            position_z: 0,
            identity: Lease::default(),
            snapshot: null_mut(),
            error: null(),
            observer: false,
        }
    }
}

/// C-heap fallback probe of the lease validator; the Rust validator calls GuestRegions directly when active.
unsafe extern "C" fn lease_probe(
    _context: *mut c_void,
    stage: u32,
    heap: u32,
    address: u32,
    block_bytes: *mut u32,
) -> bool {
    match stage {
        0 | 2 => unsafe { kmem_heap_valid(heap) },
        3 => !block_bytes.is_null() && unsafe { kmem_heap_block_size(heap, address, block_bytes) },
        _ => false,
    }
}

pub(crate) fn valid_node(address: u32, identity: &Lease, internal: u32) -> bool {
    // SAFETY: lease validation against Rust-owned records.
    unsafe {
        tsfp_audio_stream_lease_validate_record_rust(
            address,
            identity,
            internal,
            null_mut(),
            lease_probe,
        )
    }
}

fn commit_volume_routing(
    stream: u32,
    serial: u64,
    volume: u32,
    apply_pcm: bool,
    config: &Config,
) -> bool {
    let mut note = config.routing_note;
    let (apply, context): (Option<RoutingApply>, *mut c_void) = if apply_pcm {
        if note.is_none() {
            return false;
        }
        (
            Some(tsfp_audio_stream_routing_apply_note_rust),
            (&mut note as *mut Option<RoutingNote>).cast(),
        )
    } else {
        (None, null_mut())
    };
    // SAFETY: `context` outlives the synchronous call.
    unsafe { tsfp_audio_stream_routing_volume_commit_rust(stream, serial, volume, context, apply) }
}

unsafe extern "C" fn frequency_apply(
    context: *mut c_void,
    stream: u32,
    serial: u64,
    old_hz: u32,
    new_hz: u32,
) -> bool {
    if context.is_null() {
        return false;
    }
    // SAFETY: the context is the caller's `Option<FrequencyNote>`.
    let note = unsafe { *(context as *const Option<FrequencyNote>) };
    // SAFETY: host-installed note.
    note.is_some_and(|note| unsafe { note(stream, serial, old_hz, new_hz) })
}

unsafe extern "C" fn frequency_format_apply(
    context: *mut c_void,
    stream: u32,
    serial: u64,
    old_hz: u32,
    new_hz: u32,
    format_address: u32,
) -> bool {
    if context.is_null() {
        return false;
    }
    // SAFETY: the context is the caller's `Option<FormatNote>`.
    let note = unsafe { *(context as *const Option<FormatNote>) };
    // SAFETY: host-installed note.
    note.is_some_and(|note| unsafe { note(stream, serial, old_hz, new_hz, format_address) })
}

/// Input buffers are historical stack snapshots, not guest-owned state. The original setter reuses/overlaps these
/// locals after Create has consumed them.
fn aliases_headers(address: u32, bytes: u32, internal: u32) -> bool {
    // SAFETY: pure span queries on Rust-owned records.
    unsafe {
        tsfp_audio_stream_create_fixed_overlap_rust(address, bytes, internal, false)
            || tsfp_audio_stream_lease_overlaps_guest_span_rust(address, bytes)
            || tsfp_audio_stream_create_untracked_overlaps_rust(address, bytes)
    }
}

/// Protected spans: the fixed globals plus every live stream, snapshotted under the stream lock.
unsafe extern "C" fn routing_spans(
    internal: u32,
    spans: *mut GuestSpan,
    capacity: u32,
    count_out: *mut u32,
) -> bool {
    if count_out.is_null() {
        return false;
    }
    let mut stream_count = 0u32;
    // SAFETY: counting call with no span buffer.
    if !unsafe { tsfp_audio_stream_lease_guest_spans_rust(null_mut(), 0, &mut stream_count) }
        || stream_count > u32::MAX - 4
    {
        return false;
    }
    let count = stream_count + 4;
    // SAFETY: caller's output.
    unsafe { *count_out = count };
    if spans.is_null() {
        return capacity == 0;
    }
    if capacity < count {
        return false;
    }
    let fixed = [
        GuestSpan {
            address: internal,
            bytes: 44,
        },
        GuestSpan {
            address: SINGLETON,
            bytes: 4,
        },
        GuestSpan {
            address: GLOBAL_STATE,
            bytes: 4,
        },
        GuestSpan {
            address: SECONDARY,
            bytes: 60,
        },
    ];
    let mut written = 0u32;
    // SAFETY: `spans` has `capacity >= count` entries.
    unsafe {
        for (index, span) in fixed.iter().enumerate() {
            *spans.add(index) = *span;
        }
        tsfp_audio_stream_lease_guest_spans_rust(spans.add(4), capacity - 4, &mut written)
            && written == stream_count
            && written + 4 == count
    }
}

fn guest_read(address: u32) -> Option<u32> {
    let mut value = 0u32;
    // SAFETY: local output.
    unsafe { kernel_guest_read_u32(address, &mut value) }.then_some(value)
}

/// The handler body, run with the stream lock held. `Err`/`false` leaves `r.error` set.
fn owned(r: &mut Access, internal: u32, result: &mut u32) {
    if !valid_node(r.address, &r.identity, internal) {
        r.error = c"stream ownership/header/generation changed".as_ptr();
        return;
    }
    if r.observer {
        // SAFETY: `snapshot` is the observer caller's storage.
        if unsafe { tsfp_audio_stream_observe_rust(r.address, r.snapshot, &mut r.error) } {
            *result = 0;
        }
        return;
    }
    let mut lease = Lease::default();
    // SAFETY: local output for every sidecar read below.
    unsafe {
        if !tsfp_audio_stream_lease_identity_rust(r.address, &mut lease) {
            r.error = c"Rust stream lease identity is unavailable".as_ptr();
            return;
        }
    }
    // SAFETY: scope zero is valid.
    let mut scope: Scope = unsafe { std::mem::zeroed() };
    let mut format_state = FormatState::default();
    let mut volume_state = VolumeState::default();
    let mut spatial_state = SpatialState::default();
    let mut control_state = ControlState::default();
    // SAFETY: local outputs.
    unsafe {
        if !tsfp_audio_stream_lease_scope_rust(r.address, &lease, &mut scope) {
            r.error = c"Rust stream scope is unavailable for this lease".as_ptr();
            return;
        }
        if !tsfp_audio_stream_lease_format_state_rust(r.address, &lease, &mut format_state) {
            r.error = c"Rust stream format sidecar is unavailable for this lease".as_ptr();
            return;
        }
    }
    let mut format_rate = scope.sample_rate;
    if format_state.sets != 0 {
        format_rate = u32::from_ne_bytes([
            format_state.bytes[4],
            format_state.bytes[5],
            format_state.bytes[6],
            format_state.bytes[7],
        ]);
    }
    // SAFETY: local outputs.
    unsafe {
        if !tsfp_audio_stream_lease_volume_state_rust(r.address, &lease, &mut volume_state) {
            r.error = c"Rust stream volume sidecar is unavailable for this lease".as_ptr();
            return;
        }
        if !tsfp_audio_stream_lease_spatial_state_rust(r.address, &lease, &mut spatial_state) {
            r.error = c"Rust stream spatial sidecar is unavailable for this lease".as_ptr();
            return;
        }
        if !tsfp_audio_stream_lease_control_state_rust(r.address, &lease, &mut control_state) {
            r.error = c"Rust stream control sidecar is unavailable for this lease".as_ptr();
            return;
        }
    }
    let volume_seen = volume_state.seen != 0;
    let cache_mask = spatial_state.startup_cache_mask;
    let pause_seen = control_state.pause_seen != 0;
    let flush_seen = control_state.flush_seen != 0;
    let discontinuity_seen = control_state.discontinuity_seen != 0;
    let config = config::get();
    r.error = policy_error(&config);
    if !r.error.is_null() {
        return;
    }
    let stream = r.address;
    match r.entry {
        SET_FREQUENCY => {
            let mut note = config.frequency_note;
            let flags = u32::from(volume_seen)
                | u32::from(discontinuity_seen) << 1
                | u32::from(config.completion) << 2
                | u32::from(note.is_some()) << 3;
            // SAFETY: the context outlives the synchronous call.
            let admitted = unsafe {
                tsfp_audio_stream_set_frequency_rust(
                    stream,
                    lease.serial,
                    scope.flags,
                    u32::from(scope.channels),
                    scope.sample_rate,
                    format_state.sets,
                    format_state.bytes.as_ptr(),
                    cache_mask,
                    flags,
                    r.argument,
                    r.count,
                    (&mut note as *mut Option<FrequencyNote>).cast(),
                    frequency_apply,
                    &mut r.error,
                )
            };
            if admitted {
                config::update(|c| c.accepted_frequency_sets += 1);
                *result = 0;
            }
        }
        HEADROOM | MIX_BINS | MIX_VOLUMES => {
            let completion_available =
                u32::from(config.completion && config.routing_note.is_some());
            // SAFETY: pure admission query.
            if unsafe {
                tsfp_audio_stream_routing_scope_admit_rust(stream, &lease, completion_available)
            } != 0
            {
                r.error = c"real PCM routing requires admitted nonspatial stereo stream and completion policy".as_ptr();
                return;
            }
            let mut note = config.routing_note;
            let operation = match r.entry {
                MIX_BINS => 1,
                MIX_VOLUMES => 2,
                _ => 0,
            };
            // SAFETY: the context outlives the synchronous call.
            let status = unsafe {
                tsfp_audio_stream_routing_update_rust(
                    stream,
                    lease.serial,
                    operation,
                    r.argument,
                    host_pid(),
                    r.argument,
                    internal,
                    (&mut note as *mut Option<RoutingNote>).cast(),
                    tsfp_audio_stream_routing_apply_note_rust,
                    routing_spans,
                )
            };
            if status != 0 {
                r.error = match status {
                    3 => c"PCM routing rejected clock/history/lifetime or inactive renderer",
                    2 => c"mix-bin state needs unsupported DSP/HRTF route or invalid index",
                    _ => {
                        c"mix-bin descriptor or pairs outside checked bounds or aliases owned state"
                    }
                }
                .as_ptr();
                return;
            }
            *result = 0;
        }
        STATUS => {
            // SAFETY: pure admission query.
            let admitted = unsafe {
                tsfp_audio_stream_status_admit_rust(
                    scope.flags,
                    cache_mask,
                    scope.sample_rate,
                    u32::from(scope.channels),
                    u32::from(scope.block_align),
                    scope.average_bytes_per_second,
                    scope.max_packets,
                    u32::from(volume_seen),
                    volume_state.value,
                    u32::from(pause_seen),
                    u32::from(flush_seen),
                    u32::from(discontinuity_seen),
                )
            };
            if !admitted {
                r.error = c"only exact spatial or fresh stereo silence startup status or the status of a started stream is supported".as_ptr();
                return;
            }
            let mut old = 0u32;
            // SAFETY: guest accessors, probing the output before the real write.
            let inaccessible = r.argument == 0
                || u64::from(r.argument) + 4 > 1 << 32
                || aliases_headers(r.argument, 4, internal)
                || overlap(r.argument, 4, CURVE, 16)
                || !unsafe { kernel_guest_read_u32(r.argument, &mut old) }
                || !unsafe { kernel_guest_write_u32(r.argument, old) }
                || !unsafe { kernel_guest_write_u32(r.argument, 1) };
            if inaccessible {
                r.error =
                    c"startup status output is inaccessible or aliases protected state".as_ptr();
                return;
            }
            *result = 0;
        }
        SET_FORMAT => {
            set_format(
                r,
                &config,
                &lease,
                &format_state,
                format_rate,
                (cache_mask, scope.flags, volume_seen, discontinuity_seen),
                internal,
            );
            if r.error.is_null() {
                *result = 0;
            }
        }
        VOLUME => {
            let request = VolumeRequest {
                argument: r.argument,
                count: r.count,
                completion_available: u32::from(config.completion),
            };
            let mut next = VolumeState::default();
            // SAFETY: Rust SetVolume policy plan.
            if unsafe {
                tsfp_audio_stream_lease_volume_apply_rust(stream, &lease, &request, &mut next)
            } != 0
            {
                r.error = c"Rust stream SetVolume policy refused this request".as_ptr();
                return;
            }
            if !commit_volume_routing(
                stream,
                lease.serial,
                next.value as u32,
                config.routing_note.is_some(),
                &config,
            ) {
                r.error = c"stream PCM volume/routing transaction rejected".as_ptr();
                return;
            }
            // The stream lock serializes this exact-generation commit with the accepted routing transaction and every
            // observer or subsequent handler.
            // SAFETY: Rust sidecar commit.
            if !unsafe {
                tsfp_audio_stream_lease_volume_commit_rust(stream, &lease, &volume_state, &next)
            } {
                r.error =
                    c"Rust stream volume state changed during the serialized SetVolume transaction"
                        .as_ptr();
                return;
            }
            *result = 0;
        }
        DISCONTINUITY | FLUSH_EX | PAUSE => {
            let (action, refused): (u32, &'static std::ffi::CStr) = match r.entry {
                DISCONTINUITY => (
                    CONTROL_DISCONTINUITY,
                    c"Rust stream control policy refused Discontinuity",
                ),
                FLUSH_EX => (
                    CONTROL_FLUSH_EX,
                    c"Rust stream control policy refused FlushEx",
                ),
                _ => (CONTROL_PAUSE, c"Rust stream control policy refused Pause"),
            };
            let request = ControlRequest {
                action,
                argument: r.argument,
                count: r.count,
                apply: r.apply,
                completion_available: u32::from(config.completion),
                pause_callback_available: u32::from(config.pause_note.is_some()),
            };
            // SAFETY: Rust control policy plan and commit.
            if unsafe { tsfp_audio_stream_lease_control_apply_rust(stream, &lease, &request) } != 0
            {
                r.error = refused.as_ptr();
                return;
            }
            // Host request observation only: no voice, hardware, packet or playback state is synthesized (T1198).
            if r.entry == PAUSE {
                if let Some(note) = config.pause_note {
                    // SAFETY: host-installed note, called under the stream lock.
                    unsafe { note(stream, r.argument) };
                }
            }
            *result = 0;
        }
        MAX_DISTANCE => {
            if r.apply != 1 {
                r.error =
                    c"SetMaxDistance apply=0 enters the unmodeled settings commit helper".as_ptr();
                return;
            }
            // SAFETY: Rust spatial sidecar record.
            if !unsafe {
                tsfp_audio_stream_lease_max_distance_rust(stream, &lease, r.apply, r.argument)
            } {
                r.error = c"Rust stream lease refused the accepted SetMaxDistance".as_ptr();
                return;
            }
            *result = 0;
        }
        SET_POSITION => {
            // T1182: raw host record on a fully configured spatial stream; apply=0 enters the unmodeled commit helper.
            if scope.flags != 0x10 || cache_mask != 7 {
                r.error = c"only a fully configured spatial startup stream may record SetPosition"
                    .as_ptr();
                return;
            }
            if r.apply != 1 {
                r.error =
                    c"SetPosition apply=0 enters the unmodeled settings commit helper".as_ptr();
                return;
            }
            let mut next = spatial_state;
            next.position_bits = [r.argument, r.count, r.position_z];
            next.position_sets = spatial_state.position_sets.wrapping_add(1);
            next.revision = spatial_state.revision.wrapping_add(1);
            // SAFETY: Rust spatial sidecar commit.
            if !unsafe {
                tsfp_audio_stream_lease_spatial_commit_rust(stream, &lease, &spatial_state, &next)
            } {
                r.error = c"Rust stream spatial sidecar refused the accepted SetPosition".as_ptr();
                return;
            }
            *result = 0;
        }
        ROLLOFF
            if scope.flags == 0x10
                && cache_mask == 7
                && r.apply == 0
                && r.count == 4
                && matches!(r.argument, CURVE | CURVE_NEAR | CURVE_FAR) =>
        {
            // T1199: the sound update switches a configured spatial stream between the three 4 point curves; host
            // record of the pointer only.
            let mut next = spatial_state;
            next.curve_address = r.argument;
            next.curve_count = r.count;
            next.revision = spatial_state.revision.wrapping_add(1);
            // SAFETY: Rust spatial sidecar commit.
            if !unsafe {
                tsfp_audio_stream_lease_spatial_commit_rust(stream, &lease, &spatial_state, &next)
            } {
                r.error =
                    c"Rust stream spatial sidecar refused the accepted rolloff curve".as_ptr();
                return;
            }
            *result = 0;
        }
        _ => startup_spatial(r, &lease, scope.flags, internal, result),
    }
}

/// Startup spatial setters (apply=0) of a spatial stream: I3DL2, minimum distance and the first rolloff.
fn startup_spatial(
    r: &mut Access,
    lease: &Lease,
    scope_flags: u32,
    internal: u32,
    result: &mut u32,
) {
    if scope_flags != 0x10 || r.apply != 0 {
        r.error = c"only startup spatial setters/apply0 are supported".as_ptr();
        return;
    }
    let (action, count, words, refused): (u32, u32, *const u32, &'static std::ffi::CStr);
    let expected = [0u32, 0, 0xFFFF_F448, 0, 0, 0, 0, 0, 0];
    let mut words_storage = [0u32; 9];
    match r.entry {
        I3DL2 => {
            // SAFETY: guest read into local storage.
            let unreadable = aliases_headers(r.argument, 36, internal)
                || !unsafe {
                    kernel_guest_read_bytes(r.argument, words_storage.as_mut_ptr().cast(), 36)
                }
                || words_storage != expected;
            if unreadable {
                r.error = c"only exact first startup I3DL2 cache is supported".as_ptr();
                return;
            }
            (action, count, words) = (1, 0, words_storage.as_ptr());
            refused = c"Rust stream spatial sidecar refused startup I3DL2 state";
        }
        MINIMUM => {
            (action, count, words) = (2, 0, null());
            refused = c"Rust stream spatial sidecar refused startup minimum-distance state";
        }
        _ => {
            (action, count, words) = (3, r.count, null());
            refused = c"Rust stream spatial sidecar refused startup rolloff state";
        }
    }
    // SAFETY: Rust startup spatial commit; `words` is null or nine words.
    if unsafe {
        tsfp_audio_stream_lease_startup_spatial_rust(
            r.address, lease, action, r.argument, count, words,
        )
    } != 0
    {
        r.error = refused.as_ptr();
        return;
    }
    *result = 0;
}

/// SetFormat: stereo startup scope or a configured spatial stream after the started sequence; XADPCM family only
/// (T1159, T1181). The original validates nothing, so the family is policy.
fn set_format(
    r: &mut Access,
    config: &Config,
    lease: &Lease,
    format_state: &FormatState,
    format_rate: u32,
    (cache_mask, scope_flags, volume_seen, discontinuity_seen): (u32, u32, bool, bool),
    internal: u32,
) {
    // SAFETY: pure shape admission.
    let shape = unsafe {
        tsfp_audio_stream_format_admit_rust(
            scope_flags,
            cache_mask,
            u32::from(volume_seen),
            u32::from(discontinuity_seen),
        )
    };
    if shape == 0 {
        r.error = c"only the stereo startup scope or a configured spatial stream can change format"
            .as_ptr();
        return;
    }
    if shape == 3 {
        r.error = c"only a fully started (volume, Pause1, FlushEx, Discontinuity) stream can change format".as_ptr();
        return;
    }
    let mut words = [0u8; 20];
    if aliases_headers(r.argument, 20, internal) || overlap(r.argument, 20, CURVE, 16) {
        r.error = c"SetFormat input aliases protected state".as_ptr();
        return;
    }
    let channels = if shape == 1 { 2u32 } else { 1u32 };
    let mut rate = 0u32;
    // An unreadable input leaves the zero fill, which is never an XADPCM format.
    // SAFETY: local outputs.
    let family = unsafe {
        tsfp_audio_stream_format_decode_rust(
            host_pid(),
            r.argument,
            channels,
            words.as_mut_ptr(),
            &mut rate,
        )
    };
    if !family {
        r.error = c"only an ADPCM format of the stream's channel count (block 36 per channel, 64 samples, average rate*block/64, 8000 to 48000 Hz) at a readable address is supported".as_ptr();
        return;
    }
    // Original copies settings, resets pitch and programs the APU. The optional PCM/completion abort transaction
    // changes source time; guest settings/voice writes and hardware remain omitted.
    let mut frequency = FrequencyState::default();
    // SAFETY: local output.
    if !unsafe { tsfp_audio_stream_frequency_state_rust(r.address, lease.serial, &mut frequency) } {
        r.error = c"Rust frequency sidecar is unavailable for this lease".as_ptr();
        return;
    }
    let old_hz = if frequency.accepted_sets == 0 || frequency.current_hz == 0 {
        format_rate
    } else {
        frequency.current_hz
    };
    let frequency_sets = frequency.accepted_sets;
    let mut pitch = frequency.pitch;
    if frequency_sets != 0 || (config.completion && config.frequency_control_word.is_some()) {
        let mut control_word = 0u16;
        // SAFETY: host-installed control-word provider.
        let known = config
            .frequency_control_word
            .is_some_and(|read| unsafe { read(&mut control_word) });
        let mut note = config.format_note;
        let pitch_valid = known
            // SAFETY: the context outlives the synchronous call.
            && unsafe {
                tsfp_audio_stream_frequency_format_commit_rust(
                    r.address,
                    lease.serial,
                    frequency_sets,
                    old_hz,
                    rate,
                    control_word,
                    r.argument,
                    if note.is_some() { (&mut note as *mut Option<FormatNote>).cast() } else { null_mut() },
                    if note.is_some() { Some(frequency_format_apply as FormatApply) } else { None },
                    &mut pitch,
                )
            };
        if !pitch_valid {
            log_line(&format!(
                "dsound: format stream={} old={} new={} cw={} known={} pitch-valid={}\n",
                hex(r.address),
                old_hz,
                rate,
                hex(u32::from(control_word)),
                u32::from(known),
                u32::from(pitch_valid)
            ));
            r.error = c"SetFormat PCM/completion abort/reset rejected ownership, clock, words or guest CW".as_ptr();
            return;
        }
    }
    // SAFETY: Rust sidecar commit of the accepted format words.
    if !unsafe {
        tsfp_audio_stream_lease_format_commit_rust(r.address, lease, format_state, words.as_ptr())
    } {
        r.error = c"Rust stream format sidecar commit invariant failed".as_ptr();
    }
}

unsafe extern "C" fn access_owned(internal: u32, userdata: *mut c_void, result: *mut u32) {
    // SAFETY: `userdata` is the Access of `run`, `result` the device lock's output.
    let (request, result) = unsafe { (&mut *(userdata as *mut Access), &mut *result) };
    lock_streams();
    owned(request, internal, result);
    unlock_streams();
}

fn identify(address: u32, identity: &mut Lease) -> bool {
    // SAFETY: local output.
    unsafe { tsfp_audio_stream_lease_identity_rust(address, identity) }
}

/// Runs `request` against the device that owns the stream; `None` when the parent device changed.
fn run(request: &mut Access) -> Option<u32> {
    let mut result = 0u32;
    // SAFETY: `access_owned` only uses `request`, which outlives the synchronous call.
    unsafe {
        dsound_device_with_owned_interface(
            request.identity.internal_address.wrapping_add(8),
            access_owned,
            (request as *mut Access).cast(),
            &mut result,
        )
    }
    .then_some(result)
}

/// Measured spatial setters check global failure before object/settings access: `Ok(0)` or the failure HRESULT.
fn global_failure(
    entry: u32,
    reason: &'static std::ffi::CStr,
    admitted: impl Fn(&Config) -> bool,
) -> u32 {
    lock_streams();
    let config = config::get();
    let known = config::irql(&config);
    let admitted = config::enabled() && admitted(&config);
    unlock_streams();
    if !admitted || known != Some(0) {
        refuse(entry, reason.as_ptr());
    }
    match guest_read(GLOBAL_STATE) {
        None => refuse(entry, c"global audio state inaccessible".as_ptr()),
        Some(0) => 0,
        Some(_) => E_FAIL,
    }
}

fn spatial_preflight(entry: u32) -> u32 {
    global_failure(
        entry,
        c"spatial setter needs explicit policy at known IRQL0",
        |_| true,
    )
}

fn routing_preflight(entry: u32) -> u32 {
    global_failure(
        entry,
        c"routing needs explicit PCM/completion policy at known IRQL0",
        |c| c.completion && c.routing_note.is_some(),
    )
}

fn frequency_preflight() -> u32 {
    global_failure(
        SET_FREQUENCY,
        c"frequency needs explicit PCM/completion policy at IRQL0",
        |c| c.completion && c.frequency_note.is_some(),
    )
}

pub(crate) fn access_extended(
    entry: u32,
    address: u32,
    argument: u32,
    count: u32,
    apply: u32,
    position_z: u32,
) -> u32 {
    if entry == SET_POSITION || entry == MAX_DISTANCE {
        let failure = spatial_preflight(entry);
        if failure != 0 {
            return failure;
        }
    }
    let mut request = Access::new(entry, address);
    request.argument = argument;
    request.count = count;
    request.apply = apply;
    request.position_z = position_z;
    if !identify(address, &mut request.identity) {
        refuse(entry, c"stream is not owned by passive adapter".as_ptr());
    }
    let Some(result) = run(&mut request) else {
        refuse(
            entry,
            c"parent SILENT device ownership/generation changed".as_ptr(),
        );
    };
    if !request.error.is_null() {
        // SAFETY: static NUL-terminated strings.
        if unsafe { dsound_hle_degrade(c"stream".as_ptr(), entry, request.error) } {
            return 0;
        }
        refuse(entry, request.error);
    }
    result
}

pub(crate) fn access(entry: u32, address: u32, argument: u32, count: u32, apply: u32) -> u32 {
    access_extended(entry, address, argument, count, apply, 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_position(
    stream: u32,
    x: u32,
    y: u32,
    z: u32,
    apply: u32,
) -> u32 {
    access_extended(SET_POSITION, stream, x, y, apply, z)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_i3dl2(stream: u32, parameters: u32, apply: u32) -> u32 {
    access(I3DL2, stream, parameters, 0, apply)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_min_distance(stream: u32, bits: u32, apply: u32) -> u32 {
    access(MINIMUM, stream, bits, 0, apply)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_max_distance(stream: u32, bits: u32, apply: u32) -> u32 {
    access(MAX_DISTANCE, stream, bits, 0, apply)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_rolloff(
    stream: u32,
    curve: u32,
    count: u32,
    apply: u32,
) -> u32 {
    access(ROLLOFF, stream, curve, count, apply)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_pause(stream: u32, mode: u32) -> u32 {
    access(PAUSE, stream, mode, 0, 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_flush_ex(
    stream: u32,
    time_low: u32,
    time_high: u32,
    flags: u32,
) -> u32 {
    access(FLUSH_EX, stream, time_low, time_high, flags)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_discontinuity(stream: u32) -> u32 {
    access(DISCONTINUITY, stream, 0, 0, 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_get_startup_status(stream: u32, output: u32) -> u32 {
    access(STATUS, stream, output, 0, 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_frequency(stream: u32, hertz: u32, control_word: u16) -> u32 {
    let failure = frequency_preflight();
    if failure != 0 {
        failure
    } else {
        access(SET_FREQUENCY, stream, hertz, u32::from(control_word), 0)
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_headroom(stream: u32, headroom: u32) -> u32 {
    let failure = routing_preflight(HEADROOM);
    if failure != 0 {
        failure
    } else {
        access(HEADROOM, stream, headroom, 0, 0)
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_mix_bins(stream: u32, bins: u32) -> u32 {
    let failure = routing_preflight(MIX_BINS);
    if failure != 0 {
        failure
    } else {
        access(MIX_BINS, stream, bins, 0, 0)
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_set_mix_bin_volumes(stream: u32, bins: u32) -> u32 {
    let failure = routing_preflight(MIX_VOLUMES);
    if failure != 0 {
        failure
    } else {
        access(MIX_VOLUMES, stream, bins, 0, 0)
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_set_format(stream: u32, format: u32) -> u32 {
    access(SET_FORMAT, stream, format, 0, 0)
}
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_cache_volume(stream: u32, volume: i32) -> u32 {
    access(VOLUME, stream, volume as u32, 0, 0)
}

/// Read-only host observation of the passive policy. Failure preserves `output`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_get_snapshot(stream: u32, output: *mut Snapshot) -> bool {
    if output.is_null() {
        return false;
    }
    let mut value = Snapshot([0; 440]);
    let mut request = Access::new(0, stream);
    request.observer = true;
    request.snapshot = &mut value;
    if !identify(stream, &mut request.identity) {
        return false;
    }
    if run(&mut request).is_none() || !request.error.is_null() {
        return false;
    }
    // SAFETY: caller storage for one snapshot record.
    unsafe { *output = value };
    true
}
