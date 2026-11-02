/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "game_pool_rust_bridge.h"

#include "game_replace.h"

#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern __thread uint32_t g_edi;

typedef uint32_t (*game_pool_read32_fn)(void *context, uint32_t address);
typedef void (*game_pool_write32_fn)(void *context, uint32_t address, uint32_t value);
typedef void (*game_pool_reclaim_fn)(void *context, uint32_t callback, uint32_t argument);
extern uint32_t tsfp_kmem_game_fixed_pool_init(
    void *context, game_pool_read32_fn read32, game_pool_write32_fn write32,
    uint32_t pool, uint32_t stride, uint32_t storage, uint32_t count,
    uint32_t *out_head, uint32_t *out_end) __attribute__((weak));
extern uint32_t tsfp_kmem_game_pool_locked_allocate(uint32_t size, void *context,
                                                    game_pool_read32_fn read32,
                                                    game_pool_write32_fn write32,
                                                    uint32_t *out) __attribute__((weak));
extern uint32_t tsfp_kmem_game_pool_free(uint32_t pointer, void *context,
                                         game_pool_read32_fn read32,
                                         game_pool_write32_fn write32) __attribute__((weak));
extern uint32_t tsfp_kmem_game_pool_clear(void *context,
                                          game_pool_read32_fn read32,
                                          game_pool_write32_fn write32) __attribute__((weak));
extern uint32_t tsfp_kmem_game_pool_reset(
    void *context, game_pool_read32_fn read32, game_pool_write32_fn write32,
    uint8_t (*read8)(void *, uint32_t), void (*write8)(void *, uint32_t, uint8_t))
    __attribute__((weak));
extern uint32_t tsfp_kmem_game_pool_allocate_by_kind(
    uint32_t size, uint32_t kind, uint32_t argument, uint32_t callback,
    void *context, game_pool_read32_fn read32, game_pool_write32_fn write32,
    game_pool_reclaim_fn reclaim, uint32_t *out) __attribute__((weak));
extern uint32_t tsfp_kmem_game_pool_audit_alloc(uint32_t payload, uint32_t size)
    __attribute__((weak));
extern uint32_t tsfp_kmem_game_pool_audit_free(uint32_t payload)
    __attribute__((weak));
extern void tsfp_kmem_game_pool_audit_reset(void) __attribute__((weak));
extern uint32_t tsfp_kmem_game_particle_buffer_allocate(
    uint32_t owner, int32_t entry_count, uint32_t descriptor_kind, void *context,
    game_pool_read32_fn read32, game_pool_write32_fn write32,
    game_pool_reclaim_fn reclaim, uint32_t *out) __attribute__((weak));
extern uint32_t tsfp_kmem_game_particle_buffer_release(
    uint32_t owner, void *context, game_pool_read32_fn read32,
    game_pool_write32_fn write32) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_random_point_pool(
    void *context, game_pool_read32_fn read32, game_pool_write32_fn write32,
    game_pool_reclaim_fn reclaim, uint32_t (*random)(void *), uint32_t *out)
    __attribute__((weak));

static atomic_int g_rust_game_pool_mode = ATOMIC_VAR_INIT(-1);
static atomic_uint_fast64_t g_rust_game_pool_mismatches = ATOMIC_VAR_INIT(0);
static atomic_uint_fast64_t g_rust_game_pool_audit_reports = ATOMIC_VAR_INIT(0);
static atomic_int g_rust_game_pool_audit = ATOMIC_VAR_INIT(-1);

static void game_pool_audit_report(const char *message, uint32_t pointer)
{
    uint64_t count = atomic_fetch_add_explicit(&g_rust_game_pool_audit_reports, 1,
                                               memory_order_relaxed);
    if (count < 10u)
        fprintf(stderr, "[rust-game-pool-audit] %s %08x\n", message, pointer);
}

