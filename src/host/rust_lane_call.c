/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1858: host side of the Rust lane bridge's nested guest calls. Separate from rust_ports.c because
 * every binary that links libtsfp_ports.a needs the symbol (the lane module references it), also
 * the ext-bridge tests that do not link the selector. */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE /* fopencookie (T2136) */
#endif
#include "frame_state_sync.h"
#include "../game/game_replace.h"
#include "../game/x87_replace.h"
#include "rust_ext_bridge.h"
#include "host_runtime.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#ifdef TSFP_RUST_PORTS
extern __thread uint32_t g_ebp, g_ebx, g_edi;
/* crates/tsfp-ports/src/lane.rs imports this for the 17 call-bearing lane roots. It does what the
 * harness shim of tools/replace/rust_lane.py does: run the lifted callee for `target` with the full
 * register file in and out (eax, ecx, edx, ebx, esi, edi, ebp, esp), ESP included because the
 * callee may move it. The Rust side has already written the call frame (return PC and arguments
 * below ESP). A target without a lifted function is a defect and traps like the shim. */
/* T2145: the bridge used to abort() on a missing callee or a bad exit ESP, with no output: the owner's perf run died that way.
 * Now it names the target and the stack, and ends the run through the normal stop report where the thread is armed. */
extern void host_run_stop(host_stop_reason reason, uint32_t guest_address, unsigned ordinal, const char *detail) __attribute__((weak));
static _Thread_local char lane_fail_detail[200];
extern void tsfp_rust_last_hit_callback(uint32_t *out) __attribute__((weak));
void tsfp_bridge_fail(int reason, uint32_t target, uint32_t entry_esp, uint32_t exit_esp, const char *what) __attribute__((noreturn));
void tsfp_bridge_fail(int reason, uint32_t target, uint32_t entry_esp, uint32_t exit_esp, const char *what)
{
    snprintf(lane_fail_detail, sizeof lane_fail_detail, "%s: target %#x, entry esp %#x, exit esp %#x", what, target, entry_esp, exit_esp);
    fprintf(stderr, "rust lane bridge: %s\n", lane_fail_detail);
    /* T2147: the collision hit record the last 0xC4A3F indirect call went through (type +0x20, flags +0x28, the words around +0x170). */
    uint32_t hit[5];
    if (tsfp_rust_last_hit_callback != NULL) {
        tsfp_rust_last_hit_callback(hit);
        fprintf(stderr, "rust lane bridge: last hit callback site %#x record %#x target %#x type %#x flags %#x\n", hit[0], hit[1], hit[2], hit[3], hit[4]);
        if (hit[1] >= 0x10000u && hit[1] < 0x80000000u) {
            fprintf(stderr, "rust lane bridge: record +0x160..+0x17c:");
            for (uint32_t offset = 0x160u; offset < 0x180u; offset += 4u) fprintf(stderr, " %08x", guest_read32(hit[1] + offset));
            fputc('\n', stderr);
        }
    }
    if (host_run_stop != NULL) host_run_stop((host_stop_reason)reason, target, 0u, lane_fail_detail);
    abort();
}
/* T3742: the capture-based Tier 2 offline runner (tsfp-bevy --t2-offline, crates/tsfp-t2-offline) sets this hook
 * around a port run so every nested guest call replays the captured callee instead of running it. Convention 1: the
 * caller already pushed the return PC (this function); 0: the bridge pushes it (tsfp_lane_x_call, the ext trampoline).
 * NULL (always, outside that mode) keeps the normal paths. */
void (*tsfp_t2o_call_hook)(uint32_t va, rust_ext_registers *regs, int convention);
bool tsfp_t2o_call_active(void) { return tsfp_t2o_call_hook != NULL; }
void tsfp_lane_x_fp_load(rust_ext_registers *regs);
void tsfp_lane_x_fp_store(const rust_ext_registers *regs);
void tsfp_lane_guest_call(uint32_t target, uint32_t *regs);
void tsfp_lane_guest_call(uint32_t target, uint32_t *regs)
{
    if (tsfp_t2o_call_hook != NULL) {
        rust_ext_registers x = {0};
        x.eax = regs[0]; x.ecx = regs[1]; x.edx = regs[2]; x.ebx = regs[3];
        x.esi = regs[4]; x.edi = regs[5]; x.ebp = regs[6]; x.esp = regs[7];
        tsfp_lane_x_fp_load(&x);
        tsfp_t2o_call_hook(target, &x, 1);
        tsfp_lane_x_fp_store(&x);
        regs[0] = x.eax; regs[1] = x.ecx; regs[2] = x.edx; regs[3] = x.ebx;
        regs[4] = x.esi; regs[5] = x.edi; regs[7] = x.esp;
        return;
    }
    game_guest_function function = recomp_lookup != NULL ? recomp_lookup(target) : NULL;
    if (function == NULL) tsfp_bridge_fail(HOST_STOP_ICALL_UNRESOLVED, target, regs[7], regs[7], "nested guest call (tsfp_lane_guest_call) has no lifted function");
    g_eax = regs[0]; g_ecx = regs[1]; g_edx = regs[2]; g_ebx = regs[3];
    g_esi = regs[4]; g_edi = regs[5]; g_ebp = regs[6]; g_esp = regs[7];
    function();
    regs[0] = g_eax; regs[1] = g_ecx; regs[2] = g_edx; regs[3] = g_ebx;
    regs[4] = g_esi; regs[5] = g_edi; regs[7] = g_esp;
    /* T2017: regs[6] (EBP) keeps the caller's value, same reason as tsfp_lane_x_call (T2009): the lifted code holds EBP
     * in a C local and `g_ebp` is only the published frame. EBX/ESI/EDI are `#define`d to g_ebx/g_esi/g_edi in the
     * lifted code (xboxrecomp recomp_types.h), so the callee's own pops restore them and copying them back is exact. */
}

/* T1874: the inventory and pose/control lane roots (crates/tsfp-ports/src/lane_inv.rs) run the
 * ExtRegisters ports, so besides the eight registers they need the x87 stack, XMM0-7 and DF, and
 * their callees run as lifted originals through `tsfp_lane_x_call` (the same contract as
 * `rust_ext_guest_call` in rust_ext_bridge.c, which is not linked by every test binary). */
typedef union { float f[4]; double d[2]; uint32_t u[4]; int32_t i[4]; uint64_t q[2]; } lane_x_xmm;
extern __thread lane_x_xmm g_xmm0, g_xmm1, g_xmm2, g_xmm3, g_xmm4, g_xmm5, g_xmm6, g_xmm7;
extern uint32_t g_xbox_code_lo, g_xbox_code_hi;
extern __thread int g_df;
void tsfp_lane_x_fp_load(rust_ext_registers *regs);
void tsfp_lane_x_fp_store(const rust_ext_registers *regs);
void tsfp_lane_x_call(uint32_t va, rust_ext_registers *regs);
extern game_guest_function recomp_lookup_manual(uint32_t xbox_va) __attribute__((weak));
extern game_guest_function recomp_lookup_kernel(uint32_t xbox_va) __attribute__((weak));
/* T1990: return address the next tsfp_lane_x_call leaves in its call frame (0 when unset). The audio translations
 * set it before each guest call: the DirectSound buffer methods check their caller. */
static __thread uint32_t lane_return_pc;
void tsfp_lane_set_return(uint32_t pc);
void tsfp_lane_set_return(uint32_t pc) { lane_return_pc = pc; }
extern void tsfp_weapon_transfer_diagnostic_record(const uint32_t *input,
                                                   uint32_t input_words) __attribute__((weak));

static uint32_t weapon_transfer_s16(uint32_t address)
{
    const uint32_t shift = (address & 2u) * 8u;
    const int32_t value = (int16_t)((guest_read32(address & ~3u) >> shift) & 0xffffu);
    return (uint32_t)value;
}

static uint32_t weapon_transfer_channel_word(uint32_t base, uint32_t ammo, uint32_t offset)
{
    if (base < 0x10000u || base >= 0x80000000u || ammo >= 44u) return 0u;
    return weapon_transfer_s16(base + offset + ammo * 2u);
}

typedef struct { uint32_t words[32]; } weapon_transfer_capture;

