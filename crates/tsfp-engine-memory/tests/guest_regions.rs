// SPDX-License-Identifier: GPL-3.0-or-later
//! T2048 slice K, layer A: spec tests of `GuestRegions` / `KernelMemory` with the pure-Rust
//! `WindowPlacement`. Expected values are derived from `src/xbox/guest_mem.c`,
//! `kernel_memory.c` and `kernel_pool.c`. The C comparison itself lives in
//! `crates/tsfp-c-diff/tests/memory_cdiff.rs`; these tests run without any C.
use tsfp_engine_memory::guest_regions::{
    GuestRegions, HEAP_CHUNK_MIN, Placement, RegionRequest, Usage, WindowPlacement, nt,
};
use tsfp_engine_memory::kernel_mm::KernelMemory;

fn regions() -> GuestRegions<WindowPlacement> {
    GuestRegions::new(WindowPlacement::default_window())
}

fn request(bytes: u32) -> RegionRequest {
    RegionRequest {
        bytes,
        protect: nt::PAGE_READWRITE,
        state: nt::MEM_COMMIT,
        ..RegionRequest::default()
    }
}

#[test]
fn region_alloc_rounds_aligns_and_reports_status() {
    let mut r = regions();
    let a = r.region_alloc(&request(5)).unwrap();
    let region = *r.region_at(a).unwrap();
    assert_eq!(
        (region.size, region.requested, region.state),
        (0x1000, 5, nt::MEM_COMMIT)
    );
    // A reserve works in 64 KB units and 64 KB alignment.
    let reserve = RegionRequest {
        state: nt::MEM_RESERVE,
        ..request(1)
    };
    let b = r.region_alloc(&reserve).unwrap();
    assert_eq!(b % 0x10000, 0);
    assert_eq!(r.region_at(b).unwrap().size, 0x10000);
    // Explicit alignment is honoured for real.
    let c = r
        .region_alloc(&RegionRequest {
            alignment: 0x40000,
            ..request(0x1000)
        })
        .unwrap();
    assert_eq!(c % 0x40000, 0);
    // Error codes.
    assert_eq!(r.region_alloc(&request(0)), Err(nt::INVALID_PARAMETER));
    assert_eq!(
        r.region_alloc(&RegionRequest {
            protect: 6,
            ..request(1)
        }),
        Err(nt::INVALID_PAGE_PROTECTION)
    );
    assert_eq!(
        r.region_alloc(&RegionRequest {
            state: 0,
            ..request(1)
        }),
        Err(nt::INVALID_PARAMETER)
    );
    assert_eq!(
        r.region_alloc(&RegionRequest {
            alignment: 3,
            ..request(1)
        }),
        Err(nt::INVALID_PARAMETER)
    );
    let bad_window = RegionRequest {
        lowest_physical: 9,
        highest_physical: 8,
        ..request(1)
    };
    assert_eq!(r.region_alloc(&bad_window), Err(nt::INVALID_PARAMETER));
    assert_eq!(r.region_alloc(&request(0xFFFF_F001)), Err(nt::NO_MEMORY));
}

#[test]
fn fixed_base_rounds_down_and_conflicts() {
    let mut r = regions();
    let fixed = RegionRequest {
        fixed_base: 0x6000_1234,
        state: nt::MEM_RESERVE,
        ..request(0x1000)
    };
    assert_eq!(
        r.region_alloc(&fixed),
        Ok(0x6000_0000),
        "NT rounds the base down to the granularity"
    );
    assert_eq!(r.region_alloc(&fixed), Err(nt::CONFLICTING_ADDRESSES));
    let low = RegionRequest {
        fixed_base: 0x100,
        state: nt::MEM_RESERVE,
        ..request(0x1000)
    };
    assert_eq!(
        r.region_alloc(&low),
        Err(nt::INVALID_PARAMETER),
        "rounds to 0"
    );
    let misaligned = RegionRequest {
        fixed_base: 0x5000_1000,
        alignment: 0x10000,
        ..request(0x1000)
    };
    assert_eq!(r.region_alloc(&misaligned), Err(nt::CONFLICTING_ADDRESSES));
}

