// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure, opt-in UI extensions. Contract and verification debt: docs/t-ui-extensions.md.
use std::collections::BTreeMap;

pub mod advanced_settings;
pub mod controller_profile_settings;
pub mod controller_settings;
pub mod input_extras;
pub mod input_settings;
mod keyboard_text;
pub mod mk_settings;
pub mod profile_settings;
pub mod settings_page;
pub use keyboard_text::{Key, KeyEvent, KeyRejected, KeyboardText};
use tsfp_text_field::{EditError, TextField};
use tsfp_ui_input::{
    ActionArg, ActionId, BindResult, PointerFrame, PointerPolicy, UiBindings, UiEvent, UiValue,
    ValueId,
};
use tsfp_ui_model::*;
use tsfp_ui_renderer::{AnchorSpace, AnchoredRect, Edge, Vec2};

/// Page-local model identities remain u64; never truncate them to input's u32 WidgetId.
pub type Target = (PageId, WidgetId);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointerFlags {
    pub visible: bool,
    pub auto_release: bool,
}

/// T2168 menu pointer routing. Absent (the original preset) means the host feeds no pointer
/// frames to menus at all; each present flag enables one pointer gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuPointer {
    /// Hovering a usable widget moves menu focus to it.
    pub hover_focus: bool,
    /// Left press and release on the same widget activates it.
    pub click_activate: bool,
    /// Right press is Back.
    pub right_back: bool,
    /// List items per wheel notch, 0 disables the wheel.
    pub wheel_step: i32,
}

