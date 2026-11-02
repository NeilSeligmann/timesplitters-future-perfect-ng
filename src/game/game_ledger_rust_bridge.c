/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "game_ledger_rust_bridge.h"

#include "game_replace.h"

#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

typedef uint32_t (*game_ledger_read32_fn)(void *, uint32_t);
typedef void (*game_ledger_write32_fn)(void *, uint32_t, uint32_t);
typedef uint32_t (*game_ledger_read_argument_fn)(void *);
typedef uint8_t (*game_ledger_read8_fn)(void *, uint32_t);
typedef void (*game_ledger_write8_fn)(void *, uint32_t, uint8_t);
typedef void (*game_ledger_report_fn)(void *, uint32_t);

extern __thread uint32_t g_esp;

extern uint32_t tsfp_kmem_game_ledger_allocate(
    uint32_t requested, void *context, game_ledger_read32_fn read32,
    game_ledger_write32_fn write32, uint32_t *out) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_ensure_ledger_buffer(
    uint32_t force, void *context, game_ledger_read32_fn read32,
    game_ledger_write32_fn write32) __attribute__((weak));
extern uint32_t tsfp_kmem_game_ledger_history_reset(
    uint32_t mode, void *context, game_ledger_read32_fn read32,
    game_ledger_write32_fn write32, game_ledger_read8_fn read8,
    game_ledger_write8_fn write8, game_ledger_report_fn report,
    game_ledger_report_fn callout) __attribute__((weak));
extern uint32_t tsfp_kmem_game_ledger_release(
    uint32_t count, uint32_t block, void *context,
    game_ledger_read32_fn read32, game_ledger_write32_fn write32,
    uint32_t *eax, uint32_t *ecx, uint32_t *edx) __attribute__((weak));
extern uint32_t tsfp_kmem_game_ledger_secondary_append(
    void *context, game_ledger_read32_fn read32, game_ledger_write32_fn write32,
    game_ledger_read_argument_fn read_argument,
    uint32_t *eax, uint32_t *ecx, uint32_t *edx) __attribute__((weak));
extern uint32_t tsfp_kmem_game_ledger_secondary_release(
    uint32_t count, uint32_t block, void *context,
    game_ledger_read32_fn read32, game_ledger_write32_fn write32,
    uint32_t *eax, uint32_t *ecx, uint32_t *edx) __attribute__((weak));

static uint32_t game_ledger_read32(void *context, uint32_t address)
{
    (void)context;
    return guest_read32(address);
}

static void game_ledger_write32(void *context, uint32_t address, uint32_t value)
{
    (void)context;
    guest_write32(address, value);
}

static uint8_t game_ledger_read8(void *context, uint32_t address)
{
    (void)context;
    return guest_read8(address);
}

static void game_ledger_write8(void *context, uint32_t address, uint8_t value)
{
    (void)context;
    guest_write8(address, value);
}

static void game_ledger_history_report(void *context, uint32_t argument)
{
    (void)context;
    const uint32_t return_pc = guest_read32(g_esp);
    const uint32_t arguments[1] = {argument};
    if (game_guest_call(0x000389B0u, GAME_CC_cdecl, return_pc, 4u, 0u, 0u,
                        arguments, 1u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
}

static void game_ledger_history_callout(void *context, uint32_t target)
{
    (void)context;
    const uint32_t return_pc = guest_read32(g_esp);
    if (game_guest_call(target, GAME_CC_cdecl, return_pc, 0u, 0u, 0u, NULL, 0u) !=
        GAME_GUEST_CALL_OK)
        __builtin_trap();
}

static uint32_t game_ledger_read_argument0(void *context)
{
    (void)context;
    return game_stack_arg(0);
}

static int game_ledger_rust_selected(void)
{
#if defined(TSFP_RUST_GAME_LEDGER)
    static atomic_int selected = ATOMIC_VAR_INIT(-1);
    int mode = atomic_load_explicit(&selected, memory_order_acquire);
    if (mode >= 0) return mode;

    const char *value = getenv("TSFP_RUST_GAME_LEDGER");
    mode = value != NULL && (strcmp(value, "1") == 0 || strcmp(value, "rust") == 0);
    int expected = -1;
    atomic_compare_exchange_strong_explicit(&selected, &expected, mode,
                                            memory_order_release, memory_order_relaxed);
    return atomic_load_explicit(&selected, memory_order_acquire);
#else
    return 0;
#endif
}

int game_ledger_rust_allocate(uint32_t requested, uint32_t *result)
{
    if (!game_ledger_rust_selected() || tsfp_kmem_game_ledger_allocate == NULL || result == NULL)
        return 0;
    uint32_t block = 0;
    if (tsfp_kmem_game_ledger_allocate(requested, NULL, game_ledger_read32,
                                       game_ledger_write32, &block) != 0)
        return 0;
    *result = block;
    return 1;
}

int game_ledger_rust_ensure_buffer(uint32_t force)
{
    if (!game_ledger_rust_selected() ||
        tsfp_kmem_game_wrapper_ensure_ledger_buffer == NULL)
        return 0;
    if (tsfp_kmem_game_wrapper_ensure_ledger_buffer(
            force, NULL, game_ledger_read32, game_ledger_write32) != 0u)
        return -1;
    return 1;
}

int game_ledger_rust_history_reset(uint32_t mode)
{
    if (!game_ledger_rust_selected() || tsfp_kmem_game_ledger_history_reset == NULL)
        return 0;
    if (tsfp_kmem_game_ledger_history_reset(
            mode, NULL, game_ledger_read32, game_ledger_write32, game_ledger_read8,
            game_ledger_write8, game_ledger_history_report,
            game_ledger_history_callout) != 0u)
        return -1;
    return 1;
}

int game_ledger_rust_release(uint32_t count, uint32_t block,
                             uint32_t *eax, uint32_t *ecx, uint32_t *edx)
{
    if (!game_ledger_rust_selected() || tsfp_kmem_game_ledger_release == NULL ||
        eax == NULL || ecx == NULL || edx == NULL)
        return 0;
    uint32_t rust_eax = 0, rust_ecx = 0, rust_edx = 0;
    if (tsfp_kmem_game_ledger_release(count, block, NULL, game_ledger_read32,
                                      game_ledger_write32, &rust_eax, &rust_ecx,
                                      &rust_edx) != 0)
        return 0;
    *eax = rust_eax;
    *ecx = rust_ecx;
    *edx = rust_edx;
    return 1;
}

int game_ledger_rust_secondary_enabled(void)
{
    return game_ledger_rust_selected() &&
           tsfp_kmem_game_ledger_secondary_append != NULL &&
           tsfp_kmem_game_ledger_secondary_release != NULL;
}

int game_ledger_rust_secondary_append(uint32_t *eax, uint32_t *ecx, uint32_t *edx)
{
    if (!game_ledger_rust_secondary_enabled() || eax == NULL || ecx == NULL || edx == NULL)
        return 0;
    if (tsfp_kmem_game_ledger_secondary_append(
            NULL, game_ledger_read32, game_ledger_write32, game_ledger_read_argument0,
            eax, ecx, edx) != 0)
        return 0;
    return 1;
}

int game_ledger_rust_secondary_release(uint32_t count, uint32_t block,
                                       uint32_t *eax, uint32_t *ecx, uint32_t *edx)
{
    if (!game_ledger_rust_secondary_enabled() || eax == NULL || ecx == NULL || edx == NULL)
        return 0;
    if (tsfp_kmem_game_ledger_secondary_release(
            count, block, NULL, game_ledger_read32, game_ledger_write32,
            eax, ecx, edx) != 0)
        return 0;
    return 1;
}
