// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136: the in-process render runtime of the hybrid host (design: docs/t-rust-hosted-dispatcher.md).
//!
//! A [`Runtime`] owns one render thread. That thread owns the `tsfp_wgpu::live::LiveRenderer` (and so every wgpu object) and is
//! fed whole stream lines through a bounded queue, instead of the stdin pipe of a subprocess. The guest thread only appends
//! bytes to a small buffer and hands full batches to the queue. The C ABI is in [`ffi`].
pub mod animation;
pub mod animation_import;
pub mod audio;
pub mod ffi;
pub mod ffi_audio;
mod ring;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, Sender, SyncSender, TrySendError, channel, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use tsfp_wgpu::binary::{BinaryError, event_of};
use tsfp_wgpu::context::Preference;
use tsfp_wgpu::live::{DumpPlan, LiveConfig, LiveRenderer};
use tsfp_wgpu::trace_wire::{FRAME_DRAWS_OMITTED, Reader, Record};

/// Runtime options (the live example's flags plus the queue policy).
pub struct Options {
    pub live: LiveConfig,
    pub quiet: bool,
    /// Queue depth in batches (each batch is at least `batch_bytes` of whole lines, except at a flush).
    pub queue_batches: usize,
    pub batch_bytes: usize,
    /// Opt-in legacy policy: with a limit set the queue is bounded to `queue_batches` and the guest thread waits at most this long
    /// for room in one hand-over, then the live renderer is switched off (the stream is cumulative state, a gap cannot be repaired)
    /// and the guest runs on. `None` (the default, together with `stall_total` `None`) makes the queue unbounded: the guest
    /// thread never waits, a slow renderer skips frames (`skip_behind`) and catches up.
    pub stall_limit: Option<Duration>,
    /// Cumulative wait the guest may spend over the whole run (a renderer that is merely slower than the game, every wait
    /// short, still ends the run's slowdown). `None` is unlimited.
    pub stall_total: Option<Duration>,
    /// Test hook: the render thread sleeps this long per batch (a slow renderer).
    pub render_delay: Duration,
    /// Keep the newest rendered frame for [`Runtime::take_frame`] (a window shows it). Off: frames are dropped after the PNG plan.
    pub publish_frames: bool,
    /// Suppress publication while more than this many completed frames wait in the queue. Surface writes and clears still run
    /// so later frames can sample them. `None` publishes every present.
    pub skip_behind: Option<u64>,
    /// Memory cap of the unbounded queue (bytes of stream text waiting for the renderer). Past it the live renderer is switched
    /// off, because memory exhaustion is worse than a missing picture. Only the unbounded queue uses it.
    pub queue_bytes_cap: u64,
    /// Legacy producer omission threshold. Retained for argument compatibility; omission is disabled because later frames can
    /// sample a surface written by an earlier frame.
    pub drop_behind: Option<u64>,
    /// ... or while more than this many bytes of stream wait in the queue.
    pub drop_queue_bytes: u64,
    /// Worker threads that compile render pipelines (draws wait for them instead of stalling the render thread). 0: build on the
    /// render thread. The C ABI create sets 2 for the live display (`--sync-pipelines` turns it off).
    pub async_pipelines: usize,
    /// Print the counters to stderr this often (`TSFP_RT_HOST_STATS=1` in the C ABI create sets 5 s).
    pub stats_every: Option<Duration>,
    /// T2136d measurement mode (`--discard`): the render thread drops every batch after counting its presents, so only the guest
    /// side cost of producing and handing over the stream is left.
    pub discard: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            live: LiveConfig::default(),
            quiet: false,
            queue_batches: 64,
            batch_bytes: 256 * 1024,
            stall_limit: None,
            stall_total: None,
            render_delay: Duration::ZERO,
            publish_frames: false,
            skip_behind: None,
            queue_bytes_cap: 64 << 20,
            drop_behind: None,
            drop_queue_bytes: 8 << 20,
            async_pipelines: 0,
            stats_every: None,
            discard: false,
        }
    }
}

