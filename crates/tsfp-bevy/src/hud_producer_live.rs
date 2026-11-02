// SPDX-License-Identifier: GPL-3.0-or-later
//! T2329: opt-in diagnostic observer of HUD lifecycle producers. It predicts
//! typed state, retains the original producer and compares its resulting fields.
//! No runtime state write, synthetic input or duplicate cue submission.
use crate::ui_live::HostUiGuestApi;
use std::io::Write;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use tsfp_ui_model::hud_producers::{
    GenericMessagePreset, HudSounds, HudTiming, MessageAge, MessageAges, MessagePolicy,
    MessagePresetData, MessageTextMetrics, WeaponAnimation, WeaponAnimationTuning,
};

static API: AtomicPtr<HostUiGuestApi> = AtomicPtr::new(std::ptr::null_mut());
static ENABLED: OnceLock<bool> = OnceLock::new();
static EXPECTED: Mutex<Option<Prediction>> = Mutex::new(None);
static WEAPON_CALLS: AtomicU64 = AtomicU64::new(0);
static QUEUE_CALLS: AtomicU64 = AtomicU64::new(0);
static MISMATCHES: AtomicU64 = AtomicU64::new(0);
static UNREADABLE: AtomicU64 = AtomicU64::new(0);
static STATE_TRACE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();
static STATE_SAMPLES: AtomicU64 = AtomicU64::new(0);
static STATE_ERRORS: AtomicU64 = AtomicU64::new(0);
static STATE_OWNED_SAMPLES: AtomicU64 = AtomicU64::new(0);
static MESSAGE_NATIVE_TICKS: AtomicU64 = AtomicU64::new(0);
static MESSAGE_NATIVE_ERRORS: AtomicU64 = AtomicU64::new(0);
static KIND6_BEFORE_CALLS: AtomicU64 = AtomicU64::new(0);
static KIND6_AFTER_CALLS: AtomicU64 = AtomicU64::new(0);
static KIND6_MIRRORED: AtomicU64 = AtomicU64::new(0);
static KIND6_REJECTED: AtomicU64 = AtomicU64::new(0);
static KIND6_REJECT_API: AtomicU64 = AtomicU64::new(0);
static KIND6_REJECT_ARGS: AtomicU64 = AtomicU64::new(0);
static KIND6_REJECT_PLAYER: AtomicU64 = AtomicU64::new(0);
static KIND6_REJECT_RING: AtomicU64 = AtomicU64::new(0);
static KIND6_REJECT_TUNING: AtomicU64 = AtomicU64::new(0);
static MESSAGE_NATIVE_ENABLED: OnceLock<bool> = OnceLock::new();
static MESSAGE_ADMISSION_DIAGNOSTICS: OnceLock<bool> = OnceLock::new();
static MESSAGE_ADMISSION_OUTCOMES: [AtomicU64; 10] = [const { AtomicU64::new(0) }; 10];

const ADMISSION_CALLBACK: usize = 0;
const ADMISSION_ARGUMENTS_UNREADABLE: usize = 1;
const ADMISSION_KIND_7_NATIVE: usize = 2;
const ADMISSION_GUEST_QUEUE_ACTIVE: usize = 3;
const ADMISSION_IMPORT_FAILED: usize = 4;
const ADMISSION_TEXT_UNREADABLE: usize = 5;
const ADMISSION_PRESET_UNAVAILABLE: usize = 6;
const ADMISSION_STATE_UNAVAILABLE: usize = 7;
const ADMISSION_INSERTED: usize = 8;
const ADMISSION_REJECTED: usize = 9;

unsafe extern "C" {
    fn xinput_source_port_poll_count(port: u32) -> u64;
}

