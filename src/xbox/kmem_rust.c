/* SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Kernel memory ordinals on the Rust KernelMemory (crates/tsfp-kmem-capi): the Mm*, Nt*VirtualMemory and Ex pool
 * ordinal handlers read the guest stack, call the Rust op (kmem_abi.c) and write the guest out-parameters. The C
 * models (guest_mem.c, kernel_memory.c, kernel_pool.c) were retired by T3935; the guest_mem.h API itself is
 * src/xbox/kmem_abi.c. Design: docs/t-memory-replace.md section 8.5, docs/t3935-kmem-retirement-plan.md.
 */
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/sysinfo.h>
#include <time.h>

#include "guest_mem.h"
#include "kernel_call.h"
#include "kernel_hle.h"
#include "kernel_memory.h"
#include "kmem_abi.h"
#include "nt_status.h"

uint64_t tsfp_kmem_counter(uint32_t which);

/* FILE_OBJECT's Rust backend may be absent in kmem-only hosts: register the same guest heap owner on opt-in. */
typedef struct {
    uint32_t (*create)(uint32_t, uint32_t, uint32_t);
    uint32_t (*alloc)(uint32_t, uint32_t);
    bool (*valid)(uint32_t);
    bool (*free)(uint32_t, uint32_t);
    bool (*destroy)(uint32_t);
} file_object_heap_ops;
extern bool tsfp_rfio_file_object_set_native_heap(const file_object_heap_ops *) __attribute__((weak));
extern bool tsfp_rfio_file_object_native_heap_active(void) __attribute__((weak));
extern uint64_t tsfp_rfio_file_object_native_heap_allocations(void) __attribute__((weak));
extern uint32_t tsfp_kmem_audio_heap_create(uint32_t, uint32_t, uint32_t);
extern uint32_t tsfp_kmem_audio_heap_alloc(uint32_t, uint32_t);
extern bool tsfp_kmem_audio_heap_valid(uint32_t);
extern bool tsfp_kmem_heap_free(uint32_t, uint32_t);
extern bool tsfp_kmem_audio_heap_destroy(uint32_t);
static const file_object_heap_ops file_object_heap = {
    tsfp_kmem_audio_heap_create, tsfp_kmem_audio_heap_alloc, tsfp_kmem_audio_heap_valid,
    tsfp_kmem_heap_free, tsfp_kmem_audio_heap_destroy
};

#define ORD_BASE 1000u
#define ORD_ExAllocatePoolWithTag 15u
#define ORD_ExFreePool 17u
#define ORD_ExQueryPoolBlockSize 23u

typedef struct {
    unsigned ordinal;
    unsigned nargs;
    uint32_t bad_args; /* what the handler answers when its arguments cannot be read from the guest stack */
    uint64_t calls;
} ord_slot;

static ord_slot slots[] = {
    {ORD_MmAllocateContiguousMemory, 1, 0, 0},      {ORD_MmAllocateContiguousMemoryEx, 5, 0, 0},
    {ORD_MmCreateKernelStack, 2, 0, 0},             {ORD_MmDeleteKernelStack, 2, STATUS_SUCCESS, 0},
    {ORD_MmFreeContiguousMemory, 1, STATUS_SUCCESS, 0}, {ORD_MmGetPhysicalAddress, 1, 0, 0},
    {ORD_MmLockUnlockBufferPages, 3, STATUS_SUCCESS, 0}, {ORD_MmLockUnlockPhysicalPage, 2, STATUS_SUCCESS, 0},
    {ORD_MmPersistContiguousMemory, 3, STATUS_SUCCESS, 0}, {ORD_MmQueryAddressProtect, 1, 0, 0},
    {ORD_MmQueryAllocationSize, 1, 0, 0},           {ORD_MmSetAddressProtect, 3, STATUS_SUCCESS, 0},
    {ORD_NtAllocateVirtualMemory, 5, STATUS_INVALID_PARAMETER, 0},
    {ORD_NtFreeVirtualMemory, 3, STATUS_INVALID_PARAMETER, 0},
    {ORD_NtQueryVirtualMemory, 2, STATUS_INVALID_PARAMETER, 0},
    {ORD_ExAllocatePoolWithTag, 2, 0, 0},           {ORD_ExFreePool, 1, 0, 0},
    {ORD_ExQueryPoolBlockSize, 1, 0, 0},            {ORD_MmQueryStatistics, 1, STATUS_INVALID_PARAMETER, 0},
};
#define SLOT_COUNT (sizeof(slots) / sizeof(slots[0]))

