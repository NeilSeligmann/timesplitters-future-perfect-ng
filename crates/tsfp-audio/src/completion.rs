//! Completion model: the guest-visible words the game polls (stream status, packet completion, buffer
//! status and position), as Rust state with no guest memory and no C. Port of the behaviour of
//! src/audio/dsound_completion.c. The guest adapter owns memory: it passes the polled arguments in and
//! writes the returned [`Event`]s (packet completion words) and status values out.
//! Timing is the oracle's own: running virtual clock ticks, INFERRED (hardware drain is not observable).

use crate::mixer::{FreqClock, Phase, q32};

pub const PENDING_STATUS: u32 = 0x8000_000A;
pub const PACKET_LIST_FULL: u32 = 0x8878_0032;
pub const ABORTED_STATUS: u32 = 0x8000_4004;
const STREAM_STATES: usize = 32;
const BUFFER_STATES: usize = 64;
const MAX_QUEUE: u32 = 8;
const SERIAL_UNBOUND: u64 = u64::MAX;

/// Writes the guest adapter performs: `*completed = size` then `*status = status` (a zero address skips it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub stream: u32,
    pub completed: u32,
    pub size: u32,
    pub status_addr: u32,
    pub status: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The source clock left its finite contract (the oracle aborts here).
    ClockOverflow,
    /// Every stream state slot is taken by another address (the oracle refuses the call).
    NoFreeState,
}

#[derive(Clone, Copy, Default)]
struct Packet {
    size: u32,
    completed: u32,
    status: u32,
    remaining_q32: u128,
}

#[derive(Clone)]
struct Stream {
    address: u32,
    serial: u64,
    last: u64,
    running: bool,
    queued: u32,
    done: u32,
    clock: Option<FreqClock>,
    source_phase: Phase,
    active_ticks: u64,
    base_hz: u32,
    current_hz: u32,
    queue: [Packet; MAX_QUEUE as usize],
}

#[derive(Clone, Default)]
struct Buffer {
    address: u32,
    serial: u64,
    last: u64,
    remaining: u64,
    duration: u64,
    position: u64,
    samples: u64,
    loop_start: u64,
    loop_end: u64,
    loop_start_samples: u64,
    loop_end_samples: u64,
    playing: bool,
    looping: bool,
    paused: bool,
    stopped: bool,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub stream_packets: u64,
    pub stream_completed: u64,
    pub stream_aborted: u64,
    pub buffer_plays: u64,
    pub buffer_finished: u64,
    pub buffer_stops: u64,
    /// Stream and buffer observations (each advance), as the oracle's `stats.observations`.
    pub observations: u64,
}

pub struct Completion {
    clock_hz: u64,
    /// T855 mode: finished packets wait for DoWork before their words are written.
    pub dowork_delivery: bool,
    streams: Vec<Option<Stream>>,
    buffers: Vec<Option<Buffer>>,
    pub stats: Stats,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Process {
    Queued,
    ListFull,
}

fn mul_wrap_div_ceil(units: u64, clock: u64, per_second: u64) -> u64 {
    if per_second == 0 {
        return u64::MAX;
    }
    units.wrapping_mul(clock).wrapping_add(per_second - 1) / per_second
}

impl Completion {
    pub fn new(clock_hz: u64) -> Self {
        Self {
            clock_hz,
            dowork_delivery: false,
            streams: vec![None; STREAM_STATES],
            buffers: vec![None; BUFFER_STATES],
            stats: Stats::default(),
        }
    }
    pub fn clock_hz(&self) -> u64 {
        self.clock_hz
    }
    /// Reconfigure the tick rate without touching the states (the oracle's `dsound_completion_set_clock`).
    pub fn set_clock_hz(&mut self, clock_hz: u64) {
        self.clock_hz = clock_hz;
    }
    /// Forget every stream and buffer state and the counters (the oracle's `dsound_completion_reset`).
    pub fn reset(&mut self) {
        self.streams = vec![None; STREAM_STATES];
        self.buffers = vec![None; BUFFER_STATES];
        self.stats = Stats::default();
    }
    /// Packets queued on the stream state of exactly this address and serial.
    pub fn stream_queued(&self, address: u32, serial: u64) -> Option<u32> {
        self.streams
            .iter()
            .flatten()
            .find(|s| s.address == address && s.serial == serial)
            .map(|s| s.queued)
    }
    /// (serial, queued, current rate, last observation) of the state `find_bound` would pick for this lease.
    pub fn stream_info(&self, address: u32, serial: u64) -> Option<(u64, u32, u32, u64)> {
        self.find_bound(address, serial).map(|i| {
            let s = self.streams[i].as_ref().unwrap();
            (s.serial, s.queued, s.current_hz, s.last)
        })
    }
    /// Whether Play could take a state for this address: a state of the address exists or a slot is free.
    pub fn buffer_slot_available(&self, address: u32) -> bool {
        self.buffers
            .iter()
            .any(|b| b.as_ref().is_none_or(|b| b.address == address))
    }
    /// Advance the buffer under this lease to `now`; (playing, stopped) or `None` when it has no state.
    pub fn buffer_observe(&mut self, address: u32, serial: u64, now: u64) -> Option<(bool, bool)> {
        let i = self.buffer_index(address, serial)?;
        self.buffer_advance(i, now);
        let b = self.buffers[i].as_ref().unwrap();
        Some((b.playing, b.stopped))
    }
    /// Whether a buffer state exists under this lease (no advance).
    pub fn has_buffer(&self, address: u32, serial: u64) -> bool {
        self.buffer_index(address, serial).is_some()
    }
    /// Whether a stream state of this address exists (any serial).
    pub fn has_stream(&self, address: u32) -> bool {
        self.streams.iter().flatten().any(|s| s.address == address)
    }
    fn ticks_for(&self, units: u64, per_second: u64) -> u64 {
        mul_wrap_div_ceil(units, self.clock_hz, per_second)
    }