#[test]
fn contiguous_regions_get_a_lead_in_page_and_stable_addresses() {
    let mut r = regions();
    let contiguous = RegionRequest {
        contiguous: true,
        ..request(0x2000)
    };
    let a = r.region_alloc(&contiguous).unwrap();
    let b = r.region_alloc(&contiguous).unwrap();
    assert_ne!(a, b);
    // The mapping is the lead-in page plus the region, so the address sits one page above the map.
    assert!(r.placement().map_len_for_test(a - 0x1000).is_some());
    let before = *r.region_at(a).unwrap();
    for _ in 0..50 {
        let x = r.region_alloc(&request(0x3000)).unwrap();
        r.region_free(x);
    }
    assert_eq!(
        *r.region_at(a).unwrap(),
        before,
        "a live region never moves"
    );
    assert!(r.region_free(a) && !r.region_free(a));
}

#[test]
fn physical_addresses_are_linear_distinct_and_invertible() {
    let mut r = regions();
    let a = r.region_alloc(&request(0x4000)).unwrap();
    let b = r.region_alloc(&request(0x2000)).unwrap();
    let pa = r.physical_address(a);
    assert_ne!(pa, 0);
    assert_eq!(r.physical_address(a + 0x123), pa + 0x123);
    let pb = r.physical_address(b);
    assert!(
        pb >= pa + 0x4000 || pb + 0x2000 <= pa,
        "live physical ranges do not overlap"
    );
    assert_eq!(r.virtual_from_physical(pa + 0x10), Some(a + 0x10));
    assert_eq!(r.virtual_from_physical(0), None);
    assert_eq!(r.physical_address(0x1234), 0);
    // A constrained window is honoured.
    let low = RegionRequest {
        highest_physical: 0x20_0000,
        ..request(0x1000)
    };
    let c = r.region_alloc(&low).unwrap();
    assert!(r.physical_address(c) + 0x1000 <= 0x20_0001);
    let g = r.region_at(c).unwrap().generation;
    r.region_free(c);
    let d = r.region_alloc(&low).unwrap();
    assert!(
        r.region_at(d).unwrap().generation > g,
        "generations never repeat"
    );
}

#[test]
fn heap_first_fit_splits_coalesces_and_isolates() {
    let mut r = regions();
    let h = r.heap_create(0, HEAP_CHUNK_MIN, 0);
    assert_eq!(h & 0xFFF, 1);
    let a = r.heap_alloc(h, 100);
    let b = r.heap_alloc(h, 100);
    let c = r.heap_alloc(h, 100);
    assert_eq!(
        (b - a, c - b),
        (112 + 16, 112 + 16),
        "112 byte payload plus 16 byte header"
    );
    assert_eq!(a % 16, 0);
    assert_eq!(
        r.heap_block_size(h, b),
        Some(100),
        "requested size, not the rounded size"
    );
    assert!(r.heap_free(h, b));
    assert!(!r.heap_free(h, b), "double free is refused");
    assert_eq!(r.heap_alloc(h, 60), b, "first fit reuses the freed hole");
    assert!(r.heap_free(h, a) && r.heap_free(h, b) && r.heap_free(h, c));
    let chunks = r.heap_chunks(h);
    assert_eq!(chunks.len(), 1);
    // Everything coalesced back into one free block.
    let first_size = u32::from_le_bytes(chunks[0].2[0..4].try_into().unwrap());
    assert_eq!(first_size, chunks[0].1 - 16);
    // Another heap cannot free this heap's block.
    let other = r.heap_create(0, 0, 0);
    let p = r.heap_alloc(h, 8);
    assert!(!r.heap_free(other, p));
    assert!(r.heap_free(h, p));
    assert!(r.heap_destroy(h) && !r.heap_valid(h));
    assert!(r.heap_valid(other));
}

#[test]
fn destroyed_heap_handles_are_never_reissued() {
    let mut r = regions();
    let first = r.heap_create(0, 0, 0);
    assert!(r.heap_destroy(first));
    let second = r.heap_create(0, 0, 0);
    assert_eq!(first & 0xFFF, second & 0xFFF);
    assert_ne!(first, second, "the generation in the high bits differs");
    assert!(!r.heap_valid(first) && r.heap_valid(second));
}

