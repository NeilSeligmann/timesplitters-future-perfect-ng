// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024 slice 7: the live renderer. The same [`Replayer`] state machine the offline tool drives from a file, fed call by call.
//!
//! * [`LiveRenderer`] owns a headless [`Replayer`], numbers the presents, keeps the last frame and dumps PNGs at chosen presents.
//!   It takes [`Event`]s ([`LiveRenderer::feed`]) or the text lines of the stream format ([`LiveRenderer::feed_line`], what the
//!   host's `TSFP_RENDER_TRACE` writer emits, so an out of process host can pipe its lines in).
//! * [`LiveDevice`] is the in process implementation of the game's `RenderDevice` and `RenderHost` contract: hand
//!   [`LiveDevice::device`], [`LiveDevice::host`] and [`LiveDevice::memory`] to the T1898 `Renderer` and every call renders as it
//!   happens. It is the stream [`Recorder`] with its event list replaced by a sink into the renderer, so the vertex and index
//!   snapshots are taken exactly as for a recorded stream (live and offline cannot disagree about what a draw read).
//!
//! The inputs the trace only carried as artefacts are methods here: [`LiveDevice::vertex_constants`] (the constant uploads the
//! lifted code makes outside the device, `d3d8_upload_vertex_constants`), [`LiveDevice::lib_method`] (the NV2A methods the D3D8
//! library pushes itself), [`LiveDevice::surface`] (`SetRenderTarget`) and [`LiveDevice::texture`] (texel images keyed by the
//! texture record). See docs/t-wgpu-renderer.md, Slice 7.
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;

use tsfp_engine_render::device::DeviceCall;

use crate::context::{GpuContext, Preference};
use crate::replay::{Frame, Replayer};
use crate::stream::{Event, Recorder, RecorderDevice, RecorderHost, SharedMemory, Stream};

/// Which presents are written as PNG (`<base>.p<N>.png`, N the present number).
#[derive(Clone, Debug, Default)]
pub struct DumpPlan {
    /// File name prefix; `None` dumps nothing.
    pub base: Option<PathBuf>,
    /// Present numbers to dump; empty with `every` false dumps nothing.
    pub at: BTreeSet<u64>,
    /// Dump every present.
    pub every: bool,
}

/// Construction options.
#[derive(Clone, Debug)]
pub struct LiveConfig {
    pub width: u32,
    pub height: u32,
    /// Render at this pixel size (the stream stays 640x480 logical); `None` is the logical size.
    pub output: Option<(u32, u32)>,
    /// T2095: present through the sharp bilinear pass at this window size (aspect kept, black bars); `None` hands out the internal size.
    pub window: Option<(u32, u32)>,
    pub preference: Preference,
    pub dump: DumpPlan,
    /// Experiment switches (`Replayer::knobs`).
    pub knobs: Vec<(String, String)>,
    /// Presents before `from` and after `to` only update state (no image); `to` ends `feed_line` early.
    pub from: u64,
    pub to: u64,
}

impl LiveConfig {
    /// The default configuration on the software (CPU) adapter: deterministic, what the golden hashes are pinned to.
    pub fn software() -> Self {
        Self {
            preference: Preference::Software,
            ..Self::default()
        }
    }
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            width: 640,
            height: 480,
            output: None,
            window: None,
            // T2136d: the best real adapter. The deterministic hash tests ask for the software adapter with `LiveConfig::software()`.
            preference: Preference::Any,
            dump: DumpPlan::default(),
            knobs: Vec::new(),
            from: 0,
            to: u64::MAX,
        }
    }
}

/// A headless renderer that is fed events as they happen.
pub struct LiveRenderer {
    pub replayer: Replayer,
    dump: DumpPlan,
    from: u64,
    to: u64,
    presents: u64,
    rendered: u64,
    pending_number: Option<u64>,
    last: Option<Frame>,
    last_present: u64,
    line: usize,
    /// Suppress publication of the next presents while preserving their surface writes.
    skip_draw: bool,
    /// T2136f: time spent per stream item kind, see [`EVENT_KINDS`] (index 0 is the text parse), and the item counts.
    pub event_ns: [u64; 8],
    pub event_count: [u64; 8],
}

/// Names of the [`LiveRenderer::event_ns`] slots.
pub const EVENT_KINDS: [&str; 8] = [
    "parse",
    "mem",
    "texture",
    "settexture",
    "host",
    "lib",
    "call",
    "present",
];

fn event_kind(event: &Event) -> usize {
    match event {
        Event::Memory { .. } => 1,
        Event::Texture { .. } => 2,
        Event::Call(DeviceCall::SetTexture { .. }) => 3,
        Event::Host(..) => 4,
        Event::Lib { .. } => 5,
        Event::Present => 7,
        _ => 6,
    }
}

