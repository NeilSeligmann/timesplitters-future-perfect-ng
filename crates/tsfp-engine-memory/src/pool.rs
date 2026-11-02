// SPDX-License-Identifier: GPL-3.0-or-later
//! T2048 slice P: the reclaimable pool allocator (originals 0x3EA00, 0x3EAF0, 0x3EBD0, 0x3ED20).
//!
//! REPLACE system (rule 14, docs/t-memory-replace.md): a new implementation behind the documented
//! contract. The pool state stays in the guest globals 0x6B83BC..0x6B83D4 while the guest image
//! exists, so the lifted code and the ports address the same bytes. The already ported operations
//! (reset 0x3E9A0, free 0x3EA40, locked allocation 0x3ECA0) are reused from
//! [`crate::service_slice::MemoryServices`] and not duplicated.
//!
//! Evidence: Tier 1 differential against the lifted original in
//! `crates/tsfp-c-diff/tests/differential/w4.rs` (T2048 section). Original arithmetic is kept:
//! wrapping offsets, signed admission checks, unsigned remainder tests, no repair of bad states.
//! The ONE deliberate deviation is [`PoolError::Runaway`]: the original loops forever on some
//! malformed rings (T1372 "bounded nonreturning prefixes"), this reports instead of hanging.
use crate::{
    GuestMemory, MemoryFault,
    service_slice::{GameAllocator, MemoryServices},
    w4m_arena::{Arena, ArenaHooks},
};
use tsfp_types::GuestAddr;

/// Coverage inventory for the existing environment-selected C ABI, not a dispatch table.
pub const ROOTS: [u32; 1] = [0x0003ED20];

/// Pool globals (guest addresses).
pub mod globals {
    pub const KIND1_USED: u32 = 0x006B_83BC; // set to 1 by every kind-1 request, even when it fails
    pub const FREE_BYTES: u32 = 0x006B_83C0;
    pub const CAPACITY: u32 = 0x006B_83C4;
    pub const BASE: u32 = 0x006B_83C8;
    pub const CURSOR: u32 = 0x006B_83CC; // roving search cursor (a block header address)
    pub const FIRST: u32 = 0x006B_83D0; // first block
    pub const END: u32 = 0x006B_83D4;
    pub const SIZE_VALUE: u32 = 0x004C_81B8; // byte length of the pool buffer
}

/// Header size and rounding: payload = header + 0x20.
pub const HEADER: u32 = 0x20;
/// Upper bound on search steps before [`PoolError::Runaway`] (original: unbounded).
const FUEL: u32 = 1 << 22;

#[derive(Debug, PartialEq, Eq)]
pub enum PoolError {
    Memory(MemoryFault),
    /// The ring walk did not terminate. The original would hang (INFERRED reading of T1372).
    Runaway,
    /// A typed reclaim hook refused to invalidate its owner before block reuse.
    ReclaimHook,
}
impl From<MemoryFault> for PoolError {
    fn from(fault: MemoryFault) -> Self {
        PoolError::Memory(fault)
    }
}
type R<T> = Result<T, PoolError>;

/// Host side of the reclaim path. In the original the stored callback is an indirect guest call
/// `callback(argument)` (cdecl, one argument, result ignored) made while the pool is mid-search.
pub trait PoolHooks {
    fn reclaim(&mut self, callback: u32, argument: u32);

    /// Reclaim a specific block before the allocator changes its state or reuses its address.
    /// Legacy hooks keep the old callback behavior; native owner hooks can validate the payload
    /// and refuse reuse if owner invalidation cannot complete.
    fn reclaim_block(
        &mut self,
        _payload: u32,
        callback: u32,
        argument: u32,
    ) -> Result<(), PoolError> {
        self.reclaim(callback, argument);
        Ok(())
    }

