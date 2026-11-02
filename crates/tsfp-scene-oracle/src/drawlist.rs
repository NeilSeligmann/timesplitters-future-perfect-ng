// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-present adapter for the captured original RenderDevice stream.
//!
//! The stream has no typed camera, resource ownership, or draw-to-resource
//! mapping. This adapter preserves its present boundaries and reports those
//! missing semantics as gaps. Material snapshots can be attached separately
//! when the Rust owner has resolved their resource and recipe identities.
use crate::wire::Record;
use crate::{Limits as StreamLimits, Reader, Window};
use std::collections::BTreeMap;
use std::io::Read;
use tsfp_render_materials::{Assets, Error as MaterialError, MaterialState};
use tsfp_scene_compare::{Frame, FrameSource, Gaps};
use tsfp_scene_model::{DrawList, FrameSample, Sampling};

const KNOWN_FRAME_FLAGS: u32 = 1;

/// Streams one incomplete DrawList-shaped observation per present.
pub struct StreamDrawListSource<R> {
    reader: Reader<R>,
    finished: bool,
    window: Window,
    material_snapshots: BTreeMap<u64, Vec<tsfp_scene_model::Material>>,
}

/// Resolve a captured Rust-owned material snapshot into the scene contract.
///
/// The caller supplies the loader's per-frame texture/shader residency and the
/// typed material-state records captured for this frame. The producer rejects
/// missing or nonresident resources and invalid values atomically; this
/// adapter never guesses shader recipes, texture identities, or sampler state.
/// Raw `Record::Texture` payloads alone do not contain enough information to
/// construct these records.
pub fn resolve_materials(
    draws: &mut DrawList,
    assets: &Assets,
    state_records: &[MaterialState],
) -> Result<(), MaterialError> {
    let resolved = assets.produce(state_records)?;
    draws.materials = resolved;
    Ok(())
}

impl<R: Read> StreamDrawListSource<R> {
    pub fn new(input: R, window: Window) -> Result<Self, crate::Error> {
        Ok(Self {
            reader: Reader::new(input, window, StreamLimits::default())?,
            finished: false,
            window,
            material_snapshots: BTreeMap::new(),
        })
    }

    /// Attach one fully resolved material snapshot to its captured present.
    ///
    /// This sidecar is keyed by the receipt's present number, never record
    /// order. Duplicate or out-of-window snapshots fail before iteration.
    pub fn with_material_snapshot(
        mut self,
        present: u64,
        assets: &Assets,
        state_records: &[MaterialState],
    ) -> Result<Self, String> {
        let end = self
            .window
            .first_present
            .checked_add(self.window.frame_count)
            .ok_or_else(|| "material snapshot window overflows".to_owned())?;
        if present < self.window.first_present || present >= end {
            return Err(format!(
                "material snapshot present {present} is outside the receipt window"
            ));
        }
        if self.material_snapshots.contains_key(&present) {
            return Err(format!("duplicate material snapshot for present {present}"));
        }
        let mut draws = empty_draw_list();
        resolve_materials(&mut draws, assets, state_records)
            .map_err(|error| format!("material snapshot resolution failed: {error:?}"))?;
        self.material_snapshots.insert(present, draws.materials);
        Ok(self)
    }
}

fn empty_draw_list() -> DrawList {
    DrawList {
        // The RenderDevice stream has no simulation tick or sampling mode.
        // Non-original sampling plus nonzero gaps prevents accidental admission.
        sample: FrameSample {
            previous_tick: 0,
            current_tick: 0,
            sampling: Sampling::Interpolated { alpha: 0.0 },
        },
        targets: Vec::new(),
        cameras: Vec::new(),
        models: Vec::new(),
        materials: Vec::new(),
        lights: Vec::new(),
        effects: Vec::new(),
        passes: Vec::new(),
    }
}

