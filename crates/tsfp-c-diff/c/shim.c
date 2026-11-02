/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Host shim for the T1836 C-vs-Rust differential. It supplies the lifter registers the
 * unmodified src/game replacements use and a dispatcher over the replacements under test.
 * Guest address A is host pointer (arena base + A), the same identity model the harness uses. */
#include <stddef.h>
#include <stdint.h>

ptrdiff_t g_xbox_mem_offset;
__thread uint32_t g_eax, g_ecx, g_edx, g_esp, g_esi, g_fs_base, g_ebx, g_edi, g_ebp;
__thread int g_df;

/* T1836 wave 8: XMM state of the FP/scalar-SSE replacements. The replacement sources also carry
 * weak definitions of these slots, this strong definition wins. Same layout as GameFpXmm. */
typedef union ShimXmm { float f[4]; double d[2]; uint32_t u[4]; int32_t i[4]; uint64_t q[2]; } ShimXmm;
__thread ShimXmm g_xmm0 __attribute__((aligned(8)));
__thread ShimXmm g_xmm1 __attribute__((aligned(8)));
__thread ShimXmm g_xmm2 __attribute__((aligned(8)));
__thread ShimXmm g_xmm3 __attribute__((aligned(8)));
__thread ShimXmm g_xmm4 __attribute__((aligned(8)));
__thread ShimXmm g_xmm5 __attribute__((aligned(8)));
__thread ShimXmm g_xmm6 __attribute__((aligned(8)));
__thread ShimXmm g_xmm7 __attribute__((aligned(8)));

