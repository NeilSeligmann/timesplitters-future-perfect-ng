//! T2331: readable front-end transitions over Rust-owned values.
//!
//! `FrontendEffects` is the attachment to page construction, sound and mode services.
//! Tier 2 page-topology-qualified handlers are native by default, with per-handler A/B.
//! The guest adapter is a TEMPORARY-SHIM while those neighbours retain guest
//! state (removal tracked by T2044 S3); no transition decision reads guest offsets.

use crate::native_page::{GuestUi, PageRef};
use tsfp_ui_model::arcade_challenge::{ArcadeCategoryId, ChallengeSetupAttempt, SelectedChallenge};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    Story,
    Arcade,
    Challenge,
    ChallengeLocked,
    NetworkSetup,
    NetworkResume,
    StoryMissions,
    /// T2431: the Story confirm page (descriptor 0x5238C0), opened by the difficulty page.
    StoryConfirm,
    /// T2431: the page 0x5236DC whose tick 0x2D3BF0 starts the Story (shared with Credits).
    StoryStart,
    ArcadeCustom,
    ArcadeLeague,
    ChallengeMissions,
    Options,
    Cheats,
    Controls,
    Credits,
    Gallery,
    MapMaker,
    PlayerProgress,
    Settings,
    Extras,
    Main,
    Players,
}

