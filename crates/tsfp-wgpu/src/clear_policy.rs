//! Colour write mask rules for a pushed `CLEAR_SURFACE` (replay.rs `clear_with_mask` supplies the facts).
//! A pushed clear with the colour write mask applied. NV2A clears honour `SET_COLOR_MASK` (MEASURED on the traced engine: every
//! frame starts with mask 0 and `CLEAR_SURFACE` 0xF3, so the back buffer colour is kept and only depth is cleared, and the scope
//! bloom surface (160x120 render target) survives its own mask 0 clear: without this the lens samples a black surface).
//! Slice 8 limit: the BACK BUFFER keeps the unmasked colour clear unless `clear_honours_mask` is set, because the evaluation
//! windows start a few presents before the shot and the masked semantics needs the frames before (a cinematic fade measured
//! 0.967 with and 0.748 without the clear, luma 46 and 78 against the host's 44).
//! T3502 (INFERRED, not replayed): the same rule must not keep a scratch surface alive across frames. The bloom clear follows
//! draws into the surface in the same frame (the colour it keeps is this frame's picture, sampled later). A mask 0 clear
//! before the first draw of the frame only preserves what the previous frame left, which is how a screen space effect
//! surface accumulates and smears with the camera, so it wipes the channels it names. Control: `clear_keeps_stale_scratch`.

use crate::nv2a::ClearRequest;

/// `back_buffer`: the current surface is full size (display or a full size copy). `drawn`: a draw into it is already queued this
/// frame. `knob`: the replay experiment switches.
pub(crate) fn decide(
    clear: ClearRequest,
    back_buffer: bool,
    drawn: bool,
    knob: impl Fn(&str) -> bool,
) -> ClearRequest {
    if knob("clear_ignores_mask") || (back_buffer && !knob("clear_honours_mask")) {
        return clear;
    }
    if knob("clear_keeps_stale_scratch") || back_buffer || drawn {
        clear.masked()
    } else {
        clear
    }
}