/// Parse the arguments of the live example (`--png-base`, `--dump-at`, ...) plus `--queue`, `--batch-bytes`, `--stall-ms`.
pub fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        index += 1;
        let mut take = || -> Result<String, String> {
            let found = args
                .get(index)
                .cloned()
                .ok_or(format!("{arg} needs a value"))?;
            index += 1;
            Ok(found)
        };
        fn number(text: &str, name: &str) -> Result<u64, String> {
            text.parse()
                .map_err(|_| format!("{name}: bad number {text}"))
        }
        match arg {
            "--png-base" => options.live.dump.base = Some(PathBuf::from(take()?)),
            "--dump-at" => {
                let n = number(&take()?, arg)?;
                options.live.dump.at.insert(n);
            }
            "--dump-every" => options.live.dump.every = true,
            "--from" => options.live.from = number(&take()?, arg)?,
            "--to" => options.live.to = number(&take()?, arg)?,
            "--size" => {
                let text = take()?;
                let (w, h) = text.split_once('x').ok_or("--size needs WxH")?;
                options.live.width = w.parse().map_err(|_| "--size: bad width")?;
                options.live.height = h.parse().map_err(|_| "--size: bad height")?;
            }
            "--opt" => {
                let text = take()?;
                let (name, value) = text.split_once('=').unwrap_or((text.as_str(), "1"));
                options
                    .live
                    .knobs
                    .push((name.to_string(), value.to_string()));
            }
            "--output" => {
                let text = take()?;
                let (w, h) = text.split_once('x').ok_or("--output needs WxH")?;
                options.live.output = Some((
                    w.parse().map_err(|_| "--output: bad width")?,
                    h.parse().map_err(|_| "--output: bad height")?,
                ));
            }
            // integer scale of the logical size, 1 to 8
            "--scale" => {
                let factor = number(&take()?, arg)?.clamp(1, 8) as u32;
                options.live.output =
                    Some((options.live.width * factor, options.live.height * factor));
            }
            // T2095: window pixels for the present pass (sharp bilinear, aspect kept, black bars)
            "--window" => {
                let text = take()?;
                let (w, h) = text.split_once('x').ok_or("--window needs WxH")?;
                options.live.window = Some((
                    w.parse().map_err(|_| "--window: bad width")?,
                    h.parse().map_err(|_| "--window: bad height")?,
                ));
            }
            "--publish" => options.publish_frames = true,
            // handled by the C ABI create (live display defaults to skip-behind), accepted here so the text parses
            "--no-skip" => {}
            "--skip-behind" => options.skip_behind = Some(number(&take()?, arg)?),
            "--drop-behind" => {
                let frames = number(&take()?, arg)?;
                options.drop_behind = (frames != 0).then_some(frames);
            }
            "--async-pipelines" => options.async_pipelines = number(&take()?, arg)? as usize,
            "--sync-pipelines" => options.async_pipelines = 0,
            "--drop-queue-mb" => options.drop_queue_bytes = number(&take()?, arg)?.max(1) << 20,
            "--queue-mb" => options.queue_bytes_cap = number(&take()?, arg)?.max(1) << 20,
            "--stats-ms" => {
                let ms = number(&take()?, arg)?;
                options.stats_every = (ms != 0).then(|| Duration::from_millis(ms));
            }
            "--any-adapter" => options.live.preference = Preference::Any,
            // T2136d: software (CPU) adapter only on request, the deterministic hash tests pass it
            "--software" => options.live.preference = Preference::Software,
            "--adapter" => {
                let text = take()?;
                options.live.preference = text
                    .parse()
                    .map_or(Preference::Named(text), Preference::Index);
            }
            "--quiet" => options.quiet = true,
            "--discard" => options.discard = true,
            "--queue" => options.queue_batches = number(&take()?, arg)?.max(1) as usize,
            "--batch-bytes" => options.batch_bytes = number(&take()?, arg)?.max(64) as usize,
            "--stall-ms" => {
                let ms = number(&take()?, arg)?;
                options.stall_limit = (ms != 0).then(|| Duration::from_millis(ms));
            }
            "--stall-total-ms" => {
                let ms = number(&take()?, arg)?;
                options.stall_total = (ms != 0).then(|| Duration::from_millis(ms));
            }
            "--render-delay-ms" => {
                options.render_delay = Duration::from_millis(number(&take()?, arg)?)
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if options.live.dump.base.is_none() {
        options.live.dump = DumpPlan::default();
    }
    Ok(options)
}

/// Counters, readable while the runtime runs.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub batches_sent: u64,
    pub batches_dropped: u64,
    /// Number of hand-overs that had to wait, and the total and longest time the guest thread waited for queue room.
    pub stalls: u64,
    pub stall_ns_total: u64,
    pub stall_ns_max: u64,
    /// The stall limit was hit (or the render thread died) and the renderer was switched off.
    pub gave_up: bool,
    pub lines_rendered: u64,
    pub presents: u64,
    pub frames_rendered: u64,
    pub render_error: bool,
    /// Presents not drawn because the renderer was behind (`skip_behind`).
    pub frames_skipped: u64,
    /// Frames handed to [`Runtime::take_frame`] and never taken because a newer one replaced them.
    pub frames_replaced: u64,
    /// CPU latest-frame mailbox lock and replacement, including lock contention.
    pub publish_ns: u64,
    pub publish_count: u64,
    pub take_frame_ns: u64,
    pub take_frame_count: u64,
    pub readback_wait_ns: u64,
    pub readback_waits: u64,
    pub map_callback_ns: u64,
    pub row_copy_ns: u64,
    /// Presents of frames whose draws the guest omitted (it was told the renderer is behind).
    pub frames_omitted: u64,
    /// Render pipelines built, the longest build in nanoseconds, and draws skipped while a pipeline was still being built.
    pub pipelines_built: u64,
    pub pipeline_ns_max: u64,
    pub draws_deferred: u64,
    /// T2294: draws of skipped presents omitted because their target surface is never read back.
    pub unread_draws_skipped: u64,
    /// The slowest frame the renderer built, in nanoseconds, and how it split: textures, bind groups and pipelines, encode, submit.
    pub worst_frame_ns: u64,
    pub worst_frame_phases: [u64; 4],
    /// Nonblocking ring publication (memcpy and atomic release), total CPU cost.
    pub ring_publish_ns: u64,
    pub ring_publish_count: u64,
    /// Renderer busy time per present: every event feed, drawn or skipped, first 65,536 presents.
    /// Excludes queue waiting and binary event conversion before feed; draw phases cover a narrower scope.
    pub busy_ns_p50: u64,
    pub busy_ns_p99: u64,
    pub busy_ns_max: u64,
    /// Guest side cost of handing a present over (the `present` call: two lines, flush, enqueue), nanoseconds.
    pub enqueue_ns_p50: u64,
    pub enqueue_ns_p99: u64,
    pub enqueue_ns_max: u64,
    /// Frames queued and not yet finished by the renderer, now and at the peak; and the peak of queued stream bytes.
    pub queue_frames: u64,
    pub queue_frames_max: u64,
    pub queue_bytes_max: u64,
    /// T2136f: where the renderer's time goes (nanoseconds summed over drawn frames) and how often a present was a spike.
    pub phase_textures_ns: u64,
    pub phase_bind_groups_ns: u64,
    pub phase_pipelines_ns: u64,
    pub phase_encode_ns: u64,
    pub phase_submit_ns: u64,
    pub phase_readback_ns: u64,
    pub pipelines_created: u64,
    pub texture_uploads: u64,
    pub texture_upload_batches: u64,
    pub texture_staging_bytes: u64,
    /// Presents whose renderer busy time was over 16 ms / over 100 ms.
    pub spikes_16ms: u64,
    pub spikes_100ms: u64,
    /// Milliseconds spent per stream item kind (`tsfp_wgpu::live::EVENT_KINDS`) and the item counts.
    pub decodes: u64,
    pub decode_ms: u64,
    pub texel_bytes: u64,
    pub texel_bytes_changed: u64,
    pub decode_why: [u64; 3],
    pub decode_repeat: u64,
    pub wait_ms: u64,
    pub finish_ms: u64,
    pub queue_submit_ms: u64,
    pub waits: u64,
    pub gpu_latency_avg_us: u64,
    pub gpu_latency_max_us: u64,
    pub gpu_completion_observations: u64,
    pub event_ms: [u64; 8],
    pub event_count: [u64; 8],
}

impl Stats {
    /// One line for stderr.
    pub fn summary(&self) -> String {
        let summary = format!(
            "{} presents, {} drawn, {} skipped, {} omitted by the guest, {} unread draws pruned, {} pipelines (longest {} ms, {} draws deferred), worst frame {} ms (textures {}, bind groups and pipelines {}, encode {}, submit {}), queue {} frames (peak {}, {} KiB peak), enqueue p50 {} us p99 {} us max {} us, renderer busy per present p50 {} us p99 {} us max {} us, {} stalls (total {} us){}",
            self.presents,
            self.frames_rendered,
            self.frames_skipped,
            self.frames_omitted,
            self.unread_draws_skipped,
            self.pipelines_built,
            self.pipeline_ns_max / 1_000_000,
            self.draws_deferred,
            self.worst_frame_ns / 1_000_000,
            self.worst_frame_phases[0] / 1_000_000,
            self.worst_frame_phases[1] / 1_000_000,
            self.worst_frame_phases[2] / 1_000_000,
            self.worst_frame_phases[3] / 1_000_000,
            self.queue_frames,
            self.queue_frames_max,
            self.queue_bytes_max / 1024,
            self.enqueue_ns_p50 / 1000,
            self.enqueue_ns_p99 / 1000,
            self.enqueue_ns_max / 1000,
            self.busy_ns_p50 / 1000,
            self.busy_ns_p99 / 1000,
            self.busy_ns_max / 1000,
            self.stalls,
            self.stall_ns_total / 1000,
            if self.gave_up { ", GAVE UP" } else { "" }
        );
        format!(
            "{summary}, ring publish {} ms / {} batches",
            self.ring_publish_ns / 1_000_000,
            self.ring_publish_count
        ) + &self.phase_summary()
    }

