// SPDX-License-Identifier: GPL-3.0-or-later
//! T2312 UI-14: the native Rust HUD in the Bevy shell.
//!
//! * Guest thread, per legacy drawer call: the generated C gate (tools/play/hud_gate.py) asks
//!   [`tsfp_hud_native_owns`]. The call is the original's draw request for that element; the answer
//!   is the owner's per-element switch (`TSFP_HUD_NATIVE`, see `HudOwnership::parse`). Owned: the
//!   legacy body is skipped and the native HUD draws the element.
//! * Guest thread, per frame (vblank, `tsfp_ui_frame_service`): [`service`] reads the typed
//!   `HudFrame` through the host's guest API and publishes it with the frame's draw requests.
//! * Engine frame (render thread of the game's wgpu renderer): the [`tsfp_ui_wgpu::hud::HudLayer`]
//!   (frame layer `hud`, order 0, under the Rust UI page) lays the frame out
//!   (`hud_native::layout`, original preset in the 4:3 picture, or the safe area with
//!   `TSFP_HUD_ANCHOR=safe`) and records it into the frame's own encoder after scene and post (T2309).
//!   Without the wgpu renderer (the `plain` variant) no layer exists and the legacy HUD keeps every
//!   element: the gate only skips a drawer while the layer is installed.
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use tsfp_ui_model::font::{FontTableMeasure, StringSource};
use tsfp_ui_model::hud_effects::{self, EffectIcons};
use tsfp_ui_model::hud_messages::{self, GuestOp, OutlinedCall, QueueCall, QueueDraw};
use tsfp_ui_model::hud_native::{
    self, AmmoCall, AmmoRow, DrawRequest, GuestCall, GuestRead, HitSectors, HudAnchor, HudElement,
    HudFrame, HudLayout, HudOwnership, HudSpace, Reticle, ReticleCall, VitalsArcs, VitalsCall,
};
use tsfp_ui_model::hud_prompts::{self, PromptCall, PromptPanels};
use tsfp_ui_model::hud_radar::{self, RadarCall, RadarState};
use tsfp_ui_model::hud_rank::{self, RankBox, RankCall, TextWidth};
use tsfp_ui_model::hud_weapons::{self, PrimitiveCall, SlideTargets, WeaponCall, WeaponPanel};
use tsfp_ui_model::model::TextId;
use tsfp_ui_wgpu::hud::{HudAssets, HudLayer, HudTextures};

use crate::ui_live::HostUiGuestApi;

unsafe extern "C" {
    fn tsfp_render_trace_view_layer_boundary();
}

/// Owner's switch, read once at start (`TSFP_HUD_NATIVE`).
static OWNERSHIP: OnceLock<HudOwnership> = OnceLock::new();
static ANCHOR: OnceLock<HudAnchor> = OnceLock::new();
static ASSETS: OnceLock<HudAssets> = OnceLock::new();
/// Draw requests of the frame in progress (guest thread).
static PENDING: Mutex<Vec<DrawRequest>> = Mutex::new(Vec::new());
/// Last published frame: state, requests and a generation counter.
static PUBLISHED: Mutex<Option<(HudFrame, Vec<DrawRequest>)>> = Mutex::new(None);
/// T2329: game fact publications do not replace owned HUD lifecycle state.
static HUD_STATE: OnceLock<Mutex<hud_native::HudState>> = OnceLock::new();
static HUD_ROWS: OnceLock<Mutex<tsfp_ui_model::hud_rows::HudRows>> = OnceLock::new();
static OWNED_PRODUCERS: OnceLock<bool> = OnceLock::new();
static WEAPON_SEEDED: AtomicBool = AtomicBool::new(false);
static WEAPON_EVENTS_SEEN: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
static WEAPON_EVENTS_NATIVE: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];

/// Publish one player's native held buttons intersected with the measured
/// guest-owned eligibility mask. Called on the guest thread by the message
/// producer bridge, so the mask read and typed action publication share a tick.
pub(crate) fn publish_message_buttons(player: i32, current: u32, eligible_mask: u32) -> bool {
    let Some(mut state) = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
        .ok()
    else {
        return false;
    };
    state.publish_message_buttons(player, current, eligible_mask)
}

pub(crate) fn owned_producers() -> bool {
    *OWNED_PRODUCERS.get_or_init(|| std::env::var("TSFP_HUD_PRODUCERS").map_or(true, |v| v == "1"))
        && !ownership().is_empty()
        && IN_FRAME.load(Ordering::Acquire)
        && WEAPON_SEEDED.load(Ordering::Acquire)
}

pub(crate) fn frame_number() -> u32 {
    FRAMES.load(Ordering::Relaxed)
}

unsafe extern "C" {
    fn tsfp_ports_hud_metric_install(provider: Option<extern "C" fn(u32, u32, *mut u32) -> i32>);
    fn tsfp_ports_hud_rows_install(
        provider: Option<extern "C" fn(u32, u32, u32, u32, *mut u32) -> i32>,
    );
}

/// Typed per-player row contract. kind = left/right ammo/status; operation =
/// peek/set/advance/begin. No guest pointer or guest-memory write is retained.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_row_metric(
    slot: u32,
    kind: u32,
    operation: u32,
    value: u32,
    output: *mut u32,
) -> i32 {
    if !owned_producers() || slot >= 4 || kind > 2 || output.is_null() {
        return 0;
    }
    let Ok(mut rows) = HUD_ROWS
        .get_or_init(|| Mutex::new(Default::default()))
        .lock()
    else {
        return 0;
    };
    let player = slot as usize;
    let result = if kind < 2 {
        let side = kind as usize;
        match operation {
            0 => rows.ammo_index(player, side).map(u32::from),
            1 | 3 => {
                rows.set_ammo(player, side, value as u8);
                rows.ammo_index(player, side).map(u32::from)
            }
            2 => {
                let index = rows
                    .ammo_index(player, side)
                    .unwrap()
                    .wrapping_add(value as u8);
                rows.set_ammo(player, side, index);
                Some(u32::from(index))
            }
            _ => None,
        }
    } else {
        match operation {
            0 => rows.status_top(player).map(|n| n as u32),
            1 => {
                rows.set_status(player, value as i32);
                Some(value)
            }
            2 => rows.advance_status(player, value as i32).map(|n| n as u32),
            3 => rows.begin_status(player, value as i32).map(|n| n as u32),
            _ => None,
        }
    };
    let Some(value) = result else { return 0 };
    // SAFETY: callers supply a live local output word at this ABI boundary.
    unsafe { *output = value };
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_rows_begin(slot: u32) {
    if !owned_producers() {
        return;
    }
    if let Ok(mut rows) = HUD_ROWS
        .get_or_init(|| Mutex::new(Default::default()))
        .lock()
    {
        rows.begin(slot as usize);
    }
}

fn live_slot(api: &HostUiGuestApi) -> Option<u32> {
    api.read32_raw(0x7b0c7c)
        .and_then(|p| api.read32_raw(p.wrapping_add(4)))
        .filter(|slot| *slot < 4)
}

fn column_top(api: &HostUiGuestApi, cursor: u32) -> Option<i32> {
    let slot = live_slot(api)?;
    let mut top = 0;
    if tsfp_hud_row_metric(slot, 2, 0, 0, &mut top) != 0 {
        return Some(top as i32);
    }
    let origin = api.read32_raw(cursor)?;
    (tsfp_hud_row_metric(slot, 2, 3, origin, &mut top) != 0).then_some(top as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_column_advance(height: u32) -> i32 {
    if !owned_producers() {
        return 0;
    }
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return 0;
    };
    let Some(slot) = live_slot(api) else { return 0 };
    let mut top = 0;
    tsfp_hud_row_metric(slot, 2, 2, height, &mut top)
}

/// Direct native metric contract for retained hybrid callers. The field IDs
/// are phase/flash/alpha/slide/target/cycle, not guest offsets.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_weapon_metric(slot: u32, metric: u32, output: *mut u32) -> i32 {
    if !owned_producers() || slot >= 4 || output.is_null() {
        return 0;
    }
    let Some(state) = HUD_STATE.get().and_then(|s| s.lock().ok()) else {
        return 0;
    };
    let animation = state.weapons[slot as usize];
    let value = match metric {
        0 => animation.phase,
        1 => animation.flash.to_bits(),
        2 => animation.alpha.to_bits(),
        3 => animation.slide.to_bits(),
        4 => animation.target.to_bits(),
        5 => animation.cycle.to_bits(),
        _ => return 0,
    };
    // SAFETY: the hybrid boundary supplies a live local output word.
    unsafe { *output = value };
    1
}

/// Read-only T3397 projection of one Rust-owned message queue for the host
/// trace oracle. The address is a reserved trace window; this does not write
/// or mirror guest memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_hud_message_trace_snapshot(
    slot: u32,
    output: *mut u32,
    output_words: u32,
) -> bool {
    if slot >= 4 || output.is_null() || output_words < hud_native::HUD_MESSAGE_TRACE_WORDS as u32 {
        return false;
    }
    if !crate::hud_producer_live::hydrate_messages_for_trace(slot as usize) {
        return false;
    }
    let Some(state) = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
        .ok()
    else {
        return false;
    };
    // SAFETY: the C host supplies a stack buffer with the checked word count.
    let output =
        unsafe { core::slice::from_raw_parts_mut(output, hud_native::HUD_MESSAGE_TRACE_WORDS) };
    state.encode_message_trace(slot as usize, output)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_weapon_target(slot: u32, target: u32) -> i32 {
    if !owned_producers() || slot >= 4 {
        return 0;
    }
    let Some(mut state) = HUD_STATE.get().and_then(|s| s.lock().ok()) else {
        return 0;
    };
    state.weapons[slot as usize].set_slide_target(f32::from_bits(target));
    1
}

fn seed_weapon_state(api: &HostUiGuestApi) {
    if WEAPON_SEEDED.load(Ordering::Acquire) {
        return;
    }
    let mut initial = [tsfp_ui_model::hud_producers::WeaponAnimation::default(); 4];
    for (slot, state) in initial.iter_mut().enumerate() {
        let record = 0x7a6160 + slot as u32 * 0x177c;
        let Some(words) = (0..6)
            .map(|i| api.read32_raw(record + 0x1750 + i * 4))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };
        *state = tsfp_ui_model::hud_producers::WeaponAnimation {
            phase: words[0],
            flash: f32::from_bits(words[1]),
            alpha: f32::from_bits(words[2]),
            slide: f32::from_bits(words[3]),
            target: f32::from_bits(words[4]),
            cycle: f32::from_bits(words[5]),
        };
    }
    if let Ok(mut state) = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
    {
        state.weapons = initial;
        WEAPON_SEEDED.store(true, Ordering::Release);
        // SAFETY: this provider is a process-lifetime function, called on the guest thread.
        unsafe { tsfp_ports_hud_metric_install(Some(tsfp_hud_weapon_metric)) };
        unsafe { tsfp_ports_hud_rows_install(Some(tsfp_hud_row_metric)) };
    }
}

