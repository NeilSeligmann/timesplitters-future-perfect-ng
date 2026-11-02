// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline adapter for bounded live NV2A draw captures (UNVERIFIED).

use crate::FrameMesh;
use serde_json::Value;
use tsfp_scene_model::{
    Camera, CameraId, Compare, ContentId, Cull, Draw, DrawList, DrawPass, FrameSample, IDENTITY,
    InstanceId, Lifetime, Material, MaterialId, MeshRef, ModelInstance, RasterState, Rect,
    Sampling, ShaderRef, Target, Winding,
};
use tsfp_wgpu::vsh;

const MAX_CAPTURE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CAPTURE_ROWS: usize = 1024;
const MAX_LINE_BYTES: usize = 1024 * 1024;
const MAX_VERTICES: usize = 360;
const MAX_EXTENT: u32 = 16_384;
const LIVE_CLIP_MAX: f32 = 16_777_215.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuCaptureCoverage {
    pub input_rows: usize,
    pub decoded_meshes: usize,
    pub rows_missing_frame_metadata: usize,
    pub rows_with_inferred_triangle_topology: usize,
    pub invalid_programs: usize,
    pub invalid_positions: usize,
    pub invalid_shader_positions: usize,
    pub invalid_clip_positions: usize,
    pub invalid_diffuse_colours: usize,
    pub unsupported_topologies: usize,
}

struct RawDraw {
    frame: Option<u64>,
    draw: u64,
    topology: u32,
    extent: [u32; 2],
    program: Vec<[u32; 4]>,
    constants: Vec<u32>,
    vertices: Vec<[u32; 64]>,
    topology_inferred: bool,
}