fn trace_weapon(api: &HostUiGuestApi) {
    let trace = STATE_TRACE.get_or_init(|| {
        Mutex::new(std::env::var_os("TSFP_HUD_STATE_TRACE").and_then(|path| {
            std::fs::File::create(path)
                .map_err(|error| {
                    STATE_ERRORS.fetch_add(1, Ordering::Relaxed);
                    eprintln!("tsfp-bevy: HUD state trace open failed: {error}");
                })
                .ok()
        }))
    });
    let Ok(mut trace) = trace.lock() else { return };
    let Some(output) = trace.as_mut() else { return };
    let Some(slot) = api
        .read32_raw(0x7b0c7c)
        .and_then(|v| api.read32_raw(v + 4))
        .filter(|slot| *slot < 4)
    else {
        return;
    };
    let mut words = [0u32; 6];
    for (i, word) in words.iter_mut().enumerate() {
        if crate::hud_live::owned_producers() {
            if crate::hud_live::tsfp_hud_weapon_metric(slot, i as u32, word) == 0 {
                STATE_ERRORS.fetch_add(1, Ordering::Relaxed);
                return;
            }
        } else {
            let Some(value) = api.read32_raw(0x7a6160 + slot * 0x177c + 0x1750 + i as u32 * 4)
            else {
                STATE_ERRORS.fetch_add(1, Ordering::Relaxed);
                return;
            };
            *word = value;
        }
    }
    let sample = STATE_SAMPLES.fetch_add(1, Ordering::Relaxed);
    if crate::hud_live::owned_producers() {
        STATE_OWNED_SAMPLES.fetch_add(1, Ordering::Relaxed);
    }
    // SAFETY: read-only host counter; sampling runs outside the source callback.
    let poll = unsafe { xinput_source_port_poll_count(0) };
    if writeln!(
        output,
        "{sample} {poll} {} {slot} {} {} {} {} {} {}",
        crate::hud_live::frame_number(),
        words[0],
        words[1],
        words[2],
        words[3],
        words[4],
        words[5]
    )
    .is_err()
    {
        STATE_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
}

static ROW_TRACE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();
static ROW_SAMPLES: AtomicU64 = AtomicU64::new(0);
static ROW_ERRORS: AtomicU64 = AtomicU64::new(0);
static ROW_OWNED_SAMPLES: AtomicU64 = AtomicU64::new(0);
fn trace_row_actual(slot: u32, drawer: u32, first: u32, second: u32) {
    let trace = ROW_TRACE.get_or_init(|| {
        Mutex::new(
            std::env::var_os("TSFP_HUD_ROWS_TRACE")
                .and_then(|path| std::fs::File::create(path).ok()),
        )
    });
    let Ok(mut trace) = trace.lock() else { return };
    let Some(output) = trace.as_mut() else { return };
    let sample = ROW_SAMPLES.fetch_add(1, Ordering::Relaxed);
    let poll = unsafe { xinput_source_port_poll_count(0) };
    if writeln!(
        output,
        "{sample} {poll} {} {slot} {drawer} {first} {second}",
        crate::hud_live::frame_number()
    )
    .is_err()
    {
        ROW_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
}

/// Observe actual reference storage before/after its original drawer. The
/// original ABI arguments are saved by the adapter; this observer never writes.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_row_trace_abi(
    va: u32,
    esp: u32,
    cursor: u32,
    before: u32,
    finish: u32,
) -> u32 {
    if std::env::var_os("TSFP_HUD_ROWS_TRACE").is_none() {
        return 0;
    }
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return 0;
    };
    let owned = crate::hud_live::owned_producers();
    let read = || -> Option<(u32, u32, u32)> {
        let slot = api
            .read32_raw(0x7b0c7c)
            .and_then(|v| api.read32_raw(v + 4))
            .filter(|s| *s < 4)?;
        if va == 0x1334d0 {
            let side = api.read32_raw(esp.wrapping_add(20))?;
            if side > 1 {
                return None;
            }
            let mut value = 0;
            if owned {
                if crate::hud_live::tsfp_hud_row_metric(slot, side, 0, 0, &mut value) == 0 {
                    return None;
                }
            } else {
                value = (api.read32_raw(0x7a6160 + slot * 0x177c + 0x174c)? >> (side * 8)) & 255;
            }
            Some((
                slot,
                value,
                (side << 8) | (value.wrapping_sub(before) & 255),
            ))
        } else {
            let address = if va == 0x12f670 {
                api.read32_raw(esp.wrapping_add(4))?
            } else {
                cursor
            };
            let mut value = 0;
            if owned {
                if crate::hud_live::tsfp_hud_row_metric(slot, 2, 0, 0, &mut value) == 0 {
                    let origin = api.read32_raw(address)?;
                    if crate::hud_live::tsfp_hud_row_metric(slot, 2, 3, origin, &mut value) == 0 {
                        return None;
                    }
                }
            } else {
                value = api.read32_raw(address)?;
            }
            Some((slot, value, value.wrapping_sub(before)))
        }
    };
    let Some((slot, value, second)) = read() else {
        ROW_ERRORS.fetch_add(1, Ordering::Relaxed);
        return 0;
    };
    if finish != 0 {
        if owned {
            ROW_OWNED_SAMPLES.fetch_add(1, Ordering::Relaxed);
        }
        trace_row_actual(slot, va, before, second);
    }
    value
}

