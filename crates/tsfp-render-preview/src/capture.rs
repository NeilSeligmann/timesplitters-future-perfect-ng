// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline sidecar for the captured-packet DrawList preview (UNVERIFIED).
//!
//! The sidecar records bounded per-draw geometry inputs for offline inspection.
//! `frame_to_packets` restores the captured buffers when present and uses
//! placeholder geometry for older or payload-free records.
use std::io::{self, Write};
use std::sync::Arc;

use tsfp_engine_render::native_scene::RoomDrawPacket;

pub const MAX_PACKET_COUNT: u32 = 3 * 4096;

#[derive(Clone, Debug, PartialEq)]
pub struct CapturedFrame {
    pub tick: u64,
    pub extent: [u32; 2],
    /// (room, material_key, state_key, count)
    pub draws: Vec<(u32, u64, u64, u32)>,
    /// Owner camera (eye, view, projection) when the v2 sidecar recorded it.
    pub camera: Option<CapturedCamera>,
    /// One entry per draw when geometry was recorded, else empty.
    pub geometry: Vec<Option<CapturedGeometry>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CapturedCamera {
    pub eye: [f32; 3],
    pub view: [f32; 16],
    pub projection: [f32; 16],
    /// Guest camera record copy (`NativeSceneView`), set even when the owner sample is zero.
    pub position: [f32; 3],
    pub matrix: [f32; 16],
    pub viewport: [f32; 4],
}

/// Raw draw-time buffers (UNVERIFIED decode; selectors preserve source metadata).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedGeometry {
    pub stride: u32,
    pub indexed: u32,
    pub primitive: u32,
    pub first: u32,
    pub vertex_first: u32,
    pub vertices: Vec<u8>,
    pub indices: Vec<u8>,
    pub constants: Vec<u8>,
    /// NV2A vertex program microcode (16 bytes per slot) bound at draw time.
    pub program: Vec<u8>,
    /// Program entry slot, absent in older sidecars.
    pub program_start: Option<u32>,
    /// Original D3D vertex declaration selector, absent in older sidecars.
    pub vertex_declaration: Option<u32>,
    /// Raw 16-entry D3D vertex declaration block, absent in older sidecars.
    pub vertex_declaration_data: Vec<u8>,
    /// Draw-time NV2A register 1 stream from the active vertex-array binding.
    pub register1: Option<CapturedVertexInput>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedVertexInput {
    pub stride: u32,
    pub first: u32,
    pub format: u32,
    pub vertices: Vec<u8>,
}

pub const VERTEX_DECLARATION_BYTES: usize = 16 * 16;
const REGISTER1_TRAILER_MAGIC: &[u8; 4] = b"R1VS";
const REGISTER1_TRAILER_BYTES: usize = 24;
pub(crate) const FLOAT3_REGISTER_FORMAT: u32 = 0x32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VertexAttributeDeclaration {
    pub register: u32,
    pub offset: u32,
    pub kind: u32,
}

/// Decode the captured 16-entry guest declaration block. Each entry stores its
/// byte offset at +4 and NV2A attribute type at +8; type 2 denotes an unused slot.
pub fn decode_vertex_declaration(bytes: &[u8]) -> Option<Vec<VertexAttributeDeclaration>> {
    if bytes.len() != VERTEX_DECLARATION_BYTES {
        return None;
    }
    let mut attributes = Vec::new();
    for register in 0..16u32 {
        let base = register as usize * 16;
        let word = |offset: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                bytes[base + offset..base + offset + 4].try_into().ok()?,
            ))
        };
        let kind = word(8)?;
        if kind != 2 {
            attributes.push(VertexAttributeDeclaration {
                register,
                offset: word(4)?,
                kind,
            });
        }
    }
    Some(attributes)
}

