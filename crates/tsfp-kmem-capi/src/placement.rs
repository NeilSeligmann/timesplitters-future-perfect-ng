// SPDX-License-Identifier: GPL-3.0-or-later
//! Placement backends for the live binding (T2048 section 8.4 steps 1 and 2).
//!
//! * [`MmapPlacement`]: real low-4GB `mmap` placement mirroring `lowmem_map` / `lowmem_unmap` of
//!   `src/xbox/guest_mem.c`, with ordered `MAP_FIXED_NOREPLACE` slots for deterministic free placement.
//!   `TSFP_GUEST_PLACE=det` uses the C replay cursor and 64 KB allocation granularity.
//!   It also stores the heap boundary tags and moved payload bytes into the mappings it owns.
//! * [`ReplayPlacement`]: compare mode. The C host places memory; each C `mmap`/`munmap` is queued as an event and
//!   the Rust model, asking for the same mapping, is answered with the C result (and the request is checked).
// T3906: shared direct-pointer writer witness (host symbols in guest_mem.c; recording-off stand-ins under cfg(test)).
#[allow(dead_code)]
#[path = "../../tsfp-direct-witness/src/lib.rs"]
mod direct_witness;
use std::collections::{BTreeMap, VecDeque};
use std::ffi::c_void;

use tsfp_engine_memory::guest_regions::Placement;

unsafe extern "C" {
    fn mmap(addr: *mut c_void, len: usize, prot: i32, flags: i32, fd: i32, off: i64)
    -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> i32;
    fn process_vm_readv(
        pid: i32,
        local: *const IoVec,
        local_count: u64,
        remote: *const IoVec,
        remote_count: u64,
        flags: u64,
    ) -> isize;
    fn getpid() -> i32;
}

#[repr(C)]
struct IoVec {
    base: *mut c_void,
    len: usize,
}

/// Permission-aware copy out of this process (the C `kernel_guest_read_bytes` slow path): a page the guest made
/// PROT_NONE, or one unmapped meanwhile, returns false instead of faulting.
fn read_checked(addr: u32, out: &mut [u8]) -> bool {
    if out.is_empty() {
        return true;
    }
    let local = IoVec {
        base: out.as_mut_ptr().cast(),
        len: out.len(),
    };
    let remote = IoVec {
        base: addr as usize as *mut c_void,
        len: out.len(),
    };
    // SAFETY: both iovecs describe `out.len()` bytes; the kernel validates the remote range and reports EFAULT.
    let got = unsafe { process_vm_readv(getpid(), &local, 1, &remote, 1, 0) };
    got == out.len() as isize
}

const PROT_READ_WRITE: i32 = 3;
const MAP_PRIVATE: i32 = 0x02;
const MAP_ANONYMOUS: i32 = 0x20;
const MAP_FIXED_NOREPLACE: i32 = 0x100000;
const MAP_FAILED: usize = usize::MAX;

const ADDRESS_LIMIT: u64 = 0x1_0000_0000;
/// `GUEST_LOWMEM_HINT_BASE`, `_LIMIT`, `_STRIDE`, `_ATTEMPTS` of guest_mem.c / guest_mem.h.
pub const HINT_BASE: u64 = 0x4000_0000;
pub const HINT_LIMIT: u64 = 0xF000_0000;
pub const HINT_STRIDE: u64 = 0x0040_0000;
pub const HINT_ATTEMPTS: u64 = 64;

/// Real host mappings below 4 GB.
#[derive(Default)]
pub struct MmapPlacement {
    live: BTreeMap<u32, u64>,
    /// `kernel_guest_probe_change_begin` / `kernel_guest_probe_cache_flush` of the C host, called around `munmap`.
    probe_begin: Option<extern "C" fn()>,
    probe_flush: Option<extern "C" fn()>,
    /// Sum of live mapping lengths (`mapped_bytes` of guest_mem.c).
    pub mapped_bytes: u64,
}

