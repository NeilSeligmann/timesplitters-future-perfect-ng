// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024k: vertex programs on the GPU, the glue between `Replayer::draw` and [`crate::vsh_wgsl`]
//! (docs/t-wgpu-gpu-vertex-programs.md). Child module of `replay` so it sees the replayer's private state.
//!
//! A draw with enough distinct vertices whose program has a ready compute pipeline is not interpreted: the replayer reserves its
//! 84 byte vertex records (zeros), pushes its indices, and records a job (the decoded input registers of the distinct vertices,
//! a snapshot of the constant file, fog and texture size parameters). `present` uploads the job data once per frame and runs
//! one compute pass: every job's shader writes its vertex records, then the index fix kernel degenerates the primitives that
//! have a vertex with no valid clip position (what the CPU path drops). Everything else (clear detection, the grade pass,
//! small draws, knobs that change the CPU result) stays on the CPU interpreter, which is also the fallback while a program's
//! pipeline is still being built.
use std::collections::HashMap;
use std::sync::mpsc;

use super::*;
use crate::vsh_wgsl::{self, Job, Translation, WORKGROUP};

/// Draws with fewer distinct vertices than this stay on the CPU path (cheap, and the clear/grade detection reads their values).
const MIN_VERTICES: usize = 24;
/// Largest vertex or index data (bytes) one frame's storage buffers hold (the default binding limit is 128 MiB).
const MAX_BYTES: usize = 100 << 20;

enum Program {
    Pending(Translation),
    Ready(Translation, wgpu::ComputePipeline),
    Failed,
}

/// One recorded job of the frame.
struct FrameJob {
    key: u64,
    job: Job,
    first_index: u32,
    primitives: u32,
    group: u32,
}

/// Counters of the GPU path (MEASURED by `examples/replay.rs` and the rt-host stats).
#[derive(Clone, Copy, Debug, Default)]
pub struct GpuVshStats {
    pub gpu_draws: u64,
    pub gpu_vertices: u64,
    /// Draws that would have been GPU draws but ran on the CPU because the program's pipeline was not ready.
    pub pending_draws: u64,
    pub programs_ready: u64,
    pub programs_failed: u64,
    pub compile_ns: u64,
    pub compile_max_ns: u64,
    pub small_draws: u64,
    pub small_vertices: u64,
    pub input_bytes: u64,
    pub constant_bytes: u64,
    pub input_reuses: u64,
    pub constant_reuses: u64,
    pub upload_batches: u64,
    pub upload_encode_ns: u64,
    pub programs_requested: u64,
}