static uint32_t rop(uint32_t op, const uint32_t *in, uint32_t nin, uint32_t *out, uint32_t nout)
{
    return kmem_op(op, in, nin, out, nout);
}

static bool write_pair(kernel_guest_ptr base_ptr, kernel_guest_ptr size_ptr, const uint32_t out[4])
{
    return (!out[0] || kernel_guest_write_u32(base_ptr, out[1])) && (!out[2] || kernel_guest_write_u32(size_ptr, out[3]));
}

extern uint64_t xbe_mapped_bytes(void) __attribute__((weak));

static uint32_t statistics_answer(uint32_t length, uint32_t out[8])
{
    struct sysinfo info;
    memset(&info, 0, sizeof(info));
    const int host_result = sysinfo(&info);
    uint64_t total = (uint64_t)info.totalram * info.mem_unit;
    uint64_t free_ram = (uint64_t)info.freeram * info.mem_unit;
    uint64_t image = xbe_mapped_bytes ? xbe_mapped_bytes() : 0u;
    uint32_t in[8] = {length, host_result == 0, (uint32_t)total, (uint32_t)(total >> 32),
                      (uint32_t)free_ram, (uint32_t)(free_ram >> 32), (uint32_t)image, (uint32_t)(image >> 32)};
    return rop(ORD_BASE + ORD_MmQueryStatistics, in, 8, out, 8);
}

/* Preserve the ordered successful prefix on faults and 32-bit address overflow. */
static uint32_t statistics_store(uint32_t address, const uint32_t out[8])
{
    for (unsigned i = 0; i < 8u; i++) {
        uint64_t at = (uint64_t)address + 4u + i * 4u;
        if (at > UINT32_MAX) return STATUS_ACCESS_VIOLATION;
        if (!kernel_guest_write_u32((uint32_t)at, out[i])) return STATUS_ACCESS_VIOLATION;
    }
    return STATUS_SUCCESS;
}

/* Arguments already read from the guest stack. */
static uint32_t rust_ordinal(const ord_slot *slot, const uint32_t args[5])
{
    uint32_t out[8] = {0};
    switch (slot->ordinal) {
    case ORD_MmQueryStatistics: {
        uint32_t length;
        if (!kernel_guest_read_u32(args[0], &length)) return STATUS_ACCESS_VIOLATION;
        uint32_t status = statistics_answer(length, out);
        return status == STATUS_SUCCESS ? statistics_store(args[0], out) : status;
    }
    case ORD_NtAllocateVirtualMemory: {
        uint32_t base_in = 0, size_in = 0;
        if (!kernel_guest_read_u32(args[0], &base_in) || !kernel_guest_read_u32(args[2], &size_in)) {
            return STATUS_ACCESS_VIOLATION;
        }
        uint32_t in[4] = {base_in, size_in, args[3], args[4]};
        uint32_t status = rop(ORD_BASE + slot->ordinal, in, 4, out, 4);
        if (status == STATUS_SUCCESS && !write_pair(args[0], args[2], out)) {
            if (out[0]) {
                rop(KMEM_OP_REGION_FREE, &out[1], 1, NULL, 0);
            }
            return STATUS_ACCESS_VIOLATION;
        }
        return status;
    }
    case ORD_NtFreeVirtualMemory: {
        uint32_t base = 0, size = 0;
        if (!kernel_guest_read_u32(args[0], &base) || !kernel_guest_read_u32(args[1], &size)) {
            return STATUS_ACCESS_VIOLATION;
        }
        uint32_t in[3] = {base, size, args[2]};
        uint32_t status = rop(ORD_BASE + slot->ordinal, in, 3, out, 4);
        if (status == STATUS_SUCCESS && !write_pair(args[0], args[1], out)) {
            return STATUS_ACCESS_VIOLATION;
        }
        return status;
    }
    case ORD_NtQueryVirtualMemory: {
        if (kernel_guest_at_ro(args[1], GUEST_MEMORY_BASIC_INFORMATION_SIZE) == NULL) {
            return STATUS_ACCESS_VIOLATION;
        }
        rop(ORD_BASE + slot->ordinal, &args[0], 1, out, 7);
        for (unsigned i = 0; i < 7u; i++) {
            if (!kernel_guest_write_u32((kernel_guest_ptr)(args[1] + i * 4u), out[i])) {
                return STATUS_ACCESS_VIOLATION;
            }
        }
        return STATUS_SUCCESS;
    }
    default:
        return rop(ORD_BASE + slot->ordinal, args, slot->nargs, NULL, 0);
    }
}


