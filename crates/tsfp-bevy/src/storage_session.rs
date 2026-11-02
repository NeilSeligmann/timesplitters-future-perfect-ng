//! Owned shell storage command boundary. Entry adapters must supply runtime
//! authentication and native game/editor state before enabling their roots.
use std::sync::atomic::{AtomicU64, Ordering};
use tsfp_engine_fileio::{
    local_storage::LocalStorage,
    storage_flow::{LocalTitleAuthentication, SampleMaps, StorageFlow},
    storage_frontend::{Command, Completion, MapLoadSource, StorageFrontend, Target},
    storage_lifecycle::{Context, NativeStorageLifecycle},
    vfs::{NtStatus, status},
};
use tsfp_mapmaker::document_close::{ClosePromptToken, MapDocumentId};

static NEXT_MAP_DOCUMENT_ID: AtomicU64 = AtomicU64::new(1);

fn next_map_document_token() -> Option<ClosePromptToken> {
    let id = NEXT_MAP_DOCUMENT_ID
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(1)
        })
        .ok()?;
    Some(ClosePromptToken::new(MapDocumentId::new(id), id))
}

/// Identity for the document currently owned by StorageSession. A successful
/// MapLoad always starts a fresh identity; edits keep that identity until the
/// lifecycle no longer owns a document.
#[derive(Default)]
struct MapDocumentCloseIdentity {
    token: Option<ClosePromptToken>,
}

impl MapDocumentCloseIdentity {
    fn adopt_loaded_document(&mut self) -> Option<ClosePromptToken> {
        self.token = next_map_document_token();
        self.token
    }

    fn current(&mut self, owns_document: bool) -> Option<ClosePromptToken> {
        if !owns_document {
            self.token = None;
            return None;
        }
        if self.token.is_none() {
            self.token = next_map_document_token();
        }
        self.token
    }
}

pub struct StorageSession {
    pub caller: crate::storage_caller::StorageCaller,
    pub frontend: StorageFrontend<LocalTitleAuthentication>,
    pub lifecycle: NativeStorageLifecycle,
    pub profile_initializer: Option<tsfp_engine_fileio::storage_lifecycle::ProfileInitializer>,
    pub progress_defaults: Option<tsfp_engine_fileio::profile_defaults::ProgressDefaults>,
    pub route: tsfp_engine_fileio::storage_route::StorageRoute,
    device: tsfp_engine_fileio::local_storage::LocalDevice,
    editor_navigation: Option<crate::editor_navigation::EditorNavigation>,
    player: Option<usize>,
    launch_policy: Option<crate::ui_extensions::ProfileInputPolicy>,
    map_document_close_identity: MapDocumentCloseIdentity,
    destination_handler: Option<(
        crate::storage_caller::NativePageId,
        Vec<crate::storage_caller::DestinationRow>,
        DestinationHandler,
    )>,
}

impl StorageSession {
    /// Keys are derived by the runtime, never read from a configuration or log.
    pub fn new(storage: &LocalStorage, title_key: [u8; 16]) -> Result<Self, NtStatus> {
        Ok(Self {
            caller: Default::default(),
            frontend: StorageFrontend::new(StorageFlow::new(
                storage.vfs()?,
                LocalTitleAuthentication::from_runtime_title_key(title_key),
            )),
            lifecycle: NativeStorageLifecycle::default(),
            progress_defaults: None,
            profile_initializer: None,
            route: Default::default(),
            device: storage.devices()[0].clone(),
            editor_navigation: None,
            player: None,
            launch_policy: None,
            map_document_close_identity: MapDocumentCloseIdentity::default(),
            destination_handler: None,
        })
    }

    /// Return an opaque generation token only while this session owns a map
    /// document. The token is stable across edits and save receipts.
    pub fn mapmaker_document_close_token(&mut self) -> Option<ClosePromptToken> {
        self.map_document_close_identity
            .current(self.lifecycle.editor().is_some())
    }

    /// Adopt typed mission progress into the already selected native profile. The player number
    /// only addresses the lifecycle row; `save_story_progress` takes the authenticated `SaveKey`
    /// from that row and verifies it against the active storage flow before publishing.
    pub fn adopt_story_outcome(
        &mut self,
        player: usize,
        flow: &tsfp_state::story_flow::StoryFlow,
        outcome: &tsfp_gameplay::story_events::slots::MissionOutcome,
        mode: tsfp_engine_fileio::story_progress::StoryMode,
        best_time: f32,
        minimum_time: f32,
    ) -> Result<crate::story_progress_bridge::OutcomeResult, NtStatus> {
        let Some(update) = crate::story_progress_bridge::qualified_update(
            flow,
            outcome,
            mode,
            best_time,
            minimum_time,
        ) else {
            return Ok(crate::story_progress_bridge::OutcomeResult::Ignored);
        };
        let receipt = self
            .lifecycle
            .save_story_progress(&mut self.frontend, player, update)?;
        Ok(crate::story_progress_bridge::OutcomeResult::Committed {
            campaign_ordinal: update.campaign_ordinal,
            receipt,
        })
    }

