// SPDX-License-Identifier: GPL-3.0-or-later
// Configuration is installed before guest threads start. Calls/counts are atomic.
use crate::{
    bridge::{Registers, execute},
    lane::{self, LaneCpu},
    lane_inv,
    memory::MappedGuest,
};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

/// T1854 lane roots plus the T2407 object-list append seam, in lane/export order. Kept here,
/// not in `lane.rs`, so the measured `lane.rs` bytes of the T1854 receipt stay unchanged.
pub const LANE_ROOTS: [u32; 45] = [
    0x45d60, 0x63bf0, 0xd76a0, 0x26b70, 0x427b0, 0x42c30, 0x431c0, 0x246d10, 0x190d60, 0x43a90,
    0x43af0, 0x3443f0, 0x1e8d0, 0x8e3e0, 0x387d0, 0x3b20f0, 0x3b2110, 0x3b2130, 0x3b2150, 0x35e590,
    0x356290, 0x356020, 0x3bc270, 0x3c6730, 0x3ba270, 0x3b5fe0, 0x3b3120, 0x33db0, 0x35e630,
    0x356320, 0x3568c0, 0x35e6a0, 0x35eaf0, 0x356990, 0x358130, 0x358160, 0x356080, 0x356870,
    0x358560, 0x3560e0, 0x359910, 0x15c4e0, 0x235600, 0x2d74b0, 0x15bca0,
];

/// The 24 four-register roots (T1815 pilots and wave 1 to 3), then the 45 lane roots (T1858/T2407)
/// and the 129 T1874 inventory and pose/control lane roots (`lane_inv::INV_ROOTS`).
pub const HOST_ROOTS: [u32; 24] = [
    0x12820, 0x71c70, 0xd2ce0, 0x1656d0, 0x168100, 0x228c20, 0x22df10, 0x23ef30, 0x243040,
    0x264740, 0x3bd2b0, 0x3bd2d0, 0x3bd300, 0x3c3cd0, 0x3c3d00, 0x3ea20, 0x942f0, 0x151090,
    0x31dcd0, 0x31dd10, 0x3b9f00, 0x3bb710, 0x3bb930, 0x3bd320,
];
const LANE_END: usize = 24 + LANE_ROOTS.len();
const INV_END: usize = LANE_END + lane_inv::INV_ROOTS.len();
/// T3765: the generated c2r lane roots (tsfp-lane-c2r) follow the tsfp-ports lane roots.
pub const TOTAL: usize = INV_END + tsfp_lane_c2r::ROOTS.len();
pub const ROOTS: [u32; TOTAL] = {
    let mut all = [0u32; TOTAL];
    let mut i = 0;
    while i < 24 {
        all[i] = HOST_ROOTS[i];
        i += 1;
    }
    while i < LANE_END {
        all[i] = LANE_ROOTS[i - 24];
        i += 1;
    }
    while i < INV_END {
        all[i] = lane_inv::INV_ROOTS[i - LANE_END];
        i += 1;
    }
    while i < TOTAL {
        all[i] = tsfp_lane_c2r::ROOTS[i - INV_END];
        i += 1;
    }
    all
};
/// Selection bits, one per root in `ROOTS` order, grown as roots were added (T1874 four words, T1990 six, T2026 seven,
/// T1996 twelve, T2126 fourteen, T2142 fifteen, T2157 eighteen, now 44).
pub const WORDS: usize = 55;
pub type Mask = [u64; WORDS];
const _: () = assert!(TOTAL <= 64 * WORDS);
/// T2143: compile-time hash of `ROOTS`; replaces the linear `position` scans of the per-call dispatcher.
pub static ROOT_MAP: crate::rootmap::RootMap<8192> = crate::rootmap::RootMap::build(&ROOTS);
static ENABLED: [AtomicU64; WORDS] = [const { AtomicU64::new(0) }; WORDS];
static RUST_CALLS: [AtomicU64; TOTAL] = [const { AtomicU64::new(0) }; TOTAL];
static C_CALLS: [AtomicU64; TOTAL] = [const { AtomicU64::new(0) }; TOTAL];
static REA_AI_SWITCH: AtomicBool = AtomicBool::new(false);

/// The mask with only bit `index` set.
pub const fn bit(index: usize) -> Mask {
    let mut mask = [0u64; WORDS];
    mask[index / 64] = 1u64 << (index % 64);
    mask
}

