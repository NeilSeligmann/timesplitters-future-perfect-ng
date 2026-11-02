// SPDX-License-Identifier: GPL-3.0-or-later
//! UI-7 native layers. New identities never enter guest records or native guest commands.
use std::sync::{Arc, Mutex, OnceLock};
use tsfp_input::{
    bindings::{Action, Bindings},
    settings::{InputKind, InputSettings, SettingEdit},
};
use tsfp_ui_ext::mk_settings::{self, RowGesture, RowId, RowValue};
use tsfp_ui_ext::settings_page::{
    AudioWord, SettingsPageContext, SettingsPageSession, SettingsRoutes, SettingsValue,
};
use tsfp_ui_input::native_page::{GuestUi, PageRef};
use tsfp_ui_input::{native_page::NativeCommand, widgets::Step};
use tsfp_ui_model::{menu_rect, Align, Page, TextRef, Widget, WidgetId, WidgetKind};
use tsfp_ui_renderer::Vec2;
use tsfp_ui_wgpu::in_frame::LiveContent;

const CONTROLS: u32 = 0x2E_A8E0;
const ENTRY: WidgetId = WidgetId::patch(0x2190);
const ROW_BASE: u32 = 0x2191;

struct RebindCapture {
    kind: InputKind,
    previous: String,
    action: Action,
    candidate: Option<String>,
}
#[derive(Default)]
struct NativeExtensions {
    settings: InputSettings,
    player: usize,
    /// None: original rows; Some while closed: entry; Some while open: owned row.
    focus: Option<usize>,
    open: bool,
    entry_rect: Option<[f32; 4]>,
    notice: String,
    pending: Option<InputSettings>,
    capture: Option<RebindCapture>,
    suppress_command: bool,
    suppress_pointer: bool,
    devices_edited: bool,
    settings_adopted: bool,
    controller_settings: tsfp_ui_ext::controller_settings::LiveSettings,
}
static STATE: OnceLock<Mutex<NativeExtensions>> = OnceLock::new();
#[derive(Clone)]
struct NativeSettings {
    player: usize,
    session: SettingsPageSession,
}
static SETTINGS: OnceLock<Mutex<Option<NativeSettings>>> = OnceLock::new();
fn settings_state() -> &'static Mutex<Option<NativeSettings>> {
    SETTINGS.get_or_init(|| Mutex::new(None))
}
fn state() -> &'static Mutex<NativeExtensions> {
    STATE.get_or_init(|| Mutex::new(NativeExtensions::default()))
}

pub fn enabled() -> bool {
    std::env::var("TSFP_UI_EXTENSIONS").is_ok_and(|v| {
        v.split(',')
            .any(|s| matches!(s.trim(), "mk-settings" | "all"))
    })
}
pub fn initialise(bindings: &Bindings, mouse_keyboard: bool) {
    if let Ok(mut s) = state().lock() {
        s.settings.bindings = bindings.clone();
        s.settings.other_bindings = std::array::from_fn(|_| bindings.clone());
        if !mouse_keyboard {
            s.settings.players[0] = tsfp_input::settings::Device::Pad(0);
        }
    }
}
/// Native storage frontend calls this after profile load/create. No guest-memory adapter.
pub fn adopt_profile(settings: InputSettings) {
    adopt_policy(ProfileInputPolicy {
        fixed_devices: settings.fixed_devices,
        settings,
    });
}
#[derive(Clone)]
pub struct ProfileInputPolicy {
    pub settings: InputSettings,
    pub fixed_devices: bool,
}
pub fn profile_policy() -> Option<ProfileInputPolicy> {
    state().lock().ok().map(|s| ProfileInputPolicy {
        settings: s.settings.clone(),
        fixed_devices: s.devices_edited,
    })
}
/// A legacy profile inherits the launch policy without pinning devices.
pub fn adopt_policy(policy: ProfileInputPolicy) {
    if let Ok(mut s) = state().lock() {
        s.adopt_policy(policy);
    }
}
/// Native storage owner consumes edits as Command::SaveInput; a failed save must be surfaced.
pub fn take_profile_edit() -> Option<InputSettings> {
    state().lock().ok().and_then(|mut s| s.pending.take())
}
pub fn update_controller_settings(
    devices: Vec<tsfp_input::controller::ConnectedController>,
    profiles: tsfp_input::controller::ControllerProfiles,
) {
    if let Ok(mut s) = state().lock() {
        s.controller_settings.update(devices, profiles);
    }
}
pub fn take_controller_profile_edit(
) -> Option<tsfp_ui_ext::controller_settings::ControllerProfileEdit> {
    state()
        .lock()
        .ok()
        .and_then(|mut s| s.controller_settings.take_edit())
}
/// Deactivation must discard edits belonging to the previous profile.
pub fn clear_profile() {
    adopt_policy(ProfileInputPolicy {
        settings: InputSettings::default(),
        fixed_devices: false,
    });
}
/// Persistence failure is visible and never represented as a successful edit.
pub fn profile_save_failed(status: u32) {
    if let Ok(mut s) = state().lock() {
        s.notice = format!("Input settings were not saved ({status:#010x})");
    }
}
/// Physical binding capture enters the same owned handler for keyboard and mouse.
pub fn capture_input(kind: InputKind, name: &str) -> bool {
    if !enabled() {
        return false;
    }
    state()
        .lock()
        .is_ok_and(|mut s| s.capture_input(kind, name))
}
pub fn open() -> bool {
    enabled() && state().lock().is_ok_and(|s| s.open)
}

pub fn track(builder: u32) {
    if builder != CONTROLS {
        if let Ok(mut s) = state().lock() {
            s.open = false;
            s.focus = None;
            s.entry_rect = None;
            s.capture = None;
        }
    }
}
/// Unedited session defaults retain the launcher's automatic pad hot-swap.
pub fn devices_edited() -> bool {
    state().lock().is_ok_and(|s| s.devices_edited)
}
pub fn settings() -> Option<InputSettings> {
    state()
        .lock()
        .ok()
        .and_then(|s| s.effective_settings(enabled()).cloned())
}

const SETTINGS_ROOT: u32 = 0x2C_BFC0;
const SETTINGS_CONTROLS: u32 = 0x2E_A8E0;
const SETTINGS_AUDIO_VIDEO: u32 = 0x2C_3E20;
const SETTINGS_CONFIRM: u32 = 0x2E_9EC0;
const SETTINGS_SLOT_COUNT: u32 = 0x5236C4;

fn settings_routes() -> SettingsRoutes {
    SettingsRoutes {
        root: tsfp_ui_model::PageId(u64::from(SETTINGS_ROOT)),
        controls: tsfp_ui_model::PageId(u64::from(SETTINGS_CONTROLS)),
        audio_video: tsfp_ui_model::PageId(u64::from(SETTINGS_AUDIO_VIDEO)),
        confirmation: tsfp_ui_model::PageId(u64::from(SETTINGS_CONFIRM)),
    }
}