static void weapon_transfer_begin_lifted(uint32_t entry_esp, uint32_t hand,
                                         weapon_transfer_capture *capture)
{
    memset(capture, 0, sizeof *capture);
    uint32_t *event = capture->words;
    const uint32_t ext = guest_read32(entry_esp + 4u);
    const uint32_t controller = guest_read32(entry_esp + 8u);
    const uint32_t saved = guest_read32(entry_esp + 12u) != 0u;
    const bool valid_ext = ext >= 0x10000u && ext < 0x80000000u;
    const bool valid_hand = hand >= 0x10000u && hand < 0x80000000u;
    const uint32_t slot = valid_ext ? guest_read32(ext + 8u) : UINT32_MAX;
    const uint32_t side = valid_ext && hand == ext + 0xa4u ? 0u :
                          valid_ext && hand == ext + 0x1d4u ? 1u : UINT32_MAX;
    const uint32_t record_id = valid_hand ? guest_read32(hand + 0x30u) : UINT32_MAX;
    const uint32_t record_table = guest_read32(0x4f9bacu);
    uint32_t primary = UINT32_MAX, secondary = UINT32_MAX;
    uint32_t primary_capacity = 0u, secondary_capacity = 0u;
    uint32_t primary_single_round = 0u;
    if (record_id < 0x42u && record_table >= 0x10000u && record_table < 0x80000000u) {
        const uint32_t row = record_table + record_id * 0x29cu;
        const uint32_t primary_raw = guest_read32(row + 0x14u);
        const uint32_t secondary_raw = guest_read32(row + 0x1cu);
        if (primary_raw > 0u && primary_raw < 44u) primary = primary_raw;
        if (secondary_raw > 0u && secondary_raw < 44u) secondary = secondary_raw;
        primary_capacity = guest_read32(row + 0x18u);
        secondary_capacity = guest_read32(row + 0x20u);
        primary_single_round = (guest_read32(row + 0x0cu) & 0x2000u) != 0u;
    }
    const uint32_t controller_state = controller != 0u && controller < 0x80000000u
                                          ? guest_read32(controller + 0x44u)
                                          : UINT32_MAX;
    const uint32_t restrict_primary = controller_state == 7u && primary_single_round;
    event[2] = 0x1b6ec0u; /* logical root; the replay switch identifies the implementation */
    event[3] = 0u;
    event[4] = ext;
    event[5] = slot;
    event[6] = hand;
    event[7] = side;
    event[8] = record_id;
    event[9] = primary;
    event[10] = secondary;
    event[11] = primary_capacity;
    event[12] = secondary_capacity;
    event[13] = saved;
    event[14] = restrict_primary;
    event[15] = controller_state;
    event[16] = weapon_transfer_channel_word(hand, primary, 0x38u);
    event[18] = weapon_transfer_channel_word(hand, secondary, 0x38u);
    event[20] = weapon_transfer_channel_word(ext, primary, 0x61eu);
    event[22] = weapon_transfer_channel_word(ext, secondary, 0x61eu);
    event[27] = valid_hand ? guest_read32(hand + 0xe8u) : UINT32_MAX;
    event[29] = valid_hand ? guest_read32(hand + 0xf8u) : 0u;
}

static void weapon_transfer_finish_lifted(weapon_transfer_capture *capture, uint32_t eax)
{
    if (tsfp_weapon_transfer_diagnostic_record == NULL) return;
    uint32_t *event = capture->words;
    const uint32_t ext = event[4];
    const uint32_t hand = event[6];
    const uint32_t primary = event[9];
    const uint32_t secondary = event[10];
    event[17] = weapon_transfer_channel_word(hand, primary, 0x38u);
    event[19] = weapon_transfer_channel_word(hand, secondary, 0x38u);
    event[21] = weapon_transfer_channel_word(ext, primary, 0x61eu);
    event[23] = weapon_transfer_channel_word(ext, secondary, 0x61eu);
    event[24] = eax;
    event[25] = eax;
    event[26] = 1u; /* lifted call returned */
    event[28] = hand >= 0x10000u && hand < 0x80000000u
                    ? guest_read32(hand + 0xe8u) : UINT32_MAX;
    event[30] = hand >= 0x10000u && hand < 0x80000000u
                    ? guest_read32(hand + 0xf8u) : 0u;
    tsfp_weapon_transfer_diagnostic_record(event, 32u);
}
void tsfp_lane_x_fp_load(rust_ext_registers *r)
{
    r->eflags = g_df ? 0x400u : 0u; /* DF steers the rep string forms of the ports */
    memcpy(r->fpu.stack, g_fp_stack, sizeof g_fp_stack);
    r->fpu.top = (uint32_t)g_fp_top & 7u; r->fpu.control = g_fp_control_word;
    memcpy(&r->xmm[0], &g_xmm0, 16); memcpy(&r->xmm[1], &g_xmm1, 16);
    memcpy(&r->xmm[2], &g_xmm2, 16); memcpy(&r->xmm[3], &g_xmm3, 16);
    memcpy(&r->xmm[4], &g_xmm4, 16);
    memcpy(&r->xmm_hi[0], &g_xmm5, 16); memcpy(&r->xmm_hi[1], &g_xmm6, 16);
    memcpy(&r->xmm_hi[2], &g_xmm7, 16);
}
void tsfp_lane_x_fp_store(const rust_ext_registers *r)
{
    memcpy(g_fp_stack, r->fpu.stack, sizeof g_fp_stack);
    g_fp_top = (int)r->fpu.top; g_fp_control_word = r->fpu.control;
    memcpy(&g_xmm0, &r->xmm[0], 16); memcpy(&g_xmm1, &r->xmm[1], 16);
    memcpy(&g_xmm2, &r->xmm[2], 16); memcpy(&g_xmm3, &r->xmm[3], 16);
    memcpy(&g_xmm4, &r->xmm[4], 16);
    memcpy(&g_xmm5, &r->xmm_hi[0], 16); memcpy(&g_xmm6, &r->xmm_hi[1], 16);
    memcpy(&g_xmm7, &r->xmm_hi[2], 16);
}
void tsfp_lane_x_call(uint32_t va, rust_ext_registers *r)
{
    /* Lifted RECOMP_ICALL not-code path (indirect hooks): the callee does not run, EAX becomes 0. */
    if (!(va >= 0xFE000000u || g_xbox_code_hi == 0u || (va >= g_xbox_code_lo && va < g_xbox_code_hi))) {
        r->eax = 0u;
        return;
    }
    if (tsfp_t2o_call_hook != NULL) { tsfp_t2o_call_hook(va, r, 0); return; } /* T3742 offline runner */
    /* T1990: the lifted RECOMP_ICALL order: manual handlers first (the DirectSound stream methods and the
     * function census live there), then the translated function, then the kernel bridge. */
    game_guest_function function = recomp_lookup_manual != NULL ? recomp_lookup_manual(va) : NULL;
    if (function == NULL && recomp_lookup != NULL) function = recomp_lookup(va);
    if (function == NULL && recomp_lookup_kernel != NULL) function = recomp_lookup_kernel(va);
    if (function == NULL) tsfp_bridge_fail(HOST_STOP_ICALL_UNRESOLVED, va, r->esp, r->esp, "nested guest call (tsfp_lane_x_call) has no lifted, manual or kernel function");
    g_eax = r->eax; g_ecx = r->ecx; g_edx = r->edx; g_esp = r->esp;
    g_ebx = r->ebx; g_ebp = r->ebp; g_esi = r->esi; g_edi = r->edi;
    tsfp_lane_x_fp_store(r);
    g_esp -= 4u; /* return slot, the lifted RET does esp += 4 */
    guest_write32(g_esp, lane_return_pc);
    lane_return_pc = 0u;
    const uint32_t entry_esp = g_esp;
    const uint32_t chkstk_probe = g_eax;
    weapon_transfer_capture transfer_capture;
    const bool capture_transfer = va == 0x1b6ec0u;
    if (capture_transfer) weapon_transfer_begin_lifted(entry_esp, g_eax, &transfer_capture);
    TSFP_FRAME_SYNC_OUT();
    function();
    TSFP_FRAME_SYNC_IN();
    if (capture_transfer) weapon_transfer_finish_lifted(&transfer_capture, g_eax);
    /* T2314: _chkstk 0x3C9D90 allocates EAX bytes of the caller's frame, so its exit ESP is entry + 4 - EAX (the c2r
     * caller follows the original stream and expects that). Measured: 0x23A040 (frame 0x19A4) stopped the Arcade
     * pause/resume route with entry 0x4100FDA4, exit 0x4100E404. Same allowance as tests/c/c2r_bridge.c (T2027). */
    const bool chkstk_ok = va == 0x3C9D90u && g_esp == entry_esp + 4u - chkstk_probe;
    /* T1990: a stdcall callee (the DirectSound methods, `ret N`) also pops its arguments, so the exit ESP is entry + 4 + 4N
     * and the translated caller, which follows the original instruction stream, does not adjust it again. */
    if (!chkstk_ok && (g_esp < entry_esp + 4u || (g_esp - entry_esp) % 4u != 0u || g_esp - entry_esp > 4u + 64u))
        tsfp_bridge_fail(HOST_STOP_BRIDGE_CONTRACT, va, entry_esp, g_esp, "callee left ESP outside entry+4 .. entry+68 (dwords)");
    r->eax = g_eax; r->ecx = g_ecx; r->edx = g_edx; r->esp = g_esp;
    /* T2009: EBP is callee-saved and the lifted code keeps it in a C local, `g_ebp` is only the published frame
     * (`g_ebp = ebp` on pop, `g_seh_ebp` for FPO leaves), so its exit value is not the caller's EBP. Measured: 0x15D470
     * returned g_ebp 0x4172FE28 for entry 1, the 0x2AEFD0 translation then indexed 0x520A48 with it and faulted. */
    /* T2017: EBX/ESI/EDI are the globals themselves in lifted code (`#define ebx g_ebx`), restored by the callee's pops. */
    r->ebx = g_ebx; r->esi = g_esi; r->edi = g_edi;
    tsfp_lane_x_fp_load(r);
}

