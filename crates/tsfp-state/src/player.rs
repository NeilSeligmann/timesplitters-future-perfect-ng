// SPDX-License-Identifier: GPL-3.0-or-later
//! T2073 (S3-7 of T2051): Rust-owned player set. Design and honest limits: docs/t-s3-player.md.
//! Stage is Own with write-through: the owned words are authoritative for the declared extents, every
//! Rust setter stores the changed bytes into the guest image in the same call, so lifted readers
//! still see the original layout. Field names are INFERRED (player_system.rs, TsfpPlayerPrefix).
//!
//! Three pointer cells select three heap records (MEASURED on the Story/engage route, T2071 dumps):
//! the player record `[0x7B0C7C]` (0x2000 bytes owned), the controlled object `[0x7B0C48]` (0x1584
//! bytes, the SAME address as the record in all 966 samples, so the controlled object is a prefix
//! view of the record) and the stats table `[0x75CCC0]` (0x13A0 bytes, f32 cells).
use super::GuestMemory;
use super::generated_views::{TsfpPlayerPrefixOwned, TsfpPlayerPrefixView};

pub const RECORD_CELL: u32 = 0x007b_0c7c;
pub const CONTROLLED_CELL: u32 = 0x007b_0c48;
pub const STATS_CELL: u32 = 0x0075_ccc0;
pub const RECORD_SIZE: u32 = 0x2000;
pub const CONTROLLED_SIZE: u32 = 0x1584;
pub const STATS_SIZE: u32 = 0x13a0;
/// f32 stat cells per player in the stats table (`base + 4 * (player * 0x273 + stat)`).
pub const STATS_PER_PLAYER: u32 = 0x273;
const RECORD_WORDS: usize = (RECORD_SIZE / 4) as usize;
const STATS_WORDS: usize = (STATS_SIZE / 4) as usize;

/// The first field that differs between the owned value and the guest image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerDiff {
    /// A pointer cell (`"record"`, `"controlled"` or `"stats"`).
    Cell(&'static str),
    /// Byte offset into the player record.
    Record(u32),
    /// Byte offset into the stats table.
    Stats(u32),
}

/// Owner of the player record, the stats table and the controlled-object cell.
/// A zero pointer means the record does not exist (menus): nothing is loaded or stored for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerSet {
    pub record_ptr: u32,
    pub controlled_ptr: u32,
    pub stats_ptr: u32,
    pub record: [u32; RECORD_WORDS],
    pub stats: [u32; STATS_WORDS],
}

impl PlayerSet {
    pub const EMPTY: Self = Self {
        record_ptr: 0,
        controlled_ptr: 0,
        stats_ptr: 0,
        record: [0; RECORD_WORDS],
        stats: [0; STATS_WORDS],
    };

