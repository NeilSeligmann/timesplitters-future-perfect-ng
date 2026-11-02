//! T2048 slice K, layer A, ordinal semantics on top of [`GuestRegions`]: the Mm*, Nt*VirtualMemory
//! and ExAllocatePoolWithTag/ExFreePool/ExQueryPoolBlockSize exports of
//! `src/xbox/kernel_memory.c` and `src/xbox/kernel_pool.c`, as plain value-in, value-out functions.
//!
//! Boundary: the C handlers read their arguments from the guest stack and read/write guest
//! out-parameters (`PVOID *BaseAddress`, `PULONG Size`, `MEMORY_BASIC_INFORMATION`). Here the
//! caller (the Rust host's call boundary) passes values and receives the out values, and decides
//! about `STATUS_ACCESS_VIOLATION` for an unreadable pointer. Out-values are `Some` exactly when
//! the C handler writes them. `MmQueryStatistics` takes an explicit host RAM snapshot;
//! Linux ABI handling and ordered, faultable guest writes remain at the host boundary.
//!
//! Evidence class: Tier 1 differential against the C handlers (`tests/memory_cdiff.rs` in
//! `tsfp-c-diff`). Log lines go to [`GuestRegions::log`], one entry per C `kernel_hle_log` call.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::guest_regions::{
    GuestRegions, HEAP_CHUNK_MIN, PAGE_SIZE, Placement, RegionRequest, Usage, nt,
};

/// Maximum live pool allocations tracked (`KERNEL_POOL_MAX`).
pub const POOL_MAX: u32 = 4096;
/// Length word plus eight output words (kernel_memory.h GUEST_MM_STATISTICS_SIZE).
pub const MM_STATISTICS_SIZE: u32 = 36;

/// `NtAllocateVirtualMemory` result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NtAlloc {
    pub status: u32,
    pub base: Option<u32>,
    pub size: Option<u32>,
}

/// `NtFreeVirtualMemory` result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NtFree {
    pub status: u32,
    pub base: Option<u32>,
    pub size: Option<u32>,
}

#[derive(Clone, Copy)]
struct PoolEntry {
    bytes: u32,
    tag: u32,
}

/// Layer A: regions, heaps and the kernel entry semantics, one object.
pub struct KernelMemory<P: Placement> {
    pub regions: GuestRegions<P>,
    locks: BTreeMap<u32, u32>,
    physical_refused: u32,
    pool_heap: u32,
    pool_slots: BTreeMap<u32, PoolEntry>,
    pool_by_address: HashMap<u32, u32>,
    pool_free_slots: BTreeSet<u32>,
    live_bytes: u64,
    bad_free: u32,
    debugger_thread_reported: bool,
}

impl<P: Placement> KernelMemory<P> {
    pub fn new(placement: P) -> Self {
        Self {
            regions: GuestRegions::new(placement),
            locks: BTreeMap::new(),
            physical_refused: 0,
            pool_heap: 0,
            pool_slots: BTreeMap::new(),
            pool_by_address: HashMap::new(),
            pool_free_slots: (0..POOL_MAX).collect(),
            live_bytes: 0,
            bad_free: 0,
            debugger_thread_reported: false,
        }
    }

    /// MmQueryStatistics (181): Length is input-only, and errors produce no writes.
    /// Host supplies total/free RAM bytes and mapped image bytes. Arithmetic wraps like
    /// C uint64_t; every field must fit before the caller starts ordered guest stores.
    pub fn mm_query_statistics(
        &self,
        length: u32,
        host_ram: Option<(u64, u64)>,
        image_bytes: u64,
    ) -> Result<[u32; 8], u32> {
        if length != MM_STATISTICS_SIZE {
            return Err(nt::INVALID_PARAMETER);
        }
        let (total, free) = host_ram.ok_or(nt::UNSUCCESSFUL)?;
        let memory = self.regions.statistics();
        let fields = [
            total / u64::from(PAGE_SIZE),
            free / u64::from(PAGE_SIZE),
            memory.virtual_committed_bytes.wrapping_add(image_bytes),
            memory.virtual_reserved_bytes.wrapping_add(image_bytes),
            memory.cache_pages,
            memory.pool_pages,
            memory.stack_pages,
            image_bytes / u64::from(PAGE_SIZE),
        ];
        let mut out = [0; 8];
        for (dst, field) in out.iter_mut().zip(fields) {
            *dst = u32::try_from(field).map_err(|_| nt::INSUFFICIENT_RESOURCES)?;
        }
        Ok(out)
    }

    fn log(&mut self, text: String) {
        self.regions.log.push(text);
    }

