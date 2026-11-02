//! Sinks behind [`AudioSink`]: WAV file (offline, deterministic) and a pull ring (what a device
//! callback drains). A Bevy or cpal shell plugs in by implementing `AudioSink::write` (push) or by
//! owning a [`Ring`] and calling [`Ring::pull`] from its audio callback; see docs/t-audio-replace.md.

use crate::contract::{AudioSink, SoundHost};
use std::collections::VecDeque;
use std::io::{self, Seek, SeekFrom, Write};

/// Canonical 16 bit PCM WAV writer. The header is patched on `finish`.
pub struct WavSink<W: Write + Seek> {
    out: W,
    frames: u64,
    channels: u16,
    rate: u32,
}

impl<W: Write + Seek> WavSink<W> {
    pub fn new(mut out: W, rate: u32, channels: u16) -> io::Result<Self> {
        out.write_all(&header(rate, channels, 0))?;
        Ok(Self {
            out,
            frames: 0,
            channels,
            rate,
        })
    }
    pub fn frames(&self) -> u64 {
        self.frames
    }
    /// Patch the sizes and return the writer.
    pub fn finish(mut self) -> io::Result<W> {
        self.out.seek(SeekFrom::Start(0))?;
        let data = (self.frames * u64::from(self.channels) * 2) as u32;
        self.out
            .write_all(&header(self.rate, self.channels, data))?;
        self.out.seek(SeekFrom::End(0))?;
        Ok(self.out)
    }
}