    // ---- streams ----
    fn stream_find(&mut self, address: u32, serial: u64, create: bool, now: u64) -> Option<usize> {
        let mut free_slot = None;
        for i in 0..STREAM_STATES {
            match &self.streams[i] {
                Some(s) if s.address == address => {
                    if s.serial == serial {
                        return Some(i);
                    }
                    self.streams[i] = None;
                    free_slot = Some(i);
                    break;
                }
                None if free_slot.is_none() => free_slot = Some(i),
                _ => {}
            }
        }
        if !create {
            return None;
        }
        let slot = free_slot?;
        self.streams[slot] = Some(Stream {
            address,
            serial,
            last: now,
            running: false,
            queued: 0,
            done: 0,
            clock: None,
            source_phase: Phase {
                ticks: 0,
                fraction: 0,
            },
            active_ticks: 0,
            base_hz: 0,
            current_hz: 0,
            queue: [Packet::default(); MAX_QUEUE as usize],
        });
        Some(slot)
    }
    /// A state created by a Pause note carries the sentinel serial: bind it to the first lease that asks.
    fn stream_bind(&mut self, address: u32, serial: u64, now: u64) -> Option<usize> {
        for s in self.streams.iter_mut().flatten() {
            if s.address == address && s.serial == SERIAL_UNBOUND {
                s.serial = serial;
            }
        }
        self.stream_find(address, serial, true, now)
    }

    /// Pure candidate advance (the oracle's stream_candidate_advance).
    fn candidate_advance(s: &mut Stream, now: u64) -> Result<(), Error> {
        let elapsed = now.saturating_sub(s.last);
        s.last = now;
        if !s.running {
            return Ok(());
        }
        if elapsed > u64::MAX - s.active_ticks {
            return Err(Error::ClockOverflow);
        }
        s.active_ticks += elapsed;
        let phase = match &s.clock {
            None => Phase {
                ticks: s.active_ticks,
                fraction: 0,
            },
            Some(c) => c.phase(s.active_ticks).ok_or(Error::ClockOverflow)?,
        };
        let (previous, current) = (q32(s.source_phase), q32(phase));
        if current < previous {
            return Err(Error::ClockOverflow);
        }
        let mut available = current - previous;
        s.source_phase = phase;
        while s.done < s.queued && available != 0 {
            let head = &mut s.queue[s.done as usize];
            let take = available.min(head.remaining_q32);
            head.remaining_q32 -= take;
            available -= take;
            if head.remaining_q32 != 0 {
                break;
            }
            s.done += 1;
        }
        Ok(())
    }
    fn deliver_head(s: &mut Stream, stats: &mut Stats, events: &mut Vec<Event>) {
        let head = s.queue[0];
        events.push(Event {
            stream: s.address,
            completed: head.completed,
            size: head.size,
            status_addr: head.status,
            status: 0,
        });
        stats.stream_completed += 1;
        for i in 1..s.queued as usize {
            s.queue[i - 1] = s.queue[i];
        }
        s.queued -= 1;
        s.queue[s.queued as usize] = Packet::default();
    }
    fn publish(&mut self, index: usize, candidate: Stream, events: &mut Vec<Event>) {
        let dowork = self.dowork_delivery;
        self.stats.observations += 1;
        let mut state = candidate;
        if !dowork {
            while state.done != 0 {
                Self::deliver_head(&mut state, &mut self.stats, events);
                state.done -= 1;
            }
        }
        self.streams[index] = Some(state);
    }
    fn advance(&mut self, index: usize, now: u64, events: &mut Vec<Event>) -> Result<(), Error> {
        let mut candidate = self.streams[index].clone().unwrap();
        Self::candidate_advance(&mut candidate, now)?;
        self.publish(index, candidate, events);
        Ok(())
    }

