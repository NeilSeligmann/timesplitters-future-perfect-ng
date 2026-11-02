//! Native row activation attachment for the T3404 Arcade/Challenge menu model.
//!
//! The default `ArcadeChallengeMenu` backend leaves activation with the existing route. With the
//! explicit Rust backend, typed row identity resolves to an owned selection and an injected page
//! effect. No guest address, pad replay, or synthetic input is used here.

use crate::bindings::{
    ActionArg, ActionId, ActionMap, ActivationOutcome, BindResult, ListId, ListView, TextId,
    UiBindings, UiValue, ValueId,
};
use crate::frontend_handlers::FrontendState;
use crate::input::UiEvent;
use tsfp_ui_model::arcade_challenge::{
    ArcadeChallengeBackend, ArcadeChallengeMenu, CategoryRowId, ChallengeSetupPlan,
    ChallengeSetupServices, MenuHelpId, MenuModeId, MenuParticipantCount, SelectedChallenge,
    SelectionError,
};
use tsfp_ui_model::arcade_menu_session::ArcadeMenuSession;
use tsfp_ui_model::challenge_menu_page::{
    ChallengeMenuPageBuildError, ChallengeMenuPageLayout, build_challenge_menu_page,
};
use tsfp_ui_model::model::{Page, WidgetId as ModelWidgetId};
use tsfp_ui_model::{MenuAction, MenuFocus};

/// Result of routing a row activation through the opted-in Rust menu model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeRowOutcome {
    /// Defer to the existing page and activation route because the model is legacy by default.
    LegacyBackend,
    /// The detail page effect completed and the selected mission is now owned in frontend state.
    OpenedDetail(SelectedChallenge),
    /// A visible row was disabled; its help target is passed to the native help effect.
    Disabled { help: Option<MenuHelpId> },
    /// The row is not part of the currently built category projection.
    UnknownRow(CategoryRowId),
}

/// Failure to connect every Rust menu row to one concrete page widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengePageBindingError {
    /// The Rust page action map is opt-in; Legacy keeps its existing handlers.
    LegacyBackend,
    DuplicateRow(CategoryRowId),
    DuplicateWidget(ModelWidgetId),
    UnknownRow(CategoryRowId),
    MissingWidget(ModelWidgetId),
    MissingRow(CategoryRowId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeMenuProjectionError {
    Page(ChallengeMenuPageBuildError),
    Bindings(ChallengePageBindingError),
}

/// Composed page and activation map for the opt-in Rust Challenge route.
#[derive(Clone, Debug)]
pub struct ChallengeMenuProjection {
    pub page: Page,
    pub actions: ActionMap,
}

/// Result of passing one device-independent event through page focus and row bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeMenuEventDispatch {
    pub menu_action: Option<MenuAction>,
    pub binding_result: Option<BindResult>,
    pub activation: ActivationOutcome,
}

/// Build the UiModel page and its native row action map as one validated projection.
/// Presentation widgets still come from the caller's Rust-owned layout/text source.
pub fn build_challenge_menu_projection(
    menu: &ArcadeChallengeMenu,
    page_id: tsfp_ui_model::PageId,
    layout: ChallengeMenuPageLayout,
    action: ActionId,
) -> Result<ChallengeMenuProjection, ChallengeMenuProjectionError> {
    let built_page = build_challenge_menu_page(menu, page_id, layout)
        .map_err(ChallengeMenuProjectionError::Page)?;
    let actions =
        challenge_menu_action_map(menu, &built_page.page, built_page.row_widgets(), action)
            .map_err(ChallengeMenuProjectionError::Bindings)?;
    Ok(ChallengeMenuProjection {
        page: built_page.page,
        actions,
    })
}

/// Dispatch a device-independent event through the composed Challenge page and its native row
/// bindings. `UiInput` remains the device adapter; controller, keyboard and pointer events use
/// this same `UiEvent` seam. Disabled help is a separate explicit binding, never a stale row
/// activation callback.
pub fn dispatch_challenge_menu_event(
    projection: &ChallengeMenuProjection,
    focus: &mut MenuFocus,
    event: &UiEvent,
    bindings: &mut dyn UiBindings,
) -> ChallengeMenuEventDispatch {
    let menu_action = crate::menu_dispatch::dispatch_menu_event(&projection.page, focus, event);
    let activation =
        projection
            .actions
            .dispatch_activation(&projection.page, focus, event, bindings);
    let binding_result = activation.binding_result();
    ChallengeMenuEventDispatch {
        menu_action,
        binding_result,
        activation,
    }
}