impl MmapPlacement {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_probe_hooks(
        begin: Option<extern "C" fn()>,
        flush: Option<extern "C" fn()>,
    ) -> Self {
        Self {
            probe_begin: begin,
            probe_flush: flush,
            ..Self::default()
        }
    }

    fn fixed(length: u64, base: u32) -> Option<u32> {
        if base as u64 + length > ADDRESS_LIMIT {
            return None;
        }
        // SAFETY: anonymous private mapping at a caller-chosen address with NOREPLACE: it cannot replace anything.
        let got = unsafe {
            mmap(
                base as usize as *mut c_void,
                length as usize,
                PROT_READ_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE,
                -1,
                0,
            )
        } as usize;
        if got == MAP_FAILED {
            return None;
        }
        if got != base as usize {
            // An old kernel degrades NOREPLACE to a hint: give it back, the base must be honoured.
            // SAFETY: unmapping the range just mapped.
            unsafe { munmap(got as *mut c_void, length as usize) };
            return None;
        }
        Some(base)
    }

    /// Scan the low-memory window in a stable order. Every candidate is exact and
    /// non-destructive; unlike MAP_32BIT or an advisory hint, the kernel cannot
    /// silently choose a different address based on process layout or timing.
    fn map_deterministic(length: u64) -> Option<u32> {
        let page = 4096_u64;
        let step = length.saturating_add(page - 1) / page * page;
        if step == 0 {
            return None;
        }
        let mut base = HINT_BASE;
        while base
            .checked_add(length)
            .is_some_and(|end| end <= HINT_LIMIT)
        {
            if let Some(address) = Self::fixed(length, base as u32) {
                return Some(address);
            }
            base = base.checked_add(step)?;
        }
        None
    }

    /// True when `[addr, addr+len)` lies inside one mapping this placement owns.
    fn owns(&self, addr: u32, len: usize) -> bool {
        let Some((&base, &length)) = self.live.range(..=addr).next_back() else {
            return false;
        };
        addr as u64 + len as u64 <= base as u64 + length
    }
}

impl Placement for MmapPlacement {
    fn map(&mut self, length: u64, fixed: Option<u32>) -> Option<u32> {
        if length == 0 || length > ADDRESS_LIMIT {
            return None;
        }
        let base = match fixed {
            Some(base) if base != 0 => Self::fixed(length, base),
            _ if crate::replay_placement::enabled() => {
                crate::replay_placement::map(length, Self::fixed)
            }
            _ => Self::map_deterministic(length),
        }?;
        self.live.insert(base, length);
        self.mapped_bytes += length;
        Some(base)
    }

    fn unmap(&mut self, base: u32, length: u64) {
        if base == 0 || length == 0 {
            return;
        }
        if let Some(begin) = self.probe_begin {
            begin();
        }
        // SAFETY: the range is one this placement mapped (the model only unmaps what it was given).
        unsafe { munmap(base as usize as *mut c_void, length as usize) };
        if let Some(flush) = self.probe_flush {
            flush();
        }
        self.live.remove(&base);
        self.mapped_bytes = self.mapped_bytes.saturating_sub(length);
    }