impl Destination {
    fn descriptor(self) -> u32 {
        match self {
            Self::StoryMissions => 0x5238D0,
            Self::StoryConfirm => 0x5238C0,
            Self::StoryStart => 0x5236DC,
            Self::ArcadeCustom => 0x523890,
            Self::ArcadeLeague => 0x523880,
            Self::ChallengeMissions => 0x523860,
            Self::Options => 0x52377C,
            Self::Cheats => 0x523930,
            Self::Controls => 0x524A40,
            Self::Credits => 0x5236DC,
            Self::Gallery => 0x523920,
            Self::Story => 0x5238E0,
            Self::Arcade => 0x5238A0,
            Self::Challenge => 0x523850,
            Self::ChallengeLocked => 0x523840,
            Self::NetworkSetup => 0x4E92C4,
            Self::NetworkResume => 0x524588,
            Self::MapMaker => 0x523830,
            Self::PlayerProgress => 0x523940,
            Self::Settings => 0x523910,
            Self::Extras => 0x524578,
            Self::Main => 0x524598,
            Self::Players => 0x5246CC,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontendState {
    pub selected: u32,
    pub mode: u32,
    pub campaign: u32,
    /// T3404: category identity at `0x78ad38`, distinct from the selected mission below.
    pub challenge_category: ArcadeCategoryId,
    /// T3404: owned mission selection, distinct from the selected category.
    pub selected_challenge: Option<SelectedChallenge>,
    /// T3404: confirmed Rust-owned progress from accepting the selected Challenge detail page.
    pub challenge_setup: Option<ChallengeSetupAttempt>,
    pub active_player: u32,
    pub story_selected: u32,
    pub arcade_custom: u8,
    /// T2431: difficulty page button index `[0x78AD54]` (the third Story selector).
    pub story_selector: u32,
    /// T2431: Story page scroll `[0x78AE28]` as float bits (reset to 0.0 by the difficulty page).
    pub story_scroll_bits: u32,
    /// T2431: Story page flag bytes `[0x78AE1C]`, `[0x78AE1D]` (cleared by the difficulty page).
    pub story_flags: [u8; 2],
}

impl Default for FrontendState {
    fn default() -> Self {
        Self {
            selected: 0,
            mode: 0,
            campaign: 0,
            challenge_category: ArcadeCategoryId::new(0),
            selected_challenge: None,
            challenge_setup: None,
            active_player: 0,
            story_selected: 0,
            arcade_custom: 0,
            story_selector: 0,
            story_scroll_bits: 0,
            story_flags: [0; 2],
        }
    }
}

/// Services beyond the handler frontier. Implementations may construct original
/// pages while the handler itself owns the selection and mode decisions.
pub trait FrontendEffects {
    fn sound(&mut self, id: u32);
    fn challenge_available(&mut self) -> bool;
    fn prepare_network(&mut self);
    fn network_can_resume(&mut self) -> bool;
    fn publish_selection(&mut self, state: &FrontendState);
    fn open(&mut self, destination: Destination);
    fn reset_options(&mut self);
    fn snapshot_controls(&mut self, player: u32);
    fn profiles_back(&mut self, destination: Destination);
}

/// Original 0x2D0A60: remember the row id, then choose one of eight destinations.
/// An unknown id still plays the sound and publishes selection, but opens nothing.
pub fn main_select(state: &mut FrontendState, row: u32, effects: &mut dyn FrontendEffects) {
    effects.sound(0xD1E);
    state.selected = row;
    // Publication precedes the availability query, as in the original.
    effects.publish_selection(state);
    let destination = match row {
        0 => Destination::Story,
        1 => Destination::Arcade,
        2 => {
            if effects.challenge_available() {
                Destination::Challenge
            } else {
                state.mode = 7;
                effects.publish_selection(state);
                Destination::ChallengeLocked
            }
        }
        3 => {
            state.mode = 13;
            effects.publish_selection(state);
            effects.prepare_network();
            if effects.network_can_resume() {
                Destination::NetworkResume
            } else {
                Destination::NetworkSetup
            }
        }
        4 => Destination::MapMaker,
        5 => Destination::PlayerProgress,
        6 => Destination::Settings,
        7 => Destination::Extras,
        _ => return,
    };
    effects.open(destination);
}

/// Handler identity is distinct from cursor position: Story uses row ids, Arcade
/// and Extras install per-row callbacks, Settings has both row and back callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontendAction {
    StorySelect(u32),
    /// T2431, original 0x2CAD20: difficulty button `selector` of the Story page.
    StoryDifficulty(u32),
    /// T2431, original 0x2BEAA0: the confirm button of the Story page.
    StoryConfirm,
    ArcadeCustom,
    ArcadeLeague,
    ChallengeSelect(ArcadeCategoryId),
    SettingsOptions,
    SettingsCheats,
    SettingsControls,
    SettingsBack,
    ExtrasCredits,
    ExtrasGallery,
    MainBack,
    ReturnMain,
}

pub fn transition(
    state: &mut FrontendState,
    action: FrontendAction,
    effects: &mut dyn FrontendEffects,
) {
    use FrontendAction::*;
    let destination = match action {
        StorySelect(campaign) => {
            state.mode = 4;
            effects.publish_selection(state);
            effects.sound(0xD1E);
            state.campaign = campaign;
            state.story_selected = 1;
            Destination::StoryMissions
        }
        StoryDifficulty(selector) => {
            state.story_scroll_bits = 0;
            state.story_selector = selector;
            effects.publish_selection(state);
            effects.sound(0xD1E);
            state.story_flags = [0, 0];
            Destination::StoryConfirm
        }
        StoryConfirm => Destination::StoryStart,
        ArcadeCustom => {
            state.mode = 5;
            state.arcade_custom = 1;
            Destination::ArcadeCustom
        }
        ArcadeLeague => {
            state.mode = 6;
            Destination::ArcadeLeague
        }
        ChallengeSelect(category) => {
            state.challenge_category = category;
            Destination::ChallengeMissions
        }
        SettingsOptions => {
            effects.reset_options();
            Destination::Options
        }
        SettingsCheats => Destination::Cheats,
        SettingsControls => {
            effects.snapshot_controls(state.active_player);
            effects.sound(0xD1E);
            Destination::Controls
        }
        SettingsBack => {
            state.active_player = 0;
            effects.publish_selection(state);
            effects.profiles_back(Destination::Main);
            return;
        }
        ExtrasCredits => {
            state.mode = 10;
            Destination::Credits
        }
        ExtrasGallery => Destination::Gallery,
        MainBack => Destination::Players,
        ReturnMain => Destination::Main,
    };
    effects.publish_selection(state);
    effects.open(destination);
}

struct GuestEffects<'a> {
    guest: &'a mut dyn GuestUi,
    published: FrontendState,
}
impl FrontendEffects for GuestEffects<'_> {
    fn sound(&mut self, id: u32) {
        self.guest.call(0x21780, &[id]);
    }
    fn challenge_available(&mut self) -> bool {
        self.guest.call(0x32030, &[]).unwrap_or(0) != 0
    }
    fn prepare_network(&mut self) {
        self.guest.call(0x1957C0, &[]);
    }
    fn network_can_resume(&mut self) -> bool {
        self.guest
            .read32(0x784014)
            .and_then(|record| self.guest.read32(record))
            == Some(0xC77667)
    }
    fn publish_selection(&mut self, state: &FrontendState) {
        // Publish only changed fields; services may update neighbouring globals.
        for (address, before, after) in [
            (0x761400, self.published.selected, state.selected),
            (0x78AD2C, self.published.mode, state.mode),
            (0x78AD48, self.published.campaign, state.campaign),
            (
                0x78AD38,
                self.published.challenge_category.get(),
                state.challenge_category.get(),
            ),
            (0x78AD34, self.published.active_player, state.active_player),
        ] {
            if before != after {
                self.guest.write32(address, after);
            }
        }
        if self.published.story_selected != state.story_selected {
            self.guest.write32(0x78AE08, state.story_selected);
        }
        if self.published.story_selector != state.story_selector {
            self.guest.write32(0x78AD54, state.story_selector);
        }
        if self.published.story_scroll_bits != state.story_scroll_bits {
            self.guest.write32(0x78AE28, state.story_scroll_bits);
        }
        for (index, address) in [0x78AE1C_u32, 0x78AE1D].into_iter().enumerate() {
            if self.published.story_flags[index] != state.story_flags[index] {
                write_byte(self.guest, address, state.story_flags[index]);
            }
        }
        if self.published.arcade_custom != state.arcade_custom {
            write_byte(self.guest, 0x78ADAF, state.arcade_custom);
        }
        self.published = state.clone();
    }
    fn reset_options(&mut self) {
        self.guest.call(0x2C3300, &[]);
    }
    fn snapshot_controls(&mut self, player: u32) {
        let record = self.guest.call(0x69620, &[player]).unwrap_or(0);
        let viewport = self.guest.read32(0x7BA9E0).unwrap_or(0);
        let target = 0x7BA364 + viewport * 0x1E0;
        // 0x2CBE50 copies precisely 31 words, not the whole profile/control record.
        for word in 0..31 {
            let value = self.guest.read32(record + word * 4).unwrap_or(0);
            self.guest.write32(target + word * 4, value);
        }
        self.guest.write32(0x78963C, 0);
    }
    fn profiles_back(&mut self, destination: Destination) {
        let context = self.guest.read32(0x7613E4).unwrap_or(0);
        self.guest
            .call(0x193BB0, &[context, destination.descriptor()]);
    }
    fn open(&mut self, destination: Destination) {
        let context = self.guest.read32(0x7613E4).unwrap_or(0);
        self.guest
            .call(0x79B70, &[context, destination.descriptor()]);
    }
}

