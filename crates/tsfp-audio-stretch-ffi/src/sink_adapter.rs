// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline adapter for the frame-based presentation sink contract.

use crate::{Engine, tsfp_stretch_process};

/// # Safety
/// The handle and pointers must satisfy the stretch boundary contract. Input
/// and output must contain the stated frames and one overlap block respectively.
pub unsafe fn process_frames(
    engine: *mut Engine,
    operation: u32,
    input: *const i16,
    frames: usize,
    nominal: usize,
    output: *mut i16,
    chosen: *mut usize,
) -> bool {
    let Some(engine_ref) = (unsafe { engine.as_ref() }) else {
        return false;
    };
    let Some(input_len) = frames.checked_mul(engine_ref.channels) else {
        return false;
    };
    let Some(output_len) = engine_ref
        .stretch
        .overlap()
        .checked_mul(engine_ref.channels)
    else {
        return false;
    };
    unsafe {
        tsfp_stretch_process(
            engine, operation, input, input_len, nominal, output, output_len, chosen,
        )
    }
}
