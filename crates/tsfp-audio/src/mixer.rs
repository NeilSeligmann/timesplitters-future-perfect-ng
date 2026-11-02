//! Stream and buffer voice mixer. Port of the behaviour of the oracle src/audio/dsound_mixer.c and
//! dsound_stream_frequency.c (tick timeline, pause segments, frequency clock, routing, headroom,
//! Kaiser sinc upsampler with packet-join history, buffer voices with the render-horizon rule).
//! Same capacity limits and refusals as the oracle, so a recorded command stream replays identically.
//! Timing relations are INFERRED in the oracle itself and carried over unchanged.

use crate::adpcm;
use crate::contract::{Command, Key, Routing, SoundHost};
use crate::gain::volume_to_gain_q16;

const MAX_STREAMS: usize = 64;
const MAX_PACKETS: usize = 256;
const MAX_PACKET_BYTES: usize = 4 * 1024 * 1024;
const MAX_QUEUED_PCM_BYTES: usize = 16 * 1024 * 1024;
const MAX_PAUSE_SEGMENTS: usize = 64;
const MAX_BUFFER_RUNS: usize = 128;
const MAX_BUFFER_EVENTS: usize = 64;
const MAX_ROUTE_EVENTS: usize = 512;
const MAX_FREQ_EVENTS: usize = 64;
const MAX_HEADROOM_EVENTS: usize = 512;

const HALF_TAPS: i64 = 24;
const TAPS: usize = 48;
const PHASES: usize = 256;
const KAISER_BETA: f64 = 8.0;

fn mul_div_floor(v: u64, m: u64, d: u64) -> u64 {
    ((u128::from(v) * u128::from(m)) / u128::from(d)) as u64
}
fn mul_div_ceil(v: u64, m: u64, d: u64) -> u64 {
    ((u128::from(v) * u128::from(m) + u128::from(d) - 1) / u128::from(d)) as u64
}

// ---- resampler table, same arithmetic as the oracle (no libm) ----
fn r_sin(mut x: f64) -> f64 {
    let pi = 3.14159265358979323846f64;
    x -= 2.0 * pi * ((x / (2.0 * pi)) as i64 as f64);
    if x > pi {
        x -= 2.0 * pi;
    }
    if x < -pi {
        x += 2.0 * pi;
    }
    let (mut term, mut sum) = (x, x);
    for i in 1..24 {
        term *= -x * x / f64::from((2 * i) * (2 * i + 1));
        sum += term;
    }
    sum
}
fn r_sqrt(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut g = if x > 1.0 { x } else { 1.0 };
    for _ in 0..60 {
        g = 0.5 * (g + x / g);
    }
    g
}
fn r_i0(x: f64) -> f64 {
    let (mut term, mut sum) = (1.0f64, 1.0f64);
    for i in 1..40 {
        let half = x / (2.0 * f64::from(i));
        term *= half * half;
        sum += term;
    }
    sum
}
fn build_table() -> Vec<[f64; TAPS]> {
    let pi = 3.14159265358979323846f64;
    let denom = r_i0(KAISER_BETA);
    let mut table = vec![[0f64; TAPS]; PHASES + 1];
    for (phase, row) in table.iter_mut().enumerate() {
        let mut sum = 0.0;
        for (tap, slot) in row.iter_mut().enumerate() {
            let u = phase as f64 / PHASES as f64 + (HALF_TAPS - 1 - tap as i64) as f64;
            let ratio = u / HALF_TAPS as f64;
            let window = if ratio.abs() >= 1.0 {
                0.0
            } else {
                r_i0(KAISER_BETA * r_sqrt(1.0 - ratio * ratio)) / denom
            };
            let sinc = if u == 0.0 {
                1.0
            } else {
                r_sin(pi * u) / (pi * u)
            };
            *slot = sinc * window;
            sum += *slot;
        }
        for slot in row.iter_mut() {
            *slot /= sum;
        }
    }
    table
}

/// Source-time position: whole ticks plus a 32-bit fraction (oracle `dsound_frequency_phase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Phase {
    pub ticks: u64,
    pub fraction: u32,
}
pub(crate) fn q32(p: Phase) -> u128 {
    (u128::from(p.ticks) << 32) | u128::from(p.fraction)
}

#[derive(Clone, Copy)]
struct FreqEvent {
    active_ticks: u64,
    source_ticks: u64,
    frequency_q32: u64,
    fraction: u32,
}
/// Piecewise source-time clock of a stream (oracle `dsound_frequency_clock`, INFERRED model).
/// Public for the actual-C differential `tests/codec_c.rs`.
#[derive(Clone)]
pub struct FreqClock {
    pub(crate) base_hertz: u32,
    events: Vec<FreqEvent>,
}
impl FreqClock {
    pub fn init(base: u32) -> Option<Self> {
        if base == 0 || base > 200000 {
            return None;
        }
        Some(Self {
            base_hertz: base,
            events: vec![FreqEvent {
                active_ticks: 0,
                source_ticks: 0,
                frequency_q32: u64::from(base) << 32,
                fraction: 0,
            }],
        })
    }
    pub fn phase(&self, active: u64) -> Option<Phase> {
        if self.base_hertz == 0
            || self.events.is_empty()
            || self.events.len() > MAX_FREQ_EVENTS
            || active < self.events[0].active_ticks
        {
            return None;
        }
        let mut event = &self.events[0];
        for e in &self.events[1..] {
            if e.active_ticks > active {
                break;
            }
            event = e;
        }
        let denominator = u128::from(self.base_hertz) << 32;
        let product = u128::from(active - event.active_ticks) * u128::from(event.frequency_q32);
        let whole = product / denominator;
        let fraction = ((((product % denominator) << 32) / denominator) as u64)
            .wrapping_add(u64::from(event.fraction));
        let result = whole + u128::from(event.source_ticks) + u128::from(fraction >> 32);
        if result > u128::from(u64::MAX) {
            return None;
        }
        Some(Phase {
            ticks: result as u64,
            fraction: fraction as u32,
        })
    }
    /// Live frequency events (1..=64).
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
    pub fn change(&mut self, active: u64, frequency_q32: u64, retire_through: u64) -> bool {
        let last = match self.events.last() {
            Some(e) => e.active_ticks,
            None => return false,
        };
        if frequency_q32 == 0
            || frequency_q32 > (200000u64 << 32)
            || retire_through > active
            || active < last
        {
            return false;
        }
        let Some(phase) = self.phase(active) else {
            return false;
        };
        let next = FreqEvent {
            active_ticks: active,
            source_ticks: phase.ticks,
            frequency_q32,
            fraction: phase.fraction,
        };
        if active == last {
            *self.events.last_mut().unwrap() = next;
            return true;
        }
        let mut keep = 0;
        while keep + 1 < self.events.len() && self.events[keep + 1].active_ticks <= retire_through {
            keep += 1;
        }
        let retained = self.events.len() - keep;
        if retained == MAX_FREQ_EVENTS {
            return false;
        }
        self.events.drain(..keep);
        self.events.push(next);
        true
    }
}

