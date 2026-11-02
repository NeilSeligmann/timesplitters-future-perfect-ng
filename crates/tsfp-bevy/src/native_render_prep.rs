// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in host bridge for T2238. The live collector publishes ordered room
//! plans through the host mailbox; this system samples the native owner and
//! submits typed frames to an audit sink. Legacy remains the drawing path.
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use tsfp_engine_render::native_scene::{
    NativeSceneOwner, NativeViewInput, PreparedSceneLease, RoomDrawPacket, SceneFrameAuditSink,
    SceneFrameLeaseSink, SceneFrameSink,
};
use tsfp_render_prep::live::{NativeSceneView, prepare_bridge_frame};

const ROOM_CAPACITY: usize = 200;
const OBJECT_CAPACITY: usize = 0x226;
const QUEUE_CAPACITY: usize = 128;
const RETIRE_KIND: u32 = 1;
static ENABLED: AtomicBool = AtomicBool::new(false);
static MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static DRAW_CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);

#[cfg(not(tsfp_no_host))]
unsafe extern "C" {
    fn tsfp_native_scene_bridge_enable(enabled: i32);
    fn tsfp_native_scene_bridge_set_draw_capture_sample_interval(interval: u32);
    fn tsfp_native_scene_bridge_read_draw(
        after_sequence: u64,
        sequence: *mut u64,
        packet: *mut NativeSceneDrawPacket,
        payload: *mut u8,
        payload_capacity: u32,
        payload_bytes: *mut u32,
    ) -> u64;
    fn tsfp_native_scene_bridge_draw_counters(
        unscoped: *mut u64,
        dropped: *mut u64,
        scope_errors: *mut u64,
    );
    fn tsfp_native_scene_bridge_draw_capture_timing(
        calls: *mut u64,
        elapsed_ns: *mut u64,
        payload_bytes: *mut u64,
    );
    fn tsfp_native_scene_bridge_collector_counters(
        guest_calls: *mut u64,
        adapter_entries: *mut u64,
        accepted: *mut u64,
        bad_count: *mut u64,
        bad_order: *mut u64,
        last_room_count: *mut i32,
        last_draw_depth: *mut i32,
    );
    fn tsfp_native_scene_bridge_read(
        after_sequence: u64,
        sequence: *mut u64,
        rooms: *mut u8,
        room_capacity: u32,
        room_count: *mut u32,
        draw_depth: *mut i32,
        kind: *mut u32,
        object_records: *mut u32,
        object_capacity: u32,
        object_count: *mut u32,
        view: *mut NativeSceneView,
        view_count: *mut u32,
    ) -> u64;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NativeSceneDrawPacket {
    frame_sequence: u64,
    room: u32,
    indexed: u32,
    primitive: u32,
    first: u32,
    count: u32,
    vertex_buffer: u32,
    vertex_buffer_bytes: u32,
    vertex_stride: u32,
    vertex_first: u32,
    index_buffer: u32,
    index_buffer_bytes: u32,
    texture_headers: [u32; 4],
    texture_data: [u32; 4],
    texture_formats: [u32; 4],
    texture_sizes: [u32; 4],
    texture_stage_state: [u32; 128],
    viewport_words: [u32; 4],
    viewport_target: [u32; 4],
    scissor_words: [u32; 4],
    scissor_count: u32,
    scissor_exclusive: u32,
    vertex_declaration: u32,
    vertex_shader_handle: u32,
    vertex_shader_index: u32,
    vertex_program_bytes: u32,
    vertex_constants_bytes: u32,
    pixel_shader_bytes: u32,
    vertex_program_hash: u64,
    vertex_constants_hash: u64,
    pixel_shader_hash: u64,
    blend_enable: u32,
    blend_source: u32,
    blend_destination: u32,
    blend_operation: u32,
    depth_enable: u32,
    depth_write: u32,
    depth_function: u32,
    cull_mode: u32,
    stencil_enable: u32,
    stencil_function: u32,
    stencil_reference: u32,
    stencil_mask: u32,
    stencil_write_mask: u32,
    stencil_fail: u32,
    stencil_depth_fail: u32,
    stencil_pass: u32,
    vertex_bytes: u32,
    index_bytes: u32,
    texture_bytes: [u32; 4],
    payload_bytes: u32,
    payload_dropped: u32,
    render_state: [u32; 256],
    material_key: u64,
    state_key: u64,
}

impl Default for NativeSceneDrawPacket {
    fn default() -> Self {
        Self {
            frame_sequence: 0,
            room: 0,
            indexed: 0,
            primitive: 0,
            first: 0,
            count: 0,
            vertex_buffer: 0,
            vertex_buffer_bytes: 0,
            vertex_stride: 0,
            vertex_first: 0,
            index_buffer: 0,
            index_buffer_bytes: 0,
            texture_headers: [0; 4],
            texture_data: [0; 4],
            texture_formats: [0; 4],
            texture_sizes: [0; 4],
            texture_stage_state: [0; 128],
            viewport_words: [0; 4],
            viewport_target: [0; 4],
            scissor_words: [0; 4],
            scissor_count: 0,
            scissor_exclusive: 0,
            vertex_declaration: 0,
            vertex_shader_handle: 0,
            vertex_shader_index: 0,
            vertex_program_bytes: 0,
            vertex_constants_bytes: 0,
            pixel_shader_bytes: 0,
            vertex_program_hash: 0,
            vertex_constants_hash: 0,
            pixel_shader_hash: 0,
            blend_enable: 0,
            blend_source: 0,
            blend_destination: 0,
            blend_operation: 0,
            depth_enable: 0,
            depth_write: 0,
            depth_function: 0,
            cull_mode: 0,
            stencil_enable: 0,
            stencil_function: 0,
            stencil_reference: 0,
            stencil_mask: 0,
            stencil_write_mask: 0,
            stencil_fail: 0,
            stencil_depth_fail: 0,
            stencil_pass: 0,
            vertex_bytes: 0,
            index_bytes: 0,
            texture_bytes: [0; 4],
            payload_bytes: 0,
            payload_dropped: 0,
            render_state: [0; 256],
            material_key: 0,
            state_key: 0,
        }
    }
}

#[derive(Default)]
struct RoomDrawCounts {
    packets: u64,
    primitives: u64,
    vertices: u64,
    index_bytes: u64,
    texture_bytes: u64,
    by_primitive: BTreeMap<u32, u64>,
    materials: BTreeSet<u64>,
    textures: BTreeSet<(u32, u32, u32)>,
}

fn payload_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn intern_payload(
    cache: &mut BTreeMap<(u64, usize), Vec<std::sync::Arc<[u8]>>>,
    bytes: Vec<u8>,
) -> (u64, std::sync::Arc<[u8]>) {
    let hash = payload_hash(&bytes);
    let entries = cache.entry((hash, bytes.len())).or_default();
    if let Some(existing) = entries
        .iter()
        .find(|entry| entry.as_ref() == bytes.as_slice())
    {
        return (hash, existing.clone());
    }
    let owned: std::sync::Arc<[u8]> = std::sync::Arc::from(bytes);
    entries.push(owned.clone());
    (hash, owned)
}

fn primitive_count(packet: &RoomDrawPacket) -> u64 {
    let count = u64::from(packet.count);
    match packet.primitive {
        1 => count,                       // point list
        2 => count / 2,                   // line list
        3 => count.saturating_sub(1),     // line strip
        4 => count / 3,                   // triangle list
        5 | 6 => count.saturating_sub(2), // triangle strip or fan
        _ => 0,                           // preserve unknown type, report no derived total
    }
}

pub fn enable_from_env() {
    let mode: u8 = match std::env::var("TSFP_NATIVE_RENDER_PREP").as_deref() {
        Ok("1") => 1,
        Ok("2") => 2,
        _ => 0,
    };
    let enabled = mode != 0;
    let capture_interval = draw_capture_sample_interval(
        usize::from(mode),
        std::env::var("TSFP_NATIVE_RENDER_PREP_CAPTURE_EVERY")
            .ok()
            .as_deref(),
    );
    MODE.store(mode, Ordering::Release);
    DRAW_CAPTURE_ENABLED.store(capture_interval != 0, Ordering::Release);
    ENABLED.store(enabled, Ordering::Release);
    if mode == 2 {
        tsfp_render_preview::install();
    }
    #[cfg(not(tsfp_no_host))]
    unsafe {
        tsfp_native_scene_bridge_set_draw_capture_sample_interval(capture_interval);
        tsfp_native_scene_bridge_enable(if enabled { 1 } else { 0 });
    }
    if enabled {
        let mode_name = if mode == 2 {
            "UNVERIFIED DrawList preview"
        } else {
            "audit only"
        };
        eprintln!(
            "tsfp-bevy: native render-prep {mode_name} enabled (legacy drawing retained) draw_capture_every={capture_interval}"
        );
    }
}

fn draw_capture_sample_interval(mode: usize, configured: Option<&str>) -> u32 {
    if let Some(raw) = configured {
        return raw.parse::<u32>().unwrap_or(0).min(3600);
    }
    match mode {
        // Mode 1 measures room preparation without enabling the expensive D3D
        // payload observer. Set CAPTURE_EVERY explicitly for sampled packets.
        1 => 0,
        // Mode 2 is a diagnostic comparison. One bounded capture per 600 room
        // collector scopes limits payload copies to occasional samples.
        2 => 600,
        _ => 0,
    }
}

#[derive(Resource)]
struct AuditState {
    live_views: bool,
    owner: NativeSceneOwner,
    sink: SceneFrameAuditSink,
    seen_sequence: u64,
    missed_publications: u64,
    rooms: [u8; ROOM_CAPACITY],
    object_records: [u32; OBJECT_CAPACITY],
    view: NativeSceneView,
    seen_draw_sequence: u64,
    report_packets: Vec<RoomDrawPacket>,
    payload_cache: BTreeMap<(u64, usize), Vec<std::sync::Arc<[u8]>>>,
    last_unscoped: u64,
    last_dropped: u64,
    last_scope_errors: u64,
    last_capture_calls: u64,
    last_capture_elapsed_ns: u64,
    last_capture_payload_bytes: u64,
    last_report: std::time::Instant,
    prep_updates: u64,
    prep_cpu_us: u64,
    prep_cpu_us_max: u64,
}

fn is_admitted_room_packet(
    frame_sequence: u64,
    room: u32,
    tick: u64,
    room_ids: &BTreeSet<u8>,
) -> bool {
    frame_sequence == tick && (1..=200).contains(&room) && room_ids.contains(&(room as u8))
}

pub fn install(app: &mut App) {
    if ENABLED.load(Ordering::Acquire) {
        app.insert_resource(AuditState {
            live_views: tsfp_render_prep::live::enabled(
                std::env::var("TSFP_NATIVE_RENDER_PREP_VIEWS")
                    .ok()
                    .as_deref(),
            ),
            owner: NativeSceneOwner::default(),
            sink: SceneFrameAuditSink::default(),
            seen_sequence: 0,
            missed_publications: 0,
            rooms: [0; ROOM_CAPACITY],
            object_records: [0; OBJECT_CAPACITY],
            view: NativeSceneView::default(),
            seen_draw_sequence: 0,
            report_packets: Vec::new(),
            payload_cache: BTreeMap::new(),
            last_unscoped: 0,
            last_dropped: 0,
            last_scope_errors: 0,
            last_capture_calls: 0,
            last_capture_elapsed_ns: 0,
            last_capture_payload_bytes: 0,
            last_report: std::time::Instant::now(),
            prep_updates: 0,
            prep_cpu_us: 0,
            prep_cpu_us_max: 0,
        })
        .add_systems(Update, sample_and_submit);
    }
}

fn sample_and_submit(mut state: ResMut<AuditState>) {
    let update_started = std::time::Instant::now();
    let mut frame_published = false;
    #[cfg(not(tsfp_no_host))]
    {
        // Drain the bounded FIFO in collector order before sampling this
        // display frame, preserving every queued owner publication.
        for _ in 0..QUEUE_CAPACITY {
            let (
                mut sequence,
                mut room_count,
                mut kind,
                mut object_count,
                mut view_count,
                mut draw_depth,
            ) = (0u64, 0u32, 0u32, 0u32, 0u32, -1i32);
            let published = unsafe {
                tsfp_native_scene_bridge_read(
                    state.seen_sequence,
                    &mut sequence,
                    state.rooms.as_mut_ptr(),
                    ROOM_CAPACITY as u32,
                    &mut room_count,
                    &mut draw_depth,
                    &mut kind,
                    state.object_records.as_mut_ptr(),
                    OBJECT_CAPACITY as u32,
                    &mut object_count,
                    &mut state.view,
                    &mut view_count,
                )
            };
            if published == 0 {
                break;
            }
            if sequence > state.seen_sequence.saturating_add(1) {
                state.missed_publications += sequence - state.seen_sequence - 1;
            }
            if kind == RETIRE_KIND {
                if object_count == 1 {
                    let record = state.object_records[0];
                    state.owner.retire_pool_record(record);
                } else {
                    eprintln!("tsfp-bevy: native render-prep retire event malformed");
                }
                state.seen_sequence = sequence;
                continue;
            }
            let view = NativeViewInput {
                viewer: state.view.viewer,
                position: state.view.position,
                matrix: state.view.matrix,
                viewport: state.view.viewport,
                rectangle: state.view.rectangle,
            };
            let views: &[NativeViewInput] = if view_count == 1 {
                std::slice::from_ref(&view)
            } else {
                &[]
            };
            let publish_result = {
                let AuditState {
                    owner,
                    rooms,
                    object_records,
                    ..
                } = &mut *state;
                owner
                    .publish_room_plan(sequence, &rooms[..room_count as usize], draw_depth)
                    .and_then(|()| {
                        owner
                            .publish_pool_record_inputs(
                                sequence,
                                &object_records[..object_count as usize],
                                views,
                            )
                            .and_then(|()| owner.finish_tick(sequence))
                    })
            };
            match publish_result {
                Ok(()) => {
                    state.seen_sequence = sequence;
                    frame_published = true;
                }
                Err(error) => {
                    eprintln!("tsfp-bevy: native render-prep publication rejected: {error:?}");
                    break;
                }
            }
        }
    }
    #[cfg(not(tsfp_no_host))]
    if DRAW_CAPTURE_ENABLED.load(Ordering::Acquire) {
        // Draw entries carry the current room read from the original
        // 0xA79B0 guest stack argument, plus copied source spans.
        for _ in 0..8192 {
            let (mut sequence, mut raw, mut payload_len) =
                (0u64, NativeSceneDrawPacket::default(), 0u32);
            let published = unsafe {
                tsfp_native_scene_bridge_read_draw(
                    state.seen_draw_sequence,
                    &mut sequence,
                    &mut raw,
                    std::ptr::null_mut(),
                    0,
                    &mut payload_len,
                )
            };
            if published == 0 {
                break;
            }
            state.seen_draw_sequence = sequence;
            let mut payload = if payload_len == 0 || payload_len > 32 * 1024 * 1024 {
                Vec::new()
            } else {
                vec![0u8; payload_len as usize]
            };
            if !payload.is_empty() {
                let expected_sequence = sequence;
                let copied = unsafe {
                    tsfp_native_scene_bridge_read_draw(
                        sequence.saturating_sub(1),
                        &mut sequence,
                        &mut raw,
                        payload.as_mut_ptr(),
                        payload.len() as u32,
                        &mut payload_len,
                    )
                };
                if copied != expected_sequence
                    || sequence != expected_sequence
                    || payload_len as usize != payload.len()
                {
                    raw.payload_dropped = 1;
                    payload.clear();
                }
            }
            let (payload_hash, payload) = intern_payload(&mut state.payload_cache, payload);
            state.report_packets.push(RoomDrawPacket {
                frame_sequence: raw.frame_sequence,
                room: raw.room,
                indexed: raw.indexed,
                primitive: raw.primitive,
                first: raw.first,
                count: raw.count,
                vertex_buffer: raw.vertex_buffer,
                vertex_buffer_bytes: raw.vertex_buffer_bytes,
                vertex_stride: raw.vertex_stride,
                vertex_first: raw.vertex_first,
                index_buffer: raw.index_buffer,
                index_buffer_bytes: raw.index_buffer_bytes,
                texture_headers: raw.texture_headers,
                texture_data: raw.texture_data,
                texture_formats: raw.texture_formats,
                texture_sizes: raw.texture_sizes,
                texture_stage_state: raw.texture_stage_state,
                viewport_words: raw.viewport_words,
                viewport_target: raw.viewport_target,
                scissor_words: raw.scissor_words,
                scissor_count: raw.scissor_count,
                scissor_exclusive: raw.scissor_exclusive,
                vertex_declaration: raw.vertex_declaration,
                vertex_shader_handle: raw.vertex_shader_handle,
                vertex_shader_index: raw.vertex_shader_index,
                vertex_program_bytes: raw.vertex_program_bytes,
                vertex_constants_bytes: raw.vertex_constants_bytes,
                pixel_shader_bytes: raw.pixel_shader_bytes,
                vertex_program_hash: raw.vertex_program_hash,
                vertex_constants_hash: raw.vertex_constants_hash,
                pixel_shader_hash: raw.pixel_shader_hash,
                blend_enable: raw.blend_enable,
                blend_source: raw.blend_source,
                blend_destination: raw.blend_destination,
                blend_operation: raw.blend_operation,
                depth_enable: raw.depth_enable,
                depth_write: raw.depth_write,
                depth_function: raw.depth_function,
                cull_mode: raw.cull_mode,
                stencil_enable: raw.stencil_enable,
                stencil_function: raw.stencil_function,
                stencil_reference: raw.stencil_reference,
                stencil_mask: raw.stencil_mask,
                stencil_write_mask: raw.stencil_write_mask,
                stencil_fail: raw.stencil_fail,
                stencil_depth_fail: raw.stencil_depth_fail,
                stencil_pass: raw.stencil_pass,
                vertex_bytes: raw.vertex_bytes,
                index_bytes: raw.index_bytes,
                texture_bytes: raw.texture_bytes,
                payload_dropped: raw.payload_dropped != 0,
                render_state: raw.render_state,
                payload_hash,
                payload,
                material_key: raw.material_key,
                state_key: raw.state_key,
                scene_light_binding: None,
            });
        }
        if frame_published {
            if let Some((tick, room_ids)) = state.owner.room_plan().map(|plan| {
                (
                    plan.tick,
                    plan.rooms.iter().copied().collect::<BTreeSet<_>>(),
                )
            }) {
                let current: Vec<_> = state
                    .report_packets
                    .iter()
                    .cloned()
                    .filter(|packet| {
                        is_admitted_room_packet(packet.frame_sequence, packet.room, tick, &room_ids)
                    })
                    .collect();
                if MODE.load(Ordering::Acquire) == 2 && !current.is_empty() {
                    if let Some(path) = std::env::var_os("TSFP_PREVIEW_PACKET_CAPTURE") {
                        // UNVERIFIED offline sidecar for tsfp-render-preview's render_capture.
                        let [l, t, r, b] = state.view.rectangle;
                        let size = std::fs::metadata(&path).map_or(0, |m| m.len());
                        if size < 150 << 20
                            && let Ok(mut file) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path)
                        {
                            let camera = tsfp_render_preview::capture::CapturedCamera {
                                eye: state.view.camera.eye,
                                view: state.view.camera.view,
                                projection: state.view.camera.projection,
                                position: state.view.position,
                                matrix: state.view.matrix,
                                viewport: state.view.viewport,
                            };
                            let _ = tsfp_render_preview::capture::write_frame_geometry(
                                &mut file,
                                tick,
                                [(r - l).max(0) as u32, (b - t).max(0) as u32],
                                &camera,
                                &current,
                                1 << 20,
                            );
                        }
                    }
                    if let Err(error) =
                        tsfp_render_preview::publish_captured_draws(&current, &state.view, tick)
                    {
                        eprintln!("tsfp-bevy: captured geometry preview rejected frame: {error:?}");
                    }
                }
                if let Err(error) = state.owner.publish_room_draw_packets(tick, &current) {
                    eprintln!("tsfp-bevy: native render-prep draw packets rejected: {error:?}");
                }
            }
        }
    }
    if frame_published && state.owner.room_plan().is_some() {
        let AuditState {
            owner,
            sink,
            view,
            seen_sequence,
            live_views,
            ..
        } = &mut *state;
        let objects = owner.observed_objects();
        let views = owner.observed_views();
        sink.observe_owner_inputs(objects, views);
        match prepare_bridge_frame(owner, *seen_sequence, view, *live_views) {
            Ok(frame) => {
                let lease = PreparedSceneLease::new(frame);
                tsfp_scene_capture::submit(sink, lease.frame(), owner.room_draw_packets(), view);
            }
            Err(error) => {
                tsfp_scene_capture::capture_live_rejection(owner, view, &error);
                eprintln!(
                    "tsfp-bevy: native render-prep sample rejected: {error:?} camera_eye_mismatch={} camera_matrix_mismatch={}",
                    view.camera.eye != view.position,
                    view.camera.view != view.matrix
                );
            }
        }
    } else {
    }
    let update_us = update_started
        .elapsed()
        .as_micros()
        .min(u128::from(u64::MAX)) as u64;
    state.prep_updates = state.prep_updates.saturating_add(1);
    state.prep_cpu_us = state.prep_cpu_us.saturating_add(update_us);
    state.prep_cpu_us_max = state.prep_cpu_us_max.max(update_us);
    if state.last_report.elapsed() >= std::time::Duration::from_secs(1) {
        let summary = state.sink.take_summary();
        let rooms = state.owner.room_plan().map_or(0, |plan| plan.rooms.len());
        let tick = state.owner.current_tick().unwrap_or(0);
        eprintln!(
            "tsfp-bevy: native-render-prep frames={} tick={} rooms={} room_samples={} objects={} views={} effects={} missed_publications={} order_errors={} room_order_fnv={:016x} prep_updates={} prep_cpu_us_avg={} prep_cpu_us_max={}",
            summary.frames,
            summary.last_tick.unwrap_or(tick),
            rooms,
            summary.rooms,
            summary.objects,
            summary.views,
            summary.effects,
            state.missed_publications,
            summary.order_errors,
            summary.order_hash.unwrap_or(0),
            state.prep_updates,
            state.prep_cpu_us / state.prep_updates.max(1),
            state.prep_cpu_us_max,
        );
        let mut by_room: BTreeMap<u32, RoomDrawCounts> = BTreeMap::new();
        for packet in &state.report_packets {
            if packet.room == 0 {
                continue;
            }
            let entry = by_room.entry(packet.room).or_default();
            let primitives = primitive_count(packet);
            entry.packets += 1;
            entry.primitives += primitives;
            entry.vertices += u64::from(packet.vertex_bytes / packet.vertex_stride.max(1));
            entry.index_bytes += u64::from(packet.index_bytes);
            entry.texture_bytes += packet
                .texture_bytes
                .iter()
                .map(|bytes| u64::from(*bytes))
                .sum::<u64>();
            for stage in 0..4 {
                if packet.texture_headers[stage] != 0 {
                    entry.textures.insert((
                        packet.texture_headers[stage],
                        packet.texture_formats[stage],
                        packet.texture_bytes[stage],
                    ));
                }
            }
            *entry.by_primitive.entry(packet.primitive).or_default() += primitives;
            entry.materials.insert(packet.material_key);
        }
        let mut unscoped = 0u64;
        let mut dropped = 0u64;
        let mut scope_errors = 0u64;
        let payload_incomplete = state
            .report_packets
            .iter()
            .filter(|packet| packet.payload_dropped)
            .count();
        #[cfg(not(tsfp_no_host))]
        unsafe {
            tsfp_native_scene_bridge_draw_counters(&mut unscoped, &mut dropped, &mut scope_errors);
        }
        let mut capture_calls = 0u64;
        let mut capture_elapsed_ns = 0u64;
        let mut capture_payload_bytes = 0u64;
        #[cfg(not(tsfp_no_host))]
        unsafe {
            tsfp_native_scene_bridge_draw_capture_timing(
                &mut capture_calls,
                &mut capture_elapsed_ns,
                &mut capture_payload_bytes,
            );
        }
        let capture_calls_delta = capture_calls.saturating_sub(state.last_capture_calls);
        let capture_elapsed_us_delta =
            capture_elapsed_ns.saturating_sub(state.last_capture_elapsed_ns) / 1_000;
        let capture_payload_delta =
            capture_payload_bytes.saturating_sub(state.last_capture_payload_bytes);
        let mut collector_guest_calls = 0u64;
        let mut collector_adapter_entries = 0u64;
        let mut collector_accepted = 0u64;
        let mut collector_bad_count = 0u64;
        let mut collector_bad_order = 0u64;
        let mut collector_last_room_count = 0i32;
        let mut collector_last_draw_depth = 0i32;
        #[cfg(not(tsfp_no_host))]
        unsafe {
            tsfp_native_scene_bridge_collector_counters(
                &mut collector_guest_calls,
                &mut collector_adapter_entries,
                &mut collector_accepted,
                &mut collector_bad_count,
                &mut collector_bad_order,
                &mut collector_last_room_count,
                &mut collector_last_draw_depth,
            );
        }
        let whole_pass: Vec<_> = state
            .report_packets
            .iter()
            .filter(|packet| packet.room == 0)
            .collect();
        let whole_pass_primitives: u64 = whole_pass
            .iter()
            .map(|packet| primitive_count(packet))
            .sum();
        let whole_pass_materials: BTreeSet<_> = whole_pass
            .iter()
            .map(|packet| packet.material_key)
            .collect();
        let room_stats = by_room
            .iter()
            .map(|(room, counts)| {
                let by_primitive = counts
                    .by_primitive
                    .iter()
                    .map(|(primitive, count)| format!("{primitive}:{count}"))
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{room}:packets={},primitives={},vertices={},index_bytes={},textures={},texture_bytes={},by_type=[{}],materials={}",
                    counts.packets,
                    counts.primitives,
                    counts.vertices,
                    counts.index_bytes,
                    counts.textures.len(),
                    counts.texture_bytes,
                    by_primitive,
                    counts.materials.len()
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        eprintln!(
            "tsfp-bevy: native-render-prep-draw rooms=[{}] unattributed={} whole_pass_packets={} whole_pass_primitives={} whole_pass_materials={} payload_incomplete={} packet_drops={} capture_scope_errors={} collector_calls={} collector_adapter={} collector_accepted={} collector_bad_count={} collector_bad_order={} collector_last_count={} collector_last_depth={} draw_hook_calls={} draw_hook_elapsed_us={} draw_hook_payload_bytes={}",
            room_stats,
            unscoped.saturating_sub(state.last_unscoped),
            whole_pass.len(),
            whole_pass_primitives,
            whole_pass_materials.len(),
            payload_incomplete,
            dropped.saturating_sub(state.last_dropped),
            scope_errors.saturating_sub(state.last_scope_errors),
            collector_guest_calls,
            collector_adapter_entries,
            collector_accepted,
            collector_bad_count,
            collector_bad_order,
            collector_last_room_count,
            collector_last_draw_depth,
            capture_calls_delta,
            capture_elapsed_us_delta,
            capture_payload_delta,
        );
        state.last_unscoped = unscoped;
        state.last_dropped = dropped;
        state.last_scope_errors = scope_errors;
        state.last_capture_calls = capture_calls;
        state.last_capture_elapsed_ns = capture_elapsed_ns;
        state.last_capture_payload_bytes = capture_payload_bytes;
        state.report_packets.clear();
        state.payload_cache.clear();
        state.missed_publications = 0;
        state.last_report = std::time::Instant::now();
        state.prep_updates = 0;
        state.prep_cpu_us = 0;
        state.prep_cpu_us_max = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_draw_admission_rejects_unscoped_or_stale_packets() {
        let rooms = BTreeSet::from([1, 4]);
        assert!(is_admitted_room_packet(9, 4, 9, &rooms));
        assert!(!is_admitted_room_packet(9, u32::MAX, 9, &rooms));
        assert!(!is_admitted_room_packet(9, 0, 9, &rooms));
        assert!(!is_admitted_room_packet(9, 3, 9, &rooms));
        assert!(!is_admitted_room_packet(8, 4, 9, &rooms));
    }

    #[test]
    fn owned_packet_payload_dedupes_by_hash_and_content() {
        let mut cache = BTreeMap::new();
        let (first_hash, first) = intern_payload(&mut cache, vec![1, 2, 3, 4]);
        let (same_hash, same) = intern_payload(&mut cache, vec![1, 2, 3, 4]);
        let (other_hash, other) = intern_payload(&mut cache, vec![1, 2, 3, 5]);
        assert_eq!(first_hash, same_hash);
        assert!(std::sync::Arc::ptr_eq(&first, &same));
        assert_ne!(first_hash, other_hash);
        assert!(!std::sync::Arc::ptr_eq(&first, &other));
    }

    #[test]
    fn draw_capture_is_disabled_for_audit_and_sampled_for_comparison() {
        assert_eq!(draw_capture_sample_interval(0, None), 0);
        assert_eq!(draw_capture_sample_interval(1, None), 0);
        assert_eq!(draw_capture_sample_interval(2, None), 600);
        assert_eq!(draw_capture_sample_interval(1, Some("120")), 120);
        assert_eq!(draw_capture_sample_interval(2, Some("0")), 0);
        assert_eq!(draw_capture_sample_interval(2, Some("7200")), 3600);
        assert_eq!(draw_capture_sample_interval(2, Some("bad")), 0);
    }
}
