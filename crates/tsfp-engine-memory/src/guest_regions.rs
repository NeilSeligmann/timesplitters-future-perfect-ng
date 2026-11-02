//! T2048 slice K, layer A: `GuestRegions`, the pure-Rust model of the guest-visible kernel memory
//! (page allocator in the low 4 GB, region registry, synthetic physical addresses, boundary-tagged
//! first-fit heaps). It is a port of `src/xbox/guest_mem.c` (the C host), same return codes, same
//! quirks, same layout rules, with the host `mmap` replaced by the [`Placement`] trait so the Rust
//! host can later supply a real mapping backend (`docs/t-memory-replace.md`, section 8).
//!
//! Evidence class: Tier 1 differential against the C host implementation (not against the original
//! console), see `crates/tsfp-c-diff/tests/memory_cdiff.rs`. Names and policies are those of the C
//! host, which itself documents its gaps: protection is recorded and not enforced, physical
//! addresses are synthetic, no RAM budget.
//!
//! What the model owns: region bookkeeping, heap block headers and (for heap chunks only) the chunk
//! bytes, because the heap keeps its boundary tags in guest memory. Region payload bytes belong to
//! the [`Placement`] backend, so this crate never allocates 100 MB for a guest buffer.

use std::collections::BTreeMap;

/// Guest page size.
pub const PAGE_SIZE: u32 = 0x1000;
/// Reserve granularity (64 KB).
pub const ALLOCATION_GRANULARITY: u32 = 0x10000;
/// Zero readable slack mapped under every contiguous region (T434).
pub const CONTIGUOUS_LEAD_BYTES: u32 = PAGE_SIZE;
/// Smallest heap arena chunk.
pub const HEAP_CHUNK_MIN: u32 = 0x10000;
/// Heap payload alignment.
pub const HEAP_ALIGNMENT: u32 = 16;
/// Heap block header size.
pub const HEAP_HEADER_BYTES: u32 = 16;
/// Maximum live heaps.
pub const HEAP_MAX_SLOTS: usize = 256;
const HEAP_SLOT_BITS: u32 = 12;
const HEAP_SLOT_MASK: u32 = (1 << HEAP_SLOT_BITS) - 1;
const HEAP_BLOCK_IN_USE: u32 = 1;
const ADDRESS_LIMIT: u64 = 0x1_0000_0000;

/// NTSTATUS values used by layer A.
pub mod nt {
    pub const SUCCESS: u32 = 0;
    pub const UNSUCCESSFUL: u32 = 0xC000_0001;
    pub const INSUFFICIENT_RESOURCES: u32 = 0xC000_009A;
    pub const INVALID_PARAMETER: u32 = 0xC000_000D;
    pub const NO_MEMORY: u32 = 0xC000_0017;
    pub const CONFLICTING_ADDRESSES: u32 = 0xC000_0018;
    pub const INVALID_PAGE_PROTECTION: u32 = 0xC000_0045;
    pub const FREE_VM_NOT_AT_BASE: u32 = 0xC000_009F;
    pub const MEMORY_NOT_ALLOCATED: u32 = 0xC000_00A0;

    pub const MEM_COMMIT: u32 = 0x1000;
    pub const MEM_RESERVE: u32 = 0x2000;
    pub const MEM_DECOMMIT: u32 = 0x4000;
    pub const MEM_RELEASE: u32 = 0x8000;
    pub const MEM_FREE: u32 = 0x10000;
    pub const MEM_PRIVATE: u32 = 0x20000;
    pub const MEM_RESET: u32 = 0x80000;
    pub const MEM_TOP_DOWN: u32 = 0x100000;
    pub const MEM_NOZERO: u32 = 0x800000;
    pub const MEM_ALLOCATION_TYPE_VALID: u32 =
        MEM_COMMIT | MEM_RESERVE | MEM_TOP_DOWN | MEM_NOZERO | MEM_RESET;
    pub const MEM_FREE_TYPE_VALID: u32 = MEM_DECOMMIT | MEM_RELEASE;

    pub const PAGE_NOACCESS: u32 = 1;
    pub const PAGE_READONLY: u32 = 2;
    pub const PAGE_READWRITE: u32 = 4;
    pub const PAGE_WRITECOPY: u32 = 8;
    pub const PAGE_EXECUTE: u32 = 0x10;
    pub const PAGE_EXECUTE_READ: u32 = 0x20;
    pub const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    pub const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
    pub const PAGE_GUARD: u32 = 0x100;
    pub const PAGE_NOCACHE: u32 = 0x200;
    pub const PAGE_WRITECOMBINE: u32 = 0x400;
    const ACCESS: u32 = PAGE_NOACCESS
        | PAGE_READONLY
        | PAGE_READWRITE
        | PAGE_WRITECOPY
        | PAGE_EXECUTE
        | PAGE_EXECUTE_READ
        | PAGE_EXECUTE_READWRITE
        | PAGE_EXECUTE_WRITECOPY;
    const MODIFIER: u32 = PAGE_GUARD | PAGE_NOCACHE | PAGE_WRITECOMBINE;

    /// Exactly one access mode plus optional modifiers.
    pub fn page_protect_valid(protect: u32) -> bool {
        let access = protect & ACCESS;
        access != 0 && access & (access - 1) == 0 && protect & !(ACCESS | MODIFIER) == 0
    }
}

