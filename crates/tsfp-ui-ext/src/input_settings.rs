// SPDX-License-Identifier: GPL-3.0-or-later
//! T2211 model half: opt-in in-game input settings rows (mouse sensitivity and friends) on the
//! Rust UI model. The values live in `tsfp_input::shaping::PlayerSettings`, the same record the
//! host settings file (`InputConfig`) parses, so a row edit and a file edit agree by construction.
//! See docs/t2211-input-settings-rows.md for the contract.

use crate::{Extension, SettingKind, SettingRow, settings_rows};
use tsfp_input::shaping::PlayerSettings;
use tsfp_ui_input::{ActionArg, ActionId, BindResult, ListId, ListView, TextId as UiTextId};
use tsfp_ui_input::{UiBindings, UiValue, ValueId};
use tsfp_ui_model::{Insert, PageId, TextId, TextRef, UiModel, UiOption, WidgetId};
use tsfp_ui_renderer::AnchoredRect;

/// Value and action ids of this extension start here, clear of any table-derived id.
pub const ID_BASE: u32 = 0x2211_0000;
const FIELDS_PER_PLAYER: u32 = 16;

/// One editable input option of one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputField {
    SensitivityX,
    SensitivityY,
    InvertX,
    InvertY,
    Preset,
    DeadZone,
    CurveExponent,
}

impl InputField {
    pub const ALL: [InputField; 7] = [
        InputField::SensitivityX,
        InputField::SensitivityY,
        InputField::InvertX,
        InputField::InvertY,
        InputField::Preset,
        InputField::DeadZone,
        InputField::CurveExponent,
    ];

    /// Inclusive slider range, the same limits `InputConfig::parse` accepts.
    pub fn range(self) -> Option<(f32, f32)> {
        match self {
            InputField::SensitivityX | InputField::SensitivityY => Some((0.0, 100.0)),
            InputField::DeadZone => Some((0.0, 0.99)),
            InputField::CurveExponent => Some((0.1, 8.0)),
            _ => None,
        }
    }

    fn index(self) -> u32 {
        Self::ALL.iter().position(|f| *f == self).unwrap() as u32
    }

    pub fn value_id(self, player: u8) -> ValueId {
        ValueId(ID_BASE + u32::from(player) * FIELDS_PER_PLAYER + self.index())
    }

    pub fn decode(id: ValueId) -> Option<(u8, InputField)> {
        let offset = id.0.checked_sub(ID_BASE)?;
        let player = u8::try_from(offset / FIELDS_PER_PLAYER).ok()?;
        let field = *Self::ALL.get((offset % FIELDS_PER_PLAYER) as usize)?;
        Some((player, field))
    }
}

/// Per-player "Reset to defaults" action id.
pub fn reset_action(player: u8) -> ActionId {
    ActionId(ID_BASE + u32::from(player) * FIELDS_PER_PLAYER + FIELDS_PER_PLAYER - 1)
}

/// The settings state the rows edit. Defaults equal `PlayerSettings::default()`, which is
/// today's behaviour, so an untouched screen changes nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct InputSettings {
    pub players: Vec<PlayerSettings>,
    /// Preset names in cycler order; the cycler index selects one.
    pub presets: Vec<String>,
    /// Set by every accepted edit, cleared by the persistence owner after saving.
    pub dirty: bool,
}

impl InputSettings {
    pub fn new(players: usize, presets: Vec<String>) -> Self {
        Self {
            players: vec![PlayerSettings::default(); players],
            presets,
            dirty: false,
        }
    }

    pub fn read(&self, player: u8, field: InputField) -> UiValue {
        let Some(p) = self.players.get(usize::from(player)) else {
            return UiValue::None;
        };
        match field {
            InputField::SensitivityX => UiValue::Float(p.sensitivity_x),
            InputField::SensitivityY => UiValue::Float(p.sensitivity_y),
            InputField::InvertX => UiValue::Bool(p.invert_x),
            InputField::InvertY => UiValue::Bool(p.invert_y),
            // An unknown preset name (from a hand-edited file) shows as no selection.
            InputField::Preset => self
                .presets
                .iter()
                .position(|n| *n == p.preset)
                .map_or(UiValue::None, |i| UiValue::Index(i as u32)),
            // None means "use the preset's own value" and shows as no value.
            InputField::DeadZone => p.dead_zone.map_or(UiValue::None, UiValue::Float),
            InputField::CurveExponent => p.curve_exponent.map_or(UiValue::None, UiValue::Float),
        }
    }

