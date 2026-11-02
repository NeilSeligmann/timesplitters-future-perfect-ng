/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1576 batch A: hand C for guarded-jump roots (jump table or tail jmp in the closure), proved
 * with the opt-in guarded-jump-schema3 contract (`tools.replace prove --guarded-jump-contract
 * guarded-jump-schema3`). Every root keeps its real callees live through game_guest_call, so the
 * callee leftovers in EAX/ECX/EDX stay visible and each adapter is register-exact. Records:
 * docs/evidence/t1576/batch-a.md, receipts docs/data/t1576-batch-a/receipts/.
 * T1796 repairs four bodies below; historical receipts pin their earlier bytes.
 * Current fixed retail/native state controls: docs/t1796-guarded-state-repair.md. */
#include "game_replace.h"
#include "game_guest_tail.h"

#if defined(TSFP_RUST_GAME_ARENA) || defined(TSFP_RUST_GAME_POOL)
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#endif

#if defined(TSFP_RUST_GAME_POOL)
#include "game_pool_rust_bridge.h"
typedef uint32_t (*game_pool_read32_fn)(void *context, uint32_t address);
typedef void (*game_pool_write32_fn)(void *context, uint32_t address, uint32_t value);
typedef uint8_t (*game_pool_read8_fn)(void *context, uint32_t address);
typedef void (*game_pool_write8_fn)(void *context, uint32_t address, uint8_t value);
typedef void (*game_pool_report_fn)(void *context, uint32_t argument);
extern uint32_t tsfp_kmem_game_pool_allocate_and_reset(
    void *context, game_pool_read32_fn read32, game_pool_write32_fn write32,
    game_pool_read8_fn read8, game_pool_write8_fn write8, game_pool_report_fn report)
    __attribute__((weak));
#endif

extern __thread uint32_t g_ebx, g_edi, g_seh_ebp;
extern __thread int g_df;

static void call_guest(uint32_t target, game_convention convention, uint32_t return_pc,
                       uint32_t caller_bytes, uint32_t this_pointer, const uint32_t *arguments,
                       unsigned count)
{
    if (game_guest_call(target, convention, return_pc, caller_bytes, this_pointer, 0u, arguments,
                        count) != GAME_GUEST_CALL_OK)
        __builtin_trap();
}

#if defined(TSFP_RUST_GAME_POOL)
typedef struct game_pool_report_context {
    uint32_t call_index;
} game_pool_report_context;

static uint32_t game_pool_live_read32(void *context, uint32_t address)
{
    (void)context;
    return guest_read32(address);
}

static void game_pool_live_write32(void *context, uint32_t address, uint32_t value)
{
    (void)context;
    guest_write32(address, value);
}

static uint8_t game_pool_live_read8(void *context, uint32_t address)
{
    (void)context;
    return guest_read8(address);
}

static void game_pool_live_write8(void *context, uint32_t address, uint8_t value)
{
    (void)context;
    guest_write8(address, value);
}

static void game_pool_live_report(void *context, uint32_t argument)
{
    game_pool_report_context *report = context;
    const uint32_t return_pc = report->call_index++ == 0u ? 0x0003E041u : 0x0003E048u;
    const uint32_t caller_bytes = return_pc == 0x0003E041u ? 4u : 8u;
    call_guest(0x000389B0u, GAME_CC_cdecl, return_pc, caller_bytes, 0u, &argument, 1u);
}

typedef struct game_pool_compare {
    uint32_t size;
    uint32_t region_b_cursor;
    uint32_t main_remaining;
    uint32_t count;
    uint32_t addresses[32];
    uint32_t values[32];
    int mismatch;
} game_pool_compare;

static void game_pool_compare_begin(game_pool_compare *state)
{
    *state = (game_pool_compare){
        .size = guest_read32(0x004C81B8u),
        .region_b_cursor = guest_read32(0x006B837Cu),
        .main_remaining = guest_read32(0x006B8384u),
    };
}

static uint32_t game_pool_shadow_read32(void *context, uint32_t address)
{
    game_pool_compare *state = context;
    for (uint32_t i = 0; i < state->count; ++i)
        if (state->addresses[i] == address) return state->values[i];
    if (address == 0x004C81B8u) return state->size;
    if (address == 0x006B837Cu) return state->region_b_cursor;
    if (address == 0x006B8384u) return state->main_remaining;
    return guest_read32(address);
}

static void game_pool_shadow_write32(void *context, uint32_t address, uint32_t value)
{
    game_pool_compare *state = context;
    if (guest_read32(address) != value) state->mismatch = 1;
    for (uint32_t i = 0; i < state->count; ++i) {
        if (state->addresses[i] == address) {
            state->values[i] = value;
            return;
        }
    }
    if (state->count == 32u) {
        state->mismatch = 1;
        return;
    }
    state->addresses[state->count] = address;
    state->values[state->count++] = value;
}

static uint8_t game_pool_shadow_read8(void *context, uint32_t address)
{
    (void)context;
    return guest_read8(address);
}

static void game_pool_shadow_write8(void *context, uint32_t address, uint8_t value)
{
    game_pool_compare *state = context;
    if (guest_read8(address) != value) state->mismatch = 1;
}

static void game_pool_compare_report(void *context, uint32_t argument)
{
    (void)context;
    (void)argument;
}

static uint32_t game_pool_allocate_and_reset_c(void)
{
    const uint32_t arguments[3] = {64u, guest_read32(0x004C81B8u), 1u};
    call_guest(0x0003E0B0u, GAME_CC_cdecl, 0x0003EA0Fu, 12u, 0u, arguments, 3u);
    const uint32_t buffer = g_eax;
    guest_write32(0x006B83C8u, buffer);
    const uint32_t return_pc = guest_read32(g_esp);
    call_guest(0x0003E9A0u, GAME_CC_cdecl, return_pc, 0u, 0u, NULL, 0u);
    return buffer;
}

GAME_REPLACE_EXACT(0003EA00, cdecl, 0, void, game_pool_allocate_global_size_then_reset)
{
    const int mode = game_pool_rust_mode();
    const uint32_t saved_esi = g_esi;
    const uint32_t saved_edi = g_edi;
    if (mode == GAME_POOL_RUST && tsfp_kmem_game_pool_allocate_and_reset != NULL) {
        game_pool_report_context report = {0};
        if (tsfp_kmem_game_pool_allocate_and_reset(&report, game_pool_live_read32,
                                                   game_pool_live_write32, game_pool_live_read8,
                                                   game_pool_live_write8, game_pool_live_report) == 0u) {
            g_eax = guest_read32(0x006B83C8u);
            g_esi = saved_esi;
            g_edi = saved_edi;
            return;
        }
    }

    game_pool_compare compare;
    if (mode == GAME_POOL_COMPARE && tsfp_kmem_game_pool_allocate_and_reset != NULL)
        game_pool_compare_begin(&compare);
    const uint32_t c_buffer = game_pool_allocate_and_reset_c();
    if (mode == GAME_POOL_COMPARE && tsfp_kmem_game_pool_allocate_and_reset != NULL) {
        if (tsfp_kmem_game_pool_allocate_and_reset(
                &compare, game_pool_shadow_read32, game_pool_shadow_write32,
                game_pool_shadow_read8, game_pool_shadow_write8, game_pool_compare_report) != 0u)
            compare.mismatch = 1;
        if (game_pool_shadow_read32(&compare, 0x006B83C8u) != c_buffer)
            compare.mismatch = 1;
        if (compare.mismatch) {
            static atomic_uint_fast64_t mismatches = ATOMIC_VAR_INIT(0);
            if (atomic_fetch_add_explicit(&mismatches, 1u, memory_order_relaxed) == 0u)
                fprintf(stderr, "[rust-game-pool] 0x3EA00 C/Rust allocation/reset mismatch\n");
        }
    }
    g_eax = c_buffer;
    g_esi = saved_esi;
    g_edi = saved_edi;
}
#endif

/* 000379C0: cdecl(kind, flag). With flag == 0 a kind 0..9 picks (jump table at 37AA4, identity
 * map) one text id 0x1C25 0x1C26 0x1C28 0x1C27 0x1C29 0x1C2A 0x1C2B 0x1C2C 0x1C2D 0x1C2E and the
 * result is 3D7B0(id). flag == 1 with kind 10 gives 3D7B0(0x1C2F), flag == 2 with kind 11 gives
 * 3D7B0(0x1C8B). Every other request returns 0 through the single `xor eax, eax` at 37AA0, which
 * is also the JA default of the table, so all of them run through `default:` here. */
GAME_REPLACE_EXACT(000379C0, cdecl, 2, u32, game_ui_add_feedback_reason_text_items_bad_language_cheating)
{
    uint32_t slot = 0xFFFFFFFFu;
    uint32_t text = 0u;
    uint32_t return_pc = 0u;
    int direct = 0;
    g_eax = game_stack_arg(1u);
    if (g_eax == 0u) {
        g_eax = game_stack_arg(0u);
        if (g_eax <= 9u) {
            slot = g_eax;
        }
    } else if (g_eax == 1u && game_stack_arg(0u) == 0xAu) {
        text = 0x1C2Fu;
        return_pc = 0x37A82u;
        direct = 1;
    } else if (g_eax == 2u && game_stack_arg(0u) == 0xBu) {
        text = 0x1C8Bu;
        return_pc = 0x37A9Cu;
        direct = 1;
    }
    if (!direct) {
        switch (slot) {
        case 0:
            text = 0x1C25u;
            return_pc = 0x379EAu;
            break;
        case 1:
            text = 0x1C26u;
            return_pc = 0x379F8u;
            break;
        case 2:
            text = 0x1C28u;
            return_pc = 0x37A06u;
            break;
        case 3:
            text = 0x1C27u;
            return_pc = 0x37A14u;
            break;
        case 4:
            text = 0x1C29u;
            return_pc = 0x37A22u;
            break;
        case 5:
            text = 0x1C2Au;
            return_pc = 0x37A30u;
            break;
        case 6:
            text = 0x1C2Bu;
            return_pc = 0x37A3Eu;
            break;
        case 7:
            text = 0x1C2Cu;
            return_pc = 0x37A4Cu;
            break;
        case 8:
            text = 0x1C2Du;
            return_pc = 0x37A5Au;
            break;
        case 9:
            text = 0x1C2Eu;
            return_pc = 0x37A68u;
            break;
        default:
            g_eax = 0u;
            return;
        }
    }
    const uint32_t arguments[1] = {text};
    call_guest(0x0003D7B0u, GAME_CC_cdecl, return_pc, 0u, 0u, arguments, 1u);
}

/* 0003DFA0: cdecl(size, kind) arena allocator (docs/t1364-arena-allocator-naming.md). The size is
 * rounded to S = (size + 15) & ~15 (32-bit wrapping). `cmp kind, 4 / ja / jmp [kind*4 + 3E094]`:
 *   kind 0  descending shared arena: top [6B8388], remaining [6B8384]. Signed S + 16 > remaining
 *           calls the hook 389B0 twice (-1 then 1, it is a bare RET), then lowers the top,
 *           writes the "LOCALMEM" header (+0 tag, +4 tag, +8 S, +C header address) and returns
 *           header + 16 with remaining -= S + 16.
 *   kind 1  ascending shared cursor [6B837C] (signed S > remaining calls the hook twice).
 *   kind 2  ascending separate cursor [6B8370] against budget [6B8378], no capacity check.
 *   kind 4  ascending cursor [6B8390], no budget.
 *   kind 3 and every unsigned kind above 4 share the arm at 3E08D and return 0.
 * ESI is saved and restored by the original (push esi / pop esi), ECX and EDX leave as the
 * arm's last values. */
