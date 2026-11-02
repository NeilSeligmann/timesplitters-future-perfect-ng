// portreg: mirrors=tsfp-engine-object/synth_leaves
//! T1908 opt-in memory services. Every original listed here is Tier 0.
//! Guest globals remain authoritative; errors preserve ordered partial mutations.
use crate::{
    GuestMemory, MemoryFault,
    w4m_arena::{Arena, ArenaHooks, globals},
};
use tsfp_types::GuestAddr;
type Result<T> = core::result::Result<T, MemoryFault>;

/// Narrow shared allocator boundary for every caller system. Implementations must
/// preserve allocation kinds and tagged LIFO release; these are not host mallocs.
pub trait GameAllocator {
    fn allocate(&mut self, kind: u32, size: u32) -> Result<u32>;
    fn allocate_aligned(&mut self, alignment: u32, size: u32, kind: u32) -> Result<u32>;
    fn release(&mut self, pointer: u32) -> Result<()>;
    fn scratch(&mut self, size: u32) -> Result<u32>;
}
impl<M: GuestMemory, H: ArenaHooks> GameAllocator for Arena<'_, M, H> {
    fn allocate(&mut self, kind: u32, size: u32) -> Result<u32> {
        // Kind numbering is the original's (T2048 fix): 3 returns 0, 4 is the unchecked cursor.
        self.alloc_round16(kind, size)
    }
    fn allocate_aligned(&mut self, alignment: u32, size: u32, kind: u32) -> Result<u32> {
        let mask = alignment.wrapping_sub(1);
        match kind {
            1 | 2 => {
                let (cursor, remaining) = if kind == 1 {
                    (globals::REGION_B_CURSOR, globals::MAIN_REMAINING)
                } else {
                    (globals::REGION_A_CURSOR, globals::REGION_A_REMAINING)
                };
                let old = self.memory.read_u32(GuestAddr(cursor))?;
                let pad = old.wrapping_add(mask) & !mask;
                let pad = pad.wrapping_sub(old);
                self.memory
                    .write_u32(GuestAddr(cursor), old.wrapping_add(pad))?;
                let left = self.memory.read_u32(GuestAddr(remaining))?;
                self.memory
                    .write_u32(GuestAddr(remaining), left.wrapping_sub(pad))?;
            }
            0 => {
                let old = self.memory.read_u32(GuestAddr(globals::TOP_CURSOR))?;
                let pad = (old.wrapping_add(mask) & !mask).wrapping_sub(old);
                let adjust = pad.wrapping_sub(alignment);
                self.memory
                    .write_u32(GuestAddr(globals::TOP_CURSOR), old.wrapping_add(adjust))?;
                let left = self.memory.read_u32(GuestAddr(globals::MAIN_REMAINING))?;
                self.memory.write_u32(
                    GuestAddr(globals::MAIN_REMAINING),
                    left.wrapping_add(adjust),
                )?;
                let allocated = self.alloc_round16(0, size.wrapping_add(mask) & !mask)?;
                let header = self.memory.read_u32(GuestAddr(globals::TOP_CURSOR))?;
                let length = self.memory.read_u32(GuestAddr(header.wrapping_add(8)))?;
                self.memory.write_u32(
                    GuestAddr(header.wrapping_add(8)),
                    length.wrapping_add(alignment.wrapping_sub(pad)),
                )?;
                return Ok(allocated);
            }
            4 => {
                let old = self.memory.read_u32(GuestAddr(globals::KIND4_CURSOR))?;
                self.memory.write_u32(
                    GuestAddr(globals::KIND4_CURSOR),
                    old.wrapping_add(mask) & !mask,
                )?;
            }
            _ => {}
        }
        self.allocate(kind, size)
    }
    fn release(&mut self, pointer: u32) -> Result<()> {
        self.release_tagged_block(pointer)
    }
    fn scratch(&mut self, size: u32) -> Result<u32> {
        self.bump_allocate(size)
    }
}