#[derive(Clone)]
struct Packet {
    key: Key,
    rate: u32,
    start_ticks: u64,
    end_ticks: u64,
    start_fraction: u32,
    end_fraction: u32,
    pcm: Vec<i16>,
}
impl Packet {
    fn frames(&self) -> usize {
        self.pcm.len() / 2
    }
    fn start(&self) -> u128 {
        q32(Phase {
            ticks: self.start_ticks,
            fraction: self.start_fraction,
        })
    }
    fn end(&self) -> u128 {
        q32(Phase {
            ticks: self.end_ticks,
            fraction: self.end_fraction,
        })
    }
}

#[derive(Clone, Copy)]
struct RouteEvent {
    ticks: u64,
    gain: [u32; 4],
    bin_gain: [[u32; 4]; 6],
    // Source-channel gains before the speaker matrix, for the GP mixbuffer.
    source_bin_gain: [[u32; 2]; 6],
}

impl RouteEvent {
    // Preserve the individual sends until the GP has processed them. The
    // legacy speaker projection is deliberately kept separate.
    #[allow(dead_code)] // Used when GP frame production is connected.
    fn mix_bin_samples(&self, left: i32, right: i32, headroom: &[u8; 32]) -> [i64; 6] {
        std::array::from_fn(|bin| {
            let gain = self.source_bin_gain[bin];
            let shift = headroom[bin] & 7;
            let mixed = i64::from(left) * i64::from(gain[0] >> shift)
                + i64::from(right) * i64::from(gain[1] >> shift);
            (mixed + 32768) >> 16
        })
    }
}

#[derive(Clone)]
struct Stream {
    key: Key,
    paused: bool,
    attenuated: bool,
    gain_q16: u32,
    source_rate_hz: u32,
    clock: Option<FreqClock>,
    frequency_last_wall: u64,
    pause_started: u64,
    tail_ticks: u64,
    tail_fraction: u32,
    pauses: Vec<(u64, u64)>,
    route_serial: u64,
    routes: Vec<RouteEvent>,
}
impl Stream {
    fn new(key: Key) -> Self {
        Self {
            key,
            paused: false,
            attenuated: false,
            gain_q16: 65536,
            source_rate_hz: 0,
            clock: None,
            frequency_last_wall: 0,
            pause_started: 0,
            tail_ticks: 0,
            tail_fraction: 0,
            pauses: vec![],
            route_serial: 0,
            routes: vec![],
        }
    }
    /// (active ticks, paused)
    fn active_ticks(&self, wall: u64) -> (u64, bool) {
        let mut total = 0u64;
        for &(start, end) in &self.pauses {
            if wall < start {
                break;
            }
            if wall < end {
                return (start - total, true);
            }
            total += end - start;
        }
        if self.paused && wall >= self.pause_started {
            return (self.pause_started - total, true);
        }
        (wall - total, false)
    }
    fn source_phase(&self, active: u64) -> Option<Phase> {
        match &self.clock {
            None => Some(Phase {
                ticks: active,
                fraction: 0,
            }),
            Some(c) => c.phase(active),
        }
    }
}

#[derive(Clone, Copy)]
struct BufEvent {
    ticks: u64,
    position_q32: u64,
    frequency: u32,
    gain_q16: u32,
    loop_start: usize,
    loop_end: usize,
    paused: bool,
    looping: bool,
    looped: bool,
}
struct BufRun {
    key: Key,
    serial: u64,
    end_ticks: u64,
    pcm: Vec<i16>,
    events: Vec<BufEvent>,
}

#[derive(Clone, Copy)]
struct HeadroomEvent {
    ticks: u64,
    amount: [u8; 32],
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    pub packets_queued: u64,
    pub frames_emitted: u64,
    pub late_packets_refused: u64,
    pub buffer_commands_late: u64,
}

pub struct Mixer {
    out_hz: u32,
    tps: u64,
    started: bool,
    origin: u64,
    rendered: u64,
    queued_bytes: usize,
    hr_enabled: bool,
    hr_identity: u64,
    hr_last: u64,
    hr_events: Vec<HeadroomEvent>,
    streams: Vec<Stream>,
    packets: Vec<Packet>,
    buffers: Vec<BufRun>,
    counts: Counts,
    table: Vec<[f64; TAPS]>,
    pub last_buffer_refusal: &'static str,
}

impl Mixer {
    pub fn new(out_hz: u32, ticks_per_second: u64) -> Self {
        Self {
            out_hz,
            tps: ticks_per_second,
            started: false,
            origin: 0,
            rendered: 0,
            queued_bytes: 0,
            hr_enabled: false,
            hr_identity: 0,
            hr_last: 0,
            hr_events: vec![],
            streams: vec![],
            packets: vec![],
            buffers: vec![],
            counts: Counts::default(),
            table: build_table(),
            last_buffer_refusal: "unknown",
        }
    }
    pub fn counts(&self) -> Counts {
        self.counts
    }
    fn stream_idx(&self, key: Key) -> Option<usize> {
        self.streams.iter().position(|s| s.key == key)
    }
    fn stream_for(&mut self, key: Key) -> Option<usize> {
        if let Some(i) = self.stream_idx(key) {
            return Some(i);
        }
        if self.streams.len() >= MAX_STREAMS {
            return None;
        }
        self.streams.push(Stream::new(key));
        Some(self.streams.len() - 1)
    }
    fn rendered_edge(&self) -> u64 {
        self.origin + mul_div_floor(self.rendered, self.tps, u64::from(self.out_hz))
    }
    fn rendered_last(&self) -> u64 {
        self.origin
            + mul_div_floor(
                self.rendered.saturating_sub(1),
                self.tps,
                u64::from(self.out_hz),
            )
    }