pub(crate) fn advance_owned_weapon(api: &HostUiGuestApi) -> bool {
    use tsfp_ui_model::hud_producers::{HudTiming, WeaponAnimationTuning};
    if !owned_producers() {
        return false;
    }
    let read = |at| api.read32_raw(at);
    let float = |at| read(at).map(f32::from_bits);
    let advance = || -> Option<()> {
        let slot = read(read(0x7b0c7c)? + 4)? as i32;
        if !(0..4).contains(&slot) {
            return Some(());
        }
        let timing = HudTiming {
            rate_hz: read(0x4e8cec)? as i32,
            scale: float(0x4e794c)?,
            frames: float(0x4e7948)?,
        };
        let tuning = WeaponAnimationTuning {
            full: float(0x475c78)?,
            fade_rate: float(0x475db4)?,
            cycle_rate: float(0x475d10)?,
            slide_epsilon: float(0x475d4c)?,
            slide_max_step: float(0x475d40)?,
            slide_gain: float(0x47862c)?,
        };
        let ending = read(0x7de30c)? == 8;
        let mut state = HUD_STATE.get()?.lock().ok()?;
        state.weapons[slot as usize].advance(timing, tuning, ending);
        Some(())
    };
    advance().is_some()
}

/// Selection/cycle/hide handlers: typed state is updated directly. Register
/// results are supplied separately to the retained ABI adapter.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_weapon_event_at(va: u32, slot: u32, previous: *mut u32) -> i32 {
    use tsfp_ui_model::hud_producers::WeaponPanelEvent;
    let index = match va {
        0x142820 => 0,
        0x142880 => 1,
        0x142900 => 2,
        0x142990 => 3,
        0x1427d0 => 4,
        _ => return 0,
    };
    WEAPON_EVENTS_SEEN[index].fetch_add(1, Ordering::Relaxed);
    if !owned_producers() || previous.is_null() {
        return 0;
    }
    let event = match va {
        0x1427d0 => WeaponPanelEvent::Show,
        0x142820 => WeaponPanelEvent::Selected,
        0x142880 => WeaponPanelEvent::NextInGroup,
        0x142900 => WeaponPanelEvent::PreviousInGroup,
        0x142990 => WeaponPanelEvent::Hide,
        _ => return 0,
    };
    // SAFETY: host owns its static API for the process lifetime.
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return 0;
    };
    if slot >= 4 {
        return 0;
    }
    let Some(full) = api.read32_raw(0x475c78).map(f32::from_bits) else {
        return 0;
    };
    let Some(mut state) = HUD_STATE.get().and_then(|s| s.lock().ok()) else {
        return 0;
    };
    // SAFETY: the adapter supplies a valid local for the previous phase.
    unsafe { *previous = state.weapons[slot as usize].phase };
    state.weapon_event(slot as usize, event, full);
    let count = WEAPON_EVENTS_NATIVE[index].fetch_add(1, Ordering::Relaxed);
    if count < 4 {
        eprintln!(
            "tsfp-bevy: native weapon event va={va:#x} slot={slot} phase={}",
            state.weapons[slot as usize].phase
        );
    }
    1
}
/// The HUD layer is in the engine frame (wgpu renderer chosen); before that the legacy HUD draws all.
static IN_FRAME: AtomicBool = AtomicBool::new(false);
/// Evidence counters: legacy drawer calls skipped and passed through.
static SKIPPED: AtomicU64 = AtomicU64::new(0);
static PASSED: AtomicU64 = AtomicU64::new(0);
static FRAMES: AtomicU32 = AtomicU32::new(0);
/// The C host's static guest API table (set by the first frame service); the gate reads through it.
static API: AtomicPtr<HostUiGuestApi> = AtomicPtr::new(std::ptr::null_mut());
pub(crate) fn guest_read32(va: u32) -> Option<u32> {
    // T3819
    unsafe { API.load(Ordering::Acquire).as_ref() }?.read32_raw(va)
}
/// Crosshair calls the native HUD left to legacy (scope bar, recolouring modes).
static RETICLE_LEGACY: AtomicU64 = AtomicU64::new(0);
/// Vitals calls left to the original (a hit-sector indicator fading).
static VITALS_LEGACY: AtomicU64 = AtomicU64::new(0);
/// Rust-owned hit-sector state (the guest entries at 0x7A5DA0 stay 0 while the setter is gated).
static HIT_SECTORS: Mutex<HitSectors> = Mutex::new(HitSectors {
    angle: [0.0; 4],
    timer: [0.0; 4],
});
/// Hits taken by the Rust state and hit sectors drawn natively (evidence).
static HITS: AtomicU64 = AtomicU64::new(0);
static SECTORS_DRAWN: AtomicU64 = AtomicU64::new(0);
/// Rank box calls drawn natively and left to the original (recolour mode, no fonts or textures).
static RANKS_DRAWN: AtomicU64 = AtomicU64::new(0);
static RANK_LEGACY: AtomicU64 = AtomicU64::new(0);
/// Rust-owned radar state (sweep angle, pulse restart) and evidence counters.
static RADAR: Mutex<RadarState> = Mutex::new(RadarState {
    sweep: [None; 4],
    pulse_base: [0.0; 4],
    blips: Vec::new(),
});
/// Shadow check (`TSFP_HUD_RADAR_SHADOW=1`, diagnostic): the original keeps drawing the radar; each call compares
/// the original's previous results with the Rust replay of the same call. Calls checked, equal, worst errors.
static RADAR_SHADOW: OnceLock<bool> = OnceLock::new();
static RADAR_SHADOW_STATS: Mutex<(u64, u64, f32, f32, f32)> = Mutex::new((0, 0, 0.0, 0.0, 0.0));
static RADARS_DRAWN: AtomicU64 = AtomicU64::new(0);
/// Rust-owned prompt panel lifecycle (increment 6) and evidence counters; shadow check `TSFP_HUD_PROMPTS_SHADOW=1`
/// (the original keeps drawing; calls compared, heights equal, phases equal, worst alpha and timer errors).
static PROMPTS: Mutex<PromptPanels> = Mutex::new(PromptPanels { panels: [None; 4] });
static PROMPTS_DRAWN: AtomicU64 = AtomicU64::new(0);
static PROMPT_LEGACY: AtomicU64 = AtomicU64::new(0);
static PROMPT_SHADOW: OnceLock<bool> = OnceLock::new();
static PROMPT_SHADOW_STATS: Mutex<(u64, u64, u64, f32, f32)> = Mutex::new((0, 0, 0, 0.0, 0.0));
/// Effect icon rows drawn natively and left to the original (increment 6).
static EFFECTS_DRAWN: AtomicU64 = AtomicU64::new(0);
static EFFECT_LEGACY: AtomicU64 = AtomicU64::new(0);
static RADAR_LEGACY: AtomicU64 = AtomicU64::new(0);
/// Legacy drawer calls per drawer (index in the gate table order of `HudElement::ALL` drawers).
static CALLS: [AtomicU64; 64] = [const { AtomicU64::new(0) }; 64];
/// Census calls whose drawer would draw (pop-up ring entries alive, button prompt panel active).
static CENSUS_ACTIVE: [AtomicU64; 64] = [const { AtomicU64::new(0) }; 64];
/// Message queue census (`0x14AC80`, increment 7): draw-eligible entries per kind (age >= 0) over all calls.
static QUEUE_KINDS: [AtomicU64; 16] = [const { AtomicU64::new(0) }; 16];
/// Message queue (increment 7): calls replayed, entries drawn natively, entries drawn by their original drawer,
/// calls left to the original; shadow check `TSFP_HUD_MESSAGES_SHADOW=1` (the original keeps drawing, every
/// `0x41E30` call it makes after a replayed queue call is compared with the native one).
static QUEUES_DRAWN: AtomicU64 = AtomicU64::new(0);
static MESSAGES_NATIVE: AtomicU64 = AtomicU64::new(0);
static MESSAGES_TO_ORIGINAL: AtomicU64 = AtomicU64::new(0);
static MESSAGE_LEGACY: AtomicU64 = AtomicU64::new(0);
static MESSAGE_SHADOW: OnceLock<bool> = OnceLock::new();
/// Expected `0x41E30` calls of the last shadowed queue call, and the statistics: strings compared, equal, missing
/// (expected but not drawn), queue calls not checked (kinds drawn by their original drawer), worst position error.
static MESSAGE_EXPECTED: Mutex<Vec<OutlinedCall>> = Mutex::new(Vec::new());
static MESSAGE_SHADOW_STATS: Mutex<(u64, u64, u64, u64, f32)> = Mutex::new((0, 0, 0, 0, 0.0));
/// Weapon slot panel (increment 8): calls drawn natively, calls left to the original, calls with something visible
/// (panel alpha above 0), the mode 1 slide targets (Rust state, stored by the gate for the guest's fade updater).
static WEAPONS_DRAWN: AtomicU64 = AtomicU64::new(0);
static WEAPON_LEGACY: AtomicU64 = AtomicU64::new(0);
static WEAPONS_VISIBLE: AtomicU64 = AtomicU64::new(0);
static SLIDE_TARGETS: Mutex<SlideTargets> = Mutex::new(SlideTargets { target: [None; 4] });
/// Shadow check (`TSFP_HUD_WEAPONS_SHADOW=1`): the original keeps drawing; every primitive it draws after a replayed
/// panel call (`0x12CC70`, `0x561C0`, `0x81B70`) is compared word by word with the Rust list. Statistics: primitives
/// compared, equal, expected but not drawn, panel calls checked, worst float error.
static WEAPON_SHADOW: OnceLock<bool> = OnceLock::new();
static WEAPON_EXPECTED: Mutex<Vec<PrimitiveCall>> = Mutex::new(Vec::new());
static WEAPON_SHADOW_STATS: Mutex<(u64, u64, u64, u64, f32)> = Mutex::new((0, 0, 0, 0, 0.0));

