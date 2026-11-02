//! Pointer samples, design-space mapping and cursor visibility.

use crate::layout::WidgetId;

pub const DESIGN_WIDTH: f32 = 640.0;
pub const DESIGN_HEIGHT: f32 = 480.0;

/// Window pixels to the 640x480 design space. Height fits 480, width is centred, so under a
/// widescreen window x extends below 0 and above 640 (anchored widgets live there).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesignMapping {
    scale: f32,
    offset_x: f32,
}

impl DesignMapping {
    pub fn from_window(width_px: f32, height_px: f32) -> Self {
        let scale = (height_px / DESIGN_HEIGHT).max(f32::EPSILON);
        Self {
            scale,
            offset_x: (width_px - DESIGN_WIDTH * scale) * 0.5,
        }
    }
    pub fn to_design(&self, px: f32, py: f32) -> (f32, f32) {
        ((px - self.offset_x) / self.scale, py / self.scale)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerButtons {
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

/// One frame of pointer state. `pos` is design space, `None` when the device has no position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerFrame {
    pub pos: Option<(f32, f32)>,
    pub buttons: PointerButtons,
    /// Wheel notches this frame, positive = away from the user (scroll up).
    pub wheel: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointerPolicy {
    /// Cursor hides after this long without pointer activity.
    pub idle_hide_ms: u64,
    /// List items moved per wheel notch.
    pub wheel_step: i32,
    /// Hover moves focus (T2168).
    pub hover_focus: bool,
}

impl Default for PointerPolicy {
    fn default() -> Self {
        Self {
            idle_hide_ms: 3000,
            wheel_step: 1,
            hover_focus: true,
        }
    }
}

/// Pointer state of one context. `visible` is also the `wants_pointer` output.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiPointerState {
    pub visible: bool,
    pub last_activity_ms: u64,
    pub last_pos: Option<(f32, f32)>,
    pub hovered: Option<WidgetId>,
    pub(crate) pressed_left: Option<WidgetId>,
    pub(crate) prev: PointerButtons,
}
