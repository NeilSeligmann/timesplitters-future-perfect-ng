// SPDX-License-Identifier: GPL-3.0-or-later
//! T2238 immutable portal projection and room visibility.
//! Source contracts: 0x87A70, 0x87BB0, 0x87F80, 0x151FC0.
//! Native records hold IDs and values; no guest pointers, writes or counters.
use super::spatial::{Plane, points_inside};
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}
impl Rect {
    pub fn nonempty(self) -> bool {
        self.left < self.right && self.top < self.bottom
    }
    pub fn intersect(self, other: Self) -> Self {
        Self {
            left: self.left.max(other.left),
            right: self.right.min(other.right),
            top: self.top.max(other.top),
            bottom: self.bottom.min(other.bottom),
        }
    }
    fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            right: self.right.max(other.right),
            top: self.top.min(other.top),
            bottom: self.bottom.max(other.bottom),
        }
    }
    fn narrow(self) -> Self {
        Self {
            left: i32::from(self.left as i16),
            right: i32::from(self.right as i16),
            top: i32::from(self.top as i16),
            bottom: i32::from(self.bottom as i16),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionViewport {
    pub x_offset: f32,
    pub x_scale: f32,
    pub y_offset: f32,
    pub y_scale: f32,
}
/// Column-major 0x151FC0 transform, original z,y,x,translation order.
pub fn transform(matrix: &[f32; 16], point: [f32; 3]) -> [f32; 4] {
    core::array::from_fn(|row| {
        ((matrix[8 + row] * point[2] + matrix[4 + row] * point[1]) + matrix[row] * point[0])
            + matrix[12 + row]
    })
}
/// Plane flags follow the original unordered-compare acceptance. Intersection
/// uses the original distance ratio and emits previous-to-current edge order.
fn clip(
    points: &[[f32; 4]],
    axis: usize,
    fraction: f32,
    reject_less: bool,
) -> Option<Vec<[f32; 4]>> {
    if points.is_empty() {
        return None;
    }
    let outside = |p: &[f32; 4]| {
        if reject_less {
            p[axis] < p[3] * fraction
        } else {
            p[axis] > p[3] * fraction
        }
    };
    if points.iter().all(outside) {
        return None;
    }
    if !points.iter().any(outside) {
        return Some(points.to_vec());
    }
    let mut output = Vec::new();
    let mut previous = points[points.len() - 1];
    let mut previous_out = outside(&previous);
    for &current in points {
        let current_out = outside(&current);
        if previous_out != current_out {
            let distance = previous[3] * fraction - previous[axis];
            let next_distance = current[3] * fraction - current[axis];
            let t = distance / (distance - next_distance);
            output.push(core::array::from_fn(|i| {
                (current[i] - previous[i]) * t + previous[i]
            }));
        }
        if !current_out {
            output.push(current);
        }
        previous = current;
        previous_out = current_out;
    }
    Some(output)
}
/// Original projection clips w>=0, then left/right/top/bottom in homogeneous
/// coordinates. z does not participate. Input points remain untouched.
pub fn clip_project(
    points: &[[f32; 4]],
    viewport: ProjectionViewport,
    rect: Rect,
) -> Option<Vec<[f32; 2]>> {
    let points = clip(points, 3, 0., true)?;
    let points = clip(
        &points,
        0,
        (rect.left as f32 - viewport.x_offset) / viewport.x_scale,
        true,
    )?;
    let points = clip(
        &points,
        0,
        (rect.right as f32 - viewport.x_offset) / viewport.x_scale,
        false,
    )?;
    let points = clip(
        &points,
        1,
        0. - ((rect.top as f32 - viewport.y_offset) / viewport.y_scale),
        false,
    )?;
    let points = clip(
        &points,
        1,
        0. - ((rect.bottom as f32 - viewport.y_offset) / viewport.y_scale),
        true,
    )?;
    Some(
        points
            .iter()
            .map(|p| {
                [
                    viewport.x_scale * p[0] / p[3] + viewport.x_offset,
                    viewport.y_offset - viewport.y_scale * p[1] / p[3],
                ]
            })
            .collect(),
    )
}
fn truncate(value: f32) -> i32 {
    if !(-2147483648.0..2147483648.0).contains(&value) {
        i32::MIN
    } else {
        value as i32
    }
}
/// Original screen bounds round toward zero, then expand high edges by 1.
pub fn screen_bounds(points: &[[f32; 2]]) -> Option<Rect> {
    let first = points.first()?;
    let mut rect = Rect {
        left: truncate(first[0]),
        right: truncate(first[0]),
        top: truncate(first[1]),
        bottom: truncate(first[1]),
    };
    for point in &points[1..] {
        if rect.left as f32 > point[0] {
            rect.left = truncate(point[0]);
        }
        if point[0] > rect.right as f32 {
            rect.right = truncate(point[0]);
        }
        if rect.top as f32 > point[1] {
            rect.top = truncate(point[1]);
        }
        if point[1] > rect.bottom as f32 {
            rect.bottom = truncate(point[1]);
        }
    }
    rect.right = rect.right.wrapping_add(1);
    rect.bottom = rect.bottom.wrapping_add(1);
    Some(rect)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    pub portals: Vec<usize>,
    pub crossing_suppressed: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Portal {
    pub rooms: [Option<usize>; 2],
    pub normal: [f32; 3],
    /// The original plane origin is read even for a zero-vertex portal.
    pub origin: [f32; 3],
    pub points: Vec<[f32; 3]>,
    pub closed_viewers: u16,
}
#[derive(Clone, Debug, PartialEq)]
pub struct VisibilityScene {
    pub rooms: Vec<Room>,
    pub portals: Vec<Portal>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct VisibilityView {
    pub position: [f32; 3],
    pub matrix: [f32; 16],
    pub viewport: ProjectionViewport,
    pub full_rect: Rect,
    pub viewer: u32,
    pub near_portal_distance: f32,
    /// Native typed convex volumes of the per-viewer portal clip list (0x8C620).
    pub clip_groups: Vec<Vec<Plane>>,
    pub mapmaker: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomCoverage {
    pub depth: i32,
    pub rect: Rect,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibilityOutput {
    pub rooms: Vec<RoomCoverage>,
    pub portals: Vec<Rect>,
    pub portal_visits: Vec<u32>,
    pub max_depth: i32,
    pub active_rooms: Vec<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisibilityError {
    InvalidRoom,
    InvalidPortal,
    InvalidViewer,
    DepthOverflow,
}

/// Paired input and output for one immutable portal-visibility calculation.
/// Private fields prevent camera/render consumers from accidentally combining
/// one view's room result with another viewer's target metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct VisibilityPresentation {
    view: VisibilityView,
    output: VisibilityOutput,
}
impl VisibilityPresentation {
    pub fn calculate(
        scene: &VisibilityScene,
        view: VisibilityView,
        start: usize,
        depth: i32,
        rect: Rect,
        initial: &VisibilityOutput,
    ) -> Result<Self, VisibilityError> {
        let output = room_visibility(scene, &view, start, depth, rect, initial)?;
        Ok(Self { view, output })
    }

    pub fn view(&self) -> &VisibilityView {
        &self.view
    }

    pub fn output(&self) -> &VisibilityOutput {
        &self.output
    }
}

/// Owned per-view traversal scratch. Repeated calls start from the same inputs,
/// so Map Maker visit limits and path recursion flags never advance simulation.
pub fn room_visibility(
    scene: &VisibilityScene,
    view: &VisibilityView,
    start: usize,
    depth: i32,
    rect: Rect,
    initial: &VisibilityOutput,
) -> Result<VisibilityOutput, VisibilityError> {
    if view.viewer >= 16 {
        return Err(VisibilityError::InvalidViewer);
    }
    if initial.rooms.len() != scene.rooms.len()
        || initial.portals.len() != scene.portals.len()
        || initial.portal_visits.len() != scene.portals.len()
        || initial.active_rooms.len() != scene.rooms.len()
    {
        return Err(VisibilityError::InvalidRoom);
    }
    let mut output = initial.clone();
    let mut active = initial.active_rooms.clone();
    fn visit(
        scene: &VisibilityScene,
        view: &VisibilityView,
        room: usize,
        depth: i32,
        mut rect: Rect,
        output: &mut VisibilityOutput,
        active: &mut [bool],
    ) -> Result<(), VisibilityError> {
        let definition = scene.rooms.get(room).ok_or(VisibilityError::InvalidRoom)?;
        active[room] = true;
        let coverage = &mut output.rooms[room];
        if depth > coverage.depth {
            coverage.depth = depth;
            output.max_depth = output.max_depth.max(depth);
        }
        coverage.rect = coverage.rect.union(rect).narrow();
        for &index in &definition.portals {
            let portal = scene
                .portals
                .get(index)
                .ok_or(VisibilityError::InvalidPortal)?;
            let other = if portal.rooms[0] == Some(room) {
                portal.rooms[1]
            } else {
                portal.rooms[0]
            };
            let Some(other) = other else {
                continue;
            };
            let neighbour = scene.rooms.get(other).ok_or(VisibilityError::InvalidRoom)?;
            if active[other] {
                continue;
            }
            let origin = portal.origin;
            let delta = core::array::from_fn::<_, 3, _>(|i| view.position[i] - origin[i]);
            let distance = (portal.normal[2] * delta[2] + portal.normal[1] * delta[1])
                + portal.normal[0] * delta[0];
            let magnitude = f32::from_bits(distance.to_bits() & 0x7fff_ffff);
            if depth == 0 && magnitude < view.near_portal_distance {
                // The original near-plane seam recurses at depth 1 with the
                // same rectangle, before closed-mask/group/backface tests.
                output.portals[index] = rect.narrow();
                visit(scene, view, other, 1, rect, output, active)?;
                continue;
            }
            if portal.rooms[0] == Some(room) && distance > 0. {
                continue;
            }
            if portal.rooms[1] == Some(room) && distance < 0. {
                continue;
            }
            if portal.closed_viewers & (1u16 << view.viewer) != 0
                || definition.crossing_suppressed
                || neighbour.crossing_suppressed
            {
                continue;
            }
            if view
                .clip_groups
                .iter()
                .any(|planes| points_inside(&portal.points, planes))
            {
                continue;
            }
            if view.mapmaker {
                output.portal_visits[index] = output.portal_visits[index].wrapping_add(1);
                match output.portal_visits[index] {
                    2 => rect = view.full_rect,
                    3 => continue,
                    _ => {}
                }
            }
            let points: Vec<_> = portal
                .points
                .iter()
                .map(|p| transform(&view.matrix, *p))
                .collect();
            let Some(projected) = clip_project(&points, view.viewport, rect) else {
                continue;
            };
            let Some(bounds) = screen_bounds(&projected) else {
                continue;
            };
            let bounds = bounds.intersect(rect);
            let accumulated = output.portals[index].union(bounds).narrow();
            let changed = accumulated != output.portals[index];
            output.portals[index] = accumulated;
            if bounds.nonempty() && (changed || !view.mapmaker) {
                let next_depth = depth.checked_add(1).ok_or(VisibilityError::DepthOverflow)?;
                visit(scene, view, other, next_depth, bounds, output, active)?;
            }
        }
        active[room] = false;
        Ok(())
    }
    visit(scene, view, start, depth, rect, &mut output, &mut active)?;
    output.active_rooms = active;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    #[test]
    fn projection_clips_each_edge_and_preserves_input() {
        let viewport = ProjectionViewport {
            x_offset: 100.,
            x_scale: 100.,
            y_offset: 100.,
            y_scale: 100.,
        };
        let rect = Rect {
            left: 0,
            right: 200,
            top: 0,
            bottom: 200,
        };
        let points = [
            [-2., -2., 0., 1.],
            [2., -2., 0., 1.],
            [2., 2., 0., 1.],
            [-2., 2., 0., 1.],
        ];
        let original = points;
        let projected = clip_project(&points, viewport, rect).unwrap();
        assert_eq!(projected.len(), 4);
        assert_eq!(
            screen_bounds(&projected).unwrap(),
            Rect {
                left: 0,
                right: 201,
                top: 0,
                bottom: 201
            }
        );
        assert_eq!(points, original);
        assert!(clip_project(&[[0., 0., 0., -1.]], viewport, rect).is_none());
    }
    #[test]
    fn near_plane_room_cycle_is_reentrant_and_closed_mask_is_respected() {
        let rect = Rect {
            left: 0,
            right: 200,
            top: 0,
            bottom: 200,
        };
        let empty = Rect {
            left: 200,
            right: -1,
            top: 200,
            bottom: -1,
        };
        let scene = VisibilityScene {
            rooms: vec![
                Room {
                    portals: vec![0],
                    crossing_suppressed: false,
                },
                Room {
                    portals: vec![0],
                    crossing_suppressed: false,
                },
            ],
            portals: vec![Portal {
                rooms: [Some(0), Some(1)],
                normal: [0., 0., 1.],
                origin: [-0.5, -0.5, 0.],
                points: vec![
                    [-0.5, -0.5, 0.],
                    [0.5, -0.5, 0.],
                    [0.5, 0.5, 0.],
                    [-0.5, 0.5, 0.],
                ],
                closed_viewers: 1,
            }],
        };
        let view = VisibilityView {
            position: [0., 0., 0.],
            matrix: [
                1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
            ],
            viewport: ProjectionViewport {
                x_offset: 100.,
                x_scale: 100.,
                y_offset: 100.,
                y_scale: 100.,
            },
            full_rect: rect,
            viewer: 0,
            near_portal_distance: 0.01,
            clip_groups: vec![],
            mapmaker: false,
        };
        let initial = VisibilityOutput {
            rooms: vec![
                RoomCoverage {
                    depth: -1,
                    rect: empty
                };
                2
            ],
            portals: vec![empty],
            portal_visits: vec![0],
            max_depth: -1,
            active_rooms: vec![false; 2],
        };
        let a = room_visibility(&scene, &view, 0, 0, rect, &initial).unwrap();
        assert_eq!(a.rooms[1].depth, 1); // near-plane branch precedes closed mask
        assert_eq!(
            a,
            room_visibility(&scene, &view, 0, 0, rect, &initial).unwrap()
        );
        assert_eq!(initial.rooms[1].depth, -1);
        let mut far = view;
        far.position[2] = -1.;
        assert_eq!(
            room_visibility(&scene, &far, 0, 0, rect, &initial)
                .unwrap()
                .rooms[1]
                .depth,
            -1
        );
    }
}