pub fn service(api: &HostUiGuestApi) {
    API.store(api as *const _ as *mut _, Ordering::Release);
}
fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var("TSFP_HUD_PRODUCER_SHADOW").is_ok_and(|v| v == "1"))
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_producer_native(va: u32, scratch: u32) -> i32 {
    if va == 0x14a5a0 {
        if !messages_native_enabled() {
            return 0;
        }
        let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
            return 0;
        };
        return i32::from(native_message_tick(api, scratch));
    }
    if va != 0x141340 {
        return 0;
    }
    // The producer's API is served even when the diagnostic is disabled.
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return 0;
    };
    if crate::hud_live::advance_owned_weapon(api) {
        trace_weapon(api);
        1
    } else {
        if crate::hud_live::owned_producers() {
            STATE_ERRORS.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "tsfp-bevy: native weapon update lacks required inputs; trace has no verdict"
            );
        }
        0
    }
}
struct Prediction {
    va: u32,
    words: Vec<(u32, u32)>,
}
struct Sounds<'a>(&'a HostUiGuestApi, bool);
impl HudSounds for Sounds<'_> {
    fn is_playing(&mut self, handle: i32) -> bool {
        match self.0.call_raw(0x45540, &[handle as u32]) {
            Some(value) => value != 0,
            None => {
                self.1 = false;
                false
            }
        }
    }
    fn submit(&mut self, _cue: i32) {} // observation never submits twice
}

struct NativeMessageAudio<'a> {
    api: &'a HostUiGuestApi,
    request: u32,
    failed: bool,
}

impl HudSounds for NativeMessageAudio<'_> {
    fn is_playing(&mut self, handle: i32) -> bool {
        match self.api.call_raw(0x45540, &[handle as u32]) {
            Some(value) => value != 0,
            None => {
                self.failed = true;
                false
            }
        }
    }

    // TEMPORARY-SHIM: use the guest's measured request helpers with the C gate's
    // private 0x30-byte stack buffer. T2046 removes this when native audio owns cues.
    fn submit(&mut self, cue: i32) {
        let initialized = self.api.call_raw(0x45390, &[self.request]).is_some();
        if !initialized
            || !self.api.write32_raw(self.request, cue as u32)
            || self.api.call_raw(0x45820, &[self.request]).is_none()
        {
            self.failed = true;
        }
    }
}

pub(crate) fn message_policy(api: &HostUiGuestApi) -> Option<MessagePolicy> {
    let mut policy = MessagePolicy {
        limits: [0; 14],
        conflicts: vec![],
    };
    for (index, limit) in policy.limits.iter_mut().enumerate() {
        *limit = api.read32_raw(0x7490ac + index as u32 * 4)? as i32;
    }
    for address in (0x4e72e0..0x4e7348).step_by(8) {
        policy.conflicts.push((
            api.read32_raw(address)? as usize,
            api.read32_raw(address + 4)? as usize,
        ));
    }
    Some(policy)
}

fn native_message_tick(api: &HostUiGuestApi, request: u32) -> bool {
    let Some(viewer) = api.read32_raw(0x7b0c7c) else {
        return false;
    };
    let Some(player) = api.read32_raw(viewer + 4).map(|slot| slot as i32) else {
        return false;
    };
    let Ok(slot) = usize::try_from(player) else {
        return false;
    };
    let Some(current_buttons) = crate::input_router::hud_message_buttons(slot) else {
        return false;
    };
    let Some(eligible_mask) = api.read32_raw(viewer.wrapping_add(0x20)) else {
        return false;
    };
    if !crate::hud_live::publish_message_buttons(player, current_buttons, eligible_mask) {
        return false;
    }
    if slot >= 4 || request == 0 {
        return false;
    }
    // The importer rejects unowned rings; owned queues age despite the guest mirror.
    if !crate::hud_live::import_guest_message_queue(api, slot) {
        return false;
    }
    let (Some(frame_delta), Some(rate), Some(scale), Some(full), Some(policy)) = (
        api.read32_raw(0x4e7944).map(|word| word as i32),
        api.read32_raw(0x4e8cec).map(|word| word as i32),
        api.read32_raw(0x4e794c).map(f32::from_bits),
        api.read32_raw(0x475c78).map(f32::from_bits),
        message_policy(api),
    ) else {
        return false;
    };
    if rate == 0 {
        return false;
    }
    let seconds = (full / rate as f32) * scale;
    let mut audio = NativeMessageAudio {
        api,
        request,
        failed: false,
    };
    if !crate::hud_live::advance_owned_messages(player, seconds, frame_delta, &policy, &mut audio) {
        return false;
    }
    MESSAGE_NATIVE_TICKS.fetch_add(1, Ordering::Relaxed);
    if audio.failed {
        MESSAGE_NATIVE_ERRORS.fetch_add(1, Ordering::Relaxed);
    }
    true
}