/// Shadow observer of the weapon panel's primitives.
fn weapon_shadow_observe(api: &HostUiGuestApi, va: u32, esp: u32) {
    let (Ok(mut expected), Ok(mut stats)) = (WEAPON_EXPECTED.lock(), WEAPON_SHADOW_STATS.lock())
    else {
        return;
    };
    if expected.is_empty() {
        return;
    }
    let Some(arity) = hud_weapons::primitive_arity(va) else {
        return;
    };
    let observed: Vec<u32> = (0..arity as u32)
        .map(|i| api.read32_raw(esp.wrapping_add(4 + 4 * i)).unwrap_or(0))
        .collect();
    let want = expected.remove(0);
    stats.0 += 1;
    let equal = want.function == va && want.args == observed;
    stats.1 += u64::from(equal);
    if want.function == va {
        for (a, b) in want.args.iter().zip(&observed) {
            let (fa, fb) = (f32::from_bits(*a), f32::from_bits(*b));
            if fa.is_finite() && fb.is_finite() && fa.abs() < 1e6 && fb.abs() < 1e6 {
                stats.4 = stats.4.max((fa - fb).abs());
            }
        }
    }
    if !equal && stats.0 - stats.1 < 12 {
        eprintln!(
            "tsfp-bevy: hud: weapon shadow mismatch frame {}: want {:#x} {:?} got {va:#x} {:?}",
            FRAMES.load(Ordering::Relaxed),
            want.function,
            want.args
                .iter()
                .map(|w| format!("{w:#x}/{}", f32::from_bits(*w)))
                .collect::<Vec<_>>(),
            observed
                .iter()
                .map(|w| format!("{w:#x}/{}", f32::from_bits(*w)))
                .collect::<Vec<_>>()
        );
    }
}

/// The weapon slot panel's gates (increment 8). `Some(answer)` ends the gate; `None` continues with `weapons` set.
fn weapon_gate(
    api: &HostUiGuestApi,
    va: u32,
    esp: u32,
    owned: HudOwnership,
    weapons: &mut Option<hud_weapons::WeaponPanelDraw>,
) -> Option<i32> {
    let shadow = *WEAPON_SHADOW
        .get_or_init(|| std::env::var("TSFP_HUD_WEAPONS_SHADOW").is_ok_and(|v| v == "1"));
    if va != hud_weapons::weapon_addr::FN_PANEL {
        if shadow {
            weapon_shadow_observe(api, va, esp);
        }
        return Some(0);
    }
    if !owned.owns(HudElement::Weapons) {
        PASSED.fetch_add(1, Ordering::Relaxed);
        return Some(0);
    }
    if !owned_producers() && !shadow {
        return Some(0);
    }
    let call = if owned_producers() {
        let slot = api
            .read32_raw(hud_weapons::weapon_addr::VIEWER)
            .and_then(|v| api.read32_raw(v + 4));
        let animation = slot.filter(|s| *s < 4).and_then(|slot| {
            HUD_STATE
                .get()?
                .lock()
                .ok()
                .map(|s| s.weapons[slot as usize])
        });
        match animation {
            Some(animation) => WeaponPanel::read_owned(&Guest(api), &animation),
            None => WeaponCall::Nothing,
        }
    } else {
        WeaponPanel::read(&Guest(api))
    };
    if shadow {
        if let (WeaponCall::Draw(draw), Ok(mut expected), Ok(mut stats)) =
            (&call, WEAPON_EXPECTED.lock(), WEAPON_SHADOW_STATS.lock())
        {
            stats.2 += expected.len() as u64;
            stats.3 += 1;
            expected.clear();
            expected.extend(draw.primitive_calls());
        }
        PASSED.fetch_add(1, Ordering::Relaxed);
        return Some(0);
    }
    match call {
        WeaponCall::Draw(draw) => {
            let n = WEAPONS_DRAWN.fetch_add(1, Ordering::Relaxed);
            let visible = draw.panel.as_ref().is_some_and(|p| p.alpha_byte > 0)
                || draw.slots.iter().any(|s| s.shadow_colour & 0xff > 0);
            if visible {
                let v = WEAPONS_VISIBLE.fetch_add(1, Ordering::Relaxed);
                if v % 30 == 0 && v < 600 {
                    eprintln!(
                        "tsfp-bevy: hud: weapon panel frame {} mode {} slide {} panel {:?} arrow {:?} slots {:?}",
                        FRAMES.load(Ordering::Relaxed),
                        draw.mode,
                        draw.slide,
                        draw.panel.as_ref().map(|p| p.call),
                        draw.arrow_texture,
                        draw.slots
                            .iter()
                            .map(|s| (
                                s.weapon,
                                s.icon,
                                s.rect,
                                format!("{:#x}/{:#x}", s.shadow_colour, s.overlay_colour),
                                s.arrow.is_some()
                            ))
                            .collect::<Vec<_>>()
                    );
                }
            } else if n % 1800 == 0 {
                eprintln!(
                    "tsfp-bevy: hud: weapon panel frame {} mode {} hidden ({} icons)",
                    FRAMES.load(Ordering::Relaxed),
                    draw.mode,
                    draw.slots.len()
                );
            }
            // Mode 1 publishes its target directly to the owned animation updater.
            let slot = api
                .read32_raw(hud_weapons::weapon_addr::VIEWER)
                .and_then(|v| api.read32_raw(v + 4))
                .unwrap_or(0) as usize;
            if let Some(target) = SLIDE_TARGETS
                .lock()
                .ok()
                .and_then(|mut t| t.update(slot, &draw))
            {
                if owned_producers() {
                    if let Some(mut state) = HUD_STATE.get().and_then(|s| s.lock().ok()) {
                        if let Some(animation) = state.weapons.get_mut(slot) {
                            animation.set_slide_target(target);
                        }
                    }
                }
            }
            // The render state the original leaves set: state group 0, blend 1.
            let _ = api.call_raw(hud_weapons::weapon_addr::FN_STATE_GROUP, &[0]);
            let _ = api.call_raw(hud_weapons::weapon_addr::FN_BLEND, &[1]);
            *weapons = Some(draw);
            None
        }
        WeaponCall::Nothing => None,
        WeaponCall::Legacy(why) => {
            if WEAPON_LEGACY.fetch_add(1, Ordering::Relaxed) < 5 {
                eprintln!(
                    "tsfp-bevy: hud: weapon panel left to the original ({why}) frame {}",
                    FRAMES.load(Ordering::Relaxed)
                );
            }
            PASSED.fetch_add(1, Ordering::Relaxed);
            Some(0)
        }
    }
}

/// The census of the queue's draw-eligible entries per kind (evidence for the exit report).
fn queue_census(api: &HostUiGuestApi) -> bool {
    let slot = api
        .read32_raw(0x007b_0c7c)
        .and_then(|v| api.read32_raw(v.wrapping_add(4)))
        .unwrap_or(u32::MAX) as i32;
    let mut any = false;
    if (0..4).contains(&slot) {
        let record = 0x007a_6160 + slot as u32 * 0x177c;
        for i in 0..16u32 {
            let entry = record + 0x38 + i * 0x16c;
            let kind = api.read32_raw(entry).unwrap_or(0);
            let age = f32::from_bits(api.read32_raw(entry + 0x144).unwrap_or(0));
            if kind != 0 && age >= 0.0 {
                any = true;
                let n = QUEUE_KINDS[(kind as usize).min(15)].fetch_add(1, Ordering::Relaxed);
                if n % 120 == 0 && n < 1200 {
                    eprintln!(
                        "tsfp-bevy: hud: queue entry kind {kind} slot {slot} index {i} age {age:.2} at frame {}",
                        FRAMES.load(Ordering::Relaxed)
                    );
                }
            }
        }
    }
    any
}

/// The bytes of the guest string at `va` (up to the message text size).
fn guest_bytes(api: &HostUiGuestApi, va: u32) -> Vec<u8> {
    (0..0x140u32)
        .map_while(|i| {
            let at = va.wrapping_add(i);
            let byte = (api.read32_raw(at & !3).unwrap_or(0) >> ((at & 3) * 8)) as u8;
            (byte != 0).then_some(byte)
        })
        .collect()
}

/// Shadow observer of `0x41E30`: the original's string draws after a replayed queue call, compared in order.
fn message_shadow_observe(api: &HostUiGuestApi, esp: u32) {
    let (Ok(mut expected), Ok(mut stats)) = (MESSAGE_EXPECTED.lock(), MESSAGE_SHADOW_STATS.lock())
    else {
        return;
    };
    if expected.is_empty() {
        return;
    }
    let word = |offset: u32| api.read32_raw(esp.wrapping_add(offset)).unwrap_or(0);
    let observed = OutlinedCall {
        x: f32::from_bits(word(4)),
        y: f32::from_bits(word(8)),
        text: guest_bytes(api, word(0xc)),
        colour: word(0x10),
        outline: word(0x14),
    };
    let want = expected.remove(0);
    let check = hud_messages::compare(&want, &observed, 0.0);
    stats.0 += 1;
    stats.1 += u64::from(check.equal);
    stats.4 = stats.4.max(check.max_position_error);
    if !check.equal && stats.0 - stats.1 < 10 {
        eprintln!(
            "tsfp-bevy: hud: message shadow mismatch frame {}: want ({}, {}, {:#x}, {:#x}, {:?}) got ({}, {}, {:#x}, {:#x}, {:?})",
            FRAMES.load(Ordering::Relaxed),
            want.x,
            want.y,
            want.colour,
            want.outline,
            String::from_utf8_lossy(&want.text),
            observed.x,
            observed.y,
            observed.colour,
            observed.outline,
            String::from_utf8_lossy(&observed.text)
        );
    }
}

