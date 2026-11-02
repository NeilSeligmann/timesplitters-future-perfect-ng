// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in advanced settings. Presentation and device enumerations belong to the caller.
use crate::{Binding, Extension, SettingKind, SettingRow, add_row, settings_rows};
use std::{collections::BTreeMap, path::PathBuf};
use tsfp_settings::{Group, Kind, SCHEMA, Settings, SettingsSink, Status};
use tsfp_ui_input::{
    ActionArg, ActionId, BindResult, ListId, ListView, TextId as InputTextId, UiBindings, UiValue,
    ValueId,
};
use tsfp_ui_model::{
    Insert, Page, PageId, PatchOp, TextId, TextRef, UiModel, UiOption, Widget, WidgetId, WidgetKind,
};
use tsfp_ui_renderer::AnchoredRect;

pub const BASE: u32 = 0x4323_0000;
pub const APPLY: ActionId = ActionId(BASE + 0x100);
pub const CANCEL: ActionId = ActionId(BASE + 0x101);
pub const OPEN_ADVANCED: ActionId = ActionId(BASE + 0x102);
pub const OPEN_CONTROLS: ActionId = ActionId(BASE + 0x103);
pub const OPEN_GROUP_BASE: u32 = BASE + 0x110;
pub fn value_id(index: usize) -> ValueId {
    ValueId(BASE + index as u32)
}
pub const GROUPS: [Group; 5] = [
    Group::Display,
    Group::Graphics,
    Group::Audio,
    Group::Gameplay,
    Group::Diagnostics,
];

/// Each option's stable value and caller-owned text id. List options include enumerated devices,
/// resolutions and any custom entries supplied by the host.
#[derive(Clone, Debug)]
pub struct Choice {
    pub value: String,
    pub text: TextId,
}
#[derive(Clone, Debug)]
pub struct Layout {
    pub frontend_options: PageId,
    pub pause_options: PageId,
    pub root: PageId,
    pub pages: [PageId; 5],
    pub controls_page: PageId,
    pub first_widget: WidgetId,
    pub first_rect: AnchoredRect,
    pub row_step: f32,
    pub advanced_label: TextId,
    pub controls_label: TextId,
    pub group_labels: [TextId; 5],
    pub apply_label: TextId,
    pub cancel_label: TextId,
    pub labels: BTreeMap<&'static str, TextId>,
    /// Translated 'takes effect when <system> is ready' labels, keyed by system.
    pub needs_labels: BTreeMap<&'static str, TextId>,
    pub choices: BTreeMap<&'static str, Vec<Choice>>,
}

/// Recomputed from staged state both on edit and when reconstructing pages.
pub fn visible(settings: &Settings, id: &str) -> bool {
    let is = |key: &str, value: &str| settings.get(key).and_then(|v| v.as_str()) == Some(value);
    match id {
        "msaa_samples" => is("anti_aliasing", "msaa"),
        "render_scale" | "sharpening" => !is("upscaling", "native"),
        "anisotropy" => is("texture_filter", "anisotropic"),
        "shadow_resolution" | "shadow_distance" => !is("shadows", "off"),
        "fps_game_custom" => is("fps_game", "custom"),
        "fps_menus_custom" => is("fps_menus", "custom"),
        "resolution_width" | "resolution_height" => is("resolution", "custom"),
        _ => true,
    }
}