/// Where host pages for the guest come from. The C host uses `mmap` (`MAP_32BIT`, then hinted,
/// `MAP_FIXED_NOREPLACE` for a fixed base). The Rust host supplies its own backend; the model only
/// needs a base below 4 GB and to be told when the range is released.
pub trait Placement {
    /// Map `length` bytes, at exactly `fixed` when given. `None` is a refusal.
    fn map(&mut self, length: u64, fixed: Option<u32>) -> Option<u32>;
    /// Release a mapping returned by [`Placement::map`].
    fn unmap(&mut self, base: u32, length: u64);
    /// Live binding (T2048 8.4 step 2): the model mirrors every heap boundary-tag write into guest memory.
    /// A backend with real bytes behind its mappings stores them; the default drops them (the model keeps its
    /// own copy, which tests and the differential read).
    fn write_guest(&mut self, _addr: u32, _data: &[u8]) {}
    /// Copy `len` payload bytes inside guest memory (a moving realloc). Default: nothing to copy.
    fn copy_guest(&mut self, _dst: u32, _src: u32, _len: u32) {}
    /// Read `out.len()` bytes of guest memory the backend owns. `false` when the range is not backed.
    fn read_guest(&self, _addr: u32, _out: &mut [u8]) -> bool {
        false
    }
    /// True when the backend keeps real bytes behind every mapping (reads and writes above work for the
    /// whole mapping). Then the heap keeps NO Vec copy of its chunks: the guest memory is the only store
    /// (T2048 8.5.2). The default `false` keeps the model-owned copy (tests, the C differential, compare mode).
    fn backs_guest(&self) -> bool {
        false
    }
}

/// Pure-Rust placement: first fit inside `[low, high)`, page granular. No bytes are backed, it is
/// the address policy only (a real backend would also reserve host memory).
/// Requests without a fixed base use the first gap in the window; lengths round up to pages.
/// A fixed base is checked for alignment and overlap, but is not limited to the window.
///
/// ```
/// use tsfp_engine_memory::guest_regions::{Placement, WindowPlacement};
/// let mut pages = WindowPlacement::new(0x1000_0000, 0x1000_4000);
/// let first = pages.map(1, None).unwrap();
/// assert_eq!(first, 0x1000_0000);
/// pages.unmap(first, 1);
/// assert_eq!(pages.map(0x1000, None), Some(first));
/// ```
pub struct WindowPlacement {
    low: u64,
    high: u64,
    mapped: BTreeMap<u32, u64>,
}

impl WindowPlacement {
    /// Placement window `[low, high)`, both page aligned.
    pub fn new(low: u32, high: u64) -> Self {
        Self {
            low: low as u64,
            high: high.min(ADDRESS_LIMIT),
            mapped: BTreeMap::new(),
        }
    }
    /// The default window the C host effectively uses (`MAP_32BIT`: 1 GB .. 2 GB hint, then up).
    pub fn default_window() -> Self {
        Self::new(0x4000_0000, 0xF000_0000)
    }
    /// Length of the live mapping based at `base` (page rounded), for tests.
    pub fn map_len_for_test(&self, base: u32) -> Option<u64> {
        self.mapped.get(&base).copied()
    }
    fn overlaps(&self, base: u64, length: u64) -> bool {
        self.mapped
            .iter()
            .any(|(&b, &l)| (b as u64) < base + length && base < b as u64 + l)
    }
}

impl Placement for WindowPlacement {
    fn map(&mut self, length: u64, fixed: Option<u32>) -> Option<u32> {
        if length == 0 {
            return None;
        }
        let length = (length + PAGE_SIZE as u64 - 1) & !(PAGE_SIZE as u64 - 1);
        if let Some(base) = fixed {
            if base % PAGE_SIZE != 0
                || base as u64 + length > ADDRESS_LIMIT
                || self.overlaps(base as u64, length)
            {
                return None;
            }
            self.mapped.insert(base, length);
            return Some(base);
        }
        let mut cursor = self.low;
        for (&b, &l) in &self.mapped {
            if (b as u64) >= cursor + length {
                break;
            }
            if b as u64 + l > cursor {
                cursor = b as u64 + l;
            }
        }
        if cursor + length > self.high {
            return None;
        }
        self.mapped.insert(cursor as u32, length);
        Some(cursor as u32)
    }

    fn unmap(&mut self, base: u32, _length: u64) {
        self.mapped.remove(&base);
    }
}

/// How the usage counters classify a region or heap (`guest_memory_usage`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Usage {
    #[default]
    Virtual = 0,
    Pool = 1,
    Stack = 2,
    Cache = 3,
}

/// What to allocate (`guest_region_request`).
#[derive(Clone, Copy, Debug, Default)]
pub struct RegionRequest {
    pub bytes: u32,
    pub alignment: u32,
    pub lowest_physical: u32,
    pub highest_physical: u32,
    pub protect: u32,
    pub state: u32,
    pub contiguous: bool,
    pub fixed_base: u32,
}

/// A tracked guest allocation (`guest_region`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub address: u32,
    pub size: u32,
    pub requested: u32,
    pub physical: u32,
    pub protect: u32,
    pub alloc_protect: u32,
    pub state: u32,
    pub contiguous: bool,
    pub persist: bool,
    pub generation: u64,
}

#[derive(Clone, Copy)]
struct RegionSlot {
    info: Region,
    usage: Usage,
    map_base: u32,
    map_length: u64,
}

/// Usage counters (`guest_memory_statistics`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    pub virtual_committed_bytes: u64,
    pub virtual_reserved_bytes: u64,
    pub pool_pages: u64,
    pub stack_pages: u64,
    pub cache_pages: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Header {
    size: u32,
    prev_size: u32,
    requested: u32,
    flags: u32,
}