/* One X(va) entry per root under test. Add a root here and in build.rs when its file is new. */
#define ROOTS(X) \
    X(002C3AC0) \
    X(00012BA0) \
    X(00056E90) \
    X(00056F70) \
    X(000D0CA0) \
    X(0006B4E0) \
    X(000AF330) \
    X(00160B80) \
    X(000C4560) \
    X(00042F90) \
    X(001957D0) \
    X(002D2BE0) \
    X(00059F10) \
    X(00059F50) \
    X(00240580) \
    X(00233F90) \
    X(00243060) \
    X(00230510) \
    X(00264740) \
    X(00275620) \
    X(001B58C0) \
    X(0035FD70) \
    X(002ED7B0) \
    X(00032F10) \
    X(0006B3D0) \
    X(00167D20) \
    X(001C8C20) \
    X(00227880) \
    X(002278E0) \
    X(00227980) \
    X(002743A0) \
    X(002743D0) \
    X(002744A0) \
    X(00274590) \
    X(00274620) \
    X(002BE670) \
    X(002BEA00) \
    X(002DB4D0) \
    X(002DB4F0) \
    X(002F7770) \
    X(002FA0D0) \
    X(002FB1F0) \
    X(002FB9D0) \
    X(002FC0A0) \
    X(002FE4E0) \
    X(00307C20) \
    X(003118B0) \
    X(0031A860) \
    X(0031BD70) \
    X(0031D310) \
    X(0031DB00) \
    X(0031DBC0) \
    X(0031E450) \
    X(0031E8C0) \
    X(0031E910) \
    X(0031F220) \
    X(00017A80) \
    X(0003ECA0) \
    X(00038F80) \
    X(000D6920) \
    X(000DCA70) \
    X(000E0250) \
    X(00109990) \
    X(001118A0) \
    X(00119C20) \
    X(00139160) \
    X(00142990) \
    X(00156DB0) \
    X(00165760) \
    X(0016B170) \
    X(0016B190) \
    X(0016B7C0) \
    X(0017F780) \
    X(00183040) \
    X(001B5660) \
    X(001B66E0) \
    X(001B6740) \
    X(001B6C50) \
    X(001B7CC0) \
    X(001B90F0) \
    X(0020FA50) \
    X(00227400) \
    X(002275F0) \
    X(00227620) \
    X(00227A30) \
    X(00227A50) \
    X(00228FB0) \
    X(0022BDC0) \
    X(0022D110) \
    X(0022D410) \
    X(0022D460) \
    X(0022E050) \
    X(00230CC0) \
    X(0025A080) \
    X(0025B720) \
    X(00012020) \
    X(00022950) \
    X(00024E40) \
    X(00038F30) \
    X(00038F50) \
    X(0003F5E0) \
    X(00055EC0) \
    X(00058270) \
    X(000696D0) \
    X(000697D0) \
    X(00069930) \
    X(00071E60) \
    X(00080AD0) \
    X(00081C20) \
    X(000873F0) \
    X(000877C0) \
    X(000879A0) \
    X(000879C0) \
    X(000879E0) \
    X(0009A390) \
    X(0009A4C0) \
    X(0009A570) \
    X(0009C520) \
    X(0009C550) \
    X(0009E610) \
    X(000DC270) \
    X(000DCA40) \
    X(00346250) \
    X(0034C0D0) \
    X(000174E0) \
    X(00071740) \
    X(00264720) \
    X(0010EBD0) \
    X(00234270) \
    X(0036CC10) \
    X(000D7550) \
    X(0015BCA0) \
    X(00177E90) \
    X(001B5840) \
    X(001B6540) \
    X(001B6A10) \
    X(00242EE0) \
    X(00350D40) \
    X(00350E70) \
    X(0037A6F0) \
    X(00087840) \
    X(00045D60) \
    X(00063BF0) \
    X(000D76A0) \
    X(00026B70) \
    X(000427B0) \
    X(00042C30) \
    X(000431C0) \
    X(00246D10) \
    X(00190D60) \
    X(00043A90) \
    X(00043AF0) \
    X(003443F0) \
    X(0001E8D0) \
    X(0008E3E0) \
    X(000387D0) \
    X(003B20F0) \
    X(003B2110) \
    X(003B2130) \
    X(003B2150) \
    X(0035E590) \
    X(00356290) \
    X(00356020) \
    X(003BC270) \
    X(00047B30) \
    X(00046280) \
    X(0005A150) \
    X(0005A1E0) \
    X(00073530) \
    X(000A4CA0) \
    X(000AEE20) \
    X(000C9BC0) \
    X(00120220) \
    X(00151F20) \
    X(00347FA0) \
    X(000D1E20) \
    X(00238D90) \
    X(002DAF90) \
    X(00309120) \
    X(00341980) \
    X(003441A0) \
    X(001543A0) \
    X(00154470) \
    X(00150580) \
    X(00151190) \
    X(00152120) \
    X(001515E0) \
    X(001553B0) \
    X(0012B8E0) \
    X(0011FDC0) \
    X(00309160) \
    X(00319440) \
    X(0036E920) \
    X(0036EC40) \
    X(00377560) \
    X(00307E60) \
    X(000425E0) \
    X(00046210) \
    X(00065230) \
    X(000746A0) \
    X(000AEE60) \
    X(000B2D20) \
    X(000C9440) \
    X(000C9F60) \
    X(000E10E0) \
    X(000FF3D0) \
    X(0010E970) \
    X(0010EA50) \
    X(0010EA90) \
    X(0010EAB0) \
    X(0010EB10) \
    X(0010EB50) \
    X(0010EB90) \
    X(0010ECB0) \
    X(00119AB0) \
    X(0011F510) \
    X(0011F540) \
    X(0011F570) \
    X(0011FE40) \
    X(0011FE80) \
    X(00120FF0) \
    X(0012B940) \
    X(0012C130) \
    X(0012DE80) \
    X(001427D0) \
    X(00155040) \
    X(00155410) \
    X(00177930) \
    X(00196170) \
    X(001B5680) \
    X(001B62D0) \
    X(0024C920) \
    X(002649D0) \
    X(00309080) \
    X(003090D0) \
    X(00341630) \
    X(00341A10) \
    X(00349390) \
    X(000A5A60) \
    X(000B0E30) \
    X(000FF420) \
    X(00124C90) \
    X(00124F30) \
    X(0022B130) \
    X(002F7A20) \
    X(00164960) \
    X(00166060) \
    X(00166140) \
    X(003C6730) \
    X(003BA270) \
    X(003B5FE0) \
    X(003B3120) \
    X(00033DB0) \
    X(00356080) \
    X(003560E0) \
    X(00356320) \
    X(00356870) \
    X(003568C0) \
    X(00356990) \
    X(00358130) \
    X(00358160) \
    X(00358560) \
    X(00359910) \
    X(0035E630) \
    X(0035E6A0) \
    X(0035EAF0) \
    X(0015C4E0) \
    X(00235600) \
    X(002D74B0) \
    X(0008CBC0) \
    X(000B0DD0) \
    X(000B3C60) \
    X(001115F0) \
    X(00126AC0) \
    X(00128E90) \
    X(00129EC0) \
    X(00166770) \
    X(001B7AA0) \
    X(0025F9C0) \
    X(002AD370) \
    X(002E5900) \
    X(00347B80) \
    X(00347D70) \
    X(00349570) \
    X(00357DD0) \
    X(00362CC0) \
    X(00012EC0) \
    X(00070D10) \
    X(000718E0) \
    X(00071970) \
    X(00071CD0) \
    X(00074670) \
    X(00074840) \
    X(00074950) \
    X(00074AB0) \
    X(00074B80) \
    X(00087A40) \
    X(00094290) \
    X(000942D0) \
    X(0009E730) \
    X(000A2C60) \
    X(000A4C70) \
    X(000B0590) \
    X(000B0770) \
    X(000B0F90) \
    X(000B2D40) \
    X(000B3CA0) \
    X(000C2E30) \
    X(000C7360) \
    X(000DA220) \
    X(0010E780) \
    X(0011D990) \
    X(001286D0) \
    X(00158D40) \
    X(0015A270) \
    X(0015C120) \
    X(0015C3D0) \
    X(00160850) \
    X(00160F50) \
    X(00163810) \
    X(001665A0) \
    X(00167A70) \
    X(00172C80) \
    X(001766E0) \
    X(00176A40) \
    X(00177CE0) \
    X(00177EC0) \
    X(0017A120) \
    X(0017A200) \
    X(0017A260) \
    X(00188920) \
    X(001B6310) \
    X(001B63B0) \
    X(001B63D0) \
    X(001B7580) \
    X(001B75B0) \
    X(0023FF70) \
    X(00240020) \
    X(00242EC0) \
    X(00247A70) \
    X(002533E0) \
    X(002744C0) \
    X(000606E0) \
    X(00060740) \
    X(0015C390) \
    X(0017A190) \
    X(0018AC60) \
    X(001EC8E0) \
    X(001EC980) \
    X(001EC9C0) \
    X(001ECA00) \
    X(002346A0) \
    X(00247A10) \
    X(00248910) \
    X(000DB480) \
    X(00188840) \
    X(000AE910) \
    X(00190680) \
    X(00190A70) \
    X(00190690) \
    X(00190AC0) \
    X(00240560) \
    X(001DE560) \
    X(0024D260) \
    X(001CD560) \
    X(0025EF30) \
    X(001B7E80) \
    X(00208AC0) \
    X(000CB3B0) \
    X(001B7D00) \
    X(001B7D70) \
    X(00274330) \
    X(00274350) \
    X(00274380) \
    X(002744F0) \
    X(00168160) \
    X(0010E9A0) \
    X(00012F30) \
    X(00012F50) \
    X(00150200) \
    X(00227BE0) \
    X(0023EF70) \
    X(0025B770) \
    X(002586A0) \
    X(002AEFB0) \
    X(00311A80) \
    X(00311A90) \
    X(001553E0) \
    X(0022D790) \
    X(002EF160) \
    X(002E4470) \
    X(0022DE90) \
    X(002BFEC0) \
    X(000662D0) \
    X(000662F0) \
    X(00012FE0) \
    X(002B2390) \
    X(0025F320) \
    X(00075F40) \
    X(00064EF0) \
    X(002BE9B0) \
    X(00190A80) \
    X(0012D1E0) \
    X(002E4CB0) \
    X(0033ACB0) \
    X(0009E750) \
    X(00073E10) \
    X(00349710) \
    X(00147200) \
    X(0026CC80) \
    X(0026CD50) \
    X(00338150) \
    X(00070C90) \
    X(0007C1B0) \
    X(0007C4D0) \
    X(00275640) \
    X(0016C070) \
    X(0016C0D0)

