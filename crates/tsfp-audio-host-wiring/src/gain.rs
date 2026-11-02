// SPDX-License-Identifier: GPL-3.0-or-later
//! DirectSound volume (hundredths of a decibel of amplitude, 0 full, -10000 silence) to Q16 linear gain for the movie
//! stream's soundtrack, `gain = 10^(mb / 2000)`. Replaces `dsound_mixer_volume_to_gain_q16` of the retired C mixer; the
//! conversion is `tsfp_audio::gain` (its measured table is pinned there).

/// C ABI of the movie stream (`src/audio/dsound_movie_stream.c`): false and `*gain_q16` untouched when refused.
///
/// # Safety
/// `gain_q16` must be null or valid for a `u32` write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_volume_to_gain_q16(millibels: i32, gain_q16: *mut u32) -> bool {
    match (
        gain_q16.is_null(),
        tsfp_audio::gain::volume_to_gain_q16(millibels),
    ) {
        (false, Some(gain)) => {
            unsafe { *gain_q16 = gain };
            true
        }
        _ => false,
    }
}
