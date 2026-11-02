/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_SHADOW_PORT_STACK_H
#define TSFP_SHADOW_PORT_STACK_H
#include <stdbool.h>
#include <stdint.h>

/* A per-thread isolated 4 GiB address window, for audited ports that read one
 * stack argument and perform no guest writes or callbacks. The caller must
 * execute under a fault guard and always call end after a guarded fault. Only
 * one argument word is copied; other bytes in its page are not a guest image.
 * The window is released automatically at thread exit. */
bool shadow_stack_begin(uint32_t address, uint32_t argument, uintptr_t *offset);
void shadow_stack_end(void);
void shadow_stack_destroy(void);
#endif