struct Chunk {
    base: u32,
    length: u32,
    map_base: u32,
    map_length: u64,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct HeapSlot {
    token: u32,
    generation: u32,
    #[allow(dead_code)]
    flags: u32,
    maximum_size: u32,
    committed: u32,
    usage: Usage,
    chunks: Vec<Chunk>,
    in_use: bool,
}

fn is_pow2(value: u32) -> bool {
    value != 0 && value & (value - 1) == 0
}

fn align_up(value: u32, alignment: u32) -> Option<u32> {
    if !is_pow2(alignment) {
        return None;
    }
    let mask = alignment - 1;
    if value > u32::MAX - mask {
        None
    } else {
        Some((value + mask) & !mask)
    }
}

fn align_down(value: u32, alignment: u32) -> u32 {
    value & !(alignment - 1)
}

/// The pure-Rust guest memory model.
pub struct GuestRegions<P: Placement> {
    placement: P,
    mapped_bytes: u64,
    physical_cursor: u64,
    allocation_generation: u64,
    regions: Vec<Option<RegionSlot>>,
    region_live: usize,
    heaps: Vec<HeapSlot>,
    heap_live: usize,
    /// Diagnostics the C host sends through `kernel_hle_log` (text is informative, count is
    /// compared by the differential).
    pub log: Vec<String>,
}

impl<P: Placement> GuestRegions<P> {
    pub fn new(placement: P) -> Self {
        let mut heaps = Vec::with_capacity(HEAP_MAX_SLOTS);
        heaps.resize_with(HEAP_MAX_SLOTS, HeapSlot::default);
        Self {
            placement,
            mapped_bytes: 0,
            physical_cursor: PAGE_SIZE as u64,
            allocation_generation: 0,
            regions: Vec::new(),
            region_live: 0,
            heaps,
            heap_live: 0,
            log: Vec::new(),
        }
    }

    pub fn placement(&self) -> &P {
        &self.placement
    }
    pub fn placement_mut(&mut self) -> &mut P {
        &mut self.placement
    }

    // ---- low-4 GB page allocator ------------------------------------------------------------

    fn lowmem_map(&mut self, length: u64, fixed_base: u32) -> u32 {
        if length == 0 {
            return 0;
        }
        if fixed_base != 0 {
            if fixed_base as u64 + length > ADDRESS_LIMIT {
                return 0;
            }
            return match self.placement.map(length, Some(fixed_base)) {
                Some(got) if got == fixed_base => {
                    self.mapped_bytes += length;
                    fixed_base
                }
                Some(got) => {
                    self.placement.unmap(got, length);
                    0
                }
                None => 0,
            };
        }
        if let Some(base) = self.placement.map(length, None) {
            if base as u64 + length <= ADDRESS_LIMIT {
                self.mapped_bytes += length;
                return base;
            }
            self.placement.unmap(base, length);
        }
        self.log.push(format!(
            "guest_mem: FAILED to place {length} bytes below 4 GB"
        ));
        0
    }

    fn lowmem_unmap(&mut self, base: u32, length: u64) {
        if base == 0 || length == 0 {
            return;
        }
        self.placement.unmap(base, length);
        if self.mapped_bytes >= length {
            self.mapped_bytes -= length;
        }
    }

    /// Returns `(address, size, map_base, map_length)`.
    fn lowmem_alloc(
        &mut self,
        bytes: u32,
        granularity: u32,
        alignment: u32,
        lead_bytes: u32,
        fixed_base: u32,
    ) -> Option<(u32, u32, u32, u64)> {
        let size = align_up(bytes, granularity)?;
        if size == 0 {
            return None;
        }
        let mut map_length = size as u64;
        if alignment > PAGE_SIZE {
            map_length += (alignment - PAGE_SIZE) as u64;
        }
        let lead = if fixed_base == 0 {
            lead_bytes as u64
        } else {
            0
        };
        let lead_base = self.lowmem_map(map_length + lead, fixed_base);
        if lead_base == 0 {
            return None;
        }
        let map_base = lead_base.wrapping_add(lead as u32);
        let address = match align_up(map_base, alignment) {
            Some(a) => a,
            None => {
                self.lowmem_unmap(lead_base, map_length + lead);
                return None;
            }
        };
        if address as u64 + size as u64 > map_base as u64 + map_length
            || (fixed_base != 0 && address != fixed_base)
        {
            self.lowmem_unmap(lead_base, map_length + lead);
            return None;
        }
        Some((address, size, lead_base, map_length + lead))
    }

    // ---- synthetic physical addresses ---------------------------------------------------------

    fn physical_find(&self, size: u32, start: u64, top: u32, alignment: u32) -> Option<u32> {
        let mask = alignment as u64 - 1;
        let mut candidate = (start + mask) & !mask;
        while candidate + size as u64 <= top as u64 + 1 {
            let mut next = candidate;
            for slot in self.regions.iter().flatten() {
                let r = &slot.info;
                let end = r.physical as u64 + r.size as u64;
                if candidate < end && (r.physical as u64) < candidate + size as u64 && end > next {
                    next = end;
                }
            }
            if next == candidate {
                return Some(candidate as u32);
            }
            candidate = (next + mask) & !mask;
        }
        None
    }

    fn physical_reserve(
        &mut self,
        size: u32,
        lowest: u32,
        highest: u32,
        alignment: u32,
    ) -> Option<u32> {
        let align = alignment.max(PAGE_SIZE);
        let top = if highest == 0 { u32::MAX } else { highest };
        let floor = (lowest as u64).max(PAGE_SIZE as u64);
        if size == 0 || floor > top as u64 {
            return None;
        }
        let preferred = if top == u32::MAX && self.physical_cursor > floor {
            self.physical_cursor
        } else {
            floor
        };
        let physical = match self.physical_find(size, preferred, top, align) {
            Some(p) => p,
            None if preferred != floor => self.physical_find(size, floor, top, align)?,
            None => return None,
        };
        let end = physical as u64 + size as u64;
        if self.physical_cursor < end {
            self.physical_cursor = end;
        }
        Some(physical)
    }

