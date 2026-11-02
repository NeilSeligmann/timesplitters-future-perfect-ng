//! XMV movie stream model (T392/T394 extension route), port of `src/audio/dsound_movie_stream.c`.
//! Tier 1 vs the C source (unit oracles below restate the C's measured oracle cases); the Rust-host
//! path also uses the typed queue and admission seam while the C-host facade remains the oracle.
//!
//! What it owns: per stream the 10-word header sidecar (reference word 2), the two-packet queue with its
//! virtual-clock deadline chain, pause/resume, FlushEx(0,0,1) and Flush, the final-Release abort and the
//! status word. What stays with the caller: the device lease and private heap (lifecycle.rs), the policy
//! preconditions (IRQL, global state), the mixer/runtime side effects and running the guest callback.
//! Completions are returned as data ([`Completion`]) in the C's delivery order instead of calling back.

use crate::adapter::{GuestMemory, Refusal};

pub const STREAM_BYTES: u32 = 40;
pub const MAX_PACKETS: usize = 2;
pub const CALLBACK: u32 = 0x0044_5071;
pub const ABORT_STATUS: u32 = 0x8000_4004;
pub const PENDING_STATUS: u32 = 0x8000_000A;
pub const TOO_MANY_PACKETS: u32 = 0x8878_0032;
pub const E_FAIL: u32 = 0x8000_4005;
pub const VTABLE: u32 = 0x004A_1D00;
pub const SECONDARY: u32 = 0x004A_1CF0;
pub const SINGLETON: u32 = 0x0041_2B30;
pub const GLOBAL_STATE: u32 = 0x0041_24A8;
const FORMAT_AVERAGE: u64 = 176_400;
const FORMAT_RATE: u32 = 44_100;
const FORMAT_BLOCK: u32 = 4;
const MAX_PACKET_BYTES: u32 = 0x0100_0000;
const NEVER: u64 = u64::MAX;
const XMV: (u32, u32) = (0x0044_4920, 0x0044_CE49);
const TITLE: (u32, u32) = (0x0003_0280, 0x0003_0676);
pub const CREATE_RETURN: u32 = 0x0044_51F4;
/// The measured XMV return address for the movie-owned DirectSound device Release entry.
pub const DEVICE_RELEASE_CALLER: u32 = 0x0044_5726;

/// Failure reasons for the host-supplied conditions that gate the movie stream facade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoviePolicyError {
    Disabled,
    IrqlUnknownOrNonzero,
    GlobalStateUnavailableOrNonzero,
}

/// Apply the original host facade's policy order while keeping reads behind typed host callbacks.
pub fn check_policy<I, G>(
    enabled: bool,
    mut current_irql: I,
    mut global_state: G,
) -> Result<(), MoviePolicyError>
where
    I: FnMut() -> Option<u8>,
    G: FnMut() -> Option<u32>,
{
    if !enabled {
        return Err(MoviePolicyError::Disabled);
    }
    if current_irql().is_none_or(|irql| irql != 0) {
        return Err(MoviePolicyError::IrqlUnknownOrNonzero);
    }
    if global_state() != Some(0) {
        return Err(MoviePolicyError::GlobalStateUnavailableOrNonzero);
    }
    Ok(())
}

/// Result for admission of a packet into the live Rust-host movie queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketAdmission {
    StreamUnavailable,
    QueueFull,
    InvalidSize,
    CompletionWordsOverlap,
    CompletionWordsAliasQueuedPacket,
}

/// Return addresses the movie methods are admitted from (retained XMV section or the title movie loop).
pub fn caller_ok(address: u32) -> bool {
    (XMV.0..=XMV.1).contains(&address) || (TITLE.0..TITLE.1).contains(&address)
}

/// The one measured guest return address that may create the XMV stream.
pub fn create_caller_ok(address: u32) -> bool {
    address == CREATE_RETURN
}

/// Admit only the measured XMV caller of the movie-owned device Release HLE entry.
pub fn device_release_caller_ok(address: u32) -> bool {
    address == DEVICE_RELEASE_CALLER
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Packet {
    buffer: u32,
    size: u32,
    completed: u32,
    status: u32,
    context: u32,
    ticks: u64,
    deadline: u64,
    remaining: u64,
}

/// One packet leaving the queue: the caller writes nothing, [`MovieStreams`] already wrote the completion
/// words; it runs `callback(stream_context, packet_context, status)` on the guest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    pub stream: u32,
    pub callback: u32,
    pub stream_context: u32,
    pub packet_context: u32,
    pub size: u32,
    pub status: u32,
}

/// Completion data returned to the live C callback runner after Rust retires the scheduled packet.
/// The guest word addresses stay attached to the packet so the runner can publish size, then status,
/// then invoke the callback without duplicating the queue state in C.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledCompletion {
    pub event: Completion,
    pub completed_address: u32,
    pub status_address: u32,
}

/// The validated DSSTREAMDESC of the one measured EnableAudioStream call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    pub descriptor: u32,
    pub format: u32,
    pub callback: u32,
    pub context: u32,
}

fn le16(b: &[u8], at: usize) -> u32 {
    u32::from(u16::from_le_bytes([b[at], b[at + 1]]))
}
fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
use crate::guest_ranges::overlaps as overlap;
/// Completion dwords are never aligned by the original (T394), only overlap is refused.
fn words_overlap(a: u32, b: u32) -> bool {
    a.abs_diff(b) < 4
}