    /// T2136f: second part of the line, the render thread phases (ms over the drawn frames) and the spike counts.
    fn phase_summary(&self) -> String {
        format!(
            " | render phases ms: textures {} ({} uploads, {} coalesced batches / {} padded bytes), bind groups {} (pipelines {}, {} created), encode {}, submit {} (readback {}), spikes >16 ms {}, >100 ms {}",
            self.phase_textures_ns / 1_000_000,
            self.texture_uploads,
            self.texture_upload_batches,
            self.texture_staging_bytes,
            self.phase_bind_groups_ns / 1_000_000,
            self.phase_pipelines_ns / 1_000_000,
            self.pipelines_created,
            self.phase_encode_ns / 1_000_000,
            self.phase_submit_ns / 1_000_000,
            self.phase_readback_ns / 1_000_000,
            self.spikes_16ms,
            self.spikes_100ms
        ) + &format!(
            " | texture decodes {} ({} ms; new record {}, header changed {}, texels changed {}, content seen before {}), texel bytes written {} MiB, really changed {} MiB",
            self.decodes,
            self.decode_ms,
            self.decode_why[0],
            self.decode_why[1],
            self.decode_why[2],
            self.decode_repeat,
            self.texel_bytes >> 20,
            self.texel_bytes_changed >> 20
        ) + &format!(
            " | GPU: finish {} ms, queue.submit {} ms, waited {} ms in {} waits, submit-to-observed completion (2 ms idle poll; not GPU time) avg {} us max {} us / {} observations",
            self.finish_ms,
            self.queue_submit_ms,
            self.wait_ms,
            self.waits,
            self.gpu_latency_avg_us,
            self.gpu_latency_max_us,
            self.gpu_completion_observations
        ) + &format!(
            " | CPU handoff: readback wait {} us/{} waits, callback {} us, row copy {} us, publish {} us/{} frames, take {} us/{} calls",
            self.readback_wait_ns / 1000,
            self.readback_waits,
            self.map_callback_ns / 1000,
            self.row_copy_ns / 1000,
            self.publish_ns / 1000,
            self.publish_count,
            self.take_frame_ns / 1000,
            self.take_frame_count
        ) + &format!(" | items ms (count):{}", {
            let mut text = String::new();
            for (index, name) in tsfp_wgpu::live::EVENT_KINDS.iter().enumerate() {
                text += &format!(
                    " {name} {} ({})",
                    self.event_ms[index], self.event_count[index]
                );
            }
            text
        })
    }
}

/// Nearest rank percentile of an unsorted sample list, 0 when empty.
fn percentile(samples: &mut [u64], percent: usize) -> u64 {
    samples.sort_unstable();
    match samples.len() {
        0 => 0,
        count => samples[(count * percent / 100).min(count - 1)],
    }
}

/// A rendered frame: RGBA8, `width` x `height`, the host's present number.
#[derive(Clone, Debug)]
pub struct PublishedFrame {
    pub width: u32,
    pub height: u32,
    pub present: u64,
    pub rgba: Vec<u8>,
}

#[derive(Default)]
struct Shared {
    /// lines, presents, frames, error
    render: Mutex<(u64, u64, u64, bool)>,
    /// Presents queued by the guest and finished by the renderer: their difference is how many frames the renderer is behind.
    presents_sent: AtomicU64,
    presents_done: AtomicU64,
    queued_bytes: AtomicU64,
    queued_bytes_max: AtomicU64,
    queue_frames_max: AtomicU64,
    skipped: AtomicU64,
    omitted: AtomicU64,
    /// pipelines built, longest build (ns), draws deferred
    pipelines: Mutex<(u64, u64, u64, u64)>,
    worst: Mutex<(u64, [u64; 4])>,
    replaced: AtomicU64,
    publish_ns: AtomicU64,
    publish_count: AtomicU64,
    take_frame_ns: AtomicU64,
    take_frame_count: AtomicU64,
    /// The newest rendered frame, latest wins.
    frame: Mutex<Option<PublishedFrame>>,
    gpu_frames: Mutex<Option<Arc<Mutex<tsfp_wgpu::shared::SharedRing>>>>,
    /// Requested output size, `width << 32 | height`, 0 for none (T2136e runtime resize, applied by the render thread at the next
    /// present boundary).
    output_request: AtomicU64,
    /// Requested window (present target) size, `width << 32 | height`, 0 for none, same hand-off as `output_request` (T2095).
    window_request: AtomicU64,
    /// Renderer busy time per present and guest enqueue time per present, nanoseconds (first 65,536 each).
    ring_publish_ns: AtomicU64,
    ring_publish_count: AtomicU64,
    busy: Mutex<Vec<u64>>,
    enqueue: Mutex<Vec<u64>>,
    /// T2136f: the renderer's phase counters, copied after every present, and the spike counts.
    phases: Mutex<(tsfp_wgpu::replay::PhaseTimings, [u64; 8], [u64; 8])>,
    spikes_16ms: AtomicU64,
    spikes_100ms: AtomicU64,
    /// T2295: live display is closing, the render thread drops what is still queued instead of drawing frames nobody sees
    /// (a software adapter can be hundreds of frames behind, which held the window close for minutes).
    closing: AtomicBool,
    closing_dropped: AtomicU64,
}

impl Shared {
    fn counters(&self) -> Stats {
        let mut stats = Stats::default();
        {
            let counters = self.render.lock().unwrap();
            stats.lines_rendered = counters.0;
            stats.presents = counters.1;
            stats.frames_rendered = counters.2;
            stats.render_error = counters.3;
        }
        stats.frames_skipped = self.skipped.load(Relaxed);
        stats.frames_replaced = self.replaced.load(Relaxed);
        stats.publish_ns = self.publish_ns.load(Relaxed);
        stats.publish_count = self.publish_count.load(Relaxed);
        stats.take_frame_ns = self.take_frame_ns.load(Relaxed);
        stats.take_frame_count = self.take_frame_count.load(Relaxed);
        stats.frames_omitted = self.omitted.load(Relaxed);
        {
            let pipelines = self.pipelines.lock().unwrap();
            stats.pipelines_built = pipelines.0;
            stats.pipeline_ns_max = pipelines.1;
            stats.draws_deferred = pipelines.2;
            stats.unread_draws_skipped = pipelines.3;
        }
        {
            let worst = self.worst.lock().unwrap();
            stats.worst_frame_ns = worst.0;
            stats.worst_frame_phases = worst.1;
        }
        stats.queue_frames = self
            .presents_sent
            .load(Relaxed)
            .saturating_sub(self.presents_done.load(Relaxed));
        stats.queue_frames_max = self.queue_frames_max.load(Relaxed);
        stats.queue_bytes_max = self.queued_bytes_max.load(Relaxed);
        stats.ring_publish_ns = self.ring_publish_ns.load(Relaxed);
        stats.ring_publish_count = self.ring_publish_count.load(Relaxed);
        let mut busy = self.busy.lock().unwrap().clone();
        stats.busy_ns_max = busy.iter().copied().max().unwrap_or(0);
        stats.busy_ns_p50 = percentile(&mut busy, 50);
        stats.busy_ns_p99 = percentile(&mut busy, 99);
        let mut enqueue = self.enqueue.lock().unwrap().clone();
        stats.enqueue_ns_max = enqueue.iter().copied().max().unwrap_or(0);
        stats.enqueue_ns_p50 = percentile(&mut enqueue, 50);
        stats.enqueue_ns_p99 = percentile(&mut enqueue, 99);
        let (phases, event_ns, event_count) = *self.phases.lock().unwrap();
        stats.readback_wait_ns = phases.readback_wait_ns;
        stats.readback_waits = phases.readback_waits;
        stats.map_callback_ns = phases.map_callback_ns;
        stats.row_copy_ns = phases.row_copy_ns;
        stats.decodes = phases.decodes;
        stats.decode_ms = phases.decode_ns / 1_000_000;
        stats.decode_why = phases.decode_why;
        stats.decode_repeat = phases.decode_repeat;
        stats.wait_ms = phases.wait_ns / 1_000_000;
        stats.waits = phases.waits;
        stats.finish_ms = phases.finish_ns / 1_000_000;
        stats.queue_submit_ms = phases.queue_submit_ns / 1_000_000;
        stats.gpu_latency_avg_us = phases.gpu_latency_ns / 1000 / phases.gpu_latency_frames.max(1);
        stats.gpu_latency_max_us = phases.gpu_latency_max_ns / 1000;
        stats.gpu_completion_observations = phases.gpu_latency_frames;
        stats.texel_bytes = phases.texel_bytes;
        stats.texel_bytes_changed = phases.texel_bytes_changed;
        stats.event_ms = event_ns.map(|ns| ns / 1_000_000);
        stats.event_count = event_count;
        stats.phase_textures_ns = phases.textures_ns;
        stats.phase_bind_groups_ns = phases.bind_groups_ns;
        stats.phase_pipelines_ns = phases.pipelines_ns;
        stats.phase_encode_ns = phases.encode_ns;
        stats.phase_submit_ns = phases.submit_ns;
        stats.phase_readback_ns = phases.readback_ns;
        stats.pipelines_created = phases.pipelines_created;
        stats.texture_uploads = phases.texture_uploads;
        stats.texture_upload_batches = phases.texture_upload_batches;
        stats.texture_staging_bytes = phases.texture_staging_bytes;
        stats.spikes_16ms = self.spikes_16ms.load(Relaxed);
        stats.spikes_100ms = self.spikes_100ms.load(Relaxed);
        stats
    }
}

