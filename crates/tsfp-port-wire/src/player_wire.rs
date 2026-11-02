// SPDX-License-Identifier: GPL-3.0-or-later
//! T2138 (S3 Retire pilot 2): the persistent PlayerSet owner behind the wired player writers and the converted readers.
//! Design, modes, the re-base rule and the measured drift: docs/t-s3-retire-player.md. Same shape as `state_wire.rs`
//! (T2135, GameState): one `static PLAYER: PlayerRetire`, three modes (0 off = the T2118 write-through path byte for byte,
//! 1 own = the owner answers converted readers, 2 audit = readers keep the guest value and every guest/owned mismatch is
//! logged with its entry site), a startup refusal unless the required roots are selected, and mutation controls.
//!
//! Log lines (stderr, mutation host only): `RETPLAYER <kind> <root> <prev root> <offset> <guest> <owned>`. `kind` 0: the
//! word was out of sync at the entry of lane root `root` (a lifted writer ran since root `prev`), 1: out of sync in a
//! setter before it overwrote the word, 2: a converted reader found the word out of sync, 3: re-base, `offset` is the
//! pointer cell index (0 record, 1 controlled, 2 stats), `guest` the new pointer, `owned` the old one, 4: the same as 0 to 2
//! but on a pass-through word (expected, not counted), 5: first use, `guest` the record pointer, `owned` the stats pointer.
//! `offset` is the
//! record byte offset, or `0x10000 | stats offset`.
#[allow(unused_imports)]
use crate::leaf_wire::player::CONTROLLED_SIZE;
use crate::leaf_wire::player::{PTR_RECORD, PTR_STATS, PlayerRetire, Slot};
pub use crate::memory::GuestMemory;
use crate::state_wire::game_state::{RETIRE_AUDIT, RETIRE_OWN};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};

#[cfg(not(any(test, feature = "thread-retire")))]
pub static PLAYER: PlayerRetire = PlayerRetire::new();

/// T2175: under `cargo test` every test thread owns a private `PlayerRetire`, so tests that reach the owner through the
/// lane (`leaf_wire::store`, `audit_entry`) cannot race the tests that set the mode or count re-bases (the old flake of
/// `store_read_rebase_and_audit`). Production code keeps the single process-wide static. T3610: the dependent crates'
/// unit-test builds get the same per-thread owner through the `thread-retire` feature of this crate.
#[cfg(any(test, feature = "thread-retire"))]
pub static PLAYER: TestPlayer = TestPlayer;
#[cfg(any(test, feature = "thread-retire"))]
pub struct TestPlayer;
#[cfg(any(test, feature = "thread-retire"))]
impl core::ops::Deref for TestPlayer {
    type Target = PlayerRetire;
    fn deref(&self) -> &PlayerRetire {
        std::thread_local! {
            static OWNER: &'static PlayerRetire = alloc::boxed::Box::leak(alloc::boxed::Box::new(PlayerRetire::new()));
        }
        OWNER.with(|owner| *owner)
    }
}

/// `anim_start_authoritative` of the facade (`tsfp-ports::player_wire`) passes its `ANIM_WRAPPER_ROOTS[0]` as `root`.
/// In an active mode (own, audit) the readable `start` port is authoritative for `0x5A360` and its stores go through
/// `Wired`; off keeps the shadow comparison against the retained original. Runs the entry hook when active.
pub fn anim_start_authoritative_at<M: GuestMemory>(m: &M, root: u32) -> bool {
    resolve_mode();
    if PLAYER.mode() == 0 {
        return false;
    }
    audit_entry(m, root);
    true
}

static CONFIGURED: AtomicBool = AtomicBool::new(false);

/// Host option entry (`--retire-player off|own|audit`, runtime.rs `tsfp_rust_retire_player_configure`; the facade's
/// `player_wire::configure` supplies `required`, its `REQUIRED_ROOTS`). Mode 1 is refused (`Err(first missing root)`)
/// unless every required root is selected; audit only observes and is always allowed.
pub fn configure_roots(
    mode: u8,
    required: &[u32],
    is_selected: impl Fn(u32) -> bool,
) -> Result<(), u32> {
    if mode == RETIRE_OWN {
        if let Some(missing) = required.iter().find(|va| !is_selected(**va)) {
            return Err(*missing);
        }
    }
    PLAYER.set_mode(mode);
    CONFIGURED.store(true, Relaxed);
    Ok(())
}

#[cfg(rust_lane_mutation)]
fn resolve_mode() {
    static DONE: AtomicBool = AtomicBool::new(false);
    unsafe extern "C" {
        fn tsfp_lane_retire_player_mode() -> u32;
    }
    if !DONE.swap(true, Relaxed) && !CONFIGURED.load(Relaxed) {
        PLAYER.set_mode(unsafe { tsfp_lane_retire_player_mode() } as u8);
    }
}
#[cfg(not(rust_lane_mutation))]
#[inline(always)]
fn resolve_mode() {}

