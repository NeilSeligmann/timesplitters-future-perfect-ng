// SPDX-License-Identifier: GPL-3.0-or-later
//! Frame routing of the movie stream entries: the public Create/Pause/FlushEx/SetVolume calls and the stream methods.
use crate::ffi::*;
use crate::state::*;
use crate::{
    dsound_movie_stream_caller_ok, dsound_movie_stream_create_caller_ok,
    dsound_movie_stream_enabled,
};
use std::ffi::c_void;

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_owns(stream: u32) -> bool {
    locked(|st| st.find(stream).is_some())
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_count() -> usize {
    locked(|st| st.nodes.len())
}

fn frame_arg(frame: *const c_void, index: u32) -> Option<u32> {
    let mut value = 0u32;
    // SAFETY: the host passes a live call frame; the accessor bounds-checks the index.
    unsafe { kernel_frame_arg(frame, index, &mut value) }.then_some(value)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_movie_stream_route_public(
    entry: u32,
    frame: *const c_void,
    return_address: u32,
    result: *mut u32,
) -> bool {
    if frame.is_null() || result.is_null() || !dsound_movie_stream_enabled() {
        return false;
    }
    let value = if entry == CREATE {
        if !dsound_movie_stream_create_caller_ok(return_address) {
            return false;
        }
        let (Some(descriptor), Some(output)) = (frame_arg(frame, 0), frame_arg(frame, 1)) else {
            refuse(entry, "unreadable argument");
        };
        crate::create::dsound_movie_stream_create(descriptor, output)
    } else {
        if entry != PAUSE && entry != FLUSH_EX && entry != VOLUME {
            return false;
        }
        let Some(stream) = frame_arg(frame, 0).filter(|&stream| dsound_movie_stream_owns(stream))
        else {
            return false;
        };
        if !dsound_movie_stream_caller_ok(return_address) {
            refuse(entry, "movie stream called from outside the movie callers");
        }
        let count = if entry == FLUSH_EX { 4 } else { 2 };
        let mut args = [stream, 0, 0, 0];
        for (index, slot) in args.iter_mut().enumerate().take(count).skip(1) {
            let Some(word) = frame_arg(frame, index as u32) else {
                refuse(entry, "unreadable argument");
            };
            *slot = word;
        }
        match entry {
            PAUSE => crate::access::dsound_movie_stream_pause(args[0], args[1]),
            VOLUME => crate::access::dsound_movie_stream_set_volume(args[0], args[1] as i32),
            _ => crate::access::dsound_movie_stream_flush_ex(args[0], args[1], args[2], args[3]),
        }
    };
    // SAFETY: writable by contract.
    unsafe { result.write(value) };
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_method_owned(stack_pointer: u32) -> bool {
    dsound_movie_stream_enabled()
        && stack_pointer <= u32::MAX - 8
        && read_u32(stack_pointer + 4).is_some_and(|stream| dsound_movie_stream_owns(stream))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_movie_stream_route_method(
    entry: u32,
    stack_pointer: u32,
    result: *mut u32,
    pop_bytes: *mut u32,
) -> bool {
    use crate::access::*;
    let count = match entry {
        ADD_REF | RELEASE | DISCONTINUITY | FLUSH => 1u32,
        GET_STATUS => 2,
        PROCESS => 3,
        _ => return false,
    };
    if result.is_null()
        || pop_bytes.is_null()
        || !dsound_movie_stream_enabled()
        || stack_pointer > u32::MAX - 16
    {
        return false;
    }
    let Some(caller) = read_u32(stack_pointer) else {
        return false;
    };
    let Some(stream) =
        read_u32(stack_pointer + 4).filter(|&stream| dsound_movie_stream_owns(stream))
    else {
        return false;
    };
    if !dsound_movie_stream_caller_ok(caller) {
        refuse(entry, "movie stream called from outside the movie callers");
    }
    let mut args = [stream, 0, 0];
    for index in 1..count {
        let Some(word) = read_u32(stack_pointer + 4 + 4 * index) else {
            refuse(entry, "unreadable argument");
        };
        args[index as usize] = word;
    }
    let value = match entry {
        ADD_REF => dsound_movie_stream_add_ref(args[0]),
        RELEASE => dsound_movie_stream_release(args[0]),
        DISCONTINUITY => dsound_movie_stream_discontinuity(args[0]),
        FLUSH => dsound_movie_stream_flush(args[0]),
        GET_STATUS => dsound_movie_stream_get_status(args[0], args[1]),
        _ => dsound_movie_stream_process(args[0], args[1], args[2]),
    };
    // SAFETY: both writable by contract.
    unsafe {
        result.write(value);
        pop_bytes.write(4 * count);
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn dsound_movie_stream_route_work(return_address: u32) -> bool {
    if !dsound_movie_stream_enabled()
        || !dsound_movie_stream_caller_ok(return_address)
        || dsound_movie_stream_count() == 0
    {
        return false;
    }
    let _ = crate::drain::dsound_movie_stream_do_work();
    true
}
