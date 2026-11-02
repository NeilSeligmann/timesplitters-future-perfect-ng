// SPDX-License-Identifier: GPL-3.0-or-later
//! T1914: readable Tier 0 attachment slice; see docs/t-system-physics.md.
//! Guest state is borrowed, with wrapping addresses and original load/store order.
use crate::memory::GuestMemory;

pub const SHAPES: u32 = 0x74c458;
pub const SLOTS: u32 = 0x74c464;
pub const USED_COUNT: u32 = 0x74c470;
pub const SLOT_LIMIT: u32 = 0x5d;

/// Borrowed address of a 0x200-byte shape. High handle bits are generation metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape(pub u32);
impl Shape {
    pub fn resolve<M: GuestMemory>(m: &M, handle: u32) -> Self {
        Self(m.read32(SHAPES).wrapping_add((handle & 0xffff) << 9))
    }
    /// 0x167ca0, Tier 0: sequential reads matter when source overlaps destination.
    pub fn set_position<M: GuestMemory>(self, m: &mut M, source: u32) {
        copy_words(m, source, self.0.wrapping_add(0x140), 3, false);
    }
}

/// Borrowed 0x80-byte collision constraint slot; indices are deliberately unchecked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(pub u32);
impl Slot {
    pub fn resolve<M: GuestMemory>(m: &M, index: u32) -> Self {
        Self(m.read32(SLOTS).wrapping_add(index.wrapping_shl(7)))
    }
    /// 0x16b850, Tier 0.
    pub fn set_key<M: GuestMemory>(self, m: &mut M, key: u32) {
        m.write32(self.0.wrapping_add(0x78), key);
    }
    /// 0x16b800, Tier 0: preserve raw float bits, including NaN payloads.
    pub fn set_limits<M: GuestMemory>(self, m: &mut M, values: [u32; 2]) {
        m.write32(self.0.wrapping_add(0x68), values[0]);
        m.write32(self.0.wrapping_add(0x6c), values[1]);
    }
    /// 0x16b870, Tier 0.
    pub fn set_coefficients<M: GuestMemory>(self, m: &mut M, values: [u32; 2]) {
        m.write32(self.0.wrapping_add(0x40), values[0]);
        m.write32(self.0.wrapping_add(0x44), values[1]);
    }
    /// 0x16b8a0, Tier 0.
    pub fn set_scalar<M: GuestMemory>(self, m: &mut M, bits: u32) {
        m.write32(self.0.wrapping_add(0x64), bits);
    }
    /// 0x16b550, Tier 0: all four source words are captured before any write.
    pub fn set_contact<M: GuestMemory>(self, m: &mut M, source: u32) {
        let words: [u32; 4] = core::array::from_fn(|i| m.read32(source.wrapping_add(i as u32 * 4)));
        for (i, word) in words.into_iter().enumerate() {
            m.write32(self.0.wrapping_add(0x48 + i as u32 * 4), word);
        }
    }
    /// 0x16b590, Tier 0: sequential vector copy into an unchecked sub-entry.
    pub fn set_vector<M: GuestMemory>(self, m: &mut M, part: u32, source: u32) {
        copy_words(
            m,
            source,
            self.0.wrapping_add(part.wrapping_shl(4)).wrapping_add(0x20),
            3,
            false,
        );
    }
    /// 0x16b900, Tier 0: each nullable output is handled independently, in order.
    pub fn read_handles<M: GuestMemory>(self, m: &mut M, first: u32, second: u32) {
        if first != 0 {
            let word = m.read32(self.0.wrapping_add(0x70));
            m.write32(first, word);
        }
        if second != 0 {
            let word = m.read32(self.0.wrapping_add(0x74));
            m.write32(second, word);
        }
    }
}

/// 0x16b130, Tier 0: first free slot, count wraps at 16 bits; other fields survive.
pub fn allocate<M: GuestMemory>(m: &mut M) -> i32 {
    let base = m.read32(SLOTS);
    for index in 0..SLOT_LIMIT {
        let slot = base.wrapping_add(index << 7);
        if m.read32(slot.wrapping_add(0x58)) == 0 {
            let count = m.read16(USED_COUNT).wrapping_add(1);
            write16(m, USED_COUNT, count);
            m.write32(slot.wrapping_add(0x58), 1);
            m.write32(slot.wrapping_add(0x78), 0);
            return index as i32;
        }
    }
    -1
}