    fn submit_decoded(&mut self, key: Key, now: u64, rate: u32, pcm: Vec<i16>) -> bool {
        let frames = pcm.len() / 2;
        let bytes = pcm.len() * 2;
        let si = self.stream_idx(key);
        let (active_wall, _) = si.map_or((now, false), |i| self.streams[i].active_ticks(now));
        let source_now = match si {
            Some(i) => match self.streams[i].source_phase(active_wall) {
                Some(p) => p,
                None => return false,
            },
            None => Phase {
                ticks: active_wall,
                fraction: 0,
            },
        };
        let tail = si.map_or(
            Phase {
                ticks: 0,
                fraction: 0,
            },
            |i| Phase {
                ticks: self.streams[i].tail_ticks,
                fraction: self.streams[i].tail_fraction,
            },
        );
        let start = if q32(tail) > q32(source_now) {
            tail
        } else {
            source_now
        };
        let rendered_offset = if self.started {
            mul_div_floor(
                self.rendered.saturating_sub(1),
                self.tps,
                u64::from(self.out_hz),
            )
        } else {
            0
        };
        if self.started && rendered_offset > u64::MAX - self.origin {
            return false;
        }
        let rendered_tick = if self.started {
            self.origin + rendered_offset
        } else {
            0
        };
        let (rendered_active, rendered_paused) = si.map_or((rendered_tick, false), |i| {
            self.streams[i].active_ticks(rendered_tick)
        });
        let rendered_source = match si {
            Some(i) => match self.streams[i].source_phase(rendered_active) {
                Some(p) => p,
                None => return false,
            },
            None => Phase {
                ticks: rendered_active,
                fraction: 0,
            },
        };
        if self.started
            && (q32(start) < q32(rendered_source)
                || (self.rendered != 0 && !rendered_paused && q32(start) == q32(rendered_source)))
        {
            self.counts.late_packets_refused += 1;
            return false;
        }
        if self.packets.len() >= MAX_PACKETS {
            return false;
        }
        let Some(si) = self.stream_for(key) else {
            return false;
        };
        let stream = &mut self.streams[si];
        if stream.source_rate_hz == 0 {
            stream.source_rate_hz = rate;
        }
        let effective = stream.clock.as_ref().map_or(rate, |c| c.base_hertz);
        let duration = mul_div_ceil(frames as u64, self.tps, u64::from(effective));
        if duration == 0 || start.ticks > u64::MAX - duration {
            return false;
        }
        let packet = Packet {
            key,
            rate: effective,
            start_ticks: start.ticks,
            end_ticks: start.ticks + duration,
            start_fraction: start.fraction,
            end_fraction: start.fraction,
            pcm,
        };
        self.queued_bytes += bytes;
        stream.tail_ticks = packet.end_ticks;
        stream.tail_fraction = packet.end_fraction;
        self.packets.push(packet);
        if !self.started {
            self.started = true;
            self.origin = now;
        }
        self.counts.packets_queued += 1;
        true
    }

    fn submit_adpcm(&mut self, key: Key, now: u64, rate: u32, data: &[u8], mono: bool) -> bool {
        let block = if mono {
            adpcm::MONO_BLOCK_BYTES
        } else {
            2 * adpcm::MONO_BLOCK_BYTES
        };
        if key == 0
            || rate == 0
            || data.is_empty()
            || data.len() > MAX_PACKET_BYTES
            || data.len() % block != 0
        {
            return false;
        }
        let frames = data.len() / block * adpcm::FRAMES_PER_BLOCK;
        if frames * 4 > MAX_QUEUED_PCM_BYTES - self.queued_bytes {
            return false;
        }
        let Some(pcm) = adpcm::decode(data, if mono { 1 } else { 2 }) else {
            return false;
        };
        let stereo = if mono {
            pcm.iter().flat_map(|&s| [s, s]).collect()
        } else {
            pcm
        };
        self.submit_decoded(key, now, rate, stereo)
    }

    fn cut_phase(&mut self, si: usize, key: Key, phase: Phase) {
        let active_now = q32(phase);
        let mut freed = 0usize;
        self.packets.retain_mut(|p| {
            if p.key != key || p.end() <= active_now {
                return true;
            }
            if p.start() >= active_now {
                freed += p.pcm.len() * 2;
                return false;
            }
            p.end_ticks = phase.ticks;
            p.end_fraction = phase.fraction;
            true
        });
        self.queued_bytes -= freed;
        let s = &mut self.streams[si];
        if q32(Phase {
            ticks: s.tail_ticks,
            fraction: s.tail_fraction,
        }) > active_now
        {
            s.tail_ticks = phase.ticks;
            s.tail_fraction = phase.fraction;
        }
    }

    fn set_stream_frequency(&mut self, key: Key, ticks: u64, rate: u32) -> bool {
        if key == 0 || !(8000..=96000).contains(&rate) {
            return false;
        }
        let si = self.stream_idx(key);
        if let Some(i) = si {
            if ticks < self.streams[i].frequency_last_wall {
                return false;
            }
        }
        let mut edge = 0u64;
        if self.started {
            let committed = if self.rendered != 0 {
                self.rendered - 1
            } else {
                0
            };
            let offset = mul_div_floor(committed, self.tps, u64::from(self.out_hz));
            if offset > u64::MAX - self.origin {
                return false;
            }
            edge = self.origin + offset;
            if if self.rendered != 0 {
                ticks <= edge
            } else {
                ticks < edge
            } {
                return false;
            }
        }
        let active = si.map_or(ticks, |i| self.streams[i].active_ticks(ticks).0);
        let retired = si.map_or(edge, |i| self.streams[i].active_ticks(edge).0);
        let mut candidate = si.and_then(|i| self.streams[i].clock.clone());
        if candidate.is_none() {
            let base = match si {
                Some(i) if self.streams[i].source_rate_hz != 0 => self.streams[i].source_rate_hz,
                _ => rate,
            };
            candidate = FreqClock::init(base);
        }
        let Some(mut clock) = candidate else {
            return false;
        };
        if !clock.change(active, u64::from(rate) << 32, retired) {
            return false;
        }
        let Some(i) = si.or_else(|| self.stream_for(key)) else {
            return false;
        };
        let s = &mut self.streams[i];
        s.clock = Some(clock);
        s.frequency_last_wall = ticks;
        s.source_rate_hz = rate;
        true
    }

    fn frequency_stream(
        &mut self,
        key: Key,
        serial: u64,
        ticks: u64,
        base: u32,
        old: u32,
        new: u32,
    ) -> bool {
        if serial == 0 || !(8000..=96000).contains(&base) {
            return false;
        }
        let Some(i) = self.stream_idx(key) else {
            return false;
        };
        let s = &self.streams[i];
        if s.route_serial != serial
            || (s.source_rate_hz != 0 && s.source_rate_hz != old)
            || s.clock.as_ref().is_some_and(|c| c.base_hertz != base)
        {
            return false;
        }
        let previous = self.streams[i].clone();
        if self.streams[i].clock.is_none() {
            self.streams[i].clock = FreqClock::init(base);
            if self.streams[i].clock.is_none() {
                return false;
            }
        }
        if !self.set_stream_frequency(key, ticks, new) {
            self.streams[i] = previous;
            return false;
        }
        true
    }

