// SPDX-License-Identifier: GPL-3.0-or-later
// T2048 slice K, trimmed by T3935: the C kernel-memory model it differentially tested is retired, so this shim now only
// stubs the dispatcher and guest-memory entry points over the real process address space for the unmodified
// kernel_rtl.c / kernel_rtl_string.c (tests/kernel_rtl_cdiff.rs).
#define _GNU_SOURCE
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/uio.h>
#include <sys/sysinfo.h>
#include <sys/syscall.h>
#include <unistd.h>

#include "kernel_call.h"
#include "kernel_hle.h"

/* ---- stubs -------------------------------------------------------------------------------- */

static unsigned log_lines;
static int shim_log(const char *format, ...)
{
    (void)format;
    log_lines++;
    return 0;
}
kernel_log_fn kernel_hle_log(void) { return shim_log; }

static kernel_fn handlers[512];
bool kernel_hle_register(unsigned ordinal, kernel_fn handler)
{
    if (ordinal >= 512u) return false;
    handlers[ordinal] = handler;
    return true;
}

static uint32_t call_args[8];
bool kernel_frame_arg(const kernel_call_frame *frame, unsigned index, uint32_t *out)
{
    if (!frame || index >= 8u) return false;
    *out = call_args[index];
    return true;
}

static bool page_readable(uint64_t address)
{
    char byte;
    struct iovec local = {&byte, 1}, remote = {(void *)(uintptr_t)address, 1};
    return process_vm_readv(getpid(), &local, 1, &remote, 1, 0) == 1;
}

void *kernel_guest_at(kernel_guest_ptr addr, size_t length)
{
    if (addr == 0u) return NULL;
    if ((uint64_t)length > 0x100000000ULL - (uint64_t)addr) return NULL;
    if (length != 0u) {
        for (uint64_t page = addr & ~0xFFFULL; page < (uint64_t)addr + length; page += 0x1000) {
            if (!page_readable(page < addr ? addr : page)) return NULL;
        }
    }
    return (void *)(uintptr_t)addr;
}

const void *kernel_guest_at_ro(kernel_guest_ptr addr, size_t length) { return kernel_guest_at(addr, length); }

bool kernel_guest_read_bytes(kernel_guest_ptr addr, void *out, size_t length)
{
    void *at = kernel_guest_at(addr, length);
    if (!at) return false;
    memcpy(out, at, length);
    return true;
}

bool kernel_guest_read_u32(kernel_guest_ptr addr, uint32_t *out)
{
    return out && kernel_guest_read_bytes(addr, out, 4u);
}

static int write_budget = -1;
static bool statistics_fake;
static unsigned long statistics_total, statistics_free;
static unsigned statistics_unit;
static int statistics_failed;
static uint64_t statistics_image;

/* C-owned ABI adapter: Rust tests never duplicate Linux struct sysinfo. */
int sysinfo(struct sysinfo *info)
{
    if (!statistics_fake) return (int)syscall(SYS_sysinfo, info);
    memset(info, 0, sizeof(*info));
    info->totalram = statistics_total;
    info->freeram = statistics_free;
    info->mem_unit = statistics_unit;
    return statistics_failed ? -1 : 0;
}
uint64_t xbe_mapped_bytes(void) { return statistics_image; }
void shim_statistics_host(uint64_t total, uint64_t free_ram, unsigned unit,
                          uint64_t image, int failed, int writes)
{
    statistics_fake = unit != 0;
    statistics_total = (unsigned long)total;
    statistics_free = (unsigned long)free_ram;
    statistics_unit = unit;
    statistics_image = image;
    statistics_failed = failed;
    write_budget = writes;
}

bool kernel_guest_write_u32(kernel_guest_ptr addr, uint32_t value)
{
    if (write_budget == 0) return false;
    if (write_budget > 0) write_budget--;
    void *at = kernel_guest_at(addr, 4u);
    if (!at) return false;
    memcpy(at, &value, 4u);
    return true;
}

void kernel_guest_probe_change_begin(void) {}
void kernel_guest_probe_cache_flush(void) {}

/* ---- shim API ----------------------------------------------------------------------------- */

size_t kernel_rtl_register(void);
size_t kernel_rtl_string_register(void);

static uint32_t scratch;

uint32_t shim_init(void)
{
    if (scratch == 0u) {
        void *got = mmap(NULL, 0x4000, PROT_READ | PROT_WRITE,
                         MAP_PRIVATE | MAP_ANONYMOUS | MAP_32BIT, -1, 0);
        scratch = got == MAP_FAILED ? 0u : (uint32_t)(uintptr_t)got;
    }
    kernel_rtl_register();
    kernel_rtl_string_register();
    return scratch;
}

void shim_reset(void)
{
    log_lines = 0u;
}

uint32_t memshim_call(unsigned ordinal, const uint32_t *args, unsigned count)
{
    kernel_call_frame frame;
    memset(&frame, 0, sizeof(frame));
    memset(call_args, 0, sizeof(call_args));
    for (unsigned i = 0; i < count && i < 8u; i++) call_args[i] = args[i];
    return handlers[ordinal](&frame);
}

unsigned shim_log_lines(void) { return log_lines; }
bool kernel_guest_write_bytes(kernel_guest_ptr addr, const void *bytes, size_t length)
{
    if (write_budget == 0) return false;
    if (write_budget > 0) write_budget--;
    void *at = kernel_guest_at(addr, length);
    if (!at) return false;
    memcpy(at, bytes, length);
    return true;
}
void kernel_hle_fatal(unsigned ordinal, const char *format, ...)
{
    (void)ordinal;
    (void)format;
    abort(); /* Pure RTL tests must never reach exception ordinal 302. */
}
