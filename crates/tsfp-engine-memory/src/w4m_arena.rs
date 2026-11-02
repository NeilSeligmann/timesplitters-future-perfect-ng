// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 engine/memory, batch 1 (Tier 0: translated, compiles, smoke tested).
//!
//! The engine's memory arena keeps its state in guest globals 0x6B8370..0x6B83B8. This module
//! reads and writes those globals through [`GuestMemory`], so the guest image stays the single
//! source of truth. Functions carry their original VA. Out-of-memory goes through
//! [`ArenaHooks::out_of_memory`] (original 0x389B0, called with -1 and then 1).
use crate::{GuestMemory, MemoryFault};
use tsfp_types::GuestAddr;

/// Guest entry routed through the game bump allocator C ABI when the Rust option is enabled.
pub const ROOTS: [u32; 4] = [0x0003E970, 0x0003E1E0, 0x0003E240, 0x0003DFA0];

/// Arena bookkeeping globals (guest addresses).
pub mod globals {
    pub const REGION_A_CURSOR: u32 = 0x006B_8370; // kind 2 cursor
    pub const REGION_B_CURSOR: u32 = 0x006B_837C; // kind 1 cursor
    pub const REGION_A_REMAINING: u32 = 0x006B_8378;
    pub const BOTTOM_LIMIT: u32 = 0x006B_8380;
    pub const MAIN_REMAINING: u32 = 0x006B_8384;
    pub const TOP_CURSOR: u32 = 0x006B_8388; // kind 0 and 3 allocate downwards from here
    pub const KIND4_CURSOR: u32 = 0x006B_8390; // kind 4 cursor (kind 3 is the zero-return arm)
    pub const BUMP_CURSOR: u32 = 0x006B_839C;
    pub const BUMP_TOTAL: u32 = 0x006B_83B8;
    pub const LEDGER_FLAG: u32 = 0x004C_81B4;
}

const TAG_LOCA: u32 = 0x4143_4F4C;
const TAG_LMEM: u32 = 0x4D45_4D4C;
const HEADER: u32 = 0x10;

/// Services the arena calls out to.
pub trait ArenaHooks {
    /// Original 0x389B0(argument): reported when an allocation does not fit (-1 then 1) or a
    /// block header is corrupt (1).
    fn out_of_memory(&mut self, argument: u32);

    /// Native bridges may override this to invoke the guest reporter against the same image.
    /// Existing hooks preserve their behavior through this compatibility default.
    fn out_of_memory_with_memory<M: GuestMemory>(&mut self, _memory: &mut M, argument: u32) {
        self.out_of_memory(argument);
    }
}

/// Register results left by the original `0x3E1E0` tagged-block release entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TaggedReleaseRegisters {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
}

pub struct Arena<'a, M: GuestMemory, H: ArenaHooks> {
    pub memory: &'a mut M,
    pub hooks: &'a mut H,
}

type Fault<T> = Result<T, MemoryFault>;

