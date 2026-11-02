/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_HOST_RUST_EXT_BRIDGE_H
#define TSFP_HOST_RUST_EXT_BRIDGE_H
#include <stdbool.h>
#include <stdint.h>
/* Extended register record shared with crates/tsfp-ports/src/abi.rs (ExtRegisters). */
typedef struct { double stack[8]; uint32_t top; uint16_t control; } rust_ext_fpu;
typedef struct { uint32_t lanes[4]; } rust_ext_xmm;
typedef struct {
    uint32_t eax, ecx, edx, esp, ebx, ebp, esi, edi, eflags;
    rust_ext_fpu fpu;
    rust_ext_xmm xmm[5];
    rust_ext_xmm xmm_hi[3]; /* XMM5-7 (T1815 w7) */
} rust_ext_registers;
_Static_assert(sizeof(rust_ext_registers) == 240u, "ExtRegisters layout");
_Static_assert(__builtin_offsetof(rust_ext_registers, fpu) == 40u, "ExtRegisters fpu offset");
_Static_assert(__builtin_offsetof(rust_ext_registers, xmm) == 112u, "ExtRegisters xmm offset");
_Static_assert(__builtin_offsetof(rust_ext_registers, xmm_hi) == 192u, "ExtRegisters xmm_hi offset");
/* Load/store the guest register globals (g_eax.., g_fp_stack, g_xmm0..4). */
void rust_ext_load(const rust_ext_registers *regs);
void rust_ext_store(rust_ext_registers *regs);
/* GuestCall trampoline: runs the lifted function for `va` (cdecl, RET owned here). Aborts
 * if the VA has no lifted function or the callee does not return with esp == entry + 4. */
void rust_ext_guest_call(uint32_t va, rust_ext_registers *regs);
/* Try the Rust body of an ext root from a lifted entry point. Returns 1 when Rust ran
 * (guest registers updated, ESP left at entry, caller's RET unchanged), 0 otherwise. */
int rust_ext_try(uint32_t va);
extern void (*rust_ext_resume_hook)(uint32_t label, uint32_t local_ebp);
bool host_rust_ext_configure(const char *spec);
/* T2145: names the failure on stderr and ends the run through the stop report (reason is a host_stop_reason). */
void tsfp_bridge_fail(int reason, uint32_t target, uint32_t entry_esp, uint32_t exit_esp, const char *what) __attribute__((noreturn));
#endif