/* T2024 slice 2: opt-in render call trace (crates/tsfp-ports/src/render_trace.rs). Off unless the env var TSFP_RENDER_TRACE
 * names a file; TSFP_RENDER_TRACE_FRAMES=N closes the file after N game presents (src/gpu/d3d8_present.c record_frame calls
 * tsfp_render_trace_present, weak there). When off the lane pays one call that returns 0. */
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include "../gpu/d3d8_guest.h"
/* T2006: weak so the standalone c2rust differential harness (tests/c, no GPU library) still links. Only the render trace calls it. */
uint32_t d3d8_guest_load32(uint32_t address) __attribute__((weak));
void tsfp_render_trace_reset_dedupe(void) __attribute__((weak));
void tsfp_render_trace_set_legacy_textures(uint32_t on) __attribute__((weak));
int tsfp_render_trace_enabled(void);
void tsfp_render_trace_line(const char *text, size_t length);
void tsfp_render_trace_present(uint64_t number);
static FILE *render_trace_file;
/* T2136d: guest thread cost accounting (TSFP_RT_HOST_STATS=1), kept in crates/tsfp-ports/src/render_trace.rs. The C hooks below
 * report their own time as kinds 8 (lib), 9 (constants), 10 (surface), 11 (present). */
#include <time.h>
void tsfp_render_trace_stat_add(uint32_t kind, uint64_t nanoseconds, uint64_t bytes) __attribute__((weak));
void tsfp_render_trace_stat_frame(void) __attribute__((weak));
void tsfp_render_trace_stat_print(void) __attribute__((weak));
uint64_t tsfp_render_trace_now_ns(void);
uint64_t tsfp_render_trace_now_ns(void)
{
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (uint64_t)now.tv_sec * 1000000000u + (uint64_t)now.tv_nsec;
}
int tsfp_render_trace_stats_enabled(void);
int tsfp_render_trace_stats_enabled(void)
{
    static int flag = -1;
    if (flag < 0) {
        const char *stats = getenv("TSFP_RT_HOST_STATS");
        flag = stats != NULL && stats[0] != '\0' && stats[0] != '0' && tsfp_render_trace_stat_add != NULL;
    }
    return flag;
}
void tsfp_render_trace_stderr(const char *text, size_t length);
void tsfp_render_trace_stderr(const char *text, size_t length) { fwrite(text, 1u, length, stderr); }
/* T2136d: binary events. TSFP_RT_HOST_BINARY=1 with the in-process rt-host sink and no TSFP_RENDER_TRACE tee file: the lane
 * (crates/tsfp-ports/src/render_trace.rs) writes trace_wire records into a buffer and hands it over once per present through
 * tsfp_render_trace_binary_send; the C hooks below call the Rust encoders instead of formatting text. The text form stays the
 * default, a skip point still writes its constant shadow as text (the rt-host keeps the order of mixed text and binary). */
static int render_trace_binary_on;
static int render_trace_dual; /* TSFP_RT_HOST_BINARY=dual: text and binary of the same run (test mode, needs TSFP_RENDER_TRACE) */
/* T2234: opt-in only; explicit text/dual/dump capture retains every legacy diagnostic record. */
void tsfp_render_trace_fast_lib(uint32_t method, uint32_t value) __attribute__((weak));
int tsfp_render_trace_fast_hooks(void);
int tsfp_render_trace_fast_hooks(void)
{
    static int flag = -1;
    if (flag < 0) {
        const char *fast = getenv("TSFP_RT_HOST_FAST_HOOKS");
        const char *trace = getenv("TSFP_RENDER_TRACE");
        const char *dump = getenv("TSFP_RT_HOST_BINARY_DUMP");
        flag = fast != NULL && fast[0] == '1' && render_trace_binary_on && !render_trace_dual &&
            !(trace != NULL && trace[0] != '\0') && !(dump != NULL && dump[0] != '\0');
    }
    return flag;
}
void tsfp_render_trace_bin_present(uint64_t number) __attribute__((weak));
void tsfp_render_trace_bin_surface(const uint32_t *words) __attribute__((weak));
void tsfp_render_trace_bin_lib(uint32_t method, uint32_t value) __attribute__((weak));
void tsfp_render_trace_bin_constants(uint32_t index, const uint32_t *words, uint32_t count) __attribute__((weak));
/* T2136d: the renderer is behind, the lane omits draws (render_trace::dropping). Latched at each present from the rt-host. */
static int render_trace_dropping_now;
void tsfp_render_trace_bin_frame(uint32_t flags) __attribute__((weak));
int tsfp_render_trace_dropping(void);
int tsfp_render_trace_dropping(void) { return render_trace_dropping_now; }
/* T2136d: texel bytes the lane may snapshot per frame in the binary form (render_trace.rs, texel_budget_exceeded). 0: unlimited. */
uint32_t tsfp_render_trace_texel_budget(void);
uint32_t tsfp_render_trace_texel_budget(void)
{
    const char *megabytes = getenv("TSFP_RT_HOST_TEXEL_BUDGET_MB");
    return megabytes != NULL ? (uint32_t)(strtoul(megabytes, NULL, 10) << 20) : (8u << 20);
}
int tsfp_render_trace_binary_active(void);
int tsfp_render_trace_binary_active(void) { return render_trace_binary_on ? (render_trace_dual ? 2 : 1) : 0; }
/* T2024: the binary event above is the native RenderDevice implementation for Rust-originated
 * PushMethod calls. Keep the C D3D8 HLE as the default and as the text/dual oracle path. */
int tsfp_render_trace_native_push_method(void);
int tsfp_render_trace_native_push_method(void)
{
    static int requested = -1;
    static int announced;
    if (requested < 0) {
        const char *value = getenv("TSFP_RENDER_NATIVE_PUSH_METHOD");
        requested = value != NULL && strcmp(value, "1") == 0;
    }
    const int active = requested && render_trace_binary_on && !render_trace_dual;
    if (requested && !announced) {
        if (active)
            fputs("wgpu-live: native RenderDevice PushMethod enabled (C 0x3D6C90 bypassed)\n", stderr);
        else
            fputs("wgpu-live: TSFP_RENDER_NATIVE_PUSH_METHOD requires the binary non-dual renderer path; keeping C 0x3D6C90\n", stderr);
        announced = 1;
    }
    return active;
}
/* T2024: the native vertex-array operation consumes typed Rust state. Keep its opt-in independent
 * from PushMethod so the raw packet replacement can be validated on its own. */
