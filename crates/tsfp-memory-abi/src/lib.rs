// SPDX-License-Identifier: GPL-3.0-or-later
//! Narrow runtime ABI adapter for hand-ported memory services.
use core::ptr;

#[cfg(feature = "direct-witness")]
#[allow(dead_code)]
#[path = "../../tsfp-direct-witness/src/lib.rs"]
mod direct_witness;
use std::ffi::{OsStr, c_int};

use tsfp_engine_memory::{
    GuestAddr, GuestMemory, MemoryFault, MemoryServices,
    ledger::Ledgers,
    pool::{Pool, PoolHooks},
    w4m_arena,
};

#[repr(C)]
pub struct Registers {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esp: u32,
}

struct MappedGuest {
    offset: usize,
}

unsafe extern "C" {
    fn tsfp_render_trace_readable(address: u32, length: u32) -> c_int;
    fn tsfp_t2o_memory_dispatch_active() -> bool;
    fn recomp_guest_call_cdecl(
        address: u32,
        arguments: *const u32,
        count: u32,
        result: *mut u32,
    ) -> bool;
}

#[cfg(test)]
mod test_guest_call_stub {
    #[unsafe(no_mangle)]
    pub extern "C" fn recomp_guest_call_cdecl(
        _address: u32,
        _arguments: *const u32,
        _count: u32,
        _result: *mut u32,
    ) -> bool {
        // Unit tests use callbacks only as a linkable host boundary; callback
        // behavior is covered by the guest integration path.
        true
    }
}

impl GuestMemory for MappedGuest {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        self.check(address, out.len(), false)?;
        for (index, value) in out.iter_mut().enumerate() {
            // SAFETY: the shared host checker validated the complete mapped guest range.
            *value = unsafe {
                ptr::read_volatile(
                    (address.0 as usize)
                        .wrapping_add(self.offset)
                        .wrapping_add(index) as *const u8,
                )
            };
        }
        Ok(())
    }

    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        self.check(address, bytes.len(), true)?;
        #[cfg(feature = "direct-witness")]
        let token = direct_witness::prepare();
        for (index, value) in bytes.iter().copied().enumerate() {
            // SAFETY: the shared host checker validated the complete mapped guest range.
            unsafe {
                ptr::write_volatile(
                    (address.0 as usize)
                        .wrapping_add(self.offset)
                        .wrapping_add(index) as *mut u8,
                    value,
                );
            }
        }
        #[cfg(feature = "direct-witness")]
        direct_witness::report(token, address.0, bytes);
        Ok(())
    }
}

impl MappedGuest {
    fn check(&self, address: GuestAddr, length: usize, write: bool) -> Result<(), MemoryFault> {
        let length_u32 = u32::try_from(length).unwrap_or(u32::MAX);
        // SAFETY: this helper validates mapping bounds without dereferencing the guest range.
        let valid = unsafe {
            range_is_valid(
                address.0,
                length_u32,
                tsfp_render_trace_readable(address.0, length_u32) != 0,
                tsfp_t2o_memory_dispatch_active(),
            )
        };
        if valid {
            Ok(())
        } else {
            Err(MemoryFault {
                address,
                length,
                access: if write {
                    tsfp_engine_memory::Access::Write
                } else {
                    tsfp_engine_memory::Access::Read
                },
            })
        }
    }
}

fn range_is_valid(address: u32, length: u32, live_mapped: bool, t2_offline: bool) -> bool {
    live_mapped || (t2_offline && u64::from(address) + u64::from(length) <= (1u64 << 32))
}

struct NoHooks;
impl w4m_arena::ArenaHooks for NoHooks {
    fn out_of_memory(&mut self, _argument: u32) {}

    fn out_of_memory_with_memory<M: GuestMemory>(&mut self, _memory: &mut M, argument: u32) {
        let arguments = [argument];
        let mut result = 0;
        // SAFETY: the executable host provides this nested cdecl guest-call bridge. The
        // measured 0x3E1E0 path calls 0x389B0 with one stack argument and ignores its result.
        let _ = unsafe { recomp_guest_call_cdecl(0x0003_89B0, arguments.as_ptr(), 1, &mut result) };
    }
}

impl PoolHooks for NoHooks {
    fn reclaim(&mut self, callback: u32, argument: u32) {
        if callback == 0 {
            return;
        }
        let arguments = [argument];
        let mut result = 0;
        // SAFETY: the executable host provides the nested guest-call bridge. The measured
        // reclaim path invokes its stored callback as cdecl(callback, argument), ignoring result.
        let _ = unsafe { recomp_guest_call_cdecl(callback, arguments.as_ptr(), 1, &mut result) };
    }
}

impl tsfp_engine_memory::ledger::InitHooks for NoHooks {
    fn callout(&mut self, target: u32) {
        let mut result = 0;
        // SAFETY: the executable host provides the nested cdecl guest-call bridge. The measured
        // ledger initializer calls its first-enable helper without arguments and ignores EAX.
        let _ = unsafe { recomp_guest_call_cdecl(target, ptr::null(), 0, &mut result) };
    }
}

struct HistoryHooks;
impl w4m_arena::ArenaHooks for HistoryHooks {
    fn out_of_memory(&mut self, argument: u32) {
        let arguments = [argument];
        let mut result = 0;
        // SAFETY: the executable host provides the measured 0x3E740 diagnostic call bridge.
        let _ = unsafe { recomp_guest_call_cdecl(0x0003_89B0, arguments.as_ptr(), 1, &mut result) };
    }
}
impl tsfp_engine_memory::ledger::InitHooks for HistoryHooks {
    fn callout(&mut self, target: u32) {
        let mut result = 0;
        // SAFETY: the executable host provides this measured no-argument callout bridge.
        let _ = unsafe { recomp_guest_call_cdecl(target, ptr::null(), 0, &mut result) };
    }
}

/// Anchor referenced by the Bevy executable so the weak C host hook resolves.
pub fn link_anchor<T>(_anchor: T) {}

