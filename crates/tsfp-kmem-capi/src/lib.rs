// SPDX-License-Identifier: GPL-3.0-or-later
//! T2048 slice K live binding: the Rust kernel memory model inside the hybrid host.
//!
//! Design and measurements: docs/t-memory-replace.md section 8.5. The C host (`src/xbox/kmem_rust.c`, built only with
//! `-DTSFP_RUST_KMEM=ON`) routes the `guest_mem.h` API and the Mm*/Nt*VirtualMemory/Ex pool ordinals here:
//!
//! * mode 1 (`TSFP_RUST_KMEM=1`): Rust answers, backed by real low-4GB `mmap` ([`placement::MmapPlacement`]).
//! * mode 2 (`TSFP_RUST_KMEM=compare`): the C code answers, the model is driven with the same call and answered with
//!   the C host's placement results ([`placement::ReplayPlacement`]); the C side compares and counts mismatches.
//! * mode 0 / unset: the C code answers and nothing here runs.
//!
//! T2048 also exposes `0x3E970` and `0x3DFA0` through game allocator C ABI functions, using
//! callbacks into the existing guest image. The game pool and primary ledger have independent
//! opt-in routes as well. Their build and runtime switches are separate from kernel-memory mode.
mod native_heap;
pub use native_heap::tsfp_kmem_heap_free;
pub mod ops;
pub mod placement;
mod pool_audit_runtime;
mod replay_placement;

use std::sync::Mutex;

use tsfp_engine_memory::guest_regions::Placement;
use tsfp_engine_memory::kernel_mm::KernelMemory;
use tsfp_engine_memory::ledger::{InitHooks, Ledgers, SecondaryLedgerRegisters};
use tsfp_engine_memory::service_slice::{GameAllocator, MemoryServices};
use tsfp_engine_memory::wrappers::AllocWrappers;
use tsfp_engine_memory::{
    Access, GuestMemory, MemoryFault,
    pool::{Pool, PoolHooks},
    w4m_arena::{Arena, ArenaHooks, TaggedReleaseRegisters},
};
use tsfp_types::GuestAddr;

use placement::{MmapPlacement, PlaceEvent, ReplayPlacement};

/// Either backend behind one type, so the global model has one concrete `KernelMemory`.
pub enum Backend {
    Mmap(MmapPlacement),
    Replay(ReplayPlacement),
}

#[cfg(test)]
mod game_31aaa0_wrapper_tests {
    use super::*;
    use std::collections::HashMap;

    struct Image {
        top: u32,
        region_b_cursor: u32,
        remaining: u32,
        words: HashMap<u32, u32>,
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        match address {
            0x6B83_88 => image.top,
            0x6B83_7C => image.region_b_cursor,
            0x6B83_84 => image.remaining,
            _ => image.words.get(&address).copied().unwrap_or(0),
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        match address {
            0x6B83_88 => image.top = value,
            0x6B83_7C => image.region_b_cursor = value,
            0x6B83_84 => image.remaining = value,
            _ => {
                image.words.insert(address, value);
            }
        }
        image.writes.push((address, value));
    }

    #[test]
    fn game_wrapper_31aaa0_c_abi_copies_five_words_and_publishes_before_last_word() {
        let source = 0x0076_2F80;
        let values = [
            0x1122_3344,
            0x5566_7788,
            0x99AA_BBCC,
            0xDDEE_F001,
            0x2345_6789,
        ];
        let mut image = Image {
            top: 0x2000,
            region_b_cursor: 0,
            remaining: 0x1000,
            words: values
                .iter()
                .enumerate()
                .map(|(index, value)| (source + index as u32 * 4, *value))
                .collect(),
            writes: Vec::new(),
        };
        let mut out = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_pick_node_762f7c(
                source,
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut out,
            )
        };

        let block = 0x1FE0;
        assert_eq!(status, 0);
        assert_eq!(out, block);
        assert_eq!(image.top, 0x1FD0);
        assert_eq!(image.remaining, 0xFD0);
        assert_eq!(image.words.get(&0x1FD0), Some(&0x4143_4F4C));
        assert_eq!(image.words.get(&0x1FD4), Some(&0x4D45_4D4C));
        assert_eq!(image.words.get(&0x1FD8), Some(&0x20));
        assert_eq!(image.words.get(&0x1FDC), Some(&0x1FD0));
        assert_eq!(image.words.get(&0x762F_7C), Some(&block));
        for (index, value) in values.iter().enumerate() {
            assert_eq!(image.words.get(&(block + index as u32 * 4)), Some(value));
        }
        assert_eq!(image.writes[image.writes.len() - 2], (0x762F_7C, block));
        assert_eq!(image.writes.last(), Some(&(block + 0x10, values[4])));
    }

    #[test]
    fn game_wrapper_3796b0_c_abi_allocates_and_initializes_node_pool() {
        let mut image = Image {
            top: 0x4000,
            region_b_cursor: 0x2000,
            remaining: 0x2000,
            words: HashMap::new(),
            writes: Vec::new(),
        };
        let mut out_nodes = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_node_pool_772b50(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut out_nodes,
            )
        };

        let nodes = 0x2000;
        assert_eq!(status, 0);
        assert_eq!(out_nodes, nodes);
        assert_eq!(image.region_b_cursor, nodes + 0x960);
        assert_eq!(image.remaining, 0x16A0);
        assert_eq!(image.words.get(&0x772B50), Some(&nodes));
        assert_eq!(image.words.get(&0x772B54), Some(&(nodes + 0x8C0)));
        assert_eq!(image.words.get(&0x772B58), Some(&0xF));
        assert_eq!(image.words.get(&0x772B5C), Some(&0xF));
        for index in 0..0xF {
            let link = if index == 0 {
                0
            } else {
                nodes + (index - 1) * 0xA0
            };
            assert_eq!(image.words.get(&(nodes + index * 0xA0)), Some(&link));
        }
        assert_eq!(image.words.get(&(nodes + 0x960)), None);
        assert_eq!(image.writes.last(), Some(&(0x772B54, nodes + 0x8C0)));
    }

    #[test]
    fn game_wrapper_31c930_c_abi_releases_global_tagged_block_and_returns_pointer() {
        let block = 0x4020;
        let top = block - 0x10;
        let mut image = Image {
            top,
            region_b_cursor: 0,
            remaining: 0x1000,
            words: HashMap::from([
                (0x762F90, block),
                (block - 0x10, 0x4143_4F4C),
                (block - 0x0C, 0x4D45_4D4C),
                (block - 0x08, 0x30),
                (block - 0x04, top),
            ]),
            writes: Vec::new(),
        };
        let mut registers = GameArenaReleaseRegisters::default();
        let status = unsafe {
            tsfp_kmem_game_wrapper_release_mapedit_static_block_762f90(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut registers,
            )
        };

        assert_eq!(status, 0);
        assert_eq!(
            registers,
            GameArenaReleaseRegisters {
                eax: block,
                ecx: 0x30,
                edx: 0x1000
            }
        );
        assert_eq!(image.top, top + 0x40);
        assert_eq!(image.remaining, 0x1040);
        assert_eq!(image.words.get(&0x762F90), Some(&block));
    }

    #[test]
    fn game_wrapper_31e430_c_abi_releases_record_rows_tagged_block_and_returns_pointer() {
        let block = 0x5010;
        let top = block - 0x10;
        let mut image = Image {
            top,
            region_b_cursor: 0,
            remaining: 0x1000,
            words: HashMap::from([
                (0x7630B0, block),
                (block - 0x10, 0x4143_4F4C),
                (block - 0x0C, 0x4D45_4D4C),
                (block - 0x08, 0x30),
                (block - 0x04, top),
            ]),
            writes: Vec::new(),
        };
        let mut registers = GameArenaReleaseRegisters::default();
        let status = unsafe {
            tsfp_kmem_game_wrapper_release_record_rows_panel_block_7630b0(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut registers,
            )
        };

        assert_eq!(status, 0);
        assert_eq!(
            registers,
            GameArenaReleaseRegisters {
                eax: block,
                ecx: 0x30,
                edx: 0x1000
            }
        );
        assert_eq!(image.top, top + 0x40);
        assert_eq!(image.remaining, 0x1040);
        assert_eq!(image.words.get(&0x7630B0), Some(&block));
    }
}

impl Placement for Backend {
    fn map(&mut self, length: u64, fixed: Option<u32>) -> Option<u32> {
        match self {
            Backend::Mmap(p) => p.map(length, fixed),
            Backend::Replay(p) => p.map(length, fixed),
        }
    }
    fn unmap(&mut self, base: u32, length: u64) {
        match self {
            Backend::Mmap(p) => p.unmap(base, length),
            Backend::Replay(p) => p.unmap(base, length),
        }
    }
    fn write_guest(&mut self, addr: u32, data: &[u8]) {
        if let Backend::Mmap(p) = self {
            p.write_guest(addr, data);
        }
    }
    fn copy_guest(&mut self, dst: u32, src: u32, len: u32) {
        if let Backend::Mmap(p) = self {
            p.copy_guest(dst, src, len);
        }
    }
    fn read_guest(&self, addr: u32, out: &mut [u8]) -> bool {
        match self {
            Backend::Mmap(p) => p.read_guest(addr, out),
            Backend::Replay(_) => false,
        }
    }
    fn backs_guest(&self) -> bool {
        matches!(self, Backend::Mmap(_))
    }
}

pub const MODE_OFF: u32 = 0;
pub const MODE_RUST: u32 = 1;
pub const MODE_COMPARE: u32 = 2;

struct State {
    mode: u32,
    km: KernelMemory<Backend>,
    /// Compare mode: C placement events left unconsumed by the model after an op.
    leftover_events: u64,
    /// Log lines the model produced (compare mode drops them, rust mode prints them).
    log_lines: u64,
    ops: u64,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

pub type GameRead32 = unsafe extern "C" fn(context: *mut core::ffi::c_void, address: u32) -> u32;
pub type GameWrite32 =
    unsafe extern "C" fn(context: *mut core::ffi::c_void, address: u32, value: u32);
pub type GamePoolReclaim =
    unsafe extern "C" fn(context: *mut core::ffi::c_void, callback: u32, argument: u32);
pub type GamePoolRandom = unsafe extern "C" fn(context: *mut core::ffi::c_void) -> u32;
pub type GameReadArgument = unsafe extern "C" fn(context: *mut core::ffi::c_void) -> u32;
pub type GameRead8 = unsafe extern "C" fn(context: *mut core::ffi::c_void, address: u32) -> u8;
pub type GameWrite8 =
    unsafe extern "C" fn(context: *mut core::ffi::c_void, address: u32, value: u8);
pub type GameArenaReport = unsafe extern "C" fn(context: *mut core::ffi::c_void, argument: u32);

struct CallbackGameMemory {
    context: *mut core::ffi::c_void,
    read32: GameRead32,
    write32: GameWrite32,
}

struct CallbackByteGameMemory {
    context: *mut core::ffi::c_void,
    read32: GameRead32,
    write32: GameWrite32,
    read8: GameRead8,
    write8: GameWrite8,
}

struct NoArenaHooks;

impl ArenaHooks for NoArenaHooks {
    fn out_of_memory(&mut self, _argument: u32) {}
}

struct NoPoolHooks;

impl PoolHooks for NoPoolHooks {
    fn reclaim(&mut self, _callback: u32, _argument: u32) {}
}

impl ArenaHooks for NoPoolHooks {
    fn out_of_memory(&mut self, _argument: u32) {}
}

struct CallbackReclaimHooks {
    context: *mut core::ffi::c_void,
    reclaim: GamePoolReclaim,
}

impl PoolHooks for CallbackReclaimHooks {
    fn reclaim(&mut self, callback: u32, argument: u32) {
        unsafe { (self.reclaim)(self.context, callback, argument) };
    }
}

impl ArenaHooks for CallbackReclaimHooks {
    fn out_of_memory(&mut self, _argument: u32) {}
}

struct CallbackRandomPointHooks {
    context: *mut core::ffi::c_void,
    read32: GameRead32,
    write32: GameWrite32,
    reclaim: GamePoolReclaim,
    random: GamePoolRandom,
}

impl PoolHooks for CallbackRandomPointHooks {
    fn reclaim(&mut self, callback: u32, argument: u32) {
        if callback == 0x0021_0FF0 {
            let mut memory = CallbackGameMemory {
                context: self.context,
                read32: self.read32,
                write32: self.write32,
            };
            tsfp_engine_memory::wrappers::clear_random_point_pool_owner(&mut memory)
                .expect("known point-pool owner fields are writable guest dwords");
        } else {
            unsafe { (self.reclaim)(self.context, callback, argument) };
        }
    }
}

impl ArenaHooks for CallbackRandomPointHooks {
    fn out_of_memory(&mut self, _argument: u32) {}
}

impl tsfp_engine_memory::wrappers::PointPoolRandom for CallbackRandomPointHooks {
    fn next_point_random(&mut self) -> u32 {
        unsafe { (self.random)(self.context) }
    }
}

struct CallbackPoolHooks {
    context: *mut core::ffi::c_void,
    report: Option<GameArenaReport>,
}

impl PoolHooks for CallbackPoolHooks {
    fn reclaim(&mut self, _callback: u32, _argument: u32) {}
}

impl ArenaHooks for CallbackPoolHooks {
    fn out_of_memory(&mut self, argument: u32) {
        if let Some(report) = self.report {
            unsafe { report(self.context, argument) };
        }
    }
}

impl GuestMemory for CallbackGameMemory {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        if out.len() != 4 {
            return Err(MemoryFault {
                address,
                length: out.len(),
                access: Access::Read,
            });
        }
        let value = unsafe { (self.read32)(self.context, address.0) };
        out.copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        if bytes.len() != 4 {
            return Err(MemoryFault {
                address,
                length: bytes.len(),
                access: Access::Write,
            });
        }
        let value = u32::from_le_bytes(bytes.try_into().expect("checked four byte write"));
        unsafe { (self.write32)(self.context, address.0, value) };
        Ok(())
    }
}

impl GuestMemory for CallbackByteGameMemory {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        if out.len() == 4 {
            let value = unsafe { (self.read32)(self.context, address.0) };
            out.copy_from_slice(&value.to_le_bytes());
        } else {
            for (offset, byte) in out.iter_mut().enumerate() {
                *byte =
                    unsafe { (self.read8)(self.context, address.0.wrapping_add(offset as u32)) };
            }
        }
        Ok(())
    }

    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        if bytes.len() == 4 {
            let value = u32::from_le_bytes(bytes.try_into().expect("four-byte guest write"));
            unsafe { (self.write32)(self.context, address.0, value) };
        } else {
            for (offset, byte) in bytes.iter().copied().enumerate() {
                unsafe { (self.write8)(self.context, address.0.wrapping_add(offset as u32), byte) };
            }
        }
        Ok(())
    }
}

