// SPDX-License-Identifier: GPL-3.0-or-later
//! The per-method handlers of an owned movie stream (AddRef, Release, SetVolume, Pause, FlushEx, Flush, Discontinuity,
//! GetStatus, Process): each runs under the parent device mutex and the module lock through [`access_node`].
use crate::ffi::*;
use crate::state::*;
use std::ffi::c_void;

/// What an apply function may reach besides its node.
struct Aux<'a> {
    rollback: &'a [Node],
    volume_announced: &'a mut bool,
}

struct AccessRequest {
    address: u32,
    entry: u32,
    internal: u32,
    arg: [u32; 3],
    identity: Lease,
    error: Option<String>,
    destroy: bool,
    apply: fn(&mut Node, &mut AccessRequest, &mut Aux) -> u32,
}

impl AccessRequest {
    fn new(
        address: u32,
        entry: u32,
        apply: fn(&mut Node, &mut AccessRequest, &mut Aux) -> u32,
        arg: [u32; 3],
    ) -> Self {
        Self {
            address,
            entry,
            internal: 0,
            arg,
            identity: Lease::default(),
            error: None,
            destroy: false,
            apply,
        }
    }

    fn fail(&mut self, message: impl Into<String>) -> u32 {
        self.error = Some(message.into());
        0
    }
}

unsafe extern "C" fn access_owned(internal: u32, userdata: *mut c_void, result: *mut u32) {
    // SAFETY: the transaction passes the request and a writable result for this synchronous call.
    let r = unsafe { &mut *userdata.cast::<AccessRequest>() };
    lock();
    // SAFETY: the module lock is held until `unlock` below.
    let st = unsafe { state() };
    let State {
        nodes,
        rollback,
        volume_announced,
    } = st;
    let node = nodes.iter_mut().find(|n| n.stream_address == r.address);
    if !valid_node(node.as_deref(), &r.identity, internal) {
        r.error = Some("stream ownership/header/generation changed".into());
    } else if let Some(error) = policy_error() {
        r.error = Some(error.into());
    } else if let Some(node) = node {
        r.internal = internal;
        let mut aux = Aux {
            rollback,
            volume_announced,
        };
        let apply = r.apply;
        let value = apply(node, r, &mut aux);
        // SAFETY: writable result of the transaction.
        unsafe { result.write(value) };
    }
    unlock();
}

fn identify(address: u32) -> Option<Lease> {
    locked(|st| st.find(address).map(|n| n.lease))
}

/// Runs `apply` with the owned device interface held and the module lock taken.
fn access_node(r: &mut AccessRequest) -> u32 {
    count_operation();
    let Some(identity) = identify(r.address) else {
        refuse(r.entry, "stream is not owned by the movie model");
    };
    r.identity = identity;
    let mut result = 0u32;
    // SAFETY: the request outlives the synchronous call.
    let owned = unsafe {
        dsound_device_with_owned_interface(
            identity.internal_address + 8,
            access_owned,
            (&raw mut *r).cast(),
            &mut result,
        )
    };
    if !owned {
        refuse(r.entry, "parent SILENT device ownership/generation changed");
    }
    if let Some(error) = &r.error {
        refuse(r.entry, error);
    }
    result
}

/// Called with the lock held. A paused stream freezes the unplayed remainder of its head packet.
fn pause_stream(node: &mut Node, mode: u32, now: u64) -> bool {
    let mut result = 0u32;
    // SAFETY: writable local; scalar scheduler and runtime calls.
    unsafe {
        if !tsfp_audio_movie_schedule_pause_rust(node.stream_address, mode, now, &mut result)
            || result != 0
        {
            return false;
        }
        node.paused = true;
        node.pause_mode = mode;
        let _ = dsound_audio_runtime_set_stream_running(node.stream_address, now, false);
    }
    true
}

pub(crate) fn resume_stream(node: &mut Node, now: u64) -> bool {
    let mut result = 0u32;
    // SAFETY: writable local; scalar scheduler and runtime calls.
    unsafe {
        if !tsfp_audio_movie_schedule_pause_rust(node.stream_address, 0, now, &mut result)
            || result != 0
        {
            return false;
        }
        node.paused = false;
        node.pause_mode = 0;
        let _ = dsound_audio_runtime_set_stream_running(node.stream_address, now, true);
    }
    true
}

