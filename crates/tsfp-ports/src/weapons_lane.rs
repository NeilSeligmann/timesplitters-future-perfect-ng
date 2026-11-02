// SPDX-License-Identifier: GPL-3.0-or-later
//! T1967: opt-in host selection of the 120 c2r weapon roots (T1940 113, T1953 5 + 2 hand ports).
//! Separate from `runtime::ROOTS` (never part of `all`, no 256-bit mask change): the host
//! (`src/host/rust_ports.c`, `TSFP_HAVE_WEAPON_ROOTS`) forwards the spec tokens that name one of
//! these VAs, or the word `weapons`, to `tsfp_rust_weapon_configure`, and calls
//! `tsfp_rust_try_weapon` from the same `game_rust_try` hook as the lane roots. Dispatch goes
//! through `weapons_remainder_system::run` (per-function switch) then the T1953 modules.
//! The c2r bodies end with ESP at the entry value (return slot not popped, `ret N` models
//! `esp += N - 4`); the host adapter pops, so ESP is normalised to the entry value and the
//! number of roots that returned another ESP is counted (`tsfp_rust_weapon_esp_odd`).
//! T3398: the pickup tick `0xC1850` (`pickup_tick`, word `pickup-tick`) is the last lane root group.
//! T2021: the same opt-in lane also carries the 249 gameplay/player roots (248 c2r plus T2034 0x23A040) (`PLAYER_ROOTS`, word
//! `player`), dispatched through `player_system::run`, so the player system can be traced (Tier 2).
use crate::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    lane::LaneCpu,
    memory::MappedGuest,
};
use core::sync::atomic::{AtomicU64, Ordering};

unsafe extern "C" {
    fn tsfp_lane_x_fp_load(regs: *mut ExtRegisters);
    fn tsfp_lane_x_fp_store(regs: *const ExtRegisters);
    fn tsfp_lane_x_call(va: u32, regs: *mut ExtRegisters);
}
struct WeaponGuest;
impl GuestCall for WeaponGuest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        let mut memory = unsafe { MappedGuest::new(crate::raw_look::offset()) };
        // T3499/T2411: selected player roots can call the REA inventory owner through this
        // adapter. Keep its opt-in switch and typed boundary consistent with lane_inv roots.
        if crate::lane_inv::dispatch_inventory_owner(va, &mut memory, self, regs) {
            return;
        }
        if tsfp_port_gameplay_player::interaction::dispatch_nested(
            va,
            crate::player_interaction_runtime::enabled(),
            &mut memory,
            self,
            regs,
            crate::player_interaction_runtime::run_nested_targeting_gate,
        ) {
            return;
        }
        // T3398: the selected D0730 player root can reach D01C0 through this weapon/player
        // GuestCall adapter. Keep the lock-on attachment on the same default-off seam used by
        // LaneGuest, and preserve the lifted ABI outputs for the reference projection.
        if va == 0x000d_01c0 {
            let args_esp = regs.esp;
            if crate::player_interaction_runtime::run_nested(va, &mut memory, self, regs) {
                return;
            }
            unsafe { tsfp_lane_x_call(va, regs) };
            crate::player_interaction_runtime::record_lifted_reference(&memory, args_esp, regs.eax);
            return;
        }
        if va == 0x000d_00f0
            && crate::player_interaction_runtime::run_nested_candidate_filter(
                va,
                &mut memory,
                self,
                regs,
            )
        {
            return;
        }
        if crate::weapon_hands_runtime::dispatch_nested_guest_call(va, &mut memory, self, regs) {
            return;
        }
        let equipped_once_target = if crate::weapon_hands_runtime::enabled() {
            crate::weapon_hands_runtime::equipped_once_effect_target(va, &memory, regs)
        } else {
            None
        };
        // T2161: the player roots (0xD5EB0 and its 0x25D520 look call) run in this lane, so the raw look hooks sit here too.
        if crate::raw_look::watches(va) {
            crate::raw_look::around(va, false, &mut memory, self, regs, |_, _, regs| {
                unsafe { tsfp_lane_x_call(va, regs) };
                true
            });
        } else {
            unsafe { tsfp_lane_x_call(va, regs) }
        }
        if let Some((ext, entry)) = equipped_once_target {
            // TEMPORARY-SHIM: retain the measured post-callback owner update while the remainder
            // of the hand transition and its presentation delegates remain in the hybrid lane.
            let _ = crate::weapon_hands_runtime::mark_equipped_once(&mut memory, ext, entry);
        }
    }
}