    fn write_guest(&mut self, addr: u32, data: &[u8]) {
        if self.owns(addr, data.len()) {
            // SAFETY: the destination lies inside a live private anonymous read/write mapping (checked by `owns`).
            let token = direct_witness::prepare();
            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), addr as usize as *mut u8, data.len());
            }
            direct_witness::report(token, addr, data);
        }
    }

    fn read_guest(&self, addr: u32, out: &mut [u8]) -> bool {
        if !self.owns(addr, out.len()) {
            return false;
        }
        read_checked(addr, out)
    }

    fn backs_guest(&self) -> bool {
        true
    }

    fn copy_guest(&mut self, dst: u32, src: u32, len: u32) {
        if len != 0 && self.owns(dst, len as usize) && self.owns(src, len as usize) {
            // SAFETY: both ranges lie inside live read/write mappings (checked by `owns`); `copy` handles overlap.
            unsafe {
                std::ptr::copy(
                    src as usize as *const u8,
                    dst as usize as *mut u8,
                    len as usize,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_mappings_use_the_first_exact_lowmem_slot() {
        let mut placement = MmapPlacement::new();
        let first = MmapPlacement::map_deterministic(0x20_000).expect("lowmem slot available");
        assert_eq!(first as u64, HINT_BASE);
        placement.unmap(first, 0x20_000);

        let second = MmapPlacement::map_deterministic(0x20_000).expect("slot was released");
        assert_eq!(
            second, first,
            "same request and process layout must select the same guest address"
        );
        placement.unmap(second, 0x20_000);
    }

    #[test]
    fn read_guest_refuses_a_protected_page_instead_of_faulting() {
        unsafe extern "C" {
            fn mprotect(addr: *mut c_void, len: usize, prot: i32) -> i32;
        }
        let mut placement = MmapPlacement::new();
        let base = placement
            .map(0x2000, Some(0x5A00_0000))
            .expect("map two pages");
        // Header 16 bytes below the page edge, and one straddling it.
        let mut buf = [0u8; 16];
        assert!(placement.read_guest(base + 0xFF0, &mut buf));
        assert!(placement.read_guest(base + 0xFF8, &mut buf));
        // SAFETY: second page of a mapping this test owns.
        assert_eq!(
            unsafe { mprotect((base + 0x1000) as usize as *mut c_void, 0x1000, 0) },
            0
        );
        assert!(
            placement.read_guest(base + 0xFF0, &mut buf),
            "page before is readable"
        );
        assert!(
            !placement.read_guest(base + 0xFF8, &mut buf),
            "straddling into PROT_NONE"
        );
        assert!(
            !placement.read_guest(base + 0x1000, &mut buf),
            "inside PROT_NONE"
        );
        // Outside any live mapping.
        assert!(
            !placement.read_guest(base + 0x1FF8, &mut buf),
            "past the region end"
        );
        placement.unmap(base, 0x2000);
    }
}

/// One C placement event (the `guest_mem_place_hook` of guest_mem.c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceEvent {
    /// 0 map, 1 unmap.
    pub kind: u32,
    pub length: u64,
    /// Fixed base requested (map) or the base released (unmap); 0 for a free choice.
    pub address: u32,
    /// Map result, 0 for a refusal.
    pub result: u32,
}

/// Compare-mode placement: answers the model with what the C host did.
#[derive(Default)]
pub struct ReplayPlacement {
    pub events: VecDeque<PlaceEvent>,
    /// Requests the model made that the C host did not make (or made differently).
    pub request_mismatches: u64,
    pub last_mismatch: String,
    pub replayed: u64,
}

impl ReplayPlacement {
    fn mismatch(&mut self, text: String) {
        self.request_mismatches += 1;
        self.last_mismatch = text;
    }
}

impl Placement for ReplayPlacement {
    fn map(&mut self, length: u64, fixed: Option<u32>) -> Option<u32> {
        let want = fixed.unwrap_or(0);
        match self.events.front().copied() {
            Some(e) if e.kind == 0 && e.length == length && e.address == want => {
                self.events.pop_front();
                self.replayed += 1;
                (e.result != 0).then_some(e.result)
            }
            other => {
                self.mismatch(format!(
                    "map(length {length:#x}, fixed {want:#x}) versus C event {other:?}"
                ));
                None
            }
        }
    }

    fn unmap(&mut self, base: u32, length: u64) {
        match self.events.front().copied() {
            Some(e) if e.kind == 1 && e.length == length && e.address == base => {
                self.events.pop_front();
                self.replayed += 1;
            }
            other => self.mismatch(format!(
                "unmap(base {base:#x}, length {length:#x}) versus C event {other:?}"
            )),
        }
    }
}