#if defined(TSFP_RUST_GAME_ARENA)
typedef uint32_t (*game_arena_read32_fn)(void *context, uint32_t address);
typedef void (*game_arena_write32_fn)(void *context, uint32_t address, uint32_t value);
typedef void (*game_arena_report_fn)(void *context, uint32_t argument);
extern uint32_t tsfp_kmem_game_arena_allocate(uint32_t kind, uint32_t size, void *context,
                                              game_arena_read32_fn read32,
                                              game_arena_write32_fn write32,
                                              game_arena_report_fn report, uint32_t *out)
    __attribute__((weak));
extern uint32_t tsfp_kmem_game_arena_allocate_aligned(uint32_t alignment, uint32_t size,
                                                      uint32_t kind, void *context,
                                                      game_arena_read32_fn read32,
                                                      game_arena_write32_fn write32,
                                                      game_arena_report_fn report, uint32_t *out)
    __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_aligned_75b4b4(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_model_buffer_78b718(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_allocate_slot_745938(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    uint32_t *out_slot) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_buffers_54ef14(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_save_slot_778d38(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_mapedit_buffers(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_weather_74c3b0(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report, uint32_t *out_eax) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_tables_74c3b8(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report, uint32_t *out_eax) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_weather_tables_7ac824(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_effect_slots_7a156c(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report, uint32_t *out_block) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_effect_mesh_buffer_78aef0(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_story_event_instances_78cd90(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report, uint32_t size, uint32_t *out_block) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_story_event_support_arrays(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report, uint32_t index_size, uint32_t snapshot_size,
    uint32_t *out_indices, uint32_t *out_snapshot) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_story_objective_counters(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_table_78b240(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_table_75c10c(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_state_block_75c108(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_sound_state_75c124(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_timer_state_75c118(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_table_75c0f0(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_table_78b0d0(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_tables_78b08c(
    uint32_t flag, void *context, game_arena_read32_fn read32,
    game_arena_write32_fn write32, game_arena_report_fn report,
    uint32_t *out_second) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_pick_node_762f7c(
    uint32_t source, void *context, game_arena_read32_fn read32,
    game_arena_write32_fn write32, game_arena_report_fn report,
    uint32_t *out_block) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_alloc_node_pool_772b50(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report, uint32_t *out_nodes) __attribute__((weak));
typedef struct game_arena_release_registers {
    uint32_t eax;
    uint32_t ecx;
    uint32_t edx;
} game_arena_release_registers;
extern uint32_t tsfp_kmem_game_arena_release_tagged(uint32_t block, void *context,
                                                    game_arena_read32_fn read32,
                                                    game_arena_write32_fn write32,
                                                    game_arena_report_fn report,
                                                    game_arena_release_registers *out)
    __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_release_mapedit_static_block_762f90(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report, game_arena_release_registers *out) __attribute__((weak));
extern uint32_t tsfp_kmem_game_wrapper_release_record_rows_panel_block_7630b0(
    void *context, game_arena_read32_fn read32, game_arena_write32_fn write32,
    game_arena_report_fn report, game_arena_release_registers *out) __attribute__((weak));
extern uint32_t tsfp_kmem_game_arena_rewind_top(void *context,
                                                game_arena_read32_fn read32,
                                                game_arena_write32_fn write32)
    __attribute__((weak));

typedef struct game_arena_shadow {
    uint32_t region_a_cursor;
    uint32_t region_a_remaining;
    uint32_t region_b_cursor;
    uint32_t main_remaining;
    uint32_t top_cursor;
    uint32_t kind4_cursor;
    uint32_t write_count;
    uint32_t write_addresses[4];
    uint32_t write_values[4];
    int write_overflow;
} game_arena_shadow;

enum { GAME_ARENA_C = 0, GAME_ARENA_RUST = 1, GAME_ARENA_COMPARE = 2 };
static atomic_int g_rust_game_arena_mode = ATOMIC_VAR_INIT(-1);
static atomic_uint_fast64_t g_rust_game_arena_mismatches = ATOMIC_VAR_INIT(0);

static int rust_game_arena_mode(void)
{
    int mode = atomic_load_explicit(&g_rust_game_arena_mode, memory_order_acquire);
    if (mode >= 0) return mode;

    const char *value = getenv("TSFP_RUST_GAME_ARENA");
    int selected = GAME_ARENA_C;
    if (value != NULL && strcmp(value, "1") == 0) selected = GAME_ARENA_RUST;
    else if (value != NULL && strcmp(value, "compare") == 0) selected = GAME_ARENA_COMPARE;

    int expected = -1;
    atomic_compare_exchange_strong_explicit(&g_rust_game_arena_mode, &expected, selected,
                                            memory_order_release, memory_order_relaxed);
    return atomic_load_explicit(&g_rust_game_arena_mode, memory_order_acquire);
}

static game_arena_shadow game_arena_shadow_from_guest(void)
{
    game_arena_shadow shadow = {
        .region_a_cursor = guest_read32(0x006B8370u),
        .region_a_remaining = guest_read32(0x006B8378u),
        .region_b_cursor = guest_read32(0x006B837Cu),
        .main_remaining = guest_read32(0x006B8384u),
        .top_cursor = guest_read32(0x006B8388u),
        .kind4_cursor = guest_read32(0x006B8390u),
    };
    return shadow;
}

static uint32_t game_arena_shadow_read32(void *context, uint32_t address)
{
    game_arena_shadow *shadow = context;
    switch (address) {
    case 0x006B8370u: return shadow->region_a_cursor;
    case 0x006B8378u: return shadow->region_a_remaining;
    case 0x006B837Cu: return shadow->region_b_cursor;
    case 0x006B8384u: return shadow->main_remaining;
    case 0x006B8388u: return shadow->top_cursor;
    case 0x006B8390u: return shadow->kind4_cursor;
    default:
        for (uint32_t i = 0; i < shadow->write_count && i < 4u; ++i)
            if (shadow->write_addresses[i] == address) return shadow->write_values[i];
        return guest_read32(address);
    }
}

static uint32_t game_arena_live_read32(void *context, uint32_t address)
{
    (void)context;
    return guest_read32(address);
}

static void game_arena_live_write32(void *context, uint32_t address, uint32_t value)
{
    (void)context;
    guest_write32(address, value);
}

typedef struct game_arena_report_context {
    uint32_t kind;
    uint32_t call_index;
} game_arena_report_context;

static void game_arena_live_report(void *context, uint32_t argument)
{
    game_arena_report_context *report = context;
    uint32_t return_pc;
    uint32_t caller_bytes;
    if (report->kind == 0u && report->call_index < 2u) {
        return_pc = report->call_index == 0u ? 0x0003DFD3u : 0x0003DFDAu;
        caller_bytes = report->call_index == 0u ? 4u : 8u;
    } else if (report->kind == 1u && report->call_index < 2u) {
        return_pc = report->call_index == 0u ? 0x0003E041u : 0x0003E048u;
        caller_bytes = report->call_index == 0u ? 4u : 8u;
    } else if (report->kind == 2u && report->call_index < 2u) {
        return_pc = report->call_index == 0u ? 0x002FE20Du : 0x0033660Du;
        caller_bytes = 8u;
    } else {
        return;
    }
    ++report->call_index;
    call_guest(0x000389B0u, GAME_CC_cdecl, return_pc, caller_bytes, 0u, &argument, 1u);
}

static void game_arena_shadow_write32(void *context, uint32_t address, uint32_t value)
{
    game_arena_shadow *shadow = context;
    switch (address) {
    case 0x006B8370u: shadow->region_a_cursor = value; return;
    case 0x006B8378u: shadow->region_a_remaining = value; return;
    case 0x006B837Cu: shadow->region_b_cursor = value; return;
    case 0x006B8384u: shadow->main_remaining = value; return;
    case 0x006B8388u: shadow->top_cursor = value; return;
    case 0x006B8390u: shadow->kind4_cursor = value; return;
    default: break;
    }
    if (shadow->write_count < 4u) {
        shadow->write_addresses[shadow->write_count] = address;
        shadow->write_values[shadow->write_count] = value;
    } else {
        shadow->write_overflow = 1;
    }
    ++shadow->write_count;
}

typedef struct game_mapedit_buffer_shadow {
    game_arena_shadow arena;
    uint32_t first_buffer;
    uint32_t undo_buffer;
    uint32_t first_clear_count;
    uint32_t undo_clear_count;
    uint32_t head_count;
    uint32_t stage;
    int mismatch;
} game_mapedit_buffer_shadow;

static uint32_t game_mapedit_buffer_shadow_read32(void *context, uint32_t address)
{
    game_mapedit_buffer_shadow *shadow = context;
    switch (address) {
    case 0x00784014u: return shadow->first_buffer;
    case 0x00765C24u: return shadow->undo_buffer;
    case 0x00765C28u:
    case 0x00765C30u:
    case 0x00765C20u: return 0u;
    default: break;
    }
    return game_arena_shadow_read32(&shadow->arena, address);
}

static void game_mapedit_buffer_shadow_write32(void *context, uint32_t address, uint32_t value)
{
    game_mapedit_buffer_shadow *shadow = context;
    switch (address) {
    case 0x006B8370u:
    case 0x006B8378u:
    case 0x006B8384u:
    case 0x006B8388u:
    case 0x006B8390u:
        game_arena_shadow_write32(&shadow->arena, address, value);
        return;
    case 0x00784014u:
        shadow->first_buffer = value;
        shadow->first_clear_count = 0u;
        shadow->stage = 1u;
        return;
    case 0x00765C24u:
        shadow->undo_buffer = value;
        shadow->undo_clear_count = 0u;
        shadow->stage = 3u;
        return;
    case 0x00765C28u:
    case 0x00765C30u:
    case 0x00765C20u: {
        const uint32_t expected = shadow->head_count == 0u ? 0x00765C28u :
                                  shadow->head_count == 1u ? 0x00765C30u : 0x00765C20u;
        shadow->mismatch |= shadow->stage != 4u || address != expected || value != 0u;
        ++shadow->head_count;
        return;
    }
    default:
        break;
    }
    if (shadow->stage == 1u) {
        const uint32_t expected = shadow->first_buffer + shadow->first_clear_count * 4u;
        shadow->mismatch |= shadow->first_clear_count >= 0x2000u || address != expected || value != 0u;
        if (++shadow->first_clear_count == 0x2000u) shadow->stage = 2u;
        return;
    }
    if (shadow->stage == 3u) {
        const uint32_t expected = shadow->undo_buffer + shadow->undo_clear_count * 4u;
        shadow->mismatch |= shadow->undo_clear_count >= 0x4000u || address != expected || value != 0u;
        if (++shadow->undo_clear_count == 0x4000u) shadow->stage = 4u;
        return;
    }
    shadow->mismatch = 1;
}

typedef struct game_save_slot_shadow {
    game_arena_shadow arena;
    uint32_t payload_start;
    uint32_t clear_write_count;
    int clear_mismatch;
} game_save_slot_shadow;

static uint32_t game_save_slot_shadow_read32(void *context, uint32_t address)
{
    game_save_slot_shadow *shadow = context;
    return game_arena_shadow_read32(&shadow->arena, address);
}

static void game_save_slot_shadow_write32(void *context, uint32_t address, uint32_t value)
{
    game_save_slot_shadow *shadow = context;
    if (address == 0x006B837Cu || address == 0x006B8384u || address == 0x00778D38u) {
        game_arena_shadow_write32(&shadow->arena, address, value);
        return;
    }

    uint32_t expected = 0u;
    if (shadow->clear_write_count < 0x54Cu) {
        expected = shadow->payload_start + shadow->clear_write_count * 4u;
    } else if (shadow->clear_write_count == 0x54Cu) {
        expected = shadow->payload_start + 4u;
    } else if (shadow->clear_write_count == 0x54Du) {
        expected = shadow->payload_start + 8u;
    } else {
        shadow->clear_mismatch = 1;
    }
    shadow->clear_mismatch |= address != expected || value != 0u;
    ++shadow->clear_write_count;
}

static int game_arena_same_state(const game_arena_shadow *left, const game_arena_shadow *right)
{
    return left->region_a_cursor == right->region_a_cursor &&
           left->region_a_remaining == right->region_a_remaining &&
           left->region_b_cursor == right->region_b_cursor &&
           left->main_remaining == right->main_remaining && left->top_cursor == right->top_cursor &&
           left->kind4_cursor == right->kind4_cursor;
}

static void game_arena_set_rust_registers(uint32_t kind, uint32_t size,
                                          const game_arena_shadow *state, uint32_t block)
{
    const uint32_t rounded = (size + 15u) & 0xFFFFFFF0u;
    switch (kind) {
    case 0:
        g_eax = block;
        g_ecx = 0xFFFFFFF0u - rounded;
        g_edx = state->main_remaining;
        break;
    case 1:
        g_eax = state->region_b_cursor - rounded;
        g_ecx = state->region_b_cursor;
        break;
    case 2:
        g_eax = state->region_a_cursor - rounded;
        g_ecx = state->region_a_cursor;
        break;
    case 4:
        g_eax = state->kind4_cursor - rounded;
        g_ecx = state->kind4_cursor;
        break;
    default:
        g_eax = 0u;
        break;
    }
}

static int rust_game_arena_allocate(uint32_t kind, uint32_t size, void *context,
                                    game_arena_read32_fn read32,
                                    game_arena_write32_fn write32,
                                    game_arena_report_fn report, uint32_t *block)
{
    return tsfp_kmem_game_arena_allocate != NULL &&
           tsfp_kmem_game_arena_allocate(kind, size, context, read32, write32, report, block) == 0u;
}

static int rust_game_arena_allocate_aligned(uint32_t alignment, uint32_t size, uint32_t kind,
                                            void *context, game_arena_read32_fn read32,
                                            game_arena_write32_fn write32,
                                            game_arena_report_fn report, uint32_t *block)
{
    return tsfp_kmem_game_arena_allocate_aligned != NULL &&
           tsfp_kmem_game_arena_allocate_aligned(alignment, size, kind, context, read32, write32,
                                                 report, block) == 0u;
}
#endif

GAME_REPLACE_EXACT(0003DFA0, cdecl, 2, u32, game_arena_alloc_round16_by_kind)
{
#if defined(TSFP_RUST_GAME_ARENA)
    const uint32_t rust_kind = game_stack_arg(1u);
    const uint32_t rust_size = game_stack_arg(0u);
    const int rust_mode = rust_game_arena_mode();
    game_arena_shadow rust_state;
    if (rust_mode != GAME_ARENA_C) rust_state = game_arena_shadow_from_guest();
    if (rust_mode == GAME_ARENA_RUST) {
        uint32_t rust_block;
        game_arena_report_context report_context = {.kind = rust_kind, .call_index = 0u};
        guest_write32(g_esp - 4u, g_esi);
        g_esi = (rust_size + 15u) & 0xFFFFFFF0u;
        if (rust_game_arena_allocate(rust_kind, rust_size, &report_context,
                                     game_arena_live_read32, game_arena_live_write32,
                                     game_arena_live_report, &rust_block)) {
            const game_arena_shadow rust_after = game_arena_shadow_from_guest();
            g_esi = guest_read32(g_esp - 4u);
            game_arena_set_rust_registers(rust_kind, rust_size, &rust_after, rust_block);
            return;
        }
        g_esi = guest_read32(g_esp - 4u);
    }
#endif
    const uint32_t kind = game_stack_arg(1u);
    g_eax = kind;
    guest_write32(g_esp - 4u, g_esi);
    g_esi = (game_stack_arg(0u) + 15u) & 0xFFFFFFF0u;
    const uint32_t hook_minus_one[1] = {0xFFFFFFFFu};
    const uint32_t hook_one[1] = {1u};
    switch (kind) {
    case 0:
        g_ecx = guest_read32(0x006B8384u);
        g_eax = g_esi + 0x10u;
        if ((int32_t)g_eax > (int32_t)g_ecx) {
            call_guest(0x000389B0u, GAME_CC_cdecl, 0x0003DFD3u, 4u, 0u, hook_minus_one, 1u);
            call_guest(0x000389B0u, GAME_CC_cdecl, 0x0003DFDAu, 8u, 0u, hook_one, 1u);
        }
        g_eax = guest_read32(0x006B8388u);
        g_ecx = 0xFFFFFFF0u - g_esi;
        g_eax += g_ecx;
        guest_write32(0x006B8388u, g_eax);
        guest_write32(g_eax, 0x41434F4Cu);
        g_edx = guest_read32(0x006B8388u);
        guest_write32(g_edx + 4u, 0x4D454D4Cu);
        g_eax = guest_read32(0x006B8388u);
        guest_write32(g_eax + 8u, g_esi);
        g_eax = guest_read32(0x006B8388u);
        guest_write32(g_eax + 0xCu, g_eax);
        g_eax = guest_read32(0x006B8388u);
        g_edx = guest_read32(0x006B8384u);
        g_ecx = 0xFFFFFFF0u - g_esi;
        g_eax += 0x10u;
        g_edx += g_ecx;
        guest_write32(0x006B8384u, g_edx);
        break;
    case 1:
        if ((int32_t)g_esi > (int32_t)guest_read32(0x006B8384u)) {
            call_guest(0x000389B0u, GAME_CC_cdecl, 0x0003E041u, 4u, 0u, hook_minus_one, 1u);
            call_guest(0x000389B0u, GAME_CC_cdecl, 0x0003E048u, 8u, 0u, hook_one, 1u);
        }
        g_ecx = guest_read32(0x006B837Cu);
        g_eax = g_ecx;
        g_ecx += g_esi;
        guest_write32(0x006B837Cu, g_ecx);
        guest_write32(0x006B8384u, guest_read32(0x006B8384u) - g_esi);
        break;
    case 2:
        g_ecx = guest_read32(0x006B8370u);
        g_eax = g_ecx;
        g_ecx += g_esi;
        guest_write32(0x006B8370u, g_ecx);
        guest_write32(0x006B8378u, guest_read32(0x006B8378u) - g_esi);
        break;
    case 4:
        g_ecx = guest_read32(0x006B8390u);
        g_eax = g_ecx;
        g_ecx += g_esi;
        guest_write32(0x006B8390u, g_ecx);
        break;
    case 3:
    default:
        g_eax = 0u;
        break;
    }
    g_esi = guest_read32(g_esp - 4u);
#if defined(TSFP_RUST_GAME_ARENA)
    if (rust_mode == GAME_ARENA_COMPARE) {
        const uint32_t c_block = g_eax;
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        uint32_t rust_block = 0u;
        int mismatch = !rust_game_arena_allocate(rust_kind, rust_size, &rust_state,
                                                 game_arena_shadow_read32,
                                                 game_arena_shadow_write32, NULL, &rust_block);
        mismatch |= rust_block != c_block || rust_state.write_overflow != 0;
        mismatch |= rust_state.write_count != (rust_kind == 0u ? 4u : 0u);
        const game_arena_shadow rust_globals = rust_state;
        mismatch |= !game_arena_same_state(&rust_globals, &c_state);
        for (uint32_t i = 0; i < rust_state.write_count && i < 4u; ++i)
            mismatch |= guest_read32(rust_state.write_addresses[i]) != rust_state.write_values[i];
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u) {
                fprintf(stderr,
                        "game-arena: first mismatch kind=%08x size=%08x C block/state=(%08x,%08x,%08x,%08x,%08x,%08x,%08x) Rust=(%08x,%08x,%08x,%08x,%08x,%08x,%08x) writes=%u overflow=%d\n",
                        rust_kind, rust_size, c_block, c_state.region_a_cursor,
                        c_state.region_a_remaining, c_state.region_b_cursor, c_state.main_remaining,
                        c_state.top_cursor, c_state.kind4_cursor, rust_block,
                        rust_state.region_a_cursor, rust_state.region_a_remaining,
                        rust_state.region_b_cursor, rust_state.main_remaining,
                        rust_state.top_cursor, rust_state.kind4_cursor, rust_state.write_count,
                        rust_state.write_overflow);
            }
        }
    }
#endif
}

#if defined(TSFP_RUST_GAME_ARENA)
GAME_REPLACE_EXACT(0003E240, cdecl, 0, void, game_arena_rewind_top)
{
    const int rust_mode = rust_game_arena_mode();
    game_arena_shadow rust_state;
    if (rust_mode != GAME_ARENA_C) rust_state = game_arena_shadow_from_guest();
    if (rust_mode == GAME_ARENA_RUST && tsfp_kmem_game_arena_rewind_top != NULL &&
        tsfp_kmem_game_arena_rewind_top(NULL, game_arena_live_read32,
                                        game_arena_live_write32) == 0u)
        return;

    const uint32_t limit = guest_read32(0x006B8380u);
    const uint32_t top = guest_read32(0x006B8388u);
    const uint32_t remaining = guest_read32(0x006B8384u);
    guest_write32(0x006B8384u, remaining + (limit - top));
    guest_write32(0x006B8388u, limit);

    if (rust_mode == GAME_ARENA_COMPARE && tsfp_kmem_game_arena_rewind_top != NULL) {
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        const uint32_t status = tsfp_kmem_game_arena_rewind_top(
            &rust_state, game_arena_shadow_read32, game_arena_shadow_write32);
        if (status != 0u || rust_state.write_overflow || rust_state.write_count != 0u ||
            !game_arena_same_state(&rust_state, &c_state)) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u)
                fprintf(stderr, "game-arena-rewind: first mismatch limit=%08x top=%08x remaining=%08x\n",
                        limit, top, remaining);
        }
    }
}

GAME_REPLACE_EXACT(0003E1E0, cdecl, 1, u32, game_arena_release_tagged_block)
{
    const uint32_t block = game_stack_arg(0u);
    const int rust_mode = rust_game_arena_mode();
    game_arena_shadow rust_state;
    if (rust_mode != GAME_ARENA_C) rust_state = game_arena_shadow_from_guest();
    if (rust_mode == GAME_ARENA_RUST) {
        game_arena_release_registers registers;
        if (tsfp_kmem_game_arena_release_tagged != NULL &&
            tsfp_kmem_game_arena_release_tagged(block, NULL, game_arena_live_read32,
                                                game_arena_live_write32, NULL, &registers) == 0u) {
            g_eax = registers.eax;
            g_ecx = registers.ecx;
            g_edx = registers.edx;
            return;
        }
    }

    if (guest_read32(block - 16u) != 0x41434F4Cu || guest_read32(block - 12u) != 0x4D454D4Cu) {
        /* The measured 000389B0 diagnostic is state-free; this adapter omits its call event. */
    }
    if (guest_read32(block - 4u) != guest_read32(0x006B8388u)) {
        /* The independent non-LIFO diagnostic is likewise state-free; see Tier 2 trace debt. */
    }
    const uint32_t first_size = guest_read32(block - 8u);
    const uint32_t top = guest_read32(0x006B8388u);
    const uint32_t remaining = guest_read32(0x006B8384u);
    guest_write32(0x006B8388u, top + first_size + 16u);
    const uint32_t second_size = guest_read32(block - 8u);
    const uint32_t new_remaining = remaining + second_size + 16u;
    guest_write32(0x006B8384u, new_remaining);
    g_eax = new_remaining;
    g_ecx = second_size;
    g_edx = remaining;

    if (rust_mode == GAME_ARENA_COMPARE && tsfp_kmem_game_arena_release_tagged != NULL) {
        const uint32_t c_eax = g_eax;
        const uint32_t c_ecx = g_ecx;
        const uint32_t c_edx = g_edx;
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        game_arena_release_registers rust_registers = {0};
        const uint32_t status = tsfp_kmem_game_arena_release_tagged(
            block, &rust_state, game_arena_shadow_read32, game_arena_shadow_write32,
            NULL, &rust_registers);
        const int mismatch = status != 0u || rust_registers.eax != c_eax ||
                             rust_registers.ecx != c_ecx || rust_registers.edx != c_edx ||
                             !game_arena_same_state(&rust_state, &c_state);
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u) {
                fprintf(stderr,
                        "game-arena-release: first mismatch block=%08x C=(%08x,%08x,%08x) Rust=(%08x,%08x,%08x) state-top/rem=(%08x,%08x)/(%08x,%08x)\n",
                        block, c_eax, c_ecx, c_edx, rust_registers.eax, rust_registers.ecx,
                        rust_registers.edx, c_state.top_cursor, c_state.main_remaining,
                        rust_state.top_cursor, rust_state.main_remaining);
            }
        }
    }
}
#endif

/* 0003E0B0: cdecl(alignment, size, kind) alignment wrapper around 3DFA0 (docs/t1364-arena-
 * allocator-naming.md). `cmp kind, 4 / ja / jmp [kind*4 + 3E1C4]`, mask = ~(alignment - 1) and
 * up(x) = (x + alignment - 1) & mask (32-bit wrapping, no power-of-two check):
 *   kind 1  pad the cursor [6B837C] to up(cursor), debit [6B8384] by the pad, then 3DFA0(size, 1).
 *   kind 2  the same with cursor [6B8370] and budget [6B8378].
 *   kind 0  top [6B8388] and remaining [6B8384] move by D = (up(top) - top) - alignment, then
 *           3DFA0(up(size), 0), then the header size [[6B8388] + 8] grows by alignment - pad.
 *   kind 4  cursor [6B8390] = up(cursor), then falls through to the delegation below.
 *   kind 3 and every unsigned kind above 4 delegate 3DFA0(size, kind) unchanged.
 * ESI and EDI are saved and restored by the original, EAX is the callee's pointer. */
GAME_REPLACE_EXACT(0003E0B0, cdecl, 3, u32, game_arena_alloc_with_alignment_by_kind)
{
#if defined(TSFP_RUST_GAME_ARENA)
    const uint32_t rust_alignment = game_stack_arg(0u);
    const uint32_t rust_size = game_stack_arg(1u);
    const uint32_t rust_kind = game_stack_arg(2u);
    const int rust_mode = rust_game_arena_mode();
    game_arena_shadow rust_state;
    if (rust_mode != GAME_ARENA_C) rust_state = game_arena_shadow_from_guest();
    if (rust_mode == GAME_ARENA_RUST) {
        uint32_t rust_block;
        game_arena_report_context report_context = {.kind = rust_kind, .call_index = 0u};
        guest_write32(g_esp - 4u, g_esi);
        if (rust_game_arena_allocate_aligned(rust_alignment, rust_size, rust_kind, &report_context,
                                             game_arena_live_read32, game_arena_live_write32,
                                             game_arena_live_report, &rust_block)) {
            g_eax = rust_block;
            g_esi = guest_read32(g_esp - 4u);
            return;
        }
        g_esi = guest_read32(g_esp - 4u);
    }
#endif

    guest_write32(g_esp - 4u, g_esi);
    g_esi = game_stack_arg(2u);
    const uint32_t kind = g_esi;
    uint32_t arguments[2];
    switch (kind) {
    case 0: {
        g_esi = game_stack_arg(0u);
        const uint32_t alignment = g_esi;
        g_ecx = guest_read32(0x006B8388u);
        guest_write32(g_esp - 8u, g_edi);
        g_edi = g_ecx + alignment - 1u;
        g_eax = ~(alignment - 1u);
        g_edi &= g_eax;
        g_edi -= g_ecx;
        g_edx = g_edi - alignment;
        g_ecx += g_edx;
        g_edx = guest_read32(0x006B8384u);
        guest_write32(0x006B8388u, g_ecx);
        g_ecx = g_edi - alignment;
        g_edx += g_ecx;
        guest_write32(0x006B8384u, g_edx);
        g_edx = game_stack_arg(1u);
        g_ecx = alignment + g_edx - 1u;
        g_ecx &= g_eax;
        arguments[0] = g_ecx;
        arguments[1] = 0u;
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0003E183u, 8u, 0u, arguments, 2u);
        g_ecx = guest_read32(0x006B8388u);
        g_edx = guest_read32(g_ecx + 8u);
        g_esi -= g_edi;
        g_edx += g_esi;
        g_edi = guest_read32(g_esp - 8u);
        guest_write32(g_ecx + 8u, g_edx);
        break;
    }
    case 1:
        g_ecx = guest_read32(0x006B837Cu);
        g_edx = game_stack_arg(0u);
        g_eax = g_ecx + g_edx - 1u;
        g_edx = ~(g_edx - 1u);
        g_eax &= g_edx;
        g_edx = game_stack_arg(1u);
        g_eax -= g_ecx;
        g_ecx += g_eax;
        guest_write32(0x006B837Cu, g_ecx);
        g_ecx = guest_read32(0x006B8384u);
        g_ecx -= g_eax;
        guest_write32(g_esp - 8u, g_esi);
        guest_write32(g_esp - 12u, g_edx);
        guest_write32(0x006B8384u, g_ecx);
        arguments[0] = g_edx;
        arguments[1] = kind;
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0003E0FBu, 12u, 0u, NULL, 0u);
        break;
    case 2:
        g_ecx = guest_read32(0x006B8370u);
        g_edx = game_stack_arg(0u);
        g_eax = g_ecx + g_edx - 1u;
        g_edx = ~(g_edx - 1u);
        g_eax &= g_edx;
        g_edx = game_stack_arg(1u);
        g_eax -= g_ecx;
        g_ecx += g_eax;
        guest_write32(0x006B8370u, g_ecx);
        g_ecx = guest_read32(0x006B8378u);
        g_ecx -= g_eax;
        guest_write32(g_esp - 8u, g_esi);
        guest_write32(g_esp - 12u, g_edx);
        guest_write32(0x006B8378u, g_ecx);
        arguments[0] = g_edx;
        arguments[1] = kind;
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0003E136u, 12u, 0u, NULL, 0u);
        break;
    case 4:
        g_eax = game_stack_arg(0u);
        g_edx = guest_read32(0x006B8390u);
        g_ecx = g_edx + g_eax - 1u;
        g_eax = ~(g_eax - 1u);
        g_ecx &= g_eax;
        guest_write32(0x006B8390u, g_ecx);
        __attribute__((fallthrough));
    case 3:
    default:
        g_edx = game_stack_arg(1u);
        arguments[0] = g_edx;
        arguments[1] = kind;
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0003E1BDu, 4u, 0u, arguments, 2u);
        break;
    }
    g_esi = guest_read32(g_esp - 4u);
#if defined(TSFP_RUST_GAME_ARENA)
    if (rust_mode == GAME_ARENA_COMPARE && tsfp_kmem_game_arena_allocate_aligned != NULL) {
        const uint32_t c_block = g_eax;
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        uint32_t rust_block = 0u;
        int mismatch = !rust_game_arena_allocate_aligned(
            rust_alignment, rust_size, rust_kind, &rust_state, game_arena_shadow_read32,
            game_arena_shadow_write32, NULL, &rust_block);
        mismatch |= rust_block != c_block || rust_state.write_overflow != 0;
        mismatch |= rust_state.write_count != (rust_kind == 0u ? 4u : 0u);
        mismatch |= !game_arena_same_state(&rust_state, &c_state);
        for (uint32_t i = 0; i < rust_state.write_count && i < 4u; ++i)
            mismatch |= guest_read32(rust_state.write_addresses[i]) != rust_state.write_values[i];
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u) {
                fprintf(stderr,
                        "game-arena-aligned: first mismatch align=%08x kind=%08x size=%08x C block=(%08x) Rust=(%08x) write-count=%u overflow=%d\n",
                        rust_alignment, rust_kind, rust_size, c_block, rust_block,
                        rust_state.write_count, rust_state.write_overflow);
            }
        }
    }
#endif
}

#if defined(TSFP_RUST_GAME_ARENA)
GAME_REPLACE_EXACT(0012C800, cdecl, 0, u32, game_alloc_slot_745938_max17)
{
    const int rust_mode = rust_game_arena_mode();
    if (rust_mode == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_allocate_slot_745938 != NULL) {
        uint32_t slot = 0u;
        if (tsfp_kmem_game_wrapper_allocate_slot_745938(
                NULL, game_arena_live_read32, game_arena_live_write32, &slot) != 0u)
            __builtin_trap();
        g_eax = slot;
        return;
    }

    const uint32_t count = guest_read32(0x007490A0u);
    if (count >= 0x11u) {
        g_eax = 0u;
        return;
    }
    guest_write32(0x007490A0u, count + 1u);
    g_eax = 0x00745938u + count * 0x144u;
}

GAME_REPLACE_EXACT(001620C0, cdecl, 0, void, game_alloc_story_objective_counters)
{
    const int rust_mode = rust_game_arena_mode();
    if (rust_mode == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_story_objective_counters != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_story_objective_counters(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        return;
    }

    const uint32_t mode = guest_read32(0x0074C36Cu);
    const uint32_t count = guest_read32(mode + 0x78u);
    if (count == 0u) return;

    const uint32_t definitions = guest_read32(mode + 0x74u);
    const uint32_t arguments[3] = {0x10u, count * 0x18u, 1u};
    /* T1703 does not retain this caller instruction boundary; this is adapter-only trace debt. */
    call_guest(0x0003E0B0u, GAME_CC_cdecl, 0x001620D0u, 0u, 0u, arguments, 3u);
    const uint32_t counters = g_eax;
    guest_write32(0x007A2820u, counters);
    for (uint32_t i = 0; i < count * 6u; ++i)
        guest_write32(counters + i * 4u, 0u);
    for (uint32_t i = 0; i < count; ++i) {
        const uint32_t definition = definitions + i * 0x98u;
        if ((guest_read32(definition + 0x54u) & 0x8000u) != 0u)
            guest_write32(counters + i * 0x18u + 0x0Cu,
                          guest_read32(definition + 0x64u));
    }
}

GAME_REPLACE_EXACT(00119ED0, cdecl, 0, void, game_alloc_tables_7ac824_7ac828_with_reset)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_weather_tables_7ac824 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_weather_tables_7ac824(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x007AC828u);
        return;
    }

    /* Reuse the sibling's retained allocation sequence; exact original call edges are inferred. */
    const uint32_t arguments[1] = {0u};
    call_guest(0x00119D40u, GAME_CC_cdecl, 0x00119ED5u, 0u, 0u, arguments, 0u);
    guest_write32(0x004E62E0u, 0xFFFFFFFFu);
    guest_write32(0x004E62E4u, 0xFFFFFFFFu);
    guest_write32(0x0073D9B0u, 0u);
}

GAME_REPLACE_EXACT(00220E20, cdecl, 0, void, game_alloc_two_field_state_block_and_reset_table_entry)
{
    const char *rust = getenv("TSFP_RUST_GAME_220E20");
    if (rust != NULL && strcmp(rust, "1") == 0 &&
        tsfp_kmem_game_wrapper_alloc_state_block_75c108 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_state_block_75c108(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0075C108u);
        return;
    }

    /* The entry is INFERRED; preserve its visible allocation and table-reset effects. */
    const uint32_t arguments[2] = {0x10u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x00220E25u, 8u, 0u, arguments, 2u);
    guest_write32(0x0075C108u, g_eax);
    const uint32_t table = guest_read32(0x004CA9B4u);
    guest_write32(table + 0x18u * 0x14u + 0x0Cu, 0xFFFFFFFFu);
    g_eax = guest_read32(0x0075C108u);
}

GAME_REPLACE_EXACT(002226F0, cdecl, 0, void, game_alloc_timer_state_block_and_reset_five_table_entries)
{
    const char *rust = getenv("TSFP_RUST_GAME_2226F0");
    if (rust != NULL && strcmp(rust, "1") == 0 &&
        tsfp_kmem_game_wrapper_alloc_timer_state_75c118 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_timer_state_75c118(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0075C118u);
        return;
    }

    const uint32_t arguments[2] = {0x10u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002226F9u, 8u, 0u, arguments, 2u);
    guest_write32(0x0075C118u, g_eax);
    const uint32_t table = guest_read32(0x004CA9B4u);
    static const uint32_t indices[5] = {0x156Fu, 0x1570u, 0x5F3u, 0x5F4u, 0x5F5u};
    for (size_t i = 0; i < sizeof(indices) / sizeof(indices[0]); ++i)
        guest_write32(table + indices[i] * 0x14u + 0x0Cu, 0xFFFFFFFFu);
    g_eax = guest_read32(0x0075C118u);
}

GAME_REPLACE_EXACT(002259D0, cdecl, 0, void, game_alloc_small_state_block_and_reset_two_table_entries)
{
    const char *rust = getenv("TSFP_RUST_GAME_2259D0");
    if (rust != NULL && strcmp(rust, "1") == 0 &&
        tsfp_kmem_game_wrapper_alloc_sound_state_75c124 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_sound_state_75c124(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0075C124u);
        return;
    }

    /* The 0x75C124 owner is inferred from its sound-hook reader; preserve the known effects. */
    const uint32_t arguments[2] = {0x10u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002259D9u, 8u, 0u, arguments, 2u);
    guest_write32(0x0075C124u, g_eax);
    const uint32_t table = guest_read32(0x004CA9B4u);
    guest_write32(table + 0x1Bu * 0x14u + 0x0Cu, 0xFFFFFFFFu);
    guest_write32(table + 0x19u * 0x14u + 0x0Cu, 0xFFFFFFFFu);
    g_eax = guest_read32(0x0075C124u);
}

GAME_REPLACE_EXACT(002210B0, cdecl, 0, void, game_alloc_sound_handle_array_state_block)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_table_75c10c != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_table_75c10c(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0075C10Cu);
        return;
    }

    /* The original wrapper is INFERRED; this adapter uses a synthetic return EIP. */
    const uint32_t arguments[2] = {0x40u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002210B9u, 8u, 0u, arguments, 2u);
    guest_write32(0x0075C10Cu, g_eax);
}

