//! Rust UI page projection for the typed Arcade category chooser.
//!
//! The caller supplies presentation widgets from a Rust-owned layout and text-table source.
//! This builder preserves the chooser's separate row/category identities without guessing
//! geometry, strings, guest addresses, or original widget IDs.

use crate::arcade_challenge::{ArcadeCategoryMenu, ArcadeCategoryRowId, ArcadeChallengeBackend};
use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_ui_renderer::AnchoredRect;

/// One widget from the chooser presentation. Only category options carry a typed row identity.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcadeCategoryPageWidget {
    pub row: Option<ArcadeCategoryRowId>,
    pub widget: Widget,
}

/// Caller-owned layout and presentation inputs for one category chooser page.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcadeCategoryPageLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub widgets: Vec<ArcadeCategoryPageWidget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeCategoryPageBuildError {
    LegacyBackend,
    DuplicateWidget(WidgetId),
    DuplicateRow(ArcadeCategoryRowId),
    UnknownRow(ArcadeCategoryRowId),
    MissingRow(ArcadeCategoryRowId),
    RowOrderMismatch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ArcadeCategoryPage {
    pub page: Page,
    row_widgets: Vec<(ArcadeCategoryRowId, WidgetId)>,
}

impl ArcadeCategoryPage {
    pub fn row_widgets(&self) -> &[(ArcadeCategoryRowId, WidgetId)] {
        &self.row_widgets
    }

    pub fn widget_for_row(&self, row: ArcadeCategoryRowId) -> Option<WidgetId> {
        self.row_widgets
            .iter()
            .find(|(candidate, _)| *candidate == row)
            .map(|(_, widget)| *widget)
    }
}

/// Build a complete page for the opt-in chooser. The supplied option order must match the
/// Rust-owned category projection; row IDs remain distinct from category IDs.
pub fn build_arcade_category_page(
    menu: &ArcadeCategoryMenu,
    page_id: PageId,
    layout: ArcadeCategoryPageLayout,
) -> Result<ArcadeCategoryPage, ArcadeCategoryPageBuildError> {
    build_arcade_category_page_with_focus(menu, page_id, layout, None)
}