    /// `kernel_memory_register` alone: a fresh registration drops the page locks and the physical refusal counter
    /// (the C `lock_table_clear`), regions and pool are untouched.
    pub fn register(&mut self) {
        self.locks.clear();
        self.physical_refused = 0;
    }

    /// `kernel_memory_register` + `kernel_pool_reset` + `guest_mem_reset`: a fresh kernel.
    pub fn reset(&mut self) {
        self.locks.clear();
        self.physical_refused = 0;
        self.regions.reset();
        self.pool_reset();
    }

    // ---- contiguous memory --------------------------------------------------------------------

    fn contiguous_alloc(
        &mut self,
        bytes: u32,
        lowest: u32,
        highest: u32,
        alignment: u32,
        protect: u32,
    ) -> u32 {
        let request = RegionRequest {
            bytes,
            alignment,
            lowest_physical: lowest,
            highest_physical: highest,
            protect,
            state: nt::MEM_COMMIT,
            contiguous: true,
            fixed_base: 0,
        };
        match self.regions.region_alloc(&request) {
            Ok(address) => address,
            Err(status) => {
                self.log(format!(
                    "kernel: MmAllocateContiguousMemory({bytes}) failed, status {status:#x}"
                ));
                0
            }
        }
    }

    pub fn mm_allocate_contiguous_memory(&mut self, bytes: u32) -> u32 {
        self.contiguous_alloc(bytes, 0, 0, PAGE_SIZE, nt::PAGE_READWRITE)
    }

    pub fn mm_allocate_contiguous_memory_ex(
        &mut self,
        bytes: u32,
        lowest: u32,
        highest: u32,
        mut alignment: u32,
        mut protect: u32,
    ) -> u32 {
        if alignment == 0 {
            alignment = PAGE_SIZE;
        }
        if alignment & (alignment - 1) != 0 {
            self.log(format!("kernel: MmAllocateContiguousMemoryEx alignment {alignment:#x} is not a power of two"));
            return 0;
        }
        if protect == 0 {
            protect = nt::PAGE_READWRITE;
        }
        self.contiguous_alloc(bytes, lowest, highest, alignment, protect)
    }

    pub fn mm_free_contiguous_memory(&mut self, base: u32) {
        match self.regions.region_at(base) {
            Some(r) if r.contiguous => {
                self.regions.region_free(base);
            }
            _ => self.log(format!(
                "kernel: MmFreeContiguousMemory({base:#x}) is not a contiguous allocation base"
            )),
        }
    }

    pub fn mm_query_allocation_size(&self, base: u32) -> u32 {
        self.regions.region_at(base).map_or(0, |r| r.size)
    }

    pub fn mm_query_address_protect(&mut self, addr: u32) -> u32 {
        match self.regions.region_containing(addr) {
            Some(r) => r.protect,
            None => {
                self.log(format!("kernel: MmQueryAddressProtect({addr:#x}) addresses no tracked region, answering 0"));
                0
            }
        }
    }

    pub fn mm_get_physical_address(&self, addr: u32) -> u32 {
        self.regions.physical_address(addr)
    }

    pub fn mm_persist_contiguous_memory(&mut self, base: u32, _bytes: u32, persist: u32) {
        if !self.regions.region_set_persist(base, persist != 0) {
            self.log(format!(
                "kernel: MmPersistContiguousMemory({base:#x}) is not an allocation base"
            ));
        }
    }

    pub fn mm_set_address_protect(&mut self, base: u32, bytes: u32, protect: u32) {
        if !nt::page_protect_valid(protect) {
            self.log(format!(
                "kernel: MmSetAddressProtect({base:#x}) protection {protect:#x} is not valid"
            ));
            return;
        }
        if !self.regions.region_set_protect(base, bytes, protect) {
            self.log(format!(
                "kernel: MmSetAddressProtect({base:#x}) addresses no tracked region"
            ));
        }
    }

    // ---- page locks ---------------------------------------------------------------------------

    fn unlock_page(&mut self, page: u32) -> bool {
        match self.locks.get_mut(&page) {
            None => false,
            Some(depth) => {
                *depth -= 1;
                if *depth == 0 {
                    self.locks.remove(&page);
                }
                true
            }
        }
    }

