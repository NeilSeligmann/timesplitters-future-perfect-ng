//! Collect time-ordered mix-bin samples into the GP's planar 32-sample frames.
//!
//! A complete frame is handed to the caller immediately, so a caller can apply
//! control changes at frame boundaries without batching across them.

use crate::gp::FRAME_WORDS;

pub const BINS: usize = 32;
pub const SAMPLES: usize = FRAME_WORDS / BINS;

pub struct FrameBuffer {
    words: [u32; FRAME_WORDS],
    samples: usize,
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self {
            words: [0; FRAME_WORDS],
            samples: 0,
        }
    }
}

impl FrameBuffer {
    /// Add one sample per bin. Returns a completed planar frame every 32 calls.
    pub fn push(&mut self, bins: &[u32; BINS]) -> Option<[u32; FRAME_WORDS]> {
        for (bin, sample) in bins.iter().enumerate() {
            self.words[bin * SAMPLES + self.samples] = *sample;
        }
        self.samples += 1;
        if self.samples != SAMPLES {
            return None;
        }
        self.samples = 0;
        Some(self.words)
    }

    /// Discard an incomplete frame when the audio timeline is reset.
    pub fn reset(&mut self) {
        self.samples = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_planar_and_independent_across_boundaries_and_reset() {
        let mut buffer = FrameBuffer::default();
        for frame in 0..2u32 {
            for sample in 0..SAMPLES {
                let bins =
                    std::array::from_fn(|bin| frame * 10_000 + bin as u32 * 100 + sample as u32);
                let output = buffer.push(&bins);
                if sample + 1 < SAMPLES {
                    assert!(output.is_none());
                } else {
                    let output = output.unwrap();
                    for bin in 0..BINS {
                        for sample in 0..SAMPLES {
                            assert_eq!(
                                output[bin * SAMPLES + sample],
                                frame * 10_000 + bin as u32 * 100 + sample as u32
                            );
                        }
                    }
                }
            }
        }
        assert!(buffer.push(&[99; BINS]).is_none());
        buffer.reset();
        for _ in 1..SAMPLES {
            assert!(buffer.push(&[7; BINS]).is_none());
        }
        assert_eq!(buffer.push(&[7; BINS]).unwrap(), [7; FRAME_WORDS]);
    }
}