fn header(rate: u32, channels: u16, data: u32) -> [u8; 44] {
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36 + data).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&1u16.to_le_bytes());
    h[22..24].copy_from_slice(&channels.to_le_bytes());
    h[24..28].copy_from_slice(&rate.to_le_bytes());
    h[28..32].copy_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    h[32..34].copy_from_slice(&(channels * 2).to_le_bytes());
    h[34..36].copy_from_slice(&16u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

impl<W: Write + Seek> AudioSink for WavSink<W> {
    fn write(&mut self, samples: &[i16]) {
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        if self.out.write_all(&bytes).is_ok() {
            self.frames += (samples.len() / usize::from(self.channels)) as u64;
        }
    }
}

/// Bounded producer/consumer ring of interleaved samples. `write` drops the excess when full
/// (counted as overrun) and `pull` pads with silence (counted as underrun), as the C sink does.
pub struct Ring {
    queue: VecDeque<i16>,
    capacity: usize,
    pub overruns: u64,
    pub underruns: u64,
}

/// Bounded stereo ring with the timestamped media clock used by the host output contract.
/// The first write anchors the start of its PCM block; the clock advances with real frames
/// pulled and interpolates only within the most recent device pull.
pub struct MediaClockRing {
    ring: Ring,
    sample_rate: u32,
    anchor_vt_ns: Option<u64>,
    pulled_frames: u64,
    last_pull_frames: u64,
    last_pull_ns: u64,
}

/// Convert a guest virtual-clock sample to nanoseconds using the original integer-floor rule.
/// Returns `None` for an invalid frequency or a result that cannot fit the media-clock ABI.
pub fn virtual_ticks_to_ns(ticks: u64, ticks_per_second: u64) -> Option<u64> {
    if ticks_per_second == 0 {
        return None;
    }
    let ns = u128::from(ticks) * 1_000_000_000 / u128::from(ticks_per_second);
    u64::try_from(ns).ok()
}

impl MediaClockRing {
    pub fn new(capacity_samples: usize, sample_rate: u32) -> Self {
        Self {
            ring: Ring::new(capacity_samples),
            sample_rate,
            anchor_vt_ns: None,
            pulled_frames: 0,
            last_pull_frames: 0,
            last_pull_ns: 0,
        }
    }

    /// Reset queue and media-clock state when a device is attached or its rate changes.
    pub fn reset(&mut self, sample_rate: u32) {
        self.ring = Ring::new((sample_rate as usize).saturating_mul(2));
        self.sample_rate = sample_rate;
        self.anchor_vt_ns = None;
        self.pulled_frames = 0;
        self.last_pull_frames = 0;
        self.last_pull_ns = 0;
    }

    pub fn fill(&self) -> usize {
        self.ring.fill()
    }

    /// Samples available without dropping or waiting. Capacity is measured in interleaved samples.
    pub fn available(&self) -> usize {
        self.ring.capacity.saturating_sub(self.ring.fill())
    }

    pub fn overruns(&self) -> u64 {
        self.ring.overruns
    }

    pub fn underruns(&self) -> u64 {
        self.ring.underruns
    }

    /// Drop queued audio without advancing the media clock.
    pub fn clear(&mut self) -> usize {
        self.ring.clear()
    }

    /// Queue interleaved stereo samples stamped with the virtual time at the final frame.
    pub fn write_at(&mut self, samples: &[i16], end_vt_ns: u64) {
        if self.sample_rate == 0 || samples.len() % 2 != 0 {
            return;
        }
        if self.anchor_vt_ns.is_none() {
            let frames = (samples.len() / 2) as u128;
            let span_ns = frames * 1_000_000_000u128 / u128::from(self.sample_rate);
            self.anchor_vt_ns =
                Some(end_vt_ns.saturating_sub(span_ns.min(u64::MAX as u128) as u64));
        }
        self.ring.write(samples);
    }

    /// Write as much of one timestamped stereo block as fits, without counting the remaining suffix
    /// as an overrun. The caller can wait for more room and continue this same block. `block_frames`
    /// and `block_offset_frames` preserve the original end timestamp across partial writes.
    pub fn write_prefix_at(
        &mut self,
        samples: &[i16],
        block_frames: usize,
        block_offset_frames: usize,
        end_vt_ns: u64,
    ) -> usize {
        if self.sample_rate == 0 || samples.len() % 2 != 0 {
            return 0;
        }
        let room = self.available() & !1;
        let take = samples.len().min(room) & !1;
        if take == 0 {
            return 0;
        }
        if self.anchor_vt_ns.is_none() {
            let block_ns =
                (block_frames as u128) * 1_000_000_000u128 / u128::from(self.sample_rate);
            let offset_ns =
                (block_offset_frames as u128) * 1_000_000_000u128 / u128::from(self.sample_rate);
            let start_ns = end_vt_ns.saturating_sub(block_ns.min(u64::MAX as u128) as u64);
            self.anchor_vt_ns =
                Some(start_ns.saturating_add(offset_ns.min(u64::MAX as u128) as u64));
        }
        self.ring.write(&samples[..take]);
        take
    }

    /// Fill a stereo device block, padding with silence as needed, and timestamp real frames.
    pub fn pull_at(&mut self, out: &mut [i16], now_ns: u64) -> usize {
        let real_samples = self.ring.pull(out);
        let real_frames = (real_samples / 2) as u64;
        self.pulled_frames = self.pulled_frames.saturating_add(real_frames);
        self.last_pull_frames = real_frames;
        self.last_pull_ns = now_ns;
        real_samples
    }

    /// Media time at `now_ns`, held at the last real frame once playback drains or stalls.
    pub fn media_time_ns(&self, now_ns: u64, av_offset_ns: i64) -> Option<u64> {
        let anchor = self.anchor_vt_ns?;
        if self.sample_rate == 0 {
            return None;
        }
        let elapsed_ns = now_ns.saturating_sub(self.last_pull_ns) as u128;
        let elapsed_frames = elapsed_ns * u128::from(self.sample_rate) / 1_000_000_000u128;
        let inside_last_pull = elapsed_frames.min(u128::from(self.last_pull_frames)) as u64;
        let played_frames = self
            .pulled_frames
            .saturating_sub(self.last_pull_frames)
            .saturating_add(inside_last_pull);
        let played_ns = (u128::from(played_frames) * 1_000_000_000u128
            / u128::from(self.sample_rate))
        .min(u64::MAX as u128) as u64;
        let media = i128::from(anchor) + i128::from(played_ns) - i128::from(av_offset_ns);
        Some(media.clamp(i128::from(anchor), i128::from(u64::MAX)) as u64)
    }
}

impl Ring {
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            capacity: capacity_samples,
            overruns: 0,
            underruns: 0,
        }
    }
    pub fn fill(&self) -> usize {
        self.queue.len()
    }
    /// Discard queued samples without reporting an underrun. Device detachment uses this to
    /// prevent stale audio from being replayed if the device is attached again later.
    pub fn clear(&mut self) -> usize {
        let discarded = self.queue.len();
        self.queue.clear();
        discarded
    }
    /// Fill `out` from the ring; the rest is silence. Returns the real samples copied.
    pub fn pull(&mut self, out: &mut [i16]) -> usize {
        let real = out.len().min(self.queue.len());
        for (slot, sample) in out.iter_mut().zip(self.queue.drain(..real)) {
            *slot = sample;
        }
        out[real..].fill(0);
        if real < out.len() {
            self.underruns += 1;
        }
        real
    }
}

