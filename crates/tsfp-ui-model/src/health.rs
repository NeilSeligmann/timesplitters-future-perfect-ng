//! T2191 UI-8: player health and armour segment bars as typed, readable Rust (`original` preset).
//!
//! Same behaviour as the Tier 1 guest-memory port `tsfp_ui::hud::health` (0x12C830, 0x13A140,
//! 0x13A1D0, 0x13A270, 0x13A2C0), but over owned state: no guest addresses, no retail constants.
//! Each bar has 20 segments; segment `i` is lit when `value >= threshold[i] * maximum`. A lit
//! state change restarts that segment's fade timer and raises the bar flash. The threshold
//! tables and timing constants are supplied at run time from the original tables. Contract:
//! docs/t2191-ingame-ui.md, section "Health segment model".

/// Segments per bar.
pub const SEGMENTS: usize = 20;

/// Run-time constants from the original tables (never committed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HealthTuning {
    /// Health threshold fractions (originally one float per 0x14 bytes at 0x4E6D80).
    pub health_thresholds: [f32; SEGMENTS],
    /// Armour threshold fractions (0x4E6F28).
    pub armor_thresholds: [f32; SEGMENTS],
    /// Flash value set when a segment changes (0x475D10).
    pub flash: f32,
    /// Segment fade timer start value (0x4785E0).
    pub segment_fade: f32,
    /// Numerator of the per-frame step `one / rate * delta` (0x475C78).
    pub one: f32,
    /// Factor of the slot timer advance (0x475CD4).
    pub half: f32,
    /// Factor of the slot timer start (0x475CAC).
    pub slot_start_scale: f32,
}

impl HealthTuning {
    /// Load the original preset's immutable tuning from the owner's XBE at run time.
    /// Missing or truncated sections fail closed; no substitute values are invented.
    pub fn from_xbe(image: &crate::xbe::XbeImage) -> Option<Self> {
        fn float(image: &crate::xbe::XbeImage, address: u32) -> Option<f32> {
            let bytes = [
                image.byte(address)?,
                image.byte(address.checked_add(1)?)?,
                image.byte(address.checked_add(2)?)?,
                image.byte(address.checked_add(3)?)?,
            ];
            Some(f32::from_le_bytes(bytes))
        }
        let mut health_thresholds = [0.0; SEGMENTS];
        let mut armor_thresholds = [0.0; SEGMENTS];
        for index in 0..SEGMENTS {
            let offset = (index as u32).checked_mul(0x14)?;
            health_thresholds[index] = float(image, 0x4e6d80u32.checked_add(offset)?)?;
            armor_thresholds[index] = float(image, 0x4e6f28u32.checked_add(offset)?)?;
        }
        Some(Self {
            health_thresholds,
            armor_thresholds,
            flash: float(image, 0x475d10)?,
            segment_fade: float(image, 0x4785e0)?,
            one: float(image, 0x475c78)?,
            half: float(image, 0x475cd4)?,
            slot_start_scale: float(image, 0x475cac)?,
        })
    }
}

#[cfg(test)]
mod tuning_tests {
    use super::*;