    /// Bind the concrete native frontend executor to authenticated runtime rows.
    /// Installation validates before replacing the previous binding. The handler
    /// runs with exclusive owned session access; it must not reenter global APIs.
    pub fn bind_destination(
        &mut self,
        page: crate::storage_caller::NativePageId,
        rows: Vec<crate::storage_caller::DestinationRow>,
        handler: DestinationHandler,
    ) -> Result<(), NtStatus> {
        self.caller.install_destination(page, rows.clone())?;
        self.destination_handler = Some((page, rows, handler));
        Ok(())
    }
    pub fn destination_input_active(&self) -> bool {
        self.catalogue_view().is_none()
            && self.profile_return_view().is_none()
            && self
                .destination_handler
                .as_ref()
                .is_some_and(|(page, rows, _)| {
                    self.caller
                        .destination_view()
                        .is_some_and(|view| view.page.id == *page && view.rows == *rows)
                })
    }
    /// Generate and execute under the same owned-state lock. An unbound page
    /// cannot capture input or silently discard an activation request.
    pub fn destination_input(
        &mut self,
        page: crate::storage_caller::NativePageId,
        source: crate::storage_caller::PlayerInputSource,
        action: crate::storage_caller::DestinationAction,
    ) -> Result<(), NtStatus> {
        let handler = self.bound_destination_handler(page)?;
        if let Some(request) = self.caller.destination_action(page, source, action)? {
            handler(self, request)?;
        }
        Ok(())
    }
    pub fn destination_pointer_input(
        &mut self,
        rendered: &crate::storage_caller::DestinationView,
        source: crate::storage_caller::PlayerInputSource,
        focus: Option<u64>,
        accept: bool,
        back: bool,
    ) -> Result<(), NtStatus> {
        let handler = self.bound_destination_handler(rendered.page.id)?;
        if let Some(request) = self
            .caller
            .destination_pointer(rendered, source, focus, accept, back)?
        {
            handler(self, request)?;
        }
        Ok(())
    }
    fn bound_destination_handler(
        &self,
        page: crate::storage_caller::NativePageId,
    ) -> Result<DestinationHandler, NtStatus> {
        if !self.destination_input_active() {
            return Err(status::INVALID_PARAMETER);
        }
        self.destination_handler
            .as_ref()
            .filter(|(bound, _, _)| *bound == page)
            .map(|(_, _, handler)| *handler)
            .ok_or(status::INVALID_PARAMETER)
    }

    /// Five owned entry handlers. The address chooses a contract, not a guest
    /// callback. Wrong-role commands are rejected before touching state or disk.
    pub fn entry(
        &mut self,
        address: u32,
        target: Target,
        action: EntryAction,
        samples: &mut impl SampleMaps,
    ) -> Result<EntryResult, NtStatus> {
        use tsfp_engine_fileio::storage_route::Root;
        let root = Root::from_address(address).ok_or(status::INVALID_PARAMETER)?;
        if !root.accepts(target) {
            return Err(status::INVALID_PARAMETER);
        }
        match (root, action) {
            (Root::DeviceFeedback, EntryAction::DeviceReady) => {
                // Feedback acknowledges the one actual mounted device; it does
                // not open a catalogue, load a payload or fabricate completion.
                Ok(EntryResult::Device(self.device.clone()))
            }
            (Root::ProfileTick | Root::MapCatalogue, EntryAction::Open) => {
                self.open_root(root, target, samples)?;
                Ok(EntryResult::Screen(self.frontend.screen().clone()))
            }
            (Root::ProfileTick | Root::MapCatalogue, EntryAction::LoadSelection) => {
                if self.lifecycle.context() != Context::Storage(target) {
                    return Err(status::INVALID_PARAMETER);
                }
                Ok(EntryResult::Completed(self.load_selected(samples)?))
            }
            (Root::ProfileSaveTick, EntryAction::Profile(command)) => {
                let player = match &command {
                    Command::CreateProfile { player, .. }
                    | Command::SaveProfile { player, .. }
                    | Command::SaveInput { player, .. } => *player,
                    _ => return Err(status::INVALID_PARAMETER),
                };
                if target != (Target::Profile { player }) {
                    return Err(status::INVALID_PARAMETER);
                }
                Ok(EntryResult::Completed(
                    self.command(command, samples)?
                        .ok_or(status::INVALID_PARAMETER)?,
                ))
            }
            (Root::MapProgress, EntryAction::SaveEditor { new_save }) => {
                // The legacy status builder also advances its transaction.
                // Its owned replacement must serialize, save and adopt, not
                // acknowledge an empty progress page as successful I/O.
                Ok(EntryResult::Completed(self.save_editor(new_save, samples)?))
            }
            _ => Err(status::INVALID_PARAMETER),
        }
    }