impl<R: Read> FrameSource for StreamDrawListSource<R> {
    fn next_frame(&mut self, max_frame_bytes: usize) -> Result<Option<Frame>, String> {
        if self.finished {
            return Ok(None);
        }
        if max_frame_bytes == 0 {
            return Err("zero byte limit for RenderDevice frame".into());
        }
        let byte_cap = max_frame_bytes as u64;
        self.reader.tighten_limits(StreamLimits {
            record_bytes: max_frame_bytes,
            frame_bytes: byte_cap,
            records_per_frame: StreamLimits::default().records_per_frame,
        });

        let mut gaps = Gaps {
            // No camera/view record exists in the RenderDevice wire schema.
            unmatched_views: 1,
            ..Gaps::default()
        };
        while let Some(located) = self
            .reader
            .next_record()
            .map_err(|error| error.to_string())?
        {
            match located.record {
                Record::Call { .. }
                | Record::Scissor { .. }
                | Record::Host { .. }
                | Record::Lib { .. } => {
                    gaps.unsupported_states = gaps.unsupported_states.saturating_add(1);
                }
                Record::Memory { .. } | Record::Surface { .. } | Record::Texture { .. } => {
                    // Payload presence is not a stable asset/target identity.
                    gaps.missing_resources = gaps.missing_resources.saturating_add(1);
                }
                Record::Frame { flags } => {
                    if flags & !KNOWN_FRAME_FLAGS != 0 {
                        return Err(format!(
                            "unsupported RenderDevice frame flags 0x{flags:08x}"
                        ));
                    }
                    if flags & crate::wire::FRAME_DRAWS_OMITTED != 0 {
                        gaps.dropped_records = gaps.dropped_records.saturating_add(1);
                    }
                }
                Record::Present { number } => {
                    let mut draws = empty_draw_list();
                    if let Some(materials) = self.material_snapshots.remove(&number) {
                        draws.materials = materials;
                    }
                    return Ok(Some(Frame {
                        present: number,
                        gaps,
                        draws,
                    }));
                }
                Record::Unknown { tag } => {
                    return Err(format!("unknown RenderDevice record tag {tag}"));
                }
            }
        }
        self.finished = true;
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::{StreamDrawListSource, empty_draw_list, resolve_materials};
    use crate::Window;
    use crate::wire;
    use std::io::Cursor;
    use tsfp_render_materials::{AssetKey, Assets, Binding, MaterialState, Residency, Source};
    use tsfp_scene_compare::FrameSource;
    use tsfp_scene_model::{
        AddressMode, Blend, BlendComponent, BlendFactor, BlendOperation, Compare, ContentId, Cull,
        Filter, Lifetime, MaterialId, RasterState, Sampler, ShaderRef, TextureRef, Winding,
    };

    fn state() -> MaterialState {
        MaterialState {
            id: MaterialId(Lifetime {
                slot: 3,
                generation: 2,
            }),
            shader: AssetKey(11),
            textures: vec![Binding {
                slot: 0,
                source: Source::Asset(AssetKey(12)),
                sampler: Sampler {
                    min: Filter::Linear,
                    mag: Filter::Linear,
                    mip: Filter::Nearest,
                    address: [AddressMode::Clamp; 3],
                    border: [0.0; 4],
                    lod_min: 0.0,
                    lod_max: 8.0,
                    lod_bias: 0.0,
                },
                uv_set: 1,
                uv_transform: [[1.0, 0.0, 0.25], [0.0, 1.0, 0.5], [0.0, 0.0, 1.0]],
            }],
            parameters: vec![[0.25, 0.5, 0.75, 1.0]],
            raster: RasterState {
                cull: Cull::Back,
                front_face: Winding::CounterClockwise,
                blend: Some(Blend {
                    colour: BlendComponent {
                        source: BlendFactor::SourceAlpha,
                        destination: BlendFactor::OneMinusSourceAlpha,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent {
                        source: BlendFactor::One,
                        destination: BlendFactor::Zero,
                        operation: BlendOperation::Max,
                    },
                    constant: [0.0; 4],
                }),
                depth_compare: Some(Compare::LessEqual),
                depth_write: true,
                depth_bias_constant: 0,
                depth_bias_slope: 0.0,
                stencil: None,
                colour_write: [true, true, true, false],
            },
        }
    }

    fn assets() -> Assets {
        let mut assets = Assets::default();
        assets.shaders.insert(
            AssetKey(11),
            Residency::Resident(ShaderRef(ContentId([4; 32]))),
        );
        assets.textures.insert(
            AssetKey(12),
            Residency::Resident(TextureRef(ContentId([5; 32]))),
        );
        assets
    }

    #[test]
    fn resolves_captured_state_into_drawlist_materials() {
        let mut draws = empty_draw_list();
        let captured = state();

        resolve_materials(&mut draws, &assets(), &[captured.clone()]).unwrap();

        let material = &draws.materials[0];
        assert_eq!(material.id, captured.id);
        assert_eq!(material.shader, ShaderRef(ContentId([4; 32])));
        assert_eq!(
            material.textures[0].texture,
            tsfp_scene_model::TextureSource::Asset(TextureRef(ContentId([5; 32])))
        );
        assert_eq!(material.textures[0].slot, 0);
        assert_eq!(material.textures[0].uv_set, 1);
        assert_eq!(material.textures[0].uv_transform[0][2], 0.25);
        assert_eq!(material.parameters, captured.parameters);
        assert_eq!(material.raster, captured.raster);
    }

    #[test]
    fn failed_resolution_keeps_the_existing_material_snapshot() {
        let mut draws = empty_draw_list();
        resolve_materials(&mut draws, &assets(), &[state()]).unwrap();
        let before = draws.materials.clone();
        let unavailable = Assets::default();

        assert!(resolve_materials(&mut draws, &unavailable, &[state()]).is_err());
        assert_eq!(draws.materials, before);
    }

    #[test]
    fn source_attaches_each_material_snapshot_to_its_present_number() {
        let mut bytes = Vec::new();
        wire::frame(&mut bytes, 0);
        wire::call(&mut bytes, "DrawVertices", &[3, 0, 6]);
        wire::present(&mut bytes, 17);
        wire::call(&mut bytes, "DrawVertices", &[3, 6, 6]);
        wire::present(&mut bytes, 18);

        let source = StreamDrawListSource::new(
            Cursor::new(bytes),
            Window {
                first_present: 17,
                frame_count: 2,
            },
        )
        .unwrap()
        .with_material_snapshot(18, &assets(), &[state()])
        .unwrap();
        let mut source = source;

        let first = source.next_frame(64 * 1024).unwrap().unwrap();
        let second = source.next_frame(64 * 1024).unwrap().unwrap();
        assert_eq!(first.present, 17);
        assert!(first.draws.materials.is_empty());
        assert_eq!(second.present, 18);
        assert_eq!(second.draws.materials.len(), 1);
        assert_eq!(second.draws.materials[0].id, state().id);
    }
}
