//! Native input attachment for the Rust-owned Arcade category-to-detail session.
//!
//! A session page supplies its typed row map; this adapter routes the shared row action according
//! to the active Rust page and commits route changes only after the page effect succeeds. Legacy
//! remains the default and forwards through the existing `UiBindings` implementation.

use crate::bindings::{
    ActionArg, ActionId, ActionMap, ActivationOutcome, BindResult, ListId, ListView, TextId,
    UiBindings, UiValue, ValueId,
};
use crate::frontend_handlers::FrontendState;
use crate::input::UiEvent;
use crate::menu_dispatch::dispatch_menu_event;
use tsfp_ui_model::arcade_challenge::{
    ArcadeCategoryRowId, ArcadeChallengeBackend, ArcadeChallengeDataProvider, CategoryRowId,
    ChallengeDescriptor, ChallengeDetailChoices, ChallengeSetupServices, MenuHelpId, MenuModeId,
    MenuParticipantCount, OwnedArcadeChallengeData, OwnedChallengeDataProviderError,
    SelectedArcadeCategory, SelectedChallenge, SelectionError,
};
use tsfp_ui_model::arcade_menu_session::{
    ArcadeMenuDetailAcceptResult, ArcadeMenuDetailControlId, ArcadeMenuPage,
    ArcadeMenuRefreshError, ArcadeMenuSession,
};
use tsfp_ui_model::arcade_menu_view::{
    ArcadeMenuView, ArcadeMenuViewBuildError, ArcadeMenuViewLayout, build_arcade_menu_view,
};
use tsfp_ui_model::model::{Page, WidgetId as ModelWidgetId, WidgetRole};
use tsfp_ui_model::{MenuAction, MenuFocus};

/// Failure while binding the active typed route to its page widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuSessionActionMapError {
    Category(crate::arcade_category::ArcadeCategoryPageBindingError),
    Challenge(crate::challenge_menu::ChallengePageBindingError),
    MissingChallengeMenu,
    MissingDetailAcceptWidget,
    MissingDetailChoiceWidget(u64),
    DuplicateDetailChoiceControl(u64),
    DuplicateDetailChoiceWidget(u64),
    DuplicateDetailChoiceAction(u32),
    ConflictingDetailChoiceAction(u32),
    ConflictingDetailChoiceWidget(u64),
}

/// Failure while composing the active session page and its native action map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuSessionProjectionError {
    View(ArcadeMenuViewBuildError),
    ActionMap(ArcadeMenuSessionActionMapError),
}

/// Identity for one mounted Rust page projection.
///
/// UI loops can retain this beside a rendered page and submit it with queued events. The session
/// lineage distinguishes independent runtimes; a catalog or layout rebuild changes the local
/// revision, so events from another session or an earlier widget map are rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcadeMenuPageRevision {
    session_id: tsfp_ui_model::arcade_menu_session::ArcadeMenuSessionId,
    projection_revision: u64,
}

/// A detail-choice update produced for an older Rust page projection or rejected by the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuDetailChoicesError {
    StalePage,
    Session(tsfp_ui_model::arcade_menu_session::ArcadeMenuSessionError),
}

/// Event captured while a particular Rust page projection was mounted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcadeMenuQueuedEvent {
    page_revision: ArcadeMenuPageRevision,
    event: UiEvent,
}

/// Failure while refreshing an owned catalog and rebuilding the active native page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuSessionRefreshError<E> {
    Session(ArcadeMenuRefreshError),
    Effects(E),
}

/// Failure while copying provider data or applying it to the active session and page host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuSessionProviderRefreshError<E> {
    Provider(OwnedChallengeDataProviderError),
    Refresh(ArcadeMenuSessionRefreshError<E>),
}

/// Active Rust page plus the row actions attached to that page.
#[derive(Clone, Debug)]
pub struct ArcadeMenuSessionProjection {
    pub view: ArcadeMenuView,
    pub actions: ActionMap,
    session_id: tsfp_ui_model::arcade_menu_session::ArcadeMenuSessionId,
    route_revision: u64,
    projection_revision: u64,
}

/// One caller-supplied detail choice attached to a Rust page widget.
///
/// REA has not established the original controls' values or defaults. Callers must supply each
/// visible option explicitly; this type does not claim that any supplied values are original.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcadeMenuDetailChoiceOption {
    pub control: ArcadeMenuDetailControlId,
    pub widget: ModelWidgetId,
    pub action: ActionId,
    pub choices: ChallengeDetailChoices,
}

impl ArcadeMenuSessionProjection {
    pub const fn page_revision(&self) -> ArcadeMenuPageRevision {
        ArcadeMenuPageRevision {
            session_id: self.session_id,
            projection_revision: self.projection_revision,
        }
    }

    /// Attach the page identity to an event before placing it in a UI event queue.
    pub fn queue_event(&self, event: UiEvent) -> ArcadeMenuQueuedEvent {
        ArcadeMenuQueuedEvent {
            page_revision: self.page_revision(),
            event,
        }
    }

    pub fn page(&self) -> &Page {
        match &self.view {
            ArcadeMenuView::CategoryChooser(page) => &page.page,
            ArcadeMenuView::ChallengeList { page, .. } => &page.page,
            ArcadeMenuView::Detail { page, .. } => page,
        }
    }
}

/// Compose the session's active page and bind every visible typed row to one action.
pub fn build_arcade_menu_session_projection(
    session: &mut ArcadeMenuSession,
    layout: ArcadeMenuViewLayout,
    row_action: ActionId,
) -> Result<ArcadeMenuSessionProjection, ArcadeMenuSessionProjectionError> {
    build_arcade_menu_session_projection_inner(session, layout, row_action, None, &[])
}

/// Compose the active session page and attach a native action to each detail Accept widget.
///
/// The action argument is `None`; its binding must resolve the current selection from the
/// Rust-owned session. Other pages keep their ordinary row actions.
pub fn build_arcade_menu_session_projection_with_detail_accept(
    session: &mut ArcadeMenuSession,
    layout: ArcadeMenuViewLayout,
    row_action: ActionId,
    detail_accept_action: ActionId,
) -> Result<ArcadeMenuSessionProjection, ArcadeMenuSessionProjectionError> {
    build_arcade_menu_session_projection_inner(
        session,
        layout,
        row_action,
        Some(detail_accept_action),
        &[],
    )
}

fn build_arcade_menu_session_projection_inner(
    session: &mut ArcadeMenuSession,
    layout: ArcadeMenuViewLayout,
    row_action: ActionId,
    detail_accept_action: Option<ActionId>,
    detail_choice_options: &[ArcadeMenuDetailChoiceOption],
) -> Result<ArcadeMenuSessionProjection, ArcadeMenuSessionProjectionError> {
    let mut layout = layout;
    if let Some(control) = session.focused_detail_control()
        && let Some(option) = detail_choice_options
            .iter()
            .find(|option| option.control == control)
        && layout
            .detail
            .widgets
            .iter()
            .any(|widget| widget.id == option.widget)
    {
        layout.detail.initial_focus = Some(option.widget);
    }
    let view =
        build_arcade_menu_view(session, layout).map_err(ArcadeMenuSessionProjectionError::View)?;
    let actions = match &view {
        ArcadeMenuView::CategoryChooser(page) => {
            crate::arcade_category::arcade_category_action_map(
                session.category_menu(),
                &page.page,
                page.row_widgets(),
                row_action,
            )
            .map_err(|error| {
                ArcadeMenuSessionProjectionError::ActionMap(
                    ArcadeMenuSessionActionMapError::Category(error),
                )
            })?
        }
        ArcadeMenuView::ChallengeList { page, .. } => {
            let menu = session.challenge_menu().ok_or_else(|| {
                ArcadeMenuSessionProjectionError::ActionMap(
                    ArcadeMenuSessionActionMapError::MissingChallengeMenu,
                )
            })?;
            crate::challenge_menu::challenge_menu_action_map(
                menu,
                &page.page,
                page.row_widgets(),
                row_action,
            )
            .map_err(|error| {
                ArcadeMenuSessionProjectionError::ActionMap(
                    ArcadeMenuSessionActionMapError::Challenge(error),
                )
            })?
        }
        ArcadeMenuView::Detail { page, .. } => {
            let mut actions = ActionMap::new();
            validate_detail_choice_options(
                detail_choice_options,
                &page.widgets,
                row_action,
                detail_accept_action,
            )
            .map_err(ArcadeMenuSessionProjectionError::ActionMap)?;
            if let Some(action) = detail_accept_action {
                let mut bound = false;
                for widget in page
                    .widgets
                    .iter()
                    .filter(|widget| widget.role == WidgetRole::Accept)
                {
                    actions.bind_activate(
                        crate::layout::WidgetId(widget.id.0),
                        action,
                        ActionArg::None,
                    );
                    bound = true;
                }
                if !bound {
                    return Err(ArcadeMenuSessionProjectionError::ActionMap(
                        ArcadeMenuSessionActionMapError::MissingDetailAcceptWidget,
                    ));
                }
            }
            for option in detail_choice_options {
                actions.bind_activate(
                    crate::layout::WidgetId(option.widget.0),
                    option.action,
                    ActionArg::None,
                );
            }
            actions
        }
    };
    let projection_revision = session.begin_projection();
    Ok(ArcadeMenuSessionProjection {
        view,
        actions,
        session_id: session.id(),
        route_revision: session.route_revision(),
        projection_revision,
    })
}

fn validate_detail_choice_options(
    options: &[ArcadeMenuDetailChoiceOption],
    widgets: &[tsfp_ui_model::model::Widget],
    row_action: ActionId,
    detail_accept_action: Option<ActionId>,
) -> Result<(), ArcadeMenuSessionActionMapError> {
    for (index, option) in options.iter().enumerate() {
        if option.action == row_action || Some(option.action) == detail_accept_action {
            return Err(
                ArcadeMenuSessionActionMapError::ConflictingDetailChoiceAction(option.action.0),
            );
        }
        if options[..index]
            .iter()
            .any(|prior| prior.action == option.action)
        {
            return Err(
                ArcadeMenuSessionActionMapError::DuplicateDetailChoiceAction(option.action.0),
            );
        }
        if options[..index]
            .iter()
            .any(|prior| prior.control == option.control)
        {
            return Err(
                ArcadeMenuSessionActionMapError::DuplicateDetailChoiceControl(option.control.get()),
            );
        }
        if options[..index]
            .iter()
            .any(|prior| prior.widget == option.widget)
        {
            return Err(
                ArcadeMenuSessionActionMapError::DuplicateDetailChoiceWidget(option.widget.0),
            );
        }
        let Some(widget) = widgets.iter().find(|widget| widget.id == option.widget) else {
            return Err(ArcadeMenuSessionActionMapError::MissingDetailChoiceWidget(
                option.widget.0,
            ));
        };
        if detail_accept_action.is_some() && widget.role == WidgetRole::Accept {
            return Err(
                ArcadeMenuSessionActionMapError::ConflictingDetailChoiceWidget(option.widget.0),
            );
        }
    }
    Ok(())
}

/// Result of routing one device-independent event through the active session page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArcadeMenuSessionEventDispatch {
    pub menu_action: Option<MenuAction>,
    pub binding_result: Option<BindResult>,
    pub activation: ActivationOutcome,
    /// The Rust category chooser reached its menu boundary; the owning menu stack decides what
    /// closes. No page-provided Back handler is invoked for an opted-in Rust session.
    pub close_requested: bool,
}

/// Route focus and activation through the composed page and typed row action map.
pub fn dispatch_arcade_menu_session_event<E: ArcadeMenuSessionEffects>(
    projection: &ArcadeMenuSessionProjection,
    focus: &mut MenuFocus,
    event: &UiEvent,
    bindings: &mut ArcadeMenuSessionBindings<'_, E>,
) -> ArcadeMenuSessionEventDispatch {
    if projection.session_id != bindings.session.id()
        || projection.route_revision != bindings.route_revision()
        || projection.projection_revision != bindings.projection_revision()
    {
        return ArcadeMenuSessionEventDispatch {
            menu_action: None,
            binding_result: Some(BindResult::Denied),
            activation: ActivationOutcome::NotRequested,
            close_requested: false,
        };
    }

    if bindings.session.backend() == ArcadeChallengeBackend::Rust && matches!(event, UiEvent::Back)
    {
        let root_close = matches!(bindings.session.page(), ArcadeMenuPage::CategoryChooser);
        let binding_result = bindings.back();
        return ArcadeMenuSessionEventDispatch {
            // Back routing belongs to the typed session. Returning the layout's handler here
            // could let a caller replay a legacy callback after the native transition.
            menu_action: None,
            close_requested: root_close && binding_result.is_none(),
            binding_result,
            activation: ActivationOutcome::NotRequested,
        };
    }

    let menu_action = dispatch_menu_event(projection.page(), focus, event);
    if let Some(widget) = menu_action.as_ref().and_then(|action| match action {
        MenuAction::Focus(widget)
        | MenuAction::Activate { widget, .. }
        | MenuAction::Disabled(widget) => Some(*widget),
        MenuAction::Back(_) => None,
    }) {
        bindings.record_row_focus(&projection.view, widget);
    }
    let activation =
        projection
            .actions
            .dispatch_activation(projection.page(), focus, event, bindings);
    let binding_result = match event {
        UiEvent::Back => bindings.back(),
        UiEvent::Activate(_) => activation.binding_result(),
        _ => None,
    };
    ArcadeMenuSessionEventDispatch {
        menu_action,
        binding_result,
        activation,
        close_requested: false,
    }
}

/// Native effects used to expose the next page or disabled-row help to the Rust UI.
pub trait ArcadeMenuSessionEffects {
    type Error;

    /// Handle a candidate catalog refresh before the session commit.
    ///
    /// Page-owning effects rebuild and replace their projection only after that build succeeds.
    /// Effects without a stored projection must explicitly implement an intentional no-op.
    fn refresh_projection(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
    ) -> Result<(), Self::Error>;

    fn open_challenge_list(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
        selection: SelectedArcadeCategory,
    ) -> Result<(), Self::Error>;

    fn open_detail(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
        selection: SelectedChallenge,
    ) -> Result<(), Self::Error>;

    fn show_help(&mut self, help: Option<MenuHelpId>) -> Result<(), Self::Error>;

    fn return_to_challenge_list(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
        selection: SelectedChallenge,
    ) -> Result<(), Self::Error>;

    fn return_to_category_chooser(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
        selection: SelectedArcadeCategory,
    ) -> Result<(), Self::Error>;
}

/// Reports why a Rust-owned Arcade page host could not mount or replace its active projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuPageHostError<E> {
    LegacyBackend,
    Projection(ArcadeMenuSessionProjectionError),
    Help(E),
}

/// Failure while copying a provider and mounting its first native Arcade page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeMenuRuntimeMountError<E> {
    Provider(OwnedChallengeDataProviderError),
    PageHost(ArcadeMenuPageHostError<E>),
}