impl<M: GuestMemory, H: ArenaHooks> Arena<'_, M, H> {
    fn get(&mut self, address: u32) -> Fault<u32> {
        self.memory.read_u32(GuestAddr(address))
    }
    fn put(&mut self, address: u32, value: u32) -> Fault<()> {
        self.memory.write_u32(GuestAddr(address), value)
    }

    /// 0003E970 `game_bump_allocate(size)`: rounds the size up to 16, advances the bump cursor
    /// and the running total, returns the old cursor.
    // portreg: va=0x0003e970 tier=1
    pub fn bump_allocate(&mut self, size: u32) -> Fault<u32> {
        let rounded = size.wrapping_add(0xF) & !0xF;
        let cursor = self.get(globals::BUMP_CURSOR)?;
        self.put(globals::BUMP_CURSOR, cursor.wrapping_add(rounded))?;
        let total = self.get(globals::BUMP_TOTAL)?;
        self.put(globals::BUMP_TOTAL, total.wrapping_add(rounded))?;
        Ok(cursor)
    }

    fn report_overflow(&mut self) {
        self.hooks.out_of_memory_with_memory(self.memory, u32::MAX);
        self.hooks.out_of_memory_with_memory(self.memory, 1);
    }

    /// 0003DFA0 `game_arena_alloc_round16_by_kind`. The ORIGINAL stack order is (size, kind); this
    /// port takes (kind, size), callers adapt (found by the T1920 differential): kind 0 builds a tagged block
    /// (header `LOCA`, `LMEM`, size, self pointer) downwards from the top cursor; kinds 1, 2 and
    /// 4 advance their own cursor (1 checks the main remaining space, 4 keeps no remaining
    /// count). Returns 0 for kind 3 and any kind above 4. T2048 correction: the T1895 port used 3
    /// for the unchecked cursor, which the T1920 differential missed because its seeded jump table
    /// had the kind 3 and kind 4 arms swapped (the real table is in docs/t1364-arena-allocator-naming.md).
    // portreg: va=0x0003dfa0 tier=1
    pub fn alloc_round16(&mut self, kind: u32, size: u32) -> Fault<u32> {
        let rounded = size.wrapping_add(0xF) & !0xF;
        match kind {
            0 => {
                if (rounded.wrapping_add(HEADER) as i32) > self.get(globals::MAIN_REMAINING)? as i32
                {
                    self.report_overflow();
                }
                let step = 0xFFFF_FFF0u32.wrapping_sub(rounded);
                let top = self.get(globals::TOP_CURSOR)?.wrapping_add(step);
                self.put(globals::TOP_CURSOR, top)?;
                self.put(top, TAG_LOCA)?;
                self.put(top.wrapping_add(4), TAG_LMEM)?;
                self.put(top.wrapping_add(8), rounded)?;
                self.put(top.wrapping_add(12), top)?;
                let remaining = self.get(globals::MAIN_REMAINING)?;
                self.put(globals::MAIN_REMAINING, remaining.wrapping_add(step))?;
                Ok(top.wrapping_add(HEADER))
            }
            1 => self.advance_cursor(globals::REGION_B_CURSOR, globals::MAIN_REMAINING, rounded),
            2 => self.advance_cursor(
                globals::REGION_A_CURSOR,
                globals::REGION_A_REMAINING,
                rounded,
            ),
            4 => {
                let cursor = self.get(globals::KIND4_CURSOR)?;
                self.put(globals::KIND4_CURSOR, cursor.wrapping_add(rounded))?;
                Ok(cursor)
            }
            _ => Ok(0),
        }
    }

    fn advance_cursor(
        &mut self,
        cursor_global: u32,
        remaining_global: u32,
        rounded: u32,
    ) -> Fault<u32> {
        if cursor_global == globals::REGION_B_CURSOR
            && (rounded as i32) > self.get(globals::MAIN_REMAINING)? as i32
        {
            self.report_overflow();
        }
        let cursor = self.get(cursor_global)?;
        self.put(cursor_global, cursor.wrapping_add(rounded))?;
        let remaining = self.get(remaining_global)?;
        self.put(remaining_global, remaining.wrapping_sub(rounded))?;
        Ok(cursor)
    }

    /// 0003E1E0 `game_arena_release_tagged_block(block)`: checks the `LOCA`/`LMEM` tags and that
    /// the block is the most recent one, then gives the space back to the top cursor. Its original
    /// non-rejecting diagnostics and post-publication size reread are preserved.
    pub fn release_tagged_block(&mut self, block: u32) -> Fault<()> {
        self.release_tagged_block_with_registers(block).map(|_| ())
    }

    /// Release a tagged block and return the original entry's observable EAX/ECX/EDX values.
    pub fn release_tagged_block_with_registers(
        &mut self,
        block: u32,
    ) -> Fault<TaggedReleaseRegisters> {
        if self.get(block.wrapping_sub(16))? != TAG_LOCA
            || self.get(block.wrapping_sub(12))? != TAG_LMEM
        {
            self.hooks.out_of_memory_with_memory(self.memory, 1);
        }
        let header_self = self.get(block.wrapping_sub(4))?;
        if header_self != self.get(globals::TOP_CURSOR)? {
            self.hooks.out_of_memory_with_memory(self.memory, 1);
        }
        let first_size = self.get(block.wrapping_sub(8))?;
        let top = self.get(globals::TOP_CURSOR)?;
        let remaining = self.get(globals::MAIN_REMAINING)?;
        self.put(
            globals::TOP_CURSOR,
            top.wrapping_add(first_size).wrapping_add(HEADER),
        )?;
        let second_size = self.get(block.wrapping_sub(8))?;
        let new_remaining = remaining.wrapping_add(second_size).wrapping_add(HEADER);
        self.put(globals::MAIN_REMAINING, new_remaining)?;
        Ok(TaggedReleaseRegisters {
            eax: new_remaining,
            ecx: second_size,
            edx: remaining,
        })
    }

    /// 0003E240 `game_arena_rewind_top_to_original_base`: resets the top cursor to the bottom
    /// limit and credits the difference to the remaining space.
    // portreg: va=0x0003e240 tier=1
    pub fn rewind_top(&mut self) -> Fault<()> {
        let limit = self.get(globals::BOTTOM_LIMIT)?;
        let top = self.get(globals::TOP_CURSOR)?;
        let remaining = self.get(globals::MAIN_REMAINING)?;
        self.put(
            globals::MAIN_REMAINING,
            remaining.wrapping_add(limit.wrapping_sub(top)),
        )?;
        self.put(globals::TOP_CURSOR, limit)
    }

    /// 0003E420 `game_global_4c81b4_set`.
    pub fn set_ledger_flag(&mut self, value: u32) -> Fault<()> {
        self.put(globals::LEDGER_FLAG, value)
    }
    /// 0003E430 `game_global_4c81b4_get`.
    pub fn ledger_flag(&mut self) -> Fault<u32> {
        self.get(globals::LEDGER_FLAG)
    }
}
