// SPDX-License-Identifier: GPL-3.0-or-later
//! Value-in, value-out operation table over `KernelMemory` (one numeric op per C entry point).
//!
//! The C side (`src/xbox/kmem_rust.c`) reads the guest stack arguments and the guest out-parameters and calls
//! [`dispatch`] with plain `u32` values, so no guest pointer ever crosses into Rust. Ops below 1000 are the
//! `guest_mem.h` API (used by C callers that do not go through an ordinal: threads, GPU, audio, host startup).
//! Ops `1000 + ordinal` are the kernel exports; their out-values follow the `Option` convention of
//! `kernel_mm.rs`: `out[0] != 0` means "the C handler writes the first out-parameter".
use tsfp_engine_memory::guest_regions::{
    GuestRegions, Placement, Region, RegionRequest, Usage, nt,
};
use tsfp_engine_memory::kernel_mm::KernelMemory;

pub const OP_REGION_ALLOC: u32 = 1;
pub const OP_REGION_FREE: u32 = 2;
pub const OP_REGION_AT: u32 = 3;
pub const OP_REGION_CONTAINING: u32 = 4;
pub const OP_SET_PROTECT: u32 = 5;
pub const OP_SET_PERSIST: u32 = 6;
pub const OP_SET_STATE: u32 = 7;
pub const OP_SET_USAGE: u32 = 8;
pub const OP_PHYSICAL: u32 = 9;
pub const OP_GENERATION: u32 = 10;
pub const OP_VIRT_FROM_PHYS: u32 = 11;
pub const OP_HEAP_CREATE: u32 = 12;
pub const OP_HEAP_DESTROY: u32 = 13;
pub const OP_HEAP_VALID: u32 = 14;
pub const OP_HEAP_ALLOC: u32 = 15;
pub const OP_HEAP_FREE: u32 = 16;
pub const OP_HEAP_BLOCK_SIZE: u32 = 17;
pub const OP_HEAP_REALLOC: u32 = 18;
pub const OP_RESET: u32 = 19;
pub const OP_REGION_COUNT: u32 = 20;
pub const OP_HEAP_COUNT: u32 = 21;
pub const OP_MAPPED_BYTES: u32 = 22;
pub const OP_STATISTICS: u32 = 23;
pub const OP_POOL_RESET: u32 = 24;
pub const OP_KMEM_REGISTER: u32 = 25;
/// Kernel export ordinals (1000 + ordinal).
pub const ORD_BASE: u32 = 1000;

/// Words per region record of [`region_words`].
pub const REGION_WORDS: usize = 11;

fn usage_of(value: u32) -> Usage {
    match value {
        1 => Usage::Pool,
        2 => Usage::Stack,
        3 => Usage::Cache,
        _ => Usage::Virtual,
    }
}

/// A region as the 11 words the C side compares against `guest_region`.
pub fn region_words(r: &Region) -> [u32; REGION_WORDS] {
    [
        r.address,
        r.size,
        r.requested,
        r.physical,
        r.protect,
        r.alloc_protect,
        r.state,
        r.contiguous as u32,
        r.persist as u32,
        r.generation as u32,
        (r.generation >> 32) as u32,
    ]
}

fn put(out: &mut [u32], index: usize, value: u32) {
    if let Some(slot) = out.get_mut(index) {
        *slot = value;
    }
}

fn put64(out: &mut [u32], index: usize, value: u64) {
    put(out, index, value as u32);
    put(out, index + 1, (value >> 32) as u32);
}