/// The message queue's gates (increment 7). `Some(answer)` ends the gate; `None` continues with `messages` set.
fn message_gate(
    api: &HostUiGuestApi,
    va: u32,
    esp: u32,
    owned: HudOwnership,
    messages: &mut Option<QueueDraw>,
) -> Option<i32> {
    let shadow = *MESSAGE_SHADOW
        .get_or_init(|| std::env::var("TSFP_HUD_MESSAGES_SHADOW").is_ok_and(|v| v == "1"));
    if va == hud_messages::message_addr::FN_OUTLINED {
        // Observer: always passes through.
        if shadow {
            message_shadow_observe(api, esp);
        }
        return Some(0);
    }
    if queue_census(api) {
        if let Some(index) = drawer_index(va) {
            CENSUS_ACTIVE[index].fetch_add(1, Ordering::Relaxed);
        }
    }
    if !owned.owns(HudElement::Messages) {
        PASSED.fetch_add(1, Ordering::Relaxed);
        return Some(0);
    }
    let Some(fonts) = ASSETS.get().and_then(|a| a.text.as_ref()).map(|t| &t.fonts) else {
        MESSAGE_LEGACY.fetch_add(1, Ordering::Relaxed);
        PASSED.fetch_add(1, Ordering::Relaxed);
        return Some(0);
    };
    let only_kind_9 = api.read32_raw(esp.wrapping_add(4)).unwrap_or(0);
    let native = messages_native_replay(api, fonts, only_kind_9);
    match native.unwrap_or_else(|| hud_messages::replay(&Guest(api), fonts, only_kind_9)) {
        QueueCall::Draw(draw) => {
            if shadow {
                // Expected strings for the original's call that follows; leftovers of the last call were never drawn.
                if let (Ok(mut expected), Ok(mut stats)) =
                    (MESSAGE_EXPECTED.lock(), MESSAGE_SHADOW_STATS.lock())
                {
                    stats.2 += expected.len() as u64;
                    expected.clear();
                    if draw.original_calls().is_empty() {
                        expected.extend(draw.calls.iter().cloned());
                    } else {
                        stats.3 += 1;
                    }
                }
                PASSED.fetch_add(1, Ordering::Relaxed);
                return Some(0);
            }
            let n = QUEUES_DRAWN.fetch_add(1, Ordering::Relaxed);
            if !draw.calls.is_empty()
                && n % 60 == 0
                && MESSAGES_NATIVE.load(Ordering::Relaxed) < 2000
            {
                eprintln!(
                    "tsfp-bevy: hud: messages frame {} kinds {:?} strings {:?}",
                    FRAMES.load(Ordering::Relaxed),
                    draw.native_kinds,
                    draw.calls
                        .iter()
                        .map(|c| (
                            c.x,
                            c.y,
                            format!("{:#x}/{:#x}", c.colour, c.outline),
                            String::from_utf8_lossy(&c.text).into_owned()
                        ))
                        .collect::<Vec<_>>()
                );
            }
            MESSAGES_NATIVE.fetch_add(draw.native_kinds.len() as u64, Ordering::Relaxed);
            // The original's guest effects in its order: the drawers of the kinds not replayed yet (TEMPORARY-SHIM)
            // and the blend mode and font the native kinds leave selected.
            for op in &draw.ops {
                match *op {
                    GuestOp::Drawer { drawer, entry } => {
                        MESSAGES_TO_ORIGINAL.fetch_add(1, Ordering::Relaxed);
                        let _ = api.call_raw(drawer, &[entry]);
                    }
                    GuestOp::Blend(mode) => {
                        let _ = api.call_raw(hud_messages::message_addr::FN_BLEND, &[mode]);
                    }
                    GuestOp::Font(font) => {
                        let _ = api.call_raw(hud_messages::message_addr::FN_SELECT_FONT, &[font]);
                    }
                }
            }
            *messages = Some(draw);
            None
        }
        QueueCall::Nothing => None,
        QueueCall::Legacy(why) => {
            if MESSAGE_LEGACY.fetch_add(1, Ordering::Relaxed) < 5 {
                eprintln!(
                    "tsfp-bevy: hud: message queue left to the original ({why}) frame {}",
                    FRAMES.load(Ordering::Relaxed)
                );
            }
            PASSED.fetch_add(1, Ordering::Relaxed);
            Some(0)
        }
    }
}

fn messages_native_replay(
    api: &HostUiGuestApi,
    fonts: &tsfp_ui_model::font::FontSet,
    only_kind_9: u32,
) -> Option<QueueCall> {
    if !crate::hud_producer_live::messages_native_enabled() {
        return None;
    }
    let viewer = api.read32_raw(0x7b0c7c)?;
    let slot = usize::try_from(api.read32_raw(viewer + 4)? as i32)
        .ok()
        .filter(|slot| *slot < 4)?;
    if !import_guest_message_queue(api, slot) {
        return None;
    }
    let policy = crate::hud_producer_live::message_policy(api)?;
    let mut state = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
        .ok()?;
    let queue = state.messages.get(slot)?;
    if !queue.ring.entries.iter().any(|message| message.kind != 0) {
        return None;
    }
    if queue
        .ring
        .entries
        .iter()
        .any(|message| matches!(message.kind, 7 | 8 | 9))
    {
        return Some(hud_messages::replay_owned_native(
            &Guest(api),
            fonts,
            only_kind_9,
            &mut state,
            slot as i32,
            &policy,
        ));
    }
    Some(hud_messages::replay_owned(
        &Guest(api),
        fonts,
        only_kind_9,
        queue,
        &policy,
    ))
}

fn drawer_index(va: u32) -> Option<usize> {
    HudElement::ALL
        .iter()
        .flat_map(|e| e.drawers().iter().copied())
        .position(|d| d == va)
}

/// An unowned ring must drain before import; an owned typed queue stays authoritative.
pub(crate) fn guest_message_queue_empty(api: &HostUiGuestApi, slot: usize) -> bool {
    if slot >= 4 {
        return false;
    }
    let record = 0x7a6160 + slot as u32 * 0x177c;
    (0..16).all(|index| {
        let address = record + 0x38 + index * 0x16c;
        api.read32_raw(address).is_some_and(|kind| kind == 0)
    })
}

/// Transfer the drained original queue into typed Rust state once. This is a
/// read-only ownership handoff: later insert, tick, clear and draw operations
/// use the imported queue without reading or writing guest message storage.
pub(crate) fn import_guest_message_queue(api: &HostUiGuestApi, slot: usize) -> bool {
    import_guest_message_queue_inner(api, slot, false)
}

/// One-time read-only import at the kind-6 producer boundary. Unlike generic
/// message admission, this producer can run while legacy entries are active;
/// capture the full ring before mirroring its mutation into owned state.
pub(crate) fn import_guest_message_queue_for_kind6(api: &HostUiGuestApi, slot: usize) -> bool {
    import_guest_message_queue_inner(api, slot, true)
}

fn import_guest_message_queue_inner(
    api: &HostUiGuestApi,
    slot: usize,
    allow_active_queue: bool,
) -> bool {
    use tsfp_ui_model::{hud_producers::MessageQueueSnapshot, messages::Message};

    if slot >= 4 {
        return false;
    }
    let state = HUD_STATE.get_or_init(|| Mutex::new(hud_native::HudState::default()));
    if state
        .lock()
        .ok()
        .is_some_and(|state| state.message_queue_imported(slot))
    {
        return true;
    }
    if !allow_active_queue && !guest_message_queue_empty(api, slot) {
        return false;
    }

    let record = 0x7a6160 + slot as u32 * 0x177c;
    let mut entries: [Message; 16] = core::array::from_fn(|_| Message::default());
    let mut slides = [0.0; 16];
    let mut cues = [0; 16];
    let mut playing_handles = [0; 16];
    let mut progress = [0; 16];
    for index in 0..16 {
        let base = record + 0x38 + index * 0x16c;
        let index = index as usize;
        let mut text = Vec::new();
        let mut terminated = false;
        for offset in (0..320).step_by(4) {
            let Some(word) = api.read32_raw(base + 4 + offset) else {
                return false;
            };
            for byte in word.to_le_bytes() {
                if byte == 0 {
                    terminated = true;
                    break;
                }
                text.push(byte);
            }
            if terminated {
                break;
            }
        }
        if !terminated {
            return false;
        }
        let Some((
            kind,
            age,
            fade_start,
            slide,
            end,
            evict_after,
            weight,
            cue,
            handle,
            entry_progress,
            opaque,
        )) = (|| {
            Some((
                api.read32_raw(base)?,
                api.read32_raw(base + 0x144).map(f32::from_bits)?,
                api.read32_raw(base + 0x148).map(f32::from_bits)?,
                api.read32_raw(base + 0x14c).map(f32::from_bits)?,
                api.read32_raw(base + 0x150).map(f32::from_bits)?,
                api.read32_raw(base + 0x154).map(f32::from_bits)?,
                api.read32_raw(base + 0x158)?,
                api.read32_raw(base + 0x15c).map(|value| value as i32)?,
                api.read32_raw(base + 0x160).map(|value| value as i32)?,
                api.read32_raw(base + 0x164)?,
                api.read32_raw(base + 0x168)?,
            ))
        })()
        else {
            return false;
        };
        entries[index] = Message {
            kind,
            text,
            age,
            fade_start,
            end,
            evict_after,
            weight,
            opaque,
        };
        slides[index] = slide;
        cues[index] = cue;
        playing_handles[index] = handle;
        progress[index] = entry_progress;
    }
    let Some((cursor, input_progress)) = api
        .read32_raw(record + 0x30)
        .zip(api.read32_raw(record + 0x1710))
    else {
        return false;
    };
    let snapshot = MessageQueueSnapshot {
        cursor: cursor as usize,
        entries,
        slides,
        cues,
        playing_handles,
        progress,
        input_progress,
    };
    state
        .lock()
        .ok()
        .is_some_and(|mut state| state.import_message_queue(slot, snapshot))
}

/// Advance one native queue only when it owns live entries. The guest fallback remains
/// responsible for legacy entries until the native admission hooks are attached.
pub(crate) fn advance_owned_messages(
    player: i32,
    seconds: f32,
    frame_delta: i32,
    policy: &tsfp_ui_model::hud_producers::MessagePolicy,
    audio: &mut impl tsfp_ui_model::hud_producers::HudSounds,
) -> bool {
    let Some(slot) = usize::try_from(player).ok().filter(|slot| *slot < 4) else {
        return false;
    };
    let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
        return false;
    };
    if !import_guest_message_queue(api, slot) {
        return false;
    }
    let Some(mut state) = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
        .ok()
    else {
        return false;
    };
    if !state.messages[slot]
        .ring
        .entries
        .iter()
        .any(|message| message.kind != 0)
    {
        return false;
    }
    state.advance_messages(player, seconds, frame_delta, policy, audio)
}

/// Insert through the typed queue and return the borrowed original slot address
/// only for the legacy caller's immediate return ABI. It is never retained.
pub(crate) fn insert_owned_message(
    player: i32,
    kind: u32,
    text: &[u8],
    delay: f32,
    cue: Option<i32>,
    preset: &tsfp_ui_model::hud_producers::GenericMessagePreset,
) -> Option<u32> {
    use tsfp_ui_model::hud_producers::{MessageAdmission, MessageInsertion};
    let slot = usize::try_from(player).ok().filter(|slot| *slot < 4)?;
    let mut state = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
        .ok()?;
    let entry_index = match cue {
        Some(cue) => match state.insert_message_with_cue(player, kind, text, delay, cue, preset) {
            MessageAdmission::Inserted(id) => id.slot(),
            MessageAdmission::Invalid | MessageAdmission::Duplicate => return Some(0),
        },
        None => match state.insert_message(player, kind, text, delay, preset) {
            MessageInsertion::Inserted(index) => index,
            MessageInsertion::Invalid | MessageInsertion::Duplicate => return Some(0),
        },
    };
    Some(0x7a6160 + slot as u32 * 0x177c + 0x38 + entry_index as u32 * 0x16c)
}

/// Mirror the original selective clear into the Rust queue before the retained
/// function clears its guest ring. Both stores remain transient during migration.
pub(crate) fn clear_owned_messages(player: i32, kind: i32) {
    if let Some(mut state) = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
        .ok()
    {
        state.clear_messages(player, kind);
    }
}

pub(crate) fn mirror_kind6_message(
    player: i32,
    source: u32,
    age_seed: f32,
    tuning: tsfp_ui_model::hud_producers::Kind6MessageTuning,
) {
    if let Some(mut state) = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
        .ok()
    {
        let _ = state.post_kind6_message(player, source, age_seed, tuning);
    }
}

