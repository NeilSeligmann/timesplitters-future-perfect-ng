/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3935: the kmem ABI. Rust (crates/tsfp-kmem-capi) owns guest memory, heaps, the Mm and NtVirtualMemory
 * state and the Ex pool; src/xbox/kmem_abi.c packs the guest_mem.h API into its op interface and
 * src/xbox/kmem_rust.c binds the kernel ordinals. The C model it replaced is retired.
 */
#ifndef TSFP_XBOX_KMEM_ABI_H
#define TSFP_XBOX_KMEM_ABI_H

#include <stddef.h>
#include <stdint.h>

#include "guest_mem.h"

enum {
    KMEM_OP_REGION_ALLOC = 1, KMEM_OP_REGION_FREE, KMEM_OP_REGION_AT, KMEM_OP_REGION_CONTAINING,
    KMEM_OP_SET_PROTECT, KMEM_OP_SET_PERSIST, KMEM_OP_SET_STATE, KMEM_OP_SET_USAGE, KMEM_OP_PHYSICAL,
    KMEM_OP_GENERATION, KMEM_OP_VIRT_FROM_PHYS, KMEM_OP_HEAP_CREATE, KMEM_OP_HEAP_DESTROY, KMEM_OP_HEAP_VALID,
    KMEM_OP_HEAP_ALLOC, KMEM_OP_HEAP_FREE, KMEM_OP_HEAP_BLOCK_SIZE, KMEM_OP_HEAP_REALLOC, KMEM_OP_RESET,
    KMEM_OP_REGION_COUNT, KMEM_OP_HEAP_COUNT, KMEM_OP_MAPPED_BYTES, KMEM_OP_STATISTICS, KMEM_OP_POOL_RESET,
    KMEM_OP_REGISTER, KMEM_OP_COUNT
};

/* One Rust capi op under the kmem lock (recursive). */
uint32_t kmem_op(uint32_t op, const uint32_t *in, uint32_t nin, uint32_t *out, uint32_t nout);
void kmem_lock(void);
void kmem_unlock(void);

/* Register the Mm*, Nt*VirtualMemory and Ex pool ordinals with the kernel HLE (kmem_rust.c). */
size_t kmem_register_ordinals(void);
void kmem_pool_reset(void);

#endif /* TSFP_XBOX_KMEM_ABI_H */
