//! Assemble mixer samples and run the GP at its 32-sample frame boundary.

use crate::frame_buffer::{BINS, FrameBuffer, SAMPLES};
use crate::gp::{FRAME_WORDS, GpDsp, GpError};
use crate::pcm::{mix_sample_word, monitor_stereo};
use std::collections::VecDeque;
use tsfp_audio::mixer::Mixer;

/// Capture the mixer interval while its owner holds the mixer lock. The dry
/// PCM and bins describe the same interval; GP processing can happen after
/// the lock is released, or the dry PCM can be used when effects are absent.
pub fn capture_mixer_interval(
    mixer: &mut Mixer,
    now: u64,
    capacity: usize,
) -> (Vec<i16>, Vec<[i64; BINS]>) {
    let mut bins = Vec::new();
    let dry = mixer.render_until_with_bins(now, capacity, |sample| bins.push(*sample));
    (dry, bins)
}

#[derive(Default)]
pub struct Renderer {
    input: FrameBuffer,
}

/// Adapt frame-at-a-time DSP output to a sample-at-a-time PCM sink.
/// The first frame introduces 31 samples of silence while it is assembled.
pub struct SampleRenderer {
    renderer: Renderer,
    pending: VecDeque<[i16; 2]>,
}

impl Default for SampleRenderer {
    fn default() -> Self {
        Self {
            renderer: Renderer::default(),
            pending: VecDeque::new(),
        }
    }
}

impl SampleRenderer {
    /// Process bins captured separately from the mixer. This permits callers to
    /// release the mixer lock before entering the effects device's lock domain.
    pub fn render_bins_with<E>(
        &mut self,
        bins: &[[i64; BINS]],
        mut process: impl FnMut(&[u32; FRAME_WORDS]) -> Result<[u32; FRAME_WORDS], E>,
    ) -> Result<Vec<i16>, E> {
        let mut output = Vec::with_capacity(bins.len().saturating_mul(2));
        for sample in bins {
            match self.push_with(sample, &mut process) {
                Ok(stereo) => output.extend_from_slice(&stereo),
                Err(error) => {
                    self.reset();
                    return Err(error);
                }
            }
        }
        Ok(output)
    }

    /// Render mixer bins through a frame processor, preserving partial frames
    /// across calls. A processor error consumes the mixer interval and is
    /// returned to the caller; reset before attempting another interval.
    pub fn render_mixer_with<E>(
        &mut self,
        mixer: &mut Mixer,
        now: u64,
        capacity: usize,
        mut process: impl FnMut(&[u32; FRAME_WORDS]) -> Result<[u32; FRAME_WORDS], E>,
    ) -> Result<Vec<i16>, E> {
        let mut output = Vec::new();
        let mut error = None;
        mixer.render_until_with_bins(now, capacity, |bins| {
            if error.is_some() {
                return;
            }
            match self.push_with(bins, &mut process) {
                Ok(sample) => output.extend_from_slice(&sample),
                Err(e) => error = Some(e),
            }
        });
        match error {
            Some(e) => {
                self.reset();
                Err(e)
            }
            None => Ok(output),
        }
    }

    pub fn push_with<E>(
        &mut self,
        bins: &[i64; BINS],
        process: impl FnOnce(&[u32; FRAME_WORDS]) -> Result<[u32; FRAME_WORDS], E>,
    ) -> Result<[i16; 2], E> {
        if let Some(frame) = self.renderer.push_with(bins, process)? {
            self.pending
                .extend(frame.chunks_exact(2).map(|s| [s[0], s[1]]));
        }
        Ok(self.pending.pop_front().unwrap_or([0, 0]))
    }

    pub fn reset(&mut self) {
        self.renderer.reset();
        self.pending.clear();
    }
}

impl Renderer {
    /// Feed one accumulated sample per bin. A complete stereo frame is returned
    /// only after the GP has accepted and processed all 32 samples.
    pub fn push(
        &mut self,
        dsp: &mut GpDsp,
        bins: &[i64; BINS],
    ) -> Result<Option<[i16; SAMPLES * 2]>, GpError> {
        self.push_with(bins, |frame| dsp.frame(frame))
    }

    pub fn push_with<E>(
        &mut self,
        bins: &[i64; BINS],
        process: impl FnOnce(&[u32; FRAME_WORDS]) -> Result<[u32; FRAME_WORDS], E>,
    ) -> Result<Option<[i16; SAMPLES * 2]>, E> {
        let words = bins.map(mix_sample_word);
        match self.input.push(&words) {
            Some(frame) => process(&frame).map(|output| Some(monitor_stereo(&output))),
            None => Ok(None),
        }
    }

