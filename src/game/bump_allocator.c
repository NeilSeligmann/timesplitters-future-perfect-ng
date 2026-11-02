/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * A bump allocator over a region the title set up elsewhere.
 */
#include "game_replace.h"

/* The two guest globals the allocator maintains: where the next block starts, and how
 * many bytes have been handed out so far. */
#define BUMP_CURSOR_ADDRESS 0x006B839Cu
#define BUMP_TOTAL_ADDRESS 0x006B83B8u
#define BUMP_ALIGNMENT 16u

#if defined(TSFP_RUST_GAME_BUMP)
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef uint32_t (*game_read32_fn)(void *context, uint32_t address);
typedef void (*game_write32_fn)(void *context, uint32_t address, uint32_t value);
extern uint32_t tsfp_kmem_game_bump_allocate(uint32_t size, void *context,
                                              game_read32_fn read32, game_write32_fn write32,
                                              uint32_t *out)
    __attribute__((weak));

enum { BUMP_MODE_C = 0, BUMP_MODE_RUST = 1, BUMP_MODE_COMPARE = 2 };
static atomic_int g_rust_bump_mode = ATOMIC_VAR_INIT(-1);
static atomic_uint_fast64_t g_rust_bump_mismatches = ATOMIC_VAR_INIT(0);

static int rust_bump_mode(void)
{
    int mode = atomic_load_explicit(&g_rust_bump_mode, memory_order_acquire);
    if (mode >= 0) return mode;

    const char *value = getenv("TSFP_RUST_GAME_BUMP");
    int selected = BUMP_MODE_RUST; /* T3879 MEM-3: default on, TSFP_RUST_GAME_BUMP=0 selects the lifted C */
    if (value != NULL && strcmp(value, "0") == 0) selected = BUMP_MODE_C;
    else if (value != NULL && strcmp(value, "compare") == 0) selected = BUMP_MODE_COMPARE;

    int expected = -1;
    atomic_compare_exchange_strong_explicit(&g_rust_bump_mode, &expected, selected,
                                            memory_order_release, memory_order_relaxed);
    return atomic_load_explicit(&g_rust_bump_mode, memory_order_acquire);
}

static uint32_t live_guest_read32(void *context, uint32_t address)
{
    (void)context;
    return guest_read32(address);
}

static void live_guest_write32(void *context, uint32_t address, uint32_t value)
{
    (void)context;
    guest_write32(address, value);
}

typedef struct bump_shadow {
    uint32_t cursor;
    uint32_t total;
} bump_shadow;

static uint32_t shadow_guest_read32(void *context, uint32_t address)
{
    bump_shadow *shadow = context;
    if (address == BUMP_CURSOR_ADDRESS) return shadow->cursor;
    if (address == BUMP_TOTAL_ADDRESS) return shadow->total;
    return guest_read32(address);
}

static void shadow_guest_write32(void *context, uint32_t address, uint32_t value)
{
    bump_shadow *shadow = context;
    if (address == BUMP_CURSOR_ADDRESS) shadow->cursor = value;
    else if (address == BUMP_TOTAL_ADDRESS) shadow->total = value;
    else guest_write32(address, value);
}

static int rust_bump_allocate(uint32_t size, void *context, game_read32_fn read32,
                              game_write32_fn write32, uint32_t *block)
{
    if (tsfp_kmem_game_bump_allocate == NULL) return 0;
    return tsfp_kmem_game_bump_allocate(size, context, read32, write32, block) == 0;
}
#endif

/* Hands out `size` bytes rounded up to a multiple of 16 and returns the block's address.
 *
 * Nothing is checked: there is no bounds test and no zeroing, exactly as in the original.
 * The rounding is done in 32 bits, so a size within 15 of 2^32 wraps to a small block
 * (size + 15 overflows) and the cursor and total advance by that small amount. That
 * wraparound is part of the contract callers see, so it is kept and not "fixed".
 */
static uint32_t game_bump_allocate_c(uint32_t size)
{
    uint32_t block_size = (size + (BUMP_ALIGNMENT - 1u)) & ~(BUMP_ALIGNMENT - 1u);
    uint32_t block = guest_read32(BUMP_CURSOR_ADDRESS);
    guest_write32(BUMP_CURSOR_ADDRESS, block + block_size);
    guest_write32(BUMP_TOTAL_ADDRESS, guest_read32(BUMP_TOTAL_ADDRESS) + block_size);
    return block;
}

static uint32_t game_bump_allocate(uint32_t size)
{
#if defined(TSFP_RUST_GAME_BUMP)
    int mode = rust_bump_mode();
    if (mode == BUMP_MODE_RUST) {
        uint32_t block;
        if (rust_bump_allocate(size, NULL, live_guest_read32, live_guest_write32, &block)) return block;
        return game_bump_allocate_c(size);
    }
    if (mode == BUMP_MODE_COMPARE) {
        bump_shadow shadow = {
            .cursor = guest_read32(BUMP_CURSOR_ADDRESS),
            .total = guest_read32(BUMP_TOTAL_ADDRESS),
        };
        uint32_t c_block = game_bump_allocate_c(size);
        uint32_t cursor_after = guest_read32(BUMP_CURSOR_ADDRESS);
        uint32_t total_after = guest_read32(BUMP_TOTAL_ADDRESS);
        uint32_t rust_block;
        if (rust_bump_allocate(size, &shadow, shadow_guest_read32, shadow_guest_write32, &rust_block)) {
            if (rust_block != c_block || shadow.cursor != cursor_after || shadow.total != total_after) {
                uint64_t mismatches = atomic_fetch_add_explicit(&g_rust_bump_mismatches, 1,
                                                                memory_order_relaxed);
                if (mismatches == 0) {
                    fprintf(stderr, "game-bump: first mismatch size=%08x C=(%08x,%08x,%08x) Rust=(%08x,%08x,%08x)\n",
                            size, c_block, cursor_after, total_after, rust_block, shadow.cursor, shadow.total);
                }
            }
        }
        return c_block;
    }
#endif
    return game_bump_allocate_c(size);
}

GAME_REPLACE_EXACT(0003E970, cdecl, 1, u32, game_bump_allocate)
{
    uint32_t size = game_stack_arg(0);
    g_eax = game_bump_allocate(size);
    g_ecx = (size + (BUMP_ALIGNMENT - 1u)) & ~(BUMP_ALIGNMENT - 1u);
    g_edx = g_eax + g_ecx;
}