/// 0x16b8c0, Tier 0: count is only a zero gate, never the scan length.
pub fn find_key<M: GuestMemory>(m: &M, key: u32) -> i32 {
    if m.read16(USED_COUNT) == 0 {
        return -1;
    }
    let base = m.read32(SLOTS);
    for index in 0..SLOT_LIMIT {
        let slot = base.wrapping_add(index << 7);
        if m.read32(slot.wrapping_add(0x58)) != 0 && m.read32(slot.wrapping_add(0x78)) == key {
            return index as i32;
        }
    }
    -1
}

/// 0x166b60, Tier 0: zero mask or zero FULL handle returns 1, otherwise raw masked byte.
/// The low 16 handle bits select a shape, while high bits do not change its address.
///
/// ```
/// use tsfp_engine_physics::{memory::GuestMemory, tables::{shape_mask, SHAPES}};
/// struct Memory;
/// impl GuestMemory for Memory {
///     fn read8(&self, address: u32) -> u8 {
///         match address {
///             a if a == SHAPES => 0x00,
///             a if a == SHAPES + 1 => 0x10,
///             0x11c9 => 0xa5,
///             _ => 0,
///         }
///     }
///     fn write8(&mut self, _: u32, _: u8) {}
/// }
/// let memory = Memory;
/// assert_eq!(shape_mask(&memory, 0xf0, 0x1234_0000), 0xa0);
/// assert_eq!(shape_mask(&memory, 0xf0, 0), 1);
/// ```
pub fn shape_mask<M: GuestMemory>(m: &M, mask: u32, handle: u32) -> u32 {
    if mask == 0 || handle == 0 {
        return 1;
    }
    m.read8(Shape::resolve(m, handle).0.wrapping_add(0x1c9)) as u32 & mask
}

/// Direction-flag semantics are sequential dword copies, including overlap.
fn copy_words<M: GuestMemory>(
    m: &mut M,
    source: u32,
    destination: u32,
    count: u32,
    backwards: bool,
) {
    for i in 0..count {
        let delta = if backwards {
            0u32.wrapping_sub(i * 4)
        } else {
            i * 4
        };
        let word = m.read32(source.wrapping_add(delta));
        m.write32(destination.wrapping_add(delta), word);
    }
}
fn write16<M: GuestMemory>(m: &mut M, address: u32, word: u16) {
    m.write8(address, word as u8);
    m.write8(address.wrapping_add(1), (word >> 8) as u8);
}
/// 0x91390, Tier 0.
pub fn reset_contact<M: GuestMemory>(m: &mut M, backwards: bool) {
    copy_words(m, 0x70aa58, 0x70aaac, 8, backwards);
}
/// 0x913b0, Tier 0.
pub fn import_contact<M: GuestMemory>(m: &mut M, source: u32, backwards: bool) {
    copy_words(m, source, 0x70aaac, 8, backwards);
}
/// 0x913d0, Tier 0.
pub fn export_contact<M: GuestMemory>(m: &mut M, destination: u32, backwards: bool) {
    copy_words(m, 0x70aaac, destination, 8, backwards);
}

/// Per-function hybrid switch for typed arguments. None means retain the original.
/// Models behavior only: register residue and original stack writes are outside this boundary.
pub fn run<M: GuestMemory>(
    m: &mut M,
    va: u32,
    enabled: bool,
    args: [u32; 3],
    backwards: bool,
) -> Option<u32> {
    if !enabled {
        return None;
    }
    match va {
        0x167ca0 => Shape::resolve(m, args[0]).set_position(m, args[1]),
        0x16b850 => Slot::resolve(m, args[0]).set_key(m, args[1]),
        0x16b800 => Slot::resolve(m, args[0]).set_limits(m, [args[1], args[2]]),
        0x16b870 => Slot::resolve(m, args[0]).set_coefficients(m, [args[1], args[2]]),
        0x16b8a0 => Slot::resolve(m, args[0]).set_scalar(m, args[1]),
        0x16b550 => Slot::resolve(m, args[0]).set_contact(m, args[1]),
        0x16b590 => Slot::resolve(m, args[0]).set_vector(m, args[1], args[2]),
        0x16b900 => Slot::resolve(m, args[0]).read_handles(m, args[1], args[2]),
        0x16b130 => return Some(allocate(m) as u32),
        0x16b8c0 => return Some(find_key(m, args[0]) as u32),
        0x166b60 => return Some(shape_mask(m, args[0], args[1])),
        0x91390 => reset_contact(m, backwards),
        0x913b0 => import_contact(m, args[0], backwards),
        0x913d0 => export_contact(m, args[0], backwards),
        _ => return None,
    }
    Some(0) // typed void completion; not a claim about original EAX
}
