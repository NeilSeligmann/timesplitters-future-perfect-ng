// SPDX-License-Identifier: GPL-3.0-or-later
//! T1697 native attachment for the bounded settings-page contracts.
//!
//! Page identities, widget identities, row geometry and text ids come from the runtime UI
//! recorder/table loader. This module only attaches native action ids and owns an edit until the
//! matching Accept path. It does not manufacture original strings, layout, profile slots or
//! renderer commands.

use crate::{Binding, Extension, Target};
use std::collections::BTreeMap;
use tsfp_format_profile::{
    Profile,
    settings::{AudioVideo, Crosshair, SettingsEdit, Vibration, WeaponChange},
};
use tsfp_ui_input::{ActionId, BindResult, UiBindings, UiEvent, UiValue, ValueId};
use tsfp_ui_model::{PageId, UiModel, WidgetId};

/// Runtime page descriptors and the builder-created rows that open their children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsPageWidgets {
    pub root: PageId,
    pub controls: PageId,
    pub audio_video: PageId,
    pub controls_row: WidgetId,
    pub audio_video_row: WidgetId,
    /// Present only when the runtime root builder created the profile selector.
    pub profile_selector: Option<(WidgetId, ValueId)>,
    /// Optional unlock/cheats row; the original only creates it when the runtime gate is true.
    pub unlocks_row: Option<(WidgetId, ActionId)>,
    pub open_controls: ActionId,
    pub open_audio_video: ActionId,
}

/// Wire the existing recorded rows to native `UiBindings` actions.
///
/// The caller supplies the already-resolved runtime table and page model. No extension is
/// installed by the `original` preset; callers opt in by applying this returned layer.
pub fn settings_page_actions(
    model: &UiModel,
    pages: SettingsPageWidgets,
) -> Result<Extension, &'static str> {
    for page in [pages.root, pages.controls, pages.audio_video] {
        if model.page(page).is_none() {
            return Err("settings page descriptor");
        }
    }
    let mut bindings = BTreeMap::new();
    bind_widget(
        model,
        &mut bindings,
        pages.root,
        pages.controls_row,
        pages.open_controls,
    )?;
    bind_widget(
        model,
        &mut bindings,
        pages.root,
        pages.audio_video_row,
        pages.open_audio_video,
    )?;
    if let Some((widget, value)) = pages.profile_selector {
        if model
            .page(pages.root)
            .and_then(|p| p.widget(widget))
            .is_none()
        {
            return Err("settings page widget");
        }
        if bindings
            .insert((pages.root, widget), Binding::Value(value))
            .is_some()
        {
            return Err("duplicate settings page widget");
        }
    }
    if let Some((widget, action)) = pages.unlocks_row {
        bind_widget(model, &mut bindings, pages.root, widget, action)?;
    }
    let mut extension = Extension::new("native-settings-pages");
    extension.bindings = bindings;
    Ok(extension)
}

fn bind_widget(
    model: &UiModel,
    bindings: &mut BTreeMap<Target, Binding>,
    page: PageId,
    widget: WidgetId,
    action: ActionId,
) -> Result<(), &'static str> {
    if model.page(page).and_then(|p| p.widget(widget)).is_none() {
        return Err("settings page widget");
    }
    if bindings
        .insert((page, widget), Binding::Action(action))
        .is_some()
    {
        return Err("duplicate settings page widget");
    }
    Ok(())
}

/// Mode, viewport and profile slot are separate selectors in the original page family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsPageContext {
    pub mode: u32,
    pub viewport: u8,
    /// Explicit owner context; mode values alone do not identify gameplay.
    pub gameplay: bool,
    pub profile_slot: usize,
    pub configured_slot_count: usize,
    /// Runtime selector results in UI order; values need not be a local-player index.
    pub profile_slots: Vec<usize>,
}

