// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024k: the WGSL translation of a vertex program (`vsh_wgsl`) run on the GPU against the CPU interpreter (`vsh::run`), every
//! output register and component of every vertex. The programs are the 150 FABRICATED random programs of
//! `tests/data/vsh_oracle_cases.txt` (no retail bytes), each run over 32 vertices: the case's own inputs and 31 mutations drawn
//! from a fixed LCG (special values: zeros, denormals, huge, infinities, NaN, plus ordinary floats).
//!
//! Tolerance: bit exact (NaN equals NaN) for every program without RSQ, EXP, LOG or LIT; for those four the CPU computes in
//! double and rounds once while the GPU uses float32 builtins, so an output may differ by a few ulps (checked with a relative
//! bound, and counted). Adapter: `TSFP_WGPU_ADAPTER` or the best real one, the software adapter when none exists.
use tsfp_wgpu::context::{GpuContext, Preference};
use tsfp_wgpu::vsh::{self, CONSTANT_COUNT, INPUT_COUNT, OUTPUT_COUNT, ProgramMemory};
use tsfp_wgpu::vsh_wgsl::{self, Job};
use wgpu::util::DeviceExt;

const VERTICES: usize = 32;

fn word(text: &str, index: usize) -> u32 {
    u32::from_str_radix(&text[index * 8..index * 8 + 8], 16).unwrap()
}

struct Case {
    memory: ProgramMemory,
    inputs: [[f32; 4]; INPUT_COUNT],
    constants: [[f32; 4]; CONSTANT_COUNT],
}

fn cases() -> Vec<Case> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/vsh_oracle_cases.txt"
    ))
    .unwrap();
    text.lines()
        .map(|line| {
            let parts: Vec<&str> = line.split('|').collect();
            let mut memory = ProgramMemory::default();
            let bytes: Vec<u8> = (0..parts[0].len() / 2)
                .map(|i| u8::from_str_radix(&parts[0][i * 2..i * 2 + 2], 16).unwrap())
                .collect();
            for (slot, chunk) in bytes.chunks(16).enumerate() {
                for i in 0..4 {
                    memory.slots[slot][i] =
                        u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
                }
            }
            let mut inputs = [[0.0f32; 4]; INPUT_COUNT];
            for (i, v) in inputs.iter_mut().flatten().enumerate() {
                *v = f32::from_bits(word(parts[1], i));
            }
            let mut constants = [[0.0f32; 4]; CONSTANT_COUNT];
            for entry in parts[2].split(';').filter(|e| !e.is_empty()) {
                let (slot, list) = entry.split_once('=').unwrap();
                for (c, v) in list.split(',').enumerate() {
                    constants[slot.parse::<usize>().unwrap()][c] =
                        f32::from_bits(u32::from_str_radix(v, 16).unwrap());
                }
            }
            Case {
                memory,
                inputs,
                constants,
            }
        })
        .collect()
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn value(&mut self) -> f32 {
        const SPECIAL: [u32; 12] = [
            0x0000_0000,
            0x8000_0000,
            0x3f80_0000,
            0xbf80_0000,
            0x3f00_0000,
            0x4000_0000,
            0x42c8_0000,
            0x0000_0400, // denormal
            0x7f00_0000,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_0000,
        ];
        let pick = self.next();
        if pick % 4 == 0 {
            f32::from_bits(SPECIAL[(pick as usize / 4) % SPECIAL.len()])
        } else {
            ((self.next() % 2_000_001) as f32 - 1_000_000.0) / 100_000.0
        }
    }
}

fn ulps(left: f32, right: f32) -> u64 {
    let key = |x: f32| {
        let bits = x.to_bits() as i32;
        i64::from(if bits < 0 { i32::MIN - bits } else { bits })
    };
    key(left).abs_diff(key(right))
}

fn uses_transcendental(memory: &ProgramMemory) -> bool {
    vsh_wgsl::extent(memory, 0)
        .iter()
        .any(|inst| (2..=7).contains(&inst.ilu))
}