    pub fn mm_lock_unlock_buffer_pages(&mut self, base: u32, bytes: u32, unlock: u32) {
        if unlock > 1 {
            self.log(format!("kernel: MmLockUnlockBufferPages({base:#x}, {bytes:#x}, {unlock:#x}) refused: bad UnlockPages"));
            return;
        }
        if bytes == 0 {
            return;
        }
        let end = base as u64 + bytes as u64;
        if end > 0x1_0000_0000 {
            self.log(format!("kernel: MmLockUnlockBufferPages({base:#x}, {bytes:#x}, {unlock}) refused: past 4 GB"));
            return;
        }
        let first_page = base / PAGE_SIZE;
        let last_page = ((end - 1) / PAGE_SIZE as u64) as u32;
        if unlock == 0 {
            let message = match self.regions.region_containing(base) {
                None => Some("locks a range outside any one tracked region, recording it anyway"),
                Some(r) if end > r.address as u64 + r.size as u64 => {
                    Some("locks a range outside any one tracked region, recording it anyway")
                }
                Some(r) if r.state != nt::MEM_COMMIT => {
                    Some("locks memory that is not committed, recording it anyway")
                }
                Some(_) => None,
            };
            if let Some(text) = message {
                self.log(format!(
                    "kernel: MmLockUnlockBufferPages({base:#x}, {bytes:#x}) {text}"
                ));
            }
        }
        let mut not_locked = 0u32;
        for page in first_page..=last_page {
            if unlock == 0 {
                *self.locks.entry(page).or_insert(0) += 1;
            } else if !self.unlock_page(page) {
                not_locked += 1;
            }
        }
        if not_locked != 0 {
            self.log(format!("kernel: MmLockUnlockBufferPages({base:#x}, {bytes:#x}) unlocks {not_locked} page(s) that were not locked"));
        }
    }

    pub fn mm_lock_unlock_physical_page(&mut self, physical: u32, unlock: u32) {
        if unlock != 1 {
            self.physical_refused += 1;
            self.log(format!(
                "kernel: MmLockUnlockPhysicalPage({physical:#x}, {unlock:#x}) refused"
            ));
            return;
        }
        let Some(address) = self.regions.virtual_from_physical(physical) else {
            self.physical_refused += 1;
            self.log(format!(
                "kernel: MmLockUnlockPhysicalPage({physical:#x}, 1) refused: no tracked region"
            ));
            return;
        };
        if !self.unlock_page(address / PAGE_SIZE) {
            self.log(format!("kernel: MmLockUnlockPhysicalPage({physical:#x}, 1) unlocks a page that was not locked"));
        }
    }

    /// Number of distinct locked pages (`kernel_memory_locked_page_total`).
    pub fn locked_page_total(&self) -> u32 {
        self.locks.len() as u32
    }
    /// Lock depth of the page holding `address` (`kernel_memory_page_lock_count`).
    pub fn page_lock_count(&self, address: u32) -> u32 {
        self.locks.get(&(address / PAGE_SIZE)).copied().unwrap_or(0)
    }
    /// Refusals of `MmLockUnlockPhysicalPage`.
    pub fn physical_refused_count(&self) -> u32 {
        self.physical_refused
    }

    // ---- kernel stacks ------------------------------------------------------------------------

    /// Returns the stack base (top of the usable range) or 0.
    pub fn mm_create_kernel_stack(&mut self, bytes: u32, debugger_thread: u32) -> u32 {
        if bytes == 0 {
            self.log("kernel: MmCreateKernelStack with NumberOfBytes 0 refused".into());
            return 0;
        }
        if bytes & (PAGE_SIZE - 1) != 0 {
            self.log(format!(
                "kernel: MmCreateKernelStack({bytes:#x}) is not a multiple of the page size"
            ));
        }
        if debugger_thread != 0 && !self.debugger_thread_reported {
            self.debugger_thread_reported = true;
            self.log(format!(
                "kernel: MmCreateKernelStack DebuggerThread {debugger_thread:#x} is not modelled"
            ));
        }
        let request = RegionRequest {
            bytes,
            protect: nt::PAGE_READWRITE,
            state: nt::MEM_COMMIT,
            ..RegionRequest::default()
        };
        match self.regions.region_alloc_typed(&request, Usage::Stack) {
            Ok(address) => address.wrapping_add(bytes),
            Err(status) => {
                self.log(format!(
                    "kernel: MmCreateKernelStack({bytes:#x}) failed, status {status:#x}"
                ));
                0
            }
        }
    }

    pub fn mm_delete_kernel_stack(&mut self, stack_base: u32, stack_limit: u32) {
        let matches = self
            .regions
            .region_at(stack_limit)
            .is_some_and(|r| r.size == stack_base.wrapping_sub(stack_limit));
        if stack_base == 0 || stack_base < stack_limit || !matches {
            self.log(format!("kernel: MmDeleteKernelStack({stack_base:#x}, {stack_limit:#x}) does not match a tracked stack"));
            return;
        }
        self.regions.region_free(stack_limit);
    }

