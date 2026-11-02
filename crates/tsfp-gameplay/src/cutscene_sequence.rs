// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned in-engine cutscene sequencing state (T2374).
//!
//! This keeps the parsed CSINFO and ANRS resources alive with the playback
//! clock and typed track records. The slot key-window query follows lifted
//! `0x98380`; it does not resolve actor/object handles or claim their lifetime.
//! The model is opt-in and is not wired into the default game path.

use tsfp_format_csinfo::Csinfo;
use tsfp_format_igcs::Animation;

pub const PLAYBACK_SLOT_COUNT: usize = 60;

/// Owned parsed resources for one in-engine cutscene.
#[derive(Clone, Debug)]
pub struct CutsceneDefinition {
    csinfo: Csinfo,
    animation: Animation,
}

impl CutsceneDefinition {
    pub fn new(csinfo: Csinfo, animation: Animation) -> Self {
        Self { csinfo, animation }
    }

    pub fn csinfo(&self) -> &Csinfo {
        &self.csinfo
    }

    pub fn animation(&self) -> &Animation {
        &self.animation
    }
}

/// Inclusive authored key interval used by the original track query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyFrameRange {
    pub first: i32,
    pub last: i32,
}

/// Owned sequencing data for one playback slot. `node_index` is a typed index;
/// it is never a guest pointer. Actor and animation object handles are outside
/// this slice because their ownership boundary is still open in the REA record.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackTrack {
    pub animation_id: u32,
    pub node_index: u32,
    pub start_seconds: f32,
    pub enabled: bool,
    pub linkage_index: Option<u32>,
    pub key_ranges: Vec<KeyFrameRange>,
}

impl PlaybackTrack {
    pub fn new(animation_id: u32, node_index: u32, start_seconds: f32) -> Self {
        Self {
            animation_id,
            node_index,
            start_seconds,
            enabled: false,
            linkage_index: None,
            key_ranges: Vec::new(),
        }
    }
}

/// Runtime sequencing data that owns its definition and has no guest-memory
/// dependency. Timing is supplied by the caller; this type does not impose a
/// render rate or couple presentation reads to clock advancement.
#[derive(Clone, Debug)]
pub struct CutsceneSequence {
    definition: CutsceneDefinition,
    frame_count: u32,
    elapsed_seconds: f32,
    frame_rate: f32,
    playback_count: usize,
    stream_mode: u32,
    character_count: u32,
    tracks: Vec<Option<PlaybackTrack>>,
}

impl CutsceneSequence {
    /// Start an empty sequence around already parsed, owned resources.
    /// `frame_count` and `frame_rate` are explicit because the source evidence
    /// does not establish one universal CSINFO/ANRS duration pairing.
    pub fn new(definition: CutsceneDefinition, frame_count: u32, frame_rate: f32) -> Self {
        Self {
            definition,
            frame_count,
            elapsed_seconds: 0.0,
            frame_rate,
            playback_count: 0,
            stream_mode: 0,
            character_count: 0,
            tracks: vec![None; PLAYBACK_SLOT_COUNT],
        }
    }

    pub fn definition(&self) -> &CutsceneDefinition {
        &self.definition
    }

    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed_seconds
    }

    pub fn playback_count(&self) -> usize {
        self.playback_count
    }

    pub fn stream_mode(&self) -> u32 {
        self.stream_mode
    }

    pub fn character_count(&self) -> u32 {
        self.character_count
    }

    pub fn track(&self, index: usize) -> Option<&PlaybackTrack> {
        self.tracks.get(index).and_then(Option::as_ref)
    }

    /// Install a typed track. Sparse indices are allowed, matching the fixed
    /// slot table; the count covers slots through the highest installed index.
    pub fn set_track(&mut self, index: usize, track: PlaybackTrack) -> Result<(), &'static str> {
        let slot = self
            .tracks
            .get_mut(index)
            .ok_or("playback slot out of range")?;
        *slot = Some(track);
        self.playback_count = self.playback_count.max(index + 1);
        Ok(())
    }

    pub fn seek(&mut self, elapsed_seconds: f32) {
        self.elapsed_seconds = elapsed_seconds;
    }

    /// Advance the in-engine clock by the caller's simulation delta.
    pub fn advance(&mut self, delta_seconds: f32) {
        self.elapsed_seconds += delta_seconds;
    }

    /// `0x98380` semantics: convert `(clock - slot start) * frame rate` by
    /// truncation toward zero, then test the ordered key windows with the
    /// original one-frame inclusive tolerance. An empty window list returns
    /// true, as the original does. Missing slots return `None`.
    pub fn key_window_active(&self, index: usize) -> Option<bool> {
        let track = self.track(index)?;
        let frame = cvttss2si((self.elapsed_seconds - track.start_seconds) * self.frame_rate);
        Some(key_window_contains(frame, &track.key_ranges))
    }

    /// Native activation combines the track's explicit enable state with the
    /// lifted key-window predicate. A missing slot is not active.
    pub fn track_active(&self, index: usize) -> Option<bool> {
        let track = self.track(index)?;
        Some(track.enabled && self.key_window_active(index).unwrap_or(false))
    }

    pub fn active_tracks(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.playback_count).filter(|index| self.track_active(*index) == Some(true))
    }

    /// Clear ephemeral playback state while retaining the owned parsed assets.
    /// The root fields correspond to `0x6EE10`'s zeroed frame/time/count/mode
    /// state and empty slot table; resource release is a separate operation.
    pub fn reset(&mut self) {
        self.frame_count = 0;
        self.elapsed_seconds = 0.0;
        self.playback_count = 0;
        self.stream_mode = 0;
        self.character_count = 0;
        self.tracks.fill(None);
    }
}

