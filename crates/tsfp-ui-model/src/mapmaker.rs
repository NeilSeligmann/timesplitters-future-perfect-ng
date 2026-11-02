//! Typed, host-free Map Maker page construction. The caller supplies original table text IDs
//! and measured layout when available; this module contains no disc text or guest memory.
use crate::model::{
    BindingId, HandlerRef, Page, PageId, TextId, TextRef, UiModel, UiOption, Widget, WidgetId,
    WidgetKind,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EditorPage {
    Edit,
    MapSettings,
    EditorSettings,
    ToolPalette,
    Confirm,
    NameEntry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorAction {
    Preview,
    Undo,
    Redo,
    MapSettings,
    EditorSettings,
    SelectTool,
    Confirm,
    Cancel,
    CommitName,
}

impl EditorAction {
    fn binding(self) -> BindingId {
        BindingId(format!("mapmaker.action.{self:?}"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorPageIds {
    pub edit: PageId,
    pub map_settings: PageId,
    pub editor_settings: PageId,
    pub tool_palette: PageId,
    pub confirm: PageId,
    pub name_entry: PageId,
}

impl EditorPageIds {
    pub fn get(self, page: EditorPage) -> PageId {
        match page {
            EditorPage::Edit => self.edit,
            EditorPage::MapSettings => self.map_settings,
            EditorPage::EditorSettings => self.editor_settings,
            EditorPage::ToolPalette => self.tool_palette,
            EditorPage::Confirm => self.confirm,
            EditorPage::NameEntry => self.name_entry,
        }
    }
}

/// All labels are indexes into the caller's original disc string bank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorText {
    pub preview: TextId,
    pub undo: TextId,
    pub redo: TextId,
    pub map_settings: TextId,
    pub editor_settings: TextId,
    pub theme: TextId,
    pub weapon_set: TextId,
    pub palette: TextId,
    pub mode: TextId,
    pub beginner: TextId,
    pub advanced: TextId,
    pub confirm: TextId,
    pub cancel: TextId,
    pub name: TextId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteTool {
    pub id: u32,
    pub label: TextId,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub selected_theme: usize,
    pub selected_weapon_set: usize,
    pub selected_palette: usize,
    pub advanced: bool,
    pub tools: Vec<PaletteTool>,
    pub selected_tool: Option<u32>,
    pub name: String,
    pub name_limit: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    Open(EditorPage),
    Preview,
    Undo,
    Redo,
    SetTheme(usize),
    SetWeaponSet(usize),
    SetPalette(usize),
    SetAdvanced(bool),
    SelectTool(u32),
    SetName(String),
    CommitName,
    Confirm,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorError {
    Disabled,
    UnknownTool,
    InvalidChoice,
    NameTooLong,
    UnknownWidget,
}

/// A UI binding request. The host supplies the value after its own input handling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorInput {
    Activate,
    Choose(usize),
    Name(String),
}

/// Resolve a page-local widget into a typed game event. Widget IDs are deliberately
/// page-local; the host must pass the active page with every request.
pub fn event_for_widget(
    page: EditorPage,
    widget: u64,
    input: EditorInput,
    state: &EditorState,
    choices: &EditorChoices,
) -> Result<EditorEvent, EditorError> {
    use EditorEvent as E;
    use EditorInput as I;
    let event = match (page, widget, input) {
        (EditorPage::Edit, 1, I::Activate) => E::Preview,
        (EditorPage::Edit, 2, I::Activate) => E::Undo,
        (EditorPage::Edit, 3, I::Activate) => E::Redo,
        (EditorPage::Edit, 4, I::Activate) => E::Open(EditorPage::MapSettings),
        (EditorPage::Edit, 5, I::Activate) => E::Open(EditorPage::EditorSettings),
        (EditorPage::MapSettings, 1, I::Choose(n)) if n < choices.themes.len() => E::SetTheme(n),
        (EditorPage::MapSettings, 2, I::Choose(n)) if n < choices.weapon_sets.len() => {
            E::SetWeaponSet(n)
        }
        (EditorPage::MapSettings, 3, I::Choose(n)) if n < choices.palettes.len() => {
            E::SetPalette(n)
        }
        (EditorPage::EditorSettings, 1, I::Choose(0)) => E::SetAdvanced(false),
        (EditorPage::EditorSettings, 1, I::Choose(1)) => E::SetAdvanced(true),
        (EditorPage::ToolPalette, n, I::Activate) if n > 0 => {
            let tool = state
                .tools
                .get(n as usize - 1)
                .ok_or(EditorError::UnknownWidget)?;
            E::SelectTool(tool.id)
        }
        (EditorPage::Confirm, 1, I::Activate) => E::Confirm,
        (EditorPage::Confirm, 2, I::Activate) => E::Cancel,
        (EditorPage::NameEntry, 1, I::Name(name)) => E::SetName(name),
        (EditorPage::NameEntry, 2, I::Activate) => E::CommitName,
        (EditorPage::NameEntry, 3, I::Activate) => E::Cancel,
        (EditorPage::MapSettings, 1..=3, I::Choose(_))
        | (EditorPage::EditorSettings, 1, I::Choose(_)) => return Err(EditorError::InvalidChoice),
        _ => return Err(EditorError::UnknownWidget),
    };
    let mut checked = state.clone();
    checked.apply(event.clone(), choices)?;
    Ok(event)
}

impl EditorState {
    /// Pure state transition. External game edits are returned to the host as events.
    pub fn apply(
        &mut self,
        event: EditorEvent,
        choices: &EditorChoices,
    ) -> Result<EditorEvent, EditorError> {
        match &event {
            EditorEvent::Undo if !self.can_undo => return Err(EditorError::Disabled),
            EditorEvent::Redo if !self.can_redo => return Err(EditorError::Disabled),
            EditorEvent::SetTheme(n) if *n >= choices.themes.len() => {
                return Err(EditorError::InvalidChoice);
            }
            EditorEvent::SetWeaponSet(n) if *n >= choices.weapon_sets.len() => {
                return Err(EditorError::InvalidChoice);
            }
            EditorEvent::SetPalette(n) if *n >= choices.palettes.len() => {
                return Err(EditorError::InvalidChoice);
            }
            EditorEvent::SelectTool(id) if !self.tools.iter().any(|t| t.id == *id && t.enabled) => {
                return Err(EditorError::UnknownTool);
            }
            EditorEvent::SetName(name) if name.chars().count() > self.name_limit => {
                return Err(EditorError::NameTooLong);
            }
            _ => {}
        }
        match &event {
            EditorEvent::SetTheme(n) => self.selected_theme = *n,
            EditorEvent::SetWeaponSet(n) => self.selected_weapon_set = *n,
            EditorEvent::SetPalette(n) => self.selected_palette = *n,
            EditorEvent::SetAdvanced(value) => self.advanced = *value,
            EditorEvent::SelectTool(id) => self.selected_tool = Some(*id),
            EditorEvent::SetName(name) => self.name = name.clone(),
            _ => {}
        }
        Ok(event)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorChoices {
    pub themes: Vec<TextId>,
    pub weapon_sets: Vec<TextId>,
    pub palettes: Vec<TextId>,
}

fn row(id: u64, kind: WidgetKind, text: TextRef, action: Option<EditorAction>) -> Widget {
    let mut widget = Widget::new(WidgetId(id), kind);
    widget.text = text;
    if let Some(action) = action {
        widget.bound = Some(action.binding());
        widget.activate = Some(HandlerRef::Named(format!("mapmaker.{action:?}")));
    }
    widget
}

fn choice_row(id: u64, label: TextId, values: &[TextId], selected: usize, binding: &str) -> Widget {
    let mut widget = row(id, WidgetKind::Cycler, TextRef::Id(label), None);
    widget.bound = Some(BindingId(binding.to_owned()));
    widget.options = values
        .iter()
        .enumerate()
        .map(|(index, text)| UiOption {
            id: index as u32,
            text: TextRef::Id(*text),
            enabled: true,
        })
        .collect();
    widget.selected = selected.min(widget.options.len().saturating_sub(1));
    widget.enabled = !widget.options.is_empty();
    widget
}

/// Builds host-free page structure. Original geometry, fonts, callback ABI and text IDs must be
/// supplied by the capture/binding layer before claiming original-preset visual fidelity.
pub fn build_pages(
    ids: EditorPageIds,
    text: EditorText,
    state: &EditorState,
    choices: &EditorChoices,
) -> UiModel {
    let mut model = UiModel::new();
    model.root = Some(ids.edit);
    let mut edit = Page::new(ids.edit, 1);
    edit.widgets = vec![
        row(
            1,
            WidgetKind::Button,
            TextRef::Id(text.preview),
            Some(EditorAction::Preview),
        ),
        row(
            2,
            WidgetKind::Button,
            TextRef::Id(text.undo),
            Some(EditorAction::Undo),
        ),
        row(
            3,
            WidgetKind::Button,
            TextRef::Id(text.redo),
            Some(EditorAction::Redo),
        ),
        row(
            4,
            WidgetKind::Button,
            TextRef::Id(text.map_settings),
            Some(EditorAction::MapSettings),
        ),
        row(
            5,
            WidgetKind::Button,
            TextRef::Id(text.editor_settings),
            Some(EditorAction::EditorSettings),
        ),
    ];
    edit.widgets[1].enabled = state.can_undo;
    edit.widgets[2].enabled = state.can_redo;
    model.pages.insert(edit.id, edit);

    let mut settings = Page::new(ids.map_settings, 2);
    settings.widgets = vec![
        choice_row(
            1,
            text.theme,
            &choices.themes,
            state.selected_theme,
            "mapmaker.theme",
        ),
        choice_row(
            2,
            text.weapon_set,
            &choices.weapon_sets,
            state.selected_weapon_set,
            "mapmaker.weapon_set",
        ),
        choice_row(
            3,
            text.palette,
            &choices.palettes,
            state.selected_palette,
            "mapmaker.palette",
        ),
    ];
    model.pages.insert(settings.id, settings);

    let mut editor_settings = Page::new(ids.editor_settings, 2);
    editor_settings.widgets = vec![choice_row(
        1,
        text.mode,
        &[text.beginner, text.advanced],
        usize::from(state.advanced),
        "mapmaker.advanced",
    )];
    model.pages.insert(editor_settings.id, editor_settings);

    let mut palette = Page::new(ids.tool_palette, 1);
    palette.widgets = state
        .tools
        .iter()
        .enumerate()
        .map(|(index, tool)| {
            let mut widget = row(
                index as u64 + 1,
                WidgetKind::Button,
                TextRef::Id(tool.label),
                Some(EditorAction::SelectTool),
            );
            widget.bound = Some(BindingId(format!("mapmaker.tool.{}", tool.id)));
            widget.enabled = tool.enabled;
            widget
        })
        .collect();
    model.pages.insert(palette.id, palette);

    let mut confirm = Page::new(ids.confirm, 1);
    confirm.widgets = vec![
        row(
            1,
            WidgetKind::Button,
            TextRef::Id(text.confirm),
            Some(EditorAction::Confirm),
        ),
        row(
            2,
            WidgetKind::Button,
            TextRef::Id(text.cancel),
            Some(EditorAction::Cancel),
        ),
    ];
    model.pages.insert(confirm.id, confirm);

    let mut name = Page::new(ids.name_entry, 1);
    let mut field = row(1, WidgetKind::TextField, TextRef::Id(text.name), None);
    field.bound = Some(BindingId("mapmaker.name".to_owned()));
    name.widgets = vec![
        field,
        row(
            2,
            WidgetKind::Button,
            TextRef::Id(text.confirm),
            Some(EditorAction::CommitName),
        ),
        row(
            3,
            WidgetKind::Button,
            TextRef::Id(text.cancel),
            Some(EditorAction::Cancel),
        ),
    ];
    model.pages.insert(name.id, name);
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text() -> EditorText {
        EditorText {
            preview: TextId(1),
            undo: TextId(2),
            redo: TextId(3),
            map_settings: TextId(4),
            editor_settings: TextId(5),
            theme: TextId(6),
            weapon_set: TextId(7),
            palette: TextId(8),
            mode: TextId(9),
            beginner: TextId(10),
            advanced: TextId(11),
            confirm: TextId(12),
            cancel: TextId(13),
            name: TextId(14),
        }
    }
    fn state() -> EditorState {
        EditorState {
            can_undo: false,
            can_redo: true,
            selected_theme: 1,
            selected_weapon_set: 0,
            selected_palette: 0,
            advanced: false,
            tools: vec![
                PaletteTool {
                    id: 7,
                    label: TextId(15),
                    enabled: true,
                },
                PaletteTool {
                    id: 9,
                    label: TextId(16),
                    enabled: false,
                },
            ],
            selected_tool: None,
            name: String::new(),
            name_limit: 8,
        }
    }
    fn ids() -> EditorPageIds {
        EditorPageIds {
            edit: PageId(1),
            map_settings: PageId(2),
            editor_settings: PageId(3),
            tool_palette: PageId(4),
            confirm: PageId(5),
            name_entry: PageId(6),
        }
    }
    #[test]
    fn builds_six_pages_from_supplied_ids() {
        let choices = EditorChoices {
            themes: vec![TextId(20), TextId(21)],
            weapon_sets: vec![TextId(22)],
            palettes: vec![TextId(23)],
        };
        let model = build_pages(ids(), text(), &state(), &choices);
        assert_eq!(model.pages.len(), 6);
        assert!(!model.page(ids().edit).unwrap().widgets[1].enabled);
        assert_eq!(
            model.page(ids().map_settings).unwrap().widgets[0].selected,
            1
        );
        assert!(!model.page(ids().tool_palette).unwrap().widgets[1].enabled);
        assert_eq!(
            model.page(ids().name_entry).unwrap().widgets[0].kind,
            WidgetKind::TextField
        );
    }
    #[test]
    fn widget_routes_are_page_local_and_validate_state() {
        let choices = EditorChoices {
            themes: vec![TextId(20), TextId(21)],
            ..Default::default()
        };
        let state = state();
        assert_eq!(
            event_for_widget(EditorPage::Edit, 4, EditorInput::Activate, &state, &choices),
            Ok(EditorEvent::Open(EditorPage::MapSettings))
        );
        assert_eq!(
            event_for_widget(
                EditorPage::ToolPalette,
                1,
                EditorInput::Activate,
                &state,
                &choices
            ),
            Ok(EditorEvent::SelectTool(7))
        );
        assert_eq!(
            event_for_widget(
                EditorPage::ToolPalette,
                2,
                EditorInput::Activate,
                &state,
                &choices
            ),
            Err(EditorError::UnknownTool)
        );
        assert_eq!(
            event_for_widget(EditorPage::Edit, 2, EditorInput::Activate, &state, &choices),
            Err(EditorError::Disabled)
        );
        assert_eq!(
            event_for_widget(
                EditorPage::MapSettings,
                1,
                EditorInput::Choose(2),
                &state,
                &choices
            ),
            Err(EditorError::InvalidChoice)
        );
        assert_eq!(
            event_for_widget(
                EditorPage::NameEntry,
                1,
                EditorInput::Name("123456789".into()),
                &state,
                &choices
            ),
            Err(EditorError::NameTooLong)
        );
    }
    #[test]
    fn transitions_reject_invalid_changes_without_mutation() {
        let choices = EditorChoices {
            themes: vec![TextId(20)],
            ..Default::default()
        };
        let mut state = state();
        let original = state.clone();
        assert_eq!(
            state.apply(EditorEvent::Undo, &choices),
            Err(EditorError::Disabled)
        );
        assert_eq!(
            state.apply(EditorEvent::SelectTool(9), &choices),
            Err(EditorError::UnknownTool)
        );
        assert_eq!(
            state.apply(EditorEvent::SetName("123456789".into()), &choices),
            Err(EditorError::NameTooLong)
        );
        assert_eq!(
            state.apply(EditorEvent::SetTheme(2), &choices),
            Err(EditorError::InvalidChoice)
        );
        assert_eq!(state, original);
        assert_eq!(
            state.apply(EditorEvent::SelectTool(7), &choices),
            Ok(EditorEvent::SelectTool(7))
        );
        assert_eq!(state.selected_tool, Some(7));
    }
}