/// `scope_snapshot`: flags 0, two packets, callback 0x445071, nonzero context, no mixbins, PCM stereo
/// 44100 Hz 16-bit, block 4, average 176400, cbSize 0. Anything else is outside the measured scope.
pub fn scope_snapshot<M: GuestMemory + ?Sized>(mem: &M, descriptor: u32) -> Option<Scope> {
    let desc = mem.read_bytes(descriptor, 24)?;
    let format = le32(&desc, 8);
    if format == 0 || u64::from(format) + 18 > 1 << 32 || overlap(descriptor, 24, format, 18) {
        return None;
    }
    let wave = mem.read_bytes(format, 18)?;
    let desc_ok = le32(&desc, 0) == 0
        && le32(&desc, 4) == MAX_PACKETS as u32
        && le32(&desc, 12) == CALLBACK
        && le32(&desc, 16) != 0
        && le32(&desc, 20) == 0;
    let wave_ok = le16(&wave, 0) == 1
        && le16(&wave, 2) == 2
        && le32(&wave, 4) == FORMAT_RATE
        && u64::from(le32(&wave, 8)) == FORMAT_AVERAGE
        && le16(&wave, 12) == FORMAT_BLOCK
        && le16(&wave, 14) == 16
        && le16(&wave, 16) == 0;
    (desc_ok && wave_ok).then_some(Scope {
        descriptor,
        format,
        callback: le32(&desc, 12),
        context: le32(&desc, 16),
    })
}

#[derive(Clone, Debug)]
struct Node {
    address: u32,
    publication_address: Option<u32>,
    header: [u32; 10],
    callback: u32,
    context: u32,
    pause_mode: u32,
    volume: i32,
    volume_seen: bool,
    discontinuity_seen: bool,
    flush_pending: bool,
    paused: bool,
    flush_tick: u64,
    queue: Vec<Packet>,
}

impl Node {
    fn status_word(&self) -> u32 {
        let mut value = u32::from(self.queue.len() < MAX_PACKETS);
        if !self.queue.is_empty() {
            value |= if self.paused { 0x20000 } else { 0x10000 };
        }
        value
    }
    fn pause(&mut self, mode: u32, now: u64) {
        if !self.paused {
            if let Some(head) = self.queue.first_mut() {
                head.remaining = head.deadline.saturating_sub(now);
                head.deadline = NEVER;
            }
        }
        self.paused = true;
        self.pause_mode = mode;
    }
    fn resume(&mut self, now: u64) {
        if self.paused {
            if let Some(head) = self.queue.first_mut() {
                head.deadline = now + head.remaining;
            }
        }
        self.paused = false;
        self.pause_mode = 0;
    }
    fn due(&self, now: u64) -> bool {
        !self.queue.is_empty()
            && (self.flush_pending || (!self.paused && self.queue[0].deadline <= now))
    }
    /// `take_head`: a flush completes a packet normally only when its deadline passed before the flush;
    /// the next packet starts where this one ended (or at `now` when this one was aborted).
    fn take_head(&mut self, now: u64) -> (Packet, u32) {
        let done = self.queue.remove(0);
        let played =
            done.deadline != NEVER && (!self.flush_pending || done.deadline <= self.flush_tick);
        let status = if played { 0 } else { ABORT_STATUS };
        if let Some(next) = self.queue.first_mut() {
            next.deadline = if self.paused {
                NEVER
            } else {
                (if played { done.deadline } else { now }) + next.ticks
            };
        } else {
            self.flush_pending = false;
        }
        (done, status)
    }
}

/// Snapshot matching `dsound_movie_stream_snapshot` (the compare twin's view).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub refs: u32,
    pub callback: u32,
    pub context: u32,
    pub volume: i32,
    pub volume_seen: bool,
    pub discontinuity_seen: bool,
    pub flush_pending: bool,
    pub pause_mode: u32,
    pub queued: u32,
    pub packet_size: [u32; MAX_PACKETS],
    pub head_deadline: u64,
}

/// Result of Release: the reference count, or the final release's aborted packets (the caller then
/// returns the lease and destroys the private heap, as `destroy_node` does).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Released {
    Count(u32),
    Destroyed(Vec<Completion>),
}

pub struct MovieStreams {
    /// Ticks per second of the virtual clock (`KERNEL_CLOCK_FREQUENCY_HZ` by default in the C).
    pub clock_frequency: u64,
    nodes: Vec<Node>,
    pub completions: u64,
}

impl MovieStreams {
    pub fn new(clock_frequency: u64) -> Self {
        Self {
            clock_frequency,
            nodes: Vec::new(),
            completions: 0,
        }
    }
    pub fn owns(&self, stream: u32) -> bool {
        self.nodes.iter().any(|n| n.address == stream)
    }
    pub fn count(&self) -> usize {
        self.nodes.len()
    }
    /// Return the first registered stream in the same order used by the host facade.
    /// Reset uses this to sequence one synchronous lease/heap retirement at a time.
    pub fn first_stream(&self) -> Option<u32> {
        self.nodes.first().map(|node| node.address)
    }
    fn node(&mut self, stream: u32) -> Result<&mut Node, Refusal> {
        self.nodes
            .iter_mut()
            .find(|n| n.address == stream)
            .ok_or(Refusal("stream is not owned by the movie model"))
    }
    fn ticks_for(&self, size: u32) -> u64 {
        (u64::from(size) * self.clock_frequency).div_ceil(FORMAT_AVERAGE)
    }
    /// True when `address..+bytes` hits a movie stream object (part of the C `aliases_state`; the device
    /// internal, singleton, global state and secondary checks belong to the caller).
    pub fn aliases(&self, address: u32, bytes: u32) -> bool {
        self.nodes
            .iter()
            .any(|n| overlap(address, bytes, n.address, STREAM_BYTES))
    }