/// Run the readable Rust game bump allocator against the host's identity-mapped guest image.
/// The caller supplies the two guest-word accessors used by the lifted host.
/// Returns 0 on success and writes the block address to `out`.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call, and `out` must point to
/// one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_bump_allocate(
    size: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = NoArenaHooks;
    match (Arena {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .bump_allocate(size)
    {
        Ok(block) => {
            unsafe { *out = block };
            0
        }
        Err(_) => 1,
    }
}

/// Run the readable Rust arena allocator against the host's identity-mapped guest image.
/// The caller supplies the guest-word accessors used by the lifted host.
/// Returns 0 on success and writes the allocation address to `out`.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call, and `out` must point to
/// one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_arena_allocate(
    kind: u32,
    size: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (Arena {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_round16(kind, size)
    {
        Ok(block) => {
            unsafe { *out = block };
            0
        }
        Err(_) => 1,
    }
}

/// Run the readable Rust alignment wrapper (0x3E0B0) against the host guest image.
/// The function retains the guest memory image as allocator state and returns its guest address.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call, and `out` must point to
/// one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_arena_allocate_aligned(
    alignment: u32,
    size: u32,
    kind: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (Arena {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .allocate_aligned(alignment, size, kind)
    {
        Ok(block) => {
            unsafe { *out = block };
            0
        }
        Err(_) => 1,
    }
}

/// Run the readable Rust constructor for the 0x1590-byte table at 0x75B4B4.
/// The arena and the published pointer remain in the existing guest image.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_aligned_75b4b4(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_aligned_75b4b4()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Allocate and publish the 0x640-byte kind-1 block for original entry 0x24A180.
/// The caller keeps the original tail-call to the 0x2498C0 initializer.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_model_buffer_78b718(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_model_buffer_78b718()
    {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

/// Run the readable Rust wrapper for the two Region-A buffers at `0x54EF14/0x54EF18`.
/// The arena cursors and published guest addresses remain in the existing guest image.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_buffers_54ef14(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_buffers_54ef14()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the readable 0x32A860 save-slot buffer constructor on the existing guest image.
/// The Region-B cursor, mode, published pointer and payload all remain guest-addressed.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_save_slot_778d38(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_save_slot_buffer_778d38()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the readable 0x12C800 static guest-slot allocator on the existing guest image.
/// The returned address stays at the original 0x745938 table and advances the guest count.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call, and `out_slot` must
/// point to one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_allocate_slot_745938(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    out_slot: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out_slot.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks {
        context,
        report: None,
    };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .allocate_slot_745938_max17()
    {
        Ok(slot) => {
            unsafe { out_slot.write(slot) };
            0
        }
        Err(_) => 1,
    }
}

/// Run the readable 0x2FE200 -> 0x336600 Map Maker allocation path on the existing guest image.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_mapedit_buffers(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_mapedit_buffers()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the readable weather table constructor from the existing guest image.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call, and `out_eax` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_weather_74c3b0(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out_eax: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out_eax.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_weather_tables_74c3b0()
    {
        Ok(eax) => {
            unsafe { *out_eax = eax };
            0
        }
        Err(_) => 1,
    }
}

/// Run the inferred `0x119ED0` weather-table allocation and reset sequence.
/// The original guest image remains the source for its row count and pointer globals.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_weather_tables_7ac824(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_weather_tables_with_reset_7ac824()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the readable two-table constructor for original entry `0x274650`.
/// Both table pointers and allocator state remain in the existing guest image.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call, and `out_eax` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_tables_74c3b8(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out_eax: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out_eax.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_tables_74c3b8()
    {
        Ok(eax) => {
            unsafe { *out_eax = eax };
            0
        }
        Err(_) => 1,
    }
}

/// Run the Rust allocation/publication seam for the original 0x188900 constructor.
/// Its caller remains responsible for the following 0x188870 effect-slot initialization.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call, and `out_block` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_effect_slots_7a156c(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out_block: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out_block.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_effect_slots_7a156c()
    {
        Ok(block) => {
            unsafe { *out_block = block };
            0
        }
        Err(_) => 1,
    }
}

/// Run the native allocation/publication seam for effect-mesh buffer `0x2BC150`.
/// Building the mesh remains at the original tail target `0x2BBC40`.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_effect_mesh_buffer_78aef0(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_effect_mesh_buffer_78aef0()
    {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

/// Run the readable Rust allocation/publication seam for the first array in 0x237480.
/// Its caller passes the computed byte size; neighboring allocations and initialization remain outside this seam.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call, and `out_block` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_story_event_instances_78cd90(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    size: u32,
    out_block: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out_block.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_story_event_instances_78cd90(size)
    {
        Ok(block) => {
            unsafe { *out_block = block };
            0
        }
        Err(_) => 1,
    }
}

/// Allocate and publish the active-index and snapshot arrays used by lifted entry `0x237480`.
/// Snapshot zeroing and `0x235630` initialization remain with the guest adapter.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call, and both output pointers must
/// be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_story_event_support_arrays(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    index_size: u32,
    snapshot_size: u32,
    out_indices: *mut u32,
    out_snapshot: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out_indices.is_null() || out_snapshot.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_story_event_support_arrays(index_size, snapshot_size)
    {
        Ok((indices, snapshot)) => {
            unsafe {
                *out_indices = indices;
                *out_snapshot = snapshot;
            }
            0
        }
        Err(_) => 1,
    }
}

/// Run the Rust allocation and initialization seam for Story objective counters at `0x1620C0`.
/// Definition records and the resulting counter table remain at stable guest addresses.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_story_objective_counters(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_story_objective_counters_7a2820()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the Rust allocation seam for `0x2760F0`, publishing the 0x190-byte table at `0x78B240`.
/// The table pointer remains in the guest image, preserving its stable address contract.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_table_78b240(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_table_78b240()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the Rust `0x2210B0` sound-handle table allocation on the existing guest image.
/// The published pointer remains at its original guest address for lifted readers.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_table_75c10c(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_table_75c10c()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the Rust `0x220E20` state-block constructor on the guest image. The allocation address
/// stays at `0x75C108`; descriptor `0x18` readiness is reset through the table pointer at
/// `0x4CA9B4`.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_state_block_75c108(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_state_block_75c108()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the Rust `0x2259D0` sound-state constructor on the guest image. The inferred state pointer
/// remains at `0x75C124`; readiness for table entries `0x1B` and `0x19` is reset in place.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_sound_state_75c124(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_sound_state_75c124()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the Rust `0x2226F0` timer-state constructor on the existing guest image.
/// It publishes the 16-byte block at `0x75C118` and resets five descriptor readiness words.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_timer_state_75c118(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_timer_state_75c118()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the Rust `0x21AF30` 16-byte table allocation on the existing guest image.
/// The published pointer remains at its original guest address for lifted readers.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_table_75c0f0(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_table_75c0f0()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the Rust `0x2A62C0` mode-0x51 buffer allocation on the existing guest image.
/// The published pointer remains at its original guest address for lifted readers.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_table_78b0d0(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_table_78b0d0()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the Rust `0x2A8300` two-buffer constructor against the existing guest image.
/// Returns the second allocation in `out`; all three published globals remain guest-backed.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call and `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_tables_78b08c(
    flag: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_tables_78b08c(flag)
    {
        Ok(second) => {
            unsafe { *out = second };
            0
        }
        Err(_) => 1,
    }
}

/// Run the Rust 0x31AAA0 constructor against the existing guest image. `source` is the pointer
/// passed in the caller's stack slot; the result pointer is written to `out` after the arena call.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call and `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_pick_node_762f7c(
    source: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_pick_node_copy_762f7c(source)
    {
        Ok(block) => {
            unsafe { *out = block };
            0
        }
        Err(_) => 1,
    }
}

/// Run the Rust 0x3796B0 constructor against the existing guest image. The allocated node buffer
/// and freelist descriptor remain at their original guest addresses.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call and `out_nodes` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_node_pool_772b50(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out_nodes: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out_nodes.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .alloc_node_pool_772b50()
    {
        Ok(nodes) => {
            unsafe { *out_nodes = nodes };
            0
        }
        Err(_) => 1,
    }
}

/// Initialize the measured 0x156DB0 fixed-stride guest pool in Rust.
/// # Safety
/// Callbacks must remain valid for the call; both outputs must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_fixed_pool_init(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    pool: u32,
    stride: u32,
    storage: u32,
    count: u32,
    out_head: *mut u32,
    out_end: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out_head.is_null() || out_end.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    match tsfp_engine_memory::fixed_pool::initialize_freelist(
        &mut memory,
        pool,
        stride,
        storage,
        count,
    ) {
        Ok((head, end)) => {
            unsafe {
                *out_head = head;
                *out_end = end;
            }
            0
        }
        Err(_) => 1,
    }
}

/// C ABI results for a guest-image `0x3E1E0` tagged release.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GameArenaReleaseRegisters {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
}

/// Run the readable Rust tagged-block release against the host's identity-mapped guest image.
/// The register result matches the original entry's EAX, ECX and EDX on success.
///
/// # Safety
/// The callbacks must be valid C functions for the duration of the call, and `out` must point to
/// one writable [`GameArenaReleaseRegisters`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_arena_release_tagged(
    block: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out: *mut GameArenaReleaseRegisters,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (Arena {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .release_tagged_block_with_registers(block)
    {
        Ok(TaggedReleaseRegisters { eax, ecx, edx }) => {
            unsafe {
                *out = GameArenaReleaseRegisters { eax, ecx, edx };
            }
            0
        }
        Err(_) => 1,
    }
}

/// Run the Rust 0x31C930 Map Maker helper against the existing guest image. The global pointer
/// remains address-stable; the return register is the re-read pointer and ECX/EDX are from the
/// tagged arena release.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call and `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_release_mapedit_static_block_762f90(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out: *mut GameArenaReleaseRegisters,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .release_mapedit_static_block_762f90()
    {
        Ok(TaggedReleaseRegisters { eax, ecx, edx }) => {
            unsafe { *out = GameArenaReleaseRegisters { eax, ecx, edx } };
            0
        }
        Err(_) => 1,
    }
}

/// Run the Rust 0x31E430 record-rows panel release helper against the existing guest image.
/// The stable global remains the returned EAX value; ECX/EDX carry the tagged-release results.
///
/// # Safety
/// The callbacks must remain valid for the duration of the call and `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_release_record_rows_panel_block_7630b0(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    report: Option<GameArenaReport>,
    out: *mut GameArenaReleaseRegisters,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks { context, report };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .release_record_rows_panel_block_7630b0()
    {
        Ok(TaggedReleaseRegisters { eax, ecx, edx }) => {
            unsafe { *out = GameArenaReleaseRegisters { eax, ecx, edx } };
            0
        }
        Err(_) => 1,
    }
}

/// Rewind the main arena top (0x3E240) while keeping its guest address stable.
///
/// # Safety
/// The callbacks must remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_arena_rewind_top(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = NoArenaHooks;
    match (Arena {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .rewind_top()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the readable Rust locked-pool allocation path (0x3ECA0) against the live guest image.
/// This is the callback-free kind-2 path; reclaimable kind-1 allocations remain on the C path
/// until their native owner-lifetime contract is implemented.
///
/// # Safety
/// The callbacks must remain valid for this call, and `out` must point to one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_pool_locked_allocate(
    size: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = NoPoolHooks;
    let mut pool = Pool {
        memory: &mut memory,
        hooks: &mut hooks,
    };
    match pool.allocate_by_kind(size, 2, 0, 0) {
        Ok(block) => {
            unsafe { *out = block };
            0
        }
        Err(_) => 1,
    }
}

/// Run the readable kind-dispatch pool allocation path (`0x3ED20`) against the guest image.
/// Its synchronous owner callback remains a temporary host adapter until T2048 S3 moves owner
/// identity and leases into Rust-owned state.
///
/// # Safety
/// The memory and reclaim callbacks must remain valid for the whole call, including callback
/// reentry, and `out` must point to one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_pool_allocate_by_kind(
    size: u32,
    kind: u32,
    argument: u32,
    callback: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    reclaim: Option<GamePoolReclaim>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32), Some(reclaim)) = (read32, write32, reclaim) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackReclaimHooks { context, reclaim };
    let mut pool = Pool {
        memory: &mut memory,
        hooks: &mut hooks,
    };
    match pool.allocate_by_kind(size, kind, argument, callback) {
        Ok(block) => {
            unsafe { *out = block };
            0
        }
        Err(_) => 1,
    }
}

/// Allocate and publish the particle buffer owned by a guest emitter at `0x10C190`.
/// The host calls the retained old-buffer release and count helper first; this bridge owns the
/// typed owner view, count/stride rule, kind-1 allocation, and ordered guest publication.
///
/// # Safety
/// The memory and reclaim callbacks must remain valid for the whole call, including callback
/// reentry, and `out` must point to one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_particle_buffer_allocate(
    owner: u32,
    entry_count: i32,
    descriptor_kind: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    reclaim: Option<GamePoolReclaim>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32), Some(reclaim)) = (read32, write32, reclaim) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackReclaimHooks { context, reclaim };
    match tsfp_engine_memory::wrappers::allocate_particle_buffer(
        &mut memory,
        &mut hooks,
        tsfp_engine_memory::wrappers::ParticleBufferOwner::from_guest_address(owner),
        entry_count,
        descriptor_kind,
    ) {
        Ok(Some(allocation)) => {
            unsafe { *out = allocation.payload };
            0
        }
        Ok(None) => {
            unsafe { *out = 0 };
            0
        }
        Err(_) => 1,
    }
}

/// Run the readable Rust owner invalidation and nullable pool-free path for original `0x10C0E0`.
///
/// # Safety
/// The callbacks must remain valid for the whole call. The guest image must remain writable while
/// the owner record and optional pool payload are updated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_particle_buffer_release(
    owner: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = NoPoolHooks;
    match tsfp_engine_memory::wrappers::release_particle_buffer(
        &mut memory,
        &mut hooks,
        tsfp_engine_memory::wrappers::ParticleBufferOwner::from_guest_address(owner),
    ) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

#[cfg(test)]
mod particle_buffer_tests {
    use super::*;
    use std::collections::HashMap;

    struct Image {
        words: HashMap<u32, u32>,
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        image.words.get(&address).copied().unwrap_or(0)
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        image.words.insert(address, value);
        image.writes.push((address, value));
    }

    unsafe extern "C" fn reclaim(_context: *mut core::ffi::c_void, _callback: u32, _argument: u32) {
    }

    #[test]
    fn particle_buffer_c_abi_uses_captured_kind_and_publishes_count_first() {
        let owner = 0x7000;
        let descriptor = 0x7100;
        let base = 0x3000;
        let mut image = Image {
            words: HashMap::from([
                (0x006B_83BC, 0),
                (0x006B_83C0, 0x2000),
                (0x006B_83C4, 0x2000),
                (0x006B_83C8, base),
                (0x006B_83CC, base),
                (0x006B_83D0, base),
                (0x006B_83D4, base + 0x2000),
                (0x004C_81B8, 0x2000),
                (base, 0),
                (base + 4, 0x2000),
                (base + 8, 0),
                (owner + 0x0C, descriptor),
                (owner + 0x40, 0xDEAD_BEEF),
                (owner + 0x44, 99),
                // The adapter passes the kind read through its descriptor snapshot after the
                // retained release/count helpers. This deliberately differs from guest memory.
                (descriptor + 0x0C, 9),
            ]),
            writes: Vec::new(),
        };
        let mut result = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_particle_buffer_allocate(
                owner,
                2,
                6,
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                Some(reclaim),
                &mut result,
            )
        };

