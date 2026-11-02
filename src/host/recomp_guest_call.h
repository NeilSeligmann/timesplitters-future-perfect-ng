/* SPDX-License-Identifier: GPL-3.0-or-later */
#ifndef TSFP_RECOMP_GUEST_CALL_H
#define TSFP_RECOMP_GUEST_CALL_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
/* T392: call a lifted guest stdcall function from host code on the CALLING guest thread's own
 * stack, as a nested call from inside an HLE handler (for example the XMV audio completion
 * callback run from DirectSoundDoWork). Not an interrupt or DPC emulation: the frame goes just
 * below the current guest stack pointer and the integer registers are restored afterwards.
 *
 * Returns false and leaves every register as it was when the function does not exist, the
 * frame is not writable stack, or the callee did not pop exactly its frame (a wrong argument
 * count). A guest stop or fault inside the callee propagates through host_run_stop as usual.
 * At most GUEST_CALL_MAX_ARGS arguments. Registers outside eax..ebp are not saved: only an
 * integer callee is admitted, and FPU, MMX and SSE state is left to it. */
#define GUEST_CALL_MAX_ARGS 4u
typedef void (*recomp_guest_call_function)(void);
typedef struct {
    int32_t host_pid;
    recomp_guest_call_function (*lookup)(uint32_t address);
    bool (*read_registers)(uint32_t registers[8]);
    bool (*set_esp)(uint32_t esp);
    uint32_t (*get_esp)(void);
    bool (*restore_registers)(const uint32_t registers[8]);
} recomp_guest_call_host;
bool tsfp_audio_guest_call_stdcall_rust(uint32_t address, const uint32_t *arguments,
                                        uint32_t count, const recomp_guest_call_host *host);
bool recomp_guest_call_stdcall(uint32_t address, const uint32_t *arguments, unsigned count);
/* T2310: the same nested call for a cdecl callee (the callee pops only its return slot, the
 * caller pops the arguments). Used by the native Rust UI framework to reach the original page
 * handlers (UiBindings) from the frame hook on the guest thread. EAX is returned through
 * `result` (may be NULL). The x87 top and control word are restored too. */
#define GUEST_CALL_CDECL_MAX_ARGS 8u
bool recomp_guest_call_cdecl(uint32_t address, const uint32_t *arguments, unsigned count, uint32_t *result);
/* T2310: recomp_guest_call_cdecl for a callee that returns a float in ST0 (for example the text width
 * measure 0x405C0): `st0` receives the value the callee left on top of the lifted x87 stack, or NaN
 * when it left none. The x87 top is restored afterwards, so the value is popped. */
bool recomp_guest_call_cdecl_st0(uint32_t address, const uint32_t *arguments, unsigned count, double *st0);
#endif
