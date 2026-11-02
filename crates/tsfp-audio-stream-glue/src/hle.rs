// SPDX-License-Identifier: GPL-3.0-or-later
//! Services the Rust frame handlers (stream create, volume, spatial, control, frequency, routing HLE in
//! `tsfp-audio-capi`) call back into: policy snapshots, host pid, extension routes, apply entries and refusals.
use crate::access::{
    access, dsound_stream_cache_flush_ex, dsound_stream_cache_i3dl2,
    dsound_stream_cache_max_distance, dsound_stream_cache_min_distance,
    dsound_stream_cache_rolloff, dsound_stream_cache_set_format, dsound_stream_set_position,
};
use crate::config;
use crate::ffi::*;
use crate::*;
use std::ffi::c_void;

const EXTRA_REFUSAL: &std::ffi::CStr = c"Rust stream-call decoder refused frame";

#[repr(C)]
pub struct FrequencyPolicyInputs {
    enabled: u32,
    completion: u32,
    handler_available: u32,
    irql_known: u32,
    irql: u32,
}

fn guest_pid() -> i32 {
    host_pid()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_routing_hle_policy_snapshot_rust(
    admitted_out: *mut u32,
    completion_enabled_out: *mut u32,
    irql_known_out: *mut u32,
    irql_out: *mut u32,
) -> bool {
    if admitted_out.is_null()
        || completion_enabled_out.is_null()
        || irql_known_out.is_null()
        || irql_out.is_null()
    {
        return false;
    }
    lock_streams();
    let config = config::get();
    let admitted = config::enabled() && config.completion && config.routing_note.is_some();
    let irql = config::irql(&config);
    unlock_streams();
    // SAFETY: null-checked caller outputs.
    unsafe {
        *admitted_out = u32::from(admitted);
        *completion_enabled_out = u32::from(config.completion);
        *irql_known_out = u32::from(irql.is_some());
        *irql_out = u32::from(irql.unwrap_or(0));
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_routing_hle_guest_pid_rust() -> i32 {
    guest_pid()
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_routing_hle_apply_rust(
    entry: u32,
    stream: u32,
    argument: u32,
) -> u32 {
    access(entry, stream, argument, 0, 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_routing_hle_refuse_rust(entry: u32, reason: u32) -> ! {
    let message = match reason {
        1 => c"routing policy snapshot unavailable",
        2 => c"routing needs explicit PCM/completion policy at known IRQL0",
        3 => c"global audio state inaccessible",
        4 => EXTRA_REFUSAL,
        _ => c"unknown Rust stream-routing refusal",
    };
    refuse(entry, message.as_ptr())
}

fn read_caller(frame: &KernelFrame) -> Option<u32> {
    let mut caller = 0u32;
    // SAFETY: local output.
    unsafe { kernel_guest_read_u32(frame.stack_ptr, &mut caller) }.then_some(caller)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_volume_hle_extension_rust(
    context: *mut c_void,
    result_out: *mut u32,
) -> bool {
    if context.is_null() || result_out.is_null() {
        return false;
    }
    // SAFETY: the dispatcher's kernel call frame.
    let frame = unsafe { &*(context as *const KernelFrame) };
    let Some(route) = config::get().public_route else {
        return false;
    };
    let Some(caller) = read_caller(frame) else {
        return false;
    };
    // SAFETY: host-installed route.
    unsafe { route(VOLUME, frame, caller, result_out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_create_hle_extension_rust(
    context: *mut c_void,
    caller: u32,
    result_out: *mut u32,
) -> bool {
    if context.is_null() || result_out.is_null() {
        return false;
    }
    // SAFETY: the dispatcher's kernel call frame.
    let frame = unsafe { &*(context as *const KernelFrame) };
    // SAFETY: host-installed route.
    config::get()
        .public_route
        .is_some_and(|route| unsafe { route(CREATE, frame, caller, result_out) })
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_volume_hle_completion_enabled_rust() -> bool {
    config::get().completion
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_volume_hle_guest_pid_rust() -> i32 {
    guest_pid()
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_volume_hle_apply_rust(
    stream: u32,
    millibels: u32,
    after_format: bool,
) -> u32 {
    access(VOLUME, stream, millibels, u32::from(after_format), 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_volume_hle_refuse_rust(reason: u32) -> ! {
    let message = if reason == 1 {
        EXTRA_REFUSAL
    } else {
        c"unknown Rust stream-volume refusal"
    };
    refuse(VOLUME, message.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_spatial_hle_completion_enabled_rust() -> bool {
    config::get().completion
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_spatial_hle_guest_pid_rust() -> i32 {
    guest_pid()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_spatial_hle_apply_rust(entry: u32, args: *const u32) -> u32 {
    if args.is_null() {
        refuse(
            entry,
            c"Rust spatial handler supplied no decoded arguments".as_ptr(),
        );
    }
    // SAFETY: the decoder supplies the entry's argument words (five for SetPosition, four for the rest).
    let args =
        unsafe { std::slice::from_raw_parts(args, if entry == SET_POSITION { 5 } else { 4 }) };
    match entry {
        I3DL2 => dsound_stream_cache_i3dl2(args[0], args[1], args[2]),
        MINIMUM => dsound_stream_cache_min_distance(args[0], args[1], args[2]),
        MAX_DISTANCE => dsound_stream_cache_max_distance(args[0], args[1], args[2]),
        SET_POSITION => dsound_stream_set_position(args[0], args[1], args[2], args[3], args[4]),
        ROLLOFF => dsound_stream_cache_rolloff(args[0], args[1], args[2], args[3]),
        _ => refuse(
            entry,
            c"Rust spatial handler selected an unmeasured entry".as_ptr(),
        ),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_spatial_hle_refuse_rust(entry: u32, reason: u32) -> ! {
    let message = if reason == 1 {
        EXTRA_REFUSAL
    } else {
        c"unknown Rust stream-spatial refusal"
    };
    refuse(entry, message.as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_control_hle_prepare_rust(
    entry: u32,
    frame: *const KernelFrame,
    completion_out: *mut bool,
    caller_out: *mut u32,
    result_out: *mut u32,
) -> bool {
    if completion_out.is_null() || caller_out.is_null() || result_out.is_null() {
        refuse(
            entry,
            c"Rust stream-control callback supplied no prepare outputs".as_ptr(),
        );
    }
    let config = config::get();
    // SAFETY: null-checked caller outputs.
    unsafe {
        *completion_out = config.completion;
        *caller_out = 0;
        *result_out = 0;
    }
    if let Some(route) = config.public_route {
        // SAFETY: the dispatcher's kernel call frame, when supplied.
        if let Some(frame) = unsafe { frame.as_ref() } {
            if let Some(actual) = read_caller(frame) {
                // SAFETY: null-checked caller outputs; host-installed route.
                unsafe {
                    *caller_out = actual;
                    let mut extension = 0u32;
                    if route(entry, frame, actual, &mut extension) {
                        *result_out = extension;
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_control_hle_guest_pid_rust() -> i32 {
    guest_pid()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_control_hle_apply_rust(
    entry: u32,
    caller: u32,
    args: *const u32,
) -> u32 {
    if args.is_null() {
        refuse(
            entry,
            c"Rust stream-control callback supplied no decoded arguments".as_ptr(),
        );
    }
    // SAFETY: the decoder supplies the entry's argument words (four for FlushEx).
    let args = unsafe { std::slice::from_raw_parts(args, 4) };
    match entry {
        PAUSE => {
            let path = match caller {
                0x29A51 => 1,
                0x29ED1 => 2,
                _ => 0,
            };
            access(PAUSE, args[0], args[1], path, 0)
        }
        FLUSH_EX => dsound_stream_cache_flush_ex(args[0], args[1], args[2], args[3]),
        SET_FORMAT => dsound_stream_cache_set_format(args[0], args[1]),
        _ => refuse(
            entry,
            c"Rust stream-control callback selected an unmeasured entry".as_ptr(),
        ),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_control_hle_refuse_rust(entry: u32, reason: u32) -> ! {
    let message = match reason {
        1 => EXTRA_REFUSAL,
        2 => c"Rust stream caller changed during decode",
        3 => c"guest x87 control word unavailable",
        4 => c"frequency needs explicit PCM/completion policy at known IRQL0",
        5 => c"global audio state inaccessible",
        _ => c"unknown Rust stream-control refusal",
    };
    refuse(entry, message.as_ptr())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_frequency_hle_policy_inputs_rust(
    output: *mut FrequencyPolicyInputs,
) -> bool {
    if output.is_null() {
        return false;
    }
    lock_streams();
    let config = config::get();
    let enabled = config::enabled();
    let irql = config::irql(&config);
    unlock_streams();
    // SAFETY: null-checked caller output.
    unsafe {
        *output = FrequencyPolicyInputs {
            enabled: u32::from(enabled),
            completion: u32::from(config.completion),
            handler_available: u32::from(config.frequency_note.is_some()),
            irql_known: u32::from(irql.is_some()),
            irql: u32::from(irql.unwrap_or(0)),
        };
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_frequency_hle_guest_pid_rust() -> i32 {
    guest_pid()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_stream_frequency_hle_control_word_rust(output: *mut u16) -> bool {
    if output.is_null() {
        return false;
    }
    lock_streams();
    // SAFETY: host-installed provider writing the null-checked output.
    let known = config::get()
        .frequency_control_word
        .is_some_and(|read| unsafe { read(output) });
    unlock_streams();
    known
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_stream_frequency_hle_apply_rust(
    stream: u32,
    hertz: u32,
    control_word: u16,
) -> u32 {
    access(SET_FREQUENCY, stream, hertz, u32::from(control_word), 0)
}