/// Owned-only mutation (cfg `rust_lane_mutation`): `TSFP_LANE_MUTATE_OWNED=<hex key>` makes a retire-mode setter store
/// `value ^ 1` (stats cells: `value ^ 0x400000`) into the OWNED word only, the guest keeps the correct value. Keys: `7b0c7c` every record word, `7b0c48` the
/// controlled-object prefix (record offsets below 0x1584), `75ccc0` the stats table, `1` all three. `TSFP_LANE_MUTATE_NTH`
/// restricts it to the n-th matching store. In own mode a run that reads the word must DIVERGE, in audit mode it must
/// stay EQUAL (readers use the guest value) and the drift must be logged.
#[cfg(rust_lane_mutation)]
fn mutate_owned(slot: Slot, value: u32) -> u32 {
    unsafe extern "C" {
        fn tsfp_lane_mutation_owned() -> u32;
        fn tsfp_lane_mutation_nth() -> u32;
    }
    static MATCHING: AtomicU32 = AtomicU32::new(0);
    let key = unsafe { tsfp_lane_mutation_owned() };
    let hit = match slot {
        Slot::Record(i) => {
            key == 1 || key == 0x7b0c7c || (key == 0x7b0c48 && (i as u32) * 4 < CONTROLLED_SIZE)
        }
        Slot::Stats(_) => key == 1 || key == 0x75ccc0,
    };
    if PLAYER.mode() == 0 || !hit {
        return value;
    }
    let seen = MATCHING.fetch_add(1, Relaxed) + 1;
    let nth = unsafe { tsfp_lane_mutation_nth() };
    // stats cells are f32 accumulators: a bit 0 flip is absorbed by the rounding of the next accumulate (measured: the
    // first stats mutant stayed EQUAL in own mode), so the stats mutant flips mantissa bit 22
    let flip = if matches!(slot, Slot::Stats(_)) {
        0x0040_0000
    } else {
        1
    };
    if nth == 0 || nth == seen {
        value ^ flip
    } else {
        value
    }
}
#[cfg(not(rust_lane_mutation))]
#[inline(always)]
fn mutate_owned(_slot: Slot, value: u32) -> u32 {
    value
}

/// Kind of a drift line: `kind` for an authoritative word, 4 ("expected": the word is on the pass-through list) otherwise.
fn drift_log(slot: Slot, kind: u32, guest: u32, owned: u32) {
    log(
        if PLAYER.authoritative(slot) { kind } else { 4 },
        slot.log_offset(),
        guest,
        owned,
    );
}

#[cfg(rust_lane_mutation)]
fn log(kind: u32, offset: u32, guest: u32, owned: u32) {
    unsafe extern "C" {
        fn tsfp_lane_retire_player(
            kind: u32,
            root: u32,
            prev: u32,
            offset: u32,
            guest: u32,
            owned: u32,
        );
    }
    unsafe {
        tsfp_lane_retire_player(
            kind,
            LAST_ROOT.load(Relaxed),
            PREV_ROOT.load(Relaxed),
            offset,
            guest,
            owned,
        )
    };
}
#[cfg(not(rust_lane_mutation))]
fn log(_kind: u32, _offset: u32, _guest: u32, _owned: u32) {}

static LAST_ROOT: AtomicU32 = AtomicU32::new(0);
static PREV_ROOT: AtomicU32 = AtomicU32::new(0);

fn rebase_log(cell: usize, old: u32, new: u32) {
    log(3, cell as u32, new, old);
}

/// Lane root entry hook (audit mode: scan and log; own mode: only the re-base check; off: nothing).
pub fn audit_entry<M: GuestMemory>(m: &M, va: u32) {
    resolve_mode();
    match PLAYER.mode() {
        RETIRE_AUDIT => {
            PREV_ROOT.store(LAST_ROOT.load(Relaxed), Relaxed);
            LAST_ROOT.store(va, Relaxed);
            let fresh = !PLAYER.is_init();
            PLAYER.ensure_init(m);
            if fresh {
                log(5, 0, PLAYER.ptr(PTR_RECORD), PLAYER.ptr(PTR_STATS));
            }
            PLAYER.sync_ptrs(m, rebase_log);
            PLAYER.scan(m, true, |slot, guest, owned| {
                drift_log(slot, 0, guest, owned)
            });
        }
        RETIRE_OWN => {
            PLAYER.ensure_init(m);
            PLAYER.sync_ptrs(m, |_, _, _| {});
        }
        _ => {}
    }
}

