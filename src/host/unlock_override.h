/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_HOST_UNLOCK_OVERRIDE_H
#define TSFP_HOST_UNLOCK_OVERRIDE_H

#include <stdbool.h>
#include <stdint.h>

typedef bool (*host_guest_write_u32_fn)(uint32_t address, uint32_t value);

/* T2354 TEMPORARY-SHIM: set the game's own unlock override global after XBE load. */
bool host_unlock_override_apply(bool enabled, host_guest_write_u32_fn write_u32);

#endif