/// T2143: named root groups for bisecting on a real host: `--rust-ports=all,-char,-physics` (or `group:char`). A group is the set of
/// roots one lane module serves; roots that are not in `ROOTS` (the weapon and player roots go through the weapon table, `weapons`
/// and `player` tokens) are ignored. `anim` is the T2031 `-anim` token.
fn group_lists(name: &[u8]) -> Option<&'static [&'static [u32]]> {
    use crate::{
        anim_lane, anim_lane2, anim_lane3, audio_lane, char_lane, cutscene_lane, hud_lane,
        input_lane, inventory, inventory_ammo, inventory_grant, inventory_pickup, inventory_select,
        inventory_slots, leaf_lane, mapmaker_lane, modes_lane, modes_lane2, mp_lane, npc_lane,
        object_lane, physics_lane, physics_lane2, pose, pose_anim, pose_leaf, render_lane,
        render_lane2, save_lane, state_lane,
    };
    Some(match name {
        b"anim" => &[
            &anim_lane::ROOTS,
            &anim_lane2::ROOTS,
            &anim_lane3::ROOTS,
            &anim_lane3::ROOTS_MORE,
        ],
        b"physics" => &[&physics_lane::ROOTS, &physics_lane2::ROOTS],
        b"char" => &[&char_lane::ROOTS],
        b"npc" => &[&npc_lane::ROOTS],
        b"inventory" => &[
            &inventory::ROOTS,
            &inventory_select::ROOTS,
            &inventory_grant::ROOTS,
            &inventory_ammo::ROOTS,
            &inventory_slots::ROOTS,
            &inventory_pickup::ROOTS,
        ],
        b"cutscene" => &[
            &cutscene_lane::ROOTS,
            &crate::cutscene_presentation_lane::ROOTS,
        ],
        b"save" => &[&save_lane::ROOTS],
        b"audio" => &[&audio_lane::ROOTS],
        b"render" => &[&render_lane::ROOTS, &render_lane2::ROOTS],
        // T3502: the effect and particle roots, derived from the modules that own them (`play.sh --lifted-effects`).
        b"effects" => &[
            &crate::effect::ROOTS,
            &crate::effect_setup::ROOTS,
            &crate::emitter::ROOTS,
            &crate::particle::ROOTS,
            &tsfp_port_engine_render_effects_live_return::ROOTS,
        ],
        b"hud" => &[&hud_lane::ROOTS],
        b"input" => &[&input_lane::ROOTS],
        b"object" => &[&object_lane::ROOTS, &crate::rea_object_runtime::ROOTS],
        b"pose" => &[&pose::ROOTS, &pose_anim::ROOTS, &pose_leaf::ROOTS],
        b"leaf" => &[&leaf_lane::ROOTS],
        b"modes" => &[
            &modes_lane::ROOTS,
            &modes_lane2::ROOTS,
            &mp_lane::ROOTS,
            &mapmaker_lane::ROOTS,
            &crate::mapmaker_hud_panels::ROOTS,
            &crate::mapmaker_overlay::ROOTS,
            &crate::story_sequencer_lane::ROOTS,
        ],
        b"state" => &[&state_lane::ROOTS],
        _ => return None,
    })
}

/// The group names `group_lists` knows, for the error text of the host (`tsfp_rust_group_help`).
pub const GROUP_NAMES: [&str; 18] = [
    "anim",
    "physics",
    "char",
    "npc",
    "inventory",
    "cutscene",
    "save",
    "audio",
    "render",
    "effects",
    "hud",
    "input",
    "object",
    "pose",
    "leaf",
    "modes",
    "state",
    "group:<name> also accepted",
];
static GROUP_HELP: &[u8] = b"anim, physics, char, npc, inventory, cutscene, save, audio, render, effects, hud, input, object, pose, leaf, modes, state\0";

/// The valid group names as a NUL-terminated C string (static).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_group_help() -> *const u8 {
    GROUP_HELP.as_ptr()
}

fn group_mask(name: &[u8]) -> Option<Mask> {
    let name = name.strip_prefix(b"group:").unwrap_or(name);
    let lists = group_lists(name)?;
    let mut mask = [0u64; WORDS];
    for list in lists {
        for va in *list {
            if let Some(index) = ROOT_MAP.find(*va) {
                mask[index / 64] |= 1u64 << (index % 64);
            }
        }
    }
    Some(mask)
}