pub(super) struct GpuVsh {
    device: wgpu::Device,
    pub bind_layout: wgpu::BindGroupLayout,
    fix_pipeline: wgpu::ComputePipeline,
    programs: HashMap<u64, Program>,
    requests: mpsc::Sender<(u64, String)>,
    done: mpsc::Receiver<(u64, Option<wgpu::ComputePipeline>, u64)>,
    inputs: Vec<f32>,
    constants: Vec<f32>,
    jobs: Vec<FrameJob>,
    input_records: HashMap<Vec<u32>, u32>,
    constant_records: HashMap<Vec<u32>, u32>,
    pub stats: GpuVshStats,
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

impl GpuVsh {
    pub(super) fn new(device: wgpu::Device) -> Self {
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tsfp-gpuvsh-bind"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                storage_entry(2, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage_entry(4, false),
            ],
        });
        let layout = Arc::new(
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("tsfp-gpuvsh-layout"),
                bind_group_layouts: &[&bind_layout],
                push_constant_ranges: &[],
            }),
        );
        let fix_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsfp-gpuvsh-indexfix"),
            source: wgpu::ShaderSource::Wgsl(vsh_wgsl::INDEX_FIX_WGSL.into()),
        });
        let fix_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("tsfp-gpuvsh-indexfix"),
            layout: Some(&layout),
            module: &fix_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let (requests, request_rx) = mpsc::channel::<(u64, String)>();
        let (done_tx, done) = mpsc::channel();
        let worker_device = device.clone();
        std::thread::Builder::new()
            .name("tsfp-gpuvsh".into())
            .spawn(move || {
                while let Ok((key, wgsl)) = request_rx.recv() {
                    let started = std::time::Instant::now();
                    let pipeline = build_compute(&worker_device, &layout, &wgsl);
                    let nanoseconds = started.elapsed().as_nanos() as u64;
                    if done_tx.send((key, pipeline, nanoseconds)).is_err() {
                        return;
                    }
                }
            })
            .expect("gpu vsh worker thread");
        Self {
            device,
            bind_layout,
            fix_pipeline,
            programs: HashMap::new(),
            requests,
            done,
            inputs: Vec::new(),
            constants: Vec::new(),
            jobs: Vec::new(),
            input_records: HashMap::new(),
            constant_records: HashMap::new(),
            stats: GpuVshStats::default(),
        }
    }

    fn poll(&mut self) {
        while let Ok((key, pipeline, nanoseconds)) = self.done.try_recv() {
            self.stats.compile_ns += nanoseconds;
            self.stats.compile_max_ns = self.stats.compile_max_ns.max(nanoseconds);
            let Some(Program::Pending(translation)) = self.programs.remove(&key) else {
                continue;
            };
            match pipeline {
                Some(pipeline) => {
                    self.stats.programs_ready += 1;
                    self.programs
                        .insert(key, Program::Ready(translation, pipeline));
                }
                None => {
                    self.stats.programs_failed += 1;
                    self.programs.insert(key, Program::Failed);
                }
            }
        }
    }

    /// Forget the jobs of a dropped frame.
    pub(super) fn clear_frame(&mut self) {
        self.inputs.clear();
        self.constants.clear();
        self.jobs.clear();
        self.input_records.clear();
        self.constant_records.clear();
    }

    pub(super) fn has_jobs(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Encode the frame's compute pass into `encoder` over the frame's vertex and index buffers, then forget the jobs.
    pub(super) fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        vertices: &wgpu::Buffer,
        indices: &wgpu::Buffer,
    ) {
        let started = std::time::Instant::now();
        self.stats.upload_batches += 1;
        use wgpu::util::DeviceExt;
        let device = self.device.clone();
        let mut inputs = std::mem::take(&mut self.inputs);
        let mut constants = std::mem::take(&mut self.constants);
        let jobs = std::mem::take(&mut self.jobs);
        self.input_records.clear();
        self.constant_records.clear();
        self.stats.input_bytes += (inputs.len() * 4) as u64;
        self.stats.constant_bytes += (constants.len() * 4) as u64;
        if inputs.is_empty() {
            inputs.extend([0.0; 4]);
        }
        if constants.is_empty() {
            constants.extend([0.0; 4]);
        }
        // Slot i * 2 is job i, slot i * 2 + 1 its index fix parameters.
        let mut uniform = vec![0u8; jobs.len() * 2 * Job::SLOT as usize];
        for (index, frame_job) in jobs.iter().enumerate() {
            let at = index * 2 * Job::SLOT as usize;
            uniform[at..at + 96].copy_from_slice(&frame_job.job.bytes());
            let fix = Job {
                vertex_count: frame_job.primitives,
                input_base: frame_job.first_index,
                constants_base: frame_job.group,
                ..Job::default()
            };
            let at = at + Job::SLOT as usize;
            uniform[at..at + 96].copy_from_slice(&fix.bytes());
        }
        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tsfp-gpuvsh-inputs"),
            contents: &bytes_of(&inputs),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let constant_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tsfp-gpuvsh-constants"),
            contents: &bytes_of(&constants),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let job_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("tsfp-gpuvsh-jobs"),
            contents: &uniform,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tsfp-gpuvsh-group"),
            layout: &self.bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: constant_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vertices.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &job_buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(96),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: indices.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("tsfp-gpuvsh"),
            timestamp_writes: None,
        });
        for (index, frame_job) in jobs.iter().enumerate() {
            let Some(Program::Ready(_, pipeline)) = self.programs.get(&frame_job.key) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &group, &[(index * 2) as u32 * Job::SLOT as u32]);
            pass.dispatch_workgroups(frame_job.job.vertex_count.div_ceil(WORKGROUP), 1, 1);
        }
        pass.set_pipeline(&self.fix_pipeline);
        for (index, frame_job) in jobs.iter().enumerate() {
            pass.set_bind_group(0, &group, &[(index * 2 + 1) as u32 * Job::SLOT as u32]);
            pass.dispatch_workgroups(frame_job.primitives.div_ceil(WORKGROUP), 1, 1);
        }
        self.stats.upload_encode_ns += started.elapsed().as_nanos() as u64;
    }
}