impl SettingsPageContext {
    pub fn validate(self, profile_count: usize) -> Result<Self, &'static str> {
        if self.configured_slot_count == 0
            || self.profile_slots.len() != self.configured_slot_count
            || self.profile_slot >= profile_count
            || !self.profile_slots.contains(&self.profile_slot)
            || self.profile_slots.iter().any(|slot| *slot >= profile_count)
        {
            return Err("settings profile selection");
        }
        let mut unique = self.profile_slots.clone();
        unique.sort_unstable();
        unique.dedup();
        if unique.len() != self.profile_slots.len() {
            return Err("duplicate settings profile slot");
        }
        Ok(self)
    }

    /// The original selector is built only when the configured slot count exceeds one.
    pub fn has_profile_selector(&self) -> bool {
        self.configured_slot_count > 1
    }

    /// Original page callbacks use the front-end unwind when mode is at most 100.
    pub fn is_frontend(&self) -> bool {
        self.mode <= 100
    }
}

/// Typed effects requested from the neighbouring profile, input and audio systems.
/// They are requests, not proof that a save or device operation succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsEffect {
    Open(PageId),
    PopPages(u8),
    PopCurrentContext,
    SaveProfile { slot: usize },
    CopyControlsToPlayer { slot: usize, viewport: u8 },
    ReloadMapping { slot: usize, viewport: u8 },
    MarkProfileDirty { slot: usize },
    ApplyAudioSettings,
    ClearGameplaySettingsContext { viewport: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsRoutes {
    pub root: PageId,
    pub controls: PageId,
    pub audio_video: PageId,
    pub confirmation: PageId,
}

/// One native settings edit for the explicitly selected profile and viewport.
///
/// Committed profiles stay untouched until an Accept action. A session is cheap to rebuild after
/// a profile-selection change, which refreshes profile-dependent rows rather than freezing the
/// first row tree.
#[derive(Debug, Clone)]
pub struct SettingsPageSession {
    pub context: SettingsPageContext,
    pub routes: SettingsRoutes,
    profiles: Vec<Profile>,
    current_page: PageId,
    stack: Vec<PageId>,
    edit: SettingsEdit,
}

impl SettingsPageSession {
    pub fn open(
        profiles: Vec<Profile>,
        context: SettingsPageContext,
        routes: SettingsRoutes,
    ) -> Result<Self, &'static str> {
        let context = context.validate(profiles.len())?;
        let edit =
            SettingsEdit::open(&profiles[context.profile_slot]).map_err(|_| "profile settings")?;
        Ok(Self {
            context,
            routes,
            profiles,
            current_page: routes.root,
            stack: Vec::new(),
            edit,
        })
    }

    pub fn current_page(&self) -> PageId {
        self.current_page
    }

    pub fn edit(&self) -> &SettingsEdit {
        &self.edit
    }

    pub fn edit_mut(&mut self) -> &mut SettingsEdit {
        &mut self.edit
    }

    /// Read one typed row through the same value contract used by the live bindings adapter.
    pub fn setting_value(&self, setting: SettingsValue) -> Option<UiValue> {
        read_setting_value(self, setting)
    }

    /// Update a Controls or Audio/Video row in the owned working edit.
    pub fn set_setting_value(
        &mut self,
        setting: SettingsValue,
        value: UiValue,
    ) -> Result<(), &'static str> {
        write_setting_value(self, setting, value).map_err(|_| "settings value")
    }

    /// Apply the original finite Turn Speed step to the working Controls head.
    pub fn adjust_turn_speed(&mut self, increase: bool) -> Result<bool, &'static str> {
        self.edit
            .controls
            .adjust_turn_speed(increase)
            .map_err(|_| "turn speed")
    }

    pub fn selected_profile(&self) -> &Profile {
        &self.profiles[self.context.profile_slot]
    }

    pub fn profile(&self, slot: usize) -> Option<&Profile> {
        self.profiles.get(slot)
    }

    /// Change the profile selector without conflating it with viewport/player routing.
    pub fn select_profile(&mut self, slot: usize) -> Result<(), &'static str> {
        if !self.context.has_profile_selector() || !self.context.profile_slots.contains(&slot) {
            return Err("profile selector unavailable");
        }
        let profile = self.profiles.get(slot).ok_or("profile slot missing")?;
        let edit = SettingsEdit::open(profile).map_err(|_| "profile settings")?;
        self.context.profile_slot = slot;
        self.edit = edit;
        Ok(())
    }

    pub fn open_controls(&mut self) -> SettingsEffect {
        self.push(self.routes.controls)
    }

    pub fn open_audio_video(&mut self) -> SettingsEffect {
        self.push(self.routes.audio_video)
    }

    /// B from Controls: changed front-end edits open the confirmation; unchanged edits pop once.
    /// The mode>100 branch has additional profile/context predicates and is supplied explicitly
    /// through `confirm_controls_in_game` rather than guessed here.
    pub fn controls_back(&mut self) -> Result<SettingsEffect, &'static str> {
        if !self.context.is_frontend() {
            return Err("game-mode controls back requires original context predicates");
        }
        let changed = self
            .edit
            .controls_changed(self.selected_profile())
            .map_err(|_| "profile settings")?;
        if changed {
            Ok(self.push(self.routes.confirmation))
        } else {
            Ok(self.pop(1))
        }
    }

    /// Direct Accept (`0x2E9E50`): publish the owned head, reload mapping and pop one page.
    pub fn accept_controls_direct(&mut self) -> Result<[SettingsEffect; 3], &'static str> {
        if self.current_page != self.routes.controls {
            return Err("controls page state");
        }
        let slot = self.context.profile_slot;
        self.edit
            .clone()
            .accept_controls(&mut self.profiles[slot])
            .map_err(|_| "profile settings")?;
        let audio_video = self.edit.audio_video;
        self.edit = SettingsEdit::open(&self.profiles[slot]).map_err(|_| "profile settings")?;
        self.edit.audio_video = audio_video;
        self.current_page = self.routes.root;
        self.stack.clear();
        Ok([
            SettingsEffect::CopyControlsToPlayer {
                slot,
                viewport: self.context.viewport,
            },
            SettingsEffect::ReloadMapping {
                slot,
                viewport: self.context.viewport,
            },
            SettingsEffect::PopPages(1),
        ])
    }

    /// A/V Accept publishes only the six established profile words. The host owns durable save
    /// acceptance; this method emits dirty/context effects and keeps the committed profile typed.
    pub fn accept_audio_video(&mut self) -> Result<Vec<SettingsEffect>, &'static str> {
        if self.current_page != self.routes.audio_video {
            return Err("audio/video page state");
        }
        let slot = self.context.profile_slot;
        self.edit
            .clone()
            .accept_audio_video(&mut self.profiles[slot])
            .map_err(|_| "audio/video settings")?;
        let controls = self.edit.controls.clone();
        self.edit = SettingsEdit::open(&self.profiles[slot]).map_err(|_| "profile settings")?;
        self.edit.controls = controls;
        self.current_page = self.routes.root;
        self.stack.clear();
        let mut effects = vec![SettingsEffect::MarkProfileDirty { slot }];
        if self.context.gameplay {
            effects.push(SettingsEffect::ClearGameplaySettingsContext {
                viewport: self.context.viewport,
            });
        }
        effects.push(if self.context.is_frontend() {
            SettingsEffect::PopPages(1)
        } else {
            SettingsEffect::PopCurrentContext
        });
        Ok(effects)
    }

    /// Confirmation Yes in the front-end writes the working head and unwinds two pages.
    pub fn confirm_controls_yes_frontend(&mut self) -> Result<[SettingsEffect; 3], &'static str> {
        if !self.context.is_frontend() || self.current_page != self.routes.confirmation {
            return Err("front-end controls confirmation state");
        }
        let slot = self.context.profile_slot;
        self.edit
            .clone()
            .accept_controls(&mut self.profiles[slot])
            .map_err(|_| "profile settings")?;
        let audio_video = self.edit.audio_video;
        self.edit = SettingsEdit::open(&self.profiles[slot]).map_err(|_| "profile settings")?;
        self.edit.audio_video = audio_video;
        self.current_page = self.routes.root;
        self.stack.clear();
        Ok([
            SettingsEffect::CopyControlsToPlayer {
                slot,
                viewport: self.context.viewport,
            },
            SettingsEffect::ReloadMapping {
                slot,
                viewport: self.context.viewport,
            },
            SettingsEffect::PopPages(2),
        ])
    }

    /// Confirmation No returns from the confirmation only, leaving Controls and its edit alive.
    pub fn confirm_controls_no_frontend(&mut self) -> Result<SettingsEffect, &'static str> {
        if !self.context.is_frontend() || self.current_page != self.routes.confirmation {
            return Err("front-end controls confirmation state");
        }
        Ok(self.pop(1))
    }

    /// The original game-mode confirmation is entered only after its profile/context predicates
    /// request it; those predicates are owned by the caller that has the live game context.
    pub fn open_game_controls_confirmation(
        &mut self,
        original_predicate_requests_confirmation: bool,
    ) -> Result<SettingsEffect, &'static str> {
        if self.context.is_frontend() || self.current_page != self.routes.controls {
            return Err("game-mode controls page state");
        }
        if !original_predicate_requests_confirmation {
            return Err("game-mode controls confirmation not requested");
        }
        Ok(self.push(self.routes.confirmation))
    }

    /// Confirmation No in game mode uses the global context pop callback.
    pub fn confirm_controls_no_in_game(&mut self) -> Result<SettingsEffect, &'static str> {
        if self.context.is_frontend() || self.current_page != self.routes.confirmation {
            return Err("game-mode controls confirmation state");
        }
        self.edit = SettingsEdit::open(self.selected_profile()).map_err(|_| "profile settings")?;
        self.current_page = self.routes.root;
        self.stack.clear();
        Ok(SettingsEffect::PopCurrentContext)
    }

    /// The game-mode Yes callback tails through the profile-copy/dirty path and context pop.
    pub fn confirm_controls_yes_in_game(&mut self) -> Result<[SettingsEffect; 2], &'static str> {
        if self.context.is_frontend() || self.current_page != self.routes.confirmation {
            return Err("game-mode controls confirmation state");
        }
        let slot = self.context.profile_slot;
        self.edit
            .clone()
            .accept_controls(&mut self.profiles[slot])
            .map_err(|_| "profile settings")?;
        let audio_video = self.edit.audio_video;
        self.edit = SettingsEdit::open(&self.profiles[slot]).map_err(|_| "profile settings")?;
        self.edit.audio_video = audio_video;
        self.current_page = self.routes.root;
        self.stack.clear();
        Ok([
            SettingsEffect::MarkProfileDirty { slot },
            SettingsEffect::PopCurrentContext,
        ])
    }

    /// Cancel A/V restores the committed values and preserves the original two ordered audio
    /// applications: the loader applies once, then the callback applies again before popping.
    pub fn cancel_audio_video(&mut self) -> Result<[SettingsEffect; 3], &'static str> {
        if self.current_page != self.routes.audio_video {
            return Err("audio/video page state");
        }
        self.edit.audio_video = self
            .selected_profile()
            .audio_video()
            .map_err(|_| "profile settings")?;
        let pop = self.pop(1);
        Ok([
            SettingsEffect::ApplyAudioSettings,
            SettingsEffect::ApplyAudioSettings,
            pop,
        ])
    }

    /// Root Back resets the active selector to slot zero before asking the save owner to persist.
    pub fn root_back(&mut self) -> Result<[SettingsEffect; 2], &'static str> {
        if self.current_page != self.routes.root || self.profiles.is_empty() {
            return Err("settings root state");
        }
        self.context.profile_slot = 0;
        self.edit = SettingsEdit::open(&self.profiles[0]).map_err(|_| "profile settings")?;
        Ok([
            SettingsEffect::SaveProfile { slot: 0 },
            SettingsEffect::PopCurrentContext,
        ])
    }

    fn push(&mut self, page: PageId) -> SettingsEffect {
        self.stack.push(self.current_page);
        self.current_page = page;
        SettingsEffect::Open(page)
    }

    fn pop(&mut self, count: u8) -> SettingsEffect {
        for _ in 0..count {
            if let Some(page) = self.stack.pop() {
                self.current_page = page;
            }
        }
        SettingsEffect::PopPages(count)
    }
}

