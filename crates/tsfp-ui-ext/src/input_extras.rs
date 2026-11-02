// SPDX-License-Identifier: GPL-3.0-or-later
//! T2211 follow-up: opt-in rows for the input options outside `PlayerSettings` (smoothing,
//! acceleration, raw look, scope multiplier, rumble, device assignment, over
//! `tsfp_input::extras::PlayerExtras`) and key/button rebinding rows over the per-player
//! `tsfp_input::actions::ActionMap`. Same id and layering scheme as `input_settings`.
//! Contract: docs/t2211-input-extras.md.

use crate::{Binding, Extension, SettingKind, SettingRow, add_row, settings_rows};
use tsfp_input::actions::{ActionMap, Binding as InputBinding, INTENTS, InputId, Intent};
use tsfp_input::extras::{
    ACCELERATION_RANGE, DeviceAssignment, PlayerExtras, RUMBLE_RANGE, SCOPE_MULTIPLIER_RANGE,
    SMOOTHING_FRAMES_MAX,
};
use tsfp_ui_input::{ActionArg, ActionId, BindResult, ListId, ListView, TextId as UiTextId};
use tsfp_ui_input::{UiBindings, UiValue, ValueId};
use tsfp_ui_model::WidgetKind;
use tsfp_ui_model::{Insert, PageId, TextId, TextRef, UiModel, UiOption, Widget, WidgetId};
use tsfp_ui_renderer::AnchoredRect;

/// Extra-field value ids: `EXTRAS_BASE + player * 16 + field`.
pub const EXTRAS_BASE: u32 = 0x2211_1000;
/// Rebind action ids: `REBIND_BASE + player * 64 + intent index`; slot 63 resets the map.
pub const REBIND_BASE: u32 = 0x2211_2000;
const EXTRA_SLOTS: u32 = 16;
const REBIND_SLOTS: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExtraField {
    Smoothing,
    Acceleration,
    RawLook,
    ScopeMultiplier,
    Rumble,
    Device,
}

impl ExtraField {
    pub const ALL: [ExtraField; 6] = [
        ExtraField::Smoothing,
        ExtraField::Acceleration,
        ExtraField::RawLook,
        ExtraField::ScopeMultiplier,
        ExtraField::Rumble,
        ExtraField::Device,
    ];

    fn index(self) -> u32 {
        Self::ALL.iter().position(|f| *f == self).unwrap() as u32
    }

    pub fn value_id(self, player: u8) -> ValueId {
        ValueId(EXTRAS_BASE + u32::from(player) * EXTRA_SLOTS + self.index())
    }

    pub fn decode(id: ValueId) -> Option<(u8, ExtraField)> {
        let offset = id.0.checked_sub(EXTRAS_BASE)?;
        let player = u8::try_from(offset / EXTRA_SLOTS).ok()?;
        let field = *Self::ALL.get((offset % EXTRA_SLOTS) as usize)?;
        Some((player, field))
    }
}

pub fn rebind_action(player: u8, intent: Intent) -> ActionId {
    let index = INTENTS.iter().position(|(i, _)| *i == intent).unwrap() as u32;
    ActionId(REBIND_BASE + u32::from(player) * REBIND_SLOTS + index)
}

pub fn reset_bindings_action(player: u8) -> ActionId {
    ActionId(REBIND_BASE + u32::from(player) * REBIND_SLOTS + REBIND_SLOTS - 1)
}

/// What a rebind-range action id means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebindAction {
    Rebind(u8, Intent),
    Reset(u8),
}

pub fn decode_rebind(id: ActionId) -> Option<RebindAction> {
    let offset = id.0.checked_sub(REBIND_BASE)?;
    let player = u8::try_from(offset / REBIND_SLOTS).ok()?;
    let slot = offset % REBIND_SLOTS;
    if slot == REBIND_SLOTS - 1 {
        return Some(RebindAction::Reset(player));
    }
    INTENTS
        .get(slot as usize)
        .map(|(intent, _)| RebindAction::Rebind(player, *intent))
}

/// Outcome of feeding one device input to a pending capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureResult {
    NotCapturing,
    Cancelled,
    /// The input now drives `intent`; `stolen_from` lists intents of the same player that lost it.
    Bound {
        intent: Intent,
        stolen_from: Vec<Intent>,
    },
}

/// Per-player extras and action maps edited by the rows.
#[derive(Clone, Debug, PartialEq)]
pub struct InputExtras {
    pub players: Vec<PlayerExtras>,
    pub maps: Vec<ActionMap>,
    /// Rebind waiting for the next device input: (player, intent).
    pub capturing: Option<(u8, Intent)>,
    /// Set by every accepted edit, cleared by the persistence owner after saving.
    pub dirty: bool,
}

impl InputExtras {
    /// Defaults: `PlayerExtras::default()` and the owner's default keymap for every player.
    pub fn new(players: usize) -> Self {
        Self {
            players: vec![PlayerExtras::default(); players],
            maps: vec![ActionMap::default_mouse_keyboard(); players],
            capturing: None,
            dirty: false,
        }
    }