static uint32_t run_ordinal(unsigned idx, void *context)
{
    ord_slot *slot = &slots[idx];
    if (context == NULL) {
        kernel_hle_log()("kernel: ordinal %u called with no argument frame -- the call boundary did not supply one\n",
                         slot->ordinal);
        return slot->bad_args;
    }
    uint32_t args[5] = {0, 0, 0, 0, 0};
    for (unsigned i = 0; i < slot->nargs; i++) {
        if (!kernel_frame_arg((const kernel_call_frame *)context, i, &args[i])) {
            kernel_hle_log()("kernel: ordinal %u could not read argument %u from the guest stack\n", slot->ordinal, i);
            return slot->bad_args;
        }
    }
    kmem_lock();
    slot->calls++;
    uint32_t ret = rust_ordinal(slot, args);
    kmem_unlock();
    return ret;
}

#define THUNK(n)                                                                                                      \
    static uint32_t thunk_##n(void *context)                                                                          \
    {                                                                                                                 \
        return run_ordinal(n, context);                                                                               \
    }
THUNK(0) THUNK(1) THUNK(2) THUNK(3) THUNK(4) THUNK(5) THUNK(6) THUNK(7) THUNK(8) THUNK(9) THUNK(10) THUNK(11) THUNK(12)
THUNK(13) THUNK(14) THUNK(15) THUNK(16) THUNK(17) THUNK(18)
static const kernel_fn thunks[SLOT_COUNT] = {thunk_0,  thunk_1,  thunk_2,  thunk_3,  thunk_4,  thunk_5,
                                             thunk_6,  thunk_7,  thunk_8,  thunk_9,  thunk_10, thunk_11,
                                             thunk_12, thunk_13, thunk_14, thunk_15, thunk_16, thunk_17, thunk_18};
_Static_assert(SLOT_COUNT == 19, "one thunk per kmem ordinal slot");

/* T2209: pure RTL uses guarded guest access callbacks, independently of kmem mode.
 * Default is C. TSFP_RUST_RTL=all or comma-separated ordinal numbers selects Rust.
 * Exception 302 deliberately stays on the fatal C host boundary. */
uint32_t tsfp_rtl_call(uint32_t ordinal, const uint32_t *args, size_t count,
                       bool (*read_bytes)(uint32_t, void *, size_t),
                       bool (*write_bytes)(uint32_t, const void *, size_t));
