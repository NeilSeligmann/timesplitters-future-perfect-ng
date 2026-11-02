//! UiBindings: how UI events reach the game and how the game's values reach the UI.

use crate::input::UiEvent;
use crate::layout::WidgetId;
use std::collections::HashMap;
use tsfp_ui_model::{MenuFocus, Page, WidgetId as ModelWidgetId};

macro_rules! id_type {
    ($($n:ident),*) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $n(pub u32);
    )*};
}
id_type!(ActionId, ValueId, ListId, TextId);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActionArg {
    None,
    Int(i32),
    Index(usize),
    Widget(WidgetId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindResult {
    Handled,
    /// Unknown action or nothing to do.
    Ignored,
    /// Refused by game state (for example a locked level).
    Denied,
}

/// Why a focused-row activation was withheld at the `UiBindings` boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationSuppression {
    FocusChanged,
    MissingRow,
    Disabled,
    NotFocusable,
}

/// Observable result of the last-mile activation check. Disabled-help actions are separate
/// explicit bindings; an ordinary row or page handler is never reused for a disabled row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationOutcome {
    NotRequested,
    Dispatched {
        widget: ModelWidgetId,
        disabled_help: bool,
        result: BindResult,
    },
    Unbound {
        widget: ModelWidgetId,
    },
    Suppressed {
        widget: ModelWidgetId,
        reason: ActivationSuppression,
    },
}

impl ActivationOutcome {
    pub fn binding_result(self) -> Option<BindResult> {
        match self {
            Self::Dispatched { result, .. } => Some(result),
            Self::NotRequested | Self::Unbound { .. } | Self::Suppressed { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiValue {
    None,
    Bool(bool),
    Int(i32),
    Float(f32),
    Index(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItemView {
    pub text: TextId,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListView {
    pub items: Vec<ListItemView>,
    pub selected: Option<usize>,
    pub first_visible: usize,
}

/// The model calls this; `Legacy` implementations forward to ported handlers and read
/// `MenuSnapshot`, `Native` ones use `tsfp-state` structs. No code of either lives here.
pub trait UiBindings {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult;
    fn value(&self, id: ValueId) -> UiValue;
    fn set_value(&mut self, id: ValueId, value: UiValue) -> BindResult;
    fn list(&self, id: ListId) -> ListView;
    /// Resolved string for a text id (string banks, formatting, localisation behind it).
    fn text(&self, id: TextId) -> &str;
}

/// Widget activation to action table: the data form of "page handler" wiring.
#[derive(Clone, Debug, Default)]
pub struct ActionMap {
    on_activate: HashMap<WidgetId, (ActionId, ActionArg)>,
    on_disabled_help: HashMap<WidgetId, (ActionId, ActionArg)>,
    on_back: Option<ActionId>,
    on_start: Option<ActionId>,
}

impl ActionMap {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn bind_activate(&mut self, w: WidgetId, a: ActionId, arg: ActionArg) {
        self.on_activate.insert(w, (a, arg));
    }
    /// Bind a dedicated disabled-row help action. This is never treated as ordinary activation.
    pub fn bind_disabled_help(&mut self, w: WidgetId, a: ActionId, arg: ActionArg) {
        self.on_disabled_help.insert(w, (a, arg));
    }
    pub fn bind_back(&mut self, a: ActionId) {
        self.on_back = Some(a);
    }
    pub fn bind_start(&mut self, a: ActionId) {
        self.on_start = Some(a);
    }
    /// Forward non-activation events. Row activation requires `dispatch_activation`, which checks
    /// the current typed page and focus immediately before it calls `UiBindings`.
    pub fn apply(&self, ev: &UiEvent, b: &mut dyn UiBindings) -> Option<BindResult> {
        match ev {
            UiEvent::Activate(_) => None,
            UiEvent::Back => self.on_back.map(|a| b.action(a, ActionArg::None)),
            UiEvent::Start => self.on_start.map(|a| b.action(a, ActionArg::None)),
            _ => None,
        }
    }

    /// Recheck the focused row at the callback boundary. The selection is read without
    /// reconciling it, so a row disabled after navigation remains selected while its stale
    /// activation is suppressed. Only an explicitly registered disabled-help action may run.
    pub fn dispatch_activation(
        &self,
        page: &Page,
        focus: &MenuFocus,
        ev: &UiEvent,
        b: &mut dyn UiBindings,
    ) -> ActivationOutcome {
        let UiEvent::Activate(input_id) = ev else {
            return ActivationOutcome::NotRequested;
        };
        let widget = ModelWidgetId(u64::from(input_id.0));
        if focus.selected != Some(widget) {
            return ActivationOutcome::Suppressed {
                widget,
                reason: ActivationSuppression::FocusChanged,
            };
        }
        let Some(row) = page.widget(widget) else {
            return ActivationOutcome::Suppressed {
                widget,
                reason: ActivationSuppression::MissingRow,
            };
        };
        let key = WidgetId(input_id.0);
        if !row.enabled {
            return match self.on_disabled_help.get(&key) {
                Some((action, arg)) => ActivationOutcome::Dispatched {
                    widget,
                    disabled_help: true,
                    result: b.action(*action, *arg),
                },
                None => ActivationOutcome::Suppressed {
                    widget,
                    reason: ActivationSuppression::Disabled,
                },
            };
        }
        if !row.is_focusable() {
            return ActivationOutcome::Suppressed {
                widget,
                reason: ActivationSuppression::NotFocusable,
            };
        }
        match self.on_activate.get(&key) {
            Some((action, arg)) => ActivationOutcome::Dispatched {
                widget,
                disabled_help: false,
                result: b.action(*action, *arg),
            },
            None => ActivationOutcome::Unbound { widget },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RecordedCall {
    Action(ActionId, ActionArg),
    SetValue(ValueId, UiValue),
}

/// Test double: records writes, answers reads from preloaded tables.
#[derive(Debug, Default)]
pub struct RecordingBindings {
    pub calls: Vec<RecordedCall>,
    pub values: HashMap<ValueId, UiValue>,
    pub lists: HashMap<ListId, ListView>,
    pub texts: HashMap<TextId, String>,
    pub denied: Vec<ActionId>,
}

impl UiBindings for RecordingBindings {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        self.calls.push(RecordedCall::Action(id, arg));
        if self.denied.contains(&id) {
            BindResult::Denied
        } else {
            BindResult::Handled
        }
    }
    fn value(&self, id: ValueId) -> UiValue {
        self.values.get(&id).copied().unwrap_or(UiValue::None)
    }
    fn set_value(&mut self, id: ValueId, value: UiValue) -> BindResult {
        self.calls.push(RecordedCall::SetValue(id, value));
        self.values.insert(id, value);
        BindResult::Handled
    }
    fn list(&self, id: ListId) -> ListView {
        self.lists.get(&id).cloned().unwrap_or_default()
    }
    fn text(&self, id: TextId) -> &str {
        self.texts.get(&id).map_or("", String::as_str)
    }
}
