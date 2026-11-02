/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3935: the host's guest-memory API (guest_mem.h) answered by the Rust KernelMemory (crates/tsfp-kmem-capi).
 * Rust owns the region registry, the heaps, the low-4GB mmap placement and the Mm, Nt and Ex pool state; this file
 * only packs arguments into the capi op ABI. It replaces the retired C guest_mem.c / kernel_memory.c /
 * kernel_pool.c (docs/t3935-kmem-retirement-plan.md).
 *
 * One recursive lock serialises every call, so the multi-step ordinal thunks of kmem_rust.c stay atomic with
 * respect to the other guest threads.
 */
#include "kmem_abi.h"

#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>

#include "kernel_call.h"

uint32_t tsfp_kmem_init(uint32_t mode, void (*probe_begin)(void), void (*probe_flush)(void));
uint32_t tsfp_kmem_op(uint32_t op, const uint32_t *in, uint32_t nin, uint32_t *out, uint32_t nout);

#define KMEM_MODE_RUST 1u
#define REGION_WORDS 11u

static pthread_mutex_t kmem_mutex;
static pthread_once_t kmem_once = PTHREAD_ONCE_INIT;

static void kmem_start(void)
{
    pthread_mutexattr_t attr;
    pthread_mutexattr_init(&attr);
    pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_RECURSIVE);
    pthread_mutex_init(&kmem_mutex, &attr);
    if (tsfp_kmem_init(KMEM_MODE_RUST, kernel_guest_probe_change_begin, kernel_guest_probe_cache_flush) !=
        KMEM_MODE_RUST) {
        fputs("kmem: tsfp_kmem_init failed; the Rust kernel memory is the only implementation\n", stderr);
        abort();
    }
}

__attribute__((constructor)) static void kmem_constructor(void)
{
    (void)pthread_once(&kmem_once, kmem_start);
}

uint32_t kmem_op(uint32_t op, const uint32_t *in, uint32_t nin, uint32_t *out, uint32_t nout)
{
    (void)pthread_once(&kmem_once, kmem_start);
    pthread_mutex_lock(&kmem_mutex);
    const uint32_t result = tsfp_kmem_op(op, in, nin, out, nout);
    pthread_mutex_unlock(&kmem_mutex);
    return result;
}

void kmem_lock(void)
{
    (void)pthread_once(&kmem_once, kmem_start);
    pthread_mutex_lock(&kmem_mutex);
}

void kmem_unlock(void)
{
    pthread_mutex_unlock(&kmem_mutex);
}

/* Region queries return a pointer into a per-thread ring of snapshots (the Rust registry is not addressable). */
static __thread guest_region region_ring[16];
static __thread unsigned region_ring_next;

static const guest_region *region_from_words(const uint32_t w[REGION_WORDS])
{
    guest_region *r = &region_ring[region_ring_next++ % 16u];
    r->address = w[0]; r->size = w[1]; r->requested = w[2]; r->physical = w[3]; r->protect = w[4];
    r->alloc_protect = w[5]; r->state = w[6]; r->contiguous = w[7] != 0; r->persist = w[8] != 0;
    r->generation = ((uint64_t)w[10] << 32) | w[9];
    return r;
}

kernel_guest_ptr kmem_region_alloc_typed(const guest_region_request *request, nt_status *status,
                                         guest_memory_usage usage)
{
    uint32_t in[9] = {request->bytes, request->alignment, request->lowest_physical, request->highest_physical,
                      request->protect, request->state, request->contiguous, request->fixed_base, (uint32_t)usage};
    uint32_t out[1] = {0};
    const kernel_guest_ptr result = kmem_op(KMEM_OP_REGION_ALLOC, in, 9, out, 1);
    if (status) *status = out[0];
    return result;
}

kernel_guest_ptr kmem_region_alloc(const guest_region_request *request, nt_status *status)
{
    return kmem_region_alloc_typed(request, status, GUEST_MEMORY_VIRTUAL);
}

uint32_t kmem_heap_create_typed(uint32_t flags, uint32_t initial, uint32_t maximum, guest_memory_usage usage)
{
    const uint32_t in[4] = {flags, initial, maximum, (uint32_t)usage};
    return kmem_op(KMEM_OP_HEAP_CREATE, in, 4, NULL, 0);
}

uint32_t kmem_heap_create(uint32_t flags, uint32_t initial, uint32_t maximum)
{
    return kmem_heap_create_typed(flags, initial, maximum, GUEST_MEMORY_VIRTUAL);
}

bool kmem_region_free(kernel_guest_ptr base)
{
    return kmem_op(KMEM_OP_REGION_FREE, &base, 1, NULL, 0) != 0u;
}

bool kmem_region_set_protect(kernel_guest_ptr base, uint32_t bytes, uint32_t protect)
{
    const uint32_t in[3] = {base, bytes, protect};
    return kmem_op(KMEM_OP_SET_PROTECT, in, 3, NULL, 0) != 0u;
}

bool kmem_region_set_persist(kernel_guest_ptr base, bool persist)
{
    const uint32_t in[2] = {base, persist ? 1u : 0u};
    return kmem_op(KMEM_OP_SET_PERSIST, in, 2, NULL, 0) != 0u;
}

