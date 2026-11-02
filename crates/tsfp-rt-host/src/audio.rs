// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136 slice 2: the audio thread of the in-process runtime (design: docs/t-rust-hosted-dispatcher.md section 6).
//!
//! An [`AudioRuntime`] owns one thread, "tsfp-audio", that owns a `tsfp_audio::mixer::Mixer` and a sink. It is fed whole text
//! lines of the `TSFP_AUDIO_CMD_LOG` grammar (`START`, mixer commands, `R now capacity frames hash`) through a bounded queue.
//! The guest thread only appends bytes and hands over whole-line batches, it never mixes and never touches the sink.
//!
//! Back-pressure policy (audio flavour of the render policy): the command stream is cumulative mixer state, so a dropped command
//! cannot be repaired. A full queue stalls the producer by polling, at most `stall_limit` per hand-over and `stall_total` over the
//! run. Past either the audio duty is **switched off** (counted, one stderr line), the queue closes and the guest runs on in
//! silence. Underrun is never a stall: [`Ring::pull`] (the device side) pads silence and counts it; an overfull ring drops the
//! excess and counts it.
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tsfp_audio::contract::{AudioSink, NullSink, SoundHost};
use tsfp_audio::line::{Line, parse_line};
use tsfp_audio::mixer::Mixer;
use tsfp_audio::sink::{Ring, WavSink};

/// Where the thread sends rendered PCM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SinkKind {
    /// Count frames only.
    Null,
    /// Canonical 16 bit stereo WAV at the START rate (written on the audio thread, header patched at finish).
    Wav(PathBuf),
    /// Bounded pull ring of interleaved samples (what a device callback drains with [`AudioRuntime::pull`]).
    Ring(usize),
}

pub struct AudioOptions {
    pub sink: SinkKind,
    pub queue_batches: usize,
    pub batch_bytes: usize,
    pub stall_limit: Option<Duration>,
    pub stall_total: Option<Duration>,
    /// Test hook: the audio thread sleeps this long per batch (a stalled audio thread).
    pub thread_delay: Duration,
}

impl Default for AudioOptions {
    fn default() -> Self {
        Self {
            sink: SinkKind::Null,
            queue_batches: 256,
            batch_bytes: 64 * 1024,
            stall_limit: Some(Duration::from_millis(2000)),
            stall_total: Some(Duration::from_secs(30)),
            thread_delay: Duration::ZERO,
        }
    }
}

/// Parse `--sink null|wav=PATH|ring=SAMPLES`, `--queue N`, `--batch-bytes N`, `--stall-ms N`, `--stall-total-ms N` (0 =
/// unlimited), `--thread-delay-ms N`.
pub fn parse_audio_args(args: &[String]) -> Result<AudioOptions, String> {
    let mut options = AudioOptions::default();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        index += 1;
        let value = args
            .get(index)
            .cloned()
            .ok_or(format!("{arg} needs a value"))?;
        index += 1;
        let number = |text: &str| {
            text.parse::<u64>()
                .map_err(|_| format!("{arg}: bad number {text}"))
        };
        match arg {
            "--sink" => {
                options.sink = match value.split_once('=') {
                    None if value == "null" => SinkKind::Null,
                    Some(("wav", path)) => SinkKind::Wav(PathBuf::from(path)),
                    Some(("ring", n)) => SinkKind::Ring(number(n)? as usize),
                    _ => {
                        return Err(format!(
                            "--sink: expected null, wav=PATH or ring=SAMPLES, got {value}"
                        ));
                    }
                }
            }
            "--queue" => options.queue_batches = number(&value)?.max(1) as usize,
            "--batch-bytes" => options.batch_bytes = number(&value)?.max(64) as usize,
            "--stall-ms" => {
                let ms = number(&value)?;
                options.stall_limit = (ms != 0).then(|| Duration::from_millis(ms));
            }
            "--stall-total-ms" => {
                let ms = number(&value)?;
                options.stall_total = (ms != 0).then(|| Duration::from_millis(ms));
            }
            "--thread-delay-ms" => options.thread_delay = Duration::from_millis(number(&value)?),
            other => return Err(format!("unknown audio argument {other}")),
        }
    }
    Ok(options)
}

/// Counters, readable while the runtime runs.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioStats {
    // producer side
    pub batches_sent: u64,
    pub batches_dropped: u64,
    pub stalls: u64,
    pub stall_ns_total: u64,
    pub stall_ns_max: u64,
    pub gave_up: bool,
    // audio thread side
    pub lines: u64,
    pub commands: u64,
    /// Commands whose acceptance differs from the verdict the C host logged.
    pub verdict_mismatch: u64,
    /// Lines of the completion model, not a mixer concern, skipped.
    pub completion_lines_skipped: u64,
    pub renders: u64,
    pub render_equal: u64,
    pub render_diff: u64,
    pub frames: u64,
    /// FNV-1a over all PCM bytes written to the sink, in order (determinism check).
    pub pcm_hash: u64,
    pub ring_overruns: u64,
    pub ring_underruns: u64,
    pub sink_error: bool,
}

const FNV_BASIS: u64 = 1469598103934665603;

