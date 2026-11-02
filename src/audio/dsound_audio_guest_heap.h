/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_AUDIO_DSOUND_AUDIO_GUEST_HEAP_H
#define TSFP_AUDIO_DSOUND_AUDIO_GUEST_HEAP_H

#include "guest_mem.h"

/* Keep every DirectSound lease in one heap namespace. The Rust host selects
 * the authoritative Rust GuestRegions backend only when Rust kmem is active;
 * the C host and compare/off kmem modes retain the C guest heap. */
#if defined(TSFP_RUST_AUDIO_HOST) && defined(TSFP_RUST_KMEM)
extern bool tsfp_kmem_audio_heap_active(void);
extern uint32_t tsfp_kmem_audio_heap_create(uint32_t, uint32_t, uint32_t);
extern uint32_t tsfp_kmem_audio_heap_alloc(uint32_t, uint32_t);
extern bool tsfp_kmem_audio_heap_valid(uint32_t);
extern bool tsfp_kmem_audio_heap_block_size(uint32_t, uint32_t, uint32_t *);
extern bool tsfp_kmem_audio_heap_destroy(uint32_t);

static inline bool dsound_audio_heap_uses_rust(void)
{
    return tsfp_kmem_audio_heap_active();
}
#else
static inline bool dsound_audio_heap_uses_rust(void) { return false; }
#endif

static inline uint32_t dsound_audio_heap_create(uint32_t flags, uint32_t initial,
                                                uint32_t maximum)
{
#if defined(TSFP_RUST_AUDIO_HOST) && defined(TSFP_RUST_KMEM)
    if (dsound_audio_heap_uses_rust())
        return tsfp_kmem_audio_heap_create(flags, initial, maximum);
#endif
    return kmem_heap_create(flags, initial, maximum);
}

static inline uint32_t dsound_audio_heap_alloc(uint32_t heap, uint32_t bytes)
{
#if defined(TSFP_RUST_AUDIO_HOST) && defined(TSFP_RUST_KMEM)
    if (dsound_audio_heap_uses_rust())
        return tsfp_kmem_audio_heap_alloc(heap, bytes);
#endif
    return kmem_heap_alloc(heap, bytes);
}

static inline bool dsound_audio_heap_valid(uint32_t heap)
{
#if defined(TSFP_RUST_AUDIO_HOST) && defined(TSFP_RUST_KMEM)
    if (dsound_audio_heap_uses_rust())
        return tsfp_kmem_audio_heap_valid(heap);
#endif
    return kmem_heap_valid(heap);
}

static inline bool dsound_audio_heap_block_size(uint32_t heap, uint32_t address,
                                                uint32_t *size)
{
#if defined(TSFP_RUST_AUDIO_HOST) && defined(TSFP_RUST_KMEM)
    if (dsound_audio_heap_uses_rust())
        return tsfp_kmem_audio_heap_block_size(heap, address, size);
#endif
    return kmem_heap_block_size(heap, address, size);
}

static inline bool dsound_audio_heap_destroy(uint32_t heap)
{
#if defined(TSFP_RUST_AUDIO_HOST) && defined(TSFP_RUST_KMEM)
    if (dsound_audio_heap_uses_rust())
        return tsfp_kmem_audio_heap_destroy(heap);
#endif
    return kmem_heap_destroy(heap);
}

#endif
