// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/c2r/player_wired
// portreg: wraps=tsfp-port-gameplay-player/c2r/gameplay_player_3
//! T2194 native count mailbox and per-slot assist policy. The original raw-look
//! zero-stick/profile-overlay shim has been removed. ABI names remain for hosts.
//! Native handlers consume degrees and modify typed state; this module never
//! writes player, profile or input memory. docs/t-player-look.md.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
    player_wire as pw,
};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};

// Native degrees handler shared with tsfp-input. This mailbox does not project
// or write view/profile memory; the typed dispatcher owns its hybrid data boundary.
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-input/src/view.rs"]
mod view;
use view::{ViewAngles, ViewConstraints};
pub use view::{ViewAngles as NativeViewAngles, ViewConstraints as NativeViewConstraints};

pub const PLAYER_POINTER: u32 = 0x7b0c7c;
pub const CAMERA_POINTER: u32 = 0x7b0cc0;
pub const PITCH_LIMIT: u32 = 0x7b0c80;
pub const PROFILE_BASE: u32 = 0x7bedc4;
pub const PROFILE_SIZE: u32 = 0x1f54;
/// Auto Aim, Auto Lookahead and Inverse Look in `profile + 2`.
pub const ASSIST_BITS: u8 = 0x80 | 0x08 | 0x01;
pub const SLOTS: usize = 4;
const ON_FOOT: u32 = 2;

/// Native look ingress and historical scope identifiers (scope roots are no longer watched).
pub const ROOT_LOOK: u32 = 0x25d520;
pub const ROOTS_SCOPE: [u32; 5] = [0xd5eb0, 0xd0730, 0xd01c0, 0x25a7f0, 0xd0e30];

static ENABLED: AtomicU32 = AtomicU32::new(0);
/// Observe look-root calls while the interim stick owns look, without changing guest input or assist flags.
pub const MONITOR_ONLY: u32 = 1 << 31;
static INIT: AtomicBool = AtomicBool::new(false);
static OFFSET: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
const ZERO: AtomicU64 = AtomicU64::new(0);
static YAW: [AtomicU64; SLOTS] = [ZERO; SLOTS];
static PITCH: [AtomicU64; SLOTS] = [ZERO; SLOTS];
static STEPS: [AtomicU32; SLOTS] = [const { AtomicU32::new(0) }; SLOTS];
/// Native applied steps / ABI-compatible retired-overlay counter (always zero).
static APPLIED: AtomicU32 = AtomicU32::new(0);
static OVERLAID: AtomicU32 = AtomicU32::new(0);
/// T2200 self-check counters (see `tsfp_raw_look_diag`): `tsfp_raw_look_enable` calls, `0x25D520` calls seen for an enabled slot
/// (any state), those that kept the original path (not on foot, look-locked or no buffer), mailbox pushes.
static ENABLE_CALLS: AtomicU32 = AtomicU32::new(0);
static LOOK_CALLS: AtomicU32 = AtomicU32::new(0);
static LOOK_SKIPPED: AtomicU32 = AtomicU32::new(0);
static PUSHES: AtomicU32 = AtomicU32::new(0);

fn add_f64(cell: &AtomicU64, delta: f64) {
    let mut old = cell.load(Relaxed);
    while let Err(seen) = cell.compare_exchange_weak(
        old,
        (f64::from_bits(old) + delta).to_bits(),
        Relaxed,
        Relaxed,
    ) {
        old = seen;
    }
}

/// Enables raw look and the assist disable for the slots in `mask` (bit n = slot n). 0 turns everything off.
pub fn set_slots(mask: u32) {
    ENABLED.store(mask & (((1 << SLOTS) - 1) | MONITOR_ONLY), Relaxed);
}
/// T3723: slots whose player is mounted or possessed (turret, vehicle: state != ON_FOOT, not locked) at the last look
/// root call. Raw counts only turn the on-foot view, so the router keeps the virtual right stick for these slots.
static MOUNTED: AtomicU32 = AtomicU32::new(0);
pub fn enabled_mask() -> u32 {
    ENABLED.load(Relaxed)
}
/// Adds a router look delta (degrees, yaw positive = right, pitch positive = up) to the slot mailbox.
pub fn push(slot: usize, yaw_deg: f64, pitch_deg: f64) {
    if slot < SLOTS && yaw_deg.is_finite() && pitch_deg.is_finite() {
        if yaw_deg != 0.0 || pitch_deg != 0.0 {
            if latency_log() != 0 {
                let _ = FIRST_PUSH[slot].compare_exchange(0, monotonic_ns(), Relaxed, Relaxed);
            }
        }
        PUSHES.fetch_add(1, Relaxed);
        add_f64(&YAW[slot], yaw_deg);
        add_f64(&PITCH[slot], pitch_deg);
    }
}
/// Drains the mailbox of a slot.
pub fn take(slot: usize) -> (f64, f64) {
    FIRST_PUSH[slot].store(0, Relaxed);
    (
        f64::from_bits(YAW[slot].swap(0, Relaxed)),
        f64::from_bits(PITCH[slot].swap(0, Relaxed)),
    )
}

