// SPDX-License-Identifier: GPL-3.0-or-later
//! T2079 (S3-11 of T2051): Rust-owned missions state. Design, evidence and honest limits:
//! docs/t-s3-missions.md. Parts:
//!
//! * `PickupCells`: the pointer and count cells of the id/point tables (`0x715F2C` main rows,
//!   `0x715F30` links, `0x715F34` second table, `0x715F38` third table, `0x715F3C`), their counts
//!   and the scalar cells that `0x9A140`, `0x9A20C` and `0x9CE00` reset. Owned with write-through.
//! * `PickupRow` / `LinkRow`: 0x20-byte and 0x14-byte heap rows, loaded on demand (the arrays live in
//!   the guest heap, so only the row handle is owned) with write-through word setters.
//! * `Visited`: the 0x20-byte visited table at `0x7B67A0`.
//! * `SetupB4` / `RuleRecord`: the script setup entry (0xB4) and rule (0x220) records reached through
//!   the mode record lists (`ModeRecord::setups_b4`, `rules`). Layout from docs/t-mission-script-engine.md.
//!
//! Names are INFERRED unless the doc says MEASURED; unnamed words stay raw. The mode record itself is
//! `modes::ModeRecord` (T2075) and is not duplicated here.
use super::GuestMemory;

pub const MAIN_PTR: u32 = 0x0071_5f2c;
pub const LINK_PTR: u32 = 0x0071_5f30;
pub const T34_PTR: u32 = 0x0071_5f34;
pub const T38_PTR: u32 = 0x0071_5f38;
/// Allocated by `0x9A140` (0x19C80 bytes) only when the mode byte is not Story.
pub const EXTRA_PTR: u32 = 0x0071_5f3c;
pub const MAIN_COUNT: u32 = 0x007b_6664;
pub const LINK_COUNT: u32 = 0x007b_6660;
pub const T34_COUNT: u32 = 0x007b_65c0;
/// Scalar cells zeroed by `0x9CE00` / `0x9A20C` (roles unresolved; `0x7B666C` is the group id base
/// added to the row group word, `0x9AE0E`). Order is the order of this array.
pub const AUX_CELLS: [u32; 8] = [
    0x007b_665c,
    0x007b_6668,
    0x007b_666c,
    0x007b_6670,
    0x007b_6674,
    0x007b_65bc,
    0x007b_6778,
    0x007b_6780,
];
pub const PTR_CELLS: [u32; 5] = [MAIN_PTR, LINK_PTR, T34_PTR, T38_PTR, EXTRA_PTR];
/// Capacities from the `0x9A140` allocations (MEASURED code): main 0x12C00 / 0x20, links
/// 0x157C0 / 0x14, second table 0xA000 / 0x20 (level 0x64) else 0x2BC0 / 0x20, third 0x280 / 0x20.
pub const MAIN_CAPACITY: u32 = 0x960;
pub const LINK_CAPACITY: u32 = 0x1130;
pub const T34_CAPACITY_LEVEL100: u32 = 0x500;
pub const T34_CAPACITY: u32 = 0x15e;
pub const T38_CAPACITY: u32 = 0x14;
pub const ROW_SIZE: u32 = 0x20;
pub const LINK_SIZE: u32 = 0x14;
/// Visited table: `0x9ADA2..0x9ADC5` clears 8 dwords (MEASURED code); `0x9A630` marks bytes indexed by
/// row id, so ids beyond the cleared 0x20 would index past the table (unresolved).
pub const VISITED: u32 = 0x007b_67a0;
pub const VISITED_SIZE: usize = 0x20;
pub const LEVEL_CELL: u32 = super::game_state::LEVEL;
/// Guard cell of `0x9A260`.
pub const CLEAR_GUARD: u32 = 0x006b_7aa8;

/// Pointer and count cells (owned, write-through).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickupCells {
    pub ptrs: [u32; 5],
    pub main_count: u32,
    pub link_count: u32,
    pub t34_count: u32,
    pub aux: [u32; 8],
}