    fn format_stream(&mut self, key: Key, serial: u64, ticks: u64, old: u32, new: u32) -> bool {
        if !(8000..=96000).contains(&old) || !(8000..=48000).contains(&new) {
            return false;
        }
        let Some(i) = self.stream_idx(key) else {
            return false;
        };
        let s = &self.streams[i];
        if serial == 0
            || s.route_serial != serial
            || (s.source_rate_hz != 0 && s.source_rate_hz != old)
        {
            return false;
        }
        let active = s.active_ticks(ticks).0;
        let Some(phase) = s.source_phase(active) else {
            return false;
        };
        let previous = self.streams[i].clone();
        if self.streams[i].clock.is_none() {
            self.streams[i].clock = FreqClock::init(old);
            if self.streams[i].clock.is_none() {
                return false;
            }
        }
        if !self.set_stream_frequency(key, ticks, new) {
            self.streams[i] = previous;
            return false;
        }
        self.cut_phase(i, key, phase);
        true
    }

    fn route_stream(&mut self, key: Key, serial: u64, ticks: u64, routing: &Routing) -> bool {
        if key == 0 || serial == 0 || routing.count > 8 {
            return false;
        }
        const SPEAKER: [[u32; 2]; 6] = [
            [65536, 0],
            [0, 65536],
            [46341, 46341],
            [32768, 32768],
            [46341, 0],
            [0, 46341],
        ];
        let mut event = RouteEvent {
            ticks,
            gain: [0; 4],
            bin_gain: [[0; 4]; 6],
            source_bin_gain: [[0; 2]; 6],
        };
        let per_channel = routing.count / 2;
        for slot in 0..routing.count {
            let bin = routing.bins[slot as usize];
            if bin > 5 {
                return false;
            }
            if per_channel == 0 || slot >= per_channel * 2 {
                continue;
            }
            let source = (slot / per_channel) as usize;
            let gain = route_gain(routing.attenuation(slot));
            event.source_bin_gain[bin as usize][source] =
                event.source_bin_gain[bin as usize][source].wrapping_add(gain);
            for output in 0..2usize {
                let coefficient = ((u64::from(gain) * u64::from(SPEAKER[bin as usize][output])
                    + 32768)
                    >> 16) as u32;
                event.gain[output * 2 + source] =
                    event.gain[output * 2 + source].wrapping_add(coefficient);
                event.bin_gain[bin as usize][output * 2 + source] =
                    event.bin_gain[bin as usize][output * 2 + source].wrapping_add(coefficient);
            }
        }
        if self.started && self.rendered != 0 && ticks <= self.rendered_last() {
            return false;
        }
        let si = self.stream_idx(key);
        let replacement = si.is_some_and(|i| {
            !self.streams[i].routes.is_empty() && self.streams[i].route_serial != serial
        });
        if replacement && self.packets.iter().any(|p| p.key == key) {
            return false;
        }
        let edge = self.rendered_edge();
        let started = self.started;
        let mut keep = 0usize;
        let count = match si {
            Some(i) if !replacement => self.streams[i].routes.len(),
            _ => 0,
        };
        if count != 0 {
            let routes = &mut self.streams[si.unwrap()].routes;
            let last = *routes.last().unwrap();
            if ticks < last.ticks {
                return false;
            }
            if last.bin_gain == event.bin_gain && last.source_bin_gain == event.source_bin_gain {
                return true;
            }
            if ticks == last.ticks {
                *routes.last_mut().unwrap() = event;
                return true;
            }
            if started {
                while keep + 1 < count && routes[keep + 1].ticks <= edge {
                    keep += 1;
                }
            }
        }
        if count - keep == MAX_ROUTE_EVENTS {
            return false;
        }
        let Some(i) = si.or_else(|| self.stream_for(key)) else {
            return false;
        };
        let s = &mut self.streams[i];
        if replacement {
            s.routes.clear();
        }
        if count != 0 && keep != 0 {
            s.routes.drain(..keep);
        }
        s.route_serial = serial;
        s.routes.push(event);
        true
    }

    fn set_headroom(&mut self, identity: u64, ticks: u64, bin: u32, headroom: u32) -> bool {
        if !self.hr_enabled || identity == 0 || bin >= 32 {
            return false;
        }
        let replacement = self.hr_identity != 0 && self.hr_identity != identity;
        if replacement && (!self.packets.is_empty() || !self.buffers.is_empty()) {
            return false;
        }
        if self.started && self.rendered != 0 && ticks <= self.rendered_last() {
            return false;
        }
        if replacement {
            self.hr_events.clear();
        }
        let count = self.hr_events.len();
        let mut event = HeadroomEvent {
            ticks,
            amount: [0; 32],
        };
        if count != 0 {
            if ticks < self.hr_last {
                return false;
            }
            event.amount = self.hr_events[count - 1].amount;
        } else {
            event.amount[..31].fill(1);
        }
        event.amount[bin as usize] = headroom as u8;
        if count != 0 && event.amount == self.hr_events[count - 1].amount {
            self.hr_last = ticks;
            return true;
        }
        if count != 0 && ticks == self.hr_events[count - 1].ticks {
            self.hr_events[count - 1] = event;
            self.hr_last = ticks;
            return true;
        }
        let mut keep = 0;
        if self.started {
            let edge = self.rendered_edge();
            while keep + 1 < count && self.hr_events[keep + 1].ticks <= edge {
                keep += 1;
            }
        }
        if count - keep == MAX_HEADROOM_EVENTS {
            return false;
        }
        self.hr_events.drain(..keep);
        self.hr_identity = identity;
        self.hr_last = ticks;
        self.hr_events.push(event);
        true
    }

