// SPDX-License-Identifier: GPL-3.0-or-later
//! T2137: persistent Missions owner. Contract and verification debt: docs/t-s3-missions.md.
//! One game thread; atomics permit a safe static, not concurrent coherent snapshots.
//! Mission/level selection belongs to GameState, script definitions to ModeRecord.
use super::{GuestMemory, missions::*};
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering::Relaxed};

/// All statically enumerated mission writers; callees and dynamic aliases remain trace debt.
pub const REQUIRED_ROOTS: [u32; 11] = [
    0x9ce00, 0x9a140, 0x9a260, 0x9a630, 0x9a7a0, 0x9a5b0, 0x9af90, 0x2fa150, 0x9deb0, 0x9dc70,
    0x23f9d0,
];
pub const CELLS: [u32; 16] = [
    MAIN_PTR,
    LINK_PTR,
    T34_PTR,
    T38_PTR,
    EXTRA_PTR,
    MAIN_COUNT,
    LINK_COUNT,
    T34_COUNT,
    AUX_CELLS[0],
    AUX_CELLS[1],
    AUX_CELLS[2],
    AUX_CELLS[3],
    AUX_CELLS[4],
    AUX_CELLS[5],
    AUX_CELLS[6],
    AUX_CELLS[7],
];
const WORDS: [usize; 4] = [
    (MAIN_CAPACITY * ROW_SIZE / 4) as usize,
    (LINK_CAPACITY * LINK_SIZE / 4) as usize,
    (T34_CAPACITY_LEVEL100 * ROW_SIZE / 4) as usize,
    (T38_CAPACITY * ROW_SIZE / 4) as usize,
];
const TOTAL: usize = WORDS[0] + WORDS[1] + WORDS[2] + WORDS[3];

/// A typed location in mutable Missions state, independent of its current guest backing address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionSlot {
    Cell(usize),
    Table { table: usize, word: usize },
    Visited(usize),
}

