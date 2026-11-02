// SPDX-License-Identifier: GPL-3.0-or-later
//! T2081 (S3-6 of T2051): Rust-owned pad words and the 0x6B98xx audio ramp family. Design and
//! honest limits: docs/t-s3-pad-audio.md. The owners are plain word arrays over fixed guest
//! extents, load/store/diff like the other S3 slices, with write-through setters.
//!
//! * `PadState`: pad table `0x6B93C0..0x6B94F8` (78 words: the 4-word pad-to-entry offset map at
//!   `0x6B93C0`, mapped entries from `0x6B93D0` at stride 0x3C, direct entry from `0x6B94C8`; read
//!   by `0x42B60`, `0x42970`, `0x42CA0`, `0x438B0`, `0x42760`) plus the input scalars
//!   `0x6B9888..0x6B98B8` (12 words: gate `0x6B9888`, ramp state `0x6B98A4`, countdown `0x6B98B0`).
//! * `AudioRamp`: `0x6B98B8..0x6B9934` (31 words: four key-table pointer cells with counts, the
//!   `0x6B98C8` scratch buffer and the volume scale `0x6B9930`).
//!
//! Names are INFERRED from the translations in crates/tsfp-ports unless a comment says MEASURED.
use super::GuestMemory;

pub const PAD_BASE: u32 = 0x006b_93c0;
pub const PAD_WORDS: usize = 78;
/// Pad entries: mapped slots start here at stride `ENTRY_STRIDE`, the map words give byte offsets.
pub const ENTRY_BASE: u32 = 0x006b_93d0;
pub const ENTRY_STRIDE: u32 = 0x3c;
pub const MAP_WORDS: usize = 4;
pub const GATE: u32 = 0x006b_9888;
pub const INPUT_BASE: u32 = 0x006b_9888;
pub const INPUT_WORDS: usize = 12;
/// Word indices inside the input block (INFERRED names).
pub const IN_GATE: usize = 0;
/// 0x6B9898, cleared by the poll 0x438B0 (MEASURED store of 0).
pub const IN_POLL_FLAG: usize = 4;
pub const IN_RAMP_A: usize = 5; // 0x6B989C
pub const IN_RAMP_B: usize = 6; // 0x6B98A0
/// 0x6B98A4, values 1..5 (MEASURED stores of 2, 4 and others in 0x18FF70).
pub const IN_STATE: usize = 7;
pub const IN_SEL_A: usize = 8; // 0x6B98A8
pub const IN_SEL_B: usize = 9; // 0x6B98AC
/// 0x6B98B0, f32 bits.
pub const IN_COUNTDOWN: usize = 10;
/// 0x6B98B4, stored 1 by 0x42640.
pub const IN_DONE: usize = 11;

pub const AUDIO_BASE: u32 = 0x006b_98b8;
pub const AUDIO_WORDS: usize = 31;
/// Key-table groups: (pointer cell, count, second count, element stride or 0 if unknown).
/// Strides 0x64 and 0x2C are MEASURED in 0x43DA0; the pointer order is by cell.
pub const GROUPS: [(u32, u32, u32, u32); 4] = [
    (0x006b_9908, 0x006b_990c, 0x006b_9910, 0x64),
    (0x006b_98b8, 0x006b_9914, 0x006b_9918, 0x2c),
    (0x006b_98bc, 0x006b_991c, 0x006b_9920, 0),
    (0x006b_98c0, 0x006b_9924, 0x006b_9928, 0),
];
/// 0x6B98C8..0x6B9908: 16-word buffer whose address is pushed by the sound lane (MEASURED push).
pub const SCRATCH: u32 = 0x006b_98c8;
pub const SCRATCH_WORDS: usize = 16;
/// Volume scale, stepped by 0x45790 and reset to the constant at `0x475C78` by `0x45970`.
pub const SCALE: u32 = 0x006b_9930;

fn idx(base: u32, addr: u32) -> usize {
    ((addr - base) / 4) as usize
}

/// First difference between an owner and a guest image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordDiff {
    pub addr: u32,
    pub owned: u32,
    pub guest: u32,
}