impl PickupCells {
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self {
            ptrs: PTR_CELLS.map(|a| m.read32(a)),
            main_count: m.read32(MAIN_COUNT),
            link_count: m.read32(LINK_COUNT),
            t34_count: m.read32(T34_COUNT),
            aux: AUX_CELLS.map(|a| m.read32(a)),
        }
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (a, v) in PTR_CELLS.iter().zip(self.ptrs) {
            m.write32(*a, v);
        }
        m.write32(MAIN_COUNT, self.main_count);
        m.write32(LINK_COUNT, self.link_count);
        m.write32(T34_COUNT, self.t34_count);
        for (a, v) in AUX_CELLS.iter().zip(self.aux) {
            m.write32(*a, v);
        }
    }

    /// First drifted cell name, `None` when the guest cells equal the owned values.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<&'static str> {
        const NAMES: [&str; 5] = ["main_ptr", "link_ptr", "t34_ptr", "t38_ptr", "extra_ptr"];
        const AUX_NAMES: [&str; 8] = [
            "aux_665c", "aux_6668", "aux_666c", "aux_6670", "aux_6674", "aux_65bc", "aux_6778",
            "aux_6780",
        ];
        for i in 0..5 {
            if m.read32(PTR_CELLS[i]) != self.ptrs[i] {
                return Some(NAMES[i]);
            }
        }
        if m.read32(MAIN_COUNT) != self.main_count {
            return Some("main_count");
        }
        if m.read32(LINK_COUNT) != self.link_count {
            return Some("link_count");
        }
        if m.read32(T34_COUNT) != self.t34_count {
            return Some("t34_count");
        }
        (0..8)
            .find(|&i| m.read32(AUX_CELLS[i]) != self.aux[i])
            .map(|i| AUX_NAMES[i])
    }

    pub fn set_ptr<M: GuestMemory>(&mut self, m: &mut M, index: usize, v: u32) {
        self.ptrs[index] = v;
        m.write32(PTR_CELLS[index], v);
    }
    pub fn set_main_count<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.main_count = v;
        m.write32(MAIN_COUNT, v);
    }
    pub fn set_link_count<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.link_count = v;
        m.write32(LINK_COUNT, v);
    }
    pub fn set_t34_count<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.t34_count = v;
        m.write32(T34_COUNT, v);
    }
    pub fn set_aux<M: GuestMemory>(&mut self, m: &mut M, index: usize, v: u32) {
        self.aux[index] = v;
        m.write32(AUX_CELLS[index], v);
    }

    /// Mirrors the zero block of `0x9CE00`: main and link counts and the aux scalars except the group
    /// base `0x7B666C` (not the t34 count, not the pointers).
    pub fn reset_9ce00<M: GuestMemory>(&mut self, m: &mut M) {
        self.set_main_count(m, 0);
        self.set_link_count(m, 0);
        for i in [4, 6, 5, 7, 0, 3, 1] {
            // 0x7B6674, 0x7B6778, 0x7B65BC, 0x7B6780, 0x7B665C, 0x7B6670, 0x7B6668
            self.set_aux(m, i, 0);
        }
    }

    /// Mirrors the level 0x64 zero block of `0x9A20C` (MEASURED code): the three counts and the aux
    /// cells `0x7B6674, 0x7B6778, 0x7B65BC, 0x7B6780, 0x7B666C`. Returns false when the level cell
    /// is not 0x64 (the original then takes the other branch, not modelled here).
    pub fn reset_9a20c<M: GuestMemory>(&mut self, m: &mut M) -> bool {
        if m.read32(LEVEL_CELL) != 0x64 {
            return false;
        }
        self.set_main_count(m, 0);
        self.set_link_count(m, 0);
        self.set_t34_count(m, 0);
        for i in [4, 6, 5, 7, 2] {
            self.set_aux(m, i, 0);
        }
        true
    }

    /// Port of `0x9A260`: zero the four table pointers unless the guard cell is nonzero.
    pub fn clear_pointers_9a260<M: GuestMemory>(&mut self, m: &mut M) {
        if m.read32(CLEAR_GUARD) != 0 {
            return;
        }
        for i in 0..4 {
            self.set_ptr(m, i, 0);
        }
    }

    /// Guest address of main row `index` (no bounds check against the count, like the lifted code).
    pub fn main_row_address(&self, index: u32) -> u32 {
        self.ptrs[0].wrapping_add(index.wrapping_mul(ROW_SIZE))
    }
    pub fn link_address(&self, index: u32) -> u32 {
        self.ptrs[1].wrapping_add(index.wrapping_mul(LINK_SIZE))
    }
    pub fn t34_row_address(&self, index: u32) -> u32 {
        self.ptrs[2].wrapping_add(index.wrapping_mul(ROW_SIZE))
    }
}