/// Adopt the active Rust-owned profile when the original Settings page is on screen.
/// Multi-slot selector results are kept on the existing profile-selector boundary until its
/// slot-to-owned-profile adapter is available; a mismatched selector never gets guessed here.
pub fn prepare_settings_page(builder: u32, guest: &dyn GuestUi) -> bool {
    if !matches!(
        builder,
        SETTINGS_ROOT | SETTINGS_CONTROLS | SETTINGS_AUDIO_VIDEO | SETTINGS_CONFIRM
    ) {
        return false;
    }
    let Some(player) = guest
        .read32(0x78AD34)
        .map(|p| p as usize)
        .filter(|p| *p < 4)
    else {
        return false;
    };
    // The source contract exposes a configured-slot count distinct from local players. This
    // first runtime attachment owns the single-slot case; no ordinal mapping is fabricated.
    if guest.read32(SETTINGS_SLOT_COUNT) != Some(1) {
        return false;
    }
    let Some(profile) = crate::storage_live::native_profile(player).map(|p| p.saved.profile) else {
        return false;
    };
    let mode = guest.read32(0x79094C).unwrap_or(0);
    let viewport = guest.read32(0x7BA9E0).unwrap_or(0) as u8;
    let Ok(mut guard) = settings_state().lock() else {
        return false;
    };
    if let Some(existing) = guard.as_mut().filter(|s| s.player == player) {
        existing.session.context.mode = mode;
        existing.session.context.viewport = viewport;
        existing.session.context.gameplay = mode > 100;
        return true;
    }
    let Ok(session) = SettingsPageSession::open(
        vec![profile],
        SettingsPageContext {
            mode,
            viewport,
            gameplay: mode > 100,
            profile_slot: 0,
            configured_slot_count: 1,
            profile_slots: vec![0],
        },
        settings_routes(),
    ) else {
        return false;
    };
    *guard = Some(NativeSettings { player, session });
    true
}

/// Drop a settings transaction after its page family closes, so the next visit starts from the
/// profile that the native storage owner most recently published.
pub fn track_settings_page(builder: Option<u32>) {
    if builder.is_some_and(|builder| {
        matches!(
            builder,
            SETTINGS_ROOT | SETTINGS_CONTROLS | SETTINGS_AUDIO_VIDEO | SETTINGS_CONFIRM
        )
    }) {
        return;
    }
    if let Ok(mut guard) = settings_state().lock() {
        *guard = None;
    }
}

fn settings_value_for(builder: u32, row: usize) -> Option<SettingsValue> {
    use SettingsValue::*;
    match builder {
        SETTINGS_CONTROLS => Some(match row {
            1 => Crosshair,
            2 => InverseLook,
            3 => AutoAim,
            4 => Lookahead,
            5 => AimToggle,
            6 => CrouchToggle,
            7 => WeaponChange,
            8 => Vibration,
            _ => return None,
        }),
        SETTINGS_AUDIO_VIDEO => Some(match row {
            0 => AudioRaw(AudioWord::MusicEnabled),
            1 => AudioRaw(AudioWord::MusicVolume),
            2 => AudioRaw(AudioWord::SoundEnabled),
            3 => AudioRaw(AudioWord::SoundVolume),
            4 => AudioRaw(AudioWord::Option5),
            _ => return None,
        }),
        _ => None,
    }
}

/// Current owned row value for the original-preset live renderer.
pub fn settings_row_value(builder: u32, cursor_row: usize) -> Option<tsfp_ui_input::UiValue> {
    let guard = settings_state().lock().ok()?;
    let settings = guard.as_ref()?;
    if builder == SETTINGS_CONTROLS && cursor_row == 9 {
        return settings.session.setting_value(SettingsValue::TurnSpeed);
    }
    settings
        .session
        .setting_value(settings_value_for(builder, cursor_row)?)
}

pub fn settings_fingerprint(builder: u32) -> u32 {
    (0..10).fold(0x1697_2379, |state, row| {
        let value = match settings_row_value(builder, row) {
            Some(tsfp_ui_input::UiValue::Bool(value)) => u32::from(value),
            Some(tsfp_ui_input::UiValue::Index(value)) => value,
            Some(tsfp_ui_input::UiValue::Float(value)) => value.to_bits(),
            _ => return state.rotate_left(5) ^ row as u32,
        };
        state.rotate_left(5) ^ value ^ row as u32
    })
}

/// Typed A/V values for the native original-preset slider presentation.
pub fn settings_audio_video() -> Option<tsfp_format_profile::settings::AudioVideo> {
    let guard = settings_state().lock().ok()?;
    let settings = guard.as_ref()?;
    Some(tsfp_ui_ext::settings_page::audio_video_edit(
        &settings.session,
    ))
}

fn save_settings_session(player: usize, session: &SettingsPageSession) -> bool {
    let Some(candidate) = session.profile(0).cloned() else {
        return false;
    };
    let Some(active) = crate::storage_live::native_profile(player) else {
        return false;
    };
    if candidate.payload == active.saved.profile.payload {
        return true;
    }
    crate::storage_live::native_save_profile_settings(player, candidate).is_ok()
}

fn pop_pages(guest: &mut dyn GuestUi, page: PageRef, count: u8) {
    let mut parent = page;
    for _ in 0..count {
        let opened = tsfp_ui_input::page_stack::pop(guest, parent);
        if opened == 0 {
            break;
        }
        parent = PageRef(opened);
    }
}