    /// Checks an XMV completion dword against the state protected by the live C facade.
    /// The Rust-host facade supplies the current device's internal block; the scheduler owns
    /// its registered stream headers and their publication slots.
    pub fn completion_word_aliases_protected_state(&self, address: u32, internal: u32) -> bool {
        const WORD_BYTES: u32 = 4;
        const INTERNAL_BYTES: u32 = 44;
        address == 0
            || u64::from(address) + u64::from(WORD_BYTES) > (1u64 << 32)
            || overlap(address, WORD_BYTES, internal, INTERNAL_BYTES)
            || overlap(address, WORD_BYTES, SINGLETON, WORD_BYTES)
            || overlap(address, WORD_BYTES, GLOBAL_STATE, WORD_BYTES)
            || overlap(address, WORD_BYTES, SECONDARY, 60)
            || self.nodes.iter().any(|node| {
                overlap(address, WORD_BYTES, node.address, STREAM_BYTES)
                    || node
                        .publication_address
                        .is_some_and(|slot| overlap(address, WORD_BYTES, slot, WORD_BYTES))
            })
    }

    /// Registers a stream the caller allocated at `address` (40 bytes, private heap) for `internal`,
    /// writes the header and returns the C's success (0). The output pointer is published by the caller.
    pub fn attach<M: GuestMemory + ?Sized>(
        &mut self,
        mem: &mut M,
        address: u32,
        internal: u32,
        scope: &Scope,
    ) -> Result<u32, Refusal> {
        let mut header = [0u32; 10];
        header[..4].copy_from_slice(&[VTABLE, SECONDARY, 1, internal]);
        for (i, word) in header.iter().enumerate() {
            if !mem.write_u32(address + 4 * i as u32, *word) {
                return Err(Refusal("new private stream allocation/alias is invalid"));
            }
        }
        self.nodes.push(Node {
            address,
            publication_address: None,
            header,
            callback: scope.callback,
            context: scope.context,
            pause_mode: 0,
            volume: 0,
            volume_seen: false,
            discontinuity_seen: false,
            flush_pending: false,
            paused: false,
            flush_tick: 0,
            queue: Vec::with_capacity(MAX_PACKETS),
        });
        Ok(0)
    }

    /// Registers only the queue sidecar for a stream whose guest header and lease are owned by the
    /// host facade. This is the replace seam used by the Rust audio host; the facade still creates
    /// and validates the DirectSound object and runs guest callbacks.
    pub fn attach_scheduler(
        &mut self,
        address: u32,
        internal: u32,
        publication_address: u32,
        callback: u32,
        context: u32,
    ) -> Result<(), Refusal> {
        if address == 0 || self.owns(address) {
            return Err(Refusal(
                "movie scheduler stream is null or already registered",
            ));
        }
        let mut header = [0u32; 10];
        header[..4].copy_from_slice(&[VTABLE, SECONDARY, 1, internal]);
        self.nodes.push(Node {
            address,
            publication_address: Some(publication_address),
            header,
            callback,
            context,
            pause_mode: 0,
            volume: 0,
            volume_seen: false,
            discontinuity_seen: false,
            flush_pending: false,
            paused: false,
            flush_tick: 0,
            queue: Vec::with_capacity(MAX_PACKETS),
        });
        Ok(())
    }

    /// Removes an empty host-facade queue sidecar after its final completion callbacks ran.
    pub fn detach_scheduler(&mut self, stream: u32) -> bool {
        let before = self.nodes.len();
        self.nodes.retain(|node| node.address != stream);
        self.nodes.len() + 1 == before
    }

    pub fn scheduled_count(&self, stream: u32) -> Option<usize> {
        self.nodes
            .iter()
            .find(|node| node.address == stream)
            .map(|node| node.queue.len())
    }

    /// Reports whether the registered stream has room for another packet.
    pub fn scheduled_has_capacity(&self, stream: u32) -> Option<bool> {
        self.nodes
            .iter()
            .find(|node| node.address == stream)
            .map(|node| node.queue.len() < MAX_PACKETS)
    }

    /// Validates packet admission policy without changing queue state. Guest memory range checks
    /// and completion-word publication remain at the host boundary.
    pub fn scheduled_packet_admission(
        &self,
        stream: u32,
        size: u32,
        completed: u32,
        status: u32,
    ) -> Result<(), PacketAdmission> {
        let node = self
            .nodes
            .iter()
            .find(|node| node.address == stream)
            .ok_or(PacketAdmission::StreamUnavailable)?;
        if size < FORMAT_BLOCK || size % FORMAT_BLOCK != 0 || size > MAX_PACKET_BYTES {
            return Err(PacketAdmission::InvalidSize);
        }
        if words_overlap(completed, status) {
            return Err(PacketAdmission::CompletionWordsOverlap);
        }
        if node.queue.len() >= MAX_PACKETS {
            return Err(PacketAdmission::QueueFull);
        }
        if node.queue.iter().any(|packet| {
            words_overlap(packet.completed, completed)
                || words_overlap(packet.status, status)
                || words_overlap(packet.completed, status)
                || words_overlap(packet.status, completed)
        }) {
            return Err(PacketAdmission::CompletionWordsAliasQueuedPacket);
        }
        Ok(())
    }

    pub fn scheduled_words_alias(&self, stream: u32, completed: u32, status: u32) -> Option<bool> {
        let node = self.nodes.iter().find(|node| node.address == stream)?;
        Some(node.queue.iter().any(|packet| {
            words_overlap(packet.completed, completed)
                || words_overlap(packet.status, status)
                || words_overlap(packet.completed, status)
                || words_overlap(packet.status, completed)
        }))
    }