        assert_eq!(status, 0);
        assert_eq!(result, base + 0x20);
        // 0x3EBD0 rounds (0xB0 + 0x5F) down to a 0x40 boundary: 0x100.
        assert_eq!(image.words[&(base + 4)], 0x100);
        assert_eq!(image.words[&(base + 0x0C)], owner);
        assert_eq!(image.words[&(base + 0x10)], 0x10C0E0);
        assert_eq!(image.words[&(owner + 0x44)], 2);
        assert_eq!(image.words[&(owner + 0x40)], result);
        let publications: Vec<_> = image
            .writes
            .iter()
            .copied()
            .filter(|(address, _)| *address == owner + 0x44 || *address == owner + 0x40)
            .collect();
        assert_eq!(
            publications,
            vec![(owner + 0x44, 2), (owner + 0x40, result)]
        );
    }

    #[test]
    fn particle_buffer_c_abi_caps_count_and_publishes_null_on_exhaustion() {
        let owner = 0x7000;
        let descriptor = 0x7100;
        let base = 0x3000;
        let mut image = Image {
            words: HashMap::from([
                (0x006B_83BC, 0),
                (0x006B_83C0, 0x2000),
                (0x006B_83C4, 0x2000),
                (0x006B_83C8, base),
                (0x006B_83CC, base),
                (0x006B_83D0, base),
                (0x006B_83D4, base + 0x2000),
                (0x004C_81B8, 0x2000),
                (base, 0),
                (base + 4, 0x2000),
                (base + 8, 0),
                (owner + 0x0C, descriptor),
                (descriptor + 0x0C, 5),
            ]),
            writes: Vec::new(),
        };
        let mut result = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_particle_buffer_allocate(
                owner,
                900,
                5,
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                Some(reclaim),
                &mut result,
            )
        };

        assert_eq!(status, 0);
        assert_eq!(result, 0);
        assert_eq!(image.words[&(owner + 0x44)], 500);
        assert_eq!(image.words[&(owner + 0x40)], 0);
        let publications: Vec<_> = image
            .writes
            .iter()
            .copied()
            .filter(|(address, _)| *address == owner + 0x44 || *address == owner + 0x40)
            .collect();
        assert_eq!(publications, vec![(owner + 0x44, 500), (owner + 0x40, 0)]);
    }

    #[test]
    fn particle_buffer_release_c_abi_clears_null_payload_owner_in_order() {
        let owner = 0x7000;
        let mut image = Image {
            words: HashMap::from([
                (owner + 4, 1),
                (owner + 0x40, 0),
                (owner + 0x44, 7),
                (owner + 0x48, 8),
                (owner + 0x4C, 9),
                (owner + 0x50, 0xA5A5_A5A5),
            ]),
            writes: Vec::new(),
        };

        let status = unsafe {
            tsfp_kmem_game_particle_buffer_release(
                owner,
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
            )
        };

        assert_eq!(status, 0);
        assert_eq!(image.words[&(owner + 0x50)], 0xA5A5_A5A5);
        assert_eq!(
            image.writes,
            vec![
                (owner + 0x40, 0),
                (owner + 0x44, 0),
                (owner + 0x48, 0),
                (owner + 0x4C, 0),
            ]
        );
    }

    #[test]
    fn particle_buffer_release_c_abi_frees_nonnull_payload_before_owner_invalidation() {
        let owner = 0x7000;
        let base = 0x3000;
        let end = base + 0x100;
        let mut image = Image {
            words: HashMap::from([
                (0x006B_83C0, 0),
                (0x006B_83C4, 0x100),
                (0x006B_83C8, base),
                (0x006B_83CC, base),
                (0x006B_83D0, base),
                (0x006B_83D4, end),
                (base, 1),
                (base + 4, 0x100),
                (base + 8, 0),
                (owner + 4, 1),
                (owner + 0x40, base + 0x20),
                (owner + 0x44, 7),
                (owner + 0x48, 8),
                (owner + 0x4C, 9),
                (owner + 0x50, 0xA5A5_A5A5),
            ]),
            writes: Vec::new(),
        };

        let status = unsafe {
            tsfp_kmem_game_particle_buffer_release(
                owner,
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
            )
        };

        assert_eq!(status, 0);
        assert_eq!(image.words[&base], 0);
        assert_eq!(image.words[&0x006B_83C0], 0x100);
        assert_eq!(image.words[&(owner + 0x50)], 1);
        for offset in [0x40, 0x44, 0x48, 0x4C] {
            assert_eq!(image.words[&(owner + offset)], 0);
        }
        assert_eq!(
            image.writes,
            vec![
                (base, 0),
                (0x006B_83C0, 0x100),
                (owner + 0x50, 1),
                (owner + 0x40, 0),
                (owner + 0x44, 0),
                (owner + 0x48, 0),
                (owner + 0x4C, 0),
            ]
        );
    }
}

/// Run the readable Rust kind-1 pool free path (0x3EA40) against the host guest image.
/// The caller owns the opt-in/compare policy; this function performs only the allocator call.
///
/// # Safety
/// The callbacks must remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_pool_free(
    pointer: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = NoPoolHooks;
    let mut pool = Pool {
        memory: &mut memory,
        hooks: &mut hooks,
    };
    match pool.free(pointer) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the readable Rust pool retention-gate clear (0x3EA20) against guest state.
/// The call preserves the original EAX=0 result; the C adapter retains ECX's gate value.
///
/// # Safety
/// The callbacks must remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_pool_clear(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    match (MemoryServices {
        memory: &mut memory,
    })
    .clear_pool()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Reset the pool over the live game image, including a non-word-sized storage tail.
///
/// # Safety
/// All callbacks must remain valid for the complete reset operation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_pool_reset(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    read8: Option<GameRead8>,
    write8: Option<GameWrite8>,
) -> u32 {
    let (Some(read32), Some(write32), Some(read8), Some(write8)) = (read32, write32, read8, write8)
    else {
        return 1;
    };
    let mut memory = CallbackByteGameMemory {
        context,
        read32,
        write32,
        read8,
        write8,
    };
    match (MemoryServices {
        memory: &mut memory,
    })
    .reset_pool()
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the complete callback-free pool initialization (`0x3EA00`) against the guest image.
/// The allocation keeps its address in the arena globals, then the pool bytes and headers are
/// reset in place. Out-of-memory reporting is forwarded through the caller supplied hook.
///
/// # Safety
/// All callbacks must remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_pool_allocate_and_reset(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    read8: Option<GameRead8>,
    write8: Option<GameWrite8>,
    report: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32), Some(read8), Some(write8)) = (read32, write32, read8, write8)
    else {
        return 1;
    };
    let mut memory = CallbackByteGameMemory {
        context,
        read32,
        write32,
        read8,
        write8,
    };
    let mut hooks = CallbackPoolHooks { context, report };
    let mut pool = Pool {
        memory: &mut memory,
        hooks: &mut hooks,
    };
    match pool.allocate_and_reset() {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the readable `0x211040` point-table allocator, including its four guest RNG calls on
/// allocation success. Reclaim still dispatches the stored guest owner callback through the
/// supplied temporary bridge until S3 moves this owner state into Rust.
///
/// # Safety
/// The memory and reclaim callbacks must remain valid for the whole call, and `out` must point
/// to one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_alloc_random_point_pool(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    reclaim: Option<GamePoolReclaim>,
    random: Option<GamePoolRandom>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32), Some(reclaim), Some(random)) =
        (read32, write32, reclaim, random)
    else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackRandomPointHooks {
        context,
        read32,
        write32,
        reclaim,
        random,
    };
    let mut wrappers = tsfp_engine_memory::wrappers::AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    };
    match wrappers.alloc_random_point_pool_75c0cc() {
        Ok(count) => {
            unsafe { *out = count };
            0
        }
        Err(_) => 1,
    }
}

/// Allocate or reuse a primary-ledger block (0x3E440) in the caller's guest image.
///
/// # Safety
/// The callbacks must remain valid for this call, and `out` must point to one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_ledger_allocate(
    requested: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    out: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if out.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    match (Ledgers {
        memory: &mut memory,
    })
    .allocate_primary(requested)
    {
        Ok(block) => {
            unsafe { *out = block };
            0
        }
        Err(_) => 1,
    }
}

/// Run the readable 0x69EE0 ledger-buffer resolver against the existing guest image.
///
/// # Safety
/// The callbacks must remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_wrapper_ensure_ledger_buffer(
    force: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let mut hooks = CallbackArenaHooks {
        context,
        report: None,
    };
    match (AllocWrappers {
        memory: &mut memory,
        hooks: &mut hooks,
    })
    .ensure_ledger_buffer_6f1e14(force)
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Run the readable history-buffer reset (0x3E740) over guest memory.
///
/// The byte callbacks preserve the mode-zero gate read. `report` forwards OOM arguments for
/// 0x389B0; `callout` forwards the target VA from `InitHooks::callout`.
///
/// # Safety
/// All callbacks must remain valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_ledger_history_reset(
    mode: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    read8: Option<GameRead8>,
    write8: Option<GameWrite8>,
    report: Option<GameArenaReport>,
    callout: Option<GameArenaReport>,
) -> u32 {
    let (Some(read32), Some(write32), Some(read8), Some(write8)) = (read32, write32, read8, write8)
    else {
        return 1;
    };
    let mut memory = CallbackByteGameMemory {
        context,
        read32,
        write32,
        read8,
        write8,
    };
    let mut hooks = CallbackInitHooks {
        context,
        report,
        callout,
    };
    match (Ledgers {
        memory: &mut memory,
    })
    .history_reset(&mut hooks, mode)
    {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Release a primary-ledger block (0x3E4E0) and return the original EAX/ECX/EDX values.
/// The caller supplies the count sampled before reading the stack argument.
///
/// # Safety
/// The callbacks and output pointers must remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_ledger_release(
    count: u32,
    block: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    eax: *mut u32,
    ecx: *mut u32,
    edx: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if eax.is_null() || ecx.is_null() || edx.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let registers = match (Ledgers {
        memory: &mut memory,
    })
    .release_primary_with_count_and_registers(count, block)
    {
        Ok(registers) => registers,
        Err(_) => return 1,
    };
    unsafe {
        *eax = registers.eax;
        *ecx = registers.ecx;
        *edx = registers.edx;
    }
    0
}

/// Append a secondary-ledger row (0x3E560), reading the size argument after guest byte/base reads.
/// Outputs the original EAX/ECX/EDX values through the supplied pointers.
///
/// # Safety
/// The callbacks must remain valid for this call, and all output pointers must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_ledger_secondary_append(
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    read_argument: Option<GameReadArgument>,
    eax: *mut u32,
    ecx: *mut u32,
    edx: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32), Some(read_argument)) = (read32, write32, read_argument)
    else {
        return 1;
    };
    if eax.is_null() || ecx.is_null() || edx.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let registers = match (Ledgers {
        memory: &mut memory,
    })
    .append_secondary_with_argument_reader(|| unsafe { read_argument(context) })
    {
        Ok(registers) => registers,
        Err(_) => return 1,
    };
    unsafe { write_secondary_registers(registers, eax, ecx, edx) };
    0
}

/// Release a secondary-ledger row (0x3E5A0) using the count sampled before the stack argument.
///
/// # Safety
/// The callbacks and output pointers must remain valid for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_game_ledger_secondary_release(
    count: u32,
    block: u32,
    context: *mut core::ffi::c_void,
    read32: Option<GameRead32>,
    write32: Option<GameWrite32>,
    eax: *mut u32,
    ecx: *mut u32,
    edx: *mut u32,
) -> u32 {
    let (Some(read32), Some(write32)) = (read32, write32) else {
        return 1;
    };
    if eax.is_null() || ecx.is_null() || edx.is_null() {
        return 1;
    }
    let mut memory = CallbackGameMemory {
        context,
        read32,
        write32,
    };
    let registers = match (Ledgers {
        memory: &mut memory,
    })
    .release_secondary_with_count_and_registers(count, block)
    {
        Ok(registers) => registers,
        Err(_) => return 1,
    };
    unsafe { write_secondary_registers(registers, eax, ecx, edx) };
    0
}

unsafe fn write_secondary_registers(
    registers: SecondaryLedgerRegisters,
    eax: *mut u32,
    ecx: *mut u32,
    edx: *mut u32,
) {
    unsafe {
        *eax = registers.eax;
        *ecx = registers.ecx;
        *edx = registers.edx;
    }
}

struct CallbackArenaHooks {
    context: *mut core::ffi::c_void,
    report: Option<GameArenaReport>,
}

impl ArenaHooks for CallbackArenaHooks {
    fn out_of_memory(&mut self, argument: u32) {
        if let Some(report) = self.report {
            unsafe { report(self.context, argument) };
        }
    }
}

struct CallbackInitHooks {
    context: *mut core::ffi::c_void,
    report: Option<GameArenaReport>,
    callout: Option<GameArenaReport>,
}

impl ArenaHooks for CallbackInitHooks {
    fn out_of_memory(&mut self, argument: u32) {
        if let Some(report) = self.report {
            unsafe { report(self.context, argument) };
        }
    }
}

impl InitHooks for CallbackInitHooks {
    fn callout(&mut self, target: u32) {
        if let Some(callout) = self.callout {
            unsafe { callout(self.context, target) };
        }
    }
}

#[cfg(test)]
mod game_bump_tests {
    use super::*;

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<[u32; 2]>() };
        match address {
            0x6B839C => image[0],
            0x6B83B8 => image[1],
            _ => 0,
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<[u32; 2]>() };
        match address {
            0x6B839C => image[0] = value,
            0x6B83B8 => image[1] = value,
            _ => {}
        }
    }

    #[test]
    fn game_bump_c_abi_uses_guest_image_callbacks() {
        let mut image: [u32; 2] = [0x4000, 0];
        let mut block = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_bump_allocate(
                0x11,
                (&mut image as *mut [u32; 2]).cast(),
                Some(read32),
                Some(write32),
                &mut block,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(block, 0x4000);
        assert_eq!(image, [0x4020, 0x20]);
    }
}

#[cfg(test)]
mod game_arena_tests {
    use super::*;

    // Region A cursor/remaining, region B cursor, shared remaining, top, kind-4 cursor,
    // followed by the tagged block header at 0x8FC0.
    type ArenaImage = [u32; 11];

    #[repr(C)]
    struct PublishedArenaImage {
        cursor: u32,
        remaining: u32,
        published: u32,
        published_78b718: u32,
    }

    #[repr(C)]
    struct StateBlockImage {
        region_b_cursor: u32,
        main_remaining: u32,
        published: u32,
        descriptor_table: u32,
        descriptor_readiness: u32,
        descriptor_readiness_1b: u32,
        descriptor_readiness_19: u32,
        timer_descriptor_readiness: [u32; 5],
    }