/// Build the root, group pages and entry rows. Rebuild after accepted staged edits so dependent
/// rows and focus can be reconstructed by the UI model's focus graph.
pub fn generate(
    model: &UiModel,
    layout: &Layout,
    staged: &Settings,
) -> Result<Extension, &'static str> {
    let mut ext = Extension::new("advanced-settings");
    for page in std::iter::once(layout.root).chain(layout.pages) {
        ext.patch.ops.push(PatchOp::InsertPage {
            page: Page::new(page, 1),
            edge: None,
        });
    }
    let button = |ext: &mut Extension, page, id, label, action, row: usize| {
        let mut w = Widget::new(id, WidgetKind::Button);
        w.text = TextRef::Id(label);
        w.rect = layout.first_rect;
        w.rect.offset.y += row as f32 * layout.row_step;
        add_row(ext, page, Insert::End, w, Binding::Action(action));
    };
    for parent in [layout.frontend_options, layout.pause_options] {
        button(
            &mut ext,
            parent,
            layout.first_widget,
            layout.advanced_label,
            OPEN_ADVANCED,
            0,
        );
    }
    for (n, label) in layout.group_labels.iter().enumerate() {
        button(
            &mut ext,
            layout.root,
            WidgetId(layout.first_widget.0 + 1 + n as u64),
            *label,
            ActionId(OPEN_GROUP_BASE + n as u32),
            n,
        );
    }
    button(
        &mut ext,
        layout.root,
        WidgetId(layout.first_widget.0 + 10),
        layout.controls_label,
        OPEN_CONTROLS,
        5,
    );
    for (n, group) in GROUPS.iter().enumerate() {
        let page = layout.pages[n];
        let mut row = 0usize;
        for (index, s) in SCHEMA
            .iter()
            .enumerate()
            .filter(|(_, s)| s.group == *group && visible(staged, s.id))
        {
            let label = *layout.labels.get(s.id).ok_or("missing setting label")?;
            let kind = match s.kind {
                Kind::Range { .. } => SettingKind::Slider,
                _ => {
                    let choices = layout.choices.get(s.id).ok_or("missing setting choices")?;
                    let valid = |v: &str| match s.kind {
                        Kind::Bool => v == "false" || v == "true",
                        Kind::Enum(values) => values.contains(&v),
                        Kind::List => !v.is_empty(),
                        _ => false,
                    };
                    let unique = choices
                        .iter()
                        .enumerate()
                        .all(|(i, c)| !choices[..i].iter().any(|p| p.value == c.value));
                    let complete = match s.kind {
                        Kind::Bool => ["false", "true"]
                            .iter()
                            .all(|v| choices.iter().any(|c| c.value == *v)),
                        Kind::Enum(values) => {
                            values.iter().all(|v| choices.iter().any(|c| c.value == *v))
                        }
                        Kind::List if s.id == "resolution" => {
                            choices.iter().any(|c| c.value == "custom")
                        }
                        _ => true,
                    };
                    if choices.is_empty()
                        || !unique
                        || !complete
                        || choices.iter().any(|c| !valid(&c.value))
                    {
                        return Err("invalid setting choices");
                    }
                    SettingKind::Cycler(
                        choices
                            .iter()
                            .enumerate()
                            .map(|(i, c)| UiOption {
                                id: i as u32,
                                text: TextRef::Id(c.text),
                                enabled: true,
                            })
                            .collect(),
                    )
                }
            };
            let id = WidgetId(layout.first_widget.0 + 0x100 + index as u64 * 2);
            let mut rect = layout.first_rect;
            rect.offset.y += row as f32 * layout.row_step;
            let rows = [SettingRow {
                page,
                id,
                at: Insert::End,
                label,
                rect,
                value: value_id(index),
                kind,
            }];
            let generated = settings_rows(model, &rows);
            ext.patch.ops.extend(generated.patch.ops);
            ext.bindings.extend(generated.bindings);
            row += 1;
            if let Status::Placeholder(needs) = s.status {
                let mut note = Widget::new(WidgetId(id.0 + 1), WidgetKind::Label);
                note.text = TextRef::Id(
                    *layout
                        .needs_labels
                        .get(needs)
                        .ok_or("missing placeholder note")?,
                );
                note.rect = layout.first_rect;
                note.rect.offset.y += row as f32 * layout.row_step;
                note.navigable = false;
                ext.patch.ops.push(PatchOp::AddWidget {
                    page: tsfp_ui_model::PageSelector::Id(page),
                    at: Insert::End,
                    widget: note,
                    unless_role: None,
                });
                row += 1;
            }
        }
        button(
            &mut ext,
            page,
            WidgetId(layout.first_widget.0 + 0x1000),
            layout.apply_label,
            APPLY,
            row,
        );
        button(
            &mut ext,
            page,
            WidgetId(layout.first_widget.0 + 0x1001),
            layout.cancel_label,
            CANCEL,
            row + 1,
        );
    }
    Ok(ext)
}

/// Explicit host navigation requests, consumed after dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation {
    Open(PageId),
    Close,
}

