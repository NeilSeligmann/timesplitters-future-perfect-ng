/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1815: C side of the extended register bridge and the GuestCall trampoline. */
#include "rust_ext_bridge.h"
#include "host_runtime.h"
#include "frame_state_sync.h"
#include <stdlib.h>
#include <string.h>
#include "../game/game_replace.h"
#include "../game/x87_replace.h"

typedef union { float f[4]; double d[2]; uint32_t u[4]; int32_t i[4]; uint64_t q[2]; } rust_ext_guest_xmm;
extern __thread uint32_t g_ebp, g_ebx, g_edi, g_seh_ebp;
extern uint32_t g_xbox_code_lo, g_xbox_code_hi;
extern __thread rust_ext_guest_xmm g_xmm0, g_xmm1, g_xmm2, g_xmm3, g_xmm4, g_xmm5, g_xmm6, g_xmm7;
typedef void (*rust_ext_lifted)(void);
extern rust_ext_lifted recomp_lookup(uint32_t xbox_va);
/* T2409: Rust's selected 0x3D370 root still crosses the existing host frame seams
 * for 0x3C440 and 0x1538C0 so pacing, render replay, and timing selection keep working. */
extern void __wrap_sub_0003C440(void);
extern void __wrap_sub_001538C0(void);

extern bool tsfp_rust_configure_ext(const unsigned char *spec, size_t len);
extern bool tsfp_rust_try_ext(uint32_t va, rust_ext_registers *regs, uintptr_t offset,
                              void (*call)(uint32_t, rust_ext_registers *));
extern uint64_t tsfp_rust_count_ext(uint32_t va, bool rust);

void rust_ext_load(const rust_ext_registers *r)
{
    g_eax = r->eax; g_ecx = r->ecx; g_edx = r->edx; g_esp = r->esp;
    g_ebx = r->ebx; g_ebp = r->ebp; g_esi = r->esi; g_edi = r->edi;
    memcpy(g_fp_stack, r->fpu.stack, sizeof g_fp_stack);
    g_fp_top = (int)r->fpu.top; g_fp_control_word = r->fpu.control;
    memcpy(&g_xmm0, &r->xmm[0], 16); memcpy(&g_xmm1, &r->xmm[1], 16);
    memcpy(&g_xmm2, &r->xmm[2], 16); memcpy(&g_xmm3, &r->xmm[3], 16);
    memcpy(&g_xmm4, &r->xmm[4], 16);
    memcpy(&g_xmm5, &r->xmm_hi[0], 16); memcpy(&g_xmm6, &r->xmm_hi[1], 16);
    memcpy(&g_xmm7, &r->xmm_hi[2], 16);
}

void rust_ext_store(rust_ext_registers *r)
{
    r->eax = g_eax; r->ecx = g_ecx; r->edx = g_edx; r->esp = g_esp;
    r->ebx = g_ebx; r->ebp = g_ebp; r->esi = g_esi; r->edi = g_edi;
    memcpy(r->fpu.stack, g_fp_stack, sizeof g_fp_stack);
    r->fpu.top = (uint32_t)g_fp_top & 7u; r->fpu.control = g_fp_control_word;
    memcpy(&r->xmm[0], &g_xmm0, 16); memcpy(&r->xmm[1], &g_xmm1, 16);
    memcpy(&r->xmm[2], &g_xmm2, 16); memcpy(&r->xmm[3], &g_xmm3, 16);
    memcpy(&r->xmm[4], &g_xmm4, 16);
    memcpy(&r->xmm_hi[0], &g_xmm5, 16); memcpy(&r->xmm_hi[1], &g_xmm6, 16);
    memcpy(&r->xmm_hi[2], &g_xmm7, 16);
}

/* T1835 w12: optional hook installed only by the entity-tick differential. A pseudo call target
 * 0xFD000000 | label resumes the lifted tick at `label` with the local EBP in regs->ebp. */
void (*rust_ext_resume_hook)(uint32_t label, uint32_t local_ebp);

void rust_ext_guest_call(uint32_t va, rust_ext_registers *regs)
{
    if ((va & 0xFF000000u) == 0xFD000000u) {
        if (rust_ext_resume_hook == NULL) tsfp_bridge_fail(HOST_STOP_BRIDGE_CONTRACT, va, g_esp, g_esp, "resume hook not installed");
        rust_ext_load(regs);
        g_ebp = g_seh_ebp; /* the host keeps g_ebp == g_seh_ebp; the lifted frame register is local */
        rust_ext_resume_hook(va & 0x00FFFFFFu, regs->ebp);
        rust_ext_store(regs);
        return;
    }
    /* Lifted RECOMP_ICALL not-code path (indirect hooks such as definition+0x294): the callee
     * does not run, EAX becomes 0, ESP stays at the caller's value. Direct callees are in range. */
    if (!(va >= 0xFE000000u || g_xbox_code_hi == 0u || (va >= g_xbox_code_lo && va < g_xbox_code_hi))) {
        rust_ext_load(regs);
        g_eax = 0u;
        rust_ext_store(regs);
        return;
    }
    rust_ext_lifted fn = va == 0x0003C440u ? __wrap_sub_0003C440 :
                         va == 0x001538C0u ? __wrap_sub_001538C0 : recomp_lookup(va);
    if (fn == NULL) tsfp_bridge_fail(HOST_STOP_ICALL_UNRESOLVED, va, g_esp, g_esp, "ext nested guest call has no lifted function");
    rust_ext_load(regs);
    g_esp -= 4u;                       /* return slot, the lifted RET does esp += 4 */
    guest_write32(g_esp, 0u);
    const uint32_t entry_esp = g_esp;
    const uint32_t chkstk_probe = g_eax;
    TSFP_FRAME_SYNC_OUT();
    fn();
    TSFP_FRAME_SYNC_IN();
    /* T2314: _chkstk 0x3C9D90 allocates EAX bytes of the caller's frame (exit ESP entry + 4 - EAX). */
    if (!(va == 0x3C9D90u && g_esp == entry_esp + 4u - chkstk_probe) && g_esp != entry_esp + 4u) tsfp_bridge_fail(HOST_STOP_BRIDGE_CONTRACT, va, entry_esp, g_esp, "ext callee left an unexpected ESP");
    rust_ext_store(regs);
}

int rust_ext_try(uint32_t va)
{
    rust_ext_registers regs;
    rust_ext_store(&regs);
    if (!tsfp_rust_try_ext(va, &regs, (uintptr_t)g_xbox_mem_offset, rust_ext_guest_call)) return 0;
    rust_ext_load(&regs);
    return 1;
}

bool host_rust_ext_configure(const char *spec)
{
    return tsfp_rust_configure_ext((const unsigned char *)spec, strlen(spec));
}