/// # Safety
/// Plain atomics.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_raw_look_enable(mask: u32) {
    ENABLE_CALLS.fetch_add(1, Relaxed);
    if !INIT.load(Relaxed) {
        env_init();
    }
    set_slots(mask);
}
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_raw_look_push(slot: u32, yaw_deg: f64, pitch_deg: f64) {
    push(slot as usize, yaw_deg, pitch_deg);
}
/// Counters for the host stats: `[applied look steps, overlaid scopes]`.
///
/// # Safety
/// `out` points at two writable u32.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_raw_look_stats(out: *mut u32) {
    unsafe {
        *out = APPLIED.load(Relaxed);
        *out.add(1) = OVERLAID.load(Relaxed);
    }
}

/// T2200: true when the player look root can reach the hook in this run, i.e. `0x25D520` or `0xD5EB0` is selected by `--rust-ports`
/// (`player`, `weapons` roots or an explicit list). `--rust-ports=all` does NOT select them (they are opt-in), so the hook is never
/// reached and the mouse cannot turn the camera. Valid once the host configured its ports.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_raw_look_reachable() -> bool {
    reachable()
}
pub fn reachable() -> bool {
    crate::runtime::root_selected(ROOT_LOOK) || crate::runtime::root_selected(0xd5eb0)
}
/// T2200 self-check counters for the shell log: `[enable calls, look root calls for an enabled slot, look calls that kept the
/// original path, mailbox pushes, applied look steps, overlaid scopes]`.
///
/// # Safety
/// `out` points at seven writable u32 (the last is the mounted-slot mask of T3723).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_raw_look_diag(out: *mut u32) {
    let values = [
        ENABLE_CALLS.load(Relaxed),
        LOOK_CALLS.load(Relaxed),
        LOOK_SKIPPED.load(Relaxed),
        PUSHES.load(Relaxed),
        APPLIED.load(Relaxed),
        OVERLAID.load(Relaxed),
        MOUNTED.load(Relaxed),
    ];
    for (index, value) in values.into_iter().enumerate() {
        unsafe { *out.add(index) = value };
    }
}

/// Guest window offset, set by `lane_inv::execute` so a nested `GuestCall` can build its own memory view.
pub fn note_offset(offset: usize) {
    OFFSET.store(offset, Relaxed);
}
pub fn offset() -> usize {
    OFFSET.load(Relaxed)
}

/// Cheap test for the lane entry and the nested call trampoline: true when `va` must go through [`around`].
#[inline]
pub fn watches(va: u32) -> bool {
    if !INIT.load(Relaxed) {
        env_init();
    }
    let mask = ENABLED.load(Relaxed);
    va == ROOT_LOOK && mask != 0
}

fn plausible(address: u32) -> bool {
    (0x1_0000..0x8000_0000).contains(&address) && address & 3 == 0
}

/// The slot of the current player, `None` when no player exists yet or its slot is not enabled. The slot is read from the
/// profile pointer `[player + 0x1500]` (an element of the profile array), 0 when it points elsewhere.
fn active<M: GuestMemory>(m: &M) -> Option<(usize, u32, u32)> {
    let mask = ENABLED.load(Relaxed);
    let player = pw::get32(m, PLAYER_POINTER);
    if !plausible(player) {
        return None;
    }
    let profile = pw::get32(m, player.wrapping_add(0x1500));
    if profile == 0 {
        return None;
    }
    let slot = match profile.wrapping_sub(PROFILE_BASE) {
        offset if offset < PROFILE_SIZE * SLOTS as u32 => (offset / PROFILE_SIZE) as usize,
        _ => 0,
    };
    (mask & (1 << slot) != 0).then_some((slot, player, profile))
}

