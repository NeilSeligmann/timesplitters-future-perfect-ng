// SPDX-License-Identifier: GPL-3.0-or-later
//! T2347: the native colour grade pass (rule 23, prerequisite of T2336).
//!
//! The engine ends every frame with a closing full screen pass (T2140, docs/t-wgpu-renderer.md Slice 9): pixel shader
//! `0x004BD6F8`, five combiner stages that read the scene target and write the displayed image. This module recognises that
//! combiner, turns its constants into a typed [`GradeModel`] and draws the same grade with a native wgpu pass ([`NativeGrade`]),
//! so the grade no longer depends on the replayed D3D8 draw and can be run per display update by the shell.
//!
//! The grade, per pixel `t` (the scene colour), with `w` the luma weights, `s` the saturation boost, `g` the gain boost and
//! `b` the bias (every value clamped to -1..1 after each stage, the NV2A register range):
//!
//! ```text
//! luma   = dot(t, w)
//! chroma = t - luma
//! sat    = chroma * s + t          (saturation x(1 + s) around the luma, approximately)
//! graded = (sat * g - b) + sat     (gain x(1 + g), bias -b)
//! out    = clamp(graded, 0, 1), alpha 0
//! ```
//!
//! Retail values: `w` = (0.30, 0.59, 0.11) (`0x004D961C`), `s` = 0x1F/255 (alpha of the stage 2 constant), `g` = 0x17/255, `b` = 10/255. The `original`
//! preset is this model fed by the constants of the frame; the replayed grade stays the reference (`--opt replay_grade`).

use crate::combiner::PixelProgram;

/// RGB input words of the five stages of the retail grade (`0x004BD6F8`, measured in the T2317 streams).
const RGB_INPUTS: [u32; 5] = [
    0xC8C1_0000,
    0xC820_ED20,
    0xCCD1_20C8,
    0xCDC1_20E2,
    0xCC20_CD20,
];
/// RGB output words: stage 0 dot product into r1 (blue to alpha), stages 1 and 3 sum into r0, stage 2 sum into r1, stage 4 r0.
const RGB_OUTPUTS: [u32; 5] = [0x0008_20D0, 0xC00, 0xD00, 0xC00, 0xC00];
/// Final combiner: rgb = r0, alpha = r0 alpha.
const FINAL_ABCD: u32 = 0xC00;
const FINAL_EFG: u32 = 0x1C80;

/// The typed grade of one frame. Each field is the RGB of one combiner constant (0..1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradeModel {
    /// Stage 0 constant 0: the luma weights.
    pub luma: [f32; 3],
    /// Stage 2 constant 0, alpha: chroma added on top of the colour (saturation is `1 + saturation_boost`).
    pub saturation_boost: f32,
    /// Stage 3 constant 0: the graded colour added on top of itself (gain is `1 + gain_boost`).
    pub gain_boost: [f32; 3],
    /// Stage 3 constant 1: subtracted after the gain.
    pub bias: [f32; 3],
}

impl GradeModel {
    /// The grade of `program` (the effective program of the draw, pushed constants applied), `None` when it is not the
    /// engine's grade combiner (the replayed draw is kept then).
    pub fn from_program(program: &PixelProgram) -> Option<Self> {
        if program.count != 5 || program.final_abcd != FINAL_ABCD || program.final_efg != FINAL_EFG
        {
            return None;
        }
        let stages = &program.stages[..5];
        let shape = stages.iter().enumerate().all(|(index, stage)| {
            stage.rgb_inputs == RGB_INPUTS[index]
                && stage.rgb_outputs == RGB_OUTPUTS[index]
                && stage.alpha_inputs == 0
                && stage.alpha_outputs == 0
        });
        if !shape {
            return None;
        }
        let rgb = |value: [f32; 4]| [value[0], value[1], value[2]];
        Some(Self {
            luma: rgb(stages[0].constant0),
            saturation_boost: stages[2].constant0[3],
            gain_boost: rgb(stages[3].constant0),
            bias: rgb(stages[3].constant1),
        })
    }

    /// The grade of one scene colour (RGBA 0..1), the CPU form of the pass. Same operations, in the same order, as the
    /// combiner evaluation of the replay.
    pub fn apply(&self, texel: [f32; 4]) -> [f32; 4] {
        let signed = |x: f32| x.clamp(-1.0, 1.0);
        let t = [texel[0], texel[1], texel[2]].map(signed);
        let w = self.luma.map(signed);
        let luma = signed(t[0] * w[0] + t[1] * w[1] + t[2] * w[2]);
        let mut out = [0.0f32; 4];
        for channel in 0..3 {
            let chroma = signed(t[channel] * 1.0 + -luma * 1.0);
            let sat = signed(chroma * signed(self.saturation_boost) + 1.0 * t[channel]);
            let gained =
                signed(sat * signed(self.gain_boost[channel]) + 1.0 * -signed(self.bias[channel]));
            let graded = signed(gained * 1.0 + sat * 1.0);
            out[channel] = (0.0 * 0.0 + (1.0 - 0.0) * graded.clamp(0.0, 1.0) + 0.0).clamp(0.0, 1.0);
        }
        out
    }

    fn uniform_bytes(&self) -> Vec<u8> {
        [
            self.luma,
            [self.saturation_boost; 3],
            self.gain_boost,
            self.bias,
        ]
        .iter()
        .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 0.0])
        .flat_map(f32::to_le_bytes)
        .collect()
    }
}

