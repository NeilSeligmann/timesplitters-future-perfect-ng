//! Session-driven Rust page projection for Arcade category, Challenge list, and detail pages.
//!
//! Layout and text references stay caller supplied so this module does not invent original
//! geometry, strings, or widget IDs. The view follows one `ArcadeMenuSession` route and returns
//! the typed selection beside its page for the input layer to attach native actions.

use crate::arcade_category_page::{
    ArcadeCategoryPage, ArcadeCategoryPageBuildError, ArcadeCategoryPageLayout,
    build_arcade_category_page_with_focus,
};
use crate::arcade_challenge::{ArcadeCategoryId, ArcadeChallengeBackend, SelectedChallenge};
use crate::arcade_menu_session::{ArcadeMenuPage, ArcadeMenuSession};
use crate::challenge_menu_page::{
    ChallengeMenuPage, ChallengeMenuPageBuildError, ChallengeMenuPageLayout,
    build_challenge_menu_page_with_focus,
};
use crate::model::{Page, PageId, Widget, WidgetId};
use crate::{focus::NavLayout, focus::Wrap};
use tsfp_ui_renderer::AnchoredRect;

/// Rust-owned presentation inputs for the selected detail page.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcadeDetailPageLayout {
    pub page_id: PageId,
    pub widget_type: u32,
    pub panel_bounds: Option<AnchoredRect>,
    pub nav: NavLayout,
    pub wrap: Wrap,
    pub widgets: Vec<Widget>,
    pub initial_focus: Option<WidgetId>,
}

/// Presentation sources for all route pages. Only the active page is projected.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcadeMenuViewLayout {
    pub category_page_id: PageId,
    pub category: ArcadeCategoryPageLayout,
    pub challenge_page_id: PageId,
    pub challenge: ChallengeMenuPageLayout,
    pub detail: ArcadeDetailPageLayout,
}

