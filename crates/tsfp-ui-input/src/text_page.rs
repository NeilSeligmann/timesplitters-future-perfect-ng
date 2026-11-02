// SPDX-License-Identifier: GPL-3.0-or-later
//! T2197: name-entry page callbacks. A text field's terminal events map to the page's done and
//! cancel handlers read from the original builder (`tsfp_text_field::field_spec`), so the
//! bindings layer calls the same handler the original page stored at +0x1CC / +0x1D0.

use crate::input::{ContextId, UiEvent, UiInput};
use crate::layout::WidgetId;
use tsfp_text_field::field_spec::FieldSpec;
use tsfp_text_field::name_entry::NameEntry;
use tsfp_ui_model::HandlerRef;

/// Done and cancel handlers of one text page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextPageHandlers {
    pub widget: WidgetId,
    pub done: HandlerRef,
    pub cancel: HandlerRef,
}

impl TextPageHandlers {
    /// Legacy handler tokens from a spec decoded at run time.
    pub fn from_spec(widget: WidgetId, spec: &FieldSpec) -> Self {
        Self {
            widget,
            done: HandlerRef::Legacy(spec.done),
            cancel: HandlerRef::Legacy(spec.cancel),
        }
    }
}

/// What the page does after a text event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextAction {
    /// Write `text` to the field's record, then call `handler`.
    Commit { text: Vec<u8>, handler: HandlerRef },
    /// The record keeps its old text, call `handler`.
    Cancel { handler: HandlerRef },
}

/// Maps a routed text event of this page's widget to its handler; other events pass through.
pub fn dispatch_text_event(page: &TextPageHandlers, event: &UiEvent) -> Option<TextAction> {
    match event {
        UiEvent::TextCommit { widget, text } if *widget == page.widget => {
            Some(TextAction::Commit {
                text: text.clone(),
                handler: page.done.clone(),
            })
        }
        UiEvent::TextCancel(widget) if *widget == page.widget => Some(TextAction::Cancel {
            handler: page.cancel.clone(),
        }),
        _ => None,
    }
}

/// Opens the name entry's configured field in `ctx` and returns the page's handler table.
pub fn begin_name_entry(
    input: &mut UiInput,
    ctx: ContextId,
    widget: WidgetId,
    entry: &NameEntry,
) -> Option<TextPageHandlers> {
    input
        .begin_text_field(ctx, widget, entry.field.clone())
        .then(|| TextPageHandlers::from_spec(widget, &entry.spec))
}