impl LiveRenderer {
    /// Native animation draw boundary. Samples owned fixed-tick state with the
    /// scene's opt-in display policy and uploads model-selected constants before
    /// the caller submits this object's draw. No guest matrix/body is changed.
    pub fn animated_object(
        &mut self,
        scene: &tsfp_engine_anim::display_scene::AnimationScene,
        view: tsfp_engine_anim::display_scene::ViewObject,
        alpha: f32,
        source: &impl tsfp_engine_anim::display::ClipSource,
        bindings: &[crate::animation::NodeBinding],
    ) -> Result<(), String> {
        let batch = scene
            .sample_view(&[view], alpha, source)
            .map_err(|error| format!("native animation sample: {error:?}"))?;
        let events = crate::animation::prepare(&batch.palettes[0].nodes, bindings)
            .map_err(|error| format!("native animation binding: {error:?}"))?;
        for event in events {
            self.feed(&event)?;
        }
        Ok(())
    }

    pub fn new(config: LiveConfig) -> Result<Self, String> {
        let gpu = GpuContext::new(&config.preference)?;
        Self::with_gpu(config, gpu)
    }

    /// Use the caller-owned shared device and queue.
    pub fn with_gpu(config: LiveConfig, gpu: GpuContext) -> Result<Self, String> {
        let mut replayer = Replayer::new(gpu, config.width, config.height);
        if let Some((width, height)) = config.output {
            replayer.set_output_size(width, height);
        }
        replayer.set_window_size(config.window);
        replayer.knobs.extend(config.knobs);
        replayer.rendering = config.from <= 1;
        Ok(Self {
            replayer,
            dump: config.dump,
            from: config.from,
            to: config.to,
            presents: 0,
            rendered: 0,
            pending_number: None,
            last: None,
            last_present: 0,
            line: 0,
            skip_draw: false,
            event_ns: [0; 8],
            event_count: [0; 8],
        })
    }

    /// Presents seen so far (also the number of the last one unless the host named it).
    pub fn presents(&self) -> u64 {
        self.presents
    }

    /// Frames actually rendered (presents inside the `from..=to` window).
    pub fn rendered(&self) -> u64 {
        self.rendered
    }

    /// The last rendered frame.
    pub fn last_frame(&self) -> Option<&Frame> {
        self.last.as_ref()
    }

    /// Change the pixel size of the following frames (runtime window resize). State is kept, targets are recreated.
    pub fn set_output_size(&mut self, width: u32, height: u32) {
        self.replayer.set_output_size(width, height);
    }

    /// T2095: change the window sized present target (runtime window resize), `None` returns to the internal size.
    pub fn set_window_size(&mut self, size: Option<(u32, u32)>) {
        self.replayer.set_window_size(size);
    }

    /// Suppress publication while preserving draws and clears on retained surfaces.
    pub fn set_skip_draw(&mut self, skip: bool) {
        self.skip_draw = skip;
        self.replayer.rendering = (self.from..=self.to).contains(&(self.presents + 1));
    }

    pub fn last_present(&self) -> u64 {
        self.last_present
    }

    /// Move the last rendered frame out (the caller owns the pixels, no copy).
    pub fn take_last_frame(&mut self) -> Option<Frame> {
        self.last.take()
    }

    /// True after the present numbered `to`: the caller can stop feeding.
    pub fn finished(&self) -> bool {
        self.presents >= self.to
    }

    /// Name the next present (the host's own counter, a `# present N` line in the text form).
    pub fn name_next_present(&mut self, number: u64) {
        self.pending_number = Some(number);
    }

    /// Execute one event. A [`Event::Present`] resolves the frame, numbers it and writes the PNG when the plan asks.
    pub fn feed(&mut self, event: &Event) -> Result<(), String> {
        let began = std::time::Instant::now();
        let result = self.feed_timed(event);
        let kind = event_kind(event);
        self.event_ns[kind] += began.elapsed().as_nanos() as u64;
        self.event_count[kind] += 1;
        result
    }

    fn feed_timed(&mut self, event: &Event) -> Result<(), String> {
        // T2293: the ring only completes frames that are read back; an unread image (quiet, nothing published) lands in
        // `frames` like without the knob, else every drawn frame was lost from `rendered` and `frames` grew without bound.
        let pipeline = self.replayer.knob("pipelined_readback") && self.replayer.readback;
        let next = self.pending_number.unwrap_or(self.presents + 1);
        self.replayer.readback_present = next;
        self.replayer.suppress_presentation = self.skip_draw;
        self.replayer.feed(event);
        if !matches!(event, Event::Present) {
            return Ok(());
        }
        let number = self.pending_number.take().unwrap_or(self.presents + 1);
        self.presents = number;
        let completed = if pipeline {
            self.replayer.poll_readbacks(number)
        } else {
            self.replayer
                .frames
                .pop()
                .map(|f| (number, f))
                .into_iter()
                .collect()
        };
        self.accept_frames(completed)?;
        self.replayer.rendering = (self.from..=self.to).contains(&(number + 1));
        Ok(())
    }

    /// Service callbacks while the producer is idle; no invented present or blocking GPU wait.
    pub fn poll_readbacks(&mut self) -> Result<(), String> {
        self.replayer.poll_gpu();
        if !self.replayer.knob("pipelined_readback") {
            return Ok(());
        }
        let completed = self.replayer.poll_readbacks(self.presents + 1);
        self.accept_frames(completed)
    }