GAME_REPLACE_EXACT(0021AF30, cdecl, 0, void, game_ambient_voice_state_alloc)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_table_75c0f0 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_table_75c0f0(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0075C0F0u);
        return;
    }

    /* The entry label is INFERRED; keep its existing C-selected path authoritative by default. */
    const uint32_t arguments[2] = {0x10u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0021AF39u, 8u, 0u, arguments, 2u);
    guest_write32(0x0075C0F0u, g_eax);
}

GAME_REPLACE_EXACT(002A62C0, cdecl, 0, void, game_mode_0x51_alloc_buffer_7d00)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_table_78b0d0 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_table_78b0d0(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0078B0D0u);
        return;
    }

    /* The function label is INFERRED; this call site keeps the lifted C allocator authoritative by default. */
    const uint32_t arguments[2] = {0x7D00u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002A62C9u, 8u, 0u, arguments, 2u);
    guest_write32(0x0078B0D0u, g_eax);
}

GAME_REPLACE_EXACT(002A8300, cdecl, 1, void, game_alloc_pool_buffers_78b098_78b090_by_flag)
{
    const uint32_t flag = game_stack_arg(0u);
    const uint32_t count = flag != 0u ? 0x1F4u : 0x578u;
    const uint32_t size = flag != 0u ? 0x55F0u : 0xF0A0u;
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_tables_78b08c != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        uint32_t second = 0u;
        if (tsfp_kmem_game_wrapper_alloc_tables_78b08c(
                flag, &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, &second) != 0u)
            __builtin_trap();
        g_eax = second;
        return;
    }

    /* The constructor name is INFERRED; retain lifted C as the default path. */
    guest_write32(0x0078B08Cu, count);
    const uint32_t arguments[2] = {size, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl,
               flag != 0u ? 0x002A831Eu : 0x002A834Cu, 8u, 0u, arguments, 2u);
    guest_write32(0x0078B098u, g_eax);
    call_guest(0x0003DFA0u, GAME_CC_cdecl,
               flag != 0u ? 0x002A832Fu : 0x002A835Du, 8u, 0u, arguments, 2u);
    guest_write32(0x0078B090u, g_eax);
}

