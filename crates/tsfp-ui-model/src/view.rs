//! View state over a model: page stack and per-page focus. The model itself holds no game state.

use crate::focus::Dir;
use crate::model::{PageId, UiModel, WidgetId};
use std::collections::{BTreeMap, btree_map::Entry};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiView {
    stack: Vec<PageId>,
    focus: BTreeMap<PageId, WidgetId>,
}

impl UiView {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn current(&self) -> Option<PageId> {
        self.stack.last().copied()
    }
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
    /// Open a page (false if the model lacks it). Focus starts at the page start focus
    /// unless the page was visited before.
    pub fn open(&mut self, model: &UiModel, id: PageId) -> bool {
        let Some(page) = model.page(id) else {
            return false;
        };
        self.stack.push(id);
        if let Entry::Vacant(slot) = self.focus.entry(id) {
            if let Some(start) = page.start_focus() {
                slot.insert(start);
            }
        }
        true
    }
    pub fn pop(&mut self) -> Option<PageId> {
        self.stack.pop()
    }
    pub fn focused(&self) -> Option<WidgetId> {
        self.current().and_then(|p| self.focus.get(&p).copied())
    }
    /// Move focus one step. Returns the new focus if it changed.
    pub fn navigate(&mut self, model: &UiModel, dir: Dir) -> Option<WidgetId> {
        let page_id = self.current()?;
        let from = self.focused()?;
        let to = model.page(page_id)?.focus_graph().step(from, dir)?;
        if to == from {
            return None;
        }
        self.focus.insert(page_id, to);
        Some(to)
    }
}