/// Bind the visible rows of an opted-in Challenge model to widgets built by the page layer.
///
/// `row_widgets` carries original category positions. A page builder must preserve those typed
/// IDs when it omits hidden entries; compact display indices are not valid row identities.
/// Disabled rows remain bound so activation can reach their native help effect.
pub fn challenge_menu_action_map(
    menu: &ArcadeChallengeMenu,
    page: &Page,
    row_widgets: &[(CategoryRowId, ModelWidgetId)],
    action: ActionId,
) -> Result<ActionMap, ChallengePageBindingError> {
    if menu.backend() == ArcadeChallengeBackend::Legacy {
        return Err(ChallengePageBindingError::LegacyBackend);
    }

    let mut seen_rows = Vec::with_capacity(row_widgets.len());
    let mut seen_widgets = Vec::with_capacity(row_widgets.len());
    let mut actions = ActionMap::new();
    for &(row, widget) in row_widgets {
        if seen_rows.contains(&row) {
            return Err(ChallengePageBindingError::DuplicateRow(row));
        }
        if seen_widgets.contains(&widget) {
            return Err(ChallengePageBindingError::DuplicateWidget(widget));
        }
        if !menu.entries().iter().any(|entry| entry.row == row) {
            return Err(ChallengePageBindingError::UnknownRow(row));
        }
        if page.widget(widget).is_none() {
            return Err(ChallengePageBindingError::MissingWidget(widget));
        }

        seen_rows.push(row);
        seen_widgets.push(widget);
        actions.bind_activate(
            crate::layout::WidgetId(widget.0),
            action,
            ActionArg::Index(row.get() as usize),
        );
        actions.bind_disabled_help(
            crate::layout::WidgetId(widget.0),
            action,
            ActionArg::Index(row.get() as usize),
        );
    }

    for entry in menu.entries() {
        if !seen_rows.contains(&entry.row) {
            return Err(ChallengePageBindingError::MissingRow(entry.row));
        }
    }
    Ok(actions)
}

/// Narrow page/help effects required by native Challenge row activation.
pub trait ChallengeRowEffects {
    type Error;

    fn open_detail(&mut self, selection: SelectedChallenge) -> Result<(), Self::Error>;
    fn show_help(&mut self, help: Option<MenuHelpId>) -> Result<(), Self::Error>;
}

/// Routes one typed row activation. The frontend's selected mission changes only after the detail
/// effect succeeds. This failure ordering is a native Rust contract, not a claim about original
/// failure paths.
pub fn activate_challenge_row<E: ChallengeRowEffects>(
    state: &mut FrontendState,
    menu: &mut ArcadeChallengeMenu,
    row: CategoryRowId,
    effects: &mut E,
) -> Result<ChallengeRowOutcome, E::Error> {
    let selection = match menu.select(row) {
        Ok(selection) => selection,
        Err(SelectionError::LegacyBackend) => return Ok(ChallengeRowOutcome::LegacyBackend),
        Err(SelectionError::UnknownRow(row)) => {
            return Ok(ChallengeRowOutcome::UnknownRow(row));
        }
        Err(SelectionError::Disabled { help }) => {
            effects.show_help(help)?;
            return Ok(ChallengeRowOutcome::Disabled { help });
        }
    };

    effects.open_detail(selection)?;
    state.selected_challenge = Some(selection);
    state.challenge_setup = None;
    Ok(ChallengeRowOutcome::OpenedDetail(selection))
}

/// `UiBindings` adapter for a Rust-owned Challenge menu action.
///
/// The action argument is the original category row position, even when earlier rows were
/// omitted from the visible menu. With the default Legacy backend the call is forwarded
/// unchanged; the Rust backend resolves the typed row and invokes the injected native effects.
pub struct ChallengeMenuBindings<'a, E: ChallengeRowEffects> {
    inner: &'a mut dyn UiBindings,
    action: ActionId,
    state: &'a mut FrontendState,
    menu: &'a mut ArcadeChallengeMenu,
    effects: &'a mut E,
}

/// `UiBindings` adapter for accepting the currently selected Rust Challenge detail page.
/// The service receives the measured operation sequence; confirmed progress is stored in the
/// owned frontend state, including an explicit partial state if a later operation is rejected.
pub struct ChallengeDetailAcceptBindings<'a, S: ChallengeSetupServices> {
    inner: &'a mut dyn UiBindings,
    action: ActionId,
    backend: ArcadeChallengeBackend,
    mode: MenuModeId,
    participant_count: MenuParticipantCount,
    state: &'a mut FrontendState,
    services: &'a mut S,
}

