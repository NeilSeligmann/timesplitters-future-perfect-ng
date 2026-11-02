//! Native activation attachment for the Rust-owned Arcade category chooser.
//!
//! The adapter preserves the Legacy default. Opted-in rows carry `ArcadeCategoryRowId` through
//! the page action map, then ask an injected Rust effect to open the selected category. No guest
//! address, pad replay, or synthetic input is involved.

use crate::bindings::{
    ActionArg, ActionId, ActionMap, ActivationOutcome, BindResult, ListId, ListView, TextId,
    UiBindings, UiValue, ValueId,
};
use crate::frontend_handlers::FrontendState;
use crate::input::UiEvent;
use tsfp_ui_model::arcade_category_page::{
    ArcadeCategoryPageBuildError, ArcadeCategoryPageLayout, build_arcade_category_page,
};
use tsfp_ui_model::arcade_challenge::{
    ArcadeCategoryId, ArcadeCategoryMenu, ArcadeCategoryRowId, ArcadeCategorySelectionError,
    ArcadeChallengeBackend, SelectedArcadeCategory,
};
use tsfp_ui_model::model::{Page, PageId, WidgetId as ModelWidgetId};
use tsfp_ui_model::{MenuAction, MenuFocus};

/// Failure to bind every typed category row to one concrete page widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeCategoryPageBindingError {
    LegacyBackend,
    DuplicateRow(ArcadeCategoryRowId),
    DuplicateWidget(ModelWidgetId),
    UnknownRow(ArcadeCategoryRowId),
    MissingWidget(ModelWidgetId),
    MissingRow(ArcadeCategoryRowId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeCategoryProjectionError {
    Page(ArcadeCategoryPageBuildError),
    Bindings(ArcadeCategoryPageBindingError),
}

/// Page and typed activation map for an opted-in Arcade category chooser.
#[derive(Clone, Debug)]
pub struct ArcadeCategoryProjection {
    pub page: Page,
    pub actions: ActionMap,
}

/// Result of routing one device-independent event through the chooser page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcadeCategoryEventDispatch {
    pub menu_action: Option<MenuAction>,
    pub binding_result: Option<BindResult>,
    pub activation: ActivationOutcome,
}

/// Build the Rust page and row action map from caller-owned presentation widgets.
pub fn build_arcade_category_projection(
    menu: &ArcadeCategoryMenu,
    page_id: PageId,
    layout: ArcadeCategoryPageLayout,
    action: ActionId,
) -> Result<ArcadeCategoryProjection, ArcadeCategoryProjectionError> {
    let built_page = build_arcade_category_page(menu, page_id, layout)
        .map_err(ArcadeCategoryProjectionError::Page)?;
    let actions =
        arcade_category_action_map(menu, &built_page.page, built_page.row_widgets(), action)
            .map_err(ArcadeCategoryProjectionError::Bindings)?;
    Ok(ArcadeCategoryProjection {
        page: built_page.page,
        actions,
    })
}

/// Bind every visible category row using its source-order typed identity.
pub fn arcade_category_action_map(
    menu: &ArcadeCategoryMenu,
    page: &Page,
    row_widgets: &[(ArcadeCategoryRowId, ModelWidgetId)],
    action: ActionId,
) -> Result<ActionMap, ArcadeCategoryPageBindingError> {
    if menu.backend() == ArcadeChallengeBackend::Legacy {
        return Err(ArcadeCategoryPageBindingError::LegacyBackend);
    }

    let mut seen_rows = Vec::with_capacity(row_widgets.len());
    let mut seen_widgets = Vec::with_capacity(row_widgets.len());
    let mut actions = ActionMap::new();
    for &(row, widget) in row_widgets {
        if seen_rows.contains(&row) {
            return Err(ArcadeCategoryPageBindingError::DuplicateRow(row));
        }
        if seen_widgets.contains(&widget) {
            return Err(ArcadeCategoryPageBindingError::DuplicateWidget(widget));
        }
        if !menu.entries().iter().any(|entry| entry.row == row) {
            return Err(ArcadeCategoryPageBindingError::UnknownRow(row));
        }
        if page.widget(widget).is_none() {
            return Err(ArcadeCategoryPageBindingError::MissingWidget(widget));
        }

        seen_rows.push(row);
        seen_widgets.push(widget);
        actions.bind_activate(
            crate::layout::WidgetId(widget.0),
            action,
            ActionArg::Index(row.get() as usize),
        );
    }

    for entry in menu.entries() {
        if !seen_rows.contains(&entry.row) {
            return Err(ArcadeCategoryPageBindingError::MissingRow(entry.row));
        }
    }
    Ok(actions)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeCategoryRowOutcome {
    LegacyBackend,
    OpenedCategory(SelectedArcadeCategory),
    UnknownRow(ArcadeCategoryRowId),
}

/// Rust effects needed to transition from the chooser into the selected category's page.
pub trait ArcadeCategoryRowEffects {
    type Error;

    fn open_challenge_category(
        &mut self,
        selected: SelectedArcadeCategory,
    ) -> Result<(), Self::Error>;
}

/// Open one typed category. The active frontend category and old detail/setup state commit only
/// after the page effect succeeds. Menu row selection remains available for focus restoration.
pub fn activate_arcade_category_row<E: ArcadeCategoryRowEffects>(
    state: &mut FrontendState,
    menu: &mut ArcadeCategoryMenu,
    row: ArcadeCategoryRowId,
    effects: &mut E,
) -> Result<ArcadeCategoryRowOutcome, E::Error> {
    let selected = match menu.select(row) {
        Ok(selected) => selected,
        Err(ArcadeCategorySelectionError::LegacyBackend) => {
            return Ok(ArcadeCategoryRowOutcome::LegacyBackend);
        }
        Err(ArcadeCategorySelectionError::UnknownRow(row)) => {
            return Ok(ArcadeCategoryRowOutcome::UnknownRow(row));
        }
    };

    effects.open_challenge_category(selected)?;
    state.challenge_category = selected.category;
    state.selected_challenge = None;
    state.challenge_setup = None;
    Ok(ArcadeCategoryRowOutcome::OpenedCategory(selected))
}

/// `UiBindings` adapter for category row activation. Legacy forwards the action unchanged.
pub struct ArcadeCategoryMenuBindings<'a, E: ArcadeCategoryRowEffects> {
    inner: &'a mut dyn UiBindings,
    action: ActionId,
    state: &'a mut FrontendState,
    menu: &'a mut ArcadeCategoryMenu,
    effects: &'a mut E,
}