    /// IDirectSoundStream_Pause note: mode 0 runs the stream, anything else holds it.
    pub fn pause_note(&mut self, address: u32, mode: u32, now: u64) -> Result<Vec<Event>, Error> {
        Ok(self
            .pause_note_checked(address, mode, now)?
            .unwrap_or_default())
    }

    /// `pause_note`, `None` when no state exists and none is free (the oracle then records nothing).
    pub fn pause_note_checked(
        &mut self,
        address: u32,
        mode: u32,
        now: u64,
    ) -> Result<Option<Vec<Event>>, Error> {
        let mut events = vec![];
        let mut found = None;
        for (i, s) in self.streams.iter().enumerate() {
            if s.as_ref().is_some_and(|s| s.address == address) {
                found = Some(i);
            }
        }
        if found.is_none() {
            if let Some(i) = self.streams.iter().position(Option::is_none) {
                found = self
                    .stream_find(address, SERIAL_UNBOUND, true, now)
                    .or(Some(i));
            }
        }
        let Some(i) = found else { return Ok(None) };
        self.advance(i, now, &mut events)?;
        self.streams[i].as_mut().unwrap().running = mode == 0;
        Ok(Some(events))
    }

    /// GetStatus word: bit0 room for another packet, 0x10000 running with packets, 0x20000 held with packets.
    pub fn stream_status(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        max_packets: u32,
    ) -> Result<(u32, Vec<Event>), Error> {
        let mut events = vec![];
        let i = self
            .stream_bind(address, serial, now)
            .ok_or(Error::NoFreeState)?;
        self.advance(i, now, &mut events)?;
        let s = self.streams[i].as_ref().unwrap();
        let mut value = u32::from(s.queued < max_packets);
        if s.queued != 0 {
            value |= if s.running { 0x10000 } else { 0x20000 };
        }
        Ok((value, events))
    }

    /// First half of Process: advance, then say whether the list is full. `Ok(None)` = full (PACKET_LIST_FULL,
    /// nothing else may happen); `Ok(Some(running))` = proceed, `running` is the stream's current run state.
    pub fn stream_process_begin(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        max_packets: u32,
        events: &mut Vec<Event>,
    ) -> Result<Option<bool>, Error> {
        let i = self
            .stream_bind(address, serial, now)
            .ok_or(Error::NoFreeState)?;
        self.advance(i, now, events)?;
        let max = max_packets.min(MAX_QUEUE);
        let s = self.streams[i].as_ref().unwrap();
        Ok(if s.queued >= max {
            None
        } else {
            Some(s.running)
        })
    }
    /// Second half of Process: queue the packet. `average` is the format byte rate, `nominal_hz` its sample rate.
    #[allow(clippy::too_many_arguments)]
    pub fn stream_process_push(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        size: u32,
        completed: u32,
        status: u32,
        average: u32,
        nominal_hz: u32,
    ) -> bool {
        let Some(i) = self.stream_bind(address, serial, now) else {
            return false;
        };
        if self.streams[i].as_ref().unwrap().queued >= MAX_QUEUE {
            return false;
        }
        let mut remaining = u128::from(self.ticks_for(u64::from(size), u64::from(average))) << 32;
        let base = self.streams[i].as_ref().unwrap().base_hz;
        if base != 0 {
            remaining = (remaining * u128::from(nominal_hz)) / u128::from(base);
        }
        let s = self.streams[i].as_mut().unwrap();
        s.queue[s.queued as usize] = Packet {
            size,
            completed,
            status,
            remaining_q32: remaining,
        };
        s.queued += 1;
        self.stats.stream_packets += 1;
        true
    }
    /// Process in one step (no guest memory in between), for tests and replay.
    #[allow(clippy::too_many_arguments)]
    pub fn stream_process(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        size: u32,
        completed: u32,
        status: u32,
        average: u32,
        nominal_hz: u32,
        max_packets: u32,
    ) -> Result<(Process, Vec<Event>), Error> {
        let mut events = vec![];
        if self
            .stream_process_begin(address, serial, now, max_packets, &mut events)?
            .is_none()
        {
            return Ok((Process::ListFull, events));
        }
        self.stream_process_push(
            address, serial, now, size, completed, status, average, nominal_hz,
        );
        Ok((Process::Queued, events))
    }

