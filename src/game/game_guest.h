/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * How hand-written game code touches guest memory.
 *
 * A guest address is a uint32_t and stays one. Guest memory is identity-mapped into
 * the low 4 GB (see context.md section 2), so a guest address becomes a host pointer by
 * adding the same offset the lifted code adds in XBOX_PTR, which ships as 0. Going
 * through that offset, rather than casting directly, keeps hand code in step with the
 * lifter if the memory model ever moves.
 *
 * Fields are read and written with memcpy so the compiler cannot assume alignment or
 * aliasing the guest never promised. At -O2 each helper is a single mov.
 */
#ifndef TSFP_GAME_GUEST_H
#define TSFP_GAME_GUEST_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>

/* The lifter's memory-model offset, defined by src/host/recomp_runtime.c and by the
 * harness runtime. Zero in every build this project makes. */
extern ptrdiff_t g_xbox_mem_offset;

typedef uint32_t guest_addr;

/* T2409 typed seam for frame words with a Rust owner. This only covers callers
 * that use these game_guest helpers; raw lifted C memory instructions bypass
 * it and must be replaced or routed separately before those words are owned
 * by Rust across the complete live frame path. */
#if defined(__GNUC__) || defined(__clang__)
extern uint32_t tsfp_frame_state_owned_word_access(guest_addr address, uint32_t width,
                                                   uint32_t write, uint32_t *value)
    __attribute__((weak));
#else
extern uint32_t tsfp_frame_state_owned_word_access(guest_addr address, uint32_t width,
                                                   uint32_t write, uint32_t *value);
#endif

static inline int game_owned_frame_word_access(guest_addr address, uint32_t width,
                                               uint32_t write, uint32_t *value)
{
#if defined(__GNUC__) || defined(__clang__)
    if (tsfp_frame_state_owned_word_access == NULL) return 0;
#endif
    return tsfp_frame_state_owned_word_access(address, width, write, value) != 0u;
}

static inline void *game_host_ptr(guest_addr address)
{
    return (void *)((uintptr_t)address + (uintptr_t)g_xbox_mem_offset);
}

static inline uint32_t guest_read32(guest_addr address)
{
    uint32_t value;
    if (game_owned_frame_word_access(address, 4u, 0u, &value)) return value;
    memcpy(&value, game_host_ptr(address), sizeof value);
    return value;
}

static inline void guest_write32(guest_addr address, uint32_t value)
{
    if (game_owned_frame_word_access(address, 4u, 1u, &value)) return;
    memcpy(game_host_ptr(address), &value, sizeof value);
}

static inline uint8_t guest_read8(guest_addr address)
{
    uint32_t owner_value;
    if (game_owned_frame_word_access(address, 1u, 0u, &owner_value)) return (uint8_t)owner_value;
    uint8_t value;
    memcpy(&value, game_host_ptr(address), sizeof value);
    return value;
}

static inline void guest_write8(guest_addr address, uint8_t value)
{
    uint32_t widened = value;
    if (game_owned_frame_word_access(address, 1u, 1u, &widened)) return;
    memcpy(game_host_ptr(address), &value, sizeof value);
}

/* One 16-bit guest access, matching an original `word ptr` operand (a single access, no byte split). */
static inline uint16_t guest_load16(guest_addr address)
{
    uint32_t owner_value;
    if (game_owned_frame_word_access(address, 2u, 0u, &owner_value)) return (uint16_t)owner_value;
    uint16_t value;
    memcpy(&value, game_host_ptr(address), sizeof value);
    return value;
}

static inline void guest_store16(guest_addr address, uint16_t value)
{
    uint32_t widened = value;
    if (game_owned_frame_word_access(address, 2u, 1u, &widened)) return;
    memcpy(game_host_ptr(address), &value, sizeof value);
}

#endif /* TSFP_GAME_GUEST_H */
