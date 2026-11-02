// SPDX-License-Identifier: GPL-3.0-or-later
//! `dsound_movie_stream_create`: the device-lease transaction that attaches one XMV movie stream.
use crate::ffi::*;
use crate::state::*;
use std::ffi::c_void;

struct CreateRequest {
    scope: Scope,
    output: u32,
    status: u32,
    node: Option<Node>,
    error: Option<&'static str>,
    announce: bool,
    scheduler_registered: bool,
    created: u32,
}

fn header_bytes(header: &[u32; 10]) -> [u8; 40] {
    let mut bytes = [0u8; 40];
    for (chunk, word) in bytes.chunks_exact_mut(4).zip(header) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    bytes
}

/// Lease callbacks run under the device mutex. The module lock taken here is released in `create_abort` or
/// `create_finalize`; cleanup and fatal run only after the acquire returns.
unsafe extern "C" fn create_prepare(
    candidate: *const Lease,
    userdata: *mut c_void,
    child: *mut LeaseChild,
) -> bool {
    // SAFETY: the transaction passes the request and a committed candidate for this synchronous call.
    let (r, candidate) = unsafe { (&mut *userdata.cast::<CreateRequest>(), &*candidate) };
    lock();
    r.error = policy_error();
    if r.error.is_some() {
        return false;
    }
    // SAFETY: the module lock was taken above and is released by the abort or finalize callback.
    let st = unsafe { state() };
    // SAFETY: plain model query.
    let stopped = unsafe { dsound_stream_original_tables_stopped() };
    let Some(old) = stopped.then(|| read_u32(r.output)).flatten() else {
        r.error = Some("output unreadable or original vtable contains an unprotected target");
        return false;
    };
    let sc = r.scope;
    if st.aliases_state(r.output, 4, candidate.internal_address)
        || overlap(r.output, 4, sc.descriptor, 24)
        || overlap(r.output, 4, sc.format, 18)
        || overlap(sc.descriptor, 24, candidate.internal_address, 44)
        || overlap(sc.format, 18, candidate.internal_address, 44)
    {
        r.error = Some("unproven output/input alias with owned state");
        return false;
    }
    // Same-byte probe preserves contents, but is explicitly a guest write.
    // SAFETY: guest accessors validate the span themselves.
    if !unsafe { kernel_guest_write_u32(r.output, old) }
        || unsafe { kernel_guest_at_ro(r.output, 4) }.is_null()
    {
        r.error = Some("output is not writable");
        return false;
    }
    let node = r.node.insert(Node::new(r.output, sc.callback, sc.context));
    let heap = heap_create();
    node.stream_heap = heap;
    if heap == 0 {
        r.status = OOM;
        return false;
    }
    let address = heap_alloc(heap, STREAM_BYTES);
    node.stream_address = address;
    if address == 0 {
        r.status = OOM;
        return false;
    }
    node.header[0] = VTABLE;
    node.header[1] = SECONDARY;
    node.header[2] = 1;
    node.header[3] = candidate.internal_address;
    let bytes = header_bytes(&node.header);
    // SAFETY: a 40-byte readable local.
    let wrote = unsafe { kernel_guest_write_bytes(address, bytes.as_ptr().cast(), bytes.len()) };
    if st.aliases_state(address, STREAM_BYTES, candidate.internal_address)
        || overlap(address, STREAM_BYTES, r.output, 4)
        || overlap(address, STREAM_BYTES, sc.descriptor, 24)
        || overlap(address, STREAM_BYTES, sc.format, 18)
        || !wrote
    {
        r.error = Some("new private stream allocation/alias is invalid");
        return false;
    }
    // SAFETY: scalar calls of the Rust scheduler.
    let attached = unsafe {
        tsfp_audio_movie_schedule_configure_rust(clock_frequency())
            && tsfp_audio_movie_schedule_attach_rust(
                address,
                candidate.internal_address,
                r.output,
                sc.callback,
                sc.context,
            )
    };
    if !attached {
        r.error = Some("Rust movie scheduler refused stream registration");
        return false;
    }
    r.scheduler_registered = true;
    // SAFETY: `child` is the writable output of the transaction.
    unsafe {
        child.write(LeaseChild {
            heap,
            address,
            bytes: STREAM_BYTES,
        })
    };
    true
}

unsafe extern "C" fn create_abort(_userdata: *mut c_void) {
    unlock();
}