fn decode_render_declaration_layout(
    bytes: &[u8],
    offset_word: usize,
    kind_word: usize,
) -> Option<Vec<VertexAttributeDeclaration>> {
    if bytes.len() != VERTEX_DECLARATION_BYTES {
        return None;
    }
    let mut attributes = Vec::new();
    for register in 0..16u32 {
        let base = register as usize * 16;
        let word = |index: usize| -> Option<u32> {
            Some(u32::from_le_bytes(
                bytes[base + index * 4..base + index * 4 + 4]
                    .try_into()
                    .ok()?,
            ))
        };
        let kind = word(kind_word)?;
        if kind == 0 || kind == 2 {
            continue;
        }
        if !matches!(kind, 0x16 | 0x22 | 0x25 | 0x32 | 0x40) {
            return None;
        }
        let offset = word(offset_word)?;
        if offset > 4096 {
            return None;
        }
        attributes.push(VertexAttributeDeclaration {
            register,
            offset,
            kind,
        });
    }
    (!attributes.is_empty()).then_some(attributes)
}

/// Decode either the ported builder block (+4 offset, +8 kind) or the measured
/// captured block (+8 offset, +12 kind). Ambiguous or unknown layouts fail closed.
pub fn decode_vertex_declaration_for_render(
    bytes: &[u8],
) -> Option<Vec<VertexAttributeDeclaration>> {
    let builder = decode_render_declaration_layout(bytes, 1, 2);
    let captured = decode_render_declaration_layout(bytes, 2, 3);
    match (builder, captured) {
        (Some(builder), None) => Some(builder),
        (None, Some(captured)) => Some(captured),
        (Some(builder), Some(captured)) if builder == captured => Some(builder),
        _ => None,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text == "-" {
        return Some(Vec::new());
    }
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}

fn word_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

pub(crate) fn decode_register1_trailer(bytes: &[u8]) -> Result<Option<CapturedVertexInput>, ()> {
    if !bytes.starts_with(REGISTER1_TRAILER_MAGIC) {
        return Ok(None);
    }
    if bytes.len() < REGISTER1_TRAILER_BYTES
        || word_at(bytes, 4) != Some(1)
        || word_at(bytes, 16) != Some(FLOAT3_REGISTER_FORMAT)
    {
        return Err(());
    }
    let stride = word_at(bytes, 8).ok_or(())?;
    let first = word_at(bytes, 12).ok_or(())?;
    let byte_count = word_at(bytes, 20).ok_or(())? as usize;
    let end = REGISTER1_TRAILER_BYTES.checked_add(byte_count).ok_or(())?;
    if stride < 12 || byte_count == 0 || byte_count % stride as usize != 0 || end != bytes.len() {
        return Err(());
    }
    Ok(Some(CapturedVertexInput {
        stride,
        first,
        format: FLOAT3_REGISTER_FORMAT,
        vertices: bytes[REGISTER1_TRAILER_BYTES..end].to_vec(),
    }))
}

fn floats(values: &[f32]) -> String {
    values
        .iter()
        .map(|v| v.to_bits().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// v2 record: `write_frame` plus camera (`C`) and per draw geometry (`G`).
pub fn write_frame_geometry(
    out: &mut impl Write,
    tick: u64,
    extent: [u32; 2],
    camera: &CapturedCamera,
    packets: &[RoomDrawPacket],
    max_payload: usize,
) -> io::Result<()> {
    writeln!(
        out,
        "F {tick} {} {} {}",
        extent[0],
        extent[1],
        packets.len()
    )?;
    writeln!(
        out,
        "C {} {} {} {} {} {}",
        floats(&camera.eye),
        floats(&camera.view),
        floats(&camera.projection),
        floats(&camera.position),
        floats(&camera.matrix),
        floats(&camera.viewport)
    )?;
    for p in packets {
        writeln!(
            out,
            "D {} {} {} {}",
            p.room, p.material_key, p.state_key, p.count
        )?;
        let v = p.vertex_bytes as usize;
        let i = p.index_bytes as usize;
        let tex: usize = p.texture_bytes.iter().map(|&b| b as usize).sum();
        let vp = p.vertex_program_bytes as usize;
        let vc = p.vertex_constants_bytes as usize;
        let ps = p.pixel_shader_bytes as usize;
        let known_payload_bytes = v + i + tex + vp + vc + ps;
        let ok = !p.payload_dropped && known_payload_bytes <= p.payload.len() && v <= max_payload;
        if ok {
            let vertices = &p.payload[..v];
            let indices = &p.payload[v..v + i];
            let constants = &p.payload[v + i + tex + vp..v + i + tex + vp + vc];
            let program = &p.payload[v + i + tex..v + i + tex + vp];
            let trailing = &p.payload[known_payload_bytes..];
            let has_declaration = p.vertex_declaration != 0
                && !trailing.starts_with(REGISTER1_TRAILER_MAGIC)
                && trailing.len() >= VERTEX_DECLARATION_BYTES;
            let (declaration, input_tail) = if has_declaration {
                (
                    &trailing[..VERTEX_DECLARATION_BYTES],
                    &trailing[VERTEX_DECLARATION_BYTES..],
                )
            } else {
                (&[][..], trailing)
            };
            let register1 = decode_register1_trailer(input_tail).map_err(|()| {
                io::Error::new(io::ErrorKind::InvalidData, "bad register 1 capture trailer")
            })?;
            writeln!(
                out,
                "G {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
                p.vertex_stride,
                p.indexed,
                p.primitive,
                p.first,
                p.vertex_first,
                hex(vertices),
                if i == 0 { "-".into() } else { hex(indices) },
                if vc == 0 { "-".into() } else { hex(constants) },
                if vp == 0 { "-".into() } else { hex(program) },
                p.vertex_shader_index,
                p.vertex_declaration,
                if declaration.is_empty() {
                    "-".into()
                } else {
                    hex(declaration)
                },
                register1
                    .as_ref()
                    .map_or("-".into(), |input| input.stride.to_string()),
                register1
                    .as_ref()
                    .map_or("-".into(), |input| input.first.to_string()),
                register1
                    .as_ref()
                    .map_or("-".into(), |input| input.format.to_string()),
                register1
                    .as_ref()
                    .map_or("-".into(), |input| hex(&input.vertices))
            )?;
        } else {
            writeln!(out, "G -")?;
        }
    }
    out.flush()
}

pub fn write_frame(
    out: &mut impl Write,
    tick: u64,
    extent: [u32; 2],
    packets: &[RoomDrawPacket],
) -> io::Result<()> {
    writeln!(
        out,
        "F {tick} {} {} {}",
        extent[0],
        extent[1],
        packets.len()
    )?;
    for p in packets {
        writeln!(
            out,
            "D {} {} {} {}",
            p.room, p.material_key, p.state_key, p.count
        )?;
    }
    out.flush()
}

pub fn read_frames(text: &str) -> Result<Vec<CapturedFrame>, String> {
    let mut frames: Vec<CapturedFrame> = Vec::new();
    let mut expected = 0usize;
    for (n, line) in text.lines().enumerate() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| -> Result<u64, String> {
            f.get(i)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("line {}: bad field {i}", n + 1))
        };
        match f.first().copied() {
            Some("F") => {
                if expected != 0 {
                    return Err(format!("line {}: frame truncated", n + 1));
                }
                expected = num(4)? as usize;
                frames.push(CapturedFrame {
                    tick: num(1)?,
                    extent: [num(2)? as u32, num(3)? as u32],
                    draws: Vec::new(),
                    camera: None,
                    geometry: Vec::new(),
                });
            }
            Some("D") => {
                let frame = frames.last_mut().ok_or("draw before frame")?;
                frame
                    .draws
                    .push((num(1)? as u32, num(2)?, num(3)?, num(4)? as u32));
                expected = expected.saturating_sub(1);
            }
            Some("C") => {
                let frame = frames.last_mut().ok_or("camera before frame")?;
                let mut v = [0f32; 58];
                for (k, slot) in v.iter_mut().enumerate() {
                    *slot = f32::from_bits(num(k + 1).unwrap_or(0) as u32);
                }
                frame.camera = Some(CapturedCamera {
                    eye: v[0..3].try_into().unwrap(),
                    view: v[3..19].try_into().unwrap(),
                    projection: v[19..35].try_into().unwrap(),
                    position: v[35..38].try_into().unwrap(),
                    matrix: v[38..54].try_into().unwrap(),
                    viewport: v[54..58].try_into().unwrap(),
                });
            }
            Some("G") => {
                let frame = frames.last_mut().ok_or("geometry before frame")?;
                if f.get(1) == Some(&"-") {
                    frame.geometry.push(None);
                } else {
                    let bytes = |i: usize| {
                        f.get(i)
                            .and_then(|t| unhex(t))
                            .ok_or_else(|| format!("line {}: bad hex {i}", n + 1))
                    };
                    frame.geometry.push(Some(CapturedGeometry {
                        stride: num(1)? as u32,
                        indexed: num(2)? as u32,
                        primitive: num(3)? as u32,
                        first: num(4)? as u32,
                        vertex_first: num(5)? as u32,
                        vertices: bytes(6)?,
                        indices: bytes(7)?,
                        constants: bytes(8)?,
                        program: if f.len() > 9 { bytes(9)? } else { Vec::new() },
                        program_start: if f.len() > 10 {
                            Some(num(10)? as u32)
                        } else {
                            None
                        },
                        vertex_declaration: if f.len() > 11 {
                            Some(num(11)? as u32)
                        } else {
                            None
                        },
                        vertex_declaration_data: if f.len() > 12 { bytes(12)? } else { Vec::new() },
                        register1: if f.len() > 16 && f[13] != "-" {
                            let stride = num(13)? as u32;
                            let first = num(14)? as u32;
                            let format = num(15)? as u32;
                            let vertices = bytes(16)?;
                            if stride < 12
                                || format != FLOAT3_REGISTER_FORMAT
                                || vertices.is_empty()
                                || vertices.len() % stride as usize != 0
                            {
                                return Err(format!("line {}: invalid register 1 geometry", n + 1));
                            }
                            Some(CapturedVertexInput {
                                stride,
                                first,
                                format,
                                vertices,
                            })
                        } else {
                            None
                        },
                    }));
                }
            }
            _ => return Err(format!("line {}: unknown record", n + 1)),
        }
    }
    if expected != 0 {
        return Err("last frame truncated".into());
    }
    Ok(frames)
}

const MAX_CAPTURED_INPUT_VERTICES: usize = 4096;
const MAX_CAPTURED_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Rebuild captured draw packets, falling back to placeholders for absent or
/// unsupported geometry records.
pub fn frame_to_packets(frame: &CapturedFrame) -> Vec<RoomDrawPacket> {
    frame
        .draws
        .iter()
        .enumerate()
        .map(|(index, &(room, material_key, state_key, draw_count))| {
            let count = draw_count.clamp(3, MAX_PACKET_COUNT);
            if let Some(geometry) = frame.geometry.get(index).and_then(Option::as_ref)
                && let Some(packet) = captured_geometry_packet(
                    frame.tick,
                    room,
                    material_key,
                    state_key,
                    count,
                    geometry,
                )
            {
                return packet;
            }

            placeholder_packet(frame.tick, room, material_key, state_key, count)
        })
        .collect()
}

fn placeholder_packet(
    tick: u64,
    room: u32,
    material_key: u64,
    state_key: u64,
    count: u32,
) -> RoomDrawPacket {
    let bytes = count * 12;
    let mut packet = RoomDrawPacket {
        frame_sequence: tick,
        room,
        vertex_stride: 12,
        vertex_buffer_bytes: bytes,
        count,
        vertex_bytes: bytes,
        payload: Arc::from(vec![0u8; bytes as usize]),
        material_key,
        state_key,
        ..blank()
    };
    packet.primitive = 4;
    packet
}

fn captured_geometry_packet(
    tick: u64,
    room: u32,
    material_key: u64,
    state_key: u64,
    count: u32,
    geometry: &CapturedGeometry,
) -> Option<RoomDrawPacket> {
    let stride = usize::try_from(geometry.stride).ok()?;
    if stride == 0
        || geometry.indexed > 1
        || count == 0
        || geometry.vertices.is_empty()
        || geometry.vertices.len() % stride != 0
    {
        return None;
    }
    let vertex_count = geometry.vertices.len() / stride;
    if vertex_count == 0 || vertex_count > MAX_CAPTURED_INPUT_VERTICES {
        return None;
    }
    let expected_indices = if geometry.indexed == 1 {
        usize::try_from(count).ok()?.checked_mul(2)?
    } else {
        0
    };
    if geometry.indices.len() != expected_indices
        || geometry.program.len() > 136 * 16
        || geometry.program.len() % 16 != 0
        || geometry.constants.len() % 16 != 0
        || (!geometry.vertex_declaration_data.is_empty()
            && (geometry.vertex_declaration_data.len() != VERTEX_DECLARATION_BYTES
                || geometry.vertex_declaration.unwrap_or(0) == 0))
    {
        return None;
    }
    let register1_bytes = geometry.register1.as_ref().map_or(0, |input| {
        input.vertices.len().saturating_add(REGISTER1_TRAILER_BYTES)
    });
    let payload_len = geometry
        .vertices
        .len()
        .checked_add(geometry.indices.len())?
        .checked_add(geometry.program.len())?
        .checked_add(geometry.constants.len())?
        .checked_add(geometry.vertex_declaration_data.len())?
        .checked_add(register1_bytes)?;
    if payload_len > MAX_CAPTURED_PAYLOAD_BYTES {
        return None;
    }

    let vertex_first = geometry.vertex_first;
    let vertex_end = u32::try_from(vertex_count)
        .ok()?
        .checked_add(vertex_first)?;
    let vertex_buffer_bytes = vertex_end.checked_mul(geometry.stride)?;
    let index_end = geometry.first.checked_add(count)?;
    let index_buffer_bytes = if geometry.indexed == 1 {
        index_end.checked_mul(2)?
    } else {
        0
    };
    let vertex_bytes = u32::try_from(geometry.vertices.len()).ok()?;
    let index_bytes = u32::try_from(geometry.indices.len()).ok()?;
    let vertex_program_bytes = u32::try_from(geometry.program.len()).ok()?;
    let vertex_constants_bytes = u32::try_from(geometry.constants.len()).ok()?;

    let mut payload = Vec::with_capacity(payload_len);
    payload.extend_from_slice(&geometry.vertices);
    payload.extend_from_slice(&geometry.indices);
    payload.extend_from_slice(&geometry.program);
    payload.extend_from_slice(&geometry.constants);
    payload.extend_from_slice(&geometry.vertex_declaration_data);
    if let Some(input) = &geometry.register1 {
        if input.format != FLOAT3_REGISTER_FORMAT
            || input.stride < 12
            || input.vertices.is_empty()
            || input.vertices.len() % input.stride as usize != 0
        {
            return None;
        }
        payload.extend_from_slice(REGISTER1_TRAILER_MAGIC);
        for value in [
            1,
            input.stride,
            input.first,
            input.format,
            u32::try_from(input.vertices.len()).ok()?,
        ] {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        payload.extend_from_slice(&input.vertices);
    }

    let packet = RoomDrawPacket {
        frame_sequence: tick,
        room,
        indexed: geometry.indexed,
        primitive: geometry.primitive,
        first: geometry.first,
        count,
        vertex_buffer_bytes,
        vertex_stride: geometry.stride,
        vertex_first,
        index_buffer_bytes,
        vertex_declaration: geometry.vertex_declaration.unwrap_or(0),
        vertex_shader_index: geometry.program_start.unwrap_or(0),
        vertex_program_bytes,
        vertex_constants_bytes,
        vertex_bytes,
        index_bytes,
        payload: Arc::from(payload),
        material_key,
        state_key,
        ..blank()
    };
    Some(packet)
}

fn blank() -> RoomDrawPacket {
    RoomDrawPacket {
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
        payload_dropped: false,
        render_state: [0; 256],
        payload_hash: 0,
        payload: Arc::from(Vec::<u8>::new()),
        material_key: 0,
        state_key: 0,
        scene_light_binding: None,
    }
}

/// Uncompressed (stored-deflate) RGBA8 PNG, deterministic bytes.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    for row in rgba.chunks_exact(width as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        z.extend_from_slice(&(c.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
        z.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    for (kind, data) in [(b"IHDR", ihdr), (b"IDAT", z), (b"IEND", Vec::new())] {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(&data);
        png.extend_from_slice(&body);
        png.extend_from_slice(&crc32(&body).to_be_bytes());
    }
    png
}

#[cfg(test)]
mod captured_packet_tests {
    use super::*;

    #[test]
    fn frame_packets_restore_bounded_declared_geometry_payload() {
        let mut declaration = vec![0; VERTEX_DECLARATION_BYTES];
        for register in 0..16 {
            declaration[register * 16 + 8..register * 16 + 12].copy_from_slice(&2u32.to_le_bytes());
        }
        declaration[16 + 4..16 + 8].copy_from_slice(&4u32.to_le_bytes());
        declaration[16 + 8..16 + 12].copy_from_slice(&FLOAT3_REGISTER_FORMAT.to_le_bytes());
        let geometry = CapturedGeometry {
            stride: 24,
            indexed: 1,
            primitive: 6,
            first: 2,
            vertex_first: 4,
            vertices: vec![0xA5; 72],
            indices: [4u16, 5, 6]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect(),
            constants: vec![0; 16],
            program: vec![0; 16],
            program_start: Some(7),
            vertex_declaration: Some(1),
            vertex_declaration_data: declaration.clone(),
            register1: None,
        };
        let frame = CapturedFrame {
            tick: 9,
            extent: [64, 48],
            draws: vec![(3, 5, 7, 3)],
            camera: None,
            geometry: vec![Some(geometry.clone())],
        };

        let packets = frame_to_packets(&frame);
        let packet = &packets[0];
        assert_eq!(packet.vertex_stride, 24);
        assert_eq!(packet.vertex_first, 4);
        assert_eq!(packet.vertex_buffer_bytes, 168);
        assert_eq!(packet.index_buffer_bytes, 10);
        assert_eq!(packet.vertex_shader_index, 7);
        assert_eq!(packet.vertex_declaration, 1);
        assert_eq!(packet.vertex_bytes, 72);
        assert_eq!(packet.index_bytes, 6);
        assert_eq!(&packet.payload[..72], geometry.vertices);
        assert_eq!(&packet.payload[72..78], geometry.indices);
        assert_eq!(&packet.payload[94..110], geometry.constants);
        assert_eq!(&packet.payload[110..366], declaration);
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (!(crc & 1)).wrapping_add(1));
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_round_trips_and_rebuilds_valid_packets() {
        let frame = CapturedFrame {
            tick: 9,
            extent: [64, 48],
            draws: vec![(3, 5, 7, 6), (4, 5, 7, 1)],
            camera: None,
            geometry: Vec::new(),
        };
        let mut out = Vec::new();
        out.extend_from_slice(b"F 9 64 48 2\nD 3 5 7 6\nD 4 5 7 1\n");
        assert_eq!(
            read_frames(std::str::from_utf8(&out).unwrap()).unwrap(),
            vec![frame.clone()]
        );
        assert!(read_frames("F 1 2 3 2\nD 1 1 1 1\n").is_err());
        let packets = frame_to_packets(&frame);
        assert_eq!(packets[1].count, 3);
        let mut view = tsfp_render_prep::live::NativeSceneView::default();
        view.camera_valid = 1;
        view.rectangle = [0, 0, 64, 48];
        crate::build_draw_list(&packets, &view, 9)
            .unwrap()
            .validate()
            .unwrap();
    }

    #[test]
    fn geometry_sidecar_round_trips() {
        let mut p = frame_to_packets(&CapturedFrame {
            tick: 1,
            extent: [8, 8],
            draws: vec![(1, 2, 3, 3)],
            camera: None,
            geometry: Vec::new(),
        });
        p[0].vertex_bytes = 36;
        p[0].index_bytes = 0;
        p[0].vertex_shader_index = 37;
        p[0].vertex_declaration = 42;
        let mut declaration = vec![0u8; VERTEX_DECLARATION_BYTES];
        for entry in 0..16 {
            let at = entry * 16 + 8;
            declaration[at..at + 4].copy_from_slice(&2u32.to_le_bytes());
        }
        declaration[16 + 4..16 + 8].copy_from_slice(&4u32.to_le_bytes());
        declaration[16 + 8..16 + 12].copy_from_slice(&0x32u32.to_le_bytes());
        let mut register1_trailer = b"R1VS".to_vec();
        for value in [1u32, 16, 0, FLOAT3_REGISTER_FORMAT, 48] {
            register1_trailer.extend_from_slice(&value.to_le_bytes());
        }
        let mut register1_vertices = Vec::new();
        for position in [[1.0f32, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]] {
            for value in position {
                register1_vertices.extend_from_slice(&value.to_le_bytes());
            }
            register1_vertices.extend_from_slice(&[0; 4]);
        }
        register1_trailer.extend_from_slice(&register1_vertices);
        p[0].payload = Arc::from([vec![7u8; 36], declaration.clone(), register1_trailer].concat());
        let cam = CapturedCamera {
            eye: [1.0, 2.0, 3.0],
            view: [0.5; 16],
            projection: [0.25; 16],
            position: [4.0, 5.0, 6.0],
            matrix: [0.75; 16],
            viewport: [8.0, 16.0, 28.0, 36.0],
        };
        let mut out = Vec::new();
        write_frame_geometry(&mut out, 1, [8, 8], &cam, &p, 1 << 20).unwrap();
        let frames = read_frames(std::str::from_utf8(&out).unwrap()).unwrap();
        assert_eq!(frames[0].camera.as_ref().unwrap(), &cam);
        let g = frames[0].geometry[0].as_ref().unwrap();
        assert_eq!((g.stride, g.vertices.len(), g.indices.len()), (12, 36, 0));
        assert_eq!(g.program_start, Some(37));
        assert_eq!(g.vertex_declaration, Some(42));
        assert_eq!(g.vertex_declaration_data, declaration);
        assert_eq!(
            g.register1,
            Some(CapturedVertexInput {
                stride: 16,
                first: 0,
                format: FLOAT3_REGISTER_FORMAT,
                vertices: register1_vertices,
            })
        );
        assert_eq!(
            decode_vertex_declaration(&g.vertex_declaration_data),
            Some(vec![VertexAttributeDeclaration {
                register: 1,
                offset: 4,
                kind: 0x32,
            }])
        );
        let old = "F 1 8 8 1\nD 1 2 3 3\nG 12 0 4 0 0 00000000 - - -\n";
        assert_eq!(
            read_frames(old).unwrap()[0].geometry[0]
                .as_ref()
                .unwrap()
                .program_start,
            None
        );
        assert_eq!(
            read_frames(old).unwrap()[0].geometry[0]
                .as_ref()
                .unwrap()
                .vertex_declaration,
            None
        );
        assert!(
            read_frames(old).unwrap()[0].geometry[0]
                .as_ref()
                .unwrap()
                .vertex_declaration_data
                .is_empty()
        );
    }

    #[test]
    fn captured_declaration_rows_decode_observed_offset_and_kind_words() {
        let mut declaration = vec![0u8; VERTEX_DECLARATION_BYTES];
        for register in 0..16 {
            declaration[register * 16 + 12..register * 16 + 16]
                .copy_from_slice(&2u32.to_le_bytes());
        }
        declaration[2 * 16 + 8..2 * 16 + 12].copy_from_slice(&4u32.to_le_bytes());
        declaration[2 * 16 + 12..2 * 16 + 16].copy_from_slice(&0x32u32.to_le_bytes());

        assert_eq!(
            decode_vertex_declaration_for_render(&declaration),
            Some(vec![VertexAttributeDeclaration {
                register: 2,
                offset: 4,
                kind: 0x32,
            }])
        );
    }

    #[test]
    fn png_is_deterministic_with_known_crc() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let png = encode_png(2, 1, &[1, 2, 3, 255, 4, 5, 6, 255]);
        assert_eq!(png, encode_png(2, 1, &[1, 2, 3, 255, 4, 5, 6, 255]));
        assert_eq!(&png[1..4], b"PNG");
    }
}
