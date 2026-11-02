/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * T3869: host side of the Rust dispatch (see rust_dispatch.h). Compiled only with
 * -DTSFP_RUST_DISPATCH=ON (cmake/rust-dispatch.cmake). The guest register file stays C
 * thread-local here; Rust reaches it through the accessors below for the duration of one call.
 */
#include "rust_dispatch.h"

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "kernel_call.h"
#include "recomp_abi.h"

extern TSFP_RECOMP_TLS int g_fp_top;          /* lifted x87 model top (x87_replace.h) */
extern TSFP_RECOMP_TLS double g_fp_stack[8];  /* lifted x87 model registers */

/* crates/tsfp-dispatch-capi/src/lib.rs */
#define TSFP_DISPATCH_ABI_VERSION 1u
typedef struct {
    uint64_t rows;
    uint64_t checked;
    uint64_t mismatches;
    uint32_t first_mismatch;
} tsfp_dispatch_agreement;
typedef struct {
    void (*read_registers)(uint32_t *out);
    void (*write_registers)(const uint32_t *words);
    void (*read_fpu)(int32_t *top, uint16_t *control_word);
    void (*write_fpu)(int32_t top, uint16_t control_word);
    double (*fpu_register)(uint32_t index);
    bool (*write_guest)(uint32_t address, const uint8_t *bytes, size_t length);
} tsfp_dispatch_thread_host;
uint32_t tsfp_dispatch_abi_version(void);
int32_t tsfp_dispatch_install(const recomp_dispatch_row *rows, size_t count);
size_t tsfp_dispatch_count(void);
bool tsfp_dispatch_verify(recomp_func_t (*oracle)(uint32_t), tsfp_dispatch_agreement *report);
bool tsfp_dispatch_guest_call_stdcall(uint32_t address, const uint32_t *arguments, uint32_t count,
                                      const tsfp_dispatch_thread_host *host);
bool tsfp_dispatch_guest_call_cdecl(uint32_t address, const uint32_t *arguments, uint32_t count,
                                    const tsfp_dispatch_thread_host *host, uint32_t *result,
                                    double *st0);

/* Lifter patch 42. Weak: a lift generated before it links, and selection then refuses loudly. */
extern const void *recomp_dispatch_entries(size_t *count) __attribute__((weak));

static bool selected;

static void read_registers(uint32_t *out)
{
    out[0] = g_eax; out[1] = g_ecx; out[2] = g_edx; out[3] = g_ebx;
    out[4] = g_esi; out[5] = g_edi; out[6] = g_ebp; out[7] = g_esp;
}

static void write_registers(const uint32_t *words)
{
    g_eax = words[0]; g_ecx = words[1]; g_edx = words[2]; g_ebx = words[3];
    g_esi = words[4]; g_edi = words[5]; g_ebp = words[6]; g_esp = words[7];
}

static void read_fpu(int32_t *top, uint16_t *control_word)
{
    *top = g_fp_top;
    *control_word = g_fp_control_word;
}

static void write_fpu(int32_t top, uint16_t control_word)
{
    g_fp_top = top;
    g_fp_control_word = control_word;
}

static double fpu_register(uint32_t index) { return g_fp_stack[index & 7u]; }

static bool write_guest(uint32_t address, const uint8_t *bytes, size_t length)
{
    return kernel_guest_write_bytes(address, bytes, length);
}

static const tsfp_dispatch_thread_host thread_host = {
    .read_registers = read_registers,
    .write_registers = write_registers,
    .read_fpu = read_fpu,
    .write_fpu = write_fpu,
    .fpu_register = fpu_register,
    .write_guest = write_guest,
};

bool rust_dispatch_startup(void)
{
    const char *value = getenv("TSFP_RUST_DISPATCH");
    if (value == NULL || strcmp(value, "1") != 0) {
        fprintf(stderr, "rust dispatch: linked, not selected (C dispatcher; TSFP_RUST_DISPATCH=1 selects Rust)\n");
        return true;
    }
    if (tsfp_dispatch_abi_version() != TSFP_DISPATCH_ABI_VERSION) {
        fprintf(stderr, "rust dispatch: ABI %u, expected %u; refusing the run\n", tsfp_dispatch_abi_version(),
                TSFP_DISPATCH_ABI_VERSION);
        return false;
    }
    if (recomp_dispatch_entries == NULL) {
        fprintf(stderr, "rust dispatch: the lift has no recomp_dispatch_entries (lifter patch 42, T3869); "
                        "relift or unset TSFP_RUST_DISPATCH; refusing the run\n");
        return false;
    }
    size_t count = 0u;
    const recomp_dispatch_row *rows = (const recomp_dispatch_row *)recomp_dispatch_entries(&count);
    const int32_t installed = tsfp_dispatch_install(rows, count);
    if (installed != 0) {
        fprintf(stderr, "rust dispatch: install refused (%d) for %zu rows; refusing the run\n", installed, count);
        return false;
    }
    /* Rule 48: every row, both neighbours and the span edges, against the C oracle. */
    tsfp_dispatch_agreement report = {0};
    if (!tsfp_dispatch_verify(recomp_lookup, &report)) {
        fprintf(stderr, "rust dispatch: DISAGREES with C recomp_lookup: %llu of %llu probes over %llu rows, "
                        "first at 0x%08X; refusing the run\n",
                (unsigned long long)report.mismatches, (unsigned long long)report.checked,
                (unsigned long long)report.rows, report.first_mismatch);
        return false;
    }
    selected = true;
    fprintf(stderr, "rust dispatch: selected, %zu rows, %llu probes agree with C\n", tsfp_dispatch_count(),
            (unsigned long long)report.checked);
    return true;
}

bool rust_dispatch_selected(void) { return selected; }

bool rust_dispatch_guest_call_stdcall(uint32_t address, const uint32_t *arguments, unsigned count)
{
    return tsfp_dispatch_guest_call_stdcall(address, arguments, count, &thread_host);
}

bool rust_dispatch_guest_call_cdecl(uint32_t address, const uint32_t *arguments, unsigned count,
                                    uint32_t *result, double *st0)
{
    return tsfp_dispatch_guest_call_cdecl(address, arguments, count, &thread_host, result, st0);
}