/// Staged edits never reach the sink. Apply persists atomically before publication and notification;
/// on failure the committed settings and sink remain untouched. Cancel restores the committed copy.
pub struct AdvancedBindings<'a, S: SettingsSink> {
    pub committed: &'a mut Settings,
    pub staged: Settings,
    pub sink: &'a mut S,
    pub path: PathBuf,
    pub layout: &'a Layout,
    pub inner: &'a mut dyn UiBindings,
    pub navigation: Option<Navigation>,
    pub rebuild: bool,
    pub last_error: Option<String>,
}
impl<S: SettingsSink> AdvancedBindings<'_, S> {
    fn index(id: ValueId) -> Option<usize> {
        let n = id.0.checked_sub(BASE)? as usize;
        (n < SCHEMA.len()).then_some(n)
    }
}
impl<S: SettingsSink> UiBindings for AdvancedBindings<'_, S> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        if id == APPLY {
            if let Err(e) = self.staged.save(&self.path) {
                self.last_error = Some(e.to_string());
                return BindResult::Denied;
            }
            let previous = std::mem::replace(self.committed, self.staged.clone());
            self.committed.notify_changes(&previous, self.sink);
            self.last_error = None;
            self.navigation = Some(Navigation::Close);
            BindResult::Handled
        } else if id == CANCEL {
            self.staged = self.committed.clone();
            self.rebuild = true;
            self.navigation = Some(Navigation::Close);
            BindResult::Handled
        } else if id == OPEN_ADVANCED {
            self.navigation = Some(Navigation::Open(self.layout.root));
            BindResult::Handled
        } else if id == OPEN_CONTROLS {
            self.navigation = Some(Navigation::Open(self.layout.controls_page));
            BindResult::Handled
        } else if let Some(n) =
            id.0.checked_sub(OPEN_GROUP_BASE)
                .filter(|n| (*n as usize) < GROUPS.len())
        {
            self.navigation = Some(Navigation::Open(self.layout.pages[n as usize]));
            BindResult::Handled
        } else {
            self.inner.action(id, arg)
        }
    }
    fn value(&self, id: ValueId) -> UiValue {
        let Some(i) = Self::index(id) else {
            return self.inner.value(id);
        };
        let s = &SCHEMA[i];
        let Some(v) = self.staged.get(s.id) else {
            return UiValue::None;
        };
        match s.kind {
            Kind::Range { .. } => v
                .as_f64()
                .map_or(UiValue::None, |v| UiValue::Float(v as f32)),
            _ => {
                let target = match s.kind {
                    Kind::Bool => v.as_bool().map(|v| if v { "true" } else { "false" }),
                    _ => v.as_str(),
                };
                self.layout
                    .choices
                    .get(s.id)
                    .and_then(|c| c.iter().position(|c| Some(c.value.as_str()) == target))
                    .map_or(UiValue::None, |i| UiValue::Index(i as u32))
            }
        }
    }
    fn set_value(&mut self, id: ValueId, value: UiValue) -> BindResult {
        let Some(i) = Self::index(id) else {
            return self.inner.set_value(id, value);
        };
        let s = &SCHEMA[i];
        if !visible(&self.staged, s.id) {
            return BindResult::Denied;
        }
        let value = match (s.kind, value) {
            (Kind::Range { .. }, UiValue::Float(v)) if v.is_finite() => serde_json::json!(v),
            (kind, UiValue::Index(n)) => {
                let Some(c) = self
                    .layout
                    .choices
                    .get(s.id)
                    .and_then(|c| c.get(n as usize))
                else {
                    return BindResult::Denied;
                };
                if kind == Kind::Bool {
                    serde_json::json!(c.value == "true")
                } else {
                    serde_json::json!(c.value)
                }
            }
            _ => return BindResult::Denied,
        };
        if self.staged.set(s.id, value) {
            self.rebuild = true;
            BindResult::Handled
        } else {
            BindResult::Denied
        }
    }
    fn list(&self, id: ListId) -> ListView {
        self.inner.list(id)
    }
    fn text(&self, id: InputTextId) -> &str {
        self.inner.text(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExtensionPreset;
    use tsfp_ui_input::RecordingBindings;
    use tsfp_ui_renderer::{AnchorSpace, Edge, Vec2};

    #[test]
    fn extended_model_stages_cancels_and_applies() {
        let mut base = UiModel::new();
        for id in [PageId(1), PageId(2)] {
            base.pages.insert(id, Page::new(id, 1));
        }
        let mut labels = BTreeMap::new();
        let mut choices = BTreeMap::new();
        for s in SCHEMA {
            labels.insert(s.id, TextId(10));
            let values: Vec<&str> = match s.kind {
                Kind::Bool => vec!["false", "true"],
                Kind::Enum(v) => v.to_vec(),
                Kind::List if s.id == "resolution" => vec![s.default, "custom"],
                Kind::List => vec![s.default],
                Kind::Range { .. } => continue,
            };
            choices.insert(
                s.id,
                values
                    .into_iter()
                    .map(|value| Choice {
                        value: value.into(),
                        text: TextId(11),
                    })
                    .collect(),
            );
        }
        let layout = Layout {
            frontend_options: PageId(1),
            pause_options: PageId(2),
            root: PageId(3),
            pages: [PageId(4), PageId(5), PageId(6), PageId(7), PageId(8)],
            controls_page: PageId(9),
            first_widget: WidgetId::patch(1),
            first_rect: AnchoredRect {
                space: AnchorSpace::SafeArea4x3,
                horizontal: Edge::Start,
                vertical: Edge::Start,
                offset: Vec2::new(0.0, 0.0),
                size: Vec2::new(100.0, 20.0),
            },
            row_step: 20.0,
            advanced_label: TextId(1),
            controls_label: TextId(2),
            group_labels: [TextId(3); 5],
            apply_label: TextId(4),
            cancel_label: TextId(5),
            labels,
            needs_labels: ["host", "video", "renderer", "audio"]
                .into_iter()
                .map(|n| (n, TextId(12)))
                .collect(),
            choices,
        };
        let mut committed = Settings::default();
        let extension = generate(&base, &layout, &committed).unwrap();
        let model = ExtensionPreset {
            name: "advanced".into(),
            layers: vec![extension],
        }
        .apply(&base)
        .unwrap();
        assert_eq!(base.pages.len(), 2);
        struct Sink(usize);
        impl SettingsSink for Sink {
            fn changed(&mut self, _: &tsfp_settings::Setting, _: &serde_json::Value) {
                self.0 += 1;
            }
        }
        let mut sink = Sink(0);
        let mut inner = RecordingBindings::default();
        let path =
            std::env::temp_dir().join(format!("tsfp-settings-ui-test-{}.json", std::process::id()));
        let staged = committed.clone();
        let mut b = AdvancedBindings {
            committed: &mut committed,
            staged,
            sink: &mut sink,
            path: path.clone(),
            layout: &layout,
            inner: &mut inner,
            navigation: None,
            rebuild: false,
            last_error: None,
        };
        let index = SCHEMA.iter().position(|s| s.id == "ui_scale").unwrap();
        let target = (
            PageId(4),
            WidgetId(layout.first_widget.0 + 0x100 + index as u64 * 2),
        );
        assert_eq!(
            model.set_value(target, UiValue::Float(1.5), &mut b),
            BindResult::Handled
        );
        assert_eq!(
            model.activate(
                (PageId(4), WidgetId(layout.first_widget.0 + 0x1001)),
                &mut b
            ),
            BindResult::Handled
        );
        assert_eq!(b.staged.get("ui_scale"), Some(&serde_json::json!(1.0)));
        assert_eq!(
            model.set_value(target, UiValue::Float(1.5), &mut b),
            BindResult::Handled
        );
        assert_eq!(
            model.activate(
                (PageId(4), WidgetId(layout.first_widget.0 + 0x1000)),
                &mut b
            ),
            BindResult::Handled
        );
        assert_eq!(b.committed.get("ui_scale"), Some(&serde_json::json!(1.5)));
        assert_eq!(
            Settings::parse(&std::fs::read(&path).unwrap()).unwrap(),
            *b.committed
        );
        assert_eq!(b.sink.0, 0);
        b.path = path.join("cannot-create-under-file");
        assert_eq!(
            model.set_value(target, UiValue::Float(1.8), &mut b),
            BindResult::Handled
        );
        assert_eq!(
            model.activate(
                (PageId(4), WidgetId(layout.first_widget.0 + 0x1000)),
                &mut b
            ),
            BindResult::Denied
        );
        assert_eq!(b.committed.get("ui_scale"), Some(&serde_json::json!(1.5)));
        assert!(b.last_error.is_some());
        std::fs::remove_file(path).unwrap();
        assert!(!visible(b.committed, "msaa_samples"));
        assert!(b.staged.set("anti_aliasing", serde_json::json!("msaa")));
        assert!(visible(&b.staged, "msaa_samples"));
    }
}
