/* SPDX-License-Identifier: GPL-3.0-or-later
 * Original 0x0003E440: aligned allocation from the title's block ledger.
 * Reuse a released row only when its size exactly matches the aligned request;
 * otherwise append a new row and advance the recorded byte total.
 */
#include "game_replace.h"
#include "game_ledger_rust_bridge.h"

extern __thread uint32_t g_ebp, g_ebx, g_esi, g_edi, g_esp;

#define LEDGER_ROWS 0x006B7B68u
#define LEDGER_BASE 0x006B836Cu
#define LEDGER_BYTES 0x006B8394u
#define LEDGER_COUNT 0x006B8398u
#define LEDGER_ENABLED 0x004C81B4u

static void save_guest_word(uint32_t value)
{
    g_esp -= 4u;
    guest_write32(g_esp, value);
}

static uint32_t restore_guest_word(void)
{
    uint32_t value = guest_read32(g_esp);
    g_esp += 4u;
    return value;
}

static uint32_t game_allocate_ledger_c(uint32_t requested_bytes)
{
    uint32_t initial_bytes = guest_read32(LEDGER_BYTES);
    uint32_t enabled = guest_read32(LEDGER_ENABLED);
    save_guest_word(g_ebp);
    uint32_t base = guest_read32(LEDGER_BASE);
    uint32_t size = (requested_bytes + 15u) & UINT32_C(0xfffffff0);
    uint32_t appended_block = initial_bytes + base;
    if (enabled == 0u) {
        g_ebp = restore_guest_word();
        return 0u;
    }

    save_guest_word(g_ebx);
    uint32_t count = guest_read32(LEDGER_COUNT);
    save_guest_word(g_esi);
    save_guest_word(g_edi);
    uint32_t last = count - 1u;
    uint32_t offset = 0u;
    if (last != 0u && (last & UINT32_C(0x80000000)) == 0u) {
        for (uint32_t row = 0u; row < last; ++row) {
            uint32_t address = LEDGER_ROWS + row * 8u;
            if (guest_read32(address) == 0u && guest_read32(address + 4u) == size) {
                g_edi = restore_guest_word();
                uint32_t block = offset + base;
                g_esi = restore_guest_word();
                g_ebx = restore_guest_word();
                /* Keep even the redundant size write, and its ordering before
                 * the pointer publication and final saved-EBP read. */
                guest_write32(address + 4u, size);
                guest_write32(address, block);
                g_ebp = restore_guest_word();
                return block;
            }
            offset += guest_read32(address + 4u);
        }
    }

    /* Reload the recorded total after the scan, as in the original. The newest
     * existing row is excluded from exact-size reuse. There is no capacity test. */
    uint32_t next_bytes = guest_read32(LEDGER_BYTES) + size;
    guest_write32(LEDGER_ROWS + count * 8u + 4u, size);
    g_edi = restore_guest_word();
    guest_write32(LEDGER_ROWS + count * 8u, appended_block);
    guest_write32(LEDGER_BYTES, next_bytes);
    g_esi = restore_guest_word();
    guest_write32(LEDGER_COUNT, count + 1u);
    g_ebx = restore_guest_word();
    g_ebp = restore_guest_word();
    return appended_block;
}

static uint32_t game_allocate_ledger(uint32_t requested_bytes)
{
    uint32_t result = 0;
    if (game_ledger_rust_allocate(requested_bytes, &result)) return result;
    return game_allocate_ledger_c(requested_bytes);
}

/* Current caller audit proves ECX/EDX dead at all21direct sites. Preserved
 * registers, guest saved-frame writes and EAX/ESP remain part of the contract. */
GAME_REPLACE(0003E440, cdecl, 1, u32, game_allocate_ledger)

#if defined(TSFP_RUST_GAME_LEDGER)
static void game_ledger_call_guest(uint32_t target, uint32_t return_pc,
                                  uint32_t caller_bytes, const uint32_t *arguments,
                                  unsigned count)
{
    if (game_guest_call(target, GAME_CC_cdecl, return_pc, caller_bytes, 0u, 0u,
                        arguments, count) != GAME_GUEST_CALL_OK)
        __builtin_trap();
}

/* 0x3E740 resets the two history buffers after rewinding the arena top. The CMake and
 * runtime selectors default off; this readable C adapter is the C-authoritative path when
 * the Rust route is unset, and retains its real allocator/report call edges. */
static void game_history_buffer_reset_by_mode(uint32_t mode)
{
    const int rust = game_ledger_rust_history_reset(mode);
    if (rust < 0) __builtin_trap();
    if (rust > 0) return;

    const uint32_t caller_return = guest_read32(g_esp);
    if (mode == 0u) {
        const uint8_t gate = guest_read8(0x007DE455u);
        uint32_t size = gate == 0x0Au ? 0x4B000u : 0x96000u;
        guest_write32(0x006B83ACu, size);
        if (guest_read32(0x0079094Cu) == 0x64u) {
            if (gate != 0x0Au)
                game_ledger_call_guest(0x001CDAE0u, caller_return, 0u, NULL, 0u);
            size = 0xBB800u;
            guest_write32(0x006B83ACu, size);
        }
    } else if (mode == 1u) {
        guest_write32(0x006B83ACu, 0x4B000u);
    } else if (mode == 2u) {
        guest_write32(0x006B83ACu, 0x100000u);
    } else if (mode == 3u) {
        guest_write32(0x006B83ACu, 0x4000u);
    } else {
        const uint32_t argument[1] = {0u};
        game_ledger_call_guest(0x000389B0u, caller_return, 4u, argument, 1u);
    }

    game_ledger_call_guest(0x0003E240u, caller_return, 0u, NULL, 0u);
    const uint32_t size = guest_read32(0x006B83ACu);
    const uint32_t allocation[2] = {size + size, 0u};
    game_ledger_call_guest(0x0003DFA0u, caller_return, 0u, allocation, 2u);
    const uint32_t buffer = g_eax;
    const uint32_t second = buffer + guest_read32(0x006B83ACu);
    guest_write32(0x006B83B4u, buffer);
    guest_write32(0x006B83A0u, buffer);
    guest_write32(0x006B83A4u, second);
    guest_write32(0x006B839Cu, buffer);
    guest_write32(0x006B83B0u, 0u);
    const uint32_t report[1] = {mode};
    game_ledger_call_guest(0x000389B0u, caller_return, 4u, report, 1u);
}

GAME_REPLACE_EXACT(0003E740, cdecl, 1, void, game_history_buffer_reset_by_mode)
{
    game_history_buffer_reset_by_mode(game_stack_arg(0u));
}

/* 0x3E820 is the retained history-reset thunk edge to 0x3E740. Keep the guest
 * frame intact and enter the same C/Rust-selectable history service. */
GAME_REPLACE_EXACT(0003E820, cdecl, 1, void, game_history_reset_thunk)
{
    game_history_buffer_reset_by_mode(game_stack_arg(0u));
}
#endif