pub(crate) fn messages_native_enabled() -> bool {
    *MESSAGE_NATIVE_ENABLED.get_or_init(|| {
        std::env::var("TSFP_HUD_MESSAGES_LIFECYCLE").is_ok_and(|value| value == "1")
    })
}

pub(crate) fn hydrate_messages_for_trace(slot: usize) -> bool {
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return false;
    };
    crate::hud_live::import_guest_message_queue(api, slot)
}

/// Host trace selection flag for the T3397 reserved read-only owner window.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_messages_lifecycle_enabled() -> bool {
    messages_native_enabled()
}

struct GuestPresetData<'a>(&'a HostUiGuestApi);
impl MessagePresetData for GuestPresetData<'_> {
    fn word(&self, address: u32) -> Option<u32> {
        self.0.read32_raw(address)
    }
}

struct NativeMessageMetrics;
impl MessageTextMetrics for NativeMessageMetrics {
    fn outlined_weight(&self, text: &[u8]) -> Option<i32> {
        let text = &text[..text
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(text.len())];
        Some(if text.is_empty() {
            0
        } else {
            1 + text.iter().filter(|byte| **byte == b'\n').count() as i32
        })
    }

    fn text_count(&self, text: &[u8]) -> Option<i32> {
        Some(
            text.iter()
                .position(|byte| *byte == 0)
                .unwrap_or(text.len()) as i32,
        )
    }
}

fn read_guest_text(api: &HostUiGuestApi, pointer: u32) -> Option<Vec<u8>> {
    const MAX_SOURCE_BYTES: u32 = 64 * 1024;
    let mut bytes = Vec::new();
    let mut address = pointer;
    while address.wrapping_sub(pointer) < MAX_SOURCE_BYTES {
        let aligned = address & !3;
        let word = api.read32_raw(aligned)?.to_le_bytes();
        let first = (address & 3) as usize;
        for byte in word.iter().skip(first) {
            if *byte == 0 {
                return Some(bytes);
            }
            bytes.push(*byte);
            if bytes.len() >= MAX_SOURCE_BYTES as usize {
                return None;
            }
        }
        address = aligned.checked_add(4)?;
    }
    None
}