/* T3396: test-only fallback symbols; TSFP_REF_LIFTED=1 resolves the original bodies. */
void sub_0016C070(void) {}
void sub_0016C0D0(void) {}

/* T1835 wave 11: the production x87 backend of src/game/x87_replace.h (the lifted code's own
 * double-backed model). x87_roots.c declares these extern, the shim owns them. */
__thread double g_fp_stack[8];
__thread int g_fp_top;
__thread uint16_t g_fp_control_word;

#define DECLARE(va) void sub_##va(void);
ROOTS(DECLARE)


/* T2007: lifted-original reference mode. With TSFP_REF_LIFTED set, shim_call/shim_run run the
 * private lifted body of the root (strong t20_lifted_lookup in the gitignored-source object built
 * by ref_lifted.rs) instead of the C replacement, so every existing suite compares the Rust port
 * against the lifted original. Each root is announced once on stderr ("T2007-REF lifted|missing
 * VA") so a runner can attribute test results to roots. */
/* Defined by the object ref_lifted.rs builds (a stub returning 0 when the lifted tree is absent). */
extern void (*t20_lifted_lookup(uint32_t va))(void);
extern void t20_seed_ebp(uint32_t v);
#include <stdio.h>
#include <stdlib.h>
static void (*pick_reference(uint32_t va, void (*replacement)(void)))(void)
{
    static int lifted = -1;
    if (lifted < 0) lifted = getenv("TSFP_REF_LIFTED") != 0;
    if (!lifted) return replacement;
    static uint32_t seen[1024];
    static uint32_t seen_len;
    void (*fn)(void) = t20_lifted_lookup(va);
    int known = 0;
    for (uint32_t i = 0; i < seen_len; i++) known |= seen[i] == va;
    if (!known && seen_len < 1024) {
        seen[seen_len++] = va;
        fprintf(stderr, "T2007-REF %s %08X\n", fn ? "lifted" : "missing", va);
    }
    return fn ? fn : replacement;
}

void shim_set_base(void *base) { g_xbox_mem_offset = (ptrdiff_t)base; }