int tsfp_render_trace_native_vertex_bindings(void);
int tsfp_render_trace_native_vertex_bindings(void)
{
    static int requested = -1;
    static int announced;
    if (requested < 0) {
        const char *value = getenv("TSFP_RENDER_NATIVE_VERTEX_BINDINGS");
        requested = value != NULL && strcmp(value, "1") == 0;
    }
    const int active = requested && render_trace_binary_on && !render_trace_dual;
    if (requested && !announced) {
        if (active)
            fputs("wgpu-live: native RenderDevice vertex bindings enabled (raw 0x1EB00 packet bypassed)\n", stderr);
        else
            fputs("wgpu-live: TSFP_RENDER_NATIVE_VERTEX_BINDINGS requires the binary non-dual renderer path; keeping raw 0x1EB00 packet\n", stderr);
        announced = 1;
    }
    return active;
}
/* T2024: default-off typed state-handler selectors retain C as the oracle. */
int tsfp_render_trace_native_state_handler(uint32_t index);
int tsfp_render_trace_native_state_handler(uint32_t index)
{
    static int requested_8a = -1;
    static int requested_89 = -1;
    static int requested_8b = -1;
    static int requested_8c = -1;
    static int requested_8d = -1;
    static int requested_8e = -1;
    static int requested_96 = -1;
    static int requested_95 = -1;
    static int requested_97 = -1;
    static int requested_9c = -1;
    static int requested_9d = -1;
    static int requested_9e = -1;
    static int requested_99 = -1;
    static int requested_91 = -1;
    static int requested_90 = -1;
    static int requested_93 = -1;
    static int requested_94 = -1;
    static int requested_a3 = -1;
    static int requested_a4 = -1;
    static int requested_a2 = -1;
    static int requested_a1 = -1;
    static int requested_a0 = -1;
    static int requested_a5 = -1;
    static int announced_8a;
    static int announced_89;
    static int announced_8b;
    static int announced_8c;
    static int announced_8d;
    static int announced_8e;
    static int announced_96;
    static int announced_95;
    static int announced_97;
    static int announced_9c;
    static int announced_9d;
    static int announced_9e;
    static int announced_99;
    static int announced_91;
    static int announced_90;
    static int announced_93;
    static int announced_94;
    static int announced_a3;
    static int announced_a4;
    static int announced_a2;
    static int announced_a1;
    static int announced_a0;
    static int announced_a5;
    static int requested_92 = -1;
    static int announced_92;
    const int is_8a = index == 0x8Au;
    const int is_8c = index == 0x8Cu;
    int *requested;
    int *announced;
    const char *switch_name;
    const char *entry;
    switch (index) {
    case 0x89u:
        requested = &requested_89; announced = &announced_89;
        switch_name = "TSFP_RENDER_NATIVE_STATE_89"; entry = "0x3D74B0"; break;
    case 0x8Au:
        requested = &requested_8a; announced = &announced_8a;
        switch_name = "TSFP_RENDER_NATIVE_STATE_8A"; entry = "0x3D7010"; break;
    case 0x8Bu:
        requested = &requested_8b; announced = &announced_8b;
        switch_name = "TSFP_RENDER_NATIVE_STATE_8B"; entry = "0x3D7380"; break;
    case 0x8Cu:
        requested = &requested_8c; announced = &announced_8c;
        switch_name = "TSFP_RENDER_NATIVE_STATE_8C"; entry = "0x3D73D0"; break;
    case 0x8Du:
        requested = &requested_8d; announced = &announced_8d;
        switch_name = "TSFP_RENDER_NATIVE_STATE_8D"; entry = "0x3D7430"; break;
    case 0x8Eu:
        requested = &requested_8e; announced = &announced_8e;
        switch_name = "TSFP_RENDER_NATIVE_STATE_8E"; entry = "0x3D7110"; break;
    case 0x91u:
        requested = &requested_91; announced = &announced_91;
        switch_name = "TSFP_RENDER_NATIVE_STATE_91"; entry = "0x3D8010"; break;
    case 0x90u:
        requested = &requested_90; announced = &announced_90;
        switch_name = "TSFP_RENDER_NATIVE_STATE_90"; entry = "0x3D7F70"; break;
    case 0x93u:
        requested = &requested_93; announced = &announced_93;
        switch_name = "TSFP_RENDER_NATIVE_STATE_93"; entry = "0x3D7060"; break;
    case 0x92u:
        requested = &requested_92; announced = &announced_92;
        switch_name = "TSFP_RENDER_NATIVE_STATE_92"; entry = "0x3D70D0"; break;
    case 0x94u:
        requested = &requested_94; announced = &announced_94;
        switch_name = "TSFP_RENDER_NATIVE_STATE_94"; entry = "0x3D7150"; break;
    case 0xA3u:
        requested = &requested_a3; announced = &announced_a3;
        switch_name = "TSFP_RENDER_NATIVE_STATE_A3"; entry = "0x3D8190"; break;
    case 0xA4u:
        requested = &requested_a4; announced = &announced_a4;
        switch_name = "TSFP_RENDER_NATIVE_STATE_A4"; entry = "0x3D81B0"; break;
    case 0xA2u:
        requested = &requested_a2; announced = &announced_a2;
        switch_name = "TSFP_RENDER_NATIVE_STATE_A2"; entry = "0x3D8120"; break;
    case 0xA1u:
        requested = &requested_a1; announced = &announced_a1;
        switch_name = "TSFP_RENDER_NATIVE_STATE_A1"; entry = "0x3D80B0"; break;
    case 0xA0u:
        requested = &requested_a0; announced = &announced_a0;
        switch_name = "TSFP_RENDER_NATIVE_STATE_A0"; entry = "0x3D8080"; break;
    case 0xA5u:
        requested = &requested_a5; announced = &announced_a5;
        switch_name = "TSFP_RENDER_NATIVE_STATE_A5"; entry = "0x3D81D0"; break;
    case 0x96u:
        requested = &requested_96; announced = &announced_96;
        switch_name = "TSFP_RENDER_NATIVE_STATE_96"; entry = "0x3D7320"; break;
    case 0x95u:
        requested = &requested_95; announced = &announced_95;
        switch_name = "TSFP_RENDER_NATIVE_STATE_95"; entry = "0x3D72A0"; break;
    case 0x97u:
        requested = &requested_97; announced = &announced_97;
        switch_name = "TSFP_RENDER_NATIVE_STATE_97"; entry = "0x3D6F90"; break;
    case 0x99u:
        requested = &requested_99; announced = &announced_99;
        switch_name = "TSFP_RENDER_NATIVE_STATE_99"; entry = "0x3D82D0"; break;
    case 0x9Cu:
        requested = &requested_9c; announced = &announced_9c;
        switch_name = "TSFP_RENDER_NATIVE_STATE_9C"; entry = "0x3D6FD0"; break;
    case 0x9Du:
        requested = &requested_9d; announced = &announced_9d;
        switch_name = "TSFP_RENDER_NATIVE_STATE_9D"; entry = "0x3D71B0"; break;
    case 0x9Eu:
        requested = &requested_9e; announced = &announced_9e;
        switch_name = "TSFP_RENDER_NATIVE_STATE_9E"; entry = "0x3D8320"; break;
    default: return 0;
    }
    if (*requested < 0) {
        const char *value = getenv(switch_name);
        *requested = value != NULL && strcmp(value, "1") == 0;
    }
    const int active = *requested && render_trace_binary_on && !render_trace_dual;
    if (*requested && !*announced) {
        if (active && is_8a)
            fputs("wgpu-live: native RenderDevice state 0x8A fog colour enabled (C 0x3D7010 bypassed)\n", stderr);
        else if (active && is_8c)
            fputs("wgpu-live: native RenderDevice state 0x8C back fill enabled (C 0x3D73D0 bypassed)\n", stderr);
        else if (active)
            fprintf(stderr, "wgpu-live: native RenderDevice state 0x%02X enabled (C %s bypassed)\n", index, entry);
        else
            fprintf(stderr, "wgpu-live: %s requires the binary non-dual renderer path; keeping C %s\n", switch_name, entry);
        *announced = 1;
    }
    return active;
}
#define STAT_BEGIN() (tsfp_render_trace_stats_enabled() ? tsfp_render_trace_now_ns() : 0u)
#define STAT_END(kind, started, bytes) \
    do { if ((started) != 0u) tsfp_render_trace_stat_add((kind), tsfp_render_trace_now_ns() - (started), (bytes)); } while (0)
/* T2024 slice 7: opt-in live renderer. TSFP_WGPU_LIVE=1 pipes the same trace lines into the stdin of the live renderer process
 * (crates/tsfp-wgpu/examples/live.rs, path in TSFP_WGPU_LIVE_BIN, extra arguments in TSFP_WGPU_LIVE_ARGS, for example
 * "--png-base DIR/live --dump-at 300"). With TSFP_RENDER_TRACE also set the lines go to both (tee), so one run gives the file for
 * the offline replay and the live frames. Unset: nothing changes. */
static int render_trace_piped;
#ifdef TSFP_RUST_RT_HOST
/* T2136: -DTSFP_RUST_RT_HOST=ON. The same trace lines go to the in-process render runtime (crates/tsfp-rt-host, a render thread
 * owning the wgpu LiveRenderer behind a bounded queue) instead of a popen'd subprocess. The stdio call sites stay as they are:
 * the sink is a FILE built with fopencookie whose write callback hands the bytes to the runtime (and tees them into
 * TSFP_RENDER_TRACE when that is also set). TSFP_RT_HOST_PIPE=1 keeps the pipe path in the same binary for A/B. */