/// Decode live draw JSONL into a DrawList using the captured active shader, constants,
/// and assembled inputs. The result uses shader-produced vertex positions and diffuse
/// colours; textures, pixel combiners, and non-triangle topology remain unsupported.
pub fn build_draw_list(
    text: &str,
    fallback_extent: [u32; 2],
    tick: u64,
) -> Result<(DrawList, Vec<FrameMesh>, GpuCaptureCoverage), String> {
    if text.len() > MAX_CAPTURE_BYTES {
        return Err("GPU draw capture exceeds the 64 MiB limit".into());
    }
    validate_extent(fallback_extent)?;
    let mut rows = Vec::new();
    let mut coverage = GpuCaptureCoverage::default();
    for (line_index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if line.len() > MAX_LINE_BYTES {
            return Err(format!(
                "GPU draw row {} exceeds the 1 MiB limit",
                line_index + 1
            ));
        }
        if rows.len() == MAX_CAPTURE_ROWS {
            return Err("GPU draw capture exceeds the 1024 row limit".into());
        }
        let value: Value = serde_json::from_str(line)
            .map_err(|error| format!("invalid GPU draw row {}: {error}", line_index + 1))?;
        rows.push(parse_row(&value, rows.len() as u64, fallback_extent)?);
    }
    if rows.is_empty() {
        return Err("GPU draw capture has no rows".into());
    }

    let extent = rows[0].extent;
    let mut capture_frame = None;
    for row in &rows {
        if row.extent != extent {
            return Err("GPU draw capture mixes target extents".into());
        }
        if let Some(frame) = row.frame {
            if capture_frame.is_some_and(|previous| previous != frame) {
                return Err("GPU draw capture mixes renderer frames".into());
            }
            capture_frame = Some(frame);
        } else {
            coverage.rows_missing_frame_metadata += 1;
        }
        if row.topology_inferred {
            coverage.rows_with_inferred_triangle_topology += 1;
        }
    }
    coverage.input_rows = rows.len();

    let camera_id = CameraId(Lifetime {
        slot: 1,
        generation: 1,
    });
    let viewport = Rect {
        x: 0,
        y: 0,
        width: extent[0],
        height: extent[1],
    };
    let shader = ShaderRef(content_id(0x5348_4144_4552, 0));
    let mut models = Vec::new();
    let mut materials = Vec::new();
    let mut draws = Vec::new();
    let mut meshes = Vec::new();
    let mut geometry_bytes = 0usize;

    for (row_index, row) in rows.iter().enumerate() {
        if row.topology != 0 || row.vertices.is_empty() || row.vertices.len() % 3 != 0 {
            coverage.unsupported_topologies += 1;
            continue;
        }
        let mut memory = vsh::ProgramMemory::default();
        for (slot, words) in row.program.iter().enumerate() {
            memory.slots[slot] = *words;
        }
        let program = vsh::decode_from(&memory, 0);
        if program.is_empty() || program.len() != row.program.len() {
            coverage.invalid_programs += 1;
            continue;
        }
        let constants: [vsh::Vec4; vsh::CONSTANT_COUNT] = std::array::from_fn(|register| {
            std::array::from_fn(|component| f32::from_bits(row.constants[register * 4 + component]))
        });
        let mut positions = Vec::with_capacity(row.vertices.len());
        let mut clip_w = Vec::with_capacity(row.vertices.len());
        let mut colours = Vec::with_capacity(row.vertices.len());
        let mut uvs = Vec::with_capacity(row.vertices.len());
        let mut valid = true;
        for vertex in &row.vertices {
            let inputs: [vsh::Vec4; vsh::INPUT_COUNT] = std::array::from_fn(|register| {
                std::array::from_fn(|component| f32::from_bits(vertex[register * 4 + component]))
            });
            let output = vsh::run_decoded(&program, 0, &inputs, &constants);
            let position = output.position();
            if !position.iter().all(|component| component.is_finite()) {
                coverage.invalid_shader_positions += 1;
                valid = false;
                break;
            }
            let Some(clip) = vsh::to_clip_host_raster(
                position,
                (extent[0] as f32, extent[1] as f32),
                LIVE_CLIP_MAX,
            ) else {
                coverage.invalid_clip_positions += 1;
                valid = false;
                break;
            };
            if !clip.iter().all(|component| component.is_finite()) {
                coverage.invalid_clip_positions += 1;
                valid = false;
                break;
            }
            positions.push([clip[0], clip[1], clip[2]]);
            clip_w.push(clip[3]);
            let mut colour = output.diffuse();
            for component in &mut colour {
                if !component.is_finite() {
                    coverage.invalid_diffuse_colours += 1;
                    valid = false;
                    break;
                }
                *component = component.clamp(0.0, 1.0);
            }
            if !valid {
                break;
            }
            colours.push(colour);
            let texcoord = output.texcoord(0);
            uvs.push([texcoord[0], texcoord[1]]);
        }
        if !valid {
            coverage.invalid_positions += 1;
            continue;
        }
        let byte_count = positions
            .len()
            .checked_mul(32)
            .ok_or("GPU draw geometry size overflow")?;
        geometry_bytes = geometry_bytes
            .checked_add(byte_count)
            .ok_or("GPU draw geometry size overflow")?;
        if geometry_bytes > 16 * 1024 * 1024 {
            return Err("GPU draw geometry exceeds the 16 MiB limit".into());
        }

        let serial = row_index as u64 + 1;
        let mesh_ref = MeshRef(content_id(serial, row.frame.unwrap_or(tick) ^ row.draw));
        let material_id = MaterialId(Lifetime {
            slot: serial,
            generation: 1,
        });
        let instance_id = InstanceId(Lifetime {
            slot: serial,
            generation: 1,
        });
        let indices: Vec<u32> = (0..positions.len())
            .map(|index| u32::try_from(index).expect("at most 360 vertices"))
            .collect();
        meshes.push(FrameMesh {
            id: mesh_ref,
            positions,
            clip_w: Some(clip_w),
            colors: Some(colours),
            uvs: Some(uvs),
            texture: None,
            indices,
        });
        materials.push(Material {
            id: material_id,
            shader,
            textures: Vec::new(),
            parameters: vec![[1.0; 4]],
            raster: RasterState {
                cull: Cull::None,
                front_face: Winding::CounterClockwise,
                blend: None,
                depth_compare: Some(Compare::LessEqual),
                depth_write: true,
                depth_bias_constant: 0,
                depth_bias_slope: 0.0,
                stencil: None,
                colour_write: [true; 4],
            },
        });
        models.push(ModelInstance {
            id: instance_id,
            mesh: mesh_ref,
            material: material_id,
            transform: IDENTITY,
            skin_palette: Vec::new(),
            discontinuity: true,
        });
        draws.push(Draw::Mesh {
            instance: instance_id,
            lights: Vec::new(),
            scissor: viewport,
        });
        coverage.decoded_meshes += 1;
    }

    let frame = DrawList {
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
            position: [0.0; 3],
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
    frame
        .validate()
        .map_err(|error| format!("invalid decoded DrawList: {error:?}"))?;
    Ok((frame, meshes, coverage))
}

fn parse_row(
    value: &Value,
    fallback_draw: u64,
    fallback_extent: [u32; 2],
) -> Result<RawDraw, String> {
    let object = value.as_object().ok_or("GPU draw row is not an object")?;
    let parse_u64 = |key: &str| -> Result<Option<u64>, String> {
        object
            .get(key)
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| format!("GPU draw {key} is not an unsigned integer"))
            })
            .transpose()
    };
    let parse_array = |key: &str| -> Result<&Vec<Value>, String> {
        object
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("GPU draw {key} is not an array"))
    };
    let width = parse_u64("width")?.map_or(Ok(fallback_extent[0]), |value| {
        u32::try_from(value).map_err(|_| "GPU draw width exceeds u32")
    })?;
    let height = parse_u64("height")?.map_or(Ok(fallback_extent[1]), |value| {
        u32::try_from(value).map_err(|_| "GPU draw height exceeds u32")
    })?;
    let extent = [width, height];
    validate_extent(extent)?;

    let program_values = parse_array("program")?;
    if program_values.is_empty() || program_values.len() > vsh::PROGRAM_SLOTS {
        return Err("GPU draw program must contain 1..=136 slots".into());
    }
    let mut program = Vec::with_capacity(program_values.len());
    for words in program_values {
        let words = words
            .as_array()
            .ok_or("GPU draw program slot is not an array")?;
        if words.len() != 4 {
            return Err("GPU draw program slot must contain 4 words".into());
        }
        program.push([
            word(&words[0])?,
            word(&words[1])?,
            word(&words[2])?,
            word(&words[3])?,
        ]);
    }
    let constants_values = parse_array("constants")?;
    if constants_values.len() != vsh::CONSTANT_COUNT * 4 {
        return Err("GPU draw constants must contain 768 words".into());
    }
    let constants = constants_values
        .iter()
        .map(word)
        .collect::<Result<Vec<_>, _>>()?;

    let vertices_values = parse_array("vertices")?;
    if vertices_values.is_empty() || vertices_values.len() > MAX_VERTICES {
        return Err("GPU draw vertices must contain 1..=360 rows".into());
    }
    let mut vertices = Vec::with_capacity(vertices_values.len());
    for values in vertices_values {
        let values = values.as_array().ok_or("GPU draw vertex is not an array")?;
        if values.len() != vsh::INPUT_COUNT * 4 {
            return Err("GPU draw vertex must contain 64 words".into());
        }
        let words = values.iter().map(word).collect::<Result<Vec<_>, _>>()?;
        vertices.push(
            words
                .try_into()
                .map_err(|_| "GPU draw vertex size changed")?,
        );
    }
    let topology = parse_u64("topology")?.unwrap_or(0);
    let topology = u32::try_from(topology).map_err(|_| "GPU draw topology exceeds u32")?;
    Ok(RawDraw {
        frame: parse_u64("frame")?,
        draw: parse_u64("draw")?.unwrap_or(fallback_draw),
        topology,
        extent,
        program,
        constants,
        vertices,
        topology_inferred: !object.contains_key("topology"),
    })
}

