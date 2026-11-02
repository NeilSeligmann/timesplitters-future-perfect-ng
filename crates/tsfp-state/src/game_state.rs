// SPDX-License-Identifier: GPL-3.0-or-later
//! T2068 (S3-3 of T2051): Rust-owned GameState scalars. Design and honest limits:
//! docs/t-s3-gamestate.md. Stage is Own with write-through: the struct is authoritative for the
//! declared cells, every Rust writer stores the changed bytes into the guest image in the same
//! call, so lifted readers still see the original layout. Bytes around the cells stay guest-owned.
use super::GuestMemory;

pub const LEVEL: u32 = 0x0079_094c;
pub const PLAYERS: u32 = 0x0079_0950;
pub const EXTRA_PLAYERS: u32 = 0x007a_2958;
pub const MODE: u32 = 0x007d_e455;
pub const MODE_SUB: u32 = 0x007d_e456;
pub const FLAGS: u32 = 0x007d_e458;
pub const MATCH: u32 = 0x007d_e30c;
pub const TICK_DIVISOR: u32 = 0x004e_8cec;
pub const TICK_SCALE_BITS: u32 = 0x004e_794c;

/// Level/mode/player-count scalars (INFERRED names from player_system.rs and the lifted readers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameState {
    pub level: u32,
    pub players: u32,
    pub extra_players: u32,
    pub mode: u8,
    pub mode_sub: u8,
    pub flags: u32,
    pub match_state: u32,
    pub tick_divisor: u32,
    pub tick_scale_bits: u32,
}

impl GameState {
    /// Mirror: copies the scalars out of the guest image.
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self {
            level: m.read32(LEVEL),
            players: m.read32(PLAYERS),
            extra_players: m.read32(EXTRA_PLAYERS),
            mode: m.read8(MODE),
            mode_sub: m.read8(MODE_SUB),
            flags: m.read32(FLAGS),
            match_state: m.read32(MATCH),
            tick_divisor: m.read32(TICK_DIVISOR),
            tick_scale_bits: m.read32(TICK_SCALE_BITS),
        }
    }

    /// Writes every declared cell back; no other guest byte is touched.
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        m.write32(LEVEL, self.level);
        m.write32(PLAYERS, self.players);
        m.write32(EXTRA_PLAYERS, self.extra_players);
        m.write8(MODE, self.mode);
        m.write8(MODE_SUB, self.mode_sub);
        m.write32(FLAGS, self.flags);
        m.write32(MATCH, self.match_state);
        m.write32(TICK_DIVISOR, self.tick_divisor);
        m.write32(TICK_SCALE_BITS, self.tick_scale_bits);
    }

    /// Name of the first cell whose guest value differs from the owned one (drift check).
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<&'static str> {
        let g = Self::load(m);
        [
            ("level", g.level != self.level),
            ("players", g.players != self.players),
            ("extra_players", g.extra_players != self.extra_players),
            ("mode", g.mode != self.mode),
            ("mode_sub", g.mode_sub != self.mode_sub),
            ("flags", g.flags != self.flags),
            ("match_state", g.match_state != self.match_state),
            ("tick_divisor", g.tick_divisor != self.tick_divisor),
            ("tick_scale_bits", g.tick_scale_bits != self.tick_scale_bits),
        ]
        .into_iter()
        .find_map(|(name, bad)| bad.then_some(name))
    }

    // Writers with write-through. Each updates the owned value and the guest cell together.
    pub fn set_level<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.level = v;
        m.write32(LEVEL, v);
    }
    pub fn set_players<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.players = v;
        m.write32(PLAYERS, v);
    }
    pub fn set_extra_players<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.extra_players = v;
        m.write32(EXTRA_PLAYERS, v);
    }
    pub fn set_mode<M: GuestMemory>(&mut self, m: &mut M, v: u8) {
        self.mode = v;
        m.write8(MODE, v);
    }
    pub fn set_mode_sub<M: GuestMemory>(&mut self, m: &mut M, v: u8) {
        self.mode_sub = v;
        m.write8(MODE_SUB, v);
    }
    /// 32-bit flag store (the lifted writers use dword stores, some read-modify-write).
    pub fn set_flags<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.flags = v;
        m.write32(FLAGS, v);
    }
    /// Byte store into the low flag byte (the form `story_slice.rs` uses), upper bytes unchanged.
    pub fn set_flags_low8<M: GuestMemory>(&mut self, m: &mut M, v: u8) {
        self.flags = (self.flags & !0xff) | v as u32;
        m.write8(FLAGS, v);
    }
    pub fn set_match_state<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.match_state = v;
        m.write32(MATCH, v);
    }
    pub fn set_tick_words<M: GuestMemory>(&mut self, m: &mut M, divisor: u32, scale_bits: u32) {
        self.tick_divisor = divisor;
        self.tick_scale_bits = scale_bits;
        m.write32(TICK_DIVISOR, divisor);
        m.write32(TICK_SCALE_BITS, scale_bits);
    }

    // Reader helpers mirroring the lifted predicates.
    pub fn in_level(&self) -> bool {
        self.level == 2
    }
    /// 0x64D10 mode check: mode byte is 0xA (party), level not 0x64/0x38, flag bit 6 clear. INFERRED
    /// from the function name; only the byte test is used by tests.
    pub fn is_party(&self) -> bool {
        self.mode == 10
    }
    /// Total player slots the offline limit uses (local plus extra).
    pub fn slot_limit_offline(&self) -> u32 {
        self.players + self.extra_players
    }
}