    fn find_bound(&self, address: u32, serial: u64) -> Option<usize> {
        let mut found = None;
        for (i, s) in self.streams.iter().enumerate() {
            if s.as_ref().is_some_and(|s| {
                s.address == address && (s.serial == serial || s.serial == SERIAL_UNBOUND)
            }) {
                found = Some(i);
            }
        }
        found
    }

    /// PCM rate event. `runtime_ok` is asked once the candidate is valid (the mixer's verdict for pcm streams).
    #[allow(clippy::too_many_arguments)]
    pub fn stream_frequency(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        old_hz: u32,
        new_hz: u32,
        pcm: bool,
        runtime_ok: &mut dyn FnMut() -> bool,
    ) -> Result<(bool, Vec<Event>), Error> {
        self.stream_frequency_with_base(address, serial, now, old_hz, new_hz, pcm, &mut |_| {
            runtime_ok()
        })
    }

    /// `stream_frequency` where the runtime verdict also sees the stream's base rate (the oracle passes it to the mixer).
    #[allow(clippy::too_many_arguments)]
    pub fn stream_frequency_with_base(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        old_hz: u32,
        new_hz: u32,
        pcm: bool,
        runtime_ok: &mut dyn FnMut(u32) -> bool,
    ) -> Result<(bool, Vec<Event>), Error> {
        let mut events = vec![];
        if address == 0
            || serial == 0
            || !(8000..=96000).contains(&old_hz)
            || !(8000..=96000).contains(&new_hz)
        {
            return Ok((false, events));
        }
        let Some(i) = self.find_bound(address, serial) else {
            return Ok((false, events));
        };
        let state = self.streams[i].as_ref().unwrap();
        if !(now == now
            && now >= state.last
            && (state.current_hz == 0 || state.current_hz == old_hz))
        {
            return Ok((false, events));
        }
        let mut candidate = state.clone();
        candidate.serial = serial;
        Self::candidate_advance(&mut candidate, now)?;
        let base = if candidate.base_hz != 0 {
            candidate.base_hz
        } else {
            old_hz
        };
        let mut ok = true;
        if candidate.clock.is_none() {
            match FreqClock::init(base) {
                Some(c) => candidate.clock = Some(c),
                None => ok = false,
            }
        }
        if ok {
            ok = candidate.clock.as_mut().unwrap().change(
                candidate.active_ticks,
                u64::from(new_hz) << 32,
                candidate.active_ticks,
            );
        }
        candidate.base_hz = base;
        candidate.current_hz = new_hz;
        if ok && pcm {
            ok = runtime_ok(base);
        }
        if ok {
            self.publish(i, candidate, &mut events);
        }
        Ok((ok, events))
    }

    /// SetFormat: new rate, every admitted packet is aborted synchronously (words written with 0x80004004).
    pub fn stream_format(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        old_hz: u32,
        new_hz: u32,
        guest_ok: bool,
        runtime_ok: &mut dyn FnMut() -> bool,
    ) -> Result<(bool, Vec<Event>), Error> {
        self.stream_format_with(
            address,
            serial,
            now,
            old_hz,
            new_hz,
            &mut |_| guest_ok,
            runtime_ok,
        )
    }

