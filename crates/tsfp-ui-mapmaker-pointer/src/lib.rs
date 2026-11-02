// SPDX-License-Identifier: GPL-3.0-or-later
//! Native Map Maker cursor ownership and edit-view placement boundary (T2328).
//! No guest-memory writes or synthetic controller events cross this contract.

use tsfp_mapmaker::edit_view::{EditViewCursor, EditViewCursorTransform, EditViewPoint};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointerDevice {
    #[default]
    Pad,
    Mouse,
}

/// Position in the editor's 640 by 480 design space, inside its picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorPosition {
    pub x: f32,
    pub y: f32,
}

impl EditorPosition {
    pub fn new(x: f32, y: f32) -> Option<Self> {
        (x.is_finite() && y.is_finite() && (0.0..=640.0).contains(&x) && (0.0..=480.0).contains(&y))
            .then_some(Self { x, y })
    }
}

/// Implemented by the Rust-owned edit view. The view owns projection, grid snapping,
/// selection and placement; the input adapter only supplies the cursor's position.
pub trait EditViewPointer {
    type Error;
    fn hover(&mut self, position: Option<EditorPosition>) -> Result<(), Self::Error>;
    fn place(&mut self, position: EditorPosition) -> Result<(), Self::Error>;
}

/// Concrete Rust-owned consumer for editor cursor projection.
///
/// This adapter turns the design-space pointer contract into a projected hover
/// point and a queued placement intent. The edit document still owns resolving
/// that intent to a selectable/valid map object; this type does not mutate a
/// document or emulate guest pointer state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeEditViewPointer {
    transform: EditViewCursorTransform,
    hover: Option<EditViewPoint>,
    place: Option<EditViewPoint>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEditViewPointerError {
    InvalidProjection,
}

impl NativeEditViewPointer {
    pub const fn new(transform: EditViewCursorTransform) -> Self {
        Self {
            transform,
            hover: None,
            place: None,
        }
    }

    pub fn hover_point(&self) -> Option<EditViewPoint> {
        self.hover
    }

    /// Consume the latest placement intent exactly once.
    pub fn take_placement(&mut self) -> Option<EditViewPoint> {
        self.place.take()
    }
}

impl EditViewPointer for NativeEditViewPointer {
    type Error = NativeEditViewPointerError;

    fn hover(&mut self, position: Option<EditorPosition>) -> Result<(), Self::Error> {
        self.hover = position
            .and_then(|p| EditViewCursor::new(p.x, p.y))
            .and_then(|cursor| self.transform.map(cursor));
        // A queued click is only valid while its sampled hover remains current.
        // Losing the cursor must not let a later consumer place at stale coordinates.
        if self.hover.is_none() {
            self.place = None;
        }
        Ok(())
    }

