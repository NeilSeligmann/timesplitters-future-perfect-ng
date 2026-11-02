// SPDX-License-Identifier: GPL-3.0-or-later
//! Unwired C boundary for the Rust WSOLA implementation.
//!
//! The caller owns the handle and must provide valid, non-overlapping input and
//! output arrays of the stated lengths; `chosen` must not overlap them or the
//! handle. Calls on a handle must be serialized.

use tsfp_audio::stretch::Stretch;

pub mod continuous_ratio;
pub mod fill_smoother;
pub mod latency_trim;
pub mod sink_adapter;
pub mod stretch_ramp;
pub mod stretch_ratio;
pub mod stretch_ring;
pub mod stretch_fifo;
pub mod stretch_hold;
pub mod stretch_progress;

pub struct Engine {
    stretch: Stretch,
    channels: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_stretch_create(channels: u16, overlap: usize) -> *mut Engine {
    Stretch::new(channels, overlap).map_or(std::ptr::null_mut(), |stretch| {
        Box::into_raw(Box::new(Engine {
            stretch,
            channels: channels.into(),
        }))
    })
}

/// # Safety
/// The handle must be null or a live handle returned by `tsfp_stretch_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_stretch_destroy(engine: *mut Engine) {
    if !engine.is_null() {
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// # Safety
/// `engine` must be a live handle and `needed` a valid, disjoint output pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_stretch_query(
    engine: *const Engine,
    nominal: usize,
    needed: *mut usize,
) -> bool {
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        return false;
    };
    let Some(value) = engine.stretch.input_needed(nominal) else {
        return false;
    };
    if needed.is_null() {
        return false;
    }
    unsafe {
        *needed = value;
    }
    true
}

/// # Safety
/// `engine` must be null or a live handle, serialized with all other calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_stretch_primed(engine: *const Engine) -> bool {
    unsafe { engine.as_ref() }.is_some_and(|engine| engine.stretch.primed())
}

/// # Safety
/// `engine` must be null or a live handle, serialized with all other calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_stretch_overlap(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.stretch.overlap())
}

/// # Safety
/// `engine` must be null or a live handle, serialized with all other calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_stretch_search(engine: *const Engine) -> usize {
    unsafe { engine.as_ref() }.map_or(0, |engine| engine.stretch.search())
}

/// # Safety
/// Handle and arrays must satisfy the module contract. Array lengths are samples.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_stretch_process(
    engine: *mut Engine,
    operation: u32,
    input: *const i16,
    input_len: usize,
    nominal: usize,
    output: *mut i16,
    output_len: usize,
    chosen: *mut usize,
) -> bool {
    let Some(engine) = (unsafe { engine.as_mut() }) else {
        return false;
    };
    let Some(block) = engine.stretch.overlap().checked_mul(engine.channels) else {
        return false;
    };
    if output.is_null() || output_len < block || block > isize::MAX as usize / 2 {
        return false;
    }
    let output = unsafe { std::slice::from_raw_parts_mut(output, block) };
    match operation {
        2 => engine.stretch.hold(output),
        4 => engine.stretch.fade_out(output),
        0 | 1 | 3 => {
            if input.is_null() || input_len > isize::MAX as usize / 2 {
                return false;
            }
            let input = unsafe { std::slice::from_raw_parts(input, input_len) };
            match operation {
                0 => engine.stretch.prime(input, output),
                1 => match engine.stretch.next(input, nominal, output) {
                    Some(start) => {
                        if !chosen.is_null() {
                            unsafe {
                                *chosen = start;
                            }
                        }
                        true
                    }
                    None => false,
                },
                _ => engine.stretch.leave(input, output),
            }
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_refuses_short_buffers_and_preserves_state() {
        unsafe {
            assert_eq!(tsfp_stretch_overlap(std::ptr::null()), 0);
            assert_eq!(tsfp_stretch_search(std::ptr::null()), 0);
            let e = tsfp_stretch_create(1, 4);
            let input = [100i16; 8];
            let mut out = [7i16; 4];
            assert!(!tsfp_stretch_process(
                e,
                0,
                input.as_ptr(),
                8,
                0,
                out.as_mut_ptr(),
                3,
                std::ptr::null_mut()
            ));
            assert_eq!(out, [7; 4]);
            assert!(!tsfp_stretch_process(
                e,
                2,
                std::ptr::null(),
                0,
                0,
                out.as_mut_ptr(),
                4,
                std::ptr::null_mut()
            ));
            assert!(tsfp_stretch_process(
                e,
                0,
                input.as_ptr(),
                8,
                0,
                out.as_mut_ptr(),
                4,
                std::ptr::null_mut()
            ));
            assert_eq!(out, [100; 4]);
            assert!(tsfp_stretch_process(
                e,
                4,
                std::ptr::null(),
                0,
                0,
                out.as_mut_ptr(),
                4,
                std::ptr::null_mut()
            ));
            assert_eq!(out, [80, 60, 40, 20]);
            tsfp_stretch_destroy(e);
        }
    }
}