fn dispatch_round16<M: GuestMemory, H: w4m_arena::ArenaHooks>(
    arena: &mut w4m_arena::Arena<'_, M, H>,
    registers: &mut Registers,
) -> bool {
    let size = match arena
        .memory
        .read_u32(GuestAddr(registers.esp.wrapping_add(4)))
    {
        Ok(value) => value,
        Err(_) => return false,
    };
    let kind = match arena
        .memory
        .read_u32(GuestAddr(registers.esp.wrapping_add(8)))
    {
        Ok(value) => value,
        Err(_) => return false,
    };
    match arena.alloc_round16(kind, size) {
        Ok(address) => {
            registers.eax = address;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_arena_initialize<M: GuestMemory>(memory: &mut M, registers: &mut Registers) -> bool {
    let esp = registers.esp;
    let base = match memory.read_u32(GuestAddr(esp.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let size = match memory.read_u32(GuestAddr(esp.wrapping_add(8))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    if (tsfp_engine_memory::service_slice::MemoryServices { memory })
        .initialize_arena(base, size)
        .is_err()
    {
        return false;
    }
    // The lifted function leaves EAX holding the computed arena end.
    registers.eax = base.wrapping_add(size);
    true
}

fn dispatch_history_reset<M: GuestMemory, H: tsfp_engine_memory::ledger::InitHooks>(
    memory: &mut M,
    esp: u32,
    hooks: &mut H,
) -> bool {
    let mode = match memory.read_u32(GuestAddr(esp.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    tsfp_engine_memory::ledger::Ledgers { memory }
        .history_reset(hooks, mode)
        .is_ok()
}

fn dispatch_realloc_payload_move<M: GuestMemory>(memory: &mut M, esp: u32) -> bool {
    let mut args = [0; 3];
    for (index, value) in args.iter_mut().enumerate() {
        *value = match memory.read_u32(GuestAddr(esp.wrapping_add(4 + 4 * index as u32))) {
            Ok(value) => value,
            Err(_) => return false,
        };
    }
    tsfp_engine_memory::crt_memmove::move_payload(memory, args[0], args[1], args[2]).is_ok()
}

fn dispatch_arena_rewind<M: GuestMemory, H: w4m_arena::ArenaHooks>(
    arena: &mut w4m_arena::Arena<'_, M, H>,
) -> bool {
    arena.rewind_top().is_ok()
}

fn dispatch_arena_release<M: GuestMemory, H: w4m_arena::ArenaHooks>(
    arena: &mut w4m_arena::Arena<'_, M, H>,
    esp: u32,
    registers: &mut Registers,
) -> bool {
    let block = match arena.memory.read_u32(GuestAddr(esp.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    match arena.release_tagged_block_with_registers(block) {
        Ok(result) => {
            registers.eax = result.eax;
            registers.ecx = result.ecx;
            registers.edx = result.edx;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_aligned<M: GuestMemory, H: w4m_arena::ArenaHooks>(
    arena: &mut w4m_arena::Arena<'_, M, H>,
    registers: &mut Registers,
) -> bool {
    let mut args = [0; 3];
    for (index, value) in args.iter_mut().enumerate() {
        *value = match arena
            .memory
            .read_u32(GuestAddr(registers.esp.wrapping_add(4 + 4 * index as u32)))
        {
            Ok(value) => value,
            Err(_) => return false,
        };
    }
    match tsfp_engine_memory::service_slice::GameAllocator::allocate_aligned(
        arena, args[0], args[1], args[2],
    ) {
        Ok(address) => {
            registers.eax = address;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_pool_free<M: GuestMemory>(memory: &mut M, stack: u32) -> bool {
    let pointer = match memory.read_u32(GuestAddr(stack.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    MemoryServices { memory }
        .free_pool_nullable(pointer)
        .is_ok()
}

fn dispatch_pool_reset<M: GuestMemory>(memory: &mut M) -> bool {
    tsfp_engine_memory::service_slice::MemoryServices { memory }
        .reset_pool()
        .is_ok()
}

fn dispatch_pool_clear<M: GuestMemory>(memory: &mut M) -> bool {
    tsfp_engine_memory::service_slice::MemoryServices { memory }
        .clear_pool()
        .is_ok()
}

fn dispatch_arena_teardown_clear<M: GuestMemory>(memory: &mut M) -> bool {
    MemoryServices { memory }
        .clear_arena_after_teardown()
        .is_ok()
}

fn dispatch_ledger_flag_set<M: GuestMemory>(memory: &mut M, stack: u32) -> bool {
    let value = match memory.read_u32(GuestAddr(stack.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let mut hooks = NoHooks;
    let mut arena = w4m_arena::Arena {
        memory,
        hooks: &mut hooks,
    };
    arena.set_ledger_flag(value).is_ok()
}

fn dispatch_ledger_flag_get<M: GuestMemory>(memory: &mut M, result: &mut u32) -> bool {
    let mut hooks = NoHooks;
    let mut arena = w4m_arena::Arena {
        memory,
        hooks: &mut hooks,
    };
    match arena.ledger_flag() {
        Ok(value) => {
            *result = value;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_ledger_allocate<M: GuestMemory>(memory: &mut M, stack: u32, result: &mut u32) -> bool {
    let size = match memory.read_u32(GuestAddr(stack.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    match (Ledgers { memory }).allocate_primary(size) {
        Ok(address) => {
            *result = address;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_ledger_release<M: GuestMemory>(
    memory: &mut M,
    stack: u32,
    registers: &mut Registers,
) -> bool {
    let block = match memory.read_u32(GuestAddr(stack.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let count = match memory.read_u32(GuestAddr(0x006b_8398)) {
        Ok(value) => value,
        Err(_) => return false,
    };
    match (Ledgers { memory }).release_primary_with_count_and_registers(count, block) {
        Ok(result) => {
            registers.eax = result.eax;
            registers.ecx = result.ecx;
            registers.edx = result.edx;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_ledger_secondary_append<M: GuestMemory>(
    memory: &mut M,
    stack: u32,
    registers: &mut Registers,
) -> bool {
    let size = match memory.read_u32(GuestAddr(stack.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    match (Ledgers { memory }).append_secondary_with_registers(size) {
        Ok(result) => {
            registers.eax = result.eax;
            registers.ecx = result.ecx;
            registers.edx = result.edx;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_ledger_secondary_release<M: GuestMemory>(
    memory: &mut M,
    stack: u32,
    registers: &mut Registers,
) -> bool {
    let block = match memory.read_u32(GuestAddr(stack.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let count = match memory.read_u32(GuestAddr(0x006b_838c)) {
        Ok(value) => value,
        Err(_) => return false,
    };
    match (Ledgers { memory }).release_secondary_with_count_and_registers(count, block) {
        Ok(result) => {
            registers.eax = result.eax;
            registers.ecx = result.ecx;
            registers.edx = result.edx;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_ledger_bounds<M: GuestMemory, H: tsfp_engine_memory::ledger::InitHooks>(
    memory: &mut M,
    hooks: &mut H,
) -> bool {
    (Ledgers { memory }).bounds_compute(hooks).is_ok()
}

fn dispatch_scratch_swap<M: GuestMemory>(memory: &mut M, _registers: &Registers) -> bool {
    tsfp_engine_memory::service_slice::MemoryServices { memory }
        .swap_scratch()
        .is_ok()
}

fn dispatch_pool_find_or_reclaim<M: GuestMemory, H: PoolHooks + w4m_arena::ArenaHooks>(
    memory: &mut M,
    hooks: &mut H,
    stack: u32,
    result: &mut u32,
) -> bool {
    let needed = match memory.read_u32(GuestAddr(stack.wrapping_add(4))) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let mut pool = Pool { memory, hooks };
    match pool.find_or_reclaim(needed) {
        Ok(header) => {
            *result = header;
            true
        }
        Err(_) => false,
    }
}

fn dispatch_pool_kind<M: GuestMemory, H: PoolHooks + w4m_arena::ArenaHooks>(
    memory: &mut M,
    hooks: &mut H,
    stack: u32,
    result: &mut u32,
) -> bool {
    let mut args = [0; 4];
    for (index, value) in args.iter_mut().enumerate() {
        *value = match memory.read_u32(GuestAddr(stack.wrapping_add(4 + 4 * index as u32))) {
            Ok(value) => value,
            Err(_) => return false,
        };
    }
    let mut pool = Pool { memory, hooks };
    match pool.allocate_by_kind(args[0], args[1], args[2], args[3]) {
        Ok(address) => {
            *result = address;
            true
        }
        Err(_) => false,
    }
}

/// Report hand-written memory roots that this ABI can dispatch through.
#[unsafe(no_mangle)]
fn memory_abi_supports_root(va: u32) -> bool {
    matches!(
        va,
        0x0003_ea00
            | 0x0003_df70
            | 0x0003_df00
            | 0x0003_e740
            | 0x0003_e820
            | 0x0003_c9800
            | 0x0003_ea20
            | 0x0003_e420
            | 0x0003_e430
            | 0x0003_e440
            | 0x0003_e4e0
            | 0x0003_e560
            | 0x0003_e5a0
            | 0x0003_e650
            | 0x0003_e9a0
            | 0x0003_ead0
            | 0x0003_eaf0
            | 0x0003_dfa0
            | 0x0003_e0b0
            | 0x0003_ebd0
            | 0x0003_e1e0
            | 0x0003_e240
            | 0x0003_e930
            | 0x0003_e970
            | 0x0003_ea40
            | 0x0003_eca0
            | 0x0003_ed20
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_memory_abi_has_root(va: u32) -> bool {
    memory_abi_supports_root(va)
}

fn memory_native_switch_enabled(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

/// Dispatch opt-in arena and pool services including reclaimable allocation `0x3EBD0`.
///
/// SAFETY: `registers` and `offset` must describe the active guest CPU and mapping.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_memory_abi_try(
    va: u32,
    registers: *mut Registers,
    offset: usize,
) -> bool {
    if !memory_abi_supports_root(va)
        || !memory_native_switch_enabled(std::env::var_os("TSFP_REA_MEMORY_NATIVE").as_deref())
    {
        return false;
    }
    // SAFETY: guaranteed by caller contract above.
    let registers = unsafe { &mut *registers };
    let mut memory = MappedGuest { offset };
    if va == 0x0003_ea40 || va == 0x0003_ead0 {
        return dispatch_pool_free(&mut memory, registers.esp);
    }
    if va == 0x0003_ea00 {
        let mut hooks = NoHooks;
        return dispatch_pool_initialize(&mut memory, &mut hooks);
    }
    if va == 0x0003_ea20 {
        return dispatch_pool_clear(&mut memory);
    }
    if va == 0x0003_df70 {
        return dispatch_arena_teardown_clear(&mut memory);
    }
    if va == 0x0003_e420 {
        return dispatch_ledger_flag_set(&mut memory, registers.esp);
    }
    if va == 0x0003_e430 {
        return dispatch_ledger_flag_get(&mut memory, &mut registers.eax);
    }
    if va == 0x0003_df00 {
        return dispatch_arena_initialize(&mut memory, registers);
    }
    if va == 0x0003_e740 || va == 0x0003_e820 {
        let mut hooks = HistoryHooks;
        return dispatch_history_reset(&mut memory, registers.esp, &mut hooks);
    }
    if va == 0x0003_c9800 {
        return dispatch_realloc_payload_move(&mut memory, registers.esp);
    }
    if va == 0x0003_e440 {
        return dispatch_ledger_allocate(&mut memory, registers.esp, &mut registers.eax);
    }
    if va == 0x0003_e4e0 {
        return dispatch_ledger_release(&mut memory, registers.esp, registers);
    }
    if va == 0x0003_e560 {
        return dispatch_ledger_secondary_append(&mut memory, registers.esp, registers);
    }
    if va == 0x0003_e5a0 {
        return dispatch_ledger_secondary_release(&mut memory, registers.esp, registers);
    }
    if va == 0x0003_e650 {
        let mut hooks = NoHooks;
        return dispatch_ledger_bounds(&mut memory, &mut hooks);
    }
    if va == 0x0003_e9a0 {
        // The original no-argument cdecl routine preserves guest registers.
        return dispatch_pool_reset(&mut memory);
    }
    if va == 0x0003_ed20 {
        let mut hooks = NoHooks;
        return dispatch_pool_kind(&mut memory, &mut hooks, registers.esp, &mut registers.eax);
    }
    if va == 0x0003_eaf0 {
        let mut hooks = NoHooks;
        return dispatch_pool_find_or_reclaim(
            &mut memory,
            &mut hooks,
            registers.esp,
            &mut registers.eax,
        );
    }
    if va == 0x0003_eca0 {
        let size = match memory.read_u32(GuestAddr(registers.esp.wrapping_add(4))) {
            Ok(value) => value,
            Err(_) => return false,
        };
        return match allocate_locked_pool(&mut memory, size) {
            Ok(address) => {
                registers.eax = address;
                true
            }
            Err(_) => false,
        };
    }
    if va == 0x0003_ebd0 {
        return dispatch_reclaimable_pool(&mut memory, registers.esp, &mut registers.eax);
    }
    let mut hooks = NoHooks;
    let mut arena = w4m_arena::Arena {
        memory: &mut memory,
        hooks: &mut hooks,
    };
    if va == 0x0003_e970 {
        let size = match arena
            .memory
            .read_u32(GuestAddr(registers.esp.wrapping_add(4)))
        {
            Ok(value) => value,
            Err(_) => return false,
        };
        return match arena.bump_allocate(size) {
            Ok(address) => {
                registers.eax = address;
                true
            }
            Err(_) => false,
        };
    }
    if va == 0x0003_dfa0 {
        return dispatch_round16(&mut arena, registers);
    }
    if va == 0x0003_e0b0 {
        return dispatch_aligned(&mut arena, registers);
    }
    if va == 0x0003_e930 {
        // This cdecl entry has no arguments or result; preserve the guest registers.
        return dispatch_scratch_swap(arena.memory, registers);
    }
    if va == 0x0003_e240 {
        return dispatch_arena_rewind(&mut arena);
    }
    dispatch_arena_release(&mut arena, registers.esp, registers)
}

fn allocate_locked_pool<M: GuestMemory>(memory: &mut M, size: u32) -> Result<u32, MemoryFault> {
    MemoryServices { memory }.allocate_pool_permanent(size)
}

fn dispatch_pool_initialize<M: GuestMemory, H: PoolHooks + w4m_arena::ArenaHooks>(
    memory: &mut M,
    hooks: &mut H,
) -> bool {
    let mut pool = Pool { memory, hooks };
    pool.allocate_and_reset().is_ok()
}

fn dispatch_reclaimable_pool<M: GuestMemory>(memory: &mut M, stack: u32, result: &mut u32) -> bool {
    let mut args = [0; 3];
    for (index, value) in args.iter_mut().enumerate() {
        *value = match memory.read_u32(GuestAddr(stack.wrapping_add(4 + 4 * index as u32))) {
            Ok(value) => value,
            Err(_) => return false,
        };
    }
    let mut hooks = NoHooks;
    let mut pool = Pool {
        memory,
        hooks: &mut hooks,
    };
    *result = match pool.allocate_reclaimable(args[0], args[1], args[2]) {
        Ok(address) => address,
        Err(_) => return false,
    };
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_engine_memory::MappedMemory;

    #[test]
    fn memory_native_switch_requires_explicit_one() {
        assert!(!memory_native_switch_enabled(None));
        assert!(!memory_native_switch_enabled(Some(OsStr::new("0"))));
        assert!(!memory_native_switch_enabled(Some(OsStr::new("true"))));
        assert!(memory_native_switch_enabled(Some(OsStr::new("1"))));
    }

    #[test]
    fn pool_initialize_adapter_allocates_buffer_then_resets_it() {
        let mut bytes = vec![0xaa; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x4c81b8, 0x40);
        put(&mut bytes, w4m_arena::globals::REGION_B_CURSOR, 0x1000);
        put(&mut bytes, w4m_arena::globals::MAIN_REMAINING, 0x10000);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;

        assert!(dispatch_pool_initialize(&mut memory, &mut hooks));
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c8)).unwrap(), 0x1000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x1000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x1000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d4)).unwrap(), 0x1040);
        assert_eq!(memory.read_u32(GuestAddr(0x1000)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x1004)).unwrap(), 0x40);
        assert_eq!(memory.read_u32(GuestAddr(0x6b837c)).unwrap(), 0x1040);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8384)).unwrap(), 0xffc0);
    }

    #[test]
    fn ledger_flag_adapters_use_measured_cdecl_global_and_preserve_set_registers() {
        let mut bytes = vec![0; 0x006b_9000];
        bytes[0x104..0x108].copy_from_slice(&0x8000_0001u32.to_le_bytes());
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        assert!(dispatch_ledger_flag_set(&mut memory, 0x100));
        assert_eq!(memory.read_u32(GuestAddr(0x4c81b4)).unwrap(), 0x8000_0001);
        let mut result = 0xfeed_beef;
        assert!(dispatch_ledger_flag_get(&mut memory, &mut result));
        assert_eq!(result, 0x8000_0001);
        assert!(memory_abi_supports_root(0x0003_e420));
        assert!(memory_abi_supports_root(0x0003_e430));
    }

    #[test]
    fn reclaimable_pool_adapter_reads_three_cdecl_words_and_publishes_block() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        for (index, value) in [65_u32, 0x1234, 0x5678].into_iter().enumerate() {
            put(&mut bytes, 0x104 + 4 * index as u32, value);
        }
        put(&mut bytes, 0x6b83c0, 0x200);
        put(&mut bytes, 0x6b83c8, 0x800);
        put(&mut bytes, 0x6b83cc, 0x800);
        put(&mut bytes, 0x6b83d0, 0x1000);
        put(&mut bytes, 0x6b83d4, 0x1000);
        put(&mut bytes, 0x804, 0x200);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut result = 0xfeed;

        assert!(dispatch_reclaimable_pool(&mut memory, 0x100, &mut result));
        assert_eq!(result, 0x820);
        assert_eq!(memory.read_u32(GuestAddr(0x800)).unwrap(), 1);
        assert_eq!(memory.read_u32(GuestAddr(0x804)).unwrap(), 0x80);
        assert_eq!(memory.read_u32(GuestAddr(0x80c)).unwrap(), 0x1234);
        assert_eq!(memory.read_u32(GuestAddr(0x810)).unwrap(), 0x5678);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x880);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x180);
        assert_eq!(memory.read_u32(GuestAddr(0x880)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x884)).unwrap(), 0x180);
        assert_eq!(memory.read_u32(GuestAddr(0x888)).unwrap(), 0x80);
        assert_eq!(memory.read_u32(GuestAddr(0xa08)).unwrap(), 0x180);
    }

    struct Hooks;
    impl w4m_arena::ArenaHooks for Hooks {
        fn out_of_memory(&mut self, _argument: u32) {}
    }
    impl PoolHooks for Hooks {
        fn reclaim(&mut self, _callback: u32, _argument: u32) {}
    }

    #[derive(Default)]
    struct HistoryCalls {
        reports: Vec<u32>,
        callouts: Vec<u32>,
    }
    impl w4m_arena::ArenaHooks for HistoryCalls {
        fn out_of_memory(&mut self, argument: u32) {
            self.reports.push(argument);
        }
    }
    impl tsfp_engine_memory::ledger::InitHooks for HistoryCalls {
        fn callout(&mut self, target: u32) {
            self.callouts.push(target);
        }
    }

    #[test]
    fn history_reset_adapter_reads_mode_allocates_buffers_and_reports_mode() {
        let mut bytes = vec![0; 0x0080_0000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..][..4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 2);
        put(&mut bytes, w4m_arena::globals::BOTTOM_LIMIT, 0x500000);
        put(&mut bytes, w4m_arena::globals::TOP_CURSOR, 0x500000);
        put(&mut bytes, w4m_arena::globals::MAIN_REMAINING, 0x400000);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = HistoryCalls::default();

        assert!(dispatch_history_reset(&mut memory, 0x100, &mut hooks));
        assert_eq!(memory.read_u32(GuestAddr(0x6b83ac)).unwrap(), 0x100000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83b4)).unwrap(), 0x300000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83a0)).unwrap(), 0x300000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83a4)).unwrap(), 0x400000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b839c)).unwrap(), 0x300000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83b0)).unwrap(), 0);
        assert_eq!(hooks.reports, [2]);
        assert!(hooks.callouts.is_empty());
    }

    #[test]
    fn history_reset_adapter_supports_fixed_modes_one_and_three() {
        for (mode, size) in [(1, 0x4b000), (3, 0x4000)] {
            let mut bytes = vec![0; 0x0080_0000];
            let put = |bytes: &mut [u8], address: u32, value: u32| {
                bytes[address as usize..][..4].copy_from_slice(&value.to_le_bytes());
            };
            put(&mut bytes, 0x104, mode);
            put(&mut bytes, w4m_arena::globals::BOTTOM_LIMIT, 0x500000);
            put(&mut bytes, w4m_arena::globals::TOP_CURSOR, 0x500000);
            put(&mut bytes, w4m_arena::globals::MAIN_REMAINING, 0x400000);
            let mut memory = MappedMemory {
                base: GuestAddr(0),
                bytes: &mut bytes,
            };
            let mut hooks = HistoryCalls::default();

            assert!(dispatch_history_reset(&mut memory, 0x100, &mut hooks));
            let first = 0x500000 - 2 * size;
            assert_eq!(memory.read_u32(GuestAddr(0x6b83ac)).unwrap(), size);
            assert_eq!(memory.read_u32(GuestAddr(0x6b83b4)).unwrap(), first);
            assert_eq!(memory.read_u32(GuestAddr(0x6b83a0)).unwrap(), first);
            assert_eq!(memory.read_u32(GuestAddr(0x6b83a4)).unwrap(), first + size);
            assert_eq!(memory.read_u32(GuestAddr(0x6b839c)).unwrap(), first);
            assert_eq!(memory.read_u32(GuestAddr(0x6b83b0)).unwrap(), 0);
            assert_eq!(hooks.reports, [mode]);
            assert!(hooks.callouts.is_empty());
        }
    }

    #[test]
    fn history_reset_adapter_mode_zero_preserves_selector_and_callback() {
        // The lifted contract selects 0x96000 or 0x4B000 from the gate byte,
        // unless the secondary selector is 0x64, which overrides it to 0xBB800.
        for (gate, selector, expected, callout) in [
            (0x0A, 0, 0x4B000, false),
            (0x00, 0, 0x96000, false),
            (0x0A, 0x64, 0xBB800, false),
            (0x00, 0x64, 0xBB800, true),
        ] {
            let mut bytes = vec![0; 0x0080_0000];
            let put = |bytes: &mut [u8], address: u32, value: u32| {
                bytes[address as usize..][..4].copy_from_slice(&value.to_le_bytes());
            };
            put(&mut bytes, 0x104, 0);
            bytes[0x7DE455] = gate;
            put(&mut bytes, 0x79094C, selector);
            put(&mut bytes, w4m_arena::globals::BOTTOM_LIMIT, 0x500000);
            put(&mut bytes, w4m_arena::globals::TOP_CURSOR, 0x500000);
            put(&mut bytes, w4m_arena::globals::MAIN_REMAINING, 0x400000);
            let mut memory = MappedMemory {
                base: GuestAddr(0),
                bytes: &mut bytes,
            };
            let mut hooks = HistoryCalls::default();

            assert!(dispatch_history_reset(&mut memory, 0x100, &mut hooks));
            let first = 0x500000 - 2 * expected;
            assert_eq!(memory.read_u32(GuestAddr(0x6b83ac)).unwrap(), expected);
            assert_eq!(memory.read_u32(GuestAddr(0x6b83b4)).unwrap(), first);
            assert_eq!(
                memory.read_u32(GuestAddr(0x6b83a4)).unwrap(),
                first + expected
            );
            assert_eq!(hooks.reports, [0]);
            assert_eq!(
                hooks.callouts,
                if callout { vec![0x1CDAE0] } else { vec![] }
            );
        }
    }

    #[test]
    fn history_reset_adapter_invalid_mode_keeps_size_and_reports_both_values() {
        let mode = 7;
        let size = 0x4000;
        let mut bytes = vec![0; 0x0080_0000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..][..4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, mode);
        put(&mut bytes, 0x6b83ac, size);
        put(&mut bytes, w4m_arena::globals::BOTTOM_LIMIT, 0x500000);
        put(&mut bytes, w4m_arena::globals::TOP_CURSOR, 0x500000);
        put(&mut bytes, w4m_arena::globals::MAIN_REMAINING, 0x400000);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = HistoryCalls::default();

        assert!(dispatch_history_reset(&mut memory, 0x100, &mut hooks));
        let first = 0x500000 - 2 * size;
        assert_eq!(memory.read_u32(GuestAddr(0x6b83ac)).unwrap(), size);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83b4)).unwrap(), first);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83a4)).unwrap(), first + size);
        assert_eq!(hooks.reports, [0, mode]);
        assert!(hooks.callouts.is_empty());
    }

    #[test]
    fn realloc_payload_move_adapter_copies_measured_disjoint_dword_span() {
        let mut bytes = vec![0; 0x2000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..][..4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 0x800);
        put(&mut bytes, 0x108, 0x400);
        put(&mut bytes, 0x10c, 0x20);
        for (index, byte) in bytes[0x400..0x420].iter_mut().enumerate() {
            *byte = index as u8;
        }
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };

        assert!(dispatch_realloc_payload_move(&mut memory, 0x100));
        for index in 0..0x20 {
            let mut byte = [0];
            memory.read(GuestAddr(0x800 + index), &mut byte).unwrap();
            assert_eq!(byte[0], index as u8);
        }
    }

    #[test]
    fn realloc_payload_move_adapter_copies_backward_for_measured_overlap() {
        let mut bytes = vec![0; 0x1000];
        for (index, byte) in bytes[0x400..0x410].iter_mut().enumerate() {
            *byte = (index + 1) as u8;
        }
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };

        // REA measures a backward copy when source < destination < source + count.
        assert!(
            tsfp_engine_memory::crt_memmove::move_payload(&mut memory, 0x404, 0x400, 16).is_ok()
        );
        let mut actual = [0; 16];
        memory.read(GuestAddr(0x404), &mut actual).unwrap();
        assert_eq!(actual, std::array::from_fn(|index| (index + 1) as u8));
    }

    #[test]
    fn adapter_crate_exposes_the_measured_release_service() {
        // Arena globals are fixed guest addresses around 0x6B8384; include them
        // in the fixture mapping as well as the released block.
        let mut bytes = vec![0; 0x006b_9000];
        let base = 0;
        let block = base + 0x800;
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            let start = (address - base) as usize;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, block - 16, 0x41434f4c);
        put(&mut bytes, block - 12, 0x4d454d4c);
        put(&mut bytes, block - 4, w4m_arena::globals::TOP_CURSOR);
        put(&mut bytes, block - 8, 0x30);
        put(&mut bytes, w4m_arena::globals::TOP_CURSOR, block - 4);
        put(&mut bytes, w4m_arena::globals::MAIN_REMAINING, 0x100);
        put(&mut bytes, 0x104, block);
        let mut memory = MappedMemory {
            base: GuestAddr(base),
            bytes: &mut bytes,
        };
        let mut hooks = Hooks;
        let mut arena = w4m_arena::Arena {
            memory: &mut memory,
            hooks: &mut hooks,
        };
        let mut registers = Registers {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x100,
        };
        assert!(dispatch_arena_release(
            &mut arena,
            registers.esp,
            &mut registers
        ));
        assert_eq!(
            (registers.eax, registers.ecx, registers.edx),
            (0x140, 0x30, 0x100)
        );
    }

    #[test]
    fn rewind_adapter_preserves_registers_and_rewinds_measured_globals() {
        let mut bytes = vec![0; 0x006b_9000];
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        memory
            .write_u32(
                tsfp_engine_memory::GuestAddr(w4m_arena::globals::BOTTOM_LIMIT),
                0x2000,
            )
            .unwrap();
        memory
            .write_u32(
                tsfp_engine_memory::GuestAddr(w4m_arena::globals::TOP_CURSOR),
                0x2100,
            )
            .unwrap();
        memory
            .write_u32(
                tsfp_engine_memory::GuestAddr(w4m_arena::globals::MAIN_REMAINING),
                0x40,
            )
            .unwrap();
        let mut hooks = Hooks;
        let mut arena = w4m_arena::Arena {
            memory: &mut memory,
            hooks: &mut hooks,
        };
        assert!(dispatch_arena_rewind(&mut arena));
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::TOP_CURSOR))
                .unwrap(),
            0x2000
        );
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::MAIN_REMAINING))
                .unwrap(),
            0xffff_ff40
        );
    }

    #[test]
    fn scratch_swap_adapter_updates_measured_globals_and_preserves_registers() {
        let mut bytes = vec![0; 0x006b_9000];
        for (address, value) in [
            (0x6b839c, 0x1020),
            (0x6b83a0, 0x1000),
            (0x6b83a4, 0x2000),
            (0x6b83b0, 16u32),
        ] {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        }
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let registers = Registers {
            eax: 0x1234,
            ecx: 0x5678,
            edx: 0x9abc,
            esp: 0x100,
        };

        assert!(dispatch_scratch_swap(&mut memory, &registers));
        assert_eq!(
            (registers.eax, registers.ecx, registers.edx, registers.esp),
            (0x1234, 0x5678, 0x9abc, 0x100)
        );
        let read =
            |memory: &mut MappedMemory<'_>, address| memory.read_u32(GuestAddr(address)).unwrap();
        assert_eq!(read(&mut memory, 0x6b83b0), 32);
        assert_eq!(read(&mut memory, 0x6b83a0), 0x2000);
        assert_eq!(read(&mut memory, 0x6b83a4), 0x1000);
        assert_eq!(read(&mut memory, 0x6b839c), 0x2000);
    }

    #[test]
    fn offline_dispatch_accepts_high_guest_addresses_only_in_its_fault_tracked_window() {
        assert!(range_is_valid(0x4531_bf70, 16, false, true));
        assert!(!range_is_valid(0x4531_bf70, 16, false, false));
        assert!(range_is_valid(0x4531_bf70, 16, true, false));
        assert!(!range_is_valid(u32::MAX, 2, false, true));
    }

    #[test]
    fn abi_dispatch_and_registry_share_supported_root_predicate() {
        for root in [
            0x0003_df70,
            0x0003_df00,
            0x0003_e740,
            0x0003_e820,
            0x0003_c9800,
            0x0003_ea00,
            0x0003_ea20,
            0x0003_e420,
            0x0003_e430,
            0x0003_e440,
            0x0003_e4e0,
            0x0003_e560,
            0x0003_e5a0,
            0x0003_e650,
            0x0003_e9a0,
            0x0003_ead0,
            0x0003_eaf0,
            0x0003_dfa0,
            0x0003_e0b0,
            0x0003_ebd0,
            0x0003_e1e0,
            0x0003_e240,
            0x0003_e930,
            0x0003_e970,
            0x0003_ea40,
            0x0003_eca0,
            0x0003_ed20,
        ] {
            assert_eq!(
                memory_abi_supports_root(root),
                tsfp_memory_abi_has_root(root)
            );
        }
        assert!(memory_abi_supports_root(0x0003_eaf0));
        assert!(memory_abi_supports_root(0x0003_ea20));
        assert!(memory_abi_supports_root(0x0003_e420));
        assert!(memory_abi_supports_root(0x0003_e430));
        assert!(memory_abi_supports_root(0x0003_e440));
        assert!(memory_abi_supports_root(0x0003_e4e0));
        assert!(memory_abi_supports_root(0x0003_e560));
        assert!(memory_abi_supports_root(0x0003_e5a0));
        assert!(memory_abi_supports_root(0x0003_e650));
    }

    #[test]
    fn offline_dispatch_registry_contains_only_implemented_roots() {
        assert!(tsfp_memory_abi_has_root(0x0003_df00));
        assert!(tsfp_memory_abi_has_root(0x0003_e740));
        assert!(tsfp_memory_abi_has_root(0x0003_e820));
        assert!(tsfp_memory_abi_has_root(0x0003_c9800));
        assert!(tsfp_memory_abi_has_root(0x0003_e1e0));
        assert!(tsfp_memory_abi_has_root(0x0003_ea00));
        assert!(tsfp_memory_abi_has_root(0x0003_ea20));
        assert!(tsfp_memory_abi_has_root(0x0003_e420));
        assert!(tsfp_memory_abi_has_root(0x0003_e430));
        assert!(tsfp_memory_abi_has_root(0x0003_e440));
        assert!(tsfp_memory_abi_has_root(0x0003_e4e0));
        assert!(tsfp_memory_abi_has_root(0x0003_e560));
        assert!(tsfp_memory_abi_has_root(0x0003_e5a0));
        assert!(tsfp_memory_abi_has_root(0x0003_e650));
        assert!(tsfp_memory_abi_has_root(0x0003_e9a0));
        assert!(tsfp_memory_abi_has_root(0x0003_ead0));
        assert!(tsfp_memory_abi_has_root(0x0003_ebd0));
        assert!(tsfp_memory_abi_has_root(0x0003_dfa0));
        assert!(tsfp_memory_abi_has_root(0x0003_e0b0));
        assert!(tsfp_memory_abi_has_root(0x0003_e240));
        assert!(tsfp_memory_abi_has_root(0x0003_e930));
        assert!(tsfp_memory_abi_has_root(0x0003_e970));
        assert!(tsfp_memory_abi_has_root(0x0003_ea40));
        assert!(tsfp_memory_abi_has_root(0x0003_eca0));
        assert!(tsfp_memory_abi_has_root(0x0003_ed20));
        assert!(tsfp_memory_abi_has_root(0x0003_eaf0));
    }

    #[test]
    fn round16_adapter_uses_measured_cdecl_size_kind_order() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..][..4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 17);
        put(&mut bytes, 0x108, 1);
        put(&mut bytes, w4m_arena::globals::REGION_B_CURSOR, 0x1000);
        put(&mut bytes, w4m_arena::globals::MAIN_REMAINING, 0x1000);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = Hooks;
        let mut arena = w4m_arena::Arena {
            memory: &mut memory,
            hooks: &mut hooks,
        };
        let mut registers = Registers {
            eax: 0xaaaa,
            ecx: 0xbbbb,
            edx: 0xcccc,
            esp: 0x100,
        };
        assert!(dispatch_round16(&mut arena, &mut registers));
        assert_eq!(registers.eax, 0x1000);
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::REGION_B_CURSOR))
                .unwrap(),
            0x1020
        );
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::MAIN_REMAINING))
                .unwrap(),
            0xfe0
        );
    }

    #[test]
    fn arena_initialize_adapter_reads_cdecl_base_size_and_publishes_measured_ranges() {
        let mut bytes = vec![0; 0x0080_0000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..][..4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 0x200000);
        put(&mut bytes, 0x108, 0x180000);
        put(&mut bytes, 0x4c81b4, 1);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };

        let mut registers = Registers {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x100,
        };
        assert!(dispatch_arena_initialize(&mut memory, &mut registers));
        assert_eq!(registers.eax, 0x380000);
        for (address, expected) in [
            (0x7e72d4, 0x200000),
            (0x7e72d8, 0x180000),
            (0x6b8370, 0x200000),
            (0x6b8374, 0x29dc00),
            (0x6b8378, 0x9dc00),
            (0x6b837c, 0x29dc00),
            (0x6b8380, 0x380000),
            (0x6b836c, 0x11a000),
            (0x6b8384, 0xe2400),
            (0x6b8388, 0x380000),
        ] {
            assert_eq!(memory.read_u32(GuestAddr(address)).unwrap(), expected);
        }
    }

    #[test]
    fn aligned_adapter_reads_alignment_size_kind_and_updates_kind_one_state() {
        let mut bytes = vec![0; 0x006b_9000];
        for (index, value) in [32_u32, 33, 1].into_iter().enumerate() {
            let start = 0x104 + 4 * index;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            let start = address as usize;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, w4m_arena::globals::REGION_B_CURSOR, 0x1003);
        put(&mut bytes, w4m_arena::globals::MAIN_REMAINING, 0x1000);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;
        let mut arena = w4m_arena::Arena {
            memory: &mut memory,
            hooks: &mut hooks,
        };
        let mut registers = Registers {
            eax: 0,
            ecx: 0xaaaa,
            edx: 0xbbbb,
            esp: 0x100,
        };
        assert!(dispatch_aligned(&mut arena, &mut registers));
        assert_eq!(registers.eax, 0x1020);
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::REGION_B_CURSOR))
                .unwrap(),
            0x1050
        );
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::MAIN_REMAINING))
                .unwrap(),
            0xfb3
        );
    }

    #[test]
    fn aligned_adapter_preserves_zero_size_kind_one_cursor() {
        let mut bytes = vec![0; 0x006b_9000];
        for (index, value) in [16_u32, 0, 1].into_iter().enumerate() {
            let start = 0x104 + 4 * index;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[0x6b837c..0x6b8380].copy_from_slice(&0x1800_u32.to_le_bytes());
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;
        let mut arena = w4m_arena::Arena {
            memory: &mut memory,
            hooks: &mut hooks,
        };
        let mut registers = Registers {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0x100,
        };

        assert!(dispatch_aligned(&mut arena, &mut registers));
        assert_eq!(registers.eax, 0x1800);
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::REGION_B_CURSOR))
                .unwrap(),
            0x1800
        );
    }

    #[test]
    fn aligned_adapter_routes_kind_two_and_unchecked_kind_four() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        for (index, value) in [32_u32, 17, 2].into_iter().enumerate() {
            put(&mut bytes, 0x104 + 4 * index as u32, value);
        }
        put(&mut bytes, w4m_arena::globals::REGION_A_CURSOR, 0x10003);
        put(&mut bytes, w4m_arena::globals::REGION_A_REMAINING, 0x1000);
        put(&mut bytes, 0x120, 64);
        put(&mut bytes, 0x124, 17);
        put(&mut bytes, 0x128, 4);
        put(&mut bytes, w4m_arena::globals::KIND4_CURSOR, 0x180003);

        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;
        let mut arena = w4m_arena::Arena {
            memory: &mut memory,
            hooks: &mut hooks,
        };
        let mut registers = Registers {
            eax: 0,
            ecx: 0xaaaa,
            edx: 0xbbbb,
            esp: 0x100,
        };

        assert!(dispatch_aligned(&mut arena, &mut registers));
        assert_eq!(registers.eax, 0x10020);
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::REGION_A_CURSOR))
                .unwrap(),
            0x10040
        );
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::REGION_A_REMAINING))
                .unwrap(),
            0xfc3
        );

        registers.esp = 0x11c;
        assert!(dispatch_aligned(&mut arena, &mut registers));
        assert_eq!(registers.eax, 0x180040);
        assert_eq!(registers.ecx, 0xaaaa);
        assert_eq!(registers.edx, 0xbbbb);
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::KIND4_CURSOR))
                .unwrap(),
            0x180060
        );
    }

    #[test]
    fn pool_free_adapter_uses_nullable_cdecl_pointer_and_measured_state_guard() {
        let mut bytes = vec![0; 0x006b_9000];
        let block = 0x800;
        let user = block + 0x20;
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, user);
        put(&mut bytes, block, 1);
        put(&mut bytes, block + 4, 0x40);
        put(&mut bytes, block + 8, 0);
        put(&mut bytes, 0x6b83c0, 0x100);
        put(&mut bytes, 0x6b83cc, 0x900);
        put(&mut bytes, 0x6b83d4, 0xa00);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        assert!(dispatch_pool_free(&mut memory, 0x100));
        assert_eq!(memory.read_u32(GuestAddr(block)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x140);
        assert!(dispatch_pool_free(&mut memory, 0x100)); // exact-state guard: no double credit
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x140);
        for state in [2, 3, u32::MAX] {
            put(&mut memory.bytes, block, state);
            put(&mut memory.bytes, block + 4, 0x80);
            put(&mut memory.bytes, block + 8, 0x1234);
            put(&mut memory.bytes, block + 0xc, 0x5678);
            put(&mut memory.bytes, block + 0x10, 0x9abc);
            put(&mut memory.bytes, 0x6b83cc, 0x880);
            assert!(dispatch_pool_free(&mut memory, 0x100));
            assert_eq!(memory.read_u32(GuestAddr(block)).unwrap(), state);
            assert_eq!(memory.read_u32(GuestAddr(block + 4)).unwrap(), 0x80);
            assert_eq!(memory.read_u32(GuestAddr(block + 8)).unwrap(), 0x1234);
            assert_eq!(memory.read_u32(GuestAddr(block + 0xc)).unwrap(), 0x5678);
            assert_eq!(memory.read_u32(GuestAddr(block + 0x10)).unwrap(), 0x9abc);
            assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x140);
            assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x880);
        }
        put(&mut memory.bytes, 0x104, 0);
        assert!(dispatch_pool_free(&mut memory, 0x100)); // null is accepted
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x140);
    }

    #[test]
    fn pool_free_adapter_coalesces_one_successor_and_predecessor() {
        let mut bytes = vec![0; 0x006b_9000];
        let mut put = |address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(0x104, 0x860); // cdecl payload pointer
        put(0x800, 0); // free predecessor
        put(0x804, 0x40);
        put(0x808, 0);
        put(0x840, 1); // allocated block being released
        put(0x844, 0x40);
        put(0x848, 0x40);
        put(0x880, 0); // one free successor
        put(0x884, 0x40);
        put(0x888, 0x40);
        put(0x8c0, 1); // following allocated block
        put(0x8c4, 0x40);
        put(0x8c8, 0x40);
        put(0x6b83c0, 0x80);
        put(0x6b83cc, 0x880);
        put(0x6b83d4, 0x900);
        drop(put);

        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        assert!(dispatch_pool_free(&mut memory, 0x100));
        assert_eq!(memory.read_u32(GuestAddr(0x840)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x804)).unwrap(), 0xc0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0xc0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x800);
        assert_eq!(memory.read_u32(GuestAddr(0x8c8)).unwrap(), 0xc0);
    }

    #[test]
    fn pool_reset_adapter_uses_measured_globals_and_zeroes_pool_storage() {
        let mut bytes = vec![0xaa; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x4c81b8, 0x40);
        put(&mut bytes, 0x6b83c8, 0x800);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        assert!(dispatch_pool_reset(&mut memory));
        assert!(memory.bytes[0x800..0x804].iter().all(|byte| *byte == 0));
        assert!(memory.bytes[0x808..0x840].iter().all(|byte| *byte == 0));
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x800);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x800);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x40);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c4)).unwrap(), 0x40);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d4)).unwrap(), 0x840);
        assert_eq!(memory.read_u32(GuestAddr(0x800)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x804)).unwrap(), 0x40);
    }

    #[test]
    fn pool_clear_adapter_obeys_retention_gate_and_preserves_other_metadata() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        put(memory.bytes, 0x6b7aa8, 0);
        put(memory.bytes, 0x6b83c8, 0x1234);
        put(memory.bytes, 0x6b83cc, 0x5678);
        put(memory.bytes, 0x6b83c0, 0x90);
        put(memory.bytes, 0x6b83d0, 0x9876);
        assert!(dispatch_pool_clear(&mut memory));
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c8)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x90);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x9876);

        put(memory.bytes, 0x6b7aa8, 1);
        put(memory.bytes, 0x6b83c8, 0x2345);
        put(memory.bytes, 0x6b83cc, 0x6789);
        assert!(dispatch_pool_clear(&mut memory));
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c8)).unwrap(), 0x2345);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x6789);
    }

    #[test]
    fn arena_teardown_adapter_obeys_retention_gate_and_clears_only_arena_state() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        for (address, value) in [
            (0x6b7aa8, 0),
            (0x6b837c, 0x1234),
            (0x6b8380, 0x2345),
            (0x6b8384, 0x3456),
            (0x6b8388, 0x4567),
            (0x6b8370, 0x5678),
            (0x6b8390, 0x6789),
        ] {
            put(memory.bytes, address, value);
        }

        assert!(dispatch_arena_teardown_clear(&mut memory));
        for address in [0x6b837c, 0x6b8380, 0x6b8384, 0x6b8388] {
            assert_eq!(memory.read_u32(GuestAddr(address)).unwrap(), 0);
        }
        assert_eq!(memory.read_u32(GuestAddr(0x6b8370)).unwrap(), 0x5678);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8390)).unwrap(), 0x6789);

        put(memory.bytes, 0x6b7aa8, 1);
        for (address, value) in [
            (0x6b837c, 0x1111),
            (0x6b8380, 0x2222),
            (0x6b8384, 0x3333),
            (0x6b8388, 0x4444),
        ] {
            put(memory.bytes, address, value);
        }
        assert!(dispatch_arena_teardown_clear(&mut memory));
        for (address, value) in [
            (0x6b837c, 0x1111),
            (0x6b8380, 0x2222),
            (0x6b8384, 0x3333),
            (0x6b8388, 0x4444),
        ] {
            assert_eq!(memory.read_u32(GuestAddr(address)).unwrap(), value);
        }
    }

    #[test]
    fn primary_ledger_adapters_round_allocate_and_pop_release() {
        let mut bytes = vec![0; 0x006b_a000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 17);
        put(&mut bytes, 0x4c81b4, 1);
        put(&mut bytes, 0x6b836c, 0x1000);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut result = 0;
        assert!(dispatch_ledger_allocate(&mut memory, 0x100, &mut result));
        assert_eq!(result, 0x1000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b7b68)).unwrap(), 0x1000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b7b6c)).unwrap(), 0x20);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8394)).unwrap(), 0x20);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8398)).unwrap(), 1);

        put(memory.bytes, 0x104, result);
        let mut registers = Registers {
            eax: 0xaaaa,
            ecx: 0xbbbb,
            edx: 0xcccc,
            esp: 0x100,
        };
        assert!(dispatch_ledger_release(
            &mut memory,
            registers.esp,
            &mut registers
        ));
        assert_eq!(memory.read_u32(GuestAddr(0x6b8394)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8398)).unwrap(), 0);
        assert_eq!(registers.eax, 0);
        assert_eq!(registers.edx, 0);
    }

    #[test]
    fn secondary_ledger_adapters_publish_measured_registers_and_pop_rows() {
        let mut bytes = vec![0; 0x006b_a000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 0x30);
        put(&mut bytes, 0x6b8368, 0x20);
        put(&mut bytes, 0x6b8380, 0x0010_0000);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut registers = Registers {
            eax: 0xaaaa,
            ecx: 0xbbbb,
            edx: 0xcccc,
            esp: 0x100,
        };
        assert!(dispatch_ledger_secondary_append(
            &mut memory,
            registers.esp,
            &mut registers
        ));
        assert_eq!(
            (registers.eax, registers.ecx, registers.edx),
            (0x0010_0020, 1, 0x30)
        );
        assert_eq!(memory.read_u32(GuestAddr(0x6b8368)).unwrap(), 0x50);
        assert_eq!(memory.read_u32(GuestAddr(0x6b7b18)).unwrap(), 0x0010_0020);

        put(memory.bytes, 0x104, 0x0010_0020);
        registers.eax = 0xaaaa;
        registers.ecx = 0xbbbb;
        registers.edx = 0xcccc;
        assert!(dispatch_ledger_secondary_release(
            &mut memory,
            registers.esp,
            &mut registers
        ));
        assert_eq!(registers.eax, 1);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8368)).unwrap(), 0x20);
        assert_eq!(memory.read_u32(GuestAddr(0x6b838c)).unwrap(), 0);
    }

    #[test]
    fn ledger_bounds_adapter_initializes_measured_regions_and_primary_table() {
        let mut bytes = vec![0xaa; 0x0084_0000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        bytes[0x7de394] = 0x65;
        put(&mut bytes, 0x7de398, 0);
        put(&mut bytes, 0x4c81b4, 0);
        put(&mut bytes, 0x7e72d4, 0x0010_0000);
        put(&mut bytes, 0x7e72d8, 0x0050_0000);
        put(&mut bytes, 0x6b7b68, 0xfeed);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;
        assert!(dispatch_ledger_bounds(&mut memory, &mut hooks));
        assert_eq!(memory.read_u32(GuestAddr(0x4c81b4)).unwrap(), 1);
        assert_eq!(memory.read_u32(GuestAddr(0x6b837c)).unwrap(), 0x0019_dc00);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8380)).unwrap(), 0x0039_a000);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8394)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b8398)).unwrap(), 0);
        assert!(
            memory.bytes[0x6b7b68..0x6b8368]
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    #[test]
    fn reclaim_scan_adapter_reads_needed_size_and_returns_free_header() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 0x20);
        put(&mut bytes, 0x800, 1);
        put(&mut bytes, 0x804, 0x40);
        put(&mut bytes, 0x840, 0);
        put(&mut bytes, 0x844, 0x40);
        put(&mut bytes, 0x6b83c0, 0x40);
        put(&mut bytes, 0x6b83c8, 0x800);
        put(&mut bytes, 0x6b83cc, 0x800);
        put(&mut bytes, 0x6b83d0, 0x800);
        put(&mut bytes, 0x6b83d4, 0x880);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = Hooks;
        let mut result = 0;
        assert!(dispatch_pool_find_or_reclaim(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));
        assert_eq!(result, 0x840);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x840);
    }

    #[test]
    fn reclaim_scan_adapter_clears_owner_word_before_reclaiming_without_callback() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 0x80);
        put(&mut bytes, 0x800, 1);
        put(&mut bytes, 0x804, 0x40);
        put(&mut bytes, 0x80c, 0x900);
        put(&mut bytes, 0x810, 0);
        put(&mut bytes, 0x840, 0);
        put(&mut bytes, 0x844, 0x40);
        put(&mut bytes, 0x900, 0xfeed);
        put(&mut bytes, 0x6b83c0, 0);
        put(&mut bytes, 0x6b83c8, 0x800);
        put(&mut bytes, 0x6b83cc, 0x800);
        put(&mut bytes, 0x6b83d0, 0x800);
        put(&mut bytes, 0x6b83d4, 0x880);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = Hooks;
        let mut result = 0;

        assert!(dispatch_pool_find_or_reclaim(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));

        assert_eq!(result, 0x800);
        assert_eq!(memory.read_u32(GuestAddr(0x900)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x800)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x40);
    }

    #[test]
    fn reclaim_scan_adapter_invokes_callback_with_live_payload_before_free() {
        #[derive(Default)]
        struct ReclaimCalls(Vec<(u32, u32, u32, u32)>);
        impl w4m_arena::ArenaHooks for ReclaimCalls {
            fn out_of_memory(&mut self, _argument: u32) {}
        }
        impl PoolHooks for ReclaimCalls {
            fn reclaim(&mut self, _callback: u32, _argument: u32) {}

            fn reclaim_block_with_memory<M: GuestMemory>(
                &mut self,
                memory: &mut M,
                payload: u32,
                callback: u32,
                argument: u32,
            ) -> Result<(), tsfp_engine_memory::pool::PoolError> {
                let state = memory.read_u32(GuestAddr(payload - 0x20)).unwrap();
                self.0.push((payload, callback, argument, state));
                Ok(())
            }
        }

        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        put(&mut bytes, 0x104, 0x80);
        put(&mut bytes, 0x800, 1);
        put(&mut bytes, 0x804, 0x40);
        put(&mut bytes, 0x80c, 0xcafe);
        put(&mut bytes, 0x810, 0xbeef);
        put(&mut bytes, 0x840, 0);
        put(&mut bytes, 0x844, 0x40);
        put(&mut bytes, 0x6b83c0, 0);
        put(&mut bytes, 0x6b83c8, 0x800);
        put(&mut bytes, 0x6b83cc, 0x800);
        put(&mut bytes, 0x6b83d0, 0x800);
        put(&mut bytes, 0x6b83d4, 0x880);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = ReclaimCalls::default();
        let mut result = 0;

        assert!(dispatch_pool_find_or_reclaim(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));

        assert_eq!(result, 0x800);
        assert_eq!(hooks.0, [(0x820, 0xbeef, 0xcafe, 1)]);
        assert_eq!(memory.read_u32(GuestAddr(0x800)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x40);
    }

    #[test]
    fn pool_kind_adapter_reads_four_cdecl_words_and_returns_zero_for_unknown_kind() {
        let mut bytes = vec![0; 0x200];
        for (index, value) in [0x1234_u32, 7, 0x5678, 0x9abc].into_iter().enumerate() {
            let start = 0x104 + 4 * index;
            bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
        }
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;
        let mut result = 0xfeed;
        assert!(dispatch_pool_kind(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));
        assert_eq!(result, 0);
    }

    #[test]
    fn pool_kind_adapter_routes_locked_kind_to_measured_pool_carve() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: usize, value: u32| {
            bytes[address..address + 4].copy_from_slice(&value.to_le_bytes());
        };
        for (index, value) in [65_u32, 2, 0x1234, 0x5678].into_iter().enumerate() {
            put(&mut bytes, 0x104 + 4 * index, value);
        }
        put(&mut bytes, 0x6b83cc, 0x800);
        put(&mut bytes, 0x804, 0x200);
        put(&mut bytes, 0x6b83d0, 0);
        put(&mut bytes, 0x6b83c0, 0x400);
        put(&mut bytes, 0x6b83c4, 0x200);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;
        let mut result = 0;
        assert!(dispatch_pool_kind(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));
        assert_eq!(result, 0x820);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x880);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x80);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x380);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c4)).unwrap(), 0x180);
        assert_eq!(memory.read_u32(GuestAddr(0x800)).unwrap(), 2);
        assert_eq!(memory.read_u32(GuestAddr(0x804)).unwrap(), 0x80);
    }

    #[test]
    fn pool_kind_adapter_reclaims_then_wrap_reclaims_reused_extent() {
        #[derive(Default)]
        struct ReclaimCalls(Vec<(u32, u32, u32, u32)>);
        impl w4m_arena::ArenaHooks for ReclaimCalls {
            fn out_of_memory(&mut self, _argument: u32) {}
        }
        impl PoolHooks for ReclaimCalls {
            fn reclaim(&mut self, _callback: u32, _argument: u32) {}

            fn reclaim_block_with_memory<M: GuestMemory>(
                &mut self,
                memory: &mut M,
                payload: u32,
                callback: u32,
                argument: u32,
            ) -> Result<(), tsfp_engine_memory::pool::PoolError> {
                let state = memory.read_u32(GuestAddr(payload - 0x20)).unwrap();
                self.0.push((payload, callback, argument, state));
                Ok(())
            }
        }

        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: usize, value: u32| {
            bytes[address..address + 4].copy_from_slice(&value.to_le_bytes());
        };
        for (index, value) in [0x81_u32, 1, 0x1111, 0x2222].into_iter().enumerate() {
            put(&mut bytes, 0x104 + 4 * index, value);
        }
        put(&mut bytes, 0x800, 1);
        put(&mut bytes, 0x804, 0x40);
        put(&mut bytes, 0x80c, 0xcafe);
        put(&mut bytes, 0x810, 0xbeef);
        put(&mut bytes, 0x840, 0);
        put(&mut bytes, 0x844, 0x80);
        put(&mut bytes, 0x6b83c0, 0x80);
        put(&mut bytes, 0x6b83c4, 0xc0);
        put(&mut bytes, 0x6b83c8, 0x800);
        put(&mut bytes, 0x6b83cc, 0x800);
        put(&mut bytes, 0x6b83d0, 0x800);
        put(&mut bytes, 0x6b83d4, 0x8c0);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = ReclaimCalls::default();
        let mut result = 0;

        assert!(dispatch_pool_kind(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));

        assert_eq!(
            hooks.0,
            [(0x820, 0xbeef, 0xcafe, 1), (0x820, 0x2222, 0x1111, 1)]
        );
        assert_eq!(result, 0x820);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83bc)).unwrap(), 1);
        assert_eq!(memory.read_u32(GuestAddr(0x800)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(0x804)).unwrap(), 0xc0);
        assert_eq!(memory.read_u32(GuestAddr(0x80c)).unwrap(), 0x1111);
        assert_eq!(memory.read_u32(GuestAddr(0x810)).unwrap(), 0x2222);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0xc0);
    }

    #[test]
    fn pool_kind_adapter_rounds_zero_size_to_one_reclaim_granule() {
        let mut bytes = vec![0; 0x006b_9000];
        let put = |bytes: &mut [u8], address: usize, value: u32| {
            bytes[address..address + 4].copy_from_slice(&value.to_le_bytes());
        };
        for (index, value) in [0_u32, 2, 0x1234, 0x5678].into_iter().enumerate() {
            put(&mut bytes, 0x104 + 4 * index, value);
        }
        put(&mut bytes, 0x6b83cc, 0x800);
        put(&mut bytes, 0x804, 0x200);
        put(&mut bytes, 0x6b83d0, 0);
        put(&mut bytes, 0x6b83c0, 0x400);
        put(&mut bytes, 0x6b83c4, 0x200);
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;
        let mut result = 0;

        assert!(dispatch_pool_kind(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));
        assert_eq!(result, 0x820);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), 0x840);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x40);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x3c0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c4)).unwrap(), 0x1c0);
        assert_eq!(memory.read_u32(GuestAddr(0x800)).unwrap(), 2);
        assert_eq!(memory.read_u32(GuestAddr(0x804)).unwrap(), 0x40);
        assert_eq!(memory.read_u32(GuestAddr(0x844)).unwrap(), 0x1c0);
    }

    #[test]
    fn bump_allocator_adapter_returns_old_cursor_and_updates_rounded_accounting() {
        let mut bytes = vec![0; 0x006b_9000];
        let cursor: u32 = 0x0012_3450;
        let total: u32 = 0x40;
        bytes[w4m_arena::globals::BUMP_CURSOR as usize..][..4]
            .copy_from_slice(&cursor.to_le_bytes());
        bytes[w4m_arena::globals::BUMP_TOTAL as usize..][..4].copy_from_slice(&total.to_le_bytes());
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = Hooks;
        let mut arena = w4m_arena::Arena {
            memory: &mut memory,
            hooks: &mut hooks,
        };
        assert_eq!(arena.bump_allocate(17).unwrap(), cursor);
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::BUMP_CURSOR))
                .unwrap(),
            cursor + 32
        );
        assert_eq!(
            arena
                .memory
                .read_u32(GuestAddr(w4m_arena::globals::BUMP_TOTAL))
                .unwrap(),
            total + 32
        );
    }

    #[test]
    fn bump_allocator_adapter_preserves_wrapping_round_to_zero_edge() {
        let mut bytes = vec![0; 0x006b_9000];
        let cursor: u32 = 0x0012_3450;
        let total: u32 = 0x40;
        bytes[w4m_arena::globals::BUMP_CURSOR as usize..][..4]
            .copy_from_slice(&cursor.to_le_bytes());
        bytes[w4m_arena::globals::BUMP_TOTAL as usize..][..4].copy_from_slice(&total.to_le_bytes());
        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = Hooks;
        let mut arena = w4m_arena::Arena {
            memory: &mut memory,
            hooks: &mut hooks,
        };

        for size in [0xffff_fff1, 0xffff_fffe, u32::MAX] {
            assert_eq!(arena.bump_allocate(size).unwrap(), cursor);
            assert_eq!(
                arena
                    .memory
                    .read_u32(GuestAddr(w4m_arena::globals::BUMP_CURSOR))
                    .unwrap(),
                cursor
            );
            assert_eq!(
                arena
                    .memory
                    .read_u32(GuestAddr(w4m_arena::globals::BUMP_TOTAL))
                    .unwrap(),
                total
            );
        }
    }

    #[test]
    fn locked_pool_abi_service_carves_and_accounts_the_measured_prefix() {
        let mut bytes = vec![0; 0x006b_9000];
        let mut put = |address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        let block = 0x800;
        put(0x6b83cc, block);
        put(block + 4, 0x200);
        put(0x6b83d0, 0x1000);
        put(0x6b83c0, 0x400);
        put(0x6b83c4, 0x500);

        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        assert_eq!(allocate_locked_pool(&mut memory, 65).unwrap(), block + 0x20);
        assert_eq!(memory.read_u32(GuestAddr(block)).unwrap(), 2);
        assert_eq!(memory.read_u32(GuestAddr(block + 4)).unwrap(), 0x80);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), block + 0x80);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x1080);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x380);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c4)).unwrap(), 0x480);
    }

    #[test]
    fn pool_kind_abi_repeats_locked_prefix_carves_and_keeps_trailing_header() {
        let mut bytes = vec![0; 0x006b_9000];
        let mut put = |address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        let block = 0x800;
        for (index, value) in [65_u32, 2, 0xaaaa, 0xbbbb].into_iter().enumerate() {
            put(0x104 + 4 * index as u32, value);
        }
        put(0x6b83cc, block);
        put(block + 4, 0x200);
        put(block + 0x0c, 0xaaaa);
        put(block + 0x10, 0xbbbb);
        put(block + 0x14, 0xcccc);
        put(0x6b83d0, 0x1000);
        put(0x6b83c0, 0x400);
        put(0x6b83c4, 0x500);

        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        let mut hooks = NoHooks;
        let mut result = 0;
        assert!(dispatch_pool_kind(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));
        assert_eq!(result, block + 0x20);
        memory.write_u32(GuestAddr(0x104), 1).unwrap();
        assert!(dispatch_pool_kind(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));
        assert_eq!(result, block + 0xa0);
        assert_eq!(memory.read_u32(GuestAddr(block + 0x80)).unwrap(), 2);
        assert_eq!(memory.read_u32(GuestAddr(block + 0x84)).unwrap(), 0x40);
        assert_eq!(memory.read_u32(GuestAddr(block + 0x8c)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(block + 0x90)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(block + 0xc0)).unwrap(), 0);
        assert_eq!(memory.read_u32(GuestAddr(block + 0xc4)).unwrap(), 0x140);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), block + 0xc0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x10c0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x340);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c4)).unwrap(), 0x440);

        memory.write_u32(GuestAddr(0x104), 0x1000).unwrap();
        memory.write_u32(GuestAddr(0x108), 1).unwrap();
        assert!(dispatch_pool_kind(
            &mut memory,
            &mut hooks,
            0x100,
            &mut result
        ));
        assert_eq!(result, 0);
        // The wrapper publishes its "kind 1 was requested" flag even when
        // the underlying reclaimable allocation cannot satisfy the request.
        assert_eq!(memory.read_u32(GuestAddr(0x6b83bc)).unwrap(), 1);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), block + 0xc0);
        assert_eq!(memory.read_u32(GuestAddr(block + 0xc4)).unwrap(), 0x140);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x10c0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x340);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c4)).unwrap(), 0x440);
    }

    #[test]
    fn locked_pool_abi_service_preserves_wrapping_rounding_and_rejects_short_block() {
        let mut bytes = vec![0; 0x006b_9000];
        let mut put = |address: u32, value: u32| {
            bytes[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
        };
        let block = 0x800;
        put(0x6b83cc, block);
        put(block + 4, 0x100);
        put(0x6b83d0, 0x1000);
        put(0x6b83c0, 0x400);
        put(0x6b83c4, 0x500);

        let mut memory = MappedMemory {
            base: GuestAddr(0),
            bytes: &mut bytes,
        };
        // (0xffff_fff0 + 0x5f) wraps to 0x4f, then rounds to 0x40.
        assert_eq!(
            allocate_locked_pool(&mut memory, 0xffff_fff0).unwrap(),
            block + 0x20
        );
        assert_eq!(memory.read_u32(GuestAddr(block)).unwrap(), 2);
        assert_eq!(memory.read_u32(GuestAddr(block + 4)).unwrap(), 0x40);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), block + 0x40);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83d0)).unwrap(), 0x1040);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x3c0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c4)).unwrap(), 0x4c0);

        let before: Vec<u8> = memory.bytes[block as usize..block as usize + 0x100].to_vec();
        assert_eq!(allocate_locked_pool(&mut memory, 0x100).unwrap(), 0);
        assert_eq!(memory.bytes[block as usize..block as usize + 0x100], before);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83cc)).unwrap(), block + 0x40);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c0)).unwrap(), 0x3c0);
        assert_eq!(memory.read_u32(GuestAddr(0x6b83c4)).unwrap(), 0x4c0);
    }
}
