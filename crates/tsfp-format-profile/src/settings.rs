//! Owned profile options. Storage facts: docs/t1708-settings-block.md and t-save-layout.md.
//! Original defaults and page option values are supplied by the runtime table loader.
use crate::Profile;
use tsfp_format_core::{Result, word};

pub const HEAD_SIZE: usize = 0x7c;
/// The retained 0x7c-byte profile settings head. Changing the layout selector
/// leaves saved custom mappings intact until `set_custom_mapping` is called.
///
/// ```
/// use tsfp_format_profile::settings::{Settings, HEAD_SIZE};
/// let mut settings = Settings::from_head(&[0; HEAD_SIZE]).unwrap();
/// settings.set_layout(2);
/// assert_eq!(settings.layout(), 2);
/// assert_eq!(settings.mapping(), [0; 22]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    bytes: [u8; HEAD_SIZE],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crosshair {
    Off,
    Moving,
    Fixed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponChange {
    Always,
    Never,
    Best,
    IfNew,
    IfNewAndBest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vibration {
    Off,
    Fire,
    Hit,
    FireAndHit,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Controls {
    pub crosshair: Crosshair,
    pub weapon_change: WeaponChange,
    pub inverse_look: bool,
    pub auto_aim: bool,
    pub lookahead: bool,
    pub aim_toggle: bool,
    pub crouch_toggle: bool,
    pub vibration: Vibration,
}
impl Settings {
    pub fn from_head(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            bytes: bytes.try_into().map_err(|_| "settings head length")?,
        })
    }
    pub fn head(&self) -> &[u8; HEAD_SIZE] {
        &self.bytes
    }
    pub fn layout(&self) -> u16 {
        u16::from_le_bytes(self.bytes[..2].try_into().unwrap())
    }
    /// Selector changes retain saved custom bindings, as preset generation is a separate operation.
    pub fn set_layout(&mut self, layout: u16) {
        self.bytes[..2].copy_from_slice(&layout.to_le_bytes());
    }
    pub fn mapping(&self) -> [u32; 22] {
        core::array::from_fn(|i| word(&self.bytes, 4 + i * 4).unwrap())
    }
    pub fn set_custom_mapping(&mut self, mapping: [u32; 22]) {
        self.set_layout(0);
        for (i, value) in mapping.into_iter().enumerate() {
            self.bytes[4 + i * 4..8 + i * 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    pub fn turn_speed(&self) -> f32 {
        f32::from_bits(word(&self.bytes, 0x78).unwrap())
    }
    pub fn set_turn_speed(&mut self, value: f32) -> Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("turn speed range");
        }
        self.bytes[0x78..].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }
    /// Original turn-speed row adjustment (0x2EAB30): 0.05 per step.
    /// The original upper comparison uses 0.9999, then stores 1.0 when exceeded.
    pub fn adjust_turn_speed(&mut self, increase: bool) -> Result<bool> {
        let before = self.turn_speed();
        if !before.is_finite() || !(0.0..=1.0).contains(&before) {
            return Err("turn speed range");
        }
        let after = if increase {
            if before >= 1.0 {
                before
            } else {
                let next = before + 0.05;
                if next > 0.9999 { 1.0 } else { next }
            }
        } else {
            (before - 0.05).max(0.0)
        };
        self.set_turn_speed(after)?;
        Ok(after != before)
    }

    /// Decode the valid menu domain; malformed enum values are retained in head(), not silently fixed.
    pub fn controls(&self) -> Result<Controls> {
        let f = self.bytes[2];
        Ok(Controls {
            crosshair: match word(&self.bytes, 0x74)? {
                0 => Crosshair::Off,
                1 => Crosshair::Moving,
                2 => Crosshair::Fixed,
                _ => return Err("crosshair value"),
            },
            weapon_change: match word(&self.bytes, 0x68)? {
                0 => WeaponChange::Always,
                1 => WeaponChange::Never,
                2 => WeaponChange::Best,
                3 => WeaponChange::IfNew,
                4 => WeaponChange::IfNewAndBest,
                _ => return Err("weapon change value"),
            },
            inverse_look: f & 1 != 0,
            auto_aim: f & 0x80 != 0,
            lookahead: f & 8 != 0,
            aim_toggle: f & 2 != 0,
            crouch_toggle: f & 4 != 0,
            vibration: if f & 0x10 == 0 {
                Vibration::Off
            } else {
                match f & 0x60 {
                    0x40 => Vibration::Fire,
                    0x20 => Vibration::Hit,
                    0x60 => Vibration::FireAndHit,
                    _ => return Err("vibration value"),
                }
            },
        })
    }
    /// Write the menu controls while retaining the layout, bindings, speed, and spare bytes.
    ///
    /// ```
    /// use tsfp_format_profile::settings::{Controls, Crosshair, Settings, Vibration, WeaponChange, HEAD_SIZE};
    /// let mut settings = Settings::from_head(&[0xa5; HEAD_SIZE]).unwrap();
    /// let controls = Controls {
    ///     crosshair: Crosshair::Fixed, weapon_change: WeaponChange::Best,
    ///     inverse_look: true, auto_aim: false, lookahead: true,
    ///     aim_toggle: false, crouch_toggle: true, vibration: Vibration::Hit,
    /// };
    /// settings.set_controls(controls);
    /// assert_eq!(settings.controls().unwrap(), controls);
    /// assert_eq!(settings.head()[3], 0xa5);
    /// ```
    pub fn set_controls(&mut self, c: Controls) {
        self.bytes[2] = u8::from(c.inverse_look)
            | (u8::from(c.aim_toggle) << 1)
            | (u8::from(c.crouch_toggle) << 2)
            | (u8::from(c.lookahead) << 3)
            | (u8::from(c.auto_aim) << 7)
            | match c.vibration {
                Vibration::Off => 0,
                Vibration::Fire => 0x50,
                Vibration::Hit => 0x30,
                Vibration::FireAndHit => 0x70,
            };
        self.bytes[0x68..0x6c].copy_from_slice(&(c.weapon_change as u32).to_le_bytes());
        self.bytes[0x74..0x78].copy_from_slice(&(c.crosshair as u32).to_le_bytes());
    }
}
impl Profile {
    pub fn settings(&self) -> Result<Settings> {
        Ok(self.player_record()?.settings().clone())
    }
    /// Accept writes exactly 31 dwords. Caller must regenerate signature/CRC before durable save.
    pub fn accept_settings(&mut self, settings: &Settings) -> Result<()> {
        let mut record = self.player_record()?;
        *record.settings_mut() = settings.clone();
        self.replace_player_record(&record)
    }
}

/// A table-resolved original row. Text ids are resolved by the neighbouring text provider.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionRow {
    pub page: u32,
    pub row: u32,
    pub label_id: u32,
    pub binding: Binding,
    pub choices: Vec<u32>,
    pub default: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    ProfileWord { offset: usize },
    ProfileBits { offset: usize, mask: u32 },
    Host { key: u32 },
}
/// Loader reads the owner's original data at runtime, including defaults. No retail table is embedded.
pub trait OriginalTables {
    fn default_head(&mut self) -> Result<[u8; HEAD_SIZE]>;
    fn rows(&mut self) -> Result<Vec<OptionRow>>;
}
#[derive(Debug, Clone)]
pub struct SettingsPages {
    pub defaults: Settings,
    pub rows: Vec<OptionRow>,
}
impl SettingsPages {
    pub fn load(source: &mut impl OriginalTables) -> Result<Self> {
        let defaults = Settings::from_head(&source.default_head()?)?;
        let mut pages = Self {
            defaults,
            rows: vec![],
        };
        for row in source.rows()? {
            if matches!(row.binding, Binding::Host { .. }) {
                return Err("original host row");
            }
            pages.add_row(row)?;
        }
        Ok(pages)
    }
    /// Explicit opt-in extension; host rows never enter the profile envelope.
    pub fn add_row(&mut self, row: OptionRow) -> Result<()> {
        if self
            .rows
            .iter()
            .any(|r| r.page == row.page && r.row == row.row)
        {
            return Err("duplicate settings row");
        }
        if row.choices.is_empty() || !row.choices.contains(&row.default) {
            return Err("row default/choices");
        }
        match row.binding {
            Binding::ProfileWord { offset } | Binding::ProfileBits { offset, .. } => {
                if offset < 4
                    || offset % 4 != 0
                    || offset.checked_add(4).is_none_or(|end| end > 0x1290)
                {
                    return Err("profile binding extent");
                }
            }
            Binding::Host { .. } => {}
        }
        if let Binding::ProfileBits { mask, .. } = row.binding {
            if mask == 0 || row.choices.iter().any(|value| value & !mask != 0) {
                return Err("row bits outside mask");
            }
        }
        self.rows.push(row);
        Ok(())
    }
    pub fn apply(
        &self,
        profile: &mut Profile,
        page: u32,
        row: u32,
        value: u32,
        host: &mut impl HostSettings,
    ) -> Result<()> {
        // Rows are public and may have acquired duplicate keys after add_row.
        // Reject ambiguity before either the profile or host callback is changed.
        let mut matches = self.rows.iter().filter(|r| r.page == page && r.row == row);
        let r = matches.next().ok_or("unknown settings row")?;
        if matches.next().is_some() {
            return Err("duplicate settings row");
        }
        if !r.choices.contains(&value) {
            return Err("row value");
        }
        let end = match r.binding {
            Binding::ProfileWord { offset } | Binding::ProfileBits { offset, .. } => {
                let end = offset.checked_add(4).ok_or("profile binding extent")?;
                // Rows are public and can change after add_row validated them.
                if offset < 4 || offset % 4 != 0 || end > 0x1290 {
                    return Err("profile binding extent");
                }
                Some(end)
            }
            Binding::Host { .. } => None,
        };
        let (offset, value) = match r.binding {
            Binding::Host { key } => return host.set(key, value),
            Binding::ProfileWord { offset } => (offset, value),
            Binding::ProfileBits { offset, mask } => {
                // Public rows may have been edited after add_row validated the mask.
                if mask == 0 || value & !mask != 0 {
                    return Err("row bits outside mask");
                }
                (offset, (word(&profile.payload, offset)? & !mask) | value)
            }
        };
        profile
            .payload
            .get_mut(offset..end.expect("profile binding has an extent"))
            .ok_or("profile binding extent")?
            .copy_from_slice(&value.to_le_bytes());
        Ok(())
    }
}
pub trait HostSettings {
    fn set(&mut self, key: u32, value: u32) -> Result<()>;
}