/// The 120 roots: `c2r::t1940_weapons::run`, `t1953_weapons::run`, `t1953_manual::run` in table order.
pub const WEAPON_ROOTS: [u32; 120] = [
    0x1c0fa0, 0x1c13c0, 0x1c16e0, 0x1c17b0, 0x1c1ab0, 0x1c1c50, 0x1c1e10, 0x1c2090, 0x1c2af0,
    0x1c4ce0, 0x1c4dc0, 0x1c4e50, 0x1c4f60, 0x1c5260, 0x1c52c0, 0x1c5450, 0x1c60b0, 0x1c6160,
    0x1c61f0, 0x1c6260, 0x1c63d0, 0x1c64b0, 0x1c66e0, 0x1c6780, 0x1c6800, 0x1c68e0, 0x1c69c0,
    0x1c6c00, 0x1c6e40, 0x1c6ee0, 0x1c7c52, 0x1c7f20, 0x1c8010, 0x1c83a0, 0x1dde30, 0x1de020,
    0x1df2f0, 0x1e04f0, 0x1e2020, 0x1e21e0, 0x1e8d00, 0x1eca60, 0x1ed340, 0x1ef840, 0x1f0700,
    0x1f0f30, 0x1f41e0, 0x1f90d0, 0x1f9da0, 0x1fa6a0, 0x1fbe20, 0x1fd570, 0x2014c0, 0x2023a0,
    0x203250, 0x205460, 0x205b20, 0x207590, 0x2075b0, 0x2084c0, 0x2085b0, 0x208ac0, 0x20a310,
    0x20b120, 0x20b3b0, 0x20b8c0, 0x20ea60, 0x2106a0, 0x213710, 0x221210, 0x222340, 0x228e90,
    0x22bdf0, 0x22dd10, 0x22f710, 0x2533f0, 0x253430, 0x2534b0, 0x2535b0, 0x253630, 0x253f10,
    0x253fc0, 0x255fd0, 0x27d510, 0x284980, 0x289690, 0x28b5f0, 0x28c2f0, 0x2936c0, 0x2b23d0,
    0x2b24e0, 0x312a60, 0x33c050, 0x33e680, 0x33ef90, 0x351260, 0x362b60, 0x362cc0, 0x362d00,
    0x362d30, 0x3630a0, 0x363260, 0x363290, 0x363350, 0x363660, 0x363b20, 0x363b30, 0x3644c0,
    0x364810, 0x364f20, 0x364f30, 0x365270, 0x365500, 0xc15b0, 0x1c55f0, 0x1c6310, 0x1c7e60,
    0x363d90, 0x20a8a0, 0x2214e0,
];
/// T2021: the 248 translated player roots, `c2r::gameplay_player{,_2,_3}::run` in table order (index 120 on), then T2034 0x23A040.
pub const PLAYER_ROOTS: [u32; 249] = [
    0x25eeb0, 0x6bb90, 0x2382b0, 0x8d2c0, 0xcffa0, 0xd8c80, 0xcf200, 0x1b9950, 0x1b9980, 0xd0090,
    0x8e5f0, 0x242e0, 0x11ce00, 0x1ce850, 0xd9230, 0xc5a90, 0xc5b50, 0x1b9920, 0x25c120, 0xcff20,
    0x9e2a0, 0x1b8950, 0xd2190, 0x25e650, 0x25fb70, 0xd2280, 0x1ce6a0, 0x25c650, 0xd1f80, 0x25a0c0,
    0x25a7f0, 0xd0730, 0xd19f0, 0x259e50, 0x9da10, 0xd1fd0, 0xd5c70, 0xcf8d0, 0x259d90, 0x25f0c0,
    0x25f220, 0x258910, 0x259930, 0x25a3b0, 0x25ce80, 0xa0d90, 0xd5eb0, 0x25d520, 0x25ea20,
    0xd2330, 0xd2b80, 0xd84c0, 0x258f50, 0xd01c0, 0x25a490, 0xd00f0, 0x9e6f0, 0x1c91c0, 0x1c4a50,
    0x1c4c60, 0x1b9300, 0x258ce0, 0x1b98f0, 0x1c8d70, 0xd2d50, 0xd4100, 0x1b97c0, 0x695c0, 0x69d90,
    0xc53f0, 0x2be850, 0xcc920, 0xcca10, 0xd1ec0, 0x11d2c0, 0xc61d0, 0xd6960, 0x150130, 0xd2fd0,
    0xd3b00, 0x25eec0, 0xd6870, 0xd7570, 0x1b5bb0, 0xcf230, 0x190eb0, 0x243700, 0x25a420, 0x58d30,
    0x65d20, 0x69620, 0xbdba0, 0xcb110, 0xcf060, 0xcf320, 0xd2de0, 0xd4f50, 0xd50e0, 0x1b7b80,
    0x238340, 0x238dd0, 0x25ee60, 0x34eee0, 0x462e0, 0x58c70, 0x640c0, 0x641c0, 0x64240, 0x64350,
    0x64460, 0x66070, 0x66170, 0x66310, 0x668f0, 0x6c600, 0x8d040, 0x9e2b0, 0xa0d30, 0xb2fd0,
    0xbe1d0, 0xbffe0, 0xc0f20, 0xcb100, 0xcf2d0, 0xcf370, 0xcf7b0, 0xcfe40, 0xd0160, 0xd0ce0,
    0xd0e10, 0xd1220, 0xd1650, 0xd1f10, 0xd3b90, 0xd3c50, 0xd3de0, 0xd3f80, 0xd3fe0, 0xd4610,
    0xd4820, 0xd4b00, 0xd4be0, 0xd4cf0, 0xd4db0, 0xd4ea0, 0xd5480, 0xd55f0, 0xd5af0, 0xd83d0,
    0xdb690, 0xdc5d0, 0xe91f0, 0xe9380, 0xe94e0, 0xea7a0, 0x1252c0, 0x1255e0, 0x12df90, 0x162250,
    0x1b6580, 0x1b6600, 0x1b7490, 0x1b75e0, 0x1b8b50, 0x1be7d0, 0x1d2160, 0x1d5d70, 0x1e5300,
    0x1e69e0, 0x208510, 0x208550, 0x20d3b0, 0x20ee40, 0x20fb50, 0x212060, 0x214f10, 0x225c70,
    0x226350, 0x2264f0, 0x2284d0, 0x229250, 0x229c30, 0x22b1a0, 0x22cb60, 0x22d570, 0x2384a0,
    0x2384d0, 0x238550, 0x238580, 0x238670, 0x238e00, 0x239590, 0x239680, 0x239710, 0x2397a0,
    0x239820, 0x23e930, 0x23ef50, 0x23ef80, 0x243920, 0x258b80, 0x259830, 0x25a780, 0x25b350,
    0x25b3e0, 0x25b7b0, 0x25c5c0, 0x25d270, 0x25dd70, 0x25e400, 0x25ec30, 0x25ee80, 0x25ef20,
    0x25ef80, 0x25f010, 0x25f250, 0x25f2a0, 0x264850, 0x2b0b80, 0x2cbda0, 0x2cd150, 0x2e7ad0,
    0x311800, 0x33e530, 0x345780, 0x3457e0, 0x345b10, 0x34e130, 0x34e400, 0x34e590, 0x34e6b0,
    0x34e920, 0x34ea40, 0x34eb00, 0x34ecb0, 0x34ed70, 0x34eff0, 0x34f5b0, 0x34fef0, 0x3501e0,
    0x350890, 0x3508b0, 0x358590, 0x37a570, 0x37a730, 0x37a7a0, 0x37a9b0, 0xd0e30, 0x23a040,
];
/// T2305: the 40 c2r ui/settings roots: `c2r::ui_settings::ROOTS` (37), `c2r::t1892_settings::ROOTS` (2) and
/// 0x2C3720 (music/sfx slider adjust, `c2r::ui_menu_3`). Group word `settings`; the host adds it to `all` (exclude with
/// `-settings`), so `--rust-ports all` runs them in Rust. Dispatched through `c2r::{ui_settings,t1892_settings,ui_menu_3}::run`.
pub const SETTINGS_ROOTS: [u32; 40] = [
    0x715c0, 0x71e60, 0x193bb0, 0x2c3ac0, 0x2c3bf0, 0x2c4190, 0x2cbe10, 0x2cbe50, 0x2cbf00,
    0x2cbfa0, 0x2cbfc0, 0x2d5e70, 0x2e7ca0, 0x2e7f10, 0x2e7fd0, 0x2e80a0, 0x2e8150, 0x2e83a0,
    0x2e8520, 0x2e8760, 0x2e87b0, 0x2e8840, 0x2e8d60, 0x2e8e40, 0x2e92a0, 0x2e9a20, 0x2e9a80,
    0x2e9bc0, 0x2e9ea0, 0x2e9ec0, 0x2e9fc0, 0x2ea080, 0x2ea210, 0x2ea250, 0x2ea3f0, 0x2ea8e0,
    0x2f4e80, 0x2e7e50, 0x2eab30, 0x2c3720,
];
/// T3398: the pickup tick root (`pickup_tick::ROOTS`), group word `pickup-tick`.
pub const PICKUP_ROOTS: [u32; 1] = crate::pickup_tick::ROOTS;
const PLAYER_END: usize = WEAPON_ROOTS.len() + PLAYER_ROOTS.len();
const SETTINGS_END: usize = PLAYER_END + SETTINGS_ROOTS.len();
const TOTAL: usize = SETTINGS_END + PICKUP_ROOTS.len();
const WORDS: usize = TOTAL.div_ceil(64);
static ENABLED: [AtomicU64; WORDS] = [const { AtomicU64::new(0) }; WORDS];
static RUST_CALLS: [AtomicU64; TOTAL] = [const { AtomicU64::new(0) }; TOTAL];
static C_CALLS: [AtomicU64; TOTAL] = [const { AtomicU64::new(0) }; TOTAL];
static ESP_ODD: AtomicU64 = AtomicU64::new(0);