impl<'a, E: ArcadeCategoryRowEffects> ArcadeCategoryMenuBindings<'a, E> {
    pub fn new(
        inner: &'a mut dyn UiBindings,
        action: ActionId,
        state: &'a mut FrontendState,
        menu: &'a mut ArcadeCategoryMenu,
        effects: &'a mut E,
    ) -> Self {
        Self {
            inner,
            action,
            state,
            menu,
            effects,
        }
    }
}

impl<E: ArcadeCategoryRowEffects> UiBindings for ArcadeCategoryMenuBindings<'_, E> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        if id != self.action || self.menu.backend() == ArcadeChallengeBackend::Legacy {
            return self.inner.action(id, arg);
        }
        let ActionArg::Index(row) = arg else {
            return BindResult::Denied;
        };
        let Ok(row) = u32::try_from(row) else {
            return BindResult::Denied;
        };
        match activate_arcade_category_row(
            self.state,
            self.menu,
            ArcadeCategoryRowId::new(row),
            self.effects,
        ) {
            Ok(ArcadeCategoryRowOutcome::OpenedCategory(_)) => BindResult::Handled,
            Ok(
                ArcadeCategoryRowOutcome::LegacyBackend | ArcadeCategoryRowOutcome::UnknownRow(_),
            )
            | Err(_) => BindResult::Denied,
        }
    }

    fn value(&self, id: ValueId) -> UiValue {
        self.inner.value(id)
    }

    fn set_value(&mut self, id: ValueId, value: UiValue) -> BindResult {
        self.inner.set_value(id, value)
    }

    fn list(&self, id: ListId) -> ListView {
        self.inner.list(id)
    }

    fn text(&self, id: TextId) -> &str {
        self.inner.text(id)
    }
}