/// Accept adapter for the session-composed detail page. The selection is read from the active
/// Rust-owned route, then setup progress is retained in `FrontendState` by the shared helper.
pub struct ArcadeSessionDetailAcceptBindings<'a, S: ChallengeSetupServices> {
    inner: &'a mut dyn UiBindings,
    action: ActionId,
    mode: MenuModeId,
    participant_count: MenuParticipantCount,
    session: &'a ArcadeMenuSession,
    state: &'a mut FrontendState,
    services: &'a mut S,
}

impl<'a, S: ChallengeSetupServices> ChallengeDetailAcceptBindings<'a, S> {
    pub fn new(
        inner: &'a mut dyn UiBindings,
        action: ActionId,
        backend: ArcadeChallengeBackend,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        state: &'a mut FrontendState,
        services: &'a mut S,
    ) -> Self {
        Self {
            inner,
            action,
            backend,
            mode,
            participant_count,
            state,
            services,
        }
    }
}

impl<'a, S: ChallengeSetupServices> ArcadeSessionDetailAcceptBindings<'a, S> {
    pub fn new(
        inner: &'a mut dyn UiBindings,
        action: ActionId,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        session: &'a ArcadeMenuSession,
        state: &'a mut FrontendState,
        services: &'a mut S,
    ) -> Self {
        Self {
            inner,
            action,
            mode,
            participant_count,
            session,
            state,
            services,
        }
    }
}

pub(crate) fn apply_detail_accept<S: ChallengeSetupServices + ?Sized>(
    selection: SelectedChallenge,
    mode: MenuModeId,
    participant_count: MenuParticipantCount,
    state: &mut FrontendState,
    services: &mut S,
) -> BindResult {
    let plan = ChallengeSetupPlan::from_selected(selection, mode, participant_count);
    let attempt = match state.challenge_setup.take() {
        Some(previous) if previous.matches_plan(&plan) => plan.resume_tracked(services, previous),
        Some(_) => plan.apply_tracked(services),
        None => plan.apply_tracked(services),
    };
    let complete = attempt.is_complete();
    state.challenge_setup = Some(attempt);
    if complete {
        BindResult::Handled
    } else {
        BindResult::Denied
    }
}

impl<S: ChallengeSetupServices> UiBindings for ChallengeDetailAcceptBindings<'_, S> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        if id != self.action || self.backend == ArcadeChallengeBackend::Legacy {
            return self.inner.action(id, arg);
        }
        if arg != ActionArg::None {
            return BindResult::Denied;
        }
        let Some(selection) = self.state.selected_challenge else {
            return BindResult::Denied;
        };
        apply_detail_accept(
            selection,
            self.mode,
            self.participant_count,
            self.state,
            self.services,
        )
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

impl<S: ChallengeSetupServices> UiBindings for ArcadeSessionDetailAcceptBindings<'_, S> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        if id != self.action || self.session.backend() == ArcadeChallengeBackend::Legacy {
            return self.inner.action(id, arg);
        }
        if arg != ActionArg::None {
            return BindResult::Denied;
        }
        let Some(selection) = self.session.selected_challenge() else {
            return BindResult::Denied;
        };
        self.state.selected_challenge = Some(selection);
        apply_detail_accept(
            selection,
            self.mode,
            self.participant_count,
            self.state,
            self.services,
        )
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