    /// Every completion is adopted before publishing it to shell consumers.
    /// Storage errors retain the context and never issue a success receipt.
    pub fn command(
        &mut self,
        mut command: Command,
        samples: &mut impl SampleMaps,
    ) -> Result<Option<Completion>, NtStatus> {
        let target = match &command {
            Command::Open(target) => *target,
            Command::OpenMapSourceChoice | Command::OpenMapCatalogue(_) => Target::Map,
            Command::CreateProfile { player, .. }
            | Command::SaveProfile { player, .. }
            | Command::SaveInput { player, .. }
            | Command::LoadProfile { player, .. } => Target::Profile { player: *player },
            Command::SaveMap { .. } | Command::UpdateMap { .. } | Command::LoadMap { .. } => {
                Target::Map
            }
            Command::Close => {
                self.lifecycle.cancel()?;
                if let Some(navigation) = self
                    .editor_navigation
                    .as_mut()
                    .filter(|navigation| navigation.pending())
                {
                    navigation.cancel()?;
                }
                let receipt = self.frontend.handle(command, samples)?;
                self.caller.close();
                self.caller.selection.cancel();
                self.route.publish(self.frontend.screen(), receipt.clone());
                return Ok(receipt);
            }
        };
        if target == Target::Map
            && self
                .editor_navigation
                .as_ref()
                .is_some_and(|navigation| navigation.has_return())
        {
            return Err(status::INVALID_PARAMETER);
        }
        if target == Target::Map
            && let Some(navigation) = self
                .editor_navigation
                .as_ref()
                .filter(|navigation| navigation.pending())
        {
            let operation = match &command {
                Command::SaveMap { .. } | Command::UpdateMap { .. } => {
                    Some(tsfp_engine_fileio::storage_flow::Operation::MapSave)
                }
                Command::LoadMap { .. } => {
                    Some(tsfp_engine_fileio::storage_flow::Operation::MapLoad)
                }
                _ => None,
            };
            if operation.is_some_and(|operation| !navigation.operation().accepts(operation)) {
                return Err(status::INVALID_PARAMETER);
            }
        }
        if self.caller.selection.active()
            && self
                .caller
                .selection
                .next_player()
                .map(|player| Target::Profile { player })
                != Some(target)
        {
            return Err(status::INVALID_PARAMETER);
        }
        if let Command::LoadProfile { player, key } = &command
            && self.caller.selection.active()
            && (!self.caller.selection.accepts(*player)
                || !self.caller.selection.key_available(*player, key))
        {
            return Err(status::INVALID_PARAMETER);
        }
        // A command for another player/editor cannot replace an open error context.
        match self.lifecycle.context() {
            Context::Storage(current) if current != target => {
                return Err(status::INVALID_PARAMETER);
            }
            Context::Storage(_) => {}
            _ => self.lifecycle.open(target)?,
        }
        self.caller.attach(target)?;
        if matches!(target, Target::Profile { .. }) && self.launch_policy.is_none() {
            self.launch_policy =
                Some(crate::ui_extensions::profile_policy().ok_or(status::INVALID_PARAMETER)?);
        }
        // Clone the owned ring so failed writes never consume the next native
        // profile preset. Legacy/caller-supplied defaults remain explicit when
        // no runtime constructor provider has been installed.
        let mut initializer = self.profile_initializer.clone();
        let creating = matches!(&command, Command::CreateProfile { .. });
        if let Command::CreateProfile { candidate, .. } = &mut command {
            let defaults = self
                .progress_defaults
                .as_ref()
                .ok_or(status::INVALID_PARAMETER)?;
            if let Some(initializer) = initializer.as_mut() {
                *candidate = initializer.initialize_with_progress(candidate, defaults)?;
            } else {
                *candidate = tsfp_engine_fileio::storage_lifecycle::initialize_profile(candidate)?;
                defaults.reset_profile(candidate)?;
                candidate.payload[0x1290..0x1edc].fill(0);
            }
        }
        let receipt = match self.frontend.handle(command, samples) {
            Ok(receipt) => receipt,
            Err(error) => {
                self.route.publish(self.frontend.screen(), None);
                return Err(error);
            }
        };
        if let Some(receipt) = &receipt {
            let launch = self.launch_policy.as_ref();
            self.lifecycle.adopt(&self.frontend, receipt, |active| {
                if active.native_input {
                    Ok(active.input.clone())
                } else {
                    launch
                        .map(|p| p.settings.clone())
                        .ok_or(status::INVALID_PARAMETER)
                }
            })?;
            if receipt.target == Target::Map
                && receipt.operation == tsfp_engine_fileio::storage_flow::Operation::MapLoad
            {
                self.map_document_close_identity.adopt_loaded_document();
            }
            self.complete_editor_navigation(receipt)?;
            self.caller.consume(&self.lifecycle, receipt)?;
            if let Target::Profile { player } = receipt.target {
                self.player = Some(player);
                self.publish_profile();
            }
        }
        if creating && receipt.is_some() {
            self.profile_initializer = initializer;
        }
        self.route.publish(self.frontend.screen(), receipt.clone());
        Ok(receipt)
    }

    /// Caller supplies typed player/editor context; root adapters must never
    /// infer a player from an unvalidated guest global or select a device row.
    pub fn open_root(
        &mut self,
        root: tsfp_engine_fileio::storage_route::Root,
        target: Target,
        samples: &mut impl SampleMaps,
    ) -> Result<(), NtStatus> {
        if !root.accepts(target) {
            return Err(status::INVALID_PARAMETER);
        }
        self.command(Command::Open(target), samples)?;
        Ok(())
    }