    /// Mirror: copies the three cells and both records out of the guest image.
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        let mut s = Self::EMPTY;
        s.record_ptr = m.read32(RECORD_CELL);
        s.controlled_ptr = m.read32(CONTROLLED_CELL);
        s.stats_ptr = m.read32(STATS_CELL);
        s.load_bodies(m);
        s
    }

    /// Mirror of the three pointer cells only (bodies stay zero): T2118, for write-through setters that
    /// read the word they merge from the guest image (`set_record_word` writes owned word and guest).
    pub fn load_cells<M: GuestMemory>(m: &M) -> Self {
        let mut s = Self::EMPTY;
        s.record_ptr = m.read32(RECORD_CELL);
        s.controlled_ptr = m.read32(CONTROLLED_CELL);
        s.stats_ptr = m.read32(STATS_CELL);
        s
    }

    /// Re-reads the record and stats bodies at the current owned pointers (what a hybrid tick does
    /// after a lifted writer ran).
    pub fn load_bodies<M: GuestMemory>(&mut self, m: &M) {
        if self.record_ptr != 0 {
            for (i, w) in self.record.iter_mut().enumerate() {
                *w = m.read32(self.record_ptr.wrapping_add(i as u32 * 4));
            }
        }
        if self.stats_ptr != 0 {
            for (i, w) in self.stats.iter_mut().enumerate() {
                *w = m.read32(self.stats_ptr.wrapping_add(i as u32 * 4));
            }
        }
    }

    /// Writes the cells and both bodies back; no other guest byte is touched.
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        m.write32(RECORD_CELL, self.record_ptr);
        m.write32(CONTROLLED_CELL, self.controlled_ptr);
        m.write32(STATS_CELL, self.stats_ptr);
        if self.record_ptr != 0 {
            for (i, w) in self.record.iter().enumerate() {
                m.write32(self.record_ptr.wrapping_add(i as u32 * 4), *w);
            }
        }
        if self.stats_ptr != 0 {
            for (i, w) in self.stats.iter().enumerate() {
                m.write32(self.stats_ptr.wrapping_add(i as u32 * 4), *w);
            }
        }
    }

    /// First owned field whose guest bytes differ.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<PlayerDiff> {
        for (cell, addr, owned) in [
            ("record", RECORD_CELL, self.record_ptr),
            ("controlled", CONTROLLED_CELL, self.controlled_ptr),
            ("stats", STATS_CELL, self.stats_ptr),
        ] {
            if m.read32(addr) != owned {
                return Some(PlayerDiff::Cell(cell));
            }
        }
        if self.record_ptr != 0 {
            for (i, w) in self.record.iter().enumerate() {
                if m.read32(self.record_ptr.wrapping_add(i as u32 * 4)) != *w {
                    return Some(PlayerDiff::Record(i as u32 * 4));
                }
            }
        }
        if self.stats_ptr != 0 {
            for (i, w) in self.stats.iter().enumerate() {
                if m.read32(self.stats_ptr.wrapping_add(i as u32 * 4)) != *w {
                    return Some(PlayerDiff::Stats(i as u32 * 4));
                }
            }
        }
        None
    }

    /// True when the controlled object is the player record (measured: always, on the engage route).
    pub fn controlled_is_record(&self) -> bool {
        self.controlled_ptr != 0 && self.controlled_ptr == self.record_ptr
    }

    /// Prefix view of the controlled object (pose and facing, INFERRED names).
    pub fn controlled_view(&self) -> TsfpPlayerPrefixView {
        TsfpPlayerPrefixView(self.controlled_ptr)
    }

    /// Pose (+0xA8, three f32) and facing (+0xC0) taken from the owned record words.
    pub fn prefix(&self) -> TsfpPlayerPrefixOwned {
        let f = |off: u32| f32::from_bits(self.record[(off / 4) as usize]);
        TsfpPlayerPrefixOwned {
            pose: [f(0xa8), f(0xac), f(0xb0)],
            facing: f(0xc0),
        }
    }

    pub fn record_word(&self, offset: u32) -> u32 {
        self.record[(offset / 4) as usize]
    }

    /// Writer with write-through: sets one dword of the player record. Offset must be 4-aligned and
    /// inside 0x2000. Returns false (nothing written) when there is no record.
    pub fn set_record_word<M: GuestMemory>(&mut self, m: &mut M, offset: u32, value: u32) -> bool {
        assert!(
            offset % 4 == 0 && offset < RECORD_SIZE,
            "record offset {offset:#x}"
        );
        if self.record_ptr == 0 {
            return false;
        }
        self.record[(offset / 4) as usize] = value;
        m.write32(self.record_ptr.wrapping_add(offset), value);
        true
    }

    /// Writer: pose (+0xA8) and facing (+0xC0), bit-exact f32.
    pub fn set_pose<M: GuestMemory>(&mut self, m: &mut M, pose: [f32; 3]) -> bool {
        (0..3).all(|i| self.set_record_word(m, 0xa8 + 4 * i, pose[i as usize].to_bits()))
    }
    pub fn set_facing<M: GuestMemory>(&mut self, m: &mut M, facing: f32) -> bool {
        self.set_record_word(m, 0xc0, facing.to_bits())
    }

    /// Byte offset of stat `stat` of `player` in the stats table, None when outside 0x13A0.
    pub fn stat_offset(player: u32, stat: u32) -> Option<u32> {
        let offset = player
            .checked_mul(STATS_PER_PLAYER)?
            .checked_add(stat)?
            .checked_mul(4)?;
        (offset < STATS_SIZE).then_some(offset)
    }

    pub fn stat_bits(&self, player: u32, stat: u32) -> Option<u32> {
        Self::stat_offset(player, stat).map(|o| self.stats[(o / 4) as usize])
    }

    /// Writer with write-through for one stats cell (what 0x238510 and 0x2385B0 do, 16 fields).
    pub fn set_stat<M: GuestMemory>(
        &mut self,
        m: &mut M,
        player: u32,
        stat: u32,
        bits: u32,
    ) -> bool {
        match (Self::stat_offset(player, stat), self.stats_ptr) {
            (Some(offset), ptr) if ptr != 0 => {
                self.stats[(offset / 4) as usize] = bits;
                m.write32(ptr.wrapping_add(offset), bits);
                true
            }
            _ => false,
        }
    }

    /// Writer: select the controlled object (the one static write of `[0x7B0C48]`).
    pub fn set_controlled<M: GuestMemory>(&mut self, m: &mut M, ptr: u32) {
        self.controlled_ptr = ptr;
        m.write32(CONTROLLED_CELL, ptr);
    }
}