    /// `stream_format` where the guest-word admission sees the candidate's queued (completed, status) addresses after the
    /// advance, like the oracle (the done packets are still queued there).
    #[allow(clippy::too_many_arguments)]
    pub fn stream_format_with(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        old_hz: u32,
        new_hz: u32,
        guest_ok: &mut dyn FnMut(&[(u32, u32)]) -> bool,
        runtime_ok: &mut dyn FnMut() -> bool,
    ) -> Result<(bool, Vec<Event>), Error> {
        let mut events = vec![];
        if address == 0
            || serial == 0
            || !(8000..=96000).contains(&old_hz)
            || !(8000..=48000).contains(&new_hz)
        {
            return Ok((false, events));
        }
        let Some(i) = self.find_bound(address, serial) else {
            return Ok((false, events));
        };
        let state = self.streams[i].as_ref().unwrap();
        if !(now >= state.last && (state.current_hz == 0 || state.current_hz == old_hz)) {
            return Ok((false, events));
        }
        let mut candidate = state.clone();
        candidate.serial = serial;
        let base = if candidate.base_hz != 0 {
            candidate.base_hz
        } else {
            old_hz
        };
        Self::candidate_advance(&mut candidate, now)?;
        if candidate.clock.is_none() {
            match FreqClock::init(base) {
                Some(c) => candidate.clock = Some(c),
                None => return Ok((false, events)),
            }
        }
        if !candidate.clock.as_mut().unwrap().change(
            candidate.active_ticks,
            u64::from(new_hz) << 32,
            candidate.active_ticks,
        ) {
            return Ok((false, events));
        }
        candidate.base_hz = base;
        candidate.current_hz = new_hz;
        let pending: Vec<(u32, u32)> = candidate.queue[..candidate.queued as usize]
            .iter()
            .map(|p| (p.completed, p.status))
            .collect();
        if !guest_ok(&pending) || !runtime_ok() {
            return Ok((false, events));
        }
        for p in &candidate.queue[..candidate.queued as usize] {
            events.push(Event {
                stream: address,
                completed: p.completed,
                size: p.size,
                status_addr: p.status,
                status: ABORTED_STATUS,
            });
        }
        self.stats.stream_aborted += u64::from(candidate.queued);
        candidate.queued = 0;
        candidate.done = 0;
        candidate.queue = [Packet::default(); MAX_QUEUE as usize];
        let mut delivered = vec![];
        self.publish(i, candidate, &mut delivered);
        Ok((true, events))
    }

    // ---- buffers ----
    fn buffer_index(&self, address: u32, serial: u64) -> Option<usize> {
        self.buffers.iter().position(|b| {
            b.as_ref()
                .is_some_and(|b| b.address == address && b.serial == serial)
        })
    }
    fn buffer_find(&mut self, address: u32, serial: u64, create: bool, now: u64) -> Option<usize> {
        let mut free_slot = None;
        for i in 0..BUFFER_STATES {
            match &self.buffers[i] {
                Some(b) if b.address == address => {
                    if b.serial == serial {
                        return Some(i);
                    }
                    self.buffers[i] = None;
                    free_slot = Some(i);
                    break;
                }
                None if free_slot.is_none() => free_slot = Some(i),
                _ => {}
            }
        }
        if !create {
            return None;
        }
        let slot = free_slot?;
        self.buffers[slot] = Some(Buffer {
            address,
            serial,
            last: now,
            ..Buffer::default()
        });
        Some(slot)
    }
    fn buffer_advance(&mut self, i: usize, now: u64) {
        self.stats.observations += 1;
        let b = self.buffers[i].as_mut().unwrap();
        let elapsed = now.saturating_sub(b.last);
        b.last = now;
        if !b.playing || b.paused {
            return;
        }
        if b.looping {
            if b.loop_start == 0 && b.loop_end == b.duration && b.duration != 0 {
                let step = elapsed % b.duration;
                let to_wrap = b.duration - b.position;
                b.position = if step >= to_wrap {
                    step - to_wrap
                } else {
                    b.position + step
                };
            } else if b.loop_end > b.loop_start {
                let to_wrap = b.loop_end - b.position;
                b.position = if elapsed < to_wrap {
                    b.position + elapsed
                } else {
                    b.loop_start + (elapsed - to_wrap) % (b.loop_end - b.loop_start)
                };
            }
            return;
        }
        if elapsed >= b.remaining {
            b.remaining = 0;
            b.position = b.duration;
            b.playing = false;
            self.stats.buffer_finished += 1;
        } else {
            b.remaining -= elapsed;
            b.position += elapsed;
        }
    }