    /// Owned catalogue presentation and input caller. Open errors keep this
    /// context visible; Accept never guesses a player from the legacy page.
    pub fn catalogue_view(&self) -> Option<crate::storage_caller::CatalogueView> {
        Some(crate::storage_caller::CatalogueView {
            target: self.caller.target().ok()?,
            rows: if self.caller.map_name.is_some() {
                Vec::new()
            } else {
                self.route.entries().to_vec()
            },
            sample_maps: if self.caller.map_name.is_some() {
                Vec::new()
            } else {
                self.route.sample_maps().to_vec()
            },
            map_source_choice: self.route.map_source_choice(),
            cursor: if matches!(self.caller.target().ok()?, Target::Profile { .. })
                && (self.caller.create_focused || self.route.entries().is_empty())
            {
                Some(self.route.entries().len())
            } else {
                self.route.cursor()
            },
            error: self
                .caller
                .profile_name
                .as_ref()
                .and_then(|name| name.error())
                .or_else(|| self.caller.map_name.as_ref().and_then(|name| name.error()))
                .or(self.caller.creation_error)
                .or(self.route.error()),
        })
    }
    pub fn catalogue_action(
        &mut self,
        action: crate::storage_caller::CatalogueAction,
        samples: &mut impl SampleMaps,
    ) -> Result<Option<Completion>, NtStatus> {
        use crate::storage_caller::CatalogueAction;
        if self.profile_return_view().is_some() {
            match action {
                CatalogueAction::Back | CatalogueAction::Accept => {
                    self.caller.selection.cancel_pending_return();
                    self.caller.return_error = None;
                }
                CatalogueAction::Step(_) | CatalogueAction::Focus(0) => {}
                _ => return Err(status::INVALID_PARAMETER),
            }
            return Ok(None);
        }
        if self.route.map_source_choice() {
            return match action {
                CatalogueAction::Step(delta) => {
                    let current = self.route.cursor().unwrap_or(0) as i64;
                    let next = (current + i64::from(delta)).rem_euclid(2) as usize;
                    self.route.focus(next)?;
                    Ok(None)
                }
                CatalogueAction::Focus(index) => {
                    self.route.focus(index)?;
                    Ok(None)
                }
                CatalogueAction::Accept => {
                    let source = match self.route.cursor() {
                        Some(0) => MapLoadSource::Samples,
                        Some(1) => MapLoadSource::LocalStorage,
                        _ => return Err(status::OBJECT_NAME_NOT_FOUND),
                    };
                    self.command(Command::OpenMapCatalogue(source), samples)
                }
                CatalogueAction::Back => self.command(Command::Close, samples),
            };
        }
        let target = self.caller.target()?;
        if self.lifecycle.context() != Context::Storage(target) {
            return Err(status::INVALID_PARAMETER);
        }
        if self.caller.map_name.is_some() {
            return match action {
                CatalogueAction::Accept => self.submit_map_name(samples).map(Some),
                CatalogueAction::Back => self.command(Command::Close, samples),
                CatalogueAction::Step(_) | CatalogueAction::Focus(0) => Ok(None),
                _ => Err(status::INVALID_PARAMETER),
            };
        }
        if self.caller.profile_name.is_some() && matches!(action, CatalogueAction::Accept) {
            return self.submit_profile_name(samples).map(Some);
        }
        if self.caller.profile_name.is_some() {
            if !matches!(action, CatalogueAction::Back) {
                return Err(status::INVALID_PARAMETER);
            }
            // Back leaves the child text editor, retaining the participant and
            // its adopted peers. Refresh the catalogue without a device page.
            self.caller.profile_name = None;
            self.command(Command::Open(target), samples)?;
            return Ok(None);
        }
        match action {
            CatalogueAction::Step(delta) => {
                let count = self.route.entries().len()
                    + self.route.sample_maps().len()
                    + usize::from(matches!(target, Target::Profile { .. }));
                if count == 0 {
                    return Err(status::OBJECT_NAME_NOT_FOUND);
                }
                let next = (self
                    .catalogue_view()
                    .and_then(|view| view.cursor)
                    .unwrap_or(0) as i64
                    + i64::from(delta))
                .rem_euclid(count as i64) as usize;
                self.focus_catalogue(next)?;
                Ok(None)
            }
            CatalogueAction::Focus(index) => {
                self.focus_catalogue(index)?;
                Ok(None)
            }
            CatalogueAction::Accept => {
                if matches!(target, Target::Profile { .. })
                    && (self.caller.create_focused || self.route.entries().is_empty())
                {
                    self.caller.creation_requested = true;
                    self.caller.creation_error = None;
                    return Ok(None);
                }
                let receipt = self.load_selected(samples)?;
                if let Some(player) = self.caller.selection.next_player() {
                    // The current adoption is durable even if reading the next
                    // catalogue fails; retain its receipt and show that error.
                    let _ = self.open_root(
                        tsfp_engine_fileio::storage_route::Root::ProfileTick,
                        Target::Profile { player },
                        samples,
                    );
                }
                Ok(Some(receipt))
            }
            CatalogueAction::Back => {
                let result = self.command(Command::Close, samples)?;
                self.caller.selection.cancel();
                Ok(result)
            }
        }
    }

    fn focus_catalogue(&mut self, index: usize) -> Result<(), NtStatus> {
        if matches!(self.caller.target()?, Target::Profile { .. })
            && index == self.route.entries().len()
        {
            self.caller.create_focused = true;
        } else {
            self.route.focus(index)?;
            self.caller.create_focused = false;
        }
        self.caller.creation_error = None;
        Ok(())
    }

    /// Frame-boundary frontend consumer for the Create action. The explicit
    /// native player slot is the retained-record slot, not a network participant.
    /// Reads are importer-only; all subsequent edits/constructors are owned.
    pub fn service_profile_creation(
        &mut self,
        runtime: &impl tsfp_engine_fileio::storage_bootstrap::RuntimeRead,
    ) {
        if !self.caller.creation_requested {
            return;
        }
        self.caller.creation_requested = false;
        let attempt = (|| {
            let Target::Profile { player } = self.caller.target()? else {
                return Err(status::INVALID_PARAMETER);
            };
            let retained =
                tsfp_engine_fileio::storage_bootstrap::new_profile_candidate(runtime, player)?;
            let identity = tsfp_engine_fileio::storage_bootstrap::profile_identity()?;
            self.begin_profile_name(identity, retained)
        })();
        if let Err(error) = attempt {
            self.caller.creation_error = Some(error);
        }
    }

    /// Owned caller supplies participating players, as the old frontend did
    /// through its pending-player mask. Invalid rosters leave session untouched.
    /// An I/O failure leaves the first participant pending for retry or Back.
    pub fn begin_profile_selection(
        &mut self,
        players: &[usize],
        samples: &mut impl SampleMaps,
    ) -> Result<(), NtStatus> {
        if matches!(self.lifecycle.context(), Context::Storage(_))
            || self.caller.navigation.awaiting_return()
        {
            return Err(status::INVALID_PARAMETER);
        }
        self.caller.selection.begin(players)?;
        self.caller.returned_progress = None;
        self.caller.returned_availability = None;
        self.caller.return_error = None;
        self.open_root(
            tsfp_engine_fileio::storage_route::Root::ProfileTick,
            Target::Profile {
                player: self.caller.selection.next_player().unwrap(),
            },
            samples,
        )
    }

    /// Owned frontend caller supplies retained pages and both return destinations.
    /// Validate before beginning I/O; opening errors retain this navigation and
    /// the pending roster so Back can return safely without adopting a profile.
    pub fn begin_profile_selection_from(
        &mut self,
        players: &[usize],
        history: Vec<crate::storage_caller::RetainedPage>,
        selected: crate::storage_caller::NativePageId,
        cancelled: crate::storage_caller::NativePageId,
        samples: &mut impl SampleMaps,
    ) -> Result<(), NtStatus> {
        let navigation =
            crate::storage_caller::ProfileNavigation::new(history, selected, cancelled)?;
        // The existing barrier validates the roster atomically, but open_root
        // may fail after it becomes active. Keep the native return in that case.
        if self.caller.navigation.awaiting_return()
            || self.caller.selection.active()
            || self.caller.selection.pending_return().is_some()
            || matches!(self.lifecycle.context(), Context::Storage(_))
        {
            return Err(status::INVALID_PARAMETER);
        }
        let result = self.begin_profile_selection(players, samples);
        if self.caller.selection.active() {
            self.caller.navigation = navigation;
        }
        result
    }

