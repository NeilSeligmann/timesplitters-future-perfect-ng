//! Hybrid guest memory uses explicit, ordered accesses, never transmuted structs.
pub use tsfp_types::GuestAddr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryFault {
    pub address: GuestAddr,
    pub length: usize,
    pub access: Access,
}

/// Implementations validate each operation, preserving guest fault/access order.
/// An error must never be turned into zero or a silently successful write.
pub trait GuestMemory {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault>;
    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault>;
    fn read_u32(&mut self, address: GuestAddr) -> Result<u32, MemoryFault> {
        let mut bytes = [0; 4];
        self.read(address, &mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
    fn write_u32(&mut self, address: GuestAddr, value: u32) -> Result<(), MemoryFault> {
        self.write(address, &value.to_le_bytes())
    }
}

/// Bounded borrowed mapping for migration tooling; no retail mapping is assumed.
pub struct MappedMemory<'a> {
    pub base: GuestAddr,
    pub bytes: &'a mut [u8],
}
impl MappedMemory<'_> {
    fn range(
        &self,
        address: GuestAddr,
        length: usize,
        access: Access,
    ) -> Result<core::ops::Range<usize>, MemoryFault> {
        let fault = MemoryFault {
            address,
            length,
            access,
        };
        // Reject operations whose last byte wraps the guest's 32-bit address space.
        if length > 0 && (address.0 as u64 + length as u64 - 1 > u32::MAX as u64) {
            return Err(fault);
        }
        let start = address.0.checked_sub(self.base.0).ok_or(fault)? as usize;
        let end = start.checked_add(length).ok_or(fault)?;
        if end > self.bytes.len() {
            return Err(fault);
        }
        Ok(start..end)
    }
}
impl GuestMemory for MappedMemory<'_> {
    fn read(&mut self, address: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        let range = self.range(address, out.len(), Access::Read)?;
        out.copy_from_slice(&self.bytes[range]);
        Ok(())
    }
    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        let range = self.range(address, bytes.len(), Access::Write)?;
        self.bytes[range].copy_from_slice(bytes);
        Ok(())
    }
}

/// Ordered byte access contract used by the verified hybrid leaf ports.
pub mod leaf;

/// T1895 engine/memory ports (Tier 0).
pub mod w4m_arena;

/// T1908 opt-in shared allocator and bookkeeping boundary (Tier 0).
pub mod service_slice;
pub use service_slice::MemoryServices;

/// T2048 reclaimable pool allocator (0x3EA00, 0x3EAF0, 0x3EBD0, 0x3ED20).
pub mod pool;

/// T2048 fixed-stride guest freelist initializer (0x156DB0).
pub mod fixed_pool;

/// T2048 diagnostic shadow of live pool payloads (double free, stale-after-reuse).
pub mod pool_audit;

/// T2048 `GuestHeap`: one allocator boundary over arena, bump and pool.
pub mod heap;

/// Typed game-side allocator contract layered over the T2048 compatibility services.
pub mod contract;

/// Caller-specific CRT atexit-table growth and failure publication contract (T2365 revision 2).
pub mod crt_atexit;

/// CRT/XDK realloc payload move primitive (`0x3C9800`).
pub mod crt_memmove;

/// Native, generation-checked owner and pool-buffer lifecycle (T1673 attachment contract).
/// Compiled only when the explicit `native-owner-lifecycle` feature is selected.
#[cfg(feature = "native-owner-lifecycle")]
pub mod owner_lifecycle;

/// T2048 allocation ledgers (0x3E440..0x3E5A0) and init functions (0x3E650, 0x3E740).
pub mod ledger;

/// T2048 slice K, layer A: pure-Rust guest memory model (regions, low-4GB placement, first-fit
/// heap), port of `src/xbox/guest_mem.c`. Tier 1 against the C host.
pub mod guest_regions;

/// T2048 slice K, layer A: Mm*, Nt*VirtualMemory and Ex pool semantics over `GuestRegions`.
pub mod kernel_mm;

/// T2048 slice W: `game_alloc_*` wrapper constructors over the Rust ledgers and arena.
pub mod wrappers;

/// T2209 typed kernel runtime library, C-host differential evidence.
pub mod kernel_rtl;
