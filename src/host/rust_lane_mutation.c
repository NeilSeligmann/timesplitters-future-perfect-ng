/* SPDX-License-Identifier: GPL-3.0-or-later
 * T1990: host side of the lane mutants (`-DTSFP_RUST_LANE_MUTATION=ON`, never in a normal build). The Rust cfg
 * `rust_lane_mutation` (crates/tsfp-ports/src/lane_inv.rs `mutate`) asks these three functions what to break:
 * TSFP_LANE_MUTATE_VA=<hex va> restricts the fault to one root, TSFP_LANE_MUTATE_MEM=1 flips bit 0 of the newest
 * non-stack byte the root wrote (otherwise EAX bit 0 on exit), and every memory flip is logged to stderr as
 * `MUTWRITE <va> <address>` so the trace regions can be checked against what the root writes. T2082 adds a fourth
 * question, tsfp_lane_mutation_state (TSFP_LANE_MUTATE_STATE=<hex cell>|1: the GameState setters of state_wire.rs flip bit 0 of that cell). */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
uint32_t tsfp_lane_mutation_target(void);
uint32_t tsfp_lane_mutation_mem(void);
void tsfp_lane_mutation_log(uint32_t va, uint32_t address);
uint32_t tsfp_lane_mutation_state(void);
uint32_t tsfp_lane_mutation_nth(void);
uint32_t tsfp_lane_mutation_every(void);
uint32_t tsfp_lane_mutation_bit(void);
uint32_t tsfp_lane_mutation_offset(void);
uint32_t tsfp_lane_mutation_arg(void);
/* T2256: TSFP_LANE_MUTATE_EVERY=<n> mutates only every n-th call of the target root (0, 1 or unset: every call). */
uint32_t tsfp_lane_mutation_every(void)
{
    static int cached = -1;
    if (cached < 0) {
        const char *text = getenv("TSFP_LANE_MUTATE_EVERY");
        cached = text != NULL ? (int)strtoul(text, NULL, 10) : 0;
    }
    return (uint32_t)cached;
}
/* T2264: TSFP_LANE_MUTATE_BIT=<0..7> picks the bit the memory mutant flips (unset: bit 0). On the top byte of a float
 * bit 7 is the sign, bit 0 the exponent LSB (a factor of 2). */
uint32_t tsfp_lane_mutation_bit(void)
{
    static int cached = -1;
    if (cached < 0) {
        const char *text = getenv("TSFP_LANE_MUTATE_BIT");
        cached = text != NULL ? (int)(strtoul(text, NULL, 10) & 7u) : 0;
    }
    return (uint32_t)cached;
}
/* T2264: TSFP_LANE_MUTATE_OFF=<hex> byte offset into the first argument's record for TSFP_LANE_MUTATE_MEM=5 (unset: 3). */
uint32_t tsfp_lane_mutation_offset(void)
{
    static int cached = -1;
    if (cached < 0) {
        const char *text = getenv("TSFP_LANE_MUTATE_OFF");
        cached = text != NULL ? (int)strtoul(text, NULL, 16) : 3;
    }
    return (uint32_t)cached;
}
/* T2271: TSFP_LANE_MUTATE_ARG=<1..4> the argument whose record TSFP_LANE_MUTATE_MEM=6 flips (unset: 3). */
uint32_t tsfp_lane_mutation_arg(void)
{
    static int cached = -1;
    if (cached < 0) {
        const char *text = getenv("TSFP_LANE_MUTATE_ARG");
        cached = text != NULL ? (int)strtoul(text, NULL, 10) : 3;
    }
    return (uint32_t)cached;
}
uint32_t tsfp_lane_mutation_target(void)
{
    const char *text = getenv("TSFP_LANE_MUTATE_VA");
    return text ? (uint32_t)strtoul(text, NULL, 16) : 0u;
}
uint32_t tsfp_lane_mutation_mem(void)
{
    const char *text = getenv("TSFP_LANE_MUTATE_MEM");
    /* T2117: 1 = newest non-stack write, 2 = newest write into the caller's frame (above the return slot), T2256: 3 = newest write,
     * T2264: 4 = newest write outside the callee frame and the first eight argument slots,
     * 5 = byte TSFP_LANE_MUTATE_OFF of the record the first argument points to, T2271: 6 = the same for argument TSFP_LANE_MUTATE_ARG */
    return text != NULL && text[0] != '\0' ? (uint32_t)strtoul(text, NULL, 10) : 0u;
}
bool kernel_guest_read_bytes(uint32_t address, void *out, size_t length);
/* T2087: the heap page a mutant writes moves between runs (docs/t2084-player-heap-regions.md), so also log object-relative
 * --dump-guest-range specs that reach the flipped byte: `MUTOWNER <va> <address> *S+OFF` where the static dword S (XBE
 * data range) points at most OWNER_SPAN below the byte, and `**S+O1+O2` with one heap hop inside the first OWNER_SPAN
 * bytes of *S. Only the first OWNER_SCANS distinct addresses are scanned (a 4 MiB scan per write would slow the run).
 * The scorer keeps the specs that reappear across runs (tools/t2087_owner_chains.py). */
