/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_SHADOW_PORT_DENYLIST_H
#define TSFP_SHADOW_PORT_DENYLIST_H

#include <stdbool.h>
#include <stdint.h>

/* Return -1 on I/O failure, otherwise whether the exact VA is present. */
int shadow_denylist_contains(const char *path, uint32_t va);
/* Append a VA once, returning false on I/O failure. */
bool shadow_denylist_add(const char *path, uint32_t va);
#endif