// ---------------------------------------------------------------------------------------------------------
// T2138 (S3 Retire pilot 2): the persistent PlayerSet owner. Design and limits: docs/t-s3-retire-player.md.
// ---------------------------------------------------------------------------------------------------------
use super::game_state::{RETIRE_OFF, RETIRE_OWN};
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

/// Record byte offsets that the owner does NOT claim in own mode: a lifted writer that is not a lane root writes them behind
/// the owner (measured by the audit on the engage and Arcade routes, attributed in docs/t-s3-retire-player.md: the ray cast
/// `0x88950`, the math helpers `0xCF510 0xE1200 0x151E80`, the animation port `0x5A360`, the weapon slot helpers, record init
/// and writers the T2057 csv never saw). Converted readers of such a word fall through to the guest value, the owner keeps
/// tracking it and the audit reports its drift as "expected" (kind 4). Sorted.
pub const PASSTHROUGH_RECORD_OFFSETS: &[u32] = &[
    0xc, 0x8c, 0xa0, 0xa8, 0xac, 0xb0, 0xb4, 0xb8, 0xbc, 0xc0, 0xc4, 0xc8, 0xf4, 0xf8, 0xfc, 0x100,
    0x104, 0x110, 0x114, 0x14c, 0x150, 0x154, 0x158, 0x15c, 0x160, 0x1b4, 0x1b8, 0x1c4, 0x1c8,
    0x1cc, 0x1d8, 0x22c, 0x230, 0x23c, 0x26c, 0x270, 0x274, 0x27c, 0x55c, 0x560, 0x564, 0x58c,
    0x590, 0x59c, 0x5a0, 0x5a4, 0x5b0, 0x5bc, 0x5c4, 0x5f8, 0x5fc, 0x604, 0x608, 0x610, 0x614,
    0x620, 0x644, 0x648, 0x64c, 0x654, 0xdf8, 0xdfc, 0xe00, 0xe20, 0xe24, 0xe28, 0xe2c, 0xe30,
    0xe34, 0xe38, 0xea4, 0xea8, 0xeac, 0xeb0, 0xeb4, 0xeb8, 0xebc, 0xec0, 0xec4, 0xec8, 0xed8,
    0xef4, 0xf60, 0xf88, 0xf8c, 0xf90, 0xf94, 0xf98, 0xf9c, 0xfb0, 0xfb4, 0xfbc, 0xfc0, 0xfe4,
    0xfec, 0xff0, 0x100c, 0x1010, 0x1018, 0x1508, 0x1568, 0x156c, 0x17e8, 0x17ec, 0x1808, 0x180c,
    0x1818, 0x181c, 0x1838, 0x183c,
];
/// Stats table byte offsets with the same meaning (the stats writers other than `0x238510 0x2385B0`). Sorted.
pub const PASSTHROUGH_STATS_OFFSETS: &[u32] = &[
    0x4, 0x10, 0x2c, 0xb0, 0xb4, 0x114, 0x118, 0x11c, 0x760, 0x768, 0x9cc, 0x9d0, 0x9d8, 0x9dc,
    0x9e0, 0x9e4, 0x9e8, 0x9f4, 0x9f8, 0xa14, 0xa18, 0xa1c, 0xa7c, 0xa80, 0xac0, 0xae0, 0xae4,
    0xae8, 0x10fc, 0x1104, 0x1114, 0x1118, 0x111c, 0x1128, 0x1130, 0x1134, 0x1148, 0x1158, 0x1160,
    0x1164, 0x1338, 0x133c, 0x1340, 0x138c, 0x1390, 0x1394, 0x1398,
];