    // ---- buffer voices ----
    fn buffer_command_time(&mut self, ticks: &mut u64) -> bool {
        if !self.started {
            return true;
        }
        let offset = mul_div_floor(self.rendered, self.tps, u64::from(self.out_hz));
        if offset > u64::MAX - self.origin {
            return false;
        }
        let horizon = self.origin + offset;
        if *ticks < horizon {
            *ticks = horizon;
            self.counts.buffer_commands_late += 1;
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn play_buffer(
        &mut self,
        key: Key,
        serial: u64,
        mut ticks: u64,
        frequency: u32,
        pcm: &[i16],
        paused: bool,
        looping: bool,
        ls: usize,
        le: usize,
        volume: i32,
    ) -> bool {
        let Some(gain) = volume_to_gain_q16(volume) else {
            return false;
        };
        if key == 0
            || serial == 0
            || frequency == 0
            || pcm.is_empty()
            || pcm.len() > MAX_PACKET_BYTES / 2
            || (looping && (ls >= le || le > pcm.len()))
            || !self.buffer_command_time(&mut ticks)
        {
            return false;
        }
        if self.buffers.len() >= MAX_BUFFER_RUNS {
            return false;
        }
        if self
            .buffers
            .iter()
            .any(|r| r.key == key && r.events.last().unwrap().ticks > ticks)
        {
            return false;
        }
        let bytes = pcm.len() * 2;
        if bytes > MAX_QUEUED_PCM_BYTES - self.queued_bytes {
            return false;
        }
        for r in self.buffers.iter_mut() {
            if r.key == key && r.end_ticks == u64::MAX {
                r.end_ticks = ticks;
            }
        }
        self.buffers.push(BufRun {
            key,
            serial,
            end_ticks: u64::MAX,
            pcm: pcm.to_vec(),
            events: vec![BufEvent {
                ticks,
                position_q32: 0,
                frequency,
                gain_q16: gain,
                loop_start: ls,
                loop_end: le,
                paused,
                looping,
                looped: false,
            }],
        });
        self.queued_bytes += bytes;
        if !self.started {
            self.started = true;
            self.origin = ticks;
        }
        true
    }

    fn buffer_position(
        &self,
        frames: usize,
        event: &BufEvent,
        ticks: u64,
        looped: Option<&mut bool>,
    ) -> u64 {
        let mut position = u128::from(event.position_q32);
        if !event.paused {
            position += ((u128::from(ticks - event.ticks) * u128::from(event.frequency)) << 32)
                / u128::from(self.tps);
        }
        let start = (event.loop_start as u64) << 32;
        let end = (event.loop_end as u64) << 32;
        let mut flag = event.looped;
        if event.looping && position >= u128::from(end) {
            flag = true;
            position = u128::from(start) + (position - u128::from(start)) % u128::from(end - start);
        }
        if let Some(l) = looped {
            *l = flag;
        }
        let limit = (frames as u64) << 32;
        if position < u128::from(limit) {
            position as u64
        } else {
            limit
        }
    }

    fn change_buffer(&mut self, key: Key, serial: u64, mut ticks: u64, cmd: BufCmd) -> bool {
        self.last_buffer_refusal = "the render horizon overflows the tick range";
        if !self.buffer_command_time(&mut ticks) {
            return false;
        }
        self.last_buffer_refusal = "no live run for this buffer lease (its Play was retired by the end of its data or never reached the mixer)";
        let Some(ri) = self
            .buffers
            .iter()
            .position(|r| r.key == key && r.serial == serial && r.end_ticks == u64::MAX)
        else {
            return false;
        };
        let previous = *self.buffers[ri].events.last().unwrap();
        self.last_buffer_refusal =
            "the command timestamp is before the buffer's latest recorded event";
        if ticks < previous.ticks {
            return false;
        }
        self.last_buffer_refusal = "the frequency or volume value is invalid";
        let frames = self.buffers[ri].pcm.len();
        let mut event = previous;
        let mut looped = false;
        event.position_q32 = self.buffer_position(frames, &previous, ticks, Some(&mut looped));
        event.looped = looped;
        event.ticks = ticks;
        match cmd {
            BufCmd::Pause(p) => event.paused = p,
            BufCmd::Stop => {
                event.looping = false;
                event.paused = false;
            }
            BufCmd::Frequency(f) => {
                if f == 0 {
                    return false;
                }
                event.frequency = f;
            }
            BufCmd::Loop(ls, le) => {
                self.last_buffer_refusal =
                    "the live loop region is empty or past the buffer's data";
                if ls >= le || le > frames {
                    return false;
                }
                event.loop_start = ls;
                event.loop_end = le;
                if event.position_q32 < ((ls as u64) << 32) {
                    event.looped = false;
                }
            }
            BufCmd::Volume(v) => match volume_to_gain_q16(v) {
                Some(g) => event.gain_q16 = g,
                None => return false,
            },
        }
        if event.loop_start != previous.loop_start || event.loop_end != previous.loop_end {
        } else if event.paused == previous.paused
            && event.looping == previous.looping
            && event.frequency == previous.frequency
            && event.gain_q16 == previous.gain_q16
        {
            return true;
        }
        self.last_buffer_refusal =
            "the 64 event history of the run is full (events not yet rendered)";
        let run = &mut self.buffers[ri];
        if ticks != previous.ticks && run.events.len() == MAX_BUFFER_EVENTS {
            return false;
        }
        if ticks == previous.ticks {
            *run.events.last_mut().unwrap() = event;
        } else {
            run.events.push(event);
        }
        true
    }

    fn buffer_frame_at(run: &BufRun, event: &BufEvent, mut frame: i64) -> i16 {
        if event.looping
            && (frame >= event.loop_end as i64 || (event.looped && frame < event.loop_start as i64))
        {
            let length = (event.loop_end - event.loop_start) as i64;
            let mut relative = (frame - event.loop_start as i64) % length;
            if relative < 0 {
                relative += length;
            }
            frame = event.loop_start as i64 + relative;
        }
        let frame = frame.clamp(0, run.pcm.len() as i64 - 1);
        run.pcm[frame as usize]
    }

    fn buffer_sample(&self, run: &BufRun, ticks: u64) -> i32 {
        if ticks < run.events[0].ticks || ticks >= run.end_ticks {
            return 0;
        }
        let mut event = &run.events[0];
        for e in &run.events[1..] {
            if e.ticks > ticks {
                break;
            }
            event = e;
        }
        if event.paused {
            return 0;
        }
        let mut sampling = *event;
        let position =
            self.buffer_position(run.pcm.len(), event, ticks, Some(&mut sampling.looped));
        let frame = position >> 32;
        if frame >= run.pcm.len() as u64 {
            return 0;
        }
        let fraction = position & u64::from(u32::MAX);
        let sample: i64;
        if event.frequency < self.out_hz {
            let scaled = fraction * PHASES as u64;
            let phase = (scaled >> 32) as usize;
            let weight = (scaled & u64::from(u32::MAX)) as f64 / 4294967296.0;
            let mut total = 0.0f64;
            for tap in 0..TAPS {
                let coefficient =
                    self.table[phase][tap] * (1.0 - weight) + self.table[phase + 1][tap] * weight;
                total += coefficient
                    * f64::from(Self::buffer_frame_at(
                        run,
                        &sampling,
                        frame as i64 - HALF_TAPS + 1 + tap as i64,
                    ));
            }
            let s = (if total < 0.0 {
                total - 0.5
            } else {
                total + 0.5
            }) as i64;
            sample = s.clamp(-32768, 32767);
        } else {
            let first = i64::from(Self::buffer_frame_at(run, &sampling, frame as i64));
            let second = i64::from(Self::buffer_frame_at(run, &sampling, frame as i64 + 1));
            sample = (first * (1i64 << 32) + (second - first) * fraction as i64) / (1i64 << 32);
        }
        ((sample * i64::from(event.gain_q16) + 32768) >> 16) as i32
    }

    // ---- packets ----
    fn neighbour(&self, packet: &Packet, later: bool) -> Option<&Packet> {
        self.packets.iter().find(|o| {
            !std::ptr::eq(*o, packet)
                && o.key == packet.key
                && o.rate == packet.rate
                && if later {
                    o.start() == packet.end()
                } else {
                    o.end() == packet.start()
                }
        })
    }
    fn frame_at<'a>(&'a self, mut packet: &'a Packet, mut index: i64, channel: usize) -> i32 {
        loop {
            if index < 0 {
                match self.neighbour(packet, false) {
                    None => {
                        index = 0;
                        break;
                    }
                    Some(p) => {
                        packet = p;
                        index += packet.frames() as i64;
                    }
                }
            } else if index >= packet.frames() as i64 {
                match self.neighbour(packet, true) {
                    None => {
                        index = packet.frames() as i64 - 1;
                        break;
                    }
                    Some(p) => {
                        index -= packet.frames() as i64;
                        packet = p;
                    }
                }
            } else {
                break;
            }
        }
        i32::from(packet.pcm[index as usize * 2 + channel])
    }
    fn packet_sample(
        &self,
        packet: &Packet,
        at: Phase,
        playback_rate_q32: u64,
        channel: usize,
    ) -> i32 {
        let tps = u128::from(self.tps);
        let elapsed = q32(at) - packet.start();
        let position = (elapsed * u128::from(packet.rate)) >> 32;
        let source_frame = (position / tps) as u64;
        if source_frame >= packet.frames() as u64 {
            return 0;
        }
        let remainder = (position % tps) as u64;
        if playback_rate_q32 < (u64::from(self.out_hz) << 32) {
            let scaled = remainder * PHASES as u64;
            let phase = (scaled / self.tps) as usize;
            let weight = (scaled % self.tps) as f64 / self.tps as f64;
            let mut total = 0.0f64;
            for tap in 0..TAPS {
                let coefficient =
                    self.table[phase][tap] * (1.0 - weight) + self.table[phase + 1][tap] * weight;
                total += coefficient
                    * f64::from(self.frame_at(
                        packet,
                        source_frame as i64 - HALF_TAPS + 1 + tap as i64,
                        channel,
                    ));
            }
            let rounded = if total < 0.0 {
                total - 0.5
            } else {
                total + 0.5
            };
            if rounded >= 32767.0 {
                return 32767;
            }
            if rounded <= -32768.0 {
                return -32768;
            }
            return rounded as i64 as i32;
        }
        let first = i64::from(self.frame_at(packet, source_frame as i64, channel));
        let second = i64::from(self.frame_at(packet, source_frame as i64 + 1, channel));
        let weighted = first * (self.tps - remainder) as i64 + second * remainder as i64;
        (weighted / self.tps as i64) as i16 as i32
    }

    fn headroom_at(&self, at: u64) -> [u8; 32] {
        let mut h = [0u8; 32];
        if self.hr_enabled {
            h[..31].fill(1);
            for e in &self.hr_events {
                if e.ticks > at {
                    break;
                }
                h = e.amount;
            }
        }
        h
    }
}

enum BufCmd {
    Pause(bool),
    Stop,
    Frequency(u32),
    Volume(i32),
    Loop(usize, usize),
}

fn route_gain(attenuation: u16) -> u32 {
    if attenuation == 4095 {
        return 0;
    }
    if attenuation == 0 {
        return 65536;
    }
    let x = -f64::from(attenuation) * 2.302585092994046 / 1280.0 / 256.0;
    let (mut term, mut sum) = (1.0f64, 1.0f64);
    for i in 1..14 {
        term *= x / f64::from(i);
        sum += term;
    }
    for _ in 0..8 {
        sum *= sum;
    }
    (sum * 65536.0 + 0.5) as u32
}

fn submix(sample: i32, headroom: u8) -> i32 {
    let gain = 65536u32 >> (headroom & 7);
    ((i64::from(sample) * i64::from(gain) + 32768) >> 16) as i32
}

impl SoundHost for Mixer {
    fn apply(&mut self, command: &Command) -> bool {
        match command {
            Command::SubmitAdpcm {
                key,
                ticks,
                source_hz,
                mono,
                data,
            } => self.submit_adpcm(*key, *ticks, *source_hz, data, *mono),
            Command::SubmitPcm16Stereo {
                key,
                ticks,
                source_hz,
                pcm,
            } => {
                if *key == 0
                    || *source_hz == 0
                    || pcm.is_empty()
                    || pcm.len() / 2 > MAX_PACKET_BYTES / 4
                    || pcm.len() * 2 > MAX_QUEUED_PCM_BYTES - self.queued_bytes
                {
                    return false;
                }
                self.submit_decoded(*key, *ticks, *source_hz, pcm.clone())
            }
            Command::CutStream { key, ticks } => {
                if *key == 0 {
                    return false;
                }
                let Some(i) = self.stream_idx(*key) else {
                    return true;
                };
                let s = &self.streams[i];
                let Some(phase) = s.source_phase(s.active_ticks(*ticks).0) else {
                    return false;
                };
                self.cut_phase(i, *key, phase);
                true
            }
            Command::SetStreamVolume { key, millibels } => {
                let Some(g) = volume_to_gain_q16(*millibels) else {
                    return false;
                };
                if *key == 0 {
                    return false;
                }
                let Some(i) = self.stream_for(*key) else {
                    return false;
                };
                self.streams[i].attenuated = *millibels != 0;
                self.streams[i].gain_q16 = g;
                true
            }
            Command::ResetStreamVolume { key } => {
                if *key == 0 {
                    return false;
                }
                if let Some(i) = self.stream_idx(*key) {
                    self.streams[i].attenuated = false;
                    self.streams[i].gain_q16 = 65536;
                }
                true
            }
            Command::SetStreamRunning {
                key,
                ticks,
                running,
            } => {
                if *key == 0 {
                    return false;
                }
                let Some(i) = self.stream_for(*key) else {
                    return false;
                };
                let s = &mut self.streams[i];
                if *running == !s.paused {
                    return true;
                }
                if !*running {
                    s.paused = true;
                    s.pause_started = *ticks;
                    return true;
                }
                if s.pauses.len() == MAX_PAUSE_SEGMENTS || *ticks < s.pause_started {
                    return false;
                }
                s.pauses.push((s.pause_started, *ticks));
                s.paused = false;
                s.pause_started = 0;
                true
            }
            Command::SetStreamFrequency {
                key,
                ticks,
                source_hz,
            } => self.set_stream_frequency(*key, *ticks, *source_hz),
            Command::FrequencyStream {
                key,
                serial,
                ticks,
                base_hz,
                old_hz,
                new_hz,
            } => self.frequency_stream(*key, *serial, *ticks, *base_hz, *old_hz, *new_hz),
            Command::FormatStream {
                key,
                serial,
                ticks,
                old_hz,
                new_hz,
            } => self.format_stream(*key, *serial, *ticks, *old_hz, *new_hz),
            Command::RouteStream {
                key,
                serial,
                ticks,
                routing,
            } => self.route_stream(*key, *serial, *ticks, routing),
            Command::EnableHeadroom { enabled } => {
                if self.rendered != 0 {
                    return false;
                }
                self.hr_enabled = *enabled;
                self.hr_events.clear();
                self.hr_identity = 0;
                true
            }
            Command::BindHeadroom { identity } => {
                if !self.hr_enabled || *identity == 0 {
                    return false;
                }
                if self.hr_identity == *identity {
                    return true;
                }
                if !self.packets.is_empty() || !self.buffers.is_empty() {
                    return false;
                }
                self.hr_identity = *identity;
                self.hr_events.clear();
                self.hr_last = 0;
                true
            }
            Command::SetHeadroom {
                identity,
                ticks,
                bin,
                headroom,
            } => self.set_headroom(*identity, *ticks, *bin, *headroom),
            Command::PlayBuffer {
                key,
                serial,
                ticks,
                frequency,
                pcm,
                paused,
                looping,
                loop_start,
                loop_end,
                volume,
            } => self.play_buffer(
                *key,
                *serial,
                *ticks,
                *frequency,
                pcm,
                *paused,
                *looping,
                *loop_start,
                *loop_end,
                *volume,
            ),
            Command::PauseBuffer {
                key,
                serial,
                ticks,
                paused,
            } => self.change_buffer(*key, *serial, *ticks, BufCmd::Pause(*paused)),
            Command::StopBuffer { key, serial, ticks } => {
                self.change_buffer(*key, *serial, *ticks, BufCmd::Stop)
            }
            Command::FrequencyBuffer {
                key,
                serial,
                ticks,
                frequency,
            } => self.change_buffer(*key, *serial, *ticks, BufCmd::Frequency(*frequency)),
            Command::VolumeBuffer {
                key,
                serial,
                ticks,
                volume,
            } => self.change_buffer(*key, *serial, *ticks, BufCmd::Volume(*volume)),
            Command::LoopBuffer {
                key,
                serial,
                ticks,
                loop_start,
                loop_end,
            } => self.change_buffer(*key, *serial, *ticks, BufCmd::Loop(*loop_start, *loop_end)),
            Command::ResetBuffers => {
                for r in self.buffers.drain(..) {
                    self.queued_bytes -= r.pcm.len() * 2;
                }
                true
            }
        }
    }