/* regs = {eax, ecx, edx, esi, esp} in and out. Returns 0 when va is not known. */
int shim_call(uint32_t va, uint32_t regs[5])
{
    void (*fn)(void) = 0;
    switch (va) {
#define CASE(va) case 0x##va##u: fn = sub_##va; break;
    ROOTS(CASE)
    default: return 0;
    }
    fn = pick_reference(va, fn);
    g_eax = regs[0]; g_ecx = regs[1]; g_edx = regs[2]; g_esi = regs[3]; g_esp = regs[4];
    t20_seed_ebp(g_ebp);
    fn();
    regs[0] = g_eax; regs[1] = g_ecx; regs[2] = g_edx; regs[3] = g_esi; regs[4] = g_esp;
    return 1;
}

/* T1836 wave 8: same as shim_call plus the eight XMM registers (xmm[8*4] words, in and out) and
 * MXCSR. The control word is forced to the proof contract 0x1F80 (round to nearest, FTZ 0, DAZ 0,
 * all exceptions masked) around the call, the previous host value is restored. The sticky status
 * bits the replacement's own SSE instructions set are returned in *mxcsr (informational: the
 * proof masks 0x3f, so the differential does not compare them). */
int shim_call_fp(uint32_t va, uint32_t regs[5], uint32_t xmm[32], uint32_t *mxcsr)
{
    ShimXmm *slots[8] = { &g_xmm0, &g_xmm1, &g_xmm2, &g_xmm3, &g_xmm4, &g_xmm5, &g_xmm6, &g_xmm7 };
    uint32_t saved, control = 0x1F80u, after;
    for (int i = 0; i < 8; i++)
        for (int lane = 0; lane < 4; lane++) slots[i]->u[lane] = xmm[4 * i + lane];
    __asm__ volatile("stmxcsr %0" : "=m"(saved));
    __asm__ volatile("ldmxcsr %0" : : "m"(control));
    int known = shim_call(va, regs);
    __asm__ volatile("stmxcsr %0" : "=m"(after));
    __asm__ volatile("ldmxcsr %0" : : "m"(saved));
    for (int i = 0; i < 8; i++)
        for (int lane = 0; lane < 4; lane++) xmm[4 * i + lane] = slots[i]->u[lane];
    *mxcsr = after;
    return known;
}

/* T1836 wave 8: the scalar SSE instructions themselves, for the model-versus-hardware test of
 * crates/tsfp-engine-memory/src/sse.rs. The host runs them under the 0x1F80 control word. */
static float bits_f(uint32_t b) { union { uint32_t u; float f; } v; v.u = b; return v.f; }
static uint32_t f_bits(float f) { union { uint32_t u; float f; } v; v.f = f; return v.u; }

uint32_t shim_ss_op(int op, uint32_t a, uint32_t b)
{
    float dest = bits_f(a), source = bits_f(b);
    uint32_t saved, control = 0x1F80u;
    __asm__ volatile("stmxcsr %0" : "=m"(saved));
    __asm__ volatile("ldmxcsr %0" : : "m"(control));
    switch (op) {
    case 0: __asm__ volatile("addss %1, %0" : "+x"(dest) : "xm"(source)); break;
    case 1: __asm__ volatile("subss %1, %0" : "+x"(dest) : "xm"(source)); break;
    case 2: __asm__ volatile("mulss %1, %0" : "+x"(dest) : "xm"(source)); break;
    default: __asm__ volatile("divss %1, %0" : "+x"(dest) : "xm"(source)); break;
    }
    __asm__ volatile("ldmxcsr %0" : : "m"(saved));
    return f_bits(dest);
}

/* AH after `ucomiss/comiss a, b ; lahf`. */
uint32_t shim_compare_ah(int ordered_signalling, uint32_t a, uint32_t b)
{
    float x = bits_f(a), y = bits_f(b);
    unsigned char ah;
    if (ordered_signalling)
        __asm__ volatile("comiss %2,%1; lahf; movb %%ah,%0" : "=m"(ah) : "x"(x), "xm"(y) : "eax", "cc");
    else
        __asm__ volatile("ucomiss %2,%1; lahf; movb %%ah,%0" : "=m"(ah) : "x"(x), "xm"(y) : "eax", "cc");
    return ah;
}

uint32_t shim_cvttss2si(uint32_t a)
{
    int32_t r;
    float x = bits_f(a);
    __asm__ volatile("cvttss2si %1, %0" : "=r"(r) : "xm"(x));
    return (uint32_t)r;
}

uint32_t shim_cvtsi2ss(uint32_t a)
{
    float r = 0;
    __asm__ volatile("cvtsi2ss %1, %0" : "+x"(r) : "r"((int32_t)a));
    return f_bits(r);
}

static inline uint32_t guest_u32(uint32_t a) { return *(uint32_t *)((char *)g_xbox_mem_offset + a); }

/* ---- T1848 wave 4b: full register file, guest-callee stub and call trace ----
 * game_guest_call (static inline in game_replace.h) resolves its target through the weak
 * recomp_lookup. The shim defines it so every nested guest call lands in one stub that records
 * the call (target, callee ESP, return PC, up to three stack words, all registers) and returns
 * the next scripted EAX/ECX/EDX triple. The callee writes no memory (the Rust stub does not). */