/// Rust-owned destination for the active session view and typed row actions.
///
/// Layout and help handling are supplied by the caller. The host only mounts after an explicit
/// Rust backend opt-in, and builds each replacement projection from the adapter's candidate
/// session before that candidate is committed. A failed view build therefore leaves both the
/// active session and this page host on their previous projection.
pub struct ArcadeMenuPageHost<H: ArcadeMenuSessionHelp> {
    layout: ArcadeMenuViewLayout,
    row_action: ActionId,
    detail_accept_action: Option<ActionId>,
    detail_choice_options: Vec<ArcadeMenuDetailChoiceOption>,
    projection: Option<ArcadeMenuSessionProjection>,
    help: H,
}

/// Rust-owned lifetime boundary for one Arcade menu session and its active page projection.
///
/// The runtime keeps the widget focus page-local: after a route transition it initializes focus
/// from the new projection, whose initial widget was resolved from the session's stable entry
/// identity. Legacy remains the default because mounting still requires a Rust session.
pub struct ArcadeMenuSessionRuntime<H: ArcadeMenuSessionHelp> {
    session: ArcadeMenuSession,
    pages: ArcadeMenuPageHost<H>,
    focus: MenuFocus,
}

struct RuntimeDetailAccept<'a> {
    choices: RuntimeDetailChoices,
    services: &'a mut dyn ChallengeSetupServices,
}

#[derive(Clone, Copy)]
enum RuntimeDetailChoices {
    SuppliedAtAccept(ChallengeDetailChoices),
    SessionOwned,
}

/// Host callback for displaying disabled-row help without emulating controller input.
pub trait ArcadeMenuSessionHelp {
    type Error;

    fn show_help(&mut self, help: Option<MenuHelpId>) -> Result<(), Self::Error>;
}

impl<H: ArcadeMenuSessionHelp> ArcadeMenuPageHost<H> {
    pub fn new(
        layout: ArcadeMenuViewLayout,
        row_action: ActionId,
        detail_accept_action: Option<ActionId>,
        help: H,
    ) -> Self {
        Self {
            layout,
            row_action,
            detail_accept_action,
            detail_choice_options: Vec::new(),
            projection: None,
            help,
        }
    }

    /// Attach caller-supplied typed options to detail widgets.
    ///
    /// Option values remain explicit because REA has not established the original control
    /// domains or defaults. The page host rejects duplicate widgets/actions and action collisions.
    pub fn with_detail_choice_options(
        mut self,
        options: Vec<ArcadeMenuDetailChoiceOption>,
    ) -> Result<Self, ArcadeMenuSessionActionMapError> {
        validate_detail_choice_options(
            &options,
            &self.layout.detail.widgets,
            self.row_action,
            self.detail_accept_action,
        )?;
        self.detail_choice_options = options;
        Ok(self)
    }

    /// Build the initial page projection only when the owning session opted into Rust.
    pub fn mount(
        &mut self,
        session: &mut ArcadeMenuSession,
    ) -> Result<(), ArcadeMenuPageHostError<H::Error>> {
        if session.backend() != ArcadeChallengeBackend::Rust {
            return Err(ArcadeMenuPageHostError::LegacyBackend);
        }
        self.replace_projection(session)
    }

    pub fn projection(&self) -> Option<&ArcadeMenuSessionProjection> {
        self.projection.as_ref()
    }

    /// Replace the Rust-owned presentation inputs and rebuild the active page as one commit.
    ///
    /// The candidate session receives a new projection revision only if the new layout builds
    /// successfully. On failure, this host, its current projection, and the caller's session are
    /// unchanged. Layout widget identities are supplied by the Rust presentation owner; this
    /// method does not infer original widget mappings.
    pub fn replace_layout(
        &mut self,
        session: &mut ArcadeMenuSession,
        layout: ArcadeMenuViewLayout,
    ) -> Result<(), ArcadeMenuPageHostError<H::Error>> {
        if session.backend() != ArcadeChallengeBackend::Rust {
            return Err(ArcadeMenuPageHostError::LegacyBackend);
        }

        let mut prepared_session = session.clone();
        let projection = build_arcade_menu_session_projection_inner(
            &mut prepared_session,
            layout.clone(),
            self.row_action,
            self.detail_accept_action,
            &self.detail_choice_options,
        )
        .map_err(ArcadeMenuPageHostError::Projection)?;

        *session = prepared_session;
        self.layout = layout;
        self.projection = Some(projection);
        Ok(())
    }

    fn replace_projection(
        &mut self,
        session: &mut ArcadeMenuSession,
    ) -> Result<(), ArcadeMenuPageHostError<H::Error>> {
        let projection = build_arcade_menu_session_projection_inner(
            session,
            self.layout.clone(),
            self.row_action,
            self.detail_accept_action,
            &self.detail_choice_options,
        )
        .map_err(ArcadeMenuPageHostError::Projection)?;
        self.projection = Some(projection);
        Ok(())
    }
}

impl<H: ArcadeMenuSessionHelp> ArcadeMenuSessionRuntime<H> {
    /// Copy provider data into owned session state and mount its initial Rust page.
    ///
    /// The backend choice is explicit so a caller cannot accidentally enable the native
    /// projection while adapting existing Legacy menu data.
    pub fn mount_from_provider(
        provider: &impl ArcadeChallengeDataProvider,
        detail: ChallengeDescriptor,
        backend: ArcadeChallengeBackend,
        pages: ArcadeMenuPageHost<H>,
    ) -> Result<Self, ArcadeMenuRuntimeMountError<H::Error>> {
        let session = ArcadeMenuSession::with_backend_from_provider(provider, detail, backend)
            .map_err(ArcadeMenuRuntimeMountError::Provider)?;
        Self::mount(session, pages).map_err(ArcadeMenuRuntimeMountError::PageHost)
    }

    /// Mount a page host around an explicitly opted-in session and own its initial focus.
    pub fn mount(
        mut session: ArcadeMenuSession,
        mut pages: ArcadeMenuPageHost<H>,
    ) -> Result<Self, ArcadeMenuPageHostError<H::Error>> {
        pages.mount(&mut session)?;
        let focus = MenuFocus::new(
            pages
                .projection()
                .expect("mount installs the initial page projection")
                .page(),
        );
        Ok(Self {
            session,
            pages,
            focus,
        })
    }

    pub fn session(&self) -> &ArcadeMenuSession {
        &self.session
    }

    pub fn projection(&self) -> &ArcadeMenuSessionProjection {
        self.pages
            .projection()
            .expect("a mounted runtime always owns an active projection")
    }

    pub const fn focus(&self) -> &MenuFocus {
        &self.focus
    }

    /// Store typed detail-page choices in the session before its Accept action is queued.
    pub fn set_detail_choices(
        &mut self,
        choices: ChallengeDetailChoices,
    ) -> Result<(), tsfp_ui_model::arcade_menu_session::ArcadeMenuSessionError> {
        self.session.set_detail_choices(choices)
    }

    /// Apply a detail-choice update only while the page that produced it is still mounted.
    ///
    /// Native detail widgets should retain the revision from their current projection with the
    /// input they enqueue. A stale event cannot write choices into a different Challenge detail
    /// page after navigation or a layout/catalog rebuild.
    pub fn set_detail_choices_for_page_revision(
        &mut self,
        revision: ArcadeMenuPageRevision,
        choices: ChallengeDetailChoices,
    ) -> Result<(), ArcadeMenuDetailChoicesError> {
        if revision != self.page_revision() {
            return Err(ArcadeMenuDetailChoicesError::StalePage);
        }
        self.session
            .set_detail_choices(choices)
            .map_err(ArcadeMenuDetailChoicesError::Session)
    }

    /// Revision to retain with events produced from the currently mounted page.
    pub fn page_revision(&self) -> ArcadeMenuPageRevision {
        self.projection().page_revision()
    }

    /// Replace Rust-owned page layouts and rebuild the active projection transactionally.
    ///
    /// On success, page-local widget focus is resolved again from the session's typed entry
    /// identity. A rejected layout leaves the active layout, projection, session, and focus
    /// unchanged.
    pub fn replace_layout(
        &mut self,
        layout: ArcadeMenuViewLayout,
    ) -> Result<(), ArcadeMenuPageHostError<H::Error>> {
        self.pages.replace_layout(&mut self.session, layout)?;
        self.focus = MenuFocus::new(self.projection().page());
        Ok(())
    }

    /// Replace the owned catalog and rebuild the active page as one runtime commit.
    ///
    /// A successful refresh also resets page-local widget focus from the retained typed row
    /// identity. If session reconciliation or page projection fails, the old session, page, and
    /// focus remain active. Call the `_with_frontend_state` variant when maintaining the legacy
    /// compatibility mirror alongside this runtime.
    pub fn refresh_catalog(
        &mut self,
        data: OwnedArcadeChallengeData,
    ) -> Result<(), ArcadeMenuSessionRefreshError<ArcadeMenuPageHostError<H::Error>>> {
        self.refresh_catalog_inner(data, None)
    }

    /// Refresh the runtime and synchronize an attached compatibility state mirror on success.
    pub fn refresh_catalog_with_frontend_state(
        &mut self,
        data: OwnedArcadeChallengeData,
        frontend_state: &mut FrontendState,
    ) -> Result<(), ArcadeMenuSessionRefreshError<ArcadeMenuPageHostError<H::Error>>> {
        self.refresh_catalog_inner(data, Some(frontend_state))
    }

    fn refresh_catalog_inner(
        &mut self,
        data: OwnedArcadeChallengeData,
        frontend_state: Option<&mut FrontendState>,
    ) -> Result<(), ArcadeMenuSessionRefreshError<ArcadeMenuPageHostError<H::Error>>> {
        let mut prepared = self.session.clone();
        prepared
            .refresh_catalog(data)
            .map_err(ArcadeMenuSessionRefreshError::Session)?;
        self.pages
            .refresh_projection(&mut prepared)
            .map_err(ArcadeMenuSessionRefreshError::Effects)?;

        self.session = prepared;
        self.focus = MenuFocus::new(self.projection().page());
        if let Some(state) = frontend_state {
            sync_frontend_state(state, self.session.selected_challenge(), &self.session);
        }
        Ok(())
    }

    /// Copy a Rust-owned provider into the active runtime and rebuild its page transactionally.
    pub fn refresh_from_provider(
        &mut self,
        provider: &impl ArcadeChallengeDataProvider,
    ) -> Result<(), ArcadeMenuSessionProviderRefreshError<ArcadeMenuPageHostError<H::Error>>> {
        self.refresh_from_provider_inner(provider, None)
    }

    /// Provider refresh that also updates the optional frontend compatibility mirror.
    pub fn refresh_from_provider_with_frontend_state(
        &mut self,
        provider: &impl ArcadeChallengeDataProvider,
        frontend_state: &mut FrontendState,
    ) -> Result<(), ArcadeMenuSessionProviderRefreshError<ArcadeMenuPageHostError<H::Error>>> {
        self.refresh_from_provider_inner(provider, Some(frontend_state))
    }

    fn refresh_from_provider_inner(
        &mut self,
        provider: &impl ArcadeChallengeDataProvider,
        frontend_state: Option<&mut FrontendState>,
    ) -> Result<(), ArcadeMenuSessionProviderRefreshError<ArcadeMenuPageHostError<H::Error>>> {
        if self.session.backend() == ArcadeChallengeBackend::Legacy {
            return Err(ArcadeMenuSessionProviderRefreshError::Refresh(
                ArcadeMenuSessionRefreshError::Session(ArcadeMenuRefreshError::LegacyBackend),
            ));
        }
        let data = OwnedArcadeChallengeData::copy_from_provider(provider)
            .map_err(ArcadeMenuSessionProviderRefreshError::Provider)?;
        self.refresh_catalog_inner(data, frontend_state)
            .map_err(ArcadeMenuSessionProviderRefreshError::Refresh)
    }

    /// Dispatch a native page event. A configured detail-accept action is denied until its
    /// Rust setup services are supplied with `dispatch_with_detail_accept`.
    pub fn dispatch(
        &mut self,
        inner: &mut dyn UiBindings,
        event: &UiEvent,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(inner, event, None, None, None)
    }

    /// Dispatch an event only if it was produced from the current page projection.
    ///
    /// This is the safe entry point for event queues that can outlive a catalog or layout refresh.
    pub fn dispatch_for_page_revision(
        &mut self,
        revision: ArcadeMenuPageRevision,
        inner: &mut dyn UiBindings,
        event: &UiEvent,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(inner, event, None, None, Some(revision))
    }

    /// Dispatch a queued event with the page identity captured by its projection.
    pub fn dispatch_queued(
        &mut self,
        inner: &mut dyn UiBindings,
        queued: ArcadeMenuQueuedEvent,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_for_page_revision(queued.page_revision, inner, &queued.event)
    }

    /// Dispatch a native page event and synchronize the optional compatibility state mirror.
    pub fn dispatch_with_frontend_state(
        &mut self,
        inner: &mut dyn UiBindings,
        event: &UiEvent,
        frontend_state: &mut FrontendState,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(inner, event, None, Some(frontend_state), None)
    }

    /// Dispatch an event with the injected native services for a configured detail-accept action.
    pub fn dispatch_with_detail_accept(
        &mut self,
        inner: &mut dyn UiBindings,
        event: &UiEvent,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        services: &mut dyn ChallengeSetupServices,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(
            inner,
            event,
            Some(RuntimeDetailAccept {
                choices: RuntimeDetailChoices::SuppliedAtAccept(ChallengeDetailChoices::new(
                    mode,
                    participant_count,
                )),
                services,
            }),
            None,
            None,
        )
    }

    /// Dispatch a revision-checked event with the native detail setup services attached.
    pub fn dispatch_with_detail_accept_for_page_revision(
        &mut self,
        revision: ArcadeMenuPageRevision,
        inner: &mut dyn UiBindings,
        event: &UiEvent,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        services: &mut dyn ChallengeSetupServices,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(
            inner,
            event,
            Some(RuntimeDetailAccept {
                choices: RuntimeDetailChoices::SuppliedAtAccept(ChallengeDetailChoices::new(
                    mode,
                    participant_count,
                )),
                services,
            }),
            None,
            Some(revision),
        )
    }

    /// Dispatch a queued event with native detail setup services attached.
    pub fn dispatch_queued_with_detail_accept(
        &mut self,
        inner: &mut dyn UiBindings,
        queued: ArcadeMenuQueuedEvent,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        services: &mut dyn ChallengeSetupServices,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_with_detail_accept_for_page_revision(
            queued.page_revision,
            inner,
            &queued.event,
            mode,
            participant_count,
            services,
        )
    }

    /// Dispatch a queued detail event with native setup services and synchronize the optional
    /// compatibility mirror on the same revision-checked path.
    pub fn dispatch_queued_with_detail_accept_and_frontend_state(
        &mut self,
        inner: &mut dyn UiBindings,
        queued: ArcadeMenuQueuedEvent,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        services: &mut dyn ChallengeSetupServices,
        frontend_state: &mut FrontendState,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(
            inner,
            &queued.event,
            Some(RuntimeDetailAccept {
                choices: RuntimeDetailChoices::SuppliedAtAccept(ChallengeDetailChoices::new(
                    mode,
                    participant_count,
                )),
                services,
            }),
            Some(frontend_state),
            Some(queued.page_revision),
        )
    }

