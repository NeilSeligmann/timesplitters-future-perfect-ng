/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1930 test-only register/memory bridge. Lifted bodies are appended privately. */
#define RECOMP_GENERATED_CODE
#include "recomp_types.h"
ptrdiff_t g_xbox_mem_offset;
__thread uint32_t g_eax, g_ecx, g_edx, g_esp, g_ebx, g_esi, g_edi, g_ebp;
__thread uint32_t g_seh_ebp, g_fs_base;
__thread int g_df;
void t1930_dispatch(uint32_t va);
static uint32_t script_index;
void t1930_run(uint8_t *base, uint32_t va, uint32_t r[8]) {
    g_xbox_mem_offset = (ptrdiff_t)base;
    g_eax=r[0]; g_ecx=r[1]; g_edx=r[2]; g_ebx=r[3];
    g_esi=r[4]; g_edi=r[5]; g_ebp=r[6]; g_esp=r[7];
    g_df=0; g_seh_ebp=r[6];
    script_index=0;
    t1930_dispatch(va);
    r[0]=g_eax; r[1]=g_ecx; r[2]=g_edx; r[3]=g_ebx;
    r[4]=g_esi; r[5]=g_edi; r[6]=g_ebp; r[7]=g_esp;
}

/* Scripted external boundary: snapshots normalized GPRs + two argument words.
 * cdecl callees preserve nonvolatile registers; tail targets consume outer RET. */
static void scripted(uint32_t va) {
    uint32_t a = 0x30000 + 44 * script_index;
    MEM32(a)=va; MEM32(a+4)=g_eax; MEM32(a+8)=g_ecx;
    MEM32(a+12)=g_edx; MEM32(a+16)=g_ebx; MEM32(a+20)=g_esi;
    MEM32(a+24)=g_edi; MEM32(a+28)=g_ebp; MEM32(a+32)=g_esp+4;
    MEM32(a+36)=MEM32(g_esp+4); MEM32(a+40)=MEM32(g_esp+8);
    ++script_index;
    g_eax ^= va; g_ecx += script_index; g_edx ^= 0x19300000 + script_index;
    g_esp += 4;
}
#define SCRIPT(va) void sub_##va(void) { scripted(0x##va); }
SCRIPT(0012C370)
SCRIPT(00063B40)
SCRIPT(000BDAB0)
SCRIPT(001B6130)
SCRIPT(0022CE60)
SCRIPT(0012C730)
SCRIPT(002FE270)
SCRIPT(000A5E40)
/* This boundary deliberately models the external call, not a nested root. */
void t1930_boundary_257b30(void) { scripted(0x257B30); }
