//! `UiPatch`: named, ordered, opt-in layers applied to a built model (design doc section 4/6).
//! The `original` preset is the empty patch list and leaves a model unchanged.
//!
//! Rules (deterministic, see docs/t-ui-model.md):
//! * patches apply in preset order, ops in patch order, on a copy; the input is never changed
//!   and a failed apply returns an error and no model;
//! * every op sets absolute state, so applying a preset twice equals applying it once;
//! * two ops that set one target differently (same widget field, same handler slot, same widget
//!   id with different content, remove versus define) are reported as conflicts, even inside one
//!   patch; equal duplicates are fine.

use crate::model::*;
use std::collections::BTreeMap;
use tsfp_ui_renderer::AnchoredRect;

#[derive(Clone, Debug, PartialEq)]
pub enum PageSelector {
    Id(PageId),
    All,
    /// Pages whose handler in this slot is set.
    WithHandler(HandlerSlot),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Insert {
    Start,
    End,
    Before(WidgetId),
    After(WidgetId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Enabled(bool),
    Navigable(bool),
    Text(TextRef),
    Rect(AnchoredRect),
    Role(WidgetRole),
    Binding(Option<BindingId>),
    Activate(Option<HandlerRef>),
    Selected(usize),
}
impl Change {
    fn field(&self) -> &'static str {
        match self {
            Change::Enabled(_) => "enabled",
            Change::Navigable(_) => "navigable",
            Change::Text(_) => "text",
            Change::Rect(_) => "rect",
            Change::Role(_) => "role",
            Change::Binding(_) => "binding",
            Change::Activate(_) => "activate",
            Change::Selected(_) => "selected",
        }
    }
    fn apply(&self, w: &mut Widget) {
        match self {
            Change::Enabled(v) => w.enabled = *v,
            Change::Navigable(v) => w.navigable = *v,
            Change::Text(v) => w.text = *v,
            Change::Rect(v) => w.rect = *v,
            Change::Role(v) => w.role = *v,
            Change::Binding(v) => w.bound = v.clone(),
            Change::Activate(v) => w.activate = v.clone(),
            Change::Selected(v) => w.selected = *v,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatchOp {
    /// Add a widget (same content again is a no-op). `unless_role` skips pages that already
    /// have a widget with that role.
    AddWidget {
        page: PageSelector,
        at: Insert,
        widget: Widget,
        unless_role: Option<WidgetRole>,
    },
    ModifyWidget {
        page: PageSelector,
        widget: WidgetId,
        change: Change,
    },
    /// Removing a missing widget is a no-op.
    RemoveWidget {
        page: PageSelector,
        widget: WidgetId,
    },
    /// Insert a page (same content again is a no-op), optionally with the edge that reaches it.
    InsertPage {
        page: Page,
        edge: Option<PageEdge>,
    },
    /// Replace (or clear with `None`) a page handler by stable page id.
    ReplaceHandler {
        page: PageSelector,
        slot: HandlerSlot,
        handler: Option<HandlerRef>,
    },
    SetTheme(Theme),
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiPatch {
    pub name: String,
    pub ops: Vec<PatchOp>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub patches: Vec<UiPatch>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    pub target: String,
    pub first_patch: String,
    pub second_patch: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PatchError {
    Conflicts(Vec<Conflict>),
    MissingPage {
        patch: String,
        page: PageId,
    },
    MissingWidget {
        patch: String,
        page: PageId,
        widget: WidgetId,
    },
    /// An id is taken by different content.
    WidgetExists {
        patch: String,
        page: PageId,
        widget: WidgetId,
    },
    PageExists {
        patch: String,
        page: PageId,
    },
}

impl Preset {
    /// The `original` preset: no patches.
    pub fn original() -> Self {
        Self {
            name: "original".to_string(),
            patches: Vec::new(),
        }
    }

    pub fn apply(&self, base: &UiModel) -> Result<UiModel, PatchError> {
        let mut model = base.clone();
        let mut effects: BTreeMap<String, (String, String)> = BTreeMap::new();
        let mut conflicts = Vec::new();
        for patch in &self.patches {
            for op in &patch.ops {
                let mut note = |key: String, value: String| -> bool {
                    match effects.get(&key) {
                        Some((owner, existing)) => {
                            let neutral = (existing == "keep") != (value == "keep");
                            let removal = existing == "remove" || value == "remove";
                            if *existing != value && !(neutral && !removal) {
                                conflicts.push(Conflict {
                                    target: key,
                                    first_patch: owner.clone(),
                                    second_patch: patch.name.clone(),
                                });
                                return true;
                            }
                            false
                        }
                        None => {
                            effects.insert(key, (patch.name.clone(), value));
                            false
                        }
                    }
                };
                apply_op(&mut model, op, &patch.name, &mut note)?;
            }
        }
        if !conflicts.is_empty() {
            return Err(PatchError::Conflicts(conflicts));
        }
        model.preset = self.name.clone();
        Ok(model)
    }
}

fn select(model: &UiModel, selector: &PageSelector) -> Vec<PageId> {
    match selector {
        PageSelector::Id(id) => vec![*id],
        PageSelector::All => model.pages.keys().copied().collect(),
        PageSelector::WithHandler(slot) => model
            .pages
            .values()
            .filter(|p| p.handlers.slot(*slot).is_some())
            .map(|p| p.id)
            .collect(),
    }
}

fn apply_op(
    model: &mut UiModel,
    op: &PatchOp,
    patch: &str,
    note: &mut dyn FnMut(String, String) -> bool,
) -> Result<(), PatchError> {
    let missing_page = |page| PatchError::MissingPage {
        patch: patch.to_string(),
        page,
    };
    match op {
        PatchOp::AddWidget {
            page,
            at,
            widget,
            unless_role,
        } => {
            for id in select(model, page) {
                let target = model.page_mut(id).ok_or_else(|| missing_page(id))?;
                if let Some(role) = unless_role {
                    if target
                        .widgets
                        .iter()
                        .any(|w| w.role == *role && w.id != widget.id)
                    {
                        continue;
                    }
                }
                let conflicted = note(
                    format!("{:x}/w{:x}", id.0, widget.id.0),
                    format!("define {widget:?}"),
                );
                match target.widget(widget.id) {
                    Some(existing) if existing == widget => continue,
                    // Reported as a conflict by the caller, not as a hard error.
                    Some(_) if conflicted => continue,
                    Some(_) => {
                        return Err(PatchError::WidgetExists {
                            patch: patch.to_string(),
                            page: id,
                            widget: widget.id,
                        });
                    }
                    None => {}
                }
                let index = match at {
                    Insert::Start => 0,
                    Insert::End => target.widgets.len(),
                    Insert::Before(anchor) | Insert::After(anchor) => {
                        let position = target.widgets.iter().position(|w| w.id == *anchor).ok_or(
                            PatchError::MissingWidget {
                                patch: patch.to_string(),
                                page: id,
                                widget: *anchor,
                            },
                        )?;
                        position + usize::from(matches!(at, Insert::After(_)))
                    }
                };
                target.widgets.insert(index, widget.clone());
            }
        }
        PatchOp::ModifyWidget {
            page,
            widget,
            change,
        } => {
            for id in select(model, page) {
                let target = model.page_mut(id).ok_or_else(|| missing_page(id))?;
                note(format!("{:x}/w{:x}", id.0, widget.0), "keep".to_string());
                note(
                    format!("{:x}/w{:x}/{}", id.0, widget.0, change.field()),
                    format!("{change:?}"),
                );
                let found = target
                    .widget_mut(*widget)
                    .ok_or(PatchError::MissingWidget {
                        patch: patch.to_string(),
                        page: id,
                        widget: *widget,
                    })?;
                change.apply(found);
            }
        }
        PatchOp::RemoveWidget { page, widget } => {
            for id in select(model, page) {
                let target = model.page_mut(id).ok_or_else(|| missing_page(id))?;
                note(format!("{:x}/w{:x}", id.0, widget.0), "remove".to_string());
                target.widgets.retain(|w| w.id != *widget);
                target
                    .links
                    .retain(|l| l.from != *widget && l.to != *widget);
                if target.initial_focus == Some(*widget) {
                    target.initial_focus = None;
                }
            }
        }
        PatchOp::InsertPage { page, edge } => {
            let conflicted = note(format!("{:x}", page.id.0), format!("define {page:?}"));
            match model.pages.get(&page.id) {
                Some(existing) if existing == page => {}
                Some(_) if conflicted => {}
                Some(_) => {
                    return Err(PatchError::PageExists {
                        patch: patch.to_string(),
                        page: page.id,
                    });
                }
                None => {
                    model.pages.insert(page.id, page.clone());
                }
            }
            if let Some(edge) = edge {
                if !model.edges.contains(edge) {
                    model.edges.push(edge.clone());
                }
            }
        }
        PatchOp::ReplaceHandler {
            page,
            slot,
            handler,
        } => {
            for id in select(model, page) {
                let target = model.page_mut(id).ok_or_else(|| missing_page(id))?;
                note(format!("{:x}/h{slot:?}", id.0), format!("{handler:?}"));
                *target.handlers.slot_mut(*slot) = handler.clone();
            }
        }
        PatchOp::SetTheme(theme) => {
            note("theme".to_string(), format!("{theme:?}"));
            model.theme = theme.clone();
        }
    }
    Ok(())
}