#[test]
fn gpu_programs_match_the_cpu_interpreter() {
    let gpu = match GpuContext::new(&Preference::Any) {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("no adapter, skipped: {error}");
            return;
        }
    };
    eprintln!("adapter: {:?}", gpu.info);
    let device = &gpu.device;
    let entry =
        |binding: u32, ty: wgpu::BufferBindingType, dynamic: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: dynamic,
                min_binding_size: None,
            },
            count: None,
        };
    let storage = |read_only| wgpu::BufferBindingType::Storage { read_only };
    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            entry(0, storage(true), false),
            entry(1, storage(true), false),
            entry(2, storage(false), false),
            entry(3, wgpu::BufferBindingType::Uniform, true),
            entry(4, storage(false), false),
        ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_layout],
        push_constant_ranges: &[],
    });
    let mut rng = Lcg(20241011);
    let (mut exact_programs, mut tolerant_programs) = (0usize, 0usize);
    let (mut exact_values, mut close_values, mut far_values) = (0usize, 0usize, 0usize);
    let mut worst_ulps = 0u64;
    let mut nan_folds = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for (number, case) in cases().iter().enumerate() {
        let translation = vsh_wgsl::translate_with(&case.memory, 0, true);
        // inputs per vertex: vertex 0 is the case, the rest are mutations of it
        let mut vertex_inputs = Vec::new();
        for vertex in 0..VERTICES {
            let mut inputs = case.inputs;
            if vertex > 0 {
                for value in inputs.iter_mut().flatten() {
                    if rng.next() % 3 == 0 {
                        *value = rng.value();
                    }
                }
            }
            vertex_inputs.push(inputs);
        }
        let mut packed: Vec<f32> = Vec::new();
        for inputs in &vertex_inputs {
            for &register in &translation.inputs {
                packed.extend(inputs[register]);
            }
            if translation.inputs.is_empty() {
                packed.extend([0.0f32; 4]);
            }
        }
        let bytes =
            |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() };
        let constants: Vec<f32> = case.constants.iter().flatten().copied().collect();
        let job = Job {
            vertex_count: VERTICES as u32,
            screen: [640.0, 480.0],
            fog_mode: u32::MAX,
            ..Job::default()
        };
        let mut job_bytes = vec![0u8; 256];
        job_bytes[..96].copy_from_slice(&job.bytes());
        let make = |label: &str, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let inputs_buffer = make("in", &bytes(&packed), wgpu::BufferUsages::STORAGE);
        let constants_buffer = make("const", &bytes(&constants), wgpu::BufferUsages::STORAGE);
        let out_size = (VERTICES * 64 * 4) as u64;
        let out_buffer = make(
            "out",
            &vec![0u8; out_size as usize],
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        );
        let index_buffer = make("idx", &[0u8; 16], wgpu::BufferUsages::STORAGE);
        let job_buffer = make("job", &job_bytes, wgpu::BufferUsages::UNIFORM);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: out_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(translation.wgsl.as_str().into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            panic!(
                "case {number}: shader rejected: {error}\n{}",
                translation.wgsl
            );
        }
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: inputs_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: constants_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: out_buffer.as_entire_binding(),
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
                    resource: index_buffer.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[0]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, out_size);
        gpu.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| result.unwrap());
        device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let floats: Vec<f32> = data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        drop(data);
        readback.unmap();
        let tolerant = uses_transcendental(&case.memory);
        if tolerant {
            tolerant_programs += 1;
        } else {
            exact_programs += 1;
        }
        let mut program_failed = false;
        for (vertex, inputs) in vertex_inputs.iter().enumerate() {
            let expected = vsh::run(&case.memory, 0, inputs, &case.constants);
            for register in 0..OUTPUT_COUNT {
                for component in 0..4 {
                    let want = expected.registers[register][component];
                    let got = floats[vertex * 64 + register * 4 + component];
                    if got.to_bits() == want.to_bits() || (got.is_nan() && want.is_nan()) {
                        exact_values += 1;
                        continue;
                    }
                    // x + (-x) with x infinite: the driver folds it to 0 (Vulkan does not require inf and NaN to be preserved)
                    if want.is_nan() && got == 0.0 {
                        nan_folds += 1;
                        continue;
                    }
                    let distance = ulps(got, want);
                    let near_zero = got.abs() < 1e-30 && want.abs() < 1e-30;
                    let relative = ((got - want) / want).abs() <= 2e-5;
                    if tolerant && (distance <= 8 || near_zero || relative) {
                        close_values += 1;
                        worst_ulps = worst_ulps.max(distance);
                    } else {
                        far_values += 1;
                        program_failed = true;
                        if std::env::var("DUMP_CASE").is_ok_and(|v| v == number.to_string())
                            && failures.len() < 3
                        {
                            let hex: Vec<String> = inputs[..4]
                                .iter()
                                .flatten()
                                .map(|v| format!("{:08x}", v.to_bits()))
                                .collect();
                            eprintln!("inputs v0..v3 {}", hex.join(" "));
                        }
                        if failures.len() < 40 {
                            failures.push(format!(
                                "case {number} vertex {vertex} o{register}.{component}: gpu {got:e} ({:08x}) cpu {want:e} ({:08x}) tolerant={tolerant}",
                                got.to_bits(),
                                want.to_bits()
                            ));
                        }
                    }
                }
            }
        }
        if program_failed && std::env::var("DUMP_CASE").is_ok_and(|v| v == number.to_string()) {
            eprintln!(
                "{}\n{}",
                vsh::describe(&case.memory, 0),
                translation.wgsl.split("var a0").nth(1).unwrap_or("")
            );
        }
    }
    eprintln!(
        "gpu vsh oracle: {} programs bit exact class, {} with RSQ/EXP/LOG/LIT; values: {exact_values} identical, {close_values} within 8 ulps (worst {worst_ulps}), {far_values} outside, {nan_folds} NaN folded to 0 by the driver",
        exact_programs, tolerant_programs
    );
    assert!(
        failures.is_empty(),
        "{} failures, first: {:#?}",
        far_values,
        failures
    );
}
