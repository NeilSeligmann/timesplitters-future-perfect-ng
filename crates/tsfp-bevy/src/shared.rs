// SPDX-License-Identifier: GPL-3.0-or-later
//! State shared between the Bevy main thread and the game thread (the C host): the latest-wins picture mailbox, the input
//! queue behind the `tsfp_wi_*` contract, the close request and the counters. Everything is behind short mutexes or atomics,
//! the game thread never waits for Bevy.
use crate::window_input::{InputState, PumpResult, RawEvent};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};

/// One RGBA8 picture (the C sink hands RGB, converted when it is copied in).
#[derive(Default)]
pub struct Picture {
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Default)]
pub struct Mailbox {
    pub picture: Picture,
    /// A picture arrived that Bevy has not taken yet.
    pub fresh: bool,
}

/// RGB to RGBA with opaque alpha. `out` is resized to the picture.
pub fn rgb_to_rgba(rgb: &[u8], out: &mut Vec<u8>) {
    out.clear();
    out.reserve(rgb.len() / 3 * 4);
    for pixel in rgb.chunks_exact(3) {
        out.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
    }
}

#[derive(Default)]
pub struct Counters {
    /// Pictures the C sink handed in.
    pub received: AtomicU64,
    /// Pictures replaced in the mailbox before Bevy took them (latest wins).
    pub replaced: AtomicU64,
    /// Pictures Bevy took out of the mailbox and uploaded.
    pub shown: AtomicU64,
    /// Bevy `Update` runs.
    pub bevy_frames: AtomicU64,
    /// Calls to the host's Rust window/input poll boundary (T2348).
    pub host_polls: AtomicU64,
}

impl Counters {
    /// Number the next host input poll from one and return its stable index.
    pub fn next_host_poll(&self) -> u64 {
        self.host_polls.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn host_poll_index(&self) -> u64 {
        self.host_polls.load(Ordering::Acquire)
    }
}

/// Input, window requests and gamepad bookkeeping behind the tsfp_wi_* functions.
#[derive(Default)]
pub struct Wi {
    pub input: InputState,
    /// Close/redraw seen since the last `tsfp_wi_pump`.
    pub pending: PumpResult,
    pub pads: HashSet<u32>,
    pub title: Option<String>,
    pub title_changed: bool,
    /// Digital gamepad buttons currently held (a raw button event carries a value, down/up is derived).
    pub held_buttons: HashSet<(u32, u8)>,
    pub last_axis: HashMap<(u32, u8), i16>,
}

impl Wi {
    pub fn feed(&mut self, event: RawEvent) {
        let _ = self.input.handle(event, &mut self.pending);
    }
}

pub struct Shared {
    pub mailbox: Mutex<Mailbox>,
    pub wi: Mutex<Wi>,
    pub counters: Counters,
    /// The game thread returned from tsfp_host_entry: Bevy exits.
    pub game_done: AtomicBool,
    /// Wall clock of the first picture and the latest one (ms since process start), for the pacing line.
    pub first_picture_ms: AtomicU64,
    pub last_picture_ms: AtomicU64,
}

pub static SHARED: LazyLock<Shared> = LazyLock::new(|| Shared {
    mailbox: Mutex::new(Mailbox::default()),
    wi: Mutex::new(Wi::default()),
    counters: Counters::default(),
    game_done: AtomicBool::new(false),
    first_picture_ms: AtomicU64::new(0),
    last_picture_ms: AtomicU64::new(0),
});

pub static START: LazyLock<std::time::Instant> = LazyLock::new(std::time::Instant::now);

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn now_ms() -> u64 {
    START.elapsed().as_millis() as u64
}

/// T3393: the clock of native UI input (hold repeat, tap drivers). A tick-clock replay runs it on
/// the simulation tick (60 Hz) so a replayed page sees the recorded timing, not the wall clock;
/// otherwise the wall clock.
pub fn ui_now_ms() -> u64 {
    if crate::input_recording::replaying() && crate::input_recording::tick_clock() {
        crate::wi::simulation_tick() * 1000 / 60
    } else {
        now_ms()
    }
}

impl Shared {
    /// Latest-wins publish (game side). Never blocks beyond the short lock.
    pub fn publish(&self, seq: u64, width: u32, height: u32, rgb: &[u8]) {
        self.publish_with(seq, width, height, |out| rgb_to_rgba(rgb, out));
    }

    /// Same for an RGBA8 frame (alpha forced opaque).
    pub fn publish_rgba(&self, seq: u64, width: u32, height: u32, rgba: &[u8]) {
        self.publish_with(seq, width, height, |out| {
            out.clear();
            out.extend_from_slice(rgba);
            out.iter_mut()
                .skip(3)
                .step_by(4)
                .for_each(|alpha| *alpha = 255);
        });
    }