enum { OWNER_SPAN = 0x2000, OWNER_MAX = 96, OWNER_SCANS = 4 };
static bool read32(uint32_t address, uint32_t *value)
{
    return kernel_guest_read_bytes(address, value, sizeof *value);
}
static bool heap_pointer(uint32_t value)
{
    return value >= 0x04000000u && value < 0xF0000000u;
}
static void log_owner_chains(uint32_t va, uint32_t address)
{
    static uint32_t page[OWNER_SPAN / 4];
    static uint32_t scanned[OWNER_SCANS];
    static unsigned scans;
    for (unsigned index = 0; index < scans; index++) {
        if (scanned[index] == address) {
            return;
        }
    }
    if (scans == OWNER_SCANS) {
        return;
    }
    scanned[scans++] = address;
    unsigned printed = 0;
    for (uint32_t slot = 0x00500000u; slot < 0x00900000u && printed < OWNER_MAX; slot += 4u) {
        uint32_t first = 0;
        if (!read32(slot, &first) || !heap_pointer(first)) {
            continue;
        }
        if (first <= address && address - first < OWNER_SPAN) {
            fprintf(stderr, "MUTOWNER %08x %08x *0x%X+0x%X\n", va, address, slot, address - first);
            printed++;
            continue;
        }
        if (!kernel_guest_read_bytes(first, page, sizeof page)) {
            continue;
        }
        for (uint32_t hop = 0; hop < OWNER_SPAN / 4 && printed < OWNER_MAX; hop++) {
            const uint32_t second = page[hop];
            if (heap_pointer(second) && second <= address && address - second < OWNER_SPAN) {
                fprintf(stderr, "MUTOWNER %08x %08x **0x%X+0x%X+0x%X\n", va, address, slot, hop * 4u, address - second);
                printed++;
            }
        }
    }
}
/* T2121 write trace: per flipped address, each flip is decided at the first XInputGetState poll after it (the poll the
 * rust_trace sampler sees): `seen` if the byte still holds the flipped value, `lost` if something rewrote it first. A flip
 * followed by another flip of the same byte before any poll is `superseded`. lost>0 with seen=0 means every flip is
 * overwritten within one poll period, so per-poll sampling cannot see it. Each decision prints
 * `MUTSURV <va> <address> flips=N seen=S lost=L superseded=U`; tools/rust_trace_write_trace.py summarises them. */