fn native_message_admission(api: &HostUiGuestApi, va: u32, esp: u32) -> Option<u32> {
    if !matches!(va, 0x12d350 | 0x148a10) {
        return None;
    }
    let argument = |index: u32| api.read32_raw(esp.wrapping_add(4 + index * 4));
    let (Some(player), Some(kind), Some(text_pointer), Some(delay)) =
        (argument(0), argument(1), argument(2), argument(3))
    else {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_ARGUMENTS_UNREADABLE].fetch_add(1, Ordering::Relaxed);
        return None;
    };
    let player = player as i32;
    let delay = f32::from_bits(delay);
    let diagnostic_poll = if *MESSAGE_ADMISSION_DIAGNOSTICS
        .get_or_init(|| std::env::var_os("TSFP_HUD_MESSAGES_DIAGNOSTICS").is_some())
    {
        Some(unsafe { xinput_source_port_poll_count(0) })
    } else {
        None
    };
    if let Some(poll) = diagnostic_poll {
        // This opt-in event record correlates one native admission with the
        // owner trace's guest-poll key without exposing the borrowed text bytes.
        eprintln!(
            "tsfp-bevy: HUD message admission event poll={} va=0x{va:08X} player={player} kind={kind}",
            poll,
        );
    }
    let cue = if va == 0x148a10 {
        let Some(value) = argument(4) else {
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_ARGUMENTS_UNREADABLE]
                .fetch_add(1, Ordering::Relaxed);
            return None;
        };
        Some(value as i32)
    } else {
        None
    };
    if !(0..4).contains(&player) {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_REJECTED].fetch_add(1, Ordering::Relaxed);
        return Some(0);
    }
    if !crate::hud_live::guest_message_queue_empty(api, player as usize) {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_GUEST_QUEUE_ACTIVE].fetch_add(1, Ordering::Relaxed);
        return None;
    }
    if !crate::hud_live::import_guest_message_queue(api, player as usize) {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_IMPORT_FAILED].fetch_add(1, Ordering::Relaxed);
        return None;
    }
    if text_pointer == 0 {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_REJECTED].fetch_add(1, Ordering::Relaxed);
        return Some(0);
    }
    let Some(text) = read_guest_text(api, text_pointer) else {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_TEXT_UNREADABLE].fetch_add(1, Ordering::Relaxed);
        return None;
    };
    let preset = match GenericMessagePreset::load_original(
        kind,
        &text,
        &GuestPresetData(api),
        &NativeMessageMetrics,
    ) {
        Ok(preset) => preset,
        Err(tsfp_ui_model::hud_producers::MessagePresetError::InvalidKind) => {
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_REJECTED].fetch_add(1, Ordering::Relaxed);
            return Some(0);
        }
        Err(_) => {
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_PRESET_UNAVAILABLE]
                .fetch_add(1, Ordering::Relaxed);
            return None;
        }
    };
    let Some(result) =
        crate::hud_live::insert_owned_message(player, kind, &text, delay, cue, &preset)
    else {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_STATE_UNAVAILABLE].fetch_add(1, Ordering::Relaxed);
        return None;
    };
    if result != 0 {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_INSERTED].fetch_add(1, Ordering::Relaxed);
        if kind == 7 {
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_KIND_7_NATIVE].fetch_add(1, Ordering::Relaxed);
        }
    } else {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_REJECTED].fetch_add(1, Ordering::Relaxed);
    }
    if let Some(poll) = diagnostic_poll {
        eprintln!(
            "tsfp-bevy: HUD message admission result poll={poll} player={player} kind={kind} inserted={}",
            result != 0
        );
    }
    Some(result)
}

fn native_message_notice_inner(api: &HostUiGuestApi, va: u32, esp: u32) -> Option<bool> {
    let kind = match va {
        0x12d6b0 => tsfp_ui_model::hud_producers::NoticeKind::Plain,
        0x12d760 => tsfp_ui_model::hud_producers::NoticeKind::VariantC,
        _ => return None,
    };
    let argument = |index: u32| api.read32_raw(esp.wrapping_add(4 + index * 4));
    let player = argument(0)? as i32;
    if !(0..4).contains(&player) {
        return Some(true);
    }
    if !crate::hud_live::guest_message_queue_empty(api, player as usize) {
        return None;
    }
    if !crate::hud_live::import_guest_message_queue(api, player as usize) {
        return None;
    }
    let text_pointer = argument(1)?;
    if text_pointer == 0 {
        return Some(false);
    }
    let opaque = argument(2)?;
    let delay = f32::from_bits(argument(3)?);
    let text = read_guest_text(api, text_pointer)?;
    let tuning = tsfp_ui_model::messages::MessageTuning {
        fade_start: f32::from_bits(api.read32_raw(0x475d10)?),
        end: f32::from_bits(api.read32_raw(0x475d00)?),
        one: f32::from_bits(api.read32_raw(0x475c78)?),
    };
    Some(crate::hud_live::push_owned_notice(
        player, kind, &text, opaque, delay, &tuning,
    ))
}

