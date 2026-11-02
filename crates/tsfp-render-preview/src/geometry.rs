// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded decoder for the measured register-1 preview geometry path.
use tsfp_engine_render::native_scene::RoomDrawPacket;
use tsfp_wgpu::vsh;

use crate::{
    FrameMesh, MeshRef,
    capture::{FLOAT3_REGISTER_FORMAT, VertexAttributeDeclaration},
    captured::CheckedPayload,
};

const MAX_INPUT_VERTICES: usize = 4096;
const MAX_DRAW_INDICES: usize = 16_384;
const FLOAT2_REGISTER_FORMAT: u32 = 0x22;
const UB_D3D4_REGISTER_FORMAT: u32 = 0x40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeometryDecodeRejection {
    InvalidRequest,
    InvalidProgramData,
    EmptyProgram,
    UnsupportedVertexInput,
    InvalidVertexBuffer,
    TooManyVertices,
    InvalidIndexData,
    UnsupportedPrimitive,
    OutputBudget,
    InvalidConstants,
    VertexIndexOutsideBuffer,
    InvalidPositionData,
    InvalidShaderPosition,
    InvalidClipPosition,
    NonFiniteClipPosition,
    InvalidDiffuse,
    InvalidVertexTransform,
}

fn float3(bytes: &[u8], offset: usize) -> Option<[f32; 3]> {
    let read = |component: usize| {
        let start = offset + component * 4;
        Some(f32::from_le_bytes(
            bytes.get(start..start + 4)?.try_into().ok()?,
        ))
    };
    let values = [read(0)?, read(1)?, read(2)?];
    values
        .iter()
        .all(|value| value.is_finite() && value.abs() < 1.0e6)
        .then_some(values)
}

fn float2(bytes: &[u8], offset: usize) -> Option<[f32; 2]> {
    let read = |component: usize| {
        let start = offset.checked_add(component.checked_mul(4)?)?;
        Some(f32::from_le_bytes(
            bytes.get(start..start.checked_add(4)?)?.try_into().ok()?,
        ))
    };
    let values = [read(0)?, read(1)?];
    values
        .iter()
        .all(|value| value.is_finite() && value.abs() < 1.0e6)
        .then_some(values)
}

fn captured_attribute(kind: u32, vertices: &[u8], offset: usize) -> Option<[f32; 4]> {
    match kind {
        FLOAT3_REGISTER_FORMAT => {
            let [x, y, z] = float3(vertices, offset)?;
            Some([x, y, z, 1.0])
        }
        FLOAT2_REGISTER_FORMAT => {
            let [x, y] = float2(vertices, offset)?;
            Some([x, y, 0.0, 1.0])
        }
        UB_D3D4_REGISTER_FORMAT => {
            let rgba = vertices.get(offset..offset.checked_add(4)?)?;
            Some([
                f32::from(rgba[2]) / 255.0,
                f32::from(rgba[1]) / 255.0,
                f32::from(rgba[0]) / 255.0,
                f32::from(rgba[3]) / 255.0,
            ])
        }
        _ => None,
    }
}

fn program_memory(bytes: &[u8], start: usize) -> Option<vsh::ProgramMemory> {
    if bytes.is_empty() || bytes.len() % 16 != 0 || bytes.len() > vsh::PROGRAM_SLOTS * 16 {
        return None;
    }
    let mut memory = vsh::ProgramMemory::default();
    let slots = bytes.len() / 16;
    if start >= slots {
        return None;
    }
    for (slot, chunk) in memory.slots.iter_mut().zip(bytes.chunks_exact(16)) {
        for (word, bytes) in slot.iter_mut().zip(chunk.chunks_exact(4)) {
            *word = u32::from_le_bytes(bytes.try_into().ok()?);
        }
    }
    Some(memory)
}