    /// Reclaim with direct access to the guest image. Legacy hooks keep their callback adapter;
    /// native callbacks can update typed guest state before the allocator frees or reuses a block.
    fn reclaim_block_with_memory<M: GuestMemory>(
        &mut self,
        _memory: &mut M,
        payload: u32,
        callback: u32,
        argument: u32,
    ) -> Result<(), PoolError> {
        self.reclaim_block(payload, callback, argument)
    }
}

pub struct Pool<'a, M: GuestMemory, H: PoolHooks + ArenaHooks> {
    pub memory: &'a mut M,
    pub hooks: &'a mut H,
}

impl<M: GuestMemory, H: PoolHooks + ArenaHooks> Pool<'_, M, H> {
    fn get(&mut self, address: u32) -> R<u32> {
        Ok(self.memory.read_u32(GuestAddr(address))?)
    }
    fn put(&mut self, address: u32, value: u32) -> R<()> {
        Ok(self.memory.write_u32(GuestAddr(address), value)?)
    }
    fn services(&mut self) -> MemoryServices<'_, M> {
        MemoryServices {
            memory: &mut *self.memory,
        }
    }

    /// 0x3EA00: allocate the pool buffer from the arena (alignment 64, kind 1, length
    /// `[0x4C81B8]`), publish it at 0x6B83C8 and reset the pool (0x3E9A0).
    pub fn allocate_and_reset(&mut self) -> R<()> {
        let size = self.get(globals::SIZE_VALUE)?;
        let base = {
            let mut arena = Arena {
                memory: &mut *self.memory,
                hooks: &mut *self.hooks,
            };
            arena.allocate_aligned(64, size, 1)?
        };
        self.put(globals::BASE, base)?;
        Ok(self.services().reset_pool()?)
    }

    /// 0x3EAF0: advance the cursor to a free block of at least `needed` bytes, reclaiming
    /// (callback or owner clear, then free) occupied blocks when a bounded scan finds none.
    /// Returns the block header the cursor ends on.
    pub fn find_or_reclaim(&mut self, needed: u32) -> R<u32> {
        let mut fuel = FUEL;
        let mut tick = || {
            fuel = fuel.checked_sub(1).ok_or(PoolError::Runaway)?;
            Ok::<(), PoolError>(())
        };
        let mut end = self.get(globals::END)?;
        let mut first = self.get(globals::FIRST)?;
        let start = self.get(globals::CURSOR)?;
        let mut block = start;
        let mut count: u32 = 0;
        let mut saved: u32 = 0; // the original keeps the scan count in a stack slot across the free call
        loop {
            // 0x3EB14 .. 0x3EB52: bounded scan for a free block that fits
            loop {
                tick()?;
                let size = self.get(block.wrapping_add(4))?;
                block = block.wrapping_add(size);
                self.put(globals::CURSOR, block)?;
                if block >= end {
                    block = first;
                    self.put(globals::CURSOR, block)?;
                }
                if self.get(block)? == 0 && block != start {
                    if (needed as i32) <= self.get(block.wrapping_add(4))? as i32 {
                        return Ok(block);
                    }
                    count = count.wrapping_add(1);
                    continue;
                }
                count = count.wrapping_add(1);
                saved = count;
                if (count as i32) >= 0x14 || (self.get(globals::FREE_BYTES)? as i32) < needed as i32
                {
                    break;
                }
                if block == start {
                    break;
                }
            }
            // 0x3EB52: reclaim the first occupied block after the original cursor
            block = start;
            self.put(globals::CURSOR, block)?;
            if self.get(block)? == 0 {
                loop {
                    tick()?;
                    let size = self.get(block.wrapping_add(4))?;
                    block = block.wrapping_add(size);
                    self.put(globals::CURSOR, block)?;
                    if block >= end {
                        block = first;
                        self.put(globals::CURSOR, block)?;
                    }
                    if self.get(block)? != 0 {
                        break;
                    }
                }
            }
            let payload = block.wrapping_add(HEADER);
            if payload != 0 && self.get(payload.wrapping_sub(32))? != 0 {
                let callback = self.get(payload.wrapping_sub(16))?;
                let argument = self.get(payload.wrapping_sub(20))?;
                if callback != 0 {
                    self.hooks.reclaim_block_with_memory(
                        self.memory,
                        payload,
                        callback,
                        argument,
                    )?;
                } else if argument != 0 {
                    self.put(argument, 0)?;
                }
                self.services().free_pool(payload)?;
                block = self.get(globals::CURSOR)?;
                end = self.get(globals::END)?;
                first = self.get(globals::FIRST)?;
            }
            if (needed as i32) > self.get(block.wrapping_add(4))? as i32 {
                count = saved;
                continue;
            }
            return Ok(block);
        }
    }

    /// 0x3EBD0: allocate a reclaimable (state 1) block. Returns the payload (`header + 0x20`) or 0.
    pub fn allocate_reclaimable(&mut self, size: u32, argument: u32, callback: u32) -> R<u32> {
        let mut block = self.get(globals::CURSOR)?;
        let current = self.get(block.wrapping_add(4))?;
        let mut rounded = size.wrapping_add(0x5F) & !0x3F;
        if (rounded as i32) > current as i32 {
            if (rounded as i32) > self.get(globals::CAPACITY)? as i32 {
                return Ok(0);
            }
            self.find_or_reclaim(rounded)?;
            block = self.get(globals::CURSOR)?;
        }
        let block_size = self.get(block.wrapping_add(4))?;
        let remainder = block_size.wrapping_sub(rounded);
        let small = remainder < 0x20;
        self.put(block.wrapping_add(0xC), argument)?;
        self.put(block, 1)?;
        self.put(block.wrapping_add(0x10), callback)?;
        if small {
            rounded = block_size;
        }
        self.put(block.wrapping_add(4), rounded)?;
        let end = self.get(globals::END)?;
        let free = self.get(globals::FREE_BYTES)?;
        self.put(globals::FREE_BYTES, free.wrapping_sub(rounded))?;
        let next = self.get(globals::CURSOR)?.wrapping_add(rounded);
        self.put(globals::CURSOR, next)?;
        let payload = block.wrapping_add(HEADER);
        if next >= end {
            let first = self.get(globals::FIRST)?;
            self.put(globals::CURSOR, first)?;
            if self.get(first)? != 0 {
                self.find_or_reclaim(0)?;
            }
            return Ok(payload);
        }
        if rounded == block_size {
            self.find_or_reclaim(0)?;
            return Ok(payload);
        }
        self.put(next, 0)?;
        self.put(next.wrapping_add(4), remainder)?;
        self.put(next.wrapping_add(8), rounded)?;
        self.put(next.wrapping_add(0xC), 0)?;
        let after = block_size.wrapping_add(block);
        if after < self.get(globals::END)? {
            self.put(after.wrapping_add(8), remainder)?;
        }
        Ok(payload)
    }

    /// 0x3ED20: kind 1 reclaimable (then flag 0x6B83BC = 1, even when the allocation failed),
    /// kind 2 locked (0x3ECA0, optional arguments ignored), any other kind returns 0 untouched.
    pub fn allocate_by_kind(
        &mut self,
        size: u32,
        kind: u32,
        argument: u32,
        callback: u32,
    ) -> R<u32> {
        match kind {
            1 => {
                let payload = self.allocate_reclaimable(size, argument, callback)?;
                self.put(globals::KIND1_USED, 1)?;
                Ok(payload)
            }
            2 => Ok(self.services().allocate_pool_permanent(size)?),
            _ => Ok(0),
        }
    }

    /// 0x3EA40 / 0x3EAD0: free (null tolerant). Reused port, see `service_slice`.
    pub fn free(&mut self, pointer: u32) -> R<()> {
        Ok(self.services().free_pool_nullable(pointer)?)
    }
}