/// One 0x20-byte point/id row (main, second and third tables).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickupRow {
    pub base: u32,
    pub words: [u32; 8],
}

impl PickupRow {
    pub fn load<M: GuestMemory>(m: &M, base: u32) -> Self {
        Self {
            base,
            words: core::array::from_fn(|i| m.read32(base.wrapping_add(4 * i as u32))),
        }
    }
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, v) in self.words.iter().enumerate() {
            m.write32(self.base.wrapping_add(4 * i as u32), *v);
        }
    }
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<usize> {
        (0..8).find(|&i| m.read32(self.base.wrapping_add(4 * i as u32)) != self.words[i])
    }
    pub fn set_word<M: GuestMemory>(&mut self, m: &mut M, word: usize, v: u32) {
        self.words[word] = v;
        m.write32(self.base.wrapping_add(4 * word as u32), v);
    }
    /// Sort key (`0x9CC40` binary search compares it).
    pub fn key(&self) -> u32 {
        self.words[0]
    }
    pub fn flags(&self) -> u32 {
        self.words[1]
    }
    /// Bit 0: usable (MEASURED, docs/t-story-event-group-members-t1846.md).
    pub fn usable(&self) -> bool {
        self.words[1] & 1 != 0
    }
    /// Bit 1: active (INFERRED from `0x9BCF0`).
    pub fn active(&self) -> bool {
        self.words[1] & 2 != 0
    }
    /// Signed word `+8`: group id, or id of the real row for aliases (flags `& 0x400800`).
    pub fn group(&self) -> i16 {
        self.words[2] as u16 as i16
    }
    /// Signed word `+0xA`: tag compared with an id by `0x9C0E0`.
    pub fn tag(&self) -> i16 {
        (self.words[2] >> 16) as u16 as i16
    }
    /// Position floats `+0xC..+0x14` as raw bits.
    pub fn position_bits(&self) -> [u32; 3] {
        [self.words[3], self.words[4], self.words[5]]
    }
    pub fn set_flags<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.set_word(m, 1, v);
    }
    pub fn set_position<M: GuestMemory>(&mut self, m: &mut M, bits: [u32; 3]) {
        for (i, b) in bits.into_iter().enumerate() {
            self.set_word(m, 3 + i, b);
        }
    }
}

/// One 0x14-byte link row (`[0x715F30]`): flags, endpoint a, endpoint b, two unknown words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LinkRow {
    pub base: u32,
    pub words: [u32; 5],
}

impl LinkRow {
    pub fn load<M: GuestMemory>(m: &M, base: u32) -> Self {
        Self {
            base,
            words: core::array::from_fn(|i| m.read32(base.wrapping_add(4 * i as u32))),
        }
    }
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, v) in self.words.iter().enumerate() {
            m.write32(self.base.wrapping_add(4 * i as u32), *v);
        }
    }
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<usize> {
        (0..5).find(|&i| m.read32(self.base.wrapping_add(4 * i as u32)) != self.words[i])
    }
    pub fn set_word<M: GuestMemory>(&mut self, m: &mut M, word: usize, v: u32) {
        self.words[word] = v;
        m.write32(self.base.wrapping_add(4 * word as u32), v);
    }
    pub fn flags(&self) -> u32 {
        self.words[0]
    }
    pub fn a(&self) -> u32 {
        self.words[1]
    }
    pub fn b(&self) -> u32 {
        self.words[2]
    }
    /// Neighbour of `row` across this link as `0x9D830` reads it (docs/t-story-event-group-paths-t1846.md):
    /// `a == row` gives b, else `b == row && !(flags & 2)` gives a.
    pub fn other_end(&self, row: u32) -> Option<u32> {
        if self.words[1] == row {
            Some(self.words[2])
        } else if self.words[2] == row && self.words[0] & 2 == 0 {
            Some(self.words[1])
        } else {
            None
        }
    }
}