/// The page currently selected by the typed Arcade menu session.
#[derive(Clone, Debug, PartialEq)]
pub enum ArcadeMenuView {
    CategoryChooser(ArcadeCategoryPage),
    ChallengeList {
        category: ArcadeCategoryId,
        page: ChallengeMenuPage,
    },
    Detail {
        selection: SelectedChallenge,
        page: Page,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuViewBuildError {
    LegacyBackend,
    MissingChallengeMenu,
    CategoryPage(ArcadeCategoryPageBuildError),
    ChallengePage(ChallengeMenuPageBuildError),
    DuplicateDetailWidget(WidgetId),
    UnknownDetailFocus(WidgetId),
}

/// Project the active session page using Rust-owned presentation inputs.
///
/// Legacy remains the default path. The caller must opt the session into Rust before any of
/// these pages can be built through this native view.
pub fn build_arcade_menu_view(
    session: &ArcadeMenuSession,
    layout: ArcadeMenuViewLayout,
) -> Result<ArcadeMenuView, ArcadeMenuViewBuildError> {
    if session.backend() != ArcadeChallengeBackend::Rust {
        return Err(ArcadeMenuViewBuildError::LegacyBackend);
    }

    match session.page() {
        ArcadeMenuPage::CategoryChooser => build_arcade_category_page_with_focus(
            session.category_menu(),
            layout.category_page_id,
            layout.category,
            session.focused_category_row(),
        )
        .map(ArcadeMenuView::CategoryChooser)
        .map_err(ArcadeMenuViewBuildError::CategoryPage),
        ArcadeMenuPage::ChallengeList(category) => {
            let menu = session
                .challenge_menu()
                .ok_or(ArcadeMenuViewBuildError::MissingChallengeMenu)?;
            let page = build_challenge_menu_page_with_focus(
                menu,
                layout.challenge_page_id,
                layout.challenge,
                session.focused_challenge_row(),
            )
            .map_err(ArcadeMenuViewBuildError::ChallengePage)?;
            Ok(ArcadeMenuView::ChallengeList { category, page })
        }
        ArcadeMenuPage::Detail(selection) => {
            let detail = layout.detail;
            let mut ids = Vec::with_capacity(detail.widgets.len());
            for widget in &detail.widgets {
                if ids.contains(&widget.id) {
                    return Err(ArcadeMenuViewBuildError::DuplicateDetailWidget(widget.id));
                }
                ids.push(widget.id);
            }
            if let Some(focus) = detail.initial_focus
                && !ids.contains(&focus)
            {
                return Err(ArcadeMenuViewBuildError::UnknownDetailFocus(focus));
            }

            let mut page = Page::new(detail.page_id, detail.widget_type);
            page.panel_bounds = detail.panel_bounds;
            page.nav = detail.nav;
            page.wrap = detail.wrap;
            page.widgets = detail.widgets;
            page.initial_focus = detail.initial_focus;
            Ok(ArcadeMenuView::Detail { selection, page })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArcadeDetailPageLayout, ArcadeMenuView, ArcadeMenuViewBuildError, ArcadeMenuViewLayout,
        build_arcade_menu_view,
    };
    use crate::arcade_category_page::{ArcadeCategoryPageLayout, ArcadeCategoryPageWidget};
    use crate::arcade_challenge::{
        ArcadeCategoryId, ArcadeCategoryRowId, ArcadeCategorySnapshot, ArcadeChallengeBackend,
        CategoryRowData, CategoryRowId, ChallengeDescriptor, ChallengeId, ChallengeRecordSnapshot,
        EntryAvailability, MissionRecordSnapshot, OwnedArcadeChallengeData, SelectedChallenge,
    };
    use crate::arcade_menu_session::ArcadeMenuSession;
    use crate::challenge_menu_page::{ChallengeMenuPageLayout, ChallengeMenuPageWidget};
    use crate::focus::{NavLayout, Wrap};
    use crate::model::{PageId, Widget, WidgetId, WidgetKind};

    fn fabricated_session(backend: ArcadeChallengeBackend) -> ArcadeMenuSession {
        let data = OwnedArcadeChallengeData::new(
            vec![ArcadeCategorySnapshot::new(
                ArcadeCategoryId::new(12),
                vec![ChallengeId::new(100)],
            )],
            vec![ChallengeRecordSnapshot::new(
                ChallengeId::new(100),
                MissionRecordSnapshot {
                    descriptor_present: true,
                    row_data: Some(CategoryRowData {
                        hidden: false,
                        availability: EntryAvailability::Available,
                    }),
                },
            )],
        )
        .expect("fabricated catalog has unique identities");
        ArcadeMenuSession::with_backend(data, ChallengeDescriptor::new(0x523870), backend)
    }

    fn button(id: u64) -> Widget {
        Widget::new(WidgetId(id), WidgetKind::Button)
    }

    fn layout(detail_focus: Option<WidgetId>) -> ArcadeMenuViewLayout {
        ArcadeMenuViewLayout {
            category_page_id: PageId(10),
            category: ArcadeCategoryPageLayout {
                widget_type: 1,
                panel_bounds: None,
                nav: NavLayout::Grid(0),
                wrap: Wrap::Clamp,
                widgets: vec![ArcadeCategoryPageWidget {
                    row: Some(ArcadeCategoryRowId::new(0)),
                    widget: button(11),
                }],
            },
            challenge_page_id: PageId(20),
            challenge: ChallengeMenuPageLayout {
                widget_type: 1,
                panel_bounds: None,
                nav: NavLayout::Grid(0),
                wrap: Wrap::Clamp,
                widgets: vec![ChallengeMenuPageWidget {
                    row: Some(CategoryRowId::new(0)),
                    widget: button(21),
                }],
            },
            detail: ArcadeDetailPageLayout {
                page_id: PageId(30),
                widget_type: 3,
                panel_bounds: None,
                nav: NavLayout::Grid(0),
                wrap: Wrap::Clamp,
                widgets: vec![button(31)],
                initial_focus: detail_focus,
            },
        }
    }

    #[test]
    fn fabricated_view_tracks_the_category_list_and_detail_route() {
        let mut session = fabricated_session(ArcadeChallengeBackend::Rust);

        let ArcadeMenuView::CategoryChooser(category) =
            build_arcade_menu_view(&session, layout(None)).unwrap()
        else {
            panic!("category route should project the chooser");
        };
        assert_eq!(category.page.id, PageId(10));
        assert_eq!(
            category.widget_for_row(ArcadeCategoryRowId::new(0)),
            Some(WidgetId(11))
        );

        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let ArcadeMenuView::ChallengeList { category, page } =
            build_arcade_menu_view(&session, layout(None)).unwrap()
        else {
            panic!("list route should project the selected category list");
        };
        assert_eq!(category, ArcadeCategoryId::new(12));
        assert_eq!(page.page.id, PageId(20));
        assert_eq!(
            page.widget_for_row(CategoryRowId::new(0)),
            Some(WidgetId(21))
        );

        let selection = session.open_challenge_row(CategoryRowId::new(0)).unwrap();
        assert_eq!(
            selection,
            SelectedChallenge {
                category: ArcadeCategoryId::new(12),
                row: CategoryRowId::new(0),
                challenge: ChallengeId::new(100),
                detail: ChallengeDescriptor::new(0x523870),
            }
        );
        let ArcadeMenuView::Detail {
            selection: shown,
            page,
        } = build_arcade_menu_view(&session, layout(Some(WidgetId(31)))).unwrap()
        else {
            panic!("detail route should project the selected challenge");
        };
        assert_eq!(shown, selection);
        assert_eq!(page.id, PageId(30));
        assert_eq!(page.start_focus(), Some(WidgetId(31)));
    }

    #[test]
    fn fabricated_view_keeps_legacy_default_and_rejects_invalid_detail_focus() {
        let legacy = fabricated_session(ArcadeChallengeBackend::Legacy);
        assert_eq!(
            build_arcade_menu_view(&legacy, layout(None)),
            Err(ArcadeMenuViewBuildError::LegacyBackend)
        );

        let mut session = fabricated_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        session.open_challenge_row(CategoryRowId::new(0)).unwrap();
        assert_eq!(
            build_arcade_menu_view(&session, layout(Some(WidgetId(99)))),
            Err(ArcadeMenuViewBuildError::UnknownDetailFocus(WidgetId(99)))
        );
    }
}
