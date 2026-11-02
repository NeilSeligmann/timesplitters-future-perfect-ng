// SPDX-License-Identifier: GPL-3.0-or-later
//! The `audio_stretch.h` C interface of the presentation sink, served by the Rust WSOLA engine (T3879, replaces
//! src/host/audio_stretch.c). Frame counts are per channel as in the header; `tsfp_stretch_*` counts samples.
//! The opt-in `TSFP_WSOLA_CAPTURE` stream writer of the C unit was a debugging aid and is not carried over.

use crate::{
    Engine, tsfp_stretch_create, tsfp_stretch_destroy, tsfp_stretch_process, tsfp_stretch_query,
};

const PRIME: u32 = 0;
const NEXT: u32 = 1;
const HOLD: u32 = 2;
const LEAVE: u32 = 3;
const FADE_OUT: u32 = 4;

#[unsafe(no_mangle)]
pub extern "C" fn audio_stretch_create(channels: u16, overlap: usize) -> *mut Engine {
    tsfp_stretch_create(channels, overlap)
}

/// # Safety
/// `engine` must be null or a live handle from `audio_stretch_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_destroy(engine: *mut Engine) {
    unsafe { tsfp_stretch_destroy(engine) }
}

/// # Safety
/// `engine` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_overlap(engine: *const Engine) -> usize {
    unsafe { crate::tsfp_stretch_overlap(engine) }
}

/// # Safety
/// `engine` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_search(engine: *const Engine) -> usize {
    unsafe { crate::tsfp_stretch_search(engine) }
}

/// # Safety
/// `engine` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_input_needed(
    engine: *const Engine,
    nominal: usize,
) -> usize {
    let mut needed = 0usize;
    unsafe { tsfp_stretch_query(engine, nominal, &mut needed) };
    needed
}

/// # Safety
/// `engine` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_primed(engine: *const Engine) -> bool {
    unsafe { crate::tsfp_stretch_primed(engine) }
}

/// # Safety
/// `engine` must be live, `input` valid for `frames` frames and `out` for one block (`overlap` frames).
unsafe fn run(
    engine: *mut Engine,
    operation: u32,
    input: *const i16,
    frames: usize,
    nominal: usize,
    out: *mut i16,
    chosen: *mut usize,
) -> bool {
    let Some(state) = (unsafe { engine.as_ref() }) else {
        return false;
    };
    let Some(samples) = frames.checked_mul(state.channels()) else {
        return false;
    };
    let block = state.block_samples();
    unsafe {
        tsfp_stretch_process(
            engine, operation, input, samples, nominal, out, block, chosen,
        )
    }
}

/// # Safety
/// See `run`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_prime(
    engine: *mut Engine,
    input: *const i16,
    frames: usize,
    out: *mut i16,
) -> bool {
    unsafe { run(engine, PRIME, input, frames, 0, out, std::ptr::null_mut()) }
}

/// # Safety
/// See `run`; `chosen_start` may be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_next(
    engine: *mut Engine,
    input: *const i16,
    frames: usize,
    nominal: usize,
    out: *mut i16,
    chosen_start: *mut usize,
) -> bool {
    unsafe { run(engine, NEXT, input, frames, nominal, out, chosen_start) }
}

/// # Safety
/// See `run`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_hold(engine: *mut Engine, out: *mut i16) -> bool {
    unsafe {
        run(
            engine,
            HOLD,
            std::ptr::null(),
            0,
            0,
            out,
            std::ptr::null_mut(),
        )
    }
}

/// # Safety
/// See `run`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_leave(
    engine: *mut Engine,
    input: *const i16,
    frames: usize,
    out: *mut i16,
) -> bool {
    unsafe { run(engine, LEAVE, input, frames, 0, out, std::ptr::null_mut()) }
}

/// # Safety
/// See `run`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audio_stretch_fade_out(engine: *mut Engine, out: *mut i16) -> bool {
    unsafe {
        run(
            engine,
            FADE_OUT,
            std::ptr::null(),
            0,
            0,
            out,
            std::ptr::null_mut(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_interface_matches_the_sample_interface() {
        unsafe {
            let engine = audio_stretch_create(2, 4);
            assert_eq!(
                (audio_stretch_overlap(engine), audio_stretch_search(engine)),
                (4, 2)
            );
            assert_eq!(audio_stretch_input_needed(engine, 10), 20);
            let input: Vec<i16> = (0..16).collect();
            let mut out = [0i16; 8];
            assert!(!audio_stretch_prime(
                engine,
                input.as_ptr(),
                7,
                out.as_mut_ptr()
            ));
            assert!(audio_stretch_prime(
                engine,
                input.as_ptr(),
                8,
                out.as_mut_ptr()
            ));
            assert_eq!(out, [0, 1, 2, 3, 4, 5, 6, 7]);
            assert!(audio_stretch_primed(engine));
            assert!(audio_stretch_fade_out(engine, out.as_mut_ptr()));
            assert!(!audio_stretch_primed(engine));
            audio_stretch_destroy(engine);
        }
    }
}