pub(crate) fn push_owned_notice(
    player: i32,
    kind: tsfp_ui_model::hud_producers::NoticeKind,
    text: &[u8],
    opaque: u32,
    delay: f32,
    tuning: &tsfp_ui_model::messages::MessageTuning,
) -> bool {
    let Some(mut state) = HUD_STATE
        .get_or_init(|| Mutex::new(hud_native::HudState::default()))
        .lock()
        .ok()
    else {
        return false;
    };
    state.message_notice(player, kind, text, opaque, delay, tuning)
}

/// Shell start: read `TSFP_HUD_NATIVE` (`off` default), `TSFP_HUD_ANCHOR` (`picture` default or
/// `safe`) and load textures and fonts from `TSFP_HUD_DISC`, else `TSFP_UI_DISC`.
pub fn init() {
    let ownership = match std::env::var("TSFP_HUD_NATIVE") {
        Ok(text) => HudOwnership::parse(&text).unwrap_or_else(|why| {
            eprintln!("tsfp-bevy: hud: TSFP_HUD_NATIVE: {why}; legacy HUD");
            HudOwnership::NONE
        }),
        Err(_) => HudOwnership::NONE,
    };
    let _ = OWNERSHIP.set(ownership);
    let anchor = match std::env::var("TSFP_HUD_ANCHOR").as_deref() {
        Ok("safe") => HudAnchor::SafeArea,
        _ => HudAnchor::Picture,
    };
    let _ = ANCHOR.set(anchor);
    if ownership.is_empty() {
        return;
    }
    let names: Vec<&str> = {
        let table = HudLayout::original();
        let mut names = vec![table.ammo_icon_texture, table.ammo_frame_texture];
        names.extend(hud_native::RETICLE_TEXTURES.iter().map(|(_, name)| *name));
        names.extend(hud_native::ARC_PASSES.iter().map(|(_, _, name)| *name));
        names.push(hud_native::HIT_SECTOR_TEXTURE);
        names.push(hud_rank::row::FRAME_TEXTURE);
        names.push(hud_rank::row::PANEL_TEXTURE);
        names.extend(hud_radar::RADAR_TEXTURES.iter().map(|(_, name)| *name));
        names.extend(hud_prompts::PROMPT_TEXTURES);
        names.extend(hud_effects::EFFECT_TEXTURES);
        names.extend(hud_weapons::WEAPON_TEXTURES.iter().map(|(_, name)| *name));
        names.extend(hud_weapons::ARROW_TEXTURES);
        names
    };
    let disc = std::env::var("TSFP_HUD_DISC")
        .or_else(|_| std::env::var("TSFP_UI_DISC"))
        .ok()
        .filter(|d| !d.is_empty());
    let mut assets = HudAssets::default();
    let mut note = String::from("no disc (set TSFP_UI_DISC): no textures, stand-in digits");
    if let Some(disc) = disc {
        let path = std::path::Path::new(&disc);
        match tsfp_ui_wgpu::overlay::TextAssets::from_disc(path, 'e') {
            Ok(text) => assets.text = Some(text),
            Err(why) => eprintln!("tsfp-bevy: hud: no original fonts ({why})"),
        }
        match HudTextures::from_disc(path, &names) {
            Ok(textures) => assets.textures = textures,
            Err(why) => eprintln!("tsfp-bevy: hud: no HUD textures ({why})"),
        }
        let missing: Vec<&&str> = names
            .iter()
            .filter(|n| !assets.textures.images.contains_key(**n))
            .collect();
        note = format!(
            "{} of {} textures from the disc{}, {}",
            names.len() - missing.len(),
            names.len(),
            if missing.is_empty() {
                String::new()
            } else {
                format!(" (missing {missing:?})")
            },
            if assets.text.is_some() {
                "original fonts"
            } else {
                "stand-in digits"
            }
        );
    }
    let owned: Vec<&str> = ownership.elements().map(HudElement::name).collect();
    eprintln!(
        "tsfp-bevy: hud: native HUD owns {owned:?} (anchor {anchor:?}); {note}; legacy draws the rest"
    );
    let _ = ASSETS.set(assets);
}

fn ownership() -> HudOwnership {
    OWNERSHIP.get().copied().unwrap_or_default()
}

