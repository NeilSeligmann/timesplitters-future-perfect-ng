// SPDX-License-Identifier: GPL-3.0-or-later
//! T2041 data/missions attachment boundary: typed read access to the `0x715F2C` point table family and the
//! `0x55F974` vertex write pointer. Names INFERRED (layout MEASURED in docs/t-s3-missions.md). Guest memory
//! owns the state today. `PointTables` is consumed by the default-off `missions_reader`
//! selector for readable and translated point-table readers (the `0x9CC40` family);
//! `GuestTables` backs it with guest memory; `missions_wire::OwnedPoints` uses the persistent typed owner, and a `tsfp-state` `Missions` owner can implement it later.
use crate::memory::GuestMemory;

pub const MAIN_ROWS: u32 = 0x715f2c;
pub const LINKS: u32 = 0x715f30;
pub const SECOND_ROWS: u32 = 0x715f34;
pub const THIRD_ROWS: u32 = 0x715f38;
pub const MAIN_COUNT: u32 = 0x7b6664;
pub const LINK_COUNT: u32 = 0x7b6660;
pub const SECOND_COUNT: u32 = 0x7b65c0;
pub const DRAW_CURSOR: u32 = 0x55f974;
pub const ROW_SIZE: u32 = 0x20;
pub const LINK_SIZE: u32 = 0x14;

/// One 0x20-byte point row (key, flags, group, tag, position bits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub key: u32,
    pub flags: u32,
    pub group: i16,
    pub tag: i16,
    pub position_bits: [u32; 3],
}

/// Read-only table access shared by missions readers; the backing store is an implementation detail.
pub trait PointTables {
    fn main_rows(&self) -> u32;
    fn links(&self) -> u32;
    fn second_rows(&self) -> u32;
    fn main_count(&self) -> u32;
    fn link_count(&self) -> u32;
    fn second_count(&self) -> u32;
    fn third_rows(&self) -> u32;
    fn draw_cursor(&self) -> u32;
    fn main_row(&self, index: u32) -> Row;
    fn second_row(&self, index: u32) -> Row;
    fn third_row(&self, index: u32) -> Row;
    fn second_key(&self, index: u32) -> u32 {
        self.second_row(index).key
    }
    fn link_word(&self, index: u32, word: u32) -> u32;
    fn main_key(&self, index: u32) -> u32 {
        self.main_row(index).key
    }
    fn link(&self, index: u32) -> Link {
        Link {
            flags: self.link_word(index, 0),
            endpoints: [self.link_word(index, 1), self.link_word(index, 2)],
            unknown: [self.link_word(index, 3), self.link_word(index, 4)],
        }
    }
    /// Main-table branch of `0x9CC40`, with signed keys and original midpoint selection.
    /// The caller routes signed keys >= 0x2329 to the second-table search (`0x9A3C0`).
    /// Counts must be nonnegative and rows sorted by signed key. Like the original,
    /// count zero still probes row zero; the backing allocation must remain readable.
    fn find_key(&self, key: u32) -> Option<u32> {
        let (mut lo, mut hi) = (0u32, self.main_count());
        let mut mid = (hi as i32 >> 1) as u32;
        if mid == u32::MAX {
            return None;
        }
        loop {
            let row_key = self.main_key(mid) as i32;
            if key as i32 == row_key {
                return Some(mid);
            }
            if (key as i32) < row_key {
                hi = mid;
            } else {
                lo = mid;
            }
            let next = ((hi.wrapping_sub(lo) as i32 >> 1) as u32).wrapping_add(lo);
            if next == mid {
                return None;
            }
            mid = next;
        }
    }
}

/// Guest-memory backing; borrows the memory, owns nothing.
pub struct GuestTables<'a, M: GuestMemory>(pub &'a M);

impl<M: GuestMemory> PointTables for GuestTables<'_, M> {
    fn main_rows(&self) -> u32 {
        self.0.read32(MAIN_ROWS)
    }
    fn links(&self) -> u32 {
        self.0.read32(LINKS)
    }
    fn second_rows(&self) -> u32 {
        self.0.read32(SECOND_ROWS)
    }
    fn main_count(&self) -> u32 {
        self.0.read32(MAIN_COUNT)
    }
    fn link_count(&self) -> u32 {
        self.0.read32(LINK_COUNT)
    }
    fn second_count(&self) -> u32 {
        self.0.read32(SECOND_COUNT)
    }
    fn third_rows(&self) -> u32 {
        self.0.read32(THIRD_ROWS)
    }
    fn main_key(&self, index: u32) -> u32 {
        self.0
            .read32(self.main_rows().wrapping_add(index.wrapping_mul(ROW_SIZE)))
    }
    fn draw_cursor(&self) -> u32 {
        self.0.read32(DRAW_CURSOR)
    }
    fn main_row(&self, index: u32) -> Row {
        self.row_at(self.main_rows(), index)
    }
    fn second_key(&self, index: u32) -> u32 {
        self.0.read32(
            self.second_rows()
                .wrapping_add(index.wrapping_mul(ROW_SIZE)),
        )
    }
    fn second_row(&self, index: u32) -> Row {
        self.row_at(self.second_rows(), index)
    }
    fn third_row(&self, index: u32) -> Row {
        self.row_at(self.third_rows(), index)
    }
    fn link_word(&self, index: u32, word: u32) -> u32 {
        let b = self.links().wrapping_add(index.wrapping_mul(LINK_SIZE));
        self.0.read32(b.wrapping_add(4 * word))
    }
}
impl<M: GuestMemory> GuestTables<'_, M> {
    fn row_at(&self, base: u32, index: u32) -> Row {
        let b = base.wrapping_add(index.wrapping_mul(ROW_SIZE));
        Row {
            key: self.0.read32(b),
            flags: self.0.read32(b.wrapping_add(4)),
            group: self.0.read16(b.wrapping_add(8)) as i16,
            tag: self.0.read16(b.wrapping_add(10)) as i16,
            position_bits: core::array::from_fn(|i| {
                self.0.read32(b.wrapping_add(0xc + 4 * i as u32))
            }),
        }
    }
}

/// Typed link row. Unknown words remain uninterpreted, rather than guessed state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub flags: u32,
    pub endpoints: [u32; 2],
    pub unknown: [u32; 2],
}

/// Renderer-owned stream attachment, independent of the point-table owner.
/// Backing implementations must make cursor updates immediately visible. Reloads
/// preserve `0xF9080` behavior even if a stream write aliases the cursor cell.
pub trait DrawStream {
    fn cursor(&self) -> u32;
    fn set_cursor(&mut self, address: u32);
    fn write_word(&mut self, address: u32, bits: u32);
    fn push_position_bits(&mut self, bits: [u32; 3]) {
        self.write_word(self.cursor(), bits[0]);
        for word in &bits[1..] {
            let next = self.cursor().wrapping_add(4);
            self.set_cursor(next);
            self.write_word(next, *word);
        }
        self.set_cursor(self.cursor().wrapping_add(4));
    }
}

/// Borrowed draw stream; it does not own renderer buffers or allocate guest memory.
pub struct GuestDrawStream<'a, M: GuestMemory>(pub &'a mut M);
impl<M: GuestMemory> DrawStream for GuestDrawStream<'_, M> {
    fn cursor(&self) -> u32 {
        self.0.read32(DRAW_CURSOR)
    }
    fn set_cursor(&mut self, address: u32) {
        self.0.write32(DRAW_CURSOR, address);
    }
    fn write_word(&mut self, address: u32, bits: u32) {
        self.0.write32(address, bits);
    }
}