    /// Applies one row edit. Out-of-range floats clamp, non-finite floats and wrong value
    /// shapes are refused, so a bad edit never reaches the shaper.
    pub fn write(&mut self, player: u8, field: InputField, value: UiValue) -> BindResult {
        let presets = &self.presets;
        let Some(p) = self.players.get_mut(usize::from(player)) else {
            return BindResult::Ignored;
        };
        let float = |v: f32| -> Option<f32> {
            let (low, high) = field.range()?;
            v.is_finite().then(|| v.clamp(low, high))
        };
        let accepted = match (field, value) {
            (InputField::SensitivityX, UiValue::Float(v)) => assign(&mut p.sensitivity_x, float(v)),
            (InputField::SensitivityY, UiValue::Float(v)) => assign(&mut p.sensitivity_y, float(v)),
            (InputField::InvertX, UiValue::Bool(v)) => assign(&mut p.invert_x, Some(v)),
            (InputField::InvertY, UiValue::Bool(v)) => assign(&mut p.invert_y, Some(v)),
            // Off/on cyclers report the option index.
            (InputField::InvertX, UiValue::Index(i @ 0..=1)) => {
                assign(&mut p.invert_x, Some(i == 1))
            }
            (InputField::InvertY, UiValue::Index(i @ 0..=1)) => {
                assign(&mut p.invert_y, Some(i == 1))
            }
            (InputField::Preset, UiValue::Index(i)) => {
                assign(&mut p.preset, presets.get(i as usize).cloned())
            }
            (InputField::DeadZone, UiValue::Float(v)) => {
                assign(&mut p.dead_zone, float(v).map(Some))
            }
            (InputField::DeadZone, UiValue::None) => assign(&mut p.dead_zone, Some(None)),
            (InputField::CurveExponent, UiValue::Float(v)) => {
                assign(&mut p.curve_exponent, float(v).map(Some))
            }
            (InputField::CurveExponent, UiValue::None) => assign(&mut p.curve_exponent, Some(None)),
            _ => false,
        };
        if accepted {
            self.dirty = true;
            BindResult::Handled
        } else {
            BindResult::Denied
        }
    }

    pub fn reset(&mut self, player: u8) -> BindResult {
        match self.players.get_mut(usize::from(player)) {
            Some(p) => {
                *p = PlayerSettings::default();
                self.dirty = true;
                BindResult::Handled
            }
            None => BindResult::Ignored,
        }
    }
}

/// Stores `value` when the edit was accepted (`Some`).
fn assign<T>(slot: &mut T, value: Option<T>) -> bool {
    value.map(|v| *slot = v).is_some()
}

/// Routes UI events for this extension's ids to [`InputSettings`]; every other id is
/// delegated to the game's bindings unchanged.
pub struct InputSettingsBindings<'a> {
    pub settings: &'a mut InputSettings,
    pub inner: &'a mut dyn UiBindings,
}

impl UiBindings for InputSettingsBindings<'_> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        let players = self.settings.players.len() as u8;
        match (0..players).find(|p| reset_action(*p) == id) {
            Some(player) => self.settings.reset(player),
            None => self.inner.action(id, arg),
        }
    }
    fn value(&self, id: ValueId) -> UiValue {
        match InputField::decode(id) {
            Some((player, field)) => self.settings.read(player, field),
            None => self.inner.value(id),
        }
    }
    fn set_value(&mut self, id: ValueId, value: UiValue) -> BindResult {
        match InputField::decode(id) {
            Some((player, field)) => self.settings.write(player, field, value),
            None => self.inner.set_value(id, value),
        }
    }
    fn list(&self, id: ListId) -> ListView {
        self.inner.list(id)
    }
    fn text(&self, id: UiTextId) -> &str {
        self.inner.text(id)
    }
}

/// Caller-supplied presentation: label text ids (from the settings owner, never guessed bank
/// ids), the off/on option texts and one rect per row.
#[derive(Clone, Debug, PartialEq)]
pub struct InputRowsLayout {
    pub page: PageId,
    pub at: Insert,
    pub first_widget: WidgetId,
    pub labels: Vec<(InputField, TextId)>,
    pub off_on: [TextRef; 2],
    pub preset_texts: Vec<TextRef>,
    /// Rect of the first row; row n is moved down by `n * row_step` logical units.
    pub first_rect: AnchoredRect,
    pub row_step: f32,
}

/// Builds the opt-in rows for one player. Only fields named in `layout.labels` get a row, in
/// that order; rows chain `After` the previous one so the block stays contiguous.
pub fn input_rows(model: &UiModel, player: u8, layout: &InputRowsLayout) -> Extension {
    let mut rows = Vec::new();
    let mut at = layout.at;
    for (n, (field, label)) in layout.labels.iter().enumerate() {
        let id = WidgetId(layout.first_widget.0 + n as u64);
        let options = |texts: &[TextRef]| {
            texts
                .iter()
                .enumerate()
                .map(|(i, t)| UiOption {
                    id: i as u32,
                    text: *t,
                    enabled: true,
                })
                .collect()
        };
        let kind = match field {
            InputField::InvertX | InputField::InvertY => {
                SettingKind::Cycler(options(&layout.off_on))
            }
            InputField::Preset => SettingKind::Cycler(options(&layout.preset_texts)),
            _ => SettingKind::Slider,
        };
        rows.push(SettingRow {
            page: layout.page,
            id,
            at,
            label: *label,
            rect: {
                let mut rect = layout.first_rect;
                rect.offset.y += n as f32 * layout.row_step;
                rect
            },
            value: field.value_id(player),
            kind,
        });
        at = Insert::After(id);
    }
    let mut extension = settings_rows(model, &rows);
    extension.patch.name = format!("input-settings-p{player}");
    extension
}
