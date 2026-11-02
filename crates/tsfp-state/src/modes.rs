// SPDX-License-Identifier: GPL-3.0-or-later
//! T2075 (S3-8 of T2051): Rust-owned mode setup records. Design and honest limits:
//! docs/t-s3-modes.md. `ModeRows` selects the mode record M from the immutable row table
//! (0x515FFC, 87 rows of 0x80, ports of 0x6A7B0, 0x6A810 and 0x6A850), `ModeRecord` owns the 0x154-byte record itself with write-through
//! setters, `ModeSetup` couples the record with the cached pointer cell `0x6F32F0`. The mode
//! scalars (`0x7DE455/56/58`, `0x7DE30C`, level) stay in `GameState` (T2068) and are not repeated.
//! Layout from docs/t-mission-script-engine.md and docs/t1824-spawn-record-layout.md; every name is
//! INFERRED unless the doc says MEASURED, unnamed words stay raw.
use super::GuestMemory;

pub const ROW_TABLE: u32 = 0x0051_5ffc;
pub const ROW_STRIDE: u32 = 0x80;
pub const ROW_COUNT: u32 = 87;
/// Row word +0x4: mode id compared with the level cell.
pub const ROW_MODE_ID: u32 = 0x4;
/// Row pointers to M (`0x6A7B0` picks A when level <= 0x64 and the select flag is set, else B;
/// `0x6A810` always A).
pub const ROW_RECORD_A: u32 = 0x20;
pub const ROW_RECORD_B: u32 = 0x24;
/// Level cell (GameState::LEVEL) and the select flag read by `0x6A7B0`.
pub const LEVEL_CELL: u32 = super::game_state::LEVEL;
pub const SELECT_FLAG: u32 = 0x007d_e494;
pub const SELECT_LEVEL_MAX: u32 = 0x64;
/// Memo cells of `0x6A850`: last argument and last mode byte.
pub const MEMO_ARG: u32 = 0x004d_2ce4;
pub const MEMO_MODE: u32 = 0x004d_2ce8;
pub const MODE_CELL: u32 = super::game_state::MODE;
/// Mode byte value that routes `0x6A850` to slot A only (Story).
pub const STORY_MODE: u32 = 0xa;
/// Cached current mode record pointer written by `0x6A850`.
pub const CURRENT_CELL: u32 = 0x006f_32f0;
/// `0x23F8DA` clears the BSS record with 0x55 dwords.
pub const RECORD_WORDS: usize = 0x55;
pub const RECORD_SIZE: u32 = 0x154;

/// Word index of a record offset.
const fn w(offset: u32) -> usize {
    (offset / 4) as usize
}
pub const SPAWN_BASE: usize = w(0xc);
pub const SPAWN_COUNT: usize = w(0x10);
pub const GROUP_BOUND: usize = w(0xd8);
/// SETU story event definitions (stride 0xC8): pointer at +0xCC, count at +0xD0 (MEASURED, T2075).
pub const DEFS_BASE: usize = w(0xcc);
pub const DEFS_COUNT: usize = w(0xd0);
pub const LIST9C_BASE: usize = w(0x9c);
pub const LIST9C_COUNT: usize = w(0xa0);
pub const RESOURCE_NAME: usize = w(0x11c);
pub const SETUP8C_COUNT: usize = w(0x124);
pub const SETUP8C_BASE: usize = w(0x128);
pub const SETUPB4_COUNT: usize = w(0x12c);
pub const SETUPB4_BASE: usize = w(0x130);
pub const RULE_COUNT: usize = w(0x134);
pub const RULE_BASE: usize = w(0x138);
pub const SCRIPT_FLAGS: usize = w(0x150);

/// One mode row (the immutable table; setters exist only for tests and the replay).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModeRow {
    pub marker: u32,
    pub mode_id: u32,
    pub record_a: u32,
    pub record_b: u32,
}

pub struct ModeRows;

impl ModeRows {
    pub fn row_address(index: u32) -> u32 {
        ROW_TABLE + index * ROW_STRIDE
    }

    pub fn row<M: GuestMemory>(m: &M, index: u32) -> ModeRow {
        let a = Self::row_address(index);
        ModeRow {
            marker: m.read32(a),
            mode_id: m.read32(a + ROW_MODE_ID),
            record_a: m.read32(a + ROW_RECORD_A),
            record_b: m.read32(a + ROW_RECORD_B),
        }
    }

    /// Index of the first row (up to `ROW_COUNT`, zero marker terminates) whose id is `level`.
    pub fn find<M: GuestMemory>(m: &M, level: u32) -> Option<u32> {
        for i in 0..ROW_COUNT {
            let row = Self::row(m, i);
            if row.marker == 0 {
                return None;
            }
            if row.mode_id == level {
                return Some(i);
            }
        }
        None
    }

    /// Port of `0x6A810` (MEASURED from the disassembly): slot A of the row whose id is `arg`.
    pub fn select_a<M: GuestMemory>(m: &M, arg: u32) -> Option<u32> {
        let row = Self::row(m, Self::find(m, arg)?);
        (row.record_a != 0).then_some(row.record_a)
    }

    /// Port of `0x6A7B0` (MEASURED from the disassembly): the row is matched by `arg`, then slot A
    /// when the level cell is <= 0x64 and the select flag is nonzero, else slot B.
    pub fn select<M: GuestMemory>(m: &M, arg: u32, level: u32, select_flag: u32) -> Option<u32> {
        let row = Self::row(m, Self::find(m, arg)?);
        let p = if level as i32 <= SELECT_LEVEL_MAX as i32 && select_flag != 0 {
            row.record_a
        } else {
            row.record_b
        };
        (p != 0).then_some(p)
    }
}