fn word(value: &Value) -> Result<u32, String> {
    value
        .as_u64()
        .and_then(|word| u32::try_from(word).ok())
        .ok_or_else(|| "GPU draw word is not a u32".into())
}

fn validate_extent(extent: [u32; 2]) -> Result<(), String> {
    if extent.contains(&0) || extent.into_iter().any(|value| value > MAX_EXTENT) {
        return Err("GPU draw extent must be within 1..=16384".into());
    }
    Ok(())
}

fn content_id(a: u64, b: u64) -> ContentId {
    let mut bytes = [0u8; 32];
    bytes[0..8].copy_from_slice(&a.to_le_bytes());
    bytes[8..16].copy_from_slice(&b.to_le_bytes());
    bytes[31] = 0xD7;
    ContentId(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_capture(topology: u32) -> String {
        let mut constants = vec![0u32; 768];
        for (slot, value) in [
            (58, [320.0f32, 240.0, 1.0, 1.0]),
            (59, [320.0, 240.0, 0.0, 0.0]),
        ] {
            for (component, value) in value.into_iter().enumerate() {
                constants[slot * 4 + component] = value.to_bits();
            }
        }
        let mut program = [0u32; 4];
        program[1] = (1 << 21) | (1 << 9) | 0x1B;
        program[2] = 2 << 26;
        program[3] = 1 | (1 << 11) | (0xF << 12);
        let vertices = [
            [160.0f32, 360.0, 0.0],
            [480.0, 360.0, 0.0],
            [320.0, 120.0, 0.0],
        ]
        .map(|position| {
            let mut words = vec![0u32; 64];
            words[4..8].copy_from_slice(&[
                position[0].to_bits(),
                position[1].to_bits(),
                position[2].to_bits(),
                1.0f32.to_bits(),
            ]);
            words
        });
        let value = json!({
            "frame": 12,
            "draw": 3,
            "topology": topology,
            "width": 640,
            "height": 480,
            "program": [program],
            "constants": constants,
            "vertices": vertices,
        });
        format!("{value}\n")
    }

    #[test]
    fn active_program_and_constants_decode_to_coloured_triangles() {
        let (frame, meshes, coverage) =
            build_draw_list(&sample_capture(0), [640, 480], 12).unwrap();
        assert_eq!(coverage.input_rows, 1);
        assert_eq!(coverage.decoded_meshes, 1);
        assert_eq!(frame.models.len(), 1);
        assert_eq!(meshes[0].positions.len(), 3);
        assert_eq!(meshes[0].colors.as_ref().unwrap().len(), 3);
        assert_eq!(meshes[0].indices, [0, 1, 2]);
    }

    #[test]
    fn negative_clip_w_is_left_for_gpu_clipping() {
        let mut capture: Value = serde_json::from_str(sample_capture(0).trim()).unwrap();
        capture["vertices"][0][7] = json!((-1.0f32).to_bits());
        let (_, meshes, coverage) =
            build_draw_list(&format!("{capture}\n"), [640, 480], 12).unwrap();

        assert_eq!(coverage.decoded_meshes, 1);
        assert_eq!(coverage.invalid_positions, 0);
        assert_eq!(meshes[0].positions[0][0], 0.5);
        assert_eq!(meshes[0].clip_w.as_ref().unwrap()[0], -1.0);
    }

    #[test]
    fn non_finite_shader_position_is_counted_separately() {
        let mut capture: Value = serde_json::from_str(sample_capture(0).trim()).unwrap();
        capture["vertices"][0][7] = json!(f32::NAN.to_bits());
        let (_, meshes, coverage) =
            build_draw_list(&format!("{capture}\n"), [640, 480], 12).unwrap();

        assert!(meshes.is_empty());
        assert_eq!(coverage.invalid_positions, 1);
        assert_eq!(coverage.invalid_shader_positions, 1);
        assert_eq!(coverage.invalid_clip_positions, 0);
        assert_eq!(coverage.invalid_diffuse_colours, 0);
    }

    #[test]
    fn point_and_line_topologies_are_counted_and_omitted() {
        let (_, meshes, coverage) = build_draw_list(&sample_capture(2), [640, 480], 12).unwrap();
        assert!(meshes.is_empty());
        assert_eq!(coverage.unsupported_topologies, 1);
    }

    #[test]
    fn oversized_capture_and_mixed_frames_are_rejected() {
        assert!(build_draw_list(&" ".repeat(MAX_CAPTURE_BYTES + 1), [640, 480], 0).is_err());
        let first = sample_capture(0).trim().to_owned();
        let mut second: Value = serde_json::from_str(&first).unwrap();
        second["frame"] = json!(13);
        let text = format!("{first}\n{second}\n");
        assert!(build_draw_list(&text, [640, 480], 0).is_err());
    }
}