#define TRACE_WORDS 16
#define TRACE_MAX 64
static uint32_t t_trace[TRACE_MAX][TRACE_WORDS];
static uint32_t t_count, t_pending;
static uint32_t t_script[3 * 64];
static uint32_t t_script_len;
/* T1815 w8: seeded guest-memory writes a callee performs, {call index, address, value} triples. */
static uint32_t t_wr[3 * 64];
static uint32_t t_wr_len;

void shim_script(const uint32_t *words, uint32_t entries)
{
    for (uint32_t i = 0; i < 3 * entries && i < 3 * 64; i++) t_script[i] = words[i];
    t_script_len = entries > 64 ? 64 : entries;
    t_count = 0;
    t_wr_len = 0;
}
void shim_script_writes(const uint32_t *words, uint32_t entries)
{
    t_wr_len = entries > 64 ? 64 : entries;
    for (uint32_t i = 0; i < 3 * t_wr_len; i++) t_wr[i] = words[i];
}
uint32_t shim_trace_len(void) { return t_count; }
const uint32_t *shim_trace_entry(uint32_t i) { return t_trace[i]; }

/* T1900: extra callee contracts and a per-call hook, strong definitions live in the private
 * t1900 object (t1900_lifted.rs). Without it the weak defaults keep every other suite unchanged. */
__attribute__((weak)) int t1900_extra_argc(uint32_t va) { return -1; }
__attribute__((weak)) void t1900_callee_hook(uint32_t va, uint32_t index, int phase) {}
/* Stack argument count of each stubbed guest callee. Unknown targets trap. */
/* Single source of truth: the Rust differential asks this function (shim_callee_argc) instead of
 * keeping its own table. Returns -1 for an unknown callee. Extend only here. */
