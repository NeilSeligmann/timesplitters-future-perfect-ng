/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1788 batch2 five fixed-tuple admissions. See docs/t1788-gameplay-batch2.md. */
/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1788 batch2 hand translation of original instructions. GPR-exact guest order. */
#include "game_replace.h"
extern __thread uint32_t g_ebx,g_ebp,g_edi;
static void b2_push(uint32_t v) { g_esp-=4; guest_write32(g_esp,v); }
static uint32_t b2_pop(void) { uint32_t v=guest_read32(g_esp); g_esp+=4; return v; }
GAME_REPLACE_EXACT(000C63F0,cdecl,2,u32,game_gp_b2_object_218_flags) {
 g_eax=guest_read32(g_esp+4); g_ecx=guest_read32(g_eax+0x21C); g_edx=guest_read32(g_esp+8); g_ecx|=1;
 b2_push(g_esi); guest_write32(g_eax+0x218,g_edx); guest_write32(g_eax+0x21C,g_ecx);
 g_esi=guest_read32(0x7B0CE8); uint32_t active=g_esi; g_esi=b2_pop();
 if(!active && !g_edx) { g_ecx|=4; } else { g_ecx&=~4u; }
 guest_write32(g_eax+0x21C,g_ecx);
}

GAME_REPLACE_EXACT(000CFEC0,cdecl,2,u32,game_gp_b2_six_slot_nth_or_fallback) {
 b2_push(g_ebx); g_ebx=guest_read32(g_esp+8); b2_push(g_ebp); b2_push(g_esi); g_esi=guest_read32(g_esp+0x14); b2_push(g_edi); g_edi=g_ebx+0x103C;
again:
 g_edx=g_esi; g_ecx=0; g_eax=g_edi;
loop:
 g_ebp=guest_read32(g_eax-0xC);
 if(g_ebp && guest_read32(g_eax)) { if(!g_edx) goto found; --g_edx; }
 ++g_ecx; g_eax+=0xCC;
 if((int32_t)g_ecx<6) goto loop;
 if(g_esi) { g_esi=0; goto again; }
 g_eax=guest_read32(g_ebx+0xF78); goto done;
found:
 g_ecx*=0xCC; g_eax=guest_read32(g_ecx+g_ebx+0x1030);
done:
 g_edi=b2_pop(); g_esi=b2_pop(); g_ebp=b2_pop(); g_ebx=b2_pop();
}

GAME_REPLACE_EXACT(00150200,cdecl,3,u32,game_gp_b2_three_global_outputs) {
 g_eax=guest_read32(g_esp+4); g_ecx=guest_read32(0x749160); g_edx=guest_read32(g_esp+8); guest_write32(g_eax,g_ecx);
 g_eax=guest_read32(0x749158); g_ecx=guest_read32(g_esp+0xC); guest_write32(g_edx,g_eax);
 g_edx=guest_read32(0x749148); guest_write32(g_ecx,g_edx);
}

/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1788 batch2 original movss/xorps whitelist contracts. */
#include "game_replace.h"
typedef union { float f[4]; uint32_t u[4]; uint64_t q[2]; } GpB2Xmm;
__thread GpB2Xmm g_xmm0 __attribute__((weak,aligned(8)));
static void b2_load_scalar(uint32_t value) { g_xmm0.u[0]=value; g_xmm0.u[1]=0; g_xmm0.u[2]=0; g_xmm0.u[3]=0; }
GAME_REPLACE_EXACT(000B2D20,cdecl,1,u32,game_gp_b2_clear_object_c0_c8) {
 g_eax=guest_read32(g_esp+4); g_xmm0.q[0]=0; g_xmm0.q[1]=0;
 guest_write32(g_eax+0xC0,g_xmm0.u[0]); guest_write32(g_eax+0xC8,g_xmm0.u[0]); guest_write8(g_eax+0x32,1);
}

GAME_REPLACE_EXACT(000C9440,cdecl,1,u32,game_gp_b2_ordered_vector_globals) {
 g_eax=guest_read32(g_esp+4); b2_load_scalar(guest_read32(g_eax)); guest_write32(0x7B0CD0,g_xmm0.u[0]);
 b2_load_scalar(guest_read32(g_eax+4)); guest_write32(0x7B0CD4,g_xmm0.u[0]);
 b2_load_scalar(guest_read32(g_eax+8)); guest_write32(0x7B0CD8,g_xmm0.u[0]);
}