#include <stdio.h>
struct tsfp_rt_host;
struct tsfp_rt_host *tsfp_rt_host_create(const char *args);
long tsfp_rt_host_write(struct tsfp_rt_host *runtime, const unsigned char *bytes, size_t length);
void tsfp_rt_host_destroy(struct tsfp_rt_host *runtime);
int tsfp_rt_host_alive(struct tsfp_rt_host *runtime);
int tsfp_rt_host_pressure(struct tsfp_rt_host *runtime);
long tsfp_rt_host_send_binary(struct tsfp_rt_host *runtime, const unsigned char *bytes, size_t length, unsigned presents);
static struct tsfp_rt_host *render_trace_binary_runtime;
#ifdef TSFP_RUST_RT_HOST
unsigned long long tsfp_rt_host_frames_behind(struct tsfp_rt_host *runtime);
#endif
/* T2292: frames the live rt-host renderer still has to replay, 0 without one (render_rate.c sheds replay frames on it) */
unsigned long long rust_lane_render_backlog(void)
{
#ifdef TSFP_RUST_RT_HOST
    if (render_trace_binary_runtime != NULL) return tsfp_rt_host_frames_behind(render_trace_binary_runtime);
#endif
    return 0;
}
static FILE *render_trace_binary_dump; /* TSFP_RT_HOST_BINARY_DUMP=<file>: the records as sent, for tools and tests (binary_check) */
struct render_trace_cookie { struct tsfp_rt_host *runtime; FILE *tee; };
static FILE *render_trace_rt_file;
static struct render_trace_cookie *render_trace_rt_cookie;
static ssize_t render_trace_cookie_write(void *cookie, const char *buffer, size_t size)
{
    struct render_trace_cookie *state = cookie;
    if (state->tee != NULL) fwrite(buffer, 1u, size, state->tee);
    tsfp_rt_host_write(state->runtime, (const unsigned char *)buffer, size);
    return (ssize_t)size; /* the runtime never fails the guest: past its stall limit it drops the renderer, not the game */
}
static int render_trace_cookie_close(void *cookie)
{
    struct render_trace_cookie *state = cookie;
    tsfp_rt_host_destroy(state->runtime); /* waits for the render thread (PNG files) */
    if (state->tee != NULL) fclose(state->tee);
    free(state);
    return 0;
}
static void render_trace_rt_atexit(void)
{
    FILE *file = render_trace_rt_file;
    render_trace_rt_file = NULL;
    if (file != NULL) fclose(file);
}
static FILE *render_trace_open_rt(const char *path)
{
    struct render_trace_cookie *state = calloc(1u, sizeof(*state));
    if (state == NULL) return NULL;
    const char *extra = getenv("TSFP_WGPU_LIVE_ARGS");
    state->runtime = tsfp_rt_host_create(extra != NULL ? extra : "");
    if (state->runtime == NULL) { free(state); return NULL; }
    if (path != NULL && path[0] != '\0') state->tee = fopen(path, "w");
    const char *binary = getenv("TSFP_RT_HOST_BINARY");
    if (binary != NULL && binary[0] == 'd' && state->tee != NULL && tsfp_render_trace_bin_present != NULL) {
        render_trace_binary_runtime = state->runtime;
        render_trace_binary_on = 1;
        render_trace_dual = 1;
        const char *dump = getenv("TSFP_RT_HOST_BINARY_DUMP");
        if (dump != NULL && dump[0] != '\0') render_trace_binary_dump = fopen(dump, "wb");
    } else if (binary != NULL && binary[0] == 'd') {
        fputs("wgpu-live: TSFP_RT_HOST_BINARY=dual needs TSFP_RENDER_TRACE (the text file) and a Rust lane with the binary encoder\n", stderr);
    } else if (binary == NULL || binary[0] != '0') {
        /* T2136d: the binary form is the DEFAULT of the in-process runtime (the guest copies raw records, it formats nothing).
         * TSFP_RT_HOST_BINARY=0 or a TSFP_RENDER_TRACE text file asks for the text form. */
        if (state->tee != NULL) {
            if (binary != NULL) fputs("wgpu-live: TSFP_RT_HOST_BINARY ignored, TSFP_RENDER_TRACE asks for the text form\n", stderr);
        } else if (tsfp_render_trace_bin_present == NULL)
            fputs("wgpu-live: binary events unavailable, the Rust lane has no binary encoder, using the text form\n", stderr);
        else {
            render_trace_binary_runtime = state->runtime;
            render_trace_binary_on = 1;
            const char *dump = getenv("TSFP_RT_HOST_BINARY_DUMP");
            if (dump != NULL && dump[0] != '\0') render_trace_binary_dump = fopen(dump, "wb");
        }
    }
    cookie_io_functions_t io = { NULL, render_trace_cookie_write, NULL, render_trace_cookie_close };
    FILE *file = fopencookie(state, "w", io);
    if (file == NULL) { render_trace_binary_on = 0; render_trace_binary_runtime = NULL; render_trace_cookie_close(state); return NULL; }
    render_trace_rt_file = file;
    render_trace_rt_cookie = state;
    atexit(render_trace_rt_atexit);
    return file;
}
#else
/* T2305: without TSFP_RUST_RT_HOST there is no live renderer, so no backlog (render_rate.c still links the call). */
unsigned long long rust_lane_render_backlog(void) { return 0; }
#endif
/* T2136d: the live path never runs degraded. When the renderer cannot start (no adapter, helper missing) render_trace_open returns
 * NULL, tsfp_render_trace_enabled stays 0 and every lane hook is one call that returns 0: nothing is formatted, no pipe is
 * written. The same line is printed when a started renderer dies later (render_trace_disable_live). */
static void render_trace_disabled_line(const char *why)
{
    fprintf(stderr, "wgpu-live: DISABLED: %s. The game continues on the C Vulkan path, the render trace is OFF and nothing is formatted.\n",
            why);
    fflush(stderr);
}
#include <errno.h>
#include <limits.h>
/* The pipe path needs the helper process (crates/tsfp-wgpu/examples/live.rs): TSFP_WGPU_LIVE_BIN, else next to the host binary,
 * else on PATH. Returns 0 and fills `out` when an executable file was found. */
static int render_trace_find_helper(char *out, size_t size)
{
    const char *bin = getenv("TSFP_WGPU_LIVE_BIN");
    if (bin != NULL && bin[0] != '\0') {
        if (access(bin, X_OK) != 0) return -1;
        snprintf(out, size, "%s", bin);
        return 0;
    }
    char self[PATH_MAX];
    ssize_t length = readlink("/proc/self/exe", self, sizeof(self) - 1u);
    if (length > 0) {
        self[length] = '\0';
        char *slash = strrchr(self, '/');
        if (slash != NULL) {
            *slash = '\0';
            snprintf(out, size, "%s/tsfp-wgpu-live", self);
            if (access(out, X_OK) == 0) return 0;
        }
    }
    const char *path = getenv("PATH");
    while (path != NULL && *path != '\0') {
        const char *end = strchr(path, ':');
        size_t part = end != NULL ? (size_t)(end - path) : strlen(path);
        snprintf(out, size, "%.*s/tsfp-wgpu-live", (int)part, path);
        if (part != 0u && access(out, X_OK) == 0) return 0;
        path = end != NULL ? end + 1 : NULL;
    }
    return -1;
}
/* T2136d: say which render path TSFP_WGPU_LIVE selected (the owner could not tell the in-process runtime from the legacy pipe,
 * whose blocking write slows the guest about 3x). One line at start (a constructor) and one when the trace opens. The lines
 * start with "wgpu-live:" so tools.play echoes them even without --console. */
