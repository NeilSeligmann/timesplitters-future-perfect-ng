// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 owned camera/visibility inlet for native scene preparation.
//! Producers supply typed state for a single sample; observations of guest
//! matrices alone cannot populate this contract. This API is not live adoption.

pub mod live;

use std::collections::BTreeSet;
use tsfp_engine_render::scene::{
    light_state::ViewId,
    render_prep::{CameraViewRequest, PreparedSceneFrame, Sampling, SceneError},
    scene_collect::{CollectError, DrawRoom, RoomOrder, collect_rooms},
    spatial::{Bounds, Plane},
    visibility::{
        VisibilityError, VisibilityOutput, VisibilityPresentation, VisibilityScene, VisibilityView,
    },
};
use tsfp_engine_render::{camera::CameraPresentation, native_scene::NativeSceneOwner};

/// Producer provenance, shared by the sampled camera, visibility and scene.
/// `Fixed` requires current camera state and alpha zero; `BetweenTicks` uses
/// the owner's previous/current interval. Producers must sample at this alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSample {
    pub tick: u64,
    pub alpha: f32,
    pub sampling: Sampling,
}

/// Complete room enumeration in original owner order, including invisible
/// rooms. Depth and admission are computed from this sample's visibility.
#[derive(Clone, Copy, Debug)]
pub struct OrderedRoom {
    pub room: u8,
    pub after_ordinary: bool,
}

/// Borrowed fixed-tick world and sampled presentation inputs for one target.
/// Query/planes/viewport are owner-supplied projection contracts, not guessed
/// from an observed matrix. Initial portal state is cloned by the traversal.
pub struct ViewInput<'a> {
    pub sample: FrameSample,
    pub camera: CameraPresentation,
    pub scene: &'a VisibilityScene,
    pub visibility: VisibilityView,
    pub start_room: usize,
    pub start_depth: i32,
    pub initial: &'a VisibilityOutput,
    pub room_order: &'a [OrderedRoom],
    pub query: Bounds,
    pub planes: Vec<Plane>,
    pub scene_light_view: Option<ViewId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FrameError {
    InvalidSample,
    SampleMismatch,
    MissingViews,
    DuplicateViewer,
    IncompleteRoomOrder,
    Visibility(VisibilityError),
    Rooms(CollectError),
    Scene(SceneError),
}

struct OwnedView {
    camera: CameraPresentation,
    visibility: VisibilityPresentation,
    room_order: RoomOrder,
    query: Bounds,
    planes: Vec<Plane>,
    scene_light_view: Option<ViewId>,
}

/// Atomic, immutable set of views. A failed view never publishes a partial set.
/// Owns all calculated inputs, so queued frames cannot borrow producer scratch.
pub struct FrameViews {
    sample: FrameSample,
    views: Vec<OwnedView>,
}

impl FrameViews {
    pub fn calculate(sample: FrameSample, inputs: &[ViewInput<'_>]) -> Result<Self, FrameError> {
        if !sample.alpha.is_finite()
            || !(0.0..=1.0).contains(&sample.alpha)
            || (sample.sampling == Sampling::Fixed && sample.alpha != 0.0)
        {
            return Err(FrameError::InvalidSample);
        }
        if inputs.is_empty() {
            return Err(FrameError::MissingViews);
        }
        let mut viewers = BTreeSet::new();
        let mut views = Vec::with_capacity(inputs.len());
        for input in inputs {
            if input.sample != sample {
                return Err(FrameError::SampleMismatch);
            }
            if !viewers.insert(input.visibility.viewer) {
                return Err(FrameError::DuplicateViewer);
            }
            if input.camera.eye != input.visibility.position {
                return Err(FrameError::Scene(SceneError::CameraEyeMismatch));
            }
            if input.camera.view != input.visibility.matrix {
                return Err(FrameError::Scene(SceneError::CameraMatrixMismatch));
            }
            let room_ids: BTreeSet<_> = input.room_order.iter().map(|room| room.room).collect();
            if input.room_order.len() != input.scene.rooms.len()
                || room_ids.len() != input.room_order.len()
                || room_ids
                    .iter()
                    .any(|&id| usize::from(id) >= input.scene.rooms.len())
            {
                return Err(FrameError::IncompleteRoomOrder);
            }
            let visibility = VisibilityPresentation::calculate(
                input.scene,
                input.visibility.clone(),
                input.start_room,
                input.start_depth,
                input.visibility.full_rect,
                input.initial,
            )
            .map_err(FrameError::Visibility)?;
            let output = visibility.output();
            let rooms: Vec<_> = input
                .room_order
                .iter()
                .filter_map(|room| {
                    let coverage = output.rooms[usize::from(room.room)];
                    (coverage.depth >= 0 && coverage.rect.nonempty()).then_some(DrawRoom {
                        room: room.room,
                        depth: coverage.depth,
                        after_ordinary: room.after_ordinary,
                    })
                })
                .collect();
            let room_order = collect_rooms(&rooms, output.max_depth).map_err(FrameError::Rooms)?;
            views.push(OwnedView {
                camera: input.camera,
                visibility,
                room_order,
                query: input.query,
                planes: input.planes.clone(),
                scene_light_view: input.scene_light_view,
            });
        }
        Ok(Self { sample, views })
    }

    /// Sample the authoritative owner without advancing its fixed-tick history.
    /// Repeated calls retain identical views but receive distinct resource epochs.
    /// A newer scene tick invalidates old camera/visibility inputs explicitly.
    pub fn prepare(&self, owner: &NativeSceneOwner) -> Result<PreparedSceneFrame, FrameError> {
        if owner.current_tick() != Some(self.sample.tick) {
            return Err(FrameError::SampleMismatch);
        }
        let requests: Vec<_> = self
            .views
            .iter()
            .map(|view| CameraViewRequest {
                camera: &view.camera,
                visibility: &view.visibility,
                room_order: &view.room_order,
                query: view.query,
                planes: view.planes.clone(),
                scene_light_view: view.scene_light_view,
            })
            .collect();
        match self.sample.sampling {
            Sampling::Fixed => owner.prepare_original_frame(&requests),
            Sampling::BetweenTicks => owner.prepare_display_frame(&requests, self.sample.alpha),
        }
        .map_err(FrameError::Scene)
    }
}
