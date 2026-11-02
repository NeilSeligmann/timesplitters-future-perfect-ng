// SPDX-License-Identifier: GPL-3.0-or-later
//! T2048 slices L and I: the two allocation ledgers and the boot-time initialisation functions.
//!
//! All state stays in the guest globals (see docs/t-memory-replace.md section 2) and every access is
//! a separate ordered [`GuestMemory`] operation, wrapping arithmetic as in the original. Nothing
//! here repairs an invalid state: a full ledger, a release of an unknown block or a count of zero
//! behaves as the original does.
//!
//! * Primary ledger (0x3E440 alloc, 0x3E4E0 release): rows at 0x6B7B68, stride 8 `(pointer, size)`,
//!   base 0x6B836C, running byte total 0x6B8394, count 0x6B8398, enabled flag 0x4C81B4.
//! * Secondary ledger (0x3E560 append, 0x3E5A0 release): rows at 0x6B7B18, base 0x6B8380, byte total
//!   0x6B8368, count 0x6B838C.
//! * 0x3E650 `bounds_compute` and 0x3E740 `history_reset` (boot only).
use crate::w4m_arena::{Arena, ArenaHooks};
use crate::{GuestMemory, MemoryFault};
use tsfp_types::GuestAddr;

/// Coverage inventory for the existing environment-selected C ABI, not a dispatch table.
pub const ROOTS: [u32; 1] = [0x0003E440];

type Fault<T> = Result<T, MemoryFault>;

const PRIMARY_ROWS: u32 = 0x006B_7B68;
const PRIMARY_BASE: u32 = 0x006B_836C;
const PRIMARY_BYTES: u32 = 0x006B_8394;
const PRIMARY_COUNT: u32 = 0x006B_8398;
const LEDGER_ENABLED: u32 = 0x004C_81B4;
const SECOND_ROWS: u32 = 0x006B_7B18;
const SECOND_BYTES: u32 = 0x006B_8368;
const SECOND_BASE: u32 = 0x006B_8380;
const SECOND_COUNT: u32 = 0x006B_838C;
const HISTORY_SIZE: u32 = 0x006B_83AC;

/// Register values left by the primary ledger release entry (0x3E4E0).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct PrimaryReleaseRegisters {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
}

/// Register values left by the secondary ledger entries (0x3E560/0x3E5A0).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct SecondaryLedgerRegisters {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
}

/// A positive signed word (neither zero nor negative): the original's "usable stored size" test.
fn positive(value: u32) -> bool {
    value != 0 && value & 0x8000_0000 == 0
}

/// Calls out of the init functions that are not allocator state.
pub trait InitHooks: ArenaHooks {
    /// A plain guest call with no stack argument: 0x3526E0 (when 0x3E650 first enables the primary
    /// ledger) and 0x1CDAE0 (0x3E740 mode 0 when 0x79094C is 0x64 and 0x7DE455 is not 0x0A).
    fn callout(&mut self, target: u32);
}

/// Ledger functions over a guest memory.
pub struct Ledgers<'a, M: GuestMemory> {
    pub memory: &'a mut M,
}