#[test]
fn capped_heap_fills_up_and_realloc_moves_with_contents() {
    let mut r = regions();
    let h = r.heap_create(0, 0x10000, 0x10000);
    assert_ne!(h, 0);
    assert_eq!(
        r.heap_alloc(h, 0x10000),
        0,
        "a request larger than the cap fails"
    );
    let p = r.heap_alloc(h, 32);
    assert!(r.write_heap_bytes(p, &[7u8; 32]));
    assert_eq!(r.heap_realloc(h, p, 16), p, "shrink stays in place");
    assert_eq!(r.heap_block_size(h, p), Some(16));
    let q = r.heap_alloc(h, 16);
    let moved = r.heap_realloc(h, p, 200);
    assert!(moved != 0 && moved != p);
    assert_eq!(r.read_heap_bytes(moved, 16).unwrap(), vec![7u8; 16]);
    assert_eq!(q % 16, 0);
    assert_eq!(
        r.heap_create(0, 0x20000, 0x10000),
        0,
        "initial above maximum"
    );
}

#[test]
fn nt_virtual_memory_reserve_commit_decommit_release() {
    let mut km = KernelMemory::new(WindowPlacement::default_window());
    let reserved = km.nt_allocate_virtual_memory(0, 0x12345, nt::MEM_RESERVE, nt::PAGE_READWRITE);
    assert_eq!(reserved.status, nt::SUCCESS);
    let (base, size) = (reserved.base.unwrap(), reserved.size.unwrap());
    assert_eq!((base % 0x10000, size), (0, 0x20000));
    let info = km.nt_query_virtual_memory(base + 0x1234);
    assert_eq!(
        info,
        [
            base + 0x1000,
            base,
            nt::PAGE_READWRITE,
            size - 0x1000,
            nt::MEM_RESERVE,
            nt::PAGE_READWRITE,
            nt::MEM_PRIVATE
        ]
    );
    // Two-step reserve then commit at the same base keeps the address.
    let committed = km.nt_allocate_virtual_memory(base, 0x1000, nt::MEM_COMMIT, nt::PAGE_READONLY);
    assert_eq!(
        (committed.status, committed.base, committed.size),
        (nt::SUCCESS, Some(base), Some(size))
    );
    assert_eq!(km.nt_query_virtual_memory(base)[4], nt::MEM_COMMIT);
    assert_eq!(km.mm_query_address_protect(base), nt::PAGE_READONLY);
    // Free statuses.
    assert_eq!(
        km.nt_free_virtual_memory(base + 0x1000, 0, nt::MEM_RELEASE)
            .status,
        nt::FREE_VM_NOT_AT_BASE
    );
    assert_eq!(
        km.nt_free_virtual_memory(0x7000_0000, 0, nt::MEM_RELEASE)
            .status,
        nt::MEMORY_NOT_ALLOCATED
    );
    assert_eq!(
        km.nt_free_virtual_memory(base, 0, nt::MEM_RELEASE | nt::MEM_DECOMMIT)
            .status,
        nt::INVALID_PARAMETER
    );
    assert_eq!(
        km.nt_free_virtual_memory(base, 0x1000, nt::MEM_RELEASE)
            .status,
        nt::INVALID_PARAMETER
    );
    assert_eq!(
        km.nt_free_virtual_memory(base, 0, nt::MEM_DECOMMIT).size,
        Some(size)
    );
    assert_eq!(km.nt_query_virtual_memory(base)[4], nt::MEM_RESERVE);
    assert_eq!(
        km.nt_free_virtual_memory(base, 0, nt::MEM_RELEASE).status,
        nt::SUCCESS
    );
    assert_eq!(
        km.nt_query_virtual_memory(base)[4],
        nt::MEM_FREE,
        "gap extent is reported as 0, not invented"
    );
    // Argument validation order.
    assert_eq!(
        km.nt_allocate_virtual_memory(0, 0, nt::MEM_COMMIT, 4)
            .status,
        nt::INVALID_PARAMETER
    );
    assert_eq!(
        km.nt_allocate_virtual_memory(0, 1, 0x40, 4).status,
        nt::INVALID_PARAMETER
    );
    assert_eq!(
        km.nt_allocate_virtual_memory(0, 1, nt::MEM_COMMIT, 6)
            .status,
        nt::INVALID_PAGE_PROTECTION
    );
}

