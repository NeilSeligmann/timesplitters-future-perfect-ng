//! T2319 owned frontend attachment. Native callers supply a target explicitly;
//! completion installs real profile/editor records, never an empty success.
use tsfp_engine_fileio::{
    map_document::MapDocument,
    storage_flow::{Operation, SampleMapEntry, SaveEntry},
    storage_frontend::{Completion, Target},
    storage_lifecycle::{NativeStorageLifecycle, PlayerProfile},
    vfs::{NtStatus, status},
};
use tsfp_input::settings::InputSettings;

#[derive(Default)]
pub struct StorageCaller {
    target: Option<Target>,
    pub players: [Option<PlayerProfile<InputSettings>>; 4],
    pub editor: Option<MapDocument>,
    pub last_completion: Option<Completion>,
    pub selection: ProfileSelection,
    pub navigation: ProfileNavigation,
    pub players_launch: PlayersLaunch,
    destination: Option<DestinationController>,
    pub profile_name: Option<ProfileName>,
    pub map_name: Option<MapName>,
    pub create_focused: bool,
    pub creation_requested: bool,
    pub creation_error: Option<NtStatus>,
    pub returned_progress: Option<tsfp_engine_fileio::profile_progress::ProfileProgress>,
    pub return_error: Option<NtStatus>,
    pub returned_availability: Option<u32>,
}

