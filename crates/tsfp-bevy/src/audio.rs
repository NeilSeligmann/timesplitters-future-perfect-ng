// SPDX-License-Identifier: GPL-3.0-or-later
//! Host audio output through Bevy. The default path pulls the existing C sink; with
//! `TSFP_BEVY_AUDIO_RUST_RING=1`, the selected Rust audio runtime submits PCM blocks directly into the bounded
//! `tsfp-audio::sink::MediaClockRing`, and the Bevy callback consumes only that Rust-owned ring. The C pull remains the A/B baseline.
//!
//! No audio device (a container, CI): Bevy's output does not exist and nobody would pull, which would stall the paced ring
//! writer. A fallback thread then pulls at real time and discards (the role of SDL_AUDIODRIVER=dummy). It watches the time of
//! the last Bevy pull, so a device that appears or disappears later is followed.
use bevy::audio::{AddAudioSource, AudioPlayer, Decodable, PlaybackSettings, Source};
use bevy::prelude::*;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tsfp_audio::sink::MediaClockRing;

pub type PullFn = unsafe extern "C" fn(context: *mut c_void, out: *mut i16, frames: usize) -> usize;

struct Attached {
    pull: PullFn,
    context: *mut c_void,
    rate: u32,
}
// SAFETY: the pull function is called with the context the C side registered; it is thread safe by contract
// (present_audio_sink_pull is safe against a concurrent write) and detach waits on the same mutex.
unsafe impl Send for Attached {}

static ATTACHED: Mutex<Option<Attached>> = Mutex::new(None);
const RUST_BRIDGE_ENV: &str = "TSFP_BEVY_AUDIO_RUST_RING";
static RUST_BRIDGE_ENABLED: AtomicBool = AtomicBool::new(false);
static ATTACHED_AT_MS: AtomicU64 = AtomicU64::new(0);
static RUST_RING: LazyLock<SharedRing> = LazyLock::new(|| SharedRing::new(48_000));
static MONOTONIC_START: LazyLock<Instant> = LazyLock::new(Instant::now);
/// Set when the sink attached and the Bevy side has not spawned its player yet.
static NEED_PLAYER: AtomicBool = AtomicBool::new(false);
static LAST_BEVY_PULL_MS: AtomicU64 = AtomicU64::new(0);
pub static PULLED_BY_BEVY: AtomicU64 = AtomicU64::new(0);
pub static PULLED_BY_FALLBACK: AtomicU64 = AtomicU64::new(0);
pub static RUST_RING_OVERRUNS: AtomicU64 = AtomicU64::new(0);
pub static RUST_RING_UNDERRUNS: AtomicU64 = AtomicU64::new(0);
pub fn rust_ring_enabled() -> bool {
    RUST_BRIDGE_ENABLED.load(Ordering::Acquire)
}
static FALLBACK: LazyLock<()> = LazyLock::new(|| {
    std::thread::Builder::new()
        .name("tsfp-audio-fallback".into())
        .spawn(fallback_loop)
        .expect("spawn the audio fallback thread");
});

const BLOCK_FRAMES: usize = 1024;
const RUST_PACE_TIMEOUT: Duration = Duration::from_secs(2);