/// Native callback dispatch for the bounded Controls and Audio/Video session.
pub fn native_settings_handler(guest: &mut dyn GuestUi, handler: u32, page: PageRef) -> bool {
    if !matches!(
        handler,
        0x2CBE10
            | 0x2CBE50
            | 0x2CBFA0
            | 0x2E9FC0
            | 0x2E9E50
            | 0x2E9DF0
            | 0x2E9EA0
            | 0x2CBE00
            | 0x2D0970
            | 0x2C3AC0
            | 0x2C3BF0
    ) {
        return false;
    }
    let builder = page.builder(guest);
    if !prepare_settings_page(builder, guest) {
        return false;
    }
    let Some(player) = guest.read32(0x78AD34).map(|p| p as usize) else {
        return false;
    };
    let Ok(mut guard) = settings_state().lock() else {
        return false;
    };
    let Some(current) = guard.as_ref().filter(|s| s.player == player).cloned() else {
        return false;
    };
    let mut candidate = current.session;
    match handler {
        0x2CBE10 if builder == SETTINGS_ROOT => {
            candidate.open_audio_video();
            tsfp_ui_input::page_stack::push(guest, page, 0x52377C);
        }
        0x2CBE50 if builder == SETTINGS_ROOT => {
            candidate.open_controls();
            tsfp_ui_input::page_stack::push(guest, page, 0x524A40);
        }
        0x2CBFA0 if builder == SETTINGS_ROOT => {
            if candidate.root_back().is_err() || !save_settings_session(player, &candidate) {
                eprintln!("tsfp-bevy: native settings profile save failed for player {player}");
                return true;
            }
            pop_pages(guest, page, 1);
        }
        0x2E9FC0 if builder == SETTINGS_CONTROLS => {
            if candidate.context.is_frontend() {
                match candidate.controls_back() {
                    Ok(tsfp_ui_ext::settings_page::SettingsEffect::Open(_)) => {
                        tsfp_ui_input::page_stack::push(guest, page, 0x524A30);
                    }
                    Ok(tsfp_ui_ext::settings_page::SettingsEffect::PopPages(count)) => {
                        pop_pages(guest, page, count);
                    }
                    _ => return false,
                }
            } else {
                // The game-mode caller's profile/context predicate is intentionally not inferred.
                return false;
            }
        }
        0x2E9E50 if builder == SETTINGS_CONTROLS => {
            if candidate.accept_controls_direct().is_err()
                || !save_settings_session(player, &candidate)
            {
                eprintln!("tsfp-bevy: native Controls Accept failed for player {player}");
                return true;
            }
            pop_pages(guest, page, 1);
        }
        0x2E9DF0 if builder == SETTINGS_CONFIRM && candidate.context.is_frontend() => {
            if candidate.confirm_controls_yes_frontend().is_err()
                || !save_settings_session(player, &candidate)
            {
                eprintln!("tsfp-bevy: native Controls confirmation failed for player {player}");
                return true;
            }
            pop_pages(guest, page, 2);
        }
        0x2E9EA0 if builder == SETTINGS_CONFIRM && candidate.context.is_frontend() => {
            if candidate.confirm_controls_no_frontend().is_err() {
                return false;
            }
            pop_pages(guest, page, 1);
        }
        0x2CBE00 if builder == SETTINGS_CONFIRM && !candidate.context.is_frontend() => {
            if candidate.confirm_controls_yes_in_game().is_err()
                || !save_settings_session(player, &candidate)
            {
                eprintln!(
                    "tsfp-bevy: native in-game Controls confirmation failed for player {player}"
                );
                return true;
            }
            pop_pages(guest, page, 1);
        }
        0x2D0970 if builder == SETTINGS_CONFIRM && !candidate.context.is_frontend() => {
            if candidate.confirm_controls_no_in_game().is_err() {
                return false;
            }
            pop_pages(guest, page, 1);
        }
        0x2C3AC0 if builder == SETTINGS_AUDIO_VIDEO => {
            if candidate.accept_audio_video().is_err() || !save_settings_session(player, &candidate)
            {
                eprintln!("tsfp-bevy: native Audio/Video Accept failed for player {player}");
                return true;
            }
            pop_pages(guest, page, 1);
        }
        0x2C3BF0 if builder == SETTINGS_AUDIO_VIDEO => {
            if candidate.cancel_audio_video().is_err() {
                return false;
            }
            // The audio mixer remains the neighbouring replace-system contract; this restores
            // the owned edit and page stack without routing the settings callback through C2R.
            pop_pages(guest, page, 1);
        }
        _ => return false,
    }
    if let Some(settings) = guard.as_mut() {
        settings.session = candidate;
    }
    true
}

fn row_setting(builder: u32, row: usize) -> Option<SettingsValue> {
    if builder == SETTINGS_CONTROLS && row == 9 {
        return Some(SettingsValue::TurnSpeed);
    }
    settings_value_for(builder, row)
}

fn option_values(guest: &dyn GuestUi, node: u32) -> Vec<(u32, bool)> {
    use tsfp_ui_input::menu_state::layout::{option, row};
    let count = guest.read32(node + row::OPTION_COUNT).unwrap_or(0).min(32);
    let first = guest.read32(node + row::OPTIONS).unwrap_or(0);
    (0..count)
        .filter_map(|index| {
            let entry = first.checked_add(index.checked_mul(option::STRIDE)?)?;
            Some((
                guest.read32(entry + option::ID)?,
                guest.read32(entry + option::FLAGS)? & option::FLAG_DISABLED != 0,
            ))
        })
        .collect()
}

fn choice_value(setting: SettingsValue, value: u32) -> Option<tsfp_ui_input::UiValue> {
    use SettingsValue::*;
    Some(match setting {
        InverseLook | AutoAim | Lookahead | AimToggle | CrouchToggle => {
            tsfp_ui_input::UiValue::Bool(value != 0)
        }
        Crosshair | WeaponChange | Vibration | TurnSpeed | AudioRaw(_) => {
            tsfp_ui_input::UiValue::Index(value)
        }
    })
}

