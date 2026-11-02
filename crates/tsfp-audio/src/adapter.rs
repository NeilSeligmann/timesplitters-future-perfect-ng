//! DirectSound adapter over guest memory, polled-method half: the entry points the title calls every
//! frame (stream GetStatus/Process/GetInfo/Pause, buffer Play/Stop/GetStatus and the buffer setters that
//! reach the voice, DoWork). It marshals guest arguments into [`Command`]s for a [`SoundHost`], runs the
//! [`Completion`] model and applies its [`Event`]s to guest memory. No C.
//!
//! NOT here (blocked on the memory and kernel decisions of T2044 S2/S3, see docs/t-audio-replace.md):
//! the object lifecycle (DirectSoundCreate, CreateSoundBuffer/Stream, Release, SetBufferData and the
//! startup cache setters) which allocates guest heap objects, takes the kernel critical section and reads
//! the IRQL byte. This adapter therefore receives the object state as a [`StreamView`] / [`BufferView`]
//! (what the C `dsound_stream_snapshot` / `dsound_buffer_snapshot` carry) from whoever owns the objects.
//! Refusals mirror the oracle's `refuse()`: the oracle stops the guest there, so they are `Err(Refusal)`.

use crate::adpcm;
use crate::byte_words::read_u32 as le32;
use crate::completion::{Completion, Event, PACKET_LIST_FULL, PENDING_STATUS};
use crate::contract::{Command, SoundHost};