    fn place(&mut self, position: EditorPosition) -> Result<(), Self::Error> {
        let projected = EditViewCursor::new(position.x, position.y)
            .and_then(|cursor| self.transform.map(cursor));
        let Some(projected) = projected else {
            self.place = None;
            return Err(NativeEditViewPointerError::InvalidProjection);
        };
        self.place = Some(projected);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EditorPointer {
    device: PointerDevice,
    position: Option<EditorPosition>,
    last_mouse: Option<EditorPosition>,
}

impl EditorPointer {
    pub const fn new() -> Self {
        Self {
            device: PointerDevice::Pad,
            position: None,
            last_mouse: None,
        }
    }
    pub fn device(&self) -> PointerDevice {
        self.device
    }
    pub fn position(&self) -> Option<EditorPosition> {
        self.position
    }
    pub fn system_cursor(&self) -> bool {
        self.device == PointerDevice::Mouse
    }
    pub fn game_pointer(&self) -> bool {
        self.device == PointerDevice::Pad
    }

    /// A parked startup cursor does not steal ownership. Motion or a button/wheel
    /// edge does; losing the cursor clears hover but retains the active device.
    pub fn mouse(&mut self, position: Option<EditorPosition>, action_edge: bool) {
        let moved = position.is_some() && self.last_mouse.is_some() && position != self.last_mouse;
        self.last_mouse = position;
        self.position = position;
        if moved || action_edge {
            self.device = PointerDevice::Mouse;
        }
    }

    /// Call only for actual pad activity, not a connected or resting controller.
    pub fn pad_activity(&mut self) {
        self.device = PointerDevice::Pad;
    }

    /// One native edit-view input sample, independent of simulation/frame counters.
    /// A click outside the picture cannot place an object at a stale position.
    pub fn edit_view<V: EditViewPointer>(
        &self,
        view: &mut V,
        place_edge: bool,
    ) -> Result<(), V::Error> {
        if !self.system_cursor() {
            return Ok(());
        }
        view.hover(self.position)?;
        if place_edge && let Some(position) = self.position {
            view.place(position)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct View {
        hover: Option<EditorPosition>,
        placed: Vec<EditorPosition>,
    }
    impl EditViewPointer for View {
        type Error = ();
        fn hover(&mut self, position: Option<EditorPosition>) -> Result<(), ()> {
            self.hover = position;
            Ok(())
        }
        fn place(&mut self, position: EditorPosition) -> Result<(), ()> {
            self.placed.push(position);
            Ok(())
        }
    }
    #[test]
    fn ownership_follows_activity_and_pad_returns_original_pointer() {
        let mut p = EditorPointer::default();
        let a = EditorPosition::new(100.0, 200.0);
        p.mouse(a, false);
        p.mouse(a, false);
        assert!(p.game_pointer());
        p.mouse(EditorPosition::new(101.0, 200.0), false);
        assert!(p.system_cursor());
        p.pad_activity();
        p.mouse(p.position(), false);
        assert!(p.game_pointer());
        p.mouse(p.position(), true);
        assert!(p.system_cursor());
    }
    #[test]
    fn native_placement_never_uses_stale_or_invalid_positions() {
        let mut p = EditorPointer::default();
        let mut view = View::default();
        let position = EditorPosition::new(640.0, 480.0).unwrap();
        p.mouse(Some(position), true);
        p.edit_view(&mut view, true).unwrap();
        assert_eq!(view.placed, vec![position]);
        p.mouse(None, true);
        p.edit_view(&mut view, true).unwrap();
        assert_eq!(view.hover, None);
        assert_eq!(view.placed.len(), 1);
        p.pad_activity();
        p.edit_view(&mut view, true).unwrap();
        assert_eq!(view.placed.len(), 1);
        for (x, y) in [
            (f32::NAN, 0.0),
            (0.0, f32::INFINITY),
            (-1.0, 0.0),
            (641.0, 0.0),
        ] {
            assert!(EditorPosition::new(x, y).is_none());
        }
    }

    #[test]
    fn native_view_consumes_cursor_as_typed_projection_and_one_shot_place_intent() {
        let transform = EditViewCursorTransform {
            screen_origin_x: 10.0,
            screen_origin_y: 20.0,
            world_origin_x: 100.0,
            world_origin_y: -50.0,
            world_per_screen_x: 2.0,
            world_per_screen_y: 0.5,
        };
        let mut pointer = EditorPointer::default();
        let mut view = NativeEditViewPointer::new(transform);
        pointer.mouse(EditorPosition::new(30.0, 60.0), true);
        pointer.edit_view(&mut view, true).unwrap();
        let expected = EditViewPoint { x: 140.0, y: -30.0 };
        assert_eq!(view.hover_point(), Some(expected));
        assert_eq!(view.take_placement(), Some(expected));
        assert_eq!(view.take_placement(), None);

        pointer.mouse(None, false);
        pointer.edit_view(&mut view, true).unwrap();
        assert_eq!(view.hover_point(), None);
        assert_eq!(view.take_placement(), None);
    }

    #[test]
    fn lost_or_invalid_cursor_cancels_pending_placement() {
        let transform = EditViewCursorTransform {
            screen_origin_x: 0.0,
            screen_origin_y: 0.0,
            world_origin_x: 0.0,
            world_origin_y: 0.0,
            world_per_screen_x: 1.0,
            world_per_screen_y: 1.0,
        };
        let valid = EditorPosition::new(12.0, 34.0).unwrap();
        let mut view = NativeEditViewPointer::new(transform);
        view.place(valid).unwrap();
        view.hover(None).unwrap();
        assert_eq!(view.take_placement(), None);

        view.place(valid).unwrap();
        assert_eq!(
            view.place(EditorPosition {
                x: f32::NAN,
                y: 0.0
            }),
            Err(NativeEditViewPointerError::InvalidProjection)
        );
        assert_eq!(view.take_placement(), None);
    }
}