    const STATE_BLOCK_TABLE: u32 = 0x00F0_0000;
    const STATE_BLOCK_READINESS: u32 = STATE_BLOCK_TABLE + 0x18 * 0x14 + 0x0C;
    const SOUND_DESCRIPTOR_1B_READY: u32 = STATE_BLOCK_TABLE + 0x1B * 0x14 + 0x0C;
    const SOUND_DESCRIPTOR_19_READY: u32 = STATE_BLOCK_TABLE + 0x19 * 0x14 + 0x0C;
    const TIMER_DESCRIPTOR_READY: [u32; 5] = [
        STATE_BLOCK_TABLE + 0x156F * 0x14 + 0x0C,
        STATE_BLOCK_TABLE + 0x1570 * 0x14 + 0x0C,
        STATE_BLOCK_TABLE + 0x5F3 * 0x14 + 0x0C,
        STATE_BLOCK_TABLE + 0x5F4 * 0x14 + 0x0C,
        STATE_BLOCK_TABLE + 0x5F5 * 0x14 + 0x0C,
    ];

    #[repr(C)]
    struct BufferWrapperImage {
        region_a_cursor: u32,
        region_a_remaining: u32,
        published_first: u32,
        published_second: u32,
        second_size: u32,
    }

    struct SaveSlotWrapperImage {
        region_b_cursor: u32,
        shared_remaining: u32,
        mode: u32,
        published: u32,
        payload: Vec<u8>,
        writes: Vec<(u32, u32)>,
    }

    struct MapEditBufferImage {
        region_a_cursor: u32,
        region_a_remaining: u32,
        published: u32,
        undo_buffer: u32,
        undo_head: u32,
        redo_head: u32,
        undo_used: u32,
        payload: Vec<u8>,
    }

    const SAVE_SLOT_PAYLOAD_BASE: u32 = 0x0100_0000;
    const MAP_EDIT_PAYLOAD_BASE: u32 = 0x0100_0000;