impl<'a, E: ChallengeRowEffects> ChallengeMenuBindings<'a, E> {
    pub fn new(
        inner: &'a mut dyn UiBindings,
        action: ActionId,
        state: &'a mut FrontendState,
        menu: &'a mut ArcadeChallengeMenu,
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

impl<E: ChallengeRowEffects> UiBindings for ChallengeMenuBindings<'_, E> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        if id != self.action
            || self.menu.backend()
                == tsfp_ui_model::arcade_challenge::ArcadeChallengeBackend::Legacy
        {
            return self.inner.action(id, arg);
        }

        let ActionArg::Index(row) = arg else {
            return BindResult::Denied;
        };
        let Ok(row) = u32::try_from(row) else {
            return BindResult::Denied;
        };

        match activate_challenge_row(self.state, self.menu, CategoryRowId::new(row), self.effects) {
            Ok(ChallengeRowOutcome::OpenedDetail(_)) | Ok(ChallengeRowOutcome::Disabled { .. }) => {
                BindResult::Handled
            }
            Ok(ChallengeRowOutcome::LegacyBackend | ChallengeRowOutcome::UnknownRow(_))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::{RecordedCall, RecordingBindings};
    use crate::layout::WidgetId as InputWidgetId;
    use tsfp_ui_model::arcade_challenge::{
        ArcadeCategoryId, ArcadeCategorySnapshot, ArcadeChallengeBackend, CategoryRowData,
        ChallengeDescriptor, ChallengeId, ChallengeRecordSnapshot, ChallengeSetupFailure,
        ChallengeSetupOperation, ChallengeSetupServices, EntryAvailability, MenuModeId,
        MenuParticipantCount, MissionRecordSnapshot, OwnedArcadeChallengeData,
    };
    use tsfp_ui_model::model::{PageId, Widget, WidgetKind};
    use tsfp_ui_model::{NavLayout, Wrap};

    #[derive(Default)]
    struct RecordedSetupServices {
        applied: Vec<ChallengeSetupOperation>,
        fail_at: Option<usize>,
    }

    impl ChallengeSetupServices for RecordedSetupServices {
        fn apply(
            &mut self,
            operation: ChallengeSetupOperation,
        ) -> Result<(), ChallengeSetupFailure> {
            if self.fail_at == Some(self.applied.len()) {
                return Err(ChallengeSetupFailure::ServiceRejected);
            }
            self.applied.push(operation);
            Ok(())
        }
    }

    fn selected_challenge() -> SelectedChallenge {
        SelectedChallenge {
            category: ArcadeCategoryId::new(2),
            row: CategoryRowId::new(7),
            challenge: ChallengeId::new(6),
            detail: ChallengeDescriptor::new(0x523870),
        }
    }

    // FABRICATED category and rows; no guest records or original runtime input are used.
    fn menu(backend: ArcadeChallengeBackend) -> ArcadeChallengeMenu {
        let category = ArcadeCategoryId::new(2);
        let available = MissionRecordSnapshot {
            descriptor_present: true,
            row_data: Some(tsfp_ui_model::arcade_challenge::CategoryRowData {
                hidden: false,
                availability: EntryAvailability::Available,
            }),
        };
        let disabled = MissionRecordSnapshot {
            descriptor_present: true,
            row_data: Some(tsfp_ui_model::arcade_challenge::CategoryRowData {
                hidden: false,
                availability: EntryAvailability::Disabled {
                    help: Some(MenuHelpId::new(99)),
                },
            }),
        };
        let data = OwnedArcadeChallengeData::new(
            vec![ArcadeCategorySnapshot::new(
                category,
                vec![ChallengeId::new(20), ChallengeId::new(21)],
            )],
            vec![
                ChallengeRecordSnapshot::new(ChallengeId::new(20), available),
                ChallengeRecordSnapshot::new(ChallengeId::new(21), disabled),
            ],
        )
        .unwrap();
        ArcadeChallengeMenu::build_from_provider(
            category,
            ChallengeDescriptor::new(0x523870),
            &data,
            backend,
        )
        .unwrap()
    }

    fn sparse_menu(backend: ArcadeChallengeBackend) -> ArcadeChallengeMenu {
        let category = ArcadeCategoryId::new(2);
        let row = |hidden, availability| MissionRecordSnapshot {
            descriptor_present: true,
            row_data: Some(CategoryRowData {
                hidden,
                availability,
            }),
        };
        let data = OwnedArcadeChallengeData::new(
            vec![ArcadeCategorySnapshot::new(
                category,
                vec![
                    ChallengeId::new(20),
                    ChallengeId::new(21),
                    ChallengeId::new(22),
                ],
            )],
            vec![
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(20),
                    row(false, EntryAvailability::Available),
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(21),
                    row(true, EntryAvailability::Available),
                ),
                ChallengeRecordSnapshot::new(
                    ChallengeId::new(22),
                    row(
                        false,
                        EntryAvailability::Disabled {
                            help: Some(MenuHelpId::new(99)),
                        },
                    ),
                ),
            ],
        )
        .unwrap();
        ArcadeChallengeMenu::build_from_provider(
            category,
            ChallengeDescriptor::new(0x523870),
            &data,
            backend,
        )
        .unwrap()
    }

    #[derive(Default)]
    struct RecordedEffects {
        opened: Vec<SelectedChallenge>,
        help: Vec<Option<MenuHelpId>>,
        fail_open: bool,
    }

    impl ChallengeRowEffects for RecordedEffects {
        type Error = &'static str;

        fn open_detail(&mut self, selection: SelectedChallenge) -> Result<(), Self::Error> {
            if self.fail_open {
                return Err("detail page unavailable");
            }
            self.opened.push(selection);
            Ok(())
        }

        fn show_help(&mut self, help: Option<MenuHelpId>) -> Result<(), Self::Error> {
            self.help.push(help);
            Ok(())
        }
    }

    #[test]
    fn fabricated_legacy_backend_leaves_row_activation_to_existing_route() {
        let mut state = FrontendState::default();
        let original = state.clone();
        let mut menu = menu(ArcadeChallengeBackend::Legacy);
        let mut effects = RecordedEffects::default();

        assert_eq!(
            activate_challenge_row(&mut state, &mut menu, CategoryRowId::new(0), &mut effects,),
            Ok(ChallengeRowOutcome::LegacyBackend)
        );
        assert_eq!(state, original);
        assert!(effects.opened.is_empty());
        assert!(effects.help.is_empty());
    }

    #[test]
    fn fabricated_rust_activation_opens_detail_then_commits_typed_selection() {
        let mut state = FrontendState::default();
        state.challenge_category = ArcadeCategoryId::new(2);
        let mut menu = menu(ArcadeChallengeBackend::Rust);
        let mut effects = RecordedEffects::default();
        let outcome =
            activate_challenge_row(&mut state, &mut menu, CategoryRowId::new(0), &mut effects)
                .unwrap();

        let selection = state.selected_challenge.unwrap();
        assert_eq!(outcome, ChallengeRowOutcome::OpenedDetail(selection));
        assert_eq!(selection.category, ArcadeCategoryId::new(2));
        assert_eq!(selection.row, CategoryRowId::new(0));
        assert_eq!(selection.challenge, ChallengeId::new(20));
        assert_eq!(selection.detail, ChallengeDescriptor::new(0x523870));
        assert_eq!(state.challenge_category, ArcadeCategoryId::new(2));
        assert_eq!(effects.opened, [selection]);
        assert!(effects.help.is_empty());
    }

    #[test]
    fn fabricated_ui_binding_routes_typed_row_and_forwards_unrelated_actions() {
        let action = ActionId(3404);
        let mut inner = RecordingBindings::default();
        let mut state = FrontendState::default();
        let mut menu = menu(ArcadeChallengeBackend::Rust);
        let mut effects = RecordedEffects::default();
        let mut bindings =
            ChallengeMenuBindings::new(&mut inner, action, &mut state, &mut menu, &mut effects);

        assert_eq!(
            bindings.action(action, ActionArg::Index(0)),
            BindResult::Handled
        );
        assert_eq!(
            bindings.action(ActionId(7), ActionArg::Int(3)),
            BindResult::Handled
        );
        assert_eq!(effects.opened.len(), 1);
        assert_eq!(state.selected_challenge, effects.opened.first().copied());
        assert_eq!(
            inner.calls,
            vec![RecordedCall::Action(ActionId(7), ActionArg::Int(3))]
        );
    }

    #[test]
    fn fabricated_ui_binding_denies_bad_rows_and_preserves_disabled_help() {
        let action = ActionId(3404);
        let mut inner = RecordingBindings::default();
        let mut state = FrontendState::default();
        let mut menu = menu(ArcadeChallengeBackend::Rust);
        let mut effects = RecordedEffects::default();
        let mut bindings =
            ChallengeMenuBindings::new(&mut inner, action, &mut state, &mut menu, &mut effects);

        assert_eq!(
            bindings.action(action, ActionArg::Index(1)),
            BindResult::Handled
        );
        assert_eq!(
            bindings.action(action, ActionArg::Index(9)),
            BindResult::Denied
        );
        assert_eq!(
            bindings.action(action, ActionArg::Int(1)),
            BindResult::Denied
        );
        drop(bindings);
        assert_eq!(effects.help, vec![Some(MenuHelpId::new(99))]);
        assert!(effects.opened.is_empty());
        assert!(state.selected_challenge.is_none());
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_legacy_ui_binding_forwards_row_action_without_rust_effects() {
        let action = ActionId(3404);
        let mut inner = RecordingBindings::default();
        let mut state = FrontendState::default();
        let mut menu = menu(ArcadeChallengeBackend::Legacy);
        let mut effects = RecordedEffects::default();
        let mut bindings =
            ChallengeMenuBindings::new(&mut inner, action, &mut state, &mut menu, &mut effects);

        assert_eq!(
            bindings.action(action, ActionArg::Index(7)),
            BindResult::Handled
        );
        assert_eq!(
            inner.calls,
            vec![RecordedCall::Action(action, ActionArg::Index(7))]
        );
        assert!(effects.opened.is_empty());
        assert!(effects.help.is_empty());
        assert!(state.selected_challenge.is_none());
    }

    #[test]
    fn fabricated_page_action_map_preserves_sparse_row_identity_end_to_end() {
        let action = ActionId(3404);
        let mut menu = sparse_menu(ArcadeChallengeBackend::Rust);
        let mut page = Page::new(PageId(3404), 1);
        let available_widget = ModelWidgetId(40);
        let disabled_widget = ModelWidgetId(42);
        page.widgets
            .push(Widget::new(available_widget, WidgetKind::Button));
        page.widgets
            .push(Widget::new(disabled_widget, WidgetKind::Button));
        page.widgets[1].enabled = false;
        page.widgets[1].focus_when_disabled = true;
        let actions = challenge_menu_action_map(
            &menu,
            &page,
            &[
                (CategoryRowId::new(0), available_widget),
                (CategoryRowId::new(2), disabled_widget),
            ],
            action,
        )
        .unwrap();
        let projection = ChallengeMenuProjection { page, actions };

        let mut inner = RecordingBindings::default();
        let mut state = FrontendState::default();
        let mut effects = RecordedEffects::default();
        let mut bindings =
            ChallengeMenuBindings::new(&mut inner, action, &mut state, &mut menu, &mut effects);
        let mut focus = MenuFocus::new(&projection.page);
        let disabled = dispatch_challenge_menu_event(
            &projection,
            &mut focus,
            &UiEvent::Activate(InputWidgetId(disabled_widget.0)),
            &mut bindings,
        );
        assert_eq!(disabled.binding_result, Some(BindResult::Handled));
        assert!(matches!(
            disabled.activation,
            ActivationOutcome::Dispatched {
                disabled_help: true,
                ..
            }
        ));
        let available = dispatch_challenge_menu_event(
            &projection,
            &mut focus,
            &UiEvent::Activate(InputWidgetId(available_widget.0)),
            &mut bindings,
        );
        assert_eq!(available.binding_result, Some(BindResult::Handled));
        assert!(matches!(
            available.activation,
            ActivationOutcome::Dispatched {
                disabled_help: false,
                ..
            }
        ));
        drop(bindings);

        assert_eq!(effects.help, [Some(MenuHelpId::new(99))]);
        let selected = state.selected_challenge.unwrap();
        assert_eq!(selected.row, CategoryRowId::new(0));
        assert_eq!(selected.challenge, ChallengeId::new(20));
        assert_eq!(effects.opened, [selected]);
    }

    #[test]
    fn fabricated_page_action_map_rejects_compacted_and_incomplete_rows() {
        let menu = sparse_menu(ArcadeChallengeBackend::Rust);
        let legacy_menu = sparse_menu(ArcadeChallengeBackend::Legacy);
        let mut page = Page::new(PageId(3404), 1);
        page.widgets
            .push(Widget::new(ModelWidgetId(40), WidgetKind::Button));
        page.widgets
            .push(Widget::new(ModelWidgetId(42), WidgetKind::Button));

        assert!(matches!(
            challenge_menu_action_map(
                &menu,
                &page,
                &[
                    (CategoryRowId::new(0), ModelWidgetId(40)),
                    (CategoryRowId::new(1), ModelWidgetId(42)),
                ],
                ActionId(3404),
            ),
            Err(ChallengePageBindingError::UnknownRow(row)) if row == CategoryRowId::new(1)
        ));
        assert!(matches!(
            challenge_menu_action_map(
                &menu,
                &page,
                &[(CategoryRowId::new(0), ModelWidgetId(40))],
                ActionId(3404),
            ),
            Err(ChallengePageBindingError::MissingRow(row)) if row == CategoryRowId::new(2)
        ));
        assert!(matches!(
            challenge_menu_action_map(
                &menu,
                &page,
                &[
                    (CategoryRowId::new(0), ModelWidgetId(40)),
                    (CategoryRowId::new(0), ModelWidgetId(42)),
                    (CategoryRowId::new(2), ModelWidgetId(42)),
                ],
                ActionId(3404),
            ),
            Err(ChallengePageBindingError::DuplicateRow(row)) if row == CategoryRowId::new(0)
        ));
        assert!(matches!(
            challenge_menu_action_map(
                &legacy_menu,
                &page,
                &[
                    (CategoryRowId::new(0), ModelWidgetId(40)),
                    (CategoryRowId::new(2), ModelWidgetId(42)),
                ],
                ActionId(3404),
            ),
            Err(ChallengePageBindingError::LegacyBackend)
        ));
    }

    #[test]
    fn fabricated_composed_page_dispatches_sparse_row_through_native_binding() {
        let mut menu = sparse_menu(ArcadeChallengeBackend::Rust);
        let layout = ChallengeMenuPageLayout {
            widget_type: 1,
            panel_bounds: None,
            nav: NavLayout::Grid(1),
            wrap: Wrap::Clamp,
            widgets: vec![
                tsfp_ui_model::challenge_menu_page::ChallengeMenuPageWidget {
                    row: Some(CategoryRowId::new(0)),
                    widget: Widget::new(ModelWidgetId(40), WidgetKind::Button),
                },
                tsfp_ui_model::challenge_menu_page::ChallengeMenuPageWidget {
                    row: Some(CategoryRowId::new(2)),
                    widget: Widget::new(ModelWidgetId(42), WidgetKind::Button),
                },
            ],
        };
        let projection =
            build_challenge_menu_projection(&menu, PageId(3404), layout, ActionId(3404)).unwrap();
        let mut state = FrontendState::default();
        let mut effects = RecordedEffects::default();
        let mut inner = RecordingBindings::default();
        let mut bindings = ChallengeMenuBindings::new(
            &mut inner,
            ActionId(3404),
            &mut state,
            &mut menu,
            &mut effects,
        );
        let mut focus = MenuFocus::new(&projection.page);

        let focused = dispatch_challenge_menu_event(
            &projection,
            &mut focus,
            &UiEvent::Focus(InputWidgetId(42)),
            &mut bindings,
        );
        assert_eq!(
            focused.menu_action,
            Some(MenuAction::Focus(ModelWidgetId(42)))
        );
        assert_eq!(focused.binding_result, None);

        let disabled = dispatch_challenge_menu_event(
            &projection,
            &mut focus,
            &UiEvent::Activate(InputWidgetId(42)),
            &mut bindings,
        );
        assert!(matches!(
            disabled.menu_action,
            Some(MenuAction::Disabled(ModelWidgetId(42)))
        ));
        assert_eq!(disabled.binding_result, Some(BindResult::Handled));

        let available = dispatch_challenge_menu_event(
            &projection,
            &mut focus,
            &UiEvent::Activate(InputWidgetId(40)),
            &mut bindings,
        );
        assert_eq!(available.binding_result, Some(BindResult::Handled));
        drop(bindings);
        assert_eq!(effects.help, [Some(MenuHelpId::new(99))]);
        let selected = state.selected_challenge.unwrap();
        assert_eq!(selected.row, CategoryRowId::new(0));
        assert_eq!(selected.challenge, ChallengeId::new(20));
        assert_eq!(projection.page.widgets.len(), 2);
    }

    #[test]
    fn fabricated_disabled_unknown_and_failed_rows_do_not_commit_selection() {
        let mut menu = menu(ArcadeChallengeBackend::Rust);
        let mut state = FrontendState::default();
        let previous = SelectedChallenge {
            category: ArcadeCategoryId::new(9),
            row: CategoryRowId::new(5),
            challenge: ChallengeId::new(90),
            detail: ChallengeDescriptor::new(0x523870),
        };
        state.selected_challenge = Some(previous);
        let mut effects = RecordedEffects::default();

        assert_eq!(
            activate_challenge_row(&mut state, &mut menu, CategoryRowId::new(1), &mut effects,),
            Ok(ChallengeRowOutcome::Disabled {
                help: Some(MenuHelpId::new(99))
            })
        );
        assert_eq!(effects.help, [Some(MenuHelpId::new(99))]);
        assert_eq!(state.selected_challenge, Some(previous));
        assert_eq!(
            activate_challenge_row(&mut state, &mut menu, CategoryRowId::new(7), &mut effects,),
            Ok(ChallengeRowOutcome::UnknownRow(CategoryRowId::new(7)))
        );
        assert_eq!(state.selected_challenge, Some(previous));

        effects.fail_open = true;
        assert_eq!(
            activate_challenge_row(&mut state, &mut menu, CategoryRowId::new(0), &mut effects,),
            Err("detail page unavailable")
        );
        assert_eq!(state.selected_challenge, Some(previous));
    }

    #[test]
    fn fabricated_detail_accept_action_applies_plan_and_owns_confirmed_setup() {
        let action = ActionId(3405);
        let widget = InputWidgetId(405);
        let mut actions = ActionMap::new();
        actions.bind_activate(widget, action, ActionArg::None);
        let mut inner = RecordingBindings::default();
        let mut state = FrontendState {
            selected_challenge: Some(selected_challenge()),
            ..FrontendState::default()
        };
        let mut services = RecordedSetupServices::default();
        let mut bindings = ChallengeDetailAcceptBindings::new(
            &mut inner,
            action,
            ArcadeChallengeBackend::Rust,
            MenuModeId::new(7),
            MenuParticipantCount::new(3),
            &mut state,
            &mut services,
        );

        assert_eq!(
            bindings.action(action, ActionArg::None),
            BindResult::Handled
        );
        drop(bindings);
        let attempt = state.challenge_setup.as_ref().unwrap();
        assert!(attempt.is_complete());
        assert_eq!(attempt.failure(), None);
        assert_eq!(attempt.applied_operations(), services.applied.len());
        assert_eq!(attempt.state().selection(), selected_challenge());
        assert!(attempt.state().detail_acceptance_prepared());
        assert!(attempt.state().preview_slots_rebuilt());
        assert_eq!(attempt.state().opened_setup_pages().len(), 2);
        assert_eq!(
            attempt
                .state()
                .special_preview_request()
                .unwrap()
                .model()
                .get(),
            0x385
        );
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_detail_accept_failure_preserves_confirmed_partial_state() {
        let action = ActionId(3405);
        let mut inner = RecordingBindings::default();
        let mut state = FrontendState {
            selected_challenge: Some(selected_challenge()),
            ..FrontendState::default()
        };
        let mut services = RecordedSetupServices {
            fail_at: Some(5),
            ..RecordedSetupServices::default()
        };
        let mut bindings = ChallengeDetailAcceptBindings::new(
            &mut inner,
            action,
            ArcadeChallengeBackend::Rust,
            MenuModeId::new(0),
            MenuParticipantCount::new(2),
            &mut state,
            &mut services,
        );

        assert_eq!(bindings.action(action, ActionArg::None), BindResult::Denied);
        drop(bindings);
        let attempt = state.challenge_setup.as_ref().unwrap();
        assert!(!attempt.is_complete());
        assert_eq!(
            attempt.failure(),
            Some(ChallengeSetupFailure::ServiceRejected)
        );
        assert_eq!(attempt.applied_operations(), 5);
        assert_eq!(services.applied.len(), 5);
        assert_eq!(
            attempt.state().initialized_hidden_slots(),
            &[true, true, false, false]
        );
        assert!(!attempt.state().preview_slots_rebuilt());
        assert!(inner.calls.is_empty());

        let confirmed_prefix = services.applied.clone();
        services.fail_at = None;
        let mut bindings = ChallengeDetailAcceptBindings::new(
            &mut inner,
            action,
            ArcadeChallengeBackend::Rust,
            MenuModeId::new(0),
            MenuParticipantCount::new(2),
            &mut state,
            &mut services,
        );
        assert_eq!(
            bindings.action(action, ActionArg::None),
            BindResult::Handled
        );
        drop(bindings);

        let attempt = state.challenge_setup.as_ref().unwrap();
        assert!(attempt.is_complete());
        assert!(attempt.failure().is_none());
        assert_eq!(attempt.applied_operations(), services.applied.len());
        assert_eq!(
            &services.applied[..confirmed_prefix.len()],
            confirmed_prefix.as_slice()
        );
        let complete_calls = services.applied.len();
        let mut bindings = ChallengeDetailAcceptBindings::new(
            &mut inner,
            action,
            ArcadeChallengeBackend::Rust,
            MenuModeId::new(0),
            MenuParticipantCount::new(2),
            &mut state,
            &mut services,
        );
        assert_eq!(
            bindings.action(action, ActionArg::None),
            BindResult::Handled
        );
        drop(bindings);
        assert_eq!(services.applied.len(), complete_calls);
    }

    #[test]
    fn fabricated_detail_accept_requires_selection_and_keeps_legacy_fallback() {
        let action = ActionId(3405);
        let mut inner = RecordingBindings::default();
        let mut state = FrontendState::default();
        let mut services = RecordedSetupServices::default();
        let mut bindings = ChallengeDetailAcceptBindings::new(
            &mut inner,
            action,
            ArcadeChallengeBackend::Rust,
            MenuModeId::new(0),
            MenuParticipantCount::new(1),
            &mut state,
            &mut services,
        );
        assert_eq!(bindings.action(action, ActionArg::None), BindResult::Denied);
        drop(bindings);
        assert!(services.applied.is_empty());
        assert!(state.challenge_setup.is_none());

        let mut bindings = ChallengeDetailAcceptBindings::new(
            &mut inner,
            action,
            ArcadeChallengeBackend::Legacy,
            MenuModeId::new(0),
            MenuParticipantCount::new(1),
            &mut state,
            &mut services,
        );
        assert_eq!(
            bindings.action(action, ActionArg::Index(6)),
            BindResult::Handled
        );
        drop(bindings);
        assert!(services.applied.is_empty());
        assert_eq!(
            inner.calls,
            [RecordedCall::Action(action, ActionArg::Index(6))]
        );
    }
}