/// Where the guest side puts batches: unbounded by default (the guest never waits), bounded in the opt-in legacy policy.
/// One hand-over to the render thread: whole text lines, or whole records of the binary form (docs/t-rust-hosted-dispatcher.md).
enum Batch {
    Text(Vec<u8>),
    Binary(Vec<u8>),
    Ring(ring::Read),
}

impl Batch {
    fn bytes(&self) -> &[u8] {
        match self {
            Self::Text(b) | Self::Binary(b) => b,
            Self::Ring(r) => r.bytes(),
        }
    }
    fn binary(&self) -> bool {
        match self {
            Self::Text(_) => false,
            Self::Binary(_) => true,
            Self::Ring(r) => r.binary(),
        }
    }
    fn len(&self) -> usize {
        match self {
            Batch::Text(bytes) | Batch::Binary(bytes) => bytes.len(),
            Batch::Ring(read) => read.bytes().len(),
        }
    }
}

#[derive(Clone)]
enum Tx {
    Unbounded(Sender<Batch>),
    Bounded(SyncSender<Batch>),
    Ring(Arc<ring::Ring>),
}

enum Rx {
    Channel(Receiver<Batch>),
    Ring(Arc<ring::Ring>),
}
impl Rx {
    fn recv_timeout(&self, timeout: Duration) -> Result<Batch, std::sync::mpsc::RecvTimeoutError> {
        match self {
            Self::Channel(rx) => rx.recv_timeout(timeout),
            Self::Ring(ring) => {
                let started = Instant::now();
                loop {
                    if let Some(read) = ring.pop() {
                        return Ok(Batch::Ring(read));
                    }
                    if ring.drained() {
                        return Err(std::sync::mpsc::RecvTimeoutError::Disconnected);
                    }
                    if started.elapsed() >= timeout {
                        return Err(std::sync::mpsc::RecvTimeoutError::Timeout);
                    }
                    std::thread::park_timeout(Duration::from_micros(100));
                }
            }
        }
    }
}
struct Producer {
    sender: Option<Tx>,
    pending: Vec<u8>,
    /// Binary records waiting for a hand-over, and the presents among them.
    pending_binary: Vec<u8>,
    pending_binary_presents: u64,
    stats: Stats,
    /// Time spent in write calls since the last frame was handed over.
    enqueue_since_present: Duration,
    dead: bool,
    /// T2292: whole batches the full ring could not take yet, in stream order. They go first at the next hand-over, so a burst
    /// waits in memory (capped by `queue_bytes_cap`) instead of switching the live renderer off.
    spill: std::collections::VecDeque<(Vec<u8>, bool)>,
    spilled_batches: u64,
}

