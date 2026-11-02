// SPDX-License-Identifier: GPL-3.0-or-later
//! T2048 8.5.2: over a backend that backs the guest the heap keeps no Vec copy (`heap_chunks` bytes empty) and the
//! guest memory is the only store. The same op sequence over the model-owned copy (`WindowPlacement`) must give
//! identical offsets, results and block headers.
#[path = "support/witness_stubs.rs"]
mod witness_stubs;
use tsfp_engine_memory::guest_regions::{GuestRegions, Placement, WindowPlacement};
use tsfp_kmem_capi::placement::MmapPlacement;

type Outcome = (Vec<u64>, Vec<(u32, [u8; 16])>, bool);

fn run<P: Placement>(mut r: GuestRegions<P>) -> Outcome {
    let heap = r.heap_create(0, 0x100000, 0);
    assert_ne!(heap, 0);
    let (base, no_copy) = {
        let c = r.heap_chunks(heap);
        (c[0].0, c[0].2.is_empty())
    };
    let mut seed = 0x1234_5678u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed >> 8
    };
    let mut live: Vec<u32> = Vec::new();
    let mut trace = Vec::new();
    for _ in 0..3000 {
        match next() % 4 {
            0 | 1 => {
                let p = r.heap_alloc(heap, 1 + next() % 700);
                trace.push(p.wrapping_sub(base) as u64);
                if p != 0 {
                    assert!(r.write_heap_bytes(p, &[0x5A; 1]));
                    live.push(p);
                }
            }
            2 if !live.is_empty() => {
                let i = next() as usize % live.len();
                let p = live.swap_remove(i);
                trace.push(r.heap_free(heap, p) as u64);
            }
            _ if !live.is_empty() => {
                let i = next() as usize % live.len();
                let moved = r.heap_realloc(heap, live[i], 1 + next() % 1500);
                trace.push(moved.wrapping_sub(base) as u64);
                if moved != 0 {
                    live[i] = moved;
                }
            }
            _ => {}
        }
    }
    let headers = r
        .heap_block_headers()
        .into_iter()
        .map(|(a, raw)| (a.wrapping_sub(base), raw))
        .collect();
    (trace, headers, no_copy)
}

#[test]
fn heap_over_backed_guest_keeps_no_copy_and_matches_the_owned_copy() {
    let (t_mmap, h_mmap, empty_mmap) = run(GuestRegions::new(MmapPlacement::new()));
    let (t_owned, h_owned, empty_owned) = run(GuestRegions::new(WindowPlacement::default_window()));
    assert!(empty_mmap, "no Vec copy over a backed guest");
    assert!(
        !empty_owned,
        "the owned copy stays for a placement without bytes"
    );
    assert!(h_mmap.len() > 50, "headers were walked");
    assert!(t_mmap.iter().any(|&v| v != 0), "allocations happened");
    assert_eq!(t_mmap, t_owned);
    assert_eq!(h_mmap, h_owned);
}