/// Binds semantic settings actions to the native session; controller, keyboard and mouse all
/// arrive through the same `UiBindings` action route.
pub struct SettingsSessionBindings<'a> {
    pub session: &'a mut SettingsPageSession,
    pub inner: &'a mut dyn UiBindings,
    pub actions: SettingsSessionActions,
    /// Runtime row value ids; no row id or original choice catalogue is guessed here.
    pub values: BTreeMap<ValueId, SettingsValue>,
    pub profile_slot_value: Option<ValueId>,
    pub effects: Vec<SettingsEffect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsValue {
    Crosshair,
    WeaponChange,
    InverseLook,
    AutoAim,
    Lookahead,
    AimToggle,
    CrouchToggle,
    Vibration,
    TurnSpeed,
    AudioRaw(AudioWord),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioWord {
    MusicEnabled,
    SoundEnabled,
    MusicVolume,
    SoundVolume,
    Slider4,
    Option5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsSessionActions {
    pub open_controls: ActionId,
    pub open_audio_video: ActionId,
    pub controls_back: ActionId,
    pub controls_accept: ActionId,
    pub controls_yes: ActionId,
    pub controls_no: ActionId,
    pub audio_cancel: ActionId,
    pub audio_accept: ActionId,
    pub root_back: ActionId,
}

impl SettingsSessionBindings<'_> {
    fn record<const N: usize>(
        out: &mut Vec<SettingsEffect>,
        result: Result<[SettingsEffect; N], &'static str>,
    ) -> BindResult {
        match result {
            Ok(recorded) => {
                out.extend(recorded);
                BindResult::Handled
            }
            Err(_) => BindResult::Denied,
        }
    }
}

impl UiBindings for SettingsSessionBindings<'_> {
    fn action(&mut self, id: ActionId, arg: tsfp_ui_input::ActionArg) -> BindResult {
        if id == self.actions.open_controls {
            self.effects.push(self.session.open_controls());
            return BindResult::Handled;
        }
        if id == self.actions.open_audio_video {
            self.effects.push(self.session.open_audio_video());
            return BindResult::Handled;
        }
        if id == self.actions.controls_back {
            return match self.session.controls_back() {
                Ok(effect) => {
                    self.effects.push(effect);
                    BindResult::Handled
                }
                Err(_) => BindResult::Denied,
            };
        }
        if id == self.actions.controls_accept {
            let result = self.session.accept_controls_direct();
            return Self::record(&mut self.effects, result);
        }
        if id == self.actions.audio_accept {
            let result = self.session.accept_audio_video();
            return match result {
                Ok(effects) => {
                    self.effects.extend(effects);
                    BindResult::Handled
                }
                Err(_) => BindResult::Denied,
            };
        }
        if id == self.actions.controls_yes {
            if self.session.context.is_frontend() {
                let result = self.session.confirm_controls_yes_frontend();
                return Self::record(&mut self.effects, result);
            }
            let result = self.session.confirm_controls_yes_in_game();
            return Self::record(&mut self.effects, result);
        }
        if id == self.actions.controls_no {
            let result = if self.session.context.is_frontend() {
                self.session.confirm_controls_no_frontend()
            } else {
                self.session.confirm_controls_no_in_game()
            };
            return match result {
                Ok(effect) => {
                    self.effects.push(effect);
                    BindResult::Handled
                }
                Err(_) => BindResult::Denied,
            };
        }
        if id == self.actions.audio_cancel {
            let result = self.session.cancel_audio_video();
            return Self::record(&mut self.effects, result);
        }
        if id == self.actions.root_back {
            let result = self.session.root_back();
            return Self::record(&mut self.effects, result);
        }
        self.inner.action(id, arg)
    }

    fn value(&self, id: tsfp_ui_input::ValueId) -> tsfp_ui_input::UiValue {
        if self.profile_slot_value == Some(id) {
            return self
                .session
                .context
                .profile_slots
                .iter()
                .position(|slot| *slot == self.session.context.profile_slot)
                .and_then(|index| u32::try_from(index).ok())
                .map_or(UiValue::None, UiValue::Index);
        }
        if let Some(value) = self.values.get(&id) {
            return read_setting_value(self.session, *value).unwrap_or(UiValue::None);
        }
        self.inner.value(id)
    }

    fn set_value(
        &mut self,
        id: tsfp_ui_input::ValueId,
        value: tsfp_ui_input::UiValue,
    ) -> BindResult {
        if self.profile_slot_value == Some(id) {
            let UiValue::Index(choice) = value else {
                return BindResult::Denied;
            };
            let Some(slot) = self
                .session
                .context
                .profile_slots
                .get(choice as usize)
                .copied()
            else {
                return BindResult::Denied;
            };
            return match self.session.select_profile(slot) {
                Ok(()) => BindResult::Handled,
                Err(_) => BindResult::Denied,
            };
        }
        if let Some(setting) = self.values.get(&id).copied() {
            return match write_setting_value(self.session, setting, value) {
                Ok(()) => BindResult::Handled,
                Err(()) => BindResult::Denied,
            };
        }
        self.inner.set_value(id, value)
    }

    fn list(&self, id: tsfp_ui_input::ListId) -> tsfp_ui_input::ListView {
        self.inner.list(id)
    }

    fn text(&self, id: tsfp_ui_input::TextId) -> &str {
        self.inner.text(id)
    }
}