GAME_REPLACE_EXACT(0031AAA0, cdecl, 0, void, game_alloc_pick_node_copy_to_762f7c)
{
    const uint32_t source = game_stack_arg(0u);
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_pick_node_762f7c != NULL) {
        game_arena_report_context report_context = {.kind = 0u, .call_index = 0u};
        uint32_t block = 0u;
        if (tsfp_kmem_game_wrapper_alloc_pick_node_762f7c(
                source, &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, &block) != 0u)
            __builtin_trap();
        g_eax = block;
        return;
    }

    /* The retained entry is cdecl/0 but reads its caller's source pointer from [esp+4]. */
    const uint32_t arguments[2] = {0x14u, 0u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0031AAA9u, 8u, 0u, arguments, 2u);
    const uint32_t block = g_eax;
    for (uint32_t i = 0; i < 4u; ++i)
        guest_write32(block + i * 4u, guest_read32(source + i * 4u));
    const uint32_t fifth = guest_read32(source + 0x10u);
    guest_write32(0x00762F7Cu, block);
    guest_write32(block + 0x10u, fifth);
}

GAME_REPLACE_EXACT(0031C930, cdecl, 0, u32,
                   game_arena_release_tagged_block_of_static_pointer_and_return_it)
{
    const uint32_t block = guest_read32(0x00762F90u);
    const char *rust = getenv("TSFP_RUST_GAME_31C930");
    if (rust != NULL && strcmp(rust, "1") == 0 &&
        tsfp_kmem_game_wrapper_release_mapedit_static_block_762f90 != NULL) {
        game_arena_report_context report_context = {.kind = 0u, .call_index = 0u};
        game_arena_release_registers registers = {0};
        if (tsfp_kmem_game_wrapper_release_mapedit_static_block_762f90(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, &registers) != 0u)
            __builtin_trap();
        g_eax = registers.eax;
        g_ecx = registers.ecx;
        g_edx = registers.edx;
        return;
    }

    const uint32_t arguments[1] = {block};
    call_guest(0x0003E1E0u, GAME_CC_cdecl, 0x0031C93Bu, 4u, 0u, arguments, 1u);
    g_eax = guest_read32(0x00762F90u);
}

