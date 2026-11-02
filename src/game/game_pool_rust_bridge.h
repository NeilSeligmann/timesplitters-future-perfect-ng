/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_GAME_POOL_RUST_BRIDGE_H
#define TSFP_GAME_POOL_RUST_BRIDGE_H

#include <stdint.h>

enum { GAME_POOL_C = 0, GAME_POOL_RUST = 1, GAME_POOL_COMPARE = 2 };
int game_pool_rust_mode(void);
int game_pool_rust_fixed_pool_init(uint32_t pool, uint32_t stride,
                                   uint32_t storage, uint32_t count,
                                   uint32_t *out_head, uint32_t *out_end);

typedef struct game_pool_rust_shadow {
    uint32_t count;
    uint32_t addresses[32];
    uint32_t values[32];
    int overflow;
} game_pool_rust_shadow;

typedef struct game_pool_rust_call {
    int mode;
    int active;
    uint32_t result;
    uint32_t old_size;
    uint32_t rounded;
    game_pool_rust_shadow shadow;
} game_pool_rust_call;

int game_pool_rust_begin(uint32_t size, game_pool_rust_call *call,
                         uint32_t *eax, uint32_t *ecx, uint32_t *edx);
void game_pool_rust_finish(game_pool_rust_call *call,
                           uint32_t eax, uint32_t ecx, uint32_t edx);
int game_pool_rust_free_begin(uint32_t pointer, game_pool_rust_call *call);
void game_pool_rust_free_finish(game_pool_rust_call *call);
int game_pool_rust_clear_begin(game_pool_rust_call *call,
                               uint32_t *eax, uint32_t *ecx);
void game_pool_rust_clear_finish(game_pool_rust_call *call,
                                 uint32_t eax, uint32_t ecx);
int game_pool_rust_allocate_by_kind(uint32_t size, uint32_t kind,
                                    uint32_t argument, uint32_t callback,
                                    uint32_t *result);

#endif