/// Compatibility lane routing, without wrapping or mutating the original flow. `return_pushed`: the caller's esp
/// already holds the return PC (a lane root entered from lifted code); false for a nested `GuestCall`, which enters
/// with esp at the first argument because the bridge (`tsfp_lane_x_call`) pushes the return PC itself.
pub fn around<M: GuestMemory, C: GuestCall>(
    va: u32,
    return_pushed: bool,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    inner: impl FnOnce(&mut M, &mut C, &mut ExtRegisters) -> bool,
) -> bool {
    if va != ROOT_LOOK || ENABLED.load(Relaxed) == 0 {
        return inner(m, guest, regs);
    }
    if ENABLED.load(Relaxed) == MONITOR_ONLY {
        LOOK_CALLS.fetch_add(1, Relaxed);
        return inner(m, guest, regs);
    }
    // Raw physical counts are a native input mode, never a translated fallback.
    // T3788: the native root reads its arguments past a return slot, so a nested call gets one, as the bridge would
    // push it. Without it every player's look read the movement pair as its look input (I-T3788-1).
    if !return_pushed {
        regs.esp = regs.esp.wrapping_sub(4);
        m.write32(regs.esp, 0);
    }
    let handled = crate::player_look::run(va, true, m, guest, regs);
    if !return_pushed {
        regs.esp = regs.esp.wrapping_add(4);
    }
    handled
}

pub fn assists_disabled<M: GuestMemory>(m: &M, player: u32) -> bool {
    active(m).is_some_and(|(_, current, _)| current == player)
}

pub struct NativeCountStep {
    slot: usize,
    step: u32,
    pub apply: bool,
}
pub fn begin_native<M: GuestMemory>(m: &M, player: u32) -> Option<NativeCountStep> {
    let (slot, current, _) = active(m)?;
    if current != player {
        return None;
    }
    LOOK_CALLS.fetch_add(1, Relaxed);
    let state = pw::get32(m, player + 0x114);
    let locked = pw::get32(m, player + 0xc) & 0x8000 != 0;
    let apply = state == ON_FOOT && !locked;
    let bit = 1u32 << slot;
    if state != ON_FOOT && !locked {
        MOUNTED.fetch_or(bit, Relaxed);
    } else {
        MOUNTED.fetch_and(!bit, Relaxed);
    }
    if !apply {
        LOOK_SKIPPED.fetch_add(1, Relaxed);
        take(slot);
        log_skip(slot, state, locked);
    }
    let step = if apply {
        STEPS[slot].fetch_add(1, Relaxed)
    } else {
        0
    };
    if apply {
        script_feed(slot, step);
    }
    Some(NativeCountStep { slot, step, apply })
}
impl NativeCountStep {
    /// Late drain after native preparation and zoom resolution, without guest
    /// memory. Counts arriving during those operations join this fixed step.
    pub fn apply_angles(self, angles: &mut ViewAngles, constraints: ViewConstraints) {
        if !self.apply {
            return;
        }
        let pushed_at = FIRST_PUSH[self.slot].swap(0, Relaxed);
        let (yaw, pitch) = take(self.slot);
        let old = (angles.yaw, angles.pitch);
        if (yaw != 0.0 || pitch != 0.0)
            && angles.apply_degrees(yaw as f32, pitch as f32, constraints)
        {
            APPLIED.fetch_add(1, Relaxed);
            let file = LATENCY_FILE.load(Relaxed) as *mut c_void;
            if !file.is_null() && pushed_at != 0 {
                unsafe {
                    fprintf(
                        file,
                        c"%u %llu %llu\n".as_ptr(),
                        self.slot as u32,
                        pushed_at,
                        monotonic_ns(),
                    );
                    fflush(file);
                }
            }
        }
        log_step(
            self.slot,
            self.step,
            yaw,
            pitch,
            constraints.zoom,
            old,
            (angles.yaw, angles.pitch),
        );
    }
}