/// T2487: fast-forward replays (TSFP_FAST_FORWARD=1) never pace the guest on the audio device drain; the ring drops
/// its overflow instead. Audio timing is already on the guest's virtual clock.
fn fast_forward_enabled(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

fn pace_timeout() -> Duration {
    static FAST_FORWARD: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let fast = *FAST_FORWARD
        .get_or_init(|| fast_forward_enabled(std::env::var("TSFP_FAST_FORWARD").ok().as_deref()));
    if fast {
        Duration::ZERO
    } else {
        RUST_PACE_TIMEOUT
    }
}

/// Rust-owned producer/consumer queue. A real active device can pace the producer for the
/// same bounded two-second stall window as the hybrid sink; without a pulling device writes
/// remain nonblocking and retain the existing discard behavior.
struct SharedRing {
    state: Mutex<MediaClockRing>,
    room: Condvar,
}

impl SharedRing {
    fn new(sample_rate: u32) -> Self {
        Self {
            state: Mutex::new(MediaClockRing::new(
                (sample_rate as usize).saturating_mul(2),
                sample_rate,
            )),
            room: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, MediaClockRing> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn reset(&self, sample_rate: u32) {
        self.lock().reset(sample_rate);
        self.room.notify_all();
    }

    fn clear(&self) {
        self.lock().clear();
        self.room.notify_all();
    }

    fn write_block(
        &self,
        samples: &[i16],
        end_vt_ns: u64,
        device_recent: bool,
        device_pulling: bool,
        timeout: Duration,
    ) -> u64 {
        let mut ring = self.lock();
        if !device_recent {
            ring.clear();
            return 0;
        }

        let mut written = 0;
        'write: while written < samples.len() {
            let prefix = ring.write_prefix_at(
                &samples[written..],
                samples.len() / 2,
                written / 2,
                end_vt_ns,
            );
            written += prefix;
            if written == samples.len() || !device_pulling || timeout.is_zero() {
                break;
            }
            if ring.available() >= 2 {
                continue;
            }
            let deadline = Instant::now() + timeout;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break 'write;
                }
                let waited = self.room.wait_timeout(ring, remaining);
                let (next, timed_out) = match waited {
                    Ok((guard, result)) => (guard, result.timed_out()),
                    Err(poison) => {
                        let (guard, result) = poison.into_inner();
                        (guard, result.timed_out())
                    }
                };
                ring = next;
                if ring.available() >= 2 {
                    break;
                }
                if timed_out {
                    break 'write;
                }
            }
        }

        let before = ring.overruns();
        if written < samples.len() {
            ring.write_at(&samples[written..], end_vt_ns);
        }
        ring.overruns().saturating_sub(before)
    }

    fn pull_at(&self, out: &mut [i16], now_ns: u64) -> (usize, u64) {
        let mut ring = self.lock();
        let before_underruns = ring.underruns();
        let real = ring.pull_at(out, now_ns);
        let underruns = ring.underruns().saturating_sub(before_underruns);
        drop(ring);
        self.room.notify_all();
        (real, underruns)
    }

    fn media_time_ns(&self, now_ns: u64, av_offset_ns: i64) -> Option<u64> {
        self.lock().media_time_ns(now_ns, av_offset_ns)
    }
}

/// Moves C sink samples to the Rust device ring, or discards them without a device. In the baseline mode it preserves the
/// old behavior and pulls only when Bevy has not done so recently. 10 ms steps, frames owed by the wall clock.
fn fallback_loop() {
    let mut owed = 0.0f64;
    let mut last = std::time::Instant::now();
    let mut block = vec![0i16; BLOCK_FRAMES * 2];
    loop {
        std::thread::sleep(Duration::from_millis(10));
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(last).as_secs_f64();
        last = now;
        let now_ms = crate::shared::now_ms();
        let bridge = RUST_BRIDGE_ENABLED.load(Ordering::Acquire);
        let last_bevy_pull = LAST_BEVY_PULL_MS.load(Ordering::Relaxed);
        let bevy_recent = if bridge {
            if last_bevy_pull == 0 {
                now_ms.saturating_sub(ATTACHED_AT_MS.load(Ordering::Relaxed)) < 250
            } else {
                now_ms.saturating_sub(last_bevy_pull) < 250
            }
        } else {
            last_bevy_pull != 0 && now_ms.saturating_sub(last_bevy_pull) < 250
        };
        let guard = crate::shared::lock(&ATTACHED);
        let Some(attached) = guard.as_ref() else {
            owed = 0.0;
            continue;
        };
        if bridge && !bevy_recent {
            RUST_RING.clear();
        } else if !bridge && bevy_recent {
            owed = 0.0;
            continue;
        }
        // In direct-ring mode the runtime writes rendered PCM into Rust already. The
        // legacy fallback pump must never pull the C sink on this path.
        if bridge {
            owed = 0.0;
            continue;
        }
        owed += elapsed * f64::from(attached.rate);
        while owed >= BLOCK_FRAMES as f64 {
            block.fill(0);
            // SAFETY: `block` holds BLOCK_FRAMES stereo frames; the context is valid while ATTACHED is locked.
            let real =
                unsafe { (attached.pull)(attached.context, block.as_mut_ptr(), BLOCK_FRAMES) };
            PULLED_BY_FALLBACK.fetch_add(real as u64, Ordering::Relaxed);
            owed -= BLOCK_FRAMES as f64;
        }
    }
}

fn device_is_recent(now_ms: u64, attached_at_ms: u64, last_pull_ms: u64) -> bool {
    if last_pull_ms == 0 {
        now_ms.saturating_sub(attached_at_ms) < 250
    } else {
        now_ms.saturating_sub(last_pull_ms) < 250
    }
}

fn monotonic_ns() -> u64 {
    MONOTONIC_START.elapsed().as_nanos().min(u64::MAX as u128) as u64
}