    /// Discard the partial frame on a timeline reset.
    pub fn reset(&mut self) {
        self.input.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_audio::contract::{Command, SoundHost};

    #[test]
    fn detached_bins_match_streaming_across_chunks_and_reset_on_error() {
        let bins: Vec<[i64; BINS]> = (0..100)
            .map(|i| {
                let mut sample = [0; BINS];
                sample[0] = i;
                sample[1] = -i;
                sample
            })
            .collect();
        let mut detached = SampleRenderer::default();
        let mut streaming = SampleRenderer::default();
        let mut actual = Vec::new();
        for chunk in bins.chunks(17) {
            actual.extend(
                detached
                    .render_bins_with(chunk, |frame| Ok::<_, ()>(*frame))
                    .unwrap(),
            );
        }
        let expected: Vec<i16> = bins
            .iter()
            .flat_map(|sample| {
                streaming
                    .push_with(sample, |frame| Ok::<_, ()>(*frame))
                    .unwrap()
            })
            .collect();
        assert_eq!(actual, expected);
        assert!(
            detached
                .render_bins_with(&bins, |_| Err::<[u32; FRAME_WORDS], _>(()))
                .is_err()
        );
        assert_eq!(
            detached
                .render_bins_with(&bins[..1], |frame| Ok::<_, ()>(*frame))
                .unwrap(),
            [0, 0]
        );
    }

    #[test]
    fn captured_mixer_interval_preserves_dry_pcm_and_gp_frame_boundaries() {
        fn mixer() -> Mixer {
            let mut mixer = Mixer::new(48_000, 48_000);
            assert!(mixer.apply(&Command::SubmitPcm16Stereo {
                key: 1,
                ticks: 0,
                source_hz: 48_000,
                pcm: vec![1000, -400].repeat(64),
            }));
            mixer
        }
        let mut captured = mixer();
        let mut direct = mixer();
        let mut renderer = SampleRenderer::default();
        let mut reference = SampleRenderer::default();
        for capacity in [17, 47] {
            let (dry, bins) = capture_mixer_interval(&mut captured, 64, capacity);
            assert_eq!(dry.len(), bins.len() * 2);
            assert!(dry.chunks_exact(2).all(|sample| sample == [1000, -400]));
            let actual = renderer.render_bins_with(&bins, |frame| Ok::<_, ()>(*frame));
            let expected =
                reference.render_mixer_with(&mut direct, 64, capacity, |frame| Ok::<_, ()>(*frame));
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn mixer_adapter_preserves_frame_boundaries_across_chunks() {
        let mut mixer = Mixer::new(48_000, 48_000);
        assert!(mixer.apply(&Command::SubmitPcm16Stereo {
            key: 1,
            ticks: 0,
            source_hz: 48_000,
            pcm: vec![1000, -400].repeat(64),
        }));
        let mut renderer = SampleRenderer::default();
        let mut calls = 0;
        let mut process = |frame: &[u32; FRAME_WORDS]| {
            calls += 1;
            Ok::<_, ()>(*frame)
        };
        let mut output = renderer
            .render_mixer_with(&mut mixer, 64, 17, &mut process)
            .unwrap();
        output.extend(
            renderer
                .render_mixer_with(&mut mixer, 64, 47, &mut process)
                .unwrap(),
        );
        assert_eq!(calls, 2);
        assert_eq!(output.len(), 128);
        assert!(output[..62].iter().all(|&sample| sample == 0));
        assert!(
            output[62..]
                .chunks_exact(2)
                .all(|sample| sample == [1000, -400])
        );
    }

    #[test]
    fn sample_output_has_fixed_latency_across_frames_and_reset() {
        let mut renderer = SampleRenderer::default();
        for sample in 0..(SAMPLES * 3) {
            let mut bins = [0; BINS];
            bins[0] = sample as i64 + 1;
            bins[1] = -(sample as i64 + 1);
            let actual = renderer
                .push_with(&bins, |frame| Ok::<_, ()>(*frame))
                .unwrap();
            let expected = if sample < SAMPLES - 1 {
                0
            } else {
                (sample - (SAMPLES - 1) + 1) as i16
            };
            assert_eq!(actual, [expected, -expected]);
        }
        renderer.reset();
        assert_eq!(
            renderer.push_with(&[0; BINS], |frame| Ok::<_, ()>(*frame)),
            Ok([0, 0])
        );
    }

    #[test]
    fn calls_processor_once_per_frame_and_propagates_errors() {
        let mut renderer = Renderer::default();
        let mut calls = 0;
        for sample in 0..SAMPLES {
            let mut bins = [0; BINS];
            bins[0] = sample as i64;
            bins[1] = -(sample as i64);
            let output = renderer
                .push_with(&bins, |frame| {
                    calls += 1;
                    Ok::<_, ()>(*frame)
                })
                .unwrap();
            if sample + 1 == SAMPLES {
                let output = output.unwrap();
                for i in 0..SAMPLES {
                    assert_eq!((output[2 * i], output[2 * i + 1]), (i as i16, -(i as i16)));
                }
            } else {
                assert!(output.is_none());
            }
        }
        assert_eq!(calls, 1);
        for _ in 1..SAMPLES {
            assert!(
                renderer
                    .push_with(&[0; BINS], |_| Ok::<_, &str>([0; FRAME_WORDS]))
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(
            renderer.push_with(&[0; BINS], |_| Err("failed")),
            Err("failed")
        );
        assert!(
            renderer
                .push_with(&[0; BINS], |_| Ok::<_, ()>([0; FRAME_WORDS]))
                .unwrap()
                .is_none()
        );
        renderer.reset();
    }
}