/// The 0x154-byte mode record. `base` is its guest address (static `.data` record or the BSS one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeRecord {
    pub base: u32,
    pub words: [u32; RECORD_WORDS],
}

impl ModeRecord {
    pub fn load<M: GuestMemory>(m: &M, base: u32) -> Self {
        let mut words = [0u32; RECORD_WORDS];
        for (i, v) in words.iter_mut().enumerate() {
            *v = m.read32(base + 4 * i as u32);
        }
        Self { base, words }
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, v) in self.words.iter().enumerate() {
            m.write32(self.base + 4 * i as u32, *v);
        }
    }

    /// First drifted word index, `None` when the guest bytes equal the owned words.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<usize> {
        (0..RECORD_WORDS).find(|&i| m.read32(self.base + 4 * i as u32) != self.words[i])
    }

    /// Write-through store of one word.
    pub fn set_word<M: GuestMemory>(&mut self, m: &mut M, word: usize, v: u32) {
        self.words[word] = v;
        m.write32(self.base + 4 * word as u32, v);
    }

    /// Write-through store of a (count, base) pair, the shape of all script and spawn lists.
    pub fn set_list<M: GuestMemory>(
        &mut self,
        m: &mut M,
        count_word: usize,
        count: u32,
        base: u32,
    ) {
        self.set_word(m, count_word, count);
        self.set_word(m, count_word + 1, base);
    }

    pub fn setups_8c(&self) -> (u32, u32) {
        (self.words[SETUP8C_COUNT], self.words[SETUP8C_BASE])
    }
    pub fn setups_b4(&self) -> (u32, u32) {
        (self.words[SETUPB4_COUNT], self.words[SETUPB4_BASE])
    }
    pub fn rules(&self) -> (u32, u32) {
        (self.words[RULE_COUNT], self.words[RULE_BASE])
    }
    pub fn spawn_list(&self) -> (u32, u32) {
        (self.words[SPAWN_BASE], self.words[SPAWN_COUNT])
    }
    pub fn defs(&self) -> (u32, u32) {
        (self.words[DEFS_BASE], self.words[DEFS_COUNT])
    }
    pub fn group_bound(&self) -> u32 {
        self.words[GROUP_BOUND]
    }
    /// True once the level loader has filled any script or spawn list (static image: all zero).
    pub fn is_populated(&self) -> bool {
        self.setups_8c().0 != 0
            || self.setups_b4().0 != 0
            || self.rules().0 != 0
            || self.words[SPAWN_COUNT] != 0
            || self.words[DEFS_COUNT] != 0
    }

    /// Mirrors `0x23F8DA`: clears the whole record (the BSS Map Maker record).
    pub fn clear<M: GuestMemory>(&mut self, m: &mut M) {
        for i in 0..RECORD_WORDS {
            self.set_word(m, i, 0);
        }
    }
}

/// Selected record plus the cached pointer cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeSetup {
    pub current: u32,
    pub record: Option<ModeRecord>,
}

impl ModeSetup {
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        let current = m.read32(CURRENT_CELL);
        Self {
            current,
            record: (current != 0).then(|| ModeRecord::load(m, current)),
        }
    }

    /// Mirror of the pointer cell only (T2091): the record body behind it is not read, so a writer that stores a
    /// pointer it does not own yet (0x6A850 caches whatever the selector returned) never dereferences it.
    pub fn load_pointer<M: GuestMemory>(m: &M) -> Self {
        Self {
            current: m.read32(CURRENT_CELL),
            record: None,
        }
    }

    /// Write-through store of the pointer cell without reloading the record behind it (see `load_pointer`).
    pub fn set_pointer<M: GuestMemory>(&mut self, m: &mut M, base: u32) {
        self.current = base;
        self.record = None;
        m.write32(CURRENT_CELL, base);
    }

    /// Write-through cache of the pointer cell and reload of the record behind it.
    pub fn set_current<M: GuestMemory>(&mut self, m: &mut M, base: u32) {
        self.current = base;
        m.write32(CURRENT_CELL, base);
        self.record = (base != 0).then(|| ModeRecord::load(m, base));
    }

    /// Write-through store of the two memo cells of `0x6A850` (mode byte first, then arg, as the original).
    pub fn set_memo<M: GuestMemory>(&mut self, m: &mut M, arg: u32, mode: u32) {
        m.write32(MEMO_MODE, mode);
        m.write32(MEMO_ARG, arg);
    }

    /// Port of `0x6A850(arg)` (MEASURED from the disassembly). A repeat call with the same `arg` and
    /// the same mode byte `[0x7DE455]` returns the cached pointer without touching anything. Otherwise
    /// both memo cells are stored, then mode byte 0xA (Story) uses `select_a` and any other mode
    /// `select`, and the result (possibly 0) is cached in `0x6F32F0`.
    pub fn resolve<M: GuestMemory>(&mut self, m: &mut M, arg: u32) -> u32 {
        let mode = m.read8(MODE_CELL) as u32;
        if m.read32(MEMO_ARG) == arg && m.read32(MEMO_MODE) == mode {
            return m.read32(CURRENT_CELL);
        }
        m.write32(MEMO_MODE, mode);
        m.write32(MEMO_ARG, arg);
        let p = if mode == STORY_MODE {
            ModeRows::select_a(m, arg)
        } else {
            ModeRows::select(m, arg, m.read32(LEVEL_CELL), m.read32(SELECT_FLAG))
        }
        .unwrap_or(0);
        self.set_current(m, p);
        p
    }

    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<&'static str> {
        if m.read32(CURRENT_CELL) != self.current {
            return Some("current");
        }
        match &self.record {
            Some(r) if r.diff(m).is_some() => Some("record"),
            _ => None,
        }
    }
}
