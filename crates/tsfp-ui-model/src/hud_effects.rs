//! T2312 increment 6: the Arcade effect icons under the rank box as native Rust (`0x12F670`, called by the HUD
//! frame `0x14F0B0` with the row alpha in `xmm0` and the HUD column's row cursor pointer as its stack argument).
//!
//! The original (MEASURED from the XBE disassembly, docs/t2312-native-hud.md, "Effect icons"): four power-up icons
//! in a row at the HUD column's left edge (viewport x0 + the HUD rectangle's left margin `0x7A6140`), each
//! `scale_x * 64 * 0.25` wide and `16 * scale_y` high, then the row cursor moves down by the height. Each icon is
//! drawn with `0x1BA70` in a colour lerped (`0x154D50`) from a dim grey (`alpha * 127 / 4` alpha) to the icon's
//! colour by the effect's strength: cloak from the effect fade state (`0x11F780`), speed, shrink and damage from
//! their timers at `+0xAC`, `+0xA8`, `+0xB0` of the player object's power-up block (`*(viewer object + 0x7C) +
//! 0xC68`, flags `+4`). The guest is never written; the row cursor advance is applied by the C gate (the
//! original's own write, as for the rank box).
use crate::hud_native::GuestRead;

/// Guest addresses (MEASURED from the disassembly of `0x12F670` and `0x11F780`).
pub mod effect_addr {
    /// The drawer: `xmm0` alpha (f32), `[esp+4]` pointer to the row cursor (int).
    pub const FN_EFFECTS: u32 = 0x0012_f670;
    /// Viewer (`+0x14` object), object `+0x7C` the player extension, `+0xC68` the power-up block.
    pub const VIEWER: u32 = 0x007b_0c7c;
    pub const OBJECT_EXTENSION: u32 = 0x7c;
    pub const POWERUP_BLOCK: u32 = 0xc68;
    /// Block fields: flags, fade state (0 fading in, 1 off, 2 fading out), elapsed and duration, the timers.
    pub const FLAGS: u32 = 0x4;
    pub const FADE_STATE: u32 = 0x20;
    pub const FADE_ELAPSED: u32 = 0x24;
    pub const FADE_DURATION: u32 = 0x2c;
    pub const SPEED_TIMER: u32 = 0xac;
    pub const SHRINK_TIMER: u32 = 0xa8;
    pub const DAMAGE_TIMER: u32 = 0xb0;
    /// Viewport (`+0` x0), HUD rectangle left margin, HUD layout scales.
    pub const VIEWPORT_PTR: u32 = 0x007e_71c8;
    pub const HUD_LEFT: u32 = 0x007a_6140;
    pub const SCALE_X: u32 = 0x004e_68f4;
    pub const SCALE_Y: u32 = 0x004e_68f8;
}

/// Constants of the drawer (f32 values of the XBE data).
pub mod k {
    /// Icon width `scale_x * 64 (0x47D1E0) * 0.25 (0x475D4C)`; the height is the width over `scale_x` times `scale_y`.
    pub const WIDTH: f32 = 64.0;
    pub const WIDTH_FACTOR: f32 = 0.25;
    /// Alpha scale 127 (`0x475CC0`).
    pub const ALPHA: f32 = 127.0;
    /// Timer scales: speed 0.05 (`0x475CE8`), shrink and damage 1/30 (`0x47D7C4`).
    pub const SPEED_SCALE: f32 = 0.05;
    pub const TIMER_SCALE: f32 = 0.033_333_335;
    /// Icon colours (low 24 bits of the `0x1BA70` colour word, A B G R high to low) and the dim grey.
    pub const COLOURS: [u32; 4] = [0x5060, 0x0060_4000, 0x6010, 0x1060];
    pub const DIM: u32 = 0x007f_7f7f;
}

/// Texture names of the four icons (texture ids 0xF0..0xF3; name rows id - 1).
pub const EFFECT_TEXTURES: [&str; 4] = [
    "newHud_powerupCloak",
    "newHud_powerupSpeed",
    "newHud_powerupShrink",
    "newHud_powerupDamage",
];