    unsafe extern "C" fn read_map_edit_image(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<MapEditBufferImage>() };
        match address {
            0x006B_8370 => image.region_a_cursor,
            0x006B_8378 => image.region_a_remaining,
            0x0078_4014 => image.published,
            0x0076_5C24 => image.undo_buffer,
            0x0076_5C28 => image.undo_head,
            0x0076_5C30 => image.redo_head,
            0x0076_5C20 => image.undo_used,
            _ if address >= MAP_EDIT_PAYLOAD_BASE => {
                let offset = (address - MAP_EDIT_PAYLOAD_BASE) as usize;
                let Some(bytes) = image.payload.get(offset..offset.saturating_add(4)) else {
                    return 0;
                };
                u32::from_le_bytes(bytes.try_into().unwrap())
            }
            _ => 0,
        }
    }

    unsafe extern "C" fn write_map_edit_image(
        context: *mut core::ffi::c_void,
        address: u32,
        value: u32,
    ) {
        let image = unsafe { &mut *context.cast::<MapEditBufferImage>() };
        match address {
            0x006B_8370 => image.region_a_cursor = value,
            0x006B_8378 => image.region_a_remaining = value,
            0x0078_4014 => image.published = value,
            0x0076_5C24 => image.undo_buffer = value,
            0x0076_5C28 => image.undo_head = value,
            0x0076_5C30 => image.redo_head = value,
            0x0076_5C20 => image.undo_used = value,
            _ if address >= MAP_EDIT_PAYLOAD_BASE => {
                let offset = (address - MAP_EDIT_PAYLOAD_BASE) as usize;
                if let Some(bytes) = image.payload.get_mut(offset..offset.saturating_add(4)) {
                    bytes.copy_from_slice(&value.to_le_bytes());
                }
            }
            _ => {}
        }
    }

    unsafe extern "C" fn read_save_slot_image(
        context: *mut core::ffi::c_void,
        address: u32,
    ) -> u32 {
        let image = unsafe { &*context.cast::<SaveSlotWrapperImage>() };
        match address {
            0x006B_837C => image.region_b_cursor,
            0x006B_8384 => image.shared_remaining,
            0x0079_094C => image.mode,
            0x0077_8D38 => image.published,
            _ if address >= SAVE_SLOT_PAYLOAD_BASE => {
                let offset = (address - SAVE_SLOT_PAYLOAD_BASE) as usize;
                let Some(bytes) = image.payload.get(offset..offset.saturating_add(4)) else {
                    return 0;
                };
                u32::from_le_bytes(bytes.try_into().unwrap())
            }
            _ => 0,
        }
    }

    unsafe extern "C" fn write_save_slot_image(
        context: *mut core::ffi::c_void,
        address: u32,
        value: u32,
    ) {
        let image = unsafe { &mut *context.cast::<SaveSlotWrapperImage>() };
        image.writes.push((address, value));
        match address {
            0x006B_837C => image.region_b_cursor = value,
            0x006B_8384 => image.shared_remaining = value,
            0x0077_8D38 => image.published = value,
            _ if address >= SAVE_SLOT_PAYLOAD_BASE => {
                let offset = (address - SAVE_SLOT_PAYLOAD_BASE) as usize;
                if let Some(bytes) = image.payload.get_mut(offset..offset.saturating_add(4)) {
                    bytes.copy_from_slice(&value.to_le_bytes());
                }
            }
            _ => {}
        }
    }

    unsafe extern "C" fn read_buffer_wrapper_image(
        context: *mut core::ffi::c_void,
        address: u32,
    ) -> u32 {
        let image = unsafe { &*context.cast::<BufferWrapperImage>() };
        match address {
            0x006B_8370 => image.region_a_cursor,
            0x006B_8378 => image.region_a_remaining,
            0x0054_EF14 => image.published_first,
            0x0054_EF18 => image.published_second,
            0x0054_EF20 => image.second_size,
            _ => 0,
        }
    }

    unsafe extern "C" fn write_buffer_wrapper_image(
        context: *mut core::ffi::c_void,
        address: u32,
        value: u32,
    ) {
        let image = unsafe { &mut *context.cast::<BufferWrapperImage>() };
        match address {
            0x006B_8370 => image.region_a_cursor = value,
            0x006B_8378 => image.region_a_remaining = value,
            0x0054_EF14 => image.published_first = value,
            0x0054_EF18 => image.published_second = value,
            0x0054_EF20 => image.second_size = value,
            _ => {}
        }
    }

    unsafe extern "C" fn read_published_image(
        context: *mut core::ffi::c_void,
        address: u32,
    ) -> u32 {
        let image = unsafe { &*context.cast::<PublishedArenaImage>() };
        match address {
            0x006B_837C => image.cursor,
            0x006B_8384 => image.remaining,
            0x0075_B4B4 => image.published,
            0x0078_B718 => image.published_78b718,
            _ => 0,
        }
    }

    unsafe extern "C" fn write_published_image(
        context: *mut core::ffi::c_void,
        address: u32,
        value: u32,
    ) {
        let image = unsafe { &mut *context.cast::<PublishedArenaImage>() };
        match address {
            0x006B_837C => image.cursor = value,
            0x006B_8384 => image.remaining = value,
            0x0075_B4B4 => image.published = value,
            0x0078_B718 => image.published_78b718 = value,
            _ => {}
        }
    }

    unsafe extern "C" fn read_state_block_image(
        context: *mut core::ffi::c_void,
        address: u32,
    ) -> u32 {
        let image = unsafe { &*context.cast::<StateBlockImage>() };
        if let Some(index) = TIMER_DESCRIPTOR_READY
            .iter()
            .position(|&field| field == address)
        {
            return image.timer_descriptor_readiness[index];
        }
        match address {
            0x006B_837C => image.region_b_cursor,
            0x006B_8384 => image.main_remaining,
            0x0075_C108 => image.published,
            0x0075_C118 => image.published,
            0x0075_C124 => image.published,
            0x004C_A9B4 => image.descriptor_table,
            STATE_BLOCK_READINESS => image.descriptor_readiness,
            SOUND_DESCRIPTOR_1B_READY => image.descriptor_readiness_1b,
            SOUND_DESCRIPTOR_19_READY => image.descriptor_readiness_19,
            _ => 0,
        }
    }

    unsafe extern "C" fn write_state_block_image(
        context: *mut core::ffi::c_void,
        address: u32,
        value: u32,
    ) {
        let image = unsafe { &mut *context.cast::<StateBlockImage>() };
        if let Some(index) = TIMER_DESCRIPTOR_READY
            .iter()
            .position(|&field| field == address)
        {
            image.timer_descriptor_readiness[index] = value;
            return;
        }
        match address {
            0x006B_837C => image.region_b_cursor = value,
            0x006B_8384 => image.main_remaining = value,
            0x0075_C108 => image.published = value,
            0x0075_C118 => image.published = value,
            0x0075_C124 => image.published = value,
            STATE_BLOCK_READINESS => image.descriptor_readiness = value,
            SOUND_DESCRIPTOR_1B_READY => image.descriptor_readiness_1b = value,
            SOUND_DESCRIPTOR_19_READY => image.descriptor_readiness_19 = value,
            _ => {}
        }
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<ArenaImage>() };
        match address {
            0x006B_8370 => image[0],
            0x006B_8378 => image[1],
            0x006B_837C => image[2],
            0x006B_8384 => image[3],
            0x006B_8388 => image[4],
            0x006B_8380 => image[10],
            0x006B_8390 => image[5],
            0x0000_8FC0 => image[6],
            0x0000_8FC4 => image[7],
            0x0000_8FC8 => image[8],
            0x0000_8FCC => image[9],
            _ => 0,
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<ArenaImage>() };
        match address {
            0x006B_8370 => image[0] = value,
            0x006B_8378 => image[1] = value,
            0x006B_837C => image[2] = value,
            0x006B_8384 => image[3] = value,
            0x006B_8388 => image[4] = value,
            0x006B_8380 => image[10] = value,
            0x006B_8390 => image[5] = value,
            0x0000_8FC0 => image[6] = value,
            0x0000_8FC4 => image[7] = value,
            0x0000_8FC8 => image[8] = value,
            0x0000_8FCC => image[9] = value,
            _ => {}
        }
    }

    #[test]
    fn game_arena_c_abi_uses_guest_image_callbacks() {
        let mut image: ArenaImage = [0, 0x400, 0, 0x1000, 0x9000, 0x5000, 0, 0, 0, 0, 0];
        let mut block = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_arena_allocate(
                0,
                0x30,
                (&mut image as *mut ArenaImage).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut block,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(block, 0x8FD0);
        assert_eq!(image[3], 0xFC0);
        assert_eq!(image[4], 0x8FC0);
        assert_eq!(&image[6..10], &[0x4143_4F4C, 0x4D45_4D4C, 0x30, 0x8FC0]);
    }

    #[test]
    fn game_arena_aligned_c_abi_preserves_guest_cursor_and_address() {
        let mut image: ArenaImage = [0x1011, 0x1000, 0, 0x1000, 0x9000, 0x5000, 0, 0, 0, 0, 0];
        let mut block = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_arena_allocate_aligned(
                32,
                0x30,
                2,
                (&mut image as *mut ArenaImage).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut block,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(block, 0x1020);
        assert_eq!(image[0], 0x1050);
        assert_eq!(image[1], 0xFC1);
    }

    #[test]
    fn game_wrapper_220e20_c_abi_allocates_and_resets_descriptor() {
        let mut image = StateBlockImage {
            region_b_cursor: 0x8000,
            main_remaining: 0x1000,
            published: 0,
            descriptor_table: STATE_BLOCK_TABLE,
            descriptor_readiness: 0xA5A5_A5A5,
            descriptor_readiness_1b: 0xB1B1_B1B1,
            descriptor_readiness_19: 0x1919_1919,
            timer_descriptor_readiness: [0xD1D1_D1D1; 5],
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_state_block_75c108(
                (&mut image as *mut StateBlockImage).cast(),
                Some(read_state_block_image),
                Some(write_state_block_image),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published, 0x8000);
        assert_eq!(image.region_b_cursor, 0x8010);
        assert_eq!(image.main_remaining, 0x0FF0);
        assert_eq!(image.descriptor_readiness, u32::MAX);
    }

    #[test]
    fn game_wrapper_2259d0_c_abi_allocates_and_resets_sound_descriptors() {
        let mut image = StateBlockImage {
            region_b_cursor: 0x8000,
            main_remaining: 0x1000,
            published: 0,
            descriptor_table: STATE_BLOCK_TABLE,
            descriptor_readiness: 0xA5A5_A5A5,
            descriptor_readiness_1b: 0xB1B1_B1B1,
            descriptor_readiness_19: 0x1919_1919,
            timer_descriptor_readiness: [0xD1D1_D1D1; 5],
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_sound_state_75c124(
                (&mut image as *mut StateBlockImage).cast(),
                Some(read_state_block_image),
                Some(write_state_block_image),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published, 0x8000);
        assert_eq!(image.region_b_cursor, 0x8010);
        assert_eq!(image.main_remaining, 0x0FF0);
        assert_eq!(image.descriptor_readiness_1b, u32::MAX);
        assert_eq!(image.descriptor_readiness_19, u32::MAX);
        assert_eq!(image.descriptor_readiness, 0xA5A5_A5A5);
    }

    #[test]
    fn game_wrapper_2226f0_c_abi_allocates_and_resets_timer_descriptors() {
        let mut image = StateBlockImage {
            region_b_cursor: 0x8000,
            main_remaining: 0x1000,
            published: 0,
            descriptor_table: STATE_BLOCK_TABLE,
            descriptor_readiness: 0xA5A5_A5A5,
            descriptor_readiness_1b: 0xB1B1_B1B1,
            descriptor_readiness_19: 0x1919_1919,
            timer_descriptor_readiness: [0xD1D1_D1D1; 5],
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_timer_state_75c118(
                (&mut image as *mut StateBlockImage).cast(),
                Some(read_state_block_image),
                Some(write_state_block_image),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published, 0x8000);
        assert_eq!(image.region_b_cursor, 0x8010);
        assert_eq!(image.main_remaining, 0x0FF0);
        assert_eq!(image.timer_descriptor_readiness, [u32::MAX; 5]);
        assert_eq!(image.descriptor_readiness, 0xA5A5_A5A5);
        assert_eq!(image.descriptor_readiness_1b, 0xB1B1_B1B1);
        assert_eq!(image.descriptor_readiness_19, 0x1919_1919);
    }

    #[test]
    fn game_tagged_release_c_abi_preserves_address_and_register_effects() {
        let mut image: ArenaImage = [
            0,
            0x400,
            0,
            0xFC0,
            0x8FC0,
            0x5000,
            0x4143_4F4C,
            0x4D45_4D4C,
            0x30,
            0x8FC0,
            0,
        ];
        let mut registers = GameArenaReleaseRegisters::default();
        let status = unsafe {
            tsfp_kmem_game_arena_release_tagged(
                0x8FD0,
                (&mut image as *mut ArenaImage).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut registers,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(
            registers,
            GameArenaReleaseRegisters {
                eax: 0x1000,
                ecx: 0x30,
                edx: 0xFC0
            }
        );
        assert_eq!(image[3], 0x1000);
        assert_eq!(image[4], 0x9000);
        assert_eq!(&image[6..10], &[0x4143_4F4C, 0x4D45_4D4C, 0x30, 0x8FC0]);
    }

    #[test]
    fn game_arena_rewind_c_abi_restores_guest_top_and_remaining_bytes() {
        let mut image: ArenaImage = [0, 0x400, 0, 0xFC0, 0x8FC0, 0x5000, 0, 0, 0, 0, 0x9000];
        let status = unsafe {
            tsfp_kmem_game_arena_rewind_top(
                (&mut image as *mut ArenaImage).cast(),
                Some(read32),
                Some(write32),
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image[3], 0x1000);
        assert_eq!(image[4], 0x9000);
    }

    #[test]
    fn game_wrapper_1d1e80_c_abi_publishes_aligned_guest_address() {
        let mut image = PublishedArenaImage {
            cursor: 0x100,
            remaining: 0x2000,
            published: 0xDEAD_BEEF,
            published_78b718: 0xDEAD_BEEF,
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_aligned_75b4b4(
                (&mut image as *mut PublishedArenaImage).cast(),
                Some(read_published_image),
                Some(write_published_image),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published, 0x100);
        assert_eq!(image.cursor, 0x1690);
        assert_eq!(image.remaining, 0xA70);
    }

    #[test]
    fn game_wrapper_24a180_c_abi_allocates_and_publishes_guest_address() {
        let mut image = PublishedArenaImage {
            cursor: 0x100,
            remaining: 0x2000,
            published: 0xDEAD_BEEF,
            published_78b718: 0xDEAD_BEEF,
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_model_buffer_78b718(
                (&mut image as *mut PublishedArenaImage).cast(),
                Some(read_published_image),
                Some(write_published_image),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published_78b718, 0x100);
        assert_eq!(image.cursor, 0x740);
        assert_eq!(image.remaining, 0x19C0);
    }

    #[test]
    fn game_wrapper_18ca0_c_abi_allocates_and_publishes_both_guest_buffers() {
        let mut image = BufferWrapperImage {
            region_a_cursor: 0x0100_0000,
            region_a_remaining: 0x0004_0000,
            published_first: 0xDEAD_BEEF,
            published_second: 0xDEAD_BEEF,
            second_size: 0xDEAD_BEEF,
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_buffers_54ef14(
                (&mut image as *mut BufferWrapperImage).cast(),
                Some(read_buffer_wrapper_image),
                Some(write_buffer_wrapper_image),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published_first, 0x0100_0000);
        assert_eq!(image.published_second, 0x0101_A000);
        assert_eq!(image.second_size, 0x800);
        assert_eq!(image.region_a_cursor, 0x0101_A800);
        assert_eq!(image.region_a_remaining, 0x0002_5800);
    }

    #[test]
    fn game_wrapper_32a860_c_abi_allocates_and_clears_guest_buffer_for_both_modes() {
        for mode in [0x66, 0x67] {
            let allocation_size = if mode == 0x66 { 0x191530 } else { 0x1530 };
            let mut image = SaveSlotWrapperImage {
                region_b_cursor: SAVE_SLOT_PAYLOAD_BASE,
                shared_remaining: 0x20_0000,
                mode,
                published: u32::MAX,
                payload: vec![0xAA; 0x191530 + 0x20],
                writes: Vec::new(),
            };
            let status = unsafe {
                tsfp_kmem_game_wrapper_alloc_save_slot_778d38(
                    (&mut image as *mut SaveSlotWrapperImage).cast(),
                    Some(read_save_slot_image),
                    Some(write_save_slot_image),
                    None,
                )
            };
            assert_eq!(status, 0);
            assert_eq!(image.published, SAVE_SLOT_PAYLOAD_BASE);
            assert_eq!(
                image.region_b_cursor,
                SAVE_SLOT_PAYLOAD_BASE + allocation_size
            );
            assert_eq!(image.shared_remaining, 0x20_0000 - allocation_size);
            assert!(image.payload[..0x1530].iter().all(|&byte| byte == 0));
            assert!(
                image.payload[0x1530..0x1550]
                    .iter()
                    .all(|&byte| byte == 0xAA)
            );
            assert_eq!(
                image.writes[image.writes.len() - 2],
                (SAVE_SLOT_PAYLOAD_BASE + 4, 0)
            );
            assert_eq!(
                image.writes[image.writes.len() - 1],
                (SAVE_SLOT_PAYLOAD_BASE + 8, 0)
            );
        }
    }

    #[test]
    fn game_wrapper_2fe200_c_abi_allocates_and_clears_both_guest_blocks() {
        let mut image = MapEditBufferImage {
            region_a_cursor: MAP_EDIT_PAYLOAD_BASE,
            region_a_remaining: 0x20_0000,
            published: u32::MAX,
            undo_buffer: u32::MAX,
            undo_head: u32::MAX,
            redo_head: u32::MAX,
            undo_used: u32::MAX,
            payload: vec![0xAA; 0x18000],
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_mapedit_buffers(
                (&mut image as *mut MapEditBufferImage).cast(),
                Some(read_map_edit_image),
                Some(write_map_edit_image),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published, MAP_EDIT_PAYLOAD_BASE);
        assert_eq!(image.undo_buffer, MAP_EDIT_PAYLOAD_BASE + 0x8000);
        assert_eq!(image.undo_head, 0);
        assert_eq!(image.redo_head, 0);
        assert_eq!(image.undo_used, 0);
        assert_eq!(image.region_a_cursor, MAP_EDIT_PAYLOAD_BASE + 0x18000);
        assert_eq!(image.region_a_remaining, 0x20_0000 - 0x18000);
        assert!(image.payload.iter().all(|&byte| byte == 0));
    }
}

#[cfg(test)]
mod game_effect_slot_wrapper_tests {
    use super::*;

    #[derive(Default)]
    struct Image {
        region_b_cursor: u32,
        main_remaining: u32,
        published: u32,
        story_event_pool: u32,
        story_event_indices: u32,
        story_event_snapshot: u32,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        match address {
            0x6B83_7C => image.region_b_cursor,
            0x6B83_84 => image.main_remaining,
            0x7A15_6C => image.published,
            0x78CD_90 => image.story_event_pool,
            0x78CD_8C => image.story_event_indices,
            0x75CC_A4 => image.story_event_snapshot,
            _ => 0,
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        match address {
            0x6B83_7C => image.region_b_cursor = value,
            0x6B83_84 => image.main_remaining = value,
            0x7A15_6C => image.published = value,
            0x78CD_90 => image.story_event_pool = value,
            0x78CD_8C => image.story_event_indices = value,
            0x75CC_A4 => image.story_event_snapshot = value,
            _ => {}
        }
    }

    #[test]
    fn game_wrapper_188900_c_abi_allocates_and_publishes_guest_address() {
        let mut image = Image {
            region_b_cursor: 0x1000,
            main_remaining: 0x2000,
            published: 0xDEAD_BEEF,
            story_event_pool: 0,
            story_event_indices: 0,
            story_event_snapshot: 0,
        };
        let mut block = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_effect_slots_7a156c(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut block,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(block, 0x1000);
        assert_eq!(image.published, block);
        assert_eq!(image.region_b_cursor, 0x11A0);
        assert_eq!(image.main_remaining, 0x1E60);
    }

    #[test]
    fn game_wrapper_237480_c_abi_allocates_and_publishes_guest_address() {
        let mut image = Image {
            region_b_cursor: 0x2000,
            main_remaining: 0x1000,
            story_event_pool: 0xDEAD_BEEF,
            ..Image::default()
        };
        let mut block = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_story_event_instances_78cd90(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
                0x48,
                &mut block,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(block, 0x2000);
        assert_eq!(image.story_event_pool, block);
        assert_eq!(image.region_b_cursor, 0x2050);
        assert_eq!(image.main_remaining, 0xFB0);
    }

    #[test]
    fn game_wrapper_237480_c_abi_allocates_and_publishes_support_arrays_in_order() {
        let mut image = Image {
            region_b_cursor: 0x2000,
            main_remaining: 0x1000,
            story_event_indices: 0xDEAD_BEEF,
            story_event_snapshot: 0xBAD0_CAFE,
            ..Image::default()
        };
        let mut indices = u32::MAX;
        let mut snapshot = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_story_event_support_arrays(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
                0x14,
                0x44,
                &mut indices,
                &mut snapshot,
            )
        };
        assert_eq!(status, 0);
        assert_eq!((indices, snapshot), (0x2000, 0x2020));
        assert_eq!(
            (image.story_event_indices, image.story_event_snapshot),
            (indices, snapshot)
        );
        assert_eq!(image.region_b_cursor, 0x2070);
        assert_eq!(image.main_remaining, 0xF90);
    }
}

#[cfg(test)]
mod game_side_table_wrapper_tests {
    use super::*;

    #[derive(Default)]
    struct Image {
        region_b_cursor: u32,
        main_remaining: u32,
        table_fixed: u32,
        table_counted: u32,
        mode: u32,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        match address {
            0x6B83_7C => image.region_b_cursor,
            0x6B83_84 => image.main_remaining,
            0x74C3_B8 => image.table_fixed,
            0x74C3_BC => image.table_counted,
            0x7DE4_00 => image.mode,
            _ => 0,
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        match address {
            0x6B83_7C => image.region_b_cursor = value,
            0x6B83_84 => image.main_remaining = value,
            0x74C3_B8 => image.table_fixed = value,
            0x74C3_BC => image.table_counted = value,
            _ => {}
        }
    }

    #[test]
    fn game_wrapper_274650_c_abi_allocates_both_guest_tables_in_order() {
        let mut image = Image {
            region_b_cursor: 0x1000,
            main_remaining: 0x2000,
            mode: 5 << 16,
            ..Image::default()
        };
        let mut eax = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_tables_74c3b8(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
                &mut eax,
            )
        };

        assert_eq!(status, 0);
        assert_eq!(image.table_fixed, 0x1000);
        assert_eq!(image.table_counted, 0x1100);
        assert_eq!(eax, image.table_counted);
        assert_eq!(image.region_b_cursor, 0x1140);
        assert_eq!(image.main_remaining, 0x1EC0);
    }
}

#[cfg(test)]
mod game_pool_tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    type GameImage = HashMap<u32, u32>;
    static RECLAIMS: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

    struct PointPoolTestImage {
        image: GameImage,
        random: [u32; 4],
        random_index: usize,
    }

    unsafe extern "C" fn point_pool_read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let context = unsafe { &*context.cast::<PointPoolTestImage>() };
        context.image.get(&address).copied().unwrap_or(0)
    }

    unsafe extern "C" fn point_pool_write32(
        context: *mut core::ffi::c_void,
        address: u32,
        value: u32,
    ) {
        let context = unsafe { &mut *context.cast::<PointPoolTestImage>() };
        context.image.insert(address, value);
    }

    unsafe extern "C" fn point_pool_random(context: *mut core::ffi::c_void) -> u32 {
        let context = unsafe { &mut *context.cast::<PointPoolTestImage>() };
        let value = context.random[context.random_index];
        context.random_index += 1;
        value
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<GameImage>() };
        image.get(&address).copied().unwrap_or(0)
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<GameImage>() };
        image.insert(address, value);
    }

    unsafe extern "C" fn read_argument(context: *mut core::ffi::c_void) -> u32 {
        let image = unsafe { &*context.cast::<GameImage>() };
        image.get(&0xFFFF_0000).copied().unwrap_or(0)
    }

    unsafe extern "C" fn record_reclaim(
        _context: *mut core::ffi::c_void,
        callback: u32,
        argument: u32,
    ) {
        RECLAIMS.lock().unwrap().push((callback, argument));
    }

    unsafe extern "C" fn read8(context: *mut core::ffi::c_void, address: u32) -> u8 {
        let image = unsafe { &*context.cast::<GameImage>() };
        let word = image.get(&(address & !3)).copied().unwrap_or(0);
        (word >> ((address & 3) * 8)) as u8
    }

    unsafe extern "C" fn write8(context: *mut core::ffi::c_void, address: u32, value: u8) {
        let image = unsafe { &mut *context.cast::<GameImage>() };
        let key = address & !3;
        let shift = (address & 3) * 8;
        let word = image.get(&key).copied().unwrap_or(0);
        image.insert(key, (word & !(0xFF << shift)) | (u32::from(value) << shift));
    }

    fn pool_image(header_size: u32) -> GameImage {
        HashMap::from([
            (0x006B_83CC, 0x1000), // roving cursor
            (0x006B_83D0, 0x2000), // first-block prefix
            (0x006B_83C0, 0x500),  // free bytes
            (0x006B_83C4, 0x800),  // capacity
            (0x1000, 1),
            (0x1004, header_size),
            (0x1008, 0x1000),
            (0x100C, 0),
            (0x1010, 0),
        ])
    }

    #[test]
    fn game_pool_initialize_c_abi_allocates_aligned_guest_storage_and_resets_tail() {
        let mut image: GameImage = HashMap::from([
            (0x004C_81B8, 21),
            (0x006B_837C, 0x2003),
            (0x006B_8384, 0x1000),
            (0x006B_8370, 0x1800),
            (0x006B_8378, 0x800),
            (0x006B_8388, 0x3000),
            (0x006B_8390, 0x4000),
            (0x2040, 0xA5A5_A5A5),
            (0x2044, 0xA5A5_A5A5),
            (0x2048, 0xA5A5_A5A5),
            (0x204C, 0xA5A5_A5A5),
            (0x2050, 0xA5A5_A5A5),
            (0x2054, 0xA5A5_A5A5),
            (0x2058, 0xA5A5_A5A5),
            (0x205C, 0xA5A5_A5A5),
        ]);
        let status = unsafe {
            tsfp_kmem_game_pool_allocate_and_reset(
                (&mut image as *mut GameImage).cast(),
                Some(read32),
                Some(write32),
                Some(read8),
                Some(write8),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image[&0x006B_83C8], 0x2040);
        assert_eq!(image[&0x006B_837C], 0x2060);
        assert_eq!(image[&0x006B_8384], 0x0FA3);
        assert_eq!(image[&0x006B_83CC], 0x2040);
        assert_eq!(image[&0x006B_83D0], 0x2040);
        assert_eq!(image[&0x006B_83C0], 21);
        assert_eq!(image[&0x006B_83C4], 21);
        assert_eq!(image[&0x006B_83D4], 0x2055);
        assert_eq!(image[&0x006B_83BC], 0);
        assert_eq!(image[&0x2040], 0);
        assert_eq!(image[&0x2044], 21);
        assert_eq!(image[&0x2048], 0);
        assert_eq!(image[&0x204C], 0);
        assert_eq!(image[&0x2050], 0);
        assert_eq!(
            unsafe { read8((&mut image as *mut GameImage).cast(), 0x2054) },
            0
        );
        assert_eq!(
            unsafe { read8((&mut image as *mut GameImage).cast(), 0x2055) },
            0xA5
        );
    }

    #[test]
    fn game_locked_pool_c_abi_updates_the_guest_image_in_place() {
        let mut image = pool_image(0x400);
        let mut result = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_pool_locked_allocate(
                1,
                (&mut image as *mut GameImage).cast(),
                Some(read32),
                Some(write32),
                &mut result,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(result, 0x1020);
        assert_eq!(image[&0x1000], 2);
        assert_eq!(image[&0x1004], 0x40);
        assert_eq!(image[&0x006B_83D0], 0x2040);
        assert_eq!(image[&0x006B_83C0], 0x4C0);
        assert_eq!(image[&0x006B_83C4], 0x7C0);
        assert_eq!(image[&0x006B_83CC], 0x1040);
        assert_eq!(image[&0x1044], 0x3C0);
    }

    #[test]
    fn game_locked_pool_c_abi_keeps_guest_state_on_capacity_failure() {
        let mut image = pool_image(0x40);
        let before = image.clone();
        let mut result = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_pool_locked_allocate(
                0x40,
                (&mut image as *mut GameImage).cast(),
                Some(read32),
                Some(write32),
                &mut result,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(result, 0);
        assert_eq!(image, before);
    }

    #[test]
    fn game_pool_by_kind_c_abi_reclaims_through_the_supplied_owner_callback() {
        let mut image: GameImage = HashMap::from([
            (0x006B_83BC, 0),
            (0x006B_83C0, 0x40),
            (0x006B_83C4, 0x300),
            (0x006B_83CC, 0x1100),
            (0x006B_83D0, 0x1000),
            (0x006B_83D4, 0x1300),
            (0x1000, 1),
            (0x1004, 0x100),
            (0x1100, 0),
            (0x1104, 0x40),
            (0x1108, 0x100),
            (0x1140, 1),
            (0x1144, 0xC0),
            (0x1148, 0x40),
            (0x114C, 0x3000),
            (0x1150, 0x10C0E0),
            (0x1200, 1),
            (0x1204, 0x100),
            (0x1208, 0xC0),
        ]);
        RECLAIMS.lock().unwrap().clear();
        let context = (&mut image as *mut GameImage).cast();
        let mut result = 0;
        let status = unsafe {
            tsfp_kmem_game_pool_allocate_by_kind(
                0xA0,
                1,
                0x3000,
                0x10C0E0,
                context,
                Some(read32),
                Some(write32),
                Some(record_reclaim),
                &mut result,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(result, 0x1120);
        assert_eq!(*RECLAIMS.lock().unwrap(), vec![(0x10C0E0, 0x3000)]);
        assert_eq!(image[&0x006B_83BC], 1);
        assert_eq!(image[&0x1100], 1);
        assert_eq!(image[&0x1104], 0xC0);
        assert_eq!(image[&0x006B_83C0], 0x40);
        assert_eq!(image[&0x006B_83CC], 0x11C0);
    }

    #[test]
    fn random_point_pool_wrapper_c_abi_allocates_and_fills_one_guest_point() {
        let mut context = PointPoolTestImage {
            image: HashMap::from([
                (0x006B_83BC, 0),
                (0x006B_83C0, 0x2000),
                (0x006B_83C4, 0x2000),
                (0x006B_83C8, 0x1000),
                (0x006B_83CC, 0x1000),
                (0x006B_83D0, 0x1000),
                (0x006B_83D4, 0x3000),
                (0x1000, 0),
                (0x1004, 0x2000),
                (0x1008, 0),
                (0x100C, 0),
                (0x1010, 0),
                (0x75C0A8, 0xA5A5_A5A5),
                (0x75C0CC, 0x5A5A_5A5A),
                (0x47862C, 10.0f32.to_bits()),
                (0x4785D0, 1.0f32.to_bits()),
                (0x475CD4, 2.0f32.to_bits()),
                (0x475D40, 3.0f32.to_bits()),
                (0x475DB4, 4.0f32.to_bits()),
            ]),
            random: [2, 4, 6, 8],
            random_index: 0,
        };
        let context_ptr = (&mut context as *mut PointPoolTestImage).cast();
        let mut result = 0;
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_random_point_pool(
                context_ptr,
                Some(point_pool_read32),
                Some(point_pool_write32),
                Some(record_reclaim),
                Some(point_pool_random),
                &mut result,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(result, 1);
        assert_eq!(context.random_index, 4);
        assert_eq!(context.image[&0x75C0CC], 0x1020);
        assert_eq!(context.image[&0x75C0A8], 1);
        assert_eq!(f32::from_bits(context.image[&0x1020]), -8.0);
        assert_eq!(f32::from_bits(context.image[&0x1024]), -6.0);
        assert_eq!(f32::from_bits(context.image[&0x1028]), 6.0);
        assert_eq!(f32::from_bits(context.image[&0x102C]), 76.0);
        assert_eq!(context.image[&0x1030], 1);
    }

    #[test]
    fn random_point_pool_native_reclaim_clears_owner_fields_without_guest_dispatch() {
        let mut context = PointPoolTestImage {
            image: HashMap::from([(0x75C0A8, 0xA5A5_A5A5), (0x75C0CC, 0x5A5A_5A5A)]),
            random: [0; 4],
            random_index: 0,
        };
        let context_ptr = (&mut context as *mut PointPoolTestImage).cast();
        let mut hooks = CallbackRandomPointHooks {
            context: context_ptr,
            read32: point_pool_read32,
            write32: point_pool_write32,
            reclaim: record_reclaim,
            random: point_pool_random,
        };

        hooks.reclaim(0x0021_0FF0, 0x75C0CC);

        assert_eq!(context.image[&0x75C0A8], 0);
        assert_eq!(context.image[&0x75C0CC], 0);
        assert!(!RECLAIMS.lock().unwrap().contains(&(0x0021_0FF0, 0x75C0CC)));
    }

    #[test]
    fn random_point_pool_native_reclaim_clears_owner_fields() {
        let mut context = PointPoolTestImage {
            image: HashMap::from([(0x75C0A8, 0xA5A5_A5A5), (0x75C0CC, 0x5A5A_5A5A)]),
            random: [0; 4],
            random_index: 0,
        };
        let context_ptr = (&mut context as *mut PointPoolTestImage).cast();
        let mut hooks = CallbackRandomPointHooks {
            context: context_ptr,
            read32: point_pool_read32,
            write32: point_pool_write32,
            reclaim: record_reclaim,
            random: point_pool_random,
        };

        hooks.reclaim(0x0021_0FF0, 0x75C0CC);

        assert_eq!(context.image[&0x75C0A8], 0);
        assert_eq!(context.image[&0x75C0CC], 0);
    }

    #[test]
    fn game_pool_free_c_abi_coalesces_and_updates_guest_image() {
        let mut image = pool_image(0x40);
        image.insert(0x1008, 0);
        image.insert(0x1040, 0);
        image.insert(0x1044, 0x80);
        image.insert(0x1048, 0x40);
        image.insert(0x006B_83D4, 0x2000);
        let status = unsafe {
            tsfp_kmem_game_pool_free(
                0x1020,
                (&mut image as *mut GameImage).cast(),
                Some(read32),
                Some(write32),
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image[&0x1000], 0);
        assert_eq!(image[&0x1004], 0xC0);
        assert_eq!(image[&0x006B_83C0], 0x540);
        assert_eq!(image[&0x006B_83CC], 0x1000);
        assert_eq!(image[&0x10C8], 0xC0);
    }

    #[test]
    fn game_pool_clear_c_abi_honors_retention_gate() {
        for gate in [0, 1] {
            let mut image: GameImage = HashMap::from([
                (0x006B_7AA8, gate),
                (0x006B_83C8, 0x1000),
                (0x006B_83CC, 0x1040),
            ]);
            let status = unsafe {
                tsfp_kmem_game_pool_clear(
                    (&mut image as *mut GameImage).cast(),
                    Some(read32),
                    Some(write32),
                )
            };
            assert_eq!(status, 0);
            if gate == 0 {
                assert_eq!(image[&0x006B_83C8], 0);
                assert_eq!(image[&0x006B_83CC], 0);
            } else {
                assert_eq!(image[&0x006B_83C8], 0x1000);
                assert_eq!(image[&0x006B_83CC], 0x1040);
            }
        }
    }

    #[test]
    fn game_pool_reset_c_abi_clears_byte_tail_and_rebuilds_free_header() {
        let mut image: GameImage = HashMap::from([
            (0x004C_81B8, 13),
            (0x006B_83C8, 0x1000),
            (0x1000, u32::MAX),
            (0x1004, u32::MAX),
            (0x1008, u32::MAX),
            (0x100C, u32::MAX),
        ]);
        let status = unsafe {
            tsfp_kmem_game_pool_reset(
                (&mut image as *mut GameImage).cast(),
                Some(read32),
                Some(write32),
                Some(read8),
                Some(write8),
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image[&0x006B_83CC], 0x1000);
        assert_eq!(image[&0x006B_83D0], 0x1000);
        assert_eq!(image[&0x006B_83C0], 13);
        assert_eq!(image[&0x006B_83C4], 13);
        assert_eq!(image[&0x006B_83D4], 0x100D);
        assert_eq!(image[&0x1000], 0);
        assert_eq!(image[&0x1004], 13);
        assert_eq!(image[&0x1008], 0);
        assert_eq!(image[&0x100C], 0);
        assert_eq!(image[&0x006B_83BC], 0);
    }
}

/// Counter ids for [`tsfp_kmem_counter`].
pub const CTR_PLACE_REQUEST_MISMATCH: u32 = 0;
pub const CTR_PLACE_LEFTOVER: u32 = 1;
pub const CTR_PLACE_REPLAYED: u32 = 2;
pub const CTR_LOG_LINES: u32 = 3;
pub const CTR_OPS: u32 = 4;
pub const CTR_POOL_LIVE: u32 = 5;
pub const CTR_POOL_LIVE_BYTES: u32 = 6;
pub const CTR_POOL_BAD_FREE: u32 = 7;
pub const CTR_LOCKED_PAGES: u32 = 8;
pub const CTR_PHYSICAL_REFUSED: u32 = 9;
pub const CTR_MODE: u32 = 10;

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    let mut guard = STATE.lock().ok()?;
    guard.as_mut().map(f)
}

/// Create the model. `probe_begin` / `probe_flush` are the C host's `kernel_guest_probe_*` hooks, called around
/// `munmap` in mode 1. Returns the active mode (0 for an unknown mode value).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_init(
    mode: u32,
    probe_begin: Option<extern "C" fn()>,
    probe_flush: Option<extern "C" fn()>,
) -> u32 {
    let backend = match mode {
        MODE_RUST => Backend::Mmap(MmapPlacement::with_probe_hooks(probe_begin, probe_flush)),
        MODE_COMPARE => Backend::Replay(ReplayPlacement::default()),
        _ => return MODE_OFF,
    };
    let Ok(mut guard) = STATE.lock() else {
        return MODE_OFF;
    };
    *guard = Some(State {
        mode,
        km: KernelMemory::new(backend),
        leftover_events: 0,
        log_lines: 0,
        ops: 0,
    });
    mode
}

/// Whether the shared guest-memory model is active as the authoritative Rust backend.
/// Audio uses this before selecting its direct Rust heap callbacks; compare/off modes
/// must keep the C guest heap authoritative.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_audio_heap_active() -> bool {
    with_state(|state| state.mode == MODE_RUST).unwrap_or(false)
}

/// Create an untyped guest heap in the same `GuestRegions` instance used by the host.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_audio_heap_create(flags: u32, initial: u32, maximum: u32) -> u32 {
    with_state(|state| {
        (state.mode == MODE_RUST)
            .then(|| state.km.regions.heap_create(flags, initial, maximum))
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

/// Allocate a block from a guest heap in the shared Rust guest-memory model.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_audio_heap_alloc(handle: u32, bytes: u32) -> u32 {
    with_state(|state| {
        (state.mode == MODE_RUST)
            .then(|| state.km.regions.heap_alloc(handle, bytes))
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

/// Test a guest heap handle in the shared Rust guest-memory model.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_audio_heap_valid(handle: u32) -> bool {
    with_state(|state| state.mode == MODE_RUST && state.km.regions.heap_valid(handle))
        .unwrap_or(false)
}

/// Return a guest heap block's requested size in the shared Rust guest-memory model.
///
/// # Safety
/// `out_size` must point to one writable `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_audio_heap_block_size(
    handle: u32,
    address: u32,
    out_size: *mut u32,
) -> bool {
    if out_size.is_null() {
        return false;
    }
    let Some(size) = with_state(|state| {
        (state.mode == MODE_RUST)
            .then(|| state.km.regions.heap_block_size(handle, address))
            .flatten()
    })
    .flatten() else {
        return false;
    };
    // SAFETY: the caller guarantees one writable output word.
    unsafe { out_size.write(size) };
    true
}

/// Destroy a guest heap in the shared Rust guest-memory model.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_audio_heap_destroy(handle: u32) -> bool {
    with_state(|state| state.mode == MODE_RUST && state.km.regions.heap_destroy(handle))
        .unwrap_or(false)
}

/// Queue one C placement event (compare mode). `kind` 0 map, 1 unmap.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_place_event(kind: u32, length: u64, address: u32, result: u32) {
    with_state(|s| {
        if let Backend::Replay(p) = s.km.regions.placement_mut() {
            p.events.push_back(PlaceEvent {
                kind,
                length,
                address,
                result,
            });
        }
    });
}

/// Run one op (see `ops.rs`). `inp` has `nin` words, `out` has room for `nout` words. Returns the op result, or
/// `u32::MAX` when no model is active or the op is unknown.
///
/// # Safety
/// `inp` must point to `nin` readable words (or be null with `nin` 0), `out` to `nout` writable words (or be null
/// with `nout` 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_op(
    op: u32,
    inp: *const u32,
    nin: u32,
    out: *mut u32,
    nout: u32,
) -> u32 {
    // SAFETY: the caller guarantees the extents (documented above).
    let input: &[u32] = if inp.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(inp, nin as usize) }
    };
    let output: &mut [u32] = if out.is_null() {
        &mut []
    } else {
        unsafe { std::slice::from_raw_parts_mut(out, nout as usize) }
    };
    let result = with_state(|s| {
        s.ops += 1;
        let r = ops::dispatch(&mut s.km, op, input, output);
        let lines = std::mem::take(&mut s.km.regions.log);
        s.log_lines += lines.len() as u64;
        if s.mode == MODE_RUST {
            for line in &lines {
                eprintln!("{line}");
            }
        }
        if let Backend::Replay(p) = s.km.regions.placement_mut()
            && !p.events.is_empty()
        {
            s.leftover_events += p.events.len() as u64;
            p.events.clear();
        }
        r
    });
    result.unwrap_or(u32::MAX)
}

/// Fill `buf` with up to `cap` region records of [`ops::REGION_WORDS`] words. Returns the model's region count.
///
/// # Safety
/// `buf` must have room for `cap * REGION_WORDS` words (or be null with `cap` 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_dump_regions(buf: *mut u32, cap: u32) -> u32 {
    with_state(|s| {
        let mut count = 0u32;
        for region in s.km.regions.regions() {
            if count < cap && !buf.is_null() {
                let words = ops::region_words(region);
                // SAFETY: record `count` is below `cap`, the caller sized `buf` for `cap` records.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        words.as_ptr(),
                        buf.add(count as usize * ops::REGION_WORDS),
                        ops::REGION_WORDS,
                    );
                }
            }
            count += 1;
        }
        count
    })
    .unwrap_or(0)
}

/// Fill `buf` with up to `cap` heap block header records of 5 words (address, then the 16 header bytes as four
/// little-endian words). Returns the total number of records the model has.
///
/// # Safety
/// `buf` must have room for `cap * 5` words (or be null with `cap` 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_dump_headers(buf: *mut u32, cap: u32) -> u32 {
    with_state(|s| {
        let headers = s.km.regions.heap_block_headers();
        for (i, (address, raw)) in headers.iter().enumerate().take(cap as usize) {
            if buf.is_null() {
                break;
            }
            let mut words = [0u32; 5];
            words[0] = *address;
            for w in 0..4 {
                words[1 + w] = u32::from_le_bytes([
                    raw[w * 4],
                    raw[w * 4 + 1],
                    raw[w * 4 + 2],
                    raw[w * 4 + 3],
                ]);
            }
            // SAFETY: record `i` is below `cap`, the caller sized `buf` for `cap` records.
            unsafe { std::ptr::copy_nonoverlapping(words.as_ptr(), buf.add(i * 5), 5) };
        }
        headers.len() as u32
    })
    .unwrap_or(0)
}

/// Read a counter (see `CTR_*`).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_kmem_counter(which: u32) -> u64 {
    with_state(|s| {
        let (request, replayed) = match s.km.regions.placement() {
            Backend::Replay(p) => (p.request_mismatches, p.replayed),
            Backend::Mmap(_) => (0, 0),
        };
        match which {
            CTR_PLACE_REQUEST_MISMATCH => request,
            CTR_PLACE_LEFTOVER => s.leftover_events,
            CTR_PLACE_REPLAYED => replayed,
            CTR_LOG_LINES => s.log_lines,
            CTR_OPS => s.ops,
            CTR_POOL_LIVE => s.km.pool_live_count() as u64,
            CTR_POOL_LIVE_BYTES => s.km.pool_live_bytes(),
            CTR_POOL_BAD_FREE => s.km.pool_bad_free_count() as u64,
            CTR_LOCKED_PAGES => s.km.locked_page_total() as u64,
            CTR_PHYSICAL_REFUSED => s.km.physical_refused_count() as u64,
            CTR_MODE => s.mode as u64,
            _ => 0,
        }
    })
    .unwrap_or(0)
}

/// Last replay request mismatch text (compare mode), copied NUL terminated into `buf` (truncated to `cap`).
///
/// # Safety
/// `buf` must have room for `cap` bytes (or be null with `cap` 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_kmem_last_place_mismatch(buf: *mut u8, cap: u32) {
    if buf.is_null() || cap == 0 {
        return;
    }
    let text = with_state(|s| match s.km.regions.placement() {
        Backend::Replay(p) => p.last_mismatch.clone(),
        Backend::Mmap(_) => String::new(),
    })
    .unwrap_or_default();
    let bytes = text.as_bytes();
    let n = bytes.len().min(cap as usize - 1);
    // SAFETY: n + 1 <= cap bytes are written.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, n);
        *buf.add(n) = 0;
    }
}

/// T2209 RTL callbacks, independent opt-in selection from the kmem model.
pub mod rtl;

#[cfg(test)]
mod game_ledger_tests {
    use super::*;
    use std::collections::HashMap;

    type GameImage = HashMap<u32, u32>;

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<GameImage>() };
        image.get(&address).copied().unwrap_or(0)
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<GameImage>() };
        image.insert(address, value);
    }

    unsafe extern "C" fn read_argument(context: *mut core::ffi::c_void) -> u32 {
        let image = unsafe { &*context.cast::<GameImage>() };
        image.get(&0xFFFF_0000).copied().unwrap_or(0)
    }

    #[test]
    fn primary_ledger_allocate_c_abi_updates_guest_image() {
        let mut image: GameImage = HashMap::from([
            (0x004C_81B4, 1),
            (0x006B_836C, 0x4000),
            (0x006B_8394, 0x30),
            (0x006B_8398, 2),
            (0x006B_7B68, 0x4000),
            (0x006B_7B6C, 0x10),
            (0x006B_7B70, 0x4010),
            (0x006B_7B74, 0x20),
        ]);
        let mut block = u32::MAX;
        let status = unsafe {
            tsfp_kmem_game_ledger_allocate(
                0x11,
                (&mut image as *mut GameImage).cast(),
                Some(read32),
                Some(write32),
                &mut block,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(block, 0x4030);
        assert_eq!(image[&0x006B_7B78], 0x4030);
        assert_eq!(image[&0x006B_7B7C], 0x20);
        assert_eq!(image[&0x006B_8394], 0x50);
        assert_eq!(image[&0x006B_8398], 3);
    }

    #[test]
    fn ledger_buffer_wrapper_c_abi_allocates_and_selects_in_guest_image() {
        let mut image: GameImage = HashMap::from([
            (0x004C_81B4, 1),
            (0x006B_836C, 0x4000),
            (0x006B_8394, 0),
            (0x006B_8398, 0),
            (0x006F_1E10, 0),
            (0x006F_1E14, 0),
            (0x0000_400C, 0xDEAD_BEEF),
        ]);
        let context = (&mut image as *mut GameImage).cast();
        let status = unsafe {
            tsfp_kmem_game_wrapper_ensure_ledger_buffer(0, context, Some(read32), Some(write32))
        };
        assert_eq!(status, 0);
        assert_eq!(image[&0x006F_1E14], 0x4000);
        assert_eq!(image[&0x006F_1E10], 0x4000);
        assert_eq!(image[&0x006B_7B68], 0x4000);
        assert_eq!(image[&0x006B_7B6C], 0x32000);
        assert_eq!(image[&0x006B_8394], 0x32000);
        assert_eq!(image[&0x006B_8398], 1);
        assert_eq!(image[&0x0000_400C], 0);
    }

    #[test]
    fn primary_ledger_release_c_abi_returns_original_registers_and_reclaims_tombstones() {
        let mut image: GameImage = HashMap::from([
            (0x006B_8394, 0x60),
            (0x006B_8398, 3),
            (0x006B_7B68, 0x1000),
            (0x006B_7B6C, 0x10),
            (0x006B_7B70, 0x1010),
            (0x006B_7B74, 0x20),
            (0x006B_7B78, 0x1030),
            (0x006B_7B7C, 0x30),
        ]);
        let context = (&mut image as *mut GameImage).cast();
        let (mut eax, mut ecx, mut edx) = (0, 0, 0);
        let status = unsafe {
            tsfp_kmem_game_ledger_release(
                3,
                0x1010,
                context,
                Some(read32),
                Some(write32),
                &mut eax,
                &mut ecx,
                &mut edx,
            )
        };
        assert_eq!(status, 0);
        assert_eq!((eax, ecx, edx), (2, 1, 0x1010));
        assert_eq!(image[&0x006B_7B70], 0);

        let status = unsafe {
            tsfp_kmem_game_ledger_release(
                3,
                0x1030,
                context,
                Some(read32),
                Some(write32),
                &mut eax,
                &mut ecx,
                &mut edx,
            )
        };
        assert_eq!(status, 0);
        assert_eq!((eax, ecx, edx), (1, 0x1000, 0x10));
        assert_eq!(image[&0x006B_8394], 0x10);
        assert_eq!(image[&0x006B_8398], 1);
        assert_eq!(image[&0x006B_7B74], u32::MAX);
        assert_eq!(image[&0x006B_7B7C], u32::MAX);
    }

    #[test]
    fn secondary_ledger_append_c_abi_reads_argument_and_publishes_registers() {
        let mut image: GameImage = HashMap::from([
            (0xFFFF_0000, 0x20),
            (0x006B_8368, 0x30),
            (0x006B_8380, 0x4000),
            (0x006B_838C, 2),
        ]);
        let context = (&mut image as *mut GameImage).cast();
        let (mut eax, mut ecx, mut edx) = (0, 0, 0);
        let status = unsafe {
            tsfp_kmem_game_ledger_secondary_append(
                context,
                Some(read32),
                Some(write32),
                Some(read_argument),
                &mut eax,
                &mut ecx,
                &mut edx,
            )
        };
        assert_eq!(status, 0);
        assert_eq!((eax, ecx, edx), (0x4030, 3, 0x20));
        assert_eq!(image[&0x006B_8368], 0x50);
        assert_eq!(image[&0x006B_7B28], 0x4030);
        assert_eq!(image[&0x006B_7B2C], 0x20);
        assert_eq!(image[&0x006B_838C], 3);
    }

    #[test]
    fn secondary_ledger_release_c_abi_returns_registers_and_reclaims_tombstones() {
        let mut image: GameImage = HashMap::from([
            (0x006B_8368, 0x60),
            (0x006B_838C, 3),
            (0x006B_7B18, 0x1000),
            (0x006B_7B1C, 0x10),
            (0x006B_7B20, 0x1010),
            (0x006B_7B24, 0x20),
            (0x006B_7B28, 0x1030),
            (0x006B_7B2C, 0x30),
        ]);
        let context = (&mut image as *mut GameImage).cast();
        let (mut eax, mut ecx, mut edx) = (0, 0, 0);
        let status = unsafe {
            tsfp_kmem_game_ledger_secondary_release(
                3,
                0x1010,
                context,
                Some(read32),
                Some(write32),
                &mut eax,
                &mut ecx,
                &mut edx,
            )
        };
        assert_eq!(status, 0);
        assert_eq!((eax, ecx, edx), (1, 1, 0x1010));
        assert_eq!(image[&0x006B_7B20], 0);

        let status = unsafe {
            tsfp_kmem_game_ledger_secondary_release(
                3,
                0x1030,
                context,
                Some(read32),
                Some(write32),
                &mut eax,
                &mut ecx,
                &mut edx,
            )
        };
        assert_eq!(status, 0);
        assert_eq!((eax, ecx, edx), (1, 0x1000, 0x10));
        assert_eq!(image[&0x006B_8368], 0x10);
        assert_eq!(image[&0x006B_838C], 1);
        assert_eq!(image[&0x006B_7B24], u32::MAX);
        assert_eq!(image[&0x006B_7B2C], u32::MAX);
    }
}

#[cfg(test)]
mod game_history_reset_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Image {
        bytes: BTreeMap<u32, u8>,
        reports: Vec<u32>,
        callouts: Vec<u32>,
    }

    impl Image {
        fn put32(&mut self, address: u32, value: u32) {
            for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
                self.bytes.insert(address + offset as u32, byte);
            }
        }

        fn get32(&self, address: u32) -> u32 {
            let mut bytes = [0; 4];
            for (offset, byte) in bytes.iter_mut().enumerate() {
                *byte = *self.bytes.get(&(address + offset as u32)).unwrap_or(&0);
            }
            u32::from_le_bytes(bytes)
        }
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        unsafe { &*context.cast::<Image>() }.get32(address)
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        unsafe { &mut *context.cast::<Image>() }.put32(address, value);
    }

    unsafe extern "C" fn read8(context: *mut core::ffi::c_void, address: u32) -> u8 {
        *unsafe { &*context.cast::<Image>() }
            .bytes
            .get(&address)
            .unwrap_or(&0)
    }

    unsafe extern "C" fn write8(context: *mut core::ffi::c_void, address: u32, value: u8) {
        unsafe { &mut *context.cast::<Image>() }
            .bytes
            .insert(address, value);
    }

    unsafe extern "C" fn report(context: *mut core::ffi::c_void, argument: u32) {
        unsafe { &mut *context.cast::<Image>() }
            .reports
            .push(argument);
    }

    unsafe extern "C" fn callout(context: *mut core::ffi::c_void, target: u32) {
        unsafe { &mut *context.cast::<Image>() }
            .callouts
            .push(target);
    }

    #[test]
    fn game_history_reset_c_abi_publishes_buffers_and_forwards_callouts() {
        let mut image = Image::default();
        image.put32(0x006B_8380, 0x0100_0000);
        image.put32(0x006B_8384, 0x0040_0000);
        image.put32(0x006B_8388, 0x0100_0000);
        image.put32(0x0079_094C, 0x64);
        image.bytes.insert(0x007D_E455, 0x0B);
        let context = (&mut image as *mut Image).cast();

        let status = unsafe {
            tsfp_kmem_game_ledger_history_reset(
                0,
                context,
                Some(read32),
                Some(write32),
                Some(read8),
                Some(write8),
                Some(report),
                Some(callout),
            )
        };

        assert_eq!(status, 0);
        assert_eq!(image.get32(0x006B_83AC), 0x000B_B800);
        assert_eq!(image.get32(0x006B_83B4), 0x00E8_9000);
        assert_eq!(image.get32(0x006B_83A0), 0x00E8_9000);
        assert_eq!(image.get32(0x006B_83A4), 0x00F4_4800);
        assert_eq!(image.get32(0x006B_839C), 0x00E8_9000);
        assert_eq!(image.get32(0x006B_83B0), 0);
        assert_eq!(image.get32(0x006B_8388), 0x00E8_8FF0);
        assert_eq!(image.get32(0x006B_8384), 0x0028_8FF0);
        assert_eq!(image.reports, [0]);
        assert_eq!(image.callouts, [0x001C_DAE0]);

        let status = unsafe {
            tsfp_kmem_game_ledger_history_reset(
                0x001C_DAE0,
                context,
                Some(read32),
                Some(write32),
                Some(read8),
                Some(write8),
                Some(report),
                Some(callout),
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.reports, [0, 0, 0x001C_DAE0]);
        assert_eq!(image.callouts, [0x001C_DAE0]);
    }
}

#[cfg(test)]
mod game_static_slot_wrapper_tests {
    use super::*;

    #[derive(Default)]
    struct Image {
        count: u32,
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*(context.cast::<Image>()) };
        if address == 0x7490A0 { image.count } else { 0 }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *(context.cast::<Image>()) };
        assert_eq!(address, 0x7490A0);
        image.count = value;
        image.writes.push((address, value));
    }

    #[test]
    fn game_wrapper_12c800_c_abi_returns_stable_guest_slots_and_stops_at_17() {
        for initial in [0, 1, 15, 16, 17, 18, u32::MAX] {
            let mut image = Image {
                count: initial,
                writes: Vec::new(),
            };
            let mut slot = u32::MAX;
            let status = unsafe {
                tsfp_kmem_game_wrapper_allocate_slot_745938(
                    (&mut image as *mut Image).cast(),
                    Some(read32),
                    Some(write32),
                    &mut slot,
                )
            };
            assert_eq!(status, 0);
            if initial < 17 {
                assert_eq!(slot, 0x745938 + initial * 0x144);
                assert_eq!(image.count, initial + 1);
                assert_eq!(image.writes, [(0x7490A0, initial + 1)]);
            } else {
                assert_eq!(slot, 0);
                assert_eq!(image.count, initial);
                assert!(image.writes.is_empty());
            }
        }
    }
}