/// Build a category page with an optional identity-reconciled focus row from its Rust session.
///
/// The supplied row takes precedence over the menu's last activated row. Unknown rows are not
/// guessed here; normal initial focus is used if the row is absent from the completed map.
pub fn build_arcade_category_page_with_focus(
    menu: &ArcadeCategoryMenu,
    page_id: PageId,
    layout: ArcadeCategoryPageLayout,
    focused_row: Option<ArcadeCategoryRowId>,
) -> Result<ArcadeCategoryPage, ArcadeCategoryPageBuildError> {
    if menu.backend() != ArcadeChallengeBackend::Rust {
        return Err(ArcadeCategoryPageBuildError::LegacyBackend);
    }

    let mut row_widgets = Vec::with_capacity(menu.entries().len());
    let mut widget_ids = Vec::with_capacity(layout.widgets.len());
    for item in &layout.widgets {
        if widget_ids.contains(&item.widget.id) {
            return Err(ArcadeCategoryPageBuildError::DuplicateWidget(
                item.widget.id,
            ));
        }
        widget_ids.push(item.widget.id);

        if let Some(row) = item.row {
            if row_widgets.iter().any(|(candidate, _)| *candidate == row) {
                return Err(ArcadeCategoryPageBuildError::DuplicateRow(row));
            }
            if !menu.entries().iter().any(|entry| entry.row == row) {
                return Err(ArcadeCategoryPageBuildError::UnknownRow(row));
            }
            row_widgets.push((row, item.widget.id));
        }
    }

    if row_widgets.len() != menu.entries().len() {
        if let Some(entry) = menu
            .entries()
            .iter()
            .find(|entry| !row_widgets.iter().any(|(row, _)| *row == entry.row))
        {
            return Err(ArcadeCategoryPageBuildError::MissingRow(entry.row));
        }
    }
    if row_widgets
        .iter()
        .map(|(row, _)| *row)
        .ne(menu.entries().iter().map(|entry| entry.row))
    {
        return Err(ArcadeCategoryPageBuildError::RowOrderMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    page.initial_focus = focused_row
        .or_else(|| menu.selected_row())
        .and_then(|row| row_widgets.iter().find(|(candidate, _)| *candidate == row))
        .map(|(_, widget)| *widget);
    page.widgets = layout.widgets.into_iter().map(|item| item.widget).collect();

    Ok(ArcadeCategoryPage { page, row_widgets })
}

#[cfg(test)]
mod tests {
    use super::{
        ArcadeCategoryPageBuildError, ArcadeCategoryPageLayout, ArcadeCategoryPageWidget,
        build_arcade_category_page,
    };
    use crate::arcade_challenge::{
        ArcadeCategoryId, ArcadeCategoryMenu, ArcadeCategoryRowId, ArcadeCategorySnapshot,
        ArcadeChallengeBackend, OwnedArcadeChallengeData,
    };
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{PageId, TextId, TextRef, Widget, WidgetId, WidgetKind};

    fn menu(backend: ArcadeChallengeBackend) -> ArcadeCategoryMenu {
        let provider = OwnedArcadeChallengeData::new(
            vec![
                ArcadeCategorySnapshot::new(ArcadeCategoryId::new(7), vec![]),
                ArcadeCategorySnapshot::new(ArcadeCategoryId::new(2), vec![]),
            ],
            vec![],
        )
        .unwrap();
        ArcadeCategoryMenu::build_with_backend(&provider, backend)
    }

    fn row(row: u32, widget: u64) -> ArcadeCategoryPageWidget {
        let mut view = Widget::new(WidgetId(widget), WidgetKind::Button);
        view.text = TextRef::Id(TextId(100 + row));
        ArcadeCategoryPageWidget {
            row: Some(ArcadeCategoryRowId::new(row)),
            widget: view,
        }
    }

    fn layout(widgets: Vec<ArcadeCategoryPageWidget>) -> ArcadeCategoryPageLayout {
        ArcadeCategoryPageLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            widgets,
        }
    }

    #[test]
    fn fabricated_category_page_preserves_order_identity_and_selected_focus() {
        let mut native = menu(ArcadeChallengeBackend::Rust);
        native.select(ArcadeCategoryRowId::new(1)).unwrap();
        let mut heading = Widget::new(WidgetId(30), WidgetKind::Label);
        heading.text = TextRef::Id(TextId(99));
        let page = build_arcade_category_page(
            &native,
            PageId(3404),
            layout(vec![
                ArcadeCategoryPageWidget {
                    row: None,
                    widget: heading,
                },
                row(0, 40),
                row(1, 41),
            ]),
        )
        .unwrap();

        assert_eq!(page.page.widgets[0].text, TextRef::Id(TextId(99)));
        assert_eq!(page.page.initial_focus, Some(WidgetId(41)));
        assert_eq!(
            page.row_widgets(),
            &[
                (ArcadeCategoryRowId::new(0), WidgetId(40)),
                (ArcadeCategoryRowId::new(1), WidgetId(41)),
            ]
        );
        assert_eq!(
            native.entries()[1].category,
            ArcadeCategoryId::new(2),
            "category identity is separate from the chooser row"
        );
    }

    #[test]
    fn fabricated_category_page_rejects_incomplete_reordered_and_legacy_inputs() {
        let native = menu(ArcadeChallengeBackend::Rust);
        assert_eq!(
            build_arcade_category_page(&native, PageId(1), layout(vec![row(1, 41), row(0, 40)])),
            Err(ArcadeCategoryPageBuildError::RowOrderMismatch)
        );
        assert_eq!(
            build_arcade_category_page(&native, PageId(1), layout(vec![row(0, 40)])),
            Err(ArcadeCategoryPageBuildError::MissingRow(
                ArcadeCategoryRowId::new(1)
            ))
        );
        assert_eq!(
            build_arcade_category_page(&native, PageId(1), layout(vec![row(0, 40), row(0, 41)])),
            Err(ArcadeCategoryPageBuildError::DuplicateRow(
                ArcadeCategoryRowId::new(0)
            ))
        );
        assert_eq!(
            build_arcade_category_page(&native, PageId(1), layout(vec![row(0, 40), row(9, 49)])),
            Err(ArcadeCategoryPageBuildError::UnknownRow(
                ArcadeCategoryRowId::new(9)
            ))
        );
        assert_eq!(
            build_arcade_category_page(&native, PageId(1), layout(vec![row(0, 40), row(1, 40)])),
            Err(ArcadeCategoryPageBuildError::DuplicateWidget(WidgetId(40)))
        );
        assert_eq!(
            build_arcade_category_page(
                &menu(ArcadeChallengeBackend::Legacy),
                PageId(1),
                layout(vec![row(0, 40), row(1, 41)]),
            ),
            Err(ArcadeCategoryPageBuildError::LegacyBackend)
        );
    }
}