/// Where an address falls in the owned extents: record word index or stats word index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Record(usize),
    Stats(usize),
}

impl Slot {
    /// Byte offset used in logs: record offset, or `0x10000 | stats offset`.
    pub fn log_offset(self) -> u32 {
        match self {
            Slot::Record(i) => i as u32 * 4,
            Slot::Stats(i) => 0x1_0000 | (i as u32 * 4),
        }
    }
}

/// Index of the pointer cells in `PlayerRetire::ptr` (record, controlled object, stats table).
pub const PTR_RECORD: usize = 0;
pub const PTR_CONTROLLED: usize = 1;
pub const PTR_STATS: usize = 2;
const PTR_CELLS: [u32; 3] = [RECORD_CELL, CONTROLLED_CELL, STATS_CELL];

/// Persistent PlayerSet owner. Same atomics-in-a-static shape as `game_state::Retire`. The owner keeps the three
/// pointer values it believes and the two bodies. **Re-base rule:** the guest pointer cells are the source of truth for
/// WHERE the bodies live (the record is reallocated at level load by lifted code); whenever `sync_ptrs` sees a cell
/// differ from the owned pointer it adopts the new pointer and reloads that body from the guest image at the new address.
pub struct PlayerRetire {
    mode: AtomicU8,
    init: AtomicBool,
    ptr: [AtomicU32; 3],
    record: [AtomicU32; RECORD_WORDS],
    stats: [AtomicU32; STATS_WORDS],
    drift: AtomicU32,
    rebases: AtomicU32,
}

