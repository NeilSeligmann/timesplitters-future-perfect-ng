// SPDX-License-Identifier: GPL-3.0-or-later
//! T2082 (S3 writer wiring 1): the minimal adapter between the lane ports and the Rust-owned
//! `GameState` of crates/tsfp-state (T2068). tsfp-ports cannot depend on tsfp-state as a crate (the lane
//! build compiles one crate by direct rustc, the same constraint as `views.rs`), so `game_state.rs` is
//! compiled in by path with `GuestMemory` re-exported as `super::GuestMemory`. Pattern and limits:
//! docs/t-s3-wiring-gamestate.md.
//!
//! Each wired `set_*` writes through to the unchanged guest layout for lifted readers. Off mode loads
//! a temporary `GameState`; own/audit modes update persistent `RETIRE`. Converted getters and `Wired`
//! reads use that authority in own mode. Contract and remaining writer gaps: docs/t-s3-retire-gamestate.md.
pub use crate::memory::GuestMemory;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/game_state.rs"]
pub mod game_state;
pub use game_state::GameState;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/story_flow.rs"]
pub mod story_flow;

/// Mutation control (host build with `TSFP_RUST_LANE_MUTATION`, `rust_lane_mutation` cfg): with
/// `TSFP_LANE_MUTATE_STATE=<hex cell address>` wired stores to that cell flip bit 0 of the value they write (`1` means
/// every cell). `TSFP_LANE_MUTATE_NTH=<n>` restricts the fault to the n-th matching store (1-based, 0 or unset: all), so
/// a late store can be broken without ending the run at boot. A Tier 2 run with the wired roots selected must DIVERGE
/// on the GameState regions. Absent from normal builds.
#[cfg(rust_lane_mutation)]
pub fn mutate_state(cell: u32, value: u32) -> u32 {
    use core::sync::atomic::{AtomicU32, Ordering};
    unsafe extern "C" {
        fn tsfp_lane_mutation_state() -> u32;
        fn tsfp_lane_mutation_nth() -> u32;
        fn tsfp_lane_mutation_log(va: u32, address: u32);
    }
    static MATCHING: AtomicU32 = AtomicU32::new(0);
    let target = unsafe { tsfp_lane_mutation_state() };
    if target != 1 && target != cell {
        return value;
    }
    let seen = MATCHING.fetch_add(1, Ordering::Relaxed) + 1;
    // `MUTWRITE <ordinal> <cell>` per matching store (stderr), so a run lists the stores it could break
    unsafe { tsfp_lane_mutation_log(seen, cell) };
    let nth = unsafe { tsfp_lane_mutation_nth() };
    if nth == 0 || nth == seen {
        value ^ 1
    } else {
        value
    }
}
#[cfg(not(rust_lane_mutation))]
pub fn mutate_state(_cell: u32, value: u32) -> u32 {
    value
}

/// T2135 Retire pilot: the persistent GameState owner (docs/t-s3-retire-gamestate.md). Mode 0 (default) is the plain
/// write-through of T2082 and plain guest reads; 1 makes the owner authoritative for the converted readers; 2 audits.
/// Selection: `configure` validates the shipped option; mutation builds also have an env fallback.
/// Without explicit selection the mode stays off.
#[cfg(not(any(test, feature = "thread-retire")))]
pub static RETIRE: game_state::Retire = game_state::Retire::new();
/// Unit tests get one `Retire` per test thread (T3447): the process static leaked mode switches from one test into
/// every getter-reading test running in parallel. Production keeps the single process static above.
#[cfg(any(test, feature = "thread-retire"))]
pub static RETIRE: ThreadRetire = ThreadRetire;
#[cfg(any(test, feature = "thread-retire"))]
pub struct ThreadRetire;
#[cfg(any(test, feature = "thread-retire"))]
impl core::ops::Deref for ThreadRetire {
    type Target = game_state::Retire;
    fn deref(&self) -> &game_state::Retire {
        std::thread_local! {
            static PER_THREAD: &'static game_state::Retire = std::boxed::Box::leak(std::boxed::Box::new(game_state::Retire::new()));
        }
        PER_THREAD.with(|retire| *retire)
    }
}
use game_state::Cell;