// ---- headless test hooks (libc only, the crate is no_std) ----
use core::ffi::{c_char, c_void};
unsafe extern "C" {
    fn getenv(name: *const c_char) -> *const c_char;
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void;
    fn fclose(file: *mut c_void) -> i32;
    fn fprintf(file: *mut c_void, format: *const c_char, ...) -> i32;
    fn fflush(file: *mut c_void) -> i32;
    fn fgets(buffer: *mut c_char, size: i32, file: *mut c_void) -> *mut c_char;
    fn sscanf(text: *const c_char, format: *const c_char, ...) -> i32;
}
// Opt-in host-boundary diagnostic. Sparse impulses give a mailbox batch's oldest
// push-to-apply age. Independent axis atomics do not promise timestamp correlation
// for a push racing the drain; this is CPU latency, not photon evidence.
static FIRST_PUSH: [AtomicU64; SLOTS] = [ZERO; SLOTS];
static LATENCY_STATE: AtomicU32 = AtomicU32::new(0);
static LATENCY_FILE: AtomicU64 = AtomicU64::new(0);
#[repr(C)]
struct Timespec {
    sec: core::ffi::c_long,
    ns: core::ffi::c_long,
}
unsafe extern "C" {
    fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
}
fn monotonic_ns() -> u64 {
    let mut time = Timespec { sec: 0, ns: 0 };
    if unsafe { clock_gettime(1, &mut time) } == 0 {
        time.sec as u64 * 1_000_000_000 + time.ns as u64
    } else {
        0
    }
}
fn latency_log() -> u64 {
    if LATENCY_STATE
        .compare_exchange(0, 3, Relaxed, Relaxed)
        .is_ok()
    {
        let path = unsafe { getenv(c"TSFP_RAW_LOOK_LATENCY_LOG".as_ptr()) };
        let file = if path.is_null() {
            0
        } else {
            unsafe { fopen(path, c"w".as_ptr()) as u64 }
        };
        LATENCY_FILE.store(file, Relaxed);
        LATENCY_STATE.store(if file == 0 { 2 } else { 1 }, Relaxed);
    }
    LATENCY_FILE.load(Relaxed)
}

const SCRIPT_MAX: usize = 4096;
static mut SCRIPT: [(u32, u32, f64, f64); SCRIPT_MAX] = [(0, 0, 0.0, 0.0); SCRIPT_MAX];
static SCRIPT_LEN: AtomicU32 = AtomicU32::new(0);
static LOG: AtomicU64 = AtomicU64::new(0);

fn env_init() {
    INIT.store(true, Relaxed);
    unsafe {
        let slots = getenv(c"TSFP_RAW_LOOK_SLOTS".as_ptr());
        if !slots.is_null() {
            let mut mask = 0u32;
            if sscanf(slots, c"%i".as_ptr(), &mut mask as *mut u32) == 1 {
                set_slots(mask);
            }
        }
        let script = getenv(c"TSFP_RAW_LOOK_SCRIPT".as_ptr());
        if !script.is_null() {
            let file = fopen(script, c"r".as_ptr());
            if !file.is_null() {
                let mut line = [0 as c_char; 128];
                let mut length = 0usize;
                while length < SCRIPT_MAX && !fgets(line.as_mut_ptr(), 128, file).is_null() {
                    let (mut step, mut slot, mut yaw, mut pitch) = (0u32, 0u32, 0f64, 0f64);
                    if sscanf(
                        line.as_ptr(),
                        c"%u %u %lf %lf".as_ptr(),
                        &mut step as *mut u32,
                        &mut slot as *mut u32,
                        &mut yaw as *mut f64,
                        &mut pitch as *mut f64,
                    ) == 4
                    {
                        (&raw mut SCRIPT as *mut (u32, u32, f64, f64))
                            .add(length)
                            .write((step, slot, yaw, pitch));
                        length += 1;
                    }
                }
                SCRIPT_LEN.store(length as u32, Relaxed);
                fclose(file);
            }
        }
        let log = getenv(c"TSFP_RAW_LOOK_LOG".as_ptr());
        if !log.is_null() {
            LOG.store(fopen(log, c"w".as_ptr()) as u64, Relaxed);
        }
    }
}

fn script_feed(slot: usize, step: u32) {
    let length = SCRIPT_LEN.load(Relaxed) as usize;
    for index in 0..length {
        let (at, who, yaw, pitch) = unsafe {
            (&raw const SCRIPT as *const (u32, u32, f64, f64))
                .add(index)
                .read()
        };
        if at == step && who as usize == slot {
            push(slot, yaw, pitch);
        }
    }
}

