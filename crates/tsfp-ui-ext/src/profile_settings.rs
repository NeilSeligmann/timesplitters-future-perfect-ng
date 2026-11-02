// SPDX-License-Identifier: GPL-3.0-or-later
//! Runtime profile option rows projected into the opt-in Rust UI model.
//! Original table values and text ids are supplied by the caller.
use crate::{Extension, SettingKind, SettingRow, settings_rows};
use tsfp_format_profile::{
    Profile,
    settings::{HostSettings, OptionRow, SettingsPages},
};
use tsfp_ui_input::{
    ActionArg, ActionId, BindResult, ListId, ListView, TextId as UiTextId, UiBindings, UiValue,
    ValueId,
};
use tsfp_ui_model::{Insert, PageId, TextId, TextRef, UiModel, UiOption, WidgetId};
use tsfp_ui_renderer::AnchoredRect;

/// Presentation supplied by the runtime table/text owners. The ordinal selects an OptionRow.
#[derive(Clone, Debug)]
pub struct ProfileRowLayout {
    pub ordinal: usize,
    pub page: PageId,
    pub at: Insert,
    pub widget: WidgetId,
    pub rect: AnchoredRect,
    /// Text ids parallel to the runtime table choices.
    pub choice_text_ids: Vec<TextId>,
}

/// Stable only for the lifetime of a loaded SettingsPages table.
pub const VALUE_BASE: u32 = 0x2190_0000;

pub fn profile_rows(
    model: &UiModel,
    pages: &SettingsPages,
    layouts: &[ProfileRowLayout],
) -> Result<Extension, &'static str> {
    let mut rows = Vec::with_capacity(layouts.len());
    for layout in layouts {
        let row = pages
            .rows
            .get(layout.ordinal)
            .ok_or("profile row ordinal")?;
        if layout.choice_text_ids.len() != row.choices.len() {
            return Err("profile choice texts");
        }
        let ordinal = u32::try_from(layout.ordinal).map_err(|_| "profile row ordinal")?;
        let value = VALUE_BASE.checked_add(ordinal).ok_or("profile value id")?;
        rows.push(SettingRow {
            page: layout.page,
            id: layout.widget,
            at: layout.at,
            label: TextId(row.label_id),
            rect: layout.rect,
            value: ValueId(value),
            kind: SettingKind::Cycler(
                layout
                    .choice_text_ids
                    .iter()
                    .enumerate()
                    .map(|(i, text)| UiOption {
                        id: i as u32,
                        text: TextRef::Id(*text),
                        enabled: true,
                    })
                    .collect(),
            ),
        });
    }
    Ok(settings_rows(model, &rows))
}

/// Bridges model option indices to typed profile values; unknown ids delegate unchanged.
pub struct ProfileSettingsBindings<'a, H: HostSettings> {
    pub pages: &'a SettingsPages,
    pub profile: &'a mut Profile,
    pub host: &'a mut H,
    pub inner: &'a mut dyn UiBindings,
}
impl<H: HostSettings> ProfileSettingsBindings<'_, H> {
    fn row(&self, id: ValueId) -> Option<&OptionRow> {
        let index = id.0.checked_sub(VALUE_BASE)? as usize;
        self.pages.rows.get(index)
    }
    fn current(&self, row: &OptionRow) -> Option<u32> {
        use tsfp_format_profile::settings::Binding;
        match row.binding {
            Binding::ProfileWord { offset } | Binding::ProfileBits { offset, .. } => {
                let bytes: [u8; 4] = self
                    .profile
                    .payload
                    .get(offset..offset.checked_add(4)?)?
                    .try_into()
                    .ok()?;
                let word = u32::from_le_bytes(bytes);
                Some(match row.binding {
                    Binding::ProfileBits { mask, .. } => word & mask,
                    _ => word,
                })
            }
            Binding::Host { .. } => None,
        }
    }
}
impl<H: HostSettings> UiBindings for ProfileSettingsBindings<'_, H> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        self.inner.action(id, arg)
    }
    fn value(&self, id: ValueId) -> UiValue {
        let Some(row) = self.row(id) else {
            return self.inner.value(id);
        };
        self.current(row)
            .and_then(|v| row.choices.iter().position(|c| *c == v))
            .map_or(UiValue::None, |i| UiValue::Index(i as u32))
    }
    fn set_value(&mut self, id: ValueId, value: UiValue) -> BindResult {
        let Some(row) = self.row(id).cloned() else {
            return self.inner.set_value(id, value);
        };
        let UiValue::Index(index) = value else {
            return BindResult::Denied;
        };
        let Some(choice) = row.choices.get(index as usize) else {
            return BindResult::Denied;
        };
        match self
            .pages
            .apply(self.profile, row.page, row.row, *choice, self.host)
        {
            Ok(()) => BindResult::Handled,
            Err(_) => BindResult::Denied,
        }
    }
    fn list(&self, id: ListId) -> ListView {
        self.inner.list(id)
    }
    fn text(&self, id: UiTextId) -> &str {
        self.inner.text(id)
    }
}