pub(crate) fn root_at(index: usize) -> Option<u32> {
    WEAPON_ROOTS
        .get(index)
        .or_else(|| PLAYER_ROOTS.get(index.wrapping_sub(WEAPON_ROOTS.len())))
        .or_else(|| SETTINGS_ROOTS.get(index.wrapping_sub(PLAYER_END)))
        .or_else(|| PICKUP_ROOTS.get(index.wrapping_sub(SETTINGS_END)))
        .copied()
}
/// T2143: compile-time hash of the weapon then player roots, replaces a scan of all 369 per call (and per `is_weapon_root` probe).
const ALL_ROOTS: [u32; TOTAL] = {
    let mut all = [0u32; TOTAL];
    let mut index = 0;
    while index < WEAPON_ROOTS.len() {
        all[index] = WEAPON_ROOTS[index];
        index += 1;
    }
    while index < PLAYER_END {
        all[index] = PLAYER_ROOTS[index - WEAPON_ROOTS.len()];
        index += 1;
    }
    while index < SETTINGS_END {
        all[index] = SETTINGS_ROOTS[index - PLAYER_END];
        index += 1;
    }
    while index < TOTAL {
        all[index] = PICKUP_ROOTS[index - SETTINGS_END];
        index += 1;
    }
    all
};
static WEAPON_MAP: crate::rootmap::RootMap<1024> = crate::rootmap::RootMap::build(&ALL_ROOTS);
fn index_of(va: u32) -> Option<usize> {
    WEAPON_MAP.find(va)
}
/// True when `va` is a weapon or player lane root that the host selected (T2135: `--retire-gamestate` root check).
pub fn is_selected(va: u32) -> bool {
    index_of(va).is_some_and(selected)
}
fn selected(index: usize) -> bool {
    ENABLED[index / 64].load(Ordering::Relaxed) & (1u64 << (index % 64)) != 0
        || root_at(index).is_some_and(crate::weapon_hands_runtime::selects_root)
}