static int game_pool_audit_enabled(void)
{
    int enabled = atomic_load_explicit(&g_rust_game_pool_audit, memory_order_acquire);
    if (enabled >= 0) return enabled;
    const char *value = getenv("TSFP_RUST_GAME_POOL_AUDIT");
    int selected = value != NULL && strcmp(value, "1") == 0;
    int expected = -1;
    atomic_compare_exchange_strong_explicit(&g_rust_game_pool_audit, &expected, selected,
                                            memory_order_release, memory_order_relaxed);
    return atomic_load_explicit(&g_rust_game_pool_audit, memory_order_acquire);
}

int game_pool_rust_mode(void)
{
    int mode = atomic_load_explicit(&g_rust_game_pool_mode, memory_order_acquire);
    if (mode >= 0) return mode;
    const char *value = getenv("TSFP_RUST_GAME_POOL");
    int selected = GAME_POOL_C;
    if (value != NULL && strcmp(value, "1") == 0) selected = GAME_POOL_RUST;
    else if (value != NULL && strcmp(value, "compare") == 0) selected = GAME_POOL_COMPARE;
    int expected = -1;
    atomic_compare_exchange_strong_explicit(&g_rust_game_pool_mode, &expected, selected,
                                            memory_order_release, memory_order_relaxed);
    return atomic_load_explicit(&g_rust_game_pool_mode, memory_order_acquire);
}

static uint32_t live_read32(void *context, uint32_t address)
{
    (void)context;
    volatile const uint8_t *bytes = (volatile const uint8_t *)game_host_ptr(address);
    return (uint32_t)bytes[0] | ((uint32_t)bytes[1] << 8) |
           ((uint32_t)bytes[2] << 16) | ((uint32_t)bytes[3] << 24);
}

static void live_write32(void *context, uint32_t address, uint32_t value)
{
    (void)context;
    guest_write32(address, value);
}