/// Receives a rendered block directly from the selected audio runtime. Status 0 keeps
/// the legacy C sink path, 1 handled (written or discarded without a device), -1 invalid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_bevy_audio_write(
    stereo: *const i16,
    frames: usize,
    end_vt_ticks: u64,
    ticks_per_second: u64,
) -> i32 {
    if !RUST_BRIDGE_ENABLED.load(Ordering::Acquire) {
        return 0;
    }
    let Some(sample_count) = frames.checked_mul(2) else {
        return -1;
    };
    if sample_count == 0 || stereo.is_null() {
        return -1;
    }
    let Some(end_vt_ns) = tsfp_audio::sink::virtual_ticks_to_ns(end_vt_ticks, ticks_per_second)
    else {
        return -1;
    };
    // SAFETY: the C audio runtime guarantees the synchronous PCM block remains readable.
    let samples = unsafe { std::slice::from_raw_parts(stereo, sample_count) };
    let now_ms = crate::shared::now_ms();
    let recent = device_is_recent(
        now_ms,
        ATTACHED_AT_MS.load(Ordering::Relaxed),
        LAST_BEVY_PULL_MS.load(Ordering::Relaxed),
    );
    let last_pull_ms = LAST_BEVY_PULL_MS.load(Ordering::Relaxed);
    let device_pulling = last_pull_ms != 0 && recent;
    let overruns =
        RUST_RING.write_block(samples, end_vt_ns, recent, device_pulling, pace_timeout());
    RUST_RING_OVERRUNS.fetch_add(overruns, Ordering::Relaxed);
    if !recent {
        PULLED_BY_FALLBACK.fetch_add(frames as u64, Ordering::Relaxed);
    }
    1
}

/// Direct callback registered with the Rust SoundHost. The pump now enters this Rust writer
/// without routing each PCM block through the C host sink callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_bevy_audio_runtime_write(
    _context: *mut c_void,
    stereo: *const i16,
    frames: usize,
    end_vt_ticks: u64,
    ticks_per_second: u64,
) -> i32 {
    // SAFETY: the SoundHost pump invokes this synchronously with its live PCM block.
    unsafe { tsfp_bevy_audio_write(stereo, frames, end_vt_ticks, ticks_per_second) }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_bevy_audio_direct_enabled() -> bool {
    RUST_BRIDGE_ENABLED.load(Ordering::Acquire)
}

/// Returns the timestamped media clock owned by the direct Bevy ring.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_bevy_audio_media_time(vt_ns: *mut u64, av_offset_ns: i64) -> bool {
    if vt_ns.is_null() || !RUST_BRIDGE_ENABLED.load(Ordering::Acquire) {
        return false;
    }
    let Some(media_time) = RUST_RING.media_time_ns(monotonic_ns(), av_offset_ns) else {
        return false;
    };
    // SAFETY: the C caller supplies one writable u64 when the pointer is non-null.
    unsafe { *vt_ns = media_time };
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_bevy_audio_attach(
    pull: PullFn,
    context: *mut c_void,
    sample_rate: u32,
    _channels: u32,
) {
    let rust_bridge = std::env::var(RUST_BRIDGE_ENV).is_ok_and(|value| value == "1");
    RUST_BRIDGE_ENABLED.store(false, Ordering::Release);
    *crate::shared::lock(&ATTACHED) = Some(Attached {
        pull,
        context,
        rate: sample_rate,
    });
    RUST_RING.reset(sample_rate);
    ATTACHED_AT_MS.store(crate::shared::now_ms().max(1), Ordering::Relaxed);
    LAST_BEVY_PULL_MS.store(0, Ordering::Relaxed);
    RUST_BRIDGE_ENABLED.store(rust_bridge, Ordering::Release);
    NEED_PLAYER.store(true, Ordering::Release);
    LazyLock::force(&FALLBACK);
}

/// Returns once no pull is running any more (it takes the mutex the pulls hold).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_bevy_audio_detach() {
    *crate::shared::lock(&ATTACHED) = None;
    RUST_RING.clear();
    RUST_BRIDGE_ENABLED.store(false, Ordering::Release);
}

/// The Bevy asset: an endless stereo S16 stream at the sink's rate.
#[derive(Asset, TypePath)]
pub struct PcmSource {
    rate: u32,
}

pub struct PcmDecoder {
    rate: u32,
    block: Vec<i16>,
    cursor: usize,
}

impl PcmDecoder {
    fn refill(&mut self) {
        self.block.clear();
        self.block.resize(BLOCK_FRAMES * 2, 0);
        if RUST_BRIDGE_ENABLED.load(Ordering::Acquire) {
            let (_, underruns) = RUST_RING.pull_at(&mut self.block, monotonic_ns());
            RUST_RING_UNDERRUNS.fetch_add(underruns, Ordering::Relaxed);
        } else {
            let guard = crate::shared::lock(&ATTACHED);
            if let Some(attached) = guard.as_ref() {
                // SAFETY: `block` holds BLOCK_FRAMES stereo frames; the context is valid while ATTACHED is locked.
                let real = unsafe {
                    (attached.pull)(attached.context, self.block.as_mut_ptr(), BLOCK_FRAMES)
                };
                PULLED_BY_BEVY.fetch_add(real as u64, Ordering::Relaxed);
            }
        }
        LAST_BEVY_PULL_MS.store(crate::shared::now_ms().max(1), Ordering::Relaxed);
        self.cursor = 0;
    }
}

