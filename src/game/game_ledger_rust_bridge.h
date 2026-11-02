/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_GAME_LEDGER_RUST_BRIDGE_H
#define TSFP_GAME_LEDGER_RUST_BRIDGE_H

#include <stdint.h>

int game_ledger_rust_allocate(uint32_t requested, uint32_t *result);
/* 1 = Rust completed, 0 = unset/off/unavailable, -1 = selected Rust route failed. */
int game_ledger_rust_ensure_buffer(uint32_t force);
/* 1 = Rust completed, 0 = unset/off/unavailable, -1 = selected Rust route failed. */
int game_ledger_rust_history_reset(uint32_t mode);
int game_ledger_rust_release(uint32_t count, uint32_t block,
                             uint32_t *eax, uint32_t *ecx, uint32_t *edx);
int game_ledger_rust_secondary_enabled(void);
int game_ledger_rust_secondary_append(uint32_t *eax, uint32_t *ecx, uint32_t *edx);
int game_ledger_rust_secondary_release(uint32_t count, uint32_t block,
                                       uint32_t *eax, uint32_t *ecx, uint32_t *edx);

#endif
