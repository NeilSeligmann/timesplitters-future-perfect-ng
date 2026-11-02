// SPDX-License-Identifier: GPL-3.0-or-later
//! Host configuration setters, diagnostics counters, the snapshot and the reset edges of the movie stream glue.
use crate::ffi::*;
use crate::state::*;

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_set_trace(on: bool) {
    set_trace(on);
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_set_irql_provider(provider: Option<IrqlProvider>) {
    set_irql_provider(provider);
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_set_fatal(handler: Option<FatalFn>) {
    set_fatal(handler);
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_set_clock(clock: Option<ClockFn>, frequency: u64) {
    if !set_clock(clock, frequency) {
        std::process::abort();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_set_callback_runner(runner: Option<CallbackFn>) {
    // SAFETY: stores the host-supplied runner in the Rust completion service.
    if !unsafe { tsfp_audio_movie_completion_set_runner_rust(runner) } {
        std::process::abort();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_completion_count() -> u64 {
    // SAFETY: scalar counter.
    unsafe { tsfp_audio_movie_completion_count_rust() }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_operation_count() -> u64 {
    locked(|_| {
        // SAFETY: scalar counter.
        unsafe { tsfp_audio_movie_diagnostics_operation_count_rust() }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_pcm_packets() -> u64 {
    // SAFETY: scalar counter.
    unsafe { tsfp_audio_movie_pcm_packets_rust() }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_pcm_frames() -> u64 {
    // SAFETY: scalar counter.
    unsafe { tsfp_audio_movie_pcm_frames_rust() }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_pcm_refused() -> u64 {
    // SAFETY: scalar counter.
    unsafe { tsfp_audio_movie_pcm_refused_rust() }
}

/// # Safety
/// `output` must point to a writable `dsound_movie_stream_snapshot`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_movie_stream_get_snapshot(
    stream: u32,
    output: *mut StreamSnapshot,
) -> bool {
    if output.is_null() {
        return false;
    }
    locked(|st| {
        let Some(node) = st.find(stream) else {
            return false;
        };
        let mut schedule = ScheduleSnapshot::default();
        // SAFETY: writable snapshot.
        if !unsafe { tsfp_audio_movie_schedule_snapshot_rust(stream, &mut schedule) } {
            return false;
        }
        let mut packet_size = [0u32; 2];
        for (slot, size) in packet_size
            .iter_mut()
            .zip(schedule.packet_size)
            .take(schedule.queued as usize)
        {
            *slot = size;
        }
        // SAFETY: writable by contract.
        unsafe {
            output.write(StreamSnapshot {
                stream_address: node.stream_address,
                refs: schedule.refs,
                callback: node.callback,
                context: node.context,
                volume: node.volume,
                volume_seen: node.volume_seen,
                discontinuity_seen: node.discontinuity_seen,
                flush_pending: schedule.flush_pending != 0,
                pause_mode: schedule.pause_mode,
                queued: schedule.queued,
                packet_size,
                head_deadline: schedule.head_deadline,
            });
        }
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_reset_checked() -> bool {
    let _serial = RESET_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: the Rust-owned reset sweep.
    unsafe { tsfp_audio_movie_stream_reset_rust() }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_reset() {
    let _ = dsound_movie_stream_reset_checked();
}

// Narrow host edges of the Rust-owned reset and Release sweeps: the schedule chooses the next address, this module
// validates the lease and performs the synchronous detach.

/// Detach one node from its device and free its private allocation. False when refused.
fn destroy_node(address: u32, identity: &Lease) -> bool {
    struct Detach {
        address: u32,
        identity: Lease,
        node: Option<Node>,
    }
    unsafe extern "C" fn prepare(
        candidate: *const Lease,
        userdata: *mut std::ffi::c_void,
        child: *mut LeaseChild,
    ) -> bool {
        // SAFETY: the transaction passes the request and a committed candidate for this synchronous call.
        let (r, candidate) = unsafe { (&*userdata.cast::<Detach>(), &*candidate) };
        lock();
        // SAFETY: the module lock was taken above and is released by the abort or finalize callback.
        let st = unsafe { state() };
        let node = st.find(r.address);
        if !token_equal(candidate, &r.identity)
            || !valid_node(node, &r.identity, candidate.internal_address)
        {
            return false;
        }
        let heap = node.map_or(0, |n| n.stream_heap);
        // SAFETY: writable output of the transaction.
        unsafe {
            child.write(LeaseChild {
                heap,
                address: r.address,
                bytes: STREAM_BYTES,
            })
        };
        true
    }
    unsafe extern "C" fn abort(_userdata: *mut std::ffi::c_void) {
        unlock();
    }
    unsafe extern "C" fn finalize(_committed: *const Lease, userdata: *mut std::ffi::c_void) {
        // SAFETY: as in `prepare`, the module lock is still held.
        let (r, st) = unsafe { (&mut *userdata.cast::<Detach>(), state()) };
        // SAFETY: scalar call of the Rust scheduler.
        if !unsafe { tsfp_audio_movie_schedule_detach_rust(r.address) } {
            std::process::abort();
        }
        if let Some(index) = st.nodes.iter().position(|n| n.stream_address == r.address) {
            r.node = Some(st.nodes.remove(index));
        }
        unlock();
    }
    static OPS: LeaseOps = LeaseOps {
        prepare,
        abort,
        finalize,
    };
    let mut request = Detach {
        address,
        identity: *identity,
        node: None,
    };
    // SAFETY: the request outlives the synchronous transaction; the ops are static.
    let status = unsafe { dsound_device_release_lease(identity, &OPS, (&raw mut request).cast()) };
    if status != LEASE_OK {
        return false;
    }
    request
        .node
        .is_some_and(|node| heap_valid(node.stream_heap) && heap_destroy(node.stream_heap))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_reset_destroy_host_c(address: u32) -> bool {
    let identity = locked(|st| st.find(address).map(|n| n.lease));
    identity.is_some_and(|identity| destroy_node(address, &identity))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_release_drain_host_c(address: u32) -> bool {
    let _ = crate::drain::drain(RELEASE, true, address);
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_reset_rollback_first_host_c() -> u32 {
    locked(|st| st.rollback.first().map_or(0, |n| n.stream_address))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_reset_rollback_destroy_host_c(address: u32) -> bool {
    locked(|st| {
        let Some(node) = st.rollback.first() else {
            return false;
        };
        if node.stream_address != address
            || !heap_valid(node.stream_heap)
            || !heap_destroy(node.stream_heap)
        {
            return false;
        }
        st.rollback.remove(0);
        true
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_reset_tail_host_c() -> bool {
    locked(|st| {
        if !st.nodes.is_empty() || !st.rollback.is_empty() {
            return false;
        }
        // SAFETY: the Rust diagnostics reset.
        unsafe { tsfp_audio_movie_diagnostics_reset_rust() };
        true
    })
}