enum { SURV_MAX = 16 };
struct surv_entry { uint32_t va, address, flips, seen, lost, superseded; bool pending; uint8_t value; };
static struct surv_entry surv[SURV_MAX];
static unsigned surv_count;
static void surv_print(const struct surv_entry *entry)
{
    fprintf(stderr, "MUTSURV %08x %08x flips=%u seen=%u lost=%u superseded=%u\n", entry->va, entry->address, entry->flips,
            entry->seen, entry->lost, entry->superseded);
    fflush(stderr);
}
/* Called from xinput_source_poll_port (weak reference there, so normal builds do not link it). */
void tsfp_lane_mutation_poll(void);
void tsfp_lane_mutation_poll(void)
{
    for (unsigned index = 0; index < surv_count; index++) {
        struct surv_entry *entry = &surv[index];
        uint8_t now = 0;
        if (!entry->pending || !kernel_guest_read_bytes(entry->address, &now, 1)) {
            continue;
        }
        entry->pending = false;
        if (now == entry->value) {
            entry->seen++;
        } else {
            entry->lost++;
        }
        surv_print(entry);
    }
}
static void surv_record(uint32_t va, uint32_t address)
{
    unsigned slot = 0;
    while (slot < surv_count && surv[slot].address != address) {
        slot++;
    }
    if (slot == surv_count) {
        if (surv_count == SURV_MAX) {
            return;
        }
        surv[surv_count++] = (struct surv_entry){.va = va, .address = address};
    }
    struct surv_entry *entry = &surv[slot];
    entry->superseded += entry->pending ? 1u : 0u;
    entry->flips++;
    entry->pending = kernel_guest_read_bytes(address, &entry->value, 1);
}
void tsfp_lane_mutation_log(uint32_t va, uint32_t address)
{
    surv_record(va, address);
    fprintf(stderr, "MUTWRITE %08x %08x\n", va, address);
    log_owner_chains(va, address);
    fflush(stderr);
}
/* T2082: TSFP_LANE_MUTATE_STATE=1 makes the GameState setters of crates/tsfp-ports/src/state_wire.rs flip bit 0. */
uint32_t tsfp_lane_mutation_state(void)
{
    const char *text = getenv("TSFP_LANE_MUTATE_STATE");
    return text != NULL && text[0] != '\0' ? (uint32_t)strtoul(text, NULL, 16) : 0u; /* 1 = every cell, else the cell */
}
/* T2082: TSFP_LANE_MUTATE_NTH=<n> restricts the state mutant to the n-th matching store (0 or unset: every one). */
uint32_t tsfp_lane_mutation_nth(void)
{
    const char *text = getenv("TSFP_LANE_MUTATE_NTH");
    return text != NULL ? (uint32_t)strtoul(text, NULL, 10) : 0u;
}
/* T2135: Retire pilot (crates/tsfp-ports/src/state_wire.rs). TSFP_RETIRE_GAMESTATE=1 owner authoritative for converted
 * readers, 2 audit (readers keep the guest value, drift is logged), unset/0 off. */
uint32_t tsfp_lane_retire_mode(void)
{
    const char *text = getenv("TSFP_RETIRE_GAMESTATE");
    return text != NULL ? (uint32_t)strtoul(text, NULL, 10) : 0u;
}
/* TSFP_LANE_MUTATE_OWNED=<hex cell>|1: retire-mode setters corrupt the owned value only (the mirror stays correct). */
uint32_t tsfp_lane_mutation_owned(void)
{
    const char *text = getenv("TSFP_LANE_MUTATE_OWNED");
    return text != NULL && text[0] != '\0' ? (uint32_t)strtoul(text, NULL, 16) : 0u;
}
/* Audit drift log, first 256 events then every 4096th: RETDRIFT <cell> <guest> <owned> <count>. */
void tsfp_lane_retire_log(uint32_t cell, uint32_t guest, uint32_t owned, uint32_t count)
{
    if (count <= 256u || (count & 4095u) == 0u) {
        fprintf(stderr, "RETDRIFT %08x %08x %08x %u\n", cell, guest, owned, count);
        fflush(stderr);
    }
}
/* T2135 phase 2: audit site log, once per distinct (kind, cell, guest, owned) (first 512):
 * RETSITE <kind> <root> <prev root> <cell> <guest> <owned>. */