// ---------------------------------------------------------------------------------------------------------
// T2135 (S3 Retire pilot): the persistent owner. Design and limits: docs/t-s3-retire-gamestate.md.
// ---------------------------------------------------------------------------------------------------------
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

/// The six cells the 24 wired writers (T2082) and the converted readers address. `mode_sub` and the tick
/// words have no wired writer, so they are not part of the persistent owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Level,
    Players,
    ExtraPlayers,
    Mode,
    Flags,
    Match,
}

impl Cell {
    pub const ALL: [Cell; 6] = [
        Cell::Level,
        Cell::Players,
        Cell::ExtraPlayers,
        Cell::Mode,
        Cell::Flags,
        Cell::Match,
    ];
    pub const fn address(self) -> u32 {
        match self {
            Cell::Level => LEVEL,
            Cell::Players => PLAYERS,
            Cell::ExtraPlayers => EXTRA_PLAYERS,
            Cell::Mode => MODE,
            Cell::Flags => FLAGS,
            Cell::Match => MATCH,
        }
    }
    /// Width of the authoritative cell; neighbouring mode bytes remain guest-owned.
    pub const fn width(self) -> u32 {
        match self {
            Cell::Mode => 1,
            _ => 4,
        }
    }
    /// Decode an address at the guest adapter boundary, including interior flag bytes.
    pub fn containing_byte(address: u32) -> Option<(Self, u32)> {
        Self::ALL.into_iter().find_map(|cell| {
            let offset = address.wrapping_sub(cell.address());
            (offset < cell.width()).then_some((cell, offset))
        })
    }
    pub const fn name(self) -> &'static str {
        match self {
            Cell::Level => "level",
            Cell::Players => "players",
            Cell::ExtraPlayers => "extra_players",
            Cell::Mode => "mode",
            Cell::Flags => "flags",
            Cell::Match => "match_state",
        }
    }
    const fn slot(self) -> usize {
        self as usize
    }
    /// The guest cell value (audit tooling).
    pub fn read_guest_pub<M: GuestMemory>(self, m: &M) -> u32 {
        self.read_guest(m)
    }
    fn read_guest<M: GuestMemory>(self, m: &M) -> u32 {
        match self {
            Cell::Mode => m.read8(MODE) as u32,
            _ => m.read32(self.address()),
        }
    }
    fn write_guest<M: GuestMemory>(self, m: &mut M, v: u32) {
        match self {
            Cell::Mode => m.write8(MODE, v as u8),
            _ => m.write32(self.address(), v),
        }
    }
}

/// Retire stage selector. `Off` is the default and changes nothing.
pub const RETIRE_OFF: u8 = 0;
/// Authority: readers return the owned value, writers update owned value and guest mirror.
pub const RETIRE_OWN: u8 = 1;
/// Audit: readers return the guest value (behaviour unchanged) and count every guest/owned mismatch, which
/// enumerates the writers that bypass the owner.
pub const RETIRE_AUDIT: u8 = 2;

/// Persistent GameState owner (Retire pilot). Values live in atomics so a `static` instance is `Sync`; the host
/// runs game logic on one thread, the atomics (Relaxed) only make the static legal and the counters readable
/// from a monitor thread. The first use initialises the owned values from the guest image.
pub struct Retire {
    mode: AtomicU8,
    init: AtomicBool,
    owned: [AtomicU32; 6],
    drift: AtomicU32,
}

impl Default for Retire {
    fn default() -> Self {
        Self::new()
    }
}