/// Completed-load attachment for a future Rust loader. Call after ALL lifted or
/// Rust initializers finish, before any gameplay resumes, on the game thread.
/// Not wired to a live host reload yet; never call to hide entry-time drift.
pub fn reload_after_level_init<M: GuestMemory>(m: &M) {
    resolve_mode();
    if PLAYER.mode() == 0 {
        return;
    }
    PLAYER.ensure_init(m);
    PLAYER.sync_ptrs(m, rebase_log);
    PLAYER.reload_after_level_init(m);
}

fn read<M: GuestMemory>(m: &M, address: u32, width: u32) -> u32 {
    resolve_mode();
    PLAYER.read(m, address, width, rebase_log, |slot, guest, owned| {
        drift_log(slot, 2, guest, owned);
    })
}
/// Converted readers (`tools.c2rust --player-readers`): mode 0 is the plain guest read.
#[inline]
pub fn get32<M: GuestMemory>(m: &M, address: u32) -> u32 {
    if PLAYER.mode() == 0 {
        return m.read32(address);
    }
    read(m, address, 4)
}
#[inline]
pub fn get16<M: GuestMemory>(m: &M, address: u32) -> u16 {
    if PLAYER.mode() == 0 {
        return m.read16(address);
    }
    read(m, address, 2) as u16
}
#[inline]
pub fn get8<M: GuestMemory>(m: &M, address: u32) -> u8 {
    if PLAYER.mode() == 0 {
        return m.read8(address);
    }
    read(m, address, 1) as u8
}

/// Retire-mode store of `width` bytes at `address` (owner and mirror together). Returns false when the address is not in
/// the owned extents: the caller then does the raw store. Cross-word stores resolve every byte. Own: a sub-dword store
/// merges into the OWNED word, audit: into the guest word (the T2118 behaviour).
pub fn store<M: GuestMemory>(m: &mut M, address: u32, value: u32, width: u32, key: u32) -> bool {
    resolve_mode();
    if PLAYER.mode() == 0 || !matches!(width, 1 | 2 | 4) {
        return false;
    }
    PLAYER.ensure_init(m);
    PLAYER.sync_ptrs(m, rebase_log);
    // A store straddling words or an extent boundary must update every owned byte,
    // including when its first byte lies outside the extent. Leave wholly unowned
    // stores to the caller; otherwise forward unowned bytes and route owned ones.
    let crosses = (address % 4) + width > 4;
    if crosses {
        if !(0..width).any(|i| PLAYER.locate(address.wrapping_add(i)).is_some()) {
            return false;
        }
        for i in 0..width {
            let byte_address = address.wrapping_add(i);
            let byte = (value >> (8 * i)) as u8;
            if !store(m, byte_address, u32::from(byte), 1, key) {
                m.write8(byte_address, byte);
            }
        }
        return true;
    }
    let Some(slot) = PLAYER.locate(address) else {
        return false;
    };
    let aligned = address & !3;
    let guest_word = m.read32(aligned);
    let owned_word = PLAYER.word(slot);
    if guest_word != owned_word {
        // a bypassing store happened between the previous owner write of this word and now
        if PLAYER.authoritative(slot) {
            PLAYER.note_drift();
        }
        drift_log(slot, 1, guest_word, owned_word);
    }
    let base = if PLAYER.is_own() && PLAYER.authoritative(slot) {
        owned_word
    } else {
        guest_word
    };
    let shift = 8 * (address % 4);
    let word = if width == 4 {
        value
    } else {
        let mask = ((1u32 << (8 * width)) - 1) << shift;
        (base & !mask) | ((value << shift) & mask)
    };
    let guest_value = crate::state_wire::mutate_state(key, word);
    PLAYER.set_word(slot, mutate_owned(slot, guest_value));
    m.write32(aligned, guest_value);
    true
}

/// Authority-aware memory wrapper for selected hand ports: reads use the retire policy,
/// stores update the owner and mirror, and unowned bytes are forwarded.
pub struct Wired<'a, H: GuestMemory>(pub &'a mut H);

impl<H: GuestMemory> GuestMemory for Wired<'_, H> {
    fn read8(&self, address: u32) -> u8 {
        get8(&*self.0, address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        if !store(self.0, address, u32::from(value), 1, RECORD_KEY) {
            self.0.write8(address, value);
        }
    }
    fn read32(&self, address: u32) -> u32 {
        get32(&*self.0, address)
    }
    fn write32(&mut self, address: u32, value: u32) {
        if !store(self.0, address, value, 4, RECORD_KEY) {
            self.0.write32(address, value);
        }
    }
}

/// Mutation key of a record store (the pointer cell, as in T2118).
pub const RECORD_KEY: u32 = crate::leaf_wire::player::RECORD_CELL;
pub const STATS_KEY: u32 = crate::leaf_wire::player::STATS_CELL;
