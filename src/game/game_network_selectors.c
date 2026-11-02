/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "game_replace.h"
extern __thread uint32_t g_edi;
extern __thread int g_df;
GAME_REPLACE_EXACT(003B20F0, thiscall, 0, u32, game_network_b2_003b20f0)
{ g_eax=(g_eax&0xFFFFFF00u)|guest_read8(g_ecx+0x34u); if((g_eax&0xFFu)==0u) g_eax=0u; else if((int32_t)guest_read32(g_ecx+0x30u)<3) g_eax=0u; else g_eax=1u; }
GAME_REPLACE_EXACT(003B2110, thiscall, 0, u32, game_network_b2_003b2110)
{ g_eax=(g_eax&0xFFFFFF00u)|guest_read8(g_ecx+0x34u); if((g_eax&0xFFu)==0u) g_eax=0u; else if((int32_t)guest_read32(g_ecx+0x30u)<5) g_eax=0u; else g_eax=1u; }
GAME_REPLACE_EXACT(003B2130, thiscall, 0, u32, game_network_b2_003b2130)
{ g_eax=(g_eax&0xFFFFFF00u)|guest_read8(g_ecx+0x35u); if((g_eax&0xFFu)==0u) g_eax=0u; else if((int32_t)guest_read32(g_ecx+0x30u)<3) g_eax=0u; else g_eax=1u; }
GAME_REPLACE_EXACT(003B2150, thiscall, 0, u32, game_network_b2_003b2150)
{ g_eax=(g_eax&0xFFFFFF00u)|guest_read8(g_ecx+0x35u); if((g_eax&0xFFu)==0u) g_eax=0u; else if((int32_t)guest_read32(g_ecx+0x30u)<5) g_eax=0u; else g_eax=1u; }