pub trait GuestMemory {
    fn read_u32(&self, address: u32) -> Option<u32>;
    fn write_u32(&mut self, address: u32, value: u32) -> bool;
    fn read_bytes(&self, address: u32, len: usize) -> Option<Vec<u8>>;
    fn range_readable(&self, address: u32, len: u32) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal(pub &'static str);

/// The state of a stream object the adapter needs (oracle: `dsound_stream_snapshot`).
#[derive(Clone, Copy, Debug)]
pub struct StreamView {
    pub serial: u64,
    pub max_packets: u32,
    pub volume_seen: bool,
    pub discontinuity_seen: bool,
    /// Effective WAVEFORMATEX prefix (the SetFormat one when set, else the creation format).
    pub format: [u8; 20],
}

/// The state of a sound buffer object the adapter needs (oracle: `dsound_buffer_snapshot`).
#[derive(Clone, Copy, Debug)]
pub struct BufferView {
    pub serial: u64,
    pub flags: u32,
    pub cache_mask: u32,
    pub block_align: u32,
    pub frequency: u32,
    pub volume: i32,
    pub data_address: u32,
    pub data_length: u32,
    pub data_sets: u32,
    pub volume_sets: u32,
    pub pause_sets: u32,
    pub frequency_sets: u32,
    pub loop_sets: u32,
    pub loop_start: u32,
    pub loop_length: u32,
}

const BUFFER_BLOCK: u32 = 36;
const BUFFER_BLOCK_SAMPLES: u64 = 64;
const AUDIO_BUFFER_MAX_BYTES: u32 = 36 * 32768;

fn le16(b: &[u8], o: usize) -> u32 {
    u32::from(u16::from_le_bytes([b[o], b[o + 1]]))
}

/// Original ordinary mono settings subtract their +0x20 default of 600 millibels; floor is silent.
fn ordinary_volume(volume: i32) -> Option<i32> {
    if !(-10000..=0).contains(&volume) {
        return None;
    }
    Some((i64::from(volume) - 600).max(-10000) as i32)
}

pub struct DsAdapter<H: SoundHost> {
    pub completion: Completion,
    pub host: H,
}

impl<H: SoundHost> DsAdapter<H> {
    pub fn new(clock_hz: u64, host: H) -> Self {
        Self {
            completion: Completion::new(clock_hz),
            host,
        }
    }

    fn apply_events(mem: &mut dyn GuestMemory, events: &[Event]) {
        for e in events {
            if e.completed != 0 {
                mem.write_u32(e.completed, e.size);
            }
            if e.status_addr != 0 {
                mem.write_u32(e.status_addr, e.status);
            }
        }
    }
    /// `word_ok`: not null-aliasing the stream header (40 bytes), readable and writable.
    fn word_ok(mem: &mut dyn GuestMemory, address: u32, stream: u32) -> bool {
        if address == 0 {
            return true;
        }
        let end = u64::from(address) + 4;
        let overlaps = u64::from(address) < u64::from(stream) + 40 && u64::from(stream) < end;
        end <= 1 << 32
            && !overlaps
            && mem
                .read_u32(address)
                .is_some_and(|old| mem.write_u32(address, old))
    }
    fn clock_err(_: crate::completion::Error) -> Refusal {
        Refusal("source clock overflow")
    }

    /// IDirectSoundStream_Pause note (0x407B23 and relatives): mode 0 runs the stream.
    pub fn stream_pause(
        &mut self,
        mem: &mut dyn GuestMemory,
        stream: u32,
        mode: u32,
        now: u64,
    ) -> Result<(), Refusal> {
        let events = self
            .completion
            .pause_note(stream, mode, now)
            .map_err(Self::clock_err)?;
        Self::apply_events(mem, &events);
        self.host.apply(&Command::SetStreamRunning {
            key: stream,
            ticks: now,
            running: mode == 0,
        });
        Ok(())
    }

    /// IDirectSoundStream_GetStatus 0x4073D3 (stream, output). Returns the HRESULT (0).
    pub fn stream_get_status(
        &mut self,
        mem: &mut dyn GuestMemory,
        view: &StreamView,
        stream: u32,
        output: u32,
        now: u64,
    ) -> Result<u32, Refusal> {
        if !view.volume_seen {
            return Err(Refusal(
                "only a passive stream with its startup volume recorded has a status",
            ));
        }
        let out_end = u64::from(output) + 4;
        let aliases = u64::from(output) < u64::from(stream) + 40 && u64::from(stream) < out_end;
        if output == 0
            || out_end > 1 << 32
            || aliases
            || !mem
                .read_u32(output)
                .is_some_and(|old| mem.write_u32(output, old))
        {
            return Err(Refusal(
                "status output is null, aliases the stream header or is not writable",
            ));
        }
        let (value, events) = self
            .completion
            .stream_status(stream, view.serial, now, view.max_packets)
            .map_err(Self::clock_err)?;
        Self::apply_events(mem, &events);
        if !mem.write_u32(output, value) {
            return Err(Refusal("status output is not writable"));
        }
        Ok(0)
    }

    /// IDirectSoundStream_GetInfo 0x4072D4 (stream, output): {5, block, 0, 2*block}.
    pub fn stream_get_info(
        &mut self,
        mem: &mut dyn GuestMemory,
        view: &StreamView,
        stream: u32,
        output: u32,
    ) -> Result<u32, Refusal> {
        let f = &view.format;
        let block = le16(f, 12);
        let channels = le16(f, 2);
        if !((block == 72 && channels == 2) || (block == 36 && channels == 1))
            || le16(f, 0) != 0x69
            || le16(f, 14) != 4
        {
            return Err(Refusal(
                "only the measured 72 byte stereo and 36 byte mono ADPCM block streams answer GetInfo",
            ));
        }
        let out_end = u64::from(output) + 16;
        let aliases = u64::from(output) < u64::from(stream) + 40 && u64::from(stream) < out_end;
        if output == 0
            || out_end > 1 << 32
            || aliases
            || !mem
                .read_u32(output)
                .is_some_and(|old| mem.write_u32(output, old))
        {
            return Err(Refusal(
                "info output is null, aliases the stream header or is not writable",
            ));
        }
        for (i, w) in [5, block, 0, 2 * block].into_iter().enumerate() {
            if !mem.write_u32(output + 4 * i as u32, w) {
                return Err(Refusal("info output is not writable"));
            }
        }
        Ok(0)
    }

    /// IDirectSoundStream_Process 0x407424 (stream, packet, output_packet). Returns the HRESULT
    /// (0, or PACKET_LIST_FULL).
    pub fn stream_process(
        &mut self,
        mem: &mut dyn GuestMemory,
        view: &StreamView,
        stream: u32,
        packet: u32,
        output_packet: u32,
        now: u64,
    ) -> Result<u32, Refusal> {
        if !view.volume_seen || !view.discontinuity_seen {
            return Err(Refusal("only a started passive stream takes packets"));
        }
        if output_packet != 0 {
            return Err(Refusal(
                "Process output packet must be NULL (no codec output is modelled)",
            ));
        }
        let mut events = vec![];
        let running = self
            .completion
            .stream_process_begin(stream, view.serial, now, view.max_packets, &mut events)
            .map_err(Self::clock_err)?;
        Self::apply_events(mem, &events);
        let Some(running) = running else {
            return Ok(PACKET_LIST_FULL);
        };
        self.host.apply(&Command::SetStreamRunning {
            key: stream,
            ticks: now,
            running,
        });
        let mut words = [0u32; 6];
        let bytes = mem
            .read_bytes(packet, 24)
            .ok_or(Refusal("packet is unreadable"))?;
        for (i, w) in words.iter_mut().enumerate() {
            *w = le32(&bytes, i * 4);
        }
        if words[1] == 0 || words[0] == 0 || !mem.range_readable(words[0], words[1]) {
            return Err(Refusal("packet buffer is null, empty or unreadable"));
        }
        if words[4] != 0 {
            return Err(Refusal("a packet completion event is not modelled"));
        }
        if words[5] != 0 {
            return Err(Refusal("a packet timestamp is not modelled"));
        }
        let f = &view.format;
        let average = le32(f, 8);
        if !Self::word_ok(mem, words[2], stream)
            || !Self::word_ok(mem, words[3], stream)
            || (words[2] != 0 && words[2] == words[3])
        {
            return Err(Refusal(
                "packet completion words alias the stream header, each other or are not writable",
            ));
        }
        if words[2] != 0 && !mem.write_u32(words[2], 0) {
            return Err(Refusal("completion size word is not writable"));
        }
        if words[3] != 0 && !mem.write_u32(words[3], PENDING_STATUS) {
            return Err(Refusal("completion status word is not writable"));
        }
        let stereo = le16(f, 0) == 0x69
            && le16(f, 2) == 2
            && le16(f, 12) == 72
            && le16(f, 14) == 4
            && le16(f, 16) == 2
            && le16(f, 18) == 64;
        let mono = le16(f, 0) == 0x69
            && le16(f, 2) == 1
            && le16(f, 12) == 36
            && le16(f, 14) == 4
            && le16(f, 16) == 2
            && le16(f, 18) == 64;
        let rate = le32(f, 4);
        if (stereo || mono)
            && rate != 0
            && words[1] <= 4 * 1024 * 1024
            && words[1] % if mono { 36 } else { 72 } == 0
        {
            let data = mem.read_bytes(words[0], words[1] as usize).ok_or(Refusal(
                "opt-in stereo mixer could not copy the validated packet",
            ))?;
            if !self.host.apply(&Command::SubmitAdpcm {
                key: stream,
                ticks: now,
                source_hz: rate,
                mono,
                data,
            }) {
                return Err(Refusal(
                    "opt-in mixer rejected measured stereo or mono ADPCM packet or reached its bounded queue limit",
                ));
            }
        }
        if !self.completion.stream_process_push(
            stream,
            view.serial,
            now,
            words[1],
            words[2],
            words[3],
            average,
            rate,
        ) {
            return Err(Refusal("stream state changed during Process"));
        }
        Ok(0)
    }

    fn check_play_view(view: &BufferView) -> Result<(), Refusal> {
        if view.data_sets == 0
            || view.volume_sets == 0
            || view.pause_sets == 0
            || view.frequency_sets == 0
            || !(view.flags == 0 || (view.flags == 0x10 && view.cache_mask == 7))
        {
            return Err(Refusal(
                "only an ordinary buffer with its whole start (data, volume, Pause(0), frequency) recorded can Play",
            ));
        }
        Ok(())
    }

    /// IDirectSoundBuffer_Play 0x407A80 (buffer, 0, 0, flags). Returns the HRESULT (0).
    pub fn buffer_play(
        &mut self,
        mem: &mut dyn GuestMemory,
        view: &BufferView,
        buffer: u32,
        flags: u32,
        now: u64,
    ) -> Result<u32, Refusal> {
        Self::check_play_view(view)?;
        if flags > 3 {
            return Err(Refusal("only Play flags 0 to 3 are measured"));
        }
        if view.block_align != BUFFER_BLOCK
            || view.frequency == 0
            || view.data_length < BUFFER_BLOCK
        {
            return Err(Refusal(
                "only the measured 36 byte ADPCM block buffer with a frequency can Play",
            ));
        }
        if view.data_length > AUDIO_BUFFER_MAX_BYTES {
            return Err(Refusal(
                "real ordinary buffer PCM needs bounded data and volume -10000..0",
            ));
        }
        let volume = ordinary_volume(view.volume).ok_or(Refusal(
            "real ordinary buffer PCM needs bounded data and volume -10000..0",
        ))?;
        let encoded = mem
            .read_bytes(view.data_address, view.data_length as usize)
            .ok_or(Refusal(
                "real buffer PCM data is unreadable or allocation failed",
            ))?;
        let looping = flags & 1 != 0;
        if looping
            && view.loop_sets != 0
            && (view.loop_length == 0
                || view.loop_start % BUFFER_BLOCK != 0
                || view.loop_length % BUFFER_BLOCK != 0
                || u64::from(view.loop_start) + u64::from(view.loop_length)
                    > u64::from(view.data_length))
        {
            return Err(Refusal(
                "loop region must be nonempty, block-aligned and within buffer data",
            ));
        }
        let samples = u64::from(view.data_length / BUFFER_BLOCK) * BUFFER_BLOCK_SAMPLES;
        let ls = u64::from(view.loop_start / BUFFER_BLOCK) * BUFFER_BLOCK_SAMPLES;
        let le = (u64::from(view.loop_start) + u64::from(view.loop_length))
            / u64::from(BUFFER_BLOCK)
            * BUFFER_BLOCK_SAMPLES;
        let freq = u64::from(view.frequency);
        let ticks_for = |u: u64| {
            u.wrapping_mul(self.completion_clock())
                .wrapping_add(freq - 1)
                / freq
        };
        if looping && ticks_for(le) <= ticks_for(ls) {
            return Err(Refusal(
                "loop interval is below the configured virtual clock resolution",
            ));
        }
        let paused = self.completion.buffer_is_paused(buffer, view.serial);
        let pcm = adpcm::decode(&encoded, 1).ok_or(Refusal("real mono buffer decoder/mixer refused clock, data, identity, timestamp or bounded storage"))?;
        let ok = self.host.apply(&Command::PlayBuffer {
            key: buffer,
            serial: view.serial,
            ticks: now,
            frequency: view.frequency,
            pcm,
            paused,
            looping,
            loop_start: ls as usize,
            loop_end: le as usize,
            volume,
        });
        if !ok {
            return Err(Refusal(
                "real mono buffer decoder/mixer refused clock, data, identity, timestamp or bounded storage",
            ));
        }
        if !self.completion.buffer_play(
            buffer,
            view.serial,
            now,
            looping,
            samples,
            view.frequency,
            ls,
            le,
        ) {
            return Err(Refusal("no free passive buffer state"));
        }
        Ok(0)
    }

    /// IDirectSoundBuffer_Stop 0x407AA4 (buffer).
    pub fn buffer_stop(
        &mut self,
        view: &BufferView,
        buffer: u32,
        now: u64,
    ) -> Result<u32, Refusal> {
        if view.data_sets == 0
            || view.volume_sets == 0
            || view.pause_sets == 0
            || view.frequency_sets == 0
            || !(view.flags == 0 || (view.flags == 0x10 && view.cache_mask == 7))
        {
            return Err(Refusal(
                "only an ordinary buffer with its whole start (data, volume, Pause(0), frequency) recorded can Stop",
            ));
        }
        let host = &mut self.host;
        let serial = view.serial;
        if !self.completion.buffer_stop(buffer, serial, now, &mut || {
            host.apply(&Command::StopBuffer {
                key: buffer,
                serial,
                ticks: now,
            })
        }) {
            return Err(Refusal("real buffer PCM rejected Stop"));
        }
        Ok(0)
    }

    /// IDirectSoundBuffer_GetStatus 0x407AF8 (buffer, output).
    pub fn buffer_get_status(
        &mut self,
        mem: &mut dyn GuestMemory,
        view: &BufferView,
        buffer: u32,
        output: u32,
        now: u64,
    ) -> Result<u32, Refusal> {
        if output == 0
            || u64::from(output) + 4 > 1 << 32
            || !mem
                .read_u32(output)
                .is_some_and(|old| mem.write_u32(output, old))
        {
            return Err(Refusal("status output is null or not writable"));
        }
        let value = self.completion.buffer_status(buffer, view.serial, now);
        if !mem.write_u32(output, value) {
            return Err(Refusal("status output is not writable"));
        }
        Ok(0)
    }

    /// Buffer Pause(mode) of a played buffer (the completion hook of the buffer method).
    pub fn buffer_pause(&mut self, buffer: u32, serial: u64, mode: u32, now: u64) -> bool {
        let host = &mut self.host;
        self.completion
            .buffer_pause(buffer, serial, now, mode, &mut |paused| {
                host.apply(&Command::PauseBuffer {
                    key: buffer,
                    serial,
                    ticks: now,
                    paused,
                })
            })
    }
    /// Buffer SetFrequency of a played buffer.
    pub fn buffer_set_frequency(
        &mut self,
        buffer: u32,
        serial: u64,
        old_hz: u32,
        new_hz: u32,
        now: u64,
    ) -> bool {
        let host = &mut self.host;
        self.completion
            .buffer_frequency(buffer, serial, now, old_hz, new_hz, &mut || {
                host.apply(&Command::FrequencyBuffer {
                    key: buffer,
                    serial,
                    ticks: now,
                    frequency: new_hz,
                })
            })
    }
    /// Buffer SetLoopRegion (bytes of the encoded data, already validated 36 byte aligned) of a played buffer.
    pub fn buffer_set_loop(
        &mut self,
        buffer: u32,
        serial: u64,
        loop_start: u32,
        loop_length: u32,
        frequency: u32,
        now: u64,
    ) -> bool {
        if frequency == 0
            || loop_length == 0
            || loop_start % BUFFER_BLOCK != 0
            || loop_length % BUFFER_BLOCK != 0
        {
            return false;
        }
        let ls = u64::from(loop_start / BUFFER_BLOCK) * BUFFER_BLOCK_SAMPLES;
        let le = (u64::from(loop_start) + u64::from(loop_length)) / u64::from(BUFFER_BLOCK)
            * BUFFER_BLOCK_SAMPLES;
        let host = &mut self.host;
        self.completion
            .buffer_loop(buffer, serial, now, ls, le, frequency, &mut || {
                host.apply(&Command::LoopBuffer {
                    key: buffer,
                    serial,
                    ticks: now,
                    loop_start: ls as usize,
                    loop_end: le as usize,
                })
            })
    }
    /// Buffer SetVolume of a playing buffer (the completion volume hook): normalised then handed to the voice.
    pub fn buffer_set_volume(
        &mut self,
        buffer: u32,
        serial: u64,
        volume: i32,
        playing: bool,
        now: u64,
    ) -> bool {
        if !playing {
            return true;
        }
        match ordinary_volume(volume) {
            Some(v) => self.host.apply(&Command::VolumeBuffer {
                key: buffer,
                serial,
                ticks: now,
                volume: v,
            }),
            None => false,
        }
    }

    /// DirectSoundDoWork 0x407B40: advance every model, write finished packets in DoWork mode.
    pub fn do_work(&mut self, mem: &mut dyn GuestMemory, now: u64) -> Result<(), Refusal> {
        let events = self.completion.work(now).map_err(Self::clock_err)?;
        Self::apply_events(mem, &events);
        Ok(())
    }

    fn completion_clock(&self) -> u64 {
        self.completion.clock_hz()
    }
}