    /// Adds an already admitted XMV packet. Guest payload and completion-word probing/publication
    /// remain at the host seam; Rust owns packet order and virtual-clock deadlines.
    pub fn schedule_packet(
        &mut self,
        stream: u32,
        size: u32,
        completed: u32,
        status: u32,
        context: u32,
        now: u64,
    ) -> Result<(), Refusal> {
        self.scheduled_packet_admission(stream, size, completed, status)
            .map_err(|reason| match reason {
                PacketAdmission::StreamUnavailable => {
                    Refusal("stream is not owned by the movie model")
                }
                PacketAdmission::QueueFull => Refusal("movie packet queue is full"),
                PacketAdmission::InvalidSize => {
                    Refusal("movie packet size is outside the measured scope")
                }
                PacketAdmission::CompletionWordsOverlap => {
                    Refusal("packet completion words overlap")
                }
                PacketAdmission::CompletionWordsAliasQueuedPacket => {
                    Refusal("completion words alias an attached packet")
                }
            })?;
        let ticks = self.ticks_for(size);
        let node = self.node(stream)?;
        let packet = Packet {
            size,
            completed,
            status,
            context,
            ticks,
            remaining: ticks,
            deadline: if node.queue.is_empty() && !node.paused {
                now + ticks
            } else {
                NEVER
            },
            ..Packet::default()
        };
        node.queue.push(packet);
        Ok(())
    }

    /// Retires one due packet (or the first packet when `everything` is set) in node/FIFO order.
    /// The caller performs guest writes and callback execution before asking for the next event.
    pub fn take_scheduled(
        &mut self,
        now: u64,
        everything: bool,
        only: u32,
    ) -> Option<ScheduledCompletion> {
        let node = self.nodes.iter_mut().find(|node| {
            (only == 0 || node.address == only)
                && !node.queue.is_empty()
                && (everything || node.due(now))
        })?;
        if everything {
            node.flush_pending = true;
            node.flush_tick = 0;
        }
        let (packet, status) = node.take_head(now);
        self.completions += 1;
        Some(ScheduledCompletion {
            event: Completion {
                stream: node.address,
                callback: node.callback,
                stream_context: node.context,
                packet_context: packet.context,
                size: packet.size,
                status,
            },
            completed_address: packet.completed,
            status_address: packet.status,
        })
    }

    /// `valid_node` header half: the guest header must still equal the sidecar.
    pub fn header_intact<M: GuestMemory + ?Sized>(&self, mem: &M, stream: u32) -> bool {
        let Some(n) = self.nodes.iter().find(|n| n.address == stream) else {
            return false;
        };
        mem.read_bytes(stream, 40)
            .is_some_and(|b| (0..10).all(|i| le32(&b, 4 * i) == n.header[i]))
    }

    pub fn add_ref<M: GuestMemory + ?Sized>(
        &mut self,
        mem: &mut M,
        stream: u32,
    ) -> Result<u32, Refusal> {
        let n = self.node(stream)?;
        if n.header[2] == u32::MAX {
            return Err(Refusal("reference count overflow"));
        }
        if !mem.write_u32(stream + 8, n.header[2] + 1) {
            return Err(Refusal("reference word is not writable"));
        }
        n.header[2] += 1;
        Ok(n.header[2])
    }

    pub fn release<M: GuestMemory + ?Sized>(
        &mut self,
        mem: &mut M,
        stream: u32,
        now: u64,
    ) -> Result<Released, Refusal> {
        let n = self.node(stream)?;
        match n.header[2] {
            0 => Err(Refusal("reference count underflow")),
            1 => {
                let done = self.drain(mem, now, true, stream)?;
                self.nodes.retain(|n| n.address != stream);
                Ok(Released::Destroyed(done))
            }
            refs => {
                if !mem.write_u32(stream + 8, refs - 1) {
                    return Err(Refusal("reference word is not writable"));
                }
                n.header[2] = refs - 1;
                Ok(Released::Count(refs - 1))
            }
        }
    }

    /// Host request cache only (the original validates nothing, every int32 returns 0). The mixer gain
    /// range check of the runtime path is the caller's.
    pub fn set_volume(&mut self, stream: u32, volume: i32) -> Result<u32, Refusal> {
        let n = self.node(stream)?;
        n.volume = volume;
        n.volume_seen = true;
        Ok(0)
    }

    pub fn pause(&mut self, stream: u32, mode: u32, now: u64) -> Result<u32, Refusal> {
        let n = self.node(stream)?;
        match mode {
            4.. => Ok(E_FAIL),
            3 => Err(Refusal("Pause mode 3 has no measured XMV use")),
            0 => {
                n.resume(now);
                Ok(0)
            }
            _ => {
                n.pause(mode, now);
                Ok(0)
            }
        }
    }

    pub fn flush_ex(
        &mut self,
        stream: u32,
        time_low: u32,
        time_high: u32,
        flags: u32,
        now: u64,
    ) -> Result<u32, Refusal> {
        let n = self.node(stream)?;
        if (time_low, time_high, flags) != (0, 0, 1) {
            return Err(Refusal(
                "only the measured FlushEx(time 0, flags 1) is supported",
            ));
        }
        if n.queue.is_empty() {
            return Ok(0);
        }
        if n.paused {
            n.resume(now);
        }
        n.flush_pending = true;
        n.flush_tick = now;
        Ok(0)
    }

    /// Synchronous Flush: resume, then abort every attached packet in FIFO order (as the final Release).
    pub fn flush<M: GuestMemory + ?Sized>(
        &mut self,
        mem: &mut M,
        stream: u32,
        now: u64,
    ) -> Result<Vec<Completion>, Refusal> {
        self.node(stream)?.resume(now);
        self.drain(mem, now, true, stream)
    }

    pub fn discontinuity(&mut self, stream: u32) -> Result<u32, Refusal> {
        self.node(stream)?.discontinuity_seen = true;
        Ok(0)
    }