GAME_REPLACE_EXACT(0031E430, cdecl, 0, u32,
                   game_arena_release_tagged_block_of_record_rows_panel_pointer_and_return_it)
{
    const uint32_t block = guest_read32(0x007630B0u);
    const char *rust = getenv("TSFP_RUST_GAME_31E430");
    if (rust != NULL && strcmp(rust, "1") == 0 &&
        tsfp_kmem_game_wrapper_release_record_rows_panel_block_7630b0 != NULL) {
        game_arena_report_context report_context = {.kind = 0u, .call_index = 0u};
        game_arena_release_registers registers = {0};
        if (tsfp_kmem_game_wrapper_release_record_rows_panel_block_7630b0(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, &registers) != 0u)
            __builtin_trap();
        g_eax = registers.eax;
        g_ecx = registers.ecx;
        g_edx = registers.edx;
        return;
    }

    const uint32_t arguments[1] = {block};
    call_guest(0x0003E1E0u, GAME_CC_cdecl, 0x0031E43Bu, 4u, 0u, arguments, 1u);
    g_eax = guest_read32(0x007630B0u);
}

GAME_REPLACE_EXACT(003796B0, cdecl, 0, void, game_alloc_node_pool_772b50)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_node_pool_772b50 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        uint32_t nodes = 0u;
        if (tsfp_kmem_game_wrapper_alloc_node_pool_772b50(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, &nodes) != 0u)
            __builtin_trap();
        /* Match the register results left by 0x156DB0's final freelist iteration. */
        g_eax = nodes + 0x951u;
        g_ecx = g_eax;
        g_edx = nodes + 0x960u;
        return;
    }

    const uint32_t allocation_arguments[2] = {0x960u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x003796BCu, 8u, 0u,
               allocation_arguments, 2u);
    const uint32_t pool_arguments[4] = {0x772B50u, 0xA0u, g_eax, 0xFu};
    call_guest(0x00156DB0u, GAME_CC_cdecl, 0x003796CEu, 16u, 0u,
               pool_arguments, 4u);
}

GAME_REPLACE_EXACT(002760F0, cdecl, 0, void, game_alloc_0x190_to_global_78b240_and_zero_78b244)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_table_78b240 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_table_78b240(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        return;
    }

    guest_write32(0x0078B240u, 0u);
    guest_write32(0x0078B244u, 0u);
    guest_write32(0x0078B248u, 0u);
    guest_write32(0x0078B24Cu, 0u);
    const uint32_t arguments[2] = {0x190u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x00276112u, 8u, 0u, arguments, 2u);
    guest_write32(0x0078B240u, g_eax);
}