/// Parses `off` or a comma list of `weapons` (all 120), `player` (all 249),
/// `player-interaction` (the D0730 caller and D01C0 scanner), and root VAs (hex, optional `0x`).
pub fn parse(spec: &[u8]) -> Option<[u64; WORDS]> {
    let mut mask = [0u64; WORDS];
    if spec == b"off" {
        return Some(mask);
    }
    for token in spec.split(|&b| b == b',') {
        if token == b"player-interaction" {
            // The retained C2R D0730 body calls D01C0 through GuestCall. Select that caller so
            // the nested call crosses the typed D01C0 seam; selecting only D01C0 misses the
            // nested call in normal gameplay. T1680 supplies D01C0's ABI contract.
            for va in [0x000d_0730, 0x000d_01c0] {
                let index = index_of(va)?;
                mask[index / 64] |= 1u64 << (index % 64);
            }
            continue;
        }
        let group = match token {
            b"weapons" => Some(0..WEAPON_ROOTS.len()),
            b"player" => Some(WEAPON_ROOTS.len()..PLAYER_END),
            b"settings" => Some(PLAYER_END..SETTINGS_END),
            b"pickup-tick" => Some(SETTINGS_END..TOTAL),
            _ => None,
        };
        if let Some(group) = group {
            for index in group {
                mask[index / 64] |= 1u64 << (index % 64);
            }
            continue;
        }
        let digits = token.strip_prefix(b"0x").unwrap_or(token);
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
        let index = index_of(va)?;
        mask[index / 64] |= 1u64 << (index % 64);
    }
    Some(mask)
}