fn regions_op<P: Placement>(
    g: &mut GuestRegions<P>,
    op: u32,
    a: &dyn Fn(usize) -> u32,
    out: &mut [u32],
) -> u32 {
    match op {
        OP_REGION_ALLOC => {
            let request = RegionRequest {
                bytes: a(0),
                alignment: a(1),
                lowest_physical: a(2),
                highest_physical: a(3),
                protect: a(4),
                state: a(5),
                contiguous: a(6) != 0,
                fixed_base: a(7),
            };
            match g.region_alloc_typed(&request, usage_of(a(8))) {
                Ok(address) => {
                    put(out, 0, nt::SUCCESS);
                    address
                }
                Err(status) => {
                    put(out, 0, status);
                    0
                }
            }
        }
        OP_REGION_FREE => g.region_free(a(0)) as u32,
        OP_REGION_AT | OP_REGION_CONTAINING => {
            let found = if op == OP_REGION_AT {
                g.region_at(a(0)).copied()
            } else {
                g.region_containing(a(0)).copied()
            };
            match found {
                Some(r) => {
                    for (i, w) in region_words(&r).iter().enumerate() {
                        put(out, i, *w);
                    }
                    1
                }
                None => 0,
            }
        }
        OP_SET_PROTECT => g.region_set_protect(a(0), a(1), a(2)) as u32,
        OP_SET_PERSIST => g.region_set_persist(a(0), a(1) != 0) as u32,
        OP_SET_STATE => g.region_set_state(a(0), a(1)) as u32,
        OP_SET_USAGE => g.region_set_usage(a(0), a(1), usage_of(a(2))) as u32,
        OP_PHYSICAL => g.physical_address(a(0)),
        OP_GENERATION => {
            put64(out, 0, g.allocation_generation(a(0)));
            0
        }
        OP_VIRT_FROM_PHYS => match g.virtual_from_physical(a(0)) {
            Some(address) => {
                put(out, 0, address);
                1
            }
            None => 0,
        },
        OP_HEAP_CREATE => g.heap_create_typed(a(0), a(1), a(2), usage_of(a(3))),
        OP_HEAP_DESTROY => g.heap_destroy(a(0)) as u32,
        OP_HEAP_VALID => g.heap_valid(a(0)) as u32,
        OP_HEAP_ALLOC => g.heap_alloc(a(0), a(1)),
        OP_HEAP_FREE => g.heap_free(a(0), a(1)) as u32,
        OP_HEAP_BLOCK_SIZE => match g.heap_block_size(a(0), a(1)) {
            Some(size) => {
                put(out, 0, size);
                1
            }
            None => 0,
        },
        OP_HEAP_REALLOC => g.heap_realloc(a(0), a(1), a(2)),
        OP_RESET => {
            g.reset();
            0
        }
        OP_REGION_COUNT => g.region_count() as u32,
        OP_HEAP_COUNT => g.heap_count() as u32,
        OP_MAPPED_BYTES => {
            put64(out, 0, g.mapped_bytes());
            0
        }
        OP_STATISTICS => {
            let s = g.statistics();
            put64(out, 0, s.virtual_committed_bytes);
            put64(out, 2, s.virtual_reserved_bytes);
            put64(out, 4, s.pool_pages);
            put64(out, 6, s.stack_pages);
            put64(out, 8, s.cache_pages);
            0
        }
        _ => u32::MAX,
    }
}

/// Run one op. `inp` shorter than an op needs reads as zeros. Unknown op returns `u32::MAX`.
pub fn dispatch<P: Placement>(
    km: &mut KernelMemory<P>,
    op: u32,
    inp: &[u32],
    out: &mut [u32],
) -> u32 {
    let result = dispatch_inner(km, op, inp, out);
    #[cfg(feature = "fault-inject")]
    {
        return fault::apply(km, op, inp, out, result);
    }
    #[cfg(not(feature = "fault-inject"))]
    result
}