/// Route a controller, keyboard, or pointer-derived event through focus and category bindings.
pub fn dispatch_arcade_category_event(
    projection: &ArcadeCategoryProjection,
    focus: &mut MenuFocus,
    event: &UiEvent,
    bindings: &mut dyn UiBindings,
) -> ArcadeCategoryEventDispatch {
    let menu_action = crate::menu_dispatch::dispatch_menu_event(&projection.page, focus, event);
    let activation =
        projection
            .actions
            .dispatch_activation(&projection.page, focus, event, bindings);
    let binding_result = activation.binding_result();
    ArcadeCategoryEventDispatch {
        menu_action,
        binding_result,
        activation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::{RecordedCall, RecordingBindings};
    use crate::input::UiEvent;
    use crate::layout::WidgetId as InputWidgetId;
    use tsfp_ui_model::arcade_category_page::ArcadeCategoryPageWidget;
    use tsfp_ui_model::arcade_challenge::{ArcadeCategorySnapshot, OwnedArcadeChallengeData};
    use tsfp_ui_model::model::{TextId as ModelTextId, TextRef, Widget, WidgetKind};
    use tsfp_ui_model::{NavLayout, Wrap};

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
        let mut view = Widget::new(ModelWidgetId(widget), WidgetKind::Button);
        view.text = TextRef::Id(ModelTextId(100 + row));
        ArcadeCategoryPageWidget {
            row: Some(ArcadeCategoryRowId::new(row)),
            widget: view,
        }
    }

    fn layout() -> ArcadeCategoryPageLayout {
        let mut heading = Widget::new(ModelWidgetId(30), WidgetKind::Label);
        heading.text = TextRef::Id(ModelTextId(99));
        ArcadeCategoryPageLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            widgets: vec![
                ArcadeCategoryPageWidget {
                    row: None,
                    widget: heading,
                },
                row(0, 40),
                row(1, 41),
            ],
        }
    }

    #[derive(Default)]
    struct RecordedEffects {
        opened: Vec<SelectedArcadeCategory>,
        fail: bool,
    }

    impl ArcadeCategoryRowEffects for RecordedEffects {
        type Error = &'static str;

        fn open_challenge_category(
            &mut self,
            selected: SelectedArcadeCategory,
        ) -> Result<(), Self::Error> {
            self.opened.push(selected);
            if self.fail {
                Err("category page unavailable")
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn fabricated_category_activation_routes_page_event_to_owned_state() {
        let action = ActionId(3404);
        let mut menu = menu(ArcadeChallengeBackend::Rust);
        menu.select(ArcadeCategoryRowId::new(1)).unwrap();
        let projection =
            build_arcade_category_projection(&menu, PageId(3404), layout(), action).unwrap();
        let mut state = FrontendState::default();
        state.challenge_category = ArcadeCategoryId::new(99);
        let mut effects = RecordedEffects::default();
        let mut inner = RecordingBindings::default();
        let mut bindings = ArcadeCategoryMenuBindings::new(
            &mut inner,
            action,
            &mut state,
            &mut menu,
            &mut effects,
        );
        let mut focus = MenuFocus::new(&projection.page);

        let focused = dispatch_arcade_category_event(
            &projection,
            &mut focus,
            &UiEvent::Focus(InputWidgetId(41)),
            &mut bindings,
        );
        assert_eq!(focused.binding_result, None);
        let activated = dispatch_arcade_category_event(
            &projection,
            &mut focus,
            &UiEvent::Activate(InputWidgetId(41)),
            &mut bindings,
        );
        assert_eq!(activated.binding_result, Some(BindResult::Handled));
        drop(bindings);

        assert_eq!(state.challenge_category, ArcadeCategoryId::new(2));
        assert_eq!(menu.selected_row(), Some(ArcadeCategoryRowId::new(1)));
        assert_eq!(effects.opened.len(), 1);
        assert_eq!(effects.opened[0].row, ArcadeCategoryRowId::new(1));
        assert_eq!(effects.opened[0].category, ArcadeCategoryId::new(2));
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_failed_category_open_preserves_active_category() {
        let mut state = FrontendState::default();
        state.challenge_category = ArcadeCategoryId::new(99);
        let before = state.clone();
        let mut menu = menu(ArcadeChallengeBackend::Rust);
        let mut effects = RecordedEffects {
            fail: true,
            ..RecordedEffects::default()
        };

        assert_eq!(
            activate_arcade_category_row(
                &mut state,
                &mut menu,
                ArcadeCategoryRowId::new(1),
                &mut effects,
            ),
            Err("category page unavailable")
        );
        assert_eq!(state, before);
        assert_eq!(effects.opened[0].category, ArcadeCategoryId::new(2));
    }

    #[test]
    fn fabricated_legacy_category_binding_forwards_action() {
        let action = ActionId(3404);
        let mut inner = RecordingBindings::default();
        let mut state = FrontendState::default();
        let mut menu = menu(ArcadeChallengeBackend::Legacy);
        let mut effects = RecordedEffects::default();
        let mut bindings = ArcadeCategoryMenuBindings::new(
            &mut inner,
            action,
            &mut state,
            &mut menu,
            &mut effects,
        );

        assert_eq!(
            bindings.action(action, ActionArg::Index(1)),
            BindResult::Handled
        );
        assert_eq!(
            bindings.action(ActionId(5), ActionArg::None),
            BindResult::Handled
        );
        drop(bindings);

        assert_eq!(
            inner.calls,
            [
                RecordedCall::Action(action, ActionArg::Index(1)),
                RecordedCall::Action(ActionId(5), ActionArg::None),
            ]
        );
        assert_eq!(state.challenge_category, ArcadeCategoryId::new(0));
        assert!(effects.opened.is_empty());
    }

    #[test]
    fn fabricated_category_action_map_rejects_incomplete_and_legacy_pages() {
        let native = menu(ArcadeChallengeBackend::Rust);
        let projection = build_arcade_category_projection(
            &native,
            PageId(1),
            ArcadeCategoryPageLayout {
                widgets: vec![row(0, 40)],
                ..layout()
            },
            ActionId(3404),
        );
        assert!(matches!(
            projection,
            Err(ArcadeCategoryProjectionError::Page(
                ArcadeCategoryPageBuildError::MissingRow(row)
            )) if row == ArcadeCategoryRowId::new(1)
        ));

        let legacy = menu(ArcadeChallengeBackend::Legacy);
        assert!(matches!(
            build_arcade_category_projection(&legacy, PageId(1), layout(), ActionId(3404)),
            Err(ArcadeCategoryProjectionError::Page(
                ArcadeCategoryPageBuildError::LegacyBackend
            ))
        ));
    }
}