impl Retire {
    pub const fn new() -> Self {
        Self {
            mode: AtomicU8::new(RETIRE_OFF),
            init: AtomicBool::new(false),
            owned: [const { AtomicU32::new(0) }; 6],
            drift: AtomicU32::new(0),
        }
    }
    pub fn mode(&self) -> u8 {
        self.mode.load(Ordering::Relaxed)
    }
    pub fn set_mode(&self, mode: u8) {
        self.mode.store(mode, Ordering::Relaxed);
    }
    pub fn is_init(&self) -> bool {
        self.init.load(Ordering::Relaxed)
    }
    /// Forgets the owned values (next use re-initialises from the guest image), e.g. on a guest reset.
    pub fn reset(&self) {
        self.init.store(false, Ordering::Relaxed);
        self.drift.store(0, Ordering::Relaxed);
    }
    /// Initialises the owned values from the guest image on first use only.
    pub fn ensure_init<M: GuestMemory>(&self, m: &M) {
        if !self.init.load(Ordering::Relaxed) {
            for cell in Cell::ALL {
                self.owned[cell.slot()].store(cell.read_guest(m), Ordering::Relaxed);
            }
            self.init.store(true, Ordering::Relaxed);
        }
    }
    /// The owned value (no guest access). Meaningful only after `ensure_init`.
    pub fn owned(&self, cell: Cell) -> u32 {
        self.owned[cell.slot()].load(Ordering::Relaxed)
    }
    /// Number of audit/check mismatches seen so far.
    pub fn drift_count(&self) -> u32 {
        self.drift.load(Ordering::Relaxed)
    }
    /// Typed accessors for converted readers.
    pub fn level<M: GuestMemory>(&self, m: &M) -> u32 {
        self.read(m, Cell::Level, |_, _, _| {})
    }
    pub fn match_state<M: GuestMemory>(&self, m: &M) -> u32 {
        self.read(m, Cell::Match, |_, _, _| {})
    }

    /// Reader. Off: the guest value. Own: the owned value. Audit: the guest value, `on_drift(cell, guest, owned)`
    /// and the drift counter when the owner disagrees.
    pub fn read<M: GuestMemory>(
        &self,
        m: &M,
        cell: Cell,
        on_drift: impl FnOnce(Cell, u32, u32),
    ) -> u32 {
        match self.mode() {
            RETIRE_OFF => cell.read_guest(m),
            RETIRE_OWN => {
                self.ensure_init(m);
                self.owned(cell)
            }
            _ => {
                self.ensure_init(m);
                let guest = cell.read_guest(m);
                let owned = self.owned(cell);
                if guest != owned {
                    self.drift.fetch_add(1, Ordering::Relaxed);
                    on_drift(cell, guest, owned);
                }
                guest
            }
        }
    }

    /// Writer. Off: the plain guest store. Otherwise the owner is initialised from the guest image (before the
    /// store), takes `owned_value`, and the mirror takes `guest_value` (the two differ only in the mutation control).
    pub fn write<M: GuestMemory>(&self, m: &mut M, cell: Cell, guest_value: u32, owned_value: u32) {
        if self.mode() != RETIRE_OFF {
            self.ensure_init(m);
            self.owned[cell.slot()].store(owned_value, Ordering::Relaxed);
        }
        cell.write_guest(m, guest_value);
    }

    /// Byte store into the low flag byte, upper bytes unchanged (the owned upper bytes are kept).
    pub fn write_flags_low8<M: GuestMemory>(&self, m: &mut M, v: u8, owned_low: u8) {
        if self.mode() != RETIRE_OFF {
            self.ensure_init(m);
            let hi = self.owned(Cell::Flags) & !0xff;
            self.owned[Cell::Flags.slot()].store(hi | owned_low as u32, Ordering::Relaxed);
        }
        m.write8(FLAGS, v);
    }

    /// Consistency check: first cell whose guest value differs from the owned one (None when off or uninitialised).
    pub fn check<M: GuestMemory>(&self, m: &M) -> Option<Cell> {
        if self.mode() == RETIRE_OFF || !self.is_init() {
            return None;
        }
        Cell::ALL
            .into_iter()
            .find(|c| c.read_guest(m) != self.owned(*c))
    }
    /// Debug hook: panics naming the cell when the check fails (for tests and debug builds).
    pub fn assert_consistent<M: GuestMemory>(&self, m: &M) {
        if let Some(cell) = self.check(m) {
            panic!(
                "GameState owner drift on {} [{:#x}]: guest {:#x} owned {:#x}",
                cell.name(),
                cell.address(),
                cell.read_guest(m),
                self.owned(cell)
            );
        }
    }
}
