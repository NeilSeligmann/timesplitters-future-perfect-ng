//! Rust UI page projection for the typed Arcade/Challenge category model.
//!
//! The caller supplies presentation widgets from a Rust-owned layout and text-table source.
//! This builder validates row identity and assembles the page without guessing geometry,
//! strings, guest addresses, or original widget IDs.

use crate::arcade_challenge::{
    ArcadeChallengeBackend, ArcadeChallengeMenu, CategoryRowId, EntryAvailability,
};
use crate::focus::{NavLayout, Wrap};
use crate::model::{Page, PageId, Widget, WidgetId};
use tsfp_ui_renderer::AnchoredRect;

/// One widget from the page's presentation source. Only category rows carry a row identity;
/// headings and other page widgets use `row: None` and remain in their supplied order.
#[derive(Clone, Debug, PartialEq)]
pub struct ChallengeMenuPageWidget {
    pub row: Option<CategoryRowId>,
    pub widget: Widget,
}

/// Presentation inputs for one category page. Text references and geometry are owned by the
/// supplied widgets so this projection can use the original data tables without embedding them.
#[derive(Clone, Debug, PartialEq)]
pub struct ChallengeMenuPageLayout {
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub widgets: Vec<ChallengeMenuPageWidget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeMenuPageBuildError {
    LegacyBackend,
    DuplicateWidget(WidgetId),
    DuplicateRow(CategoryRowId),
    UnknownRow(CategoryRowId),
    MissingRow(CategoryRowId),
    RowOrderMismatch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChallengeMenuPage {
    pub page: Page,
    row_widgets: Vec<(CategoryRowId, WidgetId)>,
}

impl ChallengeMenuPage {
    pub fn row_widgets(&self) -> &[(CategoryRowId, WidgetId)] {
        &self.row_widgets
    }

    pub fn widget_for_row(&self, row: CategoryRowId) -> Option<WidgetId> {
        self.row_widgets
            .iter()
            .find(|(candidate, _)| *candidate == row)
            .map(|(_, widget)| *widget)
    }
}

/// Build a complete page from caller-owned presentation widgets and the opted-in menu model.
/// The row sequence must match the source projection's visible order; original category row IDs
/// remain attached even when hidden source records left gaps in that sequence.
pub fn build_challenge_menu_page(
    menu: &ArcadeChallengeMenu,
    page_id: PageId,
    layout: ChallengeMenuPageLayout,
) -> Result<ChallengeMenuPage, ChallengeMenuPageBuildError> {
    build_challenge_menu_page_with_focus(menu, page_id, layout, None)
}

/// Build a mission page with an optional identity-reconciled focus row from its Rust session.
///
/// The supplied row takes precedence over the last activated mission. If it is absent from the
/// completed row map, the page uses the existing default focus without guessing a replacement.
pub fn build_challenge_menu_page_with_focus(
    menu: &ArcadeChallengeMenu,
    page_id: PageId,
    layout: ChallengeMenuPageLayout,
    focused_row: Option<CategoryRowId>,
) -> Result<ChallengeMenuPage, ChallengeMenuPageBuildError> {
    if menu.backend() != ArcadeChallengeBackend::Rust {
        return Err(ChallengeMenuPageBuildError::LegacyBackend);
    }

    let mut row_widgets = Vec::with_capacity(menu.entries().len());
    let mut widget_ids = Vec::with_capacity(layout.widgets.len());
    for item in &layout.widgets {
        if widget_ids.contains(&item.widget.id) {
            return Err(ChallengeMenuPageBuildError::DuplicateWidget(item.widget.id));
        }
        widget_ids.push(item.widget.id);

        if let Some(row) = item.row {
            if row_widgets.iter().any(|(candidate, _)| *candidate == row) {
                return Err(ChallengeMenuPageBuildError::DuplicateRow(row));
            }
            if !menu.entries().iter().any(|entry| entry.row == row) {
                return Err(ChallengeMenuPageBuildError::UnknownRow(row));
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
            return Err(ChallengeMenuPageBuildError::MissingRow(entry.row));
        }
    }
    if row_widgets
        .iter()
        .map(|(row, _)| *row)
        .ne(menu.entries().iter().map(|entry| entry.row))
    {
        return Err(ChallengeMenuPageBuildError::RowOrderMismatch);
    }

    let mut page = Page::new(page_id, layout.widget_type);
    page.panel_bounds = layout.panel_bounds;
    page.nav = layout.nav;
    page.wrap = layout.wrap;
    page.initial_focus = focused_row
        .or_else(|| menu.selected_row())
        .and_then(|row| row_widgets.iter().find(|(candidate, _)| *candidate == row))
        .map(|(_, widget)| *widget);
    page.widgets = layout
        .widgets
        .into_iter()
        .map(|item| {
            let mut widget = item.widget;
            if item.row.is_some_and(|row| {
                menu.entries().iter().any(|entry| {
                    entry.row == row
                        && matches!(entry.availability, EntryAvailability::Disabled { .. })
                })
            }) {
                // The original grey flag blocks activation but does not remove a row from
                // focus traversal. Keep the presentation disabled while allowing the typed
                // session handler to route activation to its help action.
                widget.enabled = false;
                widget.focus_when_disabled = true;
            }
            widget
        })
        .collect();

    Ok(ChallengeMenuPage { page, row_widgets })
}

#[cfg(test)]
mod tests {
    use super::{
        ChallengeMenuPageBuildError, ChallengeMenuPageLayout, ChallengeMenuPageWidget,
        build_challenge_menu_page, build_challenge_menu_page_with_focus,
    };
    use crate::arcade_challenge::{
        ArcadeCategoryId, ArcadeCategorySnapshot, ArcadeChallengeBackend, ArcadeChallengeMenu,
        CategoryRowData, ChallengeDescriptor, ChallengeId, ChallengeRecordSnapshot,
        EntryAvailability, MenuHelpId, MissionRecordSnapshot, OwnedArcadeChallengeData,
    };
    use crate::focus::{Dir, NavLayout, Wrap};
    use crate::menu_input::{MenuAction, MenuFocus, MenuInput};
    use crate::model::{PageId, TextId, TextRef, Widget, WidgetId, WidgetKind};
    use tsfp_ui_renderer::Vec2;

    fn menu(backend: ArcadeChallengeBackend) -> ArcadeChallengeMenu {
        let category = ArcadeCategoryId::new(4);
        let records: Vec<_> = [
            (10, None),
            (
                11,
                Some(CategoryRowData {
                    hidden: false,
                    availability: EntryAvailability::Available,
                }),
            ),
            (
                12,
                Some(CategoryRowData {
                    hidden: false,
                    availability: EntryAvailability::Disabled {
                        help: Some(MenuHelpId::new(80)),
                    },
                }),
            ),
        ]
        .into_iter()
        .map(|(id, row_data)| {
            ChallengeRecordSnapshot::new(
                ChallengeId::new(id),
                MissionRecordSnapshot {
                    descriptor_present: true,
                    row_data,
                },
            )
        })
        .collect();
        let provider = OwnedArcadeChallengeData::new(
            vec![ArcadeCategorySnapshot::new(
                category,
                vec![
                    ChallengeId::new(10),
                    ChallengeId::new(11),
                    ChallengeId::new(12),
                ],
            )],
            records,
        )
        .unwrap();
        ArcadeChallengeMenu::build_from_provider(
            category,
            ChallengeDescriptor::new(0x523870),
            &provider,
            backend,
        )
        .unwrap()
    }

    fn row(row: u32, widget: u64) -> ChallengeMenuPageWidget {
        let mut view = Widget::new(WidgetId(widget), WidgetKind::Button);
        view.text = TextRef::Id(TextId(100 + row));
        view.rect.offset = Vec2::new(12.0, 24.0 + row as f32 * 20.0);
        ChallengeMenuPageWidget {
            row: Some(crate::arcade_challenge::CategoryRowId::new(row)),
            widget: view,
        }
    }

    fn layout(widgets: Vec<ChallengeMenuPageWidget>) -> ChallengeMenuPageLayout {
        ChallengeMenuPageLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            widgets,
        }
    }

    #[test]
    fn fabricated_page_builder_preserves_sparse_rows_and_selected_focus() {
        let mut rust_menu = menu(ArcadeChallengeBackend::Rust);
        rust_menu
            .select(crate::arcade_challenge::CategoryRowId::new(1))
            .unwrap();
        let mut heading = Widget::new(WidgetId(30), WidgetKind::Label);
        heading.text = TextRef::Id(TextId(99));
        let built = build_challenge_menu_page(
            &rust_menu,
            PageId(3404),
            layout(vec![
                ChallengeMenuPageWidget {
                    row: None,
                    widget: heading,
                },
                row(1, 41),
                row(2, 42),
            ]),
        )
        .unwrap();

        assert_eq!(built.page.widgets.len(), 3);
        assert_eq!(built.page.widgets[0].text, TextRef::Id(TextId(99)));
        assert_eq!(built.page.widgets[1].text, TextRef::Id(TextId(101)));
        assert_eq!(built.page.initial_focus, Some(WidgetId(41)));
        assert_eq!(
            built.row_widgets(),
            &[
                (crate::arcade_challenge::CategoryRowId::new(1), WidgetId(41)),
                (crate::arcade_challenge::CategoryRowId::new(2), WidgetId(42)),
            ]
        );
        assert_eq!(
            built.widget_for_row(crate::arcade_challenge::CategoryRowId::new(2)),
            Some(WidgetId(42))
        );
    }

    #[test]
    fn fabricated_disabled_row_remains_focusable_without_becoming_enabled() {
        let mut rust_menu = menu(ArcadeChallengeBackend::Rust);
        rust_menu
            .select(crate::arcade_challenge::CategoryRowId::new(1))
            .unwrap();
        let mut disabled = row(2, 42);
        disabled.widget.enabled = false;

        let built =
            build_challenge_menu_page(&rust_menu, PageId(3404), layout(vec![row(1, 41), disabled]))
                .unwrap();
        let disabled_widget = built.page.widget(WidgetId(42)).unwrap();
        assert!(!disabled_widget.enabled);
        assert!(disabled_widget.focus_when_disabled);

        let mut focus = MenuFocus::new(&built.page);
        assert_eq!(focus.selected, Some(WidgetId(41)));
        assert_eq!(
            focus.input(&built.page, MenuInput::Move(Dir::Down)),
            Some(MenuAction::Focus(WidgetId(42)))
        );
        assert_eq!(
            focus.input(&built.page, MenuInput::Accept),
            Some(MenuAction::Disabled(WidgetId(42)))
        );
    }

    #[test]
    fn fabricated_directional_skip_preserves_stored_focus_but_skips_as_a_target() {
        let mut rust_menu = menu(ArcadeChallengeBackend::Rust);
        rust_menu
            .select(crate::arcade_challenge::CategoryRowId::new(1))
            .unwrap();
        let mut skipped_disabled = row(2, 42);
        skipped_disabled.widget.enabled = false;
        skipped_disabled.widget.directional_skip = true;
        let mut trailing = Widget::new(WidgetId(43), WidgetKind::Button);
        trailing.text = TextRef::Id(TextId(999));
        let built = build_challenge_menu_page_with_focus(
            &rust_menu,
            PageId(3404),
            layout(vec![
                row(1, 41),
                skipped_disabled,
                ChallengeMenuPageWidget {
                    row: None,
                    widget: trailing,
                },
            ]),
            Some(crate::arcade_challenge::CategoryRowId::new(2)),
        )
        .unwrap();

        let stored = built.page.widget(WidgetId(42)).unwrap();
        assert!(!stored.enabled);
        assert!(stored.focus_when_disabled);
        assert!(stored.directional_skip);
        let mut focus = MenuFocus::new(&built.page);
        assert_eq!(focus.selected, Some(WidgetId(42)));
        assert_eq!(
            focus.input(&built.page, MenuInput::Move(Dir::Down)),
            Some(MenuAction::Focus(WidgetId(43)))
        );

        focus.selected = Some(WidgetId(41));
        assert_eq!(
            focus.input(&built.page, MenuInput::Move(Dir::Down)),
            Some(MenuAction::Focus(WidgetId(43)))
        );
    }

    #[test]
    fn fabricated_page_builder_rejects_wrong_backend_rows_and_widgets() {
        let rust_menu = menu(ArcadeChallengeBackend::Rust);
        assert_eq!(
            build_challenge_menu_page(&rust_menu, PageId(1), layout(vec![row(2, 42), row(1, 41)]),),
            Err(ChallengeMenuPageBuildError::RowOrderMismatch)
        );
        assert_eq!(
            build_challenge_menu_page(&rust_menu, PageId(1), layout(vec![row(1, 41)])),
            Err(ChallengeMenuPageBuildError::MissingRow(
                crate::arcade_challenge::CategoryRowId::new(2)
            ))
        );
        assert_eq!(
            build_challenge_menu_page(&rust_menu, PageId(1), layout(vec![row(1, 41), row(2, 41)]),),
            Err(ChallengeMenuPageBuildError::DuplicateWidget(WidgetId(41)))
        );
        assert_eq!(
            build_challenge_menu_page(&rust_menu, PageId(1), layout(vec![row(1, 41), row(1, 42)]),),
            Err(ChallengeMenuPageBuildError::DuplicateRow(
                crate::arcade_challenge::CategoryRowId::new(1)
            ))
        );
        assert_eq!(
            build_challenge_menu_page(&rust_menu, PageId(1), layout(vec![row(1, 41), row(9, 42)]),),
            Err(ChallengeMenuPageBuildError::UnknownRow(
                crate::arcade_challenge::CategoryRowId::new(9)
            ))
        );
        assert_eq!(
            build_challenge_menu_page(
                &menu(ArcadeChallengeBackend::Legacy),
                PageId(1),
                layout(vec![row(1, 41), row(2, 42)]),
            ),
            Err(ChallengeMenuPageBuildError::LegacyBackend)
        );
    }
}