/// The visited table (0x20 bytes at `0x7B67A0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visited {
    pub bytes: [u8; VISITED_SIZE],
}

impl Default for Visited {
    fn default() -> Self {
        Self {
            bytes: [0; VISITED_SIZE],
        }
    }
}

impl Visited {
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self {
            bytes: core::array::from_fn(|i| m.read8(VISITED + i as u32)),
        }
    }
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, b) in self.bytes.iter().enumerate() {
            m.write8(VISITED + i as u32, *b);
        }
    }
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<usize> {
        (0..VISITED_SIZE).find(|&i| m.read8(VISITED + i as u32) != self.bytes[i])
    }
    pub fn is_visited(&self, id: usize) -> bool {
        self.bytes.get(id).is_some_and(|b| *b != 0)
    }
    /// Write-through mark; ids past the cleared extent are refused (the original would write past
    /// it, see the limits in docs/t-s3-missions.md). Returns whether the mark was applied.
    pub fn mark<M: GuestMemory>(&mut self, m: &mut M, id: usize, v: u8) -> bool {
        if id >= VISITED_SIZE {
            return false;
        }
        self.bytes[id] = v;
        m.write8(VISITED + id as u32, v);
        true
    }
    /// Mirrors the 8-dword clear at `0x9ADA2`.
    pub fn clear<M: GuestMemory>(&mut self, m: &mut M) {
        for id in 0..VISITED_SIZE {
            self.mark(m, id, 0);
        }
    }
}

pub const SETUP_B4_SIZE: u32 = 0xb4;
pub const SETUP_8C_SIZE: u32 = 0x8c;
pub const RULE_SIZE: u32 = 0x220;

/// Script setup entry of 0xB4 bytes (condition or action), 45 dwords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetupB4 {
    pub base: u32,
    pub words: [u32; 45],
}

impl SetupB4 {
    pub fn address(list_base: u32, index: u32) -> u32 {
        list_base.wrapping_add(index.wrapping_mul(SETUP_B4_SIZE))
    }
    pub fn load<M: GuestMemory>(m: &M, base: u32) -> Self {
        Self {
            base,
            words: core::array::from_fn(|i| m.read32(base.wrapping_add(4 * i as u32))),
        }
    }
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, v) in self.words.iter().enumerate() {
            m.write32(self.base.wrapping_add(4 * i as u32), *v);
        }
    }
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<usize> {
        (0..45).find(|&i| m.read32(self.base.wrapping_add(4 * i as u32)) != self.words[i])
    }
    pub fn set_word<M: GuestMemory>(&mut self, m: &mut M, word: usize, v: u32) {
        self.words[word] = v;
        m.write32(self.base.wrapping_add(4 * word as u32), v);
    }
    /// 1 condition, 2 action.
    pub fn kind(&self) -> u32 {
        self.words[0]
    }
    pub fn key(&self) -> u32 {
        self.words[1]
    }
    pub fn subtype(&self) -> u32 {
        self.words[2]
    }
    /// Signed word `+0xC`: setup/object index.
    pub fn index(&self) -> i16 {
        self.words[3] as u16 as i16
    }
    /// `+0xA4`: rule index (rule address is `rules + 0x220 * index`).
    pub fn rule_index(&self) -> u32 {
        self.words[0xa4 / 4]
    }
    pub fn ident(&self) -> u32 {
        self.words[0xac / 4]
    }
    /// Byte `+0xB0`: flags (mask 0x3C considered by common init).
    pub fn flags(&self) -> u8 {
        self.words[0xb0 / 4] as u8
    }
}