    /// Play: `samples` in the data, rates in Hz, loop region in samples. A play of a playing buffer restarts.
    #[allow(clippy::too_many_arguments)]
    pub fn buffer_play(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        looping: bool,
        samples: u64,
        frequency: u32,
        loop_start_samples: u64,
        loop_end_samples: u64,
    ) -> bool {
        let Some(i) = self.buffer_find(address, serial, true, now) else {
            return false;
        };
        self.buffer_advance(i, now);
        let duration = self.ticks_for(samples, u64::from(frequency));
        let (ls, le) = (
            self.ticks_for(loop_start_samples, u64::from(frequency)),
            self.ticks_for(loop_end_samples, u64::from(frequency)),
        );
        let b = self.buffers[i].as_mut().unwrap();
        b.playing = true;
        b.stopped = false;
        b.looping = looping;
        b.samples = samples;
        b.duration = duration;
        b.position = 0;
        b.remaining = duration;
        b.last = now;
        b.loop_start_samples = loop_start_samples;
        b.loop_end_samples = loop_end_samples;
        b.loop_start = ls;
        b.loop_end = le;
        self.stats.buffer_plays += 1;
        true
    }
    /// Stop with flag 0: stop at the end of the current data (a looping voice drains from where it is).
    /// `runtime` is asked to stop the mixer voice when the buffer is playing and not yet stopped; false = refused
    /// (the oracle stops the guest there), nothing changes.
    pub fn buffer_stop(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        runtime: &mut dyn FnMut() -> bool,
    ) -> bool {
        if let Some(i) = self.buffer_find(address, serial, false, now) {
            self.buffer_advance(i, now);
            let b = self.buffers[i].as_ref().unwrap();
            if b.playing && !b.stopped && !runtime() {
                return false;
            }
            let b = self.buffers[i].as_mut().unwrap();
            if b.playing && b.looping {
                b.remaining = b.duration - b.position;
                b.looping = false;
            }
            if b.playing {
                b.paused = false;
                b.stopped = true;
            }
        }
        self.stats.buffer_stops += 1;
        true
    }
    /// Whether the buffer under this lease is currently paused (Play keeps a pause across a restart).
    pub fn buffer_is_paused(&self, address: u32, serial: u64) -> bool {
        self.buffer_index(address, serial)
            .is_some_and(|i| self.buffers[i].as_ref().unwrap().paused)
    }
    /// Pause mode 0 resumes, 1 and 2 hold a playing, not stopped buffer; anything else is refused.
    pub fn buffer_pause(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        mode: u32,
        runtime: &mut dyn FnMut(bool) -> bool,
    ) -> bool {
        let Some(i) = self.buffer_index(address, serial) else {
            return false;
        };
        if mode > 2 {
            return false;
        }
        self.buffer_advance(i, now);
        let b = self.buffers[i].as_ref().unwrap();
        let can_change = mode == 0 || (b.playing && !b.stopped);
        if !can_change || (b.playing && !runtime(mode != 0)) {
            return false;
        }
        let b = self.buffers[i].as_mut().unwrap();
        if mode == 0 {
            b.paused = false;
        } else {
            b.paused = true;
        }
        true
    }
    /// SetFrequency of a played buffer: keeps the samples left, rescales position and loop.
    pub fn buffer_frequency(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        old_hz: u32,
        new_hz: u32,
        runtime: &mut dyn FnMut() -> bool,
    ) -> bool {
        self.buffer_frequency_with(address, serial, now, old_hz, new_hz, &mut || {}, runtime)
    }

