// SPDX-License-Identifier: GPL-3.0-or-later
use crate::*;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sampling {
    /// Current fixed-tick endpoint, alpha zero in comparison receipts.
    Original,
    /// Previous at 0, current at 1. Simulation remains at current_tick.
    Interpolated { alpha: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSample {
    pub previous_tick: u64,
    pub current_tick: u64,
    pub sampling: Sampling,
}
impl FrameSample {
    pub fn validate(self) -> Result<(), ContractError> {
        if self.previous_tick > self.current_tick {
            return Err(ContractError::InvalidInterval);
        }
        if let Sampling::Interpolated { alpha } = self.sampling {
            if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
                return Err(ContractError::InvalidAlpha);
            }
            if self.previous_tick != self.current_tick
                && self.previous_tick.checked_add(1) != Some(self.current_tick)
            {
                return Err(ContractError::InvalidInterval);
            }
        }
        Ok(())
    }

    /// Weight consumed by owner-specific interpolation (never matrix lerp by
    /// default). Cuts, changed IDs and new objects set `discontinuity` true.
    pub fn current_weight(self, discontinuity: bool) -> Result<f32, ContractError> {
        self.validate()?;
        Ok(match self.sampling {
            Sampling::Original => 1.,
            Sampling::Interpolated { alpha } => {
                if discontinuity || self.previous_tick == self.current_tick {
                    1.
                } else {
                    alpha
                }
            }
        })
    }
}

/// Typed fixed-tick presentation state. Query trees/portal graphs remain local
/// to the prep lane. Arrays retain owner order; identity never comes from index.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneState {
    pub tick: u64,
    pub targets: Vec<RenderTarget>,
    pub cameras: Vec<Camera>,
    pub models: Vec<ModelInstance>,
    pub materials: Vec<Material>,
    pub lights: Vec<Light>,
    pub effects: Vec<EffectBatch>,
}

#[derive(Clone, Debug)]
pub struct SceneFrame {
    pub previous: Arc<SceneState>,
    pub current: Arc<SceneState>,
}
impl SceneFrame {
    pub fn sample(&self, sampling: Sampling) -> Result<FrameSample, ContractError> {
        let sample = FrameSample {
            previous_tick: self.previous.tick,
            current_tick: self.current.tick,
            sampling,
        };
        sample.validate()?;
        Ok(sample)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Surface,
    Offscreen(RenderTargetId),
}

/// Order is semantically significant, including clears and repeated instances.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Clear {
        colour: Option<[f32; 4]>,
        depth: Option<f32>,
        stencil: Option<u8>,
    },
    Mesh {
        instance: InstanceId,
        lights: Vec<LightId>,
        scissor: Rect,
    },
    Particles {
        batch: EffectId,
        lights: Vec<LightId>,
        scissor: Rect,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct DrawPass {
    pub camera: CameraId,
    pub target: Target,
    pub extent: [u32; 2],
    pub draws: Vec<Draw>,
}

/// One fully sampled frame. The prep lane owns interpolation; every matrix,
/// palette, light and particle here is the value for `sample`, not an endpoint
/// awaiting backend interpolation. Passes/draws may not be reordered or dropped.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawList {
    pub sample: FrameSample,
    pub targets: Vec<RenderTarget>,
    pub cameras: Vec<Camera>,
    pub models: Vec<ModelInstance>,
    pub materials: Vec<Material>,
    pub lights: Vec<Light>,
    pub effects: Vec<EffectBatch>,
    pub passes: Vec<DrawPass>,
}
