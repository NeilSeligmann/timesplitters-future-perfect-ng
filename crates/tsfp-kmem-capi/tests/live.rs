// SPDX-License-Identifier: GPL-3.0-or-later
//! T2048 live binding: the real mmap Placement, the heap write-through and the replay placement (no C needed).
#[path = "support/witness_stubs.rs"]
mod witness_stubs;
use tsfp_engine_memory::guest_regions::{Placement, nt};
use tsfp_engine_memory::kernel_mm::KernelMemory;
use tsfp_kmem_capi::ops::{self, ORD_BASE};
use tsfp_kmem_capi::placement::{MmapPlacement, PlaceEvent, ReplayPlacement};

fn read_u32(addr: u32) -> u32 {
    // SAFETY: tests only read addresses inside mappings they just created.
    unsafe { std::ptr::read_volatile(addr as usize as *const u32) }
}

fn write_u32(addr: u32, value: u32) {
    // SAFETY: as above.
    unsafe { std::ptr::write_volatile(addr as usize as *mut u32, value) }
}

#[test]
fn mmap_placement_maps_below_4gb_writes_and_unmaps() {
    let mut p = MmapPlacement::new();
    let a = p.map(0x10000, None).expect("map");
    assert!(a as u64 + 0x10000 <= 0x1_0000_0000);
    assert_eq!(a % 0x1000, 0);
    write_u32(a, 0xDEAD_BEEF);
    assert_eq!(read_u32(a), 0xDEAD_BEEF);
    assert_eq!(p.mapped_bytes, 0x10000);
    // A live range refuses a fixed map on top of it (NOREPLACE).
    assert_eq!(p.map(0x1000, Some(a)), None);
    p.unmap(a, 0x10000);
    assert_eq!(p.mapped_bytes, 0);
    // After the unmap the same base can be taken exactly.
    assert_eq!(p.map(0x2000, Some(a)), Some(a));
    assert_eq!(read_u32(a), 0, "fresh anonymous memory is zero");
    p.unmap(a, 0x2000);
}

#[test]
fn mmap_placement_refuses_what_cannot_fit() {
    let mut p = MmapPlacement::new();
    assert_eq!(p.map(0, None), None);
    assert_eq!(
        p.map(0x2_0000_0000, None),
        None,
        "larger than the address space"
    );
    assert_eq!(
        p.map(0x2000, Some(0xFFFF_F000)),
        None,
        "past the 4 GB limit"
    );
}

#[test]
fn free_mapping_stays_in_the_deterministic_window() {
    let mut p = MmapPlacement::new();
    let a = p.map(0x10000, None).expect("free map");
    assert!(a as u64 >= 0x4000_0000 && a as u64 + 0x10000 <= 0xF000_0000);
    write_u32(a, 7);
    p.unmap(a, 0x10000);
    assert_eq!(p.map(0xB000_0000 + 0x1000, None), None);
}