    /// `buffer_frequency` that tells the caller when the request passed admission and the buffer was advanced (the oracle
    /// records its observation line there, before the mixer's verdict).
    #[allow(clippy::too_many_arguments)]
    pub fn buffer_frequency_with(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        old_hz: u32,
        new_hz: u32,
        admitted: &mut dyn FnMut(),
        runtime: &mut dyn FnMut() -> bool,
    ) -> bool {
        let Some(i) = self.buffer_index(address, serial) else {
            return false;
        };
        if old_hz == 0 || new_hz == 0 {
            return false;
        }
        {
            let b = self.buffers[i].as_ref().unwrap();
            if b.looping
                && self.ticks_for(b.loop_end_samples, u64::from(new_hz))
                    <= self.ticks_for(b.loop_start_samples, u64::from(new_hz))
            {
                return false;
            }
        }
        self.buffer_advance(i, now);
        admitted();
        if self.buffers[i].as_ref().unwrap().playing && !runtime() {
            return false;
        }
        let (samples, lss, les) = {
            let b = self.buffers[i].as_ref().unwrap();
            (b.samples, b.loop_start_samples, b.loop_end_samples)
        };
        let (duration, ls, le) = (
            self.ticks_for(samples, u64::from(new_hz)),
            self.ticks_for(lss, u64::from(new_hz)),
            self.ticks_for(les, u64::from(new_hz)),
        );
        let b = self.buffers[i].as_mut().unwrap();
        if b.playing {
            b.position =
                ((u128::from(b.position) * u128::from(old_hz)) / u128::from(new_hz)) as u64;
            b.duration = duration;
            b.loop_start = ls;
            b.loop_end = le;
            if b.looping && b.position >= b.loop_end && b.loop_end > b.loop_start {
                b.position =
                    b.loop_start + (b.position - b.loop_start) % (b.loop_end - b.loop_start);
            } else if !b.looping {
                let scaled = u128::from(b.remaining) * u128::from(old_hz);
                b.remaining = ((scaled + u128::from(new_hz) - 1) / u128::from(new_hz)) as u64;
            }
        }
        true
    }
    /// Live SetLoopRegion in samples at `frequency`; false when empty or past the data.
    pub fn buffer_loop(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        start_samples: u64,
        end_samples: u64,
        frequency: u32,
        runtime: &mut dyn FnMut() -> bool,
    ) -> bool {
        let Some(i) = self.buffer_index(address, serial) else {
            return false;
        };
        if frequency == 0 {
            return false;
        }
        let (st, et) = (
            self.ticks_for(start_samples, u64::from(frequency)),
            self.ticks_for(end_samples, u64::from(frequency)),
        );
        self.buffer_advance(i, now);
        let b = self.buffers[i].as_mut().unwrap();
        if et <= st || end_samples > b.samples {
            return false;
        }
        if b.playing && b.looping && !runtime() {
            return false;
        }
        b.loop_start_samples = start_samples;
        b.loop_end_samples = end_samples;
        b.loop_start = st;
        b.loop_end = et;
        if b.playing && b.looping && b.position >= b.loop_end {
            b.position = b.loop_start + (b.position - b.loop_start) % (b.loop_end - b.loop_start);
        }
        true
    }
    /// GetStatus word: 0 idle, 1 playing, 2 paused, +4 looping.
    pub fn buffer_status(&mut self, address: u32, serial: u64, now: u64) -> u32 {
        match self.buffer_find(address, serial, false, now) {
            Some(i) => {
                self.buffer_advance(i, now);
                let b = self.buffers[i].as_ref().unwrap();
                if b.playing {
                    (if b.paused { 2 } else { 1 }) | (if b.looping { 4 } else { 0 })
                } else {
                    0
                }
            }
            None => 0,
        }
    }
    /// (position in samples, total samples, playing, looping, paused) at the buffer's current `frequency`.
    pub fn buffer_position(
        &mut self,
        address: u32,
        serial: u64,
        now: u64,
        frequency: u32,
    ) -> Option<(u64, u64, bool, bool, bool)> {
        let i = self.buffer_index(address, serial)?;
        self.buffer_advance(i, now);
        let b = self.buffers[i].as_ref().unwrap();
        let pos =
            ((u128::from(b.position) * u128::from(frequency)) / u128::from(self.clock_hz)) as u64;
        Some((pos, b.samples, b.playing, b.looping, b.paused))
    }

    /// DirectSoundDoWork: advance everything, deliver finished packets in DoWork mode.
    pub fn work(&mut self, now: u64) -> Result<Vec<Event>, Error> {
        let mut events = vec![];
        for i in 0..STREAM_STATES {
            if self.streams[i]
                .as_ref()
                .is_some_and(|s| s.serial != SERIAL_UNBOUND)
            {
                self.advance(i, now, &mut events)?;
                if self.dowork_delivery {
                    let s = self.streams[i].as_mut().unwrap();
                    while s.done != 0 {
                        Self::deliver_head(s, &mut self.stats, &mut events);
                        s.done -= 1;
                    }
                }
            }
        }
        for i in 0..BUFFER_STATES {
            if self.buffers[i].is_some() {
                self.buffer_advance(i, now);
            }
        }
        Ok(events)
    }
}