GAME_REPLACE_EXACT(001D1E80, cdecl, 0, void, game_alloc_aligned_75b4b4)
{
    const int rust_mode = rust_game_arena_mode();
    if (rust_mode == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_aligned_75b4b4 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_aligned_75b4b4(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0075B4B4u);
        return;
    }

    const uint32_t arguments[3] = {0x20u, 0x1590u, 1u};
    call_guest(0x0003E0B0u, GAME_CC_cdecl, 0x001D1E8Eu, 0u, 0u, arguments, 3u);
    guest_write32(0x0075B4B4u, g_eax);
}

GAME_REPLACE_EXACT(00018CA0, cdecl, 0, void, game_alloc_buffers_54ef14)
{
    const int rust_mode = rust_game_arena_mode();
    game_arena_shadow rust_state;
    uint32_t rust_status = 1u;
    const uint32_t entry_edx = g_edx;
    const int compare_rust = rust_mode == GAME_ARENA_COMPARE &&
                             tsfp_kmem_game_wrapper_alloc_buffers_54ef14 != NULL;
    if (rust_mode == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_buffers_54ef14 != NULL) {
        if (tsfp_kmem_game_wrapper_alloc_buffers_54ef14(
                NULL, game_arena_live_read32, game_arena_live_write32, NULL) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0054EF18u);
        g_ecx = guest_read32(0x006B8370u);
        return;
    }

    if (compare_rust) {
        rust_state = game_arena_shadow_from_guest();
        rust_status = tsfp_kmem_game_wrapper_alloc_buffers_54ef14(
            &rust_state, game_arena_shadow_read32, game_arena_shadow_write32, NULL);
    }

    guest_write32(0x0054EF20u, 0x800u);
    const uint32_t first_args[2] = {0x1A000u, 2u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x00018CB6u, 8u, 0u, first_args, 2u);
    guest_write32(0x0054EF14u, g_eax);
    const uint32_t second_size = (guest_read32(0x0054EF20u) + 0xFu) & 0xFFFFFFF0u;
    const uint32_t second_args[2] = {second_size, 2u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x00018CCEu, 8u, 0u, second_args, 2u);
    guest_write32(0x0054EF18u, g_eax);

    if (compare_rust) {
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        const uint32_t rust_eax = game_arena_shadow_read32(&rust_state, 0x0054EF18u);
        int mismatch = rust_status != 0u || rust_state.write_count != 3u ||
                       rust_state.write_overflow != 0;
        mismatch |= !game_arena_same_state(&rust_state, &c_state);
        mismatch |= g_eax != rust_eax || g_ecx != rust_state.region_a_cursor || g_edx != entry_edx;
        for (uint32_t i = 0; i < rust_state.write_count && i < 4u; ++i)
            mismatch |= guest_read32(rust_state.write_addresses[i]) != rust_state.write_values[i];
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u)
                fprintf(stderr,
                        "game-arena-54ef14: first mismatch EAX=%08x/%08x ECX=%08x/%08x status=%u writes=%u overflow=%d\n",
                        g_eax, rust_eax, g_ecx, rust_state.region_a_cursor, rust_status,
                        rust_state.write_count, rust_state.write_overflow);
        }
    }
}

GAME_REPLACE_EXACT(0032A860, cdecl, 0, void, game_alloc_save_slot_buffer_778d38)
{
    const int rust_mode = rust_game_arena_mode();
    const uint32_t entry_edi = g_edi;
    if (rust_mode == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_save_slot_778d38 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_save_slot_778d38(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        const uint32_t block = guest_read32(0x00778D38u);
        g_eax = 0u;
        g_ecx = block;
        g_edx = block;
        g_edi = entry_edi;
        return;
    }

    game_save_slot_shadow rust_state;
    uint32_t rust_status = 1u;
    const int compare_rust = rust_mode == GAME_ARENA_COMPARE &&
                             tsfp_kmem_game_wrapper_alloc_save_slot_778d38 != NULL;
    if (compare_rust) {
        rust_state = (game_save_slot_shadow){
            .arena = game_arena_shadow_from_guest(),
            .payload_start = guest_read32(0x006B837Cu),
        };
        rust_status = tsfp_kmem_game_wrapper_alloc_save_slot_778d38(
            &rust_state, game_save_slot_shadow_read32, game_save_slot_shadow_write32, NULL);
    }

    const uint32_t mode = guest_read32(0x0079094Cu);
    const uint32_t allocation_size = mode == 0x66u ? 0x191530u : 0x1530u;
    const uint32_t arguments[2] = {allocation_size, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0032A882u, 8u, 0u, arguments, 2u);
    const uint32_t block = g_eax;
    guest_write32(0x00778D38u, block);
    for (uint32_t i = 0; i < 0x54Cu; ++i)
        guest_write32(block + i * 4u, 0u);
    g_eax = 0u;
    g_ecx = guest_read32(0x00778D38u);
    guest_write32(g_ecx + 4u, g_eax);
    g_edx = guest_read32(0x00778D38u);
    guest_write32(g_edx + 8u, g_eax);
    g_edi = entry_edi;

    if (compare_rust) {
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        const uint32_t rust_block = game_arena_shadow_read32(&rust_state.arena, 0x00778D38u);
        int mismatch = rust_status != 0u || rust_block != block ||
                       rust_state.clear_write_count != 0x54Eu ||
                       rust_state.clear_mismatch != 0 ||
                       rust_state.arena.write_count != 1u ||
                       rust_state.arena.write_overflow != 0;
        mismatch |= !game_arena_same_state(&rust_state.arena, &c_state);
        mismatch |= guest_read32(0x00778D38u) != rust_block ||
                    g_eax != 0u || g_ecx != rust_block || g_edx != rust_block;
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u)
                fprintf(stderr,
                        "game-arena-save-slot: first mismatch C block=%08x Rust=%08x status=%u clear-writes=%u clear-error=%d\n",
                        block, rust_block, rust_status, rust_state.clear_write_count,
                        rust_state.clear_mismatch);
        }
    }
}

GAME_REPLACE_EXACT(002FE200, cdecl, 0, void, game_alloc_zeroed_8000_to_global_784014)
{
    const int rust_mode = rust_game_arena_mode();
    const uint32_t entry_edi = g_edi;
    if (rust_mode == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_mapedit_buffers != NULL) {
        game_arena_report_context report_context = {.kind = 2u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_mapedit_buffers(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = 0u;
        g_ecx = 0u;
        g_edi = entry_edi;
        return;
    }

    game_mapedit_buffer_shadow rust_state;
    uint32_t rust_status = 1u;
    const int compare_rust = rust_mode == GAME_ARENA_COMPARE &&
                             tsfp_kmem_game_wrapper_alloc_mapedit_buffers != NULL;
    if (compare_rust) {
        rust_state = (game_mapedit_buffer_shadow){
            .arena = game_arena_shadow_from_guest(),
        };
        rust_status = tsfp_kmem_game_wrapper_alloc_mapedit_buffers(
            &rust_state, game_mapedit_buffer_shadow_read32,
            game_mapedit_buffer_shadow_write32, NULL);
    }

    const uint32_t first_arguments[2] = {0x8000u, 2u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002FE20Du, 8u, 0u, first_arguments, 2u);
    const uint32_t first = g_eax;
    guest_write32(0x00784014u, first);
    for (uint32_t i = 0; i < 0x2000u; ++i)
        guest_write32(first + i * 4u, 0u);

    const uint32_t undo_arguments[2] = {0x10000u, 2u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0033660Du, 8u, 0u, undo_arguments, 2u);
    const uint32_t undo = g_eax;
    guest_write32(0x00765C24u, undo);
    for (uint32_t i = 0; i < 0x4000u; ++i)
        guest_write32(undo + i * 4u, 0u);
    guest_write32(0x00765C28u, 0u);
    guest_write32(0x00765C30u, 0u);
    guest_write32(0x00765C20u, 0u);

    if (compare_rust) {
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        int mismatch = rust_status != 0u ||
                       !game_arena_same_state(&rust_state.arena, &c_state) ||
                       rust_state.first_buffer != first || rust_state.undo_buffer != undo ||
                       rust_state.first_clear_count != 0x2000u ||
                       rust_state.undo_clear_count != 0x4000u ||
                       rust_state.head_count != 3u || rust_state.stage != 4u ||
                       rust_state.mismatch || guest_read32(0x00765C28u) != 0u ||
                       guest_read32(0x00765C30u) != 0u || guest_read32(0x00765C20u) != 0u;
        for (uint32_t i = 0; i < 0x2000u && !mismatch; ++i)
            mismatch |= guest_read32(first + i * 4u) != 0u;
        for (uint32_t i = 0; i < 0x4000u && !mismatch; ++i)
            mismatch |= guest_read32(undo + i * 4u) != 0u;
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u)
                fprintf(stderr,
                        "game-arena-2fe200: first mismatch C=%08x/%08x Rust=%08x/%08x status=%u clear-writes=%u/%u heads=%u error=%d\n",
                        first, undo, rust_state.first_buffer, rust_state.undo_buffer, rust_status,
                        rust_state.first_clear_count, rust_state.undo_clear_count,
                        rust_state.head_count, rust_state.mismatch);
        }
    }

    g_eax = 0u;
    g_ecx = 0u;
    g_edi = entry_edi;
}

GAME_REPLACE_EXACT(00271910, cdecl, 0, void, game_alloc_weather_tables_74c3b0)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_weather_74c3b0 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        uint32_t eax = 0;
        if (tsfp_kmem_game_wrapper_alloc_weather_74c3b0(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, &eax) != 0u)
            __builtin_trap();
        g_eax = eax;
        return;
    }

    const uint32_t first_args[2] = {0x550u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0027191Cu, 0u, 0u, first_args, 2u);
    guest_write32(0x0074C3B0u, g_eax);
    const uint32_t gate_byte = guest_read8(0x007DE402u);
    const int8_t gate = (int8_t)gate_byte;
    g_eax = (g_eax & 0xFFFFFF00u) | (gate_byte & 0xFFu);
    if (gate > 0) {
        const uint32_t second_args[2] = {0x970u, 1u};
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x00271939u, 0u, 0u, second_args, 2u);
        guest_write32(0x0074C3B4u, g_eax);
    }
}

/* 00274650: allocate the fixed table, then the signed-count side table. The Rust route
 * is opt-in; the C fallback retains both original allocator call boundaries. */
GAME_REPLACE_EXACT(00274650, cdecl, 0, void, game_alloc_tables_74c3b8)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_tables_74c3b8 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        uint32_t eax = 0;
        if (tsfp_kmem_game_wrapper_alloc_tables_74c3b8(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, &eax) != 0u)
            __builtin_trap();
        g_eax = eax;
        return;
    }

    const uint32_t first_args[2] = {0x100u, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0027465Cu, 0u, 0u, first_args, 2u);
    guest_write32(0x0074C3B8u, g_eax);

    const int32_t count = (int8_t)guest_read8(0x007DE402u);
    const uint32_t second_size = ((uint32_t)count * 12u + 0xFu) & 0xFFFFFFF0u;
    const uint32_t second_args[2] = {second_size, 1u};
    call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x0027467Du, 0u, 0u, second_args, 2u);
    guest_write32(0x0074C3BCu, g_eax);
}