    fn accept_frames(&mut self, completed: Vec<(u64, Frame)>) -> Result<(), String> {
        for (number, frame) in completed {
            self.last_present = number;
            let wanted = self.dump.every || self.dump.at.contains(&number);
            if let (true, Some(base)) = (wanted, &self.dump.base) {
                let name = format!("{}.p{number}.png", base.display());
                std::fs::write(&name, frame.to_png())
                    .map_err(|error| format!("{name}: {error}"))?;
            }
            self.last = Some(frame);
            self.rendered += 1;
        }
        Ok(())
    }

    /// Execute one line of the text stream format (`# present N` names the next present).
    pub fn feed_line(&mut self, line: &str) -> Result<(), String> {
        self.line += 1;
        if let Some(rest) = line.strip_prefix("# present ") {
            self.pending_number = rest.trim().parse().ok();
            return Ok(());
        }
        let began = std::time::Instant::now();
        let parsed = Stream::parse_line(self.line - 1, line);
        self.event_ns[0] += began.elapsed().as_nanos() as u64;
        self.event_count[0] += 1;
        match parsed? {
            Some(event) => self.feed(&event),
            None => Ok(()),
        }
    }

    /// The replay report (unsupported and approximated features, counts) as text.
    pub fn report(&self) -> String {
        format!("{:#?}", self.replayer.report)
    }
}

/// The in process `RenderDevice` and `RenderHost` backend: every call renders as it happens.
#[derive(Clone)]
pub struct LiveDevice {
    recorder: Recorder,
    renderer: Rc<RefCell<LiveRenderer>>,
    error: Rc<RefCell<Option<String>>>,
}

impl LiveDevice {
    pub fn new(config: LiveConfig) -> Result<Self, String> {
        let renderer = Rc::new(RefCell::new(LiveRenderer::new(config)?));
        let error = Rc::new(RefCell::new(None));
        let recorder = Recorder::new();
        let (sink_renderer, sink_error) = (renderer.clone(), error.clone());
        recorder.set_sink(Box::new(move |event| {
            if let Err(message) = sink_renderer.borrow_mut().feed(&event) {
                sink_error.borrow_mut().get_or_insert(message);
            }
        }));
        Ok(Self {
            recorder,
            renderer,
            error,
        })
    }

    /// The recorder whose sink is the renderer (`Rig::on` takes it).
    pub fn recorder(&self) -> Recorder {
        self.recorder.clone()
    }

    /// The `RenderDevice` face (hand it to `Renderer::new`).
    pub fn device(&self) -> RecorderDevice {
        self.recorder.device()
    }

    /// The `RenderHost` face.
    pub fn host(&self) -> RecorderHost {
        self.recorder.host()
    }

    /// The guest memory the renderer and the draw snapshots share.
    pub fn memory(&self) -> SharedMemory {
        self.recorder.memory()
    }

    /// The address `lock_vertex_buffer` answers (the vertex ring base in the guest image).
    pub fn set_lock_address(&self, address: u32) {
        self.recorder.set_lock_address(address);
    }

    /// First class vertex constant upload from outside the device (the lifted code's `d3d8_upload_vertex_constants`).
    pub fn vertex_constants(&self, first_register: u32, words: &[u32]) {
        self.recorder
            .emit(Event::Call(DeviceCall::SetVertexConstants {
                first_register,
                words: words.to_vec(),
            }));
    }

    /// An NV2A method the D3D8 library pushed itself (combiner words, enables, clears, texture address).
    pub fn lib_method(&self, method: u32, value: u32) {
        self.recorder.emit(Event::Lib { method, value });
    }

    /// `SetRenderTarget`: the surface header words (see `Event::Surface`).
    #[allow(clippy::too_many_arguments)]
    pub fn surface(
        &self,
        target: u32,
        depth: u32,
        data: u32,
        format: u32,
        size: u32,
        pitch: u32,
        depth_data: u32,
        flags: u32,
    ) {
        self.recorder.emit(Event::Surface {
            target,
            depth,
            data,
            format,
            size,
            pitch,
            depth_data,
            flags,
        });
    }

    /// A decoded texture image keyed by the texture record address `set_texture` receives.
    pub fn texture(&self, id: u32, width: u32, height: u32, rgba: Vec<u8>) {
        self.recorder.push_texture(id, width, height, rgba);
    }

    /// End of a frame; `number` is the host's own present counter when it has one.
    pub fn present(&self, number: Option<u64>) {
        if let Some(number) = number {
            self.renderer.borrow_mut().name_next_present(number);
        }
        self.recorder.present();
    }

    /// Run `f` on the renderer (frames, report, replayer knobs).
    pub fn with_renderer<R>(&self, f: impl FnOnce(&mut LiveRenderer) -> R) -> R {
        f(&mut self.renderer.borrow_mut())
    }

    /// The first error a sink event produced (a PNG that could not be written), taken.
    pub fn take_error(&self) -> Option<String> {
        self.error.borrow_mut().take()
    }
}
