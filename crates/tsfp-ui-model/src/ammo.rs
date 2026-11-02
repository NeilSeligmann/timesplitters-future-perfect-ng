//! T2191 UI-8: the HUD ammo and weapon readout as typed, readable Rust (`original` preset).
//!
//! Same selection rule as the guest-memory view `tsfp_ui::hud::state::PlayerHudState::read`
//! (T1642 layout): the held hand names a weapon record; a record index inside the table gives the
//! ammo type and clip size, and the loaded and reserve counts are the per-type counters for that
//! ammo type. Hands (no record) show zero. The readout is hidden for weapon entry 1 (INFERRED
//! from measured frames, the original element drawer of 0x14F0B0 is not identified). Placement
//! fractions are INFERRED from frames, not from code. Contract: docs/t2191-ingame-ui.md, section
//! "Ammo, crosshair and objective models".

/// Weapon entry for which no readout is drawn (bare hands). INFERRED.
pub const HANDS_ENTRY: u32 = 1;

/// The two fields of a weapon record the readout uses (record `+0x14`, `+0x18`, INFERRED names).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponRecord {
    pub ammo_type: u32,
    pub clip_size: u32,
}

/// Per-ammo-type counters of one player: loaded in the held hand, reserve in the inventory.
pub trait AmmoCounters {
    fn loaded(&self, ammo_type: u32) -> u16;
    fn reserve(&self, ammo_type: u32) -> u16;
}

/// What the player holds, as the player and inventory systems expose it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldWeapon {
    /// Current weapon entry (character `+0x94`, MEASURED).
    pub entry: u32,
    /// Weapon record index of the held hand; out of table range means no record (hands, -1).
    pub record_index: u32,
}

/// The values the readout shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AmmoReadout {
    pub visible: bool,
    pub weapon_entry: u32,
    pub ammo_type: u32,
    pub clip_size: u32,
    pub loaded: u32,
    pub reserve: u32,
}

impl AmmoReadout {
    /// Build the readout. `records` is the weapon record table (its length is the valid range).
    pub fn of(held: HeldWeapon, records: &[WeaponRecord], counters: &impl AmmoCounters) -> Self {
        let record = usize::try_from(held.record_index)
            .ok()
            .and_then(|index| records.get(index));
        let (ammo_type, clip_size, loaded, reserve) = match record {
            Some(record) => (
                record.ammo_type,
                record.clip_size,
                u32::from(counters.loaded(record.ammo_type)),
                u32::from(counters.reserve(record.ammo_type)),
            ),
            None => (0, 0, 0, 0),
        };
        AmmoReadout {
            visible: held.entry != HANDS_ENTRY,
            weapon_entry: held.entry,
            ammo_type,
            clip_size,
            loaded,
            reserve,
        }
    }

    /// Rounds still loaded as a fraction of the clip (None for no clip). For opt-in features.
    pub fn clip_fraction(&self) -> Option<f32> {
        (self.clip_size != 0).then(|| self.loaded as f32 / self.clip_size as f32)
    }
}

/// Screen placement of the readout inside a viewport `[left, right, top, bottom]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmoLayout {
    pub reserve_at: (i32, i32),
    pub loaded_at: (i32, i32),
    pub weapon_label_at: (i32, i32),
}

impl AmmoLayout {
    /// `original` preset: reserve at 0.81, loaded at 0.91 of the width, 0.88 of the height, the
    /// weapon label one 16 px line above. INFERRED from measured frames (atlas bullet icon between
    /// the counts not modelled).
    pub fn original(viewport: [i32; 4]) -> Self {
        let [left, right, top, bottom] = viewport;
        let width = right - left;
        let height = bottom - top;
        let y = top + height * 88 / 100;
        AmmoLayout {
            reserve_at: (left + width * 81 / 100, y),
            loaded_at: (left + width * 91 / 100, y),
            weapon_label_at: (left + width * 81 / 100, y - 16),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counters;
    impl AmmoCounters for Counters {
        fn loaded(&self, ammo_type: u32) -> u16 {
            ammo_type as u16 + 10
        }
        fn reserve(&self, ammo_type: u32) -> u16 {
            ammo_type as u16 + 100
        }
    }

    #[test]
    fn record_selects_ammo_type_and_hands_show_nothing() {
        let records = [WeaponRecord {
            ammo_type: 3,
            clip_size: 30,
        }];
        let armed = AmmoReadout::of(
            HeldWeapon {
                entry: 2,
                record_index: 0,
            },
            &records,
            &Counters,
        );
        assert_eq!(
            (armed.loaded, armed.reserve, armed.clip_size),
            (13, 103, 30)
        );
        assert!(armed.visible);
        let hands = AmmoReadout::of(
            HeldWeapon {
                entry: 1,
                record_index: u32::MAX,
            },
            &records,
            &Counters,
        );
        assert_eq!(
            hands,
            AmmoReadout {
                weapon_entry: 1,
                ..AmmoReadout::default()
            }
        );
        let past_end = AmmoReadout::of(
            HeldWeapon {
                entry: 2,
                record_index: 1,
            },
            &records,
            &Counters,
        );
        assert_eq!((past_end.loaded, past_end.visible), (0, true));
        assert_eq!(armed.clip_fraction(), Some(13.0 / 30.0));
        assert!(
            AmmoReadout::of(
                HeldWeapon {
                    entry: 0,
                    record_index: 0
                },
                &records,
                &Counters
            )
            .visible
        );
    }
}