fn bytes_of(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Build a compute pipeline from WGSL, `None` when the shader does not validate (the program then stays on the CPU path).
pub fn build_compute(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    wgsl: &str,
) -> Option<wgpu::ComputePipeline> {
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tsfp-gpuvsh-program"),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("tsfp-gpuvsh-program"),
        layout: Some(layout),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    match pollster::block_on(device.pop_error_scope()) {
        Some(error) => {
            eprintln!("gpu vsh: shader rejected: {error}");
            None
        }
        None => Some(pipeline),
    }
}

impl Replayer {
    /// Choose the GPU vertex program path: `Some` forces it, `None` follows the `gpu_vsh` knob, then `TSFP_WGPU_GPU_VSH`, then
    /// the default ([`GPU_VSH_DEFAULT`]).
    pub fn set_gpu_vsh(&mut self, on: Option<bool>) {
        self.gpu_vsh_choice = on;
        if on == Some(false) {
            self.gpu_vsh = None;
        }
    }

    /// The live path's default: the GPU path is ON unless `--opt gpu_vsh=0` or `TSFP_WGPU_GPU_VSH=0` (or a prior `set_gpu_vsh`) says
    /// otherwise. Offline tools keep [`GPU_VSH_DEFAULT`] (off) so their pixel goldens stay on the interpreter.
    pub fn enable_gpu_vsh_unless_overridden(&mut self) {
        if self.gpu_vsh_choice.is_none()
            && !self.knobs.contains_key("gpu_vsh")
            && std::env::var_os("TSFP_WGPU_GPU_VSH").is_none()
        {
            self.gpu_vsh_choice = Some(true);
        }
    }

    pub fn gpu_vsh_stats(&self) -> GpuVshStats {
        self.gpu_vsh.as_ref().map(|g| g.stats).unwrap_or_default()
    }

    fn gpu_vsh_wanted(&self) -> bool {
        if let Some(choice) = self.gpu_vsh_choice {
            return choice;
        }
        if let Some(value) = self.knobs.get("gpu_vsh") {
            return value != "0";
        }
        match std::env::var("TSFP_WGPU_GPU_VSH") {
            Ok(value) => value != "0",
            Err(_) => GPU_VSH_DEFAULT,
        }
    }

    /// Request the currently selected program ahead of its first large draw. No guest state,
    /// uploads or draw records change. Call at a loading boundary after applying program events.
    /// Nonblocking: the existing CPU fallback remains valid until the worker finishes.
    pub fn prewarm_vertex_program(&mut self) -> bool {
        let Some(start) = self.vsh_start else {
            return false;
        };
        let gpu = self
            .gpu_vsh
            .get_or_insert_with(|| GpuVsh::new(self.gpu.device.clone()));
        gpu.poll();
        let key = vsh_wgsl::program_key(&self.program_memory, start);
        if !gpu.programs.contains_key(&key) {
            let translation = vsh_wgsl::translate(&self.program_memory, start);
            if gpu.requests.send((key, translation.wgsl.clone())).is_err() {
                return false;
            }
            gpu.stats.programs_requested += 1;
            gpu.programs.insert(key, Program::Pending(translation));
        }
        !matches!(gpu.programs.get(&key), Some(Program::Failed))
    }

    /// Try to shade the draw's vertices on the GPU. On success the vertex records are reserved, the indices pushed and the number
    /// of indices returned; `None` leaves everything untouched (the CPU path runs).
    pub(super) fn gpu_shade(
        &mut self,
        base: u32,
        stride: u32,
        vertex_ids: &[u32],
        order: &[u32],
        group: usize,
    ) -> Option<u32> {
        // Preserve CPU-only clear/grade recognition even when a threshold sweep requests zero.
        let threshold = self
            .knobs
            .get("gpu_vsh_min_vertices")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(MIN_VERTICES)
            .max(5);
        if vertex_ids.len() < threshold {
            if let Some(gpu) = &mut self.gpu_vsh {
                gpu.stats.small_draws += 1;
                gpu.stats.small_vertices += vertex_ids.len() as u64;
            }
            return None;
        }
        let wanted = match self.gpu_vsh_choice {
            Some(choice) => choice,
            None => {
                let choice = self.gpu_vsh_wanted();
                self.gpu_vsh_choice = Some(choice);
                choice
            }
        };
        if !wanted {
            return None;
        }
        for knob in [
            "host_raster",
            "cpu_clip",
            "no_depth_clamp",
            "drop_behind_eye",
            "vsh_trace_tex",
            "input_report",
            "colour_report",
            "no_gpu_vsh",
        ] {
            if self.knobs.contains_key(knob) {
                return None;
            }
        }
        if self.debug_vertices > 0 {
            return None;
        }
        let start = self.vsh_start?;
        if self.gpu_vsh.is_none() {
            self.gpu_vsh = Some(GpuVsh::new(self.gpu.device.clone()));
        }
        let key = vsh_wgsl::program_key(&self.program_memory, start);
        let gpu = self.gpu_vsh.as_mut().expect("created");
        gpu.poll();
        let inputs_wanted = match gpu.programs.get(&key) {
            Some(Program::Ready(translation, _)) => translation.inputs.clone(),
            Some(Program::Pending(_)) => {
                gpu.stats.pending_draws += 1;
                return None;
            }
            Some(Program::Failed) => return None,
            None => {
                gpu.stats.programs_requested += 1;
                let translation = vsh_wgsl::translate(&self.program_memory, start);
                let _ = gpu.requests.send((key, translation.wgsl.clone()));
                gpu.programs.insert(key, Program::Pending(translation));
                gpu.stats.pending_draws += 1;
                return None;
            }
        };
        // Distinct vertices in first use order, like the CPU path. A run of consecutive ids (the usual draw) needs no table.
        let first_id = vertex_ids[0];
        let consecutive = vertex_ids
            .iter()
            .enumerate()
            .all(|(index, &id)| id == first_id.wrapping_add(index as u32));
        let mut unique: Vec<u32> = Vec::new();
        let mut positions: Vec<u32> = Vec::new();
        if consecutive {
            unique.extend_from_slice(vertex_ids);
            positions.extend(0..vertex_ids.len() as u32);
        } else {
            let low = *vertex_ids.iter().min().expect("not empty");
            let high = *vertex_ids.iter().max().expect("not empty");
            positions.reserve(vertex_ids.len());
            if high - low < 1 << 20 {
                let mut table = vec![u32::MAX; (high - low) as usize + 1];
                for &id in vertex_ids {
                    let entry = &mut table[(id - low) as usize];
                    if *entry == u32::MAX {
                        unique.push(id);
                        *entry = (unique.len() - 1) as u32;
                    }
                    positions.push(*entry);
                }
            } else {
                let mut local: HashMap<u32, u32> = HashMap::new();
                for &id in vertex_ids {
                    let slot = *local.entry(id).or_insert_with(|| {
                        unique.push(id);
                        (unique.len() - 1) as u32
                    });
                    positions.push(slot);
                }
            }
        }
        let primitives = order.len() / group;
        if unique.len() < threshold {
            let gpu = self.gpu_vsh.as_mut().expect("created");
            gpu.stats.small_draws += 1;
            gpu.stats.small_vertices += unique.len() as u64;
            return None;
        }
        if self.vertices.len() + unique.len() * Vertex::SIZE > MAX_BYTES
            || (self.indices.len() + primitives * group) * 4 > MAX_BYTES
        {
            return None;
        }
        let reuse = self.knob("batch_uploads");
        let gpu = self.gpu_vsh.as_mut().expect("created");
        if (gpu.inputs.len() + unique.len() * inputs_wanted.len().max(1) * 4) * 4 > MAX_BYTES
            || (gpu.constants.len() + CONSTANT_COUNT * 4) * 4 > MAX_BYTES
        {
            return None;
        }
        let input_start = gpu.inputs.len();
        let mut input_base = (input_start / 4) as u32;
        for &id in &unique {
            let address = base.wrapping_add(id.wrapping_mul(stride));
            for &register in &inputs_wanted {
                let value = if self.arrays_active {
                    let (offset, format) = self.arrays[register];
                    fetch_attribute(
                        &self.memory,
                        offset.wrapping_add(id.wrapping_mul(format >> 8)),
                        format & 0xFF,
                    )
                } else {
                    let entry = self.declaration[register];
                    fetch_attribute(
                        &self.memory,
                        address.wrapping_add(entry.offset),
                        entry.format,
                    )
                };
                gpu.inputs.extend(value);
            }
            if inputs_wanted.is_empty() {
                // keep the stride of one vec4 per vertex (INPUT_STRIDE is at least 1)
                gpu.inputs.extend([0.0f32; 4]);
            }
        }
        if reuse {
            let bits: Vec<u32> = gpu.inputs[input_start..]
                .iter()
                .map(|v| v.to_bits())
                .collect();
            if let Some(&base) = gpu.input_records.get(&bits) {
                gpu.inputs.truncate(input_start);
                input_base = base;
                gpu.stats.input_reuses += 1;
            } else {
                gpu.input_records.insert(bits, input_base);
            }
        }
        // Exact bit keys preserve NaN payloads and signed zero; no sampled hash can alias.
        // Constant file snapshot, shared with the previous job when unchanged.
        let constants_floats = CONSTANT_COUNT * 4;
        let same = gpu.constants.len() >= constants_floats
            && gpu.constants[gpu.constants.len() - constants_floats..]
                .iter()
                .zip(self.vsh_constants.iter().flatten())
                .all(|(a, b)| a.to_bits() == b.to_bits());
        let bits: Vec<u32> = if reuse {
            self.vsh_constants
                .iter()
                .flatten()
                .map(|v| v.to_bits())
                .collect()
        } else {
            Vec::new()
        };
        let cached = reuse
            .then(|| gpu.constant_records.get(&bits).copied())
            .flatten();
        let constants_base = if let Some(base) = cached {
            gpu.stats.constant_reuses += 1;
            base
        } else {
            if !same {
                gpu.constants.extend(self.vsh_constants.iter().flatten());
            }
            let base = ((gpu.constants.len() - constants_floats) / 4) as u32;
            if reuse {
                gpu.constant_records.insert(bits, base);
            }
            base
        };
        let (fog_mode, fog_parameters) = match self.fog_state() {
            Some(fog) => (fog.mode, [fog.parameter0, fog.parameter1]),
            None => (u32::MAX, [0.0; 2]),
        };
        let stage_ids = [
            self.texture_id,
            self.extra_textures[0],
            self.extra_textures[1],
            self.extra_textures[2],
        ];
        let mut sizes = [[0.0f32; 2]; 4];
        for (stage, id) in stage_ids.iter().enumerate() {
            let linear = self
                .textures
                .get(id)
                .and_then(|image| {
                    image
                        .linear
                        .then_some((image.width as f32, image.height as f32))
                })
                .or_else(|| {
                    self.fb_surfaces
                        .get(id)
                        .filter(|(_, _, linear)| *linear)
                        .map(|&(width, height, _)| (width as f32, height as f32))
                });
            if let Some((width, height)) = linear {
                sizes[stage] = [width, height];
            }
        }
        let vertex_base = (self.vertices.len() / Vertex::SIZE) as u32;
        let first_index = self.indices.len() as u32;
        self.vertices
            .resize(self.vertices.len() + unique.len() * Vertex::SIZE, 0);
        for primitive in order.chunks_exact(group) {
            self.indices.extend(
                primitive
                    .iter()
                    .map(|&p| vertex_base + positions[p as usize]),
            );
        }
        let gpu = self.gpu_vsh.as_mut().expect("created");
        gpu.jobs.push(FrameJob {
            key,
            job: Job {
                vertex_count: unique.len() as u32,
                input_base,
                constants_base,
                out_base: vertex_base,
                screen: [self.width as f32, self.height as f32],
                fog_mode,
                fog_parameters,
                sizes,
            },
            first_index,
            primitives: primitives as u32,
            group: group as u32,
        });
        gpu.stats.gpu_draws += 1;
        gpu.stats.gpu_vertices += unique.len() as u64;
        self.report.vsh_vertices += unique.len();
        Some((primitives * group) as u32)
    }
}