/// Handle a live left/right setting command without calling the guest row value callback.
pub fn native_settings_step(
    guest: &mut dyn GuestUi,
    page: PageRef,
    direction: Step,
) -> Option<Option<i32>> {
    if !matches!(direction, Step::Left | Step::Right) {
        return None;
    }
    let builder = page.builder(guest);
    if !matches!(builder, SETTINGS_CONTROLS | SETTINGS_AUDIO_VIDEO)
        || !prepare_settings_page(builder, guest)
    {
        return None;
    }
    let row = page.cursor(guest) as usize;
    let setting = row_setting(builder, row)?;
    let mut guard = settings_state().lock().ok()?;
    let session = &mut guard.as_mut()?.session;
    let increase = direction == Step::Right;
    if setting == SettingsValue::TurnSpeed {
        session.adjust_turn_speed(increase).ok()?;
        return Some(None);
    }
    let current = session.setting_value(setting)?;
    let row_node = page.cursor_node(guest);
    let choices = option_values(guest, row_node);
    let raw = match current {
        tsfp_ui_input::UiValue::Bool(value) => u32::from(value),
        tsfp_ui_input::UiValue::Index(value) => value,
        _ => return None,
    };
    let options = if choices.is_empty() {
        match setting {
            SettingsValue::AudioRaw(
                AudioWord::MusicEnabled | AudioWord::SoundEnabled | AudioWord::Option5,
            )
            | SettingsValue::InverseLook
            | SettingsValue::AutoAim
            | SettingsValue::Lookahead
            | SettingsValue::AimToggle
            | SettingsValue::CrouchToggle => vec![(0, false), (1, false)],
            SettingsValue::AudioRaw(AudioWord::MusicVolume | AudioWord::SoundVolume) => {
                (0..=16).map(|v| (v, false)).collect()
            }
            SettingsValue::AudioRaw(AudioWord::Slider4) => (0..=64).map(|v| (v, false)).collect(),
            _ => return None,
        }
    } else {
        choices
    };
    let current_index = options
        .iter()
        .position(|(candidate, _)| *candidate == raw)
        .unwrap_or(raw as usize);
    if options.is_empty() {
        return None;
    }
    let mut next = current_index;
    for _ in 0..options.len() {
        next = if increase {
            (next + 1) % options.len()
        } else {
            (next + options.len() - 1) % options.len()
        };
        if !options[next].1 {
            break;
        }
    }
    let (next_raw, _) = options[next];
    session
        .set_setting_value(setting, choice_value(setting, next_raw)?)
        .ok()?;
    Some(Some(next as i32))
}
impl NativeExtensions {
    fn adopt_policy(&mut self, policy: ProfileInputPolicy) {
        self.settings = policy.settings;
        self.settings.fixed_devices = policy.fixed_devices;
        self.player = 0;
        self.devices_edited = policy.fixed_devices;
        self.settings_adopted = true;
        self.pending = None;
        self.notice.clear();
        self.capture = None;
    }
    fn effective_settings(&self, ui_enabled: bool) -> Option<&InputSettings> {
        (ui_enabled || self.settings_adopted).then_some(&self.settings)
    }
}
fn ids(index: usize) -> WidgetId {
    WidgetId::patch(ROW_BASE + index as u32)
}
impl NativeExtensions {
    fn view_settings(&self) -> InputSettings {
        let mut view = self.settings.clone();
        view.bindings = self.settings.player_bindings(self.player).unwrap().clone();
        view
    }
    fn rows(&self) -> Vec<mk_settings::SettingsRow> {
        let mut rows = mk_settings::rows(&self.view_settings());
        rows.extend(self.controller_settings.rows());
        rows.push(mk_settings::SettingsRow {
            id: RowId::ResetPlayer,
            label: format!("Reset player {} to defaults", self.player + 1),
            value: RowValue::Button,
        });
        rows.push(mk_settings::SettingsRow {
            id: RowId::EditingPlayer,
            label: "Edit player".into(),
            value: RowValue::Choice {
                index: self.player,
                options: (1..=4).map(|n| format!("Player {n}")).collect(),
            },
        });
        rows
    }
}
const VISIBLE: usize = 7;
fn top_row(focus: usize) -> usize {
    focus / VISIBLE * VISIBLE
}
/// Compose owned content at frame sealing; original source content is never mutated.
pub fn compose(
    builder: u32,
    base: Arc<LiveContent>,
    focused: Option<WidgetId>,
) -> (Arc<LiveContent>, Option<WidgetId>) {
    if !enabled() || builder != CONTROLS {
        if let Ok(mut s) = state().lock() {
            s.open = false;
            s.focus = None;
        }
        return (base, focused);
    }
    let Ok(mut s) = state().lock() else {
        return (base, focused);
    };
    let mut out = (*base).clone();
    if s.open {
        let mut page = Page::new(base.page, 1);
        out.dynamic.clear();
        out.list = None;
        out.default_page = None;
        let first = top_row(s.focus.unwrap_or(0));
        for (i, row) in s.rows().into_iter().enumerate().skip(first).take(VISIBLE) {
            let value = match row.value {
                RowValue::Button => "Activate".into(),
                RowValue::Slider { value, .. } => format!("{value:.3}"),
                RowValue::Toggle(v) => if v { "On" } else { "Off" }.into(),
                RowValue::Choice { index, options } => options[index].clone(),
                RowValue::Binding { action } => action,
            };
            let mut w = Widget::new(ids(i), WidgetKind::Button);
            w.rect = menu_rect(
                Vec2::new(150.0, 140.0 + (i - first) as f32 * 28.0),
                Vec2::new(340.0, 28.0),
            );
            w.text = TextRef::Dynamic;
            w.font_index = 4;
            w.align = Align::Center;
            out.dynamic
                .insert(w.id, format!("{}: {value}", row.label).into_bytes());
            page.widgets.push(w);
        }
        for (number, x, label) in [(0, 60.0, "Previous page"), (1, 340.0, "Next page")] {
            let mut nav = Widget::new(WidgetId::patch(0x21f0 + number), WidgetKind::Button);
            nav.rect = menu_rect(Vec2::new(x, 350.0), Vec2::new(240.0, 28.0));
            nav.text = TextRef::Dynamic;
            nav.font_index = 4;
            nav.align = Align::Center;
            out.dynamic.insert(nav.id, label.as_bytes().to_vec());
            page.widgets.push(nav);
        }
        let mut help = Widget::new(WidgetId::patch(0x21ff), WidgetKind::Label);
        help.rect = menu_rect(Vec2::new(60.0, 385.0), Vec2::new(520.0, 20.0));
        help.text = TextRef::Dynamic;
        help.font_index = 4;
        help.align = Align::Center;
        out.dynamic.insert(
            help.id,
            if s.notice.is_empty() {
                b"Arrows: change / browse   Enter: change   Esc: back".to_vec()
            } else {
                s.notice.as_bytes().to_vec()
            },
        );
        page.widgets.push(help);
        out.model.pages.insert(base.page, page);
    } else if let Some(page) = out.model.pages.get_mut(&base.page) {
        if let Some(w) = inject_entry(page) {
            s.entry_rect = Some([
                w.rect.offset.x,
                w.rect.offset.y,
                w.rect.offset.x + w.rect.size.x,
                w.rect.offset.y + w.rect.size.y,
            ]);
            out.dynamic.insert(ENTRY, b"Input settings".to_vec());
            page.widgets.push(w);
        }
    }
    let focus = s
        .focus
        .map(|i| if s.open { ids(i) } else { ENTRY })
        .or(focused);
    (Arc::new(out), focus)
}

/// Split the last Controls action row (Cancel) into two owned presentation cells.
/// The guest Cancel action and record remain intact; the new cell has its own native handler.
fn inject_entry(page: &mut Page) -> Option<Widget> {
    let cancel = page.widgets.iter_mut().rev().find(|w| w.is_focusable())?;
    let mut entry = Widget::new(ENTRY, WidgetKind::Button);
    entry.rect = cancel.rect;
    let half = cancel.rect.size.x * 0.5;
    cancel.rect.size.x = half;
    if let Some(bounds) = &mut cancel.bounds {
        bounds.size.x = bounds.size.x.min(half);
    }
    cancel.align = Align::Center;
    if let Some(anchor) = &mut cancel.text_anchor {
        anchor.x = half * 0.5;
        anchor.align = Align::Center;
    }
    entry.rect.offset.x += half;
    entry.rect.size.x = half;
    entry.text = TextRef::Dynamic;
    entry.font_index = cancel.font_index;
    entry.text_anchor = cancel.text_anchor;
    entry.align = Align::Center;
    Some(entry)
}