    /// Dispatch detail setup through native services and synchronize the compatibility mirror.
    pub fn dispatch_with_detail_accept_and_frontend_state(
        &mut self,
        inner: &mut dyn UiBindings,
        event: &UiEvent,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        services: &mut dyn ChallengeSetupServices,
        frontend_state: &mut FrontendState,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(
            inner,
            event,
            Some(RuntimeDetailAccept {
                choices: RuntimeDetailChoices::SuppliedAtAccept(ChallengeDetailChoices::new(
                    mode,
                    participant_count,
                )),
                services,
            }),
            Some(frontend_state),
            None,
        )
    }

    /// Dispatch the detail Accept action using choices already held by the Rust session.
    pub fn dispatch_with_configured_detail_accept(
        &mut self,
        inner: &mut dyn UiBindings,
        event: &UiEvent,
        services: &mut dyn ChallengeSetupServices,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(
            inner,
            event,
            Some(RuntimeDetailAccept {
                choices: RuntimeDetailChoices::SessionOwned,
                services,
            }),
            None,
            None,
        )
    }

    /// Revision-checked queued Accept path using choices already held by the Rust session.
    pub fn dispatch_queued_with_configured_detail_accept(
        &mut self,
        inner: &mut dyn UiBindings,
        queued: ArcadeMenuQueuedEvent,
        services: &mut dyn ChallengeSetupServices,
    ) -> ArcadeMenuSessionEventDispatch {
        self.dispatch_inner(
            inner,
            &queued.event,
            Some(RuntimeDetailAccept {
                choices: RuntimeDetailChoices::SessionOwned,
                services,
            }),
            None,
            Some(queued.page_revision),
        )
    }

    fn dispatch_inner(
        &mut self,
        inner: &mut dyn UiBindings,
        event: &UiEvent,
        detail_accept: Option<RuntimeDetailAccept<'_>>,
        frontend_state: Option<&mut FrontendState>,
        expected_page_revision: Option<ArcadeMenuPageRevision>,
    ) -> ArcadeMenuSessionEventDispatch {
        if expected_page_revision.is_some_and(|revision| revision != self.page_revision()) {
            return ArcadeMenuSessionEventDispatch {
                menu_action: None,
                binding_result: Some(BindResult::Denied),
                activation: ActivationOutcome::NotRequested,
                close_requested: false,
            };
        }
        let Some(projection) = self.pages.projection().cloned() else {
            return ArcadeMenuSessionEventDispatch {
                menu_action: None,
                binding_result: Some(BindResult::Denied),
                activation: ActivationOutcome::NotRequested,
                close_requested: false,
            };
        };
        let prior_projection_revision = projection.projection_revision;
        let row_action = self.pages.row_action;
        let detail_accept_action = self.pages.detail_accept_action;
        let detail_choice_options = self.pages.detail_choice_options.clone();
        let mut bindings =
            ArcadeMenuSessionBindings::new(inner, row_action, &mut self.session, &mut self.pages);
        bindings = bindings.with_detail_choice_options(detail_choice_options);
        if let Some(state) = frontend_state {
            bindings = bindings.with_frontend_state(state);
        }
        if let Some(action) = detail_accept_action {
            bindings = match detail_accept {
                Some(accept) => match accept.choices {
                    RuntimeDetailChoices::SuppliedAtAccept(choices) => bindings.with_detail_accept(
                        action,
                        choices.mode(),
                        choices.participant_count(),
                        accept.services,
                    ),
                    RuntimeDetailChoices::SessionOwned => {
                        bindings.with_configured_detail_accept(action, accept.services)
                    }
                },
                None => bindings.deny_detail_accept_forwarding(action),
            };
        }
        let dispatch =
            dispatch_arcade_menu_session_event(&projection, &mut self.focus, event, &mut bindings);
        drop(bindings);

        if let Some(current) = self.pages.projection()
            && current.projection_revision != prior_projection_revision
        {
            self.focus = MenuFocus::new(current.page());
        }
        dispatch
    }
}

impl<H: ArcadeMenuSessionHelp> ArcadeMenuSessionEffects for ArcadeMenuPageHost<H> {
    type Error = ArcadeMenuPageHostError<H::Error>;

    fn refresh_projection(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
    ) -> Result<(), Self::Error> {
        self.replace_projection(prepared_session)
    }

    fn open_challenge_list(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
        _selection: SelectedArcadeCategory,
    ) -> Result<(), Self::Error> {
        self.replace_projection(prepared_session)
    }

    fn open_detail(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
        _selection: SelectedChallenge,
    ) -> Result<(), Self::Error> {
        self.replace_projection(prepared_session)
    }

    fn show_help(&mut self, help: Option<MenuHelpId>) -> Result<(), Self::Error> {
        self.help
            .show_help(help)
            .map_err(ArcadeMenuPageHostError::Help)
    }

    fn return_to_challenge_list(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
        _selection: SelectedChallenge,
    ) -> Result<(), Self::Error> {
        self.replace_projection(prepared_session)
    }

    fn return_to_category_chooser(
        &mut self,
        prepared_session: &mut ArcadeMenuSession,
        _selection: SelectedArcadeCategory,
    ) -> Result<(), Self::Error> {
        self.replace_projection(prepared_session)
    }
}

/// `UiBindings` adapter for the row action shared by chooser and mission-list pages.
pub struct ArcadeMenuSessionBindings<'a, E: ArcadeMenuSessionEffects> {
    inner: &'a mut dyn UiBindings,
    row_action: ActionId,
    session: &'a mut ArcadeMenuSession,
    effects: &'a mut E,
    frontend_state: Option<&'a mut FrontendState>,
    detail_accept: Option<SessionDetailAccept<'a>>,
    blocked_detail_accept: Option<ActionId>,
    detail_choice_options: Vec<ArcadeMenuDetailChoiceOption>,
}

struct SessionDetailAccept<'a> {
    action: ActionId,
    choices: RuntimeDetailChoices,
    services: &'a mut dyn ChallengeSetupServices,
}

impl<'a, E: ArcadeMenuSessionEffects> ArcadeMenuSessionBindings<'a, E> {
    pub fn new(
        inner: &'a mut dyn UiBindings,
        row_action: ActionId,
        session: &'a mut ArcadeMenuSession,
        effects: &'a mut E,
    ) -> Self {
        Self {
            inner,
            row_action,
            session,
            effects,
            frontend_state: None,
            detail_accept: None,
            blocked_detail_accept: None,
            detail_choice_options: Vec::new(),
        }
    }

    fn with_detail_choice_options(mut self, options: Vec<ArcadeMenuDetailChoiceOption>) -> Self {
        self.detail_choice_options = options;
        self
    }

    /// Attach the frontend-owned Challenge identity and setup progress to the same route commit.
    ///
    /// This keeps the composed session and the detail-accept adapter's state coherent. Callers
    /// that only need page routing may keep using `new`; the complete native entry path should
    /// use this attachment.
    pub fn with_frontend_state(mut self, state: &'a mut FrontendState) -> Self {
        self.frontend_state = Some(state);
        self
    }

    /// Route detail acceptance through the active session and its attached Rust-owned state.
    ///
    /// The composed binding owns the mutable session already, so a separate accept adapter could
    /// not borrow the selected entry at the same time. Legacy sessions continue forwarding this
    /// action to `inner`.
    pub fn with_detail_accept(
        mut self,
        action: ActionId,
        mode: MenuModeId,
        participant_count: MenuParticipantCount,
        services: &'a mut dyn ChallengeSetupServices,
    ) -> Self {
        self.detail_accept = Some(SessionDetailAccept {
            action,
            choices: RuntimeDetailChoices::SuppliedAtAccept(ChallengeDetailChoices::new(
                mode,
                participant_count,
            )),
            services,
        });
        self
    }

    /// Route detail acceptance through choices already owned by the active session.
    pub fn with_configured_detail_accept(
        mut self,
        action: ActionId,
        services: &'a mut dyn ChallengeSetupServices,
    ) -> Self {
        self.detail_accept = Some(SessionDetailAccept {
            action,
            choices: RuntimeDetailChoices::SessionOwned,
            services,
        });
        self
    }

    fn deny_detail_accept_forwarding(mut self, action: ActionId) -> Self {
        self.blocked_detail_accept = Some(action);
        self
    }

    /// Replace the session's owned catalog and keep its attached frontend selection coherent.
    ///
    /// A rejected refresh leaves both records untouched. A successful refresh clears setup
    /// progress when the typed selection changes or no detail selection is active; a matching
    /// detail selection retains its acknowledged progress.
    pub fn refresh_catalog(
        &mut self,
        data: OwnedArcadeChallengeData,
    ) -> Result<(), ArcadeMenuSessionRefreshError<E::Error>> {
        let mut prepared = self.session.clone();
        prepared
            .refresh_catalog(data)
            .map_err(ArcadeMenuSessionRefreshError::Session)?;
        self.effects
            .refresh_projection(&mut prepared)
            .map_err(ArcadeMenuSessionRefreshError::Effects)?;
        *self.session = prepared;
        self.sync_frontend_state(self.session.selected_challenge());
        Ok(())
    }

    /// Snapshot a provider into session-owned data and synchronize the attached frontend state.
    ///
    /// Provider-copy and identity reconciliation both happen before frontend synchronization, so
    /// a rejected replacement leaves the session and attached records unchanged.
    pub fn refresh_from_provider(
        &mut self,
        provider: &impl ArcadeChallengeDataProvider,
    ) -> Result<(), ArcadeMenuSessionProviderRefreshError<E::Error>> {
        if self.session.backend() == ArcadeChallengeBackend::Legacy {
            return Err(ArcadeMenuSessionProviderRefreshError::Refresh(
                ArcadeMenuSessionRefreshError::Session(ArcadeMenuRefreshError::LegacyBackend),
            ));
        }
        let data = OwnedArcadeChallengeData::copy_from_provider(provider)
            .map_err(ArcadeMenuSessionProviderRefreshError::Provider)?;
        self.refresh_catalog(data)
            .map_err(ArcadeMenuSessionProviderRefreshError::Refresh)
    }

    fn sync_frontend_state(&mut self, selection: Option<SelectedChallenge>) {
        if let Some(state) = self.frontend_state.as_deref_mut() {
            sync_frontend_state(state, selection, self.session);
        }
    }

    fn route_revision(&self) -> u64 {
        self.session.route_revision()
    }

    fn projection_revision(&self) -> u64 {
        self.session.projection_revision()
    }

    fn record_row_focus(&mut self, view: &ArcadeMenuView, widget: ModelWidgetId) {
        match view {
            ArcadeMenuView::CategoryChooser(page) => {
                if let Some((row, _)) = page
                    .row_widgets()
                    .iter()
                    .find(|(_, candidate_widget)| *candidate_widget == widget)
                {
                    self.session.focus_category_row(*row);
                }
            }
            ArcadeMenuView::ChallengeList { page, .. } => {
                if let Some((row, _)) = page
                    .row_widgets()
                    .iter()
                    .find(|(_, candidate_widget)| *candidate_widget == widget)
                {
                    self.session.focus_challenge_row(*row);
                }
            }
            ArcadeMenuView::Detail { .. } => {
                if let Some(option) = self
                    .detail_choice_options
                    .iter()
                    .find(|option| option.widget == widget)
                {
                    self.session.focus_detail_control(option.control);
                }
            }
        }
    }

    fn activate_row(&mut self, row: u32) -> BindResult {
        let mut prepared = self.session.clone();
        match self.session.page() {
            ArcadeMenuPage::CategoryChooser => {
                let row = ArcadeCategoryRowId::new(row);
                let Ok(selection) = prepared.open_category_row(row) else {
                    return BindResult::Denied;
                };
                if self
                    .effects
                    .open_challenge_list(&mut prepared, selection)
                    .is_err()
                {
                    return BindResult::Denied;
                }
                *self.session = prepared;
                self.sync_frontend_state(None);
                BindResult::Handled
            }
            ArcadeMenuPage::ChallengeList(_) => {
                let row = CategoryRowId::new(row);
                match prepared.open_challenge_row(row) {
                    Ok(selection) => {
                        if self
                            .effects
                            .open_detail(&mut prepared, selection)
                            .is_err()
                        {
                            return BindResult::Denied;
                        }
                        *self.session = prepared;
                        self.sync_frontend_state(Some(selection));
                        BindResult::Handled
                    }
                    Err(tsfp_ui_model::arcade_menu_session::ArcadeMenuSessionError::ChallengeSelection(
                        SelectionError::Disabled { help },
                    )) => match self.effects.show_help(help) {
                        Ok(()) => BindResult::Handled,
                        Err(_) => BindResult::Denied,
                    },
                    Err(_) => BindResult::Denied,
                }
            }
            ArcadeMenuPage::Detail(_) => BindResult::Ignored,
        }
    }

    fn back(&mut self) -> Option<BindResult> {
        if self.session.backend() == ArcadeChallengeBackend::Legacy {
            return None;
        }
        let mut prepared = self.session.clone();
        match self.session.page() {
            ArcadeMenuPage::Detail(selection) => {
                if prepared.back_to_challenge_list().is_err()
                    || self
                        .effects
                        .return_to_challenge_list(&mut prepared, selection)
                        .is_err()
                {
                    return Some(BindResult::Denied);
                }
                *self.session = prepared;
                self.sync_frontend_state(None);
                Some(BindResult::Handled)
            }
            ArcadeMenuPage::ChallengeList(_) => {
                let Some(row) = self.session.category_menu().selected_row() else {
                    return Some(BindResult::Denied);
                };
                let Some(entry) = self
                    .session
                    .category_menu()
                    .entries()
                    .iter()
                    .find(|entry| entry.row == row)
                else {
                    return Some(BindResult::Denied);
                };
                let selection = SelectedArcadeCategory {
                    row,
                    category: entry.category,
                };
                if prepared.back_to_category_chooser().is_err()
                    || self
                        .effects
                        .return_to_category_chooser(&mut prepared, selection)
                        .is_err()
                {
                    return Some(BindResult::Denied);
                }
                *self.session = prepared;
                self.sync_frontend_state(None);
                Some(BindResult::Handled)
            }
            ArcadeMenuPage::CategoryChooser => None,
        }
    }
}

fn sync_frontend_state(
    state: &mut FrontendState,
    selection: Option<SelectedChallenge>,
    session: &ArcadeMenuSession,
) {
    state.selected_challenge = selection;
    state.challenge_setup = session.setup_attempt().cloned();
    let category = match session.page() {
        ArcadeMenuPage::CategoryChooser => None,
        ArcadeMenuPage::ChallengeList(category) => Some(category),
        ArcadeMenuPage::Detail(selection) => Some(selection.category),
    };
    if let Some(category) = category {
        state.challenge_category = category;
    }
}