    fn publish_with(&self, seq: u64, width: u32, height: u32, fill: impl FnOnce(&mut Vec<u8>)) {
        // T3390: `--render none` has nobody to show a picture to; skip the RGBA conversion.
        if crate::app::render_none() {
            self.counters.replaced.fetch_add(1, Ordering::Relaxed);
            return;
        }
        {
            let mut mailbox = lock(&self.mailbox);
            if mailbox.fresh {
                self.counters.replaced.fetch_add(1, Ordering::Relaxed);
            }
            fill(&mut mailbox.picture.rgba);
            mailbox.picture.seq = seq;
            mailbox.picture.width = width;
            mailbox.picture.height = height;
            mailbox.fresh = true;
        }
        self.counters.received.fetch_add(1, Ordering::Relaxed);
        let now = now_ms().max(1);
        let _ =
            self.first_picture_ms
                .compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed);
        self.last_picture_ms.store(now, Ordering::Relaxed);
    }

    /// Bevy side: swap the fresh picture's buffer with `spare` (no copy). Returns the picture header when there was one.
    pub fn take(&self, spare: &mut Vec<u8>) -> Option<(u64, u32, u32)> {
        let mut mailbox = lock(&self.mailbox);
        if !mailbox.fresh {
            return None;
        }
        mailbox.fresh = false;
        std::mem::swap(&mut mailbox.picture.rgba, spare);
        self.counters.shown.fetch_add(1, Ordering::Relaxed);
        Some((
            mailbox.picture.seq,
            mailbox.picture.width,
            mailbox.picture.height,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_expands_to_opaque_rgba() {
        let mut out = Vec::new();
        rgb_to_rgba(&[1, 2, 3, 4, 5, 6], &mut out);
        assert_eq!(out, [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn rgba_frames_are_forced_opaque() {
        let shared = Shared {
            mailbox: Mutex::new(Mailbox::default()),
            wi: Mutex::new(Wi::default()),
            counters: Counters::default(),
            game_done: AtomicBool::new(false),
            first_picture_ms: AtomicU64::new(0),
            last_picture_ms: AtomicU64::new(0),
        };
        shared.publish_rgba(7, 2, 1, &[1, 2, 3, 0, 4, 5, 6, 9]);
        let mut spare = Vec::new();
        assert_eq!(shared.take(&mut spare), Some((7, 2, 1)));
        assert_eq!(spare, [1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn host_poll_indices_are_one_based_and_monotonic() {
        let counters = Counters::default();
        assert_eq!(counters.host_poll_index(), 0);
        assert_eq!(counters.next_host_poll(), 1);
        assert_eq!(counters.next_host_poll(), 2);
        assert_eq!(counters.host_poll_index(), 2);
    }

    #[test]
    fn mailbox_is_latest_wins_and_counts_replaced() {
        let shared = Shared {
            mailbox: Mutex::new(Mailbox::default()),
            wi: Mutex::new(Wi::default()),
            counters: Counters::default(),
            game_done: AtomicBool::new(false),
            first_picture_ms: AtomicU64::new(0),
            last_picture_ms: AtomicU64::new(0),
        };
        shared.publish(1, 1, 1, &[10, 20, 30]);
        shared.publish(2, 1, 1, &[40, 50, 60]);
        let mut spare = Vec::new();
        assert_eq!(shared.take(&mut spare), Some((2, 1, 1)));
        assert_eq!(spare, [40, 50, 60, 255]);
        assert_eq!(shared.take(&mut spare), None);
        assert_eq!(shared.counters.received.load(Ordering::Relaxed), 2);
        assert_eq!(shared.counters.replaced.load(Ordering::Relaxed), 1);
        assert_eq!(shared.counters.shown.load(Ordering::Relaxed), 1);
    }
}

/// T3394: one generation counter both sides wait on instead of spinning. The guest bumps it at every
/// host input poll (`tsfp_wi_pump`), Bevy bumps it when it delivered replay rows. A headless Bevy
/// update waits for a guest poll, the guest's replay lockstep waits for a delivery.
pub struct ProgressGate {
    generation: Mutex<u64>,
    changed: std::sync::Condvar,
}

pub static PROGRESS: ProgressGate = ProgressGate {
    generation: Mutex::new(0),
    changed: std::sync::Condvar::new(),
};

impl ProgressGate {
    pub fn current(&self) -> u64 {
        *lock(&self.generation)
    }

    pub fn bump(&self) {
        *lock(&self.generation) += 1;
        self.changed.notify_all();
    }

    /// Wait until the generation differs from `seen` or `timeout` passes; returns the generation.
    pub fn wait_past(&self, seen: u64, timeout: std::time::Duration) -> u64 {
        let guard = lock(&self.generation);
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, timeout, |generation| *generation == seen)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard
    }
}

/// T3394: Bevy updates of a `--render none` run (reported at exit with the guest poll count).
pub static HEADLESS_UPDATES: AtomicU64 = AtomicU64::new(0);
