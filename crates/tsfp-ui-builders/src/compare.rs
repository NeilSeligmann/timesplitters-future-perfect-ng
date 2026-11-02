use tsfp_ui_model::{Page, Widget};

/// Field-by-field comparison of a built page against the page loaded from the dump.
/// Layout (rect, font, align) is not produced by the builder code, so it is not compared.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PageDiff {
    pub rows_built: usize,
    pub rows_dump: usize,
    /// Rows compared positionally (page order).
    pub rows_compared: usize,
    pub id_mismatch: Vec<usize>,
    pub kind_mismatch: Vec<usize>,
    pub enabled_mismatch: Vec<usize>,
    pub navigable_mismatch: Vec<usize>,
    pub activate_mismatch: Vec<usize>,
    /// `(row index, built text id, dump text id)`; ids only, never strings.
    pub text_mismatch: Vec<(usize, Option<u32>, Option<u32>)>,
    pub option_count_mismatch: Vec<usize>,
    pub option_text_mismatch: Vec<(usize, usize, Option<u32>, Option<u32>)>,
    pub select_handler_equal: bool,
    pub back_handler_equal: bool,
    pub focus_graph_equal: bool,
}

fn text_id(w: tsfp_ui_model::TextRef) -> Option<u32> {
    match w {
        tsfp_ui_model::TextRef::Id(t) => Some(t.0),
        _ => None,
    }
}

impl PageDiff {
    pub fn exact(&self) -> bool {
        self.rows_built == self.rows_dump
            && self.id_mismatch.is_empty()
            && self.kind_mismatch.is_empty()
            && self.enabled_mismatch.is_empty()
            && self.navigable_mismatch.is_empty()
            && self.activate_mismatch.is_empty()
            && self.text_mismatch.is_empty()
            && self.option_count_mismatch.is_empty()
            && self.option_text_mismatch.is_empty()
            && self.select_handler_equal
            && self.back_handler_equal
            && self.focus_graph_equal
    }
    /// Counts fields that differ in a way that is not a text id (text ids may be bank aliases).
    pub fn structural_mismatches(&self) -> usize {
        self.rows_built.abs_diff(self.rows_dump)
            + self.id_mismatch.len()
            + self.kind_mismatch.len()
            + self.enabled_mismatch.len()
            + self.navigable_mismatch.len()
            + self.activate_mismatch.len()
            + self.option_count_mismatch.len()
            + usize::from(!self.select_handler_equal)
            + usize::from(!self.back_handler_equal)
            + usize::from(!self.focus_graph_equal)
    }
}

pub fn compare_pages(built: &Page, dump: &Page) -> PageDiff {
    let mut d = PageDiff {
        rows_built: built.widgets.len(),
        rows_dump: dump.widgets.len(),
        select_handler_equal: built.handlers.select == dump.handlers.select,
        back_handler_equal: built.handlers.back == dump.handlers.back,
        focus_graph_equal: built.focus_graph() == dump.focus_graph(),
        ..PageDiff::default()
    };
    for (i, (b, o)) in built.widgets.iter().zip(&dump.widgets).enumerate() {
        d.rows_compared += 1;
        row_diff(&mut d, i, b, o);
    }
    d
}

fn row_diff(d: &mut PageDiff, i: usize, b: &Widget, o: &Widget) {
    if b.id != o.id {
        d.id_mismatch.push(i);
    }
    if b.kind != o.kind {
        d.kind_mismatch.push(i);
    }
    if b.enabled != o.enabled {
        d.enabled_mismatch.push(i);
    }
    if b.navigable != o.navigable {
        d.navigable_mismatch.push(i);
    }
    if b.activate != o.activate {
        d.activate_mismatch.push(i);
    }
    if b.text != o.text {
        d.text_mismatch.push((i, text_id(b.text), text_id(o.text)));
    }
    if b.options.len() != o.options.len() {
        d.option_count_mismatch.push(i);
    }
    for (k, (x, y)) in b.options.iter().zip(&o.options).enumerate() {
        if x.text != y.text {
            d.option_text_mismatch
                .push((i, k, text_id(x.text), text_id(y.text)));
        }
    }
}