/// Script rule of 0x220 bytes, 136 dwords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleRecord {
    pub base: u32,
    pub words: [u32; 136],
}

impl RuleRecord {
    pub fn address(list_base: u32, index: u32) -> u32 {
        list_base.wrapping_add(index.wrapping_mul(RULE_SIZE))
    }
    pub fn load<M: GuestMemory>(m: &M, base: u32) -> Self {
        Self {
            base,
            words: core::array::from_fn(|i| m.read32(base.wrapping_add(4 * i as u32))),
        }
    }
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, v) in self.words.iter().enumerate() {
            m.write32(self.base.wrapping_add(4 * i as u32), *v);
        }
    }
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<usize> {
        (0..136).find(|&i| m.read32(self.base.wrapping_add(4 * i as u32)) != self.words[i])
    }
    pub fn set_word<M: GuestMemory>(&mut self, m: &mut M, word: usize, v: u32) {
        self.words[word] = v;
        m.write32(self.base.wrapping_add(4 * word as u32), v);
    }
    /// Byte `+4`: the direct action runner requires 1.
    pub fn direct_flag(&self) -> u8 {
        (self.words[1] & 0xff) as u8
    }
    pub fn threshold(&self) -> u32 {
        self.words[2]
    }
    /// `(count, list base)` of condition B4 indices (`+0x50`, `+0x54`).
    pub fn conditions(&self) -> (u32, u32) {
        (self.words[0x50 / 4], self.words[0x54 / 4])
    }
    /// `(count, list base)` of action B4 indices (`+0x88`, `+0x8C`).
    pub fn actions(&self) -> (u32, u32) {
        (self.words[0x88 / 4], self.words[0x8c / 4])
    }
    /// Per-condition mode byte `+0x7C+i`.
    pub fn condition_mode(&self, i: usize) -> u8 {
        (self.words[(0x7c + i) / 4] >> (8 * ((0x7c + i) % 4))) as u8
    }
    /// Nonzero selects the event-rule path, zero group construction (`+0x21C`).
    pub fn event_rule(&self) -> bool {
        self.words[0x21c / 4] != 0
    }
}

/// Draw batch cells `0x55F96C..0x55F97C` (T2041 folded in): `0x55F974` is the vertex cursor that
/// 1249 missions-side references and the render system share (docs/t-system-map.md, 8348 refs in
/// total). The cell dies with the renderer (T2024); it is owned here only so the missions boundary
/// has a typed, write-through handle. Roles of the other three words are unresolved (INFERRED
/// index batch cells from docs/t-render-slice.md).
pub const DRAW_CELLS: u32 = 0x0055_f96c;
pub const DRAW_CURSOR_WORD: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawCursor {
    pub words: [u32; 4],
}

impl DrawCursor {
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self {
            words: core::array::from_fn(|i| m.read32(DRAW_CELLS + 4 * i as u32)),
        }
    }
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, v) in self.words.iter().enumerate() {
            m.write32(DRAW_CELLS + 4 * i as u32, *v);
        }
    }
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<usize> {
        (0..4).find(|&i| m.read32(DRAW_CELLS + 4 * i as u32) != self.words[i])
    }
    pub fn set_word<M: GuestMemory>(&mut self, m: &mut M, word: usize, v: u32) {
        self.words[word] = v;
        m.write32(DRAW_CELLS + 4 * word as u32, v);
    }
    /// Vertex write pointer `0x55F974`.
    pub fn cursor(&self) -> u32 {
        self.words[DRAW_CURSOR_WORD]
    }
    pub fn set_cursor<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.set_word(m, DRAW_CURSOR_WORD, v);
    }
    /// Port of what `game_float_stream_55f974_push_3` does to the cursor (docs/t-c2rust.md, `0xF9080`):
    /// advance by `n` dwords. Returns the address the pushed words go to.
    pub fn advance<M: GuestMemory>(&mut self, m: &mut M, dwords: u32) -> u32 {
        let at = self.cursor();
        self.set_cursor(m, at.wrapping_add(4 * dwords));
        at
    }
}