#[test]
fn mm_contiguous_stack_and_locks() {
    let mut km = KernelMemory::new(WindowPlacement::default_window());
    let a = km.mm_allocate_contiguous_memory_ex(0x3000, 0, 0, 0x10000, 0);
    assert_eq!(a % 0x10000, 0);
    assert_eq!(km.mm_query_allocation_size(a), 0x3000);
    assert_eq!(
        km.mm_allocate_contiguous_memory_ex(0x1000, 0, 0, 0x1800, 0),
        0,
        "non power of two"
    );
    km.mm_free_contiguous_memory(a + 1);
    assert!(
        km.regions.region_at(a).is_some(),
        "not a base: reported, nothing freed"
    );
    km.mm_free_contiguous_memory(a);
    assert!(km.regions.region_at(a).is_none());
    // Kernel stack: the return value is the TOP, delete wants an exact span.
    let top = km.mm_create_kernel_stack(0x6000, 0);
    let limit = top - 0x6000;
    assert_eq!(km.regions.region_at(limit).unwrap().size, 0x6000);
    assert_eq!(km.regions.statistics().stack_pages, 6);
    km.mm_delete_kernel_stack(top - 0x1000, limit);
    assert!(km.regions.region_at(limit).is_some());
    km.mm_delete_kernel_stack(top, limit);
    assert!(km.regions.region_at(limit).is_none());
    // Page locks nest and unlocking an unlocked page is reported.
    let b = km.mm_allocate_contiguous_memory(0x3000);
    km.mm_lock_unlock_buffer_pages(b, 0x3000, 0);
    km.mm_lock_unlock_buffer_pages(b, 0x1000, 0);
    assert_eq!(
        (
            km.locked_page_total(),
            km.page_lock_count(b),
            km.page_lock_count(b + 0x1000)
        ),
        (3, 2, 1)
    );
    let logs = km.regions.log.len();
    km.mm_lock_unlock_buffer_pages(b, 0x3000, 1);
    km.mm_lock_unlock_buffer_pages(b, 0x3000, 1);
    assert_eq!(km.locked_page_total(), 0);
    assert!(km.regions.log.len() > logs, "unbalanced unlock is reported");
}

#[test]
fn ex_pool_tracks_sizes_tags_and_bad_frees() {
    let mut km = KernelMemory::new(WindowPlacement::default_window());
    let p = km.ex_allocate_pool_with_tag(100, 0x6B77_6168);
    assert_ne!(p, 0);
    assert_eq!(
        (
            km.ex_query_pool_block_size(p),
            km.pool_tag_of(p),
            km.pool_live_bytes()
        ),
        (100, Some(0x6B77_6168), 100)
    );
    assert_eq!(
        km.ex_allocate_pool_with_tag(0, 1),
        0,
        "zero byte requests are refused"
    );
    km.ex_free_pool(0);
    km.ex_free_pool(p + 16);
    assert_eq!(km.pool_bad_free_count(), 2);
    km.ex_free_pool(p);
    km.ex_free_pool(p);
    assert_eq!(
        (
            km.pool_live_count(),
            km.pool_live_bytes(),
            km.pool_bad_free_count()
        ),
        (0, 0, 3)
    );
    assert_eq!(km.ex_query_pool_block_size(p), 0);
    // Heap destroyed underneath the pool: the table is dropped and the heap recreated.
    let q = km.ex_allocate_pool_with_tag(8, 1);
    let heap = km.pool_heap_handle();
    km.regions.heap_destroy(heap);
    let r = km.ex_allocate_pool_with_tag(8, 1);
    assert_ne!(r, 0);
    assert_eq!(km.pool_live_count(), 1);
    assert_eq!(km.pool_size_of(q), if q == r { Some(8) } else { None });
}