/// Native keyboard/controller navigation across the extension boundary. Returns true when consumed.
pub fn command(builder: u32, command: &NativeCommand, cursor: usize, rows: usize) -> bool {
    if !enabled() || builder != CONTROLS {
        return false;
    }
    let Ok(mut s) = state().lock() else {
        return false;
    };
    let handled = s.command(command, cursor, rows);
    if handled {
        eprintln!(
            "tsfp-bevy: ui-ext: {command:?} open={} focus={:?} look={:?}",
            s.open, s.focus, s.settings.bindings.sensitivity
        );
    }
    handled
}

impl NativeExtensions {
    fn command(&mut self, command: &NativeCommand, cursor: usize, rows: usize) -> bool {
        if self.suppress_command {
            self.suppress_command = false;
            return true;
        }
        if self.capture.is_some() {
            match command {
                NativeCommand::Back => {
                    self.capture = None;
                    self.notice.clear();
                }
                NativeCommand::Accept
                    if self.capture.as_ref().is_some_and(|c| c.candidate.is_some()) =>
                {
                    self.finish_capture()
                }
                _ => {}
            }
            return true;
        }
        if !self.open && self.focus.is_none() {
            if matches!(command, NativeCommand::Step(Step::Down)) && cursor + 1 == rows
                || matches!(command, NativeCommand::Step(Step::Up)) && cursor == 0
            {
                self.focus = Some(3);
                return true;
            }
            return false;
        }
        if !self.open {
            match command {
                NativeCommand::Accept => {
                    self.open = true;
                    self.focus = Some(0);
                }
                NativeCommand::Step(Step::Up | Step::Down) | NativeCommand::Back => {
                    self.focus = None;
                    return false;
                }
                NativeCommand::Focus(_) | NativeCommand::Select(_) => {
                    self.focus = None;
                    return false;
                }
                _ => {}
            }
            return true;
        }
        let count = self.rows().len();
        let i = self.focus.unwrap_or(0).min(count - 1);
        let gesture = match command {
            NativeCommand::Back => {
                self.open = false;
                self.focus = Some(3);
                return true;
            }
            NativeCommand::Step(Step::Up) => {
                self.focus = Some((i + count - 1) % count);
                return true;
            }
            NativeCommand::Step(Step::Down) => {
                self.focus = Some((i + 1) % count);
                return true;
            }
            NativeCommand::Step(Step::Left) => RowGesture::Step(-1),
            NativeCommand::Step(Step::Right) => RowGesture::Step(1),
            NativeCommand::Accept => RowGesture::Activate,
            _ => return true,
        };
        self.edit(i, gesture);
        true
    }
}

impl NativeExtensions {
    fn edit(&mut self, index: usize, mut gesture: RowGesture) {
        let id = self.rows()[index].id.clone();
        if matches!(
            id,
            RowId::ControllerButton(_, _)
                | RowId::ControllerAxis(_, _)
                | RowId::ControllerCalibrationDeadzone(_, _)
                | RowId::ControllerCalibrationInvert(_, _)
        ) {
            match self.controller_settings.edit(&id, gesture) {
                Ok(true) => self.notice = "Controller profile changed".into(),
                Ok(false) => self.notice = "Controller disconnected".into(),
                Err(error) => self.notice = error,
            }
            return;
        }
        if id == RowId::ResetPlayer {
            // Navigation alone must never erase the player's custom map.
            if matches!(gesture, RowGesture::Activate) {
                let outcome = self.settings.reset_player(self.player).unwrap();
                self.devices_edited = true;
                self.capture = None;
                self.pending = Some(self.settings.clone());
                self.focus = Some(
                    self.rows()
                        .iter()
                        .position(|r| r.id == RowId::ResetPlayer)
                        .unwrap(),
                );
                self.notice = if let Some(other) = outcome.displaced_players.first() {
                    format!("Defaults restored; device moved from player {}", other + 1)
                } else {
                    format!("Player {} defaults restored", self.player + 1)
                };
            }
            return;
        }
        if id == RowId::EditingPlayer {
            let n = match gesture {
                RowGesture::Step(n) => n,
                _ => 1,
            };
            self.player = (self.player as i32 + n).rem_euclid(4) as usize;
            self.notice = format!("Editing player {}", self.player + 1);
            self.capture = None;
            return;
        }
        if let RowId::Binding(kind, input) = &id {
            if matches!(gesture, RowGesture::Activate) {
                let map = if *kind == mk_settings::InputKindKey::Key {
                    &self.settings.player_bindings(self.player).unwrap().keys
                } else {
                    &self.settings.player_bindings(self.player).unwrap().mouse
                };
                self.capture = Some(RebindCapture {
                    kind: if *kind == mk_settings::InputKindKey::Key {
                        InputKind::Key
                    } else {
                        InputKind::Mouse
                    },
                    previous: input.clone(),
                    action: map.get(input).copied().unwrap_or(Action::None),
                    candidate: None,
                });
                self.notice = "Press a key/button; Esc cancels".into();
                return;
            }
            use tsfp_input::bindings::Action;
            let choices = [
                Action::None,
                Action::MoveForward,
                Action::MoveBack,
                Action::MoveLeft,
                Action::MoveRight,
                Action::Walk,
                Action::A,
                Action::B,
                Action::X,
                Action::Y,
                Action::Start,
                Action::Back,
                Action::L3,
                Action::R3,
                Action::Black,
                Action::White,
                Action::Lt,
                Action::Rt,
                Action::DpadUp,
                Action::DpadDown,
                Action::DpadLeft,
                Action::DpadRight,
            ];
            let map = if *kind == mk_settings::InputKindKey::Key {
                &self.settings.player_bindings(self.player).unwrap().keys
            } else {
                &self.settings.player_bindings(self.player).unwrap().mouse
            };
            let current = map.get(input).copied().unwrap_or(Action::None);
            let at = choices.iter().position(|a| *a == current).unwrap_or(0) as i32;
            let step = match gesture {
                RowGesture::Step(n) => n,
                _ => 1,
            };
            gesture =
                RowGesture::Bind(choices[(at + step).rem_euclid(choices.len() as i32) as usize]);
        } else if matches!(gesture, RowGesture::Activate)
            && !matches!(id, RowId::InvertPitch | RowId::FixedDevices)
        {
            gesture = RowGesture::Step(1);
        }
        if let Some(edit) = mk_settings::edit_for(&self.view_settings(), &id, &gesture) {
            match self.settings.apply_player(self.player, edit) {
                Ok(outcome) => {
                    self.devices_edited = self.settings.fixed_devices;
                    self.notice = if !outcome.displaced_players.is_empty() {
                        format!(
                            "Device reassigned from player {}",
                            outcome.displaced_players[0] + 1
                        )
                    } else if !outcome.shared_with.is_empty() {
                        "Action also has other bindings".into()
                    } else {
                        "Setting changed".into()
                    };
                    self.pending = Some(self.settings.clone());
                    eprintln!("tsfp-bevy: input settings: {}", self.notice);
                }
                Err(e) => self.notice = e,
            }
        }
    }
}