fn fnv_extend(mut h: u64, pcm: &[i16]) -> u64 {
    for s in pcm {
        for b in s.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(1099511628211);
        }
    }
    h
}

struct Producer {
    sender: Option<SyncSender<Vec<u8>>>,
    pending: Vec<u8>,
    stats: AudioStats,
    dead: bool,
}

struct RingSink(Arc<Mutex<Ring>>);
impl AudioSink for RingSink {
    fn write(&mut self, samples: &[i16]) {
        self.0.lock().unwrap().write(samples);
    }
}

pub struct AudioRuntime {
    batch_bytes: usize,
    stall_limit: Option<Duration>,
    stall_total: Option<Duration>,
    producer: Mutex<Producer>,
    shared: Arc<Mutex<AudioStats>>,
    ring: Option<Arc<Mutex<Ring>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

/// Thread-side state: the mixer, the sink and the counters it owns.
struct Worker {
    mixer: Option<Mixer>,
    sink: Box<dyn AudioSink + Send>,
    wav: Option<SharedWavSink>,
    ring: Option<Arc<Mutex<Ring>>>,
    stats: AudioStats,
}

impl Worker {
    fn line(&mut self, raw: &str) {
        self.stats.lines += 1;
        match parse_line(raw) {
            Line::Start {
                rate,
                ticks_per_second,
            } => {
                self.mixer = Some(Mixer::new(rate, ticks_per_second));
            }
            Line::Command { command, c_ok, .. } => {
                self.stats.commands += 1;
                let got = match (&command, self.mixer.as_mut()) {
                    (Some(command), Some(mixer)) => mixer.apply(command),
                    _ => false,
                };
                if let Some(want) = c_ok {
                    if got != want {
                        self.stats.verdict_mismatch += 1;
                    }
                }
            }
            Line::Render {
                now,
                capacity,
                frames,
                hash,
            } => {
                let Some(mixer) = self.mixer.as_mut() else {
                    return;
                };
                let pcm = mixer.render_until(now, capacity);
                self.stats.renders += 1;
                let same = pcm.len() / 2 == frames && fnv_extend(FNV_BASIS, &pcm) == hash;
                if same {
                    self.stats.render_equal += 1;
                } else {
                    self.stats.render_diff += 1;
                }
                if !pcm.is_empty() {
                    self.sink.write(&pcm);
                    self.stats.frames += (pcm.len() / 2) as u64;
                    self.stats.pcm_hash = fnv_extend(self.stats.pcm_hash, &pcm);
                }
            }
            Line::Completion => self.stats.completion_lines_skipped += 1,
            Line::Other => {}
        }
    }
}

fn audio_loop(
    mut worker: Worker,
    receiver: Receiver<Vec<u8>>,
    shared: Arc<Mutex<AudioStats>>,
    delay: Duration,
) {
    for batch in receiver {
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        for raw in batch.split(|&b| b == b'\n') {
            if raw.is_empty() {
                continue;
            }
            worker.line(&String::from_utf8_lossy(raw));
        }
        if let Some(ring) = &worker.ring {
            let ring = ring.lock().unwrap();
            worker.stats.ring_overruns = ring.overruns;
            worker.stats.ring_underruns = ring.underruns;
        }
        *shared.lock().unwrap() = worker.stats;
    }
    if let Some(wav) = worker
        .wav
        .take()
        .and_then(|slot| slot.lock().unwrap().take())
    {
        let _ = wav.finish();
    }
    if let Some(ring) = &worker.ring {
        let ring = ring.lock().unwrap();
        worker.stats.ring_overruns = ring.overruns;
        worker.stats.ring_underruns = ring.underruns;
    }
    *shared.lock().unwrap() = worker.stats;
}

/// A WAV sink shared with the thread so `finish` can patch the header after the last write.
struct SharedWav(Arc<Mutex<Option<WavSink<BufWriter<File>>>>>);
impl AudioSink for SharedWav {
    fn write(&mut self, samples: &[i16]) {
        if let Some(wav) = self.0.lock().unwrap().as_mut() {
            wav.write(samples);
        }
    }
}

impl AudioRuntime {
    pub fn start(options: AudioOptions) -> Result<Self, String> {
        let (sender, receiver) = sync_channel::<Vec<u8>>(options.queue_batches);
        let shared = Arc::new(Mutex::new(AudioStats {
            pcm_hash: FNV_BASIS,
            ..AudioStats::default()
        }));
        let mut ring = None;
        let mut wav_slot = None;
        let mut sink: Box<dyn AudioSink + Send> = Box::new(NullSink::default());
        match &options.sink {
            SinkKind::Null => {}
            SinkKind::Ring(capacity) => {
                let shared_ring = Arc::new(Mutex::new(Ring::new(*capacity)));
                sink = Box::new(RingSink(shared_ring.clone()));
                ring = Some(shared_ring);
            }
            SinkKind::Wav(path) => {
                // The rate is only known at START, 48 kHz is the C host's output rate; the header is rewritten at finish.
                let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
                let wav = WavSink::new(BufWriter::new(file), 48000, 2)
                    .map_err(|e| format!("wav: {e}"))?;
                let slot = Arc::new(Mutex::new(Some(wav)));
                sink = Box::new(SharedWav(slot.clone()));
                wav_slot = Some(slot);
            }
        }
        let worker = Worker {
            mixer: None,
            sink,
            wav: wav_slot,
            ring: ring.clone(),
            stats: AudioStats {
                pcm_hash: FNV_BASIS,
                ..AudioStats::default()
            },
        };
        let (thread_shared, delay) = (shared.clone(), options.thread_delay);
        let thread = std::thread::Builder::new()
            .name("tsfp-audio".into())
            .spawn(move || audio_loop(worker, receiver, thread_shared, delay))
            .map_err(|e| format!("spawn: {e}"))?;
        Ok(Self {
            batch_bytes: options.batch_bytes,
            stall_limit: options.stall_limit,
            stall_total: options.stall_total,
            producer: Mutex::new(Producer {
                sender: Some(sender),
                pending: Vec::with_capacity(options.batch_bytes + 4096),
                stats: AudioStats::default(),
                dead: false,
            }),
            shared,
            ring,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// Append log bytes (any chunking). Whole lines go to the audio thread once a batch is full. One call waits at most the stall limit.
    pub fn write(&self, bytes: &[u8]) {
        let mut p = self.producer.lock().unwrap();
        if p.dead {
            return;
        }
        p.pending.extend_from_slice(bytes);
        if p.pending.len() >= self.batch_bytes {
            self.hand_over(&mut p);
        }
    }

    pub fn send_line(&self, line: &str) {
        self.write(&crate::newline_bytes(line));
    }

    /// Hand every complete line to the audio thread now.
    pub fn flush(&self) {
        let mut p = self.producer.lock().unwrap();
        if !p.dead {
            self.hand_over(&mut p);
        }
    }

    fn hand_over(&self, p: &mut Producer) {
        let Some(cut) = p.pending.iter().rposition(|&b| b == b'\n').map(|i| i + 1) else {
            return;
        };
        let tail = p.pending.split_off(cut);
        let mut batch = std::mem::replace(&mut p.pending, tail);
        let Some(sender) = p.sender.clone() else {
            return;
        };
        let started = Instant::now();
        let mut waited = false;
        loop {
            match sender.try_send(batch) {
                Ok(()) => {
                    p.stats.batches_sent += 1;
                    break;
                }
                Err(TrySendError::Full(back)) => {
                    waited = true;
                    let spent = Duration::from_nanos(p.stats.stall_ns_total) + started.elapsed();
                    if self
                        .stall_limit
                        .is_some_and(|limit| started.elapsed() >= limit)
                        || self.stall_total.is_some_and(|total| spent >= total)
                    {
                        p.stats.batches_dropped += 1;
                        p.stats.gave_up = true;
                        p.dead = true;
                        p.sender = None;
                        eprintln!(
                            "rt-host: audio queue full for {:?}, audio switched off (the command stream cannot be repaired)",
                            started.elapsed()
                        );
                        break;
                    }
                    batch = back;
                    std::thread::sleep(Duration::from_micros(200));
                }
                Err(TrySendError::Disconnected(_)) => {
                    p.stats.batches_dropped += 1;
                    p.stats.gave_up = true;
                    p.dead = true;
                    p.sender = None;
                    break;
                }
            }
        }
        if waited {
            let ns = started.elapsed().as_nanos() as u64;
            p.stats.stalls += 1;
            p.stats.stall_ns_total += ns;
            p.stats.stall_ns_max = p.stats.stall_ns_max.max(ns);
        }
    }

    /// Device side of a [`SinkKind::Ring`] sink: fill `out`, pad silence on underrun (counted). Returns the real samples copied,
    /// 0 without a ring sink (and `out` is silence).
    pub fn pull(&self, out: &mut [i16]) -> usize {
        match &self.ring {
            Some(ring) => ring.lock().unwrap().pull(out),
            None => {
                out.fill(0);
                0
            }
        }
    }

    pub fn stats(&self) -> AudioStats {
        let p = self.producer.lock().unwrap().stats;
        let mut stats = *self.shared.lock().unwrap();
        stats.batches_sent = p.batches_sent;
        stats.batches_dropped = p.batches_dropped;
        stats.stalls = p.stalls;
        stats.stall_ns_total = p.stall_ns_total;
        stats.stall_ns_max = p.stall_ns_max;
        stats.gave_up = p.gave_up;
        if let Some(ring) = &self.ring {
            let ring = ring.lock().unwrap();
            stats.ring_overruns = ring.overruns;
            stats.ring_underruns = ring.underruns;
        }
        stats
    }

    /// Flush, close the queue, wait for the thread (the WAV header is patched), return the final counters.
    pub fn finish(&self) -> AudioStats {
        {
            let mut p = self.producer.lock().unwrap();
            if !p.dead {
                p.pending.push(b'\n');
                self.hand_over(&mut p);
            }
            p.sender = None;
        }
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
        self.stats()
    }
}

/// The sink remains shared so shutdown can finish the same WAV writer.
type SharedWavSink = Arc<Mutex<Option<WavSink<BufWriter<File>>>>>;
