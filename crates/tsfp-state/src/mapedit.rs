// SPDX-License-Identifier: GPL-3.0-or-later
//! T2076 (S3-13 of T2051): Rust-owned Map Maker editor state object. Design and honest limits:
//! docs/t-s3-mapmaker.md. The global pointer cell `0x7844A8` points at one 0x11950-byte arena
//! object allocated by `0x2FE360` (`0x3DFA0(0x11950, 1)`) and cleared there for 0x11948 bytes. The
//! offset map is docs/t1712-editor-state.md (static, from instruction widths); every field name is
//! INFERRED, unnamed bytes stay raw. `MapEdit` owns a copy of the object with write-through setters.
use super::GuestMemory;

/// Guest cell holding the pointer to the editor object (0 until `0x2FE360` ran).
pub const POINTER_CELL: u32 = 0x0078_44a8;
/// Allocated size (`0x3DFA0(0x11950, 1)`), cleared size is `CLEARED_SIZE`.
pub const OBJECT_SIZE: u32 = 0x1_1950;
pub const OBJECT_WORDS: usize = (OBJECT_SIZE / 4) as usize;
/// `0x2FE37A..0x2FE37F` clears 0x4652 dwords; the last eight allocated bytes are not covered.
pub const CLEARED_SIZE: u32 = 0x1_1948;

pub const SELECTED_RECORD: u32 = 0x204;
pub const MODE_COPY: u32 = 0x208;
pub const FLAGS: u32 = 0x238;
pub const TOOL_SELECTOR: u32 = 0x254;
pub const PLACE_FLAGS: u32 = 0x25c;
pub const LAYER: u32 = 0x264;
pub const E4_COUNT: u32 = 0x280;
pub const E4_BASE: u32 = 0x288;
pub const E4_STRIDE: u32 = 0xe4;
pub const E4_CLEAR: u32 = 0x2c88;
pub const CHANGE_COUNT: u32 = 0x306e;
pub const CHANGE_BASE: u32 = 0x3070;
pub const CHANGE_STRIDE: u32 = 12;
pub const R220_COUNT: u32 = 0x41e0;
pub const R220_BASE: u32 = 0x41e4;
pub const R220_STRIDE: u32 = 0x220;
pub const R220_CLEAR: u32 = 0x3fc0;
pub const B4_COUNT: u32 = 0x81a4;
pub const B4_BASE: u32 = 0x81a8;
pub const B4_STRIDE: u32 = 0xb4;
pub const B4_CLEAR: u32 = 0x5b68;
pub const R8C_COUNT: u32 = 0xdd10;
pub const R8C_BASE: u32 = 0xdd14;
pub const R8C_STRIDE: u32 = 0x8c;
pub const R8C_CLEAR: u32 = 0x36b0;
pub const ACTIVE_TARGET: u32 = 0x1_1858;
pub const WEAPON_SET: u32 = 0x1_1904;
pub const WEAPON_COUNT: u32 = 0x1_1908;
pub const WEAPON_SLOTS: u32 = 0x1_190c;
pub const WEAPON_SLOT_COUNT: u32 = 6;

/// Reset values stored by the constructor `0x2FE2B8..0x2FE40C` (offset, value), the entries
/// docs/t1712 documents with a reset value. Other fields start at the cleared 0.
pub const RESET_VALUES: [(u32, u32); 8] = [
    (0x210, 0xffff_ffff),
    (0x214, 1),
    (0x218, 1),
    (0x220, 0xffff_ffff),
    (0x23c, 1),
    (0x24c, 0xffff_ffff),
    (0x1185c, 0xffff_ffff),
    (0x21c, 0),
];

/// One table of the object: count offset, first-element offset, stride, element capacity cleared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Table {
    pub count: u32,
    pub base: u32,
    pub stride: u32,
    pub clear_bytes: u32,
}

pub const TABLE_E4: Table = Table {
    count: E4_COUNT,
    base: E4_BASE,
    stride: E4_STRIDE,
    clear_bytes: E4_CLEAR,
};
pub const TABLE_220: Table = Table {
    count: R220_COUNT,
    base: R220_BASE,
    stride: R220_STRIDE,
    clear_bytes: R220_CLEAR,
};
pub const TABLE_B4: Table = Table {
    count: B4_COUNT,
    base: B4_BASE,
    stride: B4_STRIDE,
    clear_bytes: B4_CLEAR,
};
pub const TABLE_8C: Table = Table {
    count: R8C_COUNT,
    base: R8C_BASE,
    stride: R8C_STRIDE,
    clear_bytes: R8C_CLEAR,
};

impl Table {
    /// Element capacity implied by the clear extent (clear coverage, not a proven mutation limit).
    pub const fn capacity(&self) -> u32 {
        self.clear_bytes / self.stride
    }
}

/// The owned editor object. `base` is the guest address read from `POINTER_CELL`.
#[derive(Clone, PartialEq, Eq)]
pub struct MapEdit {
    pub base: u32,
    pub words: [u32; OBJECT_WORDS],
}

impl MapEdit {
    /// Value of the guest pointer cell; `None` while the editor object does not exist.
    pub fn pointer<M: GuestMemory>(m: &M) -> Option<u32> {
        let p = m.read32(POINTER_CELL);
        (p != 0).then_some(p)
    }

    pub fn load<M: GuestMemory>(m: &M, base: u32) -> Self {
        let mut words = [0u32; OBJECT_WORDS];
        for (i, v) in words.iter_mut().enumerate() {
            *v = m.read32(base + 4 * i as u32);
        }
        Self { base, words }
    }

