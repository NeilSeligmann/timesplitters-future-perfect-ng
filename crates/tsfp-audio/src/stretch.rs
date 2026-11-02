//! Pitch-preserving WSOLA blocks. Tier 1: synthetic differential against audio_stretch.c.
//! The caller owns the ring, pacing ratio and input window; offsets and lengths are frames.

pub struct Stretch {
    channels: usize,
    overlap: usize,
    tail: Vec<i16>,
    segment: Vec<i16>,
    primed: bool,
}

impl Stretch {
    /// Reject invalid dimensions and allocation failures without constructing partial state.
    pub fn new(channels: u16, overlap: usize) -> Option<Self> {
        if channels == 0 || overlap < 4 {
            return None;
        }
        let channels = usize::from(channels);
        let block = overlap.checked_mul(channels)?;
        let segment_len = block.checked_mul(2)?;
        let mut tail = Vec::new();
        tail.try_reserve_exact(block).ok()?;
        tail.resize(block, 0);
        let mut segment = Vec::new();
        segment.try_reserve_exact(segment_len).ok()?;
        segment.resize(segment_len, 0);
        Some(Self {
            channels,
            overlap,
            tail,
            segment,
            primed: false,
        })
    }
    pub fn overlap(&self) -> usize {
        self.overlap
    }
    pub fn search(&self) -> usize {
        self.overlap / 2
    }
    pub fn primed(&self) -> bool {
        self.primed
    }
    pub fn input_needed(&self, nominal: usize) -> Option<usize> {
        nominal
            .checked_add(self.search())?
            .checked_add(self.overlap.checked_mul(2)?)
    }
    pub fn prime(&mut self, input: &[i16], out: &mut [i16]) -> bool {
        let block = self.tail.len();
        if input.len() < self.segment.len() || out.len() < block {
            return false;
        }
        self.segment.copy_from_slice(&input[..2 * block]);
        self.tail.copy_from_slice(&input[block..2 * block]);
        out[..block].copy_from_slice(&input[..block]);
        self.primed = true;
        true
    }
    fn crossfade(&self, source: Option<&[i16]>, out: &mut [i16]) {
        for frame in 0..self.overlap {
            let weight = ((frame as u128 + 1) * 65536 / (self.overlap as u128 + 1)) as i64;
            for channel in 0..self.channels {
                let index = frame * self.channels + channel;
                let old = i64::from(self.tail[index]);
                let new = source.map_or(0, |s| i64::from(s[index]));
                out[index] = (old + (((new - old) * weight) >> 16)) as i16;
            }
        }
    }
    /// Return the selected frame offset, or refuse without changing state/output.
    pub fn next(&mut self, input: &[i16], nominal: usize, out: &mut [i16]) -> Option<usize> {
        let frames = input.len() / self.channels;
        let width = self.overlap * 2;
        if !self.primed || out.len() < self.tail.len() || nominal.checked_add(width)? > frames {
            return None;
        }
        let first = nominal.saturating_sub(self.search());
        let last = nominal.saturating_add(self.search()).min(frames - width);
        let mut best = nominal.clamp(first, last);
        let mut best_score = -1.0e300;
        for start in first..=last {
            let mut correlation = 0.0;
            let mut energy = 1.0;
            for frame in 0..self.overlap {
                let mono = |samples: &[i16], offset: usize| -> i32 {
                    samples[offset..offset + self.channels]
                        .iter()
                        .map(|v| i32::from(*v))
                        .sum()
                };
                let candidate = f64::from(mono(input, (start + frame) * self.channels));
                correlation += f64::from(mono(&self.tail, frame * self.channels)) * candidate;
                energy += candidate * candidate;
            }
            let mut score = correlation
                * if correlation < 0.0 {
                    -correlation
                } else {
                    correlation
                }
                / energy;
            if start == nominal {
                score += score.abs() * 1.0e-3;
            }
            if score > best_score {
                best_score = score;
                best = start;
            }
        }
        let offset = best * self.channels;
        self.crossfade(Some(&input[offset..]), out);
        self.segment
            .copy_from_slice(&input[offset..offset + width * self.channels]);
        let block = self.tail.len();
        self.tail
            .copy_from_slice(&input[offset + block..offset + width * self.channels]);
        Some(best)
    }
    pub fn hold(&mut self, out: &mut [i16]) -> bool {
        if !self.primed || out.len() < self.tail.len() {
            return false;
        }
        self.crossfade(Some(&self.segment), out);
        let block = self.tail.len();
        self.tail.copy_from_slice(&self.segment[block..]);
        true
    }
    pub fn leave(&mut self, input: &[i16], out: &mut [i16]) -> bool {
        if !self.primed || input.len() < self.tail.len() || out.len() < self.tail.len() {
            return false;
        }
        self.crossfade(Some(input), out);
        self.primed = false;
        true
    }
    pub fn fade_out(&mut self, out: &mut [i16]) -> bool {
        if !self.primed || out.len() < self.tail.len() {
            return false;
        }
        self.crossfade(None, out);
        self.primed = false;
        true
    }
}