fn cvttss2si(value: f32) -> i32 {
    // x86 CVTTSS2SI returns the integer-indefinite value for NaN and overflow.
    if !value.is_finite() || value < i32::MIN as f32 || value >= 2_147_483_648.0 {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}

fn key_window_contains(frame: i32, ranges: &[KeyFrameRange]) -> bool {
    if ranges.is_empty() {
        return true;
    }
    for range in ranges {
        if frame >= range.first.wrapping_sub(1) && frame <= range.last.wrapping_add(1) {
            return true;
        }
        if frame < range.last {
            break;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition() -> CutsceneDefinition {
        let csinfo = Csinfo::parse(b"FRAMES 1").unwrap();
        let mut bytes = vec![0; 256];
        bytes[..4].copy_from_slice(b"ANRS");
        for (offset, value) in [
            (4, 200),
            (12, 1),
            (16, 1),
            (20, 1),
            (24, 1),
            (28, 200),
            (32, 208),
            (36, 212),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
        }
        let animation = Animation::parse(&bytes).unwrap();
        CutsceneDefinition::new(csinfo, animation)
    }

    #[test]
    fn owned_track_query_and_clock_updates() {
        let mut sequence = CutsceneSequence::new(definition(), 120, 30.0);
        let mut track = PlaybackTrack::new(4, 0, 1.0);
        track.enabled = true;
        track.key_ranges.push(KeyFrameRange {
            first: 29,
            last: 35,
        });
        sequence.set_track(0, track).unwrap();
        sequence.seek(2.0);
        assert_eq!(sequence.key_window_active(0), Some(true));
        assert_eq!(sequence.track_active(0), Some(true));
        sequence.advance(1.0);
        assert_eq!(sequence.elapsed_seconds(), 3.0);
        assert_eq!(sequence.key_window_active(0), Some(false));
        assert_eq!(sequence.active_tracks().count(), 0);
    }

    #[test]
    fn query_preserves_empty_and_ordered_range_rules() {
        let mut sequence = CutsceneSequence::new(definition(), 1, 1.0);
        sequence
            .set_track(0, PlaybackTrack::new(0, 0, 0.0))
            .unwrap();
        assert_eq!(sequence.key_window_active(0), Some(true));
        let mut track = PlaybackTrack::new(0, 0, 0.0);
        track.key_ranges = vec![
            KeyFrameRange { first: 2, last: 4 },
            KeyFrameRange { first: 9, last: 12 },
        ];
        sequence.set_track(0, track).unwrap();
        sequence.seek(8.0);
        assert_eq!(sequence.key_window_active(0), Some(true));
        sequence.seek(6.0);
        assert_eq!(sequence.key_window_active(0), Some(false));
    }

    #[test]
    fn reset_clears_ephemeral_state_and_keeps_assets() {
        let mut sequence = CutsceneSequence::new(definition(), 1978, 30.0);
        sequence
            .set_track(59, PlaybackTrack::new(2, 3, 0.0))
            .unwrap();
        sequence.seek(42.0);
        sequence.reset();
        assert_eq!(sequence.frame_count(), 0);
        assert_eq!(sequence.elapsed_seconds(), 0.0);
        assert_eq!(sequence.playback_count(), 0);
        assert_eq!(sequence.track(59), None);
        assert_eq!(sequence.definition().csinfo().records.len(), 1);
        assert_eq!(sequence.definition().animation().nodes.len(), 1);
        assert!(
            sequence
                .set_track(PLAYBACK_SLOT_COUNT, PlaybackTrack::new(0, 0, 0.0))
                .is_err()
        );
    }
}