    fn render_until(&mut self, now: u64, capacity: usize) -> Vec<i16> {
        self.render_until_with_bins(now, capacity, |_| {})
    }
}

impl Mixer {
    /// Render stereo while exposing the pre-projection mix bins at each sample time.
    /// The callback runs synchronously and must not re-enter this mixer.
    pub fn render_until_with_bins(
        &mut self,
        now: u64,
        capacity: usize,
        mut on_bins: impl FnMut(&[i64; 32]),
    ) -> Vec<i16> {
        if !self.started || now <= self.origin || capacity == 0 {
            return vec![];
        }
        let elapsed = now - self.origin;
        let target = mul_div_ceil(elapsed, u64::from(self.out_hz), self.tps);
        if target <= self.rendered {
            return vec![];
        }
        let count = ((target - self.rendered) as usize).min(capacity);
        let mut out = vec![0i16; count * 2];
        for (fi, frame) in out.chunks_mut(2).enumerate() {
            let at = self.origin
                + mul_div_floor(self.rendered + fi as u64, self.tps, u64::from(self.out_hz));
            let headroom = self.headroom_at(at);
            let (mut ml, mut mr) = (0i32, 0i32);
            let mut bins = [0i64; 32];
            for run in &self.buffers {
                let sample = self.buffer_sample(run, at);
                ml += submix(sample, headroom[0]);
                mr += submix(sample, headroom[1]);
                bins[0] += i64::from(submix(sample, headroom[0]));
                bins[1] += i64::from(submix(sample, headroom[1]));
            }
            for packet in &self.packets {
                let Some(stream) = self.streams.iter().find(|s| s.key == packet.key) else {
                    continue;
                };
                let (active, paused) = stream.active_ticks(at);
                let Some(phase) = stream.source_phase(active) else {
                    return out;
                };
                if paused || q32(phase) < packet.start() || q32(phase) >= packet.end() {
                    continue;
                }
                let mut rate_q32 = u64::from(packet.rate) << 32;
                if let Some(c) = &stream.clock {
                    for e in &c.events {
                        if e.active_ticks > active {
                            break;
                        }
                        rate_q32 = e.frequency_q32;
                    }
                }
                let mut left = self.packet_sample(packet, phase, rate_q32, 0);
                let mut right = self.packet_sample(packet, phase, rate_q32, 1);
                let route = stream.routes.iter().take_while(|r| r.ticks <= at).last();
                if let Some(r) = route {
                    for (bin, sample) in r
                        .mix_bin_samples(left, right, &headroom)
                        .into_iter()
                        .enumerate()
                    {
                        bins[bin] += sample;
                    }
                } else {
                    let gain = if stream.attenuated {
                        stream.gain_q16
                    } else {
                        65536
                    };
                    bins[0] += i64::from(submix(
                        ((i64::from(left) * i64::from(gain) + 32768) >> 16) as i32,
                        headroom[0],
                    ));
                    bins[1] += i64::from(submix(
                        ((i64::from(right) * i64::from(gain) + 32768) >> 16) as i32,
                        headroom[1],
                    ));
                }
                match route {
                    Some(r) if self.hr_enabled => {
                        let mut gain = [0u32; 4];
                        for bin in 0..6 {
                            for ch in 0..4 {
                                gain[ch] = gain[ch]
                                    .wrapping_add(r.bin_gain[bin][ch] >> (headroom[bin] & 7));
                            }
                        }
                        let (sl, sr) = (i64::from(left), i64::from(right));
                        left = ((sl * i64::from(gain[0]) + sr * i64::from(gain[1]) + 32768) >> 16)
                            as i32;
                        right = ((sl * i64::from(gain[2]) + sr * i64::from(gain[3]) + 32768) >> 16)
                            as i32;
                    }
                    Some(r) => {
                        let (sl, sr) = (i64::from(left), i64::from(right));
                        left = ((sl * i64::from(r.gain[0]) + sr * i64::from(r.gain[1]) + 32768)
                            >> 16) as i32;
                        right = ((sl * i64::from(r.gain[2]) + sr * i64::from(r.gain[3]) + 32768)
                            >> 16) as i32;
                    }
                    None => {
                        if stream.attenuated {
                            left = ((i64::from(left) * i64::from(stream.gain_q16) + 32768) >> 16)
                                as i32;
                            right = ((i64::from(right) * i64::from(stream.gain_q16) + 32768) >> 16)
                                as i32;
                        }
                        left = submix(left, headroom[0]);
                        right = submix(right, headroom[1]);
                    }
                }
                ml += left;
                mr += right;
            }
            frame[0] = ml.clamp(-32768, 32767) as i16;
            frame[1] = mr.clamp(-32768, 32767) as i16;
            on_bins(&bins);
        }
        self.rendered += count as u64;
        self.counts.frames_emitted += count as u64;
        // retire only audio whose sample points were emitted
        let next_tick = (u128::from(self.origin)
            + u128::from(self.rendered) * u128::from(self.tps) / u128::from(self.out_hz))
        .min(u128::from(u64::MAX)) as u64;
        let mut freed = 0usize;
        let snapshot: Vec<bool> = self
            .buffers
            .iter()
            .map(|run| {
                let last = run.events.last().unwrap();
                last.ticks <= next_tick
                    && !last.paused
                    && !last.looping
                    && self.buffer_position(run.pcm.len(), last, next_tick, None)
                        >= ((run.pcm.len() as u64) << 32)
            })
            .collect();
        let mut index = 0;
        self.buffers.retain_mut(|run| {
            let finished = snapshot[index];
            index += 1;
            if run.end_ticks <= next_tick || finished {
                freed += run.pcm.len() * 2;
                return false;
            }
            let mut consumed = 0;
            while consumed + 1 < run.events.len() && run.events[consumed + 1].ticks <= next_tick {
                consumed += 1;
            }
            if consumed != 0 {
                run.events.drain(..consumed);
            }
            true
        });
        self.queued_bytes -= freed;
        let mut drop_flags = Vec::with_capacity(self.packets.len());
        for p in &self.packets {
            let stream = self.streams.iter().find(|s| s.key == p.key);
            let active_now = stream.map_or(now, |s| s.active_ticks(now).0);
            let source_now = match stream {
                Some(s) => match s.source_phase(active_now) {
                    Some(ph) => ph,
                    None => return out,
                },
                None => Phase {
                    ticks: active_now,
                    fraction: 0,
                },
            };
            let history =
                u128::from(mul_div_ceil(HALF_TAPS as u64, self.tps, u64::from(p.rate))) << 32;
            drop_flags.push(q32(source_now) >= p.end() + history);
        }
        let mut index = 0;
        let mut freed = 0usize;
        self.packets.retain(|p| {
            let drop = drop_flags[index];
            index += 1;
            if drop {
                freed += p.pcm.len() * 2;
            }
            !drop
        });
        self.queued_bytes -= freed;
        out
    }
}

#[cfg(test)]
mod gp_routing_tests {
    use super::*;