#[cfg(test)]
mod game_objective_counter_wrapper_tests {
    use super::*;

    const MODE: u32 = 0x0010_0000;
    const DEFINITIONS: u32 = 0x0020_0000;
    const COUNTERS: u32 = 0x0090_0000;

    struct Image {
        region_b_cursor: u32,
        main_remaining: u32,
        published: u32,
        count: u32,
        definitions: [(u32, u32); 2],
        counters: [u32; 12],
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        match address {
            0x74C36C => MODE,
            a if a == MODE + 0x74 => DEFINITIONS,
            a if a == MODE + 0x78 => image.count,
            0x6B837C => image.region_b_cursor,
            0x6B8384 => image.main_remaining,
            0x7A2820 => image.published,
            _ => {
                for (index, (flags, seed)) in image.definitions.iter().enumerate() {
                    let start = DEFINITIONS + index as u32 * 0x98;
                    if address == start + 0x54 {
                        return *flags;
                    }
                    if address == start + 0x64 {
                        return *seed;
                    }
                }
                if (COUNTERS..COUNTERS + 0x30).contains(&address) && (address - COUNTERS) % 4 == 0 {
                    image.counters[((address - COUNTERS) / 4) as usize]
                } else {
                    0
                }
            }
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        match address {
            0x6B837C => image.region_b_cursor = value,
            0x6B8384 => image.main_remaining = value,
            0x7A2820 => image.published = value,
            a if (COUNTERS..COUNTERS + 0x30).contains(&a) && (a - COUNTERS) % 4 == 0 => {
                image.counters[((a - COUNTERS) / 4) as usize] = value;
            }
            _ => panic!("unexpected guest write {address:#x} = {value:#x}"),
        }
        image.writes.push((address, value));
    }