    pub fn read(&self, player: u8, field: ExtraField) -> UiValue {
        let Some(p) = self.players.get(usize::from(player)) else {
            return UiValue::None;
        };
        match field {
            ExtraField::Smoothing => UiValue::Int(i32::from(p.smoothing_frames)),
            ExtraField::Acceleration => UiValue::Float(p.acceleration),
            ExtraField::RawLook => UiValue::Bool(p.raw_look),
            ExtraField::ScopeMultiplier => UiValue::Float(p.scope_multiplier),
            ExtraField::Rumble => UiValue::Float(p.rumble),
            ExtraField::Device => p.device.index().map_or(UiValue::None, UiValue::Index),
        }
    }

    /// Floats clamp to the shared limits, non-finite values and wrong shapes are refused.
    pub fn write(&mut self, player: u8, field: ExtraField, value: UiValue) -> BindResult {
        let Some(p) = self.players.get_mut(usize::from(player)) else {
            return BindResult::Ignored;
        };
        let float = |v: f32, (low, high): (f32, f32)| v.is_finite().then(|| v.clamp(low, high));
        let accepted = match (field, value) {
            (ExtraField::Smoothing, UiValue::Int(n)) => {
                p.smoothing_frames = n.clamp(0, i32::from(SMOOTHING_FRAMES_MAX)) as u8;
                true
            }
            (ExtraField::Acceleration, UiValue::Float(v)) => {
                set(&mut p.acceleration, float(v, ACCELERATION_RANGE))
            }
            (ExtraField::RawLook, UiValue::Bool(v)) => set(&mut p.raw_look, Some(v)),
            (ExtraField::RawLook, UiValue::Index(i @ 0..=1)) => set(&mut p.raw_look, Some(i == 1)),
            (ExtraField::ScopeMultiplier, UiValue::Float(v)) => {
                set(&mut p.scope_multiplier, float(v, SCOPE_MULTIPLIER_RANGE))
            }
            (ExtraField::Rumble, UiValue::Float(v)) => set(&mut p.rumble, float(v, RUMBLE_RANGE)),
            (ExtraField::Device, UiValue::Index(i)) => {
                set(&mut p.device, DeviceAssignment::from_index(i))
            }
            _ => false,
        };
        if accepted {
            self.dirty = true;
            BindResult::Handled
        } else {
            BindResult::Denied
        }
    }

    /// Starts listening for the next device input for `intent`.
    pub fn begin_capture(&mut self, player: u8, intent: Intent) -> BindResult {
        if usize::from(player) >= self.maps.len() {
            return BindResult::Ignored;
        }
        self.capturing = Some((player, intent));
        BindResult::Handled
    }

    /// Feeds one device input to a pending capture. `key:Escape` cancels (it stays the menu back
    /// key). The input replaces every binding of the intent and is removed from other intents of
    /// the same player, so one key never drives two actions after a rebind.
    pub fn capture(&mut self, input: InputId) -> CaptureResult {
        let Some((player, intent)) = self.capturing.take() else {
            return CaptureResult::NotCapturing;
        };
        if input == InputId::Key("Escape".into()) {
            return CaptureResult::Cancelled;
        }
        let map = &mut self.maps[usize::from(player)];
        let mut stolen_from = Vec::new();
        map.bindings.retain(|(i, b)| {
            let clash = *i != intent && b.chord == [input.clone()];
            if clash && !stolen_from.contains(i) {
                stolen_from.push(*i);
            }
            !clash && *i != intent
        });
        map.bindings.push((
            intent,
            InputBinding {
                chord: vec![input],
                toggle: false,
            },
        ));
        self.dirty = true;
        CaptureResult::Bound {
            intent,
            stolen_from,
        }
    }

    pub fn reset_bindings(&mut self, player: u8) -> BindResult {
        match self.maps.get_mut(usize::from(player)) {
            Some(map) => {
                *map = ActionMap::default_mouse_keyboard();
                self.dirty = true;
                BindResult::Handled
            }
            None => BindResult::Ignored,
        }
    }

    pub fn reset(&mut self, player: u8) -> BindResult {
        match self.players.get_mut(usize::from(player)) {
            Some(p) => {
                *p = PlayerExtras::default();
                self.dirty = true;
                BindResult::Handled
            }
            None => BindResult::Ignored,
        }
    }