#[test]
fn kernel_memory_over_mmap_backs_regions_and_heap_headers_in_guest_memory() {
    let mut km = KernelMemory::new(MmapPlacement::new());
    // NtAllocateVirtualMemory: the returned region is real memory.
    let r = km.nt_allocate_virtual_memory(0, 0x3000, nt::MEM_COMMIT, nt::PAGE_READWRITE);
    assert_eq!(r.status, nt::SUCCESS);
    let base = r.base.unwrap();
    write_u32(base + 0x2FFC, 0x1234_5678);
    assert_eq!(read_u32(base + 0x2FFC), 0x1234_5678);
    // Pool allocation: the heap chunk header is written through to guest memory and matches the model.
    let p1 = km.ex_allocate_pool_with_tag(100, 0x4141_4141);
    let p2 = km.ex_allocate_pool_with_tag(5000, 0x4242_4242);
    assert!(p1 != 0 && p2 > p1);
    write_u32(p1, 0xAAAA_AAAA);
    write_u32(p2, 0xBBBB_BBBB);
    for (address, raw) in km.regions.heap_block_headers() {
        let words: Vec<u32> = (0..4).map(|i| read_u32(address + i * 4)).collect();
        let model: Vec<u32> = raw
            .chunks(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        assert_eq!(words, model, "header at {address:#x}");
    }
    // Header of p1 says 100 bytes requested and in use.
    assert_eq!(read_u32(p1 - 16 + 8), 100);
    assert_eq!(read_u32(p1 - 16 + 12) & 1, 1);
    km.ex_free_pool(p1);
    assert_eq!(read_u32(p1 - 16 + 12) & 1, 0, "free is written through");
    assert_eq!(read_u32(p2), 0xBBBB_BBBB, "neighbour payload untouched");
}

#[test]
fn moving_realloc_copies_payload_in_guest_memory() {
    let mut km = KernelMemory::new(MmapPlacement::new());
    let heap = km.regions.heap_create(0, 0x10000, 0);
    assert_ne!(heap, 0);
    let a = km.regions.heap_alloc(heap, 64);
    let _guard = km.regions.heap_alloc(heap, 64); // blocks in-place growth
    for i in 0..16 {
        write_u32(a + i * 4, 0x1000 + i);
    }
    let moved = km.regions.heap_realloc(heap, a, 4096);
    assert_ne!(moved, 0);
    assert_ne!(moved, a);
    for i in 0..16 {
        assert_eq!(read_u32(moved + i * 4), 0x1000 + i, "word {i}");
    }
}

#[test]
fn replay_placement_returns_c_results_and_flags_foreign_requests() {
    let mut p = ReplayPlacement::default();
    p.events.push_back(PlaceEvent {
        kind: 0,
        length: 0x2000,
        address: 0,
        result: 0x5000_0000,
    });
    p.events.push_back(PlaceEvent {
        kind: 1,
        length: 0x2000,
        address: 0x5000_0000,
        result: 0,
    });
    p.events.push_back(PlaceEvent {
        kind: 0,
        length: 0x1000,
        address: 0x6000_0000,
        result: 0,
    });
    assert_eq!(p.map(0x2000, None), Some(0x5000_0000));
    p.unmap(0x5000_0000, 0x2000);
    assert_eq!(
        p.map(0x1000, Some(0x6000_0000)),
        None,
        "C refusal replays as None"
    );
    assert_eq!((p.request_mismatches, p.replayed), (0, 3));
    // A request C never made is a mismatch and answered with a refusal.
    assert_eq!(p.map(0x1000, None), None);
    assert_eq!(p.request_mismatches, 1);
    assert!(p.last_mismatch.contains("map(length 0x1000"));
}

#[test]
fn ops_table_matches_direct_calls() {
    let mut km = KernelMemory::new(MmapPlacement::new());
    let mut out = [0u32; 16];
    // MmAllocateContiguousMemory(0x2000) through the op table, then its physical address and size.
    let a = ops::dispatch(&mut km, ORD_BASE + 165, &[0x2000], &mut out);
    assert_ne!(a, 0);
    assert_eq!(
        ops::dispatch(&mut km, ORD_BASE + 180, &[a], &mut out),
        0x2000
    );
    assert_ne!(ops::dispatch(&mut km, ORD_BASE + 173, &[a], &mut out), 0);
    // NtAllocateVirtualMemory out convention: flags then values.
    let status = ops::dispatch(
        &mut km,
        ORD_BASE + 184,
        &[0, 0x1800, nt::MEM_COMMIT, nt::PAGE_READWRITE],
        &mut out,
    );
    assert_eq!(status, nt::SUCCESS);
    assert_eq!((out[0], out[2], out[3]), (1, 1, 0x2000));
    // Region record op.
    assert_eq!(
        ops::dispatch(&mut km, ops::OP_REGION_AT, &[out[1]], &mut out),
        1
    );
    assert_eq!(out[1], 0x2000);
    // Unknown op.
    assert_eq!(ops::dispatch(&mut km, 999, &[], &mut out), u32::MAX);
    ops::dispatch(&mut km, ORD_BASE + 171, &[a], &mut out);
}