/// T2031 bisect aid: `all,-anim,-0x5a2f0` selects everything except the named roots (`-anim` is every animation lane root, `-<hex>` one
/// root). Positive tokens (`all` or hex VAs) are unioned, then the `-` tokens are removed. At least one positive token is required.
fn parse_with_exclusions(spec: &[u8]) -> Option<Mask> {
    let mut selected = [0u64; WORDS];
    let mut removed = [0u64; WORDS];
    let mut positives = 0;
    for token in spec.split(|&b| b == b',') {
        if let Some(name) = token.strip_prefix(b"-") {
            let mask = match group_mask(name) {
                Some(mask) => mask,
                None => parse_selection(name).filter(|_| name != b"all" && name != b"off")?,
            };
            for (word, bits) in removed.iter_mut().zip(mask) {
                *word |= bits;
            }
        } else {
            let mask = parse_selection(token)?;
            positives += 1;
            for (word, bits) in selected.iter_mut().zip(mask) {
                *word |= bits;
            }
        }
    }
    if positives == 0 {
        return None;
    }
    for (word, bits) in selected.iter_mut().zip(removed) {
        *word &= !bits;
    }
    Some(selected)
}

/// T3773: `all` is the adopted roots (HOST, LANE, INV, `c2r_adopted_roots.in`); other c2r rows need their explicit VA.
pub fn adopted_mask() -> Mask {
    let mut mask = [0u64; WORDS];
    for index in (0..INV_END).chain(C2R_ADOPTED.iter().filter_map(|va| ROOT_MAP.find(*va))) {
        mask[index / 64] |= 1u64 << (index % 64);
    }
    mask
}
const C2R_ADOPTED: &[u32] = &include!("../c2r_adopted_roots.in");

pub fn parse_selection(spec: &[u8]) -> Option<Mask> {
    if spec.contains(&b'-') {
        return parse_with_exclusions(spec);
    }
    if spec == b"off" {
        return Some([0; WORDS]);
    }
    if spec == b"all" {
        return Some(adopted_mask());
    }
    let mut mask = [0u64; WORDS];
    for token in spec.split(|&b| b == b',') {
        if let Some(group) = group_mask(token) {
            for (word, bits) in mask.iter_mut().zip(group) {
                *word |= bits;
            }
            continue;
        }
        let digits = token
            .strip_prefix(b"0x")
            .or_else(|| token.strip_prefix(b"0X"))
            .unwrap_or(token);
        if digits.is_empty() || digits.len() > 8 {
            return None;
        }
        let mut va = 0u32;
        for &b in digits {
            let digit = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return None,
            };
            va = (va << 4) | u32::from(digit);
        }
        let index = ROOTS.iter().position(|&root| root == va)?;
        if mask[index / 64] & (1u64 << (index % 64)) != 0 {
            return None;
        }
        mask[index / 64] |= 1u64 << (index % 64);
    }
    Some(mask)
}

/// # Safety
/// `spec` points to `len` initialized bytes. Call only before guest threads start.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_configure(spec: *const u8, len: usize) -> bool {
    let Some(mask) = parse_selection(unsafe { core::slice::from_raw_parts(spec, len) }) else {
        return false;
    };
    for count in RUST_CALLS.iter().chain(C_CALLS.iter()) {
        count.store(0, Ordering::Relaxed);
    }
    for word in 1..WORDS {
        ENABLED[word].store(mask[word], Ordering::Relaxed);
    }
    ENABLED[0].store(mask[0], Ordering::Release);
    true
}

/// T2385: opt-in adoption of the readable REA owners. The initial admitted
/// boundary is 0x266250; require its ordinary Rust lane root to be selected too.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_rea_ai_configure(enabled: bool) -> bool {
    if enabled {
        let Some(index) = ROOT_MAP.find(0x266250) else {
            return false;
        };
        if !selected(index) {
            return false;
        }
    }
    REA_AI_SWITCH.store(enabled, Ordering::Release);
    crate::rea_ai_runtime::configure(enabled);
    true
}

/// Number of supported REA AI adapters (legacy count ABI).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_rea_ai_adapter_count() -> u32 {
    crate::rea_ai_runtime::ADAPTER_COUNT
}