/* 002BC150: the saved EDI dword is part of the tail-callee frame. The native builder at
 * 002BBC40 reads the original caller's two stack values at ESP+8/+12 after this slot is kept. */
GAME_REPLACE_EXACT(002BC150, cdecl, 0, void, game_alloc_effect_mesh_buffer_78aef0)
{
    const uint32_t entry_esp = g_esp;
    const uint32_t saved_edi = g_edi;
    const uint32_t saved_seh_ebp = g_seh_ebp;
    guest_write32(entry_esp - 4u, saved_edi);
    g_esp = entry_esp - 4u;

    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_effect_mesh_buffer_78aef0 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_effect_mesh_buffer_78aef0(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
    } else {
        const uint32_t arguments[2] = {0xD818u, 1u};
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002BC15Cu, 0u, 0u, arguments, 2u);
        guest_write32(0x0078AEF0u, g_eax);
    }

    g_seh_ebp = saved_seh_ebp;
    if (game_guest_tail(0x002BBC40u, GAME_CC_cdecl, 0u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
    g_esp = entry_esp;
}

GAME_REPLACE_EXACT(00188900, cdecl, 0, void, game_alloc_effect_slots_7a156c)
{
    const int rust_mode = rust_game_arena_mode();
    if (rust_mode == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_effect_slots_7a156c != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        uint32_t block = 0;
        if (tsfp_kmem_game_wrapper_alloc_effect_slots_7a156c(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, &block) != 0u)
            __builtin_trap();
        g_eax = block;
        call_guest(0x00188870u, GAME_CC_cdecl, guest_read32(g_esp), 0u, 0u, NULL, 0u);
        return;
    }

    game_arena_shadow rust_state;
    uint32_t rust_block = 0u;
    uint32_t rust_status = 1u;
    const int compare_rust = rust_mode == GAME_ARENA_COMPARE &&
                             tsfp_kmem_game_wrapper_alloc_effect_slots_7a156c != NULL;
    if (compare_rust) {
        rust_state = game_arena_shadow_from_guest();
        rust_status = tsfp_kmem_game_wrapper_alloc_effect_slots_7a156c(
            &rust_state, game_arena_shadow_read32, game_arena_shadow_write32, NULL,
            &rust_block);
    }

    const uint32_t arguments[3] = {0x20u, 0x194u, 1u};
    call_guest(0x0003E0B0u, GAME_CC_cdecl, 0x0018890Eu, 0u, 0u, arguments, 3u);
    const uint32_t c_block = g_eax;
    guest_write32(0x007A156Cu, c_block);

    if (compare_rust) {
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        int mismatch = rust_status != 0u || rust_block != c_block ||
                       rust_state.write_count != 1u || rust_state.write_overflow != 0;
        mismatch |= !game_arena_same_state(&rust_state, &c_state);
        for (uint32_t i = 0; i < rust_state.write_count && i < 4u; ++i)
            mismatch |= guest_read32(rust_state.write_addresses[i]) != rust_state.write_values[i];
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u)
                fprintf(stderr,
                        "game-arena-effect-slots: first mismatch C block=%08x Rust=%08x status=%u writes=%u overflow=%d\n",
                        c_block, rust_block, rust_status, rust_state.write_count,
                        rust_state.write_overflow);
        }
    }
    call_guest(0x00188870u, GAME_CC_cdecl, guest_read32(g_esp), 0u, 0u, NULL, 0u);
}

/* 00237480 allocates the Story instance arrays, clears the third, then calls 235630. */
GAME_REPLACE_EXACT(00237480, cdecl, 0, void, game_alloc_story_event_instances_78cd90)
{
    const uint32_t saved_esi = g_esi;
    const uint32_t saved_edi = g_edi;
    guest_write32(g_esp - 4u, saved_esi);
    g_esp -= 4u;
    guest_write32(g_esp - 4u, saved_edi);
    g_esp -= 4u;
    const uint32_t mode = guest_read32(0x0079094Cu);
    const uint32_t configured_count = guest_read32(0x0075CCB4u);
    const uint32_t instance_count = mode == 0x65u
                                        ? 0u
                                        : configured_count;
    const uint32_t event_bytes = instance_count * 0x48u;
    g_edx = configured_count;
    g_eax = instance_count;
    g_ecx = event_bytes;
    g_esi = instance_count;
    g_edi = mode;
    const int rust_mode = rust_game_arena_mode();
    const int rust_story_arrays =
        tsfp_kmem_game_wrapper_alloc_story_event_instances_78cd90 != NULL &&
        tsfp_kmem_game_wrapper_alloc_story_event_support_arrays != NULL;
    if (rust_mode == GAME_ARENA_RUST && rust_story_arrays) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        uint32_t block = 0u;
        const uint32_t allocator_esi = g_esi;
        g_esi = (event_bytes + 15u) & 0xFFFFFFF0u;
        if (tsfp_kmem_game_wrapper_alloc_story_event_instances_78cd90(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, event_bytes, &block) != 0u)
            __builtin_trap();
        g_esi = allocator_esi;
        g_eax = block;
    }

    game_arena_shadow rust_state;
    uint32_t rust_block = 0u;
    uint32_t rust_status = 1u;
    const int compare_rust = rust_mode == GAME_ARENA_COMPARE &&
                             tsfp_kmem_game_wrapper_alloc_story_event_instances_78cd90 != NULL;
    if (compare_rust) {
        rust_state = game_arena_shadow_from_guest();
        rust_status = tsfp_kmem_game_wrapper_alloc_story_event_instances_78cd90(
            &rust_state, game_arena_shadow_read32, game_arena_shadow_write32, NULL,
            event_bytes, &rust_block);
    }

    uint32_t c_block = g_eax;
    if (rust_mode != GAME_ARENA_RUST || !rust_story_arrays) {
        const uint32_t arguments[2] = {event_bytes, 1u};
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002374A9u, 0u, 0u, arguments, 2u);
        c_block = g_eax;
        guest_write32(0x0078CD90u, c_block);
    }

    if (compare_rust) {
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        int mismatch = rust_status != 0u || rust_block != c_block ||
                       rust_state.write_count != 1u || rust_state.write_overflow != 0;
        mismatch |= !game_arena_same_state(&rust_state, &c_state);
        for (uint32_t i = 0; i < rust_state.write_count && i < 4u; ++i)
            mismatch |= guest_read32(rust_state.write_addresses[i]) != rust_state.write_values[i];
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u)
                fprintf(stderr,
                        "game-arena-story-events: first mismatch C block=%08x Rust=%08x status=%u writes=%u overflow=%d\n",
                        c_block, rust_block, rust_status, rust_state.write_count,
                        rust_state.write_overflow);
        }
    }

    const uint32_t index_bytes = instance_count * 4u;
    const uint32_t state_bytes = instance_count << 4;
    uint32_t index_block = 0u;
    uint32_t state_block = 0u;
    game_arena_shadow support_state;
    uint32_t support_status = 1u;
    const int compare_support = rust_mode == GAME_ARENA_COMPARE && rust_story_arrays;
    if (compare_support) {
        support_state = game_arena_shadow_from_guest();
        support_status = tsfp_kmem_game_wrapper_alloc_story_event_support_arrays(
            &support_state, game_arena_shadow_read32, game_arena_shadow_write32, NULL,
            index_bytes, state_bytes, &index_block, &state_block);
    }
    if (rust_mode == GAME_ARENA_RUST && rust_story_arrays) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_story_event_support_arrays(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report, index_bytes, state_bytes, &index_block,
                &state_block) != 0u)
            __builtin_trap();
        g_esi = state_bytes;
        g_edx = state_bytes;
        g_eax = state_block;
    } else {
        g_edx = index_bytes;
        const uint32_t index_arguments[2] = {index_bytes, 1u};
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002374BDu, 0u, 0u, index_arguments, 2u);
        index_block = g_eax;
        guest_write32(0x0078CD8Cu, index_block);

        g_esi = state_bytes;
        const uint32_t state_arguments[2] = {state_bytes, 1u};
        call_guest(0x0003DFA0u, GAME_CC_cdecl, 0x002374CDu, 0u, 0u, state_arguments, 2u);
        state_block = g_eax;
        guest_write32(0x0075CCA4u, state_block);
    }
    if (compare_support) {
        const game_arena_shadow c_state = game_arena_shadow_from_guest();
        int mismatch = support_status != 0u || index_block != guest_read32(0x0078CD8Cu) ||
                       state_block != guest_read32(0x0075CCA4u) ||
                       support_state.write_count != 2u || support_state.write_overflow != 0;
        mismatch |= !game_arena_same_state(&support_state, &c_state);
        for (uint32_t i = 0; i < support_state.write_count && i < 4u; ++i)
            mismatch |= guest_read32(support_state.write_addresses[i]) !=
                        support_state.write_values[i];
        if (mismatch) {
            const uint64_t previous = atomic_fetch_add_explicit(&g_rust_game_arena_mismatches, 1u,
                                                                 memory_order_relaxed);
            if (previous == 0u)
                fprintf(stderr,
                        "game-arena-story-event-arrays: first mismatch indices=%08x snapshot=%08x Rust-status=%u writes=%u overflow=%d\n",
                        index_block, state_block, support_status, support_state.write_count,
                        support_state.write_overflow);
        }
    }
    g_esi = state_bytes;
    g_ecx = state_bytes;
    g_edx = g_ecx;
    g_edi = state_block;
    g_ecx >>= 2;
    g_eax = 0u;
    const uint32_t zero_words = g_ecx;
    if (g_df == 0u) {
        for (uint32_t i = 0; i < zero_words; ++i) {
            guest_write32(g_edi, 0u);
            g_edi += 4u;
        }
    } else {
        for (uint32_t i = 0; i < zero_words; ++i) {
            guest_write32(g_edi, 0u);
            g_edi -= 4u;
        }
    }
    g_ecx = 0u;
    g_ecx = g_edx & 3u;

    g_edi = guest_read32(g_esp);
    g_esp += 4u;
    g_esi = guest_read32(g_esp);
    g_esp += 4u;
    call_guest(0x00235630u, GAME_CC_cdecl, guest_read32(g_esp), 0u, 0u, NULL, 0u);
}
#endif

/* 00045A70: cdecl(). 1CEB00(mode [79094C]) refreshes the cached mode flag [75AF10] through its
 * byte map and two-slot jump table, then the result is (flag == 0) where the flag word is
 * [5236B8] when 1CEB00 returned non-zero and [5236B4] otherwise. */
GAME_REPLACE_EXACT(00045A70, cdecl, 0, u32, game_music_volume_setting_is_zero_by_mode_cache_flag)
{
    g_eax = guest_read32(0x0079094Cu);
    const uint32_t arguments[1] = {g_eax};
    call_guest(0x001CEB00u, GAME_CC_cdecl, 0x00045A7Bu, 0u, 0u, arguments, 1u);
    g_ecx = guest_read32(0x005236B8u);
    if (g_eax == 0u) {
        g_ecx = guest_read32(0x005236B4u);
    }
    g_eax = g_ecx == 0u ? 1u : 0u;
}

