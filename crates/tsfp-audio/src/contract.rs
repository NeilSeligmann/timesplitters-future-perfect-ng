//! The audio contract. The host adapter (DirectSound state machine over guest memory) turns each
//! accepted guest call into one [`Command`]; a [`SoundHost`] consumes commands and renders PCM.
//! Mirrors the oracle's dsound_audio_runtime.h / dsound_mixer.h, 1:1 per command, so a stream
//! recorded from the C host replays unchanged. Times are guest virtual ticks (`ticks_per_second`).
//! Output is interleaved stereo i16 at the sink rate. Refusals are `false` and change nothing.

/// Stream and buffer voices are keyed by the guest object address; `serial` is the lease serial
/// that tells a recycled guest address from the earlier object.
pub type Key = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Stereo (72 byte block) or mono (36 byte block) Xbox ADPCM stream packet.
    SubmitAdpcm { key: Key, ticks: u64, source_hz: u32, mono: bool, data: Vec<u8> },
    /// Interleaved stereo PCM16 packet (XMV movie soundtrack).
    SubmitPcm16Stereo { key: Key, ticks: u64, source_hz: u32, pcm: Vec<i16> },
    CutStream { key: Key, ticks: u64 },
    SetStreamRunning { key: Key, ticks: u64, running: bool },
    SetStreamFrequency { key: Key, ticks: u64, source_hz: u32 },
    SetStreamVolume { key: Key, millibels: i32 },
    ResetStreamVolume { key: Key },
    /// Ordinary mono buffer voice (Play snapshots the decoded samples).
    PlayBuffer {
        key: Key, serial: u64, ticks: u64, frequency: u32, pcm: Vec<i16>, paused: bool,
        looping: bool, loop_start: usize, loop_end: usize, volume: i32,
    },
    PauseBuffer { key: Key, serial: u64, ticks: u64, paused: bool },
    StopBuffer { key: Key, serial: u64, ticks: u64 },
    FrequencyBuffer { key: Key, serial: u64, ticks: u64, frequency: u32 },
    VolumeBuffer { key: Key, serial: u64, ticks: u64, volume: i32 },
    LoopBuffer { key: Key, serial: u64, ticks: u64, loop_start: usize, loop_end: usize },
    ResetBuffers,
    /// Routing and attenuation state of a stream (dsound_stream_routing), timestamped, lease `serial`.
    RouteStream { key: Key, serial: u64, ticks: u64, routing: Routing },
    /// Lease verified rate event of a stream (base clock rate, previous and new rate).
    FrequencyStream { key: Key, serial: u64, ticks: u64, base_hz: u32, old_hz: u32, new_hz: u32 },
    /// PCM format event: new rate and abort of the packets pending at the event.
    FormatStream { key: Key, serial: u64, ticks: u64, old_hz: u32, new_hz: u32 },
    EnableHeadroom { enabled: bool },
    BindHeadroom { identity: u64 },
    SetHeadroom { identity: u64, ticks: u64, bin: u32, headroom: u32 },
}

/// The guest-visible routing words of one stream (original settings +1C/+20/+24/+28/+30).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Routing {
    pub volume: u32,
    pub headroom: u32,
    pub aggregate: u32,
    pub count: u32,
    pub bins: [u8; 8],
    pub bin_volume: [u32; 32],
}

impl Routing {
    /// Original 40CBC0: NEG, SHL 6 (32-bit wrap), unsigned DIV 100, cap 0xFFF.
    pub fn attenuation(&self, slot: u32) -> u16 {
        if slot >= self.count || slot >= 8 || self.bins[slot as usize] >= 32 {
            return 4095;
        }
        let value = 0u32.wrapping_sub(self.aggregate.wrapping_add(self.bin_volume[self.bins[slot as usize] as usize]));
        let value = (value << 6) / 100;
        value.min(4095) as u16
    }
}

/// Host side of the audio system.
pub trait SoundHost {
    /// Apply one command. false = refused, state unchanged (the oracle's bool).
    fn apply(&mut self, command: &Command) -> bool;
    /// Interleaved stereo frames for every timeline point before `now_ticks`, at most
    /// `capacity` frames; a later call continues where this one stopped.
    fn render_until(&mut self, now_ticks: u64, capacity: usize) -> Vec<i16>;
}

/// Where rendered PCM goes (null, WAV, cpal, Bevy audio). Pulls never drive the mix.
pub trait AudioSink {
    fn write(&mut self, interleaved_stereo: &[i16]);
}

/// Discards samples, counting frames.
#[derive(Default)]
pub struct NullSink {
    pub frames: u64,
}
impl AudioSink for NullSink {
    fn write(&mut self, s: &[i16]) {
        self.frames += (s.len() / 2) as u64;
    }
}
