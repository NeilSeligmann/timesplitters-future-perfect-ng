// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline headless render of a preview sidecar (UNVERIFIED). Usage:
//! render_capture <sidecar> <frame-index> <out.png>
use std::collections::BTreeMap;
use tsfp_engine_render::native_scene::RoomDrawPacket;
use tsfp_render_preview::{
    GeometryDecodeRejection, NativePreviewRenderer, build_draw_list_with_meshes,
    build_draw_list_with_meshes_and_coverage, capture,
};
use tsfp_wgpu::vsh;

fn shader_input_reads(packet: &RoomDrawPacket) -> ([usize; vsh::INPUT_COUNT], usize) {
    let mut reads = [0; vsh::INPUT_COUNT];
    let program_start = (packet.vertex_bytes as usize)
        .saturating_add(packet.index_bytes as usize)
        .saturating_add(
            packet
                .texture_bytes
                .iter()
                .map(|bytes| *bytes as usize)
                .sum(),
        );
    let Some(program_end) = program_start.checked_add(packet.vertex_program_bytes as usize) else {
        return (reads, 0);
    };
    let Some(bytes) = packet.payload.get(program_start..program_end) else {
        return (reads, 0);
    };
    if bytes.is_empty() || bytes.len() % 16 != 0 || bytes.len() > vsh::PROGRAM_SLOTS * 16 {
        return (reads, 0);
    }
    let mut memory = vsh::ProgramMemory::default();
    for (slot, chunk) in memory.slots.iter_mut().zip(bytes.chunks_exact(16)) {
        for (word, raw) in slot.iter_mut().zip(chunk.chunks_exact(4)) {
            *word = u32::from_le_bytes(raw.try_into().expect("four byte word"));
        }
    }
    let program = vsh::decode_from(&memory, packet.vertex_shader_index as usize);
    for instruction in &program {
        if instruction.input < reads.len()
            && [instruction.a, instruction.b, instruction.c]
                .iter()
                .any(|source| source.mux == 2)
        {
            reads[instruction.input] += 1;
        }
    }
    (reads, program.len())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).expect("sidecar");
    let frames = capture::read_frames(&text).expect("parse");
    if args.get(2).is_some_and(|arg| arg == "--coverage") {
        let mut total_meshes = 0usize;
        let mut total_draws = 0usize;
        let mut captured_packets = 0usize;
        let mut decoded_programs = 0usize;
        let mut missing_payloads = 0usize;
        let mut rejections = BTreeMap::<GeometryDecodeRejection, usize>::new();
        let mut input_reads = [0usize; vsh::INPUT_COUNT];
        let mut position_candidate_counts = BTreeMap::<usize, usize>::new();
        let mut declaration_shapes = BTreeMap::<(u32, u32, u32), usize>::new();
        for frame in &frames {
            let mut view = tsfp_render_prep::live::NativeSceneView::default();
            view.rectangle = [0, 0, frame.extent[0] as i32, frame.extent[1] as i32];
            let packets = capture::frame_to_packets(frame);
            for (packet, geometry) in packets.iter().zip(&frame.geometry) {
                if packet.vertex_stride == 24 {
                    captured_packets += 1;
                    let (reads, instructions) = shader_input_reads(packet);
                    decoded_programs += usize::from(instructions > 0);
                    for (total, reads) in input_reads.iter_mut().zip(reads) {
                        *total += reads;
                    }
                    if let Some(geometry) = geometry {
                        if let Some(declarations) = capture::decode_vertex_declaration_for_render(
                            &geometry.vertex_declaration_data,
                        ) {
                            let candidates = declarations
                                .iter()
                                .filter(|attribute| {
                                    attribute.kind == 0x32
                                        && (attribute.register as usize) < reads.len()
                                        && reads[attribute.register as usize] > 0
                                        && (attribute.offset as usize)
                                            .checked_add(12)
                                            .is_some_and(|end| end <= geometry.stride as usize)
                                })
                                .count();
                            *position_candidate_counts.entry(candidates).or_default() += 1;
                            for attribute in declarations {
                                *declaration_shapes
                                    .entry((attribute.register, attribute.offset, attribute.kind))
                                    .or_default() += 1;
                            }
                        }
                    }
                }
            }
            let (_, meshes, coverage) =
                build_draw_list_with_meshes_and_coverage(&packets, &view, frame.tick)
                    .expect("captured frame");
            total_meshes += meshes.len();
            total_draws += packets.len();
            missing_payloads += coverage.missing_payloads;
            for (reason, count) in coverage.rejections {
                *rejections.entry(reason).or_default() += count;
            }
            println!(
                "UNVERIFIED coverage tick={} geometry_meshes={}/{}",
                frame.tick,
                meshes.len(),
                packets.len(),
            );
        }
        println!(
            "UNVERIFIED coverage total geometry_meshes={total_meshes}/{total_draws} missing_payloads={missing_payloads} rejections={rejections:?} captured_packets={captured_packets} decoded_programs={decoded_programs} input_reads={input_reads:?} position_candidate_counts={position_candidate_counts:?} declaration_shapes={declaration_shapes:?} frames={}",
            frames.len()
        );
        return;
    }
    let frame = &frames[args[2]
        .parse::<usize>()
        .expect("index")
        .min(frames.len() - 1)];
    let mut view = tsfp_render_prep::live::NativeSceneView::default();
    view.rectangle = [0, 0, frame.extent[0] as i32, frame.extent[1] as i32];
    let (list, meshes) =
        build_draw_list_with_meshes(&capture::frame_to_packets(frame), &view, frame.tick)
            .expect("list");
    let draw_count = frame.draws.len();
    let geometry_mesh_count = meshes.len();
    let valid = list.validate().expect("valid");
    let instance = wgpu::Instance::default();
    let adapter = pollster_block(instance.request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .expect("adapter");
    let (device, queue) =
        pollster_block(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let (w, h) = (frame.extent[0], frame.extent[1]);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut renderer = NativePreviewRenderer::new(&device, format);
    renderer.set_skip_unresolved_meshes(true);
    for mesh in meshes {
        renderer
            .upload_frame_mesh(&device, mesh)
            .expect("captured geometry");
    }
    let report = renderer.render(
        &device,
        &queue,
        &texture.create_view(&Default::default()),
        &valid,
    );
    let row = (w * 4).next_multiple_of(256);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(row * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(enc.finish()));
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.expect("map"));
    device.poll(wgpu::Maintain::Wait);
    let data = buffer.slice(..).get_mapped_range();
    let mut rgba = Vec::new();
    for y in 0..h as usize {
        rgba.extend_from_slice(&data[y * row as usize..y * row as usize + w as usize * 4]);
    }
    std::fs::write(&args[3], capture::encode_png(w, h, &rgba)).expect("write");
    println!(
        "UNVERIFIED preview tick={} {w}x{h} geometry_meshes={geometry_mesh_count}/{draw_count} report={report:?}",
        frame.tick,
    );
}

fn pollster_block<F: std::future::Future>(f: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct Noop;
    impl Wake for Noop {
        fn wake(self: std::sync::Arc<Self>) {}
    }
    let waker = Waker::from(std::sync::Arc::new(Noop));
    let mut cx = Context::from_waker(&waker);
    let mut f = std::pin::pin!(f);
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}