    /// Direct owned Players Accept. No controller scan, guest count/flags or
    /// default-name original calls. The catalogue owns missing-profile creation.
    pub fn begin_players_launch(
        &mut self,
        participants: crate::storage_caller::Participants,
        history: Vec<crate::storage_caller::RetainedPage>,
        selected: crate::storage_caller::NativePageId,
        cancelled: crate::storage_caller::NativePageId,
        samples: &mut impl SampleMaps,
    ) -> Result<(), NtStatus> {
        if self.caller.players_launch.pending().is_some()
            || self.caller.navigation.awaiting_return()
            || self.caller.selection.active()
            || self.caller.selection.pending_return().is_some()
            || matches!(self.lifecycle.context(), Context::Storage(_))
        {
            return Err(status::INVALID_PARAMETER);
        }
        let players = participants.players();
        let result =
            self.begin_profile_selection_from(&players, history, selected, cancelled, samples);
        // Catalogue I/O can fail after the barrier starts: retain assignments
        // for retry/Back along with the validated navigation, never on rejection.
        if self.caller.navigation.awaiting_return() && self.caller.selection.active() {
            self.caller.players_launch.begin(participants)?;
        }
        result
    }

    /// Native consumer for the participant return's progression contract.
    /// Failed preparation retains the one-shot return and adopted records.
    pub fn prepare_profile_return(
        &mut self,
        runtime: &impl tsfp_engine_fileio::storage_bootstrap::RuntimeRead,
    ) {
        use crate::storage_caller::ProfileSelectionReturn;
        if self.caller.returned_progress.is_some() {
            return;
        }
        let Some(ProfileSelectionReturn::Selected(keys)) = self.caller.selection.pending_return()
        else {
            return;
        };
        let attempt = (|| {
            let mut profiles = Vec::new();
            for (player, key) in keys.iter().enumerate() {
                if let Some(key) = key {
                    let profile = self.caller.players[player]
                        .as_ref()
                        .ok_or(status::INVALID_PARAMETER)?;
                    if &profile.saved.key != key {
                        return Err(status::INVALID_PARAMETER);
                    }
                    profiles.push(tsfp_engine_fileio::profile_progress::ProfileProgress::read(
                        &profile.saved.profile,
                    )?);
                }
            }
            let progress =
                tsfp_engine_fileio::profile_progress::ProfileProgress::combine(&profiles, runtime)?;
            let availability =
                tsfp_engine_fileio::profile_progress::AvailabilityPolicy::read(runtime)?.mask();
            Ok::<_, NtStatus>((progress, availability))
        })();
        match attempt {
            Ok((progress, availability)) => {
                self.caller.returned_progress = Some(progress);
                self.caller.returned_availability = Some(availability);
                self.caller.return_error = None;
            }
            Err(error) => {
                self.caller.return_error = Some(error);
            }
        }
    }
    /// Bound frontend navigation progresses at the frame boundary, even after
    /// adoption has closed the catalogue and released window input. Unbound
    /// diagnostic callers still consume their own receipt explicitly.
    pub fn service_profile_navigation(
        &mut self,
        runtime: &impl tsfp_engine_fileio::storage_bootstrap::RuntimeRead,
    ) -> Option<crate::storage_caller::ProfileSelectionReturn> {
        self.prepare_profile_return(runtime);
        if !self.caller.navigation.awaiting_return() {
            return None;
        }
        self.take_profile_return()
    }

    /// A pending bound return owns an explicit presentation after catalogue
    /// closure. No catalogue rows or new-profile action are fabricated here.
    pub fn profile_return_view(&self) -> Option<crate::storage_caller::CatalogueView> {
        use crate::storage_caller::ProfileSelectionReturn;
        if !self.caller.navigation.awaiting_return() {
            return None;
        }
        let ProfileSelectionReturn::Selected(keys) = self.caller.selection.pending_return()? else {
            return None;
        };
        let player = keys.iter().position(Option::is_some)?;
        Some(crate::storage_caller::CatalogueView {
            target: Target::Profile { player },
            rows: Vec::new(),
            sample_maps: Vec::new(),
            map_source_choice: false,
            cursor: Some(0),
            error: self.caller.return_error,
        })
    }

    pub fn take_profile_return(&mut self) -> Option<crate::storage_caller::ProfileSelectionReturn> {
        if matches!(
            self.caller.selection.pending_return(),
            Some(crate::storage_caller::ProfileSelectionReturn::Selected(_))
        ) && self.caller.returned_progress.is_none()
        {
            return None;
        }
        if let Some(result) = self.caller.selection.pending_return() {
            if let Err(error) = self.caller.navigation.complete(result) {
                self.caller.return_error = Some(error);
                return None;
            }
        }
        let result = self.caller.selection.take_return()?;
        self.caller.players_launch.complete(&result);
        Some(result)
    }

