/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3824: the live hit census hot path. tools.live_census.transform puts `LIVE_HIT(0xVA);` at the top of every lifted
 * function of a census build (and this include at the top of each chunk). The VA is a compile-time constant, so the
 * hit is one load from a fixed address and, on the first entry only, one byte store. No call, lock or atomic. A
 * test-and-set rather than a plain store keeps hot functions from dirtying a cache line other threads read.
 *
 * The store is an asm statement whose only output is that one byte. A plain C store of a uint8_t may alias anything
 * (char type, and the lifted code is built with -fno-strict-aliasing), so the compiler had to reload every register
 * global (g_eax, g_esp, ...) after each probe in inlined callees: the plain-store design measured about +50% on the
 * deck (docs/t3824-live-hit-census.md). The asm form tells it exactly which byte changes.
 */
#ifndef TSFP_HOST_LIVE_CENSUS_HIT_H
#define TSFP_HOST_LIVE_CENSUS_HIT_H

#include <stdint.h>

/* Guest VAs index the map directly; the XBE image lies well below 16 MB. */
#define LIVE_CENSUS_VA_SPAN 0x01000000u

/* Hidden: one RIP-relative load in the PIE host, not a GOT load first. */
extern uint8_t g_live_census_hits[LIVE_CENSUS_VA_SPAN] __attribute__((visibility("hidden")));

#if defined(__x86_64__) || defined(__i386__)
#define LIVE_CENSUS_SET(byte) __asm__ volatile("movb $1, %0" : "=m"(byte))
#else
#define LIVE_CENSUS_SET(byte) ((byte) = 1u)
#endif

#define LIVE_HIT(va)                                                                                                   \
    do {                                                                                                               \
        if ((uint32_t)(va) < LIVE_CENSUS_VA_SPAN && __builtin_expect(g_live_census_hits[(uint32_t)(va)] == 0u, 0))     \
            LIVE_CENSUS_SET(g_live_census_hits[(uint32_t)(va)]);                                                       \
    } while (0)

#endif /* TSFP_HOST_LIVE_CENSUS_HIT_H */