bool kmem_region_set_state(kernel_guest_ptr base, uint32_t state)
{
    const uint32_t in[2] = {base, state};
    return kmem_op(KMEM_OP_SET_STATE, in, 2, NULL, 0) != 0u;
}

bool kmem_region_set_usage(kernel_guest_ptr base, uint32_t bytes, guest_memory_usage usage)
{
    const uint32_t in[3] = {base, bytes, (uint32_t)usage};
    return kmem_op(KMEM_OP_SET_USAGE, in, 3, NULL, 0) != 0u;
}

uint32_t kmem_physical_address(kernel_guest_ptr addr)
{
    return kmem_op(KMEM_OP_PHYSICAL, &addr, 1, NULL, 0);
}

bool kmem_heap_destroy(uint32_t handle)
{
    return kmem_op(KMEM_OP_HEAP_DESTROY, &handle, 1, NULL, 0) != 0u;
}

bool kmem_heap_valid(uint32_t handle)
{
    return kmem_op(KMEM_OP_HEAP_VALID, &handle, 1, NULL, 0) != 0u;
}

kernel_guest_ptr kmem_heap_alloc(uint32_t handle, uint32_t bytes)
{
    const uint32_t in[2] = {handle, bytes};
    return kmem_op(KMEM_OP_HEAP_ALLOC, in, 2, NULL, 0);
}

bool kmem_heap_free(uint32_t handle, kernel_guest_ptr ptr)
{
    const uint32_t in[2] = {handle, ptr};
    return kmem_op(KMEM_OP_HEAP_FREE, in, 2, NULL, 0) != 0u;
}

kernel_guest_ptr kmem_heap_realloc(uint32_t handle, kernel_guest_ptr ptr, uint32_t bytes)
{
    const uint32_t in[3] = {handle, ptr, bytes};
    return kmem_op(KMEM_OP_HEAP_REALLOC, in, 3, NULL, 0);
}

uint64_t kmem_allocation_generation(kernel_guest_ptr addr)
{
    uint32_t out[2] = {0, 0};
    kmem_op(KMEM_OP_GENERATION, &addr, 1, out, 2);
    return ((uint64_t)out[1] << 32) | out[0];
}

bool kmem_virtual_from_physical(uint32_t physical, kernel_guest_ptr *out_address)
{
    uint32_t out[1] = {0};
    const bool result = kmem_op(KMEM_OP_VIRT_FROM_PHYS, &physical, 1, out, 1) != 0u;
    if (result && out_address) *out_address = out[0];
    return result;
}

bool kmem_heap_block_size(uint32_t handle, kernel_guest_ptr ptr, uint32_t *out_size)
{
    const uint32_t in[2] = {handle, ptr};
    uint32_t out[1] = {0};
    const bool result = kmem_op(KMEM_OP_HEAP_BLOCK_SIZE, in, 2, out, 1) != 0u;
    if (result && out_size) *out_size = out[0];
    return result;
}

static const guest_region *region_lookup(uint32_t op, kernel_guest_ptr addr)
{
    uint32_t words[REGION_WORDS];
    return kmem_op(op, &addr, 1, words, REGION_WORDS) != 0u ? region_from_words(words) : NULL;
}

const guest_region *kmem_region_at(kernel_guest_ptr base)
{
    return region_lookup(KMEM_OP_REGION_AT, base);
}

const guest_region *kmem_region_containing(kernel_guest_ptr addr)
{
    return region_lookup(KMEM_OP_REGION_CONTAINING, addr);
}

void kmem_reset(void)
{
    kmem_op(KMEM_OP_RESET, NULL, 0, NULL, 0);
}

void kmem_pool_reset(void)
{
    kmem_op(KMEM_OP_POOL_RESET, NULL, 0, NULL, 0);
}

size_t kmem_region_count(void)
{
    return kmem_op(KMEM_OP_REGION_COUNT, NULL, 0, NULL, 0);
}

size_t kmem_heap_count(void)
{
    return kmem_op(KMEM_OP_HEAP_COUNT, NULL, 0, NULL, 0);
}

uint64_t kmem_mapped_bytes(void)
{
    uint32_t out[2] = {0, 0};
    kmem_op(KMEM_OP_MAPPED_BYTES, NULL, 0, out, 2);
    return ((uint64_t)out[1] << 32) | out[0];
}

void kmem_statistics(guest_memory_statistics *out)
{
    if (!out) return;
    uint32_t w[10] = {0};
    kmem_op(KMEM_OP_STATISTICS, NULL, 0, w, 10);
    out->virtual_committed_bytes = ((uint64_t)w[1] << 32) | w[0];
    out->virtual_reserved_bytes = ((uint64_t)w[3] << 32) | w[2];
    out->pool_pages = ((uint64_t)w[5] << 32) | w[4];
    out->stack_pages = ((uint64_t)w[7] << 32) | w[6];
    out->cache_pages = ((uint64_t)w[9] << 32) | w[8];
}

/* The Rust MappedGuest write witness symbols (T3906 direct-pointer class): the capture driver that consumed
 * them was retired with the C model, so they never record. */
volatile uint8_t tsfp_hostmem_direct_active;
uint64_t tsfp_hostmem_direct_prepare(void) { return 0u; }
void tsfp_hostmem_direct_write(uint64_t token, uint32_t addr, const void *bytes, size_t length)
{
    (void)token; (void)addr; (void)bytes; (void)length;
}
