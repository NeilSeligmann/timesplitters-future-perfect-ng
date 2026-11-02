//! Movie playback picture queue (T3938), the state half of the C sink's `playback_tick` and
//! `present_video_sink_submit_at` in `src/host/present_sink.c`.
//!
//! The caller serializes access. Blocking on a full queue (the C sink waits up to
//! `PLAYBACK_ROOM_WAIT_MS` for the presenter) stays with the caller: it calls
//! [`PlaybackQueue::submit`], and on [`Submit::Full`] waits and either retries or calls
//! [`PlaybackQueue::drop_oldest_on_timeout`].

use std::collections::VecDeque;

use crate::Picture;

/// Matches `PLAYBACK_QUEUE_SLOTS`.
pub const QUEUE_SLOTS: usize = 96;
/// Matches `PLAYBACK_LATE_NS`: a picture shown later than this after its timestamp counts as late.
pub const LATE_NS: u64 = 40_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Queued {
    picture: Picture,
    vt_ns: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub dropped: u64,
    pub late_shown: u64,
    pub queue_max: usize,
    pub queue_timeouts: u64,
    pub model_gap_max_ns: u64,
    pub model_gap_after: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Submit {
    Queued,
    /// The queue holds [`QUEUE_SLOTS`] pictures; the picture was not taken.
    Full,
    /// Invalid dimensions or short buffer, or allocation failure.
    Failed,
}

#[derive(Default)]
pub struct PlaybackQueue {
    queue: VecDeque<Queued>,
    last_submit: Option<(u64, u64)>,
    timing: Timing,
    submitted: u64,
    failed: u64,
    presented: u64,
}

impl PlaybackQueue {
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn timing(&self) -> Timing {
        self.timing
    }

    /// (submitted, failed, presented)
    pub fn counts(&self) -> (u64, u64, u64) {
        (self.submitted, self.failed, self.presented)
    }

    /// Queues one decoded picture stamped `vt_ns` on the media clock.
    pub fn submit(
        &mut self,
        number: u64,
        width: u32,
        height: u32,
        rgb: &[u8],
        vt_ns: u64,
    ) -> Submit {
        self.submitted = self.submitted.wrapping_add(1);
        if self.queue.len() >= QUEUE_SLOTS {
            return Submit::Full;
        }
        let Some(len) = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(3))
            .filter(|&n| n != 0 && rgb.len() >= n)
        else {
            self.failed += 1;
            return Submit::Failed;
        };
        let mut copy = Vec::new();
        if copy.try_reserve_exact(len).is_err() {
            self.failed += 1;
            return Submit::Failed;
        }
        copy.extend_from_slice(&rgb[..len]);
        if let Some((last_vt, last_number)) = self.last_submit {
            if vt_ns > last_vt && vt_ns - last_vt > self.timing.model_gap_max_ns {
                self.timing.model_gap_max_ns = vt_ns - last_vt;
                self.timing.model_gap_after = last_number;
            }
        }
        self.last_submit = Some((vt_ns, number));
        self.queue.push_back(Queued {
            picture: Picture {
                number,
                width,
                height,
                rgb: copy,
            },
            vt_ns,
        });
        self.timing.queue_max = self.timing.queue_max.max(self.queue.len());
        Submit::Queued
    }

    /// The caller's room wait timed out: the oldest picture is dropped to make room.
    pub fn drop_oldest_on_timeout(&mut self) {
        self.timing.queue_timeouts += 1;
        if self.queue.pop_front().is_some() {
            self.timing.dropped += 1;
        }
    }

    /// One presenter tick. With `media_vt` the pictures due by then are consumed and the newest
    /// is shown, older ones count as dropped; without a clock (or at the stop, `everything`) the
    /// newest queued picture is shown at once.
    pub fn tick(&mut self, media_vt: Option<u64>, everything: bool) -> Option<Picture> {
        if self.queue.is_empty() {
            return None;
        }
        let clock = media_vt.filter(|_| !everything);
        let consumed = match clock {
            None => self.queue.len(),
            Some(vt) => self.queue.iter().take_while(|q| q.queue_due(vt)).count(),
        };
        if consumed == 0 {
            return None;
        }
        let shown = self.queue.drain(..consumed).last()?;
        if clock.is_some_and(|vt| vt - shown.vt_ns > LATE_NS) {
            self.timing.late_shown += 1;
        }
        self.timing.dropped += consumed as u64 - 1;
        self.presented += 1;
        Some(shown.picture)
    }
}