/// Owned reconstruction of the profile-menu participant barrier (0x190FD0).
/// A caller keeps its own return navigation; this controller never uses an old
/// context-stack pointer. Only authenticated, adopted choices satisfy a player.
#[derive(Default, Debug)]
pub struct ProfileSelection {
    pending: [bool; 4],
    chosen: [Option<tsfp_engine_fileio::storage_flow::SaveKey>; 4],
    active: bool,
    returned: Option<ProfileSelectionReturn>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileSelectionReturn {
    Selected([Option<tsfp_engine_fileio::storage_flow::SaveKey>; 4]),
    Cancelled,
}
impl ProfileSelection {
    pub fn begin(&mut self, players: &[usize]) -> Result<(), NtStatus> {
        let mut pending = [false; 4];
        if self.active || players.is_empty() || self.returned.is_some() {
            return Err(status::INVALID_PARAMETER);
        }
        for &player in players {
            let slot = pending.get_mut(player).ok_or(status::INVALID_PARAMETER)?;
            if *slot {
                return Err(status::INVALID_PARAMETER);
            }
            *slot = true;
        }
        self.pending = pending;
        self.chosen = Default::default();
        self.active = true;
        Ok(())
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn next_player(&self) -> Option<usize> {
        self.pending.iter().position(|pending| *pending)
    }
    pub fn accepts(&self, player: usize) -> bool {
        self.active && self.pending.get(player).copied().unwrap_or(false)
    }
    pub fn key_available(
        &self,
        player: usize,
        key: &tsfp_engine_fileio::storage_flow::SaveKey,
    ) -> bool {
        !self
            .chosen
            .iter()
            .enumerate()
            .any(|(other, chosen)| other != player && chosen.as_ref() == Some(key))
    }
    fn adopted(&mut self, player: usize, key: tsfp_engine_fileio::storage_flow::SaveKey) {
        // StorageCaller validates the actual adopted record first.
        if !self.accepts(player) {
            return;
        }
        self.chosen[player] = Some(key);
        self.pending[player] = false;
        if self.next_player().is_none() {
            self.active = false;
            self.returned = Some(ProfileSelectionReturn::Selected(self.chosen.clone()));
        }
    }
    pub fn cancel(&mut self) {
        if self.active {
            self.pending = [false; 4];
            self.chosen = Default::default();
            self.active = false;
            self.returned = Some(ProfileSelectionReturn::Cancelled);
        }
    }
    /// Back from a failed return cancels navigation without undoing profiles
    /// already adopted durably. The frontend chooses its cancelled destination.
    pub fn cancel_pending_return(&mut self) {
        if self.returned.is_some() {
            self.returned = Some(ProfileSelectionReturn::Cancelled);
        }
    }
    pub fn invalidate(&mut self, player: usize) -> bool {
        if self.pending.get(player).copied().unwrap_or(false)
            || self.chosen.get(player).is_some_and(Option::is_some)
        {
            self.active = true;
            self.cancel();
            return true;
        }
        false
    }
    pub fn pending_return(&self) -> Option<&ProfileSelectionReturn> {
        self.returned.as_ref()
    }
    pub fn take_return(&mut self) -> Option<ProfileSelectionReturn> {
        let result = self.returned.take();
        if result.is_some() {
            self.chosen = Default::default();
        }
        result
    }
}
impl StorageCaller {
    /// Atomic owned destination qualification shared by live renderer/controller
    /// callers. Pending selection or a replaced/deactivated profile is not ready.
    pub fn players_destination(&self) -> Option<(RetainedPage, Participants)> {
        if self.selection.active() || self.navigation.awaiting_return() {
            return None;
        }
        let keys = std::array::from_fn(|player| {
            self.players[player]
                .as_ref()
                .map(|profile| profile.saved.key.clone())
        });
        if !self.players_launch.matches_profiles(&keys) {
            return None;
        }
        Some((
            self.navigation.active()?.clone(),
            self.players_launch.active()?.clone(),
        ))
    }
    /// Dispatch directly to an owned destination handler. Validate at dispatch
    /// time, rather than trusting a renderer snapshot from an earlier frame.
    /// The handler receives the current authenticated profile and its settings.
    pub fn dispatch_players_input<R>(
        &self,
        page: NativePageId,
        source: PlayerInputSource,
        handler: impl FnOnce(&RetainedPage, usize, &PlayerProfile<InputSettings>) -> R,
    ) -> Result<R, NtStatus> {
        let (destination, participants) = self
            .players_destination()
            .ok_or(status::INVALID_PARAMETER)?;
        if destination.id != page {
            return Err(status::INVALID_PARAMETER);
        }
        let player = participants
            .inputs()
            .iter()
            .position(|assigned| *assigned == source)
            .ok_or(status::INVALID_PARAMETER)?;
        let profile = self.players[player]
            .as_ref()
            .ok_or(status::INVALID_PARAMETER)?;
        Ok(handler(&destination, player, profile))
    }
    /// Install concrete runtime contents on an authenticated native destination.
    /// Row identities and labels belong to the frontend, never legacy pointers.
    pub fn install_destination(
        &mut self,
        page: NativePageId,
        rows: Vec<DestinationRow>,
    ) -> Result<(), NtStatus> {
        let (retained, participants) = self
            .players_destination()
            .ok_or(status::INVALID_PARAMETER)?;
        if retained.id != page
            || rows.is_empty()
            || rows.len() > 4096
            || rows.iter().enumerate().any(|(i, row)| {
                row.id == 0
                    || row.label.is_empty()
                    || rows[..i].iter().any(|previous| previous.id == row.id)
            })
        {
            return Err(status::INVALID_PARAMETER);
        }
        let cursor = rows
            .get(retained.cursor as usize)
            .filter(|row| row.enabled)
            .map(|_| retained.cursor as usize)
            .or_else(|| rows.iter().position(|row| row.enabled));
        let owned = DestinationController {
            page,
            participants,
            keys: self.players_launch.active_keys.clone(),
            rows,
            cursor,
        };
        if let Some(cursor) = cursor {
            self.navigation.active.as_mut().unwrap().cursor = cursor as u32;
        }
        self.destination = Some(owned);
        Ok(())
    }
    pub fn destination_view(&self) -> Option<DestinationView> {
        let (page, participants) = self.players_destination()?;
        let owned = self.destination.as_ref()?;
        if owned.page != page.id
            || owned.participants != participants
            || owned.keys != self.players_launch.active_keys
        {
            return None;
        }
        Some(DestinationView {
            page,
            rows: owned.rows.clone(),
            cursor: owned.cursor,
        })
    }
    /// Shared direct keyboard/controller/pointer controller. Step clamps to
    /// enabled rows; Focus uses stable row identity rather than a stale index.
    pub fn destination_action(
        &mut self,
        page: NativePageId,
        source: PlayerInputSource,
        action: DestinationAction,
    ) -> Result<Option<DestinationRequest>, NtStatus> {
        let (player, key) = self.dispatch_players_input(page, source, |_, player, profile| {
            (player, profile.saved.key.clone())
        })?;
        self.destination_view().ok_or(status::INVALID_PARAMETER)?;
        let owned = self.destination.as_mut().unwrap();
        match action {
            DestinationAction::Step(delta) => {
                let enabled: Vec<usize> = owned
                    .rows
                    .iter()
                    .enumerate()
                    .filter_map(|(i, row)| row.enabled.then_some(i))
                    .collect();
                if let Some(position) = owned
                    .cursor
                    .and_then(|cursor| enabled.iter().position(|i| *i == cursor))
                {
                    let next = (position as i64 + i64::from(delta))
                        .clamp(0, enabled.len() as i64 - 1) as usize;
                    owned.cursor = Some(enabled[next]);
                }
            }
            DestinationAction::Focus(id) => {
                owned.cursor = Some(
                    owned
                        .rows
                        .iter()
                        .position(|row| row.id == id && row.enabled)
                        .ok_or(status::INVALID_PARAMETER)?,
                );
            }
            DestinationAction::Accept => {
                let row = owned
                    .cursor
                    .and_then(|cursor| owned.rows.get(cursor))
                    .filter(|row| row.enabled)
                    .ok_or(status::INVALID_PARAMETER)?;
                return Ok(Some(DestinationRequest {
                    page,
                    player,
                    profile: key,
                    action: DestinationRequestAction::Activate(row.id),
                }));
            }
            DestinationAction::Back => {
                return Ok(Some(DestinationRequest {
                    page,
                    player,
                    profile: key,
                    action: DestinationRequestAction::Back,
                }));
            }
        }
        if let Some(cursor) = owned.cursor {
            self.navigation.active.as_mut().unwrap().cursor = cursor as u32;
        }
        Ok(None)
    }
    /// Atomic pointer admission: geometry and source must still be current before
    /// focus changes. Back takes priority; a rejected click never falls through
    /// to Accept on the previously selected row.
    pub fn destination_pointer(
        &mut self,
        rendered: &DestinationView,
        source: PlayerInputSource,
        focus: Option<u64>,
        accept: bool,
        back: bool,
    ) -> Result<Option<DestinationRequest>, NtStatus> {
        let current = self.destination_view().ok_or(status::INVALID_PARAMETER)?;
        if current.page.id != rendered.page.id || current.rows != rendered.rows {
            return Err(status::INVALID_PARAMETER);
        }
        self.dispatch_players_input(current.page.id, source, |_, _, _| ())?;
        if back {
            return self.destination_action(current.page.id, source, DestinationAction::Back);
        }
        let Some(id) = focus else {
            return Ok(None);
        };
        self.destination_action(current.page.id, source, DestinationAction::Focus(id))?;
        if accept {
            self.destination_action(current.page.id, source, DestinationAction::Accept)
        } else {
            Ok(None)
        }
    }
    pub fn name_text(&self) -> Option<&str> {
        self.profile_name
            .as_ref()
            .map(ProfileName::text)
            .or_else(|| self.map_name.as_ref().map(MapName::text))
    }
    pub fn name_identity(&self) -> Option<&str> {
        self.profile_name
            .as_ref()
            .map(|name| name.directory.as_str())
            .or_else(|| self.map_name.as_ref().map(|name| name.directory.as_str()))
    }
    pub fn append_name(&mut self, text: &str) -> Result<(), NtStatus> {
        if let Some(name) = self.profile_name.as_mut() {
            name.append(text)
        } else if let Some(name) = self.map_name.as_mut() {
            name.append(text)
        } else {
            Err(status::INVALID_PARAMETER)
        }
    }
    pub fn backspace_name(&mut self) {
        if let Some(name) = self.profile_name.as_mut() {
            name.backspace();
        } else if let Some(name) = self.map_name.as_mut() {
            name.backspace();
        }
    }
    pub fn target(&self) -> Result<Target, NtStatus> {
        self.target.ok_or(status::INVALID_PARAMETER)
    }
    pub fn attach(&mut self, target: Target) -> Result<(), NtStatus> {
        if matches!(target, Target::Profile { player } if player >= 4)
            || self.target.is_some_and(|current| current != target)
        {
            return Err(status::INVALID_PARAMETER);
        }
        self.target = Some(target);
        Ok(())
    }
    pub fn close(&mut self) {
        self.profile_name = None;
        self.map_name = None;
        self.create_focused = false;
        self.creation_requested = false;
        self.creation_error = None;
        self.target = None;
    }
    /// Called only after authenticated I/O and lifecycle adoption. Validate the
    /// receipt against the actual adopted record before publishing to consumers.
    pub fn consume(
        &mut self,
        lifecycle: &NativeStorageLifecycle,
        receipt: &Completion,
    ) -> Result<(), NtStatus> {
        if self.target.is_some_and(|target| target != receipt.target) {
            return Err(status::INVALID_PARAMETER);
        }
        match (receipt.target, receipt.operation) {
            (Target::Profile { player }, Operation::ProfileLoad | Operation::ProfileSave) => {
                let profile = lifecycle.player(player).ok_or(status::INVALID_PARAMETER)?;
                if receipt.key.as_ref() != Some(&profile.saved.key) {
                    return Err(status::INVALID_PARAMETER);
                }
                self.players[player] = Some(profile.clone());
                self.selection.adopted(player, profile.saved.key.clone());
            }
            (Target::Map, Operation::MapLoad | Operation::MapSave) => {
                if lifecycle.editor_dirty() {
                    return Err(status::INVALID_PARAMETER);
                }
                self.editor = Some(lifecycle.editor().ok_or(status::INVALID_PARAMETER)?.clone());
            }
            _ => return Err(status::INVALID_PARAMETER),
        }
        self.last_completion = Some(receipt.clone());
        self.close();
        Ok(())
    }
}

/// The action type is shared with the Rust device-to-UI input layer.
pub use tsfp_ui_input::owned_catalogue::CatalogueAction;

/// Immutable render snapshot: labels/errors come from the runtime catalogue.
#[derive(Clone, Debug)]
pub struct CatalogueView {
    pub target: Target,
    pub rows: Vec<SaveEntry>,
    pub sample_maps: Vec<SampleMapEntry>,
    pub map_source_choice: bool,
    pub cursor: Option<usize>,
    pub error: Option<NtStatus>,
}

/// Owned text entry. Runtime identity and profile defaults come from the caller;
/// display text never becomes a filesystem path. Failed edits are atomic.
#[derive(Clone)]
pub struct ProfileName {
    player: usize,
    directory: String,
    candidate: tsfp_format_profile::Profile,
    text: String,
    error: Option<NtStatus>,
}
impl ProfileName {
    pub fn new(
        player: usize,
        directory: String,
        candidate: tsfp_format_profile::Profile,
    ) -> Result<Self, NtStatus> {
        if player >= 4 || candidate.encode().is_err() {
            return Err(status::INVALID_PARAMETER);
        }
        tsfp_engine_fileio::storage_flow::SaveKey::new(&directory, "tsftrpft")?;
        Ok(Self {
            player,
            directory,
            candidate,
            text: String::new(),
            error: None,
        })
    }
    pub fn error(&self) -> Option<NtStatus> {
        self.error
    }
    pub fn failed(&mut self, error: NtStatus) {
        self.error = Some(error);
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn replace(&mut self, text: String) -> Result<(), NtStatus> {
        if !text.is_empty() {
            tsfp_engine_fileio::storage_flow::encode_save_label(&text)?;
        }
        self.text = text;
        self.error = None;
        Ok(())
    }
    pub fn append(&mut self, text: &str) -> Result<(), NtStatus> {
        self.replace(format!("{}{}", self.text, text))
    }
    pub fn backspace(&mut self) {
        self.text.pop();
        self.error = None;
    }
    pub fn command(&self) -> Result<tsfp_engine_fileio::storage_frontend::Command, NtStatus> {
        tsfp_engine_fileio::storage_flow::encode_save_label(&self.text)?;
        Ok(
            tsfp_engine_fileio::storage_frontend::Command::CreateProfile {
                player: self.player,
                directory: self.directory.clone(),
                label: self.text.clone(),
                candidate: self.candidate.clone(),
            },
        )
    }
}

/// Native page identity, supplied by the owned frontend, never a guest pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativePageId(pub u64);

/// Retained presentation state. The frontend owns page contents and resources;
/// moving/dropping these records never calls a lifted widget destructor.
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedPage {
    pub id: NativePageId,
    pub cursor: u32,
    pub scroll: f32,
}

/// Owned context return slice of 0x79B70/0x79860/0x70CC0. A destination
/// is selected explicitly by the caller; no original callback address is used.
#[derive(Default, Debug)]
pub struct ProfileNavigation {
    history: Vec<RetainedPage>,
    active: Option<RetainedPage>,
    selected: Option<NativePageId>,
    cancelled: Option<NativePageId>,
}
impl ProfileNavigation {
    /// Validate a complete retained context before replacing any existing one.
    /// Both return targets must exist: an unknown target must never silently
    /// create a page whose original builder/handler has not been reconstructed.
    pub fn new(
        history: Vec<RetainedPage>,
        selected: NativePageId,
        cancelled: NativePageId,
    ) -> Result<Self, NtStatus> {
        if history.is_empty() || history.len() > 60 {
            return Err(status::INVALID_PARAMETER);
        }
        for (index, page) in history.iter().enumerate() {
            if page.id.0 == 0
                || !page.scroll.is_finite()
                || history[..index]
                    .iter()
                    .any(|previous| previous.id == page.id)
            {
                return Err(status::INVALID_PARAMETER);
            }
        }
        if !history.iter().any(|page| page.id == selected)
            || !history.iter().any(|page| page.id == cancelled)
        {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(Self {
            history,
            active: None,
            selected: Some(selected),
            cancelled: Some(cancelled),
        })
    }
    pub fn active(&self) -> Option<&RetainedPage> {
        self.active.as_ref()
    }
    pub fn history(&self) -> &[RetainedPage] {
        &self.history
    }
    pub fn awaiting_return(&self) -> bool {
        self.selected.is_some()
    }
    /// Return to an existing retained page, removing the target and everything
    /// above it from history. Cursor/scroll remain attached to that page. This
    /// consumes the navigation request exactly once, including cancellation.
    pub fn complete(&mut self, result: &ProfileSelectionReturn) -> Result<(), NtStatus> {
        if !self.awaiting_return() {
            return Ok(());
        }
        self.finish(matches!(result, ProfileSelectionReturn::Selected(_)))
    }
    /// Common owned retained-page return used by profile and Map controllers.
    /// `completed` names successful operation versus explicit cancellation;
    /// there is no invented profile receipt for an editor navigation event.
    pub fn finish(&mut self, completed: bool) -> Result<(), NtStatus> {
        if !self.awaiting_return() {
            return Ok(());
        }
        let destination = if completed {
            self.selected
        } else {
            self.cancelled
        }
        .ok_or(status::INVALID_PARAMETER)?;
        let index = self
            .history
            .iter()
            .position(|page| page.id == destination)
            .ok_or(status::INVALID_PARAMETER)?;
        let target = self.history[index].clone();
        self.history.truncate(index);
        self.active = Some(target);
        self.selected = None;
        self.cancelled = None;
        Ok(())
    }
}

/// Native Players controller input. Keyboard/mouse is a real source, never an
/// invented pad port. The caller supplies the complete assignment explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerInputSource {
    KeyboardMouse,
    Controller(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participants {
    inputs: Vec<PlayerInputSource>,
}
impl Participants {
    pub fn new(inputs: Vec<PlayerInputSource>) -> Result<Self, NtStatus> {
        if inputs.is_empty()
            || inputs.len() > 4
            || inputs
                .iter()
                .enumerate()
                .any(|(i, source)| inputs[..i].contains(source))
        {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(Self { inputs })
    }
    pub fn inputs(&self) -> &[PlayerInputSource] {
        &self.inputs
    }
    pub fn players(&self) -> Vec<usize> {
        (0..self.inputs.len()).collect()
    }
}

/// Owned accept/return portion of Players 0x2D2260. Assignments publish only
/// after authenticated profile adoption and prepared native destination return.
/// Cancellation preserves the last completed assignment.
#[derive(Default)]
pub struct PlayersLaunch {
    pending: Option<Participants>,
    active: Option<Participants>,
    active_keys: [Option<tsfp_engine_fileio::storage_flow::SaveKey>; 4],
}
impl PlayersLaunch {
    pub fn pending(&self) -> Option<&Participants> {
        self.pending.as_ref()
    }
    pub fn active(&self) -> Option<&Participants> {
        self.active.as_ref()
    }
    /// A completed assignment remains bound to the authenticated profile that
    /// satisfied it. Presence alone cannot qualify a subsequently replaced slot.
    pub fn matches_profiles(
        &self,
        keys: &[Option<tsfp_engine_fileio::storage_flow::SaveKey>; 4],
    ) -> bool {
        self.pending.is_none()
            && self.active.as_ref().is_some_and(|participants| {
                (0..participants.inputs.len()).all(|player| {
                    keys[player].is_some() && keys[player] == self.active_keys[player]
                })
            })
    }
    pub fn begin(&mut self, participants: Participants) -> Result<(), NtStatus> {
        if self.pending.is_some() {
            return Err(status::INVALID_PARAMETER);
        }
        self.pending = Some(participants);
        Ok(())
    }
    pub fn complete(&mut self, result: &ProfileSelectionReturn) {
        let Some(participants) = self.pending.take() else {
            return;
        };
        if let ProfileSelectionReturn::Selected(keys) = result {
            // The participant barrier produces one durable key per assignment.
            // A mismatched external receipt must never publish assignments.
            if keys.iter().enumerate().all(|(i, key)| {
                key.is_some() == (i < participants.inputs.len())
                    && (key.is_none() || !keys[..i].contains(key))
            }) {
                self.active_keys = keys.clone();
                self.active = Some(participants);
            } else {
                self.pending = Some(participants);
            }
        }
    }
}

/// Runtime destination contents. Disabled rows remain visible but cannot launch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationRow {
    pub id: u64,
    pub label: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct DestinationView {
    pub page: RetainedPage,
    pub rows: Vec<DestinationRow>,
    pub cursor: Option<usize>,
}
struct DestinationController {
    page: NativePageId,
    participants: Participants,
    keys: [Option<tsfp_engine_fileio::storage_flow::SaveKey>; 4],
    rows: Vec<DestinationRow>,
    cursor: Option<usize>,
}
#[derive(Clone, Copy, Debug)]
pub enum DestinationAction {
    Step(i32),
    Focus(u64),
    Accept,
    Back,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DestinationRequestAction {
    Activate(u64),
    Back,
}
/// Immediate qualified request, not future authorization. Delayed mutations
/// must revalidate the adopted profile and current destination before applying.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationRequest {
    pub page: NativePageId,
    pub player: usize,
    pub profile: tsfp_engine_fileio::storage_flow::SaveKey,
    pub action: DestinationRequestAction,
}

/// Native Map save-as text/identity. Its title must fit the serialized settings
/// name as well as SaveMeta; failed edits/submissions retain the prior text.
#[derive(Clone)]
pub struct MapName {
    directory: String,
    text: String,
    error: Option<NtStatus>,
}
impl MapName {
    pub fn new(directory: String) -> Result<Self, NtStatus> {
        tsfp_engine_fileio::storage_flow::SaveKey::new(&directory, "anicemap.mkr")?;
        Ok(Self {
            directory,
            text: String::new(),
            error: None,
        })
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn error(&self) -> Option<NtStatus> {
        self.error
    }
    pub fn failed(&mut self, status: NtStatus) {
        self.error = Some(status);
    }
    pub fn replace(&mut self, text: String) -> Result<(), NtStatus> {
        if !text.is_empty() {
            MapDocument::validate_title(&text)?;
        }
        self.text = text;
        self.error = None;
        Ok(())
    }
    pub fn append(&mut self, text: &str) -> Result<(), NtStatus> {
        self.replace(format!("{}{}", self.text, text))
    }
    pub fn backspace(&mut self) {
        self.text.pop();
        self.error = None;
    }
    pub fn command(
        &self,
        document: &MapDocument,
    ) -> Result<tsfp_engine_fileio::storage_frontend::Command, NtStatus> {
        Ok(tsfp_engine_fileio::storage_frontend::Command::SaveMap {
            directory: self.directory.clone(),
            label: self.text.clone(),
            payload: document.with_title(&self.text)?.serialize()?,
        })
    }
}
