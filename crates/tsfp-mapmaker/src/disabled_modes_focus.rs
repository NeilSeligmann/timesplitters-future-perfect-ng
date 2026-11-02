//! Focus query for the Rust-owned Disable Modes page entry.

use crate::ui::{EditorSurface, MapMakerUiState, MapSettingField, MapSettingsEntry};

pub trait MapMakerDisabledModesFocus {
    fn disable_modes_focused(&self) -> bool;
}

impl MapMakerDisabledModesFocus for MapMakerUiState {
    fn disable_modes_focused(&self) -> bool {
        self.editor_surface() == EditorSurface::MapSettings
            && self.focused_map_settings_entry()
                == Some(MapSettingsEntry::Field(MapSettingField::DisableModes))
    }
}