pub struct MemoryServices<'a, M: GuestMemory> {
    pub memory: &'a mut M,
}
impl<M: GuestMemory> MemoryServices<'_, M> {
    fn get(&mut self, a: u32) -> Result<u32> {
        self.memory.read_u32(GuestAddr(a))
    }
    fn put(&mut self, a: u32, v: u32) -> Result<()> {
        self.memory.write_u32(GuestAddr(a), v)
    }
    /// 0003DF00: initialize arena ranges, retaining wrapping guest arithmetic.
    pub fn initialize_arena(&mut self, base: u32, size: u32) -> Result<()> {
        let split = base.wrapping_add(0x9dc00);
        self.put(0x6b8374, split)?;
        self.put(0x6b8378, split.wrapping_sub(base))?;
        let ledger = self.get(0x4c81b4)?;
        self.put(0x7e72d4, base)?;
        self.put(0x6b8370, base)?;
        let end = base.wrapping_add(size);
        self.put(0x7e72d8, size)?;
        self.put(0x6b837c, split)?;
        self.put(0x6b8380, end)?;
        if ledger != 0 {
            self.put(0x6b836c, end.wrapping_sub(0x266000))?;
        }
        self.put(0x6b8384, end.wrapping_sub(split))?;
        self.put(0x6b8388, end)
    }
    /// 0003E560: append a secondary ledger entry, without inventing a capacity gate.
    pub fn append_secondary(&mut self, size: u32) -> Result<u32> {
        let offset = self.get(0x6b8368)?;
        let end = self.get(0x6b8380)?;
        let pointer = end.wrapping_add(offset);
        self.put(0x6b8368, offset.wrapping_add(size))?;
        let index = self.get(0x6b838c)?;
        self.put(0x6b7b18u32.wrapping_add(index.wrapping_mul(8)), pointer)?;
        self.put(0x6b7b1cu32.wrapping_add(index.wrapping_mul(8)), size)?;
        self.put(0x6b838c, index.wrapping_add(1))?;
        Ok(pointer)
    }
    /// 0003E930: signed high-water comparison, then exchange scratch buffers.
    // portreg: va=0x0003e930 tier=1
    pub fn swap_scratch(&mut self) -> Result<()> {
        let cursor = self.get(0x6b839c)?;
        let first = self.get(0x6b83a0)?;
        let high = self.get(0x6b83b0)?;
        let used = cursor.wrapping_sub(first);
        if (used as i32) > (high as i32) {
            self.put(0x6b83b0, used)?;
        }
        let second = self.get(0x6b83a4)?;
        self.put(0x6b83a4, first)?;
        self.put(0x6b83a0, second)?;
        self.put(0x6b839c, second)
    }
    /// 0003E9A0: zero pool storage and initialize the first free header.
    /// Requires the normal forward string-operation convention (DF=0).
    pub fn reset_pool(&mut self) -> Result<()> {
        let size = self.get(0x4c81b8)?;
        let base = self.get(0x6b83c8)?;
        for offset in (0..size / 4 * 4).step_by(4) {
            self.put(base.wrapping_add(offset), 0)?;
        }
        for offset in size / 4 * 4..size {
            self.memory
                .write(GuestAddr(base.wrapping_add(offset)), &[0])?;
        }
        let base = self.get(0x6b83c8)?;
        for (a, v) in [
            (0x6b83cc, base),
            (0x6b83d0, base),
            (0x6b83c0, size),
            (0x6b83c4, size),
            (0x6b83d4, base.wrapping_add(size)),
            (0x6b83bc, 0),
            (base, 0),
            (base.wrapping_add(4), size),
            (base.wrapping_add(8), 0),
            (base.wrapping_add(12), 0),
            (base.wrapping_add(16), 0),
        ] {
            self.put(a, v)?;
        }
        Ok(())
    }
    /// 0003EA20: clear base/cursor only when the external retention gate is zero.
    // portreg: va=0x0003ea20 tier=1
    pub fn clear_pool(&mut self) -> Result<()> {
        if self.get(0x6b7aa8)? == 0 {
            self.put(0x6b83c8, 0)?;
            self.put(0x6b83cc, 0)?;
        }
        Ok(())
    }
    /// 0003DF70: clear arena cursors/accounting only when teardown retention is disabled.
    // portreg: va=0x0003df70 tier=1
    // portreg: replaces=tsfp-lane-c2r/lib
    pub fn clear_arena_after_teardown(&mut self) -> Result<()> {
        if self.get(0x6b7aa8)? == 0 {
            for address in [0x6b837c, 0x6b8380, 0x6b8384, 0x6b8388] {
                self.put(address, 0)?;
            }
        }
        Ok(())
    }
    /// 0003EA40: release only kind-1 blocks; coalesce adjacent free blocks.
    pub fn free_pool(&mut self, pointer: u32) -> Result<()> {
        let mut block = pointer.wrapping_sub(32);
        if self.get(block)? != 1 {
            return Ok(());
        }
        let size = self.get(block.wrapping_add(4))?;
        self.put(block, 0)?;
        let free = self.get(0x6b83c0)?;
        self.put(0x6b83c0, free.wrapping_add(size))?;
        let size = self.get(block.wrapping_add(4))?;
        let end = self.get(0x6b83d4)?;
        let next = block.wrapping_add(size);
        if next < end && self.get(next)? == 0 {
            let following = self.get(next.wrapping_add(4))?;
            self.put(block.wrapping_add(4), following.wrapping_add(size))?;
            if self.get(0x6b83cc)? == next {
                self.put(0x6b83cc, block)?;
            }
        }
        let back = self.get(block.wrapping_add(8))?;
        let previous = block.wrapping_sub(back);
        if self.get(previous)? == 0 && previous != block {
            let size = self.get(block.wrapping_add(4))?;
            let before = self.get(previous.wrapping_add(4))?;
            self.put(previous.wrapping_add(4), before.wrapping_add(size))?;
            if self.get(0x6b83cc)? == block {
                self.put(0x6b83cc, previous)?;
            }
            block = previous;
        }
        let size = self.get(block.wrapping_add(4))?;
        let end = self.get(0x6b83d4)?;
        let next = block.wrapping_add(size);
        if next < end {
            self.put(next.wrapping_add(8), size)?;
        }
        Ok(())
    }
    /// 0003EAD0: null-tolerant pool release.
    pub fn free_pool_nullable(&mut self, pointer: u32) -> Result<()> {
        if pointer != 0 {
            self.free_pool(pointer)?;
        }
        Ok(())
    }
    /// 0003ECA0: allocate permanent kind-2 prefix; require room for a trailing header.
    pub fn allocate_pool_permanent(&mut self, size: u32) -> Result<u32> {
        let block = self.get(0x6b83cc)?;
        let old_size = self.get(block.wrapping_add(4))?;
        let rounded = size.wrapping_add(0x5f) & !0x3f;
        if rounded.wrapping_add(0x20) > old_size {
            return Ok(0);
        }
        self.put(block.wrapping_add(4), rounded)?;
        self.put(block, 2)?;
        for offset in [12, 16, 8] {
            self.put(block.wrapping_add(offset), 0)?;
        }
        let prefix = self.get(0x6b83d0)?;
        self.put(0x6b83d0, prefix.wrapping_add(rounded))?;
        let free = self.get(0x6b83c0)?;
        self.put(0x6b83c0, free.wrapping_sub(rounded))?;
        let remaining = self.get(0x6b83c4)?;
        let next = self.get(0x6b83cc)?.wrapping_add(rounded);
        self.put(0x6b83c4, remaining.wrapping_sub(rounded))?;
        self.put(0x6b83cc, next)?;
        for offset in [0, 8, 12, 16] {
            self.put(next.wrapping_add(offset), 0)?;
        }
        self.put(next.wrapping_add(4), old_size.wrapping_sub(rounded))?;
        Ok(block.wrapping_add(32))
    }
    /// 0012C800: fixed 17-slot service; signed comparison is intentional.
    pub fn allocate_slot(&mut self) -> Result<u32> {
        let count = self.get(0x7490a0)?;
        if (count as i32) >= 17 {
            return Ok(0);
        }
        self.put(0x7490a0, count.wrapping_add(1))?;
        Ok(0x745938u32.wrapping_add(count.wrapping_mul(0x144)))
    }
}