impl<M: GuestMemory> Ledgers<'_, M> {
    fn get(&mut self, a: u32) -> Fault<u32> {
        self.memory.read_u32(GuestAddr(a))
    }
    fn put(&mut self, a: u32, v: u32) -> Fault<()> {
        self.memory.write_u32(GuestAddr(a), v)
    }
    fn byte(&mut self, a: u32) -> Fault<u8> {
        let mut b = [0];
        self.memory.read(GuestAddr(a), &mut b)?;
        Ok(b[0])
    }
    /// `rep stosd` then `rep stosb` of zero (forward direction, DF = 0).
    fn zero(&mut self, start: u32, dwords: u32, bytes: u32) -> Fault<()> {
        for i in 0..dwords {
            self.put(start.wrapping_add(i.wrapping_mul(4)), 0)?;
        }
        let tail = start.wrapping_add(dwords.wrapping_mul(4));
        for i in 0..bytes {
            self.memory.write(GuestAddr(tail.wrapping_add(i)), &[0])?;
        }
        Ok(())
    }

    /// 0x3E440 `game_allocate_ledger(size)`: round to 16, reuse a released row of exactly that size
    /// (the newest existing row is never scanned), else append. No capacity test. Returns 0 while
    /// the ledger is disabled (0x4C81B4 == 0).
    pub fn allocate_primary(&mut self, requested: u32) -> Fault<u32> {
        let initial = self.get(PRIMARY_BYTES)?;
        let enabled = self.get(LEDGER_ENABLED)?;
        let base = self.get(PRIMARY_BASE)?;
        let size = requested.wrapping_add(15) & 0xFFFF_FFF0;
        let appended = initial.wrapping_add(base);
        if enabled == 0 {
            return Ok(0);
        }
        let count = self.get(PRIMARY_COUNT)?;
        let last = count.wrapping_sub(1);
        let mut offset = 0u32;
        if last != 0 && last & 0x8000_0000 == 0 {
            for row in 0..last {
                let address = PRIMARY_ROWS.wrapping_add(row.wrapping_mul(8));
                if self.get(address)? == 0 && self.get(address.wrapping_add(4))? == size {
                    let block = offset.wrapping_add(base);
                    // The original re-writes the size before publishing the pointer.
                    self.put(address.wrapping_add(4), size)?;
                    self.put(address, block)?;
                    return Ok(block);
                }
                offset = offset.wrapping_add(self.get(address.wrapping_add(4))?);
            }
        }
        let next = self.get(PRIMARY_BYTES)?.wrapping_add(size);
        let row = PRIMARY_ROWS.wrapping_add(count.wrapping_mul(8));
        self.put(row.wrapping_add(4), size)?;
        self.put(row, appended)?;
        self.put(PRIMARY_BYTES, next)?;
        self.put(PRIMARY_COUNT, count.wrapping_add(1))?;
        Ok(appended)
    }

    /// 0x3E4E0 `game_release_allocation(block)`: releasing the newest row pops it and every
    /// directly preceding tombstone with a positive size; any other block becomes a tombstone
    /// (pointer zeroed, size kept). A release with count 0 still marks the wrapped row, as the
    /// original does.
    pub fn release_primary(&mut self, block: u32) -> Fault<()> {
        let count = self.get(PRIMARY_COUNT)?;
        self.release_primary_with_count_and_registers(count, block)
            .map(|_| ())
    }

    /// Release using a previously sampled count, preserving the C adapter's read order.
    pub fn release_primary_with_count_and_registers(
        &mut self,
        count: u32,
        block: u32,
    ) -> Fault<PrimaryReleaseRegisters> {
        let last = count.wrapping_sub(1);
        let row = |index: u32| PRIMARY_ROWS.wrapping_add(index.wrapping_mul(8));
        let pointer = self.get(row(last))?;
        let mut registers = PrimaryReleaseRegisters {
            eax: last,
            ecx: pointer,
            edx: block,
        };
        if block == pointer {
            let mut bytes = self.get(PRIMARY_BYTES)?;
            bytes = bytes.wrapping_sub(self.get(row(last).wrapping_add(4))?);
            self.put(PRIMARY_BYTES, bytes)?;
            let mut last = last;
            loop {
                last = last.wrapping_sub(1);
                self.put(row(last).wrapping_add(12), u32::MAX)?;
                if last & 0x8000_0000 != 0 {
                    break;
                }
                let pointer = self.get(row(last))?;
                registers.ecx = pointer;
                if pointer != 0 {
                    break;
                }
                let size = self.get(row(last).wrapping_add(4))?;
                registers.ecx = size;
                if !positive(size) {
                    break;
                }
                bytes = bytes.wrapping_sub(size);
            }
            self.put(PRIMARY_BYTES, bytes)?;
            registers.eax = last.wrapping_add(1);
            registers.edx = bytes;
            self.put(PRIMARY_COUNT, registers.eax)?;
            return Ok(registers);
        }
        registers.ecx = 0;
        if !positive(last) {
            return Ok(registers);
        }
        let mut index = 0u32;
        loop {
            if self.get(row(index))? == block {
                self.put(row(index), 0)?;
                registers.ecx = index;
                return Ok(registers);
            }
            index = index.wrapping_add(1);
            if index >= last {
                registers.ecx = index;
                return Ok(registers);
            }
        }
    }

    /// 0x3E560 `game_append_secondary(size)`: appends a row at `base + bytes`. Returns the pointer
    /// (the original leaves it in EAX although callers treat the function as void).
    pub fn append_secondary(&mut self, size: u32) -> Fault<u32> {
        self.append_secondary_with_registers(size)
            .map(|registers| registers.eax)
    }

    /// Append while retaining the original EAX/ECX/EDX outputs for a host binding.
    pub fn append_secondary_with_registers(
        &mut self,
        size: u32,
    ) -> Fault<SecondaryLedgerRegisters> {
        let bytes = self.get(SECOND_BYTES)?;
        let base = self.get(SECOND_BASE)?;
        self.append_secondary_from_inputs(bytes, base, size)
    }

    /// Read the size argument after the guest globals, matching the original entry's access order.
    pub fn append_secondary_with_argument_reader<F>(
        &mut self,
        read_size: F,
    ) -> Fault<SecondaryLedgerRegisters>
    where
        F: FnOnce() -> u32,
    {
        let bytes = self.get(SECOND_BYTES)?;
        let base = self.get(SECOND_BASE)?;
        let size = read_size();
        self.append_secondary_from_inputs(bytes, base, size)
    }

    fn append_secondary_from_inputs(
        &mut self,
        bytes: u32,
        base: u32,
        size: u32,
    ) -> Fault<SecondaryLedgerRegisters> {
        let pointer = base.wrapping_add(bytes);
        self.put(SECOND_BYTES, bytes.wrapping_add(size))?;
        let count = self.get(SECOND_COUNT)?;
        let row = SECOND_ROWS.wrapping_add(count.wrapping_mul(8));
        self.put(row, pointer)?;
        self.put(row.wrapping_add(4), size)?;
        let next_count = count.wrapping_add(1);
        self.put(SECOND_COUNT, next_count)?;
        Ok(SecondaryLedgerRegisters {
            eax: pointer,
            ecx: next_count,
            edx: size,
        })
    }

    /// 0x3E5A0 `game_release_secondary(block)`: pops the newest row (and positive-size tombstones
    /// below it) or tombstones a middle row. Returns 1 when something was released, else 0.
    pub fn release_secondary(&mut self, block: u32) -> Fault<u32> {
        let count = self.get(SECOND_COUNT)?;
        self.release_secondary_with_count_and_registers(count, block)
            .map(|registers| registers.eax)
    }

    /// Release with a previously sampled count, preserving EAX/ECX/EDX and the original read order.
    pub fn release_secondary_with_count_and_registers(
        &mut self,
        count: u32,
        block: u32,
    ) -> Fault<SecondaryLedgerRegisters> {
        let row = |index: u32| SECOND_ROWS.wrapping_add(index.wrapping_mul(8));
        let newest = self.get(row(count).wrapping_sub(8))?;
        let last = count.wrapping_sub(1);
        let mut registers = SecondaryLedgerRegisters {
            eax: last,
            ecx: newest,
            edx: block,
        };
        if block == newest {
            let mut bytes = self.get(SECOND_BYTES)?;
            bytes = bytes.wrapping_sub(self.get(row(last).wrapping_add(4))?);
            self.put(SECOND_BYTES, bytes)?;
            let mut last = last;
            loop {
                last = last.wrapping_sub(1);
                registers.eax = last;
                self.put(row(last).wrapping_add(12), u32::MAX)?;
                if last & 0x8000_0000 != 0 {
                    break;
                }
                let pointer = self.get(row(last))?;
                registers.ecx = pointer;
                if pointer != 0 {
                    break;
                }
                let size = self.get(row(last).wrapping_add(4))?;
                registers.ecx = size;
                if !positive(size) {
                    break;
                }
                bytes = bytes.wrapping_sub(size);
            }
            registers.eax = last.wrapping_add(1);
            registers.edx = bytes;
            self.put(SECOND_COUNT, registers.eax)?;
            self.put(SECOND_BYTES, bytes)?;
            registers.eax = 1;
            return Ok(registers);
        }
        registers.ecx = 0;
        if positive(last) {
            let mut index = 0u32;
            loop {
                if self.get(row(index))? == block {
                    self.put(row(index), 0)?;
                    registers.eax = 1;
                    registers.ecx = index;
                    return Ok(registers);
                }
                index = index.wrapping_add(1);
                registers.ecx = index;
                if index >= last {
                    break;
                }
            }
        }
        registers.eax = 0;
        Ok(registers)
    }

    /// 0x3E650 `pool_bounds_compute()`: derive the region boundaries from the arena block
    /// (0x7E72D4 base, 0x7E72D8 size), reserving 0x266000 for the primary ledger while it is
    /// enabled and 0x131800 when bit 0x40 of 0x7DE398 is set, zero the kind 1 region (and the
    /// secondary ledger table in the 0x40 case), and on the first enabling of the primary ledger
    /// (0x7DE394 == 0x65 while 0x4C81B4 was 0) zero its 0x200 dword table and call 0x3526E0.
    /// Needs DF = 0 (forward string operations) like the original. A size smaller than the
    /// split makes the zero loop run for a huge count, as the original does (not repaired).
    pub fn bounds_compute<H: InitHooks>(&mut self, hooks: &mut H) -> Fault<()> {
        let tag = self.byte(0x7DE394)?;
        let old = self.get(LEDGER_ENABLED)?;
        let mut flag = old;
        if tag == 0x65 {
            flag = 1;
            self.put(LEDGER_ENABLED, 1)?;
        }
        let secondary = self.get(0x7DE398)? & 0x40;
        let size = self.get(0x7E72D8)?;
        let base = self.get(0x7E72D4)?;
        let extra = if secondary != 0 { 0x0013_1800 } else { 0 };
        let ledger = if flag != 0 { 0x0026_6000 } else { 0 };
        let end = size
            .wrapping_sub(ledger)
            .wrapping_sub(extra)
            .wrapping_add(base);
        let split = base.wrapping_add(0x9DC00);
        self.put(0x6B837C, split)?;
        self.put(0x6B8380, end)?;
        if secondary != 0 {
            self.zero(SECOND_ROWS, 0x14, 0)?;
            self.put(SECOND_COUNT, 0)?;
            self.put(SECOND_BYTES, 0)?;
        }
        let remaining = end.wrapping_sub(split);
        self.put(0x6B8384, remaining)?;
        self.zero(split, remaining >> 2, remaining & 3)?;
        let top = self.get(0x6B8380)?;
        self.put(0x6B8388, top)?;
        if old == 0 && flag != 0 {
            self.zero(PRIMARY_ROWS, 0x200, 0)?;
            self.put(PRIMARY_COUNT, 0)?;
            self.put(PRIMARY_BYTES, 0)?;
            hooks.callout(0x3526E0);
        }
        self.put(0x6B8390, 0)
    }

    /// 0x3E740 `history_buffer_reset_by_mode(mode)`: pick the history buffer size for the mode
    /// (0 depends on 0x7DE455 and 0x79094C and may call 0x1CDAE0, 1 is 0x4B000, 2 is 0x100000,
    /// 3 is 0x4000; a mode above 3 reports 0x389B0(0) and keeps the old size), rewind the top
    /// cursor (0x3E240), allocate `2 * size` from the top (0x3DFA0 kind 0), split it into the two
    /// scratch buffers, clear the high-water mark, then tail-report 0x389B0 with the caller's
    /// mode argument.
    pub fn history_reset<H: InitHooks>(&mut self, hooks: &mut H, mode: u32) -> Fault<()> {
        match mode {
            0 => {
                let gate = self.byte(0x7DE455)?;
                let selector = u32::from(gate != 0x0A).wrapping_sub(1);
                self.put(HISTORY_SIZE, (selector & 0xFFFB_5000).wrapping_add(0x96000))?;
                if self.get(0x79094C)? == 0x64 {
                    if gate != 0x0A {
                        hooks.callout(0x1CDAE0);
                    }
                    self.put(HISTORY_SIZE, 0xBB800)?;
                }
            }
            1 => self.put(HISTORY_SIZE, 0x4B000)?,
            2 => self.put(HISTORY_SIZE, 0x100000)?,
            3 => self.put(HISTORY_SIZE, 0x4000)?,
            _ => hooks.out_of_memory(0),
        }
        let buffer = {
            let mut arena = Arena {
                memory: &mut *self.memory,
                hooks: &mut *hooks,
            };
            arena.rewind_top()?;
            let size = arena.memory.read_u32(GuestAddr(HISTORY_SIZE))?;
            arena.alloc_round16(0, size.wrapping_add(size))?
        };
        let size = self.get(HISTORY_SIZE)?;
        let second = buffer.wrapping_add(size);
        self.put(0x6B83B4, buffer)?;
        self.put(0x6B83A0, buffer)?;
        self.put(0x6B83A4, second)?;
        self.put(0x6B839C, buffer)?;
        self.put(0x6B83B0, 0)?;
        hooks.out_of_memory(mode);
        Ok(())
    }
}