impl AudioSink for Ring {
    fn write(&mut self, samples: &[i16]) {
        let room = self.capacity - self.queue.len();
        let take = samples.len().min(room);
        self.queue.extend(&samples[..take]);
        if take < samples.len() {
            self.overruns += 1;
        }
    }
}

/// Render a host up to `now_ticks` into a sink in chunks of at most `chunk_frames`, like the C pump
/// (it loops while a full chunk comes back). Returns the frames written.
pub fn pump(
    host: &mut dyn SoundHost,
    sink: &mut dyn AudioSink,
    now_ticks: u64,
    chunk_frames: usize,
) -> u64 {
    let mut total = 0u64;
    loop {
        let pcm = host.render_until(now_ticks, chunk_frames);
        total += (pcm.len() / 2) as u64;
        if !pcm.is_empty() {
            sink.write(&pcm);
        }
        if pcm.len() / 2 < chunk_frames {
            return total;
        }
    }
}

#[cfg(test)]
mod media_clock_tests {
    use super::{MediaClockRing, virtual_ticks_to_ns};

    #[test]
    fn virtual_tick_conversion_matches_host_integer_floor_rule() {
        let ticks_per_second = 733_333_333;
        for ticks in [
            0,
            1,
            ticks_per_second - 1,
            ticks_per_second,
            ticks_per_second + 1,
            ticks_per_second * 7 + 123,
            ticks_per_second * 3_600,
        ] {
            let expected = (ticks / ticks_per_second) * 1_000_000_000
                + (ticks % ticks_per_second) * 1_000_000_000 / ticks_per_second;
            assert_eq!(virtual_ticks_to_ns(ticks, ticks_per_second), Some(expected));
        }
        assert_eq!(virtual_ticks_to_ns(1, 0), None);
        assert_eq!(virtual_ticks_to_ns(u64::MAX, 1), None);
    }

    #[test]
    fn timestamped_ring_anchors_and_tracks_only_consumed_audio() {
        let mut ring = MediaClockRing::new(16, 1_000);
        assert_eq!(ring.media_time_ns(1_000_000_000, 0), None);
        ring.write_at(&[1, 1, 2, 2, 3, 3, 4, 4], 1_000_000_000);

        let mut block = [0; 4];
        assert_eq!(ring.pull_at(&mut block, 1_000_000_000), 4);
        assert_eq!(block, [1, 1, 2, 2]);
        assert_eq!(ring.media_time_ns(1_001_000_000, 0), Some(997_000_000));
        assert_eq!(ring.media_time_ns(1_004_000_000, 0), Some(998_000_000));
        assert_eq!(ring.media_time_ns(1_010_000_000, 0), Some(998_000_000));

        assert_eq!(ring.pull_at(&mut block, 1_010_000_000), 4);
        assert_eq!(ring.media_time_ns(1_012_000_000, 0), Some(1_000_000_000));
        assert_eq!(
            ring.media_time_ns(1_012_000_000, -2_000_000),
            Some(1_002_000_000)
        );
        assert_eq!(
            ring.media_time_ns(1_012_000_000, 2_000_000),
            Some(998_000_000)
        );
    }

    #[test]
    fn empty_device_pull_holds_clock_at_last_real_frame() {
        let mut ring = MediaClockRing::new(8, 1_000);
        ring.write_at(&[7, 7], 10_000_000);
        let mut block = [0; 4];
        ring.pull_at(&mut block, 0);
        assert_eq!(ring.media_time_ns(5_000_000, 0), Some(10_000_000));
        ring.pull_at(&mut block, 5_000_000);
        assert_eq!(ring.media_time_ns(15_000_000, 0), Some(10_000_000));
    }

    #[test]
    fn partial_timestamped_write_keeps_the_original_block_anchor() {
        let mut ring = MediaClockRing::new(2, 1_000);
        assert_eq!(ring.write_prefix_at(&[1, 1, 2, 2], 2, 0, 1_000_000_000), 2);
        let mut block = [0; 2];
        assert_eq!(ring.pull_at(&mut block, 0), 2);
        assert_eq!(ring.media_time_ns(1_000_000, 0), Some(999_000_000));

        assert_eq!(ring.write_prefix_at(&[2, 2], 2, 1, 1_000_000_000), 2);
        assert_eq!(ring.media_time_ns(1_000_000, 0), Some(999_000_000));
    }
}
