// SPDX-License-Identifier: GPL-3.0-or-later
//! Quiescent per-object shutdown: the Reset device-lease transaction and the checked reset of all streams.
use crate::access::valid_node;
use crate::config;
use crate::ffi::*;
use crate::{STREAM_BYTES, lock_streams, log_line, unlock_streams};
use std::ffi::c_void;

struct ResetRequest {
    address: u32,
    identity: Lease,
}

unsafe extern "C" fn reset_prepare(
    candidate: *const Lease,
    userdata: *mut c_void,
    child: *mut LeaseChild,
) -> bool {
    // SAFETY: `userdata` is the ResetRequest of `release_host_c`, `candidate` and `child` the device lease's.
    let request = unsafe { &*(userdata as *const ResetRequest) };
    lock_streams();
    // SAFETY: as above.
    let candidate = unsafe { &*candidate };
    // SAFETY: lease identity comparison.
    if !unsafe { tsfp_audio_stream_lease_identity_matches_rust(candidate, &request.identity) }
        || !valid_node(
            request.address,
            &request.identity,
            candidate.internal_address,
        )
    {
        return false;
    }
    let mut heap = 0u32;
    // SAFETY: `heap` is local storage.
    if !unsafe { tsfp_audio_stream_lease_heap_rust(request.address, &mut heap) } || heap == 0 {
        return false;
    }
    // SAFETY: `child` is the device lease's output record.
    unsafe {
        *child = LeaseChild {
            heap,
            address: request.address,
            bytes: STREAM_BYTES,
        };
    }
    true
}

unsafe extern "C" fn reset_abort(_userdata: *mut c_void) {
    unlock_streams();
}

unsafe extern "C" fn reset_finalize(_committed: *const Lease, userdata: *mut c_void) {
    // SAFETY: see `reset_prepare`.
    let request = unsafe { &*(userdata as *const ResetRequest) };
    // SAFETY: Rust-owned lease and routing detach under the stream lock.
    unsafe {
        if !tsfp_audio_stream_lease_detach_rust(request.address, &request.identity) {
            log_line(&format!(
                "dsound: Rust stream lease detach invariant failed stream={} serial={}\n",
                crate::hex(request.address),
                request.identity.serial
            ));
        }
        let _ = tsfp_audio_stream_routing_retire_rust(request.address, request.identity.serial);
    }
    unlock_streams();
}

static RESET_OPS: LeaseOps = LeaseOps {
    prepare: reset_prepare,
    abort: reset_abort,
    finalize: reset_finalize,
};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_reset_release_host_c(
    address: u32,
    identity: *const Lease,
) -> u32 {
    if address == 0 || identity.is_null() {
        return LEASE_INVALID;
    }
    // SAFETY: `identity` is the caller's lease record.
    let identity = unsafe { *identity };
    let mut request = ResetRequest { address, identity };
    // SAFETY: the callbacks only use `request`, which outlives the synchronous call.
    unsafe {
        dsound_device_release_lease(
            &identity,
            &RESET_OPS,
            (&mut request as *mut ResetRequest).cast(),
        )
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_reset_validate_host_c(
    address: u32,
    identity: *const Lease,
) -> bool {
    if address == 0 || identity.is_null() {
        return false;
    }
    // SAFETY: caller's lease record.
    let identity = unsafe { *identity };
    valid_node(address, &identity, identity.internal_address)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_reset_heap_destroy_host_c(heap: u32) -> bool {
    // SAFETY: Rust-owned heap destroy.
    heap != 0 && unsafe { tsfp_audio_stream_heap_destroy_rust(heap) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_reset_log_host_c(code: u32, address: u32, serial: u64) {
    match code {
        1 => log_line(&format!(
            "dsound: Rust stream lease retire invariant failed stream={} serial={serial}\n",
            crate::hex(address)
        )),
        2 => log_line("dsound: Rust stream lease rollback retire invariant failed\n"),
        _ => {}
    }
}

/// Extension streams (T392 movie streams) lease the same device, so they go first.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_reset_checked() -> bool {
    let extension = config::get().reset_route;
    // SAFETY: the host-installed route takes no arguments.
    let movies = extension.is_none_or(|route| unsafe { route() });
    // SAFETY: reset mutex pair around the Rust reset of all streams.
    let result = unsafe {
        tsfp_audio_stream_reset_mutex_lock_rust();
        let result = tsfp_audio_stream_reset_rust();
        tsfp_audio_stream_reset_mutex_unlock_rust();
        result
    };
    movies && result
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_reset() {
    let _ = dsound_stream_reset_checked();
}