impl NativeExtensions {
    fn capture_input(&mut self, kind: InputKind, name: &str) -> bool {
        let Some(capture) = &self.capture else {
            return false;
        };
        if kind == InputKind::Key && name == "Escape" {
            self.capture = None;
            self.notice.clear();
            self.suppress_command = true;
            return true;
        }
        if capture.candidate.is_some() {
            return true;
        }
        if capture.kind != kind {
            return true;
        }
        self.suppress_command = kind == InputKind::Key
            && matches!(
                name,
                "Enter" | "NumpadEnter" | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
            );
        self.suppress_pointer = kind == InputKind::Mouse;
        let map = if kind == InputKind::Key {
            &self.settings.player_bindings(self.player).unwrap().keys
        } else {
            &self.settings.player_bindings(self.player).unwrap().mouse
        };
        let conflict = map
            .get(name)
            .is_some_and(|a| *a != capture.action && *a != Action::None);
        self.capture.as_mut().unwrap().candidate = Some(name.to_owned());
        if conflict {
            self.notice = "Conflict: Enter/click here replaces; Esc cancels".into();
        } else {
            self.finish_capture();
        }
        true
    }
    fn finish_capture(&mut self) {
        let Some(capture) = self.capture.take() else {
            return;
        };
        let Some(input) = capture.candidate else {
            return;
        };
        let mut next = self.settings.clone();
        match next.apply_player(
            self.player,
            SettingEdit::Rebind {
                kind: capture.kind,
                input: input.clone(),
                action: capture.action,
            },
        ) {
            Ok(outcome) => {
                if capture.previous != input {
                    let map = if capture.kind == InputKind::Key {
                        &mut next.player_bindings_mut(self.player).unwrap().keys
                    } else {
                        &mut next.player_bindings_mut(self.player).unwrap().mouse
                    };
                    // Parser overlays defaults: persist an explicit unbind, not an omission.
                    map.insert(capture.previous.clone(), Action::None);
                }
                self.settings = next;
                self.pending = Some(self.settings.clone());
                self.notice = if outcome.shared_with.is_empty() {
                    "Binding changed".into()
                } else {
                    "Binding changed; action also has other bindings".into()
                };
                let count = self.rows().len();
                self.focus = self.focus.map(|i| i.min(count - 1));
            }
            Err(e) => self.notice = e,
        }
    }
}

/// Pointer uses owned widget ids and handlers, never a guest row index.
pub fn pointer(
    builder: u32,
    pos: Option<(f32, f32)>,
    moved: bool,
    left: bool,
    right: bool,
) -> bool {
    if !enabled() || builder != CONTROLS {
        return false;
    }
    let Ok(mut s) = state().lock() else {
        return false;
    };
    s.pointer(pos, moved, left, right)
}

