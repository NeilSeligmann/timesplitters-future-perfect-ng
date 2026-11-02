/* SPDX-License-Identifier: GPL-3.0-or-later */
#include "recomp_guest_call.h"
#include "kernel_call.h"
#include "recomp_abi.h"
#ifdef TSFP_RUST_DISPATCH
#include "rust_dispatch.h" /* T3869: Rust twin, opt-in at startup */
#endif
extern TSFP_RECOMP_TLS int g_fp_top; /* lifted x87 model top (x87_replace.h) */
extern TSFP_RECOMP_TLS double g_fp_stack[8]; /* lifted x87 model registers (x87_replace.h) */
#ifdef TSFP_RUST_AUDIO_HOST
static bool guest_call_read_registers(uint32_t registers[8])
{
    if (registers == NULL) return false;
    registers[0] = g_eax; registers[1] = g_ecx; registers[2] = g_edx; registers[3] = g_ebx;
    registers[4] = g_esi; registers[5] = g_edi; registers[6] = g_ebp; registers[7] = g_esp;
    return true;
}
static bool guest_call_set_esp(uint32_t esp) { g_esp = esp; return true; }
static uint32_t guest_call_get_esp(void) { return g_esp; }
static bool guest_call_restore_registers(const uint32_t registers[8])
{
    if (registers == NULL) return false;
    g_eax = registers[0]; g_ecx = registers[1]; g_edx = registers[2]; g_ebx = registers[3];
    g_esi = registers[4]; g_edi = registers[5]; g_ebp = registers[6]; g_esp = registers[7];
    return true;
}
#endif
bool recomp_guest_call_stdcall(uint32_t address, const uint32_t *arguments, unsigned count)
{
#ifdef TSFP_RUST_DISPATCH
    if (rust_dispatch_selected()) return rust_dispatch_guest_call_stdcall(address, arguments, count);
#endif
#ifdef TSFP_RUST_AUDIO_HOST
    const recomp_guest_call_host host = {
        .host_pid = kernel_host_pid(),
        .lookup = recomp_lookup,
        .read_registers = guest_call_read_registers,
        .set_esp = guest_call_set_esp,
        .get_esp = guest_call_get_esp,
        .restore_registers = guest_call_restore_registers,
    };
    return tsfp_audio_guest_call_stdcall_rust(address, arguments, count, &host);
#else
    if (count > GUEST_CALL_MAX_ARGS || (count != 0u && arguments == NULL)) return false;
    const recomp_func_t function = recomp_lookup(address);
    if (function == NULL) return false;
    const uint32_t frame_bytes = 4u * (count + 1u), top = g_esp;
    if (top < frame_bytes) return false;
    const uint32_t base = top - frame_bytes;
    uint32_t frame[GUEST_CALL_MAX_ARGS + 1u] = {0u};
    for (unsigned i = 0u; i < count; i++) frame[1u + i] = arguments[i];
    /* The guarded write is all or nothing (the straddling case is pinned in the routes test), so a
     * frame that is not writable stack leaves every byte alone and is refused here. */
    if (!kernel_guest_write_bytes(base, frame, frame_bytes)) return false;
    const uint32_t saved[8] = {g_eax, g_ecx, g_edx, g_ebx, g_esi, g_edi, g_ebp, top};
    g_esp = base;
    function();
    const bool balanced = g_esp == top;
    g_eax = saved[0];
    g_ecx = saved[1];
    g_edx = saved[2];
    g_ebx = saved[3];
    g_esi = saved[4];
    g_edi = saved[5];
    g_ebp = saved[6];
    g_esp = saved[7];
    return balanced;
#endif
}

static bool guest_call_cdecl(uint32_t address, const uint32_t *arguments, unsigned count, uint32_t *result,
                             double *st0)
{
#ifdef TSFP_RUST_DISPATCH
    if (rust_dispatch_selected()) return rust_dispatch_guest_call_cdecl(address, arguments, count, result, st0);
#endif
    if (count > GUEST_CALL_CDECL_MAX_ARGS || (count != 0u && arguments == NULL)) return false;
    const recomp_func_t function = recomp_lookup(address);
    if (function == NULL) return false;
    /* 64 bytes of headroom below the interrupted frame, then [ret][args...]. */
    const uint32_t frame_bytes = 4u * (count + 1u), top = g_esp;
    if (top < frame_bytes + 64u) return false;
    const uint32_t base = top - 64u - frame_bytes;
    uint32_t frame[GUEST_CALL_CDECL_MAX_ARGS + 1u] = {0u};
    for (unsigned i = 0u; i < count; i++) frame[1u + i] = arguments[i];
    if (!kernel_guest_write_bytes(base, frame, frame_bytes)) return false;
    const uint32_t eax = g_eax, ecx = g_ecx, edx = g_edx, ebx = g_ebx, esi = g_esi, edi = g_edi, ebp = g_ebp;
    const int fp_top = g_fp_top;
    const uint16_t fp_control = g_fp_control_word;
    g_esp = base;
    function();
    const bool balanced = g_esp == base + 4u;
    if (result != NULL) *result = g_eax;
    /* A float result is the one value the callee pushed: the top moved down by one. */
    if (st0 != NULL) *st0 = (((unsigned)fp_top + 7u) & 7u) == ((unsigned)g_fp_top & 7u) ? g_fp_stack[g_fp_top & 7] : __builtin_nan("");
    g_eax = eax, g_ecx = ecx, g_edx = edx, g_ebx = ebx;
    g_esi = esi, g_edi = edi, g_ebp = ebp, g_esp = top;
    g_fp_top = fp_top;
    g_fp_control_word = fp_control;
    return balanced;
}

bool recomp_guest_call_cdecl(uint32_t address, const uint32_t *arguments, unsigned count, uint32_t *result)
{
    return guest_call_cdecl(address, arguments, count, result, NULL);
}

bool recomp_guest_call_cdecl_st0(uint32_t address, const uint32_t *arguments, unsigned count, double *st0)
{
    return guest_call_cdecl(address, arguments, count, NULL, st0);
}