pub extern "C" fn tsfp_rust_rea_ai_enabled() -> bool {
    REA_AI_SWITCH.load(Ordering::Acquire)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_rea_ai_adapter_counts() -> u64 {
    (u64::from(crate::rea_ai_runtime::default_adapter_count()) << 32)
        | u64::from(crate::rea_ai_runtime::adapter_count())
}

/// T2385 diagnostic A/B gate for one documented runtime adapter address.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_rea_ai_function_configure(va: u32, enabled: bool) -> bool {
    crate::rea_ai_runtime::configure_function(va, enabled)
}

/// T2135 phase 3: `--retire-gamestate` (0 off, 1 own, 2 audit). Call after `tsfp_rust_configure`. Mode 1 needs every root
/// of `state_wire::REQUIRED_ROOTS` selected: otherwise returns false and writes the first missing VA to `*missing`.
///
/// # Safety
/// `missing` is null or points to one writable u32. Call only before guest threads start.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_retire_configure(mode: u32, missing: *mut u32) -> bool {
    if mode > 2 {
        return false;
    }
    let is_selected = |va: u32| {
        ROOTS
            .iter()
            .position(|&root| root == va)
            .is_some_and(selected)
            || crate::weapons_lane::is_selected(va)
    };
    match crate::state_wire::configure(mode as u8, is_selected) {
        Ok(()) => true,
        Err(va) => {
            if !missing.is_null() {
                unsafe { *missing = va };
            }
            false
        }
    }
}

/// T2138: `--retire-player` (0 off, 1 own, 2 audit). Call after `tsfp_rust_configure`. Mode 1 needs every root of
/// `player_wire::REQUIRED_ROOTS` selected: otherwise returns false and writes the first missing VA to `*missing`.
///
/// # Safety
/// `missing` is null or points to one writable u32. Call only before guest threads start.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_retire_player_configure(mode: u32, missing: *mut u32) -> bool {
    if mode > 2 {
        return false;
    }
    let is_selected = |va: u32| {
        ROOTS
            .iter()
            .position(|&root| root == va)
            .is_some_and(selected)
            || crate::weapons_lane::is_selected(va)
    };
    match crate::player_wire::configure(mode as u8, is_selected) {
        Ok(()) => true,
        Err(va) => {
            if !missing.is_null() {
                unsafe { *missing = va };
            }
            false
        }
    }
}

/// T2200: true when `va` is served by Rust under the current `--rust-ports` (pilot list or the weapons and player lane).
pub(crate) fn root_selected(va: u32) -> bool {
    ROOTS
        .iter()
        .position(|&root| root == va)
        .is_some_and(selected)
        || crate::weapons_lane::is_selected(va)
}
fn selected(index: usize) -> bool {
    let first = ENABLED[0].load(Ordering::Acquire);
    let word = if index < 64 {
        first
    } else {
        ENABLED[index / 64].load(Ordering::Relaxed)
    };
    word & (1u64 << (index % 64)) != 0
        || ROOTS
            .get(index)
            .is_some_and(|root| crate::weapon_hands_runtime::selects_root(*root))
}

/// T2145: ring of the last 32 root indexes entered (any thread), read by the crash report and the panic hook. Relaxed, about 1 ns.
const RING: usize = 32;
static RECENT: [AtomicU32; RING] = [const { AtomicU32::new(u32::MAX) }; RING];
static RECENT_AT: AtomicUsize = AtomicUsize::new(0);

/// Writes up to `max` of the most recent root VAs, oldest first, to `out`; returns how many.
///
/// # Safety
/// `out` has room for `max` u32.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_last_roots(out: *mut u32, max: usize) -> usize {
    let end = RECENT_AT.load(Ordering::Relaxed);
    let mut written = 0;
    for k in (0..RING.min(max)).rev() {
        let index = RECENT[end.wrapping_sub(k + 1) % RING].load(Ordering::Relaxed);
        if let Some(&va) = ROOTS.get(index as usize) {
            unsafe { *out.add(written) = va };
            written += 1;
        }
    }
    written
}