impl Iterator for PcmDecoder {
    type Item = i16;

    fn next(&mut self) -> Option<i16> {
        if self.cursor >= self.block.len() {
            self.refill();
        }
        let sample = self.block[self.cursor];
        self.cursor += 1;
        Some(sample)
    }
}

impl Source for PcmDecoder {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for PcmSource {
    type DecoderItem = i16;
    type Decoder = PcmDecoder;

    fn decoder(&self) -> PcmDecoder {
        PcmDecoder {
            rate: self.rate,
            block: Vec::new(),
            cursor: 0,
        }
    }
}

pub struct AudioBridgePlugin;

impl Plugin for AudioBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_audio_source::<PcmSource>()
            .add_systems(Update, spawn_player);
    }
}

fn spawn_player(mut commands: Commands, mut sources: ResMut<Assets<PcmSource>>) {
    if !NEED_PLAYER.swap(false, Ordering::AcqRel) {
        return;
    }
    let rate = crate::shared::lock(&ATTACHED)
        .as_ref()
        .map_or(48_000, |attached| attached.rate);
    let handle = sources.add(PcmSource { rate });
    commands.spawn((AudioPlayer(handle), PlaybackSettings::ONCE));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};

    #[test]
    fn fast_forward_switch_parses_like_the_c_host() {
        assert!(!fast_forward_enabled(None));
        assert!(!fast_forward_enabled(Some("")));
        assert!(!fast_forward_enabled(Some("0")));
        assert!(fast_forward_enabled(Some("1")));
    }

    #[test]
    fn rust_audio_ring_preserves_samples_and_pads_short_reads() {
        let mut ring = MediaClockRing::new(6, 48_000);
        ring.write_at(&[1, 2, 3, 4], 1_000_000);
        let mut block = [0; 6];
        assert_eq!(ring.pull_at(&mut block, 10), 4);
        assert_eq!(block, [1, 2, 3, 4, 0, 0]);
        assert_eq!((ring.fill(), ring.underruns()), (0, 1));
    }

    #[test]
    fn direct_runtime_write_queues_only_while_bevy_device_is_recent() {
        let ring = SharedRing::new(2);
        assert_eq!(
            ring.write_block(&[1, 2, 3, 4], 1, true, false, Duration::ZERO),
            0
        );
        assert_eq!(ring.lock().fill(), 4);
        assert_eq!(ring.write_block(&[5, 6], 2, true, false, Duration::ZERO), 1);
        assert_eq!(ring.lock().fill(), 4);
        assert_eq!(
            ring.write_block(&[7, 8], 3, false, false, Duration::ZERO),
            0
        );
        assert_eq!(ring.lock().fill(), 0);
    }

    #[test]
    fn direct_runtime_write_uses_recent_device_window() {
        assert!(device_is_recent(120, 100, 0));
        assert!(!device_is_recent(400, 100, 0));
        assert!(device_is_recent(340, 100, 200));
        assert!(!device_is_recent(500, 100, 200));
    }

    #[test]
    fn active_device_pull_unblocks_a_full_ring_writer() {
        let ring = Arc::new(SharedRing::new(2));
        assert_eq!(
            ring.write_block(&[1, 1, 2, 2], 1_000_000, true, false, Duration::ZERO),
            0
        );

        let producer = Arc::clone(&ring);
        let (started, started_rx) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            started.send(()).unwrap();
            producer.write_block(&[3, 3, 4, 4], 3_000_000, true, true, Duration::from_secs(1))
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        std::thread::sleep(Duration::from_millis(10));

        let mut old_audio = [0; 4];
        assert_eq!(ring.pull_at(&mut old_audio, 1_000_000).0, 4);
        assert_eq!(old_audio, [1, 1, 2, 2]);
        assert_eq!(writer.join().unwrap(), 0);

        let mut new_audio = [0; 4];
        assert_eq!(ring.pull_at(&mut new_audio, 2_000_000).0, 4);
        assert_eq!(new_audio, [3, 3, 4, 4]);
    }

    #[test]
    fn stalled_full_ring_writer_drops_only_after_the_timeout() {
        let ring = SharedRing::new(2);
        assert_eq!(
            ring.write_block(&[1, 1, 2, 2], 1_000_000, true, false, Duration::ZERO),
            0
        );
        assert_eq!(
            ring.write_block(
                &[3, 3, 4, 4],
                3_000_000,
                true,
                true,
                Duration::from_millis(1),
            ),
            1
        );
        let state = ring.lock();
        assert_eq!((state.fill(), state.overruns()), (4, 1));
    }
}