static int callee_argc_or_neg(uint32_t va)
{
    switch (va) {
    /* T3396: lifted deferred-contact drain callback and allocator release. */
    case 0x00CAF000: return 2;
    case 0x0003E1E0: return 1;
    case 0x15BCC0: case 0x17A120: case 0x0606E0: case 0x23FF20: case 0x9A4C0: case 0x156E50: case 0x156E80: return 1;
    case 0x060410: case 0x0DB3B0: case 0x09E720: return 2;
    case 0x156DB0: return 4;
    case 0x0827B0: case 0x024E40: case 0x024E60: case 0x024E90: case 0x227400: case 0x2273C0: return 1;
    case 0x0827F0: case 0x069630: case 0x035DE60: case 0x153B40: case 0x210FF0: return 0;
    case 0x10C0E0: return 1;
    case 0x10C120: return 2;
    /* T2048 lifted CRT atexit-table differential: heap-size query and realloc. */
    case 0x3CC96B: case 0x3CAEEF: return 2;
    case 0x3533F0: case 0x3562E0: case 0x35E960: case 0x355F10: case 0x355EC0: return 0;
    case 0x1656D0: case 0x0B7600: case 0x09CCA0:
    case 0x0389B0: case 0x3568C0: case 0x042710: case 0x042C30: case 0x15BA10: case 0x15C3D0: return 1;
    case 0x3C8960: return 3;
    case 0x2E46A0: case 0x079C00: return 0;
    case 0x069640: case 0x0CB0F0: case 0x064D40: return 0;
    case 0x066280: return 3;
    case 0x15BD00: case 0x0D7550: return 2;
    case 0x15BE50: case 0x1CDAE0: case 0x3526E0: return 0;
    case 0x059D70: case 0x059D20: return 2;
    case 0x274380: return 1;
    case 0x15BE60: case 0x25FA10: case 0x0747C0: case 0x03D7B0: case 0x042870: case 0x055D80: return 1;
    /* T1895 w4 lifted-original roots (tests/differential/w4.rs) */
    case 0x0C63D0: case 0x1CEBA0: case 0x0D8C80: case 0x356940: case 0x1C4C60: case 0x0F8AE0: case 0x33CB90: return 0;
    case 0x234270: case 0x05F040: case 0x0DB4C0: case 0x0DC110: case 0x1B6A80: case 0x1BDE40: case 0x358560: case 0x15EAE0: case 0x045590: return 1;
    case 0x153190: case 0x238510: case 0x1C4A50: case 0x2385B0: case 0x1B8710: case 0x047560: case 0x2D7F90: case 0x1B7B0: return 4;
    /* T1920 engine/core second half roots */
    case 0x3C8FD0: case 0x0CB3B0: case 0x080F20: case 0x06F450: return 1; /* T2409: 6F450 player-name resolver */
    case 0x027700: case 0x037300: case 0x022040: case 0x087600: case 0x1F2560: return 2;
    case 0x3C848C: case 0x03C330: case 0x03C2D0: return 0;
    case 0x128440: return 5;
    case 0x38488E: return 0; /* T1920 syslink link status query */
    /* T1920 big five (235240 c1850 25f520 1c5db0) callees */
    case 0x0C0C50: case 0x0BDD50: case 0x0BF080: case 0x061CA0: return 0;
    case 0x0C15B0: case 0x177910: case 0x0B0EA0: case 0x0B0770: return 1;
    case 0x06C890: case 0x25F370: case 0x012FA0: case 0x2274C0: return 2;
    case 0x1B8690: case 0x0174E0: return 3;
    /* T2369 lifted Story writer guard and mutation helpers */
    case 0x239820: case 0x64D10: return 0;
    case 0x2D2BC0: case 0x64D60: return 1;
    case 0x1CD040: case 0x238670: case 0x69A60: return 2;
    /* T3397 lifted HUD message tick callees. */
    case 0x12DF00: return 2;
    case 0x45540: case 0x45390: case 0x45820: return 1;
    /* T3397 kind-8 direct presenter draw differential. */
    case 0x12D150: return 0;
    case 0x42490: return 4;
    /* T3397 kind-1 centered presenter metric and outlined draw boundaries. */
    case 0x405C0: return 1;
    case 0x41E30: return 5;
    /* T3397 kind-9 presenter helpers and centered outlined draw boundary. */
    case 0x3FA10: return 1;
    /* T3397 kind-7 progress presenter native text count and prefix helpers. */
    case 0x40650: return 1;
    case 0x406A0: return 2;
    case 0x40F00: return 3;
    case 0x148680: return 0;
    case 0x40620: return 1;
    case 0x42130: return 5;
    /* T3397 lifted kind-7 replay callees. */
    case 0x19D70: case 0x427B0: case 0x12D9C0: case 0x149590: return 1;
    /* T3397 kind-8 drawer boundary: the dispatcher passes the message entry. */
    case 0x12D8E0: return 1;
    /* T3397 kind-4 title drawer helpers. */
    case 0x12CB40: return 2;
    case 0x40700: return 3;
    case 0x148830: return 0;
    /* T3397 kind-9 split-notice drawer boundary: the dispatcher passes the entry. */
    case 0x149710: return 1;
    case 0x149810: return 2;
    case 0x12DF60: case 0x12CAD0: return 0;
    /* T3402 lifted animation dependency prefix. */
    case 0x6BD30: return 0;
    /* T3402 instance evaluator queries whether the owner requests a key sample. */
    case 0x60920: return 1;
    /* T3402 instance evaluator asks for the node's keyframe time bracket. */
    case 0x585B0: return 3;
    /* T3402 lifted key sampler writes its five-argument sample to guest memory. */
    case 0x59320: return 5;
    /* T3402 instance evaluator composes the current pose with no stack arguments. */
    case 0x604D0: case 0x60540: case 0x605D0: return 0;
    case 0x5A130: return 3;
    /* T3402 lifted post-load animation binding. */
    case 0x3C85D4: return 5;
    case 0x57F30: return 3;
    default: return t1900_extra_argc(va);
    }
}
int shim_callee_argc(uint32_t va) { return callee_argc_or_neg(va); }
static uint32_t callee_argc(uint32_t va)
{
    int argc = callee_argc_or_neg(va);
    if (argc < 0) __builtin_trap();
    return (uint32_t)argc;
}

static void callee_stub(void)
{
    uint32_t i = t_count++;
    if (i >= TRACE_MAX) __builtin_trap();
    t1900_callee_hook(t_pending, i, 0);
    uint32_t argc = callee_argc(t_pending);
    uint32_t *e = t_trace[i];
    e[0] = t_pending; e[1] = g_esp; e[2] = guest_u32(g_esp);
    for (uint32_t a = 0; a < 3; a++) e[3 + a] = a < argc ? guest_u32(g_esp + 4 + 4 * a) : 0;
    /* T3397: the lifted HUD ager submits a pointer to a stack-local audio request.
     * The same address is reused for each cue, so preserve the pointed-to cue now. */
    if (t_pending == 0x45820 && argc > 0) e[13] = guest_u32(e[3]);
    e[6] = g_eax; e[7] = g_ecx; e[8] = g_edx; e[9] = g_ebx; e[10] = g_esi; e[11] = g_edi; e[12] = g_ebp;
    /* T3396: snapshot the three compared payload words at callback entry. A callback may
     * rewrite the just-popped slot before the drain returns, so reading the arena afterward
     * would report the replacement row for both visits. */
    if (t_pending == 0x00CAF000u && argc == 2) {
        const uint32_t row = guest_u32(g_esp + 8);
        e[13] = guest_u32(row);
        e[14] = guest_u32(row + 4);
        e[15] = guest_u32(row + 8);
    }
    /* T3397 presenter checks need the final color argument beyond the three
     * common trace slots. Keep it at entry because the caller reuses stack. */
    if (t_pending == 0x42490u && argc == 4) e[13] = guest_u32(g_esp + 16);
    if (t_pending == 0x41E30u && argc == 5) {
        e[13] = guest_u32(g_esp + 16);
        e[14] = guest_u32(g_esp + 20);
    }
    if (t_pending == 0x42130u && argc == 5) {
        e[13] = guest_u32(g_esp + 16);
        e[14] = guest_u32(g_esp + 20);
    }
    for (uint32_t w = 0; w < t_wr_len; w++)
        if (t_wr[3 * w] == i) *(uint32_t *)((char *)g_xbox_mem_offset + t_wr[3 * w + 1]) = t_wr[3 * w + 2];
    if (i < t_script_len) {
        g_eax = t_script[3 * i]; g_ecx = t_script[3 * i + 1]; g_edx = t_script[3 * i + 2];
    } else {
        g_eax = 0; g_ecx = 0; g_edx = 0;
    }
    t1900_callee_hook(t_pending, i, 1);
    g_esp += 4; /* RET pops the return address, the cdecl caller pops the arguments */
}

