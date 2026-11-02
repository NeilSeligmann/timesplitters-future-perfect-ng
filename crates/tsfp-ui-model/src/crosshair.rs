//! T2191 UI-8: the HUD crosshair as typed Rust (`original` preset is a FABRICATED stand-in).
//!
//! The original crosshair source is not recovered: 0x136F30 is only named "aim texture quad when
//! field 0x94 is zero" (INFERRED name, no port). Until it is, the `original` preset reproduces the
//! stand-in of `tsfp_ui::hud::frame` (four 4 px arms, 2 px gap, 1 px thick, alpha 0xFF, viewport
//! centre) so both paths agree, and the style is data so the real one drops in. Contract:
//! docs/t2191-ingame-ui.md, section "Ammo, crosshair and objective models".

/// One filled rectangle, right and bottom exclusive like the fill primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArmRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Crosshair shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrosshairStyle {
    /// Distance from the centre to the inner end of an arm.
    pub gap: i32,
    /// Arm length.
    pub length: i32,
    pub thickness: i32,
    pub alpha: u32,
}

impl CrosshairStyle {
    /// FABRICATED stand-in, equal to `tsfp_ui::hud::frame`.
    pub const ORIGINAL: CrosshairStyle = CrosshairStyle {
        gap: 2,
        length: 4,
        thickness: 1,
        alpha: 0xff,
    };

    /// The arms around the centre of `viewport` (`[left, right, top, bottom]`), in the order
    /// left, right, up, down.
    pub fn arms(&self, viewport: [i32; 4]) -> [ArmRect; 4] {
        let [left, right, top, bottom] = viewport;
        let cx = left + (right - left) / 2;
        let cy = top + (bottom - top) / 2;
        let (near, far, thick) = (self.gap, self.gap + self.length, self.thickness);
        [
            ArmRect {
                left: cx - far,
                top: cy,
                right: cx - near,
                bottom: cy + thick,
            },
            ArmRect {
                left: cx + near,
                top: cy,
                right: cx + far,
                bottom: cy + thick,
            },
            ArmRect {
                left: cx,
                top: cy - far,
                right: cx + thick,
                bottom: cy - near,
            },
            ArmRect {
                left: cx,
                top: cy + near,
                right: cx + thick,
                bottom: cy + far,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arms_surround_the_centre_without_touching_it() {
        let arms = CrosshairStyle::ORIGINAL.arms([0, 640, 0, 480]);
        assert_eq!(
            arms[0],
            ArmRect {
                left: 314,
                top: 240,
                right: 318,
                bottom: 241
            }
        );
        for arm in arms {
            let covers_centre =
                arm.left <= 320 && 320 < arm.right && arm.top <= 240 && 240 < arm.bottom;
            assert!(!covers_centre);
        }
    }
}