/// # Safety
/// `spec` points to `len` initialized bytes. Call only before guest threads start.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_weapon_configure(spec: *const u8, len: usize) -> bool {
    let spec = unsafe { core::slice::from_raw_parts(spec, len) };
    let Some(mask) = parse(spec) else {
        return false;
    };
    let player_interaction_enabled = spec
        .split(|&b| b == b',')
        .any(|token| token == b"player-interaction");
    for count in RUST_CALLS.iter().chain(C_CALLS.iter()) {
        count.store(0, Ordering::Relaxed);
    }
    ESP_ODD.store(0, Ordering::Relaxed);
    for (word, value) in mask.iter().enumerate() {
        ENABLED[word].store(*value, Ordering::Relaxed);
    }
    crate::player_interaction_runtime::configure(player_interaction_enabled);
    true
}

/// True when `va` is a weapon root (selected or not): the host uses it to route spec tokens.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_is_weapon_root(va: u32) -> bool {
    index_of(va).is_some()
}

/// T2143: handler remembered per root (see `lane_inv::HANDLER`), the four-way chain ran up to four failing `match va` per call.
static HANDLER: [core::sync::atomic::AtomicU8; TOTAL] =
    [const { core::sync::atomic::AtomicU8::new(0) }; TOTAL];

fn run_module(
    id: u8,
    va: u32,
    m: &mut MappedGuest,
    guest: &mut WeaponGuest,
    r: &mut ExtRegisters,
) -> bool {
    match id {
        0 => crate::player_system::run(va, true, m, guest, r),
        1 => crate::weapons_remainder_system::run(va, true, m, guest, r),
        2 => crate::c2r::t1953_weapons::run(va, m, guest, r),
        3 => crate::c2r::t1953_manual::run(va, m, guest, r),
        4 => crate::c2r::ui_settings::run(va, m, guest, r),
        5 => crate::c2r::t1892_settings::run(va, m, guest, r),
        6 => va == 0x2c3720 && crate::c2r::ui_menu_3::run(va, m, guest, r),
        7 => crate::pickup_tick::run(va, m, guest, r),
        _ => false,
    }
}

fn dispatch(va: u32, m: &mut MappedGuest, guest: &mut WeaponGuest, r: &mut ExtRegisters) -> bool {
    if crate::raw_look::watches(va) {
        return crate::raw_look::around(va, true, m, guest, r, |m, guest, r| {
            dispatch_modules(va, m, guest, r)
        });
    }
    dispatch_modules(va, m, guest, r)
}

fn dispatch_modules(
    va: u32,
    m: &mut MappedGuest,
    guest: &mut WeaponGuest,
    r: &mut ExtRegisters,
) -> bool {
    // T3499/T2411: 0xA0D30 is selected as a player root in this lane, so give the opted-in
    // REA interaction/inventory owner the direct root before the translated player module.
    if crate::lane_inv::dispatch_inventory_owner(va, m, guest, r) {
        return true;
    }
    use core::sync::atomic::Ordering::Relaxed;
    let slot = index_of(va);
    if let Some(slot) = slot {
        let cached = HANDLER[slot].load(Relaxed);
        if cached != 0 && run_module(cached - 1, va, m, guest, r) {
            return true;
        }
    }
    // T2305: settings roots start at module 4, the weapon and player modules never own them.
    let first = match slot {
        Some(s) if s >= SETTINGS_END => 7,
        Some(s) if s >= PLAYER_END => 4,
        _ => 0,
    };
    for id in first..8u8 {
        if run_module(id, va, m, guest, r) {
            if let Some(slot) = slot {
                HANDLER[slot].store(id + 1, Relaxed);
            }
            return true;
        }
    }
    false
}