impl Default for PlayerRetire {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerRetire {
    pub const fn new() -> Self {
        Self {
            mode: AtomicU8::new(RETIRE_OFF),
            init: AtomicBool::new(false),
            ptr: [const { AtomicU32::new(0) }; 3],
            record: [const { AtomicU32::new(0) }; RECORD_WORDS],
            stats: [const { AtomicU32::new(0) }; STATS_WORDS],
            drift: AtomicU32::new(0),
            rebases: AtomicU32::new(0),
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
    /// Forgets the owned values (the next use re-initialises from the guest image).
    pub fn reset(&self) {
        self.init.store(false, Ordering::Relaxed);
        self.drift.store(0, Ordering::Relaxed);
        self.rebases.store(0, Ordering::Relaxed);
    }
    pub fn drift_count(&self) -> u32 {
        self.drift.load(Ordering::Relaxed)
    }
    pub fn rebase_count(&self) -> u32 {
        self.rebases.load(Ordering::Relaxed)
    }
    pub fn note_drift(&self) {
        self.drift.fetch_add(1, Ordering::Relaxed);
    }
    pub fn ptr(&self, which: usize) -> u32 {
        self.ptr[which].load(Ordering::Relaxed)
    }

    fn load_record<M: GuestMemory>(&self, m: &M) {
        let base = self.ptr(PTR_RECORD);
        if base != 0 {
            for (i, w) in self.record.iter().enumerate() {
                w.store(m.read32(base.wrapping_add(i as u32 * 4)), Ordering::Relaxed);
            }
        }
    }
    fn load_stats<M: GuestMemory>(&self, m: &M) {
        let base = self.ptr(PTR_STATS);
        if base != 0 {
            for (i, w) in self.stats.iter().enumerate() {
                w.store(m.read32(base.wrapping_add(i as u32 * 4)), Ordering::Relaxed);
            }
        }
    }

    /// First use: copies the three cells and both bodies out of the guest image.
    pub fn ensure_init<M: GuestMemory>(&self, m: &M) {
        if !self.is_init() {
            for (i, cell) in PTR_CELLS.into_iter().enumerate() {
                self.ptr[i].store(m.read32(cell), Ordering::Relaxed);
            }
            self.load_record(m);
            self.load_stats(m);
            self.init.store(true, Ordering::Relaxed);
        }
    }

    /// Explicit completed-load boundary, including same-address reallocation.
    /// Call only on the single game thread AFTER all record/stats initialization
    /// and BEFORE gameplay readers resume. This is never an entry-time drift
    /// repair: the caller must establish the load boundary. Off mode stays inert.
    /// Diagnostic counters survive; pointer changes remain separately observable.
    pub fn reload_after_level_init<M: GuestMemory>(&self, m: &M) {
        if self.mode() == RETIRE_OFF {
            return;
        }
        for (i, cell) in PTR_CELLS.into_iter().enumerate() {
            self.ptr[i].store(m.read32(cell), Ordering::Relaxed);
        }
        self.load_record(m);
        self.load_stats(m);
        self.init.store(true, Ordering::Relaxed);
    }

    /// Re-base check: compares the guest pointer cells with the owned pointers. For every cell that changed, adopts the
    /// new pointer (record and stats also reload their body at the new address) and calls
    /// `on_rebase(cell index, old pointer, new pointer)`. Returns the number of changed cells.
    pub fn sync_ptrs<M: GuestMemory>(
        &self,
        m: &M,
        mut on_rebase: impl FnMut(usize, u32, u32),
    ) -> u32 {
        let mut changed = 0;
        for (i, cell) in PTR_CELLS.into_iter().enumerate() {
            let new = m.read32(cell);
            let old = self.ptr[i].swap(new, Ordering::Relaxed);
            if old != new {
                changed += 1;
                self.rebases.fetch_add(1, Ordering::Relaxed);
                match i {
                    PTR_RECORD => self.load_record(m),
                    PTR_STATS => self.load_stats(m),
                    _ => {}
                }
                on_rebase(i, old, new);
            }
        }
        changed
    }

    /// Slot of a guest address in the owned record or stats extent (None: not owned, a plain guest access).
    pub fn locate(&self, address: u32) -> Option<Slot> {
        let record = self.ptr(PTR_RECORD);
        if record != 0 {
            let offset = address.wrapping_sub(record);
            if offset < RECORD_SIZE {
                return Some(Slot::Record((offset / 4) as usize));
            }
        }
        let stats = self.ptr(PTR_STATS);
        if stats != 0 {
            let offset = address.wrapping_sub(stats);
            if offset < STATS_SIZE {
                return Some(Slot::Stats((offset / 4) as usize));
            }
        }
        None
    }
    pub fn word(&self, slot: Slot) -> u32 {
        match slot {
            Slot::Record(i) => self.record[i].load(Ordering::Relaxed),
            Slot::Stats(i) => self.stats[i].load(Ordering::Relaxed),
        }
    }
    pub fn set_word(&self, slot: Slot, value: u32) {
        match slot {
            Slot::Record(i) => self.record[i].store(value, Ordering::Relaxed),
            Slot::Stats(i) => self.stats[i].store(value, Ordering::Relaxed),
        }
    }
    /// True when the owned word answers converted readers in own mode (false for the pass-through lists) and its drift counts.
    pub fn authoritative(&self, slot: Slot) -> bool {
        match slot {
            Slot::Record(i) => PASSTHROUGH_RECORD_OFFSETS
                .binary_search(&(i as u32 * 4))
                .is_err(),
            Slot::Stats(i) => PASSTHROUGH_STATS_OFFSETS
                .binary_search(&(i as u32 * 4))
                .is_err(),
        }
    }

    /// Value of a `width` byte read at `address` out of the owned word (None: the read crosses a dword).
    fn extract(word: u32, address: u32, width: u32) -> Option<u32> {
        let shift = 8 * (address % 4);
        if (address % 4) + width > 4 {
            return None;
        }
        Some(if width == 4 {
            word
        } else {
            (word >> shift) & ((1u32 << (8 * width)) - 1)
        })
    }
    fn guest_read<M: GuestMemory>(m: &M, address: u32, width: u32) -> u32 {
        match width {
            1 => m.read8(address) as u32,
            2 => m.read8(address) as u32 | (m.read8(address.wrapping_add(1)) as u32) << 8,
            _ => m.read32(address),
        }
    }

    /// Converted reader of `width` bytes at `address`. Off: the guest value. Own: the owned word when the address is in the
    /// owned extent (after the re-base check), else the guest value. Audit: the guest value, and `on_drift(slot, guest,
    /// owned)` plus the drift counter when the owned extent disagrees.
    pub fn read<M: GuestMemory>(
        &self,
        m: &M,
        address: u32,
        width: u32,
        on_rebase: impl FnMut(usize, u32, u32),
        mut on_drift: impl FnMut(Slot, u32, u32),
    ) -> u32 {
        // Unaligned reads may span two words, a pass-through word, or the end
        // of an extent. Resolve each byte under the same authority policy rather
        // than silently serving the whole read from the guest mirror.
        if matches!(width, 2 | 4) && (address % 4) + width > 4 {
            if self.mode() != RETIRE_OFF {
                self.ensure_init(m);
                self.sync_ptrs(m, on_rebase);
            }
            let mut value = 0;
            for i in 0..width {
                let byte_address = address.wrapping_add(i);
                let byte = if self.mode() != RETIRE_OFF && self.locate(byte_address).is_some() {
                    // Audit a complete owned word so its bypass is reported and
                    // adopted once, even when several requested bytes share it.
                    let word =
                        self.read_fragment(m, byte_address & !3, 4, |_, _, _| {}, &mut on_drift);
                    (word >> (8 * (byte_address % 4))) & 0xff
                } else {
                    u32::from(m.read8(byte_address))
                };
                value |= byte << (8 * i);
            }
            return value;
        }
        self.read_fragment(m, address, width, on_rebase, on_drift)
    }

    fn read_fragment<M: GuestMemory>(
        &self,
        m: &M,
        address: u32,
        width: u32,
        on_rebase: impl FnMut(usize, u32, u32),
        mut on_drift: impl FnMut(Slot, u32, u32),
    ) -> u32 {
        match self.mode() {
            RETIRE_OFF => Self::guest_read(m, address, width),
            RETIRE_OWN => {
                self.ensure_init(m);
                self.sync_ptrs(m, on_rebase);
                match self.locate(address) {
                    Some(slot) if self.authoritative(slot) => {
                        Self::extract(self.word(slot), address, width)
                            .unwrap_or_else(|| Self::guest_read(m, address, width))
                    }
                    _ => Self::guest_read(m, address, width),
                }
            }
            _ => {
                self.ensure_init(m);
                self.sync_ptrs(m, on_rebase);
                let guest = Self::guest_read(m, address, width);
                if let Some(slot) = self.locate(address) {
                    if let Some(owned) = Self::extract(self.word(slot), address, width) {
                        if owned != guest {
                            if self.authoritative(slot) {
                                self.note_drift();
                            }
                            on_drift(slot, guest, owned);
                            if width == 4 {
                                // report each bypassing store once: adopt the guest word (audit never answers from the owner)
                                self.set_word(slot, guest);
                            }
                        }
                    }
                }
                guest
            }
        }
    }

    /// Compares the whole owned extent with the guest image (audit entry scan). For every differing word calls
    /// `on_drift(slot, guest, owned)` and, when `resync`, adopts the guest word so each bypassing store is reported once.
    /// Returns the number of differing authoritative words (pass-through words are reported to `on_drift` but not counted).
    pub fn scan<M: GuestMemory>(
        &self,
        m: &M,
        resync: bool,
        mut on_drift: impl FnMut(Slot, u32, u32),
    ) -> u32 {
        let mut count = 0;
        for (slot_of, base, words) in [
            (
                Slot::Record as fn(usize) -> Slot,
                self.ptr(PTR_RECORD),
                &self.record[..],
            ),
            (
                Slot::Stats as fn(usize) -> Slot,
                self.ptr(PTR_STATS),
                &self.stats[..],
            ),
        ] {
            if base == 0 {
                continue;
            }
            for (i, w) in words.iter().enumerate() {
                let guest = m.read32(base.wrapping_add(i as u32 * 4));
                let owned = w.load(Ordering::Relaxed);
                if guest != owned {
                    let slot = slot_of(i);
                    if self.authoritative(slot) {
                        count += 1;
                        self.note_drift();
                    }
                    on_drift(slot, guest, owned);
                    if resync {
                        w.store(guest, Ordering::Relaxed);
                    }
                }
            }
        }
        count
    }

    /// Own mode (and only own mode) makes the owner answer reads.
    pub fn is_own(&self) -> bool {
        self.mode() == RETIRE_OWN
    }
}