#ifndef TSFP_GIT_SHA
#define TSFP_GIT_SHA "unknown"
#endif
#ifdef TSFP_RUST_RT_HOST
#define RENDER_PATH_BUILD "rt-host ON"
#else
#define RENDER_PATH_BUILD "rt-host OFF"
#endif
static void render_path_banner(const char *when, const char *state)
{
    fprintf(stderr, "wgpu-live: %s %s | build: %s, sha %s, %s %s\n", when, state, RENDER_PATH_BUILD, TSFP_GIT_SHA, __DATE__, __TIME__);
    fflush(stderr);
}
static const char *render_path_state(void)
{
    const char *live = getenv("TSFP_WGPU_LIVE");
    if (live == NULL || live[0] != '1') return "OFF (TSFP_WGPU_LIVE is not 1)";
#ifdef TSFP_RUST_RT_HOST
    const char *pipe_ab = getenv("TSFP_RT_HOST_PIPE");
    if (pipe_ab != NULL && pipe_ab[0] == '1')
        return "pipe (legacy popen subprocess, TSFP_RT_HOST_PIPE=1): its blocking write STALLS THE GUEST, use only for A/B";
    return "rt-host (in-process render thread, unbounded queue, frames dropped instead of stalling the guest, best real adapter "
           "unless --software / TSFP_WGPU_ADAPTER)";
#else
    return "pipe (legacy popen subprocess): this build has NO rt-host, the blocking write STALLS THE GUEST (about 3x slower "
           "measured on the owner's host); rebuild with -DTSFP_RUST_RT_HOST=ON";
#endif
}
__attribute__((constructor)) static void render_path_startup_banner(void)
{
    if (getenv("TSFP_WGPU_LIVE") != NULL) render_path_banner("requested:", render_path_state());
}
void tsfp_render_trace_binary_send(const unsigned char *bytes, size_t length, uint32_t presents);
void tsfp_render_trace_binary_send(const unsigned char *bytes, size_t length, uint32_t presents)
{
#ifdef TSFP_RUST_RT_HOST
    if (render_trace_file != NULL && (!tsfp_render_trace_fast_hooks() || render_trace_dual)) fflush(render_trace_file); /* explicit text captures keep mixed ordering */
    if (render_trace_binary_dump != NULL) { fwrite(bytes, 1u, length, render_trace_binary_dump); fflush(render_trace_binary_dump); }
    if (render_trace_binary_runtime != NULL) tsfp_rt_host_send_binary(render_trace_binary_runtime, bytes, length, presents);
#else
    (void)bytes; (void)length; (void)presents;
#endif
}
static FILE *render_trace_open(const char *path)
{
    const char *live = getenv("TSFP_WGPU_LIVE");
    int have_path = path != NULL && path[0] != '\0';
    if (live == NULL || live[0] != '1') return have_path ? fopen(path, "w") : NULL;
#ifdef TSFP_RUST_RT_HOST
    const char *pipe_ab = getenv("TSFP_RT_HOST_PIPE");
    if (pipe_ab == NULL || pipe_ab[0] != '1') {
        FILE *in_process = render_trace_open_rt(path);
        if (in_process != NULL) {
            render_path_banner("active:", render_trace_binary_on
                                ? "rt-host (in-process, binary events, the default); stats lines go to stderr when TSFP_RT_HOST_STATS=1"
                                : "rt-host (in-process, TEXT lines: TSFP_RT_HOST_BINARY=0 or a TSFP_RENDER_TRACE file); stats lines go to stderr when TSFP_RT_HOST_STATS=1");
            return in_process;
        }
        /* no adapter, bad TSFP_WGPU_LIVE_ARGS or the thread could not start: the reason is on the line above ("rt-host: ...").
         * T2136d: no silent pipe fallback, it would format every call for a renderer that is not there. */
        render_trace_disabled_line("the in-process render runtime could not start (see the rt-host line above; "
                                   "set TSFP_WGPU_ADAPTER=software or add --software to TSFP_WGPU_LIVE_ARGS to use the CPU adapter)");
        return have_path ? fopen(path, "w") : NULL;
    }
#endif
    char helper[PATH_MAX];
    if (render_trace_find_helper(helper, sizeof(helper)) != 0) {
        render_trace_disabled_line("the pipe path needs the helper process tsfp-wgpu-live and it was not found (TSFP_WGPU_LIVE_BIN, next "
                                   "to the host binary, or on PATH; build it with: cargo build --release --example live "
                                   "--manifest-path crates/tsfp-wgpu/Cargo.toml)");
        return have_path ? fopen(path, "w") : NULL;
    }
    const char *extra = getenv("TSFP_WGPU_LIVE_ARGS");
    char command[4096 + PATH_MAX];
    /* the shell tees the lines into the trace file when both are asked for */
    snprintf(command, sizeof(command), "%s%s%s'%s' %s", have_path ? "tee '" : "", have_path ? path : "", have_path ? "' | " : "",
             helper, extra != NULL ? extra : "");
    signal(SIGPIPE, SIG_IGN); /* a dead renderer must not kill the game */
    render_trace_piped = 1;
    FILE *pipe = popen(command, "w");
    if (pipe == NULL) {
        render_trace_piped = 0;
        render_trace_disabled_line("popen of the live helper failed");
        return have_path ? fopen(path, "w") : NULL;
    }
    render_path_banner("active:", "pipe (legacy popen subprocess), the guest blocks on the pipe when the renderer is slower");
    return pipe;
}
static int render_trace_state = -1; /* -1 unread, 0 off, 1 recording, 2 waiting for TSFP_RENDER_TRACE_SKIP presents */
static uint64_t render_trace_frames, render_trace_limit, render_trace_skip;
static pthread_mutex_t render_trace_lock = PTHREAD_MUTEX_INITIALIZER;
int tsfp_render_trace_enabled(void)
{
    if (render_trace_state >= 0) return render_trace_state == 1;
    pthread_mutex_lock(&render_trace_lock);
    if (render_trace_state < 0) {
        const char *path = getenv("TSFP_RENDER_TRACE");
        const char *frames = getenv("TSFP_RENDER_TRACE_FRAMES");
        render_trace_limit = frames != NULL ? strtoull(frames, NULL, 10) : 0u;
        render_trace_file = render_trace_open(path);
        if (render_trace_file != NULL && !tsfp_render_trace_fast_hooks()) fputs("# tsfp-wgpu stream v1 (T2024 slice 2, host render trace)\n", render_trace_file);
        const char *skip = getenv("TSFP_RENDER_TRACE_SKIP");
        render_trace_skip = skip != NULL ? strtoull(skip, NULL, 10) : 0u;
        const char *legacy = getenv("TSFP_RENDER_TRACE_LEGACY_TEXTURES"); /* slice 6 A/B control, default off */
        if (legacy != NULL && legacy[0] == '1' && tsfp_render_trace_set_legacy_textures != NULL) tsfp_render_trace_set_legacy_textures(1u);
        render_trace_state = render_trace_file == NULL ? 0 : (render_trace_skip != 0u ? 2 : 1);
    }
    pthread_mutex_unlock(&render_trace_lock);
    return render_trace_state == 1;
}
/* Called with render_trace_lock held: stop tracing for good (closing the sink waits for the render thread or the helper). */
static void render_trace_disable_live(const char *why)
{
    FILE *file = render_trace_file;
    render_trace_file = NULL;
    render_trace_state = 0;
    render_trace_binary_on = 0;
    render_trace_disabled_line(why);
    if (file == NULL) return;
#ifdef TSFP_RUST_RT_HOST
    if (file == render_trace_rt_file) render_trace_rt_file = NULL; /* fclose runs the runtime's destroy */
#endif
    if (render_trace_piped) pclose(file); else fclose(file);
}
void tsfp_render_trace_line(const char *text, size_t length)
{
    pthread_mutex_lock(&render_trace_lock);
    if (render_trace_file != NULL) {
        size_t written = fwrite(text, 1u, length, render_trace_file);
        fputc('\n', render_trace_file);
        if (written != length || ferror(render_trace_file)) render_trace_disable_live("the live renderer pipe broke (the helper exited)");
    }
    pthread_mutex_unlock(&render_trace_lock);
}
/* T2024 slice 5: SetRenderTarget (d3d8_set_render_target, src/gpu/d3d8_bind.c) as `call SetSurface`: the hidden SET_SURFACE_*
 * writes (colour offset 0x210, pitch 0x20C, depth offset 0x214) are what the replayer needs for render to texture. */
void tsfp_render_trace_surface(uint32_t target, uint32_t depth, uint32_t colour_data, uint32_t format, uint32_t size,
                               uint32_t pitch, uint32_t depth_data, uint32_t flags);
void tsfp_render_trace_surface(uint32_t target, uint32_t depth, uint32_t colour_data, uint32_t format, uint32_t size,
                               uint32_t pitch, uint32_t depth_data, uint32_t flags)
{
    if (!tsfp_render_trace_enabled()) return;
    const uint64_t started = STAT_BEGIN();
    if (render_trace_binary_on) {
        const uint32_t words[8] = {target, depth, colour_data, format, size, pitch, depth_data, flags};
        tsfp_render_trace_bin_surface(words);
        STAT_END(10u, started, 0u);
        if (!render_trace_dual) return;
    }
    char text[160];
    int length = snprintf(text, sizeof(text), "call SetSurface %x %x %x %x %x %x %x %x", target, depth, colour_data, format,
                          size, pitch, depth_data, flags);
    if (length > 0) tsfp_render_trace_line(text, (size_t)length);
    STAT_END(10u, started, length > 0 ? (uint64_t)length + 1u : 0u);
}
/* T2024 slice 6: constant uploads the lifted code makes straight through the library entries, as `call SetVertexConstants`
 * (the same line the Device wrapper writes, the replayer applies it in stream order). */
