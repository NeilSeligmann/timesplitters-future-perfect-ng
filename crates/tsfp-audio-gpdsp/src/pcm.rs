//! Conversion between integer mixer samples and the GP's 24-bit mixbuffer.

use crate::frame_buffer::SAMPLES;
use crate::gp::FRAME_WORDS;

/// Encode an accumulated sample in i16 units as a saturated signed 24-bit word.
/// Accumulate sources before calling this so that saturation occurs only once.
pub fn mix_sample_word(sample: i64) -> u32 {
    sample.saturating_mul(256).clamp(-0x80_0000, 0x7f_ffff) as u32 & 0xff_ffff
}

/// Project the GP monitor's first two planar output bins to interleaved PCM.
pub fn monitor_stereo(frame: &[u32; FRAME_WORDS]) -> [i16; SAMPLES * 2] {
    let mut output = [0; SAMPLES * 2];
    for i in 0..SAMPLES {
        output[2 * i] = (frame[i] >> 8) as u16 as i16;
        output[2 * i + 1] = (frame[SAMPLES + i] >> 8) as u16 as i16;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_words_saturate_and_monitor_projects_planar_bins() {
        assert_eq!(mix_sample_word(0), 0);
        assert_eq!(mix_sample_word(1), 0x100);
        assert_eq!(mix_sample_word(-1), 0xff_ff00);
        assert_eq!(mix_sample_word(32768), 0x7f_ffff);
        assert_eq!(mix_sample_word(-32769), 0x80_0000);
        assert_eq!(mix_sample_word(i64::MAX), 0x7f_ffff);
        assert_eq!(mix_sample_word(i64::MIN), 0x80_0000);

        let mut frame = [0; FRAME_WORDS];
        for i in 0..SAMPLES {
            frame[i] = mix_sample_word(i as i64);
            frame[SAMPLES + i] = mix_sample_word(-(i as i64) - 1);
            frame[2 * SAMPLES + i] = 0x7f_ffff;
        }
        let stereo = monitor_stereo(&frame);
        for i in 0..SAMPLES {
            assert_eq!(stereo[2 * i], i as i16);
            assert_eq!(stereo[2 * i + 1], -(i as i16) - 1);
        }
    }
}