/// The C gate's question, on the guest thread: is legacy drawer `va` replaced by the native HUD?
/// `esp` points at the drawer's return slot (its arguments follow); recorded for the request.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_native_owns(va: u32, esp: u32) -> i32 {
    if !IN_FRAME.load(Ordering::Acquire) {
        return 0;
    }
    let owned = ownership();
    let Some(element) = HudElement::of_drawer(va) else {
        return 0;
    };
    if let Some(index) = drawer_index(va) {
        CALLS[index].fetch_add(1, Ordering::Relaxed);
    }
    // Draw probe: primitives are charged to the census drawer that ran last; any other gated drawer ends its span.
    if PROBE_PRIMITIVES.contains(&va) {
        let current = PROBE_CURRENT.load(Ordering::Relaxed);
        if let Some(index) = (current != 0).then(|| drawer_index(current)).flatten() {
            PROBE_DRAWS[index].fetch_add(1, Ordering::Relaxed);
        }
        if element == HudElement::Census {
            return 0;
        }
    } else {
        PROBE_CURRENT.store(
            if element == HudElement::Census { va } else { 0 },
            Ordering::Relaxed,
        );
    }
    // Census drawers are only counted, the original always draws them; for the pop-up ring and the button
    // prompts the calls that would draw something are counted too (their own early-out state).
    if element == HudElement::Census {
        // SAFETY: the pointer is the C host's static API table (never freed).
        if let Some(api) = unsafe { API.load(Ordering::Acquire).as_ref() } {
            let active = match va {
                0x0013_c060 => {
                    let count = api.read32_raw(0x0074_9108).unwrap_or(0) & 0xff;
                    (0..count)
                        .any(|i| api.read32_raw(0x0074_8f10 + 0x14 * i).unwrap_or(0) >> 16 != 0)
                }
                _ => false,
            };
            let skip = CENSUS_SKIP.get_or_init(|| {
                std::env::var("TSFP_HUD_CENSUS_SKIP")
                    .unwrap_or_default()
                    .split(',')
                    .filter_map(|v| u32::from_str_radix(v.trim().trim_start_matches("0x"), 16).ok())
                    .collect()
            });
            if skip.contains(&va) {
                return 1;
            }
            if active {
                if let Some(index) = drawer_index(va) {
                    let n = CENSUS_ACTIVE[index].fetch_add(1, Ordering::Relaxed);
                    if n % 300 == 0 && n < 3000 {
                        eprintln!(
                            "tsfp-bevy: hud: census {va:#x} active at frame {}",
                            FRAMES.load(Ordering::Relaxed)
                        );
                    }
                }
            }
        }
        return 0;
    }
    // The message queue (increment 7): the queue call is replayed, `0x41E30` observed for the shadow check.
    let mut messages = None;
    if element == HudElement::Messages {
        // SAFETY: the pointer is the C host's static API table (never freed).
        let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
            return 0;
        };
        if let Some(answer) = message_gate(api, va, esp, owned, &mut messages) {
            return answer;
        }
    }
    // The weapon slot panel (increment 8): replayed; the primitives are observed for the shadow check.
    let mut weapons = None;
    if element == HudElement::Weapons {
        // SAFETY: the pointer is the C host's static API table (never freed).
        let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
            return 0;
        };
        if let Some(answer) = weapon_gate(api, va, esp, owned, &mut weapons) {
            return answer;
        }
    }
    // The prompt panel (increment 6): show, dismiss and level init pass through and drive the Rust-owned
    // lifecycle; the drawer is replayed on the live state.
    let mut prompts = None;
    if element == HudElement::Prompts {
        // SAFETY: the pointer is the C host's static API table (never freed).
        let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
            return 0;
        };
        let arg0 = api.read32_raw(esp.wrapping_add(4)).unwrap_or(u32::MAX);
        match va {
            hud_prompts::prompt_addr::FN_SHOW => {
                if let Ok(mut panels) = PROMPTS.lock() {
                    panels.show(arg0);
                }
                return 0;
            }
            hud_prompts::prompt_addr::FN_DISMISS => {
                if let Ok(mut panels) = PROMPTS.lock() {
                    panels.dismiss(arg0 as i32);
                }
                return 0;
            }
            hud_prompts::prompt_addr::FN_INIT => {
                if let Ok(mut panels) = PROMPTS.lock() {
                    panels.reset();
                }
                return 0;
            }
            _ => {}
        }
        let index = arg0 as i32;
        if index >= 0
            && api
                .read32_raw(
                    hud_prompts::prompt_addr::RECORDS
                        + hud_prompts::prompt_addr::RECORD_SIZE * index as u32
                        + hud_prompts::prompt_addr::ACTIVE,
                )
                .unwrap_or(0)
                != 0
        {
            if let Some(i) = drawer_index(va) {
                CENSUS_ACTIVE[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        if !owned.owns(element) {
            PASSED.fetch_add(1, Ordering::Relaxed);
            return 0;
        }
        let Some(fonts) = ASSETS.get().and_then(|a| a.text.as_ref()).map(|t| &t.fonts) else {
            PROMPT_LEGACY.fetch_add(1, Ordering::Relaxed);
            PASSED.fetch_add(1, Ordering::Relaxed);
            return 0;
        };
        let shadow = *PROMPT_SHADOW
            .get_or_init(|| std::env::var("TSFP_HUD_PROMPTS_SHADOW").is_ok_and(|v| v == "1"));
        let Ok(mut panels) = PROMPTS.lock() else {
            return 0;
        };
        if shadow {
            if let (Some(check), Ok(mut stats)) = (
                panels.shadow_check(&Guest(api), fonts, index),
                PROMPT_SHADOW_STATS.lock(),
            ) {
                stats.0 += 1;
                stats.1 += u64::from(check.height_equal);
                stats.2 += u64::from(check.phase_equal);
                stats.3 = stats.3.max(check.alpha_error);
                stats.4 = stats.4.max(check.timer_error);
                if (!check.height_equal || !check.phase_equal)
                    && stats.0 - stats.1.min(stats.2) < 10
                {
                    eprintln!(
                        "tsfp-bevy: hud: prompt shadow mismatch frame {}: {check:?}",
                        FRAMES.load(Ordering::Relaxed)
                    );
                }
            }
            PASSED.fetch_add(1, Ordering::Relaxed);
            return 0;
        }
        let animation = if owned_producers() {
            api.read32_raw(hud_weapons::weapon_addr::VIEWER)
                .and_then(|v| api.read32_raw(v + 4))
                .filter(|slot| *slot < 4)
                .and_then(|slot| {
                    HUD_STATE
                        .get()?
                        .lock()
                        .ok()
                        .map(|s| s.weapons[slot as usize])
                })
        } else {
            None
        };
        let call = match animation {
            Some(animation) => panels.read_owned(&Guest(api), fonts, index, &animation),
            None => panels.read(&Guest(api), fonts, index),
        };
        match call {
            PromptCall::Draw(draw) => {
                let n = PROMPTS_DRAWN.fetch_add(1, Ordering::Relaxed);
                if n % 300 == 0 && n < 3000 {
                    eprintln!(
                        "tsfp-bevy: hud: prompts frame {} height {} panel {:?} alphas {:?} texts {:?} icons {:?}",
                        FRAMES.load(Ordering::Relaxed),
                        draw.height,
                        draw.panel
                            .first()
                            .map(|p| p.rect[..2].to_vec())
                            .zip(draw.panel.get(3).map(|p| p.rect[2..].to_vec())),
                        draw.panel_alphas,
                        draw.texts
                            .iter()
                            .map(|t| (
                                t.font,
                                t.text_scale,
                                t.runs
                                    .iter()
                                    .map(|r| (
                                        String::from_utf8_lossy(&r.bytes).into_owned(),
                                        r.x,
                                        r.y
                                    ))
                                    .collect::<Vec<_>>()
                            ))
                            .collect::<Vec<_>>(),
                        draw.icons
                            .iter()
                            .map(|(i, _)| (i.texture, i.rect))
                            .collect::<Vec<_>>()
                    );
                }
                // The original leaves the last line's font selected (`0x3FA10`); keep that effect.
                if let Some(font) = draw.last_font {
                    let _ = api.call_raw(0x0003_fa10, &[font]);
                }
                prompts = Some(draw);
            }
            PromptCall::Nothing => {}
            PromptCall::Legacy(why) => {
                if PROMPT_LEGACY.fetch_add(1, Ordering::Relaxed) < 5 {
                    eprintln!(
                        "tsfp-bevy: hud: prompts left to the original ({why}) frame {}",
                        FRAMES.load(Ordering::Relaxed)
                    );
                }
                PASSED.fetch_add(1, Ordering::Relaxed);
                return 0;
            }
        }
    }
    // Hit-sector state moved to Rust: the level reset passes through and clears it; the setter's hit
    // is taken by Rust (the guest entry stays 0) while the native HUD owns the vitals.
    if va == hud_native::hit_addr::FN_RESET {
        if let Some(state) = HUD_STATE.get() {
            if let Ok(mut state) = state.lock() {
                for slot in 0..4 {
                    state.reset_slot(slot);
                }
            }
        }
        if let Ok(mut hits) = HIT_SECTORS.lock() {
            hits.reset();
        }
        if let Ok(mut radar) = RADAR.lock() {
            radar.reset();
        }
        return 0;
    }
    if va == hud_native::hit_addr::FN_SET {
        if !owned.owns(HudElement::Vitals) {
            return 0;
        }
        // SAFETY: the pointer is the C host's static API table (never freed).
        let Some(api) = (unsafe { API.load(Ordering::Acquire).as_ref() }) else {
            return 0;
        };
        let (Some(index), Some(direction)) = (
            api.read32_raw(esp.wrapping_add(4)),
            api.read32_raw(esp.wrapping_add(8)),
        ) else {
            return 0;
        };
        let taken = HIT_SECTORS
            .lock()
            .map(|mut hits| hits.hit_from_call(&Guest(api), index, direction))
            .unwrap_or(false);
        if taken {
            HITS.fetch_add(1, Ordering::Relaxed);
            SKIPPED.fetch_add(1, Ordering::Relaxed);
            return 1;
        }
        return 0;
    }
    // The reticle: replay the drawer's decisions on the live state at the call (same thread, same frame).
    let mut reticle = None;
    if element == HudElement::Crosshair && owned.owns(element) {
        // SAFETY: the pointer is the C host's static API table (never freed).
        let call = match unsafe { API.load(Ordering::Acquire).as_ref() } {
            Some(api) => Reticle::read(&Guest(api)),
            None => ReticleCall::Legacy,
        };
        match call {
            ReticleCall::NoSlot => {}
            ReticleCall::Draw(_) | ReticleCall::Nothing => {
                if let ReticleCall::Draw(r) = call {
                    reticle = Some(r);
                }
                // The skipped drawer first updates the HUD rectangle (0x14AF00, no arguments); keep that effect.
                // SAFETY: as above.
                if let Some(api) = unsafe { API.load(Ordering::Acquire).as_ref() } {
                    let _ = api.call_raw(hud_native::reticle_addr::FN_HUD_RECT, &[]);
                }
            }
            ReticleCall::Legacy => {
                RETICLE_LEGACY.fetch_add(1, Ordering::Relaxed);
                PASSED.fetch_add(1, Ordering::Relaxed);
                return 0;
            }
        }
    }
    // The ammo row: the call's arguments name the hand and the side.
    let mut args = [0u32; 5];
    let mut ammo = None;
    if element == HudElement::Ammo && owned.owns(element) {
        // SAFETY: as above.
        let call = match unsafe { API.load(Ordering::Acquire).as_ref() } {
            Some(api) => {
                for (i, arg) in args.iter_mut().enumerate() {
                    *arg = api
                        .read32_raw(esp.wrapping_add(4 + 4 * i as u32))
                        .unwrap_or(0);
                }
                if owned_producers() {
                    let mut index = 0;
                    match live_slot(api).filter(|_| args[4] < 2) {
                        Some(slot) if tsfp_hud_row_metric(slot, args[4], 0, 0, &mut index) != 0 => {
                            AmmoRow::read_owned(&Guest(api), args, index as u8)
                        }
                        _ => AmmoCall::Legacy,
                    }
                } else {
                    AmmoRow::read(&Guest(api), args)
                }
            }
            None => AmmoCall::Legacy,
        };
        match call {
            AmmoCall::Draw(row) => {
                if let Some(api) = unsafe { API.load(Ordering::Acquire).as_ref() } {
                    let Some(slot) = live_slot(api) else { return 0 };
                    let side = u32::from(row.side == hud_native::AmmoSide::Right);
                    // Reference selection observes the request then gives the whole
                    // drawer back to the original, including its cursor advance.
                    if !owned_producers() {
                        return 0;
                    }
                    let mut index = 0;
                    if tsfp_hud_row_metric(slot, side, 2, 1, &mut index) == 0 {
                        return 0;
                    }
                }
                if row.stack_index != 0 {
                    static STACKED_ROWS: AtomicU64 = AtomicU64::new(0);
                    let n = STACKED_ROWS.fetch_add(1, Ordering::Relaxed);
                    if n < 12 {
                        eprintln!(
                            "tsfp-bevy: hud: stacked ammo side {:?} row {} weapon {} (49-unit pitch)",
                            row.side, row.stack_index, row.weapon_record
                        );
                    }
                }
                ammo = Some(row);
            }
            AmmoCall::Nothing => {}
            AmmoCall::Legacy => {
                PASSED.fetch_add(1, Ordering::Relaxed);
                return 0;
            }
        }
    }
    // The health and armour arcs: replay the wrapper and the drawer's state reads.
    let mut vitals = None;
    let mut hit_sector = None;
    if element == HudElement::Vitals && owned.owns(element) {
        // SAFETY: as above.
        let call = match unsafe { API.load(Ordering::Acquire).as_ref() } {
            Some(api) => VitalsArcs::read(&Guest(api)),
            None => VitalsCall::Legacy,
        };
        match call {
            VitalsCall::Draw(arcs) => {
                vitals = Some(arcs);
                // SAFETY: as above.
                if let (Some(api), Ok(mut hits)) = (
                    unsafe { API.load(Ordering::Acquire).as_ref() },
                    HIT_SECTORS.lock(),
                ) {
                    hit_sector = hits.step(&Guest(api), arcs.hit_index);
                    if let Some(sector) = hit_sector {
                        let n = SECTORS_DRAWN.fetch_add(1, Ordering::Relaxed);
                        if n % 30 == 0 && n < 300 {
                            eprintln!(
                                "tsfp-bevy: hud: hit sector frame {} entry {} corners {:?} colour {:?}",
                                FRAMES.load(Ordering::Relaxed),
                                arcs.hit_index,
                                sector.corners,
                                sector.color
                            );
                        }
                    }
                }
            }
            VitalsCall::Nothing => {}
            VitalsCall::Legacy => {
                VITALS_LEGACY.fetch_add(1, Ordering::Relaxed);
                PASSED.fetch_add(1, Ordering::Relaxed);
                return 0;
            }
        }
    }
    // The radar: collection and drawing replayed on the live state (blips, sweep and pulse restart Rust-owned).
    let mut radar = None;
    if element == HudElement::Radar && owned.owns(element) {
        // SAFETY: as above.
        let shadow = *RADAR_SHADOW
            .get_or_init(|| std::env::var("TSFP_HUD_RADAR_SHADOW").is_ok_and(|v| v == "1"));
        let call = match (
            unsafe { API.load(Ordering::Acquire).as_ref() },
            RADAR.lock(),
        ) {
            (Some(api), Ok(mut state)) if shadow => {
                let slot = api
                    .read32_raw(hud_radar::radar_addr::VIEWER)
                    .and_then(|v| api.read32_raw(v + 4))
                    .unwrap_or(0) as usize;
                if let (Some(check), Ok(mut stats)) = (
                    state.shadow_check(&Guest(api), slot.min(3)),
                    RADAR_SHADOW_STATS.lock(),
                ) {
                    if stats.0 > 0 {
                        stats.1 += u64::from(check.blips_equal);
                        stats.2 = stats.2.max(check.max_angle_error);
                        stats.3 = stats.3.max(check.max_distance_error);
                        stats.4 = stats.4.max(check.sweep_error);
                        if !check.blips_equal && stats.0 - stats.1 < 10 {
                            eprintln!(
                                "tsfp-bevy: hud: radar shadow mismatch frame {}: {check:?}",
                                FRAMES.load(Ordering::Relaxed)
                            );
                        }
                    }
                    stats.0 += 1;
                }
                let _ = state.read(&Guest(api));
                RadarCall::Legacy("shadow check")
            }
            (Some(api), Ok(mut state)) => state.read(&Guest(api)),
            _ => RadarCall::Legacy("no guest API"),
        };
        match call {
            RadarCall::Draw(pieces) => {
                let n = RADARS_DRAWN.fetch_add(1, Ordering::Relaxed);
                if n % 600 == 0 && n < 6000 {
                    eprintln!(
                        "tsfp-bevy: hud: radar frame {} pieces {:?}",
                        FRAMES.load(Ordering::Relaxed),
                        pieces
                    );
                }
                radar = Some(pieces);
            }
            RadarCall::Nothing => {}
            RadarCall::Legacy(why) => {
                if RADAR_LEGACY.fetch_add(1, Ordering::Relaxed) == 0 {
                    eprintln!("tsfp-bevy: hud: radar left to the original ({why})");
                }
                PASSED.fetch_add(1, Ordering::Relaxed);
                return 0;
            }
        }
    }
    if let Ok(mut pending) = PENDING.lock() {
        if pending.len() < 64 {
            pending.push(DrawRequest {
                element,
                drawer: va,
                args,
                reticle,
                ammo,
                vitals,
                hit_sector,
                rank: None,
                radar,
                prompts,
                effects: None,
                messages,
                weapons,
            });
        }
    }
    if owned.owns(element) {
        if reticle.is_some() {
            // The native request is retained above; queue its renderer boundary at
            // the skipped original draw, before scene copies/effects can sample it.
            // SAFETY: linked Rust trace producer runs on this same guest thread.
            unsafe { tsfp_render_trace_view_layer_boundary() };
        }
        SKIPPED.fetch_add(1, Ordering::Relaxed);
        1
    } else {
        PASSED.fetch_add(1, Ordering::Relaxed);
        0
    }
}

/// Text widths from the original font tables (the native font layer, same tables the renderer shapes with).
struct FontWidths<'a>(FontTableMeasure<'a>);
impl TextWidth for FontWidths<'_> {
    fn width(&self, font: u32, text: &[u8]) -> Option<f32> {
        self.0.fonts.font(font)?;
        Some(self.0.width_px(font, text))
    }
}