static const unsigned rtl_ordinals[] = {260, 269, 279, 289, 301, 304, 305, 308};
static const unsigned rtl_arities[] = {3, 3, 3, 2, 1, 2, 2, 3};
static kernel_fn rtl_original[8];
static uint32_t rtl_run(unsigned index, void *context)
{
    uint32_t args[3] = {0};
    if (context == NULL) return rtl_original[index](context);
    for (unsigned i = 0; i < rtl_arities[index]; i++) {
        if (!kernel_frame_arg(context, i, &args[i])) return rtl_original[index](context);
    }
    return tsfp_rtl_call(rtl_ordinals[index], args, rtl_arities[index],
                         kernel_guest_read_bytes, kernel_guest_write_bytes);
}
#define RTL_THUNK(n) static uint32_t rtl_thunk_##n(void *context) { return rtl_run(n, context); }
RTL_THUNK(0) RTL_THUNK(1) RTL_THUNK(2) RTL_THUNK(3)
RTL_THUNK(4) RTL_THUNK(5) RTL_THUNK(6) RTL_THUNK(7)
static const kernel_fn rtl_thunks[] = {rtl_thunk_0, rtl_thunk_1, rtl_thunk_2, rtl_thunk_3,
                                      rtl_thunk_4, rtl_thunk_5, rtl_thunk_6, rtl_thunk_7};
static bool rtl_selected(unsigned ordinal)
{
    const char *selected = getenv("TSFP_RUST_RTL");
    if (selected == NULL) return false;
    if (strcmp(selected, "all") == 0) return true;
    bool matched = false;
    while (*selected != '\0') {
        char *end = NULL;
        unsigned long value = strtoul(selected, &end, 10);
        if (end == selected || (*end != '\0' && *end != ',')) return false;
        if (value == ordinal) matched = true;
        if (*end == '\0') break;
        selected = end + 1;
        if (*selected == '\0') return false;
    }
    return matched;
}


/* Only the RTL ordinals kernel_rtl.c registers are wrapped (--wrap=kernel_hle_register). */
extern bool __real_kernel_hle_register(unsigned ordinal, kernel_fn handler);
bool __wrap_kernel_hle_register(unsigned ordinal, kernel_fn handler)
{
    for (size_t i = 0; i < 8; i++) {
        if (ordinal == rtl_ordinals[i] && handler != NULL && handler != rtl_thunks[i] && rtl_selected(ordinal)) {
            rtl_original[i] = handler;
            return __real_kernel_hle_register(ordinal, rtl_thunks[i]);
        }
    }
    return __real_kernel_hle_register(ordinal, handler);
}

size_t kmem_register_ordinals(void)
{
    /* A fresh registration is a fresh kernel: the page locks belong to the guest that is gone. */
    rop(KMEM_OP_REGISTER, NULL, 0, NULL, 0);
    size_t registered = 0;
    for (size_t i = 0; i < SLOT_COUNT; i++) {
        if (kernel_hle_register(slots[i].ordinal, thunks[i])) {
            registered++;
            continue;
        }
        kernel_hle_log()("kernel: cannot register ordinal %u -- not in the ordinal table\n", slots[i].ordinal);
    }
    return registered;
}

uint64_t kmem_rust_ordinal_calls(unsigned ordinal)
{
    for (size_t i = 0; i < SLOT_COUNT; i++) {
        if (slots[i].ordinal == ordinal) return slots[i].calls;
    }
    return 0;
}

__attribute__((constructor)) static void kmem_start(void)
{
    const char *file_heap_opt_in = getenv("TSFP_EXPERIMENTAL_FILE_OBJECT_RUST_HEAP");
    if (!file_heap_opt_in || strcmp(file_heap_opt_in, "1") != 0) return;
    kmem_lock(); /* forces the Rust init in kmem_abi.c */
    kmem_unlock();
    if (tsfp_rfio_file_object_set_native_heap == NULL || tsfp_rfio_file_object_native_heap_active == NULL ||
        tsfp_rfio_file_object_native_heap_allocations == NULL) {
        fprintf(stderr, "kmem: experimental FILE_OBJECT heap requested but provider is unavailable\n");
        exit(EXIT_FAILURE);
    }
    if (!tsfp_rfio_file_object_set_native_heap(&file_object_heap) || !tsfp_rfio_file_object_native_heap_active()) {
        fprintf(stderr, "kmem: experimental FILE_OBJECT heap registration failed\n");
        exit(EXIT_FAILURE);
    }
    fprintf(stderr, "kmem: experimental FILE_OBJECT heap registered\n");
}