    /// Begin native name entry from the current participant's catalogue.
    /// Identity is supplied separately by the runtime allocator, never the label.
    fn editor_navigation_available(&self) -> Result<(), NtStatus> {
        if self
            .editor_navigation
            .as_ref()
            .is_some_and(|navigation| navigation.pending() || navigation.has_return())
            || self.caller.selection.active()
            || self.caller.navigation.awaiting_return()
            || self
                .caller
                .target()
                .is_ok_and(|target| target != Target::Map)
            || matches!(
                self.lifecycle.context(),
                Context::Storage(Target::Profile { .. })
            )
        {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(())
    }
    /// Caller supplies owned return targets; an I/O failure keeps this context
    /// for direct Back/retry. Completion publishes only after real adoption.
    pub fn begin_editor_load_from(
        &mut self,
        history: Vec<crate::storage_caller::RetainedPage>,
        completed: crate::storage_caller::NativePageId,
        cancelled: crate::storage_caller::NativePageId,
        samples: &mut impl SampleMaps,
    ) -> Result<(), NtStatus> {
        self.editor_navigation_available()?;
        let navigation = crate::editor_navigation::EditorNavigation::new(
            crate::editor_navigation::EditorOperation::Load,
            history,
            completed,
            cancelled,
        )?;
        self.editor_navigation = Some(navigation);
        self.open_root(
            tsfp_engine_fileio::storage_route::Root::MapCatalogue,
            Target::Map,
            samples,
        )
    }
    /// Begin the editor's native content-source choice. The page state and
    /// subsequent catalogue are Rust-owned; no legacy device page is entered.
    pub fn begin_editor_load_choice_from(
        &mut self,
        history: Vec<crate::storage_caller::RetainedPage>,
        completed: crate::storage_caller::NativePageId,
        cancelled: crate::storage_caller::NativePageId,
        samples: &mut impl SampleMaps,
    ) -> Result<(), NtStatus> {
        self.editor_navigation_available()?;
        let navigation = crate::editor_navigation::EditorNavigation::new(
            crate::editor_navigation::EditorOperation::Load,
            history,
            completed,
            cancelled,
        )?;
        let previous_navigation = self.editor_navigation.take();
        self.editor_navigation = Some(navigation);
        if let Err(error) = self.command(Command::OpenMapSourceChoice, samples) {
            self.editor_navigation = previous_navigation;
            return Err(error);
        }
        Ok(())
    }
    pub fn begin_editor_save_as_from(
        &mut self,
        directory: String,
        history: Vec<crate::storage_caller::RetainedPage>,
        completed: crate::storage_caller::NativePageId,
        cancelled: crate::storage_caller::NativePageId,
    ) -> Result<(), NtStatus> {
        self.editor_navigation_available()?;
        let navigation = crate::editor_navigation::EditorNavigation::new(
            crate::editor_navigation::EditorOperation::Save,
            history,
            completed,
            cancelled,
        )?;
        self.begin_map_name(directory)?;
        self.editor_navigation = Some(navigation);
        Ok(())
    }
    pub fn take_editor_return(&mut self) -> Option<crate::editor_navigation::EditorReturn> {
        self.editor_navigation.as_mut()?.take_return()
    }
    pub fn has_editor_return(&self) -> bool {
        self.editor_navigation
            .as_ref()
            .is_some_and(|navigation| navigation.pending_return().is_some())
    }
    /// Consume a one-shot editor return only when the named native page is
    /// current and the returned document still matches the adopted lifecycle.
    pub fn consume_editor_return_for(&mut self, page: crate::storage_caller::NativePageId) -> bool {
        use crate::editor_navigation::EditorReturn;
        let Some(navigation) = self.editor_navigation.as_ref() else {
            return false;
        };
        let Some(result) = navigation.pending_return() else {
            return false;
        };
        let destination = match result {
            EditorReturn::Completed { destination, .. }
            | EditorReturn::Cancelled { destination } => destination,
        };
        if destination.id != page {
            return false;
        }
        if let EditorReturn::Completed {
            source, document, ..
        } = result
            && (self.lifecycle.editor() != Some(document)
                || self.lifecycle.editor_source() != Some(source))
        {
            return false;
        }
        let consumed = self
            .editor_navigation
            .as_mut()
            .and_then(|navigation| navigation.take_return())
            .is_some();
        if consumed {
            self.editor_navigation = None;
        }
        consumed
    }
    pub fn editor_returned_page(&self) -> Option<&crate::storage_caller::RetainedPage> {
        self.editor_navigation.as_ref()?.active()
    }
    fn complete_editor_navigation(&mut self, receipt: &Completion) -> Result<(), NtStatus> {
        if receipt.target != Target::Map {
            return Ok(());
        }
        let Some(navigation) = self
            .editor_navigation
            .as_mut()
            .filter(|navigation| navigation.pending())
        else {
            return Ok(());
        };
        let source = &self
            .frontend
            .flow()
            .map()
            .ok_or(status::INVALID_PARAMETER)?
            .source;
        let document = self.lifecycle.editor().ok_or(status::INVALID_PARAMETER)?;
        navigation.complete(receipt, source, document)
    }

    /// Save-as caller owns a real editor document. Name entry does no I/O,
    /// never selects a device, and retains the old document until durable save.
    pub fn begin_map_name(&mut self, directory: String) -> Result<(), NtStatus> {
        if self.caller.profile_name.is_some()
            || self.caller.map_name.is_some()
            || self.caller.selection.active()
            || self
                .caller
                .target()
                .is_ok_and(|target| target != Target::Map)
        {
            return Err(status::INVALID_PARAMETER);
        }
        if self.editor_navigation.as_ref().is_some_and(|navigation| {
            navigation.has_return()
                || (navigation.pending()
                    && navigation.operation() != crate::editor_navigation::EditorOperation::Save)
        }) {
            return Err(status::INVALID_PARAMETER);
        }
        let name = crate::storage_caller::MapName::new(directory)?;
        self.lifecycle
            .editor()
            .ok_or(status::INVALID_PARAMETER)?
            .serialize()?;
        match self.lifecycle.context() {
            Context::Storage(Target::Map) => {}
            Context::Storage(_) => return Err(status::INVALID_PARAMETER),
            _ => self.lifecycle.open(Target::Map)?,
        }
        self.caller.attach(Target::Map)?;
        self.route
            .publish(&tsfp_engine_fileio::storage_frontend::Screen::Closed, None);
        self.caller.map_name = Some(name);
        Ok(())
    }
    pub fn submit_map_name(
        &mut self,
        samples: &mut impl SampleMaps,
    ) -> Result<Completion, NtStatus> {
        let attempt = (|| {
            let command = self
                .caller
                .map_name
                .as_ref()
                .ok_or(status::INVALID_PARAMETER)?
                .command(self.lifecycle.editor().ok_or(status::INVALID_PARAMETER)?)?;
            self.command(command, samples)?
                .ok_or(status::INVALID_PARAMETER)
        })();
        match attempt {
            Ok(receipt) => Ok(receipt),
            Err(error) => {
                if let Some(name) = self.caller.map_name.as_mut() {
                    name.failed(error);
                }
                Err(error)
            }
        }
    }

    pub fn begin_profile_name(
        &mut self,
        directory: String,
        candidate: tsfp_format_profile::Profile,
    ) -> Result<(), NtStatus> {
        let Target::Profile { player } = self.caller.target()? else {
            return Err(status::INVALID_PARAMETER);
        };
        if self.lifecycle.context() != Context::Storage(Target::Profile { player })
            || self.caller.profile_name.is_some()
        {
            return Err(status::INVALID_PARAMETER);
        }
        let name = crate::storage_caller::ProfileName::new(player, directory, candidate)?;
        self.caller.profile_name = Some(name);
        Ok(())
    }

    /// Validation/I/O failures retain editable text and never satisfy a player.
    pub fn submit_profile_name(
        &mut self,
        samples: &mut impl SampleMaps,
    ) -> Result<Completion, NtStatus> {
        let attempt = (|| {
            let command = self
                .caller
                .profile_name
                .as_ref()
                .ok_or(status::INVALID_PARAMETER)?
                .command()?;
            self.command(command, samples)?
                .ok_or(status::INVALID_PARAMETER)
        })();
        let receipt = match attempt {
            Ok(receipt) => receipt,
            Err(error) => {
                if let Some(name) = self.caller.profile_name.as_mut() {
                    name.failed(error);
                }
                return Err(error);
            }
        };
        if let Some(player) = self.caller.selection.next_player() {
            let _ = self.open_root(
                tsfp_engine_fileio::storage_route::Root::ProfileTick,
                Target::Profile { player },
                samples,
            );
        }
        Ok(receipt)
    }

    pub fn load_selected(&mut self, samples: &mut impl SampleMaps) -> Result<Completion, NtStatus> {
        let command = self.route.load_selected()?;
        self.command(command, samples)?
            .ok_or(status::INVALID_PARAMETER)
    }

    /// Explicit diagnostic on an isolated copied save root. Exercises durable
    /// settings I/O and reactivation with the live runtime authentication key.
    /// This is not a menu/root-registration acceptance test.
    pub fn qualify_settings_roundtrip(
        &mut self,
        samples: &mut impl SampleMaps,
    ) -> Result<(), NtStatus> {
        use tsfp_input::settings::SettingEdit;
        let target = Target::Profile { player: 0 };
        self.entry(0x193310, target, EntryAction::DeviceReady, samples)?;
        self.entry(0x192770, target, EntryAction::Open, samples)?;
        let view = self.catalogue_view().ok_or(status::INVALID_PARAMETER)?;
        if view.target != target || view.error.is_some() || view.rows.is_empty() {
            return Err(status::OBJECT_NAME_NOT_FOUND);
        }
        self.catalogue_action(
            crate::storage_caller::CatalogueAction::Focus(
                view.cursor.ok_or(status::INVALID_PARAMETER)?,
            ),
            samples,
        )?;
        self.catalogue_action(crate::storage_caller::CatalogueAction::Step(0), samples)?;
        self.catalogue_action(crate::storage_caller::CatalogueAction::Accept, samples)?;
        let active = self.lifecycle.player(0).ok_or(status::INVALID_PARAMETER)?;
        let original = active.extension.clone();
        let mut edited = original.clone();
        edited
            .apply(SettingEdit::InvertPitch(
                !original.bindings.sensitivity.invert_pitch,
            ))
            .map_err(|_| status::INVALID_PARAMETER)?;
        self.entry(
            0x193120,
            target,
            EntryAction::Profile(Command::SaveInput {
                player: 0,
                input: edited.clone(),
            }),
            samples,
        )?;
        self.deactivate(0)?;
        self.entry(0x192770, target, EntryAction::Open, samples)?;
        self.entry(0x192770, target, EntryAction::LoadSelection, samples)?;
        if self
            .lifecycle
            .player(0)
            .is_none_or(|profile| profile.extension != edited || !profile.saved.native_input)
        {
            return Err(status::INVALID_PARAMETER);
        }
        self.entry(
            0x193120,
            target,
            EntryAction::Profile(Command::SaveInput {
                player: 0,
                input: original,
            }),
            samples,
        )?;
        self.deactivate(0)?;
        Ok(())
    }

    /// Optional map receipt for the existing isolated-root diagnostic. No map
    /// is fabricated when the copied catalogue is empty. If present, authenticate
    /// and adopt the real document, save it through progress, and reload it.
    pub fn qualify_map_roundtrip(
        &mut self,
        samples: &mut impl SampleMaps,
    ) -> Result<bool, NtStatus> {
        self.entry(0x32b6a0, Target::Map, EntryAction::Open, samples)?;
        if self.route.entries().is_empty() {
            self.catalogue_action(crate::storage_caller::CatalogueAction::Back, samples)?;
            return Ok(false);
        }
        self.entry(0x32b6a0, Target::Map, EntryAction::LoadSelection, samples)?;
        let original = self
            .lifecycle
            .editor()
            .ok_or(status::INVALID_PARAMETER)?
            .serialize()?;
        self.entry(
            0x32aee0,
            Target::Map,
            EntryAction::SaveEditor { new_save: None },
            samples,
        )?;
        self.entry(0x32b6a0, Target::Map, EntryAction::Open, samples)?;
        self.entry(0x32b6a0, Target::Map, EntryAction::LoadSelection, samples)?;
        if self
            .lifecycle
            .editor()
            .ok_or(status::INVALID_PARAMETER)?
            .serialize()?
            != original
        {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        Ok(true)
    }

    fn publish_profile(&self) {
        if let Some(profile) = self.player.and_then(|p| self.lifecycle.player(p)) {
            if profile.saved.native_input {
                crate::ui_extensions::adopt_profile(profile.extension.clone());
            } else if let Some(launch) = &self.launch_policy {
                crate::ui_extensions::adopt_policy(launch.clone());
            }
        }
    }

    pub fn deactivate(&mut self, player: usize) -> Result<(), NtStatus> {
        self.lifecycle.deactivate(player)?;
        let cancelled_selection = self.caller.selection.invalidate(player);
        self.caller.players[player] = None;
        self.caller.returned_progress = None;
        self.caller.returned_availability = None;
        self.caller.return_error = None;
        let target = Target::Profile { player };
        // Invalidate only this player's outstanding frontend work. Another
        // player or the editor may still own the currently visible catalogue.
        if self.caller.target().ok() == Some(target)
            || (cancelled_selection && matches!(self.caller.target(), Ok(Target::Profile { .. })))
        {
            self.frontend.handle(Command::Close, &mut NoSamples)?;
            if matches!(self.lifecycle.context(), Context::Storage(_)) {
                self.lifecycle.cancel()?;
            }
            self.caller.close();
            self.route.publish(self.frontend.screen(), None);
        } else {
            self.route.invalidate_completion(target);
        }
        if self
            .caller
            .last_completion
            .as_ref()
            .is_some_and(|receipt| receipt.target == target)
        {
            self.caller.last_completion = None;
        }
        if self.player == Some(player) {
            self.player = None;
            if let Some(launch) = &self.launch_policy {
                crate::ui_extensions::adopt_policy(launch.clone());
            } else {
                crate::ui_extensions::clear_profile();
            }
        }
        Ok(())
    }

    pub fn save_editor(
        &mut self,
        new_save: Option<(String, String)>,
        samples: &mut impl SampleMaps,
    ) -> Result<Completion, NtStatus> {
        if self.editor_navigation.as_ref().is_some_and(|navigation| {
            navigation.has_return()
                || (navigation.pending()
                    && navigation.operation() != crate::editor_navigation::EditorOperation::Save)
        }) {
            return Err(status::INVALID_PARAMETER);
        }
        match self.lifecycle.context() {
            Context::Storage(Target::Map) => {}
            Context::Storage(_) => return Err(status::INVALID_PARAMETER),
            _ => self.lifecycle.open(Target::Map)?,
        }
        self.caller.attach(Target::Map)?;
        let result = self
            .lifecycle
            .save_editor(&mut self.frontend, samples, new_save);
        if let Ok(receipt) = &result {
            self.complete_editor_navigation(receipt)?;
            self.caller.consume(&self.lifecycle, receipt)?;
        }
        self.route
            .publish(self.frontend.screen(), result.as_ref().ok().cloned());
        result
    }

    /// Called at the guest-thread frame boundary, before menu actions. Failed
    /// persistence restores the durable bindings and displays the failure.
    pub fn settings_tick(&mut self, samples: &mut impl SampleMaps) {
        // Do not consume an edit before a native profile has been activated.
        let Some(player) = self.player else { return };
        let Some(input) = crate::ui_extensions::take_profile_edit() else {
            return;
        };
        if let Err(status) = self.command(Command::SaveInput { player, input }, samples) {
            self.publish_profile();
            crate::ui_extensions::profile_save_failed(status);
        }
    }
}

#[cfg(test)]
mod map_document_close_identity_tests {
    use super::MapDocumentCloseIdentity;

    #[test]
    fn loaded_document_replacement_invalidates_old_close_token() {
        let mut identity = MapDocumentCloseIdentity::default();
        let first = identity.adopt_loaded_document().unwrap();
        assert_eq!(identity.current(true), Some(first));

        let second = identity.adopt_loaded_document().unwrap();
        assert_ne!(first, second);
        assert_eq!(identity.current(true), Some(second));
    }

    #[test]
    fn missing_document_clears_identity_and_reopening_gets_a_new_token() {
        let mut identity = MapDocumentCloseIdentity::default();
        let first = identity.current(true).unwrap();
        assert_eq!(identity.current(false), None);
        let reopened = identity.current(true).unwrap();
        assert_ne!(first, reopened);
    }
}

/// Direct native inputs shared by controller, keyboard and mouse handlers.
/// The map progress entry requires the editor document already owned by lifecycle.
pub enum EntryAction {
    Open,
    LoadSelection,
    DeviceReady,
    Profile(Command),
    SaveEditor { new_save: Option<(String, String)> },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryResult {
    Screen(tsfp_engine_fileio::storage_frontend::Screen),
    Device(tsfp_engine_fileio::local_storage::LocalDevice),
    Completed(Completion),
}

/// Closing a frontend cannot request sample data. Reject an unexpected request
/// rather than letting profile invalidation read or fabricate a map.
struct NoSamples;
impl SampleMaps for NoSamples {
    fn load_sample(
        &mut self,
        _: u32,
    ) -> Result<tsfp_engine_fileio::storage_flow::MapPayload, NtStatus> {
        Err(status::INVALID_PARAMETER)
    }
}

/// Direct Rust frontend executor. Its only mutation context is the owned session;
/// page/player/profile/action have just been validated against current state.
pub type DestinationHandler =
    fn(&mut StorageSession, crate::storage_caller::DestinationRequest) -> Result<(), NtStatus>;
