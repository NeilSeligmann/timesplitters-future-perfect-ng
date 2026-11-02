//! Typed edit-view cursor projection primitives (T2328 continuation).
//!
//! The legacy cursor mapping at 0x2FBD60 transforms two cursor components by
//! per-axis origin/scale values before storing them in editor state. This module
//! keeps that operation on Rust-owned values; it does not read or write guest
//! memory and does not claim to implement the full editor projection, hit test,
//! snapping, selection, or placement pipeline.

/// Cursor components sampled in the editor's 640x480 design space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditViewCursor {
    pub x: f32,
    pub y: f32,
}

impl EditViewCursor {
    pub fn new(x: f32, y: f32) -> Option<Self> {
        (x.is_finite() && y.is_finite() && (0.0..=640.0).contains(&x) && (0.0..=480.0).contains(&y))
            .then_some(Self { x, y })
    }
}

/// Per-axis affine mapping used by the edit-view cursor conversion.
///
/// `screen_origin` and `world_origin` correspond to the source offsets and
/// biases; `world_per_screen` corresponds to its scale. The caller owns the
/// actual view parameters and can replace them as the viewport changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditViewCursorTransform {
    pub screen_origin_x: f32,
    pub screen_origin_y: f32,
    pub world_origin_x: f32,
    pub world_origin_y: f32,
    pub world_per_screen_x: f32,
    pub world_per_screen_y: f32,
}

/// Rust-owned edit-view cursor coordinate after the measured affine mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditViewPoint {
    pub x: f32,
    pub y: f32,
}

impl EditViewCursorTransform {
    /// Apply the two-axis mapping from 0x2FBD60 without mutating a guest context.
    pub fn map(self, cursor: EditViewCursor) -> Option<EditViewPoint> {
        let values = [
            self.screen_origin_x,
            self.screen_origin_y,
            self.world_origin_x,
            self.world_origin_y,
            self.world_per_screen_x,
            self.world_per_screen_y,
        ];
        if !values.into_iter().all(f32::is_finite) {
            return None;
        }
        let point = EditViewPoint {
            x: (cursor.x - self.screen_origin_x) * self.world_per_screen_x + self.world_origin_x,
            y: (cursor.y - self.screen_origin_y) * self.world_per_screen_y + self.world_origin_y,
        };
        (point.x.is_finite() && point.y.is_finite()).then_some(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_cursor_components_with_owned_view_parameters() {
        let transform = EditViewCursorTransform {
            screen_origin_x: 10.0,
            screen_origin_y: 20.0,
            world_origin_x: 100.0,
            world_origin_y: -50.0,
            world_per_screen_x: 2.0,
            world_per_screen_y: 0.5,
        };
        assert_eq!(
            transform.map(EditViewCursor::new(30.0, 60.0).unwrap()),
            Some(EditViewPoint { x: 140.0, y: -30.0 })
        );
    }

    #[test]
    fn rejects_invalid_cursor_or_non_finite_transform() {
        assert!(EditViewCursor::new(f32::NAN, 0.0).is_none());
        assert!(EditViewCursor::new(641.0, 0.0).is_none());
        let transform = EditViewCursorTransform {
            screen_origin_x: 0.0,
            screen_origin_y: 0.0,
            world_origin_x: 0.0,
            world_origin_y: 0.0,
            world_per_screen_x: f32::INFINITY,
            world_per_screen_y: 1.0,
        };
        assert!(
            transform
                .map(EditViewCursor::new(1.0, 1.0).unwrap())
                .is_none()
        );
    }
}