    #[test]
    fn game_wrapper_1620c0_allocates_clears_and_applies_optional_counter_seed() {
        let mut image = Image {
            region_b_cursor: COUNTERS,
            main_remaining: 0x1000,
            published: 0,
            count: 2,
            definitions: [(0x8000, 41), (0, 77)],
            counters: [u32::MAX; 12],
            writes: Vec::new(),
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_story_objective_counters(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published, COUNTERS);
        assert_eq!(image.region_b_cursor, COUNTERS + 0x30);
        assert_eq!(image.main_remaining, 0x1000 - 0x30);
        assert_eq!(image.counters[3], 41);
        assert!(
            image
                .counters
                .iter()
                .enumerate()
                .all(|(index, value)| index == 3 || *value == 0)
        );
        assert_eq!(image.writes.len(), 18);
        assert_eq!(image.writes[4], (0x7A2820, COUNTERS));
        assert!(
            image.writes[5..17]
                .iter()
                .enumerate()
                .all(
                    |(index, &(address, value))| address == COUNTERS + index as u32 * 4
                        && value == 0
                )
        );
        assert_eq!(image.writes[17], (COUNTERS + 0x0C, 41));
    }

    #[test]
    fn game_wrapper_1620c0_reuses_and_reinitializes_published_counter_table() {
        let mut image = Image {
            region_b_cursor: COUNTERS,
            main_remaining: 0x1000,
            published: COUNTERS,
            count: 2,
            definitions: [(0x8000, 41), (0, 77)],
            counters: [u32::MAX; 12],
            writes: Vec::new(),
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_story_objective_counters(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published, COUNTERS);
        assert_eq!(image.region_b_cursor, COUNTERS);
        assert_eq!(image.main_remaining, 0x1000);
        assert_eq!(image.counters[3], 41);
        assert!(
            image
                .counters
                .iter()
                .enumerate()
                .all(|(index, value)| index == 3 || *value == 0)
        );
        assert_eq!(image.writes.len(), 13);
        assert!(
            image.writes[..12]
                .iter()
                .enumerate()
                .all(
                    |(index, &(address, value))| address == COUNTERS + index as u32 * 4
                        && value == 0
                )
        );
        assert_eq!(image.writes[12], (COUNTERS + 0x0C, 41));
    }

    #[test]
    fn game_wrapper_1620c0_keeps_guest_state_when_mode_has_no_definitions() {
        let mut image = Image {
            region_b_cursor: COUNTERS,
            main_remaining: 0x1000,
            published: 0xDEAD_BEEF,
            count: 0,
            definitions: [(0, 0); 2],
            counters: [u32::MAX; 12],
            writes: Vec::new(),
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_story_objective_counters(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.published, 0xDEAD_BEEF);
        assert_eq!(image.region_b_cursor, COUNTERS);
        assert!(image.writes.is_empty());
    }
}

#[cfg(test)]
mod game_table_78b240_wrapper_tests {
    use super::*;

    #[derive(Default)]
    struct Image {
        region_b_cursor: u32,
        main_remaining: u32,
        table: [u32; 4],
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &mut *context.cast::<Image>() };
        match address {
            0x6B837C => image.region_b_cursor,
            0x6B8384 => image.main_remaining,
            a if (0x78B240..0x78B250).contains(&a) && (a - 0x78B240) % 4 == 0 => {
                image.table[((a - 0x78B240) / 4) as usize]
            }
            _ => panic!("unexpected guest read {address:#x}"),
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        match address {
            0x6B837C => image.region_b_cursor = value,
            0x6B8384 => image.main_remaining = value,
            a if (0x78B240..0x78B250).contains(&a) && (a - 0x78B240) % 4 == 0 => {
                image.table[((a - 0x78B240) / 4) as usize] = value;
            }
            _ => panic!("unexpected guest write {address:#x} = {value:#x}"),
        }
        image.writes.push((address, value));
    }

    #[test]
    fn game_wrapper_2760f0_allocates_and_publishes_guest_table() {
        let mut image = Image {
            region_b_cursor: 0x1000,
            main_remaining: 0x400,
            table: [u32::MAX; 4],
            writes: Vec::new(),
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_table_78b240(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.region_b_cursor, 0x1190);
        assert_eq!(image.main_remaining, 0x270);
        assert_eq!(image.table, [0x1000, 0, 0, 0]);
        assert_eq!(
            &image.writes[..4],
            &[(0x78B240, 0), (0x78B244, 0), (0x78B248, 0), (0x78B24C, 0)]
        );
        assert_eq!(image.writes.last(), Some(&(0x78B240, 0x1000)));
    }
}

#[cfg(test)]
mod game_weather_reset_wrapper_tests {
    use super::*;
    use std::collections::HashMap;

    struct Image {
        words: HashMap<u32, u32>,
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        image.words.get(&address).copied().unwrap_or(0)
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        image.words.insert(address, value);
        image.writes.push((address, value));
    }

    #[test]
    fn game_wrapper_119ed0_allocates_both_weather_tables_and_resets_fields() {
        for rows in [0x20, 0x32] {
            let mut image = Image {
                words: HashMap::from([
                    (0x6B837C, 0x0010_0000),
                    (0x6B8384, 0x0001_0000),
                    (0x73D994, rows),
                    (0x4E62E0, 7),
                    (0x4E62E4, 8),
                    (0x73D9B0, 0x3F80_0000),
                ]),
                writes: Vec::new(),
            };
            let status = unsafe {
                tsfp_kmem_game_wrapper_alloc_weather_tables_7ac824(
                    (&mut image as *mut Image).cast(),
                    Some(read32),
                    Some(write32),
                    None,
                )
            };
            assert_eq!(status, 0);
            assert_eq!(image.words.get(&0x7AC824), Some(&0x0010_0000));
            assert_eq!(image.words.get(&0x7AC828), Some(&0x0010_0900));
            assert_eq!(
                image.words.get(&0x6B837C),
                Some(&(0x0010_0900 + rows * 0xF0))
            );
            assert_eq!(
                image.words.get(&0x6B8384),
                Some(&(0x1_0000 - 0x900 - rows * 0xF0))
            );
            assert_eq!(image.words.get(&0x4E62E0), Some(&u32::MAX));
            assert_eq!(image.words.get(&0x4E62E4), Some(&u32::MAX));
            assert_eq!(image.words.get(&0x73D9B0), Some(&0));
            assert_eq!(image.writes.len(), 585);
            assert_eq!(image.writes[2], (0x7AC824, 0x0010_0000));
            assert!(
                image.writes[3..579]
                    .iter()
                    .enumerate()
                    .all(|(index, write)| { *write == (0x0010_0000 + index as u32 * 4, 0) })
            );
            assert_eq!(image.writes[581], (0x7AC828, 0x0010_0900));
            assert_eq!(
                &image.writes[582..],
                &[(0x4E62E0, u32::MAX), (0x4E62E4, u32::MAX), (0x73D9B0, 0)]
            );
        }
    }
}

#[cfg(test)]
mod effect_mesh_buffer_wrapper_tests {
    use super::*;
    use std::collections::HashMap;

    struct Image {
        words: HashMap<u32, u32>,
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        image.words.get(&address).copied().unwrap_or(0)
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        image.words.insert(address, value);
        image.writes.push((address, value));
    }

    #[test]
    fn game_wrapper_2bc150_allocates_and_publishes_effect_mesh_buffer() {
        let base = 0x0010_0000;
        let rounded_size = 0xD820;
        let mut image = Image {
            words: HashMap::from([(0x6B837C, base), (0x6B8384, 0x0001_0000)]),
            writes: Vec::new(),
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_effect_mesh_buffer_78aef0(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(image.words.get(&0x78AEF0), Some(&base));
        assert_eq!(image.words.get(&0x6B837C), Some(&(base + rounded_size)));
        assert_eq!(image.words.get(&0x6B8384), Some(&(0x1_0000 - rounded_size)));
        assert_eq!(image.writes.last(), Some(&(0x78AEF0, base)));
    }
}

#[cfg(test)]
mod game_75c10c_wrapper_tests {
    use super::*;

    struct Image {
        cursor: u32,
        remaining: u32,
        published: u32,
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        match address {
            0x6B83_7C => image.cursor,
            0x6B83_84 => image.remaining,
            0x75C1_0C | 0x75C0_F0 | 0x78B0_D0 => image.published,
            _ => 0,
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        match address {
            0x6B83_7C => image.cursor = value,
            0x6B83_84 => image.remaining = value,
            0x75C1_0C | 0x75C0_F0 | 0x78B0_D0 => image.published = value,
            _ => {}
        }
        image.writes.push((address, value));
    }

    #[test]
    fn game_wrapper_2210b0_c_abi_allocates_and_publishes_guest_address() {
        let mut image = Image {
            cursor: 0x1000,
            remaining: 0x2000,
            published: u32::MAX,
            writes: Vec::new(),
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_table_75c10c(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
            )
        };

        assert_eq!(status, 0);
        assert_eq!(image.published, 0x1000);
        assert_eq!(image.cursor, 0x1040);
        assert_eq!(image.remaining, 0x1FC0);
        assert_eq!(image.writes.last(), Some(&(0x75C1_0C, 0x1000)));
    }

    #[test]
    fn game_wrapper_21af30_c_abi_allocates_and_publishes_guest_address() {
        let mut image = Image {
            cursor: 0x1000,
            remaining: 0x2000,
            published: u32::MAX,
            writes: Vec::new(),
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_table_75c0f0(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
            )
        };

        assert_eq!(status, 0);
        assert_eq!(image.published, 0x1000);
        assert_eq!(image.cursor, 0x1010);
        assert_eq!(image.remaining, 0x1FF0);
        assert_eq!(image.writes.last(), Some(&(0x75C0_F0, 0x1000)));
    }

    #[test]
    fn game_wrapper_2a62c0_c_abi_allocates_and_publishes_guest_address() {
        let mut image = Image {
            cursor: 0x1000,
            remaining: 0x10000,
            published: u32::MAX,
            writes: Vec::new(),
        };
        let status = unsafe {
            tsfp_kmem_game_wrapper_alloc_table_78b0d0(
                (&mut image as *mut Image).cast(),
                Some(read32),
                Some(write32),
                None,
            )
        };

        assert_eq!(status, 0);
        assert_eq!(image.published, 0x1000);
        assert_eq!(image.cursor, 0x8D00);
        assert_eq!(image.remaining, 0x8300);
        assert_eq!(image.writes.last(), Some(&(0x78B0_D0, 0x1000)));
    }
}

#[cfg(test)]
mod game_2a8300_wrapper_tests {
    use super::*;
    use std::collections::HashMap;

    struct Image {
        cursor: u32,
        remaining: u32,
        words: HashMap<u32, u32>,
        writes: Vec<(u32, u32)>,
    }

    unsafe extern "C" fn read32(context: *mut core::ffi::c_void, address: u32) -> u32 {
        let image = unsafe { &*context.cast::<Image>() };
        match address {
            0x6B83_7C => image.cursor,
            0x6B83_84 => image.remaining,
            _ => image.words.get(&address).copied().unwrap_or(0),
        }
    }

    unsafe extern "C" fn write32(context: *mut core::ffi::c_void, address: u32, value: u32) {
        let image = unsafe { &mut *context.cast::<Image>() };
        match address {
            0x6B83_7C => image.cursor = value,
            0x6B83_84 => image.remaining = value,
            _ => {
                image.words.insert(address, value);
            }
        }
        image.writes.push((address, value));
    }

    #[test]
    fn game_wrapper_2a8300_c_abi_allocates_both_mode_buffers_in_order() {
        for (flag, count, size) in [(0, 0x578, 0xF0A0), (1, 0x1F4, 0x55F0)] {
            let mut image = Image {
                cursor: 0x1000,
                remaining: 0x10_0000,
                words: HashMap::new(),
                writes: Vec::new(),
            };
            let mut second_out = u32::MAX;
            let status = unsafe {
                tsfp_kmem_game_wrapper_alloc_tables_78b08c(
                    flag,
                    (&mut image as *mut Image).cast(),
                    Some(read32),
                    Some(write32),
                    None,
                    &mut second_out,
                )
            };

            let first = 0x1000;
            let second = first + size;
            assert_eq!(status, 0);
            assert_eq!(image.words.get(&0x78B08C), Some(&count));
            assert_eq!(image.words.get(&0x78B098), Some(&first));
            assert_eq!(image.words.get(&0x78B090), Some(&second));
            assert_eq!(second_out, second);
            assert_eq!(image.cursor, first + size * 2);
            assert_eq!(image.remaining, 0x10_0000 - size * 2);
            let publications: Vec<_> = image
                .writes
                .iter()
                .copied()
                .filter(|(address, _)| matches!(*address, 0x78B08C | 0x78B098 | 0x78B090))
                .collect();
            assert_eq!(
                publications,
                vec![(0x78B08C, count), (0x78B098, first), (0x78B090, second)]
            );
        }
    }
}