    /// Display text of an intent's bindings in the `ActionMap` file syntax (`key:KeyR, mouse:Left`),
    /// for the renderer to show beside the rebind row.
    pub fn binding_text(&self, player: u8, intent: Intent) -> String {
        let Some(map) = self.maps.get(usize::from(player)) else {
            return String::new();
        };
        map.bindings
            .iter()
            .filter(|(i, _)| *i == intent)
            .map(|(_, b)| binding_text(b))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn input_text(input: &InputId) -> String {
    match input {
        InputId::Key(name) => format!("key:{name}"),
        InputId::Mouse(name) => format!("mouse:{name}"),
        InputId::Pad(bits) => format!("pad:{bits:#06x}"),
    }
}

fn binding_text(binding: &InputBinding) -> String {
    let chord: Vec<String> = binding.chord.iter().map(input_text).collect();
    let prefix = if binding.toggle { "toggle:" } else { "" };
    format!("{prefix}{}", chord.join("+"))
}

fn set<T>(slot: &mut T, value: Option<T>) -> bool {
    value.map(|v| *slot = v).is_some()
}

/// Routes this module's ids to [`InputExtras`]; everything else goes to `inner`, so it stacks
/// over `InputSettingsBindings` and the game bindings.
pub struct InputExtrasBindings<'a> {
    pub extras: &'a mut InputExtras,
    pub inner: &'a mut dyn UiBindings,
}

impl UiBindings for InputExtrasBindings<'_> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        match decode_rebind(id) {
            Some(RebindAction::Rebind(player, intent)) => self.extras.begin_capture(player, intent),
            Some(RebindAction::Reset(player)) => {
                // Reset to defaults covers both the extras and the key map of that player.
                self.extras.reset(player);
                self.extras.reset_bindings(player)
            }
            None => self.inner.action(id, arg),
        }
    }
    fn value(&self, id: ValueId) -> UiValue {
        match ExtraField::decode(id) {
            Some((player, field)) => self.extras.read(player, field),
            None => self.inner.value(id),
        }
    }
    fn set_value(&mut self, id: ValueId, value: UiValue) -> BindResult {
        match ExtraField::decode(id) {
            Some((player, field)) => self.extras.write(player, field, value),
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

/// Caller-supplied presentation, same rules as `InputRowsLayout`: text ids come from the settings
/// owner (T2196), never guessed bank ids.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtrasRowsLayout {
    pub page: PageId,
    pub at: Insert,
    pub first_widget: WidgetId,
    pub labels: Vec<(ExtraField, TextId)>,
    pub off_on: [TextRef; 2],
    /// One text per `DeviceAssignment::ALL` entry.
    pub device_texts: Vec<TextRef>,
    pub first_rect: AnchoredRect,
    pub row_step: f32,
}

fn row_rect(first: AnchoredRect, n: usize, step: f32) -> AnchoredRect {
    let mut rect = first;
    rect.offset.y += n as f32 * step;
    rect
}

fn options(texts: &[TextRef]) -> Vec<UiOption> {
    texts
        .iter()
        .enumerate()
        .map(|(i, t)| UiOption {
            id: i as u32,
            text: *t,
            enabled: true,
        })
        .collect()
}

pub fn extras_rows(model: &UiModel, player: u8, layout: &ExtrasRowsLayout) -> Extension {
    let mut rows = Vec::new();
    let mut at = layout.at;
    for (n, (field, label)) in layout.labels.iter().enumerate() {
        let id = WidgetId(layout.first_widget.0 + n as u64);
        let kind = match field {
            ExtraField::RawLook => SettingKind::Cycler(options(&layout.off_on)),
            ExtraField::Device => SettingKind::Cycler(options(&layout.device_texts)),
            _ => SettingKind::Slider,
        };
        rows.push(SettingRow {
            page: layout.page,
            id,
            at,
            label: *label,
            rect: row_rect(layout.first_rect, n, layout.row_step),
            value: field.value_id(player),
            kind,
        });
        at = Insert::After(id);
    }
    let mut extension = settings_rows(model, &rows);
    extension.patch.name = format!("input-extras-p{player}");
    extension
}

#[derive(Clone, Debug, PartialEq)]
pub struct RebindRowsLayout {
    pub page: PageId,
    pub at: Insert,
    pub first_widget: WidgetId,
    /// Intents that get a row, in order, with their label text.
    pub labels: Vec<(Intent, TextId)>,
    /// Label of the trailing "Reset to defaults" row, if wanted.
    pub reset_label: Option<TextId>,
    pub first_rect: AnchoredRect,
    pub row_step: f32,
}

/// One button per intent (activating it starts a capture) plus an optional reset row.
pub fn rebind_rows(_model: &UiModel, player: u8, layout: &RebindRowsLayout) -> Extension {
    let mut extension = Extension::new(format!("input-rebind-p{player}"));
    let mut at = layout.at;
    let entries = layout
        .labels
        .iter()
        .map(|(intent, label)| (rebind_action(player, *intent), *label))
        .chain(
            layout
                .reset_label
                .map(|label| (reset_bindings_action(player), label)),
        );
    for (n, (action, label)) in entries.enumerate() {
        let id = WidgetId(layout.first_widget.0 + n as u64);
        let mut widget = Widget::new(id, WidgetKind::Button);
        widget.text = TextRef::Id(label);
        widget.rect = row_rect(layout.first_rect, n, layout.row_step);
        add_row(
            &mut extension,
            layout.page,
            at,
            widget,
            Binding::Action(action),
        );
        at = Insert::After(id);
    }
    extension
}