pub struct Runtime {
    adapter: String,
    batch_bytes: usize,
    queue_bytes_cap: u64,
    stall_limit: Option<Duration>,
    stall_total: Option<Duration>,
    /// T2293: backlog valve, `None` when producer omission is off (PNG plan, pipelined readback).
    drop_behind: Option<u64>,
    /// T2293: the valve is open (the guest omits draws) and how many times it opened.
    valve_open: AtomicBool,
    valve_episodes: AtomicU64,
    producer: Mutex<Producer>,
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

/// What the render thread needs besides the queue.
struct Policy {
    publish: bool,
    skip_behind: Option<u64>,
    quiet: bool,
    delay: Duration,
    stats_every: Option<Duration>,
    discard: bool,
}

/// T2136f: publish the renderer phase counters after a present and count (and, with the stats on, name) the spike frames.
fn record_present_phases(shared: &Shared, live: &LiveRenderer, busy: Duration, verbose: bool) {
    *shared.phases.lock().unwrap() = (live.replayer.timings, live.event_ns, live.event_count);
    if busy > Duration::from_millis(16) {
        shared.spikes_16ms.fetch_add(1, Relaxed);
        if busy > SLOW_FRAME_THRESHOLD {
            shared.spikes_100ms.fetch_add(1, Relaxed);
        }
        if verbose && busy > Duration::from_millis(50) {
            if let Some(sample) = live.replayer.samples.last() {
                eprintln!(
                    "rt-host: spike: present {} busy {} ms (draw part {} ms: textures {} ms/{} uploads, pipelines {} ms/{} new, bind {} ms, encode {} ms, submit {} ms of which readback {} ms)",
                    live.presents(),
                    busy.as_millis(),
                    sample.total_ns / 1_000_000,
                    sample.textures_ns / 1_000_000,
                    sample.texture_uploads,
                    sample.pipelines_ns / 1_000_000,
                    sample.pipelines_created,
                    sample.bind_groups_ns.saturating_sub(sample.pipelines_ns) / 1_000_000,
                    sample.encode_ns / 1_000_000,
                    sample.submit_ns / 1_000_000,
                    sample.readback_ns / 1_000_000
                );
            }
        }
    }
}

fn publish_frame(shared: &Shared, frame: tsfp_wgpu::replay::Frame, present: u64) {
    let started = std::time::Instant::now();
    let mut slot = shared.frame.lock().unwrap();
    if slot.is_some() {
        shared.replaced.fetch_add(1, Relaxed);
    }
    *slot = Some(PublishedFrame {
        width: frame.width,
        height: frame.height,
        present,
        rgba: frame.rgba,
    });
    drop(slot);
    shared
        .publish_ns
        .fetch_add(started.elapsed().as_nanos() as u64, Relaxed);
    shared.publish_count.fetch_add(1, Relaxed);
}

fn render_loop(mut live: LiveRenderer, receiver: Rx, shared: Arc<Shared>, policy: Policy) {
    let Policy {
        publish,
        skip_behind,
        quiet,
        delay,
        stats_every,
        discard,
    } = policy;
    let mut failed = false;
    let mut busy_since_present = Duration::ZERO;
    let skipping = std::cell::Cell::new(false);
    let omitted = std::cell::Cell::new(false);
    let mut last_stats = Instant::now();
    loop {
        let received = receiver.recv_timeout(RENDER_POLL_INTERVAL);
        let batch = match received {
            Ok(batch) => batch,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Err(error) = live.poll_readbacks() {
                    eprintln!("rt-host: readback error: {error}");
                    failed = true;
                }
                publish_polled_frame(&mut live, &shared, publish);
                continue;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Final picture has no following present. Service callbacks for a bounded shutdown grace.
                let deadline = Instant::now() + READBACK_SHUTDOWN_GRACE;
                while (live.replayer.readback_ring.stats.occupancy != 0
                    || live.replayer.gpu_work_pending())
                    && Instant::now() < deadline
                {
                    if live.poll_readbacks().is_err() {
                        break;
                    }
                    publish_polled_frame(&mut live, &shared, publish);
                    std::thread::yield_now();
                }
                break;
            }
        };
        shared.queued_bytes.fetch_sub(batch.len() as u64, Relaxed);
        if shared.closing.load(Relaxed) {
            shared.closing_dropped.fetch_add(1, Relaxed);
            continue;
        }
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        if failed {
            continue; // keep draining so the guest never waits on a dead renderer
        }
        if discard {
            let frames = if batch.binary() {
                Reader::new(batch.bytes())
                    .filter(|r| matches!(r, Ok(Record::Present { .. })))
                    .count() as u64
            } else {
                batch
                    .bytes()
                    .split(|&b| b == b'\n')
                    .filter(|line| *line == b"present")
                    .count() as u64
            };
            shared.presents_done.fetch_add(frames, Relaxed);
            shared.render.lock().unwrap().1 += frames;
            continue;
        }
        // One step per stream item (a text line or a binary record): `feed` applies it, `is_present` marks a frame boundary.
        let mut step = |live: &mut LiveRenderer,
                        feed: &mut dyn FnMut(&mut LiveRenderer) -> Result<(), String>,
                        is_present: bool|
         -> bool {
            let before = live.rendered();
            apply_output_requests(live, &shared, is_present);
            let item_started = Instant::now();
            let result = feed(live);
            busy_since_present += item_started.elapsed();
            if let Err(error) = result {
                eprintln!("rt-host: render error: {error}");
                shared.render.lock().unwrap().3 = true;
                return false;
            }
            {
                let mut counters = shared.render.lock().unwrap();
                counters.0 += 1;
                counters.1 = live.presents();
                counters.2 = live.rendered();
            }
            if live.rendered() != before && publish {
                if let Some(frame) = live.take_last_frame() {
                    publish_frame(&shared, frame, live.last_present());
                }
            } else if live.rendered() == before && skipping.get() && is_present {
                shared.skipped.fetch_add(1, Relaxed);
            }
            if live.rendered() != before && !quiet && !publish {
                if let Some(frame) = live.last_frame() {
                    eprintln!(
                        "live: present {} hash {:016x}",
                        live.presents(),
                        frame.hash()
                    );
                }
            }
            if is_present {
                if omitted.get() {
                    shared.omitted.fetch_add(1, Relaxed);
                }
                {
                    let timings = &live.replayer.timings;
                    *shared.pipelines.lock().unwrap() = (
                        timings.pipelines_built,
                        timings.pipeline_ns_max,
                        timings.draws_deferred,
                        live.replayer.unread_draws_skipped,
                    );
                    *shared.worst.lock().unwrap() =
                        (timings.worst_frame_ns, timings.worst_frame_phases);
                }
                {
                    let mut samples = shared.busy.lock().unwrap();
                    if samples.len() < MAX_BUSY_SAMPLES {
                        samples.push(busy_since_present.as_nanos() as u64);
                    }
                }
                record_present_phases(&shared, live, busy_since_present, stats_every.is_some());
                busy_since_present = Duration::ZERO;
                let done = shared.presents_done.fetch_add(1, Relaxed) + 1;
                if let Some(limit) = skip_behind {
                    // Decided at a present boundary for the next frame: `behind` counts the frames the guest completed and the
                    // renderer has not finished, the next one included. Suppress only publication of older frames; every draw
                    // and clear still updates retained surfaces for later feedback. Keep the newest queued frame publishable.
                    let behind = shared.presents_sent.load(Relaxed).saturating_sub(done);
                    skipping.set(behind > limit.max(1));
                    live.set_skip_draw(skipping.get() || omitted.get());
                }
                report_periodic_stats(live, &shared, stats_every, &mut last_stats);
            }
            true
        };
        {
            if !batch.binary() {
                let bytes = batch.bytes();
                for raw in bytes.split(|&b| b == b'\n') {
                    if raw.is_empty() {
                        continue;
                    }
                    let line = String::from_utf8_lossy(raw);
                    let is_present = line.trim() == "present";
                    if !step(&mut live, &mut |live| live.feed_line(&line), is_present) {
                        failed = true;
                        break;
                    }
                }
            } else {
                for record in Reader::new(batch.bytes()) {
                    let ok = match record {
                        Ok(Record::Frame { flags }) => {
                            omitted.set(flags & FRAME_DRAWS_OMITTED != 0);
                            live.set_skip_draw(skipping.get() || omitted.get());
                            true
                        }
                        Ok(record) => {
                            let is_present = matches!(record, Record::Present { .. });
                            step(
                                &mut live,
                                &mut |live| match event_of(&record) {
                                    Ok(Some(event)) => {
                                        if let Record::Present { number } = record {
                                            if number != 0 {
                                                live.name_next_present(number);
                                            }
                                        }
                                        live.feed(&event)
                                    }
                                    Ok(None) => Ok(()),
                                    Err(BinaryError(message)) => Err(message),
                                },
                                is_present,
                            )
                        }
                        Err(offset) => {
                            eprintln!(
                                "rt-host: render error: malformed binary record at byte {offset}"
                            );
                            shared.render.lock().unwrap().3 = true;
                            false
                        }
                    };
                    if !ok {
                        failed = true;
                        break;
                    }
                }
            }
        }
    }
}

impl Runtime {
    /// Start the render thread. Fails (no adapter, bad options) before returning, so the caller can fall back.
    pub fn start(options: Options) -> Result<Self, String> {
        Self::start_with_gpu(options, None)
    }