/// # Safety
/// `cpu` is a live LaneCpu and `offset` maps the guest window.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_try_weapon(va: u32, cpu: *mut LaneCpu, offset: usize) -> bool {
    let Some(index) = index_of(va) else {
        return false;
    };
    if !selected(index) {
        C_CALLS[index].fetch_add(1, Ordering::Relaxed);
        return false;
    }
    RUST_CALLS[index].fetch_add(1, Ordering::Relaxed);
    let c = unsafe { &mut *cpu };
    let mut m = unsafe { MappedGuest::new(offset) };
    crate::raw_look::note_offset(offset);
    let mut r = ExtRegisters {
        eax: c.eax,
        ecx: c.ecx,
        edx: c.edx,
        esp: c.esp,
        ebx: c.ebx,
        ebp: c.ebp,
        esi: c.esi,
        edi: c.edi,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm { lanes: [0; 4] }; 5],
        xmm_hi: [Xmm { lanes: [0; 4] }; 3],
    };
    unsafe { tsfp_lane_x_fp_load(&mut r) };
    let mut guest = WeaponGuest;
    if !dispatch(va, &mut m, &mut guest, &mut r) {
        panic!("unregistered T1967 weapon root");
    }
    unsafe { tsfp_lane_x_fp_store(&r) };
    if r.esp != c.esp {
        ESP_ODD.fetch_add(1, Ordering::Relaxed);
    }
    c.eax = r.eax;
    c.ecx = r.ecx;
    c.edx = r.edx;
    c.ebx = r.ebx;
    c.esi = r.esi;
    c.edi = r.edi;
    c.ebp = r.ebp;
    // T2021: the T1923 mutants (EAX or newest written byte) also cover these roots in a mutation build.
    crate::lane_inv::mutate(va, &mut m, c);
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_weapon_count(va: u32, rust: bool) -> u64 {
    match index_of(va) {
        Some(i) => {
            if rust {
                RUST_CALLS[i].load(Ordering::Relaxed)
            } else {
                C_CALLS[i].load(Ordering::Relaxed)
            }
        }
        None => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_weapon_root(index: usize) -> u32 {
    root_at(index).unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_weapon_esp_odd() -> u64 {
    ESP_ODD.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selector() {
        assert_eq!(parse(b"off"), Some([0; WORDS]));
        let all = parse(b"weapons").unwrap();
        assert_eq!(all.iter().map(|w| w.count_ones()).sum::<u32>(), 120);
        let player = parse(b"player").unwrap();
        assert_eq!(player.iter().map(|w| w.count_ones()).sum::<u32>(), 249);
        let interaction = parse(b"player-interaction").unwrap();
        assert_eq!(interaction.iter().map(|w| w.count_ones()).sum::<u32>(), 2);
        for interaction_root in [0x000d_0730, 0x000d_01c0] {
            let index = index_of(interaction_root).unwrap();
            assert_ne!(interaction[index / 64] & (1u64 << (index % 64)), 0);
        }
        let both = parse(b"weapons,player,0x2382b0").unwrap();
        assert_eq!(
            both.iter().map(|w| w.count_ones()).sum::<u32>(),
            PLAYER_END as u32
        );
        assert_eq!(index_of(0x2382b0).map(|i| i >= 120), Some(true));
        assert_eq!(parse(b"0x1c0fa0").unwrap()[0], 1);
        assert!(parse(b"12820").is_none());
        let settings = parse(b"settings").unwrap();
        assert_eq!(
            settings.iter().map(|w| w.count_ones()).sum::<u32>(),
            SETTINGS_ROOTS.len() as u32
        );
        assert_eq!(index_of(0x2c3720), Some(SETTINGS_END - 1));
        let pickup = parse(b"pickup-tick").unwrap();
        assert_eq!(pickup.iter().map(|w| w.count_ones()).sum::<u32>(), 1);
        assert_eq!(parse(b"0xc1850"), Some(pickup));
        assert_eq!(index_of(0xc1850), Some(TOTAL - 1));
        let mut seen = WEAPON_ROOTS.to_vec();
        seen.extend(PLAYER_ROOTS);
        seen.extend(SETTINGS_ROOTS);
        seen.extend(PICKUP_ROOTS);
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), TOTAL);
    }

    /// T2305: every settings root is owned by its c2r module and is not also a pilot or lane root (double hook).
    #[test]
    fn settings_roots_dispatch() {
        for va in SETTINGS_ROOTS {
            let owned = crate::c2r::ui_settings::ROOTS.contains(&va)
                || crate::c2r::t1892_settings::ROOTS.contains(&va)
                || va == 0x2c3720;
            assert!(owned, "{va:#x}");
            assert!(
                !crate::runtime::ROOTS.contains(&va),
                "{va:#x} is also a runtime root"
            );
        }
    }
}