    #[test]
    fn loads_strided_tables_and_rejects_missing_section() {
        let base = 0x470000u32;
        let mut bytes = vec![0; 0x90000];
        bytes[..4].copy_from_slice(b"XBEH");
        bytes[0x104..0x108].copy_from_slice(&0x10000u32.to_le_bytes());
        bytes[0x11c..0x120].copy_from_slice(&1u32.to_le_bytes());
        bytes[0x120..0x124].copy_from_slice(&0x10200u32.to_le_bytes());
        bytes[0x204..0x208].copy_from_slice(&base.to_le_bytes());
        bytes[0x208..0x20c].copy_from_slice(&0x80000u32.to_le_bytes());
        bytes[0x20c..0x210].copy_from_slice(&0x400u32.to_le_bytes());
        bytes[0x210..0x214].copy_from_slice(&0x80000u32.to_le_bytes());
        let mut put = |address: u32, value: f32| {
            let at = 0x400 + (address - base) as usize;
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        for index in 0..SEGMENTS {
            put(0x4e6d80 + index as u32 * 0x14, index as f32);
            put(0x4e6f28 + index as u32 * 0x14, (index + 20) as f32);
        }
        for address in [0x475d10, 0x4785e0, 0x475c78, 0x475cd4, 0x475cac] {
            put(address, 3.5);
        }
        let image = crate::xbe::XbeImage::parse(bytes).unwrap();
        let tuning = HealthTuning::from_xbe(&image).unwrap();
        assert_eq!(tuning.health_thresholds[19], 19.0);
        assert_eq!(tuning.armor_thresholds[19], 39.0);
        assert_eq!(tuning.flash, 3.5);
        let missing = crate::xbe::XbeImage::parse(vec![0; 0x200]);
        assert!(missing.is_none());
    }

    #[test]
    fn loads_private_retail_image_when_supplied() {
        let Ok(path) = std::env::var("TSFP_PRIVATE_XBE") else {
            return;
        };
        let image = crate::xbe::XbeImage::load(std::path::Path::new(&path)).unwrap();
        let tuning = HealthTuning::from_xbe(&image).unwrap();
        assert!(tuning.health_thresholds.iter().all(|x| x.is_finite()));
        assert!(tuning.armor_thresholds.iter().all(|x| x.is_finite()));
        assert!(tuning.health_thresholds.iter().any(|x| *x != 0.0));
        assert!(tuning.armor_thresholds.iter().any(|x| *x != 0.0));
    }
}

/// What the bars read from the player's character. `armor` is `None` unless the object is a
/// character (class 8): then no armour segment is ever lit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vitals {
    pub health: f32,
    pub max_health: f32,
    pub armor: Option<ArmorPool>,
}

/// Armour pool of a character (character `+0x78`, maximum `+0x7C`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArmorPool {
    pub value: f32,
    pub max: f32,
}

/// Lit segments of both bars as the rule computes them now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LitSegments {
    pub health: [bool; SEGMENTS],
    pub armor: [bool; SEGMENTS],
}

/// `value >= fraction * max` with the original's unordered compare (NaN is not lit).
fn reaches(value: f32, fraction: f32, max: f32) -> bool {
    value >= fraction * max
}

impl LitSegments {
    pub fn of(vitals: &Vitals, tuning: &HealthTuning) -> Self {
        let mut lit = Self {
            health: [false; SEGMENTS],
            armor: [false; SEGMENTS],
        };
        for index in 0..SEGMENTS {
            lit.health[index] = reaches(
                vitals.health,
                tuning.health_thresholds[index],
                vitals.max_health,
            );
            if let Some(pool) = vitals.armor {
                lit.armor[index] = reaches(pool.value, tuning.armor_thresholds[index], pool.max);
            }
        }
        lit
    }
}

/// Per-player segment state (the flag block at object `+0x268` in the original).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HealthBars {
    /// Last lit state that was shown.
    pub shown: LitSegments,
    pub health_fade: [f32; SEGMENTS],
    pub armor_fade: [f32; SEGMENTS],
    /// Whole-bar flash timer, counts down to 0 or below.
    pub flash: f32,
}

impl HealthBars {
    /// 0x12C830: take the current lit state, clear every timer and the flash.
    pub fn new(vitals: &Vitals, tuning: &HealthTuning) -> Self {
        Self {
            shown: LitSegments::of(vitals, tuning),
            health_fade: [0.0; SEGMENTS],
            armor_fade: [0.0; SEGMENTS],
            flash: 0.0,
        }
    }

    /// 0x13A140: whether any segment would change.
    pub fn changed(&self, vitals: &Vitals, tuning: &HealthTuning) -> bool {
        self.shown != LitSegments::of(vitals, tuning)
    }

    /// 0x13A2C0: one frame. `step = one / rate_hz * delta`. Changed segments restart their fade
    /// and raise the flash; then the flash and every running fade age by `step`, clamped at 0.
    pub fn tick(&mut self, vitals: &Vitals, tuning: &HealthTuning, rate_hz: i32, delta: f32) {
        let step = tuning.one / rate_hz as f32 * delta;
        if self.flash > 0.0 {
            self.flash -= step;
        }
        let now = LitSegments::of(vitals, tuning);
        for index in 0..SEGMENTS {
            if self.shown.health[index] != now.health[index] {
                self.flash = tuning.flash;
                self.shown.health[index] = now.health[index];
                self.health_fade[index] = tuning.segment_fade;
            }
            if self.shown.armor[index] != now.armor[index] {
                self.flash = tuning.flash;
                self.shown.armor[index] = now.armor[index];
                self.armor_fade[index] = tuning.segment_fade;
            }
        }
        for fade in self
            .health_fade
            .iter_mut()
            .chain(self.armor_fade.iter_mut())
        {
            if *fade > 0.0 {
                *fade -= step;
                if 0.0 > *fade {
                    *fade = 0.0;
                }
            }
        }
    }
}