    // ---- virtual memory -----------------------------------------------------------------------

    /// `NtAllocateVirtualMemory` with the in/out pointers already dereferenced (`*BaseAddress`,
    /// `*AllocationSize`).
    pub fn nt_allocate_virtual_memory(
        &mut self,
        base_in: u32,
        size_in: u32,
        allocation_type: u32,
        protect: u32,
    ) -> NtAlloc {
        let fail = |status| NtAlloc {
            status,
            base: None,
            size: None,
        };
        if size_in == 0 {
            return fail(nt::INVALID_PARAMETER);
        }
        if allocation_type & (nt::MEM_COMMIT | nt::MEM_RESERVE) == 0
            || allocation_type & !nt::MEM_ALLOCATION_TYPE_VALID != 0
        {
            return fail(nt::INVALID_PARAMETER);
        }
        if !nt::page_protect_valid(protect) {
            return fail(nt::INVALID_PAGE_PROTECTION);
        }
        if base_in != 0 && allocation_type & nt::MEM_COMMIT != 0 {
            if let Some(existing) = self.regions.region_at(base_in).copied() {
                if existing.state == nt::MEM_RESERVE {
                    self.regions.region_set_state(base_in, nt::MEM_COMMIT);
                    self.regions
                        .region_set_protect(base_in, existing.size, protect);
                    let size = self.regions.region_at(base_in).map_or(0, |r| r.size);
                    return NtAlloc {
                        status: nt::SUCCESS,
                        base: Some(base_in),
                        size: Some(size),
                    };
                }
            }
        }
        let request = RegionRequest {
            bytes: size_in,
            protect,
            state: allocation_type,
            fixed_base: base_in,
            ..RegionRequest::default()
        };
        match self.regions.region_alloc(&request) {
            Err(status) => fail(status),
            Ok(address) => {
                let size = self.regions.region_at(address).map_or(0, |r| r.size);
                NtAlloc {
                    status: nt::SUCCESS,
                    base: Some(address),
                    size: Some(size),
                }
            }
        }
    }

    /// Free contract (ordinal 199): validate flags before region lookup; require an
    /// exact allocation base, and zero size for release. Decommit retains the mapping,
    /// protection and generation and marks the entire region reserved, regardless of
    /// the supplied size. This preserves the C oracle's whole-region limitation.
    /// Successful calls return the original base and full region size; failures have
    /// no output writes. Guest pointer validation/write ordering belongs to the C ABI
    /// adapter; this typed model never dereferences guest output pointers.
    pub fn nt_free_virtual_memory(&mut self, base: u32, size: u32, free_type: u32) -> NtFree {
        let fail = |status| NtFree {
            status,
            base: None,
            size: None,
        };
        if free_type & nt::MEM_FREE_TYPE_VALID == 0 || free_type & !nt::MEM_FREE_TYPE_VALID != 0 {
            return fail(nt::INVALID_PARAMETER);
        }
        if free_type & nt::MEM_DECOMMIT != 0 && free_type & nt::MEM_RELEASE != 0 {
            return fail(nt::INVALID_PARAMETER);
        }
        let Some(exact) = self.regions.region_at(base).copied() else {
            return fail(if self.regions.region_containing(base).is_some() {
                nt::FREE_VM_NOT_AT_BASE
            } else {
                nt::MEMORY_NOT_ALLOCATED
            });
        };
        if free_type & nt::MEM_RELEASE != 0 {
            if size != 0 {
                return fail(nt::INVALID_PARAMETER);
            }
            if !self.regions.region_free(base) {
                return fail(nt::MEMORY_NOT_ALLOCATED);
            }
            return NtFree {
                status: nt::SUCCESS,
                base: Some(base),
                size: Some(exact.size),
            };
        }
        if !self.regions.region_set_state(base, nt::MEM_RESERVE) {
            return fail(nt::MEMORY_NOT_ALLOCATED);
        }
        NtFree {
            status: nt::SUCCESS,
            base: Some(base),
            size: Some(exact.size),
        }
    }

    /// `MEMORY_BASIC_INFORMATION` fields in guest order: BaseAddress, AllocationBase,
    /// AllocationProtect, RegionSize, State, Protect, Type.
    pub fn nt_query_virtual_memory(&self, address: u32) -> [u32; 7] {
        let page = address & !(PAGE_SIZE - 1);
        match self.regions.region_containing(address) {
            Some(r) => [
                page,
                r.address,
                r.alloc_protect,
                r.address.wrapping_add(r.size).wrapping_sub(page),
                r.state,
                r.protect,
                nt::MEM_PRIVATE,
            ],
            None => [page, 0, 0, 0, nt::MEM_FREE, nt::PAGE_NOACCESS, 0],
        }
    }

