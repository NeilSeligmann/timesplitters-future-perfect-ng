// SPDX-License-Identifier: GPL-3.0-or-later
//! Completion drain, DoWork and SynchPlayback of the movie streams.
use crate::access::resume_stream;
use crate::ffi::*;
use crate::state::*;
use std::ffi::c_void;

unsafe extern "C" fn completion_before_unlock(size: u32, status: u32, context: u32) {
    trace(|| {
        format!(
            "complete size 0x{size:X} status 0x{status:08X} context 0x{context:08X} tick {}",
            now_ticks()
        )
    });
}

/// The Rust drain gives the module lock back through this hook around the guest callback. No caller holds the module
/// lock when it drains (the C module released it first), so there is nothing to release.
unsafe extern "C" fn completion_unlock() {}

/// Completes the packets that are due, in node then queue order, until none is.
pub(crate) fn drain(entry: u32, everything: bool, only_stream: u32) -> usize {
    // SAFETY: the hooks are valid for this synchronous call and the completion callbacks do not reenter the drain.
    unsafe {
        tsfp_audio_movie_schedule_drain_rust(
            entry,
            everything,
            only_stream,
            now_ticks_c,
            host_pid_c,
            completion_before_unlock,
            completion_unlock,
            refuse_c,
        )
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_do_work() -> usize {
    count_operation();
    if let Some(error) = policy_error() {
        refuse(WORK, error);
    }
    drain(WORK, false, 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_synch_playback() {
    let refused = locked(|st| {
        let now = now_ticks();
        trace(|| format!("synch playback tick {now}"));
        let mut refused = false;
        for node in &mut st.nodes {
            if node.paused && node.pause_mode == 2 && !resume_stream(node, now) {
                refused = true;
            }
        }
        refused
    });
    if refused {
        refuse(SYNCH, "Rust movie scheduler refused SynchPlayback resume");
    }
}

unsafe extern "C" fn synch_owned_device(_internal: u32, _userdata: *mut c_void, result: *mut u32) {
    dsound_movie_stream_synch_playback();
    // SAFETY: writable result of the transaction.
    unsafe { result.write(0) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_movie_stream_route_synch(
    stack_pointer: u32,
    result: *mut u32,
) -> bool {
    if result.is_null() {
        refuse(SYNCH, "no result slot");
    }
    if let Some(error) = policy_error() {
        refuse(SYNCH, error);
    }
    let frame = (stack_pointer <= u32::MAX - 8)
        .then(|| read_u32(stack_pointer).zip(read_u32(stack_pointer + 4)))
        .flatten();
    let Some((caller, interface)) = frame else {
        refuse(SYNCH, "unreadable SynchPlayback frame");
    };
    if caller != SYNCH_RETURN {
        refuse(
            SYNCH,
            "SynchPlayback called from outside the measured XMV site",
        );
    }
    count_operation();
    // The device mutex is held while the model runs, lock order is device then movie.
    // SAFETY: `result` is writable by contract.
    if !unsafe {
        dsound_device_with_owned_interface(
            interface,
            synch_owned_device,
            std::ptr::null_mut(),
            result,
        )
    } {
        refuse(
            SYNCH,
            "SynchPlayback argument is not the owned device interface",
        );
    }
    true
}