    /// Writes the status word to `output` (the caller checks the protected-state aliases).
    pub fn get_status<M: GuestMemory + ?Sized>(
        &mut self,
        mem: &mut M,
        stream: u32,
        output: u32,
    ) -> Result<u32, Refusal> {
        let word = self.node(stream)?.status_word();
        if output == 0
            || u64::from(output) + 4 > 1 << 32
            || self.aliases(output, 4)
            || !mem.write_u32(output, word)
        {
            return Err(Refusal(
                "status output is NULL, inaccessible or aliases protected state",
            ));
        }
        Ok(0)
    }

    /// Returns the status word for a live stream without publishing it to guest memory.
    pub fn status_word(&self, stream: u32) -> Result<u32, Refusal> {
        self.nodes
            .iter()
            .find(|node| node.address == stream)
            .map(|node| node.status_word())
            .ok_or(Refusal("stream is not owned by the movie model"))
    }

    fn word_ok<M: GuestMemory + ?Sized>(&self, mem: &M, address: u32) -> bool {
        address != 0
            && u64::from(address) + 4 <= 1 << 32
            && !self.aliases(address, 4)
            && mem.read_u32(address).is_some()
    }

    /// Process: attaches one PCM packet (`packet` points at the 6-word XMEDIAPACKET). Returns
    /// `TOO_MANY_PACKETS` with two attached, before reading the packet (oracle).
    pub fn process<M: GuestMemory + ?Sized>(
        &mut self,
        mem: &mut M,
        stream: u32,
        packet: u32,
        output_packet: u32,
        now: u64,
    ) -> Result<u32, Refusal> {
        if output_packet != 0 {
            return Err(Refusal(
                "Process output packet must be NULL as the retained XMV passes it",
            ));
        }
        if self.node(stream)?.queue.len() >= MAX_PACKETS {
            return Ok(TOO_MANY_PACKETS);
        }
        let words = mem
            .read_bytes(packet, 24)
            .ok_or(Refusal("packet is unreadable"))?;
        let mut p = Packet {
            buffer: le32(&words, 0),
            size: le32(&words, 4),
            completed: le32(&words, 8),
            status: le32(&words, 12),
            context: le32(&words, 16),
            ..Packet::default()
        };
        if p.buffer == 0
            || p.size < FORMAT_BLOCK
            || p.size % FORMAT_BLOCK != 0
            || p.size > MAX_PACKET_BYTES
            || !mem.range_readable(p.buffer, p.size)
            || words_overlap(p.completed, p.status)
            || !self.word_ok(mem, p.completed)
            || !self.word_ok(mem, p.status)
        {
            return Err(Refusal(
                "packet buffer, size or completion words are outside the measured scope",
            ));
        }
        let ticks = self.ticks_for(p.size);
        let n = self.node(stream)?;
        if n.queue.iter().any(|q| {
            words_overlap(q.completed, p.completed)
                || words_overlap(q.status, p.status)
                || words_overlap(q.completed, p.status)
                || words_overlap(q.status, p.completed)
        }) {
            return Err(Refusal("completion words alias an attached packet"));
        }
        if !mem.write_u32(p.completed, 0) || !mem.write_u32(p.status, PENDING_STATUS) {
            return Err(Refusal("completion words are not writable"));
        }
        p.ticks = ticks;
        p.remaining = ticks;
        p.deadline = if n.queue.is_empty() && !n.paused {
            now + ticks
        } else {
            NEVER
        };
        n.queue.push(p);
        Ok(0)
    }

    /// DirectSoundDoWork: completes due packets, node order then queue order, until none is.
    pub fn do_work<M: GuestMemory + ?Sized>(
        &mut self,
        mem: &mut M,
        now: u64,
    ) -> Result<Vec<Completion>, Refusal> {
        self.drain(mem, now, false, 0)
    }

    /// SynchPlayback resumes every stream paused with mode 2.
    pub fn synch_playback(&mut self, now: u64) {
        for n in self
            .nodes
            .iter_mut()
            .filter(|n| n.paused && n.pause_mode == 2)
        {
            n.resume(now);
        }
    }

    fn drain<M: GuestMemory + ?Sized>(
        &mut self,
        mem: &mut M,
        now: u64,
        everything: bool,
        only: u32,
    ) -> Result<Vec<Completion>, Refusal> {
        let mut out = Vec::new();
        while let Some(n) = self.nodes.iter_mut().find(|n| {
            (only == 0 || n.address == only) && !n.queue.is_empty() && (everything || n.due(now))
        }) {
            if everything {
                n.flush_pending = true;
                n.flush_tick = 0;
            }
            let (done, status) = n.take_head(now);
            if !mem.write_u32(done.completed, done.size) || !mem.write_u32(done.status, status) {
                return Err(Refusal("packet completion words are no longer writable"));
            }
            out.push(Completion {
                stream: n.address,
                callback: n.callback,
                stream_context: n.context,
                packet_context: done.context,
                size: done.size,
                status,
            });
            self.completions += 1;
        }
        Ok(out)
    }