static PAIRS_LOGGED: AtomicU32 = AtomicU32::new(0);
/// T3788 diagnostic (`TSFP_RAW_LOOK_LOG`): one line per look-root call with the player, its profile, the raw-look slot
/// it resolved to (-1 none) and the input, movement and look pairs, so a replay shows whose input feeds whose look.
#[allow(clippy::too_many_arguments)]
pub fn log_pairs(
    player: u32,
    profile: u32,
    input: (f32, f32),
    movement: (f32, f32),
    look: (f32, f32),
    counted: bool,
) {
    if !INIT.load(Relaxed) {
        env_init();
    }
    let file = LOG.load(Relaxed) as *mut c_void;
    if file.is_null() || PAIRS_LOGGED.fetch_add(1, Relaxed) >= 1_000_000 {
        return;
    }
    let slot = match profile.wrapping_sub(PROFILE_BASE) {
        offset if offset < PROFILE_SIZE * SLOTS as u32 => (offset / PROFILE_SIZE) as i32,
        _ => -1,
    };
    unsafe {
        fprintf(
            file,
            c"pairs player %#x profile %#x slot %d mask %#x counted %u in %.3f %.3f move %.3f %.3f look %.3f %.3f\n".as_ptr(),
            player,
            profile,
            slot,
            ENABLED.load(Relaxed),
            counted as u32,
            input.0 as f64,
            input.1 as f64,
            movement.0 as f64,
            movement.1 as f64,
            look.0 as f64,
            look.1 as f64,
        );
    }
}

static SKIP_LOGGED: AtomicU32 = AtomicU32::new(0);
fn log_skip(slot: usize, state: u32, locked: bool) {
    let file = LOG.load(Relaxed) as *mut c_void;
    if file.is_null() || SKIP_LOGGED.fetch_add(1, Relaxed) >= 200 {
        return;
    }
    unsafe {
        fprintf(
            file,
            c"skip slot %u state %u locked %u\n".as_ptr(),
            slot as u32,
            state,
            locked as u32,
        );
        fflush(file);
    }
}