/// Audio/video words written by 0x2C3AC0, relative to R (0x69640 returns R+0x80).
/// Names beyond music/sound are deliberately neutral pending original labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioVideo {
    pub music_enabled: u32,
    pub sound_enabled: u32,
    pub music_volume: u32,
    pub sound_volume: u32,
    pub slider_4: u32,
    pub option_5: u32,
}
impl Profile {
    pub fn audio_video(&self) -> Result<AudioVideo> {
        Ok(AudioVideo {
            music_enabled: word(&self.payload, 0x198)?,
            sound_enabled: word(&self.payload, 0x19c)?,
            music_volume: word(&self.payload, 0x1a0)?,
            sound_volume: word(&self.payload, 0x1a4)?,
            slider_4: word(&self.payload, 0x1ac)?,
            option_5: word(&self.payload, 0x1b0)?,
        })
    }
    /// Exact Accept write set; unvalidated toggles remain raw words. Sliders have measured limits.
    pub fn accept_audio_video(&mut self, value: AudioVideo) -> Result<()> {
        if value.music_volume > 16 || value.sound_volume > 16 || value.slider_4 > 64 {
            return Err("audio/video slider range");
        }
        if self.payload.len() < 0x1b4 {
            return Err("profile audio extent");
        }
        for (offset, word) in [
            (0x198, value.music_enabled),
            (0x19c, value.sound_enabled),
            (0x1a0, value.music_volume),
            (0x1a4, value.sound_volume),
            (0x1ac, value.slider_4),
            (0x1b0, value.option_5),
        ] {
            self.payload[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
        }
        Ok(())
    }
}

/// Editing is transactional: working state is independent until Accept; dropping it cancels.
#[derive(Debug, Clone)]
pub struct SettingsEdit {
    pub controls: Settings,
    pub audio_video: AudioVideo,
}
impl SettingsEdit {
    pub fn open(profile: &Profile) -> Result<Self> {
        Ok(Self {
            controls: profile.settings()?,
            audio_video: profile.audio_video()?,
        })
    }
    /// B asks for confirmation when any word of the working control head changed.
    /// Explicit Cancel drops the edit; it does not take this confirmation path.
    pub fn controls_changed(&self, profile: &Profile) -> Result<bool> {
        Ok(self.controls.head() != profile.settings()?.head())
    }
    pub fn accept_controls(self, profile: &mut Profile) -> Result<()> {
        profile.accept_settings(&self.controls)
    }
    pub fn accept_audio_video(self, profile: &mut Profile) -> Result<()> {
        profile.accept_audio_video(self.audio_video)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> Profile {
        let mut payload = vec![0x5a; 0x1f54];
        payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
        Profile {
            payload,
            signature: [17; 20],
        }
    }
    struct Tables;
    impl OriginalTables for Tables {
        fn default_head(&mut self) -> Result<[u8; HEAD_SIZE]> {
            Ok([0; HEAD_SIZE])
        }
        fn rows(&mut self) -> Result<Vec<OptionRow>> {
            Ok(vec![OptionRow {
                page: 1,
                row: 2,
                label_id: 3,
                binding: Binding::ProfileBits {
                    offset: 0x198,
                    mask: 1,
                },
                choices: vec![0, 1],
                default: 1,
            }])
        }
    }
    struct Host(Vec<(u32, u32)>);
    impl HostSettings for Host {
        fn set(&mut self, key: u32, value: u32) -> Result<()> {
            self.0.push((key, value));
            Ok(())
        }
    }
    #[test]
    fn runtime_rows_preserve_bits_and_separate_host_settings() {
        let mut pages = SettingsPages::load(&mut Tables).unwrap();
        let mut p = profile();
        let mut h = Host(vec![]);
        pages.apply(&mut p, 1, 2, 1, &mut h).unwrap();
        assert_eq!(word(&p.payload, 0x198).unwrap(), 0x5a5a5a5b);
        let before = p.payload.clone();
        assert!(pages.apply(&mut p, 1, 2, 2, &mut h).is_err());
        assert_eq!(p.payload, before);
        let row = OptionRow {
            page: 2,
            row: 1,
            label_id: 4,
            binding: Binding::Host { key: 7 },
            choices: vec![4, 5],
            default: 4,
        };
        pages.add_row(row.clone()).unwrap();
        assert!(pages.add_row(row).is_err());
        pages.apply(&mut p, 2, 1, 5, &mut h).unwrap();
        assert_eq!(h.0, vec![(7, 5)]);
        assert_eq!(p.payload, before);
        let bad = OptionRow {
            page: 3,
            row: 0,
            label_id: 0,
            binding: Binding::ProfileWord { offset: usize::MAX },
            choices: vec![0],
            default: 0,
        };
        assert!(pages.add_row(bad).is_err());
        for (offset, mask, choices) in [
            (0x199, 1, vec![0, 1]),
            (0x198, 0, vec![0]),
            (0x198, 1, vec![0, 2]),
        ] {
            assert!(
                pages
                    .add_row(OptionRow {
                        page: 4,
                        row: 0,
                        label_id: 0,
                        binding: Binding::ProfileBits { offset, mask },
                        choices,
                        default: 0,
                    })
                    .is_err()
            );
        }
    }
    #[test]
    fn edited_public_row_with_overflowing_offset_returns_extent_error() {
        let mut pages = SettingsPages {
            defaults: Settings::from_head(&[0; HEAD_SIZE]).unwrap(),
            rows: vec![],
        };
        let mut profile = profile();
        let original = profile.payload.clone();
        let mut host = Host(vec![]);
        for binding in [
            Binding::ProfileWord { offset: usize::MAX },
            Binding::ProfileBits {
                offset: usize::MAX,
                mask: 1,
            },
        ] {
            pages.rows = vec![OptionRow {
                page: 1,
                row: 2,
                label_id: 0,
                binding,
                choices: vec![1],
                default: 1,
            }];
            assert_eq!(
                pages.apply(&mut profile, 1, 2, 1, &mut host),
                Err("profile binding extent")
            );
            assert_eq!(profile.payload, original);
            assert!(host.0.is_empty());
        }
    }
    #[test]
    fn edited_public_row_cannot_write_outside_settings_region() {
        let mut pages = SettingsPages::load(&mut Tables).unwrap();
        let mut profile = profile();
        let original = profile.payload.clone();
        let mut host = Host(vec![]);
        for offset in [0, 5, 0x128d, 0x1290, 0x1f50] {
            for binding in [
                Binding::ProfileWord { offset },
                Binding::ProfileBits { offset, mask: 1 },
            ] {
                pages.rows[0].binding = binding;
                assert_eq!(
                    pages.apply(&mut profile, 1, 2, 1, &mut host),
                    Err("profile binding extent"),
                    "offset {offset:#x}"
                );
                assert_eq!(profile.payload, original);
                assert!(host.0.is_empty());
            }
        }
    }
    #[test]
    fn edited_public_bit_mask_cannot_be_empty() {
        let mut pages = SettingsPages::load(&mut Tables).unwrap();
        pages.rows[0].binding = Binding::ProfileBits {
            offset: 0x198,
            mask: 0,
        };
        pages.rows[0].choices.push(0);
        let mut profile = profile();
        let original = profile.payload.clone();
        let mut host = Host(vec![]);
        assert_eq!(
            pages.apply(&mut profile, 1, 2, 0, &mut host),
            Err("row bits outside mask")
        );
        assert_eq!(profile.payload, original);
        assert!(host.0.is_empty());
    }
    #[test]
    fn edited_public_row_keys_cannot_make_apply_ambiguous() {
        let mut pages = SettingsPages::load(&mut Tables).unwrap();
        pages
            .add_row(OptionRow {
                page: 2,
                row: 3,
                label_id: 0,
                binding: Binding::Host { key: 7 },
                choices: vec![1],
                default: 1,
            })
            .unwrap();
        pages.rows[0].page = 2;
        pages.rows[0].row = 3;
        let mut profile = profile();
        let original = profile.payload.clone();
        let mut host = Host(vec![]);
        assert_eq!(
            pages.apply(&mut profile, 2, 3, 1, &mut host),
            Err("duplicate settings row")
        );
        assert_eq!(profile.payload, original);
        assert!(host.0.is_empty());
    }
    #[test]
    fn cancelled_edit_preserves_profile_and_accept_changes_only_head() {
        let mut p = profile();
        let before = p.payload.clone();
        let mut edit = SettingsEdit::open(&p).unwrap();
        assert!(!edit.controls_changed(&p).unwrap());
        edit.controls.set_layout(0);
        assert!(edit.controls_changed(&p).unwrap());
        drop(edit);
        assert_eq!(p.payload, before);
        let mut edit = SettingsEdit::open(&p).unwrap();
        edit.controls.set_layout(2);
        edit.accept_controls(&mut p).unwrap();
        assert_eq!(p.settings().unwrap().layout(), 2);
        assert_eq!(&p.payload[..4], &before[..4]);
        assert_eq!(&p.payload[0x80..], &before[0x80..]);
        assert_eq!(p.signature, [17; 20]);
    }
    #[test]
    fn malformed_data_and_slider_limits() {
        assert!(Settings::from_head(&[0; HEAD_SIZE - 1]).is_err());
        let mut s = Settings::from_head(&[0; HEAD_SIZE]).unwrap();
        for v in [f32::NAN, f32::INFINITY, -0.1, 1.01] {
            assert!(s.set_turn_speed(v).is_err());
        }
        s.set_turn_speed(0.9999).unwrap();
        assert_eq!(s.turn_speed(), 0.9999);
        let mut p = profile();
        let before = p.payload.clone();
        let mut a = p.audio_video().unwrap();
        a.music_volume = 17;
        assert!(p.accept_audio_video(a).is_err());
        assert_eq!(p.payload, before);
        p.payload.truncate(4);
        assert!(p.settings().is_err());
        assert!(p.accept_settings(&s).is_err());
    }
    #[test]
    fn layout_and_mapping_preserve_unrelated_head_bytes() {
        for seed in 0u8..=255 {
            let head: [u8; HEAD_SIZE] =
                core::array::from_fn(|i| seed.wrapping_add((i as u8).wrapping_mul(37)));
            let mut settings = Settings::from_head(&head).unwrap();
            let mapping = settings.mapping();
            settings.set_layout(seed as u16 + 1);
            assert_eq!(settings.mapping(), mapping);
            assert_eq!(&settings.head()[2..], &head[2..]);

            let custom: [u32; 22] = core::array::from_fn(|i| (seed as u32) << 24 | i as u32);
            settings.set_custom_mapping(custom);
            assert_eq!(settings.layout(), 0);
            assert_eq!(settings.mapping(), custom);
            assert_eq!(&settings.head()[2..4], &head[2..4]);
            assert_eq!(&settings.head()[0x5c..], &head[0x5c..]);
        }
    }
    #[test]
    fn turn_speed_steps_reach_original_endpoints() {
        let mut settings = Settings::from_head(&[0; HEAD_SIZE]).unwrap();
        for _ in 0..30 {
            settings.adjust_turn_speed(true).unwrap();
        }
        assert_eq!(settings.turn_speed(), 1.0);
        assert!(!settings.adjust_turn_speed(true).unwrap());
        for _ in 0..30 {
            settings.adjust_turn_speed(false).unwrap();
        }
        assert_eq!(settings.turn_speed(), 0.0);
        assert!(!settings.adjust_turn_speed(false).unwrap());
        settings.set_turn_speed(0.96).unwrap();
        settings.adjust_turn_speed(true).unwrap();
        assert_eq!(settings.turn_speed(), 1.0);
    }

    #[test]
    fn rejected_turn_speeds_leave_head_unchanged() {
        for seed in 0u8..=255 {
            let head = [seed; HEAD_SIZE];
            let mut settings = Settings::from_head(&head).unwrap();
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 1.01] {
                assert!(settings.set_turn_speed(value).is_err());
                assert_eq!(settings.head(), &head);
            }
        }
    }

