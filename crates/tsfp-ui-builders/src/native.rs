//! Typed, explicitly selected builders. No host switch enables these automatically.
//! Structural Tier 1 evidence uses translated builders on synthetic memory; layout,
//! fonts, hints and side effects are outside this retained-model contract.
use tsfp_ui_model::{
    BuilderError, HandlerRef, HandlerSlot, PageBuilder, PageId, TextId, TextRef, Widget, WidgetId,
    WidgetKind,
};

/// Runtime globals of 0x2EB9E0, provided by the page opener through a typed boundary.
#[derive(Clone, Debug)]
pub struct Confirmation {
    pub first: TextId,
    pub second: Option<TextId>,
    pub yes: Option<HandlerRef>,
    pub no: Option<HandlerRef>,
}
impl Confirmation {
    pub fn build(self, rec: &mut impl PageBuilder, page: PageId) -> Result<(), BuilderError> {
        rec.push_page(page, 1)?;
        if let Some(no) = self.no.clone() {
            rec.set_handler(HandlerSlot::Back, no)?;
        }
        label(rec, 0, TextRef::Id(self.first))?;
        divider(rec, 1)?;
        if let Some(second) = self.second.filter(|id| id.0 != 0) {
            label(rec, 2, TextRef::Id(second))?;
            divider(rec, 3)?;
        }
        for (id, text, handler) in [(0, 0x1799, self.yes), (1, 0x179a, self.no)] {
            let kind = if handler.is_some() {
                WidgetKind::Button
            } else {
                WidgetKind::Label
            };
            let mut w = Widget::new(WidgetId(id), kind);
            w.text = TextRef::Id(TextId(text));
            w.activate = handler;
            rec.record_row(w)?;
        }
        rec.set_initial_focus(WidgetId(1))?;
        rec.pop_page()
    }
}

/// Typed content selected by the 0x192110 save-screen state dispatcher. The caller
/// resolves its private state table; no inferred state-to-label mapping is embedded.
#[derive(Clone, Copy, Debug)]
pub enum SaveStatus {
    Empty,
    Bank(TextId),
    Runtime,
}
impl SaveStatus {
    pub fn build(self, rec: &mut impl PageBuilder, page: PageId) -> Result<(), BuilderError> {
        rec.push_page(page, 1)?;
        match self {
            Self::Empty => {}
            Self::Bank(id) => label(rec, 0, TextRef::Id(id))?,
            Self::Runtime => label(rec, 0, TextRef::Dynamic)?,
        }
        rec.pop_page()
    }
}

fn label(rec: &mut impl PageBuilder, row: usize, text: TextRef) -> Result<(), BuilderError> {
    let mut w = Widget::new(WidgetId::anon(row), WidgetKind::Label);
    w.text = text;
    w.navigable = false;
    rec.record_row(w)
}
fn divider(rec: &mut impl PageBuilder, row: usize) -> Result<(), BuilderError> {
    let mut w = Widget::new(WidgetId::anon(row), WidgetKind::Panel);
    w.navigable = false;
    rec.record_row(w)
}