/// The C register gate's question for a drawer with register arguments (the rank box `0x12EC20`:
/// `eax` the player's record, `esi` the row cursor pointer). Owned: the native rank box is queued for
/// this frame and `*advance` is the row height the gate adds to `*esi` (the original's cursor advance).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_native_owns_row(
    va: u32,
    esp: u32,
    eax: u32,
    esi: u32,
    advance: *mut u32,
) -> i32 {
    if !IN_FRAME.load(Ordering::Acquire) || advance.is_null() {
        return 0;
    }
    if va == hud_rank::points_addr::FN_POINTS_ROW {
        return points_gate(va, esp, esi, advance);
    }
    if va != hud_rank::rank_addr::FN_RANK_BOX {
        return 0;
    }
    if let Some(index) = drawer_index(va) {
        CALLS[index].fetch_add(1, Ordering::Relaxed);
    }
    if !ownership().owns(HudElement::Rank) {
        PASSED.fetch_add(1, Ordering::Relaxed);
        return 0;
    }
    let call = (|| {
        // SAFETY: the pointer is the C host's static API table (never freed).
        let api = unsafe { API.load(Ordering::Acquire).as_ref() }?;
        let assets = ASSETS.get()?;
        let text = assets.text.as_ref()?;
        let label = text
            .strings
            .bytes(TextId(hud_rank::rank_addr::LABEL_ID))?
            .to_vec();
        let widths = FontWidths(FontTableMeasure::new(&text.fonts, &text.strings));
        Some(if owned_producers() {
            RankBox::read_owned(
                &Guest(api),
                eax,
                column_top(api, esi)?,
                esp,
                Some(label),
                &widths,
            )
        } else {
            RankBox::read(&Guest(api), eax, esi, esp, Some(label), &widths)
        })
    })();
    let Some(RankCall::Draw(rank)) = call else {
        RANK_LEGACY.fetch_add(1, Ordering::Relaxed);
        PASSED.fetch_add(1, Ordering::Relaxed);
        return 0;
    };
    let n = RANKS_DRAWN.fetch_add(1, Ordering::Relaxed);
    if n % 600 == 0 && n < 6000 {
        eprintln!(
            "tsfp-bevy: hud: rank box frame {} advance {} texts {:?}",
            FRAMES.load(Ordering::Relaxed),
            rank.row.advance,
            rank.row
                .texts
                .iter()
                .map(|t| (
                    String::from_utf8_lossy(&t.bytes).into_owned(),
                    t.x,
                    t.y,
                    t.scale_x
                ))
                .collect::<Vec<_>>()
        );
    }
    // SAFETY: the C gate passes a pointer to its local.
    unsafe { *advance = rank.row.advance };
    if !owned_producers() {
        return 0;
    }
    if let Ok(mut pending) = PENDING.lock() {
        if pending.len() < 64 {
            pending.push(DrawRequest {
                element: HudElement::Rank,
                drawer: va,
                args: [0; 5],
                reticle: None,
                ammo: None,
                vitals: None,
                hit_sector: None,
                rank: Some(rank),
                radar: None,
                prompts: None,
                effects: None,
                messages: None,
                weapons: None,
            });
        }
    }
    SKIPPED.fetch_add(1, Ordering::Relaxed);
    1
}

/// Challenge points rows drawn natively and left to the original (increment 8).
static POINTS_DRAWN: AtomicU64 = AtomicU64::new(0);
static POINTS_LEGACY: AtomicU64 = AtomicU64::new(0);

/// The points row `0x12ECA0` (register `esi` row cursor, `[esp+4]` alpha).
fn points_gate(va: u32, esp: u32, esi: u32, advance: *mut u32) -> i32 {
    if let Some(index) = drawer_index(va) {
        CALLS[index].fetch_add(1, Ordering::Relaxed);
    }
    if !ownership().owns(HudElement::Points) {
        PASSED.fetch_add(1, Ordering::Relaxed);
        return 0;
    }
    let call = (|| {
        // SAFETY: the pointer is the C host's static API table (never freed).
        let api = unsafe { API.load(Ordering::Acquire).as_ref() }?;
        let text = ASSETS.get()?.text.as_ref()?;
        let widths = FontWidths(FontTableMeasure::new(&text.fonts, &text.strings));
        Some(if owned_producers() {
            hud_rank::points_row_owned(&Guest(api), column_top(api, esi)?, esp, &widths)
        } else {
            hud_rank::points_row(&Guest(api), esi, esp, &widths)
        })
    })();
    let row = match call {
        Some(hud_rank::PointsCall::Draw(row)) => row,
        Some(hud_rank::PointsCall::Nothing) => {
            // SAFETY: the C gate passes a pointer to its local.
            unsafe { *advance = 0 };
            SKIPPED.fetch_add(1, Ordering::Relaxed);
            return 1;
        }
        _ => {
            POINTS_LEGACY.fetch_add(1, Ordering::Relaxed);
            PASSED.fetch_add(1, Ordering::Relaxed);
            return 0;
        }
    };
    let n = POINTS_DRAWN.fetch_add(1, Ordering::Relaxed);
    if n % 600 == 0 && n < 6000 {
        eprintln!(
            "tsfp-bevy: hud: points row frame {} advance {} texts {:?}",
            FRAMES.load(Ordering::Relaxed),
            row.row.advance,
            row.row
                .texts
                .iter()
                .map(|t| (
                    String::from_utf8_lossy(&t.bytes).into_owned(),
                    t.x,
                    t.y,
                    t.scale_x
                ))
                .collect::<Vec<_>>()
        );
    }
    // SAFETY: the C gate passes a pointer to its local.
    unsafe { *advance = row.row.advance };
    if !owned_producers() {
        return 0;
    }
    if let Ok(mut pending) = PENDING.lock() {
        if pending.len() < 64 {
            pending.push(DrawRequest {
                element: HudElement::Points,
                drawer: va,
                args: [0; 5],
                reticle: None,
                ammo: None,
                vitals: None,
                hit_sector: None,
                rank: Some(row),
                radar: None,
                prompts: None,
                effects: None,
                messages: None,
                weapons: None,
            });
        }
    }
    SKIPPED.fetch_add(1, Ordering::Relaxed);
    1
}

/// Draw probe (increment 8): primitives observed, the census drawer whose span is open, primitive calls per drawer.
const PROBE_PRIMITIVES: [u32; 8] = [
    0x0001_b7b0,
    0x0001_f090,
    0x0001_ba70,
    0x0004_1dc0,
    0x0004_1e30,
    0x0005_61c0,
    0x0012_cc70,
    0x0008_1b70,
];
static PROBE_CURRENT: AtomicU32 = AtomicU32::new(0);
static PROBE_DRAWS: [AtomicU64; 64] = [const { AtomicU64::new(0) }; 64];

/// Diagnostic (`TSFP_HUD_CENSUS_SKIP=va,va`): census drawers skipped outright, to measure whether they draw anything
/// visible (frame comparison with and without). Never set by play.sh.
static CENSUS_SKIP: OnceLock<Vec<u32>> = OnceLock::new();

/// The C gate's question for a drawer taking `xmm0` and a stack row cursor (the effect icons `0x12F670`:
/// `xmm0` the row alpha, `[esp+4]` the row cursor pointer). Owned: the native icons are queued and `*advance` is the
/// row height consumed by the typed allocator; the guest cursor is never written.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hud_native_owns_xmm(va: u32, esp: u32, xmm0: u32, advance: *mut u32) -> i32 {
    if !IN_FRAME.load(Ordering::Acquire)
        || advance.is_null()
        || va != hud_effects::effect_addr::FN_EFFECTS
    {
        return 0;
    }
    if let Some(index) = drawer_index(va) {
        CALLS[index].fetch_add(1, Ordering::Relaxed);
    }
    if !ownership().owns(HudElement::Effects) {
        PASSED.fetch_add(1, Ordering::Relaxed);
        return 0;
    }
    // SAFETY: the pointer is the C host's static API table (never freed).
    let icons = unsafe { API.load(Ordering::Acquire).as_ref() }.and_then(|api| {
        let cursor = api.read32_raw(esp.wrapping_add(4))?;
        if owned_producers() {
            EffectIcons::read_owned(&Guest(api), f32::from_bits(xmm0), column_top(api, cursor)?)
        } else {
            EffectIcons::read(&Guest(api), f32::from_bits(xmm0), cursor)
        }
    });
    let Some(icons) = icons else {
        EFFECT_LEGACY.fetch_add(1, Ordering::Relaxed);
        PASSED.fetch_add(1, Ordering::Relaxed);
        return 0;
    };
    let n = EFFECTS_DRAWN.fetch_add(1, Ordering::Relaxed);
    if n % 600 == 0 && n < 6000 {
        eprintln!(
            "tsfp-bevy: hud: effect icons frame {} alpha {} advance {} strengths {:?} first {:?}",
            FRAMES.load(Ordering::Relaxed),
            f32::from_bits(xmm0),
            icons.advance,
            icons.strengths,
            icons.icons[0]
        );
    }
    // SAFETY: the C gate passes a pointer to its local.
    unsafe { *advance = icons.advance };
    if !owned_producers() {
        return 0;
    }
    if let Ok(mut pending) = PENDING.lock() {
        if pending.len() < 64 {
            pending.push(DrawRequest {
                element: HudElement::Effects,
                drawer: va,
                args: [0; 5],
                reticle: None,
                ammo: None,
                vitals: None,
                hit_sector: None,
                rank: None,
                radar: None,
                prompts: None,
                effects: Some(icons),
                messages: None,
                weapons: None,
            });
        }
    }
    SKIPPED.fetch_add(1, Ordering::Relaxed);
    1
}