fn native_message_notice(api: &HostUiGuestApi, va: u32, esp: u32) -> Option<bool> {
    let result = native_message_notice_inner(api, va, esp);
    if *MESSAGE_ADMISSION_DIAGNOSTICS
        .get_or_init(|| std::env::var_os("TSFP_HUD_MESSAGES_DIAGNOSTICS").is_some())
    {
        let poll = unsafe { xinput_source_port_poll_count(0) };
        let player = api.read32_raw(esp.wrapping_add(4)).map(|word| word as i32);
        eprintln!(
            "tsfp-bevy: HUD message notice poll={poll} va=0x{va:08X} player={player:?} result={result:?}"
        );
    }
    result
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_messages_native(va: u32, esp: u32, result: *mut u32) -> i32 {
    if !messages_native_enabled() {
        return 0;
    }
    MESSAGE_ADMISSION_OUTCOMES[ADMISSION_CALLBACK].fetch_add(1, Ordering::Relaxed);
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        MESSAGE_ADMISSION_OUTCOMES[ADMISSION_STATE_UNAVAILABLE].fetch_add(1, Ordering::Relaxed);
        return 0;
    };
    let Some(value) = native_message_admission(api, va, esp) else {
        return 0;
    };
    if result.is_null() {
        return 0;
    }
    // SAFETY: the generated C adapter passes a live pointer to its result local.
    unsafe { *result = value };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_messages_notice_native(va: u32, esp: u32) -> i32 {
    if !messages_native_enabled() {
        return 0;
    }
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return 0;
    };
    native_message_notice(api, va, esp).map_or(0, i32::from)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_messages_clear_native(va: u32, esp: u32) {
    if !messages_native_enabled() || va != 0x12d810 {
        return;
    }
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return;
    };
    let (Some(player), Some(kind)) = (
        api.read32_raw(esp.wrapping_add(4)).map(|word| word as i32),
        api.read32_raw(esp.wrapping_add(8)).map(|word| word as i32),
    ) else {
        return;
    };
    if !(0..4).contains(&player) {
        return;
    }
    if crate::hud_live::guest_message_queue_empty(api, player as usize)
        && crate::hud_live::import_guest_message_queue(api, player as usize)
    {
        crate::hud_live::clear_owned_messages(player, kind);
    }
}

/// Import the current guest ring before the retained kind-6 producer mutates
/// it. This keeps the Rust-owned queue aligned while the lifted call remains
/// the live ABI and cue oracle.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_kind6_before(esp: u32) {
    if !messages_native_enabled() {
        return;
    }
    KIND6_BEFORE_CALLS.fetch_add(1, Ordering::Relaxed);
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return;
    };
    let Some(player) = api
        .read32_raw(esp.wrapping_add(4))
        .and_then(|value| usize::try_from(value).ok())
        .filter(|player| *player < 4)
    else {
        return;
    };
    let _ = crate::hud_live::import_guest_message_queue_for_kind6(api, player);
}