/// One icon: rect x0, y0, x1, y1 (guest pixels) and its colour word as `0x1BA70` takes it (A B G R high to low).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectIcon {
    pub texture: &'static str,
    pub rect: [f32; 4],
    pub colour: u32,
}

/// The four icons of one call and the row cursor advance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectIcons {
    pub icons: [EffectIcon; 4],
    /// What the original adds to the row cursor (`trunc(cursor + height) - cursor`).
    pub advance: u32,
    /// The strengths (0 dim .. 1 full colour) of cloak, speed, shrink, damage.
    pub strengths: [f32; 4],
}

/// `0x154D50(on, off, t)`: per byte `trunc(off * (1 - t) + on * t)` (x87).
pub fn lerp_colour(on: u32, off: u32, t: f32) -> u32 {
    let t = f64::from(t);
    (0..4).fold(0u32, |word, byte| {
        let shift = byte * 8;
        let (a, b) = (
            f64::from((off >> shift) & 0xff),
            f64::from((on >> shift) & 0xff),
        );
        word | (((a * (1.0 - t) + b * t) as i64 as u32 & 0xff) << shift)
    })
}

/// `0x11F780`: the effect fade ratio (1 without the active flag; state 0 `1 - elapsed / duration`, 1 zero,
/// 2 `elapsed / duration`, other 1).
pub fn fade_ratio<G: GuestRead + ?Sized>(g: &G, block: u32) -> Option<f32> {
    use effect_addr::*;
    if g.read32(block + FLAGS)? & 0x10 == 0 {
        return Some(1.0);
    }
    let fraction = || Some(g.read_f32(block + FADE_ELAPSED)? / g.read_f32(block + FADE_DURATION)?);
    Some(match g.read32(block + FADE_STATE)? {
        0 => 1.0 - fraction()?,
        1 => 0.0,
        2 => fraction()?,
        _ => 1.0,
    })
}

/// The original's clamp of a strength to 0..1 (values above 1 give 1, below 0 give 0).
fn clamp01(v: f32) -> f32 {
    if v > 1.0 {
        1.0
    } else if 0.0 > v {
        0.0
    } else {
        v
    }
}

impl EffectIcons {
    /// Replays `0x12F670` with `alpha` (`xmm0`) and the row cursor at `cursor` (the pointer in `[esp+4]`).
    pub fn read<G: GuestRead + ?Sized>(g: &G, alpha: f32, cursor: u32) -> Option<Self> {
        Self::read_input(g, alpha, cursor, None)
    }

    pub fn read_owned<G: GuestRead + ?Sized>(g: &G, alpha: f32, top: i32) -> Option<Self> {
        Self::read_input(g, alpha, 0, Some(top))
    }