#[allow(clippy::too_many_arguments)]
fn log_step(
    slot: usize,
    step: u32,
    yaw_deg: f64,
    pitch_deg: f64,
    zoom: f32,
    old: (f32, f32),
    new: (f32, f32),
) {
    let file = LOG.load(Relaxed) as *mut c_void;
    if file.is_null() {
        return;
    }
    unsafe {
        fprintf(
            file,
            c"%u %u in %.9g %.9g zoom %.9g old %.9g %.9g new %.9g %.9g\n".as_ptr(),
            slot as u32,
            step,
            yaw_deg,
            pitch_deg,
            zoom as f64,
            old.0 as f64,
            old.1 as f64,
            new.0 as f64,
            new.1 as f64,
        );
        fflush(file);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::abi::{Fpu, Xmm};
    use std::{collections::HashMap, sync::Mutex};
    static SERIAL: Mutex<()> = Mutex::new(());
    #[derive(Default)]
    struct Ram(HashMap<u32, u8>);
    impl GuestMemory for Ram {
        fn read8(&self, a: u32) -> u8 {
            *self.0.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }
    struct Guest {
        late: bool,
        calls: u32,
    }
    impl GuestCall for Guest {
        fn call(&mut self, va: u32, r: &mut ExtRegisters) {
            assert_eq!(va, 0xcb3b0);
            self.calls += 1;
            if self.late {
                push(0, 3.0, 2.0);
            }
            r.fpu.push(2.0);
        }
    }
    fn regs() -> ExtRegisters {
        ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x6000,
            ebx: 12,
            ebp: 23,
            esi: 34,
            edi: 45,
            eflags: 0,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [Xmm { lanes: [0; 4] }; 5],
            xmm_hi: [Xmm { lanes: [0; 4] }; 3],
        }
    }
    const PLAYER: u32 = 0x100000;
    fn world(slot: u32) -> Ram {
        let mut m = Ram::default();
        for (a, v) in [
            (PLAYER_POINTER, PLAYER),
            (PLAYER + 0x1500, PROFILE_BASE + slot * PROFILE_SIZE),
            (PLAYER + 0x114, 2),
            (PLAYER + 0xc, 0x80),
            (0x6004, 0x20000),
            (0x6008, 0x21000),
            (0x600c, 0x22000),
            (0x4e8cec, 60),
        ] {
            m.write32(a, v);
        }
        m.write8(PROFILE_BASE + slot * PROFILE_SIZE + 2, 0xff);
        for (a, v) in [
            (PITCH_LIMIT, 60.0f32),
            (0x475c78, 1.0),
            (0x475cdc, 0.3),
            (0x475cd4, 0.5),
            (0x475d24, 0.1),
            (0x4e794c, 1.0),
            (0x4e7948, 1.0),
            (0x7b0cb8, 160.0),
            (0x7b0cbc, 68.0),
            (PROFILE_BASE + slot * PROFILE_SIZE + 0x78, 0.5),
            (PLAYER + 0x100, 10.0),
            (0x20000, 0.7),
            (0x20004, 0.9),
        ] {
            m.write32(a, v.to_bits());
        }
        m
    }
    fn reset() {
        set_slots(0);
        for slot in 0..SLOTS {
            take(slot);
        }
    }
    fn run(m: &mut Ram, g: &mut Guest) {
        let mut r = regs();
        assert!(crate::player_look::run(ROOT_LOOK, true, m, g, &mut r));
        assert_eq!(
            (r.esp, r.ebx, r.ebp, r.esi, r.edi),
            (0x6000, 12, 23, 34, 45)
        );
    }
    #[test]
    fn native_count_flow_never_calls_original_or_changes_profile_or_input() {
        let _lock = SERIAL.lock().unwrap();
        reset();
        set_slots(1);
        let mut m = world(0);
        let mut g = Guest {
            late: false,
            calls: 0,
        };
        push(0, 3.25, 2.5);
        run(&mut m, &mut g);
        assert_eq!(g.calls, 0);
        assert_eq!(f32::from_bits(m.read32(PLAYER + 0x100)), 6.75);
        assert_eq!(f32::from_bits(m.read32(PLAYER + 0x104)), 2.5);
        assert_eq!(m.read8(PROFILE_BASE + 2), 0xff);
        assert_eq!(m.read32(0x20000), 0.7f32.to_bits());
        assert_eq!(m.read32(0x20004), 0.9f32.to_bits());
        assert_eq!(m.read32(0x22000), 0);
        assert_eq!(m.read32(0x22004), 0);
        assert_eq!(take(0), (0.0, 0.0));
        reset();
    }
    /// T3788 (I-T3788-1): with a mouse and keyboard slot, a nested call of the look root (player root 0xD5EB0 in its
    /// lane) enters with esp at the first argument. A pad player's pairs must be the ones a lifted entry reads; before
    /// the fix its movement pair was read as its look input, so moving also turned the view (split screen player 2).
    #[test]
    fn nested_look_call_reads_the_same_arguments_as_a_lifted_entry() {
        let _lock = SERIAL.lock().unwrap();
        let outcome = |return_pushed: bool| {
            reset();
            set_slots(1 << 1); // slot 1 is on mouse and keyboard, the player is the pad player of slot 0
            let mut m = world(0);
            m.write32(0x21000, 0.6f32.to_bits()); // movement pair: stick fully sideways
            let mut g = Guest {
                late: false,
                calls: 0,
            };
            let mut r = regs();
            if !return_pushed {
                // The nested convention: no return slot, arguments from esp.
                for (offset, value) in [(0, 0x20000), (4, 0x21000), (8, 0x22000)] {
                    m.write32(0x6004 + offset, value);
                }
                r.esp = 0x6004;
            }
            let entry_esp = r.esp;
            assert!(around(
                ROOT_LOOK,
                return_pushed,
                &mut m,
                &mut g,
                &mut r,
                |_, _, _| false
            ));
            assert_eq!(r.esp, entry_esp);
            let words = [
                0x21000,
                0x21004,
                0x22000,
                0x22004,
                PLAYER + 0x100,
                PLAYER + 0x104,
            ];
            let state = words.map(|a| m.read32(a));
            reset();
            state
        };
        let lifted = outcome(true);
        assert_ne!(
            lifted[4],
            10.0f32.to_bits(),
            "the look stick turned the lifted player"
        );
        assert_eq!(outcome(false), lifted);
    }
    #[test]
    fn late_count_drain_joins_zoom_provider_arrivals() {
        let _lock = SERIAL.lock().unwrap();
        reset();
        set_slots(1);
        let mut m = world(0);
        let mut g = Guest {
            late: true,
            calls: 0,
        };
        m.write32(CAMERA_POINTER, 0x130000);
        m.write32(0x130384, 1.0f32.to_bits());
        run(&mut m, &mut g);
        assert_eq!(g.calls, 1);
        assert_eq!(f32::from_bits(m.read32(PLAYER + 0x100)), 8.5);
        assert_eq!(f32::from_bits(m.read32(PLAYER + 0x104)), 1.0);
        assert_eq!(take(0), (0.0, 0.0));
        reset();
    }
    #[test]
    fn native_count_transaction_wraps_clamps_and_rejects_conversion_overflow() {
        let _lock = SERIAL.lock().unwrap();
        reset();
        set_slots(1);
        let mut m = world(0);
        let mut g = Guest {
            late: false,
            calls: 0,
        };
        push(0, 100000.0, 500.0);
        run(&mut m, &mut g);
        assert_eq!(f32::from_bits(m.read32(PLAYER + 0x100)), 90.0);
        assert_eq!(f32::from_bits(m.read32(PLAYER + 0x104)), 60.0);
        let before = (m.read32(PLAYER + 0x100), m.read32(PLAYER + 0x104));
        push(0, f64::MAX, 5.0);
        run(&mut m, &mut g);
        assert_eq!((m.read32(PLAYER + 0x100), m.read32(PLAYER + 0x104)), before);
        assert_eq!(take(0), (0.0, 0.0));
        reset();
    }
    #[test]
    fn locked_counts_drop_and_other_slot_keeps_pad_profile() {
        let _lock = SERIAL.lock().unwrap();
        reset();
        set_slots(1);
        let mut m = world(0);
        let mut g = Guest {
            late: false,
            calls: 0,
        };
        m.write32(PLAYER + 0xc, 0x8000);
        let ordinal = STEPS[0].load(Relaxed);
        push(0, 5.0, 5.0);
        run(&mut m, &mut g);
        assert_eq!(
            STEPS[0].load(Relaxed),
            ordinal,
            "locked steps preserve script ordinal"
        );
        assert_eq!(m.read32(PLAYER + 0x100), 10.0f32.to_bits());
        assert_eq!(take(0), (0.0, 0.0));
        let m = world(1);
        assert!(!assists_disabled(&m, PLAYER));
        push(0, 9.0, 9.0);
        assert!(begin_native(&m, PLAYER).is_none());
        assert_eq!(take(0), (9.0, 9.0));
        reset();
    }
    #[test]
    fn targeting_uses_owned_assist_policy_without_profile_overlay() {
        let _lock = SERIAL.lock().unwrap();
        reset();
        set_slots(1);
        let mut m = world(0);
        m.write32(0x6004, PLAYER);
        let mut r = regs();
        let mut g = Guest {
            late: false,
            calls: 0,
        };
        assert!(crate::player_look_gate::run(
            0xcff20, true, &mut m, &mut g, &mut r
        ));
        assert_eq!(r.eax, 0);
        assert_eq!(g.calls, 0);
        assert_eq!(m.read8(PROFILE_BASE + 2), 0xff);
        assert_eq!(OVERLAID.load(Relaxed), 0);
        reset();
    }
    #[test]
    fn monitor_routing_observes_only_and_scope_roots_are_unwatched() {
        let _lock = SERIAL.lock().unwrap();
        reset();
        set_slots(MONITOR_ONLY);
        assert!(watches(ROOT_LOOK));
        for va in ROOTS_SCOPE {
            assert!(!watches(va));
        }
        let mut m = world(0);
        let mut g = Guest {
            late: false,
            calls: 0,
        };
        let mut r = regs();
        let mut calls = 0;
        assert!(around(
            ROOT_LOOK,
            true,
            &mut m,
            &mut g,
            &mut r,
            |m, _, _| {
                calls += 1;
                assert_eq!(m.read8(PROFILE_BASE + 2), 0xff);
                true
            }
        ));
        assert_eq!(calls, 1);
        reset();
    }
}