/// Mirror only a confirmed lifted kind-6 ring write into Rust-owned state.
/// The retained producer still performs its eligibility checks and submits
/// the cue once; after `ret`, ESP points at argument 1, not the return slot.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_kind6_after(esp: u32, cursor_before: u32) {
    if !messages_native_enabled() {
        return;
    }
    KIND6_AFTER_CALLS.fetch_add(1, Ordering::Relaxed);
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        KIND6_REJECTED.fetch_add(1, Ordering::Relaxed);
        KIND6_REJECT_API.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let Some((player, source, age_seed)) = api
        .read32_raw(esp)
        .zip(api.read32_raw(esp.wrapping_add(4)))
        .zip(api.read32_raw(esp.wrapping_add(8)))
        .map(|((player, source), age)| (player as i32, source, f32::from_bits(age)))
    else {
        KIND6_REJECTED.fetch_add(1, Ordering::Relaxed);
        KIND6_REJECT_ARGS.fetch_add(1, Ordering::Relaxed);
        return;
    };
    if !(0..4).contains(&player) {
        KIND6_REJECTED.fetch_add(1, Ordering::Relaxed);
        KIND6_REJECT_PLAYER.fetch_add(1, Ordering::Relaxed);
        return;
    }
    if source == 0 {
        KIND6_REJECTED.fetch_add(1, Ordering::Relaxed);
        KIND6_REJECT_PLAYER.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let ring = 0x7a6160 + player as u32 * 0x177c;
    let expected_cursor = (cursor_before + 1) % 16;
    let written_slot = ring + 0x38 + cursor_before % 16 * 0x16c;
    if api.read32_raw(ring + 0x30) != Some(expected_cursor)
        || api.read32_raw(written_slot) != Some(6)
        || api.read32_raw(written_slot + 0x164) != Some(source)
    {
        KIND6_REJECTED.fetch_add(1, Ordering::Relaxed);
        KIND6_REJECT_RING.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let Some(tuning) = (|| {
        Some(tsfp_ui_model::hud_producers::Kind6MessageTuning {
            fade_start: f32::from_bits(api.read32_raw(0x475d18)?),
            end: f32::from_bits(api.read32_raw(0x4760cc)?),
            evict_after: f32::from_bits(api.read32_raw(0x47d5dc)?),
            cue_scale: api.read32_raw(0x4e8cec)? as i32 as f32,
        })
    })() else {
        KIND6_REJECTED.fetch_add(1, Ordering::Relaxed);
        KIND6_REJECT_TUNING.fetch_add(1, Ordering::Relaxed);
        return;
    };
    crate::hud_live::mirror_kind6_message(player, source, age_seed, tuning);
    KIND6_MIRRORED.fetch_add(1, Ordering::Relaxed);
}

fn predict(api: &HostUiGuestApi, va: u32) -> Option<Prediction> {
    let read = |at| api.read32_raw(at);
    let float = |at| read(at).map(f32::from_bits);
    let viewer = read(0x7b0c7c)?;
    let slot = read(viewer + 4)? as i32;
    if !(0..4).contains(&slot) {
        return Some(Prediction { va, words: vec![] });
    }
    let record = 0x7a6160 + slot as u32 * 0x177c;
    let timing = HudTiming {
        rate_hz: read(0x4e8cec)? as i32,
        scale: float(0x4e794c)?,
        frames: float(0x4e7948)?,
    };
    let full = float(0x475c78)?;
    let mut words = vec![];
    match va {
        0x141340 => {
            let mut state = WeaponAnimation {
                phase: read(record + 0x1750)?,
                flash: float(record + 0x1754)?,
                alpha: float(record + 0x1758)?,
                slide: float(record + 0x175c)?,
                target: float(record + 0x1760)?,
                cycle: float(record + 0x1764)?,
            };
            let tuning = WeaponAnimationTuning {
                full,
                fade_rate: float(0x475db4)?,
                cycle_rate: float(0x475d10)?,
                slide_epsilon: float(0x475d4c)?,
                slide_max_step: float(0x475d40)?,
                slide_gain: float(0x47862c)?,
            };
            state.advance(timing, tuning, read(0x7de30c)? == 8);
            words.push((record + 0x1750, state.phase));
            for (off, value) in [
                (0x1754, state.flash),
                (0x1758, state.alpha),
                (0x175c, state.slide),
                (0x1760, state.target),
                (0x1764, state.cycle),
            ] {
                words.push((record + off, value.to_bits()));
            }
        }
        0x14a5a0 => {
            let cursor = read(record + 0x30)? as i32;
            if cursor < 0 {
                return None;
            }
            let mut queue = MessageAges {
                cursor: cursor as usize % 16,
                ..Default::default()
            };
            for (i, e) in queue.entries.iter_mut().enumerate() {
                let at = record + 0x38 + i as u32 * 0x16c;
                let kind = read(at)? as usize;
                if kind >= 14 {
                    return None;
                }
                *e = MessageAge {
                    kind,
                    age: float(at + 0x144)?,
                    end: float(at + 0x150)?,
                    evict_after: float(at + 0x154)?,
                    weight: read(at + 0x158)? as i32,
                    cue: read(at + 0x15c)? as i32,
                    playing_handle: read(at + 0x160)? as i32,
                };
            }
            let mut policy = MessagePolicy {
                limits: [0; 14],
                conflicts: vec![],
            };
            for (i, limit) in policy.limits.iter_mut().enumerate() {
                *limit = read(0x7490ac + i as u32 * 4)? as i32;
            }
            for at in (0x4e72e0..0x4e7348).step_by(8) {
                policy
                    .conflicts
                    .push((read(at)? as usize, read(at + 4)? as usize));
            }
            let mut audio = Sounds(api, true);
            queue.advance(
                (full / timing.rate_hz as f32) * timing.scale,
                read(0x4e7944)? as i32,
                &policy,
                &mut audio,
            );
            if !audio.1 {
                return None;
            }
            for (i, e) in queue.entries.iter().enumerate() {
                let at = record + 0x38 + i as u32 * 0x16c;
                words.extend([
                    (at, e.kind as u32),
                    (at + 0x144, e.age.to_bits()),
                    (at + 0x15c, e.cue as u32),
                ]);
            }
        }
        _ => return None,
    }
    Some(Prediction { va, words })
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_producer_before(va: u32) {
    if !enabled() {
        return;
    }
    // SAFETY: C owns this static API for the whole host lifetime; guest-thread callbacks only.
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return;
    };
    let prediction = predict(api, va);
    if prediction.is_none() {
        UNREADABLE.fetch_add(1, Ordering::Relaxed);
    }
    if let Ok(mut expected) = EXPECTED.lock() {
        *expected = prediction;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_producer_after(va: u32) {
    if va == 0x141340 {
        if let Some(api) = unsafe { API.load(Ordering::Acquire).as_ref() } {
            trace_weapon(api);
        }
    }
    if !enabled() {
        return;
    }
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return;
    };
    let Some(prediction) = EXPECTED.lock().ok().and_then(|mut e| e.take()) else {
        return;
    };
    if prediction.va != va {
        UNREADABLE.fetch_add(1, Ordering::Relaxed);
        return;
    }
    if prediction.words.is_empty() {
        return;
    }
    match va {
        0x141340 => {
            WEAPON_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        0x14a5a0 => {
            QUEUE_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        _ => return,
    }
    if let Some((at, want)) = prediction
        .words
        .iter()
        .find(|(at, want)| api.read32_raw(*at) != Some(*want))
    {
        let count = MISMATCHES.fetch_add(1, Ordering::Relaxed);
        if count < 8 {
            eprintln!(
                "tsfp-bevy: hud producer shadow DIVERGE va={va:#x} field={at:#x} expected={want:#x} observed={:?}",
                api.read32_raw(*at)
            );
        }
    }
}
pub fn report() {
    eprintln!(
        "tsfp-bevy: HUD row trace samples={} errors={} owned_samples={}",
        ROW_SAMPLES.load(Ordering::Relaxed),
        ROW_ERRORS.load(Ordering::Relaxed),
        ROW_OWNED_SAMPLES.load(Ordering::Relaxed)
    );
    if STATE_TRACE.get().is_some() && std::env::var_os("TSFP_HUD_STATE_TRACE").is_some() {
        eprintln!(
            "tsfp-bevy: HUD state trace samples={} errors={} owned_samples={}",
            STATE_SAMPLES.load(Ordering::Relaxed),
            STATE_ERRORS.load(Ordering::Relaxed),
            STATE_OWNED_SAMPLES.load(Ordering::Relaxed)
        );
    }
    if enabled() {
        eprintln!(
            "tsfp-bevy: hud producer shadow weapon_calls={} queue_calls={} mismatches={} no_verdict={}",
            WEAPON_CALLS.load(Ordering::Relaxed),
            QUEUE_CALLS.load(Ordering::Relaxed),
            MISMATCHES.load(Ordering::Relaxed),
            UNREADABLE.load(Ordering::Relaxed)
        );
    }
    if messages_native_enabled() {
        eprintln!(
            "tsfp-bevy: HUD message native lifecycle ticks={} audio_errors={}",
            MESSAGE_NATIVE_TICKS.load(Ordering::Relaxed),
            MESSAGE_NATIVE_ERRORS.load(Ordering::Relaxed)
        );
        eprintln!(
            "tsfp-bevy: HUD kind6 hook before={} after={} mirrored={} rejected={} api={} args={} player={} ring={} tuning={}",
            KIND6_BEFORE_CALLS.load(Ordering::Relaxed),
            KIND6_AFTER_CALLS.load(Ordering::Relaxed),
            KIND6_MIRRORED.load(Ordering::Relaxed),
            KIND6_REJECTED.load(Ordering::Relaxed),
            KIND6_REJECT_API.load(Ordering::Relaxed),
            KIND6_REJECT_ARGS.load(Ordering::Relaxed),
            KIND6_REJECT_PLAYER.load(Ordering::Relaxed),
            KIND6_REJECT_RING.load(Ordering::Relaxed),
            KIND6_REJECT_TUNING.load(Ordering::Relaxed)
        );
        eprintln!(
            "tsfp-bevy: HUD message admissions callbacks={} inserted={} rejected={} args_unreadable={} native_kind7={} deferred_guest_active={} import_failed={} text_unreadable={} preset_unavailable={} state_unavailable={}",
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_CALLBACK].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_INSERTED].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_REJECTED].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_ARGUMENTS_UNREADABLE].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_KIND_7_NATIVE].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_GUEST_QUEUE_ACTIVE].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_IMPORT_FAILED].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_TEXT_UNREADABLE].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_PRESET_UNAVAILABLE].load(Ordering::Relaxed),
            MESSAGE_ADMISSION_OUTCOMES[ADMISSION_STATE_UNAVAILABLE].load(Ordering::Relaxed)
        );
    }
}
