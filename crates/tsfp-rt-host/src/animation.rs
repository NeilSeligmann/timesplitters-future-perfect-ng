// SPDX-License-Identifier: GPL-3.0-or-later
//! Native model submission into the live render thread (T2241).
//! Sampling is immutable and opt-in through AnimationScene::set_sampling.
use tsfp_engine_anim::display::ClipSource;
use tsfp_engine_anim::display_scene::{AnimationScene, ViewObject};
use tsfp_wgpu::animation::{Geometry, NodeBinding, prepare_draw};

use crate::Runtime;

impl Runtime {
    /// Sample one native object and enqueue its palette immediately followed by
    /// its geometry as one packet. The producer mutex prevents another object
    /// from interleaving uploads/draws. Missing clips/invalid model bindings fail
    /// before publication. Geometry resources must already be installed.
    ///
    /// Success means queue acceptance, not GPU completion. The existing stats
    /// and frame publication report execution. Closed/failed queues reject.
    pub fn draw_animated(
        &self,
        scene: &AnimationScene,
        view: ViewObject,
        alpha: f32,
        source: &impl ClipSource,
        bindings: &[NodeBinding],
        geometry: Geometry,
    ) -> Result<(), String> {
        let events = prepare_draw(scene, view, alpha, source, bindings, geometry)?;
        let mut bytes = Vec::new();
        for event in &events {
            tsfp_wgpu::binary::encode_event(&mut bytes, event, 0)?;
        }
        if !self.enqueue_binary(&bytes, 0) {
            return Err("native animation render queue closed or failed".into());
        }
        Ok(())
    }
}