impl MenuPointer {
    /// Policy for `UiInput::pointer_policy`, keeping the host's idle-hide timing.
    pub fn policy(&self, host: &PointerPolicy) -> PointerPolicy {
        PointerPolicy {
            idle_hide_ms: host.idle_hide_ms,
            wheel_step: self.wheel_step,
            hover_focus: self.hover_focus,
        }
    }
    /// Masks the gestures this layer does not enable before the frame reaches `UiInput`.
    pub fn filter(&self, frame: PointerFrame) -> PointerFrame {
        let mut out = frame;
        out.buttons.left &= self.click_activate;
        out.buttons.right &= self.right_back;
        if self.wheel_step == 0 {
            out.wheel = 0;
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Action(ActionId),
    Value(ValueId),
    /// Text commit is a separate byte-oriented seam: UiValue cannot carry text.
    Text {
        commit: ActionId,
        field: TextField,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Extension {
    pub patch: UiPatch,
    pub bindings: BTreeMap<Target, Binding>,
    pub pointer: Option<PointerFlags>,
    pub menu_pointer: Option<MenuPointer>,
    pub keyboard_text: Option<KeyboardText>,
}
impl Extension {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            patch: UiPatch {
                name: name.into(),
                ops: Vec::new(),
            },
            bindings: BTreeMap::new(),
            pointer: None,
            menu_pointer: None,
            keyboard_text: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtensionPreset {
    pub name: String,
    pub layers: Vec<Extension>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ExtendedModel {
    pub model: UiModel,
    pub bindings: BTreeMap<Target, Binding>,
    /// None preserves the host's existing pointer policy.
    pub pointer: Option<PointerFlags>,
    /// None keeps menus pointer-free, as in the original game.
    pub menu_pointer: Option<MenuPointer>,
    /// None keeps text entry pad-only, as in the original game.
    pub keyboard_text: Option<KeyboardText>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum ExtensionError {
    Patch(PatchError),
    BindingConflict(Target),
    PointerConflict,
    MenuPointerConflict,
    KeyboardTextConflict,
    MissingTarget(Target),
}
impl ExtensionPreset {
    /// The unmodified UI preset. Applying it leaves the base model unchanged.
    ///
    /// ```
    /// use tsfp_ui_ext::ExtensionPreset;
    /// use tsfp_ui_model::UiModel;
    /// let base = UiModel::new();
    /// let result = ExtensionPreset::original().apply(&base).unwrap();
    /// assert_eq!(result.model, base);
    /// assert!(result.bindings.is_empty());
    /// assert_eq!(result.pointer, None);
    /// ```
    pub fn original() -> Self {
        Self {
            name: "original".into(),
            layers: Vec::new(),
        }
    }
    /// Atomic application. Empty layers preserve even the base's preset name.
    pub fn apply(&self, base: &UiModel) -> Result<ExtendedModel, ExtensionError> {
        let mut bindings = BTreeMap::new();
        let mut pointer = None;
        let mut menu_pointer = None;
        let mut keyboard_text = None;
        for layer in &self.layers {
            for (target, binding) in &layer.bindings {
                if let Some(previous) = bindings.insert(*target, binding.clone()) {
                    if previous != *binding {
                        return Err(ExtensionError::BindingConflict(*target));
                    }
                }
            }
            if let Some(flags) = layer.pointer {
                if pointer.is_some_and(|previous| previous != flags) {
                    return Err(ExtensionError::PointerConflict);
                }
                pointer = Some(flags);
            }
            if let Some(routing) = layer.menu_pointer {
                if menu_pointer.is_some_and(|previous| previous != routing) {
                    return Err(ExtensionError::MenuPointerConflict);
                }
                menu_pointer = Some(routing);
            }
            if let Some(keys) = layer.keyboard_text {
                if keyboard_text.is_some_and(|previous| previous != keys) {
                    return Err(ExtensionError::KeyboardTextConflict);
                }
                keyboard_text = Some(keys);
            }
        }
        let model = if self.layers.is_empty() {
            base.clone()
        } else {
            Preset {
                name: self.name.clone(),
                patches: self.layers.iter().map(|l| l.patch.clone()).collect(),
            }
            .apply(base)
            .map_err(ExtensionError::Patch)?
        };
        for target in bindings.keys() {
            if model
                .page(target.0)
                .and_then(|p| p.widget(target.1))
                .is_none()
            {
                return Err(ExtensionError::MissingTarget(*target));
            }
        }
        Ok(ExtendedModel {
            model,
            bindings,
            pointer,
            menu_pointer,
            keyboard_text,
        })
    }
}
impl ExtendedModel {
    /// Pointer policy for menus, or None when pointer frames must not be routed.
    pub fn menu_pointer_policy(&self, host: &PointerPolicy) -> Option<PointerPolicy> {
        self.menu_pointer.map(|routing| routing.policy(host))
    }
    /// Routed pointer input for `page`, filtered by this preset; None when routing is off.
    pub fn menu_pointer_frame(&self, frame: PointerFrame) -> Option<PointerFrame> {
        self.menu_pointer.map(|routing| routing.filter(frame))
    }
    /// Dispatches a routed Activate on `page` to an extension binding. Input and model widget
    /// ids are both u64, so the identity is carried over unchanged. Anything else, including
    /// widgets with original handlers, is Ignored and stays with the host's handler path.
    pub fn route(
        &self,
        page: PageId,
        event: &UiEvent,
        bindings: &mut dyn UiBindings,
    ) -> BindResult {
        match event {
            UiEvent::Activate(widget) => self.activate((page, WidgetId(widget.0)), bindings),
            _ => BindResult::Ignored,
        }
    }
    pub fn activate(&self, target: Target, bindings: &mut dyn UiBindings) -> BindResult {
        match self.bindings.get(&target) {
            Some(Binding::Action(action)) => bindings.action(*action, ActionArg::None),
            _ => BindResult::Ignored,
        }
    }
    pub fn value(&self, target: Target, bindings: &dyn UiBindings) -> UiValue {
        match self.bindings.get(&target) {
            Some(Binding::Value(id)) => bindings.value(*id),
            _ => UiValue::None,
        }
    }
    pub fn set_value(
        &self,
        target: Target,
        value: UiValue,
        bindings: &mut dyn UiBindings,
    ) -> BindResult {
        match self.bindings.get(&target) {
            Some(Binding::Value(id)) => bindings.set_value(*id, value),
            _ => BindResult::Ignored,
        }
    }
    /// Clone an editor per session/context; the preset's initial field is never edited.
    pub fn text_field(&self, target: Target) -> Option<(ActionId, TextField)> {
        match self.bindings.get(&target) {
            Some(Binding::Text { commit, field }) => Some((*commit, field.clone())),
            _ => None,
        }
    }
}

/// Caller supplies the runtime-localised game's Back text id and the same per-page action
/// used for pad/keyboard Back. Pages without a Back action are deliberately left alone.
pub fn back_buttons(
    model: &UiModel,
    text: TextId,
    id: WidgetId,
    actions: &BTreeMap<PageId, ActionId>,
) -> Extension {
    let mut extension = Extension::new("back-buttons");
    for page in model.pages.values() {
        if page.handlers.back.is_none() || page.widgets.iter().any(|w| w.role == WidgetRole::Back) {
            continue;
        }
        let Some(action) = actions.get(&page.id) else {
            continue;
        };
        let mut widget = Widget::new(id, WidgetKind::Button);
        widget.role = WidgetRole::Back;
        widget.text = TextRef::Id(text);
        widget.activate = page.handlers.back.clone();
        widget.rect = AnchoredRect {
            space: AnchorSpace::SafeArea4x3,
            horizontal: Edge::Start,
            vertical: Edge::End,
            offset: Vec2::new(16.0, -16.0),
            size: Vec2::new(96.0, 24.0),
        };
        add_row(
            &mut extension,
            page.id,
            Insert::End,
            widget,
            Binding::Action(*action),
        );
    }
    extension
}

#[derive(Clone, Debug, PartialEq)]
pub enum SettingKind {
    Slider,
    Cycler(Vec<UiOption>),
}
#[derive(Clone, Debug, PartialEq)]
pub struct SettingRow {
    pub page: PageId,
    pub id: WidgetId,
    pub at: Insert,
    pub label: TextId,
    pub rect: AnchoredRect,
    pub value: ValueId,
    pub kind: SettingKind,
}
/// Shapes for sensitivity, invert, presets, assignment and other T2211 values. The
/// settings owner supplies ids, ranges, option text ids and persistence; no guessed bank ids.
pub fn settings_rows(_model: &UiModel, rows: &[SettingRow]) -> Extension {
    let mut extension = Extension::new("settings-rows");
    for row in rows {
        let mut widget = Widget::new(
            row.id,
            match row.kind {
                SettingKind::Slider => WidgetKind::Slider,
                SettingKind::Cycler(_) => WidgetKind::Cycler,
            },
        );
        widget.text = TextRef::Id(row.label);
        widget.rect = row.rect;
        widget.bound = Some(BindingId(format!("ui.value.{}", row.value.0)));
        if let SettingKind::Cycler(options) = &row.kind {
            widget.options = options.clone();
        }
        add_row(
            &mut extension,
            row.page,
            row.at,
            widget,
            Binding::Value(row.value),
        );
    }
    extension
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRow {
    pub page: PageId,
    pub id: WidgetId,
    pub at: Insert,
    pub label: TextId,
    pub rect: AnchoredRect,
    pub commit: ActionId,
    pub initial: Vec<u8>,
    pub limit: usize,
    pub charset: Vec<u8>,
}
pub fn text_row(_model: &UiModel, row: &TextRow) -> Result<Extension, EditError> {
    let field = TextField::new(&row.initial, row.limit, &row.charset)?;
    let mut extension = Extension::new("text-row");
    let mut widget = Widget::new(row.id, WidgetKind::TextField);
    widget.text = TextRef::Id(row.label);
    widget.rect = row.rect;
    widget.bound = Some(BindingId(format!("ui.text.{}", row.commit.0)));
    add_row(
        &mut extension,
        row.page,
        row.at,
        widget,
        Binding::Text {
            commit: row.commit,
            field,
        },
    );
    Ok(extension)
}
fn add_row(extension: &mut Extension, page: PageId, at: Insert, widget: Widget, binding: Binding) {
    extension.bindings.insert((page, widget.id), binding);
    extension.patch.ops.push(PatchOp::AddWidget {
        page: PageSelector::Id(page),
        at,
        widget,
        unless_role: None,
    });
}

/// Explicit targets: never move every widget just because it happens to be on a HUD page.
/// Geometry is absolute, making a fixed generated patch idempotent.
pub fn anchoring(_model: &UiModel, targets: &BTreeMap<Target, AnchoredRect>) -> Extension {
    let mut extension = Extension::new("anchoring");
    for (target, rect) in targets {
        // Missing targets remain explicit errors at application, rather than silently disappearing.
        extension.patch.ops.push(PatchOp::ModifyWidget {
            page: PageSelector::Id(target.0),
            widget: target.1,
            change: Change::Rect(*rect),
        });
    }
    extension
}
/// T2168: opt-in hover focus, click activate, right-click Back and wheel scrolling in menus.
pub fn menu_pointer(_model: &UiModel, routing: MenuPointer) -> Extension {
    let mut ext = Extension::new("menu-pointer");
    ext.menu_pointer = Some(routing);
    ext
}

/// T2169 opt-in layer: the host keyboard edits text fields (see [`KeyboardText`]).
pub fn keyboard_text(_model: &UiModel, keys: KeyboardText) -> Extension {
    let mut extension = Extension::new("keyboard-text");
    extension.keyboard_text = Some(keys);
    extension
}
pub fn pointer_policy(_model: &UiModel, flags: PointerFlags) -> Extension {
    let mut extension = Extension::new("pointer-policy");
    extension.pointer = Some(flags);
    extension
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchoringPreset {
    MenuSafeArea,
    HudWindowEdges,
}
/// Preserve authored offsets, sizes and edge choices; change only the reference box.
/// HUD widgets must already declare their intended horizontal/vertical edge.
pub fn anchoring_preset(model: &UiModel, targets: &[Target], preset: AnchoringPreset) -> Extension {
    let mut extension = Extension::new(match preset {
        AnchoringPreset::MenuSafeArea => "menu-safe-area",
        AnchoringPreset::HudWindowEdges => "hud-window-edges",
    });
    for target in targets {
        if let Some(widget) = model.page(target.0).and_then(|p| p.widget(target.1)) {
            let mut rect = widget.rect;
            rect.space = match preset {
                AnchoringPreset::MenuSafeArea => AnchorSpace::SafeArea4x3,
                AnchoringPreset::HudWindowEdges => AnchorSpace::Window,
            };
            extension.patch.ops.push(PatchOp::ModifyWidget {
                page: PageSelector::Id(target.0),
                widget: target.1,
                change: Change::Rect(rect),
            });
        } else {
            // Preserve the missing target so Preset reports it, rather than accepting a typo.
            extension.patch.ops.push(PatchOp::ModifyWidget {
                page: PageSelector::Id(target.0),
                widget: target.1,
                change: Change::Enabled(true),
            });
        }
    }
    extension
}

/// UiBindings has no string-valued setter. A host adapter implements this companion
/// contract to persist validated bytes before dispatching the numeric commit action.
pub trait TextBindings {
    fn commit_text(&mut self, action: ActionId, text: &[u8]) -> BindResult;
}
pub fn commit_text(
    action: ActionId,
    field: &TextField,
    bindings: &mut dyn TextBindings,
) -> BindResult {
    if field.outcome() != tsfp_text_field::Outcome::Confirmed {
        return BindResult::Denied;
    }
    bindings.commit_text(action, field.text())
}