fn account(index: usize) -> bool {
    RECENT[RECENT_AT.fetch_add(1, Ordering::Relaxed) % RING].store(index as u32, Ordering::Relaxed);
    if !selected(index) {
        C_CALLS[index].fetch_add(1, Ordering::Relaxed);
        return false;
    }
    RUST_CALLS[index].fetch_add(1, Ordering::Relaxed);
    true
}

/// # Safety
/// `registers` is a live Registers object; offset maps the guest address window.
/// Only the 24 four-register roots: a lane root is refused here (`tsfp_rust_try_lane`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_try(va: u32, registers: *mut Registers, offset: usize) -> bool {
    let Some(index) = ROOT_MAP.find(va).filter(|&index| index < 24) else {
        return false;
    };
    if !account(index) {
        return false;
    }
    let mut memory = unsafe { MappedGuest::new(offset) };
    execute(va, &mut memory, unsafe { &mut *registers });
    true
}

/// T1858: lane roots take the whole register file and may call lifted guest code through
/// `tsfp_lane_guest_call` (defined by `src/host/rust_lane_call.c`). T1874: the inventory and
/// pose/control roots go through `lane_inv`, which also marshals x87/XMM and DF.
///
/// # Safety
/// `cpu` is a live LaneCpu (eax ecx edx ebx esi edi ebp esp); offset maps the guest window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_try_lane(va: u32, cpu: *mut LaneCpu, offset: usize) -> bool {
    // T2143: one hash probe instead of a scan of the lane roots and then of the inventory and pose roots.
    let Some(global) = ROOT_MAP.find(va) else {
        return false;
    };
    if (24..LANE_END).contains(&global) {
        let index = global - 24;
        if !account(24 + index) {
            return false;
        }
        let mut memory = unsafe { MappedGuest::new(offset) };
        lane::execute(va, &mut memory, unsafe { &mut *cpu });
        return true;
    }
    // T1918: the animation shadow mode runs the retained lifted original of `va` by calling the guest function once.
    if crate::anim_lane::declined(va)
        || crate::cutscene_presentation_lane::declined(va)
        || crate::mapmaker_hud_panels::declined(va)
        || crate::mapmaker_overlay::declined(va)
        || crate::story_sequencer_lane::declined(va)
        || crate::rea_object_runtime::declined(va)
    {
        return false;
    }
    if global < LANE_END {
        return false;
    }
    let index = global - LANE_END;
    if !account(LANE_END + index) {
        return false;
    }
    let mut memory = unsafe { MappedGuest::new(offset) };
    let cpu = unsafe { &mut *cpu };
    let profile_record =
        (va == 0x25ec70 && crate::weapon_hands_runtime::enabled()).then_some(cpu.eax);
    let handled = lane_inv::execute(va, &mut memory, cpu);
    if handled {
        if let Some(player_record) = profile_record {
            // TEMPORARY-SHIM (T2411): import the measured restore result after the translated
            // lane body until the native profile/save resolver supplies typed values.
            let _ = crate::weapon_hands_runtime::import_profile_restore_from_guest(
                &memory,
                player_record,
            );
        }
    }
    handled
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_count(va: u32, rust: bool) -> u64 {
    ROOT_MAP.find(va).map_or(0, |index| {
        if rust {
            &RUST_CALLS[index]
        } else {
            &C_CALLS[index]
        }
        .load(Ordering::Relaxed)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_root(index: usize) -> u32 {
    ROOTS.get(index).copied().unwrap_or(0)
}

/// A default-off A/B switch that keeps `roots` on the retained original (T3738 compare v8).
pub struct OptIn {
    pub env: &'static core::ffi::CStr,
    pub on: &'static core::ffi::CStr,
    pub roots: &'static [u32],
}

/// Every module switch that declines its roots at the selector; each module owns its `OPT_IN`
/// (tests/test_t2capture_opt_in.py: every `declined` module is listed).
pub const OPT_INS: [&OptIn; 6] = [
    &crate::cutscene_presentation_lane::OPT_IN,
    &crate::mapmaker_hud_panels::OPT_IN,
    &crate::mapmaker_overlay::OPT_IN,
    &crate::rea_object_runtime::OPT_IN,
    &crate::story_objective_admission_lane::OPT_IN,
    &crate::story_sequencer_lane::OPT_IN,
];

/// The `index`-th switch gating `va` (env name, enabling value), 0 when none. Read only through
/// `tsfp_t2o_host_api` (version 5) by the `--t2-offline` host; normal play never calls it.
///
/// # Safety
/// `env` and `value` are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_ports_opt_in(
    va: u32,
    index: u32,
    env: *mut *const core::ffi::c_char,
    value: *mut *const core::ffi::c_char,
) -> i32 {
    let mut gating = OPT_INS.iter().filter(|switch| switch.roots.contains(&va));
    let Some(switch) = gating.nth(index as usize) else {
        return 0;
    };
    unsafe {
        *env = switch.env.as_ptr();
        *value = switch.on.as_ptr();
    }
    1
}

#[cfg(test)]
mod root_map_tests {
    use super::*;

    /// T2143: the hash lookup must give what `position` gave: every root at its own index, no VA twice.
    #[test]
    fn groups_select_their_roots_and_the_help_names_them() {
        let all = parse_selection(b"all").unwrap();
        for name in &GROUP_NAMES[..17] {
            let group = parse_selection(name.as_bytes()).unwrap_or_else(|| panic!("group {name}"));
            assert!(
                group.iter().any(|w| *w != 0),
                "group {name} selects nothing"
            );
            let without = parse_selection(format!("all,-{name}").as_bytes()).unwrap();
            let removed: u32 = all
                .iter()
                .zip(without)
                .map(|(a, w)| (a & !w).count_ones())
                .sum();
            let size: u32 = group.iter().map(|w| w.count_ones()).sum();
            assert_eq!(removed, size, "all,-{name}");
            assert!(
                GROUP_HELP.windows(name.len()).any(|w| w == name.as_bytes()),
                "help names {name}"
            );
        }
        // T3502: `effects` is exactly the union of the effect and particle modules' roots, all known to the registry.
        let mut expected = [0u64; WORDS];
        for list in group_lists(b"effects").unwrap() {
            for va in *list {
                let index = ROOT_MAP
                    .find(*va)
                    .unwrap_or_else(|| panic!("effects root {va:#x} not in the registry"));
                expected[index / 64] |= 1u64 << (index % 64);
            }
        }
        assert_eq!(parse_selection(b"effects"), Some(expected));
        assert!(expected.iter().map(|w| w.count_ones()).sum::<u32>() >= 10);
        assert!(parse_selection(b"anim,0x45d60").is_some());
        assert!(parse_selection(b"nonsense").is_none());
        assert_eq!(parse_selection(b"group:char"), parse_selection(b"char"));
    }

    #[test]
    fn rea_actor_append_is_a_selected_host_root() {
        let index = ROOT_MAP.find(0x15bca0).expect("REA actor append root");
        let all = parse_selection(b"all").unwrap();
        assert_ne!(all[index / 64] & (1u64 << (index % 64)), 0);
    }

    /// T2145: the crash report's ring lists the last entered roots, oldest first, and wraps without losing order.
    #[test]
    fn recent_ring_reports_last_roots_oldest_first() {
        for index in 0..(RING + 5) {
            RECENT[RECENT_AT.fetch_add(1, Ordering::Relaxed) % RING]
                .store((index % 24) as u32, Ordering::Relaxed);
        }
        let mut out = [0u32; RING];
        let count = unsafe { tsfp_rust_last_roots(out.as_mut_ptr(), RING) };
        assert_eq!(count, RING);
        let expected: Vec<u32> = (5..RING + 5).map(|i| ROOTS[i % 24]).collect();
        assert_eq!(&out[..count], &expected[..]);
        assert_eq!(unsafe { tsfp_rust_last_roots(out.as_mut_ptr(), 2) }, 2);
        assert_eq!(&out[..2], &expected[RING - 2..]);
    }

    #[test]
    fn root_map_matches_the_linear_scan() {
        for (index, &va) in ROOTS.iter().enumerate() {
            assert_eq!(ROOT_MAP.find(va), Some(index), "root {va:#x}");
        }
        assert_eq!(ROOT_MAP.find(0xDEAD_0000), None);
        assert_eq!(ROOT_MAP.find(0), None);
    }
}

/// T3840 (rule 24, I-T3840-1): the default-on root set, derived by running the selection code on the
/// owner's `play.sh` default `--rust-ports` spec, must equal `tools/data/default_on_roots.json`.
/// `tools.agents.default_on_guard` refuses a landing that grows that snapshot without an adoption
/// receipt in `tools/data/adoptions.json`. Re-bless: `TSFP_BLESS_DEFAULT_ON=1 cargo test --lib default_on`.
#[cfg(test)]
mod default_on_snapshot {
    use super::*;
    use std::{collections::BTreeSet, path::PathBuf, string::String, vec::Vec};

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The `default)` variant's `--rust-ports=` value with `$blocked_roots` expanded.
    fn play_default_spec() -> String {
        let path = repo().join("tools/dev/play.sh");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        let quoted = |line: &str, key: &str| -> Option<String> {
            let start = line.find(key)? + key.len();
            Some(line[start..].split('"').next()?.into())
        };
        let blocked = text
            .lines()
            .find_map(|l| {
                l.strip_prefix("blocked_roots=")
                    .and_then(|_| quoted(l, "blocked_roots=\""))
            })
            .expect("play.sh: no blocked_roots= line");
        let spec = text
            .lines()
            .filter(|l| l.trim_start().starts_with("default)"))
            .find_map(|l| quoted(l, "ports=\"--rust-ports="))
            .expect("play.sh: no default) variant with ports=\"--rust-ports=...\"");
        spec.replace("$blocked_roots", &blocked)
    }

    /// Splits the spec the way `host_rust_ports_configure` does: weapon tokens to the weapon
    /// lane (`all` adds `settings` unless `-settings`), everything else to `parse_selection`.
    fn default_on() -> BTreeSet<u32> {
        let spec = play_default_spec();
        let weapon_words = [
            "weapons",
            "player",
            "settings",
            "player-interaction",
            "pickup-tick",
        ];
        let (mut pilot, mut weapon) = (Vec::new(), Vec::new());
        let tokens: Vec<&str> = spec.split(',').collect();
        for token in &tokens {
            let va = u32::from_str_radix(token.trim_start_matches("0x"), 16).ok();
            let is_weapon = weapon_words.contains(token)
                || va.is_some_and(|va| crate::weapons_lane::tsfp_rust_is_weapon_root(va));
            if *token != "-settings" {
                if is_weapon {
                    weapon.push(*token)
                } else {
                    pilot.push(*token)
                }
            }
        }
        if tokens.contains(&"all") && !tokens.contains(&"-settings") {
            weapon.push("settings");
        }
        let mut set = BTreeSet::new();
        let mask = parse_selection(pilot.join(",").as_bytes()).expect("pilot spec parses");
        set.extend(
            (0..TOTAL)
                .filter(|i| mask[i / 64] >> (i % 64) & 1 != 0)
                .map(|i| ROOTS[i]),
        );
        if !weapon.is_empty() {
            let mask =
                crate::weapons_lane::parse(weapon.join(",").as_bytes()).expect("weapon spec");
            let bits = (0..mask.len() * 64).filter(|i| mask[i / 64] >> (i % 64) & 1 != 0);
            set.extend(bits.filter_map(crate::weapons_lane::root_at));
        }
        set
    }

    #[test]
    fn default_on_roots_match_the_tracked_snapshot() {
        let roots = default_on();
        assert!(
            roots.len() > 100,
            "default-on set suspiciously small: {}",
            roots.len()
        );
        let lines: Vec<String> = roots
            .iter()
            .map(|va| std::format!("  \"{va:#x}\""))
            .collect();
        let text = std::format!(
            "{{\n \"_doc\": \"T3840 generated by crates/tsfp-ports/src/runtime.rs default_on_snapshot (TSFP_BLESS_DEFAULT_ON=1 cargo test --lib default_on): VAs the play.sh default --rust-ports spec selects. Additions need tools/data/adoptions.json (rule 24).\",\n \"spec\": \"{}\",\n \"roots\": [\n{}\n ]\n}}\n",
            play_default_spec(),
            lines.join(",\n")
        );
        let path = repo().join("tools/data/default_on_roots.json");
        if std::env::var_os("TSFP_BLESS_DEFAULT_ON").is_some() {
            std::fs::write(&path, &text).unwrap();
        }
        let tracked = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            tracked == text,
            "default-on roots differ from {path:?}; re-bless with TSFP_BLESS_DEFAULT_ON=1 and cite \
             an adoption receipt for every added VA in tools/data/adoptions.json (rule 24, T3840)"
        );
    }
}