static void game_pool_release_particle_owner(uint32_t owner, uint32_t return_pc)
{
#if defined(TSFP_RUST_GAME_POOL)
    if (game_pool_rust_mode() == GAME_POOL_RUST &&
        tsfp_kmem_game_particle_buffer_release != NULL) {
        if (tsfp_kmem_game_particle_buffer_release(
                owner, NULL, live_read32, live_write32) != 0u)
            __builtin_trap();
        return;
    }
#endif
    const uint32_t arguments[] = {owner};
    if (game_guest_call(0x0010C0E0u, GAME_CC_cdecl, return_pc, 0u,
                        0u, 0u, arguments, 1u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
}

#if defined(TSFP_RUST_GAME_POOL)
static void game_pool_guest_reclaim(void *context, uint32_t callback, uint32_t argument)
{
    (void)context;
    if (callback == 0x0010C0E0u) {
        game_pool_release_particle_owner(argument, 0x0003EB98u);
        return;
    }
    const uint32_t arguments[] = {argument};
    if (game_guest_call(callback, GAME_CC_cdecl, 0x0003EB98u, 0u, 0u, 0u,
                        arguments, 1u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
}

static uint32_t game_pool_guest_random(void *context)
{
    (void)context;
    if (game_guest_call(0x00153B40u, GAME_CC_cdecl, 0x0021108Au, 0u, 0u, 0u,
                        NULL, 0u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
    return g_eax;
}

int game_pool_rust_fixed_pool_init(uint32_t pool, uint32_t stride,
                                   uint32_t storage, uint32_t count,
                                   uint32_t *out_head, uint32_t *out_end)
{
#if defined(TSFP_RUST_GAME_POOL)
    if (game_pool_rust_mode() != GAME_POOL_RUST ||
        tsfp_kmem_game_fixed_pool_init == NULL || out_head == NULL || out_end == NULL)
        return 0;
    if (tsfp_kmem_game_fixed_pool_init(NULL, live_read32, live_write32, pool, stride,
                                       storage, count, out_head, out_end) != 0u)
        return -1;
    return 1;
#else
    (void)pool; (void)stride; (void)storage; (void)count; (void)out_head; (void)out_end;
    return 0;
#endif
}
#endif

#if !defined(TSFP_RUST_GAME_POOL)
int game_pool_rust_fixed_pool_init(uint32_t pool, uint32_t stride,
                                   uint32_t storage, uint32_t count,
                                   uint32_t *out_head, uint32_t *out_end)
{
    (void)pool; (void)stride; (void)storage; (void)count; (void)out_head; (void)out_end;
    return 0;
}
#endif

int game_pool_rust_allocate_by_kind(uint32_t size, uint32_t kind,
                                    uint32_t argument, uint32_t callback,
                                    uint32_t *result)
{
#if defined(TSFP_RUST_GAME_POOL)
    if (game_pool_rust_mode() != GAME_POOL_RUST ||
        tsfp_kmem_game_pool_allocate_by_kind == NULL || result == NULL)
        return 0;
    uint32_t block = 0;
    if (tsfp_kmem_game_pool_allocate_by_kind(
            size, kind, argument, callback, NULL, live_read32, live_write32,
            game_pool_guest_reclaim, &block) != 0u)
        return -1;
    *result = block;
    if (kind == 1u && block != 0u && game_pool_audit_enabled() &&
        tsfp_kmem_game_pool_audit_alloc != NULL &&
        tsfp_kmem_game_pool_audit_alloc(block, size) != 0u)
        game_pool_audit_report("duplicate live allocation", block);
    return 1;
#else
    (void)size;
    (void)kind;
    (void)argument;
    (void)callback;
    (void)result;
    return 0;
#endif
}

#if defined(TSFP_RUST_GAME_POOL)
static uint8_t live_read8(void *context, uint32_t address)
{
    (void)context;
    return *(volatile const uint8_t *)game_host_ptr(address);
}

static void live_write8(void *context, uint32_t address, uint8_t value)
{
    (void)context;
    *(volatile uint8_t *)game_host_ptr(address) = value;
}
#endif

static uint32_t shadow_read32(void *context, uint32_t address)
{
    game_pool_rust_shadow *shadow = context;
    for (uint32_t i = 0; i < shadow->count; ++i)
        if (shadow->addresses[i] == address) return shadow->values[i];
    return live_read32(NULL, address);
}

static void shadow_write32(void *context, uint32_t address, uint32_t value)
{
    game_pool_rust_shadow *shadow = context;
    for (uint32_t i = 0; i < shadow->count; ++i) {
        if (shadow->addresses[i] == address) {
            shadow->values[i] = value;
            return;
        }
    }
    if (shadow->count == 32u) {
        shadow->overflow = 1;
        return;
    }
    shadow->addresses[shadow->count] = address;
    shadow->values[shadow->count] = value;
    ++shadow->count;
}

int game_pool_rust_begin(uint32_t size, game_pool_rust_call *call,
                         uint32_t *eax, uint32_t *ecx, uint32_t *edx)
{
    *call = (game_pool_rust_call){0};
    int mode = game_pool_rust_mode();
    if (mode == GAME_POOL_C || tsfp_kmem_game_pool_locked_allocate == NULL) return 0;

    uint32_t cursor = live_read32(NULL, 0x006B83CCu);
    call->mode = mode;
    call->old_size = live_read32(NULL, cursor + 4u);
    call->rounded = (size + 0x5Fu) & 0xFFFFFFC0u;

    if (mode == GAME_POOL_RUST) {
        if (tsfp_kmem_game_pool_locked_allocate(size, NULL, live_read32, live_write32,
                                                &call->result) != 0u)
            return 0;
        *eax = call->result;
        *ecx = live_read32(NULL, 0x006B83CCu);
        *edx = call->result != 0u ? call->old_size - call->rounded : call->old_size;
        return 1;
    }

    if (tsfp_kmem_game_pool_locked_allocate(size, &call->shadow, shadow_read32,
                                            shadow_write32, &call->result) != 0u)
        return 0;
    call->active = 1;
    return 0;
}

void game_pool_rust_finish(game_pool_rust_call *call,
                           uint32_t eax, uint32_t ecx, uint32_t edx)
{
    if (!call->active) return;
    uint32_t expected_ecx = shadow_read32(&call->shadow, 0x006B83CCu);
    uint32_t expected_edx = call->result != 0u
                                ? call->old_size - call->rounded : call->old_size;
    int mismatch = call->shadow.overflow || eax != call->result ||
                   ecx != expected_ecx || edx != expected_edx;
    for (uint32_t i = 0; i < call->shadow.count; ++i)
        if (live_read32(NULL, call->shadow.addresses[i]) != call->shadow.values[i])
            mismatch = 1;
    if (mismatch) {
        uint64_t count = atomic_fetch_add_explicit(&g_rust_game_pool_mismatches, 1,
                                                   memory_order_relaxed);
        if (count < 10u)
            fprintf(stderr, "[rust-game-pool] 0x3ECA0 C/Rust mismatch size=%08x C=%08x Rust=%08x\n",
                    call->rounded, eax, call->result);
    }
    call->active = 0;
}

int game_pool_rust_free_begin(uint32_t pointer, game_pool_rust_call *call)
{
    *call = (game_pool_rust_call){0};
    int mode = game_pool_rust_mode();
    if (mode == GAME_POOL_C || tsfp_kmem_game_pool_free == NULL) return 0;
    call->mode = mode;
    if (mode == GAME_POOL_RUST) {
        uint32_t status = tsfp_kmem_game_pool_free(pointer, NULL, live_read32, live_write32);
        if (status == 0u && game_pool_audit_enabled() && tsfp_kmem_game_pool_audit_free != NULL &&
            tsfp_kmem_game_pool_audit_free(pointer) != 0u)
            game_pool_audit_report("free of non-live payload", pointer);
        return status == 0u;
    }
    call->rounded = pointer;
    if (tsfp_kmem_game_pool_free(pointer, &call->shadow, shadow_read32,
                                 shadow_write32) != 0u)
        return 0;
    call->active = 1;
    return 0;
}

void game_pool_rust_free_finish(game_pool_rust_call *call)
{
    if (!call->active) return;
    int mismatch = call->shadow.overflow;
    for (uint32_t i = 0; i < call->shadow.count; ++i)
        if (live_read32(NULL, call->shadow.addresses[i]) != call->shadow.values[i])
            mismatch = 1;
    if (mismatch) {
        uint64_t count = atomic_fetch_add_explicit(&g_rust_game_pool_mismatches, 1,
                                                   memory_order_relaxed);
        if (count < 10u)
            fprintf(stderr, "[rust-game-pool] 0x3EA40 C/Rust mismatch pointer=%08x\n",
                    call->rounded);
    }
    call->active = 0;
}

int game_pool_rust_clear_begin(game_pool_rust_call *call,
                               uint32_t *eax, uint32_t *ecx)
{
    *call = (game_pool_rust_call){0};
    int mode = game_pool_rust_mode();
    if (mode == GAME_POOL_C || tsfp_kmem_game_pool_clear == NULL) return 0;
    call->mode = mode;
    call->old_size = live_read32(NULL, 0x006B7AA8u);
    call->result = live_read32(NULL, 0x006B83C8u);
    if (mode == GAME_POOL_RUST) {
        if (tsfp_kmem_game_pool_clear(NULL, live_read32, live_write32) != 0u) return 0;
        if (call->result != 0u && live_read32(NULL, 0x006B83C8u) == 0u &&
            game_pool_audit_enabled() && tsfp_kmem_game_pool_audit_reset != NULL)
            tsfp_kmem_game_pool_audit_reset();
        *eax = 0u;
        *ecx = call->old_size;
        return 1;
    }
    if (tsfp_kmem_game_pool_clear(&call->shadow, shadow_read32, shadow_write32) != 0u)
        return 0;
    call->active = 1;
    return 0;
}

void game_pool_rust_clear_finish(game_pool_rust_call *call,
                                 uint32_t eax, uint32_t ecx)
{
    if (!call->active) return;
    int mismatch = call->shadow.overflow || eax != 0u || ecx != call->old_size;
    for (uint32_t i = 0; i < call->shadow.count; ++i)
        if (live_read32(NULL, call->shadow.addresses[i]) != call->shadow.values[i])
            mismatch = 1;
    if (mismatch) {
        uint64_t count = atomic_fetch_add_explicit(&g_rust_game_pool_mismatches, 1,
                                                   memory_order_relaxed);
        if (count < 10u)
            fprintf(stderr, "[rust-game-pool] 0x3EA20 C/Rust mismatch gate=%08x\n",
                    call->old_size);
    }
    call->active = 0;
}

static void game_pool_reset_compat(void)
{
    const uint32_t size = live_read32(NULL, 0x004C81B8u);
    const uint32_t base = live_read32(NULL, 0x006B83C8u);
    memset(game_host_ptr(base), 0, size);
    guest_write32(0x006B83CCu, base);
    guest_write32(0x006B83D0u, base);
    guest_write32(0x006B83C0u, size);
    guest_write32(0x006B83C4u, size);
    guest_write32(0x006B83D4u, base + size);
    guest_write32(0x006B83BCu, 0u);
    guest_write32(base, 0u);
    guest_write32(base + 4u, size);
    guest_write32(base + 8u, 0u);
    guest_write32(base + 12u, 0u);
    guest_write32(base + 16u, 0u);
}

#if defined(TSFP_RUST_GAME_POOL)
typedef struct game_pool_reset_compare {
    int mismatch;
} game_pool_reset_compare;

static void reset_compare_write32(void *context, uint32_t address, uint32_t value)
{
    game_pool_reset_compare *compare = context;
    if (live_read32(NULL, address) != value) compare->mismatch = 1;
}

static void reset_compare_write8(void *context, uint32_t address, uint8_t value)
{
    game_pool_reset_compare *compare = context;
    if (live_read8(NULL, address) != value) compare->mismatch = 1;
}

static int game_pool_rust_reset_begin(game_pool_rust_call *call)
{
    *call = (game_pool_rust_call){0};
    const int mode = game_pool_rust_mode();
    if (mode == GAME_POOL_C || tsfp_kmem_game_pool_reset == NULL) return 0;
    call->mode = mode;
    if (mode == GAME_POOL_RUST) {
        uint32_t status = tsfp_kmem_game_pool_reset(NULL, live_read32, live_write32,
                                                    live_read8, live_write8);
        if (status == 0u && game_pool_audit_enabled() && tsfp_kmem_game_pool_audit_reset != NULL)
            tsfp_kmem_game_pool_audit_reset();
        return status == 0u;
    }
    return 0;
}

static void game_pool_rust_reset_finish(game_pool_rust_call *call)
{
    if (call->mode != GAME_POOL_COMPARE || tsfp_kmem_game_pool_reset == NULL) return;
    game_pool_reset_compare compare = {0};
    if (tsfp_kmem_game_pool_reset(&compare, live_read32, reset_compare_write32,
                                  live_read8, reset_compare_write8) != 0u)
        compare.mismatch = 1;
    if (compare.mismatch) {
        const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_pool_mismatches, 1,
                                                             memory_order_relaxed);
        if (previous == 0u)
            fprintf(stderr, "[rust-game-pool] 0x3E9A0 C/Rust pool reset mismatch\n");
    }
}
#endif

GAME_REPLACE_EXACT(0003E9A0, cdecl, 0, void, game_pool_6b83c8_reset)
{
#if defined(TSFP_RUST_GAME_POOL)
    game_pool_rust_call call = {0};
    if (game_pool_rust_reset_begin(&call)) return;
#endif
    game_pool_reset_compat();
#if defined(TSFP_RUST_GAME_POOL)
    game_pool_rust_reset_finish(&call);
#endif
}

static void game_pool_free_compat(uint32_t pointer)
{
    uint32_t block = pointer - 0x20u;
    if (live_read32(NULL, block) != 1u) return;
    uint32_t size = live_read32(NULL, block + 4u);
    guest_write32(block, 0u);
    guest_write32(0x006B83C0u, live_read32(NULL, 0x006B83C0u) + size);
    size = live_read32(NULL, block + 4u);
    uint32_t end = live_read32(NULL, 0x006B83D4u);
    uint32_t next = block + size;
    if (next < end && live_read32(NULL, next) == 0u) {
        uint32_t following = live_read32(NULL, next + 4u);
        guest_write32(block + 4u, following + size);
        if (live_read32(NULL, 0x006B83CCu) == next)
            guest_write32(0x006B83CCu, block);
    }
    uint32_t back = live_read32(NULL, block + 8u);
    uint32_t previous = block - back;
    if (live_read32(NULL, previous) == 0u && previous != block) {
        size = live_read32(NULL, block + 4u);
        uint32_t before = live_read32(NULL, previous + 4u);
        guest_write32(previous + 4u, before + size);
        if (live_read32(NULL, 0x006B83CCu) == block)
            guest_write32(0x006B83CCu, previous);
        block = previous;
    }
    size = live_read32(NULL, block + 4u);
    end = live_read32(NULL, 0x006B83D4u);
    next = block + size;
    if (next < end) guest_write32(next + 8u, size);
}

GAME_REPLACE_EXACT(0003EA40, cdecl, 1, void, game_pool_6b83c8_free_and_coalesce)
{
    uint32_t pointer = live_read32(NULL, g_esp + 4u);
#if defined(TSFP_RUST_GAME_POOL)
    game_pool_rust_call call;
    if (game_pool_rust_free_begin(pointer, &call)) return;
#endif
    game_pool_free_compat(pointer);
#if defined(TSFP_RUST_GAME_POOL)
    game_pool_rust_free_finish(&call);
#endif
}

/* 0x3EAD0 is the null-tolerant public free wrapper around 0x3EA40. Keep the null test at
 * this entry: the lower-level service expects a real block payload and subtracts its header. */
GAME_REPLACE_EXACT(0003EAD0, cdecl, 1, void, game_pool_6b83c8_free_nullable)
{
    uint32_t pointer = live_read32(NULL, g_esp + 4u);
    if (pointer == 0u) return;
#if defined(TSFP_RUST_GAME_POOL)
    game_pool_rust_call call;
    if (game_pool_rust_free_begin(pointer, &call)) return;
#endif
    game_pool_free_compat(pointer);
#if defined(TSFP_RUST_GAME_POOL)
    game_pool_rust_free_finish(&call);
#endif
}

/* The callback route is a TEMPORARY-SHIM: Rust owns allocator transitions while this adapter
 * dispatches the retained guest owner callback. T2048 S3 removes it with guest owner migration. */
GAME_REPLACE_EXACT(0003ED20, cdecl, 4, u32, game_pool_6b83c8_allocate_by_kind)
{
    const uint32_t size = game_stack_arg(0u);
    const uint32_t kind = game_stack_arg(1u);
    const uint32_t argument = game_stack_arg(2u);
    const uint32_t callback = game_stack_arg(3u);
#if defined(TSFP_RUST_GAME_POOL)
    uint32_t result = 0u;
    const int rust = game_pool_rust_allocate_by_kind(size, kind, argument,
                                                    callback, &result);
    if (rust < 0) __builtin_trap();
    if (rust > 0) {
        g_eax = result;
        return;
    }
#endif
    const uint32_t args_kind1[] = {size, argument, callback};
    const uint32_t args_kind2[] = {size};
    if (kind == 1u) {
        if (game_guest_call(0x0003EBD0u, GAME_CC_cdecl, 0x0003ED5Cu, 0u,
                            0u, 0u, args_kind1, 3u) != GAME_GUEST_CALL_OK)
            __builtin_trap();
        guest_write32(0x006B83BCu, 1u);
        return;
    }
    if (kind == 2u) {
        if (game_guest_call(0x0003ECA0u, GAME_CC_cdecl, 0x0003ED5Cu, 0u,
                            0u, 0u, args_kind2, 1u) != GAME_GUEST_CALL_OK)
            __builtin_trap();
        return;
    }
    g_eax = 0u;
    g_ecx = kind - 2u;
}

/* Rust owns particle buffer release, count/stride selection, pool allocation and ordered
 * publication for this owner. Other stored guest callbacks remain a TEMPORARY-SHIM until T2044 S3
 * moves owner identity and lifetime into Rust-owned state. */
GAME_REPLACE_EXACT(0010C190, cdecl, 1, void, game_particle_allocate_owner_buffer)
{
    const uint32_t owner = game_stack_arg(0u);
    if (owner == 0u) return;
    const uint32_t descriptor = live_read32(NULL, owner + 0x0Cu);
    if (descriptor == 0u) return;

    game_pool_release_particle_owner(owner, 0x0010C1A8u);
    const uint32_t count_args[] = {owner, descriptor};
    if (game_guest_call(0x0010C120u, GAME_CC_cdecl, 0x0010C1AFu, 0u,
                        0u, 0u, count_args, 2u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
    const int32_t entry_count = (int32_t)g_eax;
    /* The original reads the kind through the emitter definition: [[descriptor + 0x4C] + 0xC].
     * T1703/I-T1703-8: reading [descriptor + 0xC] picked the 0x34 stride for 0x58-stride kinds,
     * a pool buffer overrun that corrupted the next block header (Story L1 crash in 0x3EAF0). */
    const uint32_t definition = live_read32(NULL, descriptor + 0x4Cu);
    const uint32_t descriptor_kind = live_read32(NULL, definition + 0x0Cu);

#if defined(TSFP_RUST_GAME_POOL)
    if (game_pool_rust_mode() == GAME_POOL_RUST &&
        tsfp_kmem_game_particle_buffer_allocate != NULL) {
        uint32_t payload = 0u;
        if (tsfp_kmem_game_particle_buffer_allocate(
                owner, entry_count,
                descriptor_kind,
                NULL, live_read32, live_write32,
                game_pool_guest_reclaim, &payload) != 0u)
            __builtin_trap();
        g_eax = payload;
        return;
    }
#endif

    uint32_t count = (uint32_t)entry_count;
    if (entry_count >= 500) count = 500u;
    const uint32_t stride = (descriptor_kind == 5u || descriptor_kind == 6u)
                                ? 0x58u : 0x34u;
    const uint32_t size = count * stride;
    const uint32_t alloc_args[] = {size, 1u, owner, 0x0010C0E0u};
    const uint32_t return_pc = stride == 0x58u ? 0x0010C204u : 0x0010C1E4u;
    if (game_guest_call(0x0003ED20u, GAME_CC_cdecl, return_pc, 0u,
                        0u, 0u, alloc_args, 4u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
    const uint32_t payload = g_eax;
    guest_write32(owner + 0x44u, count);
    guest_write32(owner + 0x40u, payload);
}

GAME_REPLACE_EXACT(00211040, cdecl, 0, void, game_alloc_random_point_pool_75c0cc)
{
    const uint32_t saved_ecx = g_ecx;
    const uint32_t saved_edi = g_edi;
#if defined(TSFP_RUST_GAME_POOL)
    if (game_pool_rust_mode() == GAME_POOL_RUST &&
        tsfp_kmem_game_wrapper_alloc_random_point_pool != NULL) {
        uint32_t count = 0u;
        if (tsfp_kmem_game_wrapper_alloc_random_point_pool(
                NULL, live_read32, live_write32, game_pool_guest_reclaim,
                game_pool_guest_random, &count) != 0u)
            __builtin_trap();
        g_eax = count;
        g_ecx = saved_ecx;
        g_edi = saved_edi;
        return;
    }
#endif
    const uint32_t arguments[4] = {0x1770u, 1u, 0x75C0CCu, 0x210FF0u};
    if (game_guest_call(0x0003ED20u, GAME_CC_cdecl, 0x00211058u, 0u, 0u, 0u,
                        arguments, 4u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
    const uint32_t result = g_eax;
    guest_write32(0x0075C0CCu, result);
    guest_write32(0x0075C0A8u, 0u);
    g_eax = result;
    g_ecx = saved_ecx;
    g_edi = saved_edi;
}