typedef void (*guest_fn)(void);
guest_fn recomp_lookup(uint32_t va) { t_pending = va; return callee_stub; }

/* r = {eax, ecx, edx, ebx, esi, edi, ebp, esp} in and out. */
int shim_run(uint32_t va, uint32_t r[8])
{
    void (*fn)(void) = 0;
    switch (va) {
#define CASE2(va) case 0x##va##u: fn = sub_##va; break;
    ROOTS(CASE2)
    default: return 0;
    }
    fn = pick_reference(va, fn);
    g_eax = r[0]; g_ecx = r[1]; g_edx = r[2]; g_ebx = r[3]; g_esi = r[4]; g_edi = r[5]; g_ebp = r[6]; g_esp = r[7];
    t_count = 0;
    t20_seed_ebp(g_ebp);
    fn();
    r[0] = g_eax; r[1] = g_ecx; r[2] = g_edx; r[3] = g_ebx; r[4] = g_esi; r[5] = g_edi; r[6] = g_ebp; r[7] = g_esp;
    return 1;
}

/* T1835 wave 9: callee-saved registers and the direction flag, which shim_call does not carry.
 * shim_set_extra seeds {ebx, edi, ebp, df} before a call and shim_get_extra reads the values the
 * replacement left (after its own pops) so the differential can compare the restored registers. */
void shim_set_extra(const uint32_t extra[4])
{
    g_ebx = extra[0]; g_edi = extra[1]; g_ebp = extra[2]; g_df = (int)extra[3];
}

void shim_get_extra(uint32_t extra[4])
{
    extra[0] = g_ebx; extra[1] = g_edi; extra[2] = g_ebp; extra[3] = (uint32_t)g_df;
}

/* ---- T1835 wave 11: x87 model state and the host FPU as the reference ---- */

/* Seeds and reads the lifted double-backed x87 model around a shim_run/shim_call. */
void shim_x87_set(const uint64_t stack[8], uint32_t top, uint32_t control)
{
    for (int i = 0; i < 8; i++) __builtin_memcpy(&g_fp_stack[i], &stack[i], 8);
    g_fp_top = (int)top;
    g_fp_control_word = (uint16_t)control;
}

void shim_x87_get(uint64_t stack[8], uint32_t *top, uint32_t *control)
{
    for (int i = 0; i < 8; i++) __builtin_memcpy(&stack[i], &g_fp_stack[i], 8);
    *top = (uint32_t)g_fp_top;
    *control = g_fp_control_word;
}

/* Result of one hardware x87 program: status word before the pops, AH-derived flags of the sahf
 * programs, the two store results, and the remaining stack as raw 80-bit values, ST(0) first. */
typedef struct HwX87 {
    uint16_t status;
    uint8_t flags;
    uint8_t depth;
    uint32_t st32;
    uint32_t pad;
    uint64_t st64;
    uint8_t raw[8][10];
} HwX87;

/* The programs. a, b, c are float bits (single != 0) or double bits. Memory operands are `b`.
 *  0 fadd  1 fsub  2 fsubr  3 fmul  4 fdiv  5 fdivr   (fld a ; f<op> m)
 *  6 fcom  7 fcomp                                     (fld a ; fcom m)
 *  8 fcompp 9 fxch 10 fsubp 11 fdivp 12 faddp 13 fmulp 14 fsubrp 15 fdivrp  (fld a ; fld b ; op)
 * 16 fld a,b,c ; fxch st(2)   17 fld a ; fcom m ; fnstsw ax ; sahf ; lahf
 * 18 fucompp   19 fld a ; fstp m32   20 fld a ; fstp m64   21 fld a ; fld b ; fld st(1)
 * Returns 0 for an unknown program. The host control word is restored. */