    // ---- executive pool -----------------------------------------------------------------------

    fn ensure_pool_heap(&mut self) -> u32 {
        if self.pool_heap != 0 && !self.regions.heap_valid(self.pool_heap) {
            let dropped = self.pool_slots.len();
            self.log(format!("kernel: pool heap {:#x} was destroyed underneath the pool ({dropped} allocations dropped)", self.pool_heap));
            self.pool_slots.clear();
            self.pool_by_address.clear();
            self.pool_free_slots = (0..POOL_MAX).collect();
            self.live_bytes = 0;
            self.pool_heap = 0;
        }
        if self.pool_heap == 0 {
            self.pool_heap = self
                .regions
                .heap_create_typed(0, HEAP_CHUNK_MIN, 0, Usage::Pool);
            if self.pool_heap == 0 {
                self.log("kernel: could not create the executive pool heap".into());
            }
        }
        self.pool_heap
    }

    pub fn ex_allocate_pool_with_tag(&mut self, bytes: u32, tag: u32) -> u32 {
        if bytes == 0 {
            self.log("kernel: ExAllocatePoolWithTag(0) -- zero-byte pool request refused".into());
            return 0;
        }
        let heap = self.ensure_pool_heap();
        if heap == 0 {
            return 0;
        }
        let Some(&slot) = self.pool_free_slots.iter().next() else {
            self.log(format!(
                "kernel: executive pool tracking table full at {POOL_MAX} entries"
            ));
            return 0;
        };
        let address = self.regions.heap_alloc(heap, bytes);
        if address == 0 {
            self.log(format!("kernel: ExAllocatePoolWithTag({bytes}) failed -- the pool heap could not satisfy it"));
            return 0;
        }
        self.pool_free_slots.remove(&slot);
        self.pool_slots.insert(slot, PoolEntry { bytes, tag });
        self.pool_by_address.insert(address, slot);
        self.live_bytes += bytes as u64;
        address
    }

    pub fn ex_free_pool(&mut self, address: u32) {
        if address == 0 {
            self.bad_free += 1;
            self.log("kernel: ExFreePool(NULL)".into());
            return;
        }
        let Some(&slot) = self.pool_by_address.get(&address) else {
            self.bad_free += 1;
            self.log(format!(
                "kernel: ExFreePool({address:#x}) -- the pool never issued this address"
            ));
            return;
        };
        let heap = self.pool_heap;
        if heap == 0 || !self.regions.heap_free(heap, address) {
            self.log(format!(
                "kernel: ExFreePool({address:#x}) -- the backing heap refuses to free it"
            ));
        }
        let entry = self.pool_slots.remove(&slot).expect("tracked slot");
        self.pool_by_address.remove(&address);
        self.pool_free_slots.insert(slot);
        self.live_bytes -= entry.bytes as u64;
    }

    /// Requested size of a pool block, 0 (reported) for an address the pool never issued.
    pub fn ex_query_pool_block_size(&mut self, address: u32) -> u32 {
        match self.pool_size_of(address) {
            Some(bytes) => bytes,
            None => {
                self.log(format!(
                    "kernel: ExQueryPoolBlockSize({address:#x}) -- never issued, reporting 0"
                ));
                0
            }
        }
    }

    pub fn pool_size_of(&self, address: u32) -> Option<u32> {
        let slot = self.pool_by_address.get(&address)?;
        self.pool_slots.get(slot).map(|e| e.bytes)
    }
    pub fn pool_tag_of(&self, address: u32) -> Option<u32> {
        let slot = self.pool_by_address.get(&address)?;
        self.pool_slots.get(slot).map(|e| e.tag)
    }
    /// Handle of the backing pool heap, 0 before first use.
    pub fn pool_heap_handle(&self) -> u32 {
        self.pool_heap
    }
    pub fn pool_live_count(&self) -> u32 {
        self.pool_slots.len() as u32
    }
    pub fn pool_live_bytes(&self) -> u64 {
        self.live_bytes
    }
    pub fn pool_bad_free_count(&self) -> u32 {
        self.bad_free
    }

    /// `kernel_pool_reset`.
    pub fn pool_reset(&mut self) {
        if self.pool_heap != 0 && self.regions.heap_valid(self.pool_heap) {
            self.regions.heap_destroy(self.pool_heap);
        }
        self.pool_heap = 0;
        self.pool_slots.clear();
        self.pool_by_address.clear();
        self.pool_free_slots = (0..POOL_MAX).collect();
        self.live_bytes = 0;
        self.bad_free = 0;
    }
}
