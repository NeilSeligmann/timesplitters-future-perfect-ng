//! Per-voice low-pass used for the DLS2 / ParaEQ filter modes (xemu PR 3105 model).
//!
//! The VP applies a resonant state-variable low-pass per channel, driven by the
//! voice's FC0 (cutoff, log2 in 1/4096 steps) and FC1 (resonance) fields.

/// Voice filter mode field (`CFG_MISC.FMODE`) as seen by the filter stage.
pub fn filter_enabled(voice_is_3d: bool, stereo: bool, fmode: u32) -> bool {
    if voice_is_3d {
        // 1: DLS2+I3DL2, 2: ParaEQ+I3DL2, 3: I3DL2
        fmode == 1
    } else if stereo {
        fmode == 1
    } else {
        fmode & 1 != 0
    }
}

/// Cutoff coefficient from the signed FC0 register value.
pub fn cutoff_coefficient(fc0: i16) -> f32 {
    (2f64.powf(f64::from(fc0) / 4096.0) as f32).clamp(0.003906, 1.0)
}

/// Damping coefficient from the FC1 register value.
pub fn damping_coefficient(fc1: u16) -> f32 {
    (f32::from(fc1) / 32768.0).clamp(0.079407, 1.0)
}

/// One channel of the resonant low-pass.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LowPass {
    band: f32,
    low: f32,
}

impl LowPass {
    /// Filter one sample with the given cutoff and damping coefficients.
    pub fn run(&mut self, input: f32, cutoff: f32, damping: f32) -> f32 {
        let gain = (damping / 2.0 + 0.01).sqrt();
        let x = gain * input;
        self.band -= self.band * self.band * self.band * 0.001;
        let high = x - self.low - damping * self.band;
        self.band += cutoff * high;
        self.low += cutoff * self.band;
        self.low
    }

    /// Filter a frame in place from raw register values, clamping to [-1, 1].
    pub fn run_frame(&mut self, samples: &mut [f32], fc0: i16, fc1: u16) {
        let (cutoff, damping) = (cutoff_coefficient(fc0), damping_coefficient(fc1));
        for s in samples {
            *s = f64::from(self.run(*s, cutoff, damping)).clamp(-1.0, 1.0) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_selection() {
        assert!(filter_enabled(true, false, 1));
        assert!(!filter_enabled(true, false, 2));
        assert!(filter_enabled(false, false, 3));
        assert!(!filter_enabled(false, true, 3));
        assert!(!filter_enabled(false, false, 0));
    }

    #[test]
    fn coefficients_clamp() {
        assert_eq!(cutoff_coefficient(0), 1.0);
        assert_eq!(cutoff_coefficient(i16::MIN), 0.00390625);
        assert_eq!(damping_coefficient(0), 0.079407);
        assert_eq!(damping_coefficient(0x8000), 1.0);
    }

    #[test]
    fn dc_settles_and_stays_bounded() {
        let mut f = LowPass::default();
        let mut buf = [0.5f32; 4096];
        f.run_frame(&mut buf, -4096, 0x4000);
        assert!(buf.iter().all(|s| s.abs() <= 1.0));
        assert!(buf[4095] > 0.0 && buf[0] < buf[4095]);
    }
}