/* T2241: native presenter overlay. This changes the render sink's owned words
 * only; Xbox CPU shadow, pushbuffer and simulation state keep the original data.
 * The read-only legacy inlet is a TEMPORARY-SHIM, removal T2244. */
extern uint32_t tsfp_rt_animation_enabled(void) __attribute__((weak));
extern uint32_t tsfp_rt_animation_constants(uint32_t index, const uint32_t *words, uint32_t count, uint32_t *output, uint64_t frame) __attribute__((weak));
extern bool kernel_guest_read_bytes(uint32_t address, void *out, size_t length) __attribute__((weak));
extern uint32_t tsfp_rt_animation_source_constants(uint32_t index, uint32_t source, const uint32_t *words, uint32_t count, uint32_t *output, uint64_t frame) __attribute__((weak));
void tsfp_render_trace_vertex_constants_source(uint32_t index, uint32_t source, const uint32_t *words, uint32_t count);
static void render_trace_vertex_constants(uint32_t index, uint32_t source, const uint32_t *words, uint32_t count);
void tsfp_render_trace_vertex_constants(uint32_t index, const uint32_t *words, uint32_t count);
/* T2024 slice 10: constants uploaded before the skip point through ANY traced library entry (the generic upload, shadowed or
 * not, and the one and four vector entries; the unshadowed ones never write the CPU shadow the skip snapshot reads, which
 * left c13 of the fire sprite program at zero). Latest value per register, overlaid on the CPU shadow at the skip point. */
static uint32_t pre_skip_constant[192u * 4u];
static uint8_t pre_skip_constant_set[192u];
void tsfp_render_trace_vertex_constants(uint32_t index, const uint32_t *words, uint32_t count)
{
    render_trace_vertex_constants(index, 0u, words, count);
}
void tsfp_render_trace_vertex_constants_source(uint32_t index, uint32_t source, const uint32_t *words, uint32_t count)
{
    render_trace_vertex_constants(index, source, words, count);
}
static void render_trace_vertex_constants(uint32_t index, uint32_t source, const uint32_t *words, uint32_t count)
{
    (void)tsfp_render_trace_enabled();
    if (render_trace_state == 2) {
        for (uint32_t i = 0; i < count && index * 4u + i < 192u * 4u; i++) {
            pre_skip_constant[index * 4u + i] = words[i];
            pre_skip_constant_set[(index * 4u + i) / 4u] = 1u;
        }
        return;
    }
    if (!tsfp_render_trace_enabled() || count > 768u) return;
    static int lifted_off = -1; /* A/B control, default on; read once (a getenv per upload was about 400 ns of the 536 measured) */
    if (lifted_off < 0) {
        const char *off = getenv("TSFP_RENDER_TRACE_NO_LIFTED_CONSTANTS");
        lifted_off = off != NULL && off[0] == '1';
    }
    if (lifted_off) return;
    uint32_t fractional[768u];
    if (tsfp_rt_animation_enabled != NULL && tsfp_rt_animation_enabled() != 0u) {
        /* Historical GPU libraries expose only copied words. Their documented
         * D3D8 constant ABI has (index, source) in ECX and EDX (kernel_frame_reg_arg).
         * Recover that source only when the index agrees; the Rust inlet
         * additionally qualifies every word against explicit native provenance.
         * Current libraries pass source directly through the source-aware entry. */
        if (source == 0u && g_ecx == index) source = g_edx;
        uint32_t replaced = source != 0u && tsfp_rt_animation_source_constants != NULL
            ? tsfp_rt_animation_source_constants(index, source, words, count, fractional, guest_read32(0x6B7AF0u)) : 0u;
        if (replaced == 0u && tsfp_rt_animation_constants != NULL)
            replaced = tsfp_rt_animation_constants(index, words, count, fractional, guest_read32(0x6B7AF0u));
        if (replaced != 0u) words = fractional;
    }
    const uint64_t started = STAT_BEGIN();
    if (render_trace_binary_on) {
        tsfp_render_trace_bin_constants(index, words, count);
        STAT_END(9u, started, 0u);
        if (!render_trace_dual) return;
    }
    char text[48 + 768u * 9u];
    int length = snprintf(text, sizeof(text), "call SetVertexConstants %x", index);
    for (uint32_t i = 0; i < count && length > 0; i++) length += snprintf(text + length, sizeof(text) - (size_t)length, " %x", words[i]);
    if (length > 0) tsfp_render_trace_line(text, (size_t)length);
    STAT_END(9u, started, length > 0 ? (uint64_t)length + 1u : 0u);
}
/* T2024 slice 6: library pushes made before the recording window of a skip stream. The last value of every state method is kept
 * (not the ones that trigger work or carry vertex, program or constant data) and written at the start of the stream. */
static uint32_t lib_shadow_value[0x800];
static uint8_t lib_shadow_set[0x800];
static uint32_t lib_drop_value[0x800];
static uint8_t lib_drop_set[0x800];
static int lib_drop_any;
static int lib_shadow_keeps(uint32_t method)
{
    if (method >= 0x2000u) return 0;
    if (method >= 0x1700u && method < 0x1840u) return 0; /* vertex arrays, draw data */
    if (method >= 0x0B00u && method < 0x0C00u) return 0; /* program and constant data */
    if (method >= 0x1D90u && method <= 0x1DA0u) return 0; /* clears */
    if (method == 0x1E9Cu || method == 0x1EA0u || method == 0x1EA4u) return 0;
    return 1;
}
int tsfp_render_trace_wants_lib(void);
int tsfp_render_trace_wants_lib(void)
{
    (void)tsfp_render_trace_enabled();
    return render_trace_state == 1 || render_trace_state == 2;
}
void tsfp_render_trace_lib_method(uint32_t method, uint32_t value);
void tsfp_render_trace_lib_method(uint32_t method, uint32_t value)
{
    if (render_trace_state == 2) {
        if (lib_shadow_keeps(method)) {
            lib_shadow_value[method >> 2] = value;
            lib_shadow_set[method >> 2] = 1u;
        }
        return;
    }
    if (tsfp_render_trace_fast_hooks() && tsfp_render_trace_fast_lib != NULL) {
        tsfp_render_trace_fast_lib(method, value);
        return;
    }
    const uint64_t started = STAT_BEGIN();
    if (render_trace_binary_on && render_trace_dropping_now && !render_trace_dual && lib_shadow_keeps(method)) {
        /* omitted frame: only the last value of a state method matters, written once at the present (60 thousand pushes per
         * frame became a few hundred records) */
        lib_drop_value[method >> 2] = value;
        lib_drop_set[method >> 2] = 1u;
        lib_drop_any = 1;
        STAT_END(8u, started, 0u);
        return;
    }
    if (render_trace_binary_on) {
        tsfp_render_trace_bin_lib(method, value);
        STAT_END(8u, started, 0u);
        if (!render_trace_dual) return;
    }
    char text[48];
    int length = snprintf(text, sizeof(text), "lib %x %x", method, value);
    if (length > 0) tsfp_render_trace_line(text, (size_t)length);
    STAT_END(8u, started, length > 0 ? (uint64_t)length + 1u : 0u);
}
/* T2024 slice 3: texture Data words are synthetic physical addresses (docs/d3d8-usage.md 13.8), the trace needs the guest
 * address that holds the texels. 0 when no live region owns the physical address. */