    #[test]
    fn bin_feed_preserves_routed_sources_and_stereo_output() {
        let mut mixer = Mixer::new(48_000, 48_000);
        let routing = Routing {
            volume: 0,
            headroom: 0,
            aggregate: 0,
            count: 2,
            bins: [2, 4, 0, 0, 0, 0, 0, 0],
            bin_volume: [0; 32],
        };
        assert!(mixer.route_stream(1, 1, 0, &routing));
        assert!(mixer.apply(&Command::SubmitPcm16Stereo {
            key: 1,
            ticks: 0,
            source_hz: 48_000,
            pcm: vec![1000, -400, 1000, -400],
        }));
        let mut samples = Vec::new();
        let pcm = mixer.render_until_with_bins(2, 2, |bins| samples.push(*bins));
        assert_eq!(samples.len(), 2);
        for bins in samples {
            assert_eq!((bins[2], bins[4]), (1000, -400));
            assert_eq!(bins.iter().sum::<i64>(), 600);
        }
        assert_eq!(pcm, vec![424, 707, 424, 707]);
    }

    #[test]
    fn source_gains_keep_mix_bins_and_channels_before_projection() {
        let mut mixer = Mixer::new(48_000, 48_000);
        let mut routing = Routing {
            volume: 0,
            headroom: 0,
            aggregate: 0,
            count: 4,
            bins: [2, 4, 2, 5, 0, 0, 0, 0],
            bin_volume: [0; 32],
        };
        assert!(mixer.route_stream(1, 1, 0, &routing));
        let event = mixer.streams[0].routes[0];
        assert_eq!(event.source_bin_gain[2], [65536, 65536]);
        assert_eq!(event.source_bin_gain[4], [65536, 0]);
        assert_eq!(event.source_bin_gain[5], [0, 65536]);
        assert_eq!(event.gain, [92682, 46341, 46341, 92682]);
        routing.bins[0] = 6;
        assert!(!mixer.route_stream(1, 1, 1, &routing));
        assert_eq!(mixer.streams[0].routes.len(), 1);
    }

    #[test]
    fn routed_samples_remain_separate_and_apply_per_bin_headroom() {
        let mut mixer = Mixer::new(48_000, 48_000);
        let routing = Routing {
            volume: 0,
            headroom: 0,
            aggregate: 0,
            count: 4,
            bins: [2, 4, 2, 5, 0, 0, 0, 0],
            bin_volume: [0; 32],
        };
        assert!(mixer.route_stream(1, 1, 0, &routing));
        let event = mixer.streams[0].routes[0];
        let mut headroom = [0; 32];
        headroom[2] = 1;
        headroom[5] = 2;
        assert_eq!(
            event.mix_bin_samples(1000, -400, &headroom),
            [0, 0, 300, 0, 1000, -100]
        );
    }
}