const SHADER: &str = r#"
struct Grade { luma: vec4<f32>, saturation: vec4<f32>, gain: vec4<f32>, bias: vec4<f32> };
@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var<uniform> grade: Grade;

@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 4.0 - 1.0, 0.0, 1.0);
}

fn range1(x: vec3<f32>) -> vec3<f32> { return clamp(x, vec3<f32>(-1.0), vec3<f32>(1.0)); }

// Same operation shapes as run_combiner in shader.wgsl (products, then their sum, clamped per stage).
@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(scene));
    let at = clamp(vec2<i32>(pos.xy), vec2<i32>(0), size - vec2<i32>(1));
    let one = vec3<f32>(1.0);
    let t = range1(textureLoad(scene, at, 0).rgb);
    let luma = range1(vec3<f32>(dot(t, range1(grade.luma.rgb))));
    let chroma = range1(t * one + (-luma) * one);
    let sat = range1(chroma * range1(grade.saturation.rgb) + one * t);
    let gained = range1(sat * range1(grade.gain.rgb) + one * -range1(grade.bias.rgb));
    let graded = range1(gained * one + sat * one);
    return vec4<f32>(clamp(clamp(graded, vec3<f32>(0.0), one), vec3<f32>(0.0), one), 0.0);
}
"#;

/// The native grade pass: reads a scene texture, writes the graded image into a target of the same size (load op Load, the
/// scissor limits the written region). Target format `Rgba8Unorm` unless built for another format.
pub struct NativeGrade {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
}

impl NativeGrade {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tsfp-native-grade"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tsfp-native-grade"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tsfp-native-grade"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tsfp-native-grade"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self { pipeline, layout }
    }

    /// Record the grade of `scene` into `target`. `scissor` is `(x, y, w, h)` in target pixels, `None` for the whole target.
    /// `scene` and `target` must be different textures (a display copy of the clean scene, for example).
    pub fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        scene: &wgpu::TextureView,
        target: &wgpu::TextureView,
        model: &GradeModel,
        scissor: Option<(u32, u32, u32, u32)>,
    ) {
        let uniform = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("tsfp-native-grade"),
                contents: &model.uniform_bytes(),
                usage: wgpu::BufferUsages::UNIFORM,
            },
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tsfp-native-grade"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tsfp-native-grade"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &group, &[]);
        if let Some((x, y, w, h)) = scissor {
            pass.set_scissor_rect(x, y, w, h);
        }
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combiner::{PixelProgram, argb};

    /// The retail grade definition (structure words of `0x004BD6F8`) with the retail pushed constants, FABRICATED as words
    /// (no retail bytes copied): the same layout `PixelProgram::from_words` decodes.
    fn retail_program() -> PixelProgram {
        let mut words = vec![0u32; 0x40];
        words[0x88 / 4..0x88 / 4 + 5].copy_from_slice(&RGB_INPUTS);
        words[0xB4 / 4..0xB4 / 4 + 5].copy_from_slice(&RGB_OUTPUTS);
        words[0x20 / 4] = FINAL_ABCD;
        words[0x24 / 4] = FINAL_EFG;
        words[0x28 / 4] = 0x004D_961C;
        words[0xD4 / 4] = 0x0001_1105;
        words[0xD8 / 4] = 1;
        let mut program = PixelProgram::from_words(&words).expect("definition");
        program.stages[2].constant0 = argb(0x1F1F_1F1F);
        program.stages[3].constant0 = argb(0x1717_1717);
        program.stages[3].constant1 = argb(0x0A0A_0A0A);
        program
    }

    #[test]
    fn the_retail_grade_is_recognised_with_its_typed_constants() {
        let model = GradeModel::from_program(&retail_program()).expect("grade");
        assert_eq!(
            model.luma,
            [
                0x4D as f32 / 255.0,
                0x96 as f32 / 255.0,
                0x1C as f32 / 255.0
            ]
        );
        assert_eq!(model.saturation_boost, 0x1F as f32 / 255.0);
        assert_eq!(model.gain_boost, [0x17 as f32 / 255.0; 3]);
        assert_eq!(model.bias, [10.0 / 255.0; 3]);
    }

    #[test]
    fn another_combiner_is_not_a_grade() {
        let mut program = retail_program();
        program.stages[4].rgb_outputs = 0xD00;
        assert_eq!(GradeModel::from_program(&program), None);
        let mut program = retail_program();
        program.count = 4;
        assert_eq!(GradeModel::from_program(&program), None);
    }

    /// Tier 1: the typed model against the generic combiner evaluation on every 8 bit grey and 4096 seeded colours.
    #[test]
    fn the_model_equals_the_combiner_evaluation() {
        let program = retail_program();
        let model = GradeModel::from_program(&program).expect("grade");
        let mut seed = 0x2347u32;
        let mut colours: Vec<[f32; 4]> = (0..=255)
            .map(|v| [v as f32 / 255.0, v as f32 / 255.0, v as f32 / 255.0, 1.0])
            .collect();
        for _ in 0..4096 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            colours.push(argb(seed | 0xFF00_0000));
        }
        assert!(!colours.is_empty());
        for texel in colours {
            let reference =
                program.evaluate_stages([0.0; 4], [texel, [0.0; 4], [0.0; 4], [0.0; 4]]);
            let native = model.apply(texel);
            let byte = |x: f32| (x * 255.0).round() as u8;
            assert_eq!(native.map(byte), reference.map(byte), "texel {texel:?}");
        }
    }
}