    /// Load through the pointer cell.
    pub fn load_current<M: GuestMemory>(m: &M) -> Option<Self> {
        Self::pointer(m).map(|p| Self::load(m, p))
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, v) in self.words.iter().enumerate() {
            m.write32(self.base + 4 * i as u32, *v);
        }
    }

    /// First drifted byte offset, `None` when the guest object equals the owned copy.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<u32> {
        (0..OBJECT_WORDS)
            .find(|&i| m.read32(self.base + 4 * i as u32) != self.words[i])
            .map(|i| 4 * i as u32)
    }

    /// Mirrors the constructor `0x2FE360`: new zeroed object (0x11948 bytes cleared, the trailing eight
    /// bytes keep `tail`), pointer cell stored, reset values applied. Write-through.
    pub fn construct<M: GuestMemory>(m: &mut M, base: u32, tail: [u32; 2]) -> Self {
        let mut o = Self {
            base,
            words: [0; OBJECT_WORDS],
        };
        o.words[OBJECT_WORDS - 2] = tail[0];
        o.words[OBJECT_WORDS - 1] = tail[1];
        m.write32(POINTER_CELL, base);
        for (off, v) in RESET_VALUES {
            o.words[(off / 4) as usize] = v;
        }
        o.store(m);
        o
    }

    pub fn get32(&self, offset: u32) -> u32 {
        self.words[(offset / 4) as usize]
    }
    pub fn get16(&self, offset: u32) -> u16 {
        (self.get32(offset & !3) >> (8 * (offset & 2))) as u16
    }
    pub fn get8(&self, offset: u32) -> u8 {
        (self.get32(offset & !3) >> (8 * (offset & 3))) as u8
    }

    /// Write-through dword store.
    pub fn set32<M: GuestMemory>(&mut self, m: &mut M, offset: u32, v: u32) {
        self.words[(offset / 4) as usize] = v;
        m.write32(self.base + offset, v);
    }
    /// Write-through word store (little endian inside the owning dword).
    pub fn set16<M: GuestMemory>(&mut self, m: &mut M, offset: u32, v: u16) {
        let s = 8 * (offset & 2);
        let w = &mut self.words[(offset / 4) as usize];
        *w = (*w & !(0xffff << s)) | ((v as u32) << s);
        m.write8(self.base + offset, v as u8);
        m.write8(self.base + offset + 1, (v >> 8) as u8);
    }
    pub fn set8<M: GuestMemory>(&mut self, m: &mut M, offset: u32, v: u8) {
        let s = 8 * (offset & 3);
        let w = &mut self.words[(offset / 4) as usize];
        *w = (*w & !(0xff << s)) | ((v as u32) << s);
        m.write8(self.base + offset, v);
    }

    pub fn flags(&self) -> u32 {
        self.get32(FLAGS)
    }
    pub fn set_flags<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.set32(m, FLAGS, v);
    }
    /// `+0x238` bit 0x20 requests a snapshot (read by `0x337760`, INFERRED name).
    pub fn snapshot_requested(&self) -> bool {
        self.flags() & 0x20 != 0
    }
    pub fn tool_selector(&self) -> u32 {
        self.get32(TOOL_SELECTOR)
    }
    pub fn layer(&self) -> u32 {
        self.get32(LAYER)
    }

    /// Signed count of a table (the count words are tested signed).
    pub fn count(&self, t: Table) -> i32 {
        self.get32(t.count) as i32
    }
    pub fn set_count<M: GuestMemory>(&mut self, m: &mut M, t: Table, n: i32) {
        self.set32(m, t.count, n as u32);
    }
    /// Offset of element `i` of table `t` inside the object.
    pub fn element_offset(t: Table, i: u32) -> u32 {
        t.base + i * t.stride
    }
    /// Element `i` as dwords, `None` beyond the cleared extent.
    pub fn element(&self, t: Table, i: u32) -> Option<&[u32]> {
        (i < t.capacity()).then(|| {
            let o = (Self::element_offset(t, i) / 4) as usize;
            &self.words[o..o + (t.stride / 4) as usize]
        })
    }
    /// Write-through store of one element dword.
    pub fn set_element_word<M: GuestMemory>(
        &mut self,
        m: &mut M,
        t: Table,
        i: u32,
        word: u32,
        v: u32,
    ) {
        debug_assert!(i < t.capacity() && word < t.stride / 4);
        self.set32(m, Self::element_offset(t, i) + 4 * word, v);
    }
    /// Zero a table body (the clears at `0x321C4F..0x321C95`), count left to the caller.
    pub fn clear_table<M: GuestMemory>(&mut self, m: &mut M, t: Table) {
        for off in (0..t.clear_bytes).step_by(4) {
            self.set32(m, t.base + off, 0);
        }
    }

    /// Change records (stride 12): word triple position, type byte +7, attribute +9, key +10.
    pub fn change_count(&self) -> u16 {
        self.get16(CHANGE_COUNT)
    }
    pub fn set_change_count<M: GuestMemory>(&mut self, m: &mut M, n: u16) {
        self.set16(m, CHANGE_COUNT, n);
    }
    pub fn change_key(&self, i: u32) -> i16 {
        self.get16(CHANGE_BASE + 0xa + CHANGE_STRIDE * i) as i16
    }
    pub fn set_change_key<M: GuestMemory>(&mut self, m: &mut M, i: u32, key: i16) {
        self.set16(m, CHANGE_BASE + 0xa + CHANGE_STRIDE * i, key as u16);
    }

    /// Weapon-set slots (six entries, -1 empty).
    pub fn weapon_slot(&self, i: u32) -> i32 {
        self.get32(WEAPON_SLOTS + 4 * i) as i32
    }
    pub fn set_weapon_slot<M: GuestMemory>(&mut self, m: &mut M, i: u32, v: i32) {
        debug_assert!(i < WEAPON_SLOT_COUNT);
        self.set32(m, WEAPON_SLOTS + 4 * i, v as u32);
    }
}