    pub fn snapshot(&self, stream: u32) -> Option<Snapshot> {
        let n = self.nodes.iter().find(|n| n.address == stream)?;
        let mut packet_size = [0; MAX_PACKETS];
        for (slot, p) in packet_size.iter_mut().zip(&n.queue) {
            *slot = p.size;
        }
        Some(Snapshot {
            refs: n.header[2],
            callback: n.callback,
            context: n.context,
            volume: n.volume,
            volume_seen: n.volume_seen,
            discontinuity_seen: n.discontinuity_seen,
            flush_pending: n.flush_pending,
            pause_mode: if n.paused { n.pause_mode } else { 0 },
            queued: n.queue.len() as u32,
            packet_size,
            head_deadline: n.queue.first().map_or(0, |p| p.deadline),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<u32, u8>);
    impl GuestMemory for Mem {
        fn read_u32(&self, a: u32) -> Option<u32> {
            self.read_bytes(a, 4).map(|b| le32(&b, 0))
        }
        fn write_u32(&mut self, a: u32, v: u32) -> bool {
            for (i, b) in v.to_le_bytes().into_iter().enumerate() {
                self.0.insert(a + i as u32, b);
            }
            true
        }
        fn read_bytes(&self, a: u32, len: usize) -> Option<Vec<u8>> {
            (0..len as u32)
                .map(|i| self.0.get(&(a + i)).copied())
                .collect()
        }
        fn range_readable(&self, a: u32, len: u32) -> bool {
            (0..len).all(|i| self.0.contains_key(&(a + i)))
        }
    }

    const S: u32 = 0x10_0000;
    const HZ: u64 = 176_400; // one tick per byte keeps the arithmetic readable

    #[test]
    fn reset_selection_follows_registered_stream_order() {
        let mut scheduler = MovieStreams::new(HZ);
        assert_eq!(scheduler.first_stream(), None);
        scheduler
            .attach_scheduler(S, 0x2000, 0x7000, 0x44, 0x55)
            .unwrap();
        scheduler
            .attach_scheduler(S + 0x100, 0x2000, 0x7100, 0x66, 0x77)
            .unwrap();
        assert_eq!(scheduler.first_stream(), Some(S));
        assert!(scheduler.detach_scheduler(S));
        assert_eq!(scheduler.first_stream(), Some(S + 0x100));
        assert!(scheduler.detach_scheduler(S + 0x100));
        assert_eq!(scheduler.first_stream(), None);
    }

    #[test]
    fn movie_policy_short_circuits_in_host_contract_order() {
        let reads = std::cell::RefCell::new(Vec::new());
        assert_eq!(
            check_policy(
                false,
                || {
                    reads.borrow_mut().push("irql");
                    Some(0)
                },
                || {
                    reads.borrow_mut().push("global");
                    Some(0)
                }
            ),
            Err(MoviePolicyError::Disabled)
        );
        assert!(reads.borrow().is_empty());

        assert_eq!(
            check_policy(
                true,
                || {
                    reads.borrow_mut().push("irql");
                    None
                },
                || {
                    reads.borrow_mut().push("global");
                    Some(0)
                }
            ),
            Err(MoviePolicyError::IrqlUnknownOrNonzero)
        );
        assert_eq!(*reads.borrow(), ["irql"]);

        reads.borrow_mut().clear();
        assert_eq!(
            check_policy(
                true,
                || {
                    reads.borrow_mut().push("irql");
                    Some(1)
                },
                || {
                    reads.borrow_mut().push("global");
                    Some(0)
                }
            ),
            Err(MoviePolicyError::IrqlUnknownOrNonzero)
        );
        assert_eq!(*reads.borrow(), ["irql"]);

        reads.borrow_mut().clear();
        assert_eq!(
            check_policy(
                true,
                || {
                    reads.borrow_mut().push("irql");
                    Some(0)
                },
                || {
                    reads.borrow_mut().push("global");
                    None
                }
            ),
            Err(MoviePolicyError::GlobalStateUnavailableOrNonzero)
        );
        assert_eq!(*reads.borrow(), ["irql", "global"]);

        assert_eq!(check_policy(true, || Some(0), || Some(0)), Ok(()));
    }

    #[test]
    fn caller_admission_preserves_xmv_title_and_create_boundaries() {
        assert!(caller_ok(0x0044_4920));
        assert!(caller_ok(0x0044_CE49));
        assert!(caller_ok(0x0003_0280));
        assert!(caller_ok(0x0003_0675));
        assert!(!caller_ok(0x0044_491F));
        assert!(!caller_ok(0x0044_CE4A));
        assert!(!caller_ok(0x0003_027F));
        assert!(!caller_ok(0x0003_0676));
        assert!(create_caller_ok(CREATE_RETURN));
        assert!(!create_caller_ok(CREATE_RETURN - 1));
        assert!(!create_caller_ok(CREATE_RETURN + 1));
    }

    #[test]
    fn device_release_admission_matches_only_the_measured_xmv_return_address() {
        assert!(device_release_caller_ok(DEVICE_RELEASE_CALLER));
        assert!(!device_release_caller_ok(DEVICE_RELEASE_CALLER - 1));
        assert!(!device_release_caller_ok(DEVICE_RELEASE_CALLER + 1));
        assert!(!device_release_caller_ok(CREATE_RETURN));
    }

    fn setup() -> (MovieStreams, Mem) {
        let mut m = Mem::default();
        for (i, w) in [0u32, 2, 0x2000, CALLBACK, 0xDEC0, 0]
            .into_iter()
            .enumerate()
        {
            m.write_u32(0x1000 + 4 * i as u32, w);
        }
        for (i, b) in [1u8, 0, 2, 0].into_iter().enumerate() {
            m.0.insert(0x2000 + i as u32, b);
        }
        m.write_u32(0x2004, 44_100);
        m.write_u32(0x2008, 176_400);
        m.write_u32(0x200C, 4 | 16 << 16);
        m.0.insert(0x2010, 0);
        m.0.insert(0x2011, 0);
        let scope = scope_snapshot(&m, 0x1000).expect("measured scope");
        let mut ms = MovieStreams::new(HZ);
        assert_eq!(ms.attach(&mut m, S, 0x9000, &scope), Ok(0));
        (ms, m)
    }

    /// Packet descriptor at `at`: buffer, size, completed/status words at at+0x100/+0x105 (odd, as XMV).
    fn packet(m: &mut Mem, at: u32, size: u32, context: u32) {
        for (i, w) in [at + 0x1000, size, at + 0x100, at + 0x105, context, 0]
            .into_iter()
            .enumerate()
        {
            m.write_u32(at + 4 * i as u32, w);
        }
        m.write_u32(at + 0x100, 0xFFFF_FFFF);
        m.write_u32(at + 0x105, 0xFFFF_FFFF);
        for i in 0..size {
            m.0.insert(at + 0x1000 + i, 0);
        }
    }

    #[test]
    fn scope_refuses_other_formats() {
        let (_, mut m) = setup();
        m.write_u32(0x2004, 48_000);
        assert_eq!(scope_snapshot(&m, 0x1000), None);
    }

    #[test]
    fn header_and_refcount() {
        let (mut ms, mut m) = setup();
        assert_eq!(m.read_u32(S), Some(VTABLE));
        assert_eq!(m.read_u32(S + 12), Some(0x9000));
        assert!(ms.header_intact(&m, S));
        assert_eq!(ms.add_ref(&mut m, S), Ok(2));
        assert_eq!(m.read_u32(S + 8), Some(2));
        assert_eq!(ms.release(&mut m, S, 0), Ok(Released::Count(1)));
        assert_eq!(ms.release(&mut m, S, 0), Ok(Released::Destroyed(vec![])));
        assert!(!ms.owns(S));
    }

    #[test]
    fn scheduler_admission_owns_capacity_size_and_completion_alias_rules() {
        let (mut streams, _) = setup();
        assert_eq!(streams.scheduled_has_capacity(S), Some(true));
        assert_eq!(
            streams.scheduled_packet_admission(S, 3, 0x3000, 0x3004),
            Err(PacketAdmission::InvalidSize)
        );
        assert_eq!(
            streams.scheduled_packet_admission(S, 4, 0x3000, 0x3002),
            Err(PacketAdmission::CompletionWordsOverlap)
        );
        assert_eq!(
            streams.scheduled_packet_admission(S, 4, 0x3000, 0x3004),
            Ok(())
        );
        streams
            .schedule_packet(S, 4, 0x3000, 0x3004, 0xA, 100)
            .unwrap();
        assert_eq!(
            streams.scheduled_packet_admission(S, 4, 0x3004, 0x3008),
            Err(PacketAdmission::CompletionWordsAliasQueuedPacket)
        );
        streams
            .schedule_packet(S, 4, 0x3100, 0x3104, 0xB, 100)
            .unwrap();
        assert_eq!(streams.scheduled_has_capacity(S), Some(false));
        assert_eq!(
            streams.scheduled_packet_admission(S, 4, 0x3200, 0x3204),
            Err(PacketAdmission::QueueFull)
        );
    }

    #[test]
    fn queue_deadlines_status_and_completion() {
        let (mut ms, mut m) = setup();
        packet(&mut m, 0x4000, 400, 0xA);
        packet(&mut m, 0x6000, 800, 0xB);
        packet(&mut m, 0x8000, 4, 0xC);
        let mut st = |ms: &mut MovieStreams, m: &mut Mem| {
            ms.get_status(m, S, 0x3000).unwrap();
            m.read_u32(0x3000).unwrap()
        };
        assert_eq!(st(&mut ms, &mut m), 1);
        assert_eq!(ms.process(&mut m, S, 0x4000, 0, 100), Ok(0));
        assert_eq!(m.read_u32(0x4105), Some(PENDING_STATUS));
        assert_eq!(m.read_u32(0x4100), Some(0));
        assert_eq!(st(&mut ms, &mut m), 0x10001);
        assert_eq!(ms.process(&mut m, S, 0x6000, 0, 150), Ok(0));
        assert_eq!(st(&mut ms, &mut m), 0x10000);
        assert_eq!(ms.process(&mut m, S, 0x8000, 0, 150), Ok(TOO_MANY_PACKETS));
        assert_eq!(ms.snapshot(S).unwrap().head_deadline, 500);
        assert!(ms.do_work(&mut m, 499).unwrap().is_empty());
        // head played at 500, the second starts there (not at the poll tick) and ends at 1300
        let done = ms.do_work(&mut m, 900).unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(
            (
                done[0].packet_context,
                done[0].status,
                done[0].stream_context
            ),
            (0xA, 0, 0xDEC0)
        );
        assert_eq!(m.read_u32(0x4100), Some(400));
        assert_eq!(m.read_u32(0x4105), Some(0));
        assert_eq!(ms.snapshot(S).unwrap().head_deadline, 1300);
        assert_eq!(ms.do_work(&mut m, 1300).unwrap()[0].packet_context, 0xB);
        assert_eq!(ms.completions, 2);
    }

    #[test]
    fn pause_freezes_remainder_and_synch_resumes_mode2() {
        let (mut ms, mut m) = setup();
        packet(&mut m, 0x4000, 400, 0xA);
        ms.process(&mut m, S, 0x4000, 0, 0).unwrap();
        assert_eq!(ms.pause(S, 2, 100), Ok(0));
        ms.get_status(&mut m, S, 0x3000).unwrap();
        assert_eq!(m.read_u32(0x3000), Some(0x20001));
        assert!(ms.do_work(&mut m, 10_000).unwrap().is_empty());
        ms.synch_playback(1000);
        assert_eq!(ms.snapshot(S).unwrap().head_deadline, 1300);
        assert_eq!(ms.pause(S, 4, 0), Ok(E_FAIL));
        assert!(ms.pause(S, 3, 0).is_err());
        // mode 1 is not resumed by SynchPlayback
        ms.pause(S, 1, 1100).unwrap();
        ms.synch_playback(1200);
        assert_eq!(ms.snapshot(S).unwrap().pause_mode, 1);
    }

    #[test]
    fn flush_ex_plays_passed_deadline_and_aborts_rest() {
        let (mut ms, mut m) = setup();
        packet(&mut m, 0x4000, 400, 0xA);
        packet(&mut m, 0x6000, 400, 0xB);
        assert_eq!(ms.flush_ex(S, 0, 0, 1, 0), Ok(0)); // empty: no change
        assert!(!ms.snapshot(S).unwrap().flush_pending);
        ms.process(&mut m, S, 0x4000, 0, 0).unwrap();
        ms.process(&mut m, S, 0x6000, 0, 0).unwrap();
        assert!(ms.flush_ex(S, 0, 0, 0, 0).is_err());
        ms.flush_ex(S, 0, 0, 1, 450).unwrap();
        let done = ms.do_work(&mut m, 460).unwrap();
        assert_eq!(
            done.iter().map(|c| c.status).collect::<Vec<_>>(),
            vec![0, ABORT_STATUS]
        );
        assert!(!ms.snapshot(S).unwrap().flush_pending);
    }

    #[test]
    fn final_release_and_flush_abort_everything_fifo() {
        let (mut ms, mut m) = setup();
        packet(&mut m, 0x4000, 400, 0xA);
        packet(&mut m, 0x6000, 400, 0xB);
        ms.process(&mut m, S, 0x4000, 0, 0).unwrap();
        ms.process(&mut m, S, 0x6000, 0, 0).unwrap();
        ms.pause(S, 1, 10).unwrap();
        let done = ms.flush(&mut m, S, 20).unwrap();
        assert_eq!(
            done.iter()
                .map(|c| (c.packet_context, c.status))
                .collect::<Vec<_>>(),
            vec![(0xA, ABORT_STATUS), (0xB, ABORT_STATUS)]
        );
        assert_eq!(ms.snapshot(S).unwrap().pause_mode, 0);
        ms.process(&mut m, S, 0x4000, 0, 0).unwrap();
        // a deadline that passed unobserved still aborts on the final release (as the C destructor)
        let Released::Destroyed(done) = ms.release(&mut m, S, 10_000).unwrap() else {
            panic!()
        };
        assert_eq!(done[0].status, ABORT_STATUS);
        assert_eq!(m.read_u32(0x4100), Some(400));
    }

    #[test]
    fn process_refusals() {
        let (mut ms, mut m) = setup();
        packet(&mut m, 0x4000, 6, 0xA); // not a block multiple
        assert!(ms.process(&mut m, S, 0x4000, 0, 0).is_err());
        packet(&mut m, 0x4000, 8, 0xA);
        assert!(ms.process(&mut m, S, 0x4000, 4, 0).is_err());
        m.write_u32(0x4008, 0x4103); // completed overlaps status (0x4105)
        assert!(ms.process(&mut m, S, 0x4000, 0, 0).is_err());
        packet(&mut m, 0x4000, 8, 0xA);
        packet(&mut m, 0x6000, 8, 0xB);
        m.0.insert(0x4104, 0);
        m.write_u32(0x6008, 0x4102); // aliases the attached packet's completed word
        ms.process(&mut m, S, 0x4000, 0, 0).unwrap();
        assert_eq!(
            ms.process(&mut m, S, 0x6000, 0, 0),
            Err(Refusal("completion words alias an attached packet"))
        );
        assert!(ms.get_status(&mut m, S, S + 4).is_err());
        assert!(caller_ok(0x0044_5716) && caller_ok(0x0003_0280) && !caller_ok(0x0003_0676));
    }

    #[test]
    fn live_scheduler_owns_fifo_deadlines_and_pause_remainder() {
        let mut scheduler = MovieStreams::new(HZ);
        scheduler
            .attach_scheduler(S, 0x2000, 0x7000, CALLBACK, 0xDEC0)
            .unwrap();
        assert!(!scheduler.completion_word_aliases_protected_state(0x5000, 0x2000));
        assert!(scheduler.completion_word_aliases_protected_state(0x2010, 0x2000));
        assert!(scheduler.completion_word_aliases_protected_state(S + 3, 0x2000));
        assert!(scheduler.completion_word_aliases_protected_state(0x7002, 0x2000));
        assert!(scheduler.completion_word_aliases_protected_state(SINGLETON, 0x2000));
        assert!(scheduler.completion_word_aliases_protected_state(GLOBAL_STATE, 0x2000));
        assert!(scheduler.completion_word_aliases_protected_state(SECONDARY + 59, 0x2000));
        assert!(scheduler.completion_word_aliases_protected_state(0xFFFF_FFFE, 0x2000));
        scheduler
            .schedule_packet(S, 400, 0x4100, 0x4200, 0xA, 100)
            .unwrap();
        scheduler
            .schedule_packet(S, 400, 0x4300, 0x4400, 0xB, 100)
            .unwrap();
        assert_eq!(scheduler.scheduled_count(S), Some(2));
        assert!(scheduler.scheduled_words_alias(S, 0x4102, 0x4500).unwrap());

        scheduler.pause(S, 1, 200).unwrap();
        scheduler.pause(S, 0, 300).unwrap();
        assert_eq!(scheduler.snapshot(S).unwrap().head_deadline, 600);
        assert!(scheduler.take_scheduled(599, false, 0).is_none());
        let first = scheduler.take_scheduled(600, false, 0).unwrap();
        assert_eq!(
            first,
            ScheduledCompletion {
                event: Completion {
                    stream: S,
                    callback: CALLBACK,
                    stream_context: 0xDEC0,
                    packet_context: 0xA,
                    size: 400,
                    status: 0,
                },
                completed_address: 0x4100,
                status_address: 0x4200,
            }
        );
        assert_eq!(scheduler.snapshot(S).unwrap().head_deadline, 1000);
        let second = scheduler.take_scheduled(1000, false, 0).unwrap();
        assert_eq!(second.event.packet_context, 0xB);
        assert_eq!(scheduler.scheduled_count(S), Some(0));
        assert!(scheduler.detach_scheduler(S));
        assert_eq!(scheduler.count(), 0);
    }
}