fn diff_words<M: GuestMemory>(m: &M, base: u32, words: &[u32]) -> Option<WordDiff> {
    words.iter().enumerate().find_map(|(i, w)| {
        let addr = base + 4 * i as u32;
        let guest = m.read32(addr);
        (guest != *w).then_some(WordDiff {
            addr,
            owned: *w,
            guest,
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadState {
    pub table: [u32; PAD_WORDS],
    pub input: [u32; INPUT_WORDS],
}

impl Default for PadState {
    fn default() -> Self {
        Self {
            table: [0; PAD_WORDS],
            input: [0; INPUT_WORDS],
        }
    }
}

impl PadState {
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        let mut s = Self::default();
        for (i, w) in s.table.iter_mut().enumerate() {
            *w = m.read32(PAD_BASE + 4 * i as u32);
        }
        for (i, w) in s.input.iter_mut().enumerate() {
            *w = m.read32(INPUT_BASE + 4 * i as u32);
        }
        s
    }

    /// Writes only the owned extents.
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, w) in self.table.iter().enumerate() {
            m.write32(PAD_BASE + 4 * i as u32, *w);
        }
        for (i, w) in self.input.iter().enumerate() {
            m.write32(INPUT_BASE + 4 * i as u32, *w);
        }
    }

    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<WordDiff> {
        diff_words(m, PAD_BASE, &self.table).or_else(|| diff_words(m, INPUT_BASE, &self.input))
    }

    /// Write-through setter for a table word by guest address (false if outside the table).
    pub fn set_table<M: GuestMemory>(&mut self, m: &mut M, addr: u32, v: u32) -> bool {
        if addr < PAD_BASE || addr >= PAD_BASE + 4 * PAD_WORDS as u32 || addr % 4 != 0 {
            return false;
        }
        self.table[idx(PAD_BASE, addr)] = v;
        m.write32(addr, v);
        true
    }

    pub fn set_input<M: GuestMemory>(&mut self, m: &mut M, i: usize, v: u32) {
        self.input[i] = v;
        m.write32(INPUT_BASE + 4 * i as u32, v);
    }

    pub fn gate(&self) -> u32 {
        self.input[IN_GATE]
    }

    /// Port of `0x426A0` (MEASURED from the translation): store `v`, return the old gate.
    pub fn exchange_gate<M: GuestMemory>(&mut self, m: &mut M, v: u32) -> u32 {
        let old = self.input[IN_GATE];
        self.set_input(m, IN_GATE, v);
        old
    }

    /// Port of `0x42690` (`game_global_6b98b4_get`).
    pub fn done_flag(&self) -> u32 {
        self.input[IN_DONE]
    }

    pub fn state(&self) -> u32 {
        self.input[IN_STATE]
    }

    pub fn countdown(&self) -> f32 {
        f32::from_bits(self.input[IN_COUNTDOWN])
    }

    pub fn set_countdown<M: GuestMemory>(&mut self, m: &mut M, v: f32) {
        self.set_input(m, IN_COUNTDOWN, v.to_bits());
    }

    /// Stores of `0x42640` (MEASURED `[0x6B98A4] = 4`, `[0x6B98B4] = 1`; the call into `0x69EA0` and
    /// the argument store through `[0x6F1E10]` stay in the lifted code).
    pub fn enter_state_4<M: GuestMemory>(&mut self, m: &mut M) {
        self.set_input(m, IN_STATE, 4);
        self.set_input(m, IN_DONE, 1);
    }

    /// Mapped entry address for pad `slot` (INFERRED: map word is the byte offset from `ENTRY_BASE`).
    pub fn entry_addr(&self, slot: usize) -> u32 {
        ENTRY_BASE.wrapping_add(self.table[slot % MAP_WORDS])
    }

    /// A field of the mapped entry, if the word lies inside the owned table.
    pub fn entry_word(&self, slot: usize, field: u32) -> Option<u32> {
        let a = self.entry_addr(slot).wrapping_add(field);
        (a >= PAD_BASE && a < PAD_BASE + 4 * PAD_WORDS as u32 && a % 4 == 0)
            .then(|| self.table[idx(PAD_BASE, a)])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioRamp {
    pub words: [u32; AUDIO_WORDS],
}

impl Default for AudioRamp {
    fn default() -> Self {
        Self {
            words: [0; AUDIO_WORDS],
        }
    }
}

impl AudioRamp {
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        let mut s = Self::default();
        for (i, w) in s.words.iter_mut().enumerate() {
            *w = m.read32(AUDIO_BASE + 4 * i as u32);
        }
        s
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, w) in self.words.iter().enumerate() {
            m.write32(AUDIO_BASE + 4 * i as u32, *w);
        }
    }

    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<WordDiff> {
        diff_words(m, AUDIO_BASE, &self.words)
    }

    pub fn word(&self, addr: u32) -> u32 {
        self.words[idx(AUDIO_BASE, addr)]
    }

    /// Write-through setter by guest address; false if outside `0x6B98B8..0x6B9934`.
    pub fn set<M: GuestMemory>(&mut self, m: &mut M, addr: u32, v: u32) -> bool {
        if addr < AUDIO_BASE || addr >= AUDIO_BASE + 4 * AUDIO_WORDS as u32 || addr % 4 != 0 {
            return false;
        }
        self.words[idx(AUDIO_BASE, addr)] = v;
        m.write32(addr, v);
        true
    }

    pub fn scale(&self) -> f32 {
        f32::from_bits(self.word(SCALE))
    }

    pub fn set_scale<M: GuestMemory>(&mut self, m: &mut M, v: f32) {
        self.set(m, SCALE, v.to_bits());
    }

    /// Key table group `g` (0..4): (pointer, count, second count).
    pub fn group(&self, g: usize) -> (u32, u32, u32) {
        let (p, c, c2, _) = GROUPS[g];
        (self.word(p), self.word(c), self.word(c2))
    }

    /// Prefix of `0x450E0` (MEASURED): clears the eight count words `0x6B990C..0x6B9928`.
    pub fn clear_counts<M: GuestMemory>(&mut self, m: &mut M) {
        for a in (0x006b_990c..=0x006b_9928).step_by(4) {
            self.set(m, a, 0);
        }
    }

    /// Tail of `0x45160` (MEASURED): the four pointer cells and the first count are zeroed.
    pub fn release_tables<M: GuestMemory>(&mut self, m: &mut M) {
        for a in [
            0x006b_9908,
            0x006b_98b8,
            0x006b_98bc,
            0x006b_98c0,
            0x006b_990c,
        ] {
            self.set(m, a, 0);
        }
    }

    /// Pointer store of `0x43DA0` for group `g`: the fresh table address.
    pub fn set_table_pointer<M: GuestMemory>(&mut self, m: &mut M, g: usize, ptr: u32) {
        self.set(m, GROUPS[g].0, ptr);
    }
}