/// Per-player HUD slot timer (HUD block `+0x20`), counts frames up to `2 * rate_hz`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotTimer(pub i32);

/// `cvttss2si`: truncate toward zero, NaN and out of range give `i32::MIN`.
fn truncate(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

impl SlotTimer {
    /// 0x13A270 (timer part): start at `2 * rate_hz * slot_start_scale` frames. The original
    /// then rebuilds the bars with [`HealthBars::new`] when the player has a flag block.
    pub fn start(rate_hz: i32, tuning: &HealthTuning) -> Self {
        Self(truncate(
            rate_hz.wrapping_mul(2) as f32 * tuning.slot_start_scale,
        ))
    }

    /// 0x13A1D0: advance by `2 * rate_hz * scale * half`, capped at `2 * rate_hz`. Then the
    /// flash is raised unless a segment changed while either session flag (0x6D612C, 0x6D6130)
    /// is clear: `both_session_flags` is true when both are set.
    pub fn update(
        &mut self,
        bars: &mut HealthBars,
        vitals: &Vitals,
        tuning: &HealthTuning,
        rate_hz: i32,
        scale: f32,
        both_session_flags: bool,
    ) {
        let cap = rate_hz.wrapping_mul(2);
        self.0 = self
            .0
            .wrapping_add(truncate(cap as f32 * scale * tuning.half));
        if self.0 > cap {
            self.0 = cap;
        }
        if bars.changed(vitals, tuning) && !both_session_flags {
            return;
        }
        bars.flash = tuning.flash;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> HealthTuning {
        // FABRICATED stand-ins: evenly spaced fractions, round timing numbers.
        let mut thresholds = [0.0; SEGMENTS];
        for (index, value) in thresholds.iter_mut().enumerate() {
            *value = (index as f32 + 1.0) / SEGMENTS as f32;
        }
        HealthTuning {
            health_thresholds: thresholds,
            armor_thresholds: thresholds,
            flash: 1.0,
            segment_fade: 0.5,
            one: 1.0,
            half: 0.5,
            slot_start_scale: 0.25,
        }
    }

    fn vitals(health: f32, armor: Option<f32>) -> Vitals {
        Vitals {
            health,
            max_health: 100.0,
            armor: armor.map(|value| ArmorPool { value, max: 100.0 }),
        }
    }

    #[test]
    fn segments_follow_thresholds_and_armor_needs_a_character() {
        let lit = LitSegments::of(&vitals(50.0, None), &tuning());
        assert_eq!(lit.health.iter().filter(|lit| **lit).count(), 10);
        assert!(lit.armor.iter().all(|lit| !lit));
        let lit = LitSegments::of(&vitals(50.0, Some(100.0)), &tuning());
        assert!(lit.armor.iter().all(|lit| *lit));
    }

    #[test]
    fn damage_restarts_fades_and_flash_then_decays() {
        let tuning = tuning();
        let mut bars = HealthBars::new(&vitals(100.0, None), &tuning);
        let hurt = vitals(80.0, None);
        assert!(bars.changed(&hurt, &tuning));
        bars.tick(&hurt, &tuning, 10, 1.0);
        assert_eq!(bars.flash, 1.0);
        assert_eq!(bars.health_fade[19], 0.4);
        assert_eq!(bars.health_fade[15], 0.0);
        for _ in 0..10 {
            bars.tick(&hurt, &tuning, 10, 1.0);
        }
        assert!(bars.health_fade.iter().all(|fade| *fade == 0.0));
        assert!(!bars.changed(&hurt, &tuning));
    }

    #[test]
    fn slot_timer_caps_and_gates_flash() {
        let tuning = tuning();
        let mut timer = SlotTimer::start(30, &tuning);
        assert_eq!(timer, SlotTimer(15));
        let mut bars = HealthBars::new(&vitals(100.0, None), &tuning);
        timer.update(&mut bars, &vitals(10.0, None), &tuning, 30, 4.0, false);
        assert_eq!(timer, SlotTimer(60));
        assert_eq!(bars.flash, 0.0);
        timer.update(&mut bars, &vitals(10.0, None), &tuning, 30, 0.0, true);
        assert_eq!(bars.flash, 1.0);
    }
}