    fn read_input<G: GuestRead + ?Sized>(
        g: &G,
        alpha: f32,
        cursor: u32,
        owned_top: Option<i32>,
    ) -> Option<Self> {
        use effect_addr::*;
        let object = g.read32(g.read32(VIEWER)? + 0x14)?;
        let block = g
            .read32(object + OBJECT_EXTENSION)?
            .wrapping_add(POWERUP_BLOCK);
        let (sx, sy) = (g.read_f32(SCALE_X)?, g.read_f32(SCALE_Y)?);
        let width = sx * k::WIDTH * k::WIDTH_FACTOR;
        let height = width / sx * sy;
        let viewport = g.read32(VIEWPORT_PTR).filter(|&p| p != 0)?;
        let mut x = (g.read32(viewport)? as i32).wrapping_add(g.read32(HUD_LEFT)? as i32) as f32;
        let row = match owned_top {
            Some(top) => top,
            None => g.read32(cursor)? as i32,
        };
        let y = row as f32;
        let a = (alpha * k::ALPHA) as i32;
        let dim = (((a >> 2) as u32) << 24) | k::DIM;
        let flags = g.read32(block + FLAGS)?;
        let strengths = [
            if flags & 0x10 != 0 {
                clamp01(1.0 - fade_ratio(g, block)?)
            } else {
                0.0
            },
            if flags & 0x1000 != 0 {
                clamp01(g.read_f32(block + SPEED_TIMER)? * k::SPEED_SCALE)
            } else {
                0.0
            },
            if flags & 0x400 != 0 {
                clamp01(g.read_f32(block + SHRINK_TIMER)? * k::TIMER_SCALE)
            } else {
                0.0
            },
            if flags & 0x800 != 0 {
                clamp01(g.read_f32(block + DAMAGE_TIMER)? * k::TIMER_SCALE)
            } else {
                0.0
            },
        ];
        let icons = std::array::from_fn(|i| {
            let icon = EffectIcon {
                texture: EFFECT_TEXTURES[i],
                rect: [x, y, x + width, y + height],
                colour: lerp_colour(((a as u32) << 24) | k::COLOURS[i], dim, strengths[i]),
            };
            x += width;
            icon
        });
        let advance = ((y + height) as i32).wrapping_sub(row) as u32;
        Some(EffectIcons {
            icons,
            advance,
            strengths,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED guest values on the real offsets.
    use super::*;
    use std::collections::HashMap;

    struct Guest(HashMap<u32, u32>);
    impl GuestRead for Guest {
        fn read32(&self, va: u32) -> Option<u32> {
            self.0.get(&va).copied()
        }
    }

    #[test]
    fn lerp_goes_from_the_dim_grey_to_the_colour() {
        let on = (127u32 << 24) | 0x5060;
        let off = (31u32 << 24) | 0x007f_7f7f;
        assert_eq!(lerp_colour(on, off, 0.0), off);
        assert_eq!(lerp_colour(on, off, 1.0), on);
        assert_eq!(
            lerp_colour(on, off, 0.5),
            (79 << 24) | (63 << 16) | (0x67 << 8) | 0x6f
        );
    }

    #[test]
    fn icons_row_and_strengths_replay_the_drawer() {
        use effect_addr::*;
        let mut g = Guest(HashMap::new());
        g.0.insert(VIEWER, 0x100);
        g.0.insert(0x114, 0x200);
        g.0.insert(0x200 + OBJECT_EXTENSION, 0x1000);
        let block = 0x1000 + POWERUP_BLOCK;
        // Cloak fading in (state 0, a quarter through), speed timer 10 (0.5), shrink off, damage 60 (clamped to 1).
        g.0.insert(block + FLAGS, 0x10 | 0x1000 | 0x800);
        g.0.insert(block + FADE_STATE, 0);
        g.0.insert(block + FADE_ELAPSED, 1.0f32.to_bits());
        g.0.insert(block + FADE_DURATION, 4.0f32.to_bits());
        g.0.insert(block + SPEED_TIMER, 10.0f32.to_bits());
        g.0.insert(block + SHRINK_TIMER, 30.0f32.to_bits());
        g.0.insert(block + DAMAGE_TIMER, 60.0f32.to_bits());
        g.0.insert(SCALE_X, 1.25f32.to_bits());
        g.0.insert(SCALE_Y, 1.0f32.to_bits());
        g.0.insert(VIEWPORT_PTR, 0x300);
        g.0.insert(0x300, 0);
        g.0.insert(HUD_LEFT, 32);
        g.0.insert(0x400, 80);
        let icons = EffectIcons::read(&g, 1.0, 0x400).unwrap();
        assert_eq!(icons.strengths, [0.25, 0.5, 0.0, 1.0]);
        assert_eq!(icons.icons[0].rect, [32.0, 80.0, 52.0, 96.0]);
        assert_eq!(icons.icons[3].rect, [92.0, 80.0, 112.0, 96.0]);
        assert_eq!(icons.advance, 16);
        // Shrink is off: the dim grey at a quarter of the alpha; damage full: the icon's colour.
        assert_eq!(icons.icons[2].colour, (31 << 24) | 0x007f_7f7f);
        assert_eq!(icons.icons[3].colour, (127 << 24) | 0x1060);
    }
}