fn constants(bytes: &[u8]) -> Option<[[f32; 4]; vsh::CONSTANT_COUNT]> {
    if bytes.len() < 72 * 16 || bytes.len() % 16 != 0 || bytes.len() > vsh::CONSTANT_COUNT * 16 {
        return None;
    }
    let mut values = [[0.0; 4]; vsh::CONSTANT_COUNT];
    for (row, chunk) in values.iter_mut().zip(bytes.chunks_exact(16)) {
        for (value, bytes) in row.iter_mut().zip(chunk.chunks_exact(4)) {
            *value = f32::from_le_bytes(bytes.try_into().ok()?);
            if !value.is_finite() {
                return None;
            }
        }
    }
    Some(values)
}

fn triangles(primitive: u32, indices: &[usize]) -> Option<Vec<[usize; 3]>> {
    let mut result = Vec::new();
    match primitive {
        4 => result.extend(
            indices
                .chunks_exact(3)
                .map(|triangle| [triangle[0], triangle[1], triangle[2]]),
        ),
        5 => {
            for index in 2..indices.len() {
                result.push(if index % 2 == 0 {
                    [indices[index - 2], indices[index - 1], indices[index]]
                } else {
                    [indices[index - 1], indices[index - 2], indices[index]]
                });
            }
        }
        6 => {
            for index in 2..indices.len() {
                result.push([indices[0], indices[index - 1], indices[index]]);
            }
        }
        _ => return None,
    }
    (!result.is_empty()).then_some(result)
}

struct CapturedPositionInput<'a> {
    first: u32,
    stride: usize,
    offset: usize,
    register: usize,
    vertices: &'a [u8],
    attribute_stride: usize,
    attribute_vertices: &'a [u8],
    attributes: Vec<VertexAttributeDeclaration>,
}

fn program_reads_input(program: &[vsh::Instruction], register: usize) -> bool {
    program.iter().any(|instruction| {
        instruction.input == register
            && [instruction.a, instruction.b, instruction.c]
                .iter()
                .any(|source| source.mux == 2)
    })
}

fn shader_position_has_dividable_w(w: f32) -> bool {
    w.abs() > 1.0e-6
}

fn captured_position_input<'a>(
    packet: &'a RoomDrawPacket,
    payload: &'a CheckedPayload,
    program: &[vsh::Instruction],
) -> Option<CapturedPositionInput<'a>> {
    let declaration = payload.declaration.as_ref()?;
    let attribute_vertices = packet.payload.get(payload.vertices.clone())?;
    let attributes = |position_register| {
        declaration
            .iter()
            .filter(|attribute| {
                attribute.register as usize != position_register
                    && (attribute.register as usize) < vsh::INPUT_COUNT
                    && program_reads_input(program, attribute.register as usize)
                    && matches!(
                        attribute.kind,
                        FLOAT3_REGISTER_FORMAT | FLOAT2_REGISTER_FORMAT | UB_D3D4_REGISTER_FORMAT
                    )
            })
            .copied()
            .collect::<Vec<_>>()
    };
    if let Some(input) = payload
        .register1
        .as_ref()
        .filter(|_| program_reads_input(program, 1))
    {
        if input.format != FLOAT3_REGISTER_FORMAT
            || input.stride != 16
            || input.first != packet.vertex_first
        {
            return None;
        }
        return Some(CapturedPositionInput {
            first: input.first,
            stride: input.stride as usize,
            offset: 0,
            register: 1,
            vertices: &input.vertices,
            attribute_stride: packet.vertex_stride as usize,
            attribute_vertices,
            attributes: attributes(1),
        });
    }

    if packet.vertex_stride != 24 {
        return None;
    }
    let stride = packet.vertex_stride as usize;
    let vertices = packet.payload.get(payload.vertices.clone())?;
    let mut candidates = declaration.iter().filter(|attribute| {
        attribute.kind == FLOAT3_REGISTER_FORMAT
            && (attribute.offset as usize)
                .checked_add(12)
                .is_some_and(|end| end <= stride)
            && program_reads_input(program, attribute.register as usize)
    });
    let attribute = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(CapturedPositionInput {
        first: packet.vertex_first,
        stride,
        offset: attribute.offset as usize,
        register: attribute.register as usize,
        vertices,
        attribute_stride: stride,
        attribute_vertices: vertices,
        attributes: attributes(attribute.register as usize),
    })
}