    #[test]
    fn controls_round_trip_and_preserve_other_settings_bytes() {
        let crosshairs = [Crosshair::Off, Crosshair::Moving, Crosshair::Fixed];
        let weapons = [
            WeaponChange::Always,
            WeaponChange::Never,
            WeaponChange::Best,
            WeaponChange::IfNew,
            WeaponChange::IfNewAndBest,
        ];
        let vibrations = [
            Vibration::Off,
            Vibration::Fire,
            Vibration::Hit,
            Vibration::FireAndHit,
        ];
        for (seed, crosshair) in crosshairs.into_iter().enumerate() {
            for weapon_change in weapons {
                for vibration in vibrations {
                    for flags in 0..32u8 {
                        let before: [u8; HEAD_SIZE] = core::array::from_fn(|i| {
                            (i as u8).wrapping_mul(37).wrapping_add(seed as u8)
                        });
                        let mut settings = Settings::from_head(&before).unwrap();
                        let controls = Controls {
                            crosshair,
                            weapon_change,
                            vibration,
                            inverse_look: flags & 1 != 0,
                            auto_aim: flags & 2 != 0,
                            lookahead: flags & 4 != 0,
                            aim_toggle: flags & 8 != 0,
                            crouch_toggle: flags & 16 != 0,
                        };
                        settings.set_controls(controls);
                        assert_eq!(settings.controls().unwrap(), controls);
                        for offset in 0..HEAD_SIZE {
                            if offset != 2
                                && !(0x68..0x6c).contains(&offset)
                                && !(0x74..0x78).contains(&offset)
                            {
                                assert_eq!(
                                    settings.head()[offset],
                                    before[offset],
                                    "offset {offset:#x}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
