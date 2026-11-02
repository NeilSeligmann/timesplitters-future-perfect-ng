/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1476 claude-104: 0008C8E0 passed all six fixed tuples (seeds 20261001/20261006 x O0/O2/O3). */
#include "game_replace.h"

static int32_t sgn(uint32_t value)
{
    return (int32_t)value;
}

/* 0008C8E0: cdecl(). Clears field +0 of unowned (+-4 == 0) 0x24-stride entries of the
 * table at 0x7B69A0 whose field is -1. */
GAME_REPLACE_EXACT(0008C8E0, cdecl, 0, u32, game_light_table_7b69a0_clear_minus_one_field_of_unowned_entries)
{
    g_ecx = guest_read32(0x0070AAE0u);
    if (sgn(g_ecx) <= 0)
        return;
    g_eax = 0x007B69C0u;
    do {
        g_edx = guest_read32(g_eax - 4u);
        if (g_edx == 0u && guest_read32(g_eax) == 0xFFFFFFFFu)
            guest_write32(g_eax, 0u);
        g_eax += 0x24u;
        g_ecx--;
    } while (g_ecx != 0u);
}