/// A helper to make the common caller flow explicit: event -> existing model binding -> UiBindings.
pub fn route_settings_event(
    extension: &Extension,
    page: PageId,
    event: &UiEvent,
    bindings: &mut dyn UiBindings,
) -> BindResult {
    match event {
        UiEvent::Activate(widget) => match extension.bindings.get(&(page, WidgetId(widget.0))) {
            Some(Binding::Action(action)) => {
                bindings.action(*action, tsfp_ui_input::ActionArg::None)
            }
            _ => BindResult::Ignored,
        },
        _ => BindResult::Ignored,
    }
}

/// Read the session's current A/V values without exposing a mutable committed profile.
pub fn audio_video_edit(session: &SettingsPageSession) -> AudioVideo {
    session.edit.audio_video
}

fn read_setting_value(session: &SettingsPageSession, setting: SettingsValue) -> Option<UiValue> {
    use tsfp_format_profile::settings::Controls;
    let controls: Controls = session.edit.controls.controls().ok()?;
    Some(match setting {
        SettingsValue::Crosshair => UiValue::Index(match controls.crosshair {
            Crosshair::Off => 0,
            Crosshair::Moving => 1,
            Crosshair::Fixed => 2,
        }),
        SettingsValue::WeaponChange => UiValue::Index(match controls.weapon_change {
            WeaponChange::Always => 0,
            WeaponChange::Never => 1,
            WeaponChange::Best => 2,
            WeaponChange::IfNew => 3,
            WeaponChange::IfNewAndBest => 4,
        }),
        SettingsValue::InverseLook => UiValue::Bool(controls.inverse_look),
        SettingsValue::AutoAim => UiValue::Bool(controls.auto_aim),
        SettingsValue::Lookahead => UiValue::Bool(controls.lookahead),
        SettingsValue::AimToggle => UiValue::Bool(controls.aim_toggle),
        SettingsValue::CrouchToggle => UiValue::Bool(controls.crouch_toggle),
        SettingsValue::Vibration => UiValue::Index(match controls.vibration {
            Vibration::Off => 0,
            Vibration::Fire => 1,
            Vibration::Hit => 2,
            Vibration::FireAndHit => 3,
        }),
        SettingsValue::TurnSpeed => UiValue::Float(session.edit.controls.turn_speed()),
        SettingsValue::AudioRaw(word) => {
            let av = session.edit.audio_video;
            UiValue::Index(match word {
                AudioWord::MusicEnabled => av.music_enabled,
                AudioWord::SoundEnabled => av.sound_enabled,
                AudioWord::MusicVolume => av.music_volume,
                AudioWord::SoundVolume => av.sound_volume,
                AudioWord::Slider4 => av.slider_4,
                AudioWord::Option5 => av.option_5,
            })
        }
    })
}