#include "game_guest_tail.h"

#if defined(TSFP_RUST_GAME_ARENA)
/* 0024A180: Rust may own the allocator wrapper while the original initializer remains the
 * immediate tail target. The runtime switch is unset by default. */
GAME_REPLACE_EXACT(0024A180, cdecl, 0, void, game_alloc_640_aligned_20_to_78b718_then_init)
{
    if (rust_game_arena_mode() == GAME_ARENA_RUST &&
        tsfp_kmem_game_wrapper_alloc_model_buffer_78b718 != NULL) {
        game_arena_report_context report_context = {.kind = 1u, .call_index = 0u};
        if (tsfp_kmem_game_wrapper_alloc_model_buffer_78b718(
                &report_context, game_arena_live_read32, game_arena_live_write32,
                game_arena_live_report) != 0u)
            __builtin_trap();
        g_eax = guest_read32(0x0078B718u);
    } else {
        const uint32_t arguments[3] = {0x20u, 0x640u, 1u};
        call_guest(0x0003E0B0u, GAME_CC_cdecl, 0x0024A18Eu, 0u, 0u, arguments, 3u);
        guest_write32(0x0078B718u, g_eax);
    }

    if (game_guest_tail(0x002498C0u, GAME_CC_cdecl, 0u) != GAME_GUEST_CALL_OK)
        __builtin_trap();
}
#endif

/* 0008E380: cdecl(blob). With blob and [blob+0x10] non-null the array at [blob+0x10] (16-byte
 * records, key in the first dword, terminated by key -1) is walked: each key goes to 18CE0
 * (which marks a record flag and may call 18300), then the record key is overwritten with
 * 0x40000000. The function ends in a TAIL jmp to 389B0 (a bare RET) once the array is done
 * (also when the first key is already -1). A null blob or null array returns directly with EAX
 * as it entered. EBX, ESI and EDI are saved and restored by the original. */
GAME_REPLACE_EXACT(0008E380, cdecl, 1, u32, game_model_blob_release_texture_entries_and_mark_keys_unloaded)
{
    guest_write32(g_esp - 4u, g_ebx);
    g_ebx = game_stack_arg(0u);
    if (g_ebx == 0u) {
        g_ebx = guest_read32(g_esp - 4u);
        return;
    }
    guest_write32(g_esp - 8u, g_esi);
    g_esi = guest_read32(g_ebx + 0x10u);
    if (g_esi == 0u) {
        g_esi = guest_read32(g_esp - 8u);
        g_ebx = guest_read32(g_esp - 4u);
        return;
    }
    if (guest_read32(g_esi) != 0xFFFFFFFFu) {
        guest_write32(g_esp - 12u, g_edi);
        g_edi = 0u;
        do {
            g_eax = guest_read32(g_esi);
            const uint32_t arguments[1] = {g_eax};
            call_guest(0x00018CE0u, GAME_CC_cdecl, 0x0008E3A8u, 12u, 0u, arguments, 1u);
            guest_write32(g_esi, 0x40000000u);
            g_ecx = guest_read32(g_ebx + 0x10u);
            g_edi += 0x10u;
            g_eax = guest_read32(g_ecx + g_edi);
            g_esi = g_ecx + g_edi;
        } while (g_eax != 0xFFFFFFFFu);
        g_edi = guest_read32(g_esp - 12u);
    }
    g_esi = guest_read32(g_esp - 8u);
    g_ebx = guest_read32(g_esp - 4u);
    if (game_guest_tail(0x000389B0u, GAME_CC_cdecl, 0u) != GAME_GUEST_CALL_OK) {
        __builtin_trap();
    }
}

/* 000BE860: cdecl(type_mask, index). Masks 1..0x40 go through
 * `dec mask / cmp 0x3F / ja / movzx ecx, byte [mask - 1 + BE950] / jmp [ecx*4 + BE934]`
 * (seven slots, the byte map sends the unused masks to slot 6): slots 0..4 return
 * table + index * 0x54, slot 5 (mask 0x40) asks 1B99B0(index) for a weapon slot and returns
 * 4EA484 + slot * 0xE0 when it is in 1..0x2B. Every exit that returns 0 in the original shares
 * the one instruction `xor eax, eax` at BE931 (negative index, unknown masks, the JA default,
 * slot 6, a failed weapon lookup), so each of them runs through `case 6: default:` here.
 * Masks above 0x80 are plain compares (0x100 and 0x1000 share a table, 0x200, 0x400 and 0x800
 * have their own, 0x800 returns the table address itself). ECX leaves as the byte (the slot)
 * on the table path and as mask - 1 on the JA path. */
GAME_REPLACE_EXACT(000BE860, cdecl, 2, u32, game_object_type_entry_address_by_type_mask_and_index)
{
    uint32_t slot = 0xFFFFFFFFu;
    g_eax = game_stack_arg(1u);
    if ((int32_t)g_eax >= 0) {
        g_ecx = game_stack_arg(0u);
        if ((int32_t)g_ecx > 0x80) {
            if ((int32_t)g_ecx > 0x400) {
                if (g_ecx == 0x800u) {
                    g_eax = 0x004DC568u;
                    return;
                }
                if (g_ecx == 0x1000u) {
                    g_eax = g_eax * 0x54u + 0x004DC768u;
                    return;
                }
            } else if (g_ecx == 0x400u) {
                g_eax = g_eax * 0x54u + 0x004DC510u;
                return;
            } else if (g_ecx == 0x100u) {
                g_eax = g_eax * 0x54u + 0x004DC768u;
                return;
            } else if (g_ecx == 0x200u) {
                g_eax = g_eax * 0x54u + 0x004DDAE8u;
                return;
            }
        } else if (g_ecx == 0x80u) {
            g_eax = g_eax * 0x54u + 0x004DC3C0u;
            return;
        } else {
            g_ecx -= 1u;
            if (g_ecx <= 0x3Fu) {
                g_ecx = guest_read8(0x000BE950u + g_ecx);
                slot = g_ecx;
            }
        }
    }
    switch (slot) {
    case 0:
        g_eax = g_eax * 0x54u + 0x004DC218u;
        break;
    case 1:
        g_eax = g_eax * 0x54u + 0x004FB190u;
        break;
    case 2:
        g_eax = g_eax * 0x54u + 0x004DBA38u;
        break;
    case 3:
        g_eax = g_eax * 0x54u + 0x004DBE28u;
        break;
    case 4:
        g_eax = g_eax * 0x54u + 0x004DC668u;
        break;
    case 5: {
        const uint32_t arguments[1] = {g_eax};
        call_guest(0x001B99B0u, GAME_CC_cdecl, 0x000BE8B3u, 0u, 0u, arguments, 1u);
        if ((int32_t)g_eax > 0 && (int32_t)g_eax < 0x2C) {
            g_eax = g_eax * 0xE0u + 0x004EA484u;
            break;
        }
        __attribute__((fallthrough));
    }
    case 6:
    default:
        g_eax = 0u;
        break;
    }
}

/* 000C0E60: cdecl(type, index, _, hint, weapon). A type 0x40 request with 0 <= index < 0x46, a
 * zero fourth argument and a fifth argument in 1..0x2B returns 4EA484 + weapon * 0xE0 directly;
 * every other request is forwarded to BE860(type, index) (ECX = index, EDX = type, EAX = result). */
GAME_REPLACE_EXACT(000C0E60, cdecl, 5, u32, game_object_type_entry_address_with_weapon_table_4ea484_for_type_0x40)
{
    g_ecx = game_stack_arg(1u);
    g_edx = game_stack_arg(0u);
    if ((int32_t)g_ecx >= 0 && g_edx == 0x40u && (int32_t)g_ecx < 0x46) {
        g_eax = game_stack_arg(3u);
        if (g_eax == 0u) {
            g_eax = game_stack_arg(4u);
            if ((int32_t)g_eax > 0 && (int32_t)g_eax < 0x2C) {
                g_eax = g_eax * 0xE0u + 0x004EA484u;
                return;
            }
        }
    }
    const uint32_t arguments[2] = {g_edx, g_ecx};
    call_guest(0x000BE860u, GAME_CC_cdecl, 0x000C0E9Eu, 0u, 0u, arguments, 2u);
}

/* 000D6ED0: cdecl(id). D6DD0(id) (byte map over id - 0x12, bound 0x98, two-slot jump table) is
 * non-zero for the ids that map to slot 0; the result is 1 only when it returned 0 and the id is
 * none of 0x4D, 0x4F, 0x64. ECX holds the id across the call, EDX and the rest are untouched. */
GAME_REPLACE_EXACT(000D6ED0, cdecl, 1, u32, game_char_type_id_not_in_set_d6dd0_and_not_0x4d_0x4f_0x64)
{
    g_ecx = game_stack_arg(0u);
    const uint32_t arguments[1] = {g_ecx};
    call_guest(0x000D6DD0u, GAME_CC_cdecl, 0x000D6EDAu, 0u, 0u, arguments, 1u);
    if (g_eax != 0u || g_ecx == 0x4Du || g_ecx == 0x4Fu || g_ecx == 0x64u) {
        g_eax = 0u;
        return;
    }
    g_eax = 1u;
}

/* 000E05B0: cdecl(owner, result_block). Zeroes the seven dwords of the block, then walks the
 * entries first..last (the globals [736680] and [737E44]) of the 0x22C-byte table at [737BD4]:
 * every live entry (dword 0 is not -1) whose +0x158 equals `owner` is passed to E0540 with
 * ECX = the entry type and EAX = the block (E0540 is a byte map plus eight-slot jump table
 * that increments one dword of the block). ESI and EDI are saved and restored. */
GAME_REPLACE_EXACT(000E05B0, cdecl, 2, u32, game_table_737bd4_for_entries_with_field_0x158_equal_arg_call_e0540)
{
    g_ecx = guest_read32(0x00737E44u);
    g_eax = game_stack_arg(1u);
    g_edx = 0u;
    guest_write32(g_esp - 4u, g_esi);
    g_esi = guest_read32(0x00736680u);
    guest_write32(g_esp - 8u, g_edi);
    g_edi = g_eax;
    for (uint32_t offset = 0u; offset <= 0x18u; offset += 4u) {
        guest_write32(g_eax + offset, g_edx);
    }
    g_ecx += 1u;
    if ((int32_t)g_esi >= (int32_t)g_ecx) {
        g_edi = guest_read32(g_esp - 8u);
        g_esi = guest_read32(g_esp - 4u);
        return;
    }
    g_edi = guest_read32(0x00737BD4u);
    g_edx = g_esi * 0x22Cu + g_edi;
    g_edi = game_stack_arg(0u);
    g_ecx -= g_esi;
    g_esi = g_ecx;
    do {
        g_ecx = guest_read32(g_edx);
        if (g_ecx != 0xFFFFFFFFu && guest_read32(g_edx + 0x158u) == g_edi) {
            call_guest(0x000E0540u, GAME_CC_cdecl, 0x000E060Bu, 8u, 0u, NULL, 0u);
        }
        g_edx += 0x22Cu;
        g_esi -= 1u;
    } while (g_esi != 0u);
    g_edi = guest_read32(g_esp - 8u);
    g_esi = guest_read32(g_esp - 4u);
}