/// Decode a bounded FLOAT3 input from either the existing register-1 trailer
/// or the measured 24-byte stream/declaration pair, then run the captured VSH.
pub(crate) fn decode_packet_mesh(
    packet: &RoomDrawPacket,
    payload: &CheckedPayload,
    mesh_id: MeshRef,
    extent: [u32; 2],
    output_budget: usize,
) -> Result<FrameMesh, GeometryDecodeRejection> {
    if extent.contains(&0) || packet.count as usize > MAX_DRAW_INDICES {
        return Err(GeometryDecodeRejection::InvalidRequest);
    }
    let shader_start = packet.vertex_shader_index as usize;
    let memory = program_memory(&packet.payload[payload.program.clone()], shader_start)
        .ok_or(GeometryDecodeRejection::InvalidProgramData)?;
    let program = vsh::decode_from(&memory, shader_start);
    if program.is_empty() {
        return Err(GeometryDecodeRejection::EmptyProgram);
    }
    let input = captured_position_input(packet, payload, &program)
        .ok_or(GeometryDecodeRejection::UnsupportedVertexInput)?;
    if input.stride == 0 || input.vertices.is_empty() || input.vertices.len() % input.stride != 0 {
        return Err(GeometryDecodeRejection::InvalidVertexBuffer);
    }
    let vertex_count = input.vertices.len() / input.stride;
    if vertex_count == 0 || vertex_count > MAX_INPUT_VERTICES {
        return Err(GeometryDecodeRejection::TooManyVertices);
    }

    let indices = if packet.indexed == 1 {
        packet.payload[payload.indices.clone()]
            .chunks_exact(2)
            .map(|bytes| usize::from(u16::from_le_bytes([bytes[0], bytes[1]])))
            .collect::<Vec<_>>()
    } else {
        let end = packet
            .first
            .checked_add(packet.count)
            .ok_or(GeometryDecodeRejection::InvalidIndexData)?;
        (packet.first..end).map(|index| index as usize).collect()
    };
    if indices.len() != packet.count as usize {
        return Err(GeometryDecodeRejection::InvalidIndexData);
    }
    let triangles = triangles(packet.primitive, &indices)
        .ok_or(GeometryDecodeRejection::UnsupportedPrimitive)?;
    let output_bytes = triangles
        .len()
        .checked_mul(36)
        .ok_or(GeometryDecodeRejection::OutputBudget)?;
    if output_bytes > output_budget {
        return Err(GeometryDecodeRejection::OutputBudget);
    }

    let constants = constants(&packet.payload[payload.constants.clone()])
        .ok_or(GeometryDecodeRejection::InvalidConstants)?;
    let mut transformed: Vec<Option<Result<([f32; 3], [f32; 4]), GeometryDecodeRejection>>> =
        vec![None; vertex_count];
    let mut last_transform_rejection = None;
    let mut transform_source =
        |source_index: usize| -> Result<([f32; 3], [f32; 4]), GeometryDecodeRejection> {
            let local = source_index
                .checked_sub(input.first as usize)
                .ok_or(GeometryDecodeRejection::VertexIndexOutsideBuffer)?;
            let cached = transformed
                .get_mut(local)
                .ok_or(GeometryDecodeRejection::VertexIndexOutsideBuffer)?;
            if cached.is_none() {
                let result = (|| {
                    let offset = local
                        .checked_mul(input.stride)
                        .and_then(|base| base.checked_add(input.offset))
                        .ok_or(GeometryDecodeRejection::InvalidPositionData)?;
                    let position = float3(input.vertices, offset)
                        .ok_or(GeometryDecodeRejection::InvalidPositionData)?;
                    let mut inputs = [[0.0, 0.0, 0.0, 1.0]; vsh::INPUT_COUNT];
                    for attribute in &input.attributes {
                        let attribute_offset = local
                            .checked_mul(input.attribute_stride)
                            .and_then(|base| base.checked_add(attribute.offset as usize))
                            .ok_or(GeometryDecodeRejection::InvalidPositionData)?;
                        inputs[attribute.register as usize] = captured_attribute(
                            attribute.kind,
                            input.attribute_vertices,
                            attribute_offset,
                        )
                        .ok_or(GeometryDecodeRejection::InvalidPositionData)?;
                    }
                    inputs[input.register] = [position[0], position[1], position[2], 1.0];
                    let output = vsh::run_decoded(&program, shader_start, &inputs, &constants);
                    let position = output.position();
                    if !shader_position_has_dividable_w(position[3]) {
                        return Err(GeometryDecodeRejection::InvalidShaderPosition);
                    }
                    let clip =
                        vsh::to_clip(position, (extent[0] as f32, extent[1] as f32), &constants)
                            .ok_or(GeometryDecodeRejection::InvalidClipPosition)?;
                    let w = clip[3];
                    if w.abs() <= 1.0e-6 {
                        return Err(GeometryDecodeRejection::InvalidClipPosition);
                    }
                    let point = [clip[0] / w, clip[1] / w, clip[2] / w];
                    if !point.iter().all(|value| value.is_finite()) {
                        return Err(GeometryDecodeRejection::NonFiniteClipPosition);
                    }
                    let diffuse = output.diffuse();
                    if !diffuse.iter().all(|value| value.is_finite()) {
                        return Err(GeometryDecodeRejection::InvalidDiffuse);
                    }
                    let color = diffuse.map(|value| value.clamp(0.0, 1.0));
                    Ok((point, color))
                })();
                *cached = Some(result);
            }
            let result = cached
                .as_ref()
                .copied()
                .unwrap_or(Err(GeometryDecodeRejection::InvalidVertexTransform));
            if let Err(rejection) = result {
                last_transform_rejection = Some(rejection);
            }
            result
        };

    let mut positions = Vec::with_capacity(triangles.len() * 3);
    let mut colors = Vec::with_capacity(triangles.len() * 3);
    let mut output_indices = Vec::with_capacity(triangles.len() * 3);
    for triangle in triangles {
        let Ok(vertices) = triangle
            .into_iter()
            .map(&mut transform_source)
            .collect::<Result<Vec<_>, _>>()
        else {
            continue;
        };
        let first =
            u32::try_from(positions.len()).map_err(|_| GeometryDecodeRejection::OutputBudget)?;
        for (position, color) in vertices {
            positions.push(position);
            colors.push(color);
        }
        output_indices.extend([first, first + 1, first + 2]);
    }
    if output_indices.is_empty() {
        return Err(
            last_transform_rejection.unwrap_or(GeometryDecodeRejection::InvalidVertexTransform)
        );
    }
    Ok(FrameMesh {
        id: mesh_id,
        positions,
        clip_w: None,
        colors: Some(colors),
        uvs: None,
        texture: None,
        indices: output_indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_float2_expands_missing_components_for_shader_inputs() {
        let bytes = [2.5f32, -3.0]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            captured_attribute(FLOAT2_REGISTER_FORMAT, &bytes, 0),
            Some([2.5, -3.0, 0.0, 1.0])
        );
    }

    #[test]
    fn captured_d3d_bytes_are_swizzled_and_normalized_for_shader_inputs() {
        assert_eq!(
            captured_attribute(UB_D3D4_REGISTER_FORMAT, &[10, 20, 30, 40], 0),
            Some([30.0 / 255.0, 20.0 / 255.0, 10.0 / 255.0, 40.0 / 255.0])
        );
    }

    #[test]
    fn negative_shader_w_is_preserved_for_gpu_clipping() {
        assert!(shader_position_has_dividable_w(-2.0));
        assert!(shader_position_has_dividable_w(2.0));
        assert!(!shader_position_has_dividable_w(0.0));
        assert!(!shader_position_has_dividable_w(-1.0e-6));
        assert!(!shader_position_has_dividable_w(f32::NAN));
    }
}
