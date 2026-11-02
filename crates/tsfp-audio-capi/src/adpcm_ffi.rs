// SPDX-License-Identifier: GPL-3.0-or-later
//! T3933: Rust-owned Xbox ADPCM decode entry points for the Rust audio host. Replaces `src/audio/dsound_adpcm.c`
//! (kept as the `TSFP_RUST_AUDIO_HOST=0` fallback) under the same C symbols; the decoder is `tsfp_audio::adpcm`, already
//! differential-tested against the C file (`crates/tsfp-audio/tests/codec_c.rs`). Refusals write nothing.

use tsfp_audio::adpcm;

/// # Safety
/// Pointers are null or valid for the stated lengths; `frames_written` is writable when non-null.
unsafe fn decode(
    encoded: *const u8,
    encoded_bytes: usize,
    pcm: *mut i16,
    pcm_frame_capacity: usize,
    frames_written: *mut usize,
    channels: usize,
) -> bool {
    if frames_written.is_null()
        || (encoded_bytes != 0 && encoded.is_null())
        || (pcm_frame_capacity != 0 && pcm.is_null())
    {
        return false;
    }
    let input: &[u8] = if encoded_bytes == 0 {
        &[]
    } else {
        // SAFETY: non-null and valid for `encoded_bytes` by contract.
        unsafe { std::slice::from_raw_parts(encoded, encoded_bytes) }
    };
    let Some(samples) = adpcm::decode(input, channels) else {
        return false;
    };
    let frames = samples.len() / channels;
    if frames > pcm_frame_capacity {
        return false;
    }
    if !samples.is_empty() {
        // SAFETY: capacity checked above; `pcm` is non-null because capacity is non-zero here.
        unsafe { std::ptr::copy_nonoverlapping(samples.as_ptr(), pcm, samples.len()) };
    }
    // SAFETY: checked non-null above.
    unsafe { *frames_written = frames };
    true
}

/// `dsound_adpcm_xbox_decode_stereo`.
///
/// # Safety
/// See [`decode`].
#[cfg_attr(feature = "rust-audio-host", unsafe(no_mangle))]
pub unsafe extern "C" fn dsound_adpcm_xbox_decode_stereo(
    encoded: *const u8,
    encoded_bytes: usize,
    pcm: *mut i16,
    pcm_frame_capacity: usize,
    frames_written: *mut usize,
) -> bool {
    // SAFETY: forwarded contract.
    unsafe {
        decode(
            encoded,
            encoded_bytes,
            pcm,
            pcm_frame_capacity,
            frames_written,
            2,
        )
    }
}

/// `dsound_adpcm_xbox_decode_mono`.
///
/// # Safety
/// See [`decode`].
#[cfg_attr(feature = "rust-audio-host", unsafe(no_mangle))]
pub unsafe extern "C" fn dsound_adpcm_xbox_decode_mono(
    encoded: *const u8,
    encoded_bytes: usize,
    pcm: *mut i16,
    pcm_frame_capacity: usize,
    frames_written: *mut usize,
) -> bool {
    // SAFETY: forwarded contract.
    unsafe {
        decode(
            encoded,
            encoded_bytes,
            pcm,
            pcm_frame_capacity,
            frames_written,
            1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(seed: u8) -> Vec<u8> {
        let mut b = vec![0u8; adpcm::MONO_BLOCK_BYTES];
        b[0] = seed;
        b[1] = 1;
        b[2] = 5; // step index
        for (i, v) in b.iter_mut().enumerate().skip(4) {
            *v = (i as u8).wrapping_mul(37).wrapping_add(seed);
        }
        b
    }

    #[test]
    fn mono_matches_the_model_and_refusals_write_nothing() {
        let data = block(9);
        let mut pcm = [0x5a5ai16; adpcm::FRAMES_PER_BLOCK];
        let mut frames = 77usize;
        let ok = unsafe {
            dsound_adpcm_xbox_decode_mono(
                data.as_ptr(),
                data.len(),
                pcm.as_mut_ptr(),
                64,
                &mut frames,
            )
        };
        assert!(ok);
        assert_eq!(frames, 64);
        assert_eq!(&pcm[..], &adpcm::decode(&data, 1).unwrap()[..]);
        let mut short = [0x5a5ai16; 63];
        let mut frames = 77usize;
        let refused = unsafe {
            dsound_adpcm_xbox_decode_mono(
                data.as_ptr(),
                data.len(),
                short.as_mut_ptr(),
                63,
                &mut frames,
            )
        };
        assert!(!refused);
        assert_eq!((frames, short[0]), (77, 0x5a5a));
        let partial = unsafe {
            dsound_adpcm_xbox_decode_mono(data.as_ptr(), 35, pcm.as_mut_ptr(), 64, &mut frames)
        };
        assert!(!partial);
    }

    #[test]
    fn empty_input_is_zero_frames_and_null_output_is_refused() {
        let mut frames = 9usize;
        assert!(unsafe {
            dsound_adpcm_xbox_decode_stereo(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
                &mut frames,
            )
        });
        assert_eq!(frames, 0);
        assert!(!unsafe {
            dsound_adpcm_xbox_decode_stereo(
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
            )
        });
    }
}