fn apply_add_ref(node: &mut Node, r: &mut AccessRequest, _aux: &mut Aux) -> u32 {
    let mut refs = 0u32;
    // SAFETY: writable local.
    if !unsafe { tsfp_audio_movie_schedule_add_ref_rust(node.stream_address, &mut refs) } {
        return r.fail("Rust movie reference increment refused");
    }
    node.header[2] = refs;
    refs
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_add_ref(stream: u32) -> u32 {
    access_node(&mut AccessRequest::new(
        stream,
        ADD_REF,
        apply_add_ref,
        [0; 3],
    ))
}

fn apply_set_volume(node: &mut Node, r: &mut AccessRequest, aux: &mut Aux) -> u32 {
    // HOST request cache only. The original also writes its settings object and APU gains, both omitted. It validates
    // nothing (oracle: every int32 returns 0), so neither does this.
    node.volume = r.arg[0] as i32;
    node.volume_seen = true;
    // With the audio runtime the soundtrack gain follows (T818). The title builds the value as round(2000 * log10(gain))
    // clamped to -10000..0 (sub_00028180), so anything else is unmeasured for the PCM and refused by name.
    // SAFETY: scalar runtime calls and a writable local.
    unsafe {
        if dsound_audio_runtime_active() {
            let mut gain = 0u32;
            if !tsfp_audio_volume_to_gain_q16(node.volume, &mut gain) {
                return r.fail(
                    "SetVolume outside the DirectSound range -10000..0 has no measured movie gain",
                );
            }
            if !dsound_audio_runtime_set_stream_volume(node.stream_address, node.volume) {
                return r.fail("the audio mixer refused the movie stream volume");
            }
            if node.volume != 0 && !*aux.volume_announced {
                *aux.volume_announced = true;
                log_line(
                    "dsound: movie stream SetVolume is applied to the soundtrack gain \
                     (10^(hundredths dB / 2000), -10000 mutes)\n",
                );
            }
        }
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_set_volume(stream: u32, volume: i32) -> u32 {
    access_node(&mut AccessRequest::new(
        stream,
        VOLUME,
        apply_set_volume,
        [volume as u32, 0, 0],
    ))
}

fn apply_pause(node: &mut Node, r: &mut AccessRequest, _aux: &mut Aux) -> u32 {
    let mode = r.arg[0];
    trace(|| {
        format!(
            "pause mode {mode} tick {} queued {}",
            now_ticks(),
            node.queued
        )
    });
    if mode >= 4 {
        return E_FAIL_STATUS; // the original fails modes 4 and up (oracle)
    }
    if mode == 3 {
        return r.fail("Pause mode 3 has no measured XMV use");
    }
    let now = now_ticks();
    if mode == 0 {
        if !resume_stream(node, now) {
            return r.fail("Rust movie scheduler refused Resume");
        }
    } else if !pause_stream(node, mode, now) {
        return r.fail("Rust movie scheduler refused Pause");
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_pause(stream: u32, mode: u32) -> u32 {
    access_node(&mut AccessRequest::new(
        stream,
        PAUSE,
        apply_pause,
        [mode, 0, 0],
    ))
}

fn apply_flush_ex(node: &mut Node, r: &mut AccessRequest, _aux: &mut Aux) -> u32 {
    if r.arg[0] != 0 || r.arg[1] != 0 || r.arg[2] != 1 {
        return r.fail("only the measured FlushEx(time 0, flags 1) is supported");
    }
    if node.queued == 0 {
        return 0; // nothing to flush, the original changes nothing (oracle)
    }
    let now = now_ticks();
    if node.paused && !resume_stream(node, now) {
        // an async flush lets a paused stream drain (oracle)
        return r.fail("Rust movie scheduler refused FlushEx resume");
    }
    // SAFETY: scalar scheduler and runtime calls.
    unsafe {
        if !tsfp_audio_movie_schedule_flush_ex_rust(node.stream_address, 0, 0, 1, now) {
            return r.fail("Rust movie scheduler refused FlushEx policy");
        }
        node.flush_pending = true;
        node.flush_tick = now;
        let _ = dsound_audio_runtime_cut_stream(node.stream_address, now);
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_flush_ex(
    stream: u32,
    time_low: u32,
    time_high: u32,
    flags: u32,
) -> u32 {
    access_node(&mut AccessRequest::new(
        stream,
        FLUSH_EX,
        apply_flush_ex,
        [time_low, time_high, flags],
    ))
}

/// `IDirectSoundStream::Flush` (0x407388, T394), measured on the original bytes, identical to the synchronous
/// FlushEx(0, 0, 0): every attached packet completes at once with its full size and 0x80004004 and its callback, in FIFO
/// order, and a paused stream is left running (oracle). The model aborts all of them, a packet whose deadline passed but
/// that no DoWork observed included, exactly as the final Release does.
fn apply_flush(node: &mut Node, r: &mut AccessRequest, _aux: &mut Aux) -> u32 {
    trace(|| format!("flush tick {} queued {}", now_ticks(), node.queued));
    if node.paused && !resume_stream(node, now_ticks()) {
        return r.fail("Rust movie scheduler refused Flush resume");
    }
    // SAFETY: scalar runtime call.
    let _ = unsafe { dsound_audio_runtime_cut_stream(node.stream_address, now_ticks()) };
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_flush(stream: u32) -> u32 {
    let result = access_node(&mut AccessRequest::new(stream, FLUSH, apply_flush, [0; 3]));
    let _ = crate::drain::drain(FLUSH, true, stream);
    result
}

fn apply_discontinuity(node: &mut Node, _r: &mut AccessRequest, _aux: &mut Aux) -> u32 {
    // HOST request observation only. The original sets a voice flag the omitted APU consumes.
    node.discontinuity_seen = true;
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_discontinuity(stream: u32) -> u32 {
    access_node(&mut AccessRequest::new(
        stream,
        DISCONTINUITY,
        apply_discontinuity,
        [0; 3],
    ))
}

fn apply_get_status(node: &mut Node, r: &mut AccessRequest, aux: &mut Aux) -> u32 {
    let output = r.arg[0];
    let aliases = aux
        .rollback
        .iter()
        .any(|n| n.stream_address != 0 && overlap(output, 4, n.stream_address, STREAM_BYTES));
    // SAFETY: scalar scheduler call; guest spans are validated inside.
    if aliases
        || !unsafe {
            tsfp_audio_movie_schedule_get_status_rust(
                host_pid(),
                node.stream_address,
                r.internal,
                output,
            )
        }
    {
        return r.fail("status output is NULL, inaccessible or aliases protected state");
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_get_status(stream: u32, output: u32) -> u32 {
    access_node(&mut AccessRequest::new(
        stream,
        GET_STATUS,
        apply_get_status,
        [output, 0, 0],
    ))
}

/// The original stores dwords through the completion pointers and never aligns them (oracle, offsets 1 to 3), and the
/// retained XMV keeps them at odd addresses (T394), so only overlap is refused.
fn word_probe(rollback: &[Node], address: u32, internal: u32, stream: u32) -> bool {
    if address == 0 || u64::from(address) + 4 > 1 << 32 {
        return false;
    }
    if rollback
        .iter()
        .any(|n| n.stream_address != 0 && overlap(address, 4, n.stream_address, STREAM_BYTES))
    {
        return false;
    }
    // SAFETY: scalar scheduler and completion probes.
    unsafe {
        tsfp_audio_movie_schedule_completion_word_protected_rust(stream, internal, address)
            && tsfp_audio_movie_completion_word_probe_rust(host_pid(), address)
    }
}

/// The attached packet is the XMV decoder's PCM (the one admitted format: 44100 Hz stereo signed 16-bit little endian).
fn apply_process(node: &mut Node, r: &mut AccessRequest, aux: &mut Aux) -> u32 {
    if r.arg[1] != 0 {
        return r.fail("Process output packet must be NULL as the retained XMV passes it");
    }
    // The original rejects before reading the packet when two are attached (oracle).
    node.queued = queued_count(node);
    let stream = node.stream_address;
    // SAFETY: scalar scheduler query.
    if !unsafe { tsfp_audio_movie_schedule_has_capacity_rust(stream) } {
        return TOO_MANY_PACKETS;
    }
    let mut words = [0u32; 6];
    // SAFETY: six writable words.
    if !unsafe { tsfp_audio_movie_packet_decode_rust(host_pid(), r.arg[0], words.as_mut_ptr()) } {
        return r.fail("packet is unreadable");
    }
    // words[5] is the timestamp slot: the retained XMV leaves it unset and nothing here reads it.
    let [buffer, size, completed, status, context, _] = words;
    // SAFETY: scalar scheduler query.
    let admission =
        unsafe { tsfp_audio_movie_schedule_admission_rust(stream, size, completed, status) };
    if admission == 2 {
        return TOO_MANY_PACKETS;
    }
    if admission == 5 {
        return r.fail("completion words alias an attached packet");
    }
    if admission != 0 {
        return r.fail(format!(
            "Rust movie scheduler rejected packet admission (reason {admission}, buffer 0x{buffer:08X} size 0x{size:X} \
             completed 0x{completed:08X} status 0x{status:08X} context 0x{context:08X} queued {})",
            node.queued
        ));
    }
    // SAFETY: scalar probe of the Rust packet model.
    let payload_readable = buffer != 0
        && unsafe { tsfp_audio_stream_packet_buffer_probe_rust(host_pid(), buffer, size) };
    let completed_ok = word_probe(aux.rollback, completed, r.internal, stream);
    let status_ok = word_probe(aux.rollback, status, r.internal, stream);
    if !payload_readable || !completed_ok || !status_ok {
        // Name the measured values, a refusal must say which condition failed (T394).
        return r.fail(format!(
            "packet buffer, size or completion words are outside the measured scope (buffer 0x{buffer:08X} size 0x{size:X} \
             completed 0x{completed:08X} status 0x{status:08X} context 0x{context:08X}, queued {}, buffer readable {}, \
             completed ok {}, status ok {})",
            node.queued,
            u32::from(payload_readable),
            u32::from(completed_ok),
            u32::from(status_ok)
        ));
    }
    // Sub-block sizes (below 4) are accepted by the original but never attached: they would stay pending forever, so
    // they are refused above. Distinct completion words keep order visible.
    // SAFETY: scalar completion and scheduler calls.
    unsafe {
        if !tsfp_audio_movie_pending_publish_rust(host_pid(), completed, status) {
            return r.fail("completion words are not writable");
        }
        let _ = tsfp_audio_movie_schedule_submit_pcm_rust(
            host_pid(),
            stream,
            now_ticks(),
            buffer,
            size,
        );
        if !tsfp_audio_movie_schedule_enqueue_rust(
            stream,
            size,
            completed,
            status,
            context,
            now_ticks(),
        ) {
            return r.fail("Rust movie scheduler refused admitted packet");
        }
    }
    node.queued += 1;
    let mut schedule = ScheduleSnapshot::default();
    // SAFETY: writable snapshot.
    let deadline = if unsafe { tsfp_audio_movie_schedule_snapshot_rust(stream, &mut schedule) } {
        schedule.head_deadline
    } else {
        0
    };
    trace(|| {
        format!(
            "process size 0x{size:X} context 0x{context:08X} tick {} deadline {deadline} queued {} paused {}",
            now_ticks(),
            node.queued,
            u32::from(node.paused)
        )
    });
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_process(stream: u32, packet: u32, output_packet: u32) -> u32 {
    access_node(&mut AccessRequest::new(
        stream,
        PROCESS,
        apply_process,
        [packet, output_packet, 0],
    ))
}

fn apply_release(node: &mut Node, r: &mut AccessRequest, _aux: &mut Aux) -> u32 {
    let mut is_final = false;
    // SAFETY: writable local.
    if !unsafe {
        tsfp_audio_movie_schedule_release_is_final_rust(node.stream_address, &mut is_final)
    } {
        return r.fail("Rust movie reference state unavailable");
    }
    if is_final {
        r.destroy = true;
        return 0;
    }
    if node.header[2] == 0 {
        return r.fail("reference count underflow");
    }
    let mut refs = 0u32;
    // SAFETY: writable local.
    if !unsafe { tsfp_audio_movie_schedule_release_ref_rust(node.stream_address, &mut refs) } {
        return r.fail("Rust movie reference decrement refused");
    }
    node.header[2] = refs;
    refs
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_release(stream: u32) -> u32 {
    let mut r = AccessRequest::new(stream, RELEASE, apply_release, [0; 3]);
    let result = access_node(&mut r);
    if !r.destroy {
        return result;
    }
    // SAFETY: scalar call of the Rust teardown.
    if !unsafe { tsfp_audio_movie_release_teardown_rust(stream) } {
        refuse(RELEASE, "Rust final movie Release teardown refused");
    }
    0
}