struct Guest<'a>(&'a HostUiGuestApi);
impl GuestRead for Guest<'_> {
    fn read32(&self, va: u32) -> Option<u32> {
        self.0.read32_raw(va)
    }
}
impl GuestCall for Guest<'_> {
    fn call(&self, va: u32, args: &[u32]) -> Option<u32> {
        self.0.call_raw(va, args)
    }
}

/// Frame boundary on the guest thread: publish the typed HUD state with this frame's requests.
pub fn service(api: &HostUiGuestApi) {
    crate::hud_producer_live::service(api);
    let owned = ownership();
    if owned.is_empty() {
        return;
    }
    API.store(
        api as *const HostUiGuestApi as *mut HostUiGuestApi,
        Ordering::Release,
    );
    if IN_FRAME.load(Ordering::Acquire)
        && *OWNED_PRODUCERS
            .get_or_init(|| std::env::var("TSFP_HUD_PRODUCERS").map_or(true, |v| v == "1"))
        && !ownership().is_empty()
    {
        seed_weapon_state(api);
    }
    let requests = PENDING
        .lock()
        .map(|mut p| std::mem::take(&mut *p))
        .unwrap_or_default();
    let frame = HudFrame::read(&Guest(api)).and_then(|inputs| {
        let mut state = HUD_STATE
            .get_or_init(|| Mutex::new(hud_native::HudState::default()))
            .lock()
            .ok()?;
        state.publish_frame(inputs);
        state.frames[inputs.slot as usize]
    });
    let frames_seen = FRAMES.load(Ordering::Relaxed);
    // Evidence: health and armour changes with the frame number (first 200).
    static LAST_VITALS: Mutex<(f32, f32, u32)> = Mutex::new((0.0, 0.0, 0));
    if let (Some(f), Ok(mut last)) = (frame.as_ref(), LAST_VITALS.lock()) {
        if (f.health, f.armour) != (last.0, last.1) && last.2 < 200 {
            eprintln!(
                "tsfp-bevy: hud: vitals frame {frames_seen} health {}/{} armour {}/{}",
                f.health, f.max_health, f.armour, f.armour_max
            );
            *last = (f.health, f.armour, last.2 + 1);
        }
    }
    if frames_seen % 1800 == 900 {
        let brief: Vec<String> = requests
            .iter()
            .map(|r| {
                format!(
                    "{}:{:#x} reticle={:?} ammo={:?} vitals={}",
                    r.element.name(),
                    r.drawer,
                    r.reticle,
                    r.ammo,
                    r.vitals.is_some()
                )
            })
            .collect();
        eprintln!("tsfp-bevy: hud: state {frame:?}, requests {brief:?}");
    }
    if let Ok(mut published) = PUBLISHED.lock() {
        *published = frame.map(|f| (f, requests));
    }
    let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    if frames % 1800 == 0 {
        eprintln!(
            "tsfp-bevy: hud: {frames} frames, legacy drawer calls skipped {} passed {}",
            SKIPPED.load(Ordering::Relaxed),
            PASSED.load(Ordering::Relaxed)
        );
    }
}

/// Exit report line (evidence).
pub fn report() {
    crate::hud_producer_live::report();
    if ownership().is_empty() {
        return;
    }
    eprintln!(
        "tsfp-bevy: weapon events seen={:?} native={:?}",
        WEAPON_EVENTS_SEEN
            .each_ref()
            .map(|n| n.load(Ordering::Relaxed)),
        WEAPON_EVENTS_NATIVE
            .each_ref()
            .map(|n| n.load(Ordering::Relaxed))
    );
    eprintln!(
        "tsfp-bevy: hud: frames={} legacy drawer calls skipped={} passed={} reticle_left_to_legacy={} vitals_left_to_legacy={} hits_to_rust={} hit_sectors_drawn={} rank_boxes_drawn={} rank_left_to_legacy={} radars_drawn={} radar_left_to_legacy={}",
        FRAMES.load(Ordering::Relaxed),
        SKIPPED.load(Ordering::Relaxed),
        PASSED.load(Ordering::Relaxed),
        RETICLE_LEGACY.load(Ordering::Relaxed),
        VITALS_LEGACY.load(Ordering::Relaxed),
        HITS.load(Ordering::Relaxed),
        SECTORS_DRAWN.load(Ordering::Relaxed),
        RANKS_DRAWN.load(Ordering::Relaxed),
        RANK_LEGACY.load(Ordering::Relaxed),
        RADARS_DRAWN.load(Ordering::Relaxed),
        RADAR_LEGACY.load(Ordering::Relaxed)
    );
    if let Ok(stats) = RADAR_SHADOW_STATS.lock() {
        if stats.0 > 0 {
            eprintln!(
                "tsfp-bevy: hud: radar shadow check: calls={} blips_equal={} max_angle_error={} max_distance_error={} max_sweep_error={}",
                stats.0.saturating_sub(1),
                stats.1,
                stats.2,
                stats.3,
                stats.4
            );
        }
    }
    eprintln!(
        "tsfp-bevy: hud: prompts_drawn={} prompts_left_to_legacy={} effect_rows_drawn={} effects_left_to_legacy={}",
        PROMPTS_DRAWN.load(Ordering::Relaxed),
        PROMPT_LEGACY.load(Ordering::Relaxed),
        EFFECTS_DRAWN.load(Ordering::Relaxed),
        EFFECT_LEGACY.load(Ordering::Relaxed)
    );
    if let Ok(stats) = PROMPT_SHADOW_STATS.lock() {
        if stats.0 > 0 {
            eprintln!(
                "tsfp-bevy: hud: prompt shadow check: calls={} heights_equal={} phases_equal={} max_alpha_error={} max_timer_error={}",
                stats.0, stats.1, stats.2, stats.3, stats.4
            );
        }
    }
    let per_drawer: Vec<String> = HudElement::ALL
        .iter()
        .flat_map(|e| e.drawers().iter().map(move |va| (*e, *va)))
        .filter_map(|(e, va)| {
            let calls = CALLS[drawer_index(va)?].load(Ordering::Relaxed);
            let active = CENSUS_ACTIVE[drawer_index(va)?].load(Ordering::Relaxed);
            (calls > 0).then(|| {
                if active > 0 {
                    format!("{}:{va:#x}={calls}(active {active})", e.name())
                } else {
                    format!("{}:{va:#x}={calls}", e.name())
                }
            })
        })
        .collect();
    eprintln!(
        "tsfp-bevy: hud: legacy drawer calls per drawer: {}",
        per_drawer.join(" ")
    );
    let probe: Vec<String> = hud_native::CENSUS_DRAWERS
        .iter()
        .filter_map(|&va| {
            let n = PROBE_DRAWS[drawer_index(va)?].load(Ordering::Relaxed);
            (n > 0).then(|| format!("{va:#x}={n}"))
        })
        .collect();
    eprintln!(
        "tsfp-bevy: hud: draw probe (primitives after each census drawer): {}",
        if probe.is_empty() {
            "none".to_owned()
        } else {
            probe.join(" ")
        }
    );
    eprintln!(
        "tsfp-bevy: hud: message_queues_drawn={} messages_native={} messages_to_original_drawer={} messages_left_to_legacy={}",
        QUEUES_DRAWN.load(Ordering::Relaxed),
        MESSAGES_NATIVE.load(Ordering::Relaxed),
        MESSAGES_TO_ORIGINAL.load(Ordering::Relaxed),
        MESSAGE_LEGACY.load(Ordering::Relaxed)
    );
    if let Ok(stats) = MESSAGE_SHADOW_STATS.lock() {
        if stats.0 > 0 || stats.3 > 0 {
            eprintln!(
                "tsfp-bevy: hud: message shadow check: strings={} equal={} missing={} queue_calls_unchecked={} max_position_error={}",
                stats.0, stats.1, stats.2, stats.3, stats.4
            );
        }
    }
    eprintln!(
        "tsfp-bevy: hud: points_rows_drawn={} points_left_to_legacy={}",
        POINTS_DRAWN.load(Ordering::Relaxed),
        POINTS_LEGACY.load(Ordering::Relaxed)
    );
    eprintln!(
        "tsfp-bevy: hud: weapon_panels_drawn={} weapon_panels_visible={} weapons_left_to_legacy={} slide_targets={:?}",
        WEAPONS_DRAWN.load(Ordering::Relaxed),
        WEAPONS_VISIBLE.load(Ordering::Relaxed),
        WEAPON_LEGACY.load(Ordering::Relaxed),
        SLIDE_TARGETS.lock().map(|t| t.target).unwrap_or_default()
    );
    if let Ok(stats) = WEAPON_SHADOW_STATS.lock() {
        if stats.3 > 0 {
            eprintln!(
                "tsfp-bevy: hud: weapon shadow check: primitives={} equal={} missing={} panel_calls={} max_float_error={}",
                stats.0, stats.1, stats.2, stats.3, stats.4
            );
        }
    }
    let kinds: Vec<String> = QUEUE_KINDS
        .iter()
        .enumerate()
        .filter_map(|(kind, n)| {
            let n = n.load(Ordering::Relaxed);
            (n > 0).then(|| format!("{kind}={n}"))
        })
        .collect();
    eprintln!(
        "tsfp-bevy: hud: message queue entries drawn per kind (census): {}",
        kinds.join(" ")
    );
}

/// Put the native HUD into the engine frame (call once the wgpu renderer is chosen, before the
/// game starts). Nothing happens while the owner keeps the legacy HUD (`TSFP_HUD_NATIVE=off`).
pub fn install_in_frame() {
    let (owned, Some(assets)) = (ownership(), ASSETS.get()) else {
        return;
    };
    if owned.is_empty() {
        return;
    }
    let anchor = ANCHOR.get().copied().unwrap_or_default();
    let table = HudLayout::original();
    let layer = HudLayer::new(assets, move |width, height| {
        let (frame, requests) = PUBLISHED.lock().ok()?.clone()?;
        let space = HudSpace::for_window(width as f32, height as f32, anchor, table.safe_margin);
        match hud_native::layout(&frame, &requests, owned, &table, &space) {
            Ok(draw) => Some(draw),
            Err(why) => {
                static REPORTED: AtomicBool = AtomicBool::new(false);
                if !REPORTED.swap(true, Ordering::AcqRel) {
                    eprintln!("tsfp-bevy: hud: layout failed ({why:?}); nothing drawn this frame");
                }
                None
            }
        }
    });
    // Under the Rust UI page (order 10, ui_overlay.rs).
    tsfp_wgpu::frame_layer::install("hud", 0, Box::new(layer));
    IN_FRAME.store(true, Ordering::Release);
    eprintln!(
        "tsfp-bevy: hud: native HUD drawn INSIDE the engine frame by the wgpu renderer (after scene and post, under the UI page); \
         the legacy drawers of {:?} are skipped",
        owned.elements().map(HudElement::name).collect::<Vec<_>>()
    );
}