void tsfp_lane_retire_site(uint32_t kind, uint32_t root, uint32_t prev, uint32_t cell, uint32_t guest, uint32_t owned)
{
    static uint32_t seen[512][4];
    static uint32_t count;
    for (uint32_t i = 0; i < count; i++) {
        if (seen[i][0] == kind && seen[i][1] == cell && seen[i][2] == guest && seen[i][3] == owned) {
            return;
        }
    }
    if (count >= 512u) {
        return;
    }
    seen[count][0] = kind;
    seen[count][1] = cell;
    seen[count][2] = guest;
    seen[count][3] = owned;
    count++;
    fprintf(stderr, "RETSITE %u %x %x %08x %08x %08x\n", kind, root, prev, cell, guest, owned);
    fflush(stderr);
}

/* T2138: PlayerSet Retire pilot (crates/tsfp-ports/src/player_wire.rs). TSFP_RETIRE_PLAYER=1 owner authoritative for the
 * converted readers, 2 audit (readers keep the guest value, drift is logged), unset/0 off. */
uint32_t tsfp_lane_retire_player_mode(void)
{
    const char *text = getenv("TSFP_RETIRE_PLAYER");
    return text != NULL ? (uint32_t)strtoul(text, NULL, 10) : 0u;
}
/* Audit log, aggregated per (kind, root, prev root, offset): the first sight of a key prints
 *   RETPLAYER <kind> <root> <prev> <offset> <guest> <owned>
 * (first 4096 keys), every event is counted, and `RETPLAYERSUM <kind> <root> <prev> <offset> <count> <guest> <owned>`
 * lists every key with its event count at exit (and every 262144 events, so a killed host still leaves a summary).
 * kind 0 entry scan, 1 setter found the word out of sync, 2 converted reader found it out of sync, 3 re-base
 * (offset = pointer cell index 0 record 1 controlled 2 stats, guest = new pointer, owned = old pointer), 4 the same as
 * 0 to 2 on a pass-through word (expected), 5 first use (guest = record pointer, owned = stats pointer). */
#define RP_SLOTS 16384u
typedef struct {
    uint32_t kind, root, prev, offset, guest, owned, count;
    bool used;
} rp_entry;
static rp_entry rp_table[RP_SLOTS];
static uint32_t rp_keys, rp_events, rp_kind_events[6];
static void rp_summary(void)
{
    fprintf(stderr, "RETPLAYERTOTAL events %u keys %u kind0 %u kind1 %u kind2 %u kind3 %u kind4 %u kind5 %u\n", rp_events, rp_keys,
            rp_kind_events[0], rp_kind_events[1], rp_kind_events[2], rp_kind_events[3], rp_kind_events[4], rp_kind_events[5]);
    for (uint32_t i = 0; i < RP_SLOTS; i++) {
        const rp_entry *e = &rp_table[i];
        if (e->used) {
            fprintf(stderr, "RETPLAYERSUM %u %x %x %x %u %08x %08x\n", e->kind, e->root, e->prev, e->offset, e->count,
                    e->guest, e->owned);
        }
    }
    fflush(stderr);
}
void tsfp_lane_retire_player(uint32_t kind, uint32_t root, uint32_t prev, uint32_t offset, uint32_t guest, uint32_t owned)
{
    static bool registered;
    if (!registered) {
        registered = true;
        atexit(rp_summary);
    }
    rp_events++;
    if (kind < 6u) rp_kind_events[kind]++;
    uint32_t h = (kind * 0x9E3779B1u) ^ (root * 0x85EBCA77u) ^ (prev * 0xC2B2AE3Du) ^ (offset * 0x27D4EB2Fu);
    h ^= h >> 15;
    for (uint32_t probe = 0; probe < RP_SLOTS; probe++) {
        rp_entry *e = &rp_table[(h + probe) % RP_SLOTS];
        if (!e->used) {
            if (rp_keys >= RP_SLOTS - 1u) return;
            *e = (rp_entry){kind, root, prev, offset, guest, owned, 1u, true};
            rp_keys++;
            if (rp_keys <= 4096u) {
                fprintf(stderr, "RETPLAYER %u %x %x %x %08x %08x\n", kind, root, prev, offset, guest, owned);
                fflush(stderr);
            }
            break;
        }
        if (e->kind == kind && e->root == root && e->prev == prev && e->offset == offset) {
            e->count++;
            break;
        }
    }
    if ((rp_events & 262143u) == 0u) rp_summary();
}
