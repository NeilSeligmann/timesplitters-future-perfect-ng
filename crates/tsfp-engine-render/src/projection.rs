//! Typed, host-free camera projection contract for the future scene camera port.
//! No runtime caller selects this path yet; the original guest projection remains active.

/// A camera viewport measured in logical pixels, before the presentation scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
}

/// The original camera stores these tangent extents at +0x37c and +0x380.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerspectiveExtents {
    pub vertical: f32,
    pub horizontal: f32,
}

/// Compute Hor+ extents while keeping vertical FOV fixed. `content_aspect` is the
/// aspect of the full logical picture; `viewport` is the actual camera cell.
/// A split-screen cell therefore gets its own aspect. The original optional
/// ratio cap is represented by `max_ratio`; `None` leaves it uncapped.
///
/// Invalid dimensions or FOV return `None` instead of sending NaNs to a renderer.
pub fn hor_plus(
    vertical_fov_degrees: f32,
    viewport: Viewport,
    content_aspect: f32,
    max_ratio: Option<f32>,
) -> Option<PerspectiveExtents> {
    if !vertical_fov_degrees.is_finite()
        || !(0.0..180.0).contains(&vertical_fov_degrees)
        || !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width <= 0.0
        || viewport.height <= 0.0
        || !content_aspect.is_finite()
        || content_aspect <= 0.0
    {
        return None;
    }
    let mut ratio = viewport.width / viewport.height * content_aspect / (4.0 / 3.0);
    if let Some(cap) = max_ratio {
        if !cap.is_finite() || cap <= 0.0 {
            return None;
        }
        ratio = ratio.min(cap);
    }
    let vertical = (vertical_fov_degrees.to_radians() * 0.5).tan();
    let horizontal = vertical * ratio;
    if !vertical.is_finite() || !horizontal.is_finite() {
        return None;
    }
    Some(PerspectiveExtents {
        vertical,
        horizontal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_vertical_fov_expands_horizontally() {
        let viewport = Viewport {
            width: 640.0,
            height: 480.0,
        };
        for (aspect, expected) in [
            (4.0 / 3.0, 69.5),
            (16.0 / 9.0, 85.6),
            (7.0 / 3.0, 101.1),
            (32.0 / 9.0, 123.2),
        ] {
            let extents = hor_plus(55.0, viewport, aspect, None).unwrap();
            assert!((extents.vertical - 0.520567).abs() < 0.00001);
            let horizontal_degrees = 2.0 * extents.horizontal.atan().to_degrees();
            assert!(
                (horizontal_degrees - expected).abs() < 0.15,
                "{aspect}: {horizontal_degrees}"
            );
        }
    }

    #[test]
    fn split_cell_uses_its_own_viewport_and_cap() {
        let half = Viewport {
            width: 320.0,
            height: 480.0,
        };
        let full = Viewport {
            width: 640.0,
            height: 480.0,
        };
        let a = hor_plus(55.0, half, 7.0 / 3.0, None).unwrap();
        let b = hor_plus(55.0, full, 7.0 / 3.0, None).unwrap();
        assert!((a.horizontal * 2.0 - b.horizontal).abs() < 0.00001);
        let capped = hor_plus(55.0, full, 8.0, Some(5.0)).unwrap();
        assert!((capped.horizontal - capped.vertical * 5.0).abs() < 0.00001);
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        let viewport = Viewport {
            width: 640.0,
            height: 480.0,
        };
        assert!(hor_plus(180.0, viewport, 4.0 / 3.0, None).is_none());
        assert!(hor_plus(55.0, viewport, f32::NAN, None).is_none());
        assert!(hor_plus(
            55.0,
            Viewport {
                height: 0.0,
                ..viewport
            },
            1.0,
            None
        )
        .is_none());
        assert!(hor_plus(55.0, viewport, 1.0, Some(0.0)).is_none());
    }
}
