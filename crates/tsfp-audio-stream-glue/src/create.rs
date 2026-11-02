// SPDX-License-Identifier: GPL-3.0-or-later
//! The Create device-lease transaction. The Rust model (`tsfp-audio-capi`, stream_create_hle_ffi.rs) owns the lifecycle
//! records; this module owns the synchronous request that lives on this stack for the whole lease callback sequence.
use crate::ffi::*;
use crate::scope::dsound_stream_scope_snapshot;
use crate::{CREATE, config, hex, lock_streams, log_line, policy_error, refuse, unlock_streams};
use std::ffi::{c_char, c_void};
use std::ptr::null_mut;

const OOM: u32 = 0x8007_000E;
const OUTCOME_READY: u32 = 0;
const OUTCOME_OOM: u32 = 1;
const ANNOUNCEMENT: &str = "dsound: explicit HEADLESS streams; passive40-byte public headers and host startup setter caches; omitted settings/hardware/list/packet objects, APU/DSP/FP/critical-section operations; no audio output or packet completion\n";

#[repr(C)]
struct FinalizeContext {
    host_pid: i32,
    stream_address: u32,
    output_address: u32,
    routing_reserved: u32,
    failures: u32,
    announce: u32,
}

/// Layout shared with the Rust prepare/cleanup/finalize callbacks (128 bytes).
#[repr(C)]
struct CreateRequest {
    finalize: FinalizeContext,
    scope: Scope,
    output: u32,
    status: u32,
    node: *mut c_void,
    error: *const c_char,
    announce: bool,
    routing_reserved: bool,
    lease_reserved: bool,
}
const _: () = assert!(std::mem::size_of::<FinalizeContext>() == 24);
const _: () = assert!(std::mem::size_of::<CreateRequest>() == 128);

static CREATE_OPS: LeaseOps = LeaseOps {
    prepare: tsfp_audio_stream_create_prepare_callback_rust,
    abort: tsfp_audio_stream_create_abort_unlock_rust,
    finalize: tsfp_audio_stream_create_finalize_callback_rust,
};

fn cleanup_aborted(request: &mut CreateRequest) {
    if request.node.is_null() {
        return;
    }
    // SAFETY: the Rust cleanup reads exactly this request layout.
    unsafe {
        tsfp_audio_stream_create_cleanup_aborted_rust((request as *mut CreateRequest).cast())
    };
}

/// Host edge of the Rust-owned frame-to-lease transaction; callbacks stay synchronous so the stack request and lock
/// lifetime match the oracle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_acquire_host_c(
    device: u32,
    request: *mut c_void,
    status: *mut u32,
    lease: *mut Lease,
) {
    if status.is_null() || lease.is_null() {
        return;
    }
    // SAFETY: caller supplies the request the Rust callbacks understand.
    unsafe { *status = dsound_device_acquire_lease(device, &CREATE_OPS, request, lease) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_cleanup_host_c(request: *mut c_void) {
    // SAFETY: a CreateRequest of this module.
    cleanup_aborted(unsafe { &mut *(request as *mut CreateRequest) });
}

fn log_failures(request: &CreateRequest, serial: u64) {
    let stream = hex(request.finalize.stream_address);
    if request.finalize.failures & 1 != 0 {
        log_line(&format!(
            "dsound: Rust stream lease bind invariant failed stream={stream} serial={serial}\n"
        ));
    }
    if request.finalize.failures & 2 != 0 {
        log_line(&format!(
            "dsound: Rust stream routing lease bind invariant failed stream={stream} serial={serial}\n"
        ));
    }
    if request.finalize.failures & 4 != 0 {
        log_line(&format!(
            "dsound: Rust stream output publication failed after lease commit stream={stream}\n"
        ));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_report_host_c(
    request: *const c_void,
    lease: *const Lease,
    status: u32,
) {
    if request.is_null() || lease.is_null() {
        return;
    }
    // SAFETY: a CreateRequest and lease of the caller.
    let (request, lease) = unsafe { (&*(request as *const CreateRequest), &*lease) };
    log_failures(request, lease.serial);
    if status == LEASE_OK && request.announce {
        log_line(ANNOUNCEMENT);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_stream_create_refuse_host_c(reason: *const c_char) -> ! {
    let text = if reason.is_null() {
        c"device lease transaction refused".as_ptr()
    } else {
        reason
    };
    refuse(CREATE, text)
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_create(descriptor: u32, output: u32) -> u32 {
    // SAFETY: zero is a valid CreateRequest (null pointers, false flags, zero scope).
    let mut request: CreateRequest = unsafe { std::mem::zeroed() };
    request.output = output;
    request.finalize.host_pid = crate::host_pid();
    lock_streams();
    let error = policy_error(&config::get());
    unlock_streams();
    if !error.is_null() {
        refuse(CREATE, error);
    }
    // SAFETY: `scope` is the request's own storage.
    if !unsafe { dsound_stream_scope_snapshot(descriptor, &mut request.scope) } {
        refuse(
            CREATE,
            c"descriptor/format outside measured startup scope".as_ptr(),
        );
    }
    let mut device = 0u32;
    // SAFETY: local output.
    if !unsafe { tsfp_audio_stream_device_singleton_rust(crate::host_pid(), &mut device) }
        || device == 0
        || device > u32::MAX - 8
    {
        refuse(CREATE, c"no owned SILENT device".as_ptr());
    }
    let mut lease = Lease::default();
    // SAFETY: the callbacks only use `request`, which outlives the synchronous call.
    let status = unsafe {
        dsound_device_acquire_lease(
            device + 8,
            &CREATE_OPS,
            (&mut request as *mut CreateRequest).cast(),
            &mut lease,
        )
    };
    request.announce = request.finalize.announce != 0;
    log_failures(&request, lease.serial);
    if status != LEASE_OK {
        cleanup_aborted(&mut request);
    }
    // The committed lifecycle, address, and heap now live only in Rust.
    // SAFETY: releases the rollback node the Rust prepare allocated (null is accepted).
    unsafe { tsfp_audio_stream_create_release_request_rust(request.node) };
    request.node = null_mut();
    // SAFETY: pure outcome classification.
    let outcome = unsafe {
        tsfp_audio_stream_create_outcome_rust(
            status,
            request.status,
            u32::from(!request.error.is_null()),
        )
    };
    if outcome == OUTCOME_OOM {
        return OOM;
    }
    if outcome != OUTCOME_READY {
        let reason = if request.error.is_null() {
            c"device lease transaction refused".as_ptr()
        } else {
            request.error
        };
        refuse(CREATE, reason);
    }
    if request.announce {
        log_line(ANNOUNCEMENT);
    }
    0
}