/// Owns every word in the four measured allocations, cells and visited bytes.
/// The fifth allocation's layout is unresolved and deliberately stays guest-owned.
pub struct Missions {
    mode: AtomicU8,
    ready: AtomicBool,
    cells: [AtomicU32; 16],
    words: [AtomicU32; TOTAL],
    visited: [AtomicU8; VISITED_SIZE],
    level100: AtomicBool,
    drift: AtomicU32,
    rebases: AtomicU32,
    root: AtomicU32,
}
impl Default for Missions {
    fn default() -> Self {
        Self::new()
    }
}
impl Missions {
    pub const fn new() -> Self {
        Self {
            mode: AtomicU8::new(0),
            ready: AtomicBool::new(false),
            cells: [const { AtomicU32::new(0) }; 16],
            words: [const { AtomicU32::new(0) }; TOTAL],
            visited: [const { AtomicU8::new(0) }; VISITED_SIZE],
            level100: AtomicBool::new(false),
            drift: AtomicU32::new(0),
            rebases: AtomicU32::new(0),
            root: AtomicU32::new(0),
        }
    }
    /// Library-only A/B selector. No host option is introduced. Own refuses incomplete root selection.
    pub fn configure(&self, mode: u8, selected: impl Fn(u32) -> bool) -> Result<(), u32> {
        if mode > 2 {
            return Err(u32::MAX);
        }
        if mode == 1 {
            if let Some(va) = REQUIRED_ROOTS.iter().find(|va| !selected(**va)) {
                return Err(*va);
            }
        }
        self.mode.store(mode, Relaxed);
        self.reset();
        Ok(())
    }
    /// Call when replacing the whole guest image (including same-address reloads).
    pub fn reset(&self) {
        self.ready.store(false, Relaxed);
        self.drift.store(0, Relaxed);
        self.rebases.store(0, Relaxed);
        self.root.store(0, Relaxed);
    }
    /// Named cells are measured; objective counters retain their unresolved raw aux names.
    pub fn cells<M: GuestMemory>(&self, m: &M) -> PickupCells {
        if self.mode() == 0 {
            return PickupCells::load(m);
        }
        self.initialise(m);
        PickupCells {
            ptrs: core::array::from_fn(|i| self.read32(m, CELLS[i])),
            main_count: self.read32(m, MAIN_COUNT),
            link_count: self.read32(m, LINK_COUNT),
            t34_count: self.read32(m, T34_COUNT),
            aux: core::array::from_fn(|i| self.read32(m, AUX_CELLS[i])),
        }
    }
    pub fn pickup_row<M: GuestMemory>(&self, m: &M, table: usize, index: u32) -> Option<PickupRow> {
        if !matches!(table, 0 | 2 | 3) {
            return None;
        }
        self.initialise(m);
        let words = if self.mode() == 0 && table == 2 {
            if m.read32(LEVEL_CELL) == 100 {
                WORDS[2]
            } else {
                (T34_CAPACITY * ROW_SIZE / 4) as usize
            }
        } else {
            self.capacity(table)
        };
        if index >= (words as u32 * 4 / ROW_SIZE) {
            return None;
        }
        let base = self.cells(m).ptrs[table];
        if base == 0 {
            return None;
        }
        let base = base.wrapping_add(index * ROW_SIZE);
        Some(PickupRow {
            base,
            words: core::array::from_fn(|i| self.read32(m, base + i as u32 * 4)),
        })
    }
    pub fn link_row<M: GuestMemory>(&self, m: &M, index: u32) -> Option<LinkRow> {
        if index >= LINK_CAPACITY {
            return None;
        }
        let base = self.cells(m).ptrs[1];
        if base == 0 {
            return None;
        }
        let base = base.wrapping_add(index * LINK_SIZE);
        Some(LinkRow {
            base,
            words: core::array::from_fn(|i| self.read32(m, base + i as u32 * 4)),
        })
    }
    /// Recipe kind-0 audit. Callback includes current and previous root and typed location.
    /// Full allocation scan is deliberately opt-in; callers choose their trace logging sink.
    pub fn audit_entry<M: GuestMemory>(
        &self,
        m: &M,
        root: u32,
        mut report: impl FnMut(u32, u32, MissionSlot, u32, u32),
    ) {
        if self.mode() != 2 {
            return;
        }
        self.initialise(m);
        let previous = self.root.swap(root, Relaxed);
        let mut compare = |slot, address, owned| {
            let guest = m.read32(address);
            if guest != owned {
                self.drift.fetch_add(1, Relaxed);
                report(root, previous, slot, guest, owned);
            }
        };
        for (i, address) in CELLS.iter().enumerate() {
            compare(MissionSlot::Cell(i), *address, self.cells[i].load(Relaxed));
        }
        for table in 0..4 {
            let base = self.cells[table].load(Relaxed);
            if base == 0 {
                continue;
            }
            for word in 0..self.capacity(table) {
                compare(
                    MissionSlot::Table { table, word },
                    base.wrapping_add(word as u32 * 4),
                    self.words[Self::start(table) + word].load(Relaxed),
                );
            }
        }
        for i in 0..VISITED_SIZE {
            let guest = u32::from(m.read8(VISITED + i as u32));
            let owned = u32::from(self.visited[i].load(Relaxed));
            if guest != owned {
                self.drift.fetch_add(1, Relaxed);
                report(root, previous, MissionSlot::Visited(i), guest, owned);
            }
        }
    }
    pub fn mode(&self) -> u8 {
        self.mode.load(Relaxed)
    }
    pub fn drift_count(&self) -> u32 {
        self.drift.load(Relaxed)
    }
    pub fn rebase_count(&self) -> u32 {
        self.rebases.load(Relaxed)
    }
    fn capacity(&self, table: usize) -> usize {
        if table == 2 && !self.level100.load(Relaxed) {
            (T34_CAPACITY * ROW_SIZE / 4) as usize
        } else {
            WORDS[table]
        }
    }
    fn start(table: usize) -> usize {
        WORDS[..table].iter().sum()
    }
    fn load_table<M: GuestMemory>(&self, m: &M, table: usize, base: u32) {
        let start = Self::start(table);
        for word in 0..WORDS[table] {
            let value = if base != 0 && word < self.capacity(table) {
                m.read32(base.wrapping_add(word as u32 * 4))
            } else {
                0
            };
            self.words[start + word].store(value, Relaxed);
        }
    }
    /// Copies the guest image once. Pointer/level changes reload only the affected allocations.
    /// Counts never resize capacity: constructors can append past the currently used prefix.
    pub fn initialise<M: GuestMemory>(&self, m: &M) {
        if self.mode() == 0 {
            return;
        }
        if !self.ready.load(Relaxed) {
            for (cell, address) in self.cells.iter().zip(CELLS) {
                cell.store(m.read32(address), Relaxed);
            }
            self.level100.store(m.read32(LEVEL_CELL) == 0x64, Relaxed);
            for table in 0..4 {
                self.load_table(m, table, self.cells[table].load(Relaxed));
            }
            for (i, byte) in self.visited.iter().enumerate() {
                byte.store(m.read8(VISITED + i as u32), Relaxed);
            }
            self.ready.store(true, Relaxed);
        } else {
            let level100 = m.read32(LEVEL_CELL) == 0x64;
            let resized = level100 != self.level100.swap(level100, Relaxed);
            for (table, address) in PTR_CELLS.iter().enumerate() {
                let base = m.read32(*address);
                let old = self.cells[table].load(Relaxed);
                if base != old || (table == 2 && resized) {
                    self.cells[table].store(base, Relaxed);
                    self.load_table(m, table, base);
                    self.rebases.fetch_add(1, Relaxed);
                }
            }
        }
    }
    pub fn classify(&self, address: u32) -> Option<MissionSlot> {
        if let Some(i) = CELLS.iter().position(|a| address >= *a && address - *a < 4) {
            return Some(MissionSlot::Cell(i));
        }
        if (VISITED..VISITED + VISITED_SIZE as u32).contains(&address) {
            return Some(MissionSlot::Visited((address - VISITED) as usize));
        }
        for table in 0..4 {
            let base = self.cells[table].load(Relaxed);
            // Reject wrapped/unaligned allocations rather than claiming unrelated bytes.
            if base != 0
                && base % 4 == 0
                && address >= base
                && (address - base) / 4 < self.capacity(table) as u32
            {
                return Some(MissionSlot::Table {
                    table,
                    word: ((address - base) / 4) as usize,
                });
            }
        }
        None
    }
    fn byte(&self, slot: MissionSlot, address: u32) -> u8 {
        match slot {
            MissionSlot::Visited(i) => self.visited[i].load(Relaxed),
            MissionSlot::Cell(i) => {
                (self.cells[i].load(Relaxed) >> (8 * (address - CELLS[i]))) as u8
            }
            MissionSlot::Table { table, word } => {
                (self.words[Self::start(table) + word].load(Relaxed) >> (8 * (address % 4))) as u8
            }
        }
    }
    /// Converted readers may consult authority; existing lifted readers still require the mirror.
    pub fn read8<M: GuestMemory>(&self, m: &M, address: u32) -> u8 {
        if self.mode() == 0 {
            return m.read8(address);
        }
        self.initialise(m);
        let Some(slot) = self.classify(address) else {
            return m.read8(address);
        };
        let owned = self.byte(slot, address);
        if self.mode() == 1 {
            return owned;
        }
        let guest = m.read8(address);
        if guest != owned {
            self.drift.fetch_add(1, Relaxed);
        }
        guest
    }
    pub fn read32<M: GuestMemory>(&self, m: &M, address: u32) -> u32 {
        if self.mode() == 0 {
            return m.read32(address);
        }
        u32::from_le_bytes(core::array::from_fn(|i| {
            self.read8(m, address.wrapping_add(i as u32))
        }))
    }
    /// Set one owned byte and its mirror together; return false for guest-owned addresses.
    pub fn set_byte<M: GuestMemory>(&self, m: &mut M, address: u32, value: u8) -> bool {
        self.store(m, address, u32::from(value), 1)
    }
    fn set_byte_loaded<M: GuestMemory>(&self, m: &mut M, address: u32, value: u8) -> bool {
        let Some(slot) = self.classify(address) else {
            return false;
        };
        if self.mode() == 2 && m.read8(address) != self.byte(slot, address) {
            self.drift.fetch_add(1, Relaxed);
        }
        match slot {
            MissionSlot::Visited(i) => self.visited[i].store(value, Relaxed),
            _ => {
                let (cell, shift) = match slot {
                    MissionSlot::Cell(i) => (&self.cells[i], 8 * (address - CELLS[i])),
                    MissionSlot::Table { table, word } => {
                        (&self.words[Self::start(table) + word], 8 * (address % 4))
                    }
                    _ => unreachable!(),
                };
                let old = cell.load(Relaxed);
                cell.store(
                    (old & !(255 << shift)) | (u32::from(value) << shift),
                    Relaxed,
                );
            }
        }
        m.write8(address, value);
        true
    }
    /// Preserves store width in off mode; own/audit split partial and straddling stores.
    /// Pointer stores commit all bytes before reloading the new allocation.
    pub fn store<M: GuestMemory>(&self, m: &mut M, address: u32, value: u32, width: u32) -> bool {
        if self.mode() == 0 || !matches!(width, 1 | 2 | 4) {
            return false;
        }
        self.initialise(m);
        if (0..width).all(|i| self.classify(address.wrapping_add(i)).is_none()) {
            return false;
        }
        // Aligned dword writes are the allocation-fill hot path. Commit a whole typed
        // word at once instead of merging four bytes (same value and mirror order).
        if width == 4 && address % 4 == 0 {
            if let Some(slot) = self.classify(address) {
                let word = match slot {
                    MissionSlot::Cell(i) => Some(&self.cells[i]),
                    MissionSlot::Table { table, word } => {
                        Some(&self.words[Self::start(table) + word])
                    }
                    MissionSlot::Visited(_) => None,
                };
                if let Some(word) = word {
                    if self.mode() == 2 && m.read32(address) != word.load(Relaxed) {
                        self.drift.fetch_add(1, Relaxed);
                    }
                    let old = word.swap(value, Relaxed);
                    m.write32(address, value);
                    if let MissionSlot::Cell(table) = slot {
                        if table < 4 && old != value {
                            self.load_table(m, table, value);
                            self.rebases.fetch_add(1, Relaxed);
                        }
                    }
                    return true;
                }
            }
        }
        // Avoid rebasing between the individual bytes of a pointer write.
        let mut pointers = [0; 4];
        for (i, pointer) in pointers.iter_mut().enumerate() {
            *pointer = self.cells[i].load(Relaxed);
        }
        for i in 0..width {
            let a = address.wrapping_add(i);
            let v = (value >> (8 * i)) as u8;
            // For pointer bytes, update the mirror now and the complete cell below.
            if CELLS[..4].iter().any(|base| a >= *base && a - *base < 4)
                || !self.set_byte_loaded(m, a, v)
            {
                m.write8(a, v);
            }
        }
        for (table, old) in pointers.into_iter().enumerate() {
            let base = m.read32(PTR_CELLS[table]);
            if base != old {
                self.cells[table].store(base, Relaxed);
                self.load_table(m, table, base);
                self.rebases.fetch_add(1, Relaxed);
            }
        }
        true
    }
}