fn cleanup_aborted_create(r: &mut CreateRequest) {
    let Some(node) = r.node.take() else { return };
    lock();
    // SAFETY: the module lock is held until `unlock` below.
    let st = unsafe { state() };
    if r.scheduler_registered {
        // SAFETY: scalar call of the Rust scheduler.
        let _ = unsafe { tsfp_audio_movie_schedule_detach_rust(node.stream_address) };
        r.scheduler_registered = false;
    }
    let heap = node.stream_heap;
    if heap == 0 || (heap_valid(heap) && heap_destroy(heap)) {
        drop(node);
    } else {
        st.rollback.insert(0, node);
        r.error = Some("private allocation rollback failed; unleased cleanup retained");
    }
    unlock();
}

unsafe extern "C" fn create_finalize(committed: *const Lease, userdata: *mut c_void) {
    // SAFETY: as in `create_prepare`; the module lock is still held from there.
    let (r, committed, st) =
        unsafe { (&mut *userdata.cast::<CreateRequest>(), *committed, state()) };
    if let Some(mut node) = r.node.take() {
        node.lease = committed;
        r.created = node.stream_address;
        // SAFETY: the Rust diagnostics service.
        r.announce = unsafe { tsfp_audio_movie_diagnostics_announce_once_rust() };
        let (address, output) = (node.stream_address, r.output);
        st.nodes.push(node);
        // SAFETY: guest accessor validates the span itself.
        let _ = unsafe { kernel_guest_write_u32(output, address) };
    }
    unlock();
}

static CREATE_OPS: LeaseOps = LeaseOps {
    prepare: create_prepare,
    abort: create_abort,
    finalize: create_finalize,
};

const RUNTIME_ANNOUNCE: &str = "dsound: explicit HEADLESS movie stream; bounded CPU model of the XMV PCM stream (2 packets, \
44100 Hz stereo 16-bit). Its PCM packets feed the audio mixer and sink on the virtual clock (timing INFERRED, the same as \
the menu music). Packets complete only in DirectSoundDoWork at the format byte rate, callback included. APU, DSP and \
interrupt objects are omitted\n";
const SILENT_ANNOUNCE: &str = "dsound: explicit HEADLESS movie stream; bounded CPU model of the XMV PCM stream (2 packets, \
44100 Hz stereo 16-bit). No audio is produced. Packets complete only in DirectSoundDoWork on the virtual clock at the \
format byte rate, callback included. APU, DSP and interrupt objects are omitted\n";

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_create(descriptor: u32, output: u32) -> u32 {
    count_operation();
    if let Some(error) = policy_error() {
        refuse(CREATE, error);
    }
    let Some(scope) = scope_snapshot(descriptor) else {
        refuse(
            CREATE,
            "descriptor/format outside the measured XMV movie scope",
        );
    };
    let device = match read_u32(SINGLETON) {
        Some(device) if device != 0 && device <= u32::MAX - 8 => device,
        _ => refuse(CREATE, "no owned SILENT device"),
    };
    let mut r = CreateRequest {
        scope,
        output,
        status: 0,
        node: None,
        error: None,
        announce: false,
        scheduler_registered: false,
        created: 0,
    };
    let mut lease = Lease::default();
    // SAFETY: the request outlives the synchronous transaction; the ops are static.
    let status = unsafe {
        dsound_device_acquire_lease(device + 8, &CREATE_OPS, (&raw mut r).cast(), &mut lease)
    };
    if status != LEASE_OK {
        cleanup_aborted_create(&mut r);
    }
    if status == LEASE_OUT_OF_MEMORY
        || (status == LEASE_PREPARE_FAILED && r.status == OOM && r.error.is_none())
    {
        return OOM;
    }
    if status != LEASE_OK {
        refuse(
            CREATE,
            r.error.unwrap_or("device lease transaction refused"),
        );
    }
    // The mixer keys persistent voice state by guest address. A reused address starts at unity; resetting an absent key
    // must not reserve a slot.
    // SAFETY: scalar call of the Rust scheduler.
    let _ = unsafe { tsfp_audio_movie_schedule_reset_volume_rust(r.created) };
    if r.announce {
        // SAFETY: plain query.
        let runtime = unsafe { dsound_audio_runtime_active() };
        log_line(if runtime {
            RUNTIME_ANNOUNCE
        } else {
            SILENT_ANNOUNCE
        });
    }
    0
}
