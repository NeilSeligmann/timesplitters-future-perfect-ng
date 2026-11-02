// SPDX-License-Identifier: GPL-3.0-or-later
//! Live collected-room inlet. This is a fixed-sample room pass, not a portal
//! rerun or a full object producer. T2244 removes the copied coverage adapter.
//! Complete native object producers use `FrameViews` with their query/planes.
use tsfp_engine_render::{
    camera::{CameraPresentation, CameraProjection},
    native_scene::NativeSceneOwner,
    scene::{
        render_prep::{PreparedSceneFrame, PreparedView, SceneError, View, ViewTarget},
        scene_collect::{CollectError, DrawRoom, collect_rooms},
        visibility::{ProjectionViewport, Rect},
    },
    spatial::Bounds,
};
pub use tsfp_render_prep_wire::NativeSceneView;

#[derive(Clone, Debug, PartialEq)]
pub enum LiveError {
    MissingCamera,
    InvalidView,
    CameraMismatch,
    StalePublication,
    RoomPlanMismatch,
    ObjectInputsRequired,
    Rooms(CollectError),
    Scene(SceneError),
}

pub fn enabled(value: Option<&str>) -> bool {
    value == Some("1")
}

fn rectangle(value: [i32; 4]) -> Rect {
    Rect {
        left: value[0],
        top: value[1],
        right: value[2],
        bottom: value[3],
    }
}

// The collector carries the row-major view-projection product, not the view.
// Allow rounding from the original f32 multiply and additions, bounded by the
// magnitude of the terms so cancellation does not cause a false rejection.
fn matches_view_projection(view: &[f32; 16], projection: &[f32; 16], actual: &[f32; 16]) -> bool {
    (0..4).all(|row| {
        (0..4).all(|col| {
            let terms =
                (0..4).map(|k| f64::from(view[row * 4 + k]) * f64::from(projection[k * 4 + col]));
            let (sum, magnitude) = terms.fold((0.0, 0.0), |(s, m), x| (s + x, m + x.abs()));
            (f64::from(actual[row * 4 + col]) - sum).abs()
                <= 8.0 * f64::from(f32::EPSILON) * magnitude.max(1.0)
        })
    })
}

/// The packet and owner publication must come from the same FIFO entry. The
/// owned Rust camera sample was taken at that entry's producer boundary, never
/// sampled later from a different tick. No history rotates on this call.
pub fn prepare_bridge_frame(
    owner: &NativeSceneOwner,
    sequence: u64,
    packet: &NativeSceneView,
    enabled: bool,
) -> Result<PreparedSceneFrame, LiveError> {
    if !enabled {
        return owner
            .prepare_display_frame(&[], 0.0)
            .map_err(LiveError::Scene);
    }
    if owner.current_tick() != Some(sequence) {
        return Err(LiveError::StalePublication);
    }
    let plan = owner.room_plan().ok_or(LiveError::StalePublication)?;
    if plan.tick != sequence {
        return Err(LiveError::StalePublication);
    }
    if packet.camera_valid != 1 {
        return Err(LiveError::MissingCamera);
    }
    if packet.viewer >= 16
        || packet.room_count as usize >= packet.coverage.len()
        || !rectangle(packet.rectangle).nonempty()
        || packet.viewport[1] == 0.
        || packet.viewport[3] == 0.
        || !packet
            .position
            .iter()
            .chain(packet.matrix.iter())
            .chain(packet.viewport.iter())
            .chain(packet.camera.eye.iter())
            .chain(packet.camera.view.iter())
            .chain(packet.camera.projection.iter())
            .chain(packet.camera.tangent_extents.iter())
            .all(|v| v.is_finite())
        || packet.camera.tangent_extents.iter().any(|v| *v <= 0.)
        || packet
            .camera
            .visible_rooms
            .iter()
            .any(|room| *room > packet.room_count as i32)
    {
        return Err(LiveError::InvalidView);
    }
    if packet.camera.eye != packet.position
        || !matches_view_projection(
            &packet.camera.view,
            &packet.camera.projection,
            &packet.matrix,
        )
    {
        return Err(LiveError::CameraMismatch);
    }
    // A room observation is not permission to omit objects silently or invent
    // their culling query. This gate must be removed only with complete inputs.
    if owner.object_count() != Some(0) {
        return Err(LiveError::ObjectInputsRequired);
    }
    let rooms: Vec<_> = (1..=packet.room_count)
        .map(|room| {
            let coverage = packet.coverage[room as usize];
            DrawRoom {
                room: room as u8,
                depth: coverage.depth,
                after_ordinary: coverage.after_ordinary != 0,
            }
        })
        .collect();
    let order = collect_rooms(&rooms, packet.max_depth).map_err(LiveError::Rooms)?;
    let ordered: Vec<_> = if order.draw_max_depth < 0 {
        Vec::new()
    } else {
        order.buckets[..=order.draw_max_depth as usize]
            .iter()
            .flatten()
            .copied()
            .collect()
    };
    if ordered != plan.rooms || order.draw_max_depth != plan.draw_max_depth {
        return Err(LiveError::RoomPlanMismatch);
    }
    let camera = CameraPresentation {
        eye: packet.camera.eye,
        view: packet.camera.view,
        projection: CameraProjection {
            matrix: packet.camera.projection,
            tangent_extents: packet.camera.tangent_extents,
        },
        visible_rooms: packet.camera.visible_rooms,
    };
    // This pass has no object query. Record the conservative finite domain;
    // ObjectInputsRequired above prevents using it to claim object culling.
    let view = View {
        query: Bounds {
            min: [-f32::MAX; 3],
            max: [f32::MAX; 3],
        },
        planes: Vec::new(),
        visible_rooms: ordered.iter().map(|room| u16::from(*room)).collect(),
    };
    let mut frame = owner
        .prepare_original_frame(&[])
        .map_err(LiveError::Scene)?;
    frame.views.push(PreparedView {
        tick: sequence,
        scene_light_view: None,
        view,
        camera: Some(camera),
        // A copied room result is not a complete portal traversal. Keep that
        // distinction visible rather than manufacturing a VisibilityPresentation.
        visibility: None,
        target: Some(ViewTarget {
            viewer: packet.viewer,
            viewport: ProjectionViewport {
                x_offset: packet.viewport[0],
                x_scale: packet.viewport[1],
                y_offset: packet.viewport[2],
                y_scale: packet.viewport[3],
            },
            rectangle: rectangle(packet.rectangle),
        }),
        room_order: Some(order),
        objects: Vec::new(),
    });
    Ok(frame)
}