/// Complete original control payload (31 words). Float equality in the gameplay
/// dirty check treats NaN as changed even when its bits are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlsSnapshot(pub [u32; 31]);
impl ControlsSnapshot {
    pub fn changed(&self, saved: &Self, gameplay: bool) -> bool {
        self != saved
            || (gameplay && {
                let current = f32::from_bits(self.0[30]);
                let previous = f32::from_bits(saved.0[30]);
                current != previous
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlsBackState {
    pub gameplay: bool,
    pub changed: bool,
    pub prompt_active: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlsBackAction {
    ConfirmChanges,
    RestoreFrontendParent,
    Pop,
    ResumePrompt,
}
pub fn controls_back_action(state: ControlsBackState) -> ControlsBackAction {
    if state.changed {
        ControlsBackAction::ConfirmChanges
    } else if !state.gameplay {
        ControlsBackAction::RestoreFrontendParent
    } else if state.prompt_active {
        ControlsBackAction::ResumePrompt
    } else {
        ControlsBackAction::Pop
    }
}

fn controls_snapshot(guest: &dyn GuestUi, base: u32) -> ControlsSnapshot {
    ControlsSnapshot(std::array::from_fn(|word| {
        guest.read32(base + word as u32 * 4).unwrap_or(0)
    }))
}

/// 0x2E9FC0, frontend and gameplay back policy. The context/page services remain
/// neighbouring contracts; the callback itself is never invoked in this lane.
fn controls_back(guest: &mut dyn GuestUi) {
    let gameplay = guest.read32(0x79094C).unwrap_or(0) as i32 > 0x64;
    let viewport = guest.read32(0x7BA9E0).unwrap_or(0);
    let saved_base = if gameplay {
        let player = guest.read32(0x78AD34).unwrap_or(0);
        guest.call(0x69620, &[player]).unwrap_or(0)
    } else {
        let root = guest.read32(0x7B0C7C).unwrap_or(0);
        guest.read32(root + 0x1500).unwrap_or(0)
    };
    let current = controls_snapshot(guest, 0x7BA364 + viewport * 0x1E0);
    let saved = controls_snapshot(guest, saved_base);
    let state = ControlsBackState {
        gameplay,
        changed: current.changed(&saved, gameplay),
        prompt_active: guest.read32(0x78963C).unwrap_or(0) != 0,
    };
    let context = guest
        .read32(if gameplay {
            0x7613E4
        } else {
            0x761BBC + viewport * 4
        })
        .unwrap_or(0);
    match controls_back_action(state) {
        ControlsBackAction::ConfirmChanges => {
            guest.call(0x79B70, &[context, 0x524A30]);
        }
        ControlsBackAction::RestoreFrontendParent => {
            crate::page_stack::pop(guest, PageRef(context));
            // Pop may change the context table; the original reloads it.
            let context = guest.read32(0x761BBC + viewport * 4).unwrap_or(0);
            guest.call(0x79B70, &[context, 0x524700]);
        }
        ControlsBackAction::Pop => {
            crate::page_stack::pop(guest, PageRef(context));
        }
        ControlsBackAction::ResumePrompt => {
            guest.call(0x2CBD20, &[]);
        }
    }
}

/// Options/settings back compares the six persisted fields touched by this page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsValues(pub [u32; 6]);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsBackAction {
    ConfirmChanges,
    Pop,
}
pub fn options_back_action(current: &OptionsValues, saved: &OptionsValues) -> OptionsBackAction {
    if current == saved {
        OptionsBackAction::Pop
    } else {
        OptionsBackAction::ConfirmChanges
    }
}
fn options_back(guest: &mut dyn GuestUi, page: PageRef) {
    let profile = guest.call(0x69640, &[]).unwrap_or(0);
    let saved = OptionsValues(
        [0x120, 0x118, 0x11C, 0x12C, 0x124, 0x130]
            .map(|offset| guest.read32(profile + offset).unwrap_or(0)),
    );
    let current = OptionsValues(
        [0x5236BC, 0x5236B4, 0x5236B8, 0x7613D8, 0x5236C0, 0x7613DC]
            .map(|address| guest.read32(address).unwrap_or(0)),
    );
    let action = options_back_action(&current, &saved);
    let frontend = guest.read32(0x79094C).unwrap_or(0) as i32 <= 0x64;
    if frontend && guest.read32(0x7BA16C).unwrap_or(0) != 0 {
        let table = guest.read32(0x7BA178).unwrap_or(0);
        let sound = guest.read32(table + 4).unwrap_or(0);
        guest.call(0x21780, &[sound]);
    }
    let context = if frontend {
        let viewport = guest.read32(page.0 + 0x38).unwrap_or(0);
        guest.call(0x2E46A0, &[viewport]).unwrap_or(0)
    } else {
        guest.read32(0x7613E4).unwrap_or(0)
    };
    match action {
        OptionsBackAction::ConfirmChanges => {
            guest.call(0x79B70, &[context, 0x52376C]);
        }
        OptionsBackAction::Pop => {
            crate::page_stack::pop(guest, PageRef(context));
            if frontend {
                guest.write32(0x7894FC, 0);
            }
        }
    }
}

/// Per-handler A/B boundary: off keeps the lifted oracle. No synthetic pad input.
/// T2319 Arcade level accept 0x2CA330 is admitted at Tier 0 (synthetic) for the no-prompt storage goal.
/// Tier 2 page-topology coverage: docs/evidence/t2331/*.json. Cold callbacks
/// (Cheats 0x2CBE30, neighbouring Map Maker return 0x2C8900) remain Tier 0 opt-ins.
/// T2431: Story difficulty and confirm handlers are default-on after a full-route
/// Tier 2 EQUAL and fault-free 180-second owner Story replay. Set
/// `TSFP_UI_FRONTEND_HANDLERS=0` to restore the lifted handlers for A/B checks.
pub const ADMITTED_HANDLERS: &[u32] = &[
    0x2D0A60, 0x2BEA80, 0x2CAFE0, 0x2CA1E0, 0x2CA210, 0x2C8B20, 0x2CBE10, 0x2CBE50, 0x2CBFA0,
    0x2C3D30, 0x2E9FC0, 0x2D0880, 0x2D08A0, 0x2CA330, 0x2CAD20, 0x2BEAA0,
];

pub fn enabled_for(handler: u32) -> bool {
    match std::env::var("TSFP_UI_FRONTEND_HANDLERS") {
        Ok(value) => selected_handler(&value, handler),
        Err(_) => ADMITTED_HANDLERS.contains(&handler),
    }
}
fn selected_handler(selection: &str, handler: u32) -> bool {
    selection == "1"
        || selection.split(',').any(|item| {
            let item = item.trim();
            u32::from_str_radix(item.strip_prefix("0x").unwrap_or(item), 16) == Ok(handler)
        })
}

pub fn main_accept(guest: &mut dyn GuestUi, page: PageRef) {
    log_handler(0x2D0A60);
    let row = guest.read32(page.0 + 0x18C).unwrap_or(0);
    let mut state = import_state(guest);
    let published = state.clone();
    main_select(&mut state, row, &mut GuestEffects { guest, published });
}

fn read_byte(guest: &dyn GuestUi, address: u32) -> u8 {
    (guest.read32(address & !3).unwrap_or(0) >> ((address & 3) * 8)) as u8
}
fn write_byte(guest: &mut dyn GuestUi, address: u32, value: u8) {
    let aligned = address & !3;
    let shift = (address & 3) * 8;
    let before = guest.read32(aligned).unwrap_or(0);
    guest.write32(
        aligned,
        (before & !(0xFF << shift)) | (u32::from(value) << shift),
    );
}
fn import_state(guest: &dyn GuestUi) -> FrontendState {
    FrontendState {
        selected: guest.read32(0x761400).unwrap_or(0),
        mode: guest.read32(0x78AD2C).unwrap_or(0),
        campaign: guest.read32(0x78AD48).unwrap_or(0),
        challenge_category: ArcadeCategoryId::new(guest.read32(0x78AD38).unwrap_or(0)),
        selected_challenge: None,
        challenge_setup: None,
        active_player: guest.read32(0x78AD34).unwrap_or(0),
        story_selected: guest.read32(0x78AE08).unwrap_or(0),
        arcade_custom: read_byte(guest, 0x78ADAF),
        story_selector: guest.read32(0x78AD54).unwrap_or(0),
        story_scroll_bits: guest.read32(0x78AE28).unwrap_or(0),
        story_flags: [read_byte(guest, 0x78AE1C), read_byte(guest, 0x78AE1D)],
    }
}

/// Consume a known callback. Unknown callbacks and the off lane retain the oracle.
/// This adapter never calls the callback it has replaced.
pub fn dispatch(guest: &mut dyn GuestUi, page: PageRef, handler: u32) -> bool {
    if guest.native_settings_handler(handler, page) {
        return true;
    }
    if !enabled_for(handler) {
        return false;
    }
    dispatch_selected(guest, page, handler)
}

/// Arcade custom-map level row of the level table 0x4D2390 (stride 0x3C).
pub const CUSTOM_MAP_LEVEL: u8 = 0x64;
/// Where the Arcade level accept 0x2CA330 goes next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcadeLevelNext {
    /// Modes other than 0xB..0xD open the setup page 0x523800.
    Setup,
    /// Modes 0xB..0xD with an ordinary level open 0x523890.
    ModeSetup,
    /// The custom-map row loads a saved map: storage opener 0x32AFE0 without Sample Maps,
    /// returning to 0x4E9C84 on success and 0x5238B0 otherwise.
    CustomMap,
}
pub fn arcade_level_next(mode: u32, level: u8) -> ArcadeLevelNext {
    match mode {
        0xB..=0xD if level == CUSTOM_MAP_LEVEL => ArcadeLevelNext::CustomMap,
        0xB..=0xD => ArcadeLevelNext::ModeSetup,
        _ => ArcadeLevelNext::Setup,
    }
}
/// Original 0x2CA330. T2319: the custom-map load reaches the single local storage without a
/// device chooser (`mapmaker_handlers::begin_frontend_storage`).
fn arcade_level_accept(guest: &mut dyn GuestUi, page: PageRef) {
    let row = guest.read32(page.0 + 0x18C).unwrap_or(0);
    guest.write32(0x78AD4C, row);
    let level = read_byte(guest, 0x4D2390u32.wrapping_add(row.wrapping_mul(0x3C)));
    write_byte(guest, 0x78ADAC, level);
    let mode = guest.read32(0x78AD2C).unwrap_or(0);
    let context = guest.read32(0x7613E4).unwrap_or(0);
    match arcade_level_next(mode, level) {
        ArcadeLevelNext::Setup => {
            guest.call(0x79B70, &[context, 0x523800]);
        }
        ArcadeLevelNext::ModeSetup => {
            guest.call(0x79B70, &[context, 0x523890]);
        }
        ArcadeLevelNext::CustomMap => {
            crate::mapmaker_handlers::begin_frontend_storage(guest, context, 0x4E9C84, 0x5238B0, 0)
        }
    }
}

fn log_handler(handler: u32) {
    if std::env::var("TSFP_UI_NATIVE_LOG").is_ok_and(|value| value == "1") {
        eprintln!("ui-frontier: native handler {handler:#x}");
    }
}
pub fn dispatch_selected(guest: &mut dyn GuestUi, page: PageRef, handler: u32) -> bool {
    if handler == 0x2C3D30 {
        log_handler(handler);
        options_back(guest, page);
        return true;
    }
    if handler == 0x2CA330 {
        log_handler(handler);
        arcade_level_accept(guest, page);
        return true;
    }
    if handler == 0x2E9FC0 {
        log_handler(handler);
        controls_back(guest);
        return true;
    }
    use FrontendAction::*;
    let action = match handler {
        0x2CAFE0 => StorySelect(guest.read32(page.0 + 0x18C).unwrap_or(0)),
        0x2CAD20 => StoryDifficulty(guest.read32(page.0 + 0x18C).unwrap_or(0)),
        0x2BEAA0 => StoryConfirm,
        0x2CA1E0 => ArcadeCustom,
        0x2CA210 => ArcadeLeague,
        0x2C8B20 => ChallengeSelect(ArcadeCategoryId::new(
            guest.read32(page.0 + 0x18C).unwrap_or(0),
        )),
        0x2CBE10 => SettingsOptions,
        0x2CBE30 => SettingsCheats,
        0x2CBE50 => SettingsControls,
        0x2CBFA0 => SettingsBack,
        0x2D0880 => ExtrasCredits,
        0x2D08A0 => ExtrasGallery,
        0x2BEA80 => MainBack,
        0x2C8900 => ReturnMain,
        _ => return false,
    };
    log_handler(handler);
    let mut state = import_state(guest);
    let published = state.clone();
    transition(&mut state, action, &mut GuestEffects { guest, published });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Effects {
        calls: Vec<String>,
        available: bool,
        resume: bool,
    }
    impl FrontendEffects for Effects {
        fn sound(&mut self, id: u32) {
            self.calls.push(format!("sound {id}"));
        }
        fn challenge_available(&mut self) -> bool {
            self.calls.push("challenge".into());
            self.available
        }
        fn prepare_network(&mut self) {
            self.calls.push("prepare".into());
        }
        fn network_can_resume(&mut self) -> bool {
            self.calls.push("resume".into());
            self.resume
        }
        fn publish_selection(&mut self, state: &FrontendState) {
            self.calls
                .push(format!("state {} {}", state.selected, state.mode));
        }
        fn open(&mut self, destination: Destination) {
            self.calls.push(format!("open {destination:?}"));
        }
        fn reset_options(&mut self) {
            self.calls.push("reset".into());
        }
        fn snapshot_controls(&mut self, player: u32) {
            self.calls.push(format!("snapshot {player}"));
        }
        fn profiles_back(&mut self, destination: Destination) {
            self.calls.push(format!("profiles {destination:?}"));
        }
    }
    #[test]
    fn all_main_destinations_and_unknown_rows() {
        for (row, target) in [
            (0, "Story"),
            (1, "Arcade"),
            (4, "MapMaker"),
            (5, "PlayerProgress"),
            (6, "Settings"),
            (7, "Extras"),
        ] {
            let mut state = FrontendState {
                mode: 42,
                ..Default::default()
            };
            let mut effects = Effects::default();
            main_select(&mut state, row, &mut effects);
            assert_eq!(
                effects.calls,
                [
                    "sound 3358".to_owned(),
                    format!("state {row} 42"),
                    format!("open {target}")
                ]
            );
            assert_eq!(
                state,
                FrontendState {
                    selected: row,
                    mode: 42,
                    ..Default::default()
                }
            );
        }
        let mut state = FrontendState::default();
        let mut effects = Effects::default();
        main_select(&mut state, u32::MAX, &mut effects);
        assert_eq!(effects.calls.len(), 2);
        assert_eq!(state.selected, u32::MAX);
    }
    #[test]
    fn challenge_gate_and_network_preparation_preserve_order() {
        for available in [false, true] {
            let mut state = FrontendState::default();
            let mut effects = Effects {
                available,
                ..Default::default()
            };
            main_select(&mut state, 2, &mut effects);
            assert_eq!(effects.calls[1], "state 2 0");
            assert_eq!(effects.calls[2], "challenge");
            assert_eq!(state.mode, if available { 0 } else { 7 });
            assert_eq!(
                effects.calls.last().unwrap(),
                if available {
                    "open Challenge"
                } else {
                    "open ChallengeLocked"
                }
            );
        }
        for resume in [false, true] {
            let mut state = FrontendState::default();
            let mut effects = Effects {
                resume,
                ..Default::default()
            };
            main_select(&mut state, 3, &mut effects);
            assert_eq!(
                &effects.calls[1..5],
                ["state 3 0", "state 3 13", "prepare", "resume"]
            );
            assert_eq!(
                effects.calls.last().unwrap(),
                if resume {
                    "open NetworkResume"
                } else {
                    "open NetworkSetup"
                }
            );
        }
    }
    #[test]
    fn page_transitions_update_owned_state_and_service_order() {
        let mut state = FrontendState {
            active_player: 3,
            ..Default::default()
        };
        let mut effects = Effects::default();
        transition(&mut state, FrontendAction::StorySelect(12), &mut effects);
        assert_eq!(state.mode, 4);
        assert_eq!(state.campaign, 12);
        assert_eq!(state.story_selected, 1);
        assert_eq!(
            effects.calls,
            ["state 0 4", "sound 3358", "state 0 4", "open StoryMissions"]
        );
        for (action, target, mode) in [
            (FrontendAction::ArcadeCustom, "ArcadeCustom", 5),
            (FrontendAction::ArcadeLeague, "ArcadeLeague", 6),
            (
                FrontendAction::ChallengeSelect(ArcadeCategoryId::new(7)),
                "ChallengeMissions",
                6,
            ),
            (FrontendAction::SettingsCheats, "Cheats", 6),
            (FrontendAction::ExtrasCredits, "Credits", 10),
            (FrontendAction::ExtrasGallery, "Gallery", 10),
            (FrontendAction::MainBack, "Players", 10),
        ] {
            effects.calls.clear();
            transition(&mut state, action, &mut effects);
            assert_eq!(state.mode, mode);
            assert_eq!(effects.calls.last().unwrap(), &format!("open {target}"));
        }
        assert_eq!(state.arcade_custom, 1);
        assert_eq!(state.challenge_category, ArcadeCategoryId::new(7));
        effects.calls.clear();
        transition(&mut state, FrontendAction::SettingsControls, &mut effects);
        assert_eq!(
            effects.calls,
            ["snapshot 3", "sound 3358", "state 0 10", "open Controls"]
        );
        effects.calls.clear();
        transition(&mut state, FrontendAction::SettingsOptions, &mut effects);
        assert_eq!(effects.calls[0], "reset");
        effects.calls.clear();
        transition(&mut state, FrontendAction::SettingsBack, &mut effects);
        assert_eq!(state.active_player, 0);
        assert_eq!(effects.calls, ["state 0 10", "profiles Main"]);
    }

    #[test]
    fn story_difficulty_and_confirm_pages_keep_the_original_order() {
        // 0x2CAD20: scroll reset and selector, sound, flags cleared, then the confirm page.
        let mut state = FrontendState {
            story_scroll_bits: 0x3f80_0000,
            story_flags: [1, 1],
            ..Default::default()
        };
        let mut effects = Effects::default();
        transition(&mut state, FrontendAction::StoryDifficulty(2), &mut effects);
        assert_eq!(state.story_selector, 2);
        assert_eq!(state.story_scroll_bits, 0);
        assert_eq!(state.story_flags, [0, 0]);
        assert_eq!(
            effects.calls,
            ["state 0 0", "sound 3358", "state 0 0", "open StoryConfirm"]
        );
        // 0x2BEAA0 only opens the page 0x5236DC.
        effects.calls.clear();
        transition(&mut state, FrontendAction::StoryConfirm, &mut effects);
        assert_eq!(effects.calls, ["state 0 0", "open StoryStart"]);
        assert_eq!(Destination::StoryConfirm.descriptor(), 0x5238C0);
        assert_eq!(Destination::StoryStart.descriptor(), 0x5236DC);
    }

    #[test]
    fn controls_back_policy_covers_frontend_gameplay_and_nan() {
        for gameplay in [false, true] {
            for prompt_active in [false, true] {
                let state = ControlsBackState {
                    gameplay,
                    changed: true,
                    prompt_active,
                };
                assert_eq!(
                    controls_back_action(state),
                    ControlsBackAction::ConfirmChanges
                );
                let state = ControlsBackState {
                    changed: false,
                    ..state
                };
                assert_eq!(
                    controls_back_action(state),
                    if !gameplay {
                        ControlsBackAction::RestoreFrontendParent
                    } else if prompt_active {
                        ControlsBackAction::ResumePrompt
                    } else {
                        ControlsBackAction::Pop
                    }
                );
            }
        }
        let saved = ControlsSnapshot([0; 31]);
        assert!(!saved.changed(&saved, false));
        for word in 0..31 {
            let mut current = saved.clone();
            current.0[word] = 1;
            assert!(current.changed(&saved, false), "word {word}");
        }
        let mut nan = saved;
        nan.0[30] = f32::NAN.to_bits();
        assert!(!nan.changed(&nan, false));
        assert!(nan.changed(&nan, true));
    }

    #[test]
    fn handler_selector_can_bisect_candidates_individually() {
        assert!(selected_handler("1", 0x2D0A60));
        assert!(selected_handler("0x2cafe0, 2d0a60", 0x2D0A60));
        assert!(selected_handler("0x2cafe0, 2d0a60", 0x2CAFE0));
        for selection in ["0", "off", "", "0x2cafe0", "garbage"] {
            assert!(!selected_handler(selection, 0x2D0A60));
        }
    }
    #[derive(Default)]
    struct Guest {
        memory: std::collections::HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
    }
    impl GuestUi for Guest {
        fn read32(&self, address: u32) -> Option<u32> {
            Some(*self.memory.get(&address).unwrap_or(&0))
        }
        fn write32(&mut self, address: u32, value: u32) -> bool {
            self.memory.insert(address, value);
            true
        }
        fn call(&mut self, address: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((address, args.to_vec()));
            Some(if address == 0x69620 { 0x10000 } else { 0 })
        }
    }
    #[test]
    fn guest_adapter_never_calls_replaced_row_handler_and_preserves_packed_siblings() {
        let mut guest = Guest::default();
        guest.memory.insert(0x7613E4, 0x1000);
        guest.memory.insert(0x78ADAC, 0xAB123456);
        assert!(dispatch_selected(&mut guest, PageRef(0x2000), 0x2CA1E0));
        assert_eq!(guest.memory[&0x78ADAC], 0x01123456);
        assert_eq!(guest.memory[&0x78AD2C], 5);
        assert_eq!(guest.calls, [(0x79B70, vec![0x1000, 0x523890])]);
        assert!(!dispatch_selected(&mut guest, PageRef(0x2000), 0xDEAD));
        assert_eq!(guest.calls.len(), 1);
    }
    #[test]
    fn arcade_custom_map_skips_the_single_device_chooser() {
        assert_eq!(
            arcade_level_next(5, CUSTOM_MAP_LEVEL),
            ArcadeLevelNext::Setup
        );
        assert_eq!(arcade_level_next(0xC, 3), ArcadeLevelNext::ModeSetup);
        assert_eq!(
            arcade_level_next(0xD, CUSTOM_MAP_LEVEL),
            ArcadeLevelNext::CustomMap
        );
        #[derive(Default)]
        struct Storage {
            memory: std::collections::HashMap<u32, u32>,
            calls: Vec<(u32, Vec<u32>)>,
            devices: Vec<u32>,
        }
        impl GuestUi for Storage {
            fn read32(&self, address: u32) -> Option<u32> {
                Some(*self.memory.get(&address).unwrap_or(&0))
            }
            fn write32(&mut self, address: u32, value: u32) -> bool {
                self.memory.insert(address, value);
                true
            }
            fn call(&mut self, address: u32, args: &[u32]) -> Option<u32> {
                self.calls.push((address, args.to_vec()));
                let device = args
                    .first()
                    .is_some_and(|index| self.devices.contains(index));
                Some(u32::from(matches!(address, 0x190AC0 | 0x24E60) && device))
            }
        }
        let run = |devices: Vec<u32>| {
            let mut guest = Storage {
                devices,
                ..Storage::default()
            };
            guest.memory.insert(0x7613E4, 0x1000);
            guest.memory.insert(0x78AD2C, 0xB);
            guest.memory.insert(0x2000 + 0x18C, 2);
            guest
                .memory
                .insert(0x4D2390 + 2 * 0x3C, u32::from(CUSTOM_MAP_LEVEL));
            assert!(dispatch_selected(&mut guest, PageRef(0x2000), 0x2CA330));
            assert_eq!(guest.memory[&0x7630CC], 0x4E9C84);
            assert_eq!(guest.memory[&0x765AFC], 0x5238B0);
            assert_eq!(guest.memory[&0x7657F8], 0);
            guest
        };
        let single = run(vec![9]);
        let pushes: Vec<_> = single
            .calls
            .iter()
            .filter(|call| call.0 == 0x79B70)
            .collect();
        assert_eq!(pushes, [&(0x79B70, vec![0x1000, 0x52EA98])]);
        assert!(single.calls.contains(&(0x827D0, vec![9])));
        assert_eq!(single.memory[&0x765B04], 4);
        let two = run(vec![0, 9]);
        let pushes: Vec<_> = two.calls.iter().filter(|call| call.0 == 0x79B70).collect();
        assert_eq!(pushes, [&(0x79B70, vec![0x1000, 0x52EA68])]);
    }
    #[test]
    fn controls_open_copies_31_words_only_and_calls_neighbour_services() {
        let mut guest = Guest::default();
        guest.memory.insert(0x78AD34, 2);
        guest.memory.insert(0x7BA9E0, 1);
        guest.memory.insert(0x7613E4, 0x1000);
        let target = 0x7BA364 + 0x1E0;
        guest.memory.insert(target + 31 * 4, 0x12345678);
        for word in 0..31 {
            guest.memory.insert(0x10000 + word * 4, 100 + word);
        }
        assert!(dispatch_selected(&mut guest, PageRef(0x2000), 0x2CBE50));
        for word in 0..31 {
            assert_eq!(guest.memory[&(target + word * 4)], 100 + word);
        }
        assert_eq!(guest.memory[&(target + 31 * 4)], 0x12345678);
        assert_eq!(
            guest.calls,
            [
                (0x69620, vec![2]),
                (0x21780, vec![0xD1E]),
                (0x79B70, vec![0x1000, 0x524A40])
            ]
        );
    }
    #[test]
    fn options_back_checks_each_of_the_six_persisted_fields() {
        let saved = OptionsValues([1, 2, 3, 4, 5, 6]);
        assert_eq!(options_back_action(&saved, &saved), OptionsBackAction::Pop);
        for index in 0..6 {
            let mut current = saved.clone();
            current.0[index] += 1;
            assert_eq!(
                options_back_action(&current, &saved),
                OptionsBackAction::ConfirmChanges
            );
        }
    }
    #[test]
    fn admission_includes_traced_callbacks_and_keeps_cold_ones_opt_in() {
        for handler in [
            0x2D0A60, 0x2BEA80, 0x2CAFE0, 0x2CA1E0, 0x2CA210, 0x2C8B20, 0x2CBE10, 0x2CBE50,
            0x2CBFA0, 0x2C3D30, 0x2E9FC0, 0x2D0880, 0x2D08A0, 0x2CA330,
        ] {
            assert!(ADMITTED_HANDLERS.contains(&handler));
            assert!(!selected_handler("0", handler));
        }
        assert_eq!(ADMITTED_HANDLERS.len(), 16);
        for handler in [0x2CBE30, 0x2C8900, 0xDEAD] {
            assert!(!ADMITTED_HANDLERS.contains(&handler));
        }
    }
}
