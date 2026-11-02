/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "unlock_override.h"

bool host_unlock_override_apply(bool enabled, host_guest_write_u32_fn write_u32)
{
    if (!enabled) return true;
    return write_u32 != 0 && write_u32(0x0075AEF4u, 1u);
}
