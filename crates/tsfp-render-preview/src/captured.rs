// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, explicitly fabricated DrawList adapter for captured room packets.
use std::{collections::BTreeMap, ops::Range};

use tsfp_engine_render::native_scene::RoomDrawPacket;
use tsfp_render_prep::live::NativeSceneView;
use tsfp_scene_model::{
    Camera, CameraId, ContentId, Cull, Draw, DrawList, DrawPass, FrameSample, IDENTITY, InstanceId,
    Lifetime, Material, MaterialId, Matrix4, MeshRef, ModelInstance, RasterState, Rect, Sampling,
    ShaderRef, Target, Winding,
};

use crate::{FrameMesh, geometry};

const MAX_DRAWS: usize = 1024;
const MAX_EXTENT: u32 = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapturedDrawError {
    MissingCamera,
    InvalidExtent,
    TooManyDraws,
    DroppedPayload,
    InvalidPayloadBounds,
    InvalidIndices,
    InvalidSceneContract,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CheckedPayload {
    pub(crate) vertices: Range<usize>,
    pub(crate) indices: Range<usize>,
    pub(crate) textures: [Range<usize>; 4],
    pub(crate) program: Range<usize>,
    pub(crate) constants: Range<usize>,
    pub(crate) declaration: Option<Vec<crate::capture::VertexAttributeDeclaration>>,
    pub(crate) register1: Option<crate::capture::CapturedVertexInput>,
}

fn checked_payload_ranges(packet: &RoomDrawPacket) -> Result<CheckedPayload, CapturedDrawError> {
    if packet.payload_dropped {
        return Err(CapturedDrawError::DroppedPayload);
    }
    let mut offset = 0usize;
    let mut range = |length: u32| -> Result<Range<usize>, CapturedDrawError> {
        let start = offset;
        offset = offset
            .checked_add(length as usize)
            .ok_or(CapturedDrawError::InvalidPayloadBounds)?;
        Ok(start..offset)
    };
    let vertices = range(packet.vertex_bytes)?;
    let indices = range(packet.index_bytes)?;
    let textures = packet
        .texture_bytes
        .map(|length| range(length))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let textures: [Range<usize>; 4] = textures
        .try_into()
        .map_err(|_| CapturedDrawError::InvalidPayloadBounds)?;
    let program = range(packet.vertex_program_bytes)?;
    let constants = range(packet.vertex_constants_bytes)?;
    let _pixel_shader = range(packet.pixel_shader_bytes)?;
    if offset > packet.payload.len() {
        return Err(CapturedDrawError::InvalidPayloadBounds);
    }
    let trailing = &packet.payload[offset..];
    let (declaration_bytes, register1_bytes) = if trailing.starts_with(b"R1VS") {
        (&[][..], trailing)
    } else if packet.vertex_declaration != 0
        && trailing.len() >= crate::capture::VERTEX_DECLARATION_BYTES
    {
        trailing.split_at(crate::capture::VERTEX_DECLARATION_BYTES)
    } else if trailing.is_empty() {
        (&[][..], &[][..])
    } else {
        return Err(CapturedDrawError::InvalidPayloadBounds);
    };
    let declaration = if declaration_bytes.is_empty() {
        None
    } else {
        Some(
            crate::capture::decode_vertex_declaration_for_render(declaration_bytes)
                .ok_or(CapturedDrawError::InvalidPayloadBounds)?,
        )
    };
    if !register1_bytes.is_empty() && !register1_bytes.starts_with(b"R1VS") {
        return Err(CapturedDrawError::InvalidPayloadBounds);
    }
    let register1 = crate::capture::decode_register1_trailer(register1_bytes)
        .map_err(|()| CapturedDrawError::InvalidPayloadBounds)?;
    let stride = packet.vertex_stride as usize;
    let expected_index_bytes = packet
        .count
        .checked_mul(2)
        .ok_or(CapturedDrawError::InvalidPayloadBounds)?;
    if stride == 0
        || vertices.is_empty()
        || vertices.len() % stride != 0
        || packet.indexed > 1
        || packet.count == 0
        || (packet.indexed == 1 && packet.index_bytes != expected_index_bytes)
        || (packet.indexed == 0 && packet.index_bytes != 0)
    {
        return Err(CapturedDrawError::InvalidPayloadBounds);
    }
    let vertex_count = (vertices.len() / stride) as u32;
    let vertex_end = packet
        .vertex_first
        .checked_add(vertex_count)
        .ok_or(CapturedDrawError::InvalidPayloadBounds)?;
    let vertex_end_bytes = u64::from(vertex_end)
        .checked_mul(u64::from(packet.vertex_stride))
        .ok_or(CapturedDrawError::InvalidPayloadBounds)?;
    if vertex_end_bytes > u64::from(packet.vertex_buffer_bytes) {
        return Err(CapturedDrawError::InvalidPayloadBounds);
    }
    if packet.indexed == 1 {
        if packet
            .first
            .checked_add(packet.count)
            .is_none_or(|end| end > packet.index_buffer_bytes / 2)
        {
            return Err(CapturedDrawError::InvalidIndices);
        }
        for bytes in packet.payload[indices.clone()].chunks_exact(2) {
            let index = u32::from(u16::from_le_bytes([bytes[0], bytes[1]]));
            if !(packet.vertex_first..vertex_end).contains(&index) {
                return Err(CapturedDrawError::InvalidIndices);
            }
        }
    } else if packet.first < packet.vertex_first
        || packet
            .first
            .checked_add(packet.count)
            .is_none_or(|end| end > vertex_end)
    {
        return Err(CapturedDrawError::InvalidIndices);
    }
    Ok(CheckedPayload {
        vertices,
        indices,
        textures,
        program,
        constants,
        declaration,
        register1,
    })
}

fn placeholder_content_id(seed: u64) -> ContentId {
    let mut bytes = [0u8; 32];
    for (index, chunk) in bytes.chunks_exact_mut(8).enumerate() {
        let value = seed
            .wrapping_add((index as u64).wrapping_mul(0x9e3779b97f4a7c15))
            .to_le_bytes();
        chunk.copy_from_slice(&value);
    }
    ContentId(bytes)
}

/// Convert admitted captured draws to a DrawList and the bounded measured geometry resources.
/// Unsupported packets keep placeholder identities and are omitted by the live preview renderer.
pub fn build_draw_list(
    packets: &[RoomDrawPacket],
    view: &NativeSceneView,
    tick: u64,
) -> Result<DrawList, CapturedDrawError> {
    build_draw_list_with_meshes(packets, view, tick).map(|(frame, _)| frame)
}

const MAX_FRAME_GEOMETRY_BYTES: usize = 16 * 1024 * 1024;

pub fn build_draw_list_with_meshes(
    packets: &[RoomDrawPacket],
    view: &NativeSceneView,
    tick: u64,
) -> Result<(DrawList, Vec<FrameMesh>), CapturedDrawError> {
    build_draw_list_with_meshes_and_coverage(packets, view, tick)
        .map(|(frame, meshes, _)| (frame, meshes))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct GeometryDecodeCoverage {
    pub admitted_meshes: usize,
    pub missing_payloads: usize,
    pub rejections: BTreeMap<geometry::GeometryDecodeRejection, usize>,
}

pub fn build_draw_list_with_meshes_and_coverage(
    packets: &[RoomDrawPacket],
    view: &NativeSceneView,
    tick: u64,
) -> Result<(DrawList, Vec<FrameMesh>, GeometryDecodeCoverage), CapturedDrawError> {
    let [left, top, right, bottom] = view.rectangle;
    if left < 0 || top < 0 || right <= left || bottom <= top {
        return Err(CapturedDrawError::InvalidExtent);
    }
    let extent = [right as u32, bottom as u32];
    if extent.contains(&0) || extent.into_iter().any(|value| value > MAX_EXTENT) {
        return Err(CapturedDrawError::InvalidExtent);
    }
    if packets.is_empty() || packets.len() > MAX_DRAWS {
        return Err(CapturedDrawError::TooManyDraws);
    }
    let camera_id = CameraId(Lifetime {
        slot: 1,
        generation: 1,
    });
    let viewport = Rect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    };
    let mut models = Vec::with_capacity(packets.len());
    let mut materials = Vec::new();
    let mut material_ids = BTreeMap::new();
    let mut draws = Vec::with_capacity(packets.len());
    let mut meshes = Vec::new();
    let mut coverage = GeometryDecodeCoverage::default();
    let mut geometry_budget = MAX_FRAME_GEOMETRY_BYTES;
    for (index, packet) in packets.iter().enumerate() {
        let payload = (!packet.payload_dropped)
            .then(|| checked_payload_ranges(packet))
            .transpose()?;
        let key = (packet.material_key, packet.state_key);
        let material_id = if let Some(&id) = material_ids.get(&key) {
            id
        } else {
            let id = MaterialId(Lifetime {
                slot: material_ids.len() as u64 + 1,
                generation: 1,
            });
            let colour = [
                0.25 + ((packet.material_key >> 0) & 0xff) as f32 / 510.0,
                0.25 + ((packet.material_key >> 8) & 0xff) as f32 / 510.0,
                0.25 + ((packet.material_key >> 16) & 0xff) as f32 / 510.0,
                1.0,
            ];
            materials.push(Material {
                id,
                shader: ShaderRef(placeholder_content_id(
                    packet.material_key ^ packet.state_key,
                )),
                textures: Vec::new(),
                parameters: vec![colour],
                raster: RasterState {
                    cull: Cull::None,
                    front_face: Winding::CounterClockwise,
                    blend: None,
                    depth_compare: Some(tsfp_scene_model::Compare::LessEqual),
                    depth_write: true,
                    depth_bias_constant: 0,
                    depth_bias_slope: 0.0,
                    stencil: None,
                    colour_write: [true; 4],
                },
            });
            material_ids.insert(key, id);
            id
        };
        let id = InstanceId(Lifetime {
            slot: index as u64 + 1,
            generation: 1,
        });
        let x = (index % 8) as f32 * 0.24 - 0.84;
        let y = -(((index / 8) % 8) as f32) * 0.24 + 0.84;
        let placeholder_transform: Matrix4 = [
            [0.09, 0.0, 0.0, 0.0],
            [0.0, 0.09, 0.0, 0.0],
            [0.0, 0.0, 0.09, 0.0],
            [x, y, 0.5, 1.0],
        ];
        let mesh_ref = MeshRef(placeholder_content_id(packet.payload_hash ^ index as u64));
        let transform = match payload.as_ref() {
            None => {
                coverage.missing_payloads += 1;
                placeholder_transform
            }
            Some(payload) => match geometry::decode_packet_mesh(
                packet,
                payload,
                mesh_ref,
                [(right - left) as u32, (bottom - top) as u32],
                geometry_budget,
            ) {
                Ok(mut resource) => {
                    let texture_bytes = &packet.payload[payload.textures[0].clone()];
                    if !texture_bytes.is_empty() {
                        resource.texture = crate::texture::decode_texture_rgba(
                            packet.texture_formats[0],
                            packet.texture_sizes[0],
                            texture_bytes,
                        )
                        .ok();
                    }
                    coverage.admitted_meshes += 1;
                    geometry_budget = geometry_budget.saturating_sub(resource.indices.len() * 12);
                    meshes.push(resource);
                    IDENTITY
                }
                Err(reason) => {
                    *coverage.rejections.entry(reason).or_default() += 1;
                    placeholder_transform
                }
            },
        };
        models.push(ModelInstance {
            id,
            mesh: mesh_ref,
            material: material_id,
            transform,
            skin_palette: Vec::new(),
            discontinuity: true,
        });
        draws.push(Draw::Mesh {
            instance: id,
            lights: Vec::new(),
            scissor: viewport,
        });
    }
    let list = DrawList {
        sample: FrameSample {
            previous_tick: tick,
            current_tick: tick,
            sampling: Sampling::Original,
        },
        targets: Vec::new(),
        cameras: vec![Camera {
            id: camera_id,
            view: IDENTITY,
            projection: IDENTITY,
            position: view.position,
            viewport,
            depth_range: [0.0, 1.0],
            cut: true,
        }],
        models,
        materials,
        lights: Vec::new(),
        effects: Vec::new(),
        passes: vec![DrawPass {
            camera: camera_id,
            target: Target::Surface,
            extent,
            draws,
        }],
    };
    list.validate()
        .map_err(|_| CapturedDrawError::InvalidSceneContract)?;
    Ok((list, meshes, coverage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn packet() -> RoomDrawPacket {
        let mut payload = vec![0u8; 36];
        payload.extend_from_slice(&[0, 0, 1, 0, 2, 0]);
        RoomDrawPacket {
            frame_sequence: 7,
            room: 1,
            indexed: 1,
            primitive: 4,
            first: 0,
            count: 3,
            vertex_buffer: 1,
            vertex_buffer_bytes: 36,
            vertex_stride: 12,
            vertex_first: 0,
            index_buffer: 1,
            index_buffer_bytes: 6,
            texture_headers: [0; 4],
            texture_data: [0; 4],
            texture_formats: [0; 4],
            texture_sizes: [0; 4],
            texture_stage_state: [0; 128],
            viewport_words: [0; 4],
            viewport_target: [640, 480, 0, 0],
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
            vertex_bytes: 36,
            index_bytes: 6,
            texture_bytes: [0; 4],
            payload_dropped: false,
            render_state: [0; 256],
            payload_hash: 17,
            payload: Arc::from(payload),
            material_key: 23,
            state_key: 29,
            scene_light_binding: None,
        }
    }

    fn view() -> NativeSceneView {
        let mut view = NativeSceneView::default();
        view.camera_valid = 1;
        view.rectangle = [0, 0, 640, 480];
        view.position = [0.0, 0.0, 0.0];
        view
    }

    fn register1_geometry_packet() -> RoomDrawPacket {
        let mut packet = packet();
        packet.vertex_stride = 28;
        packet.vertex_first = 4;
        packet.vertex_buffer_bytes = 7 * 28;
        packet.vertex_bytes = 3 * 28;
        packet.vertex_declaration = 1;
        packet.vertex_program_bytes = 16;
        packet.vertex_constants_bytes = 72 * 16;
        packet.index_buffer_bytes = 6;
        let mut payload = vec![0u8; packet.vertex_bytes as usize];
        payload.extend_from_slice(&[4, 0, 5, 0, 6, 0]);
        let mut program = [0u32; 4];
        program[1] = (1 << 21) | (1 << 9) | 0x1B;
        program[2] = 2 << 26;
        program[3] = 1 | (1 << 11) | (0xF << 12);
        for word in program {
            payload.extend_from_slice(&word.to_le_bytes());
        }
        let mut constants = vec![0u8; packet.vertex_constants_bytes as usize];
        constants[58 * 16..58 * 16 + 16].copy_from_slice(
            &[320.0f32, 240.0, 1.0, 1.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        constants[59 * 16..59 * 16 + 16].copy_from_slice(
            &[320.0f32, 240.0, 0.0, 0.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        payload.extend_from_slice(&constants);

        let mut declaration = vec![0u8; crate::capture::VERTEX_DECLARATION_BYTES];
        for register in 0..16 {
            declaration[register * 16 + 8..register * 16 + 12].copy_from_slice(&2u32.to_le_bytes());
        }
        declaration[16 + 4..16 + 8].copy_from_slice(&4u32.to_le_bytes());
        declaration[16 + 8..16 + 12].copy_from_slice(&0x32u32.to_le_bytes());
        payload.extend_from_slice(&declaration);

        payload.extend_from_slice(b"R1VS");
        for value in [1u32, 16, 4, 0x32, 48] {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        for point in [
            [160.0f32, 360.0, 0.0],
            [480.0, 360.0, 0.0],
            [320.0, 120.0, 0.0],
        ] {
            for value in point {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            payload.extend_from_slice(&[0; 4]);
        }
        packet.payload = Arc::from(payload);
        packet
    }

    fn declared_stride24_geometry_packet() -> RoomDrawPacket {
        let mut packet = packet();
        packet.vertex_stride = 24;
        packet.vertex_first = 4;
        packet.vertex_buffer_bytes = 7 * 24;
        packet.vertex_bytes = 3 * 24;
        packet.vertex_declaration = 1;
        packet.vertex_program_bytes = 16;
        packet.vertex_constants_bytes = 72 * 16;
        packet.index_buffer_bytes = 6;

        let mut payload = Vec::new();
        for point in [
            [160.0f32, 360.0, 0.0],
            [480.0, 360.0, 0.0],
            [320.0, 120.0, 0.0],
        ] {
            payload.extend_from_slice(&[0xA5; 4]);
            for value in point {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            payload.extend_from_slice(&[0x5A; 8]);
        }
        payload.extend_from_slice(&[4, 0, 5, 0, 6, 0]);

        let mut program = [0u32; 4];
        program[1] = (1 << 21) | (2 << 9) | 0x1B;
        program[2] = 2 << 26;
        program[3] = 1 | (1 << 11) | (0xF << 12);
        for word in program {
            payload.extend_from_slice(&word.to_le_bytes());
        }

        let mut constants = vec![0u8; packet.vertex_constants_bytes as usize];
        constants[58 * 16..58 * 16 + 16].copy_from_slice(
            &[320.0f32, 240.0, 1.0, 1.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        constants[59 * 16..59 * 16 + 16].copy_from_slice(
            &[320.0f32, 240.0, 0.0, 0.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        payload.extend_from_slice(&constants);

        let mut declaration = vec![0u8; crate::capture::VERTEX_DECLARATION_BYTES];
        for register in 0..16 {
            declaration[register * 16 + 12..register * 16 + 16]
                .copy_from_slice(&2u32.to_le_bytes());
        }
        declaration[2 * 16 + 8..2 * 16 + 12].copy_from_slice(&4u32.to_le_bytes());
        declaration[2 * 16 + 12..2 * 16 + 16].copy_from_slice(&0x32u32.to_le_bytes());
        payload.extend_from_slice(&declaration);

        packet.payload = Arc::from(payload);
        packet
    }

    #[test]
    fn captured_draw_list_checks_payload_indices_and_material_bindings() {
        let original = packet();
        let list = build_draw_list(&[original.clone()], &view(), 7).unwrap();
        assert_eq!(list.validate().unwrap().frame().models.len(), 1);

        let mut truncated = original.clone();
        truncated.vertex_bytes = 35;
        assert_eq!(
            checked_payload_ranges(&truncated),
            Err(CapturedDrawError::InvalidPayloadBounds)
        );

        let mut bad_index = original;
        let mut payload = bad_index.payload.to_vec();
        payload[37..39].copy_from_slice(&3u16.to_le_bytes());
        bad_index.payload = Arc::from(payload);
        assert_eq!(
            checked_payload_ranges(&bad_index),
            Err(CapturedDrawError::InvalidIndices)
        );

        let mut missing_material = list;
        missing_material.models[0].material = MaterialId(Lifetime {
            slot: u64::MAX,
            generation: 1,
        });
        assert!(matches!(
            missing_material.validate(),
            Err(tsfp_scene_model::ContractError::MissingBinding(
                "instance.material"
            ))
        ));
    }

    #[test]
    fn placeholder_draws_do_not_require_an_owner_camera() {
        let mut missing_camera = view();
        missing_camera.camera_valid = 0;
        let list = build_draw_list(&[packet()], &missing_camera, 7).unwrap();
        assert_eq!(list.models.len(), 1);
        assert_eq!(list.cameras[0].view, IDENTITY);
        assert_eq!(list.cameras[0].projection, IDENTITY);
        assert!(list.validate().is_ok());
    }

    #[test]
    fn dropped_payload_keeps_placeholder_without_weakening_present_validation() {
        let mut dropped = packet();
        dropped.payload_dropped = true;
        dropped.payload = Arc::from([]);
        let list = build_draw_list(&[packet(), dropped], &view(), 7).unwrap();
        assert_eq!(list.models.len(), 2);
        assert!(list.validate().is_ok());

        let mut malformed = packet();
        malformed.payload = Arc::from([]);
        assert!(matches!(
            build_draw_list(&[malformed], &view(), 7),
            Err(CapturedDrawError::InvalidPayloadBounds)
        ));
    }

    #[test]
    fn declared_register1_tail_builds_a_bounded_frame_mesh() {
        let packet = register1_geometry_packet();
        let checked = checked_payload_ranges(&packet).unwrap();
        assert!(checked.declaration.is_some());
        assert_eq!(checked.register1.as_ref().unwrap().first, 4);

        let (frame, meshes) = build_draw_list_with_meshes(&[packet], &view(), 12).unwrap();
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].indices.len(), 3);
        assert_eq!(frame.models[0].transform, IDENTITY);
        assert!(frame.validate().is_ok());
    }

    #[test]
    fn declared_stride24_stream_uses_the_shader_read_float3_attribute() {
        let packet = declared_stride24_geometry_packet();
        let checked = checked_payload_ranges(&packet).unwrap();
        assert!(checked.register1.is_none());
        assert_eq!(
            checked.declaration.as_ref().unwrap(),
            &[crate::capture::VertexAttributeDeclaration {
                register: 2,
                offset: 4,
                kind: 0x32,
            }]
        );

        let (frame, meshes) = build_draw_list_with_meshes(&[packet], &view(), 12).unwrap();
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].indices.len(), 3);
        assert_eq!(meshes[0].positions[0], [-0.5, -0.5, 0.0]);
        assert_eq!(meshes[0].positions[1], [0.5, -0.5, 0.0]);
        assert_eq!(meshes[0].positions[2], [0.0, 0.5, 0.0]);
        assert_eq!(frame.models[0].transform, IDENTITY);
        assert!(frame.validate().is_ok());
    }

    #[test]
    fn register1_trailer_supplies_shader_input_for_stride24_stream() {
        let mut packet = declared_stride24_geometry_packet();
        let mut payload = packet.payload.to_vec();
        let program_word = packet.vertex_bytes as usize + packet.index_bytes as usize + 4;
        let old = u32::from_le_bytes(payload[program_word..program_word + 4].try_into().unwrap());
        payload[program_word..program_word + 4]
            .copy_from_slice(&((old & !(0xF << 9)) | (1 << 9)).to_le_bytes());
        payload.extend_from_slice(b"R1VS");
        for value in [1u32, 16, packet.vertex_first, 0x32, 48] {
            payload.extend_from_slice(&value.to_le_bytes());
        }
        for point in [
            [160.0f32, 360.0, 0.0],
            [480.0, 360.0, 0.0],
            [320.0, 120.0, 0.0],
        ] {
            for value in point {
                payload.extend_from_slice(&value.to_le_bytes());
            }
            payload.extend_from_slice(&[0; 4]);
        }
        packet.payload = Arc::from(payload);

        let checked = checked_payload_ranges(&packet).unwrap();
        assert_eq!(
            checked.register1.as_ref().unwrap().first,
            packet.vertex_first
        );
        let (frame, meshes) = build_draw_list_with_meshes(&[packet], &view(), 13).unwrap();
        assert_eq!(meshes.len(), 1);
        assert_eq!(meshes[0].positions[0], [-0.5, -0.5, 0.0]);
        assert_eq!(meshes[0].positions[1], [0.5, -0.5, 0.0]);
        assert_eq!(meshes[0].positions[2], [0.0, 0.5, 0.0]);
        assert_eq!(frame.models[0].transform, IDENTITY);
        assert!(frame.validate().is_ok());
    }
}