impl NativeExtensions {
    fn pointer(&mut self, pos: Option<(f32, f32)>, moved: bool, left: bool, right: bool) -> bool {
        if self.suppress_pointer {
            self.suppress_pointer = false;
            return true;
        }
        if self.capture.is_some() {
            if right {
                self.capture = None;
                self.notice.clear();
            } else if left
                && self.capture.as_ref().is_some_and(|c| c.candidate.is_some())
                && pos
                    .is_some_and(|(x, y)| (60.0..580.0).contains(&x) && (385.0..405.0).contains(&y))
            {
                self.finish_capture();
            }
            return true;
        }
        if self.open {
            if right {
                self.open = false;
                self.focus = Some(3);
                return true;
            }
            if left
                && pos
                    .is_some_and(|(x, y)| (60.0..580.0).contains(&x) && (350.0..378.0).contains(&y))
            {
                let count = self.rows().len();
                let first = top_row(self.focus.unwrap_or(0));
                self.focus = Some(if pos.unwrap().0 < 320.0 {
                    first.saturating_sub(VISIBLE)
                } else {
                    (first + VISIBLE).min(count - 1)
                });
                return true;
            }
            if let Some((_, y)) = pos.filter(|(x, _)| (150.0..490.0).contains(x)) {
                let first = top_row(self.focus.unwrap_or(0));
                let count = self.rows().len();
                for i in first..(first + VISIBLE).min(count) {
                    let top = 140.0 + (i - first) as f32 * 28.0;
                    if (top..top + 28.0).contains(&y) {
                        if moved || left {
                            self.focus = Some(i);
                        }
                        if left {
                            self.edit(i, RowGesture::Activate);
                        }
                    }
                }
            }
            return true;
        }
        if pos
            .zip(self.entry_rect)
            .is_some_and(|((x, y), r)| (r[0]..r[2]).contains(&x) && (r[1]..r[3]).contains(&y))
        {
            if moved {
                self.focus = Some(3);
            }
            if left {
                self.open = true;
                self.focus = Some(0);
                eprintln!("tsfp-bevy: ui-ext: pointer opened owned mouse settings");
            }
            return true;
        }
        if moved || left {
            self.focus = None;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_profiles_apply_with_hidden_ui_and_legacy_profiles_restore_launch_policy() {
        let mut state = NativeExtensions::default();
        assert!(state.effective_settings(false).is_none());
        let mut launch = InputSettings::default();
        launch.apply(SettingEdit::SensitivityYaw(0.08)).unwrap();
        state.adopt_policy(ProfileInputPolicy {
            settings: launch.clone(),
            fixed_devices: false,
        });
        assert_eq!(state.effective_settings(false), Some(&launch));
        assert!(!state.devices_edited);
        let mut native = launch.clone();
        native.apply(SettingEdit::SensitivityYaw(0.12)).unwrap();
        native.fixed_devices = true;
        state.adopt_policy(ProfileInputPolicy {
            settings: native.clone(),
            fixed_devices: true,
        });
        assert_eq!(state.effective_settings(false), Some(&native));
        assert!(state.devices_edited);
        state.pending = Some(native);
        state.adopt_policy(ProfileInputPolicy {
            settings: launch.clone(),
            fixed_devices: false,
        });
        assert!(state.pending.is_none());
        assert_eq!(state.effective_settings(false), Some(&launch));
        assert!(!state.devices_edited);
    }
    #[test]
    fn device_policy_toggle_is_saved_and_restored_without_look_edit_pinning() {
        let mut s = NativeExtensions::default();
        s.edit(0, RowGesture::Step(1));
        let saved = InputSettings::parse(&s.pending.take().unwrap().to_text()).unwrap();
        assert!(!saved.fixed_devices);
        s.adopt_policy(ProfileInputPolicy {
            fixed_devices: saved.fixed_devices,
            settings: saved,
        });
        assert!(!s.devices_edited);
        let policy_row = s
            .rows()
            .iter()
            .position(|r| r.id == RowId::FixedDevices)
            .unwrap();
        s.edit(policy_row, RowGesture::Activate);
        assert!(s.devices_edited);
        let saved = InputSettings::parse(&s.pending.take().unwrap().to_text()).unwrap();
        assert!(saved.fixed_devices);
        s.edit(policy_row, RowGesture::Step(-1));
        assert!(!s.devices_edited);
        assert!(!s.pending.take().unwrap().fixed_devices);
    }

    #[test]
    fn injected_page_navigation_and_edits_are_owned() {
        let mut s = NativeExtensions::default();
        assert!(!s.command(&NativeCommand::Step(Step::Down), 0, 5));
        assert!(s.command(&NativeCommand::Step(Step::Down), 4, 5));
        assert_eq!(s.focus, Some(3));
        assert!(s.command(&NativeCommand::Accept, 4, 5));
        assert!(s.open);
        let before = s.settings.bindings.sensitivity;
        assert!(s.command(&NativeCommand::Step(Step::Right), 4, 5));
        assert!(
            !s.devices_edited,
            "look edits preserve automatic device hot-swap"
        );
        assert!(
            (s.settings.bindings.sensitivity.yaw_deg_per_count - before.yaw_deg_per_count - 0.001)
                .abs()
                < 1e-12
        );
        s.command(&NativeCommand::Step(Step::Down), 4, 5);
        s.command(&NativeCommand::Step(Step::Down), 4, 5);
        s.command(&NativeCommand::Accept, 4, 5);
        assert_eq!(
            s.settings.bindings.sensitivity.invert_pitch,
            !before.invert_pitch
        );
        assert!(s.command(&NativeCommand::Back, 4, 5));
        assert!(!s.open);
        assert!(!s.command(&NativeCommand::Step(Step::Down), 4, 5));
        assert_eq!(s.focus, None, "navigation hands back to the original rows");
    }
    #[test]
    fn owned_values_clamp_and_up_wraps_without_guest_calls() {
        let mut s = NativeExtensions {
            open: true,
            focus: Some(0),
            ..Default::default()
        };
        s.settings.bindings.sensitivity.yaw_deg_per_count = tsfp_input::settings::SENSITIVITY_MAX;
        assert!(s.command(&NativeCommand::Step(Step::Right), 0, 1));
        assert_eq!(
            s.settings.bindings.sensitivity.yaw_deg_per_count,
            tsfp_input::settings::SENSITIVITY_MAX
        );
        assert!(s.command(&NativeCommand::Step(Step::Up), 0, 1));
        assert_eq!(s.focus, Some(s.rows().len() - 1));
        assert!(
            s.command(&NativeCommand::Select(5000), 0, 1),
            "never forwards owned-page actions to guest rows"
        );
    }
}

/// Bounded, explicit test route through the native command seam (not a player input adapter).
/// Main menu entry uses TSFP_UI_AUTOSELECT=6; this selects Controls then exercises the owned page.
pub fn diagnostic(builder: u32, now: u64) -> Vec<NativeCommand> {
    let check = std::env::var("TSFP_UI_EXTENSION_CHECK").unwrap_or_default();
    if check != "1" && check != "bindings" {
        return Vec::new();
    }
    static ROUTE: Mutex<(u32, u64, usize)> = Mutex::new((0, 0, 0));
    let Ok(mut route) = ROUTE.lock() else {
        return Vec::new();
    };
    if route.0 != builder {
        *route = (builder, now + 2000, 0);
    }
    if now < route.1 {
        return Vec::new();
    }
    let mut commands = if builder == 0x2C_BFC0 {
        vec![NativeCommand::Select(1)]
    } else if builder == CONTROLS && enabled() {
        vec![
            NativeCommand::Step(Step::Up),
            NativeCommand::Accept,
            NativeCommand::Step(Step::Right),
            NativeCommand::Step(Step::Down),
            NativeCommand::Step(Step::Right),
            NativeCommand::Step(Step::Down),
            NativeCommand::Accept,
            NativeCommand::Back,
        ]
    } else {
        Vec::new()
    };
    if check == "bindings" && builder == CONTROLS && enabled() {
        commands.pop();
        commands.extend(std::iter::repeat_n(NativeCommand::Step(Step::Down), 5));
        commands.push(NativeCommand::Step(Step::Right));
    }
    let command = commands.get(route.2).copied();
    route.2 += usize::from(command.is_some());
    route.1 = now + 2000;
    command.into_iter().collect()
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn entry_shares_cancel_band_without_overlapping_or_expanding_the_page() {
        let mut page = Page::new(tsfp_ui_model::PageId(1), 1);
        let mut cancel = Widget::new(WidgetId(7), WidgetKind::Button);
        cancel.rect = menu_rect(Vec2::new(70.0, 380.0), Vec2::new(500.0, 24.0));
        cancel.text_anchor = Some(tsfp_ui_model::TextAnchor {
            x: 250.0,
            y: 2.0,
            align: Align::Center,
        });
        page.widgets.push(cancel);
        let entry = inject_entry(&mut page).unwrap();
        let cancel = &page.widgets[0];
        assert_eq!(
            cancel.rect.offset.x + cancel.rect.size.x,
            entry.rect.offset.x
        );
        assert_eq!(entry.rect.offset.x + entry.rect.size.x, 570.0);
        assert_eq!(entry.rect.offset.y, cancel.rect.offset.y);
        assert_eq!(entry.rect.size.y, cancel.rect.size.y);
        assert_eq!(cancel.text_anchor.unwrap().x, 125.0);
        assert_eq!(entry.text_anchor, cancel.text_anchor);
        assert!(inject_entry(&mut Page::new(page.id, 1)).is_none());
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::*;
    #[test]
    fn mouse_edits_owned_rows_and_returns_without_guest_row_indices() {
        let mut s = NativeExtensions {
            entry_rect: Some([320.0, 380.0, 570.0, 404.0]),
            ..Default::default()
        };
        assert!(
            !s.pointer(Some((100.0, 390.0)), true, true, false),
            "Cancel stays original"
        );
        assert!(s.pointer(Some((400.0, 390.0)), true, true, false));
        assert!(s.open);
        let before = s.settings.bindings.sensitivity.yaw_deg_per_count;
        assert!(s.pointer(Some((300.0, 150.0)), true, true, false));
        assert!((s.settings.bindings.sensitivity.yaw_deg_per_count - before - 0.001).abs() < 1e-12);
        assert!(s.pointer(Some((300.0, 210.0)), true, true, false));
        assert!(s.settings.bindings.sensitivity.invert_pitch);
        assert_eq!(s.focus, Some(2));
        assert!(s.pointer(None, false, false, true));
        assert!(!s.open);
        assert!(!s.pointer(None, true, false, false));
        assert_eq!(s.focus, None);
    }
}

#[cfg(test)]
mod profile_edit_tests {
    use super::*;
    #[test]
    fn page_navigation_device_conflicts_and_binding_edits_emit_owned_save_candidates() {
        let mut s = NativeExtensions {
            open: true,
            focus: Some(3),
            ..Default::default()
        };
        s.command(&NativeCommand::Step(Step::Right), 0, 1);
        assert!(s.pending.is_some());
        assert!(s.devices_edited);
        let count = s.rows().len();
        s.focus = Some(8);
        let before = s.settings.bindings.clone();
        s.command(&NativeCommand::Step(Step::Right), 0, 1);
        assert_ne!(s.settings.bindings, before);
        assert!(s.pending.is_some());
        assert!(s.pointer(Some((450.0, 360.0)), false, true, false));
        assert_eq!(s.focus, Some(14.min(count - 1)));
        assert!(s.command(&NativeCommand::Back, 0, 1));
        assert!(!s.open);
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    #[test]
    fn reset_requires_activation_and_publishes_only_selected_player_defaults() {
        let mut s = NativeExtensions {
            open: true,
            player: 2,
            ..Default::default()
        };
        s.edit(0, RowGesture::Step(5));
        s.settings
            .apply_player(
                2,
                tsfp_input::settings::SettingEdit::Rebind {
                    kind: InputKind::Key,
                    input: "KeyZ".into(),
                    action: Action::Start,
                },
            )
            .unwrap();
        let neighbour = s.settings.bindings.clone();
        let reset = s
            .rows()
            .iter()
            .position(|r| r.id == RowId::ResetPlayer)
            .unwrap();
        let before = s.settings.clone();
        s.edit(reset, RowGesture::Step(1));
        assert_eq!(s.settings, before);
        s.focus = Some(reset);
        s.command(&NativeCommand::Accept, 0, 1);
        assert_eq!(
            s.settings.player_bindings(2),
            Some(&tsfp_input::bindings::Bindings::default())
        );
        assert_eq!(s.settings.bindings, neighbour);
        assert_eq!(s.pending.as_ref(), Some(&s.settings));
        assert!(s.devices_edited);
        assert_eq!(s.rows()[s.focus.unwrap()].id, RowId::ResetPlayer);
        assert_eq!(
            InputSettings::parse(&s.pending.clone().unwrap().to_text()).unwrap(),
            s.settings
        );
        s.edit(0, RowGesture::Step(9));
        let reset = s
            .rows()
            .iter()
            .position(|r| r.id == RowId::ResetPlayer)
            .unwrap();
        s.focus = Some(reset);
        let y = 140.0 + (reset - top_row(reset)) as f32 * 28.0 + 10.0;
        assert!(s.pointer(Some((300.0, y)), true, true, false));
        assert_eq!(
            s.settings.player_bindings(2),
            Some(&tsfp_input::bindings::Bindings::default())
        );
    }

    #[test]
    fn player_selector_edits_and_captures_only_the_selected_map() {
        let mut s = NativeExtensions {
            open: true,
            ..Default::default()
        };
        let selector = s.rows().len() - 1;
        s.edit(selector, RowGesture::Step(1));
        assert_eq!(s.player, 1);
        let original = s.settings.bindings.clone();
        s.edit(0, RowGesture::Step(5));
        let index = s.rows().iter().position(|r| matches!(&r.id, RowId::Binding(mk_settings::InputKindKey::Key, k) if k == "KeyW")).unwrap();
        s.edit(index, RowGesture::Activate);
        s.capture_input(InputKind::Key, "KeyZ");
        assert_eq!(s.settings.bindings, original);
        assert_eq!(
            s.settings.player_bindings(1).unwrap().keys.get("KeyZ"),
            Some(&Action::MoveForward)
        );
        assert_eq!(
            s.settings.player_bindings(1).unwrap().keys.get("KeyW"),
            Some(&Action::None)
        );
        assert_eq!(
            InputSettings::parse(&s.settings.to_text()).unwrap(),
            s.settings
        );
        assert_ne!(
            s.settings.player_bindings(1).unwrap().sensitivity,
            original.sensitivity
        );
        assert_eq!(s.pending.as_ref(), Some(&s.settings));
        s.edit(s.rows().len() - 1, RowGesture::Step(-1));
        assert_eq!(s.view_settings().bindings, original);
    }

    #[test]
    fn physical_key_conflict_needs_confirmation_and_cancel_retains_both_bindings() {
        let mut s = NativeExtensions {
            open: true,
            ..Default::default()
        };
        let before = s.settings.clone();
        let index = s.rows().iter().position(|r| matches!(&r.id, RowId::Binding(mk_settings::InputKindKey::Key, k) if k == "KeyW")).unwrap();
        s.focus = Some(index);
        s.command(&NativeCommand::Accept, 0, 1);
        assert!(s.capture.is_some());
        assert!(s.capture_input(InputKind::Key, "KeyS"));
        assert!(s.capture.as_ref().unwrap().candidate.is_some());
        assert_eq!(s.settings, before);
        s.command(&NativeCommand::Back, 0, 1);
        assert_eq!(s.settings, before);
        s.command(&NativeCommand::Accept, 0, 1);
        s.capture_input(InputKind::Key, "KeyS");
        s.command(&NativeCommand::Accept, 0, 1);
        assert_eq!(
            s.settings.bindings.keys.get("KeyS"),
            before.bindings.keys.get("KeyW")
        );
        assert_eq!(s.settings.bindings.keys.get("KeyW"), Some(&Action::None));
        assert_eq!(
            InputSettings::parse(&s.settings.to_text()).unwrap(),
            s.settings
        );
        assert!(s.pending.is_some());
    }
    #[test]
    fn escape_cancels_capture_without_leaving_page_and_mouse_confirmation_is_explicit() {
        let mut s = NativeExtensions {
            open: true,
            ..Default::default()
        };
        s.capture = Some(RebindCapture {
            kind: InputKind::Key,
            previous: "KeyW".into(),
            action: Action::MoveForward,
            candidate: None,
        });
        s.capture_input(InputKind::Key, "Escape");
        s.command(&NativeCommand::Back, 0, 1);
        assert!(s.open);
        s.capture = Some(RebindCapture {
            kind: InputKind::Mouse,
            previous: "Left".into(),
            action: Action::Rt,
            candidate: None,
        });
        let before = s.settings.clone();
        s.capture_input(InputKind::Mouse, "Right");
        assert_eq!(s.settings, before);
        s.pointer(Some((300.0, 395.0)), false, true, false); // consume captured click
        assert_eq!(s.settings, before);
        s.pointer(Some((300.0, 395.0)), false, true, false); // explicit replace
        assert_eq!(s.settings.bindings.mouse.get("Right"), Some(&Action::Rt));
    }
}