    // ---- region registry ----------------------------------------------------------------------

    /// Allocate a region. `Err` carries the NTSTATUS, as the C `*status` out-param does.
    pub fn region_alloc(&mut self, request: &RegionRequest) -> Result<u32, u32> {
        if request.bytes == 0 {
            return Err(nt::INVALID_PARAMETER);
        }
        if !nt::page_protect_valid(request.protect) {
            return Err(nt::INVALID_PAGE_PROTECTION);
        }
        if request.state & (nt::MEM_COMMIT | nt::MEM_RESERVE) == 0 {
            return Err(nt::INVALID_PARAMETER);
        }
        if request.alignment != 0 && !is_pow2(request.alignment) {
            return Err(nt::INVALID_PARAMETER);
        }
        if request.highest_physical != 0 && request.lowest_physical > request.highest_physical {
            return Err(nt::INVALID_PARAMETER);
        }
        let granularity = if request.state & nt::MEM_RESERVE != 0 {
            ALLOCATION_GRANULARITY
        } else {
            PAGE_SIZE
        };
        let mut alignment = if request.alignment != 0 {
            request.alignment
        } else {
            granularity
        };
        if alignment < granularity {
            alignment = granularity;
        }
        let mut fixed_base = request.fixed_base;
        if fixed_base != 0 {
            fixed_base = align_down(fixed_base, granularity);
            if fixed_base == 0 {
                return Err(nt::INVALID_PARAMETER);
            }
            if fixed_base & (alignment - 1) != 0 {
                return Err(nt::CONFLICTING_ADDRESSES);
            }
        }
        if self.allocation_generation == u64::MAX {
            self.log
                .push("guest_mem: allocation generation exhausted".into());
            return Err(nt::NO_MEMORY);
        }
        let lead = if request.contiguous {
            CONTIGUOUS_LEAD_BYTES
        } else {
            0
        };
        let (address, size, map_base, map_length) =
            match self.lowmem_alloc(request.bytes, granularity, alignment, lead, fixed_base) {
                Some(v) => v,
                None => {
                    return Err(if fixed_base != 0 {
                        nt::CONFLICTING_ADDRESSES
                    } else {
                        nt::NO_MEMORY
                    });
                }
            };
        let Some(physical) = self.physical_reserve(
            size,
            request.lowest_physical,
            request.highest_physical,
            alignment,
        ) else {
            self.lowmem_unmap(map_base, map_length);
            self.log.push(format!(
                "guest_mem: no synthetic physical range for {size} bytes"
            ));
            return Err(nt::NO_MEMORY);
        };
        self.allocation_generation += 1;
        let slot = RegionSlot {
            info: Region {
                address,
                size,
                requested: request.bytes,
                physical,
                protect: request.protect,
                alloc_protect: request.protect,
                state: if request.state & nt::MEM_COMMIT != 0 {
                    nt::MEM_COMMIT
                } else {
                    nt::MEM_RESERVE
                },
                contiguous: request.contiguous,
                persist: false,
                generation: self.allocation_generation,
            },
            usage: Usage::Virtual,
            map_base,
            map_length,
        };
        let index = self
            .regions
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                self.regions.push(None);
                self.regions.len() - 1
            });
        self.regions[index] = Some(slot);
        self.region_live += 1;
        Ok(address)
    }

    /// `guest_region_alloc_typed`: allocation and usage classification in one step.
    pub fn region_alloc_typed(
        &mut self,
        request: &RegionRequest,
        usage: Usage,
    ) -> Result<u32, u32> {
        let address = self.region_alloc(request)?;
        if let Some(slot) = self.slot_at_mut(address) {
            slot.usage = usage;
        }
        Ok(address)
    }

    fn slot_at(&self, base: u32) -> Option<&RegionSlot> {
        self.regions
            .iter()
            .flatten()
            .find(|s| s.info.address == base)
    }
    fn slot_at_mut(&mut self, base: u32) -> Option<&mut RegionSlot> {
        self.regions
            .iter_mut()
            .flatten()
            .find(|s| s.info.address == base)
    }
    fn slot_containing_mut(&mut self, addr: u32) -> Option<&mut RegionSlot> {
        self.regions.iter_mut().flatten().find(|s| {
            addr >= s.info.address && (addr as u64) < s.info.address as u64 + s.info.size as u64
        })
    }

    /// Region based exactly at `base`.
    pub fn region_at(&self, base: u32) -> Option<&Region> {
        self.slot_at(base).map(|s| &s.info)
    }

    /// Region containing `addr`.
    pub fn region_containing(&self, addr: u32) -> Option<&Region> {
        self.regions
            .iter()
            .flatten()
            .find(|s| {
                addr >= s.info.address && (addr as u64) < s.info.address as u64 + s.info.size as u64
            })
            .map(|s| &s.info)
    }

    /// Release a region by its exact base.
    pub fn region_free(&mut self, base: u32) -> bool {
        let Some(index) = self
            .regions
            .iter()
            .position(|s| s.is_some_and(|s| s.info.address == base))
        else {
            return false;
        };
        let slot = self.regions[index].take().expect("slot found above");
        self.lowmem_unmap(slot.map_base, slot.map_length);
        self.region_live = self.region_live.saturating_sub(1);
        true
    }

    /// Record a protection on the region containing `base` (range granularity is not modelled).
    pub fn region_set_protect(&mut self, base: u32, _bytes: u32, protect: u32) -> bool {
        match self.slot_containing_mut(base) {
            Some(slot) => {
                slot.info.protect = protect;
                true
            }
            None => false,
        }
    }

    pub fn region_set_persist(&mut self, base: u32, persist: bool) -> bool {
        match self.slot_at_mut(base) {
            Some(slot) => {
                slot.info.persist = persist;
                true
            }
            None => false,
        }
    }

    pub fn region_set_state(&mut self, base: u32, state: u32) -> bool {
        match self.slot_at_mut(base) {
            Some(slot) => {
                slot.info.state = if state & nt::MEM_COMMIT != 0 {
                    nt::MEM_COMMIT
                } else {
                    nt::MEM_RESERVE
                };
                true
            }
            None => false,
        }
    }

    /// Classify a complete non-contiguous mapping (`guest_region_set_usage`).
    pub fn region_set_usage(&mut self, base: u32, bytes: u32, usage: Usage) -> bool {
        match self.slot_at_mut(base) {
            Some(slot) if slot.info.size == bytes && !slot.info.contiguous => {
                slot.usage = usage;
                true
            }
            _ => false,
        }
    }

    /// Synthetic physical address, linear within a region, 0 for an untracked address.
    pub fn physical_address(&self, addr: u32) -> u32 {
        match self.region_containing(addr) {
            Some(r) => r.physical.wrapping_add(addr - r.address),
            None => 0,
        }
    }

    /// Lifetime identity of the region containing `addr`, 0 if untracked.
    pub fn allocation_generation(&self, addr: u32) -> u64 {
        self.region_containing(addr).map_or(0, |r| r.generation)
    }

    /// Inverse of [`GuestRegions::physical_address`].
    pub fn virtual_from_physical(&self, physical: u32) -> Option<u32> {
        self.regions.iter().flatten().find_map(|s| {
            let r = &s.info;
            (physical >= r.physical && (physical as u64) < r.physical as u64 + r.size as u64)
                .then(|| r.address.wrapping_add(physical - r.physical))
        })
    }

    /// Live regions in slot order (slot reuse is first-free, as in the C registry).
    pub fn regions(&self) -> impl Iterator<Item = &Region> {
        self.regions.iter().flatten().map(|s| &s.info)
    }

    // ---- heaps --------------------------------------------------------------------------------

    fn heap_index(&self, handle: u32) -> Option<usize> {
        if handle == 0 {
            return None;
        }
        let slot_index = (handle & HEAP_SLOT_MASK) as usize;
        if slot_index == 0 || slot_index > HEAP_MAX_SLOTS {
            return None;
        }
        let heap = &self.heaps[slot_index - 1];
        (heap.in_use && heap.token == handle).then_some(slot_index - 1)
    }

    pub fn heap_valid(&self, handle: u32) -> bool {
        self.heap_index(handle).is_some()
    }

    /// Read `out.len()` bytes at `offset` inside a chunk, from the backend memory when it backs the guest, else
    /// from the model's own copy. `None` when the range leaves the chunk.
    fn chunk_read(&self, h: usize, c: usize, offset: u64, out: &mut [u8]) -> Option<()> {
        let chunk = &self.heaps[h].chunks[c];
        let end = offset.checked_add(out.len() as u64)?;
        if end > chunk.length as u64 {
            return None;
        }
        if self.placement.backs_guest() {
            let at = chunk.base.wrapping_add(offset as u32);
            return self.placement.read_guest(at, out).then_some(());
        }
        out.copy_from_slice(&chunk.bytes[offset as usize..end as usize]);
        Some(())
    }

    fn block_read(&self, h: usize, c: usize, offset: u64) -> Option<Header> {
        let mut b = [0u8; HEAP_HEADER_BYTES as usize];
        self.chunk_read(h, c, offset, &mut b)?;
        let word = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        Some(Header {
            size: word(0),
            prev_size: word(4),
            requested: word(8),
            flags: word(12),
        })
    }

    fn block_write(&mut self, h: usize, c: usize, offset: u64, header: &Header) -> bool {
        let chunk = &mut self.heaps[h].chunks[c];
        let Some(end) = offset.checked_add(HEAP_HEADER_BYTES as u64) else {
            return false;
        };
        if end > chunk.length as u64 {
            return false;
        }
        let mut raw = [0u8; 16];
        raw[0..4].copy_from_slice(&header.size.to_le_bytes());
        raw[4..8].copy_from_slice(&header.prev_size.to_le_bytes());
        raw[8..12].copy_from_slice(&header.requested.to_le_bytes());
        raw[12..16].copy_from_slice(&header.flags.to_le_bytes());
        if !chunk.bytes.is_empty() {
            chunk.bytes[offset as usize..end as usize].copy_from_slice(&raw);
        }
        let at = chunk.base.wrapping_add(offset as u32);
        self.placement.write_guest(at, &raw);
        true
    }

    fn heap_grow(&mut self, h: usize, need: u32, must_fit: bool) -> bool {
        let Some(mut wanted) = align_up(need, HEAP_HEADER_BYTES) else {
            return false;
        };
        if wanted > u32::MAX - HEAP_HEADER_BYTES {
            return false;
        }
        wanted += HEAP_HEADER_BYTES;
        if wanted < HEAP_CHUNK_MIN {
            wanted = HEAP_CHUNK_MIN;
        }
        let Some(mut chunk_bytes) = align_up(wanted, PAGE_SIZE) else {
            return false;
        };
        let (maximum, committed) = (self.heaps[h].maximum_size, self.heaps[h].committed);
        if maximum != 0 {
            if committed >= maximum {
                return false;
            }
            let budget = maximum - committed;
            if chunk_bytes > budget {
                let reduced = align_down(budget, PAGE_SIZE);
                if must_fit {
                    if (reduced as u64) < need as u64 + HEAP_HEADER_BYTES as u64 {
                        return false;
                    }
                } else if reduced < HEAP_HEADER_BYTES + HEAP_ALIGNMENT {
                    return false;
                }
                chunk_bytes = reduced;
            }
        }
        let Some((address, size, map_base, map_length)) =
            self.lowmem_alloc(chunk_bytes, PAGE_SIZE, HEAP_ALIGNMENT, 0, 0)
        else {
            return false;
        };
        // A backend that backs the guest is the only store (fresh anonymous mappings are zero); otherwise keep a copy.
        let mut bytes = if self.placement.backs_guest() {
            Vec::new()
        } else {
            vec![0u8; size as usize]
        };
        let first = Header {
            size: size - HEAP_HEADER_BYTES,
            prev_size: 0,
            requested: 0,
            flags: 0,
        };
        if !bytes.is_empty() {
            bytes[0..4].copy_from_slice(&first.size.to_le_bytes());
        }
        self.placement
            .write_guest(address, &first.size.to_le_bytes());
        let heap = &mut self.heaps[h];
        heap.chunks.push(Chunk {
            base: address,
            length: size,
            map_base,
            map_length,
            bytes,
        });
        heap.committed += size;
        true
    }

    /// Create a heap. 0 on failure. `maximum_size` 0 is growable.
    pub fn heap_create(&mut self, flags: u32, initial_size: u32, maximum_size: u32) -> u32 {
        if maximum_size != 0 && initial_size > maximum_size {
            return 0;
        }
        let Some(h) = self.heaps.iter().position(|s| !s.in_use) else {
            self.log.push(format!(
                "guest_mem: all {HEAP_MAX_SLOTS} heap slots are in use"
            ));
            return 0;
        };
        let slot_index = h as u32 + 1;
        let generation = self.heaps[h].generation + 1;
        if generation > u32::MAX >> HEAP_SLOT_BITS {
            self.log.push(format!(
                "guest_mem: heap slot {slot_index} has exhausted its handle generations"
            ));
            return 0;
        }
        let keep = std::mem::take(&mut self.heaps[h].chunks);
        self.heaps[h] = HeapSlot {
            token: (generation << HEAP_SLOT_BITS) | slot_index,
            generation,
            flags,
            maximum_size,
            committed: 0,
            usage: Usage::Virtual,
            chunks: keep,
            in_use: true,
        };
        self.heaps[h].chunks.clear();
        if initial_size != 0 && !self.heap_grow(h, initial_size, false) {
            self.heaps[h].in_use = false;
            self.heaps[h].token = 0;
            return 0;
        }
        self.heap_live += 1;
        self.heaps[h].token
    }

    /// `guest_heap_create_typed`.
    pub fn heap_create_typed(
        &mut self,
        flags: u32,
        initial: u32,
        maximum: u32,
        usage: Usage,
    ) -> u32 {
        let handle = self.heap_create(flags, initial, maximum);
        if let Some(h) = self.heap_index(handle) {
            self.heaps[h].usage = usage;
        }
        handle
    }

    fn heap_release_chunks(&mut self, h: usize) {
        let chunks = std::mem::take(&mut self.heaps[h].chunks);
        for chunk in &chunks {
            self.lowmem_unmap(chunk.map_base, chunk.map_length);
        }
        self.heaps[h].committed = 0;
    }

    pub fn heap_destroy(&mut self, handle: u32) -> bool {
        let Some(h) = self.heap_index(handle) else {
            return false;
        };
        self.heap_release_chunks(h);
        let heap = &mut self.heaps[h];
        heap.in_use = false;
        heap.token = 0;
        heap.maximum_size = 0;
        heap.flags = 0;
        self.heap_live = self.heap_live.saturating_sub(1);
        true
    }

    fn heap_fix_following(&mut self, h: usize, c: usize, offset: u32, size: u32) -> bool {
        let following = offset as u64 + HEAP_HEADER_BYTES as u64 + size as u64;
        if following >= self.heaps[h].chunks[c].length as u64 {
            return true;
        }
        let Some(mut header) = self.block_read(h, c, following) else {
            return false;
        };
        header.prev_size = size;
        self.block_write(h, c, following, &header)
    }

    fn heap_alloc_from_chunk(&mut self, h: usize, c: usize, need: u32, requested: u32) -> u32 {
        let (base, length) = (self.heaps[h].chunks[c].base, self.heaps[h].chunks[c].length);
        let mut offset: u32 = 0;
        while offset < length {
            let Some(mut header) = self.block_read(h, c, offset as u64) else {
                return 0;
            };
            let at = base.wrapping_add(offset);
            if header.size == 0 {
                self.log.push(format!(
                    "guest_mem: heap block at {at:#x} has zero size -- arena corrupt"
                ));
                return 0;
            }
            if header.flags & HEAP_BLOCK_IN_USE == 0 && header.size >= need {
                let remainder = header.size - need;
                if remainder >= HEAP_HEADER_BYTES + HEAP_ALIGNMENT {
                    let tail_offset = offset.wrapping_add(HEAP_HEADER_BYTES).wrapping_add(need);
                    let tail_size = remainder - HEAP_HEADER_BYTES;
                    let tail = Header {
                        size: tail_size,
                        prev_size: need,
                        requested: 0,
                        flags: 0,
                    };
                    if !self.block_write(h, c, tail_offset as u64, &tail) {
                        return 0;
                    }
                    if !self.heap_fix_following(h, c, tail_offset, tail_size) {
                        return 0;
                    }
                    header.size = need;
                }
                header.flags |= HEAP_BLOCK_IN_USE;
                header.requested = requested;
                if !self.block_write(h, c, offset as u64, &header) {
                    return 0;
                }
                return at.wrapping_add(HEAP_HEADER_BYTES);
            }
            offset = offset
                .wrapping_add(HEAP_HEADER_BYTES)
                .wrapping_add(header.size);
        }
        0
    }

    /// First-fit allocation. 0 on failure.
    pub fn heap_alloc(&mut self, handle: u32, bytes: u32) -> u32 {
        let Some(h) = self.heap_index(handle) else {
            return 0;
        };
        let Some(need) = align_up(if bytes == 0 { 1 } else { bytes }, HEAP_ALIGNMENT) else {
            return 0;
        };
        for c in 0..self.heaps[h].chunks.len() {
            let got = self.heap_alloc_from_chunk(h, c, need, bytes);
            if got != 0 {
                return got;
            }
        }
        if !self.heap_grow(h, need, true) {
            return 0;
        }
        let last = self.heaps[h].chunks.len() - 1;
        self.heap_alloc_from_chunk(h, last, need, bytes)
    }

    /// Chunk index and header offset of the block `ptr` in heap `h` (isolation between heaps).
    fn heap_find_block(&self, h: usize, ptr: u32) -> Option<(usize, u32)> {
        if ptr < HEAP_HEADER_BYTES {
            return None;
        }
        let header_at = ptr - HEAP_HEADER_BYTES;
        for (c, chunk) in self.heaps[h].chunks.iter().enumerate() {
            if header_at < chunk.base {
                continue;
            }
            let offset = header_at - chunk.base;
            if offset as u64 + HEAP_HEADER_BYTES as u64 > chunk.length as u64 {
                continue;
            }
            if offset % HEAP_ALIGNMENT != 0 {
                continue;
            }
            return Some((c, offset));
        }
        None
    }

    /// The size originally requested for a block.
    pub fn heap_block_size(&self, handle: u32, ptr: u32) -> Option<u32> {
        let h = self.heap_index(handle)?;
        let (c, offset) = self.heap_find_block(h, ptr)?;
        let header = self.block_read(h, c, offset as u64)?;
        (header.flags & HEAP_BLOCK_IN_USE != 0).then_some(header.requested)
    }

    /// Free a block (coalesces forward then backward, same write order as the C host).
    pub fn heap_free(&mut self, handle: u32, ptr: u32) -> bool {
        let Some(h) = self.heap_index(handle) else {
            return false;
        };
        let Some((c, offset)) = self.heap_find_block(h, ptr) else {
            return false;
        };
        let at = self.heaps[h].chunks[c].base.wrapping_add(offset);
        let Some(mut header) = self.block_read(h, c, offset as u64) else {
            return false;
        };
        if header.flags & HEAP_BLOCK_IN_USE == 0 {
            self.log
                .push(format!("guest_mem: double free of heap block {ptr:#x}"));
            return false;
        }
        header.flags &= !HEAP_BLOCK_IN_USE;
        header.requested = 0;
        let next_offset = offset as u64 + HEAP_HEADER_BYTES as u64 + header.size as u64;
        if next_offset < self.heaps[h].chunks[c].length as u64 {
            let Some(next) = self.block_read(h, c, next_offset) else {
                return false;
            };
            if next.flags & HEAP_BLOCK_IN_USE == 0 {
                header.size = header
                    .size
                    .wrapping_add(HEAP_HEADER_BYTES)
                    .wrapping_add(next.size);
            }
        }
        if !self.block_write(h, c, offset as u64, &header) {
            return false;
        }
        if !self.heap_fix_following(h, c, offset, header.size) {
            return false;
        }
        if offset != 0 {
            // u32 arithmetic on purpose: the C check wraps for a corrupt prev_size near 2^32.
            if header.prev_size == 0 || header.prev_size.wrapping_add(HEAP_HEADER_BYTES) > offset {
                self.log.push(format!(
                    "guest_mem: heap block {at:#x} has an impossible prev_size {}",
                    header.prev_size
                ));
                return false;
            }
            let prev_offset = offset
                .wrapping_sub(HEAP_HEADER_BYTES)
                .wrapping_sub(header.prev_size);
            let Some(mut prev) = self.block_read(h, c, prev_offset as u64) else {
                return false;
            };
            if prev.flags & HEAP_BLOCK_IN_USE == 0 {
                prev.size = prev
                    .size
                    .wrapping_add(HEAP_HEADER_BYTES)
                    .wrapping_add(header.size);
                if !self.block_write(h, c, prev_offset as u64, &prev) {
                    return false;
                }
                if !self.heap_fix_following(h, c, prev_offset, prev.size) {
                    return false;
                }
            }
        }
        true
    }

    /// Resize: in place when it fits the existing block (even a shrink), else move.
    pub fn heap_realloc(&mut self, handle: u32, ptr: u32, bytes: u32) -> u32 {
        if ptr == 0 {
            return self.heap_alloc(handle, bytes);
        }
        let Some(h) = self.heap_index(handle) else {
            return 0;
        };
        let Some((c, offset)) = self.heap_find_block(h, ptr) else {
            return 0;
        };
        let Some(mut header) = self.block_read(h, c, offset as u64) else {
            return 0;
        };
        if header.flags & HEAP_BLOCK_IN_USE == 0 {
            return 0;
        }
        if bytes <= header.size {
            header.requested = bytes;
            if !self.block_write(h, c, offset as u64, &header) {
                return 0;
            }
            return ptr;
        }
        let moved = self.heap_alloc(handle, bytes);
        if moved == 0 {
            return 0;
        }
        match self.read_heap_bytes(ptr, header.requested) {
            Some(data) if self.write_heap_bytes(moved, &data) => {}
            _ => {
                self.heap_free(handle, moved);
                return 0;
            }
        }
        if !self.placement.backs_guest() {
            self.placement.copy_guest(moved, ptr, header.requested);
        }
        self.heap_free(handle, ptr);
        moved
    }

    /// Read bytes that lie inside one heap chunk (the only guest bytes this model stores).
    pub fn read_heap_bytes(&self, addr: u32, len: u32) -> Option<Vec<u8>> {
        for heap in self.heaps.iter().filter(|h| h.in_use) {
            for chunk in &heap.chunks {
                if addr >= chunk.base
                    && addr as u64 + len as u64 <= chunk.base as u64 + chunk.length as u64
                {
                    let start = (addr - chunk.base) as usize;
                    if self.placement.backs_guest() {
                        let mut out = vec![0u8; len as usize];
                        return self.placement.read_guest(addr, &mut out).then_some(out);
                    }
                    return Some(chunk.bytes[start..start + len as usize].to_vec());
                }
            }
        }
        None
    }

    /// Write bytes inside one heap chunk.
    pub fn write_heap_bytes(&mut self, addr: u32, data: &[u8]) -> bool {
        let backs = self.placement.backs_guest();
        for heap in self.heaps.iter_mut().filter(|h| h.in_use) {
            for chunk in &mut heap.chunks {
                if addr >= chunk.base
                    && addr as u64 + data.len() as u64 <= chunk.base as u64 + chunk.length as u64
                {
                    if backs {
                        self.placement.write_guest(addr, data);
                    } else {
                        let start = (addr - chunk.base) as usize;
                        chunk.bytes[start..start + data.len()].copy_from_slice(data);
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Every block header of every live heap as `(address of the header, 16 raw bytes)`, walking each chunk by
    /// the recorded block sizes. Used to compare the model against the 16 bytes the C allocator keeps in guest
    /// memory. A corrupt size ends the walk of that chunk.
    pub fn heap_block_headers(&self) -> Vec<(u32, [u8; 16])> {
        let mut out = Vec::new();
        for (h, heap) in self.heaps.iter().enumerate().filter(|(_, h)| h.in_use) {
            for (c, chunk) in heap.chunks.iter().enumerate() {
                let mut offset = 0usize;
                while offset + 16 <= chunk.length as usize {
                    let mut raw = [0u8; 16];
                    if self.chunk_read(h, c, offset as u64, &mut raw).is_none() {
                        break;
                    }
                    out.push((chunk.base.wrapping_add(offset as u32), raw));
                    let size = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
                    if size == 0 && offset != 0 {
                        break;
                    }
                    offset += 16 + size;
                }
            }
        }
        out
    }

    /// Chunks of a heap as `(base, length, bytes)`, for layout comparison. `bytes` is empty when the placement
    /// backs the guest (read the headers with `heap_block_headers` then).
    pub fn heap_chunks(&self, handle: u32) -> Vec<(u32, u32, &[u8])> {
        self.heap_index(handle)
            .map(|h| {
                self.heaps[h]
                    .chunks
                    .iter()
                    .map(|c| (c.base, c.length, c.bytes.as_slice()))
                    .collect()
            })
            .unwrap_or_default()
    }

    // ---- teardown and statistics --------------------------------------------------------------

    /// Release every region and heap. Placement highwater and generations do not rewind.
    pub fn reset(&mut self) {
        for slot in std::mem::take(&mut self.regions).into_iter().flatten() {
            self.lowmem_unmap(slot.map_base, slot.map_length);
        }
        self.region_live = 0;
        for h in 0..HEAP_MAX_SLOTS {
            if self.heaps[h].in_use {
                self.heap_release_chunks(h);
                self.heaps[h].in_use = false;
                self.heaps[h].token = 0;
            }
            self.heaps[h].chunks.clear();
        }
        self.heap_live = 0;
    }

    pub fn region_count(&self) -> usize {
        self.region_live
    }
    pub fn heap_count(&self) -> usize {
        self.heap_live
    }
    pub fn mapped_bytes(&self) -> u64 {
        self.mapped_bytes
    }

    /// Counters for `MmQueryStatistics` (contiguous regions are not VM).
    pub fn statistics(&self) -> Statistics {
        let mut out = Statistics::default();
        let mut add = |usage: Usage, bytes: u64, committed: bool| match usage {
            Usage::Virtual => {
                out.virtual_reserved_bytes += bytes;
                if committed {
                    out.virtual_committed_bytes += bytes;
                }
            }
            _ if committed => {
                let pages = bytes / PAGE_SIZE as u64;
                match usage {
                    Usage::Pool => out.pool_pages += pages,
                    Usage::Stack => out.stack_pages += pages,
                    Usage::Cache => out.cache_pages += pages,
                    Usage::Virtual => {}
                }
            }
            _ => {}
        };
        for slot in self.regions.iter().flatten().filter(|s| !s.info.contiguous) {
            add(
                slot.usage,
                slot.info.size as u64,
                slot.info.state == nt::MEM_COMMIT,
            );
        }
        for heap in self.heaps.iter().filter(|h| h.in_use) {
            for chunk in &heap.chunks {
                add(heap.usage, chunk.map_length, true);
            }
        }
        out
    }
}