#include "../xbox/guest_mem.h"
uint32_t tsfp_render_trace_readable(uint32_t address, uint32_t length);
uint32_t tsfp_render_trace_readable(uint32_t address, uint32_t length)
{
    /* The guest image (code and data, below 0x00800000) is always mapped, anything above must lie in one tracked region. */
    if (length == 0u || (uint64_t)address + length <= 0x00800000u) return 1u;
    const uint32_t first = kmem_physical_address(address);
    return first != 0u && kmem_physical_address(address + length - 1u) == first + length - 1u;
}
uint32_t tsfp_render_trace_virtual_of_physical(uint32_t physical, uint32_t length);
uint32_t tsfp_render_trace_virtual_of_physical(uint32_t physical, uint32_t length)
{
    /* The original Register masks the allocation's virtual address into the 28-bit Data word (d3d8_resource.c) and the game's
     * heap is at 0x4xxxxxxx, so the first candidate is 0x40000000 | Data when one tracked region covers the whole span. The
     * (physical, virtual) registry of contiguous allocations is the fallback. */
    if (length == 0u) return 0u;
    const kernel_guest_ptr candidate = 0x40000000u | physical;
    const uint32_t first = kmem_physical_address(candidate);
    if (first != 0u && kmem_physical_address(candidate + length - 1u) == first + length - 1u) return (uint32_t)candidate;
    kernel_guest_ptr address = 0u;
    kernel_guest_ptr last = 0u;
    if (!kmem_virtual_from_physical(physical, &address) ||
        !kmem_virtual_from_physical(physical + length - 1u, &last) || last - address != length - 1u)
        return 0u; /* not one live region over the whole span */
    return (uint32_t)address;
}
/* T2024 slice 4: opt-in hold for a paired host shot. TSFP_RENDER_TRACE_HOLD=1 makes the guest thread block at the present
 * AFTER the one that closed the trace (TSFP_RENDER_TRACE_FRAMES reached), so the display pipeline (it trails the guest by 20 or
 * more presents) drains and the window shows exactly the last traced present. It writes "<trace file>.held" with that present
 * number and sleeps until the driver kills the host. Off unless the env var is set. */
static int render_trace_hold_armed;
static char render_trace_hold_path[1024];
static uint64_t render_trace_hold_number;
void tsfp_render_trace_present(uint64_t number)
{
    (void)tsfp_render_trace_enabled();
    if (tsfp_render_trace_stats_enabled()) {
        /* T2136d: the unbiased pace of the whole guest (no per hook timers), printed in every mode, trace on or off */
        static uint64_t pace_time, pace_number;
        const uint64_t now = tsfp_render_trace_now_ns();
        if (pace_time == 0u) { pace_time = now; pace_number = number; }
        else if (number >= pace_number + 250u) {
            fprintf(stderr, "rt-host: guest-pace: present %llu, %.1f presents/s over the last %llu presents\n", (unsigned long long)number,
                    (double)(number - pace_number) * 1e9 / (double)(now - pace_time), (unsigned long long)(number - pace_number));
            pace_time = now; pace_number = number;
        }
    }
    if (render_trace_hold_armed == 2) {
        FILE *marker = fopen(render_trace_hold_path, "w");
        if (marker != NULL) {
            fprintf(marker, "%llu\n", (unsigned long long)render_trace_hold_number);
            fclose(marker);
        }
        render_trace_hold_armed = 3;
        for (;;) sleep(1);
    }
    pthread_mutex_lock(&render_trace_lock);
#ifdef TSFP_RUST_RT_HOST
    if (render_trace_state == 1 && render_trace_rt_cookie != NULL && render_trace_file == render_trace_rt_file &&
        !tsfp_rt_host_alive(render_trace_rt_cookie->runtime))
        render_trace_disable_live("the live renderer was switched off (queue memory cap or stall limit)");
#endif
    if (render_trace_state == 2 && number >= render_trace_skip) {
        render_trace_state = 1; /* recording starts at a frame boundary */
        /* T2024 slice 6: the constants set before the skip point (c10 and the like are written once) from the library's CPU
         * shadow, as one `call SetVertexConstants` line, so a skip stream starts with the constant file the engine had. */
        /* A/B controls for the slice 6 findings (default both on): TSFP_RENDER_TRACE_NO_RESET=1 keeps the dedupe caches that
         * were filled while the lines were dropped, TSFP_RENDER_TRACE_NO_SHADOW=1 omits the constant file line. */
        const char *no_reset = getenv("TSFP_RENDER_TRACE_NO_RESET");
        const char *no_shadow = getenv("TSFP_RENDER_TRACE_NO_SHADOW");
        if (tsfp_render_trace_reset_dedupe != NULL && !(no_reset != NULL && no_reset[0] == '1')) tsfp_render_trace_reset_dedupe();
        const int pre_skip_off = getenv("TSFP_RENDER_TRACE_NO_PRE_SKIP_CONSTANTS") != NULL;
        if (render_trace_file != NULL && !(no_shadow != NULL && no_shadow[0] == '1')) {
            if (render_trace_binary_on) {
                /* T2136d: binary mode writes the skip point state as records too (no text in the middle of the stream) */
                static uint32_t shadow[192u * 4u];
                for (uint32_t i = 0; i < 192u * 4u; i++)
                    shadow[i] = pre_skip_constant_set[i / 4u] && !pre_skip_off ? pre_skip_constant[i] : d3d8_guest_load32(0x003E2EA0u + i * 4u);
                tsfp_render_trace_bin_constants(0u, shadow, 192u * 4u);
            }
            if (!render_trace_binary_on || render_trace_dual) {
                fputs("# constant shadow at the skip point\ncall SetVertexConstants 0", render_trace_file);
                for (uint32_t i = 0; i < 192u * 4u; i++)
                    fprintf(render_trace_file, " %x", pre_skip_constant_set[i / 4u] && !pre_skip_off ? pre_skip_constant[i] : d3d8_guest_load32(0x003E2EA0u + i * 4u));
                fputc('\n', render_trace_file);
            }
        }
        if (render_trace_file != NULL && !(no_shadow != NULL && no_shadow[0] == '1')) {
            if (!render_trace_binary_on || render_trace_dual) fputs("# library state pushed before the skip point\n", render_trace_file);
            for (uint32_t slot = 0; slot < 0x800u; slot++) {
                if (!lib_shadow_set[slot]) continue;
                if (render_trace_binary_on) tsfp_render_trace_bin_lib(slot << 2, lib_shadow_value[slot]);
                if (!render_trace_binary_on || render_trace_dual) fprintf(render_trace_file, "lib %x %x\n", slot << 2, lib_shadow_value[slot]);
            }
        }
    }
    if (render_trace_state == 1 && render_trace_file != NULL) {
        const uint64_t started = STAT_BEGIN();
        if (render_trace_binary_on) {
            if (lib_drop_any) {
                for (uint32_t slot = 0; slot < 0x800u; slot++) {
                    if (!lib_drop_set[slot]) continue;
                    lib_drop_set[slot] = 0u;
                    tsfp_render_trace_bin_lib(slot << 2, lib_drop_value[slot]);
                }
                lib_drop_any = 0;
            }
            tsfp_render_trace_bin_present(number);
#ifdef TSFP_RUST_RT_HOST
            if (render_trace_binary_runtime != NULL && tsfp_render_trace_bin_frame != NULL) {
                const int behind = tsfp_rt_host_pressure(render_trace_binary_runtime);
                if (behind != render_trace_dropping_now) {
                    render_trace_dropping_now = behind;
                    tsfp_render_trace_bin_frame(behind ? 1u : 0u); /* the renderer drops the draws of the frames that follow */
                }
            }
#endif
        }
        if (!render_trace_binary_on || render_trace_dual) {
            fprintf(render_trace_file, "# present %llu\npresent\n", (unsigned long long)number);
            fflush(render_trace_file);
        }
        if (started != 0u) {
            static uint64_t last_print;
            STAT_END(11u, started, 0u);
            tsfp_render_trace_stat_frame();
            if (last_print == 0u) last_print = started;
            if (started - last_print > 5000000000u && tsfp_render_trace_stat_print != NULL) {
                last_print = started;
                tsfp_render_trace_stat_print();
            }
        }
        if (++render_trace_frames >= render_trace_limit && render_trace_limit != 0u) {
            if (tsfp_render_trace_stats_enabled() && tsfp_render_trace_stat_print != NULL) tsfp_render_trace_stat_print();
#ifdef TSFP_RUST_RT_HOST
            if (render_trace_file == render_trace_rt_file) render_trace_rt_file = NULL; /* fclose runs the runtime's destroy */
#endif
            if (render_trace_piped) pclose(render_trace_file); else fclose(render_trace_file);
            render_trace_file = NULL;
            render_trace_state = 0;
            const char *hold = getenv("TSFP_RENDER_TRACE_HOLD");
            const char *path = getenv("TSFP_RENDER_TRACE");
            if (hold != NULL && hold[0] == '1' && path != NULL && strlen(path) + 6u < sizeof(render_trace_hold_path)) {
                snprintf(render_trace_hold_path, sizeof(render_trace_hold_path), "%s.held", path);
                render_trace_hold_number = number;
                render_trace_hold_armed = 2;
            }
        }
    }
    pthread_mutex_unlock(&render_trace_lock);
}
#endif
