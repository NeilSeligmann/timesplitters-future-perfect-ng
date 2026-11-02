//! Host video latch state for the opt-in T3938 presentation sink.
//!
//! The caller serializes access and owns the display thread. A failed submission
//! leaves the last good picture pending, just as the C sink does.

pub mod playback;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub number: u64,
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub submitted: u64,
    pub failed: u64,
    pub replaced: u64,
    pub presented: u64,
    pub vblanks: u64,
    pub last_number: u64,
}

#[derive(Default)]
pub struct Latch {
    pending: Option<Picture>,
    counts: Counts,
}

impl Latch {
    /// Borrows the most recently submitted picture before it is displayed.
    pub fn pending(&self) -> Option<&Picture> {
        self.pending.as_ref()
    }

    /// Records a submission. A missing or undersized RGB buffer is a failure.
    /// Failed submissions preserve a previously latched frame.
    pub fn submit(&mut self, number: u64, width: u32, height: u32, rgb: Option<&[u8]>) {
        self.counts.submitted = self.counts.submitted.wrapping_add(1);
        self.counts.last_number = number;
        let len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(3));
        let Some((len, pixels)) = len.filter(|_| width != 0 && height != 0).and_then(|len| {
            rgb.filter(|bytes| bytes.len() >= len)
                .map(|bytes| (len, bytes))
        }) else {
            self.counts.failed = self.counts.failed.wrapping_add(1);
            return;
        };
        let mut copy = Vec::new();
        if copy.try_reserve_exact(len).is_err() {
            self.counts.failed = self.counts.failed.wrapping_add(1);
            return;
        }
        copy.extend_from_slice(&pixels[..len]);
        if self.pending.is_some() {
            self.counts.replaced = self.counts.replaced.wrapping_add(1);
        }
        self.pending = Some(Picture {
            number,
            width,
            height,
            rgb: copy,
        });
    }

    /// Takes the newest valid picture at a display opportunity.
    pub fn take(&mut self) -> Option<Picture> {
        self.pending.take()
    }

    /// One modelled vblank: shows the latched picture if a new one arrived since the last vblank.
    pub fn vblank(&mut self) -> Option<Picture> {
        self.counts.vblanks = self.counts.vblanks.wrapping_add(1);
        let shown = self.pending.take();
        if shown.is_some() {
            self.counts.presented = self.counts.presented.wrapping_add(1);
        }
        shown
    }

    pub fn counts(&self) -> Counts {
        self.counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_and_replaced_frames_preserve_latest_valid_picture() {
        let mut latch = Latch::default();
        latch.submit(1, 1, 1, Some(&[1, 2, 3]));
        latch.submit(2, 0, 1, None);
        latch.submit(3, 1, 1, Some(&[4, 5, 6]));
        latch.submit(4, 1, 1, Some(&[7]));
        assert_eq!(
            latch.counts(),
            Counts {
                submitted: 4,
                failed: 2,
                replaced: 1,
                presented: 0,
                vblanks: 0,
                last_number: 4
            }
        );
        assert_eq!(latch.take().unwrap().rgb, [4, 5, 6]);
        assert_eq!(latch.take(), None);
    }

    /// The same script the C window sink runs in tests/c/test_present_latch_trace.c.
    #[test]
    fn matches_c_oracle_golden_trace() {
        let mut latch = Latch::default();
        let mut lines = 0;
        for line in include_str!("../golden/latch_trace.txt").lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let (op, want) = line.split_once('|').unwrap();
            let op: Vec<&str> = op.split_whitespace().collect();
            match op[0] {
                "S" => {
                    let (n, w, h) = (
                        op[1].parse().unwrap(),
                        op[2].parse().unwrap(),
                        op[3].parse().unwrap(),
                    );
                    let pixels = vec![9u8; (w * h * 3) as usize];
                    latch.submit(n, w, h, (op[4] == "ok").then_some(&pixels[..]));
                }
                "V" => {
                    latch.vblank();
                }
                other => panic!("bad op {other}"),
            }
            let c = latch.counts();
            let got = [
                c.submitted,
                c.failed,
                c.replaced,
                c.presented,
                c.vblanks,
                c.last_number,
            ];
            let want: Vec<u64> = want
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            assert_eq!(got.to_vec(), want, "after `{line}`");
            lines += 1;
        }
        assert_eq!(lines, 9);
    }
}