/// Roots that must ALL run in Rust for retire mode 1 (owner authoritative) to be sound: the 24 wired writers of T2082
/// (21 `state_lane` roots plus `65D20 668F0 23E930`) and the four modes-lane writers (T2135 phase 2). A lifted run of
/// any of them writes a cell behind the owner (measured: 23 drift events without the four modes roots).
pub const REQUIRED_ROOTS: [u32; 28] = [
    0x65230, 0x653b0, 0x65420, 0x65470, 0x658d0, 0x65c70, 0x65d20, 0x65f40, 0x667f0, 0x668a0,
    0x668f0, 0x66fc0, 0x670c0, 0x674c0, 0x67450, 0x67e60, 0x67f00, 0x68720, 0x15df20, 0x15e4f0,
    0x22ce60, 0x23e930, 0x23efe0, 0x25f830, 0x67280, 0x68610, 0x681c0, 0x68830,
];

static CONFIGURED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Host option entry (`--retire-gamestate off|own|audit`, runtime.rs `tsfp_rust_retire_configure`). `is_selected` answers
/// whether a VA runs in Rust. Mode 1 is refused (`Err(first missing root)`) unless every required root is selected;
/// audit (2) only observes and is always allowed. Once configured, the mutation-host env reader is ignored.
pub fn configure(mode: u8, is_selected: impl Fn(u32) -> bool) -> Result<(), u32> {
    if mode == game_state::RETIRE_OWN {
        if let Some(missing) = REQUIRED_ROOTS.iter().find(|va| !is_selected(**va)) {
            return Err(*missing);
        }
    }
    RETIRE.set_mode(mode);
    CONFIGURED.store(true, core::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// Resolves the host-selected mode once (first wired call or converted read) in a mutation host; a mode that
/// `configure` installed wins over the env reader.
#[cfg(rust_lane_mutation)]
fn resolve_mode() {
    use core::sync::atomic::{AtomicBool, Ordering};
    unsafe extern "C" {
        fn tsfp_lane_retire_mode() -> u32;
    }
    static DONE: AtomicBool = AtomicBool::new(false);
    if !DONE.swap(true, Ordering::Relaxed) && !CONFIGURED.load(Ordering::Relaxed) {
        RETIRE.set_mode(unsafe { tsfp_lane_retire_mode() } as u8);
    }
}
#[cfg(not(rust_lane_mutation))]
#[inline(always)]
fn resolve_mode() {}

/// Mutation control (cfg `rust_lane_mutation`): `TSFP_LANE_MUTATE_OWNED=<hex cell>|1` makes a retire-mode setter store
/// `value ^ 1` into the OWNED value only, the guest mirror keeps the correct value. With converted readers in mode 1 the
/// run must DIVERGE (readers use the owned value), in mode 2 it must stay EQUAL (readers use the guest value) but drift.
/// `TSFP_LANE_MUTATE_NTH` restricts it to the n-th matching store.
#[cfg(rust_lane_mutation)]
fn mutate_owned(cell: u32, value: u32) -> u32 {
    use core::sync::atomic::{AtomicU32, Ordering};
    unsafe extern "C" {
        fn tsfp_lane_mutation_owned() -> u32;
        fn tsfp_lane_mutation_nth() -> u32;
    }
    static MATCHING: AtomicU32 = AtomicU32::new(0);
    let target = unsafe { tsfp_lane_mutation_owned() };
    if RETIRE.mode() == 0 || target == 0 || (target != 1 && target != cell) {
        return value;
    }
    let seen = MATCHING.fetch_add(1, Ordering::Relaxed) + 1;
    let nth = unsafe { tsfp_lane_mutation_nth() };
    if nth == 0 || nth == seen {
        value ^ 1
    } else {
        value
    }
}
#[cfg(not(rust_lane_mutation))]
#[inline(always)]
fn mutate_owned(_cell: u32, value: u32) -> u32 {
    value
}

/// Audit log: `RETDRIFT <cell> <guest> <owned> <count>` on stderr for the first drifts (mutation host only).
#[cfg(rust_lane_mutation)]
fn on_drift(cell: Cell, guest: u32, owned: u32) {
    unsafe extern "C" {
        fn tsfp_lane_retire_log(cell: u32, guest: u32, owned: u32, count: u32);
    }
    unsafe { tsfp_lane_retire_log(cell.address(), guest, owned, RETIRE.drift_count()) };
}
#[cfg(not(rust_lane_mutation))]
fn on_drift(_cell: Cell, _guest: u32, _owned: u32) {}

/// Audit site log (T2135 phase 2): `RETSITE <kind> <root> <prev root> <cell> <guest> <owned>` on stderr, once per distinct
/// (kind, cell, guest, owned). `kind` 0: found out of sync at the entry of lane root `root` (a lifted writer, or a port
/// that bypasses the owner, ran since the previous root `prev`), 1: found out of sync by a setter, before it overwrote
/// the owner (the bypassing store happened between the previous owner write of that cell and this one).
#[cfg(rust_lane_mutation)]
fn on_site(kind: u32, root: u32, prev: u32, cell: u32, guest: u32, owned: u32) {
    unsafe extern "C" {
        fn tsfp_lane_retire_site(
            kind: u32,
            root: u32,
            prev: u32,
            cell: u32,
            guest: u32,
            owned: u32,
        );
    }
    unsafe { tsfp_lane_retire_site(kind, root, prev, cell, guest, owned) };
}
#[cfg(not(rust_lane_mutation))]
fn on_site(_kind: u32, _root: u32, _prev: u32, _cell: u32, _guest: u32, _owned: u32) {}

/// Watched cells outside the owner (kind 2, T2135 phase 3): `0x7DE456` mode sub and the tick words `0x4E8CEC` /
/// `0x4E794C`. They have no owner, so "drift" is a change of the guest value between two lane-root entries that no
/// wired setter made; the line carries `guest` = new value and `owned` = the value seen at the previous entry.
const WATCHED: [u32; 3] = [
    game_state::MODE_SUB,
    game_state::TICK_DIVISOR,
    game_state::TICK_SCALE_BITS,
];
static WATCH: [core::sync::atomic::AtomicU32; 3] =
    [const { core::sync::atomic::AtomicU32::new(0) }; 3];
static WATCH_INIT: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

fn watch_read<M: GuestMemory>(m: &M, address: u32) -> u32 {
    if address == game_state::MODE_SUB {
        m.read8(address) as u32
    } else {
        m.read32(address)
    }
}

/// A wired setter made this value: the shadow follows it so only bypassing stores are reported.
fn watch_note(address: u32, value: u32) {
    if let Some(i) = WATCHED.iter().position(|a| *a == address) {
        WATCH[i].store(value, core::sync::atomic::Ordering::Relaxed);
    }
}

fn watch_scan<M: GuestMemory>(m: &M) {
    use core::sync::atomic::Ordering::Relaxed;
    let (root, prev) = (LAST_ROOT.load(Relaxed), PREV_ROOT.load(Relaxed));
    let first = !WATCH_INIT.swap(true, Relaxed);
    for (i, address) in WATCHED.iter().enumerate() {
        let now = watch_read(m, *address);
        let before = WATCH[i].swap(now, Relaxed);
        if !first && now != before {
            on_site(2, root, prev, *address, now, before);
        }
    }
}

static LAST_ROOT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static PREV_ROOT: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

fn audit_scan<M: GuestMemory>(m: &M, kind: u32, only: Option<Cell>) {
    use core::sync::atomic::Ordering::Relaxed;
    RETIRE.ensure_init(m);
    let (root, prev) = (LAST_ROOT.load(Relaxed), PREV_ROOT.load(Relaxed));
    for cell in Cell::ALL {
        if only.is_some_and(|c| c.address() != cell.address()) {
            continue;
        }
        let (guest, owned) = (cell.read_guest_pub(m), RETIRE.owned(cell));
        if guest != owned {
            on_site(kind, root, prev, cell.address(), guest, owned);
        }
    }
}

/// Lane root entry hook (audit mode only): records `va` as the current root and logs every cell whose guest value
/// differs from the owner. No effect in modes 0 and 1.
pub fn audit_entry<M: GuestMemory>(m: &M, va: u32) {
    use core::sync::atomic::Ordering::Relaxed;
    resolve_mode();
    if RETIRE.mode() != game_state::RETIRE_AUDIT {
        return;
    }
    PREV_ROOT.store(LAST_ROOT.load(Relaxed), Relaxed);
    LAST_ROOT.store(va, Relaxed);
    audit_scan(m, 0, None);
    watch_scan(m);
}

fn write_cell<M: GuestMemory>(m: &mut M, cell: Cell, value: u32) {
    resolve_mode();
    let guest = mutate_state(cell.address(), value);
    if RETIRE.mode() == 0 {
        // mode 0: exactly the T2082 path (load, owned setter with write-through)
        let mut state = GameState::load(m);
        match cell {
            Cell::Level => state.set_level(m, guest),
            Cell::Players => state.set_players(m, guest),
            Cell::ExtraPlayers => state.set_extra_players(m, guest),
            Cell::Mode => state.set_mode(m, guest as u8),
            Cell::Flags => state.set_flags(m, guest),
            Cell::Match => state.set_match_state(m, guest),
        }
    } else {
        if RETIRE.mode() == game_state::RETIRE_AUDIT {
            audit_scan(m, 1, Some(cell));
        }
        RETIRE.write(m, cell, guest, mutate_owned(cell.address(), guest));
    }
}

macro_rules! wired_setter {
    ($(#[$doc:meta])* $name:ident, $ty:ty, $cell:expr) => {
        $(#[$doc])*
        pub fn $name<M: GuestMemory>(m: &mut M, value: $ty) {
            write_cell(m, $cell, value as u32);
        }
    };
}

wired_setter!(/// `[0x79094C]` level id.
    set_level, u32, Cell::Level);
wired_setter!(/// `[0x790950]` local player count.
    set_players, u32, Cell::Players);
wired_setter!(/// `[0x7A2958]` extra player count.
    set_extra_players, u32, Cell::ExtraPlayers);
wired_setter!(/// `[0x7DE455]` mode byte.
    set_mode, u8, Cell::Mode);
wired_setter!(/// `[0x7DE458]` session flags, dword store.
    set_flags, u32, Cell::Flags);
wired_setter!(/// `[0x7DE30C]` match state.
    set_match_state, u32, Cell::Match);

/// `[0x7DE456]` mode sub byte: no wired writer, not part of the persistent owner (plain T2082 path).
pub fn set_mode_sub<M: GuestMemory>(m: &mut M, value: u8) {
    let guest = mutate_state(game_state::MODE_SUB, value as u32) as u8;
    let mut state = GameState::load(m);
    state.set_mode_sub(m, guest);
    watch_note(game_state::MODE_SUB, guest as u32);
}

/// `[0x7DE458]` low byte store, upper bytes unchanged.
pub fn set_flags_low8<M: GuestMemory>(m: &mut M, value: u8) {
    resolve_mode();
    let guest = mutate_state(game_state::FLAGS, value as u32) as u8;
    if RETIRE.mode() == 0 {
        let mut state = GameState::load(m);
        state.set_flags_low8(m, guest);
    } else {
        RETIRE.write_flags_low8(
            m,
            guest,
            mutate_owned(game_state::FLAGS, guest as u32) as u8,
        );
    }
}

// Typed readers for converted code (`tools.c2rust run --gamestate-readers`). Mode 0 is the plain guest read.
macro_rules! wired_getter {
    ($(#[$doc:meta])* $name:ident, $cell:expr) => {
        $(#[$doc])*
        #[inline]
        pub fn $name<M: GuestMemory>(m: &M) -> u32 {
            resolve_mode();
            RETIRE.read(m, $cell, on_drift)
        }
    };
}
wired_getter!(/// `[0x79094C]` level id.
    get_level, Cell::Level);
wired_getter!(/// `[0x790950]` local player count.
    get_players, Cell::Players);
wired_getter!(/// `[0x7A2958]` extra player count.
    get_extra_players, Cell::ExtraPlayers);
wired_getter!(/// `[0x7DE458]` session flags dword.
    get_flags, Cell::Flags);
wired_getter!(/// `[0x7DE30C]` match state.
    get_match_state, Cell::Match);
/// `[0x7DE455]` mode byte.
#[inline]
pub fn get_mode<M: GuestMemory>(m: &M) -> u8 {
    resolve_mode();
    RETIRE.read(m, Cell::Mode, on_drift) as u8
}

/// `[0x7DE458]` low byte of the session flags (T2135 phase 4: byte reads of the flags dword).
#[inline]
pub fn get_flags_low8<M: GuestMemory>(m: &M) -> u8 {
    get_flags(m) as u8
}

/// Memory wrapper for ports that write the cells through a generic host (the T1896 `ModeHost` ports over
/// `HostBridge`): dword stores to the level, players, extra players, match and flags cells and byte stores to the mode
/// byte and the flags low byte go through the `set_*` writers (T2135 phase 2). Reads, including partial and straddling reads,
/// use the persistent owner for its six cells when enabled; other bytes remain guest-owned.
/// Raw slices are unavailable: a guest slice would bypass authority. Bulk reads use `read8`.
/// Writer coverage is unchanged: upper-byte and arbitrary straddling stores still need explicit wiring.
pub struct Wired<'a, H: GuestMemory>(pub &'a mut H);

impl<H: GuestMemory> Wired<'_, H> {
    fn overlaps_owned(address: u32, width: u32) -> bool {
        (0..width).any(|offset| Cell::containing_byte(address.wrapping_add(offset)).is_some())
    }
}

impl<H: GuestMemory> GuestMemory for Wired<'_, H> {
    fn read8(&self, address: u32) -> u8 {
        if let Some((cell, offset)) = Cell::containing_byte(address) {
            resolve_mode();
            if RETIRE.mode() == game_state::RETIRE_OFF {
                return self.0.read8(address);
            }
            (RETIRE.read(self.0, cell, on_drift) >> (8 * offset)) as u8
        } else {
            self.0.read8(address)
        }
    }
    fn read16(&self, address: u32) -> u16 {
        if Self::overlaps_owned(address, 2) {
            resolve_mode();
            if RETIRE.mode() == game_state::RETIRE_OFF {
                return self.0.read16(address);
            }
            u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
        } else {
            self.0.read16(address)
        }
    }
    fn write8(&mut self, address: u32, value: u8) {
        match address {
            game_state::MODE => set_mode(self.0, value),
            game_state::FLAGS => set_flags_low8(self.0, value),
            game_state::MODE_SUB => set_mode_sub(self.0, value),
            _ => self.0.write8(address, value),
        }
    }
    fn read32(&self, address: u32) -> u32 {
        match address {
            game_state::LEVEL => get_level(self.0),
            game_state::PLAYERS => get_players(self.0),
            game_state::EXTRA_PLAYERS => get_extra_players(self.0),
            game_state::FLAGS => get_flags(self.0),
            game_state::MATCH => get_match_state(self.0),
            _ if Self::overlaps_owned(address, 4) => {
                resolve_mode();
                if RETIRE.mode() == game_state::RETIRE_OFF {
                    return self.0.read32(address);
                }
                u32::from_le_bytes(core::array::from_fn(|i| {
                    self.read8(address.wrapping_add(i as u32))
                }))
            }
            _ => self.0.read32(address),
        }
    }
    fn write32(&mut self, address: u32, value: u32) {
        match address {
            game_state::LEVEL => set_level(self.0, value),
            game_state::PLAYERS => set_players(self.0, value),
            game_state::EXTRA_PLAYERS => set_extra_players(self.0, value),
            game_state::FLAGS => set_flags(self.0, value),
            game_state::MATCH => set_match_state(self.0, value),
            // the dword that holds [mode_flag 0x7DE454][mode 0x7DE455][sub 0x7DE456][0x7DE457] (the mode setup block copy)
            a if a == game_state::MODE - 1 => {
                let bytes = value.to_le_bytes();
                self.0.write8(a, bytes[0]);
                set_mode(self.0, bytes[1]);
                set_mode_sub(self.0, bytes[2]);
                self.0.write8(a + 3, bytes[3]);
            }
            _ => self.0.write32(address, value),
        }
    }
}

/// Serialises the unit tests that read or switch the process-wide `RETIRE` mode.
#[cfg(any(test, feature = "thread-retire"))]
pub static MODE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    struct Ram(std::collections::BTreeMap<u32, u8>);
    impl GuestMemory for Ram {
        fn read8(&self, a: u32) -> u8 {
            *self.0.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }

    #[test]
    fn getters_are_plain_reads_while_retire_is_off() {
        let _guard = MODE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // RETIRE is a process static that stays at mode 0 in unit tests (host selection needs the mutation build)
        let mut m = Ram(Default::default());
        m.write32(0x79094c, 0x65);
        m.write32(0x790950, 2);
        m.write32(0x7a2958, 3);
        m.write8(0x7de455, 10);
        m.write32(0x7de458, 0xdead_0040);
        m.write32(0x7de30c, 9);
        assert_eq!(
            (
                get_level(&m),
                get_players(&m),
                get_extra_players(&m),
                get_mode(&m),
                get_flags(&m),
                get_match_state(&m)
            ),
            (0x65, 2, 3, 10, 0xdead_0040, 9)
        );
        assert_eq!(RETIRE.mode(), 0);
    }

    #[test]
    fn mode_one_needs_every_required_root() {
        let _guard = MODE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // the static RETIRE is shared by the unit tests, so only the refusal path (no state change) is exercised here
        let all_but = |hole: u32| move |va: u32| va != hole;
        for hole in REQUIRED_ROOTS {
            assert_eq!(configure(game_state::RETIRE_OWN, all_but(hole)), Err(hole));
        }
        assert_eq!(REQUIRED_ROOTS.len(), 28);
        assert_eq!(RETIRE.mode(), 0);
        assert!(configure(game_state::RETIRE_OFF, |_| false).is_ok());
    }

    #[test]
    fn setters_write_only_their_cell() {
        let mut m = Ram(Default::default());
        m.write32(0x7de458, 0x1122_3344);
        m.write32(0x7de45c, 0xdead_beef);
        set_flags_low8(&mut m, 0x99);
        assert_eq!(m.read32(0x7de458), 0x1122_3399);
        set_match_state(&mut m, 7);
        set_level(&mut m, 0x65);
        set_players(&mut m, 2);
        set_mode(&mut m, 10);
        assert_eq!(
            (
                m.read32(0x7de30c),
                m.read32(0x79094c),
                m.read32(0x790950),
                m.read8(0x7de455)
            ),
            (7, 0x65, 2, 10)
        );
        assert_eq!(m.read32(0x7de45c), 0xdead_beef);
    }

    #[test]
    fn wired_memory_routes_cell_stores_and_the_block_copy_dword() {
        let mut m = Ram(Default::default());
        {
            let mut w = Wired(&mut m);
            w.write32(0x7de30c, 7);
            w.write32(0x7de458, 0x4000_0040);
            w.write32(0x7de454, 0x4433_0a99);
            w.write8(0x7de455, 3);
            w.write32(0x7de45c, 5);
        }
        assert_eq!(
            (
                m.read32(0x7de30c),
                m.read32(0x7de458),
                m.read32(0x7de454),
                m.read32(0x7de45c)
            ),
            (7, 0x4000_0040, 0x4433_0399, 5)
        );
    }
}