/// Invariants over seeded sequences with the pure-Rust placement: no overlapping live ranges,
/// everything below 4 GB, heap chunks tiled exactly by their blocks, reset releases everything.
#[test]
fn seeded_sequences_keep_layout_invariants() {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    };
    let mut km = KernelMemory::new(WindowPlacement::default_window());
    let mut heap = 0;
    let mut blocks: Vec<u32> = Vec::new();
    for _ in 0..4000 {
        match next() % 8 {
            0 | 1 => {
                let size = 1 + next() % 0x30000;
                let _ = km.nt_allocate_virtual_memory(
                    0,
                    size,
                    nt::MEM_COMMIT | nt::MEM_RESERVE,
                    nt::PAGE_READWRITE,
                );
            }
            2 => {
                let bases: Vec<u32> = km.regions.regions().map(|r| r.address).collect();
                if !bases.is_empty() {
                    let base = bases[next() as usize % bases.len()];
                    km.regions.region_free(base);
                }
            }
            3 if heap == 0 => heap = km.regions.heap_create_typed(0, 0, 0, Usage::Cache),
            3 | 4 | 5 => {
                let p = km.regions.heap_alloc(heap, next() % 5000);
                if p != 0 {
                    blocks.push(p);
                }
            }
            _ => {
                if !blocks.is_empty() {
                    let p = blocks.swap_remove(next() as usize % blocks.len());
                    assert!(km.regions.heap_free(heap, p));
                }
            }
        }
    }
    let mut live: Vec<(u64, u64)> = km
        .regions
        .regions()
        .map(|r| (r.address as u64, r.address as u64 + r.size as u64))
        .collect();
    live.sort();
    assert!(
        live.windows(2).all(|w| w[0].1 <= w[1].0),
        "live regions overlap"
    );
    assert!(live.iter().all(|r| r.1 <= 1 << 32));
    for (_, length, bytes) in km.regions.heap_chunks(heap) {
        let mut offset = 0usize;
        while offset < length as usize {
            let size = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            assert!(size >= 16 && size % 16 == 0);
            offset += 16 + size;
        }
        assert_eq!(offset, length as usize, "blocks tile the chunk exactly");
    }
    km.reset();
    assert_eq!(
        (
            km.regions.region_count(),
            km.regions.heap_count(),
            km.regions.mapped_bytes()
        ),
        (0, 0, 0)
    );
}

#[test]
fn window_placement_is_first_fit_and_refuses_overlap() {
    let mut p = WindowPlacement::new(0x1000_0000, 0x1010_0000);
    let a = p.map(0x4000, None).unwrap();
    let b = p.map(0x1000, None).unwrap();
    assert_eq!((a, b), (0x1000_0000, 0x1000_4000));
    p.unmap(a, 0x4000);
    assert_eq!(p.map(0x2000, None), Some(0x1000_0000));
    assert_eq!(
        p.map(0x1000, Some(0x1000_4000)),
        None,
        "fixed over a live mapping"
    );
    assert_eq!(p.map(0x20_0000, None), None, "window exhausted");
}

#[test]
fn window_placement_reuses_page_rounded_gaps() {
    const PAGE: u32 = 0x1000;
    for hole_pages in 1..=16u32 {
        for extra_bytes in [0, 1, PAGE] {
            let mut placement = WindowPlacement::new(0x1000_0000, 0x1004_0000);
            let left = placement.map(PAGE as u64, None).unwrap();
            let hole = placement.map((hole_pages * PAGE) as u64, None).unwrap();
            let right = placement.map(PAGE as u64, None).unwrap();
            assert_eq!(hole, left + PAGE);
            assert_eq!(right, hole + hole_pages * PAGE);
            assert_eq!(placement.map(1, Some(hole)), None);
            placement.unmap(hole, (hole_pages * PAGE) as u64);

            let requested = ((hole_pages - 1) * PAGE + 1 + extra_bytes) as u64;
            let rounded = ((requested + PAGE as u64 - 1) / PAGE as u64) * PAGE as u64;
            let reused = placement.map(requested, None);
            if rounded <= (hole_pages * PAGE) as u64 {
                assert_eq!(reused, Some(hole));
                assert_eq!(placement.map_len_for_test(hole), Some(rounded));
            } else {
                assert_eq!(reused, Some(right + PAGE));
                assert_eq!(placement.map_len_for_test(hole), None);
            }
            assert_eq!(placement.map(1, Some(right)), None);
        }
    }
}