impl<E: ArcadeMenuSessionEffects> UiBindings for ArcadeMenuSessionBindings<'_, E> {
    fn action(&mut self, id: ActionId, arg: ActionArg) -> BindResult {
        if let Some(option) = self
            .detail_choice_options
            .iter()
            .find(|option| option.action == id)
        {
            if self.session.backend() == ArcadeChallengeBackend::Legacy {
                return self.inner.action(id, arg);
            }
            if id == self.row_action
                || self.blocked_detail_accept == Some(id)
                || self
                    .detail_accept
                    .as_ref()
                    .is_some_and(|accept| accept.action == id)
                || arg != ActionArg::None
                || self.session.set_detail_choices(option.choices).is_err()
            {
                return BindResult::Denied;
            }
            self.sync_frontend_state(self.session.selected_challenge());
            return BindResult::Handled;
        }

        if id == self.row_action {
            if self.session.backend() == ArcadeChallengeBackend::Legacy {
                return self.inner.action(id, arg);
            }
            let ActionArg::Index(row) = arg else {
                return BindResult::Denied;
            };
            let Ok(row) = u32::try_from(row) else {
                return BindResult::Denied;
            };
            return self.activate_row(row);
        }

        let Some(accept) = self
            .detail_accept
            .as_mut()
            .filter(|accept| accept.action == id)
        else {
            if self.session.backend() == ArcadeChallengeBackend::Rust
                && self.blocked_detail_accept == Some(id)
            {
                return BindResult::Denied;
            }
            return self.inner.action(id, arg);
        };
        if self.session.backend() == ArcadeChallengeBackend::Legacy {
            return self.inner.action(id, arg);
        }
        if id == self.row_action || arg != ActionArg::None {
            return BindResult::Denied;
        }
        let result = match accept.choices {
            RuntimeDetailChoices::SuppliedAtAccept(choices) => self.session.accept_detail(
                choices.mode(),
                choices.participant_count(),
                accept.services,
            ),
            RuntimeDetailChoices::SessionOwned => {
                self.session.accept_configured_detail(accept.services)
            }
        };
        self.sync_frontend_state(self.session.selected_challenge());
        match result {
            ArcadeMenuDetailAcceptResult::SetupComplete => BindResult::Handled,
            ArcadeMenuDetailAcceptResult::SetupPending(_)
            | ArcadeMenuDetailAcceptResult::BackendDisabled
            | ArcadeMenuDetailAcceptResult::NotOnDetail
            | ArcadeMenuDetailAcceptResult::ChoicesNotConfigured => BindResult::Denied,
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
    use std::{cell::RefCell, rc::Rc};
    use tsfp_ui_model::arcade_category_page::{ArcadeCategoryPageLayout, ArcadeCategoryPageWidget};
    use tsfp_ui_model::arcade_challenge::{
        ArcadeCategoryId, ArcadeCategorySnapshot, ArcadeChallengeCatalog, CategoryRowData,
        ChallengeDescriptor, ChallengeDetailChoices, ChallengeId, ChallengeRecordSnapshot,
        ChallengeSetupFailure, ChallengeSetupOperation, ChallengeSetupServices, EntryAvailability,
        MenuModeId, MenuParticipantCount, MissionRecordSnapshot, OwnedArcadeChallengeData,
        OwnedChallengeDataError,
    };
    use tsfp_ui_model::arcade_menu_view::{ArcadeDetailPageLayout, ArcadeMenuViewLayout};
    use tsfp_ui_model::challenge_menu_page::{ChallengeMenuPageLayout, ChallengeMenuPageWidget};
    use tsfp_ui_model::model::WidgetRole;
    use tsfp_ui_model::model::{PageId, Widget, WidgetKind};
    use tsfp_ui_model::{MenuFocus, NavLayout, Wrap, arcade_challenge::ArcadeChallengeBackend};

    #[derive(Default)]
    struct Effects {
        opened_categories: Vec<SelectedArcadeCategory>,
        opened_challenges: Vec<SelectedChallenge>,
        helped: Vec<Option<MenuHelpId>>,
        returned_challenges: Vec<SelectedChallenge>,
        returned_categories: Vec<SelectedArcadeCategory>,
        reject_category: bool,
        reject_detail: bool,
        reject_return_category: bool,
        reject_return_detail: bool,
    }

    impl ArcadeMenuSessionEffects for Effects {
        type Error = ();

        fn refresh_projection(
            &mut self,
            _prepared: &mut ArcadeMenuSession,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        fn open_challenge_list(
            &mut self,
            prepared: &mut ArcadeMenuSession,
            selection: SelectedArcadeCategory,
        ) -> Result<(), Self::Error> {
            assert_eq!(
                prepared.page(),
                ArcadeMenuPage::ChallengeList(selection.category)
            );
            self.opened_categories.push(selection);
            if self.reject_category {
                Err(())
            } else {
                Ok(())
            }
        }

        fn open_detail(
            &mut self,
            prepared: &mut ArcadeMenuSession,
            selection: SelectedChallenge,
        ) -> Result<(), Self::Error> {
            assert_eq!(prepared.page(), ArcadeMenuPage::Detail(selection));
            self.opened_challenges.push(selection);
            if self.reject_detail { Err(()) } else { Ok(()) }
        }

        fn show_help(&mut self, help: Option<MenuHelpId>) -> Result<(), Self::Error> {
            self.helped.push(help);
            Ok(())
        }

        fn return_to_challenge_list(
            &mut self,
            prepared: &mut ArcadeMenuSession,
            selection: SelectedChallenge,
        ) -> Result<(), Self::Error> {
            assert_eq!(
                prepared.page(),
                ArcadeMenuPage::ChallengeList(selection.category)
            );
            self.returned_challenges.push(selection);
            if self.reject_return_detail {
                Err(())
            } else {
                Ok(())
            }
        }

        fn return_to_category_chooser(
            &mut self,
            prepared: &mut ArcadeMenuSession,
            selection: SelectedArcadeCategory,
        ) -> Result<(), Self::Error> {
            assert_eq!(prepared.page(), ArcadeMenuPage::CategoryChooser);
            self.returned_categories.push(selection);
            if self.reject_return_category {
                Err(())
            } else {
                Ok(())
            }
        }
    }

    fn initial_catalog() -> OwnedArcadeChallengeData {
        let category = ArcadeCategoryId::new(7);
        let locked = ChallengeId::new(60);
        let available = ChallengeId::new(61);
        OwnedArcadeChallengeData::new(
            vec![ArcadeCategorySnapshot::new(
                category,
                vec![locked, available],
            )],
            vec![
                ChallengeRecordSnapshot::new(
                    locked,
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Disabled {
                                help: Some(MenuHelpId::new(9)),
                            },
                        }),
                    },
                ),
                ChallengeRecordSnapshot::new(
                    available,
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Available,
                        }),
                    },
                ),
            ],
        )
        .unwrap()
    }

    fn make_session(backend: ArcadeChallengeBackend) -> ArcadeMenuSession {
        ArcadeMenuSession::with_backend(
            initial_catalog(),
            ChallengeDescriptor::new(0x523870),
            backend,
        )
    }

    fn reordered_catalog() -> OwnedArcadeChallengeData {
        let locked = ChallengeId::new(60);
        let available = ChallengeId::new(61);
        OwnedArcadeChallengeData::new(
            vec![ArcadeCategorySnapshot::new(
                ArcadeCategoryId::new(7),
                vec![available, locked],
            )],
            vec![
                ChallengeRecordSnapshot::new(
                    locked,
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Disabled {
                                help: Some(MenuHelpId::new(9)),
                            },
                        }),
                    },
                ),
                ChallengeRecordSnapshot::new(
                    available,
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Available,
                        }),
                    },
                ),
            ],
        )
        .unwrap()
    }

    struct DuplicateCategoryProvider(Vec<ArcadeCategorySnapshot>);

    impl ArcadeChallengeCatalog for DuplicateCategoryProvider {
        fn mission_record(&self, _challenge: ChallengeId) -> Option<MissionRecordSnapshot> {
            None
        }
    }

    impl ArcadeChallengeDataProvider for DuplicateCategoryProvider {
        fn category_snapshots(&self) -> &[ArcadeCategorySnapshot] {
            &self.0
        }

        fn category_challenges(&self, category: ArcadeCategoryId) -> Option<&[ChallengeId]> {
            self.0
                .iter()
                .find(|snapshot| snapshot.category() == category)
                .map(ArcadeCategorySnapshot::challenges)
        }
    }

    fn button(id: u64) -> Widget {
        Widget::new(ModelWidgetId(id), WidgetKind::Button)
    }

    fn layout() -> ArcadeMenuViewLayout {
        ArcadeMenuViewLayout {
            category_page_id: PageId(1),
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
            challenge_page_id: PageId(2),
            challenge: ChallengeMenuPageLayout {
                widget_type: 1,
                panel_bounds: None,
                nav: NavLayout::Grid(0),
                wrap: Wrap::Clamp,
                widgets: vec![
                    ChallengeMenuPageWidget {
                        row: Some(CategoryRowId::new(0)),
                        widget: button(21),
                    },
                    ChallengeMenuPageWidget {
                        row: Some(CategoryRowId::new(1)),
                        widget: button(22),
                    },
                ],
            },
            detail: ArcadeDetailPageLayout {
                page_id: PageId(3),
                widget_type: 3,
                panel_bounds: None,
                nav: NavLayout::Grid(0),
                wrap: Wrap::Clamp,
                widgets: vec![button(31)],
                initial_focus: Some(ModelWidgetId(31)),
            },
        }
    }

    #[derive(Default)]
    struct TestHelp;

    impl ArcadeMenuSessionHelp for TestHelp {
        type Error = ();

        fn show_help(&mut self, _help: Option<MenuHelpId>) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct RecordingHelp(Rc<RefCell<Vec<Option<MenuHelpId>>>>);

    impl ArcadeMenuSessionHelp for RecordingHelp {
        type Error = ();

        fn show_help(&mut self, help: Option<MenuHelpId>) -> Result<(), Self::Error> {
            self.0.borrow_mut().push(help);
            Ok(())
        }
    }

    #[test]
    fn fabricated_page_host_owns_each_route_projection_and_rejects_old_pages() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let mut host = ArcadeMenuPageHost::new(layout(), action, None, TestHelp);
        host.mount(&mut session).unwrap();
        let category_projection = host.projection().unwrap().clone();
        assert!(matches!(
            &category_projection.view,
            ArcadeMenuView::CategoryChooser(_)
        ));

        let mut inner = RecordingBindings::default();
        let mut category_focus = MenuFocus::new(category_projection.page());
        let category_dispatch = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &category_projection,
                &mut category_focus,
                &UiEvent::Activate(InputWidgetId(11)),
                &mut bindings,
            )
        };
        assert_eq!(category_dispatch.binding_result, Some(BindResult::Handled));
        assert!(matches!(
            &host.projection().unwrap().view,
            ArcadeMenuView::ChallengeList { .. }
        ));

        let mut stale_focus = MenuFocus::new(category_projection.page());
        let stale_dispatch = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &category_projection,
                &mut stale_focus,
                &UiEvent::Activate(InputWidgetId(11)),
                &mut bindings,
            )
        };
        assert_eq!(stale_dispatch.binding_result, Some(BindResult::Denied));
        assert!(matches!(
            session.page(),
            ArcadeMenuPage::ChallengeList(category) if category == ArcadeCategoryId::new(7)
        ));

        let list_projection = host.projection().unwrap().clone();
        let mut list_focus = MenuFocus::new(list_projection.page());
        let detail_dispatch = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &list_projection,
                &mut list_focus,
                &UiEvent::Activate(InputWidgetId(22)),
                &mut bindings,
            )
        };
        assert_eq!(detail_dispatch.binding_result, Some(BindResult::Handled));
        assert!(matches!(
            &host.projection().unwrap().view,
            ArcadeMenuView::Detail { .. }
        ));

        let detail_projection = host.projection().unwrap().clone();
        let mut detail_focus = MenuFocus::new(detail_projection.page());
        let back_dispatch = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &detail_projection,
                &mut detail_focus,
                &UiEvent::Back,
                &mut bindings,
            )
        };
        assert_eq!(back_dispatch.binding_result, Some(BindResult::Handled));
        assert!(matches!(
            &host.projection().unwrap().view,
            ArcadeMenuView::ChallengeList { .. }
        ));

        let list_projection = host.projection().unwrap().clone();
        let mut list_focus = MenuFocus::new(list_projection.page());
        let back_dispatch = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &list_projection,
                &mut list_focus,
                &UiEvent::Back,
                &mut bindings,
            )
        };
        assert_eq!(back_dispatch.binding_result, Some(BindResult::Handled));
        assert!(matches!(
            &host.projection().unwrap().view,
            ArcadeMenuView::CategoryChooser(_)
        ));
    }

    #[test]
    fn fabricated_page_host_replaces_layout_transactionally_and_invalidates_old_row_maps() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let mut host = ArcadeMenuPageHost::new(layout(), action, None, TestHelp);
        host.mount(&mut session).unwrap();
        let old_projection = host.projection().unwrap().clone();
        let before_session = session.clone();

        let mut incomplete = layout();
        incomplete.category.widgets.clear();
        assert!(matches!(
            host.replace_layout(&mut session, incomplete),
            Err(ArcadeMenuPageHostError::Projection(_))
        ));
        assert_eq!(session, before_session);
        assert_eq!(
            host.projection().unwrap().projection_revision,
            old_projection.projection_revision
        );
        assert_eq!(host.projection().unwrap().page(), old_projection.page());

        let mut replacement = layout();
        replacement.category.widgets[0].widget = button(101);
        host.replace_layout(&mut session, replacement).unwrap();
        let new_projection = host.projection().unwrap().clone();
        assert_eq!(
            new_projection.projection_revision,
            old_projection.projection_revision + 1
        );
        assert!(
            new_projection
                .page()
                .widgets
                .iter()
                .any(|widget| widget.id == ModelWidgetId(101))
        );
        assert!(
            !new_projection
                .page()
                .widgets
                .iter()
                .any(|widget| widget.id == ModelWidgetId(11))
        );
        let after_replace_session = session.clone();

        let mut inner = RecordingBindings::default();
        let mut old_focus = MenuFocus::new(old_projection.page());
        let stale = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &old_projection,
                &mut old_focus,
                &UiEvent::Activate(InputWidgetId(11)),
                &mut bindings,
            )
        };
        assert_eq!(stale.binding_result, Some(BindResult::Denied));
        assert_eq!(session, after_replace_session);

        let mut new_focus = MenuFocus::new(new_projection.page());
        let current = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &new_projection,
                &mut new_focus,
                &UiEvent::Activate(InputWidgetId(101)),
                &mut bindings,
            )
        };
        assert_eq!(current.binding_result, Some(BindResult::Handled));
        assert!(matches!(
            session.page(),
            ArcadeMenuPage::ChallengeList(category) if category == ArcadeCategoryId::new(7)
        ));
    }

    #[test]
    fn fabricated_page_host_keeps_legacy_as_default_off() {
        let mut session = make_session(ArcadeChallengeBackend::Legacy);
        let mut host = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp);
        assert_eq!(
            host.mount(&mut session),
            Err(ArcadeMenuPageHostError::LegacyBackend)
        );
        assert!(host.projection().is_none());
        assert_eq!(session.projection_revision(), 0);
    }

    #[test]
    fn fabricated_session_runtime_owns_route_and_restores_identity_based_focus() {
        let row_action = ActionId(70);
        let session = make_session(ArcadeChallengeBackend::Rust);
        let pages = ArcadeMenuPageHost::new(layout(), row_action, None, TestHelp);
        let mut runtime = ArcadeMenuSessionRuntime::mount(session, pages).unwrap();
        let mut inner = RecordingBindings::default();

        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(11)));
        let opened_category = runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        assert_eq!(opened_category.binding_result, Some(BindResult::Handled));
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(21)));
        assert!(matches!(
            runtime.session().page(),
            ArcadeMenuPage::ChallengeList(category) if category == ArcadeCategoryId::new(7)
        ));

        let opened_detail = runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));
        assert_eq!(opened_detail.binding_result, Some(BindResult::Handled));
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(31)));

        let returned_to_list = runtime.dispatch(&mut inner, &UiEvent::Back);
        assert_eq!(returned_to_list.binding_result, Some(BindResult::Handled));
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(22)));

        let returned_to_categories = runtime.dispatch(&mut inner, &UiEvent::Back);
        assert_eq!(
            returned_to_categories.binding_result,
            Some(BindResult::Handled)
        );
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(11)));
        assert!(matches!(
            runtime.session().page(),
            ArcadeMenuPage::CategoryChooser
        ));
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_runtime_mounts_owned_provider_snapshot() {
        let provider = initial_catalog();
        let pages = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp);
        let runtime = ArcadeMenuSessionRuntime::mount_from_provider(
            &provider,
            ChallengeDescriptor::new(0x523870),
            ArcadeChallengeBackend::Rust,
            pages,
        )
        .unwrap();
        drop(provider);

        assert_eq!(runtime.session().backend(), ArcadeChallengeBackend::Rust);
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(11)));
        assert_eq!(runtime.session().category_menu().entries().len(), 1);
        assert!(matches!(
            runtime.session().page(),
            ArcadeMenuPage::CategoryChooser
        ));
    }

    #[test]
    fn fabricated_session_runtime_provider_mount_keeps_legacy_off_and_reports_copy_errors() {
        let provider = initial_catalog();
        let pages = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp);
        assert!(matches!(
            ArcadeMenuSessionRuntime::mount_from_provider(
                &provider,
                ChallengeDescriptor::new(0x523870),
                ArcadeChallengeBackend::Legacy,
                pages,
            ),
            Err(ArcadeMenuRuntimeMountError::PageHost(
                ArcadeMenuPageHostError::LegacyBackend
            ))
        ));

        let duplicate = ArcadeCategoryId::new(12);
        let provider = DuplicateCategoryProvider(vec![
            ArcadeCategorySnapshot::new(duplicate, Vec::new()),
            ArcadeCategorySnapshot::new(duplicate, Vec::new()),
        ]);
        let pages = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp);
        assert!(matches!(
            ArcadeMenuSessionRuntime::mount_from_provider(
                &provider,
                ChallengeDescriptor::new(0x523870),
                ArcadeChallengeBackend::Rust,
                pages,
            ),
            Err(ArcadeMenuRuntimeMountError::Provider(
                OwnedChallengeDataProviderError::InvalidData(
                    OwnedChallengeDataError::DuplicateCategory(category)
                )
            )) if category == duplicate
        ));
    }

    #[test]
    fn fabricated_pointer_hover_keeps_disabled_challenge_focus_by_identity_on_refresh() {
        let row_action = ActionId(70);
        let session = make_session(ArcadeChallengeBackend::Rust);
        let help_calls = Rc::new(RefCell::new(Vec::new()));
        let mut disabled_row_layout = layout();
        disabled_row_layout.challenge.widgets[0].widget.enabled = false;
        let pages = ArcadeMenuPageHost::new(
            disabled_row_layout,
            row_action,
            None,
            RecordingHelp(Rc::clone(&help_calls)),
        );
        let mut runtime = ArcadeMenuSessionRuntime::mount(session, pages).unwrap();
        let mut inner = RecordingBindings::default();

        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        // The list starts with the first row selected, so move away before hovering the
        // disabled row. Hovering an already selected widget correctly emits no new action.
        assert_eq!(
            runtime
                .dispatch(&mut inner, &UiEvent::Hover(InputWidgetId(22)))
                .menu_action,
            Some(MenuAction::Focus(ModelWidgetId(22)))
        );
        assert_eq!(
            runtime
                .dispatch(&mut inner, &UiEvent::Hover(InputWidgetId(21)))
                .menu_action,
            Some(MenuAction::Focus(ModelWidgetId(21)))
        );
        assert_eq!(
            runtime.session().focused_entry(),
            Some(
                tsfp_ui_model::arcade_menu_session::ArcadeMenuFocusedEntry::Challenge(
                    ChallengeId::new(60)
                )
            )
        );

        runtime.refresh_catalog(reordered_catalog()).unwrap();

        assert_eq!(
            runtime.session().focused_challenge(),
            Some(ChallengeId::new(60))
        );
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(22)));
        assert_eq!(
            runtime
                .dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)))
                .binding_result,
            Some(BindResult::Handled)
        );
        assert_eq!(*help_calls.borrow(), vec![Some(MenuHelpId::new(9))]);
        assert_eq!(
            runtime.session().focused_challenge(),
            Some(ChallengeId::new(60))
        );
        assert_eq!(
            runtime.session().page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(7))
        );
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_runtime_denies_queued_event_from_previous_page_revision() {
        let row_action = ActionId(70);
        let session = make_session(ArcadeChallengeBackend::Rust);
        let help_calls = Rc::new(RefCell::new(Vec::new()));
        let pages = ArcadeMenuPageHost::new(
            layout(),
            row_action,
            None,
            RecordingHelp(Rc::clone(&help_calls)),
        );
        let mut runtime = ArcadeMenuSessionRuntime::mount(session, pages).unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));

        // Widget 22 names available Challenge 61 in the original map, then disabled Challenge 60
        // after reorder. An event held by the old page must not activate its new occupant.
        let old_projection = runtime.projection().clone();
        let old_page = old_projection.page_revision();
        let stale_queued = old_projection.queue_event(UiEvent::Activate(InputWidgetId(22)));
        runtime.refresh_catalog(reordered_catalog()).unwrap();
        let current_page = runtime.page_revision();
        assert_ne!(old_page, current_page);

        let stale = runtime.dispatch_queued(&mut inner, stale_queued);
        assert_eq!(stale.menu_action, None);
        assert_eq!(stale.binding_result, Some(BindResult::Denied));
        assert_eq!(runtime.session().selected_challenge(), None);
        assert!(help_calls.borrow().is_empty());
        assert!(inner.calls.is_empty());

        let current_queued = runtime
            .projection()
            .queue_event(UiEvent::Activate(InputWidgetId(22)));
        let current = runtime.dispatch_queued(&mut inner, current_queued);
        assert_eq!(current.binding_result, Some(BindResult::Handled));
        assert_eq!(*help_calls.borrow(), vec![Some(MenuHelpId::new(9))]);
        assert!(matches!(
            runtime.session().page(),
            ArcadeMenuPage::ChallengeList(category) if category == ArcadeCategoryId::new(7)
        ));
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_runtime_denies_page_revision_from_another_session() {
        let row_action = ActionId(70);
        let make_runtime = || {
            let help_calls = Rc::new(RefCell::new(Vec::new()));
            let pages = ArcadeMenuPageHost::new(
                layout(),
                row_action,
                None,
                RecordingHelp(Rc::clone(&help_calls)),
            );
            let mut runtime =
                ArcadeMenuSessionRuntime::mount(make_session(ArcadeChallengeBackend::Rust), pages)
                    .unwrap();
            let mut inner = RecordingBindings::default();
            runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
            runtime.refresh_catalog(reordered_catalog()).unwrap();
            (runtime, help_calls)
        };

        let (mut target, target_help) = make_runtime();
        let (source, _source_help) = make_runtime();
        assert_eq!(
            target.projection().projection_revision,
            source.projection().projection_revision
        );
        let source_queued = source
            .projection()
            .queue_event(UiEvent::Activate(InputWidgetId(22)));
        let mut inner = RecordingBindings::default();

        let result = target.dispatch_queued(&mut inner, source_queued);

        assert_eq!(result.menu_action, None);
        assert_eq!(result.binding_result, Some(BindResult::Denied));
        assert_eq!(target.session().selected_challenge(), None);
        assert!(target_help.borrow().is_empty());
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_runtime_replaces_layout_and_focus_transactionally() {
        let action = ActionId(70);
        let session = make_session(ArcadeChallengeBackend::Rust);
        let pages = ArcadeMenuPageHost::new(layout(), action, None, TestHelp);
        let mut runtime = ArcadeMenuSessionRuntime::mount(session, pages).unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Focus(InputWidgetId(22)));
        assert_eq!(
            runtime.session().focused_challenge(),
            Some(ChallengeId::new(61))
        );

        let mut replacement = layout();
        replacement.challenge.widgets[0].widget = button(121);
        replacement.challenge.widgets[1].widget = button(122);
        runtime.replace_layout(replacement.clone()).unwrap();

        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(122)));
        assert_eq!(
            runtime.projection().page().initial_focus,
            Some(ModelWidgetId(122))
        );
        assert_eq!(
            runtime.session().focused_challenge(),
            Some(ChallengeId::new(61))
        );

        let prior_session = runtime.session().clone();
        let prior_projection = runtime.projection().clone();
        let prior_focus = runtime.focus().selected;
        let mut incomplete = replacement;
        incomplete.challenge.widgets.clear();
        assert!(matches!(
            runtime.replace_layout(incomplete),
            Err(ArcadeMenuPageHostError::Projection(_))
        ));
        assert_eq!(runtime.session(), &prior_session);
        assert_eq!(
            runtime.projection().projection_revision,
            prior_projection.projection_revision
        );
        assert_eq!(runtime.projection().page(), prior_projection.page());
        assert_eq!(runtime.focus().selected, prior_focus);

        let stale_widget = runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));
        assert_ne!(stale_widget.binding_result, Some(BindResult::Handled));
        assert_eq!(runtime.session(), &prior_session);
        let current_widget = runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(122)));
        assert_eq!(current_widget.binding_result, Some(BindResult::Handled));
        assert!(matches!(
            runtime.session().page(),
            ArcadeMenuPage::Detail(selection) if selection.challenge == ChallengeId::new(61)
        ));
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_runtime_keeps_category_route_when_list_projection_fails() {
        let action = ActionId(70);
        let mut incomplete = layout();
        incomplete.challenge.widgets.clear();
        let pages = ArcadeMenuPageHost::new(incomplete, action, None, TestHelp);
        let mut runtime =
            ArcadeMenuSessionRuntime::mount(make_session(ArcadeChallengeBackend::Rust), pages)
                .unwrap();
        let prior_route_revision = runtime.session().route_revision();
        let prior_projection = runtime.projection().clone();
        let prior_focus = runtime.focus().selected;
        let mut inner = RecordingBindings::default();
        let mut frontend_state = FrontendState {
            challenge_category: ArcadeCategoryId::new(99),
            ..FrontendState::default()
        };
        let prior_frontend_state = frontend_state.clone();

        let result = runtime.dispatch_with_frontend_state(
            &mut inner,
            &UiEvent::Activate(InputWidgetId(11)),
            &mut frontend_state,
        );

        assert_eq!(result.binding_result, Some(BindResult::Denied));
        assert_eq!(runtime.session().page(), ArcadeMenuPage::CategoryChooser);
        assert_eq!(runtime.session().route_revision(), prior_route_revision);
        assert_eq!(
            runtime.session().focused_entry(),
            Some(
                tsfp_ui_model::arcade_menu_session::ArcadeMenuFocusedEntry::Category(
                    ArcadeCategoryId::new(7)
                )
            )
        );
        assert_eq!(
            runtime.projection().projection_revision,
            prior_projection.projection_revision
        );
        assert_eq!(runtime.projection().page(), prior_projection.page());
        assert_eq!(runtime.focus().selected, prior_focus);
        assert_eq!(frontend_state, prior_frontend_state);
        assert!(matches!(
            runtime.session().page(),
            ArcadeMenuPage::CategoryChooser
        ));
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_runtime_refreshes_provider_and_restores_typed_focus() {
        let row_action = ActionId(70);
        let session = make_session(ArcadeChallengeBackend::Rust);
        let pages = ArcadeMenuPageHost::new(layout(), row_action, None, TestHelp);
        let mut runtime = ArcadeMenuSessionRuntime::mount(session, pages).unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));
        runtime.dispatch(&mut inner, &UiEvent::Back);
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(22)));
        let mut frontend_state = FrontendState::default();

        runtime
            .refresh_from_provider_with_frontend_state(&reordered_catalog(), &mut frontend_state)
            .unwrap();

        assert_eq!(
            runtime.session().focused_challenge(),
            Some(ChallengeId::new(61))
        );
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(21)));
        assert!(matches!(
            runtime.session().page(),
            ArcadeMenuPage::ChallengeList(category) if category == ArcadeCategoryId::new(7)
        ));
        assert_eq!(frontend_state.challenge_category, ArcadeCategoryId::new(7));
        assert_eq!(
            frontend_state.selected_challenge,
            runtime.session().selected_challenge()
        );
        assert_eq!(
            frontend_state.challenge_setup.as_ref(),
            runtime.session().setup_attempt()
        );
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_runtime_refresh_failure_preserves_page_session_and_focus() {
        let row_action = ActionId(70);
        let session = make_session(ArcadeChallengeBackend::Rust);
        let pages = ArcadeMenuPageHost::new(layout(), row_action, None, TestHelp);
        let mut runtime = ArcadeMenuSessionRuntime::mount(session, pages).unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));
        runtime.dispatch(&mut inner, &UiEvent::Back);
        assert!(matches!(
            runtime.session().page(),
            ArcadeMenuPage::ChallengeList(category) if category == ArcadeCategoryId::new(7)
        ));
        let prior_session = runtime.session().clone();
        let prior_projection_revision = runtime.projection().projection_revision;
        let prior_focus = runtime.focus().selected;
        let mut frontend_state = FrontendState {
            challenge_category: ArcadeCategoryId::new(99),
            selected_challenge: runtime.session().selected_challenge(),
            ..FrontendState::default()
        };
        let prior_frontend_state = frontend_state.clone();

        let locked = ChallengeId::new(60);
        let available = ChallengeId::new(61);
        let candidate = OwnedArcadeChallengeData::new(
            vec![ArcadeCategorySnapshot::new(
                ArcadeCategoryId::new(7),
                vec![locked, available],
            )],
            vec![
                ChallengeRecordSnapshot::new(
                    locked,
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: true,
                            availability: EntryAvailability::Available,
                        }),
                    },
                ),
                ChallengeRecordSnapshot::new(
                    available,
                    MissionRecordSnapshot {
                        descriptor_present: true,
                        row_data: Some(CategoryRowData {
                            hidden: false,
                            availability: EntryAvailability::Available,
                        }),
                    },
                ),
            ],
        )
        .unwrap();

        let result = runtime.refresh_catalog_with_frontend_state(candidate, &mut frontend_state);

        assert!(matches!(
            result,
            Err(ArcadeMenuSessionRefreshError::Effects(
                ArcadeMenuPageHostError::Projection(_)
            ))
        ));
        assert_eq!(runtime.session(), &prior_session);
        assert_eq!(
            runtime.projection().projection_revision,
            prior_projection_revision
        );
        assert_eq!(runtime.focus().selected, prior_focus);
        assert_eq!(frontend_state, prior_frontend_state);
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_runtime_provider_copy_failure_preserves_owned_state() {
        let category = ArcadeCategoryId::new(7);
        let duplicate_provider = DuplicateCategoryProvider(vec![
            ArcadeCategorySnapshot::new(category, vec![]),
            ArcadeCategorySnapshot::new(category, vec![]),
        ]);
        let pages = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp);
        let mut runtime =
            ArcadeMenuSessionRuntime::mount(make_session(ArcadeChallengeBackend::Rust), pages)
                .unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Focus(InputWidgetId(22)));
        let prior_session = runtime.session().clone();
        let prior_projection = runtime.projection().clone();
        let prior_focus = runtime.focus().selected;
        let mut frontend_state = FrontendState {
            challenge_category: ArcadeCategoryId::new(99),
            selected_challenge: Some(SelectedChallenge {
                category,
                row: CategoryRowId::new(17),
                challenge: ChallengeId::new(999),
                detail: ChallengeDescriptor::new(999),
            }),
            ..FrontendState::default()
        };
        let prior_frontend_state = frontend_state.clone();

        let result = runtime
            .refresh_from_provider_with_frontend_state(&duplicate_provider, &mut frontend_state);

        assert_eq!(
            result,
            Err(ArcadeMenuSessionProviderRefreshError::Provider(
                OwnedChallengeDataProviderError::InvalidData(
                    OwnedChallengeDataError::DuplicateCategory(category)
                )
            ))
        );
        assert_eq!(runtime.session(), &prior_session);
        assert_eq!(
            runtime.projection().projection_revision,
            prior_projection.projection_revision
        );
        assert_eq!(runtime.projection().page(), prior_projection.page());
        assert_eq!(runtime.focus().selected, prior_focus);
        assert_eq!(frontend_state, prior_frontend_state);
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_runtime_keeps_legacy_default_off() {
        let row_action = ActionId(70);
        let session = make_session(ArcadeChallengeBackend::Legacy);
        let pages = ArcadeMenuPageHost::new(layout(), row_action, None, TestHelp);
        assert!(matches!(
            ArcadeMenuSessionRuntime::mount(session, pages),
            Err(ArcadeMenuPageHostError::LegacyBackend)
        ));
    }

    #[test]
    fn fabricated_detail_choice_widget_updates_owned_state_only_from_current_page() {
        let row_action = ActionId(70);
        let option_action = ActionId(72);
        let choices = ChallengeDetailChoices::new(MenuModeId::new(3), MenuParticipantCount::new(2));
        let control = ArcadeMenuDetailControlId::new(1);
        let mut detail_layout = layout();
        detail_layout.detail.widgets.push(button(32));
        let pages = ArcadeMenuPageHost::new(detail_layout.clone(), row_action, None, TestHelp)
            .with_detail_choice_options(vec![ArcadeMenuDetailChoiceOption {
                control,
                widget: ModelWidgetId(32),
                action: option_action,
                choices,
            }])
            .unwrap();
        let mut runtime =
            ArcadeMenuSessionRuntime::mount(make_session(ArcadeChallengeBackend::Rust), pages)
                .unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));

        let stale = runtime
            .projection()
            .queue_event(UiEvent::Activate(InputWidgetId(32)));
        runtime.replace_layout(detail_layout).unwrap();
        let stale_result = runtime.dispatch_queued(&mut inner, stale);
        assert_eq!(stale_result.binding_result, Some(BindResult::Denied));
        assert_eq!(runtime.session().detail_choices(), None);

        let current = runtime
            .projection()
            .queue_event(UiEvent::Activate(InputWidgetId(32)));
        let current_result = runtime.dispatch_queued(&mut inner, current);
        assert_eq!(current_result.binding_result, Some(BindResult::Handled));
        assert_eq!(runtime.session().detail_choices(), Some(choices));
        assert!(inner.calls.is_empty());

        runtime.dispatch(&mut inner, &UiEvent::Focus(InputWidgetId(32)));
        assert_eq!(runtime.session().focused_detail_control(), Some(control));
        runtime.dispatch(&mut inner, &UiEvent::Back);
        runtime.dispatch(&mut inner, &UiEvent::Back);
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(32)));
    }

    #[test]
    fn fabricated_detail_choice_options_reject_missing_widgets_and_action_collisions() {
        let choices = ChallengeDetailChoices::new(MenuModeId::new(3), MenuParticipantCount::new(2));
        let missing_widget = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp)
            .with_detail_choice_options(vec![ArcadeMenuDetailChoiceOption {
                control: ArcadeMenuDetailControlId::new(1),
                widget: ModelWidgetId(99),
                action: ActionId(72),
                choices,
            }]);
        assert!(matches!(
            missing_widget,
            Err(ArcadeMenuSessionActionMapError::MissingDetailChoiceWidget(
                99
            ))
        ));

        let conflicting_action = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp)
            .with_detail_choice_options(vec![ArcadeMenuDetailChoiceOption {
                control: ArcadeMenuDetailControlId::new(1),
                widget: ModelWidgetId(31),
                action: ActionId(70),
                choices,
            }]);
        assert!(matches!(
            conflicting_action,
            Err(ArcadeMenuSessionActionMapError::ConflictingDetailChoiceAction(70))
        ));

        let mut detail_layout = layout();
        detail_layout.detail.widgets.push(button(32));
        let duplicate_control =
            ArcadeMenuPageHost::new(detail_layout, ActionId(70), None, TestHelp)
                .with_detail_choice_options(vec![
                    ArcadeMenuDetailChoiceOption {
                        control: ArcadeMenuDetailControlId::new(1),
                        widget: ModelWidgetId(31),
                        action: ActionId(72),
                        choices,
                    },
                    ArcadeMenuDetailChoiceOption {
                        control: ArcadeMenuDetailControlId::new(1),
                        widget: ModelWidgetId(32),
                        action: ActionId(73),
                        choices,
                    },
                ]);
        assert!(matches!(
            duplicate_control,
            Err(ArcadeMenuSessionActionMapError::DuplicateDetailChoiceControl(1))
        ));
    }

    #[test]
    fn fabricated_session_runtime_denies_unwired_accept_and_runs_native_setup() {
        let row_action = ActionId(70);
        let accept_action = ActionId(71);
        let mut detail_layout = layout();
        detail_layout.detail.widgets[0].role = WidgetRole::Accept;
        let session = make_session(ArcadeChallengeBackend::Rust);
        let pages =
            ArcadeMenuPageHost::new(detail_layout, row_action, Some(accept_action), TestHelp);
        let mut runtime = ArcadeMenuSessionRuntime::mount(session, pages).unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));

        let unwired = runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(31)));
        assert_eq!(unwired.binding_result, Some(BindResult::Denied));
        assert!(inner.calls.is_empty());
        assert!(runtime.session().setup_attempt().is_none());

        let mut frontend_state = FrontendState::default();
        let mut services = SetupServices::default();
        let stale = runtime
            .projection()
            .queue_event(UiEvent::Activate(InputWidgetId(31)));
        let mut replacement_layout = layout();
        replacement_layout.detail.widgets[0].role = WidgetRole::Accept;
        runtime.replace_layout(replacement_layout).unwrap();
        let denied = runtime.dispatch_queued_with_detail_accept_and_frontend_state(
            &mut inner,
            stale,
            MenuModeId::new(0),
            MenuParticipantCount::new(1),
            &mut services,
            &mut frontend_state,
        );
        assert_eq!(denied.binding_result, Some(BindResult::Denied));
        assert!(services.0.is_empty());
        assert!(runtime.session().setup_attempt().is_none());
        assert_eq!(frontend_state, FrontendState::default());

        let queued = runtime
            .projection()
            .queue_event(UiEvent::Activate(InputWidgetId(31)));
        let accepted = runtime.dispatch_queued_with_detail_accept_and_frontend_state(
            &mut inner,
            queued,
            MenuModeId::new(0),
            MenuParticipantCount::new(1),
            &mut services,
            &mut frontend_state,
        );
        assert_eq!(accepted.binding_result, Some(BindResult::Handled));
        let attempt = runtime.session().setup_attempt().unwrap();
        assert!(attempt.is_complete());
        assert_eq!(
            frontend_state.selected_challenge,
            runtime.session().selected_challenge()
        );
        assert_eq!(frontend_state.challenge_setup.as_ref(), Some(attempt));
        assert_eq!(
            services.0.first(),
            Some(&ChallengeSetupOperation::PrepareDetailAcceptance)
        );
        assert_eq!(
            services.0.last(),
            Some(&ChallengeSetupOperation::OpenSetupPage(
                tsfp_ui_model::arcade_challenge::ChallengeSetupPage::new(0x5237f0)
            ))
        );
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_runtime_accept_uses_choices_owned_by_its_detail_session() {
        let row_action = ActionId(70);
        let accept_action = ActionId(71);
        let mut detail_layout = layout();
        detail_layout.detail.widgets[0].role = WidgetRole::Accept;
        let pages =
            ArcadeMenuPageHost::new(detail_layout, row_action, Some(accept_action), TestHelp);
        let mut runtime =
            ArcadeMenuSessionRuntime::mount(make_session(ArcadeChallengeBackend::Rust), pages)
                .unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));

        let choices = ChallengeDetailChoices::new(MenuModeId::new(7), MenuParticipantCount::new(2));
        runtime.set_detail_choices(choices).unwrap();
        let queued = runtime
            .projection()
            .queue_event(UiEvent::Activate(InputWidgetId(31)));
        let mut services = SetupServices::default();
        let accepted = runtime.dispatch_queued_with_configured_detail_accept(
            &mut inner,
            queued,
            &mut services,
        );

        assert_eq!(accepted.binding_result, Some(BindResult::Handled));
        assert_eq!(runtime.session().detail_choices(), Some(choices));
        assert!(runtime.session().setup_attempt().unwrap().is_complete());
        assert!(services.0.iter().any(|operation| matches!(
            operation,
            ChallengeSetupOperation::InitializeParticipants(participants)
                if participants.count() == MenuParticipantCount::new(2)
        )));
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_runtime_refresh_preserves_partial_setup_for_same_selection() {
        let row_action = ActionId(70);
        let accept_action = ActionId(71);
        let mut detail_layout = layout();
        detail_layout.detail.widgets[0].role = WidgetRole::Accept;
        let pages =
            ArcadeMenuPageHost::new(detail_layout, row_action, Some(accept_action), TestHelp);
        let mut runtime =
            ArcadeMenuSessionRuntime::mount(make_session(ArcadeChallengeBackend::Rust), pages)
                .unwrap();
        let mut inner = RecordingBindings::default();
        let mut frontend_state = FrontendState {
            challenge_category: ArcadeCategoryId::new(99),
            ..FrontendState::default()
        };
        runtime.dispatch_with_frontend_state(
            &mut inner,
            &UiEvent::Activate(InputWidgetId(11)),
            &mut frontend_state,
        );
        assert_eq!(frontend_state.challenge_category, ArcadeCategoryId::new(7));
        runtime.dispatch_with_frontend_state(
            &mut inner,
            &UiEvent::Activate(InputWidgetId(22)),
            &mut frontend_state,
        );
        assert_eq!(
            frontend_state.selected_challenge,
            runtime.session().selected_challenge()
        );

        let mode = MenuModeId::new(0);
        let participants = MenuParticipantCount::new(1);
        let mut services = RejectOnceSetupServices {
            reject_after: Some(2),
            ..RejectOnceSetupServices::default()
        };
        let rejected = runtime.dispatch_with_detail_accept_and_frontend_state(
            &mut inner,
            &UiEvent::Activate(InputWidgetId(31)),
            mode,
            participants,
            &mut services,
            &mut frontend_state,
        );
        assert_eq!(rejected.binding_result, Some(BindResult::Denied));
        let partial = runtime.session().setup_attempt().unwrap().clone();
        assert_eq!(partial.applied_operations(), 2);
        assert_eq!(frontend_state.challenge_setup.as_ref(), Some(&partial));
        runtime
            .refresh_catalog_with_frontend_state(initial_catalog(), &mut frontend_state)
            .unwrap();
        assert_eq!(runtime.session().setup_attempt(), Some(&partial));
        assert_eq!(
            frontend_state.selected_challenge,
            runtime.session().selected_challenge()
        );
        assert_eq!(
            frontend_state.challenge_setup.as_ref(),
            runtime.session().setup_attempt()
        );
        assert_eq!(runtime.focus().selected, Some(ModelWidgetId(31)));

        let resumed = runtime.dispatch_with_detail_accept_and_frontend_state(
            &mut inner,
            &UiEvent::Activate(InputWidgetId(31)),
            mode,
            participants,
            &mut services,
            &mut frontend_state,
        );
        assert_eq!(resumed.binding_result, Some(BindResult::Handled));
        let complete = runtime.session().setup_attempt().unwrap();
        assert!(complete.is_complete());
        assert_eq!(frontend_state.challenge_setup.as_ref(), Some(complete));
        assert_eq!(services.applied.len(), complete.applied_operations());
        assert_eq!(
            services.applied.get(2),
            Some(&ChallengeSetupOperation::ReleaseExistingSetup)
        );
        runtime
            .refresh_catalog_with_frontend_state(reordered_catalog(), &mut frontend_state)
            .unwrap();
        let refreshed_selection = runtime.session().selected_challenge().unwrap();
        assert_eq!(refreshed_selection.challenge, ChallengeId::new(61));
        assert_eq!(refreshed_selection.row, CategoryRowId::new(0));
        assert!(runtime.session().setup_attempt().is_none());
        assert_eq!(frontend_state.selected_challenge, Some(refreshed_selection));
        assert!(frontend_state.challenge_setup.is_none());
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_page_host_keeps_old_route_when_next_projection_cannot_bind_rows() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let mut incomplete_layout = layout();
        incomplete_layout.challenge.widgets.clear();
        let mut host = ArcadeMenuPageHost::new(incomplete_layout, action, None, TestHelp);
        host.mount(&mut session).unwrap();
        let category_projection = host.projection().unwrap().clone();

        let mut inner = RecordingBindings::default();
        let mut focus = MenuFocus::new(category_projection.page());
        let dispatch = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &category_projection,
                &mut focus,
                &UiEvent::Activate(InputWidgetId(11)),
                &mut bindings,
            )
        };

        assert_eq!(dispatch.binding_result, Some(BindResult::Denied));
        assert_eq!(session.page(), ArcadeMenuPage::CategoryChooser);
        assert!(matches!(
            &host.projection().unwrap().view,
            ArcadeMenuView::CategoryChooser(_)
        ));
    }

    #[test]
    fn fabricated_catalog_refresh_rebuilds_the_host_row_map_by_challenge_identity() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let mut host = ArcadeMenuPageHost::new(layout(), action, None, TestHelp);
        host.mount(&mut session).unwrap();
        let category_projection = host.projection().unwrap().clone();
        let mut inner = RecordingBindings::default();
        let mut category_focus = MenuFocus::new(category_projection.page());
        {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &category_projection,
                &mut category_focus,
                &UiEvent::Activate(InputWidgetId(11)),
                &mut bindings,
            );
        }

        let list_projection = host.projection().unwrap().clone();
        let mut list_focus = MenuFocus::new(list_projection.page());
        {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &list_projection,
                &mut list_focus,
                &UiEvent::Activate(InputWidgetId(22)),
                &mut bindings,
            );
        }
        let detail_projection = host.projection().unwrap().clone();
        let mut detail_focus = MenuFocus::new(detail_projection.page());
        {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &detail_projection,
                &mut detail_focus,
                &UiEvent::Back,
                &mut bindings,
            );
        }
        let stale_list = host.projection().unwrap().clone();
        assert_eq!(
            session.challenge_menu().unwrap().selected_row(),
            Some(CategoryRowId::new(1))
        );

        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
        bindings.refresh_catalog(reordered_catalog()).unwrap();
        drop(bindings);
        assert_eq!(
            session.challenge_menu().unwrap().selected_row(),
            Some(CategoryRowId::new(0))
        );

        let mut stale_focus = MenuFocus::new(stale_list.page());
        let stale_result = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &stale_list,
                &mut stale_focus,
                &UiEvent::Activate(InputWidgetId(21)),
                &mut bindings,
            )
        };
        assert_eq!(stale_result.binding_result, Some(BindResult::Denied));

        let current_list = host.projection().unwrap().clone();
        let mut current_focus = MenuFocus::new(current_list.page());
        let current_result = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut host);
            dispatch_arcade_menu_session_event(
                &current_list,
                &mut current_focus,
                &UiEvent::Activate(InputWidgetId(21)),
                &mut bindings,
            )
        };
        assert_eq!(current_result.binding_result, Some(BindResult::Handled));
        assert_eq!(
            session.selected_challenge().unwrap().challenge,
            ChallengeId::new(61)
        );
    }

    #[test]
    fn fabricated_catalog_refresh_projection_failure_preserves_session_frontend_and_page() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selected = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        session.back_to_challenge_list().unwrap();
        let mut host = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp);
        host.mount(&mut session).unwrap();
        let before_session = session.clone();
        let before_page = host.projection().unwrap().page().clone();
        let mut state = FrontendState {
            challenge_category: selected.category,
            selected_challenge: None,
            ..FrontendState::default()
        };
        let before_state = state.clone();
        host.layout.challenge.widgets.clear();
        let mut inner = RecordingBindings::default();

        let result = {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, ActionId(70), &mut session, &mut host)
                    .with_frontend_state(&mut state);
            bindings.refresh_catalog(reordered_catalog())
        };

        assert!(matches!(
            result,
            Err(ArcadeMenuSessionRefreshError::Effects(
                ArcadeMenuPageHostError::Projection(_)
            ))
        ));
        assert_eq!(session, before_session);
        assert_eq!(state, before_state);
        assert_eq!(host.projection().unwrap().page(), &before_page);
    }

    #[test]
    fn fabricated_session_projection_dispatches_category_help_and_detail_natively() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let category_projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let mut category_focus = MenuFocus::new(category_projection.page());
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let result = dispatch_arcade_menu_session_event(
            &category_projection,
            &mut category_focus,
            &UiEvent::Activate(InputWidgetId(11)),
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(result.binding_result, Some(BindResult::Handled));
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(7))
        );
        assert_eq!(effects.opened_categories.len(), 1);

        let challenge_projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let mut challenge_focus = MenuFocus::new(challenge_projection.page());
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let disabled = dispatch_arcade_menu_session_event(
            &challenge_projection,
            &mut challenge_focus,
            &UiEvent::Activate(InputWidgetId(21)),
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(
            disabled.menu_action,
            Some(MenuAction::Disabled(tsfp_ui_model::WidgetId(21)))
        );
        assert_eq!(disabled.binding_result, Some(BindResult::Handled));
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(7))
        );
        assert_eq!(effects.helped, vec![Some(MenuHelpId::new(9))]);

        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let available = dispatch_arcade_menu_session_event(
            &challenge_projection,
            &mut challenge_focus,
            &UiEvent::Activate(InputWidgetId(22)),
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(available.binding_result, Some(BindResult::Handled));
        assert!(
            matches!(session.page(), ArcadeMenuPage::Detail(SelectedChallenge { challenge, .. }) if challenge == ChallengeId::new(61))
        );
        assert_eq!(effects.opened_challenges.len(), 1);

        let mut detail_projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let ArcadeMenuView::Detail { page, .. } = &mut detail_projection.view else {
            panic!("detail projection expected");
        };
        page.handlers.back = Some(tsfp_ui_model::HandlerRef::Named(
            "legacy-detail-back".into(),
        ));
        let mut detail_focus = MenuFocus::new(detail_projection.page());
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let backed_out = dispatch_arcade_menu_session_event(
            &detail_projection,
            &mut detail_focus,
            &UiEvent::Back,
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(backed_out.menu_action, None);
        assert_eq!(backed_out.binding_result, Some(BindResult::Handled));
        assert!(!backed_out.close_requested);
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(7))
        );
        assert_eq!(effects.returned_challenges.len(), 1);

        let mut challenge_projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let ArcadeMenuView::ChallengeList { page, .. } = &mut challenge_projection.view else {
            panic!("mission-list projection expected");
        };
        page.page.handlers.back = Some(tsfp_ui_model::HandlerRef::Named("legacy-list-back".into()));
        let mut challenge_focus = MenuFocus::new(challenge_projection.page());
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let backed_out = dispatch_arcade_menu_session_event(
            &challenge_projection,
            &mut challenge_focus,
            &UiEvent::Back,
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(backed_out.menu_action, None);
        assert_eq!(backed_out.binding_result, Some(BindResult::Handled));
        assert!(!backed_out.close_requested);
        assert_eq!(session.page(), ArcadeMenuPage::CategoryChooser);
        assert_eq!(effects.returned_categories.len(), 1);
    }

    #[test]
    fn fabricated_focus_tracks_mission_identity_across_row_reorder() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let action = ActionId(70);
        let projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let mut focus = MenuFocus::new(projection.page());
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);

        let result = dispatch_arcade_menu_session_event(
            &projection,
            &mut focus,
            &UiEvent::Focus(InputWidgetId(22)),
            &mut bindings,
        );
        drop(bindings);

        assert_eq!(
            result.menu_action,
            Some(MenuAction::Focus(ModelWidgetId(22)))
        );
        assert_eq!(
            session.focused_entry(),
            Some(
                tsfp_ui_model::arcade_menu_session::ArcadeMenuFocusedEntry::Challenge(
                    ChallengeId::new(61)
                )
            )
        );
        session.refresh_catalog(reordered_catalog()).unwrap();
        let refreshed =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        assert_eq!(refreshed.page().initial_focus, Some(ModelWidgetId(21)));
    }

    #[test]
    fn fabricated_rust_root_back_requests_close_without_returning_page_handler() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let mut projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let ArcadeMenuView::CategoryChooser(page) = &mut projection.view else {
            panic!("category chooser projection expected");
        };
        page.page.handlers.back = Some(tsfp_ui_model::HandlerRef::Named("legacy-back".into()));

        let mut focus = MenuFocus::new(projection.page());
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let result = dispatch_arcade_menu_session_event(
            &projection,
            &mut focus,
            &UiEvent::Back,
            &mut bindings,
        );

        assert_eq!(result.menu_action, None);
        assert_eq!(result.binding_result, None);
        assert!(result.close_requested);
        assert_eq!(session.page(), ArcadeMenuPage::CategoryChooser);
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_stale_projection_is_denied_after_route_revision_changes() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let old_projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let mut focus = MenuFocus::new(old_projection.page());
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();

        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let result = dispatch_arcade_menu_session_event(
            &old_projection,
            &mut focus,
            &UiEvent::Activate(InputWidgetId(11)),
            &mut bindings,
        );
        drop(bindings);

        assert_eq!(result.menu_action, None);
        assert_eq!(result.binding_result, Some(BindResult::Denied));
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(7))
        );
        assert!(effects.opened_challenges.is_empty());
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_projection_from_another_session_is_denied() {
        let mut source = make_session(ArcadeChallengeBackend::Rust);
        let mut target = make_session(ArcadeChallengeBackend::Rust);
        assert_ne!(source.id(), target.id());
        let action = ActionId(70);
        let foreign_projection =
            build_arcade_menu_session_projection(&mut source, layout(), action).unwrap();
        let _target_projection =
            build_arcade_menu_session_projection(&mut target, layout(), action).unwrap();
        assert_eq!(source.route_revision(), target.route_revision());
        assert_eq!(source.projection_revision(), target.projection_revision());
        let before = target.clone();
        let mut focus = MenuFocus::new(foreign_projection.page());
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut target, &mut effects);

        let result = dispatch_arcade_menu_session_event(
            &foreign_projection,
            &mut focus,
            &UiEvent::Activate(InputWidgetId(11)),
            &mut bindings,
        );
        drop(bindings);

        assert_eq!(result.menu_action, None);
        assert_eq!(result.binding_result, Some(BindResult::Denied));
        assert_eq!(target, before);
        assert!(effects.opened_categories.is_empty());
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_same_route_layout_rebuild_invalidates_old_row_mapping() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let old_projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let mut rebuilt_layout = layout();
        rebuilt_layout.category.widgets[0].widget = button(12);
        let rebuilt_projection =
            build_arcade_menu_session_projection(&mut session, rebuilt_layout, action).unwrap();
        let mut old_focus = MenuFocus::new(old_projection.page());
        let mut rebuilt_focus = MenuFocus::new(rebuilt_projection.page());
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);

        let stale = dispatch_arcade_menu_session_event(
            &old_projection,
            &mut old_focus,
            &UiEvent::Activate(InputWidgetId(11)),
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(stale.menu_action, None);
        assert_eq!(stale.binding_result, Some(BindResult::Denied));
        assert_eq!(session.page(), ArcadeMenuPage::CategoryChooser);
        assert!(effects.opened_categories.is_empty());

        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let current = dispatch_arcade_menu_session_event(
            &rebuilt_projection,
            &mut rebuilt_focus,
            &UiEvent::Activate(InputWidgetId(12)),
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(current.binding_result, Some(BindResult::Handled));
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(7))
        );
        assert_eq!(effects.opened_categories.len(), 1);
    }

    #[test]
    fn fabricated_effect_rejection_keeps_the_owned_route_and_legacy_forwards() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        let action = ActionId(70);
        let projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let mut focus = MenuFocus::new(projection.page());
        let mut inner = RecordingBindings::default();
        let mut effects = Effects {
            reject_category: false,
            reject_return_category: true,
            ..Effects::default()
        };
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let result = dispatch_arcade_menu_session_event(
            &projection,
            &mut focus,
            &UiEvent::Activate(InputWidgetId(11)),
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(result.binding_result, Some(BindResult::Handled));
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(7))
        );

        let list_projection =
            build_arcade_menu_session_projection(&mut session, layout(), action).unwrap();
        let mut focus = MenuFocus::new(list_projection.page());
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects);
        let result = dispatch_arcade_menu_session_event(
            &list_projection,
            &mut focus,
            &UiEvent::Back,
            &mut bindings,
        );
        drop(bindings);
        assert_eq!(result.binding_result, Some(BindResult::Denied));
        assert_eq!(
            session.page(),
            ArcadeMenuPage::ChallengeList(ArcadeCategoryId::new(7))
        );

        let mut legacy = make_session(ArcadeChallengeBackend::Legacy);
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut legacy, &mut effects);
        assert_eq!(
            bindings.action(action, ActionArg::Index(3)),
            BindResult::Handled
        );
        drop(bindings);
        assert!(matches!(
            build_arcade_menu_session_projection(&mut legacy, layout(), action),
            Err(ArcadeMenuSessionProjectionError::View(
                ArcadeMenuViewBuildError::LegacyBackend
            ))
        ));
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, action, &mut legacy, &mut effects);
        let result = bindings.back();
        drop(bindings);
        assert_eq!(result, None);
        assert_eq!(
            inner.calls,
            vec![RecordedCall::Action(action, ActionArg::Index(3))]
        );
        assert!(effects.opened_categories.is_empty());
    }

    #[derive(Default)]
    struct SetupServices(Vec<ChallengeSetupOperation>);

    impl ChallengeSetupServices for SetupServices {
        fn apply(
            &mut self,
            operation: ChallengeSetupOperation,
        ) -> Result<(), ChallengeSetupFailure> {
            self.0.push(operation);
            Ok(())
        }
    }

    #[derive(Default)]
    struct RejectOnceSetupServices {
        applied: Vec<ChallengeSetupOperation>,
        reject_after: Option<usize>,
        rejected: bool,
    }

    impl ChallengeSetupServices for RejectOnceSetupServices {
        fn apply(
            &mut self,
            operation: ChallengeSetupOperation,
        ) -> Result<(), ChallengeSetupFailure> {
            if !self.rejected && self.reject_after == Some(self.applied.len()) {
                self.rejected = true;
                return Err(ChallengeSetupFailure::ServiceRejected);
            }
            self.applied.push(operation);
            Ok(())
        }
    }

    #[test]
    fn fabricated_session_detail_accept_uses_active_selection_and_owned_setup_state() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selected = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let mut state = FrontendState::default();
        let mut services = SetupServices::default();
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let result = {
            let mut bindings = ArcadeMenuSessionBindings::new(
                &mut inner,
                ActionId(70),
                &mut session,
                &mut effects,
            )
            .with_frontend_state(&mut state)
            .with_detail_accept(
                ActionId(71),
                MenuModeId::new(0),
                MenuParticipantCount::new(1),
                &mut services,
            );
            bindings.action(ActionId(71), ActionArg::None)
        };
        assert_eq!(result, BindResult::Handled);
        assert_eq!(state.selected_challenge, Some(selected));
        let attempt = state.challenge_setup.as_ref().unwrap();
        assert!(attempt.is_complete());
        assert_eq!(session.setup_attempt(), Some(attempt));
        assert!(!services.0.is_empty());
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_accept_uses_detail_choices_owned_before_the_event() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let choices = ChallengeDetailChoices::new(MenuModeId::new(7), MenuParticipantCount::new(2));
        session.set_detail_choices(choices).unwrap();

        let mut services = SetupServices::default();
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let result = {
            let mut bindings = ArcadeMenuSessionBindings::new(
                &mut inner,
                ActionId(70),
                &mut session,
                &mut effects,
            )
            .with_configured_detail_accept(ActionId(71), &mut services);
            bindings.action(ActionId(71), ActionArg::None)
        };

        assert_eq!(result, BindResult::Handled);
        assert_eq!(session.detail_choices(), Some(choices));
        assert!(session.setup_attempt().unwrap().is_complete());
        assert!(services.0.iter().any(|operation| matches!(
            operation,
            ChallengeSetupOperation::InitializeParticipants(participants)
                if participants.count() == MenuParticipantCount::new(2)
        )));
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_stale_detail_choice_update_cannot_mutate_a_new_page() {
        let session = make_session(ArcadeChallengeBackend::Rust);
        let pages = ArcadeMenuPageHost::new(layout(), ActionId(70), None, TestHelp);
        let mut runtime = ArcadeMenuSessionRuntime::mount(session, pages).unwrap();
        let mut inner = RecordingBindings::default();
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(11)));
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));
        let old_detail_revision = runtime.page_revision();

        runtime.dispatch(&mut inner, &UiEvent::Back);
        runtime.dispatch(&mut inner, &UiEvent::Activate(InputWidgetId(22)));
        let current_detail_revision = runtime.page_revision();
        assert_ne!(old_detail_revision, current_detail_revision);

        let stale = ChallengeDetailChoices::new(MenuModeId::new(7), MenuParticipantCount::new(2));
        assert_eq!(
            runtime.set_detail_choices_for_page_revision(old_detail_revision, stale),
            Err(ArcadeMenuDetailChoicesError::StalePage)
        );
        assert_eq!(runtime.session().detail_choices(), None);

        let current = ChallengeDetailChoices::new(MenuModeId::new(3), MenuParticipantCount::new(1));
        assert_eq!(
            runtime.set_detail_choices_for_page_revision(current_detail_revision, current),
            Ok(())
        );
        assert_eq!(runtime.session().detail_choices(), Some(current));
    }

    #[test]
    fn fabricated_session_detail_accept_owns_progress_without_frontend_mirror() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let mut services = RejectOnceSetupServices {
            reject_after: Some(2),
            ..RejectOnceSetupServices::default()
        };
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let result = {
            let mut bindings = ArcadeMenuSessionBindings::new(
                &mut inner,
                ActionId(70),
                &mut session,
                &mut effects,
            )
            .with_detail_accept(
                ActionId(71),
                MenuModeId::new(0),
                MenuParticipantCount::new(1),
                &mut services,
            );
            bindings.action(ActionId(71), ActionArg::None)
        };
        assert_eq!(result, BindResult::Denied);
        let partial = session.setup_attempt().unwrap();
        assert_eq!(partial.applied_operations(), 2);
        assert_eq!(
            partial.failure(),
            Some(ChallengeSetupFailure::ServiceRejected)
        );

        let result = {
            let mut bindings = ArcadeMenuSessionBindings::new(
                &mut inner,
                ActionId(70),
                &mut session,
                &mut effects,
            )
            .with_detail_accept(
                ActionId(71),
                MenuModeId::new(0),
                MenuParticipantCount::new(1),
                &mut services,
            );
            bindings.action(ActionId(71), ActionArg::None)
        };
        assert_eq!(result, BindResult::Handled);
        assert!(session.setup_attempt().unwrap().is_complete());
        assert_eq!(
            services.applied.len(),
            session.setup_attempt().unwrap().applied_operations()
        );
        session.back_to_challenge_list().unwrap();
        assert!(session.setup_attempt().is_none());
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_detail_accept_resumes_only_confirmed_setup_operations() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selected = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let mut state = FrontendState::default();
        let mut services = RejectOnceSetupServices {
            reject_after: Some(2),
            ..RejectOnceSetupServices::default()
        };
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let result = {
            let mut bindings = ArcadeMenuSessionBindings::new(
                &mut inner,
                ActionId(70),
                &mut session,
                &mut effects,
            )
            .with_frontend_state(&mut state)
            .with_detail_accept(
                ActionId(71),
                MenuModeId::new(0),
                MenuParticipantCount::new(1),
                &mut services,
            );
            bindings.action(ActionId(71), ActionArg::None)
        };
        assert_eq!(result, BindResult::Denied);
        assert_eq!(state.selected_challenge, Some(selected));
        let failed = state.challenge_setup.as_ref().unwrap();
        assert_eq!(
            failed.failure(),
            Some(ChallengeSetupFailure::ServiceRejected)
        );
        assert_eq!(failed.applied_operations(), 2);
        assert_eq!(session.setup_attempt(), Some(failed));
        assert_eq!(services.applied.len(), 2);

        let result = {
            let mut bindings = ArcadeMenuSessionBindings::new(
                &mut inner,
                ActionId(70),
                &mut session,
                &mut effects,
            )
            .with_frontend_state(&mut state)
            .with_detail_accept(
                ActionId(71),
                MenuModeId::new(0),
                MenuParticipantCount::new(1),
                &mut services,
            );
            bindings.action(ActionId(71), ActionArg::None)
        };
        assert_eq!(result, BindResult::Handled);
        assert!(state.challenge_setup.as_ref().unwrap().is_complete());
        assert_eq!(state.challenge_setup.as_ref(), session.setup_attempt());
        assert_eq!(
            services.applied.len(),
            state.challenge_setup.as_ref().unwrap().applied_operations()
        );
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_session_detail_accept_keeps_legacy_forwarding() {
        let mut session = make_session(ArcadeChallengeBackend::Legacy);
        let mut state = FrontendState::default();
        let mut services = SetupServices::default();
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, ActionId(70), &mut session, &mut effects)
                .with_frontend_state(&mut state)
                .with_detail_accept(
                    ActionId(71),
                    MenuModeId::new(0),
                    MenuParticipantCount::new(1),
                    &mut services,
                );

        assert_eq!(
            bindings.action(ActionId(71), ActionArg::None),
            BindResult::Handled
        );
        drop(bindings);
        assert_eq!(
            inner.calls,
            vec![RecordedCall::Action(ActionId(71), ActionArg::None)]
        );
        assert!(services.0.is_empty());
        assert!(state.challenge_setup.is_none());
    }

    #[test]
    fn fabricated_session_route_and_frontend_setup_state_commit_together() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selected = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let mode = MenuModeId::new(0);
        let participants = MenuParticipantCount::new(1);
        let plan = tsfp_ui_model::arcade_challenge::ChallengeSetupPlan::from_selected(
            selected,
            mode,
            participants,
        );
        let mut setup_services = SetupServices::default();
        let previous_attempt = plan.apply_tracked(&mut setup_services);
        let mut state = FrontendState {
            challenge_category: ArcadeCategoryId::new(999),
            selected_challenge: Some(selected),
            challenge_setup: Some(previous_attempt),
            ..FrontendState::default()
        };
        let action = ActionId(70);
        let mut inner = RecordingBindings::default();
        let mut effects = Effects {
            reject_return_detail: true,
            ..Effects::default()
        };

        {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects)
                    .with_frontend_state(&mut state);
            assert_eq!(bindings.back(), Some(BindResult::Denied));
        }
        assert_eq!(state.challenge_category, ArcadeCategoryId::new(999));
        assert_eq!(state.selected_challenge, Some(selected));
        assert!(state.challenge_setup.is_some());

        effects.reject_return_detail = false;
        {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects)
                    .with_frontend_state(&mut state);
            assert_eq!(bindings.back(), Some(BindResult::Handled));
        }
        assert_eq!(state.selected_challenge, None);
        assert_eq!(state.challenge_setup, None);
        assert_eq!(state.challenge_category, selected.category);

        {
            let mut bindings =
                ArcadeMenuSessionBindings::new(&mut inner, action, &mut session, &mut effects)
                    .with_frontend_state(&mut state);
            assert_eq!(
                bindings.action(action, ActionArg::Index(1)),
                BindResult::Handled
            );
        }
        assert_eq!(state.selected_challenge, session.selected_challenge());
        assert!(state.selected_challenge.is_some());
        assert_eq!(state.challenge_setup, None);
        assert_eq!(state.challenge_category, selected.category);
    }

    #[test]
    fn fabricated_catalog_refresh_updates_attached_selection_after_row_reorder() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let old_selection = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let plan = tsfp_ui_model::arcade_challenge::ChallengeSetupPlan::from_selected(
            old_selection,
            MenuModeId::new(0),
            MenuParticipantCount::new(1),
        );
        let mut setup_services = SetupServices::default();
        let attempt = plan.apply_tracked(&mut setup_services);
        let mut state = FrontendState {
            challenge_category: old_selection.category,
            selected_challenge: Some(old_selection),
            challenge_setup: Some(attempt),
            ..FrontendState::default()
        };
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, ActionId(70), &mut session, &mut effects)
                .with_frontend_state(&mut state);

        let provider = reordered_catalog();
        bindings.refresh_from_provider(&provider).unwrap();
        drop(provider);
        drop(bindings);

        let updated = session.selected_challenge().unwrap();
        assert_eq!(updated.challenge, old_selection.challenge);
        assert_eq!(updated.row, CategoryRowId::new(0));
        assert_eq!(state.selected_challenge, Some(updated));
        assert_eq!(state.challenge_category, old_selection.category);
        assert!(state.challenge_setup.is_none());

        assert_eq!(
            session.accept_detail(
                MenuModeId::new(0),
                MenuParticipantCount::new(1),
                &mut SetupServices::default(),
            ),
            tsfp_ui_model::arcade_menu_session::ArcadeMenuDetailAcceptResult::SetupComplete
        );
        let preserved_setup = session.setup_attempt().cloned();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, ActionId(70), &mut session, &mut effects)
                .with_frontend_state(&mut state);
        bindings.refresh_catalog(reordered_catalog()).unwrap();
        drop(bindings);
        assert_eq!(state.challenge_setup, preserved_setup);
    }

    #[test]
    fn fabricated_catalog_refresh_clears_setup_without_active_detail() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selected = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let plan = tsfp_ui_model::arcade_challenge::ChallengeSetupPlan::from_selected(
            selected,
            MenuModeId::new(0),
            MenuParticipantCount::new(1),
        );
        let attempt = plan.apply_tracked(&mut SetupServices::default());
        session.back_to_challenge_list().unwrap();
        let mut state = FrontendState {
            challenge_category: selected.category,
            selected_challenge: None,
            challenge_setup: Some(attempt),
            ..FrontendState::default()
        };
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, ActionId(70), &mut session, &mut effects)
                .with_frontend_state(&mut state);

        bindings.refresh_catalog(reordered_catalog()).unwrap();

        assert_eq!(state.selected_challenge, None);
        assert!(state.challenge_setup.is_none());
        assert_eq!(state.challenge_category, selected.category);
    }

    #[test]
    fn fabricated_rejected_catalog_refresh_preserves_attached_state() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selected = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let plan = tsfp_ui_model::arcade_challenge::ChallengeSetupPlan::from_selected(
            selected,
            MenuModeId::new(0),
            MenuParticipantCount::new(1),
        );
        let mut setup_services = SetupServices::default();
        let attempt = plan.apply_tracked(&mut setup_services);
        let mut state = FrontendState {
            selected_challenge: Some(selected),
            challenge_setup: Some(attempt),
            ..FrontendState::default()
        };
        let prior_session = session.clone();
        let prior_state = state.clone();
        let mut inner = RecordingBindings::default();
        let mut effects = Effects::default();
        let mut bindings =
            ArcadeMenuSessionBindings::new(&mut inner, ActionId(70), &mut session, &mut effects)
                .with_frontend_state(&mut state);

        assert_eq!(
            bindings.refresh_from_provider(&OwnedArcadeChallengeData::new(vec![], vec![]).unwrap()),
            Err(ArcadeMenuSessionProviderRefreshError::Refresh(
                ArcadeMenuSessionRefreshError::Session(ArcadeMenuRefreshError::MissingCategory(
                    selected.category
                ))
            ))
        );

        assert_eq!(*bindings.session, prior_session);
        assert_eq!(*bindings.frontend_state.unwrap(), prior_state);
    }

    #[test]
    fn fabricated_detail_accept_uses_session_selection_and_owned_setup_state() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selection = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let row_action = ActionId(70);
        let accept_action = ActionId(71);

        let revision = session.projection_revision();
        assert!(matches!(
            build_arcade_menu_session_projection_with_detail_accept(
                &mut session,
                layout(),
                row_action,
                accept_action,
            ),
            Err(ArcadeMenuSessionProjectionError::ActionMap(
                ArcadeMenuSessionActionMapError::MissingDetailAcceptWidget
            ))
        ));
        assert_eq!(session.projection_revision(), revision);

        let mut detail_layout = layout();
        detail_layout.detail.widgets[0].role = WidgetRole::Accept;
        let projection = build_arcade_menu_session_projection_with_detail_accept(
            &mut session,
            detail_layout,
            row_action,
            accept_action,
        )
        .unwrap();
        let mut state = crate::frontend_handlers::FrontendState::default();
        let mut services = SetupServices::default();
        let mut inner = RecordingBindings::default();
        let mut bindings = crate::challenge_menu::ArcadeSessionDetailAcceptBindings::new(
            &mut inner,
            accept_action,
            MenuModeId::new(0),
            MenuParticipantCount::new(1),
            &session,
            &mut state,
            &mut services,
        );
        let applied = bindings.action(accept_action, ActionArg::None);
        assert_eq!(applied, BindResult::Handled);
        drop(bindings);

        assert_eq!(state.selected_challenge, Some(selection));
        let attempt = state.challenge_setup.as_ref().unwrap();
        assert!(attempt.is_complete());
        assert_eq!(attempt.state().selection(), selection);
        assert_eq!(
            services.0.first(),
            Some(&ChallengeSetupOperation::PrepareDetailAcceptance)
        );
        assert_eq!(
            services.0.last(),
            Some(&ChallengeSetupOperation::OpenSetupPage(
                tsfp_ui_model::arcade_challenge::ChallengeSetupPage::new(0x5237f0)
            ))
        );
        assert!(inner.calls.is_empty());
    }

    #[test]
    fn fabricated_detail_accept_restarts_when_owned_attempt_is_for_another_plan() {
        let mut session = make_session(ArcadeChallengeBackend::Rust);
        session
            .open_category_row(ArcadeCategoryRowId::new(0))
            .unwrap();
        let selection = session.open_challenge_row(CategoryRowId::new(1)).unwrap();
        let mode = MenuModeId::new(0);
        let participant_count = MenuParticipantCount::new(1);
        let previous_selection = tsfp_ui_model::arcade_challenge::SelectedChallenge {
            category: ArcadeCategoryId::new(8),
            row: CategoryRowId::new(2),
            challenge: ChallengeId::new(99),
            detail: ChallengeDescriptor::new(0x523870),
        };
        let previous_plan = tsfp_ui_model::arcade_challenge::ChallengeSetupPlan::from_selected(
            previous_selection,
            mode,
            participant_count,
        );
        let mut previous_services = SetupServices::default();
        let previous_attempt = previous_plan.apply_tracked(&mut previous_services);
        assert!(previous_attempt.is_complete());

        let mut detail_layout = layout();
        detail_layout.detail.widgets[0].role = WidgetRole::Accept;
        let projection = build_arcade_menu_session_projection_with_detail_accept(
            &mut session,
            detail_layout,
            ActionId(70),
            ActionId(71),
        )
        .unwrap();
        let mut state = crate::frontend_handlers::FrontendState {
            selected_challenge: Some(previous_selection),
            challenge_setup: Some(previous_attempt),
            ..crate::frontend_handlers::FrontendState::default()
        };
        let mut services = SetupServices::default();
        let mut inner = RecordingBindings::default();
        let mut bindings = crate::challenge_menu::ArcadeSessionDetailAcceptBindings::new(
            &mut inner,
            ActionId(71),
            mode,
            participant_count,
            &session,
            &mut state,
            &mut services,
        );

        assert_eq!(
            bindings.action(ActionId(71), ActionArg::None),
            BindResult::Handled
        );
        drop(bindings);

        let attempt = state.challenge_setup.as_ref().unwrap();
        assert_eq!(state.selected_challenge, Some(selection));
        assert!(attempt.is_complete());
        assert_eq!(attempt.state().selection(), selection);
        assert_eq!(
            services.0.len(),
            tsfp_ui_model::arcade_challenge::ChallengeSetupPlan::from_selected(
                selection,
                mode,
                participant_count,
            )
            .operations()
            .len()
        );
        assert!(inner.calls.is_empty());
    }
}