#define HW_BODY(SFX, TYPE)                                                                        \
    TYPE va, vb, vc;                                                                              \
    __builtin_memcpy(&va, &a, sizeof va); __builtin_memcpy(&vb, &b, sizeof vb);                    \
    __builtin_memcpy(&vc, &c, sizeof vc);                                                         \
    switch (prog) {                                                                               \
    case 0: __asm__ volatile("fld" SFX " %0\n fadd" SFX " %1" : : "m"(va), "m"(vb)); depth = 1; break;  \
    case 1: __asm__ volatile("fld" SFX " %0\n fsub" SFX " %1" : : "m"(va), "m"(vb)); depth = 1; break;  \
    case 2: __asm__ volatile("fld" SFX " %0\n fsubr" SFX " %1" : : "m"(va), "m"(vb)); depth = 1; break; \
    case 3: __asm__ volatile("fld" SFX " %0\n fmul" SFX " %1" : : "m"(va), "m"(vb)); depth = 1; break;  \
    case 4: __asm__ volatile("fld" SFX " %0\n fdiv" SFX " %1" : : "m"(va), "m"(vb)); depth = 1; break;  \
    case 5: __asm__ volatile("fld" SFX " %0\n fdivr" SFX " %1" : : "m"(va), "m"(vb)); depth = 1; break; \
    case 6: __asm__ volatile("fld" SFX " %0\n fcom" SFX " %1" : : "m"(va), "m"(vb)); depth = 1; break;  \
    case 7: __asm__ volatile("fld" SFX " %0\n fcomp" SFX " %1" : : "m"(va), "m"(vb)); depth = 0; break; \
    case 8: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n fcompp" : : "m"(va), "m"(vb)); depth = 0; break; \
    case 9: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n fxch %%st(1)" : : "m"(va), "m"(vb)); depth = 2; break; \
    /* DE E9 fsubp st1 (st1 = st1 - st0), DE F9 fdivp, DE C1 faddp, DE C9 fmulp, DE E1 fsubrp, DE F1 fdivrp */ \
    case 10: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n .byte 0xde,0xe9" : : "m"(va), "m"(vb)); depth = 1; break; \
    case 11: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n .byte 0xde,0xf9" : : "m"(va), "m"(vb)); depth = 1; break; \
    case 12: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n .byte 0xde,0xc1" : : "m"(va), "m"(vb)); depth = 1; break; \
    case 13: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n .byte 0xde,0xc9" : : "m"(va), "m"(vb)); depth = 1; break; \
    case 14: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n .byte 0xde,0xe1" : : "m"(va), "m"(vb)); depth = 1; break; \
    case 15: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n .byte 0xde,0xf1" : : "m"(va), "m"(vb)); depth = 1; break; \
    case 16: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n fld" SFX " %2\n fxch %%st(2)" : : "m"(va), "m"(vb), "m"(vc)); depth = 3; break; \
    case 17: {                                                                                    \
        uint8_t fl;                                                                               \
        __asm__ volatile("fld" SFX " %1\n fcom" SFX " %2\n fnstsw %%ax\n sahf\n lahf\n movb %%ah, %0" \
                         : "=m"(fl) : "m"(va), "m"(vb) : "eax", "cc");                            \
        out->flags = fl;                                                                          \
        depth = 1;                                                                                \
        __asm__ volatile("fnstsw %0" : "=m"(out->status));                                        \
        have_status = 1;                                                                          \
        break; }                                                                                  \
    case 18: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n fucompp" : : "m"(va), "m"(vb)); depth = 0; break; \
    case 19: { float f; __asm__ volatile("fld" SFX " %1\n fsts %0" : "=m"(f) : "m"(va)); __builtin_memcpy(&out->st32, &f, 4); depth = 1; break; } \
    case 20: { double d; __asm__ volatile("fld" SFX " %1\n fstl %0" : "=m"(d) : "m"(va)); __builtin_memcpy(&out->st64, &d, 8); depth = 1; break; } \
    case 21: __asm__ volatile("fld" SFX " %0\n fld" SFX " %1\n fld %%st(1)" : : "m"(va), "m"(vb)); depth = 3; break; \
    default: ok = 0;                                                                              \
    }

int shim_hw_x87(int prog, uint32_t control, uint64_t a, uint64_t b, uint64_t c, int single, HwX87 *out)
{
    uint16_t saved, cw = (uint16_t)control;
    int ok = 1, depth = 0, have_status = 0;
    __builtin_memset(out, 0, sizeof *out);
    __asm__ volatile("fnstcw %0" : "=m"(saved));
    __asm__ volatile("fninit");
    __asm__ volatile("fldcw %0" : : "m"(cw));
    if (single) { HW_BODY("s", float) } else { HW_BODY("l", double) }
    if (ok) {
        if (!have_status) __asm__ volatile("fnstsw %0" : "=m"(out->status));
        out->depth = (uint8_t)depth;
        for (int i = 0; i < depth; i++) __asm__ volatile("fstpt %0" : "=m"(out->raw[i]));
    }
    __asm__ volatile("fninit");
    __asm__ volatile("fldcw %0" : : "m"(saved));
    return ok;
}