/// Detection-power check of the live comparison (never in the default build): `TSFP_KMEM_FAULT=status|region|heap|statistics|statistics-status`
/// makes the model deviate from the C code on purpose, the compare mode must then report mismatches.
#[cfg(feature = "fault-inject")]
mod fault {
    use super::*;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};

    static KIND: OnceLock<String> = OnceLock::new();
    static COUNT: AtomicU64 = AtomicU64::new(0);

    pub fn apply<P: Placement>(
        km: &mut KernelMemory<P>,
        op: u32,
        inp: &[u32],
        out: &mut [u32],
        result: u32,
    ) -> u32 {
        let kind = KIND.get_or_init(|| std::env::var("TSFP_KMEM_FAULT").unwrap_or_default());
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        match (kind.as_str(), op) {
            ("statistics", o) if o == ORD_BASE + 181 && result == nt::SUCCESS => {
                if let Some(field) = out.get_mut(5) {
                    *field ^= 1;
                }
                result
            }
            ("statistics-status", o) if o == ORD_BASE + 181 => result ^ 0x10,
            ("status", o) if o == ORD_BASE + 199 && n % 7 == 0 => result ^ 0x10,
            ("region", OP_REGION_ALLOC) if result != 0 && n % 5 == 0 => {
                km.regions.region_set_protect(result, 0, nt::PAGE_READONLY);
                result
            }
            ("heap", OP_HEAP_ALLOC) if n % 50 == 0 => {
                km.regions.heap_alloc(inp.first().copied().unwrap_or(0), 16);
                result
            }
            _ => result,
        }
    }
}

fn dispatch_inner<P: Placement>(
    km: &mut KernelMemory<P>,
    op: u32,
    inp: &[u32],
    out: &mut [u32],
) -> u32 {
    let a = |i: usize| inp.get(i).copied().unwrap_or(0);
    if op < ORD_BASE {
        if op == OP_KMEM_REGISTER {
            km.register();
            return 0;
        }
        if op == OP_POOL_RESET {
            km.pool_reset();
            return 0;
        }
        return regions_op(&mut km.regions, op, &a, out);
    }
    match op - ORD_BASE {
        181 => {
            let wide = |i| u64::from(a(i)) | (u64::from(a(i + 1)) << 32);
            let host = (a(1) != 0).then(|| (wide(2), wide(4)));
            match km.mm_query_statistics(a(0), host, wide(6)) {
                Ok(words) => {
                    for (i, word) in words.into_iter().enumerate() {
                        put(out, i, word);
                    }
                    nt::SUCCESS
                }
                Err(status) => status,
            }
        }
        165 => km.mm_allocate_contiguous_memory(a(0)),
        166 => km.mm_allocate_contiguous_memory_ex(a(0), a(1), a(2), a(3), a(4)),
        169 => km.mm_create_kernel_stack(a(0), a(1)),
        170 => {
            km.mm_delete_kernel_stack(a(0), a(1));
            0
        }
        171 => {
            km.mm_free_contiguous_memory(a(0));
            0
        }
        173 => km.mm_get_physical_address(a(0)),
        175 => {
            km.mm_lock_unlock_buffer_pages(a(0), a(1), a(2));
            0
        }
        176 => {
            km.mm_lock_unlock_physical_page(a(0), a(1));
            0
        }
        178 => {
            km.mm_persist_contiguous_memory(a(0), a(1), a(2));
            0
        }
        179 => km.mm_query_address_protect(a(0)),
        180 => km.mm_query_allocation_size(a(0)),
        182 => {
            km.mm_set_address_protect(a(0), a(1), a(2));
            0
        }
        184 => {
            let r = km.nt_allocate_virtual_memory(a(0), a(1), a(2), a(3));
            put(out, 0, r.base.is_some() as u32);
            put(out, 1, r.base.unwrap_or(0));
            put(out, 2, r.size.is_some() as u32);
            put(out, 3, r.size.unwrap_or(0));
            r.status
        }
        199 => {
            let r = km.nt_free_virtual_memory(a(0), a(1), a(2));
            put(out, 0, r.base.is_some() as u32);
            put(out, 1, r.base.unwrap_or(0));
            put(out, 2, r.size.is_some() as u32);
            put(out, 3, r.size.unwrap_or(0));
            r.status
        }
        217 => {
            for (i, w) in km.nt_query_virtual_memory(a(0)).iter().enumerate() {
                put(out, i, *w);
            }
            nt::SUCCESS
        }
        15 => km.ex_allocate_pool_with_tag(a(0), a(1)),
        17 => {
            km.ex_free_pool(a(0));
            0
        }
        23 => km.ex_query_pool_block_size(a(0)),
        _ => u32::MAX,
    }
}