fn write_setting_value(
    session: &mut SettingsPageSession,
    setting: SettingsValue,
    value: UiValue,
) -> Result<(), ()> {
    use tsfp_format_profile::settings::Controls;
    match setting {
        SettingsValue::TurnSpeed => {
            let UiValue::Float(speed) = value else {
                return Err(());
            };
            return session.edit.controls.set_turn_speed(speed).map_err(|_| ());
        }
        SettingsValue::AudioRaw(word) => {
            let UiValue::Index(raw) = value else {
                return Err(());
            };
            let av = &mut session.edit.audio_video;
            match word {
                AudioWord::MusicEnabled => av.music_enabled = raw,
                AudioWord::SoundEnabled => av.sound_enabled = raw,
                AudioWord::MusicVolume if raw <= 16 => av.music_volume = raw,
                AudioWord::SoundVolume if raw <= 16 => av.sound_volume = raw,
                AudioWord::Slider4 if raw <= 64 => av.slider_4 = raw,
                AudioWord::Option5 => av.option_5 = raw,
                _ => return Err(()),
            }
            return Ok(());
        }
        _ => {}
    }
    let mut controls: Controls = session.edit.controls.controls().map_err(|_| ())?;
    match setting {
        SettingsValue::Crosshair => {
            let UiValue::Index(index) = value else {
                return Err(());
            };
            controls.crosshair = match index {
                0 => Crosshair::Off,
                1 => Crosshair::Moving,
                2 => Crosshair::Fixed,
                _ => return Err(()),
            };
        }
        SettingsValue::WeaponChange => {
            let UiValue::Index(index) = value else {
                return Err(());
            };
            controls.weapon_change = match index {
                0 => WeaponChange::Always,
                1 => WeaponChange::Never,
                2 => WeaponChange::Best,
                3 => WeaponChange::IfNew,
                4 => WeaponChange::IfNewAndBest,
                _ => return Err(()),
            };
        }
        SettingsValue::Vibration => {
            let UiValue::Index(index) = value else {
                return Err(());
            };
            controls.vibration = match index {
                0 => Vibration::Off,
                1 => Vibration::Fire,
                2 => Vibration::Hit,
                3 => Vibration::FireAndHit,
                _ => return Err(()),
            };
        }
        SettingsValue::InverseLook => controls.inverse_look = boolean(value)?,
        SettingsValue::AutoAim => controls.auto_aim = boolean(value)?,
        SettingsValue::Lookahead => controls.lookahead = boolean(value)?,
        SettingsValue::AimToggle => controls.aim_toggle = boolean(value)?,
        SettingsValue::CrouchToggle => controls.crouch_toggle = boolean(value)?,
        SettingsValue::TurnSpeed | SettingsValue::AudioRaw(_) => unreachable!(),
    }
    session.edit.controls.set_controls(controls);
    Ok(())
}

fn boolean(value: UiValue) -> Result<bool, ()> {
    if let UiValue::Bool(value) = value {
        Ok(value)
    } else {
        Err(())
    }
}