impl Queued {
    fn queue_due(&self, media_vt: u64) -> bool {
        self.vt_ns <= media_vt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(q: &mut PlaybackQueue, n: u64, vt: u64) -> Submit {
        q.submit(n, 1, 1, &[n as u8; 3], vt)
    }

    #[test]
    fn due_pictures_collapse_to_newest_and_count_drops() {
        let mut q = PlaybackQueue::default();
        for n in 0..4 {
            feed(&mut q, n, n * 16_000_000);
        }
        assert!(
            q.tick(Some(10_000_000), false)
                .is_some_and(|p| p.number == 0)
        );
        assert_eq!(q.tick(Some(10_000_000), false), None);
        // media jumps to 50 ms: pictures 1 (16), 2 (32), 3 (48) all due; 3 shown, 2 dropped.
        assert_eq!(q.tick(Some(50_000_000), false).unwrap().number, 3);
        assert_eq!(q.timing().dropped, 2);
        assert_eq!(q.timing().late_shown, 0);
        assert!(q.is_empty());
    }

    #[test]
    fn late_picture_is_counted_after_forty_ms() {
        let mut q = PlaybackQueue::default();
        feed(&mut q, 1, 0);
        q.tick(Some(LATE_NS), false);
        feed(&mut q, 2, 0);
        q.tick(Some(LATE_NS + 1), false);
        assert_eq!(q.timing().late_shown, 1);
    }

    #[test]
    fn no_clock_or_stop_shows_newest_immediately() {
        let mut q = PlaybackQueue::default();
        feed(&mut q, 1, 1_000);
        feed(&mut q, 2, 2_000);
        assert_eq!(q.tick(None, false).unwrap().number, 2);
        feed(&mut q, 3, 9_000_000_000);
        assert_eq!(q.tick(Some(0), true).unwrap().number, 3);
        assert_eq!(q.timing().dropped, 1);
        assert_eq!(q.counts(), (3, 0, 2));
    }

    #[test]
    fn full_queue_reports_full_then_timeout_drops_oldest() {
        let mut q = PlaybackQueue::default();
        for n in 0..QUEUE_SLOTS as u64 {
            assert_eq!(feed(&mut q, n, n), Submit::Queued);
        }
        assert_eq!(feed(&mut q, 99, 99), Submit::Full);
        q.drop_oldest_on_timeout();
        assert_eq!(feed(&mut q, 99, 99), Submit::Queued);
        let t = q.timing();
        assert_eq!(
            (t.dropped, t.queue_timeouts, t.queue_max),
            (1, 1, QUEUE_SLOTS)
        );
        assert_eq!(q.tick(None, false).unwrap().number, 99);
    }

    #[test]
    fn invalid_pictures_fail_and_gap_is_tracked() {
        let mut q = PlaybackQueue::default();
        assert_eq!(q.submit(1, 0, 1, &[], 0), Submit::Failed);
        assert_eq!(q.submit(2, 1, 1, &[1], 0), Submit::Failed);
        feed(&mut q, 3, 10);
        feed(&mut q, 4, 40);
        feed(&mut q, 5, 50);
        let t = q.timing();
        assert_eq!((t.model_gap_max_ns, t.model_gap_after), (30, 3));
        assert_eq!(q.counts().1, 2);
    }
}