    fn start_with_gpu(
        mut options: Options,
        gpu: Option<tsfp_wgpu::context::GpuContext>,
    ) -> Result<Self, String> {
        let sharing = gpu.is_some();
        if sharing {
            options.publish_frames = false;
            options.quiet = true;
            options
                .live
                .knobs
                .retain(|(name, _)| name != "pipelined_readback");
        }
        // T2293: the backlog valve is an emergency alternative to switching the renderer off, so it keeps the limit.
        let valve_behind = options.drop_behind;
        // Required persistent surface writes cannot be reconstructed from producer-omitted draws.
        if options
            .live
            .knobs
            .iter()
            .any(|(name, value)| name == "pipelined_readback" && value != "0")
        {
            options.drop_behind = None;
            options.drop_queue_bytes = u64::MAX;
        }
        let bounded = options.stall_limit.is_some() || options.stall_total.is_some();
        let (sender, receiver) = if std::env::var("TSFP_RT_HOST_FAST_HOOKS").as_deref() == Ok("1")
            && !bounded
            && std::env::var_os("TSFP_RENDER_TRACE").is_none()
            && std::env::var_os("TSFP_RT_HOST_BINARY_DUMP").is_none()
            && std::env::var("TSFP_RT_HOST_BINARY").as_deref() != Ok("dual")
            && std::env::var("TSFP_RT_HOST_BINARY").as_deref() != Ok("0")
        {
            let ring = ring::Ring::new(options.queue_batches.max(8), options.batch_bytes);
            (Tx::Ring(ring.clone()), Rx::Ring(ring))
        } else if bounded {
            let (tx, rx) = sync_channel::<Batch>(options.queue_batches);
            (Tx::Bounded(tx), Rx::Channel(rx))
        } else {
            let (tx, rx) = channel::<Batch>();
            (Tx::Unbounded(tx), Rx::Channel(rx))
        };
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<String, String>>();
        let shared = Arc::new(Shared::default());
        let ring = sharing.then(|| Arc::new(Mutex::new(tsfp_wgpu::shared::SharedRing::default())));
        *shared.gpu_frames.lock().unwrap() = ring.clone();
        let quiet = options.quiet;
        let policy = Policy {
            publish: options.publish_frames,
            skip_behind: options.skip_behind,
            quiet,
            delay: options.render_delay,
            stats_every: options.stats_every,
            discard: options.discard,
        };
        let needs_pixels = options.live.dump.base.is_some() || options.publish_frames || !quiet;
        let config = options.live;
        let async_pipelines = options.async_pipelines;
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("tsfp-render".into())
            .spawn(move || {
                // The LiveRenderer holds wgpu objects and `Rc`s, it is created, used and dropped on this thread only.
                let created = match gpu {
                    Some(gpu) => LiveRenderer::with_gpu(config, gpu),
                    None => LiveRenderer::new(config),
                };
                let live = match created {
                    Ok(mut live) => {
                        // No CPU readback unless something consumes pixels: a PNG plan, the published frame, or the hash print.
                        live.replayer.readback = needs_pixels && !sharing;
                        live.replayer.shared_frames = ring;
                        live.replayer.set_async_pipelines(async_pipelines);
                        // T2024k: vertex programs on the GPU in the live path (TSFP_WGPU_GPU_VSH=0 turns it off)
                        live.replayer.enable_gpu_vsh_unless_overridden();
                        live
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(live.replayer.adapter_description()));
                render_loop(live, receiver, thread_shared, policy);
            })
            .map_err(|e| format!("spawn: {e}"))?;
        let adapter;
        match ready_rx.recv() {
            Ok(Ok(description)) => {
                adapter = description;
                if !quiet {
                    eprintln!("live: adapter {adapter}");
                }
            }
            Ok(Err(error)) => {
                let _ = thread.join();
                return Err(format!("no usable adapter: {error}"));
            }
            Err(_) => return Err("render thread died during start".into()),
        }
        Ok(Self {
            adapter,
            batch_bytes: options.batch_bytes,
            queue_bytes_cap: options.queue_bytes_cap,
            stall_limit: options.stall_limit,
            stall_total: options.stall_total,
            drop_behind: valve_behind,
            valve_open: std::sync::atomic::AtomicBool::new(false),
            valve_episodes: AtomicU64::new(0),
            producer: Mutex::new(Producer {
                sender: Some(sender),
                pending: Vec::with_capacity(options.batch_bytes + 4096),
                pending_binary: Vec::new(),
                pending_binary_presents: 0,
                stats: Stats::default(),
                enqueue_since_present: Duration::ZERO,
                dead: false,
                spill: std::collections::VecDeque::new(),
                spilled_batches: 0,
            }),
            shared,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// Append stream bytes. Whole lines are handed to the render thread once a batch is full; a partial last line stays.
    /// One call waits at most the stall limit.
    pub fn write(&self, bytes: &[u8]) {
        let started = Instant::now();
        let mut p = self.producer.lock().unwrap();
        if p.dead {
            return;
        }
        if !p.pending_binary.is_empty() {
            self.hand_over_binary(&mut p); // keep the stream order when text follows binary
        }
        p.pending.extend_from_slice(bytes);
        // A frame is handed over as soon as its `present` line is complete (the C host flushes at each present), so the renderer
        // sees whole frames promptly and `presents_sent` counts frames; a long frame is also cut at `batch_bytes`.
        let frame_end = p.pending.ends_with(b"\npresent\n") || p.pending == b"present\n";
        if frame_end || p.pending.len() >= self.batch_bytes {
            self.hand_over(&mut p);
        }
        p.enqueue_since_present += started.elapsed();
    }

    /// Append one line (a newline is added).
    pub fn send_line(&self, line: &str) {
        self.write(&newline_bytes(line));
    }

    /// The present number of a binary batch that ends with a `Present` record (`u32 length 9, u8 tag, u64 number`), none
    /// when the batch ends otherwise or the number is 0 (an unnamed present).
    pub fn trailing_present(bytes: &[u8]) -> Option<u64> {
        let record = bytes.get(bytes.len().checked_sub(13)?..)?;
        let length = u32::from_le_bytes(record[0..4].try_into().ok()?);
        if length != 9 || record[4] != tsfp_wgpu::trace_wire::TAG_PRESENT {
            return None;
        }
        let number = u64::from_le_bytes(record[5..13].try_into().ok()?);
        (number != 0).then_some(number)
    }

    /// The host presented frame `number`: name it and push everything queued so far to the render thread.
    pub fn present(&self, number: u64) {
        // T2313: frame layers latch this guest frame's decisions (the render thread composes it as present `number`).
        tsfp_wgpu::frame_layer::seal(number);
        self.send_line(&format!("# present {number}"));
        self.send_line("present");
        self.flush();
    }

    /// Hand every complete line to the render thread now.
    pub fn flush(&self) {
        let mut p = self.producer.lock().unwrap();
        if !p.dead {
            self.hand_over_binary(&mut p);
            self.hand_over(&mut p);
        }
    }

    fn give_up(&self, p: &mut Producer, message: &str) {
        p.stats.batches_dropped += 1;
        p.stats.gave_up = true;
        p.dead = true;
        if let Some(Tx::Ring(ring)) = &p.sender {
            ring.close();
        }
        p.sender = None;
        eprintln!("rt-host: {message}, live renderer switched off (the stream cannot be repaired)");
    }

    fn hand_over(&self, p: &mut Producer) {
        // a trailing partial line stays behind for the next call
        let Some(cut) = p.pending.iter().rposition(|&b| b == b'\n').map(|i| i + 1) else {
            return;
        };
        let tail = p.pending.split_off(cut);
        let batch = std::mem::replace(&mut p.pending, tail);
        // Frames completed in this batch (`present` lines).
        let frames = batch
            .split(|&b| b == b'\n')
            .filter(|line| *line == b"present")
            .count() as u64;
        self.dispatch(p, Batch::Text(batch), frames);
    }

    /// Append binary records (whole records, see `tsfp_wgpu::trace_wire`) of which `presents` are frame ends. A frame end hands
    /// everything over at once; otherwise the records wait until `batch_bytes` are queued.
    pub fn write_binary(&self, bytes: &[u8], presents: u64) {
        let _ = self.enqueue_binary(bytes, presents);
    }

    /// Native command callers need a queue acceptance verdict. This is not a GPU
    /// completion verdict; render errors are reported through the runtime stats.
    fn enqueue_binary(&self, bytes: &[u8], presents: u64) -> bool {
        // T2315: the binary lane ends a frame with its `Present` record instead of calling [`Runtime::present`], so the frame
        // layers latch that guest frame here, before the render thread can compose it. Without it the page layer asked its
        // source on the render thread and raced the guest (the blank Player Progress frames of T2311).
        if presents > 0
            && let Some(number) = Self::trailing_present(bytes)
        {
            tsfp_wgpu::frame_layer::seal(number);
        }
        let started = Instant::now();
        let mut p = self.producer.lock().unwrap();
        if p.dead || p.sender.is_none() {
            return false;
        }
        if p.pending.contains(&b'\n') {
            self.hand_over(&mut p); // keep the stream order when binary follows text
        }
        if matches!(p.sender, Some(Tx::Ring(_))) && p.pending_binary.is_empty() {
            self.dispatch_bytes(&mut p, bytes, true, presents);
        } else if p.pending_binary.is_empty() && (presents > 0 || bytes.len() >= self.batch_bytes) {
            // the common case: one copy straight into the batch
            self.dispatch(&mut p, Batch::Binary(bytes.to_vec()), presents);
        } else {
            p.pending_binary.extend_from_slice(bytes);
            p.pending_binary_presents += presents;
            if presents > 0 || p.pending_binary.len() >= self.batch_bytes {
                self.hand_over_binary(&mut p);
            }
        }
        p.enqueue_since_present += started.elapsed();
        !p.dead
    }

    fn hand_over_binary(&self, p: &mut Producer) {
        if p.pending_binary.is_empty() {
            return;
        }
        let batch = std::mem::take(&mut p.pending_binary);
        let frames = std::mem::take(&mut p.pending_binary_presents);
        self.dispatch(p, Batch::Binary(batch), frames);
    }

    fn dispatch_bytes(&self, p: &mut Producer, bytes: &[u8], binary: bool, frames: u64) {
        let Some(Tx::Ring(ring)) = p.sender.clone() else {
            return;
        };
        let length = bytes.len() as u64;
        // `presents_sent` is counted before the send so `done` can never exceed `sent`. The guest side cost of the frames (every
        // write call since the last frame, this hand over included) is recorded once per frame as `enqueue_ns_*`; the C side
        // formatting of the trace lines happens before the write and is not part of it.
        if frames > 0 {
            let sent = self.shared.presents_sent.fetch_add(frames, Relaxed) + frames;
            let depth = sent.saturating_sub(self.shared.presents_done.load(Relaxed));
            self.shared.queue_frames_max.fetch_max(depth, Relaxed);
            let each = (p.enqueue_since_present / frames as u32).as_nanos() as u64;
            p.enqueue_since_present = Duration::ZERO;
            // Telemetry must not block publication behind a stats reader.
            if let Ok(mut samples) = self.shared.enqueue.try_lock() {
                for _ in 0..frames {
                    if samples.len() < MAX_BUSY_SAMPLES {
                        samples.push(each);
                    }
                }
            }
        }
        let queued = self.shared.queued_bytes.fetch_add(length, Relaxed) + length;
        self.shared.queued_bytes_max.fetch_max(queued, Relaxed);
        let started = Instant::now();
        if queued > self.queue_bytes_cap {
            self.shared.queued_bytes.fetch_sub(length, Relaxed);
            ring.close();
            self.give_up(
                p,
                &format!(
                    "nonblocking render ring and spill hold {} MiB",
                    queued >> 20
                ),
            );
            return;
        }
        // T2292: a full ring parks the batch in the spill (order kept) instead of switching the renderer off; the renderer skips
        // draws while behind (`skip_behind`) and the guest may shed replay frames (`frames_behind`).
        Self::drain_spill(p, &ring);
        let sent = p.spill.is_empty() && ring.push(bytes, binary);
        self.shared
            .ring_publish_ns
            .fetch_add(started.elapsed().as_nanos() as u64, Relaxed);
        self.shared.ring_publish_count.fetch_add(1, Relaxed);
        if !sent {
            if p.spilled_batches == 0 {
                eprintln!("rt-host: render ring full, batches wait in the spill (T2292)");
            }
            p.spilled_batches += 1;
            p.spill.push_back((bytes.to_vec(), binary));
        }
        p.stats.batches_sent += 1;
    }

    /// Move spilled batches into the ring while it has room.
    fn drain_spill(p: &mut Producer, ring: &ring::Ring) {
        while let Some((bytes, binary)) = p.spill.front() {
            if !ring.push(bytes, *binary) {
                break;
            }
            p.spill.pop_front();
        }
    }

    /// Queue one batch holding `frames` frame ends.
    fn dispatch(&self, p: &mut Producer, mut batch: Batch, frames: u64) {
        if matches!(p.sender, Some(Tx::Ring(_))) {
            self.dispatch_bytes(p, batch.bytes(), batch.binary(), frames);
            return;
        }
        let Some(sender) = p.sender.clone() else {
            return;
        };
        let length = batch.len() as u64;
        // `presents_sent` is counted before the send so `done` can never exceed `sent`. The guest side cost of the frames (every
        // write call since the last frame, this hand over included) is recorded once per frame as `enqueue_ns_*`; the C side
        // formatting of the trace lines happens before the write and is not part of it.
        if frames > 0 {
            let sent = self.shared.presents_sent.fetch_add(frames, Relaxed) + frames;
            let depth = sent.saturating_sub(self.shared.presents_done.load(Relaxed));
            self.shared.queue_frames_max.fetch_max(depth, Relaxed);
            let each = (p.enqueue_since_present / frames as u32).as_nanos() as u64;
            p.enqueue_since_present = Duration::ZERO;
            let mut samples = self.shared.enqueue.lock().unwrap();
            for _ in 0..frames {
                if samples.len() < MAX_BUSY_SAMPLES {
                    samples.push(each);
                }
            }
        }
        let queued = self.shared.queued_bytes.fetch_add(length, Relaxed) + length;
        self.shared.queued_bytes_max.fetch_max(queued, Relaxed);
        let sender = match sender {
            Tx::Unbounded(sender) => {
                // Never waits. The only limit is memory: a renderer this far behind (even state only) is hopeless.
                if queued > self.queue_bytes_cap {
                    self.shared.queued_bytes.fetch_sub(length, Relaxed);
                    self.give_up(p, &format!("render queue holds {} MiB", queued >> 20));
                } else if sender.send(batch).is_err() {
                    self.shared.queued_bytes.fetch_sub(length, Relaxed);
                    p.stats.batches_dropped += 1;
                    p.stats.gave_up = true;
                    p.dead = true;
                    if let Some(Tx::Ring(ring)) = &p.sender {
                        ring.close();
                    }
                    p.sender = None;
                } else {
                    p.stats.batches_sent += 1;
                }
                return;
            }
            Tx::Bounded(sender) => sender,
            Tx::Ring(_) => unreachable!(),
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
                        self.shared.queued_bytes.fetch_sub(length, Relaxed);
                        let message = format!("render queue full for {:?}", started.elapsed());
                        self.give_up(p, &message);
                        break;
                    }
                    batch = back;
                    std::thread::sleep(Duration::from_micros(200));
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.shared.queued_bytes.fetch_sub(length, Relaxed);
                    p.stats.batches_dropped += 1;
                    p.stats.gave_up = true;
                    p.dead = true;
                    if let Some(Tx::Ring(ring)) = &p.sender {
                        ring.close();
                    }
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

    /// The newest rendered frame not yet taken (needs `publish_frames`), at the scaled size. Latest wins: a frame that was not
    /// taken before the next one finished is replaced and counted in `frames_replaced`.
    pub fn take_frame(&self) -> Option<PublishedFrame> {
        let started = std::time::Instant::now();
        let frame = self.shared.frame.lock().unwrap().take();
        self.shared
            .take_frame_ns
            .fetch_add(started.elapsed().as_nanos() as u64, Relaxed);
        self.shared.take_frame_count.fetch_add(1, Relaxed);
        frame
    }

    /// Ask for a new output size (T2136e): applied at the next present boundary, state is kept, the frames after it have this
    /// size. Safe to call from any thread at any rate, the newest request wins.
    pub fn set_output_size(&self, width: u32, height: u32) {
        let request = if width > 0 && height > 0 {
            (u64::from(width) << 32) | u64::from(height)
        } else {
            0
        };
        self.shared.output_request.store(request, Relaxed);
    }

    /// Ask for a window (present target) size (T2095): from the next present on frames are the internal image mapped to this size
    /// with the sharp bilinear pass, aspect kept. Same hand-off rules as [`Runtime::set_output_size`].
    pub fn set_window_size(&self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.shared
                .window_request
                .store((u64::from(width) << 32) | u64::from(height), Relaxed);
        }
    }

    /// Size of the frame `take_frame` would return.
    pub fn frame_size(&self) -> Option<(u32, u32)> {
        self.shared
            .frame
            .lock()
            .unwrap()
            .as_ref()
            .map(|frame| (frame.width, frame.height))
    }

    /// Producer draw omission. Off in normal lag: the renderer needs every surface write for later feedback (T2226).
    /// T2293 emergency valve: once the backlog holds half of `queue_bytes_cap` (the renderer would otherwise be switched off
    /// at the cap) the guest omits draws until the renderer is back within `drop_behind` frames. Retained feedback surfaces
    /// may be stale for the frames of such an episode; that beats losing the live renderer.
    pub fn pressure(&self) -> bool {
        let Some(limit) = self.drop_behind else {
            return false;
        };
        if self.valve_open.load(Relaxed) {
            if self.frames_behind() <= limit.max(1) {
                self.valve_open.store(false, Relaxed);
            }
        } else if self.shared.queued_bytes.load(Relaxed) > self.queue_bytes_cap / 2 {
            self.valve_open.store(true, Relaxed);
            if self.valve_episodes.fetch_add(1, Relaxed) == 0 {
                eprintln!(
                    "rt-host: backlog over half the queue cap, the guest omits draws until the renderer catches up (T2293)"
                );
            }
        }
        self.valve_open.load(Relaxed)
    }

    /// T2293: times the backlog valve opened.
    pub fn valve_episodes(&self) -> u64 {
        self.valve_episodes.load(Relaxed)
    }

    /// T2292: completed frames handed over and not yet replayed by the render thread (spilled ones included). The guest can
    /// shed optional work (the T2263 replay frames) while this grows.
    pub fn frames_behind(&self) -> u64 {
        self.shared
            .presents_sent
            .load(Relaxed)
            .saturating_sub(self.shared.presents_done.load(Relaxed))
    }

    /// Batches that waited in the spill because the ring was full (T2292).
    pub fn spilled_batches(&self) -> u64 {
        self.producer.lock().unwrap().spilled_batches
    }

    /// The adapter the render thread opened: name, backend, type, driver and limits on one line.
    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    /// False once the live renderer was switched off (queue memory cap, stall limit, dead render thread): the guest then has no
    /// reason to keep producing the stream.
    pub fn alive(&self) -> bool {
        !self.producer.lock().unwrap().dead
    }

    /// Current counters.
    pub fn stats(&self) -> Stats {
        let producer = self.producer.lock().unwrap().stats;
        let mut stats = self.shared.counters();
        stats.batches_sent = producer.batches_sent;
        stats.batches_dropped = producer.batches_dropped;
        stats.stalls = producer.stalls;
        stats.stall_ns_total = producer.stall_ns_total;
        stats.stall_ns_max = producer.stall_ns_max;
        stats.gave_up = producer.gave_up;
        stats
    }

    /// Flush, close the queue and wait for the render thread to finish what was queued (PNG files included).
    pub fn finish(&self) -> Stats {
        if self.valve_episodes() > 0 {
            eprintln!(
                "rt-host: backlog valve opened {} times (T2293)",
                self.valve_episodes()
            );
        }
        {
            let mut p = self.producer.lock().unwrap();
            if !p.dead {
                self.hand_over_binary(&mut p);
                p.pending.push(b'\n');
                self.hand_over(&mut p);
            }
            // T2295: a live display (drop_behind set) shows nothing after close, so the backlog is dropped, not drawn.
            if self.drop_behind.is_some() {
                self.shared.closing.store(true, Relaxed);
            }
            if let Some(Tx::Ring(ring)) = p.sender.clone() {
                // the render thread is still reading: hand it the rest of the spill before closing
                while !p.spill.is_empty()
                    && !self
                        .thread
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_none_or(|t| t.is_finished())
                {
                    Self::drain_spill(&mut p, &ring);
                    if !p.spill.is_empty() {
                        std::thread::sleep(Duration::from_micros(200));
                    }
                }
                ring.close();
            }
            p.sender = None;
        }
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
        let dropped = self.shared.closing_dropped.load(Relaxed);
        if dropped > 0 {
            eprintln!(
                "rt-host: closing: dropped {dropped} queued batches instead of drawing them (T2295)"
            );
        }
        self.stats()
    }
}

/// Encode one command with exactly one appended stream delimiter.
fn newline_bytes(line: &str) -> Vec<u8> {
    let mut text = Vec::with_capacity(line.len() + 1);
    text.extend_from_slice(line.as_bytes());
    text.push(b'\n');
    text
}

/// Refresh observable renderer state after polling and publish a completed image if requested.
fn publish_polled_frame(live: &mut LiveRenderer, shared: &Shared, publish: bool) {
    shared.phases.lock().unwrap().0 = live.replayer.timings;
    shared.render.lock().unwrap().2 = live.rendered();
    if publish {
        if let Some(frame) = live.take_last_frame() {
            publish_frame(shared, frame, live.last_present());
        }
    }
}

/// Poll without holding up command batches or readback callbacks.
const RENDER_POLL_INTERVAL: Duration = Duration::from_millis(2);
/// Bound final-image callback servicing when the producer disconnects.
const READBACK_SHUTDOWN_GRACE: Duration = Duration::from_millis(100);
/// Cap diagnostic sample memory without changing the measured samples.
const MAX_BUSY_SAMPLES: usize = 65_536;

/// Apply producer resize requests before feeding the present that finishes this frame.
fn apply_output_requests(live: &mut LiveRenderer, shared: &Shared, is_present: bool) {
    if is_present {
        // T2136e: a requested output size takes effect for the frame this present finishes (draws are only recorded until
        // the present, so nothing of the frame is lost).
        let request = shared.output_request.load(Relaxed);
        if request != 0 {
            let (width, height) = ((request >> 32) as u32, request as u32);
            if live.replayer.output_size() != (width, height) {
                live.set_output_size(width, height);
            }
        }
    }
    if is_present {
        let request = shared.window_request.load(Relaxed);
        if request != 0 {
            let size = ((request >> 32) as u32, request as u32);
            if live.replayer.window_size() != Some(size) {
                live.set_window_size(Some(size));
            }
        }
    }
}

/// Emit the existing opt-in counters without changing the interval or message text.
fn report_periodic_stats(
    live: &LiveRenderer,
    shared: &Shared,
    every: Option<Duration>,
    last_stats: &mut Instant,
) {
    if let Some(every) = every {
        if last_stats.elapsed() >= every {
            *last_stats = Instant::now();
            if live.replayer.knob("pipelined_readback") {
                eprintln!(
                    "rt-host: readback ring: {:?}",
                    live.replayer.readback_ring.stats
                );
            }
            eprintln!("rt-host: stats: {}", shared.counters().summary());
            // T2024k: the GPU vertex program counters and where the CPU side of vertex shading goes
            let t = &live.replayer.timings;
            eprintln!(
                "rt-host: gpu-vsh: {:?}, shade_vertices {} ms (interpreter {} ms, fetch {} ms), job recording {} ms",
                live.replayer.gpu_vsh_stats(),
                t.shade_ns / 1_000_000,
                t.interp_ns / 1_000_000,
                t.fetch_ns / 1_000_000,
                t.gpu_prep_ns / 1_000_000
            );
        }
    }
}

/// Threshold of the existing spikes_100ms diagnostic counter.
const SLOW_FRAME_THRESHOLD: Duration = Duration::from_millis(100);
